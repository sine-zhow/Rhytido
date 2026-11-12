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
