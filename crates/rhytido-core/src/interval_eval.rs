// SPDX-License-Identifier: Apache-2.0
//! Interval evaluation of interned expressions.
//!
//! Evaluates an [`Expr`] over an [`Interval`] at a requested working precision,
//! memoizing over the interned DAG so shared subtrees are computed once. The
//! structure mirrors the point evaluator in [`crate::eval`]; the difference is
//! that every node returns a range that contains the true range of the node
//! over the input interval, which is what the faithful (Ziv) engine in
//! [`crate::faithful`] certifies against.
//!
//! # Precision model
//!
//! The backend interval kernels round their endpoints to nearest and then
//! outward ([`rhytido_backend::Interval`] module documentation), so an endpoint
//! can sit up to half an ulp *of its own precision* on the wrong side of the
//! true value. Evaluating the whole DAG at a guard precision and rounding only
//! the final result outward to the requested precision pushes that residual
//! down to `2^(-GUARD_BITS)` of an ulp of the requested precision:
//!
//! * the residual is what limits certificate rigor, and it shrinks with every
//!   round of the faithful loop (the requested precision grows), so the engine
//!   never needs to know about it explicitly;
//! * a node whose value is exactly representable at the requested precision is
//!   still returned as a *point* interval, which is what lets the engine report
//!   [`Quality::Exact`](crate::Quality) instead of a mere correct rounding;
//! * the input interval is re-enclosed at the guard precision before the walk,
//!   because the kernels derive their width from their operands' precision
//!   labels — see [`eval_interval`].
//!
//! The residual is documented, not hidden: it accumulates linearly with DAG
//! depth, so expressions with more than about `2^GUARD_BITS` nested inexact
//! nodes could see a larger effect (unreachable in practice; the faithful loop
//! bounds the depth by construction).
//!
//! # Conservatism
//!
//! * `Pi` uses rigorous two-sided bounds on pi, not a rounded single value.
//! * `Pow` with a base interval that is not strictly positive is answered with
//!   exact integer-exponent parity analysis when the exponent is an integer
//!   point, and with the NaN interval otherwise (see [`eval_interval`]).
//! * A function node consults the optional [`IntervalFnHook`] first; the hook's
//!   range is trusted as given (the hook is the caller's own enclosure).

use std::collections::HashMap;

use rhytido_backend::{BigFloat, Format, Interval, Rounding};

use crate::expr::{BaseFn, Expr, Interner, NodeId};

/// Guard bits added to the requested precision inside the DAG evaluation.
///
/// See the module documentation: the residual endpoint error of the backend
/// kernels is half an ulp at the precision it actually used, so evaluating at
/// `prec + GUARD_BITS` and rounding the result outward to `prec` makes the
/// un-certainty `2^(-GUARD_BITS)` of an ulp of `prec`.
const GUARD_BITS: usize = 32;

/// Extra bits used for the rigorous pi bounds (the `Pi` node).
///
/// `pi_with_precision` is `4 * atan(1)` with correctly rounded atan, so the
/// error is at most half an ulp; pi lies in `[2, 4)`, so that is at most
/// `2^(1-w)` for a computation at precision `w`. The explicit margin
/// `2^(3-w)` below is four times larger, which also absorbs the rounding of
/// the margin itself. The same derivation lives in the backend interval layer
/// (`rhytido-backend/src/interval.rs`, `pi_bounds`).
const PI_GUARD_BITS: usize = 10;

/// Hook that lets a caller supply its own enclosure for a base function.
///
/// Returning `Some(range)` short-circuits the backend dispatch for that node;
/// returning `None` falls through to the interval function in
/// `rhytido-backend`. Hooks are threaded explicitly through the evaluator
/// arguments (zero global state, AGENTS.md §3), and the range a hook returns is
/// trusted as a valid enclosure of the true image.
///
/// The trait is object safe and requires [`Sync`] so an evaluation can be
/// shared across threads by value.
///
/// # Examples
///
/// ```
/// use rhytido_backend::{BigFloat, Interval};
/// use rhytido_core::{BaseFn, Expr, IntervalFnHook, Interner, eval_interval};
///
/// /// Encloses `sqrt` by the directed endpoint image at `prec`.
/// struct SqrtByEndpoints;
///
/// impl IntervalFnHook for SqrtByEndpoints {
///     fn apply_fn(&self, f: BaseFn, arg: &Interval, _prec: usize) -> Option<Interval> {
///         if f != BaseFn::Sqrt {
///             return None;
///         }
///         // Interval::new rounds the endpoints outward.
///         Some(Interval::new(arg.lower()?.sqrt(), arg.upper()?.sqrt()))
///     }
/// }
///
/// let mut it = Interner::new();
/// let four = it.intern(Expr::Constant(BigFloat::from_i64(4)));
/// let sqrt = it.intern(Expr::Apply(BaseFn::Sqrt, four));
/// let unit = Interval::point(BigFloat::one());
/// let naive = eval_interval(&it, sqrt, &unit, 53, None);
/// let hooked = eval_interval(&it, sqrt, &unit, 53, Some(&SqrtByEndpoints));
/// assert!(naive.contains(&BigFloat::from_i64(2)));
/// assert!(hooked.contains(&BigFloat::from_i64(2)));
/// ```
pub trait IntervalFnHook: Sync {
    /// Encloses `f` applied to `arg` at working precision `prec`.
    ///
    /// `arg` is the evaluated argument interval at the internal working
    /// precision (the requested precision plus [`GUARD_BITS`][self::GUARD_BITS]
    /// guard bits, which is the precision the rest of the evaluation uses);
    /// `prec` is that internal precision, passed so a hook can round to the
    /// same grid as the evaluator.
    fn apply_fn(&self, f: BaseFn, arg: &Interval, prec: usize) -> Option<Interval>;
}

/// Evaluates the expression `id` over the interval `x` at working precision
/// `prec`, with an optional function hook.
///
/// The free variable is bound to `x`; a constant expression ignores `x`. The
/// result contains the true range of the expression over `x` up to the
/// documented guard residual, and propagates the backend interval semantics:
/// `NaN` when an operand or a domain boundary is `NaN` (for example the
/// logarithm of a non-positive interval, or division by an interval containing
/// zero), and infinite endpoints when the range is unbounded.
///
/// Memoization is keyed by `(NodeId, prec)`, so each interned subtree is
/// evaluated once per call at one precision (the cache is per call: the input
/// interval and hook are fixed for its lifetime).
///
/// # Panics
///
/// Never for a `NodeId` produced by `it`; a foreign `NodeId` panics inside
/// [`Interner::get`], which is the interner's documented invariant. A hook
/// panic propagates to the caller unchanged (hooks are the caller's code).
///
/// # Examples
///
/// ```
/// use rhytido_backend::{BigFloat, Interval};
/// use rhytido_core::{BaseFn, Expr, Interner, eval_interval};
///
/// let mut it = Interner::new();
/// let x = it.intern(Expr::Variable);
/// let two = it.intern(Expr::Constant(BigFloat::from_i64(2)));
/// // exp(x) evaluated over the point interval [1, 1].
/// let expr = it.intern(Expr::Apply(BaseFn::Exp, x));
/// let arg = Interval::point(BigFloat::one());
/// let y = eval_interval(&it, expr, &arg, 53, None);
/// assert!(y.contains(&BigFloat::one().exp()));
/// // x^2 over [-1, 1] must contain 0, which plain endpoint arithmetic misses.
/// let sq = it.intern(Expr::Pow(x, two));
/// let span = Interval::new(-BigFloat::one(), BigFloat::one());
/// assert!(eval_interval(&it, sq, &span, 53, None).contains(&BigFloat::zero()));
/// ```
pub fn eval_interval(
    it: &Interner,
    id: NodeId,
    x: &Interval,
    prec: usize,
    hook: Option<&dyn IntervalFnHook>,
) -> Interval {
    // The DAG is evaluated at the guard precision; the result is rounded back
    // outward to the requested precision (module documentation).
    let work = prec.saturating_add(GUARD_BITS);
    // The backend kernels take their rounding width from their operands'
    // precision labels, so an input interval labelled below `work` would cap
    // the whole evaluation at that label and let an inexact kernel result
    // collapse to a "point" enclosure once `round_outward` reaches `prec` — a
    // false Exact/CorrectlyRoundedInexact conclusion for an irrational value.
    // Re-enclosing the input at `work` (exact when the endpoints already fit,
    // widening by at most one work-ulp otherwise, and a no-op for NaN/empty)
    // is what makes the guard model hold for the variable leaf too.
    let input = round_outward(x, work);
    let mut cache = HashMap::new();
    let raw = eval_rec(it, id, &input, work, hook, &mut cache);
    round_outward(&raw, prec)
}

/// Recursive worker: evaluates `id` at precision `prec` using `cache`.
fn eval_rec(
    it: &Interner,
    id: NodeId,
    x: &Interval,
    prec: usize,
    hook: Option<&dyn IntervalFnHook>,
    cache: &mut HashMap<(NodeId, usize), Interval>,
) -> Interval {
    let key = (id, prec);
    if let Some(v) = cache.get(&key) {
        return v.clone();
    }
    let expr = it.get(id).clone();
    let result = match expr {
        // A constant is an exact dyadic: directed rounding to `prec` gives the
        // tightest enclosure, and a point when `prec` can represent it.
        Expr::Constant(c) => Interval::new(
            c.round_to(prec, Rounding::Down),
            c.round_to(prec, Rounding::Up),
        ),
        Expr::Pi => pi_interval(prec),
        Expr::Variable => x.clone(),
        Expr::Add(a, b) => {
            let av = eval_rec(it, a, x, prec, hook, cache);
            let bv = eval_rec(it, b, x, prec, hook, cache);
            av.add(&bv)
        }
        Expr::Sub(a, b) => {
            let av = eval_rec(it, a, x, prec, hook, cache);
            let bv = eval_rec(it, b, x, prec, hook, cache);
            av.sub(&bv)
        }
        Expr::Mul(a, b) => {
            let av = eval_rec(it, a, x, prec, hook, cache);
            let bv = eval_rec(it, b, x, prec, hook, cache);
            av.mul(&bv)
        }
        Expr::Div(a, b) => {
            let av = eval_rec(it, a, x, prec, hook, cache);
            let bv = eval_rec(it, b, x, prec, hook, cache);
            av.div(&bv)
        }
        Expr::Neg(a) => {
            let av = eval_rec(it, a, x, prec, hook, cache);
            negate(&av)
        }
        Expr::Pow(a, b) => {
            let av = eval_rec(it, a, x, prec, hook, cache);
            let bv = eval_rec(it, b, x, prec, hook, cache);
            pow_interval(&av, &bv)
        }
        Expr::Apply(f, a) => {
            let av = eval_rec(it, a, x, prec, hook, cache);
            match hook.and_then(|h| h.apply_fn(f, &av, prec)) {
                Some(range) => range,
                None => apply_interval(f, &av),
            }
        }
    };
    cache.insert(key, result.clone());
    result
}

/// Rigorous two-sided bounds on pi rounded outward to `prec`.
///
/// The derivation is the one in the backend interval layer (see
/// [`PI_GUARD_BITS`]): pi is computed at `prec + PI_GUARD_BITS` with a
/// correctly rounded `atan`, and the explicit margin `2^(3-w)` covers both the
/// half-ulp of that computation and the rounding of the margin itself.
fn pi_interval(prec: usize) -> Interval {
    let w = prec.saturating_add(PI_GUARD_BITS);
    let center = BigFloat::pi_with_precision(w);
    let eps = pow2(3 - w as i64);
    Interval::new(
        (center.clone() - eps.clone()).round_to(prec, Rounding::Down),
        (center + eps).round_to(prec, Rounding::Up),
    )
}

/// Negates an interval: the image of `-t` over `[lo, hi]` is `[-hi, -lo]`.
///
/// The backend has no inherent interval negation, and negation is exactly
/// representable (sign flip of a dyadic is a bit pattern change), so no
/// outward rounding is lost by expressing it as `0 - [lo, hi]`.
fn negate(iv: &Interval) -> Interval {
    Interval::point(BigFloat::zero()).sub(iv)
}

/// Exact power of two `2^exp` (any sign of `exp`).
///
/// Built by repeated squaring so a huge exponent costs `O(log exp)`
/// multiplications. Every intermediate is a one-bit significand, so the
/// arithmetic is exact at any precision and no rounding is involved; the
/// negative case inverts an exact power of two, which is also exact.
fn pow2(exp: i64) -> BigFloat {
    let mut acc = BigFloat::one();
    let mut base = BigFloat::from_i64(2);
    let mut n = exp.unsigned_abs();
    while n > 0 {
        if n & 1 == 1 {
            acc = acc * base.clone();
        }
        if n > 1 {
            base = base.clone() * base;
        }
        n >>= 1;
    }
    if exp >= 0 { acc } else { BigFloat::one() / acc }
}

/// Endpoint access for a bounded range; `None` for NaN or empty.
fn endpoints(iv: &Interval) -> Option<(BigFloat, BigFloat)> {
    match (iv.lower(), iv.upper()) {
        (Some(lo), Some(hi)) => Some((lo, hi)),
        _ => None,
    }
}

/// Power `A^B` over intervals.
///
/// * Strictly positive base: `exp(B * ln A)`, whose outward widening is handled
///   by the backend `ln`/`exp` interval kernels. This is the standard
///   monotone-through-logarithm route and is conservative.
/// * Otherwise, when the exponent is a point holding an exact integer: parity
///   analysis decides the shape of the image. Odd exponents are increasing, so
///   the endpoint image is the range. Even exponents are even functions, so the
///   minimum is at zero when the base interval contains zero and otherwise at
///   the endpoint with the smaller magnitude. A negative exponent together with
///   a base interval containing zero is unbounded, and `x^0 = 1` for every
///   operand (the IEEE convention the backend point `pow` also follows).
/// * Otherwise the base is not strictly positive and the exponent is not an
///   integer point: the real power is not defined on a sub-interval, so the
///   result is the NaN interval. That is a deliberate *conservative* choice (a
///   function that is not real on the input is reported as non-real rather than
///   clipped), matching the domain conservatism of the backend `map_mono`
///   kernels; the faithful engine maps it to `Quality::NonReal`.
///
/// Endpoint powers are evaluated by the backend point `pow` (round to nearest)
/// and re-rounded outward, so each carries the same half-ulp residual as every
/// other kernel here, bounded by the guard precision.
fn pow_interval(a: &Interval, b: &Interval) -> Interval {
    let (alo, ahi) = match endpoints(a) {
        Some(pair) => pair,
        None => return Interval::NAN,
    };
    let (blo, bhi) = match endpoints(b) {
        Some(pair) => pair,
        None => return Interval::NAN,
    };
    let zero = BigFloat::zero();
    if alo > zero {
        // exp(B * ln A): the logarithm of a strictly positive interval.
        return b.mul(&a.ln()).exp();
    }
    if blo != bhi {
        return Interval::NAN;
    }
    if !blo.is_finite() || !blo.is_integer() {
        return Interval::NAN;
    }
    if blo.is_zero() {
        return Interval::point(BigFloat::one());
    }
    if blo < zero && a.contains_zero() {
        // A pole inside the base interval: the range is not a bounded real.
        return Interval::NAN;
    }
    let even = (blo.clone() / BigFloat::from_i64(2)).is_integer();
    if even {
        // Even exponent: the image of the magnitudes, minimum at zero when the
        // base interval spans zero.
        let mag_lo = if a.contains_zero() {
            zero
        } else {
            let p1 = alo.clone().pow(&blo);
            let p2 = ahi.clone().pow(&blo);
            if p1 < p2 { p1 } else { p2 }
        };
        let m1 = alo.abs().pow(&blo);
        let m2 = ahi.abs().pow(&blo);
        let mag_hi = if m1 > m2 { m1 } else { m2 };
        Interval::new(mag_lo, mag_hi)
    } else {
        // Odd exponent: strictly increasing over the reals.
        Interval::new(alo.pow(&blo), ahi.pow(&blo))
    }
}

/// Dispatches a base function to the backend interval kernel.
///
/// Every one of the thirty [`BaseFn`] variants has an interval version: the
/// elementary functions through the `map_mono` endpoint images or the rigorous
/// extremum/pole lattices (`sin`/`cos`/`tan`) in `rhytido-backend`, the error
/// functions through checked error bounds (`erf`/`erfc`), the new smooth
/// functions (`log2`/`log10`/`log1p`/`expm1`) and the rounding functions
/// (`abs`/`ceil`/`floor`/`nearestint`) through their backend interval kernels,
/// and the format conversions through nearest endpoint rounding
/// ([`format_interval`]). Each widens outward as documented in the backend.
fn apply_interval(f: BaseFn, arg: &Interval) -> Interval {
    match f {
        BaseFn::Sqrt => arg.sqrt(),
        BaseFn::Exp => arg.exp(),
        BaseFn::Ln => arg.ln(),
        BaseFn::Sin => arg.sin(),
        BaseFn::Cos => arg.cos(),
        BaseFn::Tan => arg.tan(),
        BaseFn::Asin => arg.asin(),
        BaseFn::Acos => arg.acos(),
        BaseFn::Atan => arg.atan(),
        BaseFn::Sinh => arg.sinh(),
        BaseFn::Cosh => arg.cosh(),
        BaseFn::Tanh => arg.tanh(),
        BaseFn::Asinh => arg.asinh(),
        BaseFn::Acosh => arg.acosh(),
        BaseFn::Atanh => arg.atanh(),
        BaseFn::Erf => arg.erf(),
        BaseFn::Erfc => arg.erfc(),
        BaseFn::Log2 => arg.log2(),
        BaseFn::Log10 => arg.log10(),
        BaseFn::Log1p => arg.log1p(),
        BaseFn::Expm1 => arg.expm1(),
        BaseFn::Abs => arg.abs(),
        BaseFn::Ceil => arg.ceil(),
        BaseFn::Floor => arg.floor(),
        BaseFn::NearestInt => arg.nearestint(),
        BaseFn::Double => format_interval(Format::Double, arg),
        BaseFn::Single => format_interval(Format::Single, arg),
        BaseFn::HalfPrecision => format_interval(Format::Half, arg),
        BaseFn::Quad => format_interval(Format::Quad, arg),
        BaseFn::DoubleExtended => format_interval(Format::DoubleExtended, arg),
    }
}

/// Encloses format conversion by nearest rounding of the endpoints.
///
/// Format conversion is round-to-nearest (Sollya semantics), and
/// round-to-nearest-format is monotone: for any `x` in `[lo, hi]`,
/// `rn(lo) <= rn(x) <= rn(hi)`, so `[rn(lo), rn(hi)]` contains the whole
/// image. Nearest endpoints are used rather than directed ones because they
/// are the tightest valid enclosure: the directed pair `[rd(lo), ru(hi)]`
/// brackets two adjacent format floats even for a point input, which would
/// cost the faithful engine its point collapse (Test 1) and could return the
/// lower neighbor instead of the nearest one. With nearest endpoints a point
/// input folds to a point, so the engine certifies Exact / CorrectlyRounded.
/// The endpoints are exactly representable at the format's width, so the
/// outward rounding of [`Interval::new`] adds no further width. NaN and empty
/// inputs propagate unchanged; infinite endpoints pass through
/// `round_to_format` untouched.
fn format_interval(format: Format, arg: &Interval) -> Interval {
    match (arg.lower(), arg.upper()) {
        (Some(lo), Some(hi)) => Interval::new(
            lo.round_to_format(format, Rounding::Nearest),
            hi.round_to_format(format, Rounding::Nearest),
        ),
        _ => arg.clone(),
    }
}

/// Rounds a range outward from its own precision to `prec`.
///
/// NaN and empty intervals propagate unchanged; infinite endpoints are
/// preserved by `round_to`, so an unbounded range stays unbounded and the
/// faithful engine can see that the value is not a bounded real.
fn round_outward(iv: &Interval, prec: usize) -> Interval {
    match endpoints(iv) {
        Some((lo, hi)) => Interval::new(
            lo.round_to(prec, Rounding::Down),
            hi.round_to(prec, Rounding::Up),
        ),
        None => iv.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bf(v: f64) -> BigFloat {
        BigFloat::from_f64(v)
    }

    fn iv(lo: BigFloat, hi: BigFloat) -> Interval {
        Interval::new(lo, hi)
    }

    /// One ulp of `prec` at `v` (the neighbour span), used as the slack for
    /// comparisons against a raw kernel dispatch: `apply_interval` and
    /// `pow_interval` run at their operands' precision label and get their
    /// guard width from `eval_interval`, so a unit-level call is only expected
    /// to bracket the truth to within a few ulps of that label.
    fn ulp_at(v: &BigFloat, prec: usize) -> BigFloat {
        v.next_above(prec) - v.next_below(prec)
    }

    /// `enc` widened by `slack` contains `v`.
    fn encloses_within(enc: &Interval, v: &BigFloat, slack: &BigFloat) -> bool {
        let lo = enc.lower().expect("bounded range") - slack.clone();
        let hi = enc.upper().expect("bounded range") + slack.clone();
        lo <= *v && *v <= hi
    }

    /// The reference value of a base function at a width far above the tested
    /// one, used only to check that a private kernel lands on its own function.
    fn reference(f: BaseFn, x: &BigFloat) -> BigFloat {
        let w = x.with_precision(400);
        match f {
            BaseFn::Sqrt => w.sqrt(),
            BaseFn::Exp => w.exp(),
            BaseFn::Ln => w.ln(),
            BaseFn::Sin => w.sin(),
            BaseFn::Cos => w.cos(),
            BaseFn::Tan => w.tan(),
            BaseFn::Asin => w.asin(),
            BaseFn::Acos => w.acos(),
            BaseFn::Atan => w.atan(),
            BaseFn::Sinh => w.sinh(),
            BaseFn::Cosh => w.cosh(),
            BaseFn::Tanh => w.tanh(),
            BaseFn::Asinh => w.asinh(),
            BaseFn::Acosh => w.acosh(),
            BaseFn::Atanh => w.atanh(),
            BaseFn::Erf => w.erf(),
            BaseFn::Erfc => w.erfc(),
            BaseFn::Log2 => w.log2(),
            BaseFn::Log10 => w.log10(),
            BaseFn::Log1p => w.log1p(),
            BaseFn::Expm1 => w.expm1(),
            BaseFn::Abs => w.abs(),
            BaseFn::Ceil => w.ceil(),
            BaseFn::Floor => w.floor(),
            BaseFn::NearestInt => w.nearestint(),
            BaseFn::Double => w.round_to_format(Format::Double, Rounding::Nearest),
            BaseFn::Single => w.round_to_format(Format::Single, Rounding::Nearest),
            BaseFn::HalfPrecision => w.round_to_format(Format::Half, Rounding::Nearest),
            BaseFn::Quad => w.round_to_format(Format::Quad, Rounding::Nearest),
            BaseFn::DoubleExtended => w.round_to_format(Format::DoubleExtended, Rounding::Nearest),
        }
    }

    // ---- pow2 ---------------------------------------------------------

    #[test]
    fn normal_pow2_is_exact_in_both_directions() {
        assert_eq!(pow2(0), BigFloat::one());
        assert_eq!(pow2(1), BigFloat::from_i64(2));
        assert_eq!(pow2(10), BigFloat::from_i64(1024));
        assert_eq!(pow2(-1), bf(0.5));
        assert_eq!(pow2(-10), BigFloat::one() / BigFloat::from_i64(1024));
        // Round trip: 2^n * 2^-n == 1 exactly (both are one-bit significands).
        for n in [1i64, 7, 63, 300, 1000] {
            let product = pow2(n) * pow2(-n);
            assert_eq!(product, BigFloat::one(), "2^{n} * 2^-{n}");
        }
    }

    #[test]
    fn extreme_pow2_handles_large_exponents_exactly() {
        // Repeated squaring: the value is a one-bit significand at any width,
        // so no rounding can occur even far beyond the default precision.
        let big = pow2(1000);
        assert_eq!(big.canonical_parts().expect("finite").1, 1000);
        assert_eq!(big.canonical_parts().expect("finite").0.to_string(), "1");
        let small = pow2(-1000);
        assert_eq!(small.canonical_parts().expect("finite").1, -1000);
        assert!(small < BigFloat::one() && small > BigFloat::zero());
    }

    #[test]
    fn robustness_pow2_is_monotone() {
        let mut previous = BigFloat::zero();
        for n in -40..=40i64 {
            let v = pow2(n);
            assert!(v > previous, "2^{n} did not exceed 2^{}", n - 1);
            previous = v;
        }
    }

    // ---- round_outward / endpoints -----------------------------------

    #[test]
    fn normal_round_outward_widens_and_is_idempotent() {
        let third = BigFloat::one() / BigFloat::from_i64(3);
        let point = Interval::point(third.clone());
        let wide = round_outward(&point, 8);
        let lo = wide.lower().expect("range");
        let hi = wide.upper().expect("range");
        assert!(lo <= third.clone() && third.clone() <= hi, "{lo}..{hi}");
        assert!(lo < hi, "1/3 needs a real bracket at eight bits");
        assert_eq!(
            format!("{:?}", round_outward(&wide, 8)),
            format!("{wide:?}")
        );
    }

    #[test]
    fn extreme_round_outward_preserves_special_representations() {
        assert!(round_outward(&Interval::NAN, 53).is_nan());
        assert!(round_outward(&Interval::empty(), 53).is_empty());
        let unbounded = iv(BigFloat::neg_infinity(), BigFloat::infinity());
        let kept = round_outward(&unbounded, 8);
        assert!(!kept.lower().expect("range").is_finite());
        assert!(!kept.upper().expect("range").is_finite());
    }

    #[test]
    fn normal_endpoints_reports_nan_and_empty_as_none() {
        assert!(endpoints(&Interval::NAN).is_none());
        assert!(endpoints(&Interval::empty()).is_none());
        assert!(endpoints(&iv(bf(-1.0), bf(1.0))).is_some());
    }

    // ---- negate -------------------------------------------------------

    #[test]
    fn normal_negate_swaps_and_flips_the_endpoints() {
        let negated = negate(&iv(bf(1.0), bf(2.5)));
        assert!(
            negated.contains(&bf(-2.0))
                && negated.contains(&bf(-1.5))
                && !negated.contains(&bf(1.0))
        );
        let lo = negated.lower().expect("range");
        let hi = negated.upper().expect("range");
        assert_eq!(lo, -bf(2.5));
        assert_eq!(hi, -BigFloat::one());
    }

    #[test]
    fn extreme_negate_handles_zero_unbounded_and_nan() {
        // Negating a point at zero keeps a zero (sign aside) rather than NaN.
        assert!(negate(&Interval::point(BigFloat::zero())).contains(&BigFloat::zero()));
        // -[1, +inf] = [-inf, -1]: the infinite side flips, the finite one maps.
        let unbounded = negate(&iv(BigFloat::one(), BigFloat::infinity()));
        assert_eq!(unbounded.lower().expect("range"), BigFloat::neg_infinity());
        assert_eq!(unbounded.upper().expect("range"), -BigFloat::one());
        assert!(negate(&Interval::NAN).is_nan());
        assert!(negate(&Interval::empty()).is_empty());
        // Involution on a bounded range.
        let source = iv(bf(-3.25), bf(2.5));
        assert_eq!(
            negate(&negate(&source)).lower(),
            source.lower(),
            "negation must be an involution"
        );
        assert_eq!(negate(&negate(&source)).upper(), source.upper());
    }

    // ---- pi_interval --------------------------------------------------

    #[test]
    fn normal_pi_interval_brackets_pi_and_tightens() {
        let reference = BigFloat::pi_with_precision(500);
        let mut previous = None::<BigFloat>;
        for prec in [8usize, 24, 53, 120] {
            let enc = pi_interval(prec);
            assert!(enc.contains(&reference), "pi at {prec}: {enc}");
            assert!(!enc.is_point(), "pi is irrational and must keep width");
            let w = enc.upper().expect("range") - enc.lower().expect("range");
            if let Some(p) = &previous {
                assert!(w <= *p, "pi bounds loosened at {prec}");
            }
            previous = Some(w);
        }
    }

    #[test]
    fn extreme_pi_interval_at_the_smallest_width() {
        let reference = BigFloat::pi_with_precision(200);
        let enc = pi_interval(2);
        assert!(enc.contains(&reference), "pi at two bits: {enc}");
        // At two bits pi is bracketed by 3 and 3.5, the adjacent representables.
        assert!(enc.lower().expect("range") <= BigFloat::from_i64(3));
        assert!(enc.upper().expect("range") >= BigFloat::from_i64(3));
    }

    // ---- pow_interval -------------------------------------------------

    #[test]
    fn normal_pow_interval_positive_base_uses_the_log_route() {
        // 2^0.5 through exp(y*ln x): the image lands on sqrt(2) within the
        // operand-width slack (a swapped kernel would be orders of magnitude
        // away). 2^-1 likewise lands on one half.
        let enc = pow_interval(
            &Interval::point(BigFloat::from_i64(2)),
            &Interval::point(bf(0.5)),
        );
        let reference = BigFloat::from_i64(2).with_precision(400).sqrt();
        let slack = ulp_at(&reference, 165);
        assert!(encloses_within(&enc, &reference, &slack), "2^0.5: {enc}");
        let half = pow_interval(
            &Interval::point(BigFloat::from_i64(2)),
            &Interval::point(-BigFloat::one()),
        );
        assert!(half.contains(&bf(0.5)), "2^-1: {half}");
    }

    #[test]
    fn normal_pow_interval_integer_parity_on_a_negative_base() {
        let minus_two = Interval::point(-BigFloat::one() - BigFloat::one());
        let odd = pow_interval(&minus_two, &Interval::point(BigFloat::from_i64(3)));
        assert_eq!(odd.lower().expect("point"), BigFloat::from_i64(-8));
        assert!(odd.is_point());
        let even = pow_interval(&minus_two, &Interval::point(BigFloat::from_i64(2)));
        assert_eq!(even.lower().expect("point"), BigFloat::from_i64(4));
        // A spanning base with an even exponent must include the minimum at 0.
        let sq = pow_interval(
            &iv(bf(-1.0), bf(1.0)),
            &Interval::point(BigFloat::from_i64(2)),
        );
        assert!(sq.contains(&BigFloat::zero()));
        assert!(sq.lower().expect("range") >= BigFloat::zero());
        // Odd exponents are increasing, so a spanning base spans the images.
        let cube = pow_interval(
            &iv(bf(-1.0), bf(1.0)),
            &Interval::point(BigFloat::from_i64(3)),
        );
        assert!(cube.contains(&bf(-1.0)) && cube.contains(&bf(1.0)));
    }

    #[test]
    fn error_pow_interval_is_conservative_outside_the_real_domain() {
        // Non-integer exponent on a non-positive base.
        let third = BigFloat::one() / BigFloat::from_i64(3);
        let enc = pow_interval(&Interval::point(-BigFloat::one()), &Interval::point(third));
        assert!(enc.is_nan());
        // A pole: negative integer exponent with a base interval over zero.
        let enc = pow_interval(
            &iv(BigFloat::zero(), bf(1.0)),
            &Interval::point(-BigFloat::one()),
        );
        assert!(enc.is_nan());
        // NaN / empty operands.
        let enc = pow_interval(&Interval::NAN, &Interval::point(BigFloat::from_i64(2)));
        assert!(enc.is_nan());
        let enc = pow_interval(&Interval::point(BigFloat::from_i64(2)), &Interval::empty());
        assert!(enc.is_nan());
        // A non-point exponent with a non-positive base is undecided.
        let enc = pow_interval(&iv(bf(-2.0), bf(-1.0)), &iv(bf(1.0), bf(2.0)));
        assert!(enc.is_nan());
    }

    #[test]
    fn extreme_pow_interval_zero_exponent_is_one_everywhere() {
        // The IEEE convention the backend's point pow follows, documented in
        // `pow_interval`.
        for base in [
            iv(bf(-2.0), bf(2.0)),
            Interval::point(BigFloat::zero()),
            Interval::point(bf(3.5)),
        ] {
            let enc = pow_interval(&base, &Interval::point(BigFloat::zero()));
            assert!(enc.is_point(), "{base:?}^0 should be the point 1");
            assert_eq!(enc.lower().expect("point"), BigFloat::one());
        }
    }

    // ---- apply_interval: dispatch coverage ----------------------------

    #[test]
    fn normal_apply_interval_dispatches_every_variant_to_its_own_kernel() {
        // Each arm must route to its own backend kernel: a swapped arm lands
        // far outside the slack, while the correct one agrees with the point
        // facade to within a couple of ulps of the operand label.
        let cases = [
            (BaseFn::Sqrt, bf(2.0)),
            (BaseFn::Exp, bf(1.0)),
            (BaseFn::Ln, bf(2.0)),
            (BaseFn::Sin, bf(1.0)),
            (BaseFn::Cos, bf(1.0)),
            (BaseFn::Tan, bf(1.0)),
            (BaseFn::Asin, bf(0.5)),
            (BaseFn::Acos, bf(0.5)),
            (BaseFn::Atan, bf(1.0)),
            (BaseFn::Sinh, bf(1.0)),
            (BaseFn::Cosh, bf(1.0)),
            (BaseFn::Tanh, bf(1.0)),
            (BaseFn::Asinh, bf(1.0)),
            (BaseFn::Acosh, bf(2.0)),
            (BaseFn::Atanh, bf(0.5)),
            (BaseFn::Erf, bf(1.0)),
            (BaseFn::Erfc, bf(1.0)),
            (BaseFn::Log2, bf(2.0)),
            (BaseFn::Log10, bf(10.0)),
            (BaseFn::Log1p, bf(0.5)),
            (BaseFn::Expm1, bf(1.0)),
            (BaseFn::Abs, bf(-2.0)),
            (BaseFn::Ceil, bf(1.5)),
            (BaseFn::Floor, bf(1.5)),
            (BaseFn::NearestInt, bf(2.5)),
            (BaseFn::Double, bf(0.1)),
            (BaseFn::Single, bf(0.1)),
            (BaseFn::HalfPrecision, bf(0.1)),
            (BaseFn::Quad, bf(0.1)),
            (BaseFn::DoubleExtended, bf(0.1)),
        ];
        let operand_prec = BigFloat::from_i64(2).precision();
        for (f, x) in cases {
            let enc = apply_interval(f, &Interval::point(x.clone()));
            let reference = reference(f, &x);
            let slack = ulp_at(&reference, operand_prec);
            assert!(
                encloses_within(&enc, &reference, &slack),
                "{f:?}({x}): {enc} is not its own kernel value {reference}"
            );
            assert!(!enc.is_nan(), "{f:?}({x}) reported NaN inside its domain");
        }
    }

    #[test]
    fn error_apply_interval_propagates_nan_and_empty() {
        for f in [
            BaseFn::Sqrt,
            BaseFn::Ln,
            BaseFn::Erf,
            BaseFn::Erfc,
            BaseFn::Tan,
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
            assert!(apply_interval(f, &Interval::NAN).is_nan(), "{f:?} NaN");
            assert!(
                apply_interval(f, &Interval::empty()).is_empty(),
                "{f:?} empty"
            );
        }
        // Out of domain: sqrt and ln of a negative point.
        assert!(apply_interval(BaseFn::Sqrt, &Interval::point(-BigFloat::one())).is_nan());
        assert!(apply_interval(BaseFn::Ln, &Interval::point(-BigFloat::one())).is_nan());
        // The new logarithms share the domain conservatism of ln.
        assert!(apply_interval(BaseFn::Log2, &Interval::point(-BigFloat::one())).is_nan());
        assert!(apply_interval(BaseFn::Log10, &Interval::point(-BigFloat::one())).is_nan());
        assert!(apply_interval(BaseFn::Log1p, &Interval::point(-BigFloat::from_i64(2))).is_nan());
    }

    // ---- eval_rec: the memoized walk, through the public entry point --

    #[test]
    fn normal_relabelled_input_reaches_the_kernel_width() {
        // The guard model depends on `eval_interval` re-enclosing the input at
        // the internal width: a Variable leaf labelled at the requested
        // precision must not cap the kernel (see `eval_interval`).
        let mut it = Interner::new();
        let x = it.intern(Expr::Variable);
        let sqrt_x = it.intern(Expr::Apply(BaseFn::Sqrt, x));
        let arg = bf(1.75);
        let coarse = iv(
            arg.round_to(8, Rounding::Down),
            arg.round_to(8, Rounding::Up),
        );
        let enc = eval_interval(&it, sqrt_x, &coarse, 8, None);
        let reference = BigFloat::from_f64(1.75).sqrt().with_precision(300);
        assert!(enc.contains(&reference), "{enc} misses {reference}");
        assert!(
            !enc.is_point(),
            "an irrational must not collapse to a point"
        );
    }
}
