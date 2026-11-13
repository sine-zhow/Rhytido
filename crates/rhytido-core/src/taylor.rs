// SPDX-License-Identifier: Apache-2.0
//! Taylor (mean-value) interval refinement.
//!
//! [`eval_taylor`] encloses the image of an expression over an interval using
//! the mean-value form
//!
//! ```text
//! f(x_i) ⊆ f(m) + f'(x_i) · (x_i − m),   m = midpoint(x_i).
//! ```
//!
//! For every `x ∈ x_i` the mean-value theorem gives `f(x) = f(m) + f'(ξ)(x − m)`
//! for some `ξ` between `x` and `m`. Because `m ∈ x_i`, that `ξ` also lies in
//! `x_i`, so `f'(ξ)` is covered by any enclosure of `f'(x_i)` and `(x − m)` by the
//! interval `x_i − m`. Enclosing `f(m)`, `f'(x_i)` and `x_i − m` and combining them
//! with outward-rounded interval arithmetic therefore encloses `f(x_i)`. This is
//! what lets the refiner tighten *dependency* blow-up: `f(m)` is evaluated at a
//! single point (no dependency), so an expression that is identically constant
//! but syntactically cancels (e.g. `x·x − (x−1)(x+1) − 1`) gets a tight enclosure
//! even though the naive interval walk is wide.
//!
//! # Enclosing `f(m)` (the inclusion key)
//!
//! `f(m)` must be enclosed, not merely approximated: a nearest-rounded point
//! value alone could sit on the wrong side of the truth and break inclusion.
//! The refiner computes `f(m)` with [`eval_interval`] over the *point* interval
//! `[m, m]` at the guard precision. That evaluator runs every kernel at
//! `guard + GUARD_BITS` with round-to-nearest and then wraps each result outward
//! (the precision model documented in [`crate::interval_eval`]), so the returned
//! range is exactly "the guard-precision nearest value wrapped with its full
//! rounding tolerance". A fixed ulp margin around a bare [`crate::eval_point`]
//! result would *not* be rigorous — accumulated rounding can be amplified by
//! later operations — which is why the point-interval enclosure is used.
//!
//! # Recursion (`k`)
//!
//! The rule is applied **at least once**. With `k = 0` the slope `f'(x_i)` is a
//! plain [`eval_interval`]; with `k > 0` the same mean-value rule is re-applied
//! to the derivative, `k` more times (`k + 1` applications total). Each extra
//! recursion can tighten the slope when the derivative itself has dependency.
//! Derivatives come from [`differentiate`]; because the public entry takes an
//! immutable [`Interner`], the refiner works on a clone so the caller's arena is
//! never mutated (the clone preserves node order, so existing `NodeId`s stay
//! valid).
//!
//! # Degenerate inputs
//!
//! The mean-value form needs a bounded, non-point interval with a finite
//! midpoint. For a NaN/empty input, a point input, or an unbounded input the
//! refiner falls back to the naive [`eval_interval`] enclosure, which is always
//! valid. For a point input the deviation `x_i − m` is exactly zero, so the naive
//! point enclosure already is the tightest mean-value result.
//!
//! The same fallback applies when the mean-value computation itself fails to
//! produce a bounded real enclosure: the derivative is not simplified (M4), so
//! it can carry removable singularities — `d/dx x^0` is `0 · x^-1`, whose
//! interval evaluation over a zero-spanning range is NaN although the product
//! is identically zero. The refinement never loses a valid enclosure: the
//! naive enclosure of the same expression is returned instead, so the result
//! always contains the true range (up to the documented guard residual).
//!
//! # Non-smooth functions
//!
//! The mean-value theorem requires differentiability on the whole interval.
//! `abs` has a kink at zero, and `ceil`/`floor`/`nearestint` and the format
//! conversions (`double`/`single`/`halfprecision`/`quad`/`doubleextended`) are
//! step functions: a slope enclosure cannot bound their jumps, and a midpoint
//! form across a breakpoint can exclude the true image. Whenever the
//! expression contains a non-smooth function ([`BaseFn::is_smooth`]), the
//! refiner therefore returns the naive [`eval_interval`] enclosure unchanged.
//! Derivatives of smooth expressions are themselves smooth (the derivative
//! table only builds smooth functions), so the check at the top level covers
//! every recursion into the derivative as well.

use std::collections::HashSet;

use rhytido_backend::{BigFloat, Interval, Rounding};

use crate::diff::differentiate;
use crate::expr::{Expr, Interner, NodeId};
use crate::interval_eval::{IntervalFnHook, eval_interval};

/// Guard bits added to the requested precision, matching the precision model of
/// [`crate::interval_eval`]: computing at `prec + GUARD_BITS` and rounding the
/// final result outward to `prec` leaves a residual of `2^-GUARD_BITS` of an ulp
/// of `prec`.
const GUARD_BITS: usize = 32;

/// Encloses the image of `id` over `x_i` with the mean-value (Taylor) form.
///
/// `prec` is the requested working precision of the returned enclosure; `k` is
/// the number of extra recursions into the derivative (the rule is applied
/// `k + 1` times, at least once). `hook` is threaded to every interval
/// evaluation exactly as in [`eval_interval`]. The result contains the true
/// range of the expression over `x_i` (up to the documented guard residual).
///
/// For NaN/empty, point, or unbounded `x_i` the naive [`eval_interval`] enclosure
/// is returned unchanged; the same fallback applies when the expression
/// contains a non-smooth function (see the module documentation's "Non-smooth
/// functions") and when the mean-value computation cannot produce a bounded
/// real enclosure (an unsimplified derivative with a removable singularity),
/// so the inclusion contract is unconditional.
///
/// # Panics
///
/// Never for a `NodeId` produced by `it`; a foreign `NodeId` panics inside
/// [`Interner::get`], the interner's documented invariant. A hook panic
/// propagates unchanged (hooks are the caller's code).
///
/// # Examples
///
/// ```
/// use rhytido_backend::{BigFloat, Interval};
/// use rhytido_core::{Expr, Interner, eval_taylor};
///
/// let mut it = Interner::new();
/// let x = it.intern(Expr::Variable);
/// // f = x - x is identically zero; the naive walk over [-1, 1] is wide, but
/// // the mean-value form collapses it because f' = 1 - 1 = 0.
/// let f = it.intern(Expr::Sub(x, x));
/// let span = Interval::new(-BigFloat::one(), BigFloat::one());
/// let enc = eval_taylor(&it, f, &span, 53, 0, None);
/// assert!(enc.contains(&BigFloat::zero()));
/// ```
pub fn eval_taylor(
    it: &Interner,
    id: NodeId,
    x_i: &Interval,
    prec: usize,
    k: u32,
    hook: Option<&dyn IntervalFnHook>,
) -> Interval {
    // The mean-value form needs a bounded, non-point interval. Every other case
    // falls back to the naive enclosure, which is always a valid enclosure.
    let (lo, hi) = match (x_i.lower(), x_i.upper()) {
        (Some(lo), Some(hi)) => (lo, hi),
        _ => return eval_interval(it, id, x_i, prec, hook),
    };
    if x_i.is_point() || !lo.is_finite() || !hi.is_finite() {
        return eval_interval(it, id, x_i, prec, hook);
    }
    // Non-smooth functions (the abs kink, integer rounding and format
    // conversion steps) violate the differentiability premise of the
    // mean-value theorem: a slope enclosure cannot bound the jumps, so the
    // naive enclosure is the only unconditional one (module documentation).
    if contains_non_smooth(it, id) {
        return eval_interval(it, id, x_i, prec, hook);
    }
    // Differentiation interns new nodes, so work on a clone: the caller's
    // interner is immutable here and must not be mutated. The clone keeps node
    // order, so `id` (and every subtree) stays valid.
    let mut owned = it.clone();
    taylor_rec(&mut owned, id, x_i, prec, k, hook)
}

/// Recursive worker: applies the mean-value rule to `id` over `x_i`, returning an
/// enclosure rounded outward to `prec`.
///
/// `x_i` is a bounded non-point interval (the [`eval_taylor`] invariant); the
/// recursion re-applies the rule to the derivative over the same `x_i`.
fn taylor_rec(
    it: &mut Interner,
    id: NodeId,
    x_i: &Interval,
    prec: usize,
    k: u32,
    hook: Option<&dyn IntervalFnHook>,
) -> Interval {
    let work = prec.saturating_add(GUARD_BITS);
    // Caller invariant: bounded non-point input, so both endpoints are present.
    let lo = x_i.lower().expect("bounded input (eval_taylor invariant)");
    let hi = x_i.upper().expect("bounded input (eval_taylor invariant)");
    let m = midpoint(&lo, &hi, work);
    // f(m) enclosed at guard precision: the point-interval evaluation wraps the
    // full rounding tolerance of the guard-precision nearest computation
    // (module documentation, "Enclosing f(m)").
    let fm = eval_interval(it, id, &Interval::point(m.clone()), work, hook);
    let deriv = differentiate(it, id);
    // Slope: an enclosure of f'(x_i). k = 0 uses a plain interval evaluation;
    // k > 0 re-applies the mean-value rule to the derivative (returned at the
    // parent's working precision so it combines with fm and dev below).
    let slope = if k == 0 {
        eval_interval(it, deriv, x_i, work, hook)
    } else {
        taylor_rec(it, deriv, x_i, work, k - 1, hook)
    };
    // dev encloses {x - m : x ∈ x_i} and contains 0 because m ∈ x_i.
    let dev = x_i.sub(&Interval::point(m));
    let raw = fm.add(&dev.mul(&slope));
    // The mean-value form is only a refinement: when it fails to produce a
    // bounded real enclosure, fall back to the naive enclosure of the same
    // expression, which is always valid. This happens when the unsimplified
    // derivative carries a removable singularity — d/dx x^0 is 0 * x^-1, and
    // interval evaluation of x^-1 over a zero-spanning range is NaN even
    // though the product is identically zero (simplification belongs to M4).
    // Falling back keeps the inclusion contract unconditional.
    let bounded = match (raw.lower(), raw.upper()) {
        (Some(lo), Some(hi)) => lo.is_finite() && hi.is_finite(),
        _ => false,
    };
    if bounded {
        round_outward(&raw, prec)
    } else {
        eval_interval(it, id, x_i, prec, hook)
    }
}

/// The midpoint of `[lo, hi]`, computed exactly as a dyadic and clamped into the
/// interval so the mean-value theorem's `ξ ∈ x_i` premise holds.
fn midpoint(lo: &BigFloat, hi: &BigFloat, work: usize) -> BigFloat {
    // Two extra bits absorb the carry of `lo + hi`, so the sum and the halving
    // are exact and the midpoint is the true center whenever it is representable.
    let w = work
        .max(lo.precision())
        .max(hi.precision())
        .saturating_add(2);
    let sum = lo.with_precision(w) + hi.with_precision(w);
    let mut m = (sum / BigFloat::from_i64(2)).with_precision(w);
    if m < *lo {
        m = lo.clone();
    }
    if m > *hi {
        m = hi.clone();
    }
    m
}

/// Rounds a range outward from its own precision to `prec`, preserving NaN and
/// empty (mirrors the helper in [`crate::interval_eval`]).
fn round_outward(iv: &Interval, prec: usize) -> Interval {
    match (iv.lower(), iv.upper()) {
        (Some(lo), Some(hi)) => Interval::new(
            lo.round_to(prec, Rounding::Down),
            hi.round_to(prec, Rounding::Up),
        ),
        _ => iv.clone(),
    }
}

/// Whether `id` contains a non-smooth base function anywhere in its DAG.
///
/// Memoized over the interned nodes so a shared subtree is visited once. A
/// revisited node answered `false` on its first complete visit (a `true`
/// would have short-circuited the whole walk), so skipping it is sound.
fn contains_non_smooth(it: &Interner, id: NodeId) -> bool {
    let mut seen = HashSet::new();
    contains_non_smooth_rec(it, id, &mut seen)
}

/// Recursive worker of [`contains_non_smooth`].
fn contains_non_smooth_rec(it: &Interner, id: NodeId, seen: &mut HashSet<NodeId>) -> bool {
    if !seen.insert(id) {
        return false;
    }
    match it.get(id) {
        Expr::Apply(f, a) => !f.is_smooth() || contains_non_smooth_rec(it, *a, seen),
        Expr::Constant(_) | Expr::Pi | Expr::Variable => false,
        Expr::Neg(a) => contains_non_smooth_rec(it, *a, seen),
        Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b) | Expr::Pow(a, b) => {
            contains_non_smooth_rec(it, *a, seen) || contains_non_smooth_rec(it, *b, seen)
        }
    }
}
