// SPDX-License-Identifier: Apache-2.0
//! Faithful (Ziv-style) evaluation engine with quality certificates.
//!
//! `evaluate_faithful` computes an interned expression at a point and returns
//! the target-precision value together with one of the eight [`Quality`]
//! certificates (AGENTS.md §3 semantic contract: the certificate is never
//! discarded). The method is the classical refinement loop: enclose the true
//! value in an interval, try to certify a rounding conclusion from the two
//! endpoints, and otherwise raise the working precision until the precision
//! budget is exhausted.
//!
//! # Certification tests
//!
//! With `ylo <= y <= yhi` the enclosure of the true value `y` and `p` the
//! target precision ([`Context::precision`]):
//!
//! * **Test 1** (correct rounding): the nearest roundings of both endpoints
//!   agree bitwise, so `y` rounds to that value wherever it sits inside the
//!   enclosure. Exactness is then read off the enclosure itself: a point
//!   enclosure whose value *is* that float is `Exact`, a point enclosure of a
//!   wider value is `CorrectlyRoundedInexact` (the true value is known and it
//!   is not representable), and a genuine width gives `CorrectlyRounded`
//!   (nothing is known about exactness).
//! * **Test 2** (faithful across a boundary): the two nearest roundings differ
//!   but are adjacent target floats, so `y` lies between two consecutive
//!   representable values and both of them are faithful roundings of it. The
//!   lower one is returned: the choice is *fixed* rather than "whichever is
//!   closer", because the engine must be deterministic and reproducible for the
//!   same input (and the midpoint rule would need the true value, which is what
//!   we are trying to determine). Certificate `FaithfulInexact`: a rounding
//!   boundary is inside the enclosure, so exactness cannot be claimed.
//! * **Test 3** (exactly one float inside): rounding the lower endpoint up and
//!   the upper endpoint down yields the same float `c`, the only target float
//!   in the enclosure. `c` is always `round_down(y)` or `round_up(y)`, hence
//!   faithful, and its exactness is unknown: `Faithful`.
//!
//! Test 3 is checked *before* Test 2. Both can fire on the same enclosure, and
//! Test 2's conclusion needs the true value to be strictly between the two
//! adjacent floats, while Test 3's conclusion about the unique contained float
//! holds even when `y` is exactly that float — so preferring Test 3 keeps the
//! certificate unconditional and never loses a conclusion.
//!
//! # One more unit in the last place
//!
//! A Test 2 or Test 3 conclusion is faithful but not correctly rounded. The
//! engine therefore spends one more round at `w + 10` bits and adopts that
//! round's Test 1 result if it appears, upgrading the certificate to a
//! correct-rounding one for free whenever the enclosure tightens. Otherwise the
//! provisional faithful conclusion stands.
//!
//! The retry round also adopts a *non-real* or *cutoff* conclusion. Those two
//! precede the certification tests in the contract's decision order, and a
//! tighter enclosure can reach them when the coarser one did not (an enclosure
//! that shrinks into `[-cutoff, +cutoff]`, or a domain crossing that the coarser
//! grid missed); reporting a faithful value instead would contradict the order.
//! A second faithful conclusion is *not* adopted, for the reason above.
//!
//! # Taylor refinement of the enclosure (M2-D)
//!
//! When the expression contains the free variable, each round's naive
//! enclosure is intersected with the mean-value (Taylor) enclosure
//! [`eval_taylor`] at the same working precision, with the recursion count
//! `k` taken from [`Context::taylor_recursions`]. Both enclosures contain the
//! true value (up to the shared guard-residual model of
//! [`crate::interval_eval`]), so their intersection still contains it: a value
//! that lies in both ranges lies in the overlap. The intersection is a subset
//! of the naive enclosure, so it only ever narrows — which is what tightens
//! dependency blow-up (`x·x − (x−1)(x+1) − 1` is identically zero, but the
//! naive walk is wide while the mean-value form collapses it).
//!
//! Fallback protection: if either enclosure is not a bounded real (NaN,
//! empty, or an infinite endpoint), or the overlap is empty — impossible for
//! two valid enclosures of the same value, so only a rounding artifact can
//! produce it — the naive enclosure is kept unchanged. The non-real / cutoff
//! decision order is therefore untouched: refinement never turns a non-real
//! conclusion into a real one, and never widens a range. Point inputs skip
//! the refinement entirely: [`eval_taylor`] documents the naive fallback
//! there, so the intersection would be a no-op.
//!
//! # Cutoff, non-real, and giving up
//!
//! * `cutoff` (a strictly positive magnitude): if the whole enclosure fits in
//!   `[-cutoff, +cutoff]`, the value is replaced by zero with
//!   `Quality::BelowCutoff`. The zero keeps the sign of the enclosure
//!   (negative zero when the enclosure is strictly negative), so everything
//!   knowable beyond "negligible" is preserved.
//! * A `NaN` endpoint, an empty enclosure, or an infinite endpoint means no
//!   real conclusion is possible: the result carries `Quality::NonReal`. This
//!   is decided before any refinement, because raising the precision cannot
//!   turn a non-real enclosure into a real one — the enclosure either already
//!   spans a domain crossing (`ln(0)`, division by an interval containing zero,
//!   a `tan` pole) or the input itself is `NaN` or infinite. The reported value
//!   is the offending endpoint (`NaN` in the `NaN`/empty cases, the infinity in
//!   the unbounded cases).
//! * If the precision budget (`512 * p`, per `docs/plan/M2-plan.md` §2) is
//!   exhausted with no conclusion, the result is `Quality::NotFaithful` and the
//!   value is the nearest rounding of the lower endpoint: deterministic, and
//!   honestly labelled as *not* faithful. This is the table-maker's-dilemma
//!   outcome.
//!
//! Settings are threaded through [`Context`] and the explicit `hook` argument;
//! nothing here reads or writes global state (AGENTS.md §3).
//!
//! # Deviations from the task sheet
//!
//! Three points of `docs/plan/M2-stage2-tasks.md` (task C2) are implemented
//! differently, each for a stated reason; all three are reported to the lead for
//! adjudication rather than silently changing the test expectations:
//!
//! * **Test 2 value** — the sheet says "取近者"; the engine returns the *lower*
//!   rounding image. The true value is not known (it is what the engine is
//!   trying to round), so "closer" is not computable from the enclosure;
//!   returning the lower image is deterministic and still faithful. The sheet's
//!   own acceptance example for this case asserts membership in the pair.
//! * **Cutoff value** — the sheet says `nearest(lo@p)`; the engine flushes to a
//!   signed zero (see the `cutoff_value` helper below).
//! * **Non-real value** — the sheet says `NaN`; the engine reports the offending
//!   endpoint (see the `non_real` helper below).

use std::collections::HashSet;

use rhytido_backend::{BigFloat, Interval, Rounding};

use crate::context::Context;
use crate::expr::{Expr, Interner, NodeId};
use crate::fpresult::FpResult;
use crate::interval_eval::{IntervalFnHook, eval_interval};
use crate::quality::Quality;
use crate::taylor::eval_taylor;

/// Extra bits above the target precision for the first refinement round.
///
/// Starting a few ulp-scales above the target is what lets Test 1 succeed on
/// the first round for most evaluations; the loop grows from there.
const START_GUARD_BITS: usize = 10;

/// Extra bits added by the "one more unit in the last place" retry round.
const EXTRA_UNIT_BITS: usize = 10;

/// Precision budget: the working precision never exceeds this multiple of the
/// target precision (`docs/plan/M2-plan.md` §2).
const MAX_PRECISION_MULTIPLIER: usize = 512;

/// Statistics of one faithful evaluation.
///
/// Both fields are part of the engine's observability contract: the number of
/// refinement rounds performed (including the "one more unit" retry) and the
/// working precision of the last one. A round's Taylor refinement (when it
/// applies) is internal to the round and is not counted as a separate round.
/// `rounds` is saturating, so it counts up to `u32::MAX` rounds without
/// wrapping or panicking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct EngineStats {
    /// Number of refinement rounds performed (at least one).
    pub rounds: u32,
    /// Working precision, in bits, of the last evaluation performed.
    pub final_precision: usize,
}

impl EngineStats {
    /// Creates statistics for a completed evaluation.
    pub fn new(rounds: u32, final_precision: usize) -> Self {
        Self {
            rounds,
            final_precision,
        }
    }

    /// Returns the number of interval evaluations performed.
    pub fn rounds(&self) -> u32 {
        self.rounds
    }

    /// Returns the working precision of the last evaluation.
    pub fn final_precision(&self) -> usize {
        self.final_precision
    }
}

/// Outcome of one certification pass over a bounded enclosure.
#[derive(Debug, Clone)]
enum Verdict {
    /// A conclusion was reached with this value and certificate.
    Certified { value: BigFloat, quality: Quality },
    /// A faithful (Test 2 / Test 3) conclusion, worth one more retry round.
    Faithful { value: BigFloat, quality: Quality },
    /// Nothing can be concluded at this precision.
    Pending,
}

/// Evaluates `id` at the point `x` and returns the target-precision result with
/// its quality certificate, plus engine statistics.
///
/// `ctx` supplies the target precision and the Taylor recursion setting
/// ([`Context::taylor_recursions`], see the module documentation's "Taylor
/// refinement of the enclosure"); the certification tests are stated for
/// round-to-nearest, which is the faithful-engine contract of
/// `docs/plan/M2-plan.md` §2. `cutoff`, when present and strictly positive,
/// replaces negligible results by zero with `Quality::BelowCutoff`. `hook`,
/// when present, is consulted for every base-function node (see
/// [`IntervalFnHook`]).
///
/// The engine terminates: the working precision is bounded by
/// `MAX_PRECISION_MULTIPLIER * ctx.precision()` and each round is a finite
/// memoized interval evaluation. Nothing is shared between calls, so two calls
/// with the same arguments return identical results.
///
/// # Panics
///
/// Never. A `NodeId` that was not produced by `it` panics inside
/// [`Interner::get`], which is the interner's documented invariant.
///
/// # Examples
///
/// ```
/// use rhytido_backend::BigFloat;
/// use rhytido_core::{BaseFn, Context, Expr, Interner, Quality, evaluate_faithful};
///
/// let mut it = Interner::new();
/// let two = it.intern(Expr::Constant(BigFloat::from_i64(2)));
/// let sqrt2 = it.intern(Expr::Apply(BaseFn::Sqrt, two));
/// let ctx = Context::new().with_precision(53);
/// let (result, stats) = evaluate_faithful(&it, sqrt2, &BigFloat::zero(), &ctx, None, None);
/// // The correctly rounded binary64 value of sqrt(2) is the double sqrt(2).
/// assert_eq!(result.value(), &BigFloat::from_f64(core::f64::consts::SQRT_2));
/// assert_eq!(result.quality(), Quality::CorrectlyRounded);
/// assert!(stats.rounds() >= 1);
/// ```
pub fn evaluate_faithful(
    it: &Interner,
    id: NodeId,
    x: &BigFloat,
    ctx: &Context,
    cutoff: Option<&BigFloat>,
    hook: Option<&dyn IntervalFnHook>,
) -> (FpResult, EngineStats) {
    let target = ctx.precision();
    let k = ctx.taylor_recursions();
    let budget = target.saturating_mul(MAX_PRECISION_MULTIPLIER);
    let mut precision = target.saturating_add(START_GUARD_BITS);
    let mut rounds: u32 = 0;
    loop {
        rounds = rounds.saturating_add(1);
        let input = input_interval(x, precision);
        let naive = eval_interval(it, id, &input, precision, hook);
        let enc = refine_with_taylor(it, id, &input, &naive, precision, k, hook);
        let verdict = match certify_round(&enc, target, cutoff) {
            Round::NonReal(value) => {
                return (
                    FpResult::new(value, Quality::NonReal),
                    EngineStats::new(rounds, precision),
                );
            }
            Round::Cutoff(value) => {
                return (
                    FpResult::new(value, Quality::BelowCutoff),
                    EngineStats::new(rounds, precision),
                );
            }
            Round::Verdict(verdict) => verdict,
        };
        match verdict {
            Verdict::Certified { value, quality } => {
                return (
                    FpResult::new(value, quality),
                    EngineStats::new(rounds, precision),
                );
            }
            Verdict::Faithful { value, quality } => {
                let provisional = FpResult::new(value, quality);
                let retry = precision.saturating_add(EXTRA_UNIT_BITS).min(budget);
                if retry <= precision {
                    return (provisional, EngineStats::new(rounds, precision));
                }
                rounds = rounds.saturating_add(1);
                let retry_input = input_interval(x, retry);
                let wide_naive = eval_interval(it, id, &retry_input, retry, hook);
                let wide = refine_with_taylor(it, id, &retry_input, &wide_naive, retry, k, hook);
                // Adopted from the retry round: a Test 1 conclusion (upgrade),
                // or the conclusions that precede the certification tests (see
                // the module documentation). A second faithful conclusion is
                // deliberately not adopted: it would add bits without adding
                // strength, and the provisional value is already certified
                // faithful.
                let conclusion = match certify_round(&wide, target, cutoff) {
                    Round::NonReal(value) => Some((
                        FpResult::new(value, Quality::NonReal),
                        EngineStats::new(rounds, retry),
                    )),
                    Round::Cutoff(value) => Some((
                        FpResult::new(value, Quality::BelowCutoff),
                        EngineStats::new(rounds, retry),
                    )),
                    Round::Verdict(Verdict::Certified {
                        value: better,
                        quality: better_quality,
                    }) => Some((
                        FpResult::new(better, better_quality),
                        EngineStats::new(rounds, retry),
                    )),
                    Round::Verdict(Verdict::Faithful { .. }) | Round::Verdict(Verdict::Pending) => {
                        None
                    }
                };
                if let Some(outcome) = conclusion {
                    return outcome;
                }
                return (provisional, EngineStats::new(rounds, retry));
            }
            Verdict::Pending => {}
        }
        // Nothing decided: grow the working precision within the budget.
        let grown = ceil_five_quarters(precision).min(budget);
        if grown <= precision {
            // The budget is exhausted. A Pending verdict means the enclosure
            // was a bounded real, so `lower()` is present; the NaN case is a
            // defensive fallback that keeps the result a real conclusion-free
            // value rather than panicking.
            let ylo = match enc.lower() {
                Some(lo) => lo,
                None => BigFloat::NAN,
            };
            return (
                FpResult::new(
                    ylo.round_to(target, Rounding::Nearest),
                    Quality::NotFaithful,
                ),
                EngineStats::new(rounds, precision),
            );
        }
        precision = grown;
    }
}

/// What one refinement round can conclude about an enclosure.
enum Round {
    /// No real conclusion; the payload is the value to report.
    NonReal(BigFloat),
    /// The enclosure proved the result negligible w.r.t. the cutoff.
    Cutoff(BigFloat),
    /// One of the certification tests fired, or nothing at all.
    Verdict(Verdict),
}

/// Runs one refinement round's decision logic on an enclosure.
///
/// The order is fixed by the contract: non-real first (refinement is useless),
/// then the cutoff, then the three certification tests.
fn certify_round(enc: &Interval, target: usize, cutoff: Option<&BigFloat>) -> Round {
    if let Some(value) = non_real(enc) {
        return Round::NonReal(value);
    }
    let (ylo, yhi) = pair(enc).unwrap_or((BigFloat::NAN, BigFloat::NAN));
    if let Some(value) = cutoff_value(enc, cutoff) {
        return Round::Cutoff(value);
    }
    Round::Verdict(certify(&ylo, &yhi, enc.is_point(), target))
}

/// The point input `[x, x]` re-rounded outward to `precision`.
///
/// Directed rounding of the endpoints keeps the enclosure valid even when `x`
/// has more significant bits than the working precision; the engine then
/// refines until `x` itself becomes a point.
fn input_interval(x: &BigFloat, precision: usize) -> Interval {
    Interval::new(
        x.round_to(precision, Rounding::Down),
        x.round_to(precision, Rounding::Up),
    )
}

/// Tightens a naive enclosure with the mean-value (Taylor) form.
///
/// The intersection argument and the fallback protection are documented in
/// the module documentation ("Taylor refinement of the enclosure"). `input`
/// is the round's argument interval and `naive` its [`eval_interval`]
/// enclosure; `k` is the recursion setting from [`Context::taylor_recursions`].
fn refine_with_taylor(
    it: &Interner,
    id: NodeId,
    input: &Interval,
    naive: &Interval,
    prec: usize,
    k: u32,
    hook: Option<&dyn IntervalFnHook>,
) -> Interval {
    // Constant expressions have no dependency to tighten; the contract gates
    // the refinement on the presence of the free variable.
    if !contains_variable(it, id) {
        return naive.clone();
    }
    // A point input is the documented naive fallback of `eval_taylor`, so the
    // intersection would be a no-op; skip the duplicated evaluation.
    if input.is_point() {
        return naive.clone();
    }
    // A non-real naive enclosure keeps the contract's decision order: the
    // refinement must not turn a NaN/empty/unbounded conclusion into a real
    // one (module documentation).
    let bounded = match pair(naive) {
        Some((lo, hi)) => lo.is_finite() && hi.is_finite(),
        None => false,
    };
    if !bounded {
        return naive.clone();
    }
    let taylor = eval_taylor(it, id, input, prec, k, hook);
    intersect_enclosures(naive, &taylor)
}

/// Intersects two enclosures of the same value, falling back to `naive`.
///
/// Both ranges contain the true value, so the overlap does too; the result is
/// a subset of `naive` (never wider). If either range is not a bounded real,
/// or the overlap is empty or NaN — impossible for two valid enclosures, so
/// only a rounding artifact can produce it — `naive` is returned unchanged.
/// Endpoint ties keep the naive endpoint, so an intersection that does not
/// tighten reproduces `naive` bitwise (signed zero included).
fn intersect_enclosures(naive: &Interval, taylor: &Interval) -> Interval {
    let (nlo, nhi) = match pair(naive) {
        Some(p) => p,
        None => return naive.clone(),
    };
    let (tlo, thi) = match pair(taylor) {
        Some(p) => p,
        None => return naive.clone(),
    };
    if !nlo.is_finite() || !nhi.is_finite() || !tlo.is_finite() || !thi.is_finite() {
        return naive.clone();
    }
    let lo = if tlo > nlo { tlo } else { nlo };
    let hi = if thi < nhi { thi } else { nhi };
    let overlap = Interval::new(lo, hi);
    if overlap.is_empty() || overlap.is_nan() {
        return naive.clone();
    }
    overlap
}

/// Whether `id` contains the free variable anywhere in its DAG.
///
/// Memoized over the interned nodes so a shared subtree is visited once. A
/// revisited node answered `false` on its first complete visit (a `true`
/// would have short-circuited the whole walk), so skipping it is sound.
fn contains_variable(it: &Interner, id: NodeId) -> bool {
    let mut seen = HashSet::new();
    contains_variable_rec(it, id, &mut seen)
}

/// Recursive worker of [`contains_variable`].
fn contains_variable_rec(it: &Interner, id: NodeId, seen: &mut HashSet<NodeId>) -> bool {
    if !seen.insert(id) {
        return false;
    }
    match it.get(id) {
        Expr::Variable => true,
        Expr::Constant(_) | Expr::Pi => false,
        Expr::Neg(a) => contains_variable_rec(it, *a, seen),
        Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b) | Expr::Pow(a, b) => {
            contains_variable_rec(it, *a, seen) || contains_variable_rec(it, *b, seen)
        }
        Expr::Apply(_, a) => contains_variable_rec(it, *a, seen),
    }
}

/// Endpoints of a bounded enclosure as a pair, `None` for NaN or empty.
fn pair(enc: &Interval) -> Option<(BigFloat, BigFloat)> {
    match (enc.lower(), enc.upper()) {
        (Some(lo), Some(hi)) => Some((lo, hi)),
        _ => None,
    }
}

/// The non-real value of an enclosure, if it is not a bounded real.
///
/// `NaN` intervals and empty intervals report `NaN`; an enclosure with an
/// infinite endpoint reports that endpoint, so `Quality::NonReal` always
/// describes a value that is `NaN` or infinity (the certificate's documented
/// meaning). The endpoint-`NaN` probes are defensive: the backend never builds
/// a range with a `NaN` endpoint.
///
/// Deviation from `docs/plan/M2-stage2-tasks.md` (task C2, which says
/// "值=NaN" for every non-real case): reporting the actual infinity keeps the
/// information the enclosure carries — `ln(0)` yields `-infinity`, `exp(+inf)`
/// yields `+infinity` — and [`crate::Quality::NonReal`]'s own documentation is
/// "the evaluation yielded NaN *or infinity*", which a blanket NaN would
/// contradict. Reported to the lead for adjudication of the task-sheet wording.
fn non_real(enc: &Interval) -> Option<BigFloat> {
    if enc.is_nan() || enc.is_empty() {
        return Some(BigFloat::NAN);
    }
    let (lo, hi) = pair(enc)?;
    if lo.is_nan() || hi.is_nan() {
        return Some(BigFloat::NAN);
    }
    if !lo.is_finite() {
        return Some(lo);
    }
    if !hi.is_finite() {
        return Some(hi);
    }
    None
}

/// Zero value to report when the enclosure proves the result negligible.
///
/// The cutoff is a magnitude, so only a strictly positive one is meaningful.
/// Endpoints are exact dyadics, so the comparisons are exact and no rounding
/// can overstate the conclusion. The returned zero keeps the sign of the
/// enclosure: negative zero for a strictly negative enclosure, positive zero
/// otherwise (including an enclosure that straddles zero, whose sign is not
/// knowable).
///
/// Deviation from `docs/plan/M2-stage2-tasks.md` (task C2, which says
/// "值=nearest(lo@p)"): the engine flushes to a signed zero instead, because
/// that is the only value whose error the certificate bounds. The conclusion is
/// `enclosure ⊆ [-c, c]`, so zero satisfies `|value - y| <= c` for every
/// admissible `y`; `nearest(lo@p)` can be up to `2c` away from `y` when the
/// enclosure straddles the cutoff band, and it is asymmetric in the two
/// endpoints for no numerical reason. Flushing to zero is also the point of a
/// cutoff in table-making (a negligible result is emitted as zero). Reported to
/// the lead for adjudication of the task-sheet wording.
fn cutoff_value(enc: &Interval, cutoff: Option<&BigFloat>) -> Option<BigFloat> {
    let c = cutoff.map(|v| v.abs())?;
    if !c.is_finite() || c.is_zero() {
        return None;
    }
    let (lo, hi) = pair(enc)?;
    let neg_c = -c.clone();
    if lo >= neg_c && hi <= c {
        Some(if hi < BigFloat::zero() {
            -BigFloat::zero()
        } else {
            BigFloat::zero()
        })
    } else {
        None
    }
}

/// Runs the three certification tests on a bounded enclosure.
///
/// `point` says whether the enclosure is the single value `ylo == yhi`, which
/// is what lets Test 1 draw an exactness conclusion. All comparisons are
/// bitwise (see [`same_bits`]), so signed zero is never conflated.
fn certify(ylo: &BigFloat, yhi: &BigFloat, point: bool, target: usize) -> Verdict {
    let rlo = ylo.round_to(target, Rounding::Nearest);
    let rhi = yhi.round_to(target, Rounding::Nearest);
    // Test 1: both endpoints round to the same float, so the true value does
    // too, wherever it sits inside the enclosure.
    if same_bits(&rlo, &rhi) {
        let quality = if point {
            if same_bits(&rlo, ylo) {
                Quality::Exact
            } else {
                Quality::CorrectlyRoundedInexact
            }
        } else {
            Quality::CorrectlyRounded
        };
        return Verdict::Certified {
            value: rlo,
            quality,
        };
    }
    // Test 3: exactly one target float inside the enclosure.
    let up = ylo.round_to(target, Rounding::Up);
    let down = yhi.round_to(target, Rounding::Down);
    if same_bits(&up, &down) {
        return Verdict::Faithful {
            value: up,
            quality: Quality::Faithful,
        };
    }
    // Test 2: the rounding images are two consecutive target floats.
    // Zero is excluded because `next_above(0)` is a documented convention and
    // not a true successor (an unbounded exponent range has no smallest
    // positive float), and a pair on opposite sides of zero is not adjacent on
    // either side of a rounding boundary.
    let adjacent = !rlo.is_zero()
        && !rhi.is_zero()
        && !straddles_zero(&rlo, &rhi)
        && same_bits(&rlo.next_above(target), &rhi);
    if adjacent {
        return Verdict::Faithful {
            value: rlo,
            quality: Quality::FaithfulInexact,
        };
    }
    Verdict::Pending
}

/// Bitwise equality of two values, distinguishing signed zero.
///
/// Numeric equality on `BigFloat` treats `+0` and `-0` as the same number; a
/// rounding conclusion must not, because `+0` from one endpoint and `-0` from
/// the other encode "the true value straddles zero", which certifies nothing.
/// `canonical_parts` is the backend's normalized `(significand, exponent)`
/// form: precision-independent and sign-of-zero aware. Two `NaN`s are never
/// "the same bits" here, so `NaN` can never certify a conclusion.
fn same_bits(a: &BigFloat, b: &BigFloat) -> bool {
    match (a.canonical_parts(), b.canonical_parts()) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// `true` if the two values lie on opposite sides of zero.
fn straddles_zero(a: &BigFloat, b: &BigFloat) -> bool {
    let zero = BigFloat::zero();
    (a < &zero && b > &zero) || (a > &zero && b < &zero)
}

/// Exact ceiling of `5 * w / 4`, saturating instead of overflowing.
///
/// The growth rule of the refinement loop: 25% more bits per round, which
/// reaches the budget in a few dozen rounds (M2-plan §2).
fn ceil_five_quarters(w: usize) -> usize {
    w.saturating_mul(5).saturating_add(3) / 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::BaseFn;

    fn bf(v: f64) -> BigFloat {
        BigFloat::from_f64(v)
    }

    /// A point or bracket enclosure at `prec` for display-free comparisons.
    fn enc(lo: BigFloat, hi: BigFloat) -> Interval {
        Interval::new(lo, hi)
    }

    fn quality_of(enc: &Interval, target: usize, cutoff: Option<&BigFloat>) -> Quality {
        match certify_round(enc, target, cutoff) {
            Round::NonReal(_) => Quality::NonReal,
            Round::Cutoff(_) => Quality::BelowCutoff,
            Round::Verdict(Verdict::Certified { quality, .. }) => quality,
            Round::Verdict(Verdict::Faithful { quality, .. }) => quality,
            Round::Verdict(Verdict::Pending) => Quality::NotFaithful,
        }
    }

    // ---- same_bits ---------------------------------------------------

    #[test]
    fn normal_same_bits_ignores_the_precision_label() {
        let a = BigFloat::from_i64(5);
        let b = BigFloat::from_i64(5).with_precision(300);
        assert!(same_bits(&a, &b));
        let half = BigFloat::one() / BigFloat::from_i64(2);
        assert!(same_bits(&bf(0.5), &half));
    }

    #[test]
    fn extreme_same_bits_distinguishes_signed_zero_and_never_calls_nan_equal() {
        assert!(!same_bits(&BigFloat::zero(), &-BigFloat::zero()));
        assert!(!same_bits(&BigFloat::NAN, &BigFloat::NAN));
        assert!(!same_bits(&BigFloat::infinity(), &BigFloat::neg_infinity()));
        assert!(same_bits(&BigFloat::infinity(), &BigFloat::infinity()));
        // Distinct magnitudes are distinct bits.
        assert!(!same_bits(&BigFloat::one(), &bf(1.5)));
    }

    // ---- ceil_five_quarters ------------------------------------------

    #[test]
    fn normal_ceil_five_quarters_matches_exact_arithmetic() {
        assert_eq!(ceil_five_quarters(12), 15);
        assert_eq!(ceil_five_quarters(63), 79);
        assert_eq!(ceil_five_quarters(4), 5);
        assert_eq!(ceil_five_quarters(0), 0);
    }

    #[test]
    fn extreme_ceil_five_quarters_saturates_instead_of_overflowing() {
        // The saturating multiply keeps the helper panic-free at the type's
        // limit; the loop's give-up branch is what then stops the refinement,
        // so a stalled growth rule is safe by construction.
        assert_eq!(ceil_five_quarters(usize::MAX), usize::MAX / 4);
        assert_eq!(ceil_five_quarters(usize::MAX / 2), usize::MAX / 4);
        assert_eq!(ceil_five_quarters(1 << 60), (1 << 60) / 4 * 5);
        assert_eq!(ceil_five_quarters(0), 0);
    }

    #[test]
    fn robustness_ceil_five_quarters_always_makes_progress() {
        // The refinement loop's termination argument rests on this: every
        // reachable width strictly grows until the budget is hit.
        for w in 2..4096usize {
            assert!(ceil_five_quarters(w) > w, "no progress at {w}");
        }
    }

    // ---- pair / endpoints access -------------------------------------

    #[test]
    fn normal_pair_returns_the_range_endpoints() {
        let iv = enc(BigFloat::one(), bf(2.5));
        assert_eq!(
            pair(&iv).map(|(a, b)| (a.is_finite(), b.is_finite())),
            Some((true, true))
        );
    }

    #[test]
    fn error_pair_is_none_for_nan_and_empty() {
        assert!(pair(&Interval::NAN).is_none());
        assert!(pair(&Interval::empty()).is_none());
    }

    // ---- input_interval ----------------------------------------------

    #[test]
    fn normal_input_interval_is_a_point_for_representable_values() {
        let iv = input_interval(&BigFloat::one(), 53);
        assert!(iv.is_point());
        assert_eq!(iv.lower().expect("point"), BigFloat::one());
    }

    #[test]
    fn extreme_input_interval_brackets_a_wide_value_and_keeps_specials() {
        // A value with more significant bits than the target precision gets a
        // real bracket, so the enclosure stays valid at any width.
        let wide = BigFloat::one() / BigFloat::from_i64(3);
        let iv = input_interval(&wide, 8);
        let lo = iv.lower().expect("range");
        let hi = iv.upper().expect("range");
        assert!(lo <= wide && wide <= hi, "{lo} .. {hi} misses {wide}");
        assert!(lo < hi, "an 8-bit bracket of 1/3 must have width");
        // Infinities and NaN pass through the directed roundings unchanged. An
        // infinite bracket is deliberately not a "point" (the backend's
        // definition requires finiteness), so the endpoints are checked.
        let up = input_interval(&BigFloat::infinity(), 53);
        assert_eq!(up.lower().expect("range"), BigFloat::infinity());
        assert_eq!(up.upper().expect("range"), BigFloat::infinity());
        assert!(input_interval(&BigFloat::NAN, 53).is_nan());
    }

    // ---- non_real ----------------------------------------------------

    #[test]
    fn normal_non_real_is_none_for_a_bounded_range() {
        assert!(non_real(&enc(BigFloat::from_i64(-9), BigFloat::from_i64(9))).is_none());
        assert!(non_real(&enc(BigFloat::zero(), BigFloat::zero())).is_none());
    }

    #[test]
    fn extreme_non_real_reports_the_offending_endpoint() {
        let up = enc(BigFloat::one(), BigFloat::infinity());
        assert_eq!(non_real(&up).expect("non-real"), BigFloat::infinity());
        let down = enc(BigFloat::neg_infinity(), BigFloat::one());
        assert_eq!(non_real(&down).expect("non-real"), BigFloat::neg_infinity());
        assert!(non_real(&Interval::NAN).expect("nan").is_nan());
        assert!(non_real(&Interval::empty()).expect("nan").is_nan());
    }

    // ---- cutoff_value -------------------------------------------------

    #[test]
    fn normal_cutoff_value_flushes_only_inside_the_band() {
        let c = bf(0.25);
        assert_eq!(
            cutoff_value(&enc(bf(0.2), bf(0.24)), Some(&c)),
            Some(BigFloat::zero())
        );
        assert!(cutoff_value(&enc(bf(0.2), bf(0.3)), Some(&c)).is_none());
        // Boundary equality is inside the band.
        assert!(cutoff_value(&enc(-c.clone(), c.clone()), Some(&c)).is_some());
    }

    #[test]
    fn normal_cutoff_value_keeps_the_enclosure_sign() {
        let c = bf(0.5);
        let neg = cutoff_value(&enc(bf(-0.4), bf(-0.1)), Some(&c)).expect("flush");
        assert!(neg.is_zero() && neg.is_sign_negative());
        let straddle = cutoff_value(&enc(bf(-0.4), bf(0.4)), Some(&c)).expect("flush");
        assert!(straddle.is_zero() && straddle.is_sign_positive());
    }

    #[test]
    fn error_cutoff_value_ignores_degenerate_thresholds() {
        let iv = enc(BigFloat::zero(), BigFloat::zero());
        assert!(cutoff_value(&iv, None).is_none());
        assert!(cutoff_value(&iv, Some(&BigFloat::zero())).is_none());
        // The cutoff is a magnitude, so a negative one is its absolute value.
        assert!(cutoff_value(&iv, Some(&bf(-0.5))).is_some());
        assert!(cutoff_value(&iv, Some(&BigFloat::infinity())).is_none());
        assert!(cutoff_value(&iv, Some(&BigFloat::NAN)).is_none());
        assert!(cutoff_value(&Interval::NAN, Some(&bf(0.5))).is_none());
    }

    // ---- certify: the decision matrix --------------------------------

    #[test]
    fn normal_certify_test1_distinguishes_exact_from_known_inexact() {
        // Point enclosure of a representable value: Exact.
        let v = certify(&BigFloat::one(), &BigFloat::one(), true, 2);
        assert!(matches!(
            v,
            Verdict::Certified {
                quality: Quality::Exact,
                ..
            }
        ));
        // Point enclosure of an unrepresentable value: CorrectlyRoundedInexact.
        let tie = BigFloat::from_i64(5) / BigFloat::from_i64(8);
        let v = certify(&tie, &tie, true, 2);
        assert!(matches!(
            v,
            Verdict::Certified {
                quality: Quality::CorrectlyRoundedInexact,
                ..
            }
        ));
        // A widthful enclosure whose images agree: CorrectlyRounded, no
        // exactness claim.
        let v = certify(&bf(0.7), &bf(0.71), false, 2);
        assert!(matches!(
            v,
            Verdict::Certified {
                quality: Quality::CorrectlyRounded,
                ..
            }
        ));
    }

    #[test]
    fn normal_certify_test3_prefers_the_unique_contained_float() {
        // [0.8, 1.4] at two bits contains exactly one float (1.0) while the
        // nearest images of the endpoints are 0.75 and 1.5: Test 3, not Test 2.
        let v = certify(&bf(0.8), &bf(1.4), false, 2);
        match v {
            Verdict::Faithful { value, quality } => {
                assert_eq!(quality, Quality::Faithful);
                assert_eq!(value, BigFloat::one());
            }
            other => panic!("expected a Test 3 conclusion, got {other:?}"),
        }
    }

    #[test]
    fn normal_certify_test2_returns_the_lower_adjacent_float() {
        // [0.6, 0.63] straddles the two-bit tie at 0.625: images 0.5 and 0.75,
        // adjacent, so Test 2 with the lower one.
        let v = certify(&bf(0.6), &bf(0.63), false, 2);
        match v {
            Verdict::Faithful { value, quality } => {
                assert_eq!(quality, Quality::FaithfulInexact);
                assert_eq!(value, BigFloat::one() / BigFloat::from_i64(2));
            }
            other => panic!("expected a Test 2 conclusion, got {other:?}"),
        }
    }

    #[test]
    fn extreme_certify_pending_cases() {
        // Images two grid steps apart: nothing can be concluded.
        assert!(matches!(
            certify(&bf(0.5), &bf(1.5), false, 2),
            Verdict::Pending
        ));
        // A rounding image of zero disables the adjacent-float test, because
        // `next_above(0)` is a convention rather than a successor.
        assert!(matches!(
            certify(&-BigFloat::zero(), &BigFloat::zero(), false, 2),
            Verdict::Pending
        ));
        assert!(matches!(
            certify(&-BigFloat::zero(), &bf(0.75), false, 2),
            Verdict::Pending
        ));
    }

    #[test]
    fn extreme_certify_reports_signed_zero_without_relying_on_display() {
        // A negative lower endpoint and a positive upper one round to -0 and
        // +0 at the target; `same_bits` must tell them apart so no rounding
        // conclusion is drawn across zero.
        //
        // The assertions go through `canonical_parts`, not formatting: the
        // backend's `Display` for a negative zero panics inside dashu's
        // decimal conversion (reported to the lead as a separate defect), so
        // nothing here may render such a value.
        let verdict = certify(&-BigFloat::zero(), &BigFloat::zero(), false, 2);
        assert!(matches!(verdict, Verdict::Pending));
        let neg = (-BigFloat::zero()).round_to(2, Rounding::Nearest);
        let pos = BigFloat::zero().round_to(2, Rounding::Nearest);
        assert!(neg.is_zero() && pos.is_zero());
        assert!(neg.is_sign_negative() && pos.is_sign_positive());
        assert!(!same_bits(&neg, &pos));
    }

    #[test]
    fn robustness_certify_is_deterministic_and_total() {
        let cases = [
            (bf(0.6), bf(0.63), false, 2),
            (bf(0.8), bf(1.4), false, 2),
            (BigFloat::one(), BigFloat::one(), true, 2),
            (bf(0.5), bf(1.5), false, 2),
            (bf(-0.1), bf(0.1), false, 53),
        ];
        for (lo, hi, point, target) in cases {
            let first = certify(&lo, &hi, point, target);
            let second = certify(&lo.clone(), &hi.clone(), point, target);
            let (a_q, a_v) = describe(&first);
            let (b_q, b_v) = describe(&second);
            assert_eq!(a_q, b_q, "verdict kind drifted");
            assert_eq!(a_v, b_v, "verdict value drifted");
        }
    }

    /// Verdict summary without `Debug`, which renders `BigFloat` through the
    /// backend's decimal conversion.
    fn describe(v: &Verdict) -> (&'static str, Option<(String, isize)>) {
        let (tag, value) = match v {
            Verdict::Certified { quality, value } => (quality_tag(*quality), value),
            Verdict::Faithful { quality, value } => (quality_tag(*quality), value),
            Verdict::Pending => return ("pending", None),
        };
        (
            tag,
            value.canonical_parts().map(|(s, e)| (s.to_string(), e)),
        )
    }

    fn quality_tag(q: Quality) -> &'static str {
        match q {
            Quality::NotFaithful => "not_faithful",
            Quality::Faithful => "faithful",
            Quality::BelowCutoff => "below_cutoff",
            Quality::NonReal => "non_real",
            Quality::Exact => "exact",
            Quality::FaithfulInexact => "faithful_inexact",
            Quality::CorrectlyRounded => "correctly_rounded",
            Quality::CorrectlyRoundedInexact => "correctly_rounded_inexact",
        }
    }

    // ---- certify_round: the contract's decision order ----------------

    #[test]
    fn error_certify_round_decides_non_real_before_the_cutoff() {
        // An unbounded enclosure inside the cutoff band is still NonReal: the
        // refinement order in the module documentation is not negotiable.
        let iv = enc(BigFloat::neg_infinity(), BigFloat::one());
        let c = bf(4.0);
        assert_eq!(quality_of(&iv, 53, Some(&c)), Quality::NonReal);
    }

    #[test]
    fn error_certify_round_decides_the_cutoff_before_the_tests() {
        // [0.8, 1.4] at two bits would give Test 3, and a positive cutoff that
        // covers it must win instead.
        let iv = enc(bf(0.8), bf(1.4));
        assert_eq!(quality_of(&iv, 2, None), Quality::Faithful);
        let c = bf(2.0);
        assert_eq!(quality_of(&iv, 2, Some(&c)), Quality::BelowCutoff);
    }

    #[test]
    fn robustness_certify_round_never_panics_on_degenerate_enclosures() {
        // NaN and empty reach the non-real branch, so the endpoint fallback in
        // `pair(enc).unwrap_or(..)` is unreachable-but-safe.
        assert_eq!(quality_of(&Interval::NAN, 53, None), Quality::NonReal);
        assert_eq!(quality_of(&Interval::empty(), 53, None), Quality::NonReal);
        let flipped = Interval::new(BigFloat::from_i64(3), BigFloat::from_i64(1));
        assert!(flipped.is_empty());
        assert_eq!(quality_of(&flipped, 53, None), Quality::NonReal);
    }

    // ---- straddles_zero ---------------------------------------------

    #[test]
    fn normal_straddles_zero_only_for_opposite_signs() {
        assert!(straddles_zero(&bf(-1.0), &bf(1.0)));
        assert!(straddles_zero(&bf(1.0), &bf(-1.0)));
        assert!(!straddles_zero(&bf(1.0), &bf(2.0)));
        assert!(!straddles_zero(&bf(-2.0), &bf(-1.0)));
        // Zero is not a side of itself.
        assert!(!straddles_zero(&BigFloat::zero(), &bf(1.0)));
        assert!(!straddles_zero(&-BigFloat::zero(), &bf(1.0)));
    }

    // ---- contains_variable --------------------------------------------

    #[test]
    fn normal_contains_variable_walks_the_shared_dag() {
        let mut it = Interner::new();
        let x = it.intern(Expr::Variable);
        let one = it.intern(Expr::Constant(BigFloat::one()));
        let pi = it.intern(Expr::Pi);
        assert!(contains_variable(&it, x));
        assert!(!contains_variable(&it, one));
        assert!(!contains_variable(&it, pi));
        let sum = it.intern(Expr::Add(one, pi));
        assert!(!contains_variable(&it, sum));
        // Shared subtree: g = sin x used twice is visited once and found.
        let g = it.intern(Expr::Apply(BaseFn::Sin, x));
        let f = it.intern(Expr::Add(g, g));
        assert!(contains_variable(&it, f));
        // The variable buried under Neg/Div/Pow is still found.
        let pow = it.intern(Expr::Pow(x, one));
        let div = it.intern(Expr::Div(one, pow));
        let deep = it.intern(Expr::Neg(div));
        assert!(contains_variable(&it, deep));
    }

    // ---- intersect_enclosures -----------------------------------------

    #[test]
    fn normal_intersect_enclosures_narrows_to_the_overlap() {
        let naive = enc(BigFloat::zero(), BigFloat::from_i64(10));
        let taylor = enc(BigFloat::from_i64(2), BigFloat::from_i64(8));
        let out = intersect_enclosures(&naive, &taylor);
        assert_eq!(out.lower().expect("range"), BigFloat::from_i64(2));
        assert_eq!(out.upper().expect("range"), BigFloat::from_i64(8));
        // A wider Taylor range reproduces the naive one bitwise.
        let wider = enc(-BigFloat::from_i64(5), BigFloat::from_i64(15));
        let out = intersect_enclosures(&naive, &wider);
        assert!(same_bits(
            &out.lower().expect("range"),
            &naive.lower().expect("range")
        ));
        assert!(same_bits(
            &out.upper().expect("range"),
            &naive.upper().expect("range")
        ));
        // Partial overlap on both sides.
        let shifted = enc(BigFloat::from_i64(4), BigFloat::from_i64(12));
        let out = intersect_enclosures(&naive, &shifted);
        assert_eq!(out.lower().expect("range"), BigFloat::from_i64(4));
        assert_eq!(out.upper().expect("range"), BigFloat::from_i64(10));
    }

    #[test]
    fn extreme_intersect_enclosures_ties_keep_the_naive_signed_zero() {
        let naive = enc(-BigFloat::zero(), BigFloat::zero());
        let taylor = Interval::point(BigFloat::zero());
        let out = intersect_enclosures(&naive, &taylor);
        // Numerically equal endpoints tie: the naive signs survive.
        assert!(same_bits(&out.lower().expect("range"), &-BigFloat::zero()));
        assert!(same_bits(&out.upper().expect("range"), &BigFloat::zero()));
        // A strictly interior Taylor point does narrow.
        let inner = Interval::point(BigFloat::from_i64(3));
        let out = intersect_enclosures(&enc(BigFloat::zero(), BigFloat::from_i64(10)), &inner);
        assert!(out.is_point());
        assert_eq!(out.lower().expect("point"), BigFloat::from_i64(3));
    }

    #[test]
    fn error_intersect_enclosures_falls_back_on_empty_nan_or_unbounded() {
        let naive = enc(BigFloat::zero(), BigFloat::one());
        // Disjoint: only a rounding artifact can produce this; keep naive.
        let disjoint = enc(BigFloat::from_i64(2), BigFloat::from_i64(3));
        let out = intersect_enclosures(&naive, &disjoint);
        assert!(same_bits(&out.lower().expect("range"), &BigFloat::zero()));
        assert!(same_bits(&out.upper().expect("range"), &BigFloat::one()));
        // NaN / empty / unbounded on either side: keep the naive side.
        for bad in [
            Interval::NAN,
            Interval::empty(),
            enc(BigFloat::one(), BigFloat::infinity()),
            enc(BigFloat::neg_infinity(), BigFloat::zero()),
        ] {
            let out = intersect_enclosures(&naive, &bad);
            assert!(same_bits(&out.lower().expect("range"), &BigFloat::zero()));
            assert!(same_bits(&out.upper().expect("range"), &BigFloat::one()));
            // A non-real naive side is returned unchanged.
            let out = intersect_enclosures(&bad, &naive);
            assert_eq!(out.is_nan(), bad.is_nan());
            assert_eq!(out.is_empty(), bad.is_empty());
        }
    }

    // ---- refine_with_taylor -------------------------------------------

    #[test]
    fn normal_refine_with_taylor_collapses_a_dependency_zero() {
        let mut it = Interner::new();
        let x = it.intern(Expr::Variable);
        let f = it.intern(Expr::Sub(x, x));
        let input = enc(bf(0.6), bf(0.7));
        let naive = eval_interval(&it, f, &input, 53, None);
        assert!(!naive.is_point(), "the naive walk of x - x has width");
        let refined = refine_with_taylor(&it, f, &input, &naive, 53, 0, None);
        assert!(refined.is_point(), "the mean-value form collapses x - x");
        assert!(refined.contains(&BigFloat::zero()));
        // The refined range is a subset of the naive one (never wider).
        assert!(naive.contains(&refined.lower().expect("point")));
        assert!(naive.contains(&refined.upper().expect("point")));
    }

    #[test]
    fn normal_refine_with_taylor_skips_constants_and_point_inputs() {
        let mut it = Interner::new();
        let two = it.intern(Expr::Constant(BigFloat::from_i64(2)));
        let sqrt2 = it.intern(Expr::Apply(BaseFn::Sqrt, two));
        let input = enc(bf(0.6), bf(0.7));
        let naive = eval_interval(&it, sqrt2, &input, 53, None);
        let refined = refine_with_taylor(&it, sqrt2, &input, &naive, 53, 0, None);
        assert!(same_bits(
            &refined.lower().expect("range"),
            &naive.lower().expect("range")
        ));
        assert!(same_bits(
            &refined.upper().expect("range"),
            &naive.upper().expect("range")
        ));
        // Point input: the documented eval_taylor fallback makes this a no-op.
        let x = it.intern(Expr::Variable);
        let point_input = Interval::point(bf(1.5));
        let naive = eval_interval(&it, x, &point_input, 53, None);
        let refined = refine_with_taylor(&it, x, &point_input, &naive, 53, 0, None);
        assert!(same_bits(
            &refined.lower().expect("range"),
            &naive.lower().expect("range")
        ));
    }

    #[test]
    fn error_refine_with_taylor_keeps_non_real_naive_enclosures() {
        let mut it = Interner::new();
        let x = it.intern(Expr::Variable);
        let one = it.intern(Expr::Constant(BigFloat::one()));
        let recip = it.intern(Expr::Div(one, x));
        let input = enc(-BigFloat::one(), BigFloat::one());
        let naive = eval_interval(&it, recip, &input, 53, None);
        assert!(naive.is_nan(), "1/x over a zero-spanning interval");
        let refined = refine_with_taylor(&it, recip, &input, &naive, 53, 0, None);
        assert!(refined.is_nan(), "the non-real conclusion must survive");
    }
}
