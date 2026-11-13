//! Point evaluation of interned expressions.
//!
//! Evaluates an [`Expr`] at a point to a [`BigFloat`] at a given working
//! precision, memoizing over the interned DAG so shared subtrees are computed
//! once. This is the foundation for the faithful (Ziv) engine; by itself it does
//! not carry a rounding-quality certificate.

use std::collections::HashMap;

use rhytido_backend::BigFloat;

use crate::expr::{BaseFn, Expr, Interner, NodeId};

/// Evaluates the expression `id` at the point `x` with working precision `prec`.
///
/// The free variable is bound to `x`. Constant expressions ignore `x`.
pub fn eval_point(it: &Interner, id: NodeId, x: &BigFloat, prec: usize) -> BigFloat {
    let mut cache = HashMap::new();
    eval_rec(it, id, x, prec, &mut cache)
}

fn eval_rec(
    it: &Interner,
    id: NodeId,
    x: &BigFloat,
    prec: usize,
    cache: &mut HashMap<NodeId, BigFloat>,
) -> BigFloat {
    if let Some(v) = cache.get(&id) {
        return v.clone();
    }
    let expr = it.get(id).clone();
    let result = match expr {
        Expr::Constant(c) => c.with_precision(prec),
        Expr::Pi => BigFloat::pi_with_precision(prec),
        Expr::Variable => x.with_precision(prec),
        Expr::Add(a, b) => {
            let av = eval_rec(it, a, x, prec, cache);
            let bv = eval_rec(it, b, x, prec, cache);
            (av + bv).with_precision(prec)
        }
        Expr::Sub(a, b) => {
            let av = eval_rec(it, a, x, prec, cache);
            let bv = eval_rec(it, b, x, prec, cache);
            (av - bv).with_precision(prec)
        }
        Expr::Mul(a, b) => {
            let av = eval_rec(it, a, x, prec, cache);
            let bv = eval_rec(it, b, x, prec, cache);
            (av * bv).with_precision(prec)
        }
        Expr::Div(a, b) => {
            let av = eval_rec(it, a, x, prec, cache);
            let bv = eval_rec(it, b, x, prec, cache);
            (av / bv).with_precision(prec)
        }
        Expr::Neg(a) => {
            let av = eval_rec(it, a, x, prec, cache);
            (-av).with_precision(prec)
        }
        Expr::Pow(a, b) => {
            let av = eval_rec(it, a, x, prec, cache);
            let bv = eval_rec(it, b, x, prec, cache);
            av.pow(&bv).with_precision(prec)
        }
        Expr::Apply(f, a) => {
            let av = eval_rec(it, a, x, prec, cache);
            apply_fn(f, &av).with_precision(prec)
        }
    };
    cache.insert(id, result.clone());
    result
}

fn apply_fn(f: BaseFn, v: &BigFloat) -> BigFloat {
    match f {
        BaseFn::Sqrt => v.sqrt(),
        BaseFn::Exp => v.exp(),
        BaseFn::Ln => v.ln(),
        BaseFn::Sin => v.sin(),
        BaseFn::Cos => v.cos(),
        BaseFn::Tan => v.tan(),
        BaseFn::Asin => v.asin(),
        BaseFn::Acos => v.acos(),
        BaseFn::Atan => v.atan(),
        BaseFn::Sinh => v.sinh(),
        BaseFn::Cosh => v.cosh(),
        BaseFn::Tanh => v.tanh(),
        BaseFn::Asinh => v.asinh(),
        BaseFn::Acosh => v.acosh(),
        BaseFn::Atanh => v.atanh(),
        BaseFn::Erf => v.erf(),
        BaseFn::Erfc => v.erfc(),
    }
}
