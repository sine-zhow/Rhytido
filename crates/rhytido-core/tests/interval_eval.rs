//! Module tests for `interval_eval` (src/interval_eval.rs): interval evaluation
//! of interned expressions and the `IntervalFnHook` dispatch point.
//!
//! Layer: module tests (AGENTS.md §4) — external view of the public API only.

use std::sync::atomic::{AtomicUsize, Ordering};

use rhytido_backend::{BigFloat, Interval, Rounding};
use rhytido_core::{BaseFn, Expr, Interner, IntervalFnHook, NodeId, eval_interval, eval_point};

/// Precision of the point evaluator used as the containment reference. Far
/// above every tested working precision, and far above the documented guard
/// residual of the interval layer.
const REF_PREC: usize = 600;

/// Precisions at which containment is sampled (module documentation).
const SAMPLE_PRECS: [usize; 4] = [24, 53, 165, 300];

fn point(x: &BigFloat) -> Interval {
    Interval::point(x.clone())
}

fn ev(it: &Interner, id: NodeId, x: &BigFloat, prec: usize) -> Interval {
    eval_interval(it, id, &point(x), prec, None)
}

/// Reference value of the same tree from the point evaluator at `REF_PREC`.
fn reference(it: &Interner, id: NodeId, x: &BigFloat) -> BigFloat {
    eval_point(it, id, x, REF_PREC)
}

fn c(it: &mut Interner, v: i64) -> NodeId {
    it.intern(Expr::Constant(BigFloat::from_i64(v)))
}

fn apply(it: &mut Interner, f: BaseFn, a: NodeId) -> NodeId {
    it.intern(Expr::Apply(f, a))
}

/// Every base function, listed explicitly. `fn_name` below is the exhaustive
/// `match` that fails to compile if the enum grows, so a new variant cannot be
/// skipped silently.
fn all_base_fns() -> [BaseFn; 30] {
    [
        BaseFn::Sqrt,
        BaseFn::Exp,
        BaseFn::Ln,
        BaseFn::Sin,
        BaseFn::Cos,
        BaseFn::Tan,
        BaseFn::Asin,
        BaseFn::Acos,
        BaseFn::Atan,
        BaseFn::Sinh,
        BaseFn::Cosh,
        BaseFn::Tanh,
        BaseFn::Asinh,
        BaseFn::Acosh,
        BaseFn::Atanh,
        BaseFn::Erf,
        BaseFn::Erfc,
        BaseFn::Log2,
        BaseFn::Log10,
        BaseFn::Log1p,
        BaseFn::Expm1,
        BaseFn::Abs,
        BaseFn::Ceil,
        BaseFn::Floor,
        BaseFn::NearestInt,
        BaseFn::Double,
        BaseFn::Single,
        BaseFn::HalfPrecision,
        BaseFn::Quad,
        BaseFn::DoubleExtended,
    ]
}

fn fn_name(f: BaseFn) -> &'static str {
    match f {
        BaseFn::Sqrt => "sqrt",
        BaseFn::Exp => "exp",
        BaseFn::Ln => "ln",
        BaseFn::Sin => "sin",
        BaseFn::Cos => "cos",
        BaseFn::Tan => "tan",
        BaseFn::Asin => "asin",
        BaseFn::Acos => "acos",
        BaseFn::Atan => "atan",
        BaseFn::Sinh => "sinh",
        BaseFn::Cosh => "cosh",
        BaseFn::Tanh => "tanh",
        BaseFn::Asinh => "asinh",
        BaseFn::Acosh => "acosh",
        BaseFn::Atanh => "atanh",
        BaseFn::Erf => "erf",
        BaseFn::Erfc => "erfc",
        BaseFn::Log2 => "log2",
        BaseFn::Log10 => "log10",
        BaseFn::Log1p => "log1p",
        BaseFn::Expm1 => "expm1",
        BaseFn::Abs => "abs",
        BaseFn::Ceil => "ceil",
        BaseFn::Floor => "floor",
        BaseFn::NearestInt => "nearestint",
        BaseFn::Double => "double",
        BaseFn::Single => "single",
        BaseFn::HalfPrecision => "halfprecision",
        BaseFn::Quad => "quad",
        BaseFn::DoubleExtended => "doubleextended",
    }
}

/// One in-domain, finite argument per base function, so the containment
/// assertions test dispatch rather than domain handling.
fn sample_arg(f: BaseFn) -> BigFloat {
    match f {
        BaseFn::Sqrt | BaseFn::Ln | BaseFn::Acosh => BigFloat::from_i64(2),
        BaseFn::Asin | BaseFn::Acos | BaseFn::Atanh => BigFloat::from_f64(0.5),
        BaseFn::Log2 | BaseFn::Log10 => BigFloat::from_i64(3),
        BaseFn::Log1p => BigFloat::from_f64(0.5),
        BaseFn::Ceil | BaseFn::Floor | BaseFn::NearestInt => BigFloat::from_f64(1.5),
        BaseFn::Double
        | BaseFn::Single
        | BaseFn::HalfPrecision
        | BaseFn::Quad
        | BaseFn::DoubleExtended => BigFloat::one() / BigFloat::from_i64(3),
        _ => BigFloat::one(),
    }
}

/// Enclosure width; only for bounded ranges. The subtraction is relabelled
/// two bits above the operands so the sum's carry bit cannot round the width
/// itself (a rounded width can invert a genuine monotonicity comparison).
fn width(enc: &Interval) -> BigFloat {
    let hi = enc.upper().expect("bounded enclosure");
    let lo = enc.lower().expect("bounded enclosure");
    let p = hi.precision().max(lo.precision()).saturating_add(2);
    hi.with_precision(p) - lo.with_precision(p)
}

fn endpoints_eq(enc: &Interval, lo: &BigFloat, hi: &BigFloat) -> bool {
    enc.lower().as_ref() == Some(lo) && enc.upper().as_ref() == Some(hi)
}

// ---- Normal cases -----------------------------------------------------

#[test]
fn normal_point_interval_matches_eval_point() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let one = c(&mut it, 1);
    let two = c(&mut it, 2);
    let sqrt_x = apply(&mut it, BaseFn::Sqrt, x);
    let exp_x = apply(&mut it, BaseFn::Exp, x);
    let ln_x = apply(&mut it, BaseFn::Ln, x);
    let sin_x = apply(&mut it, BaseFn::Sin, x);
    let erf_x = apply(&mut it, BaseFn::Erf, x);
    let erfc_x = apply(&mut it, BaseFn::Erfc, x);
    let arg = BigFloat::one();
    let exprs = [
        it.intern(Expr::Add(x, one)),
        it.intern(Expr::Sub(x, one)),
        it.intern(Expr::Mul(x, two)),
        it.intern(Expr::Div(two, x)),
        it.intern(Expr::Neg(x)),
        it.intern(Expr::Pow(x, two)),
        sqrt_x,
        exp_x,
        ln_x,
        sin_x,
        erf_x,
        erfc_x,
    ];
    for &id in &exprs {
        let enc = ev(&it, id, &arg, 53);
        let near = eval_point(&it, id, &arg, 53);
        assert!(
            !enc.is_nan() && !enc.is_empty(),
            "{:?} produced a non-range",
            it.get(id)
        );
        assert!(
            enc.contains(&near),
            "enclosure {enc} misses the point evaluation {near}"
        );
    }
}

#[test]
fn normal_exact_dyadic_trees_give_point_enclosures() {
    let mut it = Interner::new();
    let two = c(&mut it, 2);
    let three = c(&mut it, 3);
    let four = c(&mut it, 4);
    let sqrt_four = apply(&mut it, BaseFn::Sqrt, four);
    let cases = [
        (it.intern(Expr::Add(two, three)), BigFloat::from_i64(5)),
        (it.intern(Expr::Sub(three, two)), BigFloat::from_i64(1)),
        (it.intern(Expr::Mul(two, three)), BigFloat::from_i64(6)),
        (it.intern(Expr::Div(two, two)), BigFloat::from_i64(1)),
        (it.intern(Expr::Neg(two)), BigFloat::from_i64(-2)),
        (sqrt_four, BigFloat::from_i64(2)),
    ];
    for (id, want) in cases {
        let enc = ev(&it, id, &BigFloat::zero(), 53);
        assert!(enc.is_point(), "{enc} should be the point {want}");
        assert_eq!(enc.lower().expect("point"), want);
    }
    // A strictly positive base goes through exp(y*ln x) (task C2 contract), so
    // 2^3 is enclosed rather than returned as an exact point.
    let pow = it.intern(Expr::Pow(two, three));
    let enc = ev(&it, pow, &BigFloat::zero(), 53);
    assert!(enc.contains(&BigFloat::from_i64(8)));
    assert!(
        !enc.is_point(),
        "2^3 via exp/ln must stay an enclosure: {enc}"
    );
}

#[test]
fn normal_containment_sampling_under_precision_widening() {
    let mut it = Interner::new();
    for f in all_base_fns() {
        let arg = sample_arg(f);
        let a = it.intern(Expr::Constant(arg.clone()));
        let id = it.intern(Expr::Apply(f, a));
        let reference = reference(&it, id, &BigFloat::zero());
        assert!(
            reference.is_finite(),
            "{} reference is not finite",
            fn_name(f)
        );
        let mut previous = None::<BigFloat>;
        for prec in SAMPLE_PRECS {
            let enc = ev(&it, id, &BigFloat::zero(), prec);
            assert!(
                enc.contains(&reference),
                "{}({}) at prec {prec}: enclosure {enc} misses {reference}",
                fn_name(f),
                arg
            );
            // Widening the working precision must not loosen the enclosure.
            let w = width(&enc);
            if let Some(p) = &previous {
                assert!(w <= *p, "{}: width grew at prec {prec}", fn_name(f));
            }
            previous = Some(w);
        }
    }
}

#[test]
fn normal_atoms_constant_pi_variable() {
    let mut it = Interner::new();
    let five = c(&mut it, 5);
    let enc = ev(&it, five, &BigFloat::zero(), 53);
    let want = BigFloat::from_i64(5);
    assert!(enc.is_point() && endpoints_eq(&enc, &want, &want), "{enc}");

    let x = it.intern(Expr::Variable);
    let lo = BigFloat::from_i64(-3);
    let hi = BigFloat::from_i64(7);
    let wide = Interval::new(lo.clone(), hi.clone());
    let enc = eval_interval(&it, x, &wide, 53, None);
    assert!(
        endpoints_eq(&enc, &lo, &hi),
        "Variable must return x: {enc}"
    );

    let pi = it.intern(Expr::Pi);
    let reference = BigFloat::pi_with_precision(REF_PREC);
    let mut previous = None::<BigFloat>;
    for prec in SAMPLE_PRECS {
        let enc = ev(&it, pi, &BigFloat::zero(), prec);
        assert!(enc.contains(&reference), "pi at prec {prec}: {enc}");
        assert!(!enc.is_point(), "pi at prec {prec} must not be a point");
        let w = width(&enc);
        if let Some(p) = &previous {
            assert!(w <= *p, "pi bounds did not tighten at prec {prec}");
        }
        previous = Some(w);
    }
}

#[test]
fn normal_erf_erfc_dispatch_matches_point_evaluation() {
    // A wrong dispatch (e.g. erf routed to tan) misses the reference value.
    // The 15-significant-digit public constants are the ones task B pinned
    // down; the exact double values are asserted in tests/faithful.rs.
    let mut it = Interner::new();
    let one = c(&mut it, 1);
    let erf_one = apply(&mut it, BaseFn::Erf, one);
    let erfc_one = apply(&mut it, BaseFn::Erfc, one);
    let tol = BigFloat::from_f64(1e-15);
    for (id, constant) in [(erf_one, 0.842700792949715), (erfc_one, 0.157299207050285)] {
        let enc = ev(&it, id, &BigFloat::zero(), 53);
        let reference = reference(&it, id, &BigFloat::zero());
        assert!(
            enc.contains(&reference),
            "{enc} misses the reference {reference}"
        );
        assert!(enc.contains(&reference.round_to(53, Rounding::Nearest)));
        assert!(enc.contains(&eval_point(&it, id, &BigFloat::zero(), 53)));
        let drift = reference - BigFloat::from_f64(constant);
        assert!(
            drift.abs() < tol,
            "{enc}: reference misses the public constant"
        );
    }
    // erf and erfc are complements: the two enclosures together bracket 1.
    let e = ev(&it, erf_one, &BigFloat::zero(), 53);
    let c1 = ev(&it, erfc_one, &BigFloat::zero(), 53);
    let sum_lo = e.lower().expect("bounded") + c1.lower().expect("bounded");
    let sum_hi = e.upper().expect("bounded") + c1.upper().expect("bounded");
    let unit = BigFloat::one();
    assert!(
        sum_lo <= unit.clone() && unit <= sum_hi,
        "erf+erfc bracket {sum_lo}..{sum_hi} misses 1"
    );
}

/// Hook that replaces `sqrt` by an explicitly wider but still valid enclosure.
struct WiderSqrt;

impl IntervalFnHook for WiderSqrt {
    fn apply_fn(&self, f: BaseFn, arg: &Interval, _prec: usize) -> Option<Interval> {
        if f != BaseFn::Sqrt || !arg.contains(&BigFloat::from_i64(2)) {
            return None;
        }
        // Valid enclosure of sqrt(2): 1.4 <= sqrt(2) <= 1.5.
        Some(Interval::new(
            BigFloat::from_f64(1.4),
            BigFloat::from_f64(1.5),
        ))
    }
}

/// Hook that records every consultation and never overrides the backend.
struct Witness {
    calls: AtomicUsize,
    min_prec: AtomicUsize,
}

impl Witness {
    fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            min_prec: AtomicUsize::new(usize::MAX),
        }
    }
}

impl IntervalFnHook for Witness {
    fn apply_fn(&self, _f: BaseFn, _arg: &Interval, prec: usize) -> Option<Interval> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.min_prec.fetch_min(prec, Ordering::SeqCst);
        None
    }
}

#[test]
fn normal_low_precision_input_label_still_gets_the_guard_width() {
    // Regression: the kernels take their rounding width from their operands'
    // precision labels, so an input interval labelled at `prec` (exactly what
    // the faithful engine's own input interval is) used to cap the whole DAG at
    // `prec`. The inexact kernel result was then already representable at
    // `prec`, so the result collapsed to a *point* that did not contain the
    // true value — which the engine reads as an exactness conclusion. The layer
    // re-encloses the input at the guard precision to keep the model sound.
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let sqrt_x = apply(&mut it, BaseFn::Sqrt, x);
    let arg = BigFloat::from_f64(1.75);
    let down = arg.round_to(34, Rounding::Down);
    let up = arg.round_to(34, Rounding::Up);
    let input = Interval::new(down, up);
    let enc = eval_interval(&it, sqrt_x, &input, 34, None);
    let reference = eval_point(&it, sqrt_x, &arg, 400);
    assert!(
        !enc.is_point(),
        "a 34-bit input label must not eat the guard width: {enc}"
    );
    assert!(enc.contains(&reference), "{enc} misses {reference}");
    // Same for a wide input interval and for a transcendental with an even
    // coarser label.
    let exp_x = apply(&mut it, BaseFn::Exp, x);
    let loose = Interval::new(
        arg.round_to(8, Rounding::Down),
        arg.round_to(8, Rounding::Up),
    );
    let enc = eval_interval(&it, exp_x, &loose, 53, None);
    assert!(
        enc.contains(&eval_point(&it, exp_x, &arg, 400)),
        "{enc} misses the reference for an 8-bit input label"
    );
}

#[test]
fn normal_hook_short_circuits_backend_dispatch() {
    let mut it = Interner::new();
    let two = c(&mut it, 2);
    let sqrt2 = it.intern(Expr::Apply(BaseFn::Sqrt, two));
    let x = BigFloat::zero();
    let naive = ev(&it, sqrt2, &x, 53);
    let hooked = eval_interval(&it, sqrt2, &point(&x), 53, Some(&WiderSqrt));
    let low = BigFloat::from_f64(1.4);
    let high = BigFloat::from_f64(1.5);
    let true_double = BigFloat::from_f64(std::f64::consts::SQRT_2);
    // The hook's range is adopted as given (only rounded outward to `prec`),
    // which is far looser than the backend kernel's enclosure.
    assert!(hooked.lower().expect("hooked") <= low);
    assert!(hooked.upper().expect("hooked") >= high);
    assert!(width(&hooked) > width(&naive), "hook range was not adopted");
    assert!(naive.contains(&true_double));
    assert!(hooked.contains(&true_double));
    // Other functions are untouched by the hook.
    let exp1 = apply(&mut it, BaseFn::Exp, two);
    let hooked_exp = eval_interval(&it, exp1, &point(&x), 53, Some(&WiderSqrt));
    let plain_exp = ev(&it, exp1, &x, 53);
    assert!(endpoints_eq(
        &hooked_exp,
        &plain_exp.lower().expect("b"),
        &plain_exp.upper().expect("b")
    ));
}

#[test]
fn normal_hook_sees_internal_precision_and_falls_through() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let sqrt_x = apply(&mut it, BaseFn::Sqrt, x);
    let exp_x = apply(&mut it, BaseFn::Exp, x);
    let expr = it.intern(Expr::Mul(sqrt_x, exp_x));
    let witness = Witness::new();
    let arg = BigFloat::one();
    let hooked = eval_interval(&it, expr, &point(&arg), 53, Some(&witness));
    let plain = ev(&it, expr, &arg, 53);
    assert_eq!(hooked.lower(), plain.lower());
    assert_eq!(hooked.upper(), plain.upper());
    // Two distinct function nodes, each consulted once for the one call.
    assert_eq!(witness.calls.load(Ordering::SeqCst), 2);
    // The hook is passed the internal working precision, never the bare target.
    assert!(witness.min_prec.load(Ordering::SeqCst) > 53);
}

#[test]
fn normal_memoized_shared_subtree_hits_hook_once() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let sqrt_x = apply(&mut it, BaseFn::Sqrt, x);
    let same = apply(&mut it, BaseFn::Sqrt, x);
    assert_eq!(sqrt_x, same, "interner must share the identical subtree");
    let squared = it.intern(Expr::Mul(sqrt_x, sqrt_x));
    let witness = Witness::new();
    let arg = BigFloat::from_i64(4);
    let enc = eval_interval(&it, squared, &point(&arg), 53, Some(&witness));
    // One node, one call: memoization over the interned DAG (module docs).
    assert_eq!(witness.calls.load(Ordering::SeqCst), 1);
    assert!(enc.contains(&BigFloat::from_i64(4)));
}

// ---- Extreme cases ----------------------------------------------------

#[test]
fn extreme_even_power_over_interval_spanning_zero_contains_zero() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let two = c(&mut it, 2);
    let sq = it.intern(Expr::Pow(x, two));
    let span = Interval::new(BigFloat::from_i64(-1), BigFloat::from_i64(1));
    let enc = eval_interval(&it, sq, &span, 53, None);
    assert!(enc.contains(&BigFloat::zero()), "x^2 over [-1,1]: {enc}");
    assert!(enc.lower().expect("bounded") >= BigFloat::zero());
    assert!(enc.upper().expect("bounded") >= BigFloat::one());
    // Odd exponent is increasing: the endpoint image is the range.
    let three = c(&mut it, 3);
    let cube = it.intern(Expr::Pow(x, three));
    let enc = eval_interval(&it, cube, &span, 53, None);
    assert!(
        enc.contains(&BigFloat::from_i64(-8)) || enc.lower().expect("b") <= BigFloat::from_i64(-1)
    );
    assert!(enc.contains(&BigFloat::from_i64(-1)));
    assert!(enc.contains(&BigFloat::one()));
}

#[test]
fn extreme_pow_negative_base_integer_exponent_parity() {
    let mut it = Interner::new();
    let minus_two = c(&mut it, -2);
    let minus_eight = c(&mut it, -8);
    let two = c(&mut it, 2);
    let three = c(&mut it, 3);
    let zero = c(&mut it, 0);

    let odd = it.intern(Expr::Pow(minus_two, three));
    let enc = ev(&it, odd, &BigFloat::zero(), 53);
    assert!(enc.is_point(), "(-2)^3 should be the point -8, got {enc}");
    assert_eq!(enc.lower().expect("point"), BigFloat::from_i64(-8));

    let even = it.intern(Expr::Pow(minus_two, two));
    let enc = ev(&it, even, &BigFloat::zero(), 53);
    assert_eq!(enc.lower().expect("point"), BigFloat::from_i64(4));

    let power_zero = it.intern(Expr::Pow(minus_two, zero));
    assert!(ev(&it, power_zero, &BigFloat::zero(), 53).is_point());

    // Non-integer exponent on a non-positive base: conservatively non-real.
    let third = it.intern(Expr::Constant(BigFloat::one() / BigFloat::from_i64(3)));
    let cbrt = it.intern(Expr::Pow(minus_eight, third));
    assert!(ev(&it, cbrt, &BigFloat::zero(), 53).is_nan());

    // Negative integer exponent with a base interval containing zero: a pole.
    let x = it.intern(Expr::Variable);
    let minus_one = c(&mut it, -1);
    let recip = it.intern(Expr::Pow(x, minus_one));
    let span = Interval::new(BigFloat::zero(), BigFloat::one());
    assert!(eval_interval(&it, recip, &span, 53, None).is_nan());
}

#[test]
fn extreme_tan_pole_is_not_a_bounded_real() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let tan = apply(&mut it, BaseFn::Tan, x);
    // [1, 2] provably contains pi/2, so the image is not a bounded real.
    let span = Interval::new(BigFloat::from_f64(1.0), BigFloat::from_f64(2.0));
    let enc = eval_interval(&it, tan, &span, 53, None);
    let bounded = enc
        .lower()
        .zip(enc.upper())
        .is_some_and(|(lo, hi)| lo.is_finite() && hi.is_finite());
    assert!(
        !bounded || enc.contains(&BigFloat::from_i64(125_500)),
        "tan across pi/2 gave the bounded range {enc}"
    );
}

#[test]
fn extreme_unbounded_input_interval_images() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let whole = Interval::new(BigFloat::neg_infinity(), BigFloat::infinity());
    let sin = apply(&mut it, BaseFn::Sin, x);
    let enc = eval_interval(&it, sin, &whole, 53, None);
    assert!(enc.contains(&BigFloat::one()) && enc.contains(&(-BigFloat::one())));
    let erf = apply(&mut it, BaseFn::Erf, x);
    let enc = eval_interval(&it, erf, &whole, 53, None);
    assert!(enc.contains(&BigFloat::one()) && enc.contains(&(-BigFloat::one())));
    let exp = apply(&mut it, BaseFn::Exp, x);
    let enc = eval_interval(&it, exp, &whole, 53, None);
    assert!(!enc.upper().expect("range").is_finite(), "{enc}");
}

#[test]
fn extreme_nan_and_empty_input_propagate() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let two = c(&mut it, 2);
    let expr = it.intern(Expr::Mul(x, two));
    assert!(eval_interval(&it, x, &Interval::NAN, 53, None).is_nan());
    assert!(eval_interval(&it, expr, &Interval::NAN, 53, None).is_nan());
    assert!(eval_interval(&it, x, &Interval::empty(), 53, None).is_empty());
    assert!(eval_interval(&it, expr, &Interval::empty(), 53, None).is_empty());
    // An inverted range is empty and stays empty through a function node too.
    let sqrt = apply(&mut it, BaseFn::Sqrt, x);
    let inverted = Interval::new(BigFloat::from_i64(3), BigFloat::from_i64(1));
    assert!(eval_interval(&it, sqrt, &inverted, 53, None).is_empty());
}

#[test]
fn extreme_division_by_interval_containing_zero() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let one = c(&mut it, 1);
    let recip = it.intern(Expr::Div(one, x));
    let span = Interval::new(BigFloat::from_i64(-1), BigFloat::from_i64(1));
    assert!(eval_interval(&it, recip, &span, 53, None).is_nan());
    // Away from zero the quotient is a bounded real containing 1/x.
    let positive = Interval::new(BigFloat::from_i64(2), BigFloat::from_i64(4));
    let enc = eval_interval(&it, recip, &positive, 53, None);
    assert!(enc.contains(&BigFloat::from_f64(0.25)));
    assert!(enc.contains(&BigFloat::from_f64(0.5)));
}

#[test]
fn extreme_minimum_precision_still_encloses_the_reference() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let sqrt_x = apply(&mut it, BaseFn::Sqrt, x);
    let exp_x = apply(&mut it, BaseFn::Exp, x);
    let erf_x = apply(&mut it, BaseFn::Erf, x);
    let cases = [sqrt_x, exp_x, erf_x, it.intern(Expr::Pi)];
    let arg = BigFloat::one();
    for id in cases {
        let enc = ev(&it, id, &arg, 2);
        assert!(
            enc.contains(&reference(&it, id, &arg)),
            "prec 2 enclosure {enc} misses the reference"
        );
    }
}

// ---- Error cases (异常报错) -------------------------------------------

#[test]
fn error_out_of_domain_functions_report_nan() {
    let mut it = Interner::new();
    let one = c(&mut it, 1);
    let two = c(&mut it, 2);
    let half = it.intern(Expr::Div(one, two));
    let minus_one = c(&mut it, -1);
    let minus_two = c(&mut it, -2);
    let sqrt_neg = apply(&mut it, BaseFn::Sqrt, minus_one);
    let ln_neg = apply(&mut it, BaseFn::Ln, minus_two);
    let asin_two = apply(&mut it, BaseFn::Asin, two);
    let acos_two = apply(&mut it, BaseFn::Acos, two);
    let atanh_two = apply(&mut it, BaseFn::Atanh, two);
    let acosh_half = apply(&mut it, BaseFn::Acosh, half);
    for id in [sqrt_neg, ln_neg, asin_two, acos_two, atanh_two, acosh_half] {
        let enc = ev(&it, id, &BigFloat::zero(), 53);
        assert!(
            enc.is_nan(),
            "{:?} outside its domain gave {enc}",
            it.get(id)
        );
    }
}

#[test]
fn error_nan_operand_propagates_through_arithmetic() {
    let mut it = Interner::new();
    let nan = it.intern(Expr::Constant(BigFloat::NAN));
    let two = c(&mut it, 2);
    let add = it.intern(Expr::Add(nan, two));
    let mul = it.intern(Expr::Mul(two, nan));
    let div = it.intern(Expr::Div(two, nan));
    let pow = it.intern(Expr::Pow(nan, two));
    let sin_nan = apply(&mut it, BaseFn::Sin, nan);
    let sub = it.intern(Expr::Sub(nan, nan));
    let neg = it.intern(Expr::Neg(nan));
    let cases = [add, mul, div, pow, sin_nan, sub, neg];
    for id in cases {
        assert!(
            ev(&it, id, &BigFloat::zero(), 53).is_nan(),
            "{:?} did not propagate NaN",
            it.get(id)
        );
    }
}

#[test]
fn error_ln_of_zero_gives_an_unbounded_enclosure() {
    let mut it = Interner::new();
    let zero = c(&mut it, 0);
    let ln0 = apply(&mut it, BaseFn::Ln, zero);
    let enc = ev(&it, ln0, &BigFloat::zero(), 53);
    assert!(!enc.is_nan(), "ln(0) is -infinity, not NaN: {enc}");
    assert!(
        !enc.lower().expect("range").is_finite(),
        "ln(0) enclosure {enc} is not unbounded below"
    );
}

/// Hook that returns an enclosure the backend would never produce.
struct Wild;

impl IntervalFnHook for Wild {
    fn apply_fn(&self, _f: BaseFn, _arg: &Interval, _prec: usize) -> Option<Interval> {
        Some(Interval::point(BigFloat::from_i64(7)))
    }
}

#[test]
fn error_hook_range_is_trusted_as_given() {
    // eval_interval does not re-verify a hook's enclosure; it is the caller's
    // own enclosure (module documentation). A caller that returns an invalid
    // range therefore gets that range back, widened only to the target grid.
    let mut it = Interner::new();
    let two = c(&mut it, 2);
    let sqrt2 = it.intern(Expr::Apply(BaseFn::Sqrt, two));
    let enc = eval_interval(&it, sqrt2, &point(&BigFloat::zero()), 53, Some(&Wild));
    assert_eq!(enc.lower().expect("point"), BigFloat::from_i64(7));
    assert!(!enc.contains(&BigFloat::from_f64(std::f64::consts::SQRT_2)));
}

// ---- Interruption / robustness cases (异常中断) -----------------------

#[test]
fn robustness_special_value_matrix_never_panics() {
    let mut it = Interner::new();
    let args = [
        BigFloat::zero(),
        BigFloat::one(),
        BigFloat::from_i64(-1),
        BigFloat::from_i64(2),
        BigFloat::from_f64(0.5),
        BigFloat::from_f64(-0.5),
        BigFloat::from_i64(1_000_000),
        BigFloat::infinity(),
        BigFloat::neg_infinity(),
        BigFloat::NAN,
    ];
    let x = it.intern(Expr::Variable);
    for f in all_base_fns() {
        let a = it.intern(Expr::Constant(BigFloat::one()));
        let id = it.intern(Expr::Apply(f, a));
        let fx = apply(&mut it, f, x);
        for arg in &args {
            for prec in [2usize, 3, 53, 165] {
                let _ = ev(&it, id, arg, prec);
                let _ = eval_interval(&it, fx, &point(arg), prec, None);
            }
        }
    }
}

#[test]
fn robustness_repeated_calls_are_identical() {
    // The memo cache is per call and nothing is global (AGENTS.md §3), so two
    // calls with the same arguments return the same enclosure.
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let sin_x = apply(&mut it, BaseFn::Sin, x);
    let erf_x = apply(&mut it, BaseFn::Erf, x);
    let expr = it.intern(Expr::Add(sin_x, erf_x));
    let arg = BigFloat::from_f64(0.25);
    let first = ev(&it, expr, &arg, 53);
    let second = ev(&it, expr, &arg, 53);
    assert_eq!(first.lower(), second.lower());
    assert_eq!(first.upper(), second.upper());
}

#[test]
fn robustness_deep_chain_terminates() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let mut node = x;
    for _ in 0..40 {
        node = it.intern(Expr::Apply(BaseFn::Sqrt, node));
    }
    let arg = BigFloat::from_i64(2);
    let enc = ev(&it, node, &arg, 53);
    // sqrt^n(2) = 2^(1/2^n) stays strictly above 1 and close to it.
    assert!(enc.contains(&reference(&it, node, &arg)), "{enc}");
    assert!(enc.lower().expect("bounded") > BigFloat::one());
    assert!(width(&enc) < BigFloat::from_f64(1e-6));
}

#[test]
fn robustness_positive_base_power_stays_enclosed_and_tightens() {
    let mut it = Interner::new();
    let four = c(&mut it, 4);
    let third = it.intern(Expr::Constant(BigFloat::one() / BigFloat::from_i64(3)));
    let expr = it.intern(Expr::Pow(four, third));
    let reference = reference(&it, expr, &BigFloat::zero());
    let mut previous = None::<BigFloat>;
    let mut last = None::<Interval>;
    for prec in SAMPLE_PRECS {
        let enc = ev(&it, expr, &BigFloat::zero(), prec);
        assert!(enc.contains(&reference), "4^(1/3) at {prec}: {enc}");
        let w = width(&enc);
        if let Some(p) = &previous {
            assert!(w <= *p, "width grew at prec {prec}");
        }
        previous = Some(w);
        last = Some(enc);
    }
    let enc = last.expect("at least one precision");
    assert!(enc.lower().expect("bounded") > BigFloat::from_f64(1.58));
}

// ---- M2-E2 additions: the thirteen new functions ------------------------

#[test]
fn normal_m2e2_new_smooth_functions_contain_their_images() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let cases = [
        (BaseFn::Log2, BigFloat::from_f64(0.5), BigFloat::from_i64(4)),
        (
            BaseFn::Log10,
            BigFloat::from_f64(0.5),
            BigFloat::from_i64(4),
        ),
        (
            BaseFn::Log1p,
            BigFloat::from_f64(-0.5),
            BigFloat::from_i64(2),
        ),
        (
            BaseFn::Expm1,
            BigFloat::from_f64(-1.0),
            BigFloat::from_i64(1),
        ),
    ];
    for (f, lo, hi) in cases {
        let id = apply(&mut it, f, x);
        let span = Interval::new(lo.clone(), hi.clone());
        let mid = (lo.clone() + hi.clone()) / BigFloat::from_i64(2);
        let mut previous = None::<BigFloat>;
        for prec in SAMPLE_PRECS {
            let enc = eval_interval(&it, id, &span, prec, None);
            for probe in [&lo, &hi, &mid] {
                let want = reference(&it, id, probe);
                assert!(
                    enc.contains(&want),
                    "{} over [{lo}, {hi}] at {prec}: {enc} misses {want}",
                    fn_name(f)
                );
            }
            let w = width(&enc);
            if let Some(p) = &previous {
                assert!(w <= *p, "{}: width grew at {prec}", fn_name(f));
            }
            previous = Some(w);
        }
    }
}

#[test]
fn normal_m2e2_abs_interval_has_its_minimum_at_zero() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let abs_x = apply(&mut it, BaseFn::Abs, x);
    // A zero-spanning interval: the image is [0, max(|lo|, |hi|)].
    let span = Interval::new(BigFloat::from_i64(-2), BigFloat::from_i64(3));
    let enc = eval_interval(&it, abs_x, &span, 53, None);
    assert!(enc.contains(&BigFloat::zero()), "{enc}");
    assert!(enc.contains(&BigFloat::from_i64(3)), "{enc}");
    assert!(enc.contains(&BigFloat::from_i64(2)), "{enc}");
    assert!(enc.lower().expect("bounded") >= BigFloat::zero());
    // One-sided intervals map monotonically.
    let neg = Interval::new(BigFloat::from_i64(-5), BigFloat::from_i64(-2));
    let enc = eval_interval(&it, abs_x, &neg, 53, None);
    assert!(enc.contains(&BigFloat::from_i64(2)) && enc.contains(&BigFloat::from_i64(5)));
    assert!(enc.lower().expect("bounded") >= BigFloat::from_i64(2));
}

#[test]
fn normal_m2e2_integer_rounding_images_cross_breakpoints() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let ceil_x = apply(&mut it, BaseFn::Ceil, x);
    let floor_x = apply(&mut it, BaseFn::Floor, x);
    let near_x = apply(&mut it, BaseFn::NearestInt, x);
    let span = Interval::new(BigFloat::from_f64(1.2), BigFloat::from_f64(2.8));
    // ceil [1.2, 2.8] = [2, 3]; floor = [1, 2]; nearestint = [1, 3].
    let enc = eval_interval(&it, ceil_x, &span, 53, None);
    assert!(enc.contains(&BigFloat::from_i64(2)) && enc.contains(&BigFloat::from_i64(3)));
    assert_eq!(enc.lower().expect("bounded"), BigFloat::from_i64(2));
    assert_eq!(enc.upper().expect("bounded"), BigFloat::from_i64(3));
    let enc = eval_interval(&it, floor_x, &span, 53, None);
    assert_eq!(enc.lower().expect("bounded"), BigFloat::from_i64(1));
    assert_eq!(enc.upper().expect("bounded"), BigFloat::from_i64(2));
    let enc = eval_interval(&it, near_x, &span, 53, None);
    assert_eq!(enc.lower().expect("bounded"), BigFloat::from_i64(1));
    assert_eq!(enc.upper().expect("bounded"), BigFloat::from_i64(3));
    // Crossing zero: ceil [-0.5, 0.5] = [0, 1], floor = [-1, 0].
    let zero_span = Interval::new(BigFloat::from_f64(-0.5), BigFloat::from_f64(0.5));
    let enc = eval_interval(&it, ceil_x, &zero_span, 53, None);
    assert!(enc.contains(&BigFloat::zero()) && enc.contains(&BigFloat::one()));
    let enc = eval_interval(&it, floor_x, &zero_span, 53, None);
    assert!(enc.contains(&-BigFloat::one()) && enc.contains(&BigFloat::zero()));
    // Crossing the half-integer tie: nearestint [2.4, 2.6] = [2, 3].
    let tie_span = Interval::new(BigFloat::from_f64(2.4), BigFloat::from_f64(2.6));
    let enc = eval_interval(&it, near_x, &tie_span, 53, None);
    assert_eq!(enc.lower().expect("bounded"), BigFloat::from_i64(2));
    assert_eq!(enc.upper().expect("bounded"), BigFloat::from_i64(3));
    // A point at the tie itself rounds to even: nearestint(2.5) = 2.
    let tie_point = Interval::point(BigFloat::from_f64(2.5));
    let enc = eval_interval(&it, near_x, &tie_point, 53, None);
    assert!(enc.is_point());
    assert_eq!(enc.lower().expect("point"), BigFloat::from_i64(2));
}

#[test]
fn normal_m2e2_format_intervals_use_directed_endpoint_rounding() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let formats = [
        (BaseFn::Double, rhytido_backend::Format::Double),
        (BaseFn::Single, rhytido_backend::Format::Single),
        (BaseFn::HalfPrecision, rhytido_backend::Format::Half),
        (BaseFn::Quad, rhytido_backend::Format::Quad),
        (
            BaseFn::DoubleExtended,
            rhytido_backend::Format::DoubleExtended,
        ),
    ];
    let lo = BigFloat::from_f64(0.3);
    let hi = BigFloat::from_f64(0.4);
    let span = Interval::new(lo.clone(), hi.clone());
    for (f, format) in formats {
        let id = apply(&mut it, f, x);
        let enc = eval_interval(&it, id, &span, 165, None);
        // Directed endpoint images: lower rounded down, upper rounded up.
        let want_lo = lo.round_to_format(format, Rounding::Down);
        let want_hi = hi.round_to_format(format, Rounding::Up);
        assert!(
            enc.contains(&want_lo),
            "{}: {enc} misses {want_lo}",
            fn_name(f)
        );
        assert!(
            enc.contains(&want_hi),
            "{}: {enc} misses {want_hi}",
            fn_name(f)
        );
        // Every interior point's nearest image is contained too.
        for probe in [0.32, 0.35, 0.37, 0.39] {
            let p = BigFloat::from_f64(probe);
            let image = p.round_to_format(format, Rounding::Nearest);
            assert!(enc.contains(&image), "{}: {enc} misses {image}", fn_name(f));
        }
    }
}

#[test]
fn normal_m2e2_format_intervals_cross_rounding_boundaries() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let double_x = apply(&mut it, BaseFn::Double, x);
    let single_x = apply(&mut it, BaseFn::Single, x);
    // binary64 tie: 2^53+1 sits exactly between 2^53 and 2^53+2; the image of
    // the spanning interval must contain both neighbours (and the tie's
    // ties-to-even image 2^53).
    let lo = BigFloat::from_i64(9007199254740992_i64);
    let hi = BigFloat::from_i64(9007199254740994_i64);
    let span = Interval::new(lo.clone(), hi.clone());
    let enc = eval_interval(&it, double_x, &span, 165, None);
    assert!(enc.contains(&lo), "{enc}");
    assert!(enc.contains(&hi), "{enc}");
    assert!(
        enc.contains(
            &BigFloat::from_i64(9007199254740993_i64)
                .round_to_format(rhytido_backend::Format::Double, Rounding::Nearest)
        )
    );
    // binary32 tie: 2^24+1 between 2^24 and 2^24+2.
    let lo = BigFloat::from_i64(16777216);
    let hi = BigFloat::from_i64(16777218);
    let span = Interval::new(lo.clone(), hi.clone());
    let enc = eval_interval(&it, single_x, &span, 165, None);
    assert!(enc.contains(&lo) && enc.contains(&hi), "{enc}");
    // A point input converts exactly (nearest), at every working precision.
    let third = BigFloat::one() / BigFloat::from_i64(3);
    for prec in SAMPLE_PRECS {
        let enc = eval_interval(&it, double_x, &point(&third), prec, None);
        assert!(
            enc.contains(
                &third.round_to_format(rhytido_backend::Format::Double, Rounding::Nearest)
            ),
            "double(1/3) at {prec}: {enc}"
        );
    }
}

#[test]
fn error_m2e2_new_logarithm_domains_are_conservative() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let log2_x = apply(&mut it, BaseFn::Log2, x);
    let log10_x = apply(&mut it, BaseFn::Log10, x);
    let log1p_x = apply(&mut it, BaseFn::Log1p, x);
    // A domain-crossing interval is conservatively NaN (backend map_mono).
    let crossing = Interval::new(BigFloat::from_i64(-1), BigFloat::from_i64(2));
    assert!(eval_interval(&it, log2_x, &crossing, 53, None).is_nan());
    assert!(eval_interval(&it, log10_x, &crossing, 53, None).is_nan());
    let below = Interval::new(BigFloat::from_i64(-2), BigFloat::from_f64(-0.5));
    assert!(eval_interval(&it, log1p_x, &below, 53, None).is_nan());
    // The pole itself: log2(0) / log10(0) / log1p(-1) are unbounded below.
    for (id, arg) in [
        (log2_x, BigFloat::zero()),
        (log10_x, BigFloat::zero()),
        (log1p_x, -BigFloat::one()),
    ] {
        let enc = eval_interval(&it, id, &point(&arg), 53, None);
        assert!(!enc.is_nan(), "{enc} should be unbounded, not NaN");
        assert!(!enc.lower().expect("range").is_finite(), "{enc}");
    }
}

#[test]
fn robustness_m2e2_format_special_intervals_propagate() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    for f in [
        BaseFn::Double,
        BaseFn::Single,
        BaseFn::HalfPrecision,
        BaseFn::Quad,
        BaseFn::DoubleExtended,
        BaseFn::Abs,
        BaseFn::Ceil,
        BaseFn::Floor,
        BaseFn::NearestInt,
    ] {
        let id = apply(&mut it, f, x);
        assert!(eval_interval(&it, id, &Interval::NAN, 53, None).is_nan());
        assert!(eval_interval(&it, id, &Interval::empty(), 53, None).is_empty());
    }
    // Unbounded endpoints pass through the format conversions untouched.
    let double_x = apply(&mut it, BaseFn::Double, x);
    let whole = Interval::new(BigFloat::neg_infinity(), BigFloat::infinity());
    let enc = eval_interval(&it, double_x, &whole, 53, None);
    assert!(!enc.lower().expect("range").is_finite());
    assert!(!enc.upper().expect("range").is_finite());
}
