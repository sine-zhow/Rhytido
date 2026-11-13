//! Module tests for `print` (src/print.rs): expression rendering.
//!
//! Layer: module tests (AGENTS.md §4) — external view of the public API only.

use rhytido_backend::BigFloat;
use rhytido_core::{BaseFn, Expr, Interner, NodeId, print_expr};

fn c(it: &mut Interner, v: i64) -> NodeId {
    it.intern(Expr::Constant(BigFloat::from_i64(v)))
}

// ---- Normal cases -----------------------------------------------------

#[test]
fn normal_atoms() {
    let mut it = Interner::new();
    let five = c(&mut it, 5);
    assert_eq!(print_expr(&it, five), "5");
    let var = it.intern(Expr::Variable);
    assert_eq!(print_expr(&it, var), "_x_");
    let pi = it.intern(Expr::Pi);
    assert_eq!(print_expr(&it, pi), "pi");
}

#[test]
fn normal_binary_ops() {
    let mut it = Interner::new();
    let a = c(&mut it, 2);
    let b = c(&mut it, 3);
    let add = it.intern(Expr::Add(a, b));
    assert_eq!(print_expr(&it, add), "2 + 3");
    let sub = it.intern(Expr::Sub(a, b));
    assert_eq!(print_expr(&it, sub), "2 - 3");
    let mul = it.intern(Expr::Mul(a, b));
    assert_eq!(print_expr(&it, mul), "2*3");
    let div = it.intern(Expr::Div(a, b));
    assert_eq!(print_expr(&it, div), "2/3");
    let pow = it.intern(Expr::Pow(a, b));
    assert_eq!(print_expr(&it, pow), "2^3");
}

#[test]
fn normal_precedence_parenthesization() {
    let mut it = Interner::new();
    let a = c(&mut it, 2);
    let b = c(&mut it, 3);
    let d = c(&mut it, 4);
    // (2+3)*4
    let sum = it.intern(Expr::Add(a, b));
    let prod = it.intern(Expr::Mul(sum, d));
    assert_eq!(print_expr(&it, prod), "(2 + 3)*4");
    // 2+3*4 (no parens needed)
    let prod2 = it.intern(Expr::Mul(b, d));
    let sum2 = it.intern(Expr::Add(a, prod2));
    assert_eq!(print_expr(&it, sum2), "2 + 3*4");
}

#[test]
fn normal_function_application() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let sqrt = it.intern(Expr::Apply(BaseFn::Sqrt, x));
    assert_eq!(print_expr(&it, sqrt), "sqrt(_x_)");
    let exp = it.intern(Expr::Apply(BaseFn::Exp, sqrt));
    assert_eq!(print_expr(&it, exp), "exp(sqrt(_x_))");
}

#[test]
fn normal_negation() {
    let mut it = Interner::new();
    let a = c(&mut it, 5);
    let neg = it.intern(Expr::Neg(a));
    assert_eq!(print_expr(&it, neg), "-5");
    let sum = it.intern(Expr::Add(a, a));
    let neg_sum = it.intern(Expr::Neg(sum));
    assert_eq!(print_expr(&it, neg_sum), "-(5 + 5)");
}

// ---- Extreme cases ----------------------------------------------------

#[test]
fn extreme_right_associativity_parens() {
    let mut it = Interner::new();
    let a = c(&mut it, 1);
    let b = c(&mut it, 2);
    let d = c(&mut it, 3);
    // 1 - (2 - 3)
    let inner = it.intern(Expr::Sub(b, d));
    let outer = it.intern(Expr::Sub(a, inner));
    assert_eq!(print_expr(&it, outer), "1 - (2 - 3)");
    // 1 / (2 / 3)
    let inner_div = it.intern(Expr::Div(b, d));
    let outer_div = it.intern(Expr::Div(a, inner_div));
    assert_eq!(print_expr(&it, outer_div), "1/(2/3)");
}

#[test]
fn extreme_deep_nesting() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let mut node = x;
    for _ in 0..30 {
        node = it.intern(Expr::Apply(BaseFn::Sqrt, node));
    }
    let s = print_expr(&it, node);
    assert!(s.starts_with("sqrt("));
    assert_eq!(s.matches("sqrt(").count(), 30);
}

#[test]
fn extreme_power_precedence() {
    let mut it = Interner::new();
    let a = c(&mut it, 2);
    let b = c(&mut it, 3);
    let sum = it.intern(Expr::Add(a, b));
    let pow = it.intern(Expr::Pow(sum, b));
    assert_eq!(print_expr(&it, pow), "(2 + 3)^3");
}

// ---- Error cases (异常报错) -------------------------------------------

#[test]
fn error_nan_constant_prints() {
    let mut it = Interner::new();
    let nan = it.intern(Expr::Constant(BigFloat::NAN));
    assert_eq!(print_expr(&it, nan), "NaN");
}

// ---- Interruption / robustness cases (异常中断) -----------------------

#[test]
fn robustness_print_never_panics_on_special() {
    let mut it = Interner::new();
    let nan = it.intern(Expr::Constant(BigFloat::NAN));
    let inf = it.intern(Expr::Constant(BigFloat::infinity()));
    let x = it.intern(Expr::Variable);
    for a in [nan, inf, x] {
        let neg = it.intern(Expr::Neg(a));
        let _ = print_expr(&it, neg);
        let add = it.intern(Expr::Add(a, a));
        let _ = print_expr(&it, add);
        let ln = it.intern(Expr::Apply(BaseFn::Ln, a));
        let _ = print_expr(&it, ln);
    }
}

#[test]
fn robustness_shared_subtree_prints_consistently() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let sq = it.intern(Expr::Apply(BaseFn::Sqrt, x));
    let e1 = it.intern(Expr::Add(sq, sq));
    let e2 = it.intern(Expr::Add(sq, sq));
    assert_eq!(print_expr(&it, e1), print_expr(&it, e2));
    assert_eq!(print_expr(&it, e1), "sqrt(_x_) + sqrt(_x_)");
}

// ---- M2-C additions: erf/erfc names (appended, existing cases intact) ----

#[test]
fn normal_erf_erfc_names() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let erf = it.intern(Expr::Apply(BaseFn::Erf, x));
    let erfc = it.intern(Expr::Apply(BaseFn::Erfc, x));
    assert_eq!(print_expr(&it, erf), "erf(_x_)");
    assert_eq!(print_expr(&it, erfc), "erfc(_x_)");
    // Nested and combined with operators: application binds as an atom.
    let inner = it.intern(Expr::Apply(BaseFn::Erfc, erf));
    assert_eq!(print_expr(&it, inner), "erfc(erf(_x_))");
    let two = it.intern(Expr::Constant(BigFloat::from_i64(2)));
    let scaled = it.intern(Expr::Mul(two, erf));
    assert_eq!(print_expr(&it, scaled), "2*erf(_x_)");
    let summed = it.intern(Expr::Add(erf, erfc));
    assert_eq!(print_expr(&it, summed), "erf(_x_) + erfc(_x_)");
    let negated = it.intern(Expr::Neg(erf));
    assert_eq!(print_expr(&it, negated), "-erf(_x_)");
    let powered = it.intern(Expr::Pow(erf, two));
    assert_eq!(print_expr(&it, powered), "erf(_x_)^2");
}

#[test]
fn extreme_all_function_names_are_distinct() {
    // A name collision (e.g. erfc printed as "erf" + junk) would break the
    // byte-level round trip that M2-E will gate on.
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let fns = [
        (BaseFn::Sqrt, "sqrt"),
        (BaseFn::Exp, "exp"),
        (BaseFn::Ln, "ln"),
        (BaseFn::Sin, "sin"),
        (BaseFn::Cos, "cos"),
        (BaseFn::Tan, "tan"),
        (BaseFn::Asin, "asin"),
        (BaseFn::Acos, "acos"),
        (BaseFn::Atan, "atan"),
        (BaseFn::Sinh, "sinh"),
        (BaseFn::Cosh, "cosh"),
        (BaseFn::Tanh, "tanh"),
        (BaseFn::Asinh, "asinh"),
        (BaseFn::Acosh, "acosh"),
        (BaseFn::Atanh, "atanh"),
        (BaseFn::Erf, "erf"),
        (BaseFn::Erfc, "erfc"),
    ];
    let mut printed = Vec::new();
    for (f, name) in fns {
        let node = it.intern(Expr::Apply(f, x));
        let text = print_expr(&it, node);
        assert_eq!(text, format!("{name}(_x_)"));
        printed.push(text);
    }
    for (i, a) in printed.iter().enumerate() {
        for b in printed.iter().skip(i + 1) {
            assert_ne!(a, b, "two functions share a rendering");
        }
    }
}
