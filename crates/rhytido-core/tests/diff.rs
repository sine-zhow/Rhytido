//! Module tests for `diff` (src/diff.rs): symbolic differentiation (autodiff).
//!
//! Layer: module tests (AGENTS.md §4) — external view of the public API only.
//! Four categories are covered: known-derivative identities, an independent
//! central-difference cross-check, structural rules (chain/product/quotient/
//! constant/shared subtree), and special-value / robustness cases.

use rhytido_backend::BigFloat;
use rhytido_core::{BaseFn, Expr, Interner, NodeId, differentiate, eval_point};

/// All thirty base functions, mirroring the exhaustive derivative table.
const ALL_FNS: [BaseFn; 30] = [
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

fn bf(v: f64) -> BigFloat {
    BigFloat::from_f64(v)
}

fn c(it: &mut Interner, v: i64) -> NodeId {
    it.intern(Expr::Constant(BigFloat::from_i64(v)))
}

fn rat(it: &mut Interner, num: i64, den: i64) -> NodeId {
    it.intern(Expr::Constant(
        BigFloat::from_i64(num) / BigFloat::from_i64(den),
    ))
}

fn var(it: &mut Interner) -> NodeId {
    it.intern(Expr::Variable)
}

/// Exact power of two `2^n`, any sign of `n`.
fn pow2(n: i64) -> BigFloat {
    let k = BigFloat::from_i64(n.abs());
    let p = BigFloat::from_i64(2).pow(&k);
    if n >= 0 { p } else { BigFloat::one() / p }
}

/// `|a - b| <= 2^-bits * max(1, |a|, |b|)`: a relative tolerance with an absolute
/// floor of `2^-bits`, so values near zero are still checked meaningfully.
fn close_enough(a: &BigFloat, b: &BigFloat, bits: u32) -> bool {
    let diff = (a.clone() - b.clone()).abs();
    let mut scale = BigFloat::one();
    if a.abs() > scale {
        scale = a.abs();
    }
    if b.abs() > scale {
        scale = b.abs();
    }
    let tol = pow2(-(bits as i64)) * scale;
    diff <= tol
}

/// Sample points per function, chosen to stay inside the domain and away from
/// singularities (no `0` for `sqrt`/`ln`, no `±1` for `asin`/`acos`/`atanh`, no
/// `1` for `acosh`, no `±pi/2` for `tan`) and away from the breakpoints of the
/// non-smooth functions (no integers for `ceil`/`floor`, no half- integers for
/// `nearestint`, no zero for `abs`; the format samples are exact dyadics, far
/// from any rounding boundary of any format).
fn sample_points(f: BaseFn) -> Vec<BigFloat> {
    let pts: &[f64] = match f {
        BaseFn::Sqrt | BaseFn::Ln | BaseFn::Log2 | BaseFn::Log10 => &[0.5, 1.0, 2.0, 3.0, 10.0],
        BaseFn::Asin | BaseFn::Acos | BaseFn::Atanh => &[-0.9, -0.5, 0.0, 0.5, 0.9],
        BaseFn::Acosh => &[1.5, 2.0, 3.0, 5.0, 10.0],
        BaseFn::Tan => &[-1.0, -0.5, 0.0, 0.5, 1.0],
        BaseFn::Log1p => &[-0.9, -0.5, 0.0, 0.5, 1.0, 2.0],
        BaseFn::Abs => &[-2.0, -0.5, 0.5, 1.0, 2.0],
        BaseFn::Ceil | BaseFn::Floor => &[-1.5, -0.5, 0.25, 0.5, 1.5, 2.75],
        BaseFn::NearestInt
        | BaseFn::Double
        | BaseFn::Single
        | BaseFn::HalfPrecision
        | BaseFn::Quad
        | BaseFn::DoubleExtended => &[-1.25, -0.75, 0.25, 0.75, 1.25, 2.75],
        _ => &[-2.0, -0.5, 0.0, 0.5, 1.0, 2.0],
    };
    pts.iter().map(|&v| bf(v)).collect()
}

/// Independently constructed closed-form derivative of `f(x)`, built as an AST
/// *without* calling `differentiate`. Where possible a different algebraic form
/// is used (e.g. `tan' = 1 + tan^2` rather than `1/cos^2`, `sqrt' = (1/2)x^-1/2`
/// rather than `1/(2 sqrt(x))`) so a structural bug in the table is not mirrored.
fn closed_form(it: &mut Interner, f: BaseFn, x: NodeId) -> NodeId {
    let one = c(it, 1);
    let two = c(it, 2);
    let half = rat(it, 1, 2);
    let neg_half = rat(it, -1, 2);
    let neg_one = rat(it, -1, 1);
    let x2 = it.intern(Expr::Mul(x, x));
    match f {
        BaseFn::Sqrt => {
            let p = it.intern(Expr::Pow(x, neg_half));
            it.intern(Expr::Mul(half, p))
        }
        BaseFn::Exp => it.intern(Expr::Apply(BaseFn::Exp, x)),
        BaseFn::Ln => it.intern(Expr::Pow(x, neg_one)),
        BaseFn::Sin => it.intern(Expr::Apply(BaseFn::Cos, x)),
        BaseFn::Cos => {
            let s = it.intern(Expr::Apply(BaseFn::Sin, x));
            it.intern(Expr::Neg(s))
        }
        BaseFn::Tan => {
            let t = it.intern(Expr::Apply(BaseFn::Tan, x));
            let t2 = it.intern(Expr::Mul(t, t));
            it.intern(Expr::Add(one, t2))
        }
        BaseFn::Asin => {
            let d = it.intern(Expr::Sub(one, x2));
            it.intern(Expr::Pow(d, neg_half))
        }
        BaseFn::Acos => {
            let d = it.intern(Expr::Sub(one, x2));
            let p = it.intern(Expr::Pow(d, neg_half));
            it.intern(Expr::Neg(p))
        }
        BaseFn::Atan => {
            let d = it.intern(Expr::Add(one, x2));
            it.intern(Expr::Pow(d, neg_one))
        }
        BaseFn::Sinh => it.intern(Expr::Apply(BaseFn::Cosh, x)),
        BaseFn::Cosh => it.intern(Expr::Apply(BaseFn::Sinh, x)),
        BaseFn::Tanh => {
            let ch = it.intern(Expr::Apply(BaseFn::Cosh, x));
            let ch2 = it.intern(Expr::Mul(ch, ch));
            it.intern(Expr::Div(one, ch2))
        }
        BaseFn::Asinh => {
            let d = it.intern(Expr::Add(one, x2));
            it.intern(Expr::Pow(d, neg_half))
        }
        BaseFn::Acosh => {
            let d = it.intern(Expr::Sub(x2, one));
            it.intern(Expr::Pow(d, neg_half))
        }
        BaseFn::Atanh => {
            let d = it.intern(Expr::Sub(one, x2));
            it.intern(Expr::Pow(d, neg_one))
        }
        BaseFn::Erf => {
            let pi = it.intern(Expr::Pi);
            let spi = it.intern(Expr::Apply(BaseFn::Sqrt, pi));
            let nx2 = it.intern(Expr::Neg(x2));
            let e = it.intern(Expr::Apply(BaseFn::Exp, nx2));
            let num = it.intern(Expr::Mul(two, e));
            it.intern(Expr::Div(num, spi))
        }
        BaseFn::Erfc => {
            let pi = it.intern(Expr::Pi);
            let spi = it.intern(Expr::Apply(BaseFn::Sqrt, pi));
            let nx2 = it.intern(Expr::Neg(x2));
            let e = it.intern(Expr::Apply(BaseFn::Exp, nx2));
            let num = it.intern(Expr::Mul(two, e));
            let d = it.intern(Expr::Div(num, spi));
            it.intern(Expr::Neg(d))
        }
        BaseFn::Log2 => {
            // Independent form: log2(e) / x (the table uses 1 / (x * ln 2)).
            let e = it.intern(Expr::Apply(BaseFn::Exp, one));
            let log2e = it.intern(Expr::Apply(BaseFn::Log2, e));
            it.intern(Expr::Div(log2e, x))
        }
        BaseFn::Log10 => {
            // Independent form: log10(e) / x.
            let e = it.intern(Expr::Apply(BaseFn::Exp, one));
            let log10e = it.intern(Expr::Apply(BaseFn::Log10, e));
            it.intern(Expr::Div(log10e, x))
        }
        BaseFn::Log1p => {
            // Independent form: (1 + x)^-1 (the table uses 1 / (1 + x)).
            let d = it.intern(Expr::Add(one, x));
            it.intern(Expr::Pow(d, neg_one))
        }
        BaseFn::Expm1 => it.intern(Expr::Apply(BaseFn::Exp, x)),
        BaseFn::Abs => {
            // Independent sign form: x / sqrt(x^2) (the table uses x / |x|).
            let root = it.intern(Expr::Apply(BaseFn::Sqrt, x2));
            it.intern(Expr::Div(x, root))
        }
        BaseFn::Ceil
        | BaseFn::Floor
        | BaseFn::NearestInt
        | BaseFn::Double
        | BaseFn::Single
        | BaseFn::HalfPrecision
        | BaseFn::Quad
        | BaseFn::DoubleExtended => {
            // Conservative a.e. derivative of a step function: 0 between the
            // breakpoints (the sample points stay away from them).
            c(it, 0)
        }
    }
}

/// Central difference `(f(x+h) - f(x-h)) / (2h)` of the *original* `f`, computed
/// at working precision `prec`. This reference does not consult the derivative
/// table, so it cross-checks `differentiate` independently.
fn central_diff(it: &Interner, f: NodeId, x: &BigFloat, h: &BigFloat, prec: usize) -> BigFloat {
    // Relabel to `prec` before adding so `x ± h` keeps the tiny `h` exactly.
    let xp = (x.with_precision(prec) + h.with_precision(prec)).with_precision(prec);
    let xm = (x.with_precision(prec) - h.with_precision(prec)).with_precision(prec);
    let fp = eval_point(it, f, &xp, prec);
    let fm = eval_point(it, f, &xm, prec);
    let two_h = (h.with_precision(prec) * BigFloat::from_i64(2)).with_precision(prec);
    ((fp - fm) / two_h).with_precision(prec)
}

// ---- Normal: known-derivative identities (all 30) ---------------------

#[test]
fn normal_known_derivative_identities_all_thirty() {
    // Both the symbolic derivative and the independent closed form are evaluated
    // at 250 bits; they are mathematically identical, so they agree to within a
    // few ulps of 250 (~2^-245). A 2^-180 relative tolerance leaves ~65 bits of
    // margin while still catching any real rule error (which would be O(1)).
    let mut it = Interner::new();
    let x = var(&mut it);
    let prec = 250;
    for f in ALL_FNS {
        let func = it.intern(Expr::Apply(f, x));
        let deriv = differentiate(&mut it, func);
        let expect = closed_form(&mut it, f, x);
        for xv in sample_points(f) {
            let got = eval_point(&it, deriv, &xv, prec);
            let want = eval_point(&it, expect, &xv, prec);
            assert!(
                close_enough(&got, &want, 180),
                "{f:?} at {xv}: derivative {got} != closed form {want}"
            );
        }
    }
}

// ---- Normal: independent central-difference cross-check (all 30) ------

#[test]
fn normal_central_difference_cross_check_all_thirty() {
    // Tolerance derivation (working precision / h / truncation vs rounding):
    // * Working precision p = 250 bits, so each f evaluation carries an absolute
    //   rounding error of at most a few ulps, ~ |f| * 2^-245.
    // * Step h = 2^-60. The central difference has truncation error
    //   (h^2 / 6) * f'''(xi) = 2^-120 / 6 * f''' ~ 2^-123 * f'''. Sample points
    //   stay away from singularities, so |f'''| is modest (well under 2^13),
    //   giving truncation < 2^-110.
    // * The rounding contribution is amplified by 1/(2h): ~ |f| * 2^-245 / 2^-60
    //   = |f| * 2^-185, negligible next to the truncation term.
    // * Total error is therefore < 2^-110. We assert a 2^-80 relative tolerance
    //   (absolute floor 2^-80), leaving ~30 bits of margin. Any genuine rule bug
    //   would produce an O(1) discrepancy, far outside this band.
    let mut it = Interner::new();
    let x = var(&mut it);
    let prec = 250;
    for f in ALL_FNS {
        // The step must stay below half a grid step of the function's own
        // staircase: for the format conversions the grid is the format ulp,
        // so quad (2^-114 near the samples) and doubleextended (2^-65) need a
        // much smaller h than the smooth functions, or the quotient measures
        // the staircase's average slope (~1) instead of the a.e. derivative.
        // The samples are exact dyadics, so x ± h rounds back to x and the
        // quotient is exactly 0, matching the conservative rule.
        let h = match f {
            BaseFn::Quad => pow2(-130),
            BaseFn::DoubleExtended => pow2(-80),
            _ => pow2(-60),
        };
        let func = it.intern(Expr::Apply(f, x));
        let deriv = differentiate(&mut it, func);
        for xv in sample_points(f) {
            let sym = eval_point(&it, deriv, &xv, prec);
            let cd = central_diff(&it, func, &xv, &h, prec);
            assert!(
                close_enough(&sym, &cd, 80),
                "{f:?} at {xv}: symbolic {sym} != central difference {cd}"
            );
        }
    }
}

// ---- Normal: chain / product / quotient rules -------------------------

#[test]
fn normal_chain_rule_composite() {
    // d/dx sin(x^2) = cos(x^2) * 2x.
    let mut it = Interner::new();
    let x = var(&mut it);
    let two = c(&mut it, 2);
    let x2 = it.intern(Expr::Pow(x, two));
    let f = it.intern(Expr::Apply(BaseFn::Sin, x2));
    let d = differentiate(&mut it, f);
    let cos_x2 = it.intern(Expr::Apply(BaseFn::Cos, x2));
    let two_x = it.intern(Expr::Mul(two, x));
    let expect = it.intern(Expr::Mul(cos_x2, two_x));
    for xv in [bf(0.5), bf(1.0), bf(1.5), bf(-1.0)] {
        let got = eval_point(&it, d, &xv, 250);
        let want = eval_point(&it, expect, &xv, 250);
        assert!(
            close_enough(&got, &want, 180),
            "chain at {xv}: {got} vs {want}"
        );
    }
}

#[test]
fn normal_product_rule() {
    // d/dx (x * sin x) = sin x + x * cos x.
    let mut it = Interner::new();
    let x = var(&mut it);
    let sin_x = it.intern(Expr::Apply(BaseFn::Sin, x));
    let f = it.intern(Expr::Mul(x, sin_x));
    let d = differentiate(&mut it, f);
    let cos_x = it.intern(Expr::Apply(BaseFn::Cos, x));
    let x_cos = it.intern(Expr::Mul(x, cos_x));
    let expect = it.intern(Expr::Add(sin_x, x_cos));
    for xv in [bf(0.5), bf(1.0), bf(2.0), bf(-1.0)] {
        let got = eval_point(&it, d, &xv, 250);
        let want = eval_point(&it, expect, &xv, 250);
        assert!(
            close_enough(&got, &want, 180),
            "product at {xv}: {got} vs {want}"
        );
    }
}

#[test]
fn normal_quotient_rule() {
    // d/dx (sin x / x) = (x cos x - sin x) / x^2.
    let mut it = Interner::new();
    let x = var(&mut it);
    let sin_x = it.intern(Expr::Apply(BaseFn::Sin, x));
    let f = it.intern(Expr::Div(sin_x, x));
    let d = differentiate(&mut it, f);
    let cos_x = it.intern(Expr::Apply(BaseFn::Cos, x));
    let x_cos = it.intern(Expr::Mul(x, cos_x));
    let num = it.intern(Expr::Sub(x_cos, sin_x));
    let x2 = it.intern(Expr::Mul(x, x));
    let expect = it.intern(Expr::Div(num, x2));
    for xv in [bf(0.5), bf(1.0), bf(2.0), bf(-1.0)] {
        let got = eval_point(&it, d, &xv, 250);
        let want = eval_point(&it, expect, &xv, 250);
        assert!(
            close_enough(&got, &want, 180),
            "quotient at {xv}: {got} vs {want}"
        );
    }
}

// ---- Normal: constants -> 0, variable -> 1, shared subtree ------------

#[test]
fn normal_constant_and_pi_differentiate_to_zero() {
    let mut it = Interner::new();
    let zero = c(&mut it, 0);
    let five = c(&mut it, 5);
    assert_eq!(differentiate(&mut it, five), zero);
    let pi = it.intern(Expr::Pi);
    assert_eq!(differentiate(&mut it, pi), zero);
    // A constant expression ignores the variable entirely.
    let sum = it.intern(Expr::Add(five, pi));
    let d = differentiate(&mut it, sum);
    assert!(eval_point(&it, d, &bf(1.0), 53).is_zero());
}

#[test]
fn normal_variable_differentiates_to_one() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let one = c(&mut it, 1);
    assert_eq!(differentiate(&mut it, x), one);
}

#[test]
fn normal_shared_subtree_derivative_is_shared() {
    // f = g + g with g = sin x. Memoization differentiates g once, so the two
    // children of f' are the *same* interned node (structural sharing).
    let mut it = Interner::new();
    let x = var(&mut it);
    let g = it.intern(Expr::Apply(BaseFn::Sin, x));
    let f = it.intern(Expr::Add(g, g));
    let d = differentiate(&mut it, f);
    match it.get(d) {
        Expr::Add(a, b) => assert_eq!(*a, *b, "shared subtree must give identical nodes"),
        other => panic!("expected Add, got {other:?}"),
    }
    // And it evaluates to 2 cos x.
    let two = c(&mut it, 2);
    let cos_x = it.intern(Expr::Apply(BaseFn::Cos, x));
    let expect = it.intern(Expr::Mul(two, cos_x));
    for xv in [bf(0.5), bf(1.0), bf(-1.0)] {
        let got = eval_point(&it, d, &xv, 250);
        let want = eval_point(&it, expect, &xv, 250);
        assert!(
            close_enough(&got, &want, 180),
            "shared at {xv}: {got} vs {want}"
        );
    }
}

// ---- Extreme cases ----------------------------------------------------

#[test]
fn extreme_derivative_at_zero_and_special_points() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let prec = 300;
    // erf'(0) = 2/sqrt(pi), erfc'(0) = -2/sqrt(pi). The reference is computed
    // from BigFloat pi (no decimal literal, clippy approx_constant).
    let two_over_sqrt_pi = BigFloat::from_i64(2) / BigFloat::pi_with_precision(prec).sqrt();
    let erf = it.intern(Expr::Apply(BaseFn::Erf, x));
    let erfc = it.intern(Expr::Apply(BaseFn::Erfc, x));
    let d_erf = differentiate(&mut it, erf);
    let d_erfc = differentiate(&mut it, erfc);
    let got = eval_point(&it, d_erf, &BigFloat::zero(), prec);
    assert!(close_enough(&got, &two_over_sqrt_pi, 200), "erf'(0): {got}");
    let got = eval_point(&it, d_erfc, &BigFloat::zero(), prec);
    assert!(
        close_enough(&got, &-two_over_sqrt_pi.clone(), 200),
        "erfc'(0): {got}"
    );
    // sin'(0)=1, cos'(0)=0, exp'(0)=1, tanh'(0)=1.
    let cases = [
        (BaseFn::Sin, BigFloat::one()),
        (BaseFn::Cos, BigFloat::zero()),
        (BaseFn::Exp, BigFloat::one()),
        (BaseFn::Tanh, BigFloat::one()),
    ];
    for (f, want) in cases {
        let func = it.intern(Expr::Apply(f, x));
        let d = differentiate(&mut it, func);
        let got = eval_point(&it, d, &BigFloat::zero(), prec);
        assert!(close_enough(&got, &want, 200), "{f:?}'(0): {got} vs {want}");
    }
}

#[test]
fn extreme_nan_constant_differentiates_to_zero() {
    // A NaN constant is still a constant: its derivative is the zero node.
    let mut it = Interner::new();
    let nan = it.intern(Expr::Constant(BigFloat::NAN));
    let zero = c(&mut it, 0);
    assert_eq!(differentiate(&mut it, nan), zero);
}

#[test]
fn extreme_second_derivative_and_deep_tree() {
    // d2/dx2 sin x = -sin x.
    let mut it = Interner::new();
    let x = var(&mut it);
    let sin_x = it.intern(Expr::Apply(BaseFn::Sin, x));
    let d1 = differentiate(&mut it, sin_x);
    let d2 = differentiate(&mut it, d1);
    let neg_sin = it.intern(Expr::Neg(sin_x));
    for xv in [bf(0.5), bf(1.0), bf(-2.0)] {
        let got = eval_point(&it, d2, &xv, 250);
        let want = eval_point(&it, neg_sin, &xv, 250);
        assert!(
            close_enough(&got, &want, 180),
            "sin'' at {xv}: {got} vs {want}"
        );
    }
    // A deep nested sqrt tree in the variable still differentiates and evaluates
    // finitely; the derivative of nested sqrt is positive for x > 0.
    let mut node = x;
    for _ in 0..12 {
        node = it.intern(Expr::Apply(BaseFn::Sqrt, node));
    }
    let d = differentiate(&mut it, node);
    let got = eval_point(&it, d, &bf(2.0), 200);
    assert!(
        got.is_finite(),
        "deep sqrt derivative should be finite: {got}"
    );
    assert!(
        got > BigFloat::zero(),
        "derivative of nested sqrt at 2 is positive"
    );
}

// ---- Error cases (domain errors surface when evaluating) --------------

#[test]
fn error_derivative_outside_domain_is_non_real() {
    // differentiate is symbolic and total, but evaluating the derivative outside
    // the function's real domain yields NaN (the backend's domain conservatism).
    let mut it = Interner::new();
    let x = var(&mut it);
    let sqrt_x = it.intern(Expr::Apply(BaseFn::Sqrt, x));
    let d_sqrt = differentiate(&mut it, sqrt_x);
    // sqrt'(-1) = 1/(2 sqrt(-1)) -> NaN.
    let got = eval_point(&it, d_sqrt, &bf(-1.0), 100);
    assert!(got.is_nan(), "sqrt'(-1) should be NaN: {got}");
    let asin_x = it.intern(Expr::Apply(BaseFn::Asin, x));
    let d_asin = differentiate(&mut it, asin_x);
    // asin'(2) = 1/sqrt(1-4) -> NaN.
    let got = eval_point(&it, d_asin, &bf(2.0), 100);
    assert!(got.is_nan(), "asin'(2) should be NaN: {got}");
}

// ---- Interruption / robustness cases ----------------------------------

#[test]
fn robustness_differentiate_is_deterministic_and_idempotent() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let f = it.intern(Expr::Apply(BaseFn::Exp, x));
    let d1 = differentiate(&mut it, f);
    let d2 = differentiate(&mut it, f);
    assert_eq!(d1, d2, "interning must dedup repeated differentiation");
    // A fresh interner with the same tree gives the same evaluated derivative.
    let mut other = Interner::new();
    let ox = var(&mut other);
    let of = other.intern(Expr::Apply(BaseFn::Exp, ox));
    let od = differentiate(&mut other, of);
    let a = eval_point(&it, d1, &bf(1.0), 200);
    let b = eval_point(&other, od, &bf(1.0), 200);
    assert!(
        close_enough(&a, &b, 180),
        "cross-interner drift: {a} vs {b}"
    );
}

#[test]
fn robustness_large_shared_dag_terminates() {
    // f = sin x added to itself 100 times (a wide DAG sharing one sin x node).
    // Memoization differentiates the shared node once; the result is 100 cos x.
    let mut it = Interner::new();
    let x = var(&mut it);
    let g = it.intern(Expr::Apply(BaseFn::Sin, x));
    let mut f = g;
    for _ in 0..99 {
        f = it.intern(Expr::Add(f, g));
    }
    let d = differentiate(&mut it, f);
    let got = eval_point(&it, d, &bf(1.0), 200);
    let want = BigFloat::from_i64(100) * bf(1.0).with_precision(200).cos();
    assert!(close_enough(&got, &want, 180), "100*sin' : {got} vs {want}");
}

// ---- M2-E2 additions: the new functions' derivative rules ---------------

#[test]
fn normal_m2e2_smooth_derivative_known_values() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let prec = 250;
    // log2'(8) = 1/(8 ln 2), log10'(1000) = 1/(1000 ln 10).
    let log2_x = it.intern(Expr::Apply(BaseFn::Log2, x));
    let d = differentiate(&mut it, log2_x);
    let want =
        BigFloat::one() / (BigFloat::from_i64(8) * BigFloat::from_i64(2).with_precision(prec).ln());
    let got = eval_point(&it, d, &BigFloat::from_i64(8), prec);
    assert!(close_enough(&got, &want, 200), "log2'(8): {got} vs {want}");
    let log10_x = it.intern(Expr::Apply(BaseFn::Log10, x));
    let d = differentiate(&mut it, log10_x);
    let want = BigFloat::one()
        / (BigFloat::from_i64(1000) * BigFloat::from_i64(10).with_precision(prec).ln());
    let got = eval_point(&it, d, &BigFloat::from_i64(1000), prec);
    assert!(
        close_enough(&got, &want, 200),
        "log10'(1000): {got} vs {want}"
    );
    // log1p'(0) = 1 and expm1'(0) = 1.
    for f in [BaseFn::Log1p, BaseFn::Expm1] {
        let func = it.intern(Expr::Apply(f, x));
        let d = differentiate(&mut it, func);
        let got = eval_point(&it, d, &BigFloat::zero(), prec);
        assert!(
            close_enough(&got, &BigFloat::one(), 200),
            "{f:?}'(0): {got}"
        );
    }
    // expm1'(1) = e.
    let func = it.intern(Expr::Apply(BaseFn::Expm1, x));
    let d = differentiate(&mut it, func);
    let got = eval_point(&it, d, &BigFloat::one(), prec);
    let want = BigFloat::one().with_precision(prec).exp();
    assert!(close_enough(&got, &want, 200), "expm1'(1): {got} vs {want}");
}

#[test]
fn normal_m2e2_non_smooth_derivatives_are_conservative() {
    let mut it = Interner::new();
    let x = var(&mut it);
    // Step functions: the a.e. derivative is 0 between breakpoints.
    for f in [
        BaseFn::Ceil,
        BaseFn::Floor,
        BaseFn::NearestInt,
        BaseFn::Double,
        BaseFn::Single,
        BaseFn::HalfPrecision,
        BaseFn::Quad,
        BaseFn::DoubleExtended,
    ] {
        let func = it.intern(Expr::Apply(f, x));
        let d = differentiate(&mut it, func);
        for xv in [bf(0.25), bf(1.5), bf(-2.75)] {
            let got = eval_point(&it, d, &xv, 100);
            assert!(got.is_zero(), "{f:?}'({xv}) should be 0 a.e., got {got}");
        }
    }
    // abs: sign away from the kink.
    let abs_x = it.intern(Expr::Apply(BaseFn::Abs, x));
    let d = differentiate(&mut it, abs_x);
    assert_eq!(eval_point(&it, d, &bf(2.0), 100), BigFloat::one());
    assert_eq!(eval_point(&it, d, &bf(-2.0), 100), -BigFloat::one());
}

#[test]
fn error_m2e2_abs_derivative_is_undefined_at_the_kink() {
    // sign(0) = 0/|0| is NaN under evaluation: the rule deliberately does not
    // invent a value at the non-differentiable point.
    let mut it = Interner::new();
    let x = var(&mut it);
    let abs_x = it.intern(Expr::Apply(BaseFn::Abs, x));
    let d = differentiate(&mut it, abs_x);
    assert!(eval_point(&it, d, &BigFloat::zero(), 100).is_nan());
}

#[test]
fn robustness_m2e2_chain_rule_through_new_functions() {
    // d/dx double(sin x) = 0 * cos x = 0 a.e. (the conversion is a step
    // function of its argument); d/dx expm1(x^2) = exp(x^2) * 2x.
    let mut it = Interner::new();
    let x = var(&mut it);
    let two = c(&mut it, 2);
    let sin_x = it.intern(Expr::Apply(BaseFn::Sin, x));
    let f = it.intern(Expr::Apply(BaseFn::Double, sin_x));
    let d = differentiate(&mut it, f);
    let got = eval_point(&it, d, &bf(0.5), 100);
    assert!(got.is_zero(), "double(sin x)' should be 0 a.e., got {got}");
    let x2 = it.intern(Expr::Pow(x, two));
    let g = it.intern(Expr::Apply(BaseFn::Expm1, x2));
    let dg = differentiate(&mut it, g);
    let e_x2 = it.intern(Expr::Apply(BaseFn::Exp, x2));
    let two_x = it.intern(Expr::Mul(two, x));
    let expect = it.intern(Expr::Mul(e_x2, two_x));
    for xv in [bf(0.5), bf(1.0), bf(-1.0)] {
        let got = eval_point(&it, dg, &xv, 250);
        let want = eval_point(&it, expect, &xv, 250);
        assert!(close_enough(&got, &want, 180), "expm1(x^2)' at {xv}");
    }
}
