// SPDX-License-Identifier: Apache-2.0
//! Module tests for `taylor` (src/taylor.rs): mean-value interval refinement.
//!
//! Layer: module tests (AGENTS.md §4) — external view of the public API only.
//! Four categories are covered: the inclusion property (dense high-precision
//! sampling), the dependency pathology and the `k` recursion semantics,
//! degenerate-input fallbacks and domain errors, and determinism / robustness.
//!
//! The inclusion checks compare against `eval_point` at a precision far above
//! the enclosure's, so the reference's own rounding error (~2^-truth_prec) is
//! negligible next to the enclosure's documented guard residual (2^-32 of an
//! ulp). The identically-zero pathology is checked against its *analytic*
//! truth (exactly 0) instead: a finite-precision `eval_point` of a cancelling
//! expression carries its own rounding noise and is not the exact truth.

use rhytido_backend::{BigFloat, Interval};
use rhytido_core::{BaseFn, Expr, Interner, NodeId, eval_interval, eval_point, eval_taylor};

fn bf(v: f64) -> BigFloat {
    BigFloat::from_f64(v)
}

fn c(it: &mut Interner, v: i64) -> NodeId {
    it.intern(Expr::Constant(BigFloat::from_i64(v)))
}

fn var(it: &mut Interner) -> NodeId {
    it.intern(Expr::Variable)
}

fn iv(lo: BigFloat, hi: BigFloat) -> Interval {
    Interval::new(lo, hi)
}

/// Exact power of two `2^n`, any sign of `n`.
fn pow2(n: i64) -> BigFloat {
    let k = BigFloat::from_i64(n.abs());
    let p = BigFloat::from_i64(2).pow(&k);
    if n >= 0 { p } else { BigFloat::one() / p }
}

/// Width of a bounded interval.
fn width(enc: &Interval) -> BigFloat {
    enc.upper().expect("bounded") - enc.lower().expect("bounded")
}

/// Bitwise equality of two values (precision-label independent, sign-of-zero
/// aware), mirroring the engine's `same_bits`.
fn same_bits(a: &BigFloat, b: &BigFloat) -> bool {
    match (a.canonical_parts(), b.canonical_parts()) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// The fallback contract is "unchanged", not merely "equivalent": NaN/empty
/// flags and both endpoints must match bitwise.
fn assert_same_enclosure(a: &Interval, b: &Interval) {
    assert_eq!(a.is_nan(), b.is_nan(), "NaN flag drifted");
    assert_eq!(a.is_empty(), b.is_empty(), "empty flag drifted");
    match (a.lower(), b.lower()) {
        (Some(x), Some(y)) => assert!(same_bits(&x, &y), "lower endpoint drifted"),
        (None, None) => {}
        _ => panic!("endpoint presence drifted"),
    }
    match (a.upper(), b.upper()) {
        (Some(x), Some(y)) => assert!(same_bits(&x, &y), "upper endpoint drifted"),
        (None, None) => {}
        _ => panic!("endpoint presence drifted"),
    }
}

/// Dense-sample inclusion property: `enc` must contain `f(x)` for 33 grid
/// points across `span`, the truth taken from `eval_point` at `truth_prec`.
/// The grid arithmetic is relabelled to `truth_prec` before each operation so
/// the samples are accurate to ~2^-truth_prec; the last sample is the upper
/// endpoint itself, avoiding accumulated drift.
fn assert_contains_samples(
    it: &Interner,
    id: NodeId,
    span: &Interval,
    enc: &Interval,
    truth_prec: usize,
) {
    assert!(enc.is_bounded(), "enclosure must be bounded: {enc:?}");
    let lo = span
        .lower()
        .expect("bounded span")
        .with_precision(truth_prec);
    let hi = span
        .upper()
        .expect("bounded span")
        .with_precision(truth_prec);
    let n = 32i64;
    let step = ((hi.clone() - lo.clone()) / BigFloat::from_i64(n).with_precision(truth_prec))
        .with_precision(truth_prec);
    for i in 0..=n {
        let xv = if i == n {
            hi.clone()
        } else {
            (lo.clone() + step.clone() * BigFloat::from_i64(i).with_precision(truth_prec))
                .with_precision(truth_prec)
        };
        let truth = eval_point(it, id, &xv, truth_prec);
        assert!(
            enc.contains(&truth),
            "sample {i}/{n}: {enc:?} misses the point value at grid index {i}"
        );
    }
}

/// `f = x*x - (x-1)*(x+1) - 1`: identically zero, but the naive interval walk
/// suffers full dependency blow-up while the mean-value form collapses it.
fn pathology(it: &mut Interner) -> NodeId {
    let x = var(it);
    let one = c(it, 1);
    let xx = it.intern(Expr::Mul(x, x));
    let xm1 = it.intern(Expr::Sub(x, one));
    let xp1 = it.intern(Expr::Add(x, one));
    let prod = it.intern(Expr::Mul(xm1, xp1));
    let diff = it.intern(Expr::Sub(xx, prod));
    it.intern(Expr::Sub(diff, one))
}

// ---- Normal: the inclusion property -----------------------------------

#[test]
fn normal_inclusion_property_across_expressions_and_intervals() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let one = c(&mut it, 1);
    let two = c(&mut it, 2);
    let three = c(&mut it, 3);
    let sq = it.intern(Expr::Pow(x, two));
    let cube = it.intern(Expr::Pow(x, three));
    let cubic = it.intern(Expr::Sub(cube, x));
    let sin_x = it.intern(Expr::Apply(BaseFn::Sin, x));
    let exp_x = it.intern(Expr::Apply(BaseFn::Exp, x));
    let sqrt_x = it.intern(Expr::Apply(BaseFn::Sqrt, x));
    let recip = it.intern(Expr::Div(one, x));
    let cases = [
        (sq, iv(-BigFloat::one(), BigFloat::one())),
        (sq, iv(bf(0.5), bf(2.0))),
        (sq, iv(bf(-3.0), bf(-0.5))),
        (cubic, iv(bf(-1.5), bf(1.5))),
        (sin_x, iv(bf(-2.0), bf(2.0))),
        (sin_x, iv(BigFloat::zero(), bf(3.0))),
        (exp_x, iv(-BigFloat::one(), BigFloat::one())),
        (sqrt_x, iv(bf(0.25), bf(4.0))),
        (recip, iv(bf(0.5), bf(2.0))),
    ];
    for (id, span) in cases {
        let enc = eval_taylor(&it, id, &span, 53, 0, None);
        assert_contains_samples(&it, id, &span, &enc, 200);
    }
}

#[test]
fn normal_dependency_pathology_is_much_tighter_and_still_contains_zero() {
    // The analytic truth is exactly 0 everywhere, so inclusion is checked
    // against 0 itself (see the module documentation for why eval_point
    // samples are not the exact truth of a cancelling expression).
    let mut it = Interner::new();
    let f = pathology(&mut it);
    let span = iv(bf(-2.0), bf(2.0));
    let naive = eval_interval(&it, f, &span, 53, None);
    let t0 = eval_taylor(&it, f, &span, 53, 0, None);
    let t1 = eval_taylor(&it, f, &span, 53, 1, None);
    assert!(naive.contains(&BigFloat::zero()), "naive: {naive:?}");
    assert!(t0.contains(&BigFloat::zero()), "k=0: {t0:?}");
    assert!(t1.contains(&BigFloat::zero()), "k=1: {t1:?}");
    // The naive walk is wide (x*x in [0,4], (x-1)(x+1) in [-9,3], so the
    // difference spans at least several units); one mean-value application
    // still carries the slope's own dependency, but the second application
    // collapses it: f'(m) = 0 exactly and f'' = 2 - 2 = 0.
    assert!(width(&naive) >= BigFloat::from_i64(8), "naive: {naive:?}");
    assert!(width(&t1) <= pow2(-40), "k=1 not tight: {t1:?}");
    assert!(
        width(&t1) * BigFloat::from_i64(1024) < width(&naive),
        "Taylor must be dramatically narrower"
    );
}

#[test]
fn normal_k_zero_applies_the_rule_at_least_once() {
    let mut it = Interner::new();
    let x = var(&mut it);
    // f = x - x: the naive walk over [-1,1] is [-2,2], but one mean-value
    // application gives f(m) = 0 and f' = 1 - 1 = 0, i.e. the point zero.
    // k = 0 is therefore NOT a naive pass-through: the rule fires once.
    let f = it.intern(Expr::Sub(x, x));
    let span = iv(-BigFloat::one(), BigFloat::one());
    let naive = eval_interval(&it, f, &span, 53, None);
    let t0 = eval_taylor(&it, f, &span, 53, 0, None);
    assert!(width(&naive) >= BigFloat::from_i64(4));
    assert!(t0.contains(&BigFloat::zero()));
    assert!(width(&t0) <= pow2(-40), "k=0 must collapse x - x: {t0:?}");
    // For an affine f the mean-value form is exact: the enclosure reproduces
    // the naive one endpoint for endpoint.
    let affine_span = iv(bf(0.6), bf(0.7));
    let naive_x = eval_interval(&it, x, &affine_span, 53, None);
    let taylor_x = eval_taylor(&it, x, &affine_span, 53, 0, None);
    assert_same_enclosure(&naive_x, &taylor_x);
}

#[test]
fn normal_k_recursions_width_monotone_on_the_pathology() {
    // Width monotonicity is asserted ONLY on this monotone-comparable sample
    // (the task sheet's conservative clause): in general two enclosures at
    // different k are both valid but need not be nested, because each is an
    // independent outward-rounded computation. Here the widths are 32, 0, 0.
    let mut it = Interner::new();
    let f = pathology(&mut it);
    let span = iv(bf(-2.0), bf(2.0));
    let w0 = width(&eval_taylor(&it, f, &span, 53, 0, None));
    let w1 = width(&eval_taylor(&it, f, &span, 53, 1, None));
    let w2 = width(&eval_taylor(&it, f, &span, 53, 2, None));
    assert!(w1 <= w0, "k=1 wider than k=0: {w1:?} vs {w0:?}");
    assert!(w2 <= w1, "k=2 wider than k=1: {w2:?} vs {w1:?}");
    for k in 0..=2u32 {
        assert!(eval_taylor(&it, f, &span, 53, k, None).contains(&BigFloat::zero()));
    }
}

#[test]
fn normal_unsimplified_derivative_singularity_falls_back_and_still_encloses() {
    // d3/dx3 x^2 contains 0 * x^-1 (the power rule applied to x^0): interval
    // evaluation of x^-1 over a zero-spanning range is NaN although the
    // product is identically zero. The refiner must fall back to a valid
    // enclosure instead of losing inclusion (simplification belongs to M4).
    let mut it = Interner::new();
    let x = var(&mut it);
    let two = c(&mut it, 2);
    let sq = it.intern(Expr::Pow(x, two));
    let span = iv(-BigFloat::one(), BigFloat::one());
    for k in [2u32, 3] {
        let enc = eval_taylor(&it, sq, &span, 53, k, None);
        assert_contains_samples(&it, sq, &span, &enc, 200);
    }
}

#[test]
fn normal_constant_expressions_ignore_the_interval() {
    let mut it = Interner::new();
    let two = c(&mut it, 2);
    let sqrt2 = it.intern(Expr::Apply(BaseFn::Sqrt, two));
    let pi = it.intern(Expr::Pi);
    let span = iv(bf(-5.0), bf(5.0));
    for id in [sqrt2, pi] {
        let enc = eval_taylor(&it, id, &span, 53, 0, None);
        assert_contains_samples(&it, id, &span, &enc, 200);
    }
    let reference = BigFloat::from_i64(2).with_precision(400).sqrt();
    assert!(eval_taylor(&it, sqrt2, &span, 53, 0, None).contains(&reference));
}

/// Hook answering `exp` with the valid but loose enclosure [0.5, 3.5], which
/// contains exp([0,1]) = [1, e]. The refiner must consult it for both f(m)
/// and the slope, so the hooked result is strictly wider than the plain one.
struct ExpWide;

impl rhytido_core::IntervalFnHook for ExpWide {
    fn apply_fn(&self, f: BaseFn, _arg: &Interval, _prec: usize) -> Option<Interval> {
        if f != BaseFn::Exp {
            return None;
        }
        Some(iv(bf(0.5), bf(3.5)))
    }
}

#[test]
fn normal_hook_is_threaded_through_the_refiner() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let exp_x = it.intern(Expr::Apply(BaseFn::Exp, x));
    let span = iv(BigFloat::zero(), BigFloat::one());
    let plain = eval_taylor(&it, exp_x, &span, 53, 0, None);
    let hooked = eval_taylor(&it, exp_x, &span, 53, 0, Some(&ExpWide));
    assert_contains_samples(&it, exp_x, &span, &hooked, 200);
    assert!(
        width(&hooked) > width(&plain),
        "the loose hook must widen the enclosure: {hooked:?} vs {plain:?}"
    );
}

// ---- Extreme: degenerate inputs fall back to the naive enclosure -------

#[test]
fn extreme_point_interval_falls_back_to_naive() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let two = c(&mut it, 2);
    let sq = it.intern(Expr::Pow(x, two));
    let sin_x = it.intern(Expr::Apply(BaseFn::Sin, x));
    let exp_x = it.intern(Expr::Apply(BaseFn::Exp, x));
    let f = pathology(&mut it);
    let point = Interval::point(bf(1.5));
    for id in [sq, sin_x, exp_x, f] {
        let naive = eval_interval(&it, id, &point, 53, None);
        let taylor = eval_taylor(&it, id, &point, 53, 0, None);
        assert_same_enclosure(&naive, &taylor);
        // The deviation xI - m is exactly zero at a point, so the naive
        // point enclosure already is the tightest mean-value result.
        let k2 = eval_taylor(&it, id, &point, 53, 2, None);
        assert_same_enclosure(&naive, &k2);
    }
}

#[test]
fn extreme_nan_empty_and_unbounded_fall_back_to_naive() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let two = c(&mut it, 2);
    let sq = it.intern(Expr::Pow(x, two));
    let sin_x = it.intern(Expr::Apply(BaseFn::Sin, x));
    let spans = [
        Interval::NAN,
        Interval::empty(),
        iv(BigFloat::neg_infinity(), BigFloat::one()),
        iv(BigFloat::one(), BigFloat::infinity()),
        iv(BigFloat::neg_infinity(), BigFloat::infinity()),
    ];
    for id in [sq, sin_x] {
        for span in &spans {
            let naive = eval_interval(&it, id, span, 53, None);
            let taylor = eval_taylor(&it, id, span, 53, 0, None);
            assert_same_enclosure(&naive, &taylor);
        }
    }
}

#[test]
fn extreme_tiny_huge_and_asymmetric_spans_keep_inclusion() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let two = c(&mut it, 2);
    let three = c(&mut it, 3);
    let sq = it.intern(Expr::Pow(x, two));
    let cube = it.intern(Expr::Pow(x, three));
    let sin_x = it.intern(Expr::Apply(BaseFn::Sin, x));
    let cases = [
        // A tiny span at the bottom of the exponent range.
        (sq, iv(BigFloat::zero(), pow2(-60))),
        // A huge midpoint with a unit width: the midpoint computation must
        // stay exact and clamped into the interval.
        (sq, iv(pow2(30), pow2(30) + BigFloat::one())),
        // A huge symmetric span with an odd power.
        (cube, iv(-pow2(30), pow2(30))),
        // Many periods of sin: the slope enclosure spans [-1, 1].
        (sin_x, iv(bf(100.0), bf(101.0))),
    ];
    for (id, span) in cases {
        let enc = eval_taylor(&it, id, &span, 53, 0, None);
        assert_contains_samples(&it, id, &span, &enc, 200);
    }
    // The tiny span's image must contain both endpoint images.
    let tiny = eval_taylor(&it, sq, &iv(BigFloat::zero(), pow2(-60)), 53, 0, None);
    assert!(tiny.contains(&BigFloat::zero()));
    assert!(tiny.contains(&pow2(-120)));
}

#[test]
fn extreme_deep_recursion_terminates_and_contains() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let sin_x = it.intern(Expr::Apply(BaseFn::Sin, x));
    let span = iv(-BigFloat::one(), BigFloat::one());
    // The derivative cycle sin -> cos -> -sin -> -cos is interned, so a deep
    // recursion reuses nodes instead of growing the arena without bound.
    let enc = eval_taylor(&it, sin_x, &span, 53, 12, None);
    assert_contains_samples(&it, sin_x, &span, &enc, 200);
    let f = pathology(&mut it);
    let deep = eval_taylor(&it, f, &span, 53, 6, None);
    assert!(deep.contains(&BigFloat::zero()));
    assert!(
        width(&deep) <= pow2(-40),
        "deep k must stay collapsed: {deep:?}"
    );
}

// ---- Error: domain conservatism and the interner invariant -------------

#[test]
fn error_out_of_domain_inputs_are_non_real() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let one = c(&mut it, 1);
    let sqrt_x = it.intern(Expr::Apply(BaseFn::Sqrt, x));
    let ln_x = it.intern(Expr::Apply(BaseFn::Ln, x));
    let asin_x = it.intern(Expr::Apply(BaseFn::Asin, x));
    let recip = it.intern(Expr::Div(one, x));
    // sqrt of a negative span: f(m) is already NaN.
    assert!(eval_taylor(&it, sqrt_x, &iv(bf(-2.0), bf(-1.0)), 53, 0, None).is_nan());
    // ln across zero: the slope 1/x hits a zero-spanning divisor.
    assert!(
        eval_taylor(
            &it,
            ln_x,
            &iv(-BigFloat::one(), BigFloat::one()),
            53,
            0,
            None
        )
        .is_nan()
    );
    // asin outside [-1, 1].
    assert!(eval_taylor(&it, asin_x, &iv(bf(2.0), bf(3.0)), 53, 0, None).is_nan());
    // 1/x across the pole.
    assert!(
        eval_taylor(
            &it,
            recip,
            &iv(-BigFloat::one(), BigFloat::one()),
            53,
            0,
            None
        )
        .is_nan()
    );
}

#[test]
#[should_panic]
fn error_foreign_node_id_panics() {
    // The documented interner invariant: a NodeId not produced by `it`
    // panics inside Interner::get. A high id from a larger arena is foreign
    // to the small one.
    let mut big = Interner::new();
    let mut foreign = c(&mut big, 0);
    for i in 1..40 {
        foreign = c(&mut big, i);
    }
    let mut small = Interner::new();
    var(&mut small);
    let span = iv(-BigFloat::one(), BigFloat::one());
    eval_taylor(&small, foreign, &span, 53, 0, None);
}

// ---- Robustness: determinism, purity, and the precision/k matrix -------

#[test]
fn robustness_deterministic_and_caller_interner_untouched() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let sin_x = it.intern(Expr::Apply(BaseFn::Sin, x));
    let path = pathology(&mut it);
    let f = it.intern(Expr::Add(path, sin_x));
    let span = iv(bf(-2.0), bf(2.0));
    let before = it.len();
    let first = eval_taylor(&it, f, &span, 53, 1, None);
    let second = eval_taylor(&it, f, &span, 53, 1, None);
    assert_same_enclosure(&first, &second);
    // Differentiation interns into a clone: the caller's arena is untouched.
    assert_eq!(it.len(), before, "eval_taylor must not mutate the interner");
    // A fresh interner holding the same tree gives the same enclosure.
    let mut other = Interner::new();
    let ox = var(&mut other);
    let osin = other.intern(Expr::Apply(BaseFn::Sin, ox));
    let opath = pathology(&mut other);
    let of = other.intern(Expr::Add(opath, osin));
    let rebuilt = eval_taylor(&other, of, &span, 53, 1, None);
    assert_same_enclosure(&first, &rebuilt);
}

#[test]
fn robustness_inclusion_matrix_across_precision_and_k() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let one = c(&mut it, 1);
    let two = c(&mut it, 2);
    let sq = it.intern(Expr::Pow(x, two));
    let sin_x = it.intern(Expr::Apply(BaseFn::Sin, x));
    let exp_x = it.intern(Expr::Apply(BaseFn::Exp, x));
    let recip = it.intern(Expr::Div(one, x));
    let cases = [
        (sq, iv(-BigFloat::one(), BigFloat::one())),
        (sin_x, iv(bf(-2.0), bf(2.0))),
        (exp_x, iv(-BigFloat::one(), BigFloat::one())),
        (recip, iv(bf(0.5), bf(2.0))),
    ];
    for (id, span) in cases {
        for prec in [8usize, 24, 53, 120] {
            for k in 0..=2u32 {
                let enc = eval_taylor(&it, id, &span, prec, k, None);
                assert_contains_samples(&it, id, &span, &enc, prec + 150);
            }
        }
    }
}

// ---- M2-E2: non-smooth functions must fall back to the naive enclosure --

/// A span straddling the rounding tie between two adjacent `p`-bit floats
/// near 1/3: `[mid - 2^-(p+3), mid + 2^-(p+3)]` with `mid` the exact midpoint.
/// Every arithmetic step is relabelled so the tie is represented exactly.
fn format_tie_span(p: usize) -> Interval {
    let w = p + 10;
    let third = (BigFloat::one() / BigFloat::from_i64(3)).with_precision(w);
    let a = third.round_to(p, rhytido_backend::Rounding::Nearest);
    let next = a.next_above(p);
    let mid = (a.with_precision(w) + next.with_precision(w)) / BigFloat::from_i64(2);
    let margin = pow2(-((p + 3) as i64));
    iv(mid.clone() - margin.clone(), mid + margin)
}

/// The nine non-smooth functions, each paired with a span that crosses one of
/// their breakpoints (zero / an integer / a half-integer / a format tie).
fn non_smooth_cases(it: &mut Interner) -> Vec<(NodeId, Interval)> {
    let x = var(it);
    let apply = |it: &mut Interner, f: BaseFn| it.intern(Expr::Apply(f, x));
    let abs_x = apply(it, BaseFn::Abs);
    let ceil_x = apply(it, BaseFn::Ceil);
    let floor_x = apply(it, BaseFn::Floor);
    let near_x = apply(it, BaseFn::NearestInt);
    let double_x = apply(it, BaseFn::Double);
    let single_x = apply(it, BaseFn::Single);
    let half_x = apply(it, BaseFn::HalfPrecision);
    let quad_x = apply(it, BaseFn::Quad);
    let de_x = apply(it, BaseFn::DoubleExtended);
    vec![
        // The abs kink at zero.
        (abs_x, iv(bf(-1.5), bf(2.5))),
        // Integer breakpoints.
        (ceil_x, iv(bf(1.2), bf(2.8))),
        (floor_x, iv(bf(1.2), bf(2.8))),
        // The half-integer tie of nearestint.
        (near_x, iv(bf(2.4), bf(2.6))),
        // Format rounding ties (exact midpoints of adjacent format floats).
        (double_x, format_tie_span(53)),
        (single_x, format_tie_span(24)),
        (half_x, format_tie_span(11)),
        (quad_x, format_tie_span(113)),
        (de_x, format_tie_span(64)),
    ]
}

#[test]
fn normal_m2e2_non_smooth_falls_back_to_naive_bitwise() {
    // The fallback contract is "the naive enclosure unchanged": for every
    // non-smooth function and every recursion count, the result must match
    // eval_interval endpoint for endpoint. A mean-value application would
    // differ (and for the step functions it would be *wrong*: ceil's a.e.
    // derivative is 0, so the midpoint form over [1.2, 2.8] would collapse to
    // the point ceil(2) = 2 and drop the image value 3).
    let mut it = Interner::new();
    for (id, span) in non_smooth_cases(&mut it) {
        let naive = eval_interval(&it, id, &span, 53, None);
        for k in 0..=2u32 {
            let taylor = eval_taylor(&it, id, &span, 53, k, None);
            assert_same_enclosure(&naive, &taylor);
        }
    }
}

#[test]
fn normal_m2e2_non_smooth_inclusion_across_breakpoints() {
    // The dedicated inclusion property: dense samples across the integer,
    // half-integer, zero and rounding-tie breakpoints must all lie inside the
    // enclosure, at several precisions and recursion counts.
    let mut it = Interner::new();
    for (id, span) in non_smooth_cases(&mut it) {
        for prec in [24usize, 53, 120] {
            for k in 0..=1u32 {
                let enc = eval_taylor(&it, id, &span, prec, k, None);
                assert_contains_samples(&it, id, &span, &enc, 250);
            }
        }
    }
}

#[test]
fn normal_m2e2_non_smooth_buried_in_composites_still_falls_back() {
    // The DAG walk must find a non-smooth function under smooth wrappers and
    // arithmetic, not only at the root.
    let mut it = Interner::new();
    let x = var(&mut it);
    let abs_x = it.intern(Expr::Apply(BaseFn::Abs, x));
    let ceil_x = it.intern(Expr::Apply(BaseFn::Ceil, x));
    let floor_x = it.intern(Expr::Apply(BaseFn::Floor, x));
    let double_x = it.intern(Expr::Apply(BaseFn::Double, x));
    let sin_abs = it.intern(Expr::Apply(BaseFn::Sin, abs_x));
    let sum = it.intern(Expr::Add(abs_x, ceil_x));
    let diff = it.intern(Expr::Sub(double_x, floor_x));
    let cases = [
        (sin_abs, iv(bf(-1.5), bf(2.5))),
        (sum, iv(bf(-1.5), bf(2.5))),
        (diff, iv(bf(1.2), bf(2.8))),
    ];
    for (id, span) in cases {
        let naive = eval_interval(&it, id, &span, 53, None);
        let taylor = eval_taylor(&it, id, &span, 53, 1, None);
        assert_same_enclosure(&naive, &taylor);
        assert_contains_samples(&it, id, &span, &taylor, 250);
    }
}

#[test]
fn normal_m2e2_new_smooth_functions_keep_the_mean_value_refinement() {
    // The four new smooth functions must NOT trigger the fallback: inclusion
    // holds, and a cancelling expression still collapses (the refinement is
    // active, not silently disabled for the whole new family).
    let mut it = Interner::new();
    let x = var(&mut it);
    let log2_x = it.intern(Expr::Apply(BaseFn::Log2, x));
    let log10_x = it.intern(Expr::Apply(BaseFn::Log10, x));
    let log1p_x = it.intern(Expr::Apply(BaseFn::Log1p, x));
    let expm1_x = it.intern(Expr::Apply(BaseFn::Expm1, x));
    let cases = [
        (log2_x, iv(bf(0.5), bf(4.0))),
        (log10_x, iv(bf(0.5), bf(4.0))),
        (log1p_x, iv(bf(-0.5), bf(2.0))),
        (expm1_x, iv(bf(-1.0), bf(1.0))),
    ];
    for (id, span) in cases {
        let enc = eval_taylor(&it, id, &span, 53, 0, None);
        assert_contains_samples(&it, id, &span, &enc, 250);
    }
    // f = expm1(x) - expm1(x) is identically zero. No derivative of expm1 is
    // a constant, so the mean-value form cannot collapse it to a point (that
    // is the polynomial pathology's special shape); it must still be strictly
    // tighter than the naive walk, which proves the refinement is active for
    // the new smooth family rather than silently falling back.
    let f = it.intern(Expr::Sub(expm1_x, expm1_x));
    let span = iv(bf(0.5), bf(1.5));
    let naive = eval_interval(&it, f, &span, 53, None);
    let t1 = eval_taylor(&it, f, &span, 53, 1, None);
    assert!(width(&naive) > BigFloat::zero(), "naive: {naive:?}");
    assert!(t1.contains(&BigFloat::zero()));
    assert!(
        width(&t1) * BigFloat::from_i64(2) <= width(&naive),
        "k=1 must tighten the dependency: {t1:?} vs {naive:?}"
    );
}

#[test]
fn extreme_m2e2_non_smooth_degenerate_inputs_stay_naive() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let ceil_x = it.intern(Expr::Apply(BaseFn::Ceil, x));
    let double_x = it.intern(Expr::Apply(BaseFn::Double, x));
    let spans = [
        Interval::point(bf(2.5)),
        Interval::NAN,
        Interval::empty(),
        iv(BigFloat::neg_infinity(), BigFloat::one()),
        iv(BigFloat::one(), BigFloat::infinity()),
    ];
    for id in [ceil_x, double_x] {
        for span in &spans {
            let naive = eval_interval(&it, id, span, 53, None);
            let taylor = eval_taylor(&it, id, span, 53, 2, None);
            assert_same_enclosure(&naive, &taylor);
        }
    }
}

#[test]
fn robustness_m2e2_non_smooth_shared_dag_is_pure_and_deterministic() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let abs_x = it.intern(Expr::Apply(BaseFn::Abs, x));
    let f = it.intern(Expr::Add(abs_x, abs_x));
    let span = iv(bf(-1.5), bf(2.5));
    let before = it.len();
    let first = eval_taylor(&it, f, &span, 53, 1, None);
    let second = eval_taylor(&it, f, &span, 53, 1, None);
    assert_same_enclosure(&first, &second);
    assert_eq!(
        it.len(),
        before,
        "the fallback must not mutate the interner"
    );
    assert_contains_samples(&it, f, &span, &first, 250);
}
