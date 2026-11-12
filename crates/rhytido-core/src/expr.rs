//! Interned expression AST (hash-consing).
//!
//! Expressions are immutable nodes stored in an arena and referenced by
//! [`NodeId`]. Structurally identical expressions share a single node
//! (hash-consing), so structural equality is `NodeId` equality and caches never
//! need invalidation (ADR-1). This is a clean-room design; it does not copy
//! Sollya's node representation.
//!
//! M1 covers mathematical expressions; language constructs (commands, control
//! flow, procedures) are added in M3.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use rhytido_backend::BigFloat;

/// A handle to an interned expression node.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId(u32);

/// Base mathematical functions (the elementary function family).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BaseFn {
    /// Square root.
    Sqrt,
    /// Exponential.
    Exp,
    /// Natural logarithm.
    Ln,
    /// Sine.
    Sin,
    /// Cosine.
    Cos,
    /// Tangent.
    Tan,
    /// Arcsine.
    Asin,
    /// Arccosine.
    Acos,
    /// Arctangent.
    Atan,
    /// Hyperbolic sine.
    Sinh,
    /// Hyperbolic cosine.
    Cosh,
    /// Hyperbolic tangent.
    Tanh,
    /// Inverse hyperbolic sine.
    Asinh,
    /// Inverse hyperbolic cosine.
    Acosh,
    /// Inverse hyperbolic tangent.
    Atanh,
}

/// An expression node. Children are interned [`NodeId`]s.
#[derive(Clone, Debug)]
pub enum Expr {
    /// A numeric constant.
    Constant(BigFloat),
    /// The constant pi.
    Pi,
    /// The free variable.
    Variable,
    /// Addition.
    Add(NodeId, NodeId),
    /// Subtraction.
    Sub(NodeId, NodeId),
    /// Multiplication.
    Mul(NodeId, NodeId),
    /// Division.
    Div(NodeId, NodeId),
    /// Negation.
    Neg(NodeId),
    /// Power.
    Pow(NodeId, NodeId),
    /// Application of a base function.
    Apply(BaseFn, NodeId),
}

/// A hash-consing interner for [`Expr`] nodes.
///
/// Identical expressions (by canonical structure) are interned to the same
/// [`NodeId`]. Constants are compared by canonical value (significand and
/// exponent), independent of working precision, with signed zero distinguished.
#[derive(Default)]
pub struct Interner {
    nodes: Vec<Expr>,
    buckets: HashMap<u64, Vec<NodeId>>,
}

impl Interner {
    /// Creates an empty interner.
    pub fn new() -> Self {
        Self::default()
    }

    /// Interns an expression, returning its (possibly shared) [`NodeId`].
    pub fn intern(&mut self, expr: Expr) -> NodeId {
        let hash = expr_hash(&expr);
        if let Some(candidates) = self.buckets.get(&hash) {
            for &id in candidates {
                if expr_eq(&self.nodes[id.0 as usize], &expr) {
                    return id;
                }
            }
        }
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(expr);
        self.buckets.entry(hash).or_default().push(id);
        id
    }

    /// Returns a reference to the node for `id`.
    ///
    /// # Panics
    ///
    /// Panics if `id` was not produced by this interner (an internal invariant).
    pub fn get(&self, id: NodeId) -> &Expr {
        &self.nodes[id.0 as usize]
    }

    /// Returns the number of distinct interned nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns `true` if no nodes are interned.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// Computes the structural hash of an expression (children by `NodeId`).
fn expr_hash(expr: &Expr) -> u64 {
    let mut h = DefaultHasher::new();
    match expr {
        Expr::Constant(c) => {
            0u8.hash(&mut h);
            match c.canonical_parts() {
                None => 0u8.hash(&mut h), // NaN
                Some((sig, exp)) => {
                    1u8.hash(&mut h);
                    sig.hash(&mut h);
                    exp.hash(&mut h);
                }
            }
        }
        Expr::Pi => 1u8.hash(&mut h),
        Expr::Variable => 2u8.hash(&mut h),
        Expr::Add(a, b) => {
            3u8.hash(&mut h);
            a.hash(&mut h);
            b.hash(&mut h);
        }
        Expr::Sub(a, b) => {
            4u8.hash(&mut h);
            a.hash(&mut h);
            b.hash(&mut h);
        }
        Expr::Mul(a, b) => {
            5u8.hash(&mut h);
            a.hash(&mut h);
            b.hash(&mut h);
        }
        Expr::Div(a, b) => {
            6u8.hash(&mut h);
            a.hash(&mut h);
            b.hash(&mut h);
        }
        Expr::Neg(a) => {
            7u8.hash(&mut h);
            a.hash(&mut h);
        }
        Expr::Pow(a, b) => {
            8u8.hash(&mut h);
            a.hash(&mut h);
            b.hash(&mut h);
        }
        Expr::Apply(f, a) => {
            9u8.hash(&mut h);
            f.hash(&mut h);
            a.hash(&mut h);
        }
    }
    h.finish()
}

/// Structural equality matching [`expr_hash`]. Constants compare by canonical
/// value (signed zero distinguished); other nodes compare variant and children.
fn expr_eq(a: &Expr, b: &Expr) -> bool {
    match (a, b) {
        (Expr::Constant(x), Expr::Constant(y)) => {
            match (x.canonical_parts(), y.canonical_parts()) {
                (None, None) => true,
                (Some(p), Some(q)) => p == q,
                _ => false,
            }
        }
        (Expr::Pi, Expr::Pi) => true,
        (Expr::Variable, Expr::Variable) => true,
        (Expr::Add(a1, a2), Expr::Add(b1, b2))
        | (Expr::Sub(a1, a2), Expr::Sub(b1, b2))
        | (Expr::Mul(a1, a2), Expr::Mul(b1, b2))
        | (Expr::Div(a1, a2), Expr::Div(b1, b2))
        | (Expr::Pow(a1, a2), Expr::Pow(b1, b2)) => a1 == b1 && a2 == b2,
        (Expr::Neg(a1), Expr::Neg(b1)) => a1 == b1,
        (Expr::Apply(f1, a1), Expr::Apply(f2, b1)) => f1 == f2 && a1 == b1,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn constant(it: &mut Interner, v: i64) -> NodeId {
        it.intern(Expr::Constant(BigFloat::from_i64(v)))
    }

    // ---- Normal cases -----------------------------------------------------

    #[test]
    fn normal_structural_eq_implies_hash_eq() {
        let mut it = Interner::new();
        let a = constant(&mut it, 2);
        let b = constant(&mut it, 3);
        let pairs = [
            (Expr::Pi, Expr::Pi),
            (Expr::Variable, Expr::Variable),
            (Expr::Add(a, b), Expr::Add(a, b)),
            (Expr::Sub(a, b), Expr::Sub(a, b)),
            (Expr::Mul(a, b), Expr::Mul(a, b)),
            (Expr::Div(a, b), Expr::Div(a, b)),
            (Expr::Neg(a), Expr::Neg(a)),
            (Expr::Pow(a, b), Expr::Pow(a, b)),
            (Expr::Apply(BaseFn::Sqrt, a), Expr::Apply(BaseFn::Sqrt, a)),
            (
                Expr::Constant(BigFloat::from_i64(7)),
                Expr::Constant(BigFloat::from_i64(7).with_precision(300)),
            ),
        ];
        for (x, y) in &pairs {
            assert!(expr_eq(x, y));
            assert_eq!(expr_hash(x), expr_hash(y));
        }
    }

    #[test]
    fn normal_distinct_variants_hash_differently() {
        let mut it = Interner::new();
        let a = constant(&mut it, 2);
        let b = constant(&mut it, 3);
        let variants = [
            Expr::Constant(BigFloat::from_i64(2)),
            Expr::Pi,
            Expr::Variable,
            Expr::Add(a, b),
            Expr::Sub(a, b),
            Expr::Mul(a, b),
            Expr::Div(a, b),
            Expr::Neg(a),
            Expr::Pow(a, b),
            Expr::Apply(BaseFn::Sqrt, a),
            Expr::Apply(BaseFn::Exp, a),
        ];
        for (i, x) in variants.iter().enumerate() {
            for y in variants.iter().skip(i + 1) {
                assert!(!expr_eq(x, y));
                assert_ne!(expr_hash(x), expr_hash(y));
            }
        }
    }

    #[test]
    fn normal_operand_order_and_function_matter() {
        let mut it = Interner::new();
        let a = constant(&mut it, 2);
        let b = constant(&mut it, 3);
        assert!(!expr_eq(&Expr::Add(a, b), &Expr::Add(b, a)));
        assert_ne!(expr_hash(&Expr::Add(a, b)), expr_hash(&Expr::Add(b, a)));
        assert!(!expr_eq(
            &Expr::Apply(BaseFn::Sqrt, a),
            &Expr::Apply(BaseFn::Exp, a)
        ));
    }

    // ---- Extreme cases ----------------------------------------------------

    #[test]
    fn extreme_nan_constant_hash_and_eq() {
        let nan1 = Expr::Constant(BigFloat::NAN);
        let nan2 = Expr::Constant(BigFloat::NAN);
        // NaN constants are structurally equal and hash identically.
        assert!(expr_eq(&nan1, &nan2));
        assert_eq!(expr_hash(&nan1), expr_hash(&nan2));
        let finite = Expr::Constant(BigFloat::from_i64(1));
        assert!(!expr_eq(&nan1, &finite));
        assert!(!expr_eq(&finite, &nan1));
    }

    #[test]
    fn extreme_signed_zero_hash_and_eq() {
        let pz = Expr::Constant(BigFloat::zero());
        let nz = Expr::Constant(-BigFloat::zero());
        // Signed zero is distinguished by the canonical parts.
        assert!(!expr_eq(&pz, &nz));
        assert_ne!(expr_hash(&pz), expr_hash(&nz));
        assert!(expr_eq(&pz, &Expr::Constant(BigFloat::zero())));
        assert_eq!(expr_hash(&pz), expr_hash(&Expr::Constant(BigFloat::zero())));
    }

    // ---- Interruption / robustness cases (异常中断) -----------------------

    #[test]
    fn robustness_intern_discriminates_colliding_candidates() {
        let mut it = Interner::new();
        let a = constant(&mut it, 1);
        let b = constant(&mut it, 2);
        let add = it.intern(Expr::Add(a, b));
        let sub = it.intern(Expr::Sub(a, b));
        // Simulate a hash collision: a non-matching node sits ahead of the
        // matching one in the same bucket, so intern must fall back to expr_eq
        // instead of trusting the hash alone.
        let add_hash = expr_hash(&Expr::Add(a, b));
        it.buckets.get_mut(&add_hash).unwrap().insert(0, sub);
        assert_eq!(it.intern(Expr::Add(a, b)), add);
        assert_eq!(it.intern(Expr::Sub(a, b)), sub);
        assert_eq!(it.len(), 4);
    }

    #[test]
    fn robustness_intern_stable_over_many_nodes() {
        let mut it = Interner::new();
        // Many distinct nodes stress the bucket lists; re-interning must be
        // idempotent and never merge distinct structures.
        let mut ids = Vec::new();
        for i in 0..1000i64 {
            let a = constant(&mut it, i);
            let b = constant(&mut it, i + 1);
            ids.push(it.intern(Expr::Add(a, b)));
        }
        let len = it.len();
        assert_eq!(len, 2001); // 1001 constants + 1000 sums
        for (i, &id) in ids.iter().enumerate() {
            let a = constant(&mut it, i as i64);
            let b = constant(&mut it, i as i64 + 1);
            assert_eq!(it.intern(Expr::Add(a, b)), id);
            assert!(matches!(it.get(id), Expr::Add(_, _)));
        }
        assert_eq!(it.len(), len);
    }
}
