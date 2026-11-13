// SPDX-License-Identifier: Apache-2.0
//! Symbolic differentiation of interned expressions (autodiff).
//!
//! [`differentiate`] computes the symbolic derivative of an [`Expr`] with
//! respect to the single free variable [`Expr::Variable`]. The walk is memoized
//! over the interned DAG (`HashMap<NodeId, NodeId>`), so a shared subtree is
//! differentiated once, and every produced node goes through [`Interner::intern`]
//! so structurally identical derivative subtrees are shared (ADR-1).
//!
//! # No simplification
//!
//! The result is *not* simplified: `d/dx (x + x)` is `1 + 1`, not `2`, and the
//! product/quotient rules keep their full expanded shape. Automatic
//! simplification belongs to milestone M4; until then the derivative of a large
//! expression can grow substantially (the product rule doubles the term count at
//! each `Mul`). Callers that evaluate the derivative should rely on the memoized
//! evaluator rather than on a small printed form.
//!
//! # Derivative table
//!
//! The base-function table covers **all seventeen** [`BaseFn`] variants; the
//! `match` is exhaustive, so the compiler rejects the build if a future variant
//! is added without a derivative rule. Every rule is built purely as an [`Expr`]
//! AST (constants via [`BigFloat::from_i64`], [`Expr::Pi`], `Apply`, `Pow`, …);
//! no rule evaluates anything at differentiation time.

use std::collections::HashMap;

use rhytido_backend::BigFloat;

use crate::expr::{BaseFn, Expr, Interner, NodeId};

/// Differentiates `id` with respect to the free variable, returning the interned
/// derivative.
///
/// Constants and [`Expr::Pi`] differentiate to `0`, [`Expr::Variable`] to `1`.
/// The result shares subtrees with `it` and with itself (hash-consing), and is
/// memoized so repeated subtrees are differentiated once. No simplification is
/// performed (see the module documentation); the derivative may be larger than
/// the input.
///
/// # Panics
///
/// Never for a `NodeId` produced by `it`; a foreign `NodeId` panics inside
/// [`Interner::get`], which is the interner's documented invariant.
///
/// # Examples
///
/// ```
/// use rhytido_backend::BigFloat;
/// use rhytido_core::{Expr, Interner, differentiate, eval_point};
///
/// let mut it = Interner::new();
/// let x = it.intern(Expr::Variable);
/// let two = it.intern(Expr::Constant(BigFloat::from_i64(2)));
/// // d/dx x^2 = 2 * x^(2-1) * 1, which evaluates to 2*x.
/// let sq = it.intern(Expr::Pow(x, two));
/// let d = differentiate(&mut it, sq);
/// let val = eval_point(&it, d, &BigFloat::from_i64(3), 53);
/// assert_eq!(val, BigFloat::from_i64(6));
/// ```
pub fn differentiate(it: &mut Interner, id: NodeId) -> NodeId {
    let mut memo = HashMap::new();
    diff_rec(it, id, &mut memo)
}

/// Recursive worker: differentiates `id`, caching results in `memo`.
fn diff_rec(it: &mut Interner, id: NodeId, memo: &mut HashMap<NodeId, NodeId>) -> NodeId {
    if let Some(&d) = memo.get(&id) {
        return d;
    }
    let expr = it.get(id).clone();
    let d = match expr {
        Expr::Constant(_) | Expr::Pi => constant(it, 0),
        Expr::Variable => constant(it, 1),
        Expr::Add(a, b) => {
            let da = diff_rec(it, a, memo);
            let db = diff_rec(it, b, memo);
            it.intern(Expr::Add(da, db))
        }
        Expr::Sub(a, b) => {
            let da = diff_rec(it, a, memo);
            let db = diff_rec(it, b, memo);
            it.intern(Expr::Sub(da, db))
        }
        Expr::Neg(a) => {
            let da = diff_rec(it, a, memo);
            it.intern(Expr::Neg(da))
        }
        Expr::Mul(a, b) => {
            // Product rule: (f*g)' = f'*g + f*g'.
            let da = diff_rec(it, a, memo);
            let db = diff_rec(it, b, memo);
            let left = it.intern(Expr::Mul(da, b));
            let right = it.intern(Expr::Mul(a, db));
            it.intern(Expr::Add(left, right))
        }
        Expr::Div(a, b) => {
            // Quotient rule: (f/g)' = (f'*g - f*g') / g^2.
            let da = diff_rec(it, a, memo);
            let db = diff_rec(it, b, memo);
            let num_left = it.intern(Expr::Mul(da, b));
            let num_right = it.intern(Expr::Mul(a, db));
            let num = it.intern(Expr::Sub(num_left, num_right));
            let den = it.intern(Expr::Mul(b, b));
            it.intern(Expr::Div(num, den))
        }
        Expr::Pow(a, b) => diff_pow(it, a, b, memo),
        Expr::Apply(f, a) => {
            // Chain rule: (f(u))' = f'(u) * u'.
            let da = diff_rec(it, a, memo);
            let df = base_fn_derivative(it, f, a);
            it.intern(Expr::Mul(df, da))
        }
    };
    memo.insert(id, d);
    d
}

/// Differentiates `a^b`, splitting on whether the exponent or base is constant.
fn diff_pow(it: &mut Interner, a: NodeId, b: NodeId, memo: &mut HashMap<NodeId, NodeId>) -> NodeId {
    // Constant exponent c: power rule c * a^(c-1) * a'.
    if let Expr::Constant(c) = it.get(b).clone() {
        let da = diff_rec(it, a, memo);
        // c-1 is itself a constant; compute it with enough guard bits that the
        // subtraction does not round the exponent for the common exact cases.
        let w = c.precision().saturating_add(2).max(64);
        let cm1 = c.with_precision(w) - BigFloat::one().with_precision(w);
        let cm1_node = it.intern(Expr::Constant(cm1));
        let pow = it.intern(Expr::Pow(a, cm1_node));
        let cp = it.intern(Expr::Mul(b, pow));
        return it.intern(Expr::Mul(cp, da));
    }
    // Constant base c: c^b * ln(c) * b'.
    if matches!(it.get(a), Expr::Constant(_)) {
        let db = diff_rec(it, b, memo);
        let pow = it.intern(Expr::Pow(a, b));
        let ln = it.intern(Expr::Apply(BaseFn::Ln, a));
        let pl = it.intern(Expr::Mul(pow, ln));
        return it.intern(Expr::Mul(pl, db));
    }
    // General case: a^b * (b' * ln(a) + b * a'/a).
    let da = diff_rec(it, a, memo);
    let db = diff_rec(it, b, memo);
    let pow = it.intern(Expr::Pow(a, b));
    let ln = it.intern(Expr::Apply(BaseFn::Ln, a));
    let t1 = it.intern(Expr::Mul(db, ln));
    let da_over_a = it.intern(Expr::Div(da, a));
    let t2 = it.intern(Expr::Mul(b, da_over_a));
    let sum = it.intern(Expr::Add(t1, t2));
    it.intern(Expr::Mul(pow, sum))
}

/// Builds `f'(u)` as an AST for every [`BaseFn`] variant (exhaustive).
///
/// Each arm is the closed-form derivative expressed with [`Expr`] nodes only;
/// nothing is evaluated here. The forms match the task sheet: `sqrt -> 1/(2√x)`,
/// `tan -> 1/cos²`, `tanh -> 1-tanh²`, `erf -> (2/√π)·exp(-x²)`, and so on.
fn base_fn_derivative(it: &mut Interner, f: BaseFn, u: NodeId) -> NodeId {
    let one = constant(it, 1);
    let two = constant(it, 2);
    match f {
        BaseFn::Sqrt => {
            // 1 / (2 * sqrt(u)).
            let sqrt_u = it.intern(Expr::Apply(BaseFn::Sqrt, u));
            let den = it.intern(Expr::Mul(two, sqrt_u));
            it.intern(Expr::Div(one, den))
        }
        BaseFn::Exp => it.intern(Expr::Apply(BaseFn::Exp, u)),
        BaseFn::Ln => it.intern(Expr::Div(one, u)),
        BaseFn::Sin => it.intern(Expr::Apply(BaseFn::Cos, u)),
        BaseFn::Cos => {
            let sin_u = it.intern(Expr::Apply(BaseFn::Sin, u));
            it.intern(Expr::Neg(sin_u))
        }
        BaseFn::Tan => {
            // 1 / cos(u)^2.
            let cos_u = it.intern(Expr::Apply(BaseFn::Cos, u));
            let den = square(it, cos_u);
            it.intern(Expr::Div(one, den))
        }
        BaseFn::Asin => {
            // 1 / sqrt(1 - u^2).
            let u2 = square(it, u);
            let sub = it.intern(Expr::Sub(one, u2));
            let sqrt = it.intern(Expr::Apply(BaseFn::Sqrt, sub));
            it.intern(Expr::Div(one, sqrt))
        }
        BaseFn::Acos => {
            // -1 / sqrt(1 - u^2).
            let u2 = square(it, u);
            let sub = it.intern(Expr::Sub(one, u2));
            let sqrt = it.intern(Expr::Apply(BaseFn::Sqrt, sub));
            let div = it.intern(Expr::Div(one, sqrt));
            it.intern(Expr::Neg(div))
        }
        BaseFn::Atan => {
            // 1 / (1 + u^2).
            let u2 = square(it, u);
            let den = it.intern(Expr::Add(one, u2));
            it.intern(Expr::Div(one, den))
        }
        BaseFn::Sinh => it.intern(Expr::Apply(BaseFn::Cosh, u)),
        BaseFn::Cosh => it.intern(Expr::Apply(BaseFn::Sinh, u)),
        BaseFn::Tanh => {
            // 1 - tanh(u)^2.
            let tanh_u = it.intern(Expr::Apply(BaseFn::Tanh, u));
            let t2 = square(it, tanh_u);
            it.intern(Expr::Sub(one, t2))
        }
        BaseFn::Asinh => {
            // 1 / sqrt(1 + u^2).
            let u2 = square(it, u);
            let add = it.intern(Expr::Add(one, u2));
            let sqrt = it.intern(Expr::Apply(BaseFn::Sqrt, add));
            it.intern(Expr::Div(one, sqrt))
        }
        BaseFn::Acosh => {
            // 1 / sqrt(u^2 - 1).
            let u2 = square(it, u);
            let sub = it.intern(Expr::Sub(u2, one));
            let sqrt = it.intern(Expr::Apply(BaseFn::Sqrt, sub));
            it.intern(Expr::Div(one, sqrt))
        }
        BaseFn::Atanh => {
            // 1 / (1 - u^2).
            let u2 = square(it, u);
            let den = it.intern(Expr::Sub(one, u2));
            it.intern(Expr::Div(one, den))
        }
        BaseFn::Erf => erf_derivative_core(it, u),
        BaseFn::Erfc => {
            let core = erf_derivative_core(it, u);
            it.intern(Expr::Neg(core))
        }
    }
}

/// The shared `erf`/`erfc` derivative core `(2 / sqrt(pi)) * exp(-u^2)`.
fn erf_derivative_core(it: &mut Interner, u: NodeId) -> NodeId {
    let pi = it.intern(Expr::Pi);
    let sqrt_pi = it.intern(Expr::Apply(BaseFn::Sqrt, pi));
    let two = constant(it, 2);
    let coeff = it.intern(Expr::Div(two, sqrt_pi));
    let u2 = square(it, u);
    let neg_u2 = it.intern(Expr::Neg(u2));
    let exp_neg = it.intern(Expr::Apply(BaseFn::Exp, neg_u2));
    it.intern(Expr::Mul(coeff, exp_neg))
}

/// Interns the integer constant `v`.
fn constant(it: &mut Interner, v: i64) -> NodeId {
    it.intern(Expr::Constant(BigFloat::from_i64(v)))
}

/// Interns `n * n` (a square), reusing `n` so the subtree stays shared.
fn square(it: &mut Interner, n: NodeId) -> NodeId {
    it.intern(Expr::Mul(n, n))
}
