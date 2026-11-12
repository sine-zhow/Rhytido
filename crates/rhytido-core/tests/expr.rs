//! Module tests for `expr` (src/expr.rs): the interned expression AST.
//!
//! Layer: module tests (AGENTS.md §4) — external view of the public API only.

use rhytido_backend::{BigFloat, Rounding};
use rhytido_core::{BaseFn, Expr, Interner, NodeId};

fn constant(it: &mut Interner, v: i64) -> NodeId {
    it.intern(Expr::Constant(BigFloat::from_i64(v)))
}

// ---- Normal cases -----------------------------------------------------

#[test]
fn normal_intern_and_get() {
    let mut it = Interner::new();
    let c = constant(&mut it, 5);
    assert!(matches!(it.get(c), Expr::Constant(_)));
    let v = it.intern(Expr::Variable);
    assert!(matches!(it.get(v), Expr::Variable));
    assert_eq!(it.len(), 2);
}

#[test]
fn normal_structural_sharing() {
    let mut it = Interner::new();
    let a = constant(&mut it, 2);
    let b = constant(&mut it, 3);
    let sum1 = it.intern(Expr::Add(a, b));
    let sum2 = it.intern(Expr::Add(a, b));
    assert_eq!(sum1, sum2); // identical structure shares a node
    assert_eq!(it.len(), 3); // 2 constants + 1 add
}

#[test]
fn normal_constants_dedup_by_value() {
    let mut it = Interner::new();
    let c1 = it.intern(Expr::Constant(BigFloat::from_i64(7)));
    let c2 = it.intern(Expr::Constant(BigFloat::from_i64(7)));
    assert_eq!(c1, c2);
    // Same value at different precision still dedups.
    let hi = BigFloat::from_i64(7).with_precision(300);
    let c3 = it.intern(Expr::Constant(hi));
    assert_eq!(c1, c3);
}

#[test]
fn normal_distinct_structures_distinct_nodes() {
    let mut it = Interner::new();
    let a = constant(&mut it, 2);
    let b = constant(&mut it, 3);
    let add = it.intern(Expr::Add(a, b));
    let mul = it.intern(Expr::Mul(a, b));
    let sub = it.intern(Expr::Sub(a, b));
    assert_ne!(add, mul);
    assert_ne!(add, sub);
    // Operand order matters.
    let add_rev = it.intern(Expr::Add(b, a));
    assert_ne!(add, add_rev);
}

#[test]
fn normal_nested_sharing() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let two = constant(&mut it, 2);
    let x2 = it.intern(Expr::Pow(x, two));
    let expr1 = it.intern(Expr::Add(x2, x));
    let expr2 = it.intern(Expr::Add(x2, x));
    assert_eq!(expr1, expr2);
}

// ---- Extreme cases ----------------------------------------------------

#[test]
fn extreme_signed_zero_distinct() {
    let mut it = Interner::new();
    let pz = it.intern(Expr::Constant(BigFloat::zero()));
    let nz = it.intern(Expr::Constant(-BigFloat::zero()));
    assert_ne!(pz, nz); // signed zero is distinguished
}

#[test]
fn extreme_nan_constants() {
    let mut it = Interner::new();
    let n1 = it.intern(Expr::Constant(BigFloat::NAN));
    let n2 = it.intern(Expr::Constant(BigFloat::NAN));
    assert_eq!(n1, n2); // NaN constants dedup
    let c = constant(&mut it, 1);
    assert_ne!(n1, c);
}

#[test]
fn extreme_large_constant() {
    let mut it = Interner::new();
    let big = BigFloat::from_i64(2).round_to(200, Rounding::Nearest);
    let c1 = it.intern(Expr::Constant(big.clone()));
    let c2 = it.intern(Expr::Constant(big));
    assert_eq!(c1, c2);
}

#[test]
fn extreme_deep_chain() {
    let mut it = Interner::new();
    let mut node = constant(&mut it, 1);
    for _ in 0..200 {
        node = it.intern(Expr::Neg(node));
    }
    assert!(matches!(it.get(node), Expr::Neg(_)));
    assert_eq!(it.len(), 201);
}

// ---- Error cases (异常报错) -------------------------------------------

#[test]
fn error_distinct_variants_not_equal() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let pi = it.intern(Expr::Pi);
    let sqrt_x = it.intern(Expr::Apply(BaseFn::Sqrt, x));
    let exp_x = it.intern(Expr::Apply(BaseFn::Exp, x));
    assert_ne!(x, pi);
    assert_ne!(sqrt_x, exp_x);
    assert_ne!(sqrt_x, x);
}

#[test]
fn error_constant_vs_pi_distinct() {
    let mut it = Interner::new();
    let three = constant(&mut it, 3);
    let pi = it.intern(Expr::Pi);
    assert_ne!(three, pi);
}

// ---- Interruption / robustness cases (异常中断) -----------------------

#[test]
fn robustness_intern_idempotent_under_repetition() {
    let mut it = Interner::new();
    let a = constant(&mut it, 1);
    let b = constant(&mut it, 2);
    let first = it.intern(Expr::Add(a, b));
    for _ in 0..100 {
        assert_eq!(it.intern(Expr::Add(a, b)), first);
    }
    assert_eq!(it.len(), 3);
}

#[test]
fn robustness_empty_interner() {
    let it = Interner::new();
    assert!(it.is_empty());
    assert_eq!(it.len(), 0);
}

#[test]
fn robustness_all_node_kinds_intern() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let c = constant(&mut it, 2);
    let nodes = [
        it.intern(Expr::Pi),
        it.intern(Expr::Add(x, c)),
        it.intern(Expr::Sub(x, c)),
        it.intern(Expr::Mul(x, c)),
        it.intern(Expr::Div(x, c)),
        it.intern(Expr::Neg(x)),
        it.intern(Expr::Pow(x, c)),
        it.intern(Expr::Apply(BaseFn::Sqrt, x)),
        it.intern(Expr::Apply(BaseFn::Exp, x)),
        it.intern(Expr::Apply(BaseFn::Ln, x)),
    ];
    // All distinct kinds produce distinct nodes.
    for i in 0..nodes.len() {
        for j in (i + 1)..nodes.len() {
            assert_ne!(nodes[i], nodes[j]);
        }
    }
}
