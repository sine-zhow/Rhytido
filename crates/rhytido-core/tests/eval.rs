//! Module tests for `eval` (src/eval.rs): point evaluation of expressions.
//!
//! Layer: module tests (AGENTS.md §4) — external view of the public API only.

use rhytido_backend::BigFloat;
use rhytido_core::{BaseFn, Expr, Interner, NodeId, eval_point};

const PREC: usize = 165;

fn close(a: &BigFloat, b: &BigFloat, bits: usize) -> bool {
    a.with_precision(bits) == b.with_precision(bits)
}

fn c(it: &mut Interner, v: i64) -> NodeId {
    it.intern(Expr::Constant(BigFloat::from_i64(v)))
}

// ---- Normal cases -----------------------------------------------------

#[test]
fn normal_constant_and_variable() {
    let mut it = Interner::new();
    let five = c(&mut it, 5);
    assert_eq!(
        eval_point(&it, five, &BigFloat::zero(), PREC),
        BigFloat::from_i64(5)
    );
    let var = it.intern(Expr::Variable);
    assert_eq!(
        eval_point(&it, var, &BigFloat::from_i64(3), PREC),
        BigFloat::from_i64(3)
    );
}

#[test]
fn normal_pi() {
    let mut it = Interner::new();
    let pi = it.intern(Expr::Pi);
    let v = eval_point(&it, pi, &BigFloat::zero(), PREC);
    assert!(close(&v, &BigFloat::from_f64(std::f64::consts::PI), 50));
}

#[test]
fn normal_arithmetic() {
    let mut it = Interner::new();
    let two = c(&mut it, 2);
    let three = c(&mut it, 3);
    let add = it.intern(Expr::Add(two, three));
    assert_eq!(
        eval_point(&it, add, &BigFloat::zero(), PREC),
        BigFloat::from_i64(5)
    );
    let sub = it.intern(Expr::Sub(three, two));
    assert_eq!(
        eval_point(&it, sub, &BigFloat::zero(), PREC),
        BigFloat::from_i64(1)
    );
    let mul = it.intern(Expr::Mul(two, three));
    assert_eq!(
        eval_point(&it, mul, &BigFloat::zero(), PREC),
        BigFloat::from_i64(6)
    );
    let neg = it.intern(Expr::Neg(two));
    assert_eq!(
        eval_point(&it, neg, &BigFloat::zero(), PREC),
        BigFloat::from_i64(-2)
    );
}

#[test]
fn normal_div_and_pow() {
    let mut it = Interner::new();
    let six = c(&mut it, 6);
    let three = c(&mut it, 3);
    let div = it.intern(Expr::Div(six, three));
    assert_eq!(
        eval_point(&it, div, &BigFloat::zero(), PREC),
        BigFloat::from_i64(2)
    );
    let two = c(&mut it, 2);
    let pow = it.intern(Expr::Pow(two, three));
    assert_eq!(
        eval_point(&it, pow, &BigFloat::zero(), PREC),
        BigFloat::from_i64(8)
    );
}

#[test]
fn normal_function_application() {
    let mut it = Interner::new();
    let four = c(&mut it, 4);
    let sqrt = it.intern(Expr::Apply(BaseFn::Sqrt, four));
    assert_eq!(
        eval_point(&it, sqrt, &BigFloat::zero(), PREC),
        BigFloat::from_i64(2)
    );
    let zero = c(&mut it, 0);
    let exp0 = it.intern(Expr::Apply(BaseFn::Exp, zero));
    assert!(close(
        &eval_point(&it, exp0, &BigFloat::zero(), PREC),
        &BigFloat::one(),
        60
    ));
    let sin0 = it.intern(Expr::Apply(BaseFn::Sin, zero));
    assert!(eval_point(&it, sin0, &BigFloat::zero(), PREC).is_zero());
}

#[test]
fn normal_composite_with_variable() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let two = c(&mut it, 2);
    let x2 = it.intern(Expr::Pow(x, two));
    let one = c(&mut it, 1);
    let expr = it.intern(Expr::Add(x2, one));
    // x^2 + 1 at x=2 -> 5
    assert_eq!(
        eval_point(&it, expr, &BigFloat::from_i64(2), PREC),
        BigFloat::from_i64(5)
    );
}

// ---- Extreme cases ----------------------------------------------------

#[test]
fn extreme_shared_subexpression_memoized() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let sum = it.intern(Expr::Add(x, x)); // 2x
    let prod = it.intern(Expr::Mul(sum, sum)); // (2x)^2 = 4x^2
    // at x=3 -> 36
    assert_eq!(
        eval_point(&it, prod, &BigFloat::from_i64(3), PREC),
        BigFloat::from_i64(36)
    );
}

#[test]
fn extreme_division_by_zero() {
    let mut it = Interner::new();
    let one = c(&mut it, 1);
    let zero = c(&mut it, 0);
    let div = it.intern(Expr::Div(one, zero));
    assert!(eval_point(&it, div, &BigFloat::zero(), PREC).is_infinite());
}

#[test]
fn extreme_deep_nesting() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let mut node = x;
    for _ in 0..40 {
        node = it.intern(Expr::Apply(BaseFn::Sqrt, node));
    }
    // sqrt^40(2) is close to 1 (repeated sqrt tends to 1).
    let v = eval_point(&it, node, &BigFloat::from_i64(2), PREC);
    assert!(close(&v, &BigFloat::one(), 20));
}

// ---- Error cases (异常报错) -------------------------------------------

#[test]
fn error_domain_errors_propagate() {
    let mut it = Interner::new();
    let neg = c(&mut it, -1);
    let sqrt_neg = it.intern(Expr::Apply(BaseFn::Sqrt, neg));
    assert!(eval_point(&it, sqrt_neg, &BigFloat::zero(), PREC).is_nan());
    let two = c(&mut it, 2);
    let asin2 = it.intern(Expr::Apply(BaseFn::Asin, two));
    assert!(eval_point(&it, asin2, &BigFloat::zero(), PREC).is_nan());
    let zero = c(&mut it, 0);
    let ln0 = it.intern(Expr::Apply(BaseFn::Ln, zero));
    assert!(eval_point(&it, ln0, &BigFloat::zero(), PREC).is_infinite());
}

// ---- Interruption / robustness cases (异常中断) -----------------------

#[test]
fn robustness_special_values_never_panic() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let one = c(&mut it, 1);
    let exprs = [
        it.intern(Expr::Add(x, one)),
        it.intern(Expr::Mul(x, one)),
        it.intern(Expr::Div(one, x)),
        it.intern(Expr::Apply(BaseFn::Ln, x)),
        it.intern(Expr::Apply(BaseFn::Sqrt, x)),
        it.intern(Expr::Pow(x, one)),
    ];
    for xv in [
        BigFloat::NAN,
        BigFloat::infinity(),
        BigFloat::neg_infinity(),
        BigFloat::zero(),
    ] {
        for &e in &exprs {
            let _ = eval_point(&it, e, &xv, PREC);
        }
    }
}

#[test]
fn robustness_all_functions_evaluable() {
    let mut it = Interner::new();
    let half = it.intern(Expr::Constant(BigFloat::from_f64(0.5)));
    let fns = [
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
    ];
    for f in fns {
        let node = it.intern(Expr::Apply(f, half));
        let v = eval_point(&it, node, &BigFloat::zero(), PREC);
        // acosh(0.5) is NaN (domain); all others are finite at 0.5.
        if f == BaseFn::Acosh {
            assert!(v.is_nan());
        } else {
            assert!(
                v.is_finite(),
                "function {:?} did not yield a finite value",
                f
            );
        }
    }
}

// ---- M2-C additions: erf/erfc dispatch (appended, existing cases intact) --

#[test]
fn normal_erf_erfc_dispatch_matches_the_backend_facade() {
    // The two new BaseFn variants must route to BigFloat::erf / ::erfc, not to
    // any other kernel: the point evaluator's answer is compared with the
    // facade call on the same operand, and with the public constants at 15
    // significant digits (the same cross-check budget as the backend's
    // tests/erf.rs, which owns the numerical proof).
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let erf_x = it.intern(Expr::Apply(BaseFn::Erf, x));
    let erfc_x = it.intern(Expr::Apply(BaseFn::Erfc, x));
    let tol = BigFloat::from_f64(1e-15);
    for arg in [
        BigFloat::from_f64(0.5),
        BigFloat::one(),
        BigFloat::from_f64(2.0),
        BigFloat::from_f64(-1.0),
    ] {
        let direct_erf = arg.erf();
        let direct_erfc = arg.erfc();
        assert_eq!(
            eval_point(&it, erf_x, &arg, PREC),
            direct_erf,
            "erf dispatch differs from the facade for {arg}"
        );
        assert_eq!(
            eval_point(&it, erfc_x, &arg, PREC),
            direct_erfc,
            "erfc dispatch differs from the facade for {arg}"
        );
        // The two kernels must not collapse onto each other (both differ from
        // each other for every argument sampled here).
        assert!((direct_erf.clone() - direct_erfc.clone()).abs() > tol.clone());
    }
    let one = BigFloat::one();
    let erf1 = eval_point(&it, erf_x, &one, PREC);
    let erfc1 = eval_point(&it, erfc_x, &one, PREC);
    assert!((erf1.clone() - BigFloat::from_f64(0.842700792949715)).abs() < tol.clone());
    assert!((erfc1.clone() - BigFloat::from_f64(0.157299207050285)).abs() < tol.clone());
    // Complements within the tested precision.
    assert!((erf1 + erfc1 - BigFloat::one()).abs() < tol);
}

#[test]
fn robustness_all_thirty_functions_evaluable() {
    // The 15-function roster above predates erf/erfc; this one covers all
    // thirty variants so no dispatch arm can be dropped silently.
    let mut it = Interner::new();
    let half = it.intern(Expr::Constant(BigFloat::from_f64(0.5)));
    let fns = [
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
    ];
    assert_eq!(fns.len(), 30);
    for f in fns {
        let node = it.intern(Expr::Apply(f, half));
        let v = eval_point(&it, node, &BigFloat::zero(), PREC);
        match f {
            // 0.5 is outside acosh's domain; every other function is defined.
            BaseFn::Acosh => assert!(v.is_nan(), "{f:?} should be out of domain"),
            _ => assert!(v.is_finite(), "{f:?} did not yield a finite value"),
        }
    }
}

#[test]
fn extreme_erf_erfc_special_values_route_to_their_limits() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let erf_x = it.intern(Expr::Apply(BaseFn::Erf, x));
    let erfc_x = it.intern(Expr::Apply(BaseFn::Erfc, x));
    for (expr, arg, want) in [
        (erf_x, BigFloat::infinity(), BigFloat::one()),
        (erf_x, BigFloat::neg_infinity(), -BigFloat::one()),
        (erfc_x, BigFloat::infinity(), BigFloat::zero()),
        (erfc_x, BigFloat::neg_infinity(), BigFloat::from_i64(2)),
    ] {
        let v = eval_point(&it, expr, &arg, PREC);
        assert_eq!(v, want, "{:?} at {arg}", it.get(expr));
    }
    for &expr in &[erf_x, erfc_x] {
        assert!(eval_point(&it, expr, &BigFloat::NAN, PREC).is_nan());
        assert!(eval_point(&it, expr, &BigFloat::zero(), PREC).is_finite());
    }
    // erf is odd: erf(-0) keeps the sign through the evaluator.
    let neg_zero = eval_point(&it, erf_x, &-BigFloat::zero(), PREC);
    assert!(neg_zero.is_zero() && neg_zero.is_sign_negative());
}

// ---- M2-E2 additions: the thirteen new functions ------------------------

/// Interns `f(x)` for a free variable and evaluates it at `arg`.
fn eval_fn(f: BaseFn, arg: &BigFloat) -> BigFloat {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let node = it.intern(Expr::Apply(f, x));
    eval_point(&it, node, arg, PREC)
}

#[test]
fn normal_m2e2_logarithm_known_values() {
    // Exact powers: the correctly rounded kernels land on the exact integer.
    assert_eq!(
        eval_fn(BaseFn::Log2, &BigFloat::from_i64(8)),
        BigFloat::from_i64(3)
    );
    assert_eq!(
        eval_fn(BaseFn::Log10, &BigFloat::from_i64(1000)),
        BigFloat::from_i64(3)
    );
    assert_eq!(
        eval_fn(BaseFn::Log2, &BigFloat::from_f64(0.5)),
        -BigFloat::one()
    );
    assert_eq!(
        eval_fn(BaseFn::Log10, &BigFloat::from_i64(100)),
        BigFloat::from_i64(2)
    );
    // log2(1) = log10(1) = log1p(0) = expm1(0) = 0.
    for (f, arg) in [
        (BaseFn::Log2, BigFloat::one()),
        (BaseFn::Log10, BigFloat::one()),
        (BaseFn::Log1p, BigFloat::zero()),
        (BaseFn::Expm1, BigFloat::zero()),
    ] {
        assert!(eval_fn(f, &arg).is_zero(), "{f:?}({arg})");
    }
}

#[test]
fn normal_m2e2_rounding_known_values() {
    let cases = [
        (BaseFn::Abs, -5.0, 5.0),
        (BaseFn::Abs, 5.0, 5.0),
        (BaseFn::Ceil, 2.1, 3.0),
        (BaseFn::Ceil, -2.1, -2.0),
        (BaseFn::Ceil, 3.0, 3.0),
        (BaseFn::Floor, 2.9, 2.0),
        (BaseFn::Floor, -2.9, -3.0),
        (BaseFn::Floor, -3.0, -3.0),
        // nearestint is ties-to-even.
        (BaseFn::NearestInt, 2.5, 2.0),
        (BaseFn::NearestInt, 3.5, 4.0),
        (BaseFn::NearestInt, -2.5, -2.0),
        (BaseFn::NearestInt, -3.5, -4.0),
        (BaseFn::NearestInt, 2.4, 2.0),
        (BaseFn::NearestInt, 2.6, 3.0),
        (BaseFn::NearestInt, 0.5, 0.0),
        (BaseFn::NearestInt, -0.5, 0.0),
    ];
    for (f, arg, want) in cases {
        assert_eq!(
            eval_fn(f, &BigFloat::from_f64(arg)),
            BigFloat::from_f64(want),
            "{f:?}({arg})"
        );
    }
    // Integers are fixed points of all three roundings.
    for f in [BaseFn::Ceil, BaseFn::Floor, BaseFn::NearestInt] {
        for n in [-7i64, 0, 1, 1024] {
            assert_eq!(eval_fn(f, &BigFloat::from_i64(n)), BigFloat::from_i64(n));
        }
    }
}

#[test]
fn normal_m2e2_log1p_expm1_small_x_accuracy_and_inverse() {
    // x = 2^-80: ln(1+x) must keep the small-x information (a naive ln(1+x)
    // at binary64 would lose it entirely). The true correction is ~x/2 =
    // 2^-81 relative, so the relative error must be far below that scale's
    // own resolution: assert it is < 2^-70, which a lost-term implementation
    // (error ~1 relative) cannot pass.
    let x = BigFloat::from_f64(2.0f64.powi(-80));
    let bound = BigFloat::from_f64(2.0f64.powi(-70));
    let log1p_x = eval_fn(BaseFn::Log1p, &x);
    let rel = (log1p_x - x.clone()).abs() / x.clone();
    assert!(rel < bound, "log1p(2^-80) lost the small-x term: rel {rel}");
    let expm1_x = eval_fn(BaseFn::Expm1, &x);
    let rel = (expm1_x - x.clone()).abs() / x.clone();
    assert!(rel < bound, "expm1(2^-80) lost the small-x term: rel {rel}");
    // expm1(log1p(x)) == x and log1p(expm1(x)) == x within the working width.
    let mut it = Interner::new();
    let var = it.intern(Expr::Variable);
    let l = it.intern(Expr::Apply(BaseFn::Log1p, var));
    let e_of_l = it.intern(Expr::Apply(BaseFn::Expm1, l));
    let e = it.intern(Expr::Apply(BaseFn::Expm1, var));
    let l_of_e = it.intern(Expr::Apply(BaseFn::Log1p, e));
    assert!(close(&eval_point(&it, e_of_l, &x, PREC), &x, 100));
    assert!(close(&eval_point(&it, l_of_e, &x, PREC), &x, 100));
    // A larger argument: expm1(ln 2) = 1 exactly in the limit.
    let ln2 = BigFloat::from_i64(2).with_precision(PREC).ln();
    assert!(close(&eval_fn(BaseFn::Expm1, &ln2), &BigFloat::one(), 150));
}

#[test]
fn normal_m2e2_format_conversions_round_to_the_format_grid() {
    let third = BigFloat::one() / BigFloat::from_i64(3);
    // double(1/3) is the binary64 value of 1/3; single(1/3) the binary32 one.
    assert_eq!(
        eval_fn(BaseFn::Double, &third),
        BigFloat::from_f64(1.0 / 3.0)
    );
    assert_eq!(
        eval_fn(BaseFn::Single, &third),
        BigFloat::from_f64(f64::from(1.0f32 / 3.0))
    );
    // Ties-to-even at the format boundary: 2^53+1 -> 2^53, 2^24+1 -> 2^24.
    assert_eq!(
        eval_fn(BaseFn::Double, &BigFloat::from_i64(9007199254740993)),
        BigFloat::from_i64(9007199254740992_i64)
    );
    assert_eq!(
        eval_fn(BaseFn::Single, &BigFloat::from_i64(16777217)),
        BigFloat::from_i64(16777216)
    );
    // 2^53-1 is exactly representable in binary64: a fixed point.
    let m53 = BigFloat::from_i64(9007199254740991_i64);
    assert_eq!(eval_fn(BaseFn::Double, &m53), m53);
    // quad and doubleextended keep more bits than double: the 1/3 images
    // differ from the binary64 one and match the format-width rounding of a
    // wide reference (the backend owns the rounding proof).
    let wide = third.with_precision(300);
    let quad = eval_fn(BaseFn::Quad, &third);
    assert_eq!(quad, wide.round_to(113, rhytido_backend::Rounding::Nearest));
    assert_ne!(quad, BigFloat::from_f64(1.0 / 3.0));
    let de = eval_fn(BaseFn::DoubleExtended, &third);
    assert_eq!(de, wide.round_to(64, rhytido_backend::Rounding::Nearest));
    assert_ne!(de, BigFloat::from_f64(1.0 / 3.0));
    // halfprecision: 1 and 2048 are exact binary16 values; 1/3 rounds to the
    // 11-bit grid and differs from the binary32 image.
    assert_eq!(
        eval_fn(BaseFn::HalfPrecision, &BigFloat::one()),
        BigFloat::one()
    );
    assert_eq!(
        eval_fn(BaseFn::HalfPrecision, &BigFloat::from_i64(2048)),
        BigFloat::from_i64(2048)
    );
    let half = eval_fn(BaseFn::HalfPrecision, &third);
    assert_eq!(half, wide.round_to(11, rhytido_backend::Rounding::Nearest));
    assert_ne!(half, eval_fn(BaseFn::Single, &third));
    // Idempotence: converting an already-converted value changes nothing.
    for f in [
        BaseFn::Double,
        BaseFn::Single,
        BaseFn::HalfPrecision,
        BaseFn::Quad,
        BaseFn::DoubleExtended,
    ] {
        let once = eval_fn(f, &third);
        assert_eq!(eval_fn(f, &once), once, "{f:?} is not idempotent");
    }
}

#[test]
fn error_m2e2_domains_and_special_limits() {
    let neg_inf = BigFloat::neg_infinity();
    // log2/log10: 0 -> -infinity, negative -> NaN.
    for f in [BaseFn::Log2, BaseFn::Log10] {
        let at_zero = eval_fn(f, &BigFloat::zero());
        assert!(
            at_zero.is_infinite() && at_zero.is_sign_negative(),
            "{f:?}(0)"
        );
        assert!(eval_fn(f, &-BigFloat::one()).is_nan(), "{f:?}(-1)");
        assert!(eval_fn(f, &neg_inf).is_nan(), "{f:?}(-inf)");
        assert!(
            eval_fn(f, &BigFloat::infinity()).is_infinite(),
            "{f:?}(+inf)"
        );
    }
    // log1p: -1 -> -infinity, below -1 -> NaN.
    let at_m1 = eval_fn(BaseFn::Log1p, &-BigFloat::one());
    assert!(at_m1.is_infinite() && at_m1.is_sign_negative());
    assert!(eval_fn(BaseFn::Log1p, &BigFloat::from_i64(-2)).is_nan());
    assert!(eval_fn(BaseFn::Log1p, &neg_inf).is_nan());
    // expm1: total, with the two infinite limits.
    assert_eq!(eval_fn(BaseFn::Expm1, &neg_inf), -BigFloat::one());
    assert!(eval_fn(BaseFn::Expm1, &BigFloat::infinity()).is_infinite());
    // abs maps either infinity to +infinity and -0 to +0.
    assert_eq!(eval_fn(BaseFn::Abs, &neg_inf), BigFloat::infinity());
    let abs_nz = eval_fn(BaseFn::Abs, &-BigFloat::zero());
    assert!(abs_nz.is_zero() && abs_nz.is_sign_positive());
    // NaN propagates through every new function.
    for f in [
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
    ] {
        assert!(eval_fn(f, &BigFloat::NAN).is_nan(), "{f:?}(NaN)");
    }
}

#[test]
fn robustness_m2e2_dispatch_matches_the_facade() {
    // Each new arm must route to its own backend method: compare the
    // evaluator with the facade call on the same operand at the same width.
    let args = [
        BigFloat::from_f64(0.5),
        BigFloat::from_f64(2.5),
        BigFloat::from_f64(-1.25),
        BigFloat::one() / BigFloat::from_i64(3),
    ];
    for arg in args {
        let a = arg.with_precision(PREC);
        let cases = [
            (BaseFn::Log2, a.log2()),
            (BaseFn::Log10, a.log10()),
            (BaseFn::Log1p, a.log1p()),
            (BaseFn::Expm1, a.expm1()),
            (BaseFn::Abs, a.abs()),
            (BaseFn::Ceil, a.ceil()),
            (BaseFn::Floor, a.floor()),
            (BaseFn::NearestInt, a.nearestint()),
            (
                BaseFn::Double,
                a.round_to_format(
                    rhytido_backend::Format::Double,
                    rhytido_backend::Rounding::Nearest,
                ),
            ),
            (
                BaseFn::Single,
                a.round_to_format(
                    rhytido_backend::Format::Single,
                    rhytido_backend::Rounding::Nearest,
                ),
            ),
            (
                BaseFn::HalfPrecision,
                a.round_to_format(
                    rhytido_backend::Format::Half,
                    rhytido_backend::Rounding::Nearest,
                ),
            ),
            (
                BaseFn::Quad,
                a.round_to_format(
                    rhytido_backend::Format::Quad,
                    rhytido_backend::Rounding::Nearest,
                ),
            ),
            (
                BaseFn::DoubleExtended,
                a.round_to_format(
                    rhytido_backend::Format::DoubleExtended,
                    rhytido_backend::Rounding::Nearest,
                ),
            ),
        ];
        for (f, want) in cases {
            let got = eval_fn(f, &arg);
            // BigFloat equality is IEEE-style: NaN != NaN, so domain cases
            // are compared by classification.
            if want.is_nan() {
                assert!(got.is_nan(), "{f:?}({arg}) should be NaN");
            } else {
                assert_eq!(got, want, "{f:?}({arg}) dispatch");
            }
        }
    }
}

#[test]
fn robustness_m2e2_special_values_never_panic() {
    for f in [
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
    ] {
        for arg in [
            BigFloat::NAN,
            BigFloat::infinity(),
            BigFloat::neg_infinity(),
            BigFloat::zero(),
            -BigFloat::zero(),
        ] {
            let _ = eval_fn(f, &arg);
        }
    }
}
