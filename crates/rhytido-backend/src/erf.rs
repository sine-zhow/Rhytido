// SPDX-License-Identifier: Apache-2.0
//! Certified `erf` and `erfc`.
//!
//! Implements the algorithm of `docs/research/erf-erfc-design.md`: the
//! everywhere-convergent Taylor series
//!
//! ```text
//! erf(x) = (2/sqrt(pi)) * sum_{n>=0} (-1)^n x^(2n+1) / (n! (2n+1))
//! ```
//!
//! truncated with a *geometric* remainder bound, driven at a working precision
//! `P = p + 3*ceil(x^2) + guard` that absorbs the cancellation of
//! `erfc = 1 - erf` (design doc section 3), plus a large-argument threshold
//! derived from the strict bound `erfc(x) <= exp(-x^2)/(x*sqrt(pi))` (design
//! doc section 1).
//!
//! Correctness model: every elementary operation runs at the working precision
//! with the backend's single nearest rounding, so it contributes at most half an
//! ulp, i.e. a relative error of `2^-prec`. Margins below are written as exact
//! powers of two (`pow2_below(prec, k) = 2^(k-prec)`, i.e. `2^k` ulps) chosen
//! to dominate those errors, and every one-sided bound finishes with a directed
//! rounding so it can only move outward. Each bound is derived where it is used.
//! No `unsafe`, and no backend type escapes the facade.

use crate::{BigFloat, DEFAULT_PRECISION, Integer, Interval, Rounding};

/// Guard bits on top of the cancellation allowance `3*ceil(x^2)` (design doc
/// section 3 recommends 10-20).
const GUARD_BITS: usize = 20;

/// Guard bits added per escalation round while the certificate does not yet
/// prove correct rounding at the target precision (Ziv loop, design doc
/// section 4 step 2).
const GUARD_STEP_BITS: usize = 20;

/// Maximum number of escalation rounds. Past the cap a conservative value is
/// returned; the (larger) proven bound is what `*_checked` reports, so the
/// certificate invariant never overstates the accuracy.
const MAX_GUARD_RETRIES: usize = 4;

/// Guard bits used for the pi bounds, mirroring `interval.rs`.
const PI_GUARD_BITS: usize = 10;

/// Log2 of the power-of-two slack folded into the series rounding budget.
///
/// The budget counts the rounded operations explicitly (see [`series_bound`]);
/// this slack additionally pays for the `(1 - gamma)^-1` factor of the standard
/// perturbation model, for the perturbation of the rounded product `x^2` that
/// the term recurrence uses (relative `n * 2^(1-prec)`), and for the gap between
/// a computed and the exact magnitude of the first omitted term. It is kept
/// small on purpose: `erfc` needs *relative* accuracy after the cancellation,
/// and every slack bit costs one bit of that accuracy.
const BOUND_SLACK_BITS: usize = 6;

/// Highest working precision used for a single attempt.
const MAX_WORKING_PRECISION: usize = 1 << 20;

/// Highest series length allowed for a single attempt.
const MAX_SERIES_TERMS: usize = 1 << 20;

/// Highest exponent materialised by the exact threshold test; beyond it the fast
/// path is refused, which only costs the (still correct) series path.
const MAX_THRESHOLD_SHIFT: i64 = 1 << 16;

/// A rigorous bracket `[lo, hi]` of one exact value, both endpoints at the
/// working precision `prec`.
///
/// Brackets — not point estimates — are what make the certificate auditable:
/// every derivation step widens outward, so containment of the exact value is
/// preserved by construction, and the width directly yields the error bound.
struct Bracket {
    lo: BigFloat,
    hi: BigFloat,
    prec: usize,
}

impl Bracket {
    /// An upper bound on the bracket width `hi - lo`.
    ///
    /// The subtraction carries one nearest rounding (relative `2^-prec`), so the
    /// result is widened by four ulps and rounded up; the width is at least the
    /// distance between two `prec`-bit endpoints, hence the widening cannot make
    /// it negative.
    fn width(&self) -> BigFloat {
        let w = (self.hi.clone() - self.lo.clone()).with_precision(self.prec);
        let widen = (w.abs() * pow2_below(self.prec, 2)).round_to(self.prec, Rounding::Up);
        (w + widen).round_to(self.prec, Rounding::Up)
    }

    /// `Test 1` of the certificate loop: both endpoints round to the same `p`-bit
    /// value. Nearest rounding is monotone, so every value in the bracket — in
    /// particular the exact one — then rounds to that same value, i.e. the result
    /// is correctly rounded.
    fn proves_rounding(&self, p: usize) -> bool {
        self.lo.round_to(p, Rounding::Nearest) == self.hi.round_to(p, Rounding::Nearest)
    }

    /// The `p`-bit value with its strict absolute error bound.
    ///
    /// The value is the lower endpoint re-rounded to `p`. If the bracket proves
    /// correct rounding, the exact value lies within half an ulp of it, which
    /// [`rounding_allowance`] bounds from above (and strictly below one ulp, so
    /// the certificate stays finer than the value's own resolution). Otherwise
    /// the bracket width is added, which remains valid but coarser — that is how
    /// an exhausted retry budget shows up in the reported bound.
    fn value_and_bound(&self, p: usize) -> (BigFloat, BigFloat) {
        let value = self.lo.round_to(p, Rounding::Nearest);
        let allowance = rounding_allowance(&value, p, self.prec);
        if self.proves_rounding(p) {
            (value, allowance)
        } else {
            (
                value.clone(),
                outward(&self.width(), &allowance, self.prec, true),
            )
        }
    }
}

/// Working precision of an operand: `precision()` with the same unlimited (`0`)
/// fallback used by `interval.rs` and `make_range`.
fn work_prec(x: &BigFloat) -> usize {
    match x.precision() {
        0 => DEFAULT_PRECISION,
        p => p,
    }
}

/// Exact `2^exponent` as a `BigFloat` (a one-bit dyadic, never rounded).
fn pow2_int(exponent: usize) -> BigFloat {
    BigFloat::from_integer(Integer::from(2).pow(exponent))
}

/// Exact integer `2^shift`.
fn pow2_integer(shift: usize) -> Integer {
    Integer::from(2).pow(shift)
}

/// Exact `2^(k - prec)` as a `BigFloat`.
///
/// Both `2^k` and `2^prec` are one-bit dyadics, so the quotient is exact at any
/// precision; this is the canonical way to write a margin of `2^k` ulps of a
/// `prec`-bit quantity.
fn pow2_below(prec: usize, k: usize) -> BigFloat {
    if k >= prec {
        pow2_int(k - prec)
    } else {
        pow2_int(k) / pow2_int(prec)
    }
}

/// Exact `2^-exponent`, the positive certificate returned for values attained
/// exactly (special values and the large-argument threshold).
fn pow2_neg(exponent: usize) -> BigFloat {
    pow2_below(exponent, 0)
}

/// Integer `value` labelled at precision `prec` (exact: small integers need few
/// bits, so the relabelling does not round).
fn exact_int(value: i64, prec: usize) -> BigFloat {
    BigFloat::from_i64(value).with_precision(prec)
}

/// Rounding allowance for a value rounded to `p` bits: `|v| * 2^-p`, exact
/// because the factor is a power of two.
///
/// Derivation: with `2^E <= |v| < 2^(E+1)`, one ulp at precision `p` is
/// `2^(E+1-p)`, so `|v| * 2^-p` lies in `[2^(E-p), 2^(E+1-p))` — at least half
/// an ulp, hence an upper bound on the nearest-rounding error, and strictly
/// below one ulp, hence a certificate finer than the value's resolution. A zero
/// value can only arise from an exactly zero result, where the positive floor
/// `2^-prec` keeps the bound's sign contract.
fn rounding_allowance(v: &BigFloat, p: usize, prec: usize) -> BigFloat {
    let allowance = (v.abs() * pow2_neg(p)).round_to(prec, Rounding::Up);
    if allowance.is_zero() {
        pow2_below(prec, 0)
    } else {
        allowance
    }
}

/// Smallest `k` with `2^k >= v` for `v >= 1` (integer ceiling of log2).
fn log2_ceil(v: usize) -> usize {
    if v <= 1 {
        0
    } else {
        usize::BITS as usize - (v - 1).leading_zeros() as usize
    }
}

/// Exact `usize` value of a non-negative, integer-valued `BigFloat`.
///
/// Works on the canonical `(significand, exponent)` parts, so no decimal or
/// binary rounding is involved; `None` when the value is negative, fractional
/// or too wide to fit.
fn to_usize_exact(v: &BigFloat) -> Option<usize> {
    if v.is_nan() || v.is_infinite() || !v.is_integer() {
        return None;
    }
    let (sig, exp) = v.canonical_parts()?;
    if sig.is_negative() {
        return None;
    }
    let shift = u32::try_from(exp).ok()?;
    let mantissa: u64 = sig.to_string().parse().ok()?;
    usize::try_from(mantissa.checked_shl(shift)?).ok()
}

/// `ceil(v)` as a `usize` for a non-negative finite `v`, saturating at
/// [`MAX_SERIES_TERMS`] so a pathological operand cannot request an absurd
/// number of terms.
fn ceil_index(v: &BigFloat) -> usize {
    match to_usize_exact(&v.ceil()) {
        Some(n) => n.min(MAX_SERIES_TERMS),
        None => MAX_SERIES_TERMS,
    }
}

/// Rigorous lower/upper bounds on pi for `prec`-bit arithmetic.
///
/// Derivation: `pi_with_precision(w)` is `4*atan(1)`; multiplying by 4 is an
/// exact exponent shift, so the whole error is that of the correctly rounded
/// `atan(1)`. Since `atan(1) = pi/4` lies in `[0.5, 1)`, half an ulp there is
/// `2^(-w-1)`, hence `|4*atan(1) - pi| <= 2^(1-w)`. The margin `2^(3-w)` is four
/// times larger, so it also absorbs the rounding of the subtraction and of the
/// label widening (`pi` lies in `[3, 4)`, so an ulp of the guard-precision value
/// is at most `2^(2-w)`); the final directed rounding only moves outward.
fn pi_bounds(prec: usize) -> (BigFloat, BigFloat) {
    let w = prec.saturating_add(PI_GUARD_BITS).max(8);
    let center = BigFloat::pi_with_precision(w);
    let eps = pow2_below(w, 3);
    let lo = (center.clone() - eps.clone()).round_to(w, Rounding::Down);
    let hi = (center + eps).round_to(w, Rounding::Up);
    (lo, hi)
}

/// Rigorous lower/upper bounds on `sqrt(pi)` at `prec` bits.
///
/// Derivation: the square root is increasing on the positive reals, so the pi
/// bounds carry through: `sqrt(pi_lo) <= sqrt(pi) <= sqrt(pi_hi)`. Each root is
/// formed with one nearest rounding at the guard precision `w >= prec` (relative
/// error at most `2^-w`, and `sqrt(pi)` lies in `[1, 2)`), while the widening
/// `2^(4-prec)` is at least eight ulps at `prec` and about `2^14` times the
/// guard-precision error; the directed rounding then only moves outward.
fn sqrt_pi_bounds(prec: usize) -> (BigFloat, BigFloat) {
    let (pi_lo, pi_hi) = pi_bounds(prec);
    let widen = pow2_below(prec, 4);
    let lo = (pi_lo.sqrt() - widen.clone()).round_to(prec, Rounding::Down);
    let hi = (pi_hi.sqrt() + widen).round_to(prec, Rounding::Up);
    (lo, hi)
}

/// Rigorous one-sided quotient bound: `num / den` at `prec` bits, returned as a
/// guaranteed upper bound (`up = true`) or lower bound.
///
/// Derivation: the backend forms the quotient with a single nearest rounding,
/// contributing at most half an ulp (relative `2^-prec`); widening by eight ulps
/// and rounding in the requested direction leaves a factor of at least four, and
/// the directed rounding itself only moves outward.
fn div_bound(num: &BigFloat, den: &BigFloat, prec: usize, up: bool) -> BigFloat {
    let q = (num.clone() / den.clone()).with_precision(prec);
    let widen = (q.abs() * pow2_below(prec, 4)).round_to(prec, Rounding::Up);
    if up {
        (q + widen).round_to(prec, Rounding::Up)
    } else {
        (q - widen).round_to(prec, Rounding::Down)
    }
}

/// Rigorous outward sum or difference: `v + delta` as an upper bound (`up`) or
/// `v - delta` as a lower bound, at `prec` bits.
///
/// Derivation: the backend adds with one nearest rounding, i.e. at most half an
/// ulp of the result; the absolute margin `2^(3-prec)` is eight ulps of a unit
/// magnitude and every quantity passed here is at most `2`, so the margin
/// dominates that rounding and the directed rounding only widens further.
fn outward(v: &BigFloat, delta: &BigFloat, prec: usize, up: bool) -> BigFloat {
    let margin = pow2_below(prec, 3);
    if up {
        (v.clone() + delta.clone() + margin).round_to(prec, Rounding::Up)
    } else {
        (v.clone() - delta.clone() - margin).round_to(prec, Rounding::Down)
    }
}

/// Rigorous upper bound of `a^2` for a positive finite operand, at `prec`.
///
/// Derivation: the product is formed with one nearest rounding (relative error
/// at most half an ulp, `2^-prec`), so adding four ulps of itself and rounding up
/// cannot land below the exact square.
fn square_upper(a: &BigFloat, prec: usize) -> BigFloat {
    let prec = prec.max(8);
    let prod = (a.clone() * a.clone()).with_precision(prec);
    let widen = (prod.clone() * pow2_below(prec, 2)).round_to(prec, Rounding::Up);
    (prod + widen).round_to(prec, Rounding::Up)
}

/// Rigorous decision of `a^2 >= t` for positive finite `a` and integral `t`.
///
/// The test must never fire on a *smaller* true square, because the caller then
/// replaces the value by its exact limit. Each step is sound on its own:
/// - `a < 1` implies `a^2 < 1 <= t` -> `false`;
/// - `a >= t` implies `a^2 >= t^2 >= t` for `t >= 1` -> `true`;
/// - otherwise `1 <= a < t` and the comparison `sig^2 * 2^(2e) >= t` on the
///   canonical dyadic parts `a = sig * 2^e` is an exact integer relation, so no
///   rounding can flip the verdict. Absurd shifts are refused (`false`).
fn square_at_least(a: &BigFloat, t: usize) -> bool {
    if t == 0 {
        return true;
    }
    let one = BigFloat::one();
    if a < &one {
        return false;
    }
    let Ok(t_i) = i64::try_from(t) else {
        return false;
    };
    if a >= &BigFloat::from_i64(t_i) {
        return true;
    }
    let Some((sig, exp)) = a.canonical_parts() else {
        return false;
    };
    if sig.is_zero() {
        return false;
    }
    let shift = 2i64.saturating_mul(exp as i64);
    if shift >= MAX_THRESHOLD_SHIFT {
        return true;
    }
    if -shift >= MAX_THRESHOLD_SHIFT {
        return false;
    }
    let sq = &sig * &sig;
    let t_int = Integer::from(t_i);
    if shift >= 0 {
        sq * pow2_integer(shift as usize) >= t_int
    } else {
        sq >= t_int * pow2_integer((-shift) as usize)
    }
}

/// One term of the Taylor series from its predecessor, design doc section 2:
/// `a_{n+1} = a_n * (-x^2) * (2n+1) / ((n+1)*(2n+3))`.
///
/// The integer factors are labelled at `prec` so every intermediate carries the
/// working precision; three rounded operations per step.
fn next_term(prev: &BigFloat, neg_x2: &BigFloat, n: usize, prec: usize) -> BigFloat {
    let odd = BigFloat::from_integer(Integer::from(2 * n as i64 + 1)).with_precision(prec);
    let den = BigFloat::from_integer(Integer::from(n as i64 + 1) * Integer::from(2 * n as i64 + 3))
        .with_precision(prec);
    let step = (prev.clone() * neg_x2.clone()).with_precision(prec);
    ((step * odd).with_precision(prec) / den).with_precision(prec)
}

/// `n`-th term of the series from the closed form `(-1)^n a^(2n+1)/(n!(2n+1))`.
///
/// Independent of [`next_term`]; used only by the unit layer to cross-check the
/// recurrence, which is why it builds the factorials and powers directly.
#[cfg(test)]
fn closed_form_term(a: &BigFloat, n: usize, prec: usize) -> BigFloat {
    let mut fact = Integer::from(1);
    for k in 1..=n {
        fact = fact * Integer::from(k as i64);
    }
    let den = BigFloat::from_integer(fact * Integer::from(2 * n as i64 + 1)).with_precision(prec);
    let mut num = exact_int(1, prec);
    for _ in 0..(2 * n + 1) {
        num = (num * a.clone()).with_precision(prec);
    }
    let q = (num / den).with_precision(prec);
    if n % 2 == 0 { q } else { -q }
}

/// Geometric remainder bound for truncation after index `n` (design doc
/// section 2), or `None` while the monotone-decrease precondition
/// `n + 2 > x2_up >= x^2` is not met.
///
/// Derivation: the exact term ratio is
/// `|a_{k+1}/a_k| = x^2 (2k+1)/((k+1)(2k+3)) <= x^2/(k+1)`, because
/// `2k+1 <= 2k+3`. For `k >= n+1` that ratio is bounded by `r = x2_up/(n+2) < 1`,
/// so the omitted tail is dominated by the geometric series
/// `|a_{n+1}| * (1 + r + r^2 + ...) = |a_{n+1}| / (1 - r)`. The returned value is
/// a rigorous upper bound: `r` is rounded up, `1 - r` rounded down, and the two
/// divisions go through [`div_bound`].
fn geometric_remainder(
    first_omitted: &BigFloat,
    x2_up: &BigFloat,
    n: usize,
    prec: usize,
) -> Option<BigFloat> {
    let denom = BigFloat::from_integer(Integer::from(n as i64 + 2));
    if denom <= *x2_up {
        return None;
    }
    let r = div_bound(x2_up, &denom, prec, true);
    let gap = (exact_int(1, prec) - r).round_to(prec, Rounding::Down);
    if gap.is_sign_negative() || gap.is_zero() {
        return None;
    }
    Some(div_bound(&first_omitted.abs(), &gap, prec, true))
}

/// Adaptive truncation of the Taylor series for `a > 0`, returning the partial
/// sum and a rigorous bound on its distance to the exact series sum `S`
/// (so `erf(a) = (2/sqrt(pi)) * S`).
///
/// Two contributions, matching design doc section 3:
/// - **truncation**: the geometric bound of [`geometric_remainder`]. If the
///   iteration cap is reached before the monotone precondition holds, the
///   unconditional envelope `|R_n| <= |S| + |S_n| <= 1 + mag` is used instead
///   (`S = (sqrt(pi)/2)*erf(a)` and `erf(a) < 1`), which is honest but much
///   larger and therefore shows up in the certificate.
/// - **rounding**: the loop performs at most four rounded operations per term,
///   so the standard forward-error model bounds the accumulated error by
///   `(8(n+1)+16) * 2^-prec * mag`; [`BOUND_SLACK_BITS`] multiplies that budget
///   by a further power of two.
///
/// The two are **added**, not merged: the truncation bound is an exact
/// mathematical remainder and must not be scaled by the `2^-prec` budget.
fn series_bound(
    a: &BigFloat,
    prec: usize,
    x2_up: &BigFloat,
    max_terms: usize,
) -> (BigFloat, BigFloat) {
    let a0 = a.clone().with_precision(prec);
    // The recurrence uses the rounded square; its half-ulp relative perturbation
    // is covered by the bound slack (see BOUND_SLACK_BITS).
    let x2 = (a0.clone() * a0.clone()).with_precision(prec);
    let neg_x2 = -x2;
    let mut term = a0.clone();
    let mut sum = a0.clone();
    // Accumulated magnitude sum, kept an upper bound by rounding each add up.
    let mut mag = a0.abs().round_to(prec, Rounding::Up);
    // Absolute truncation target: 16 ulps of a unit magnitude. The partial sum
    // is at most sqrt(pi)/2, so this keeps the remainder far below the rounding
    // accumulation instead of spending terms on an irrelevant tail.
    let target = pow2_below(prec, 4);
    let mut n: usize = 0;
    let trunc = loop {
        let next = next_term(&term, &neg_x2, n, prec);
        if let Some(bound) = geometric_remainder(&next, x2_up, n, prec) {
            if bound <= target {
                break bound;
            }
        }
        term = next;
        sum = (sum + term.clone()).with_precision(prec);
        mag = (mag + term.abs()).round_to(prec, Rounding::Up);
        n += 1;
        if n >= max_terms {
            let envelope = exact_int(1, prec) + mag.clone();
            break envelope.round_to(prec, Rounding::Up);
        }
    };
    let slack = BOUND_SLACK_BITS + log2_ceil(8 * (n + 1) + 16);
    let rounding = (mag * pow2_below(prec, slack)).round_to(prec, Rounding::Up);
    let err = (rounding + trunc).round_to(prec, Rounding::Up);
    (sum, err)
}

/// Working precision for a target of `p` bits and operand `a` (design doc
/// section 3): `P = p + 3*ceil(a^2) + GUARD_BITS`, never below the operand's own
/// precision, so the operand itself is used exactly.
///
/// Derivation of the allowance: the sum of absolute term magnitudes is at most
/// `a*e^(a^2) = 2^(1.443*a^2 + O(1))`, so a per-operation relative error of
/// `2^-P` becomes an absolute error of about `2^(1.443*a^2 - P)` in `erf`. The
/// cancellation of `erfc = 1 - erf`, whose value is about `2^(-1.443*a^2)`,
/// turns that into a *relative* error of `2^(2.886*a^2 - P)`, which must stay
/// below `2^-p`; hence `P >= p + 2.886*a^2`, and `3*ceil(a^2)` covers it with
/// the guard bits paying for the operation count.
fn working_precision(a: &BigFloat, p: usize) -> usize {
    let own = match a.precision() {
        0 => DEFAULT_PRECISION,
        q => q,
    };
    let x2_up = square_upper(a, p.max(own));
    let allowance = 3usize.saturating_mul(ceil_index(&x2_up));
    p.saturating_add(allowance)
        .saturating_add(GUARD_BITS)
        .max(own)
        .min(MAX_WORKING_PRECISION)
}

/// Rigorous bracket of `erf(a)` at working precision `prec`, for `a > 0`.
///
/// Derivation (design doc section 4 step 1): [`series_bound`] brackets the series
/// sum by `[sum - err, sum + err]`, [`sqrt_pi_bounds`] brackets `sqrt(pi)` by
/// `[s_lo, s_hi]`, and all quantities are non-negative, so
/// `2*(sum - err)/s_hi <= erf(a) <= 2*(sum + err)/s_lo`. Both quotients go
/// through [`div_bound`], hence stay on their respective sides. A negative lower
/// endpoint is clamped to zero, which is valid because `erf(a) > 0` for `a > 0`
/// (its integrand `exp(-t^2)` is positive).
fn certify_at(a: &BigFloat, prec: usize) -> Bracket {
    let x2_up = square_upper(a, prec);
    let max_terms = ceil_index(&x2_up)
        .saturating_add(prec)
        .saturating_add(64)
        .min(MAX_SERIES_TERMS);
    let (sum, err_sum) = series_bound(a, prec, &x2_up, max_terms);
    let (s_lo, s_hi) = sqrt_pi_bounds(prec);
    let two = pow2_int(1);
    let num_lo = (sum.clone() - err_sum.clone()).round_to(prec, Rounding::Down);
    let num_lo = if num_lo.is_sign_negative() {
        BigFloat::zero()
    } else {
        num_lo
    };
    let num_hi = (sum + err_sum).round_to(prec, Rounding::Up);
    let lo = div_bound(&(num_lo * two.clone()), &s_hi, prec, false);
    let hi = div_bound(&(num_hi * two), &s_lo, prec, true);
    Bracket { lo, hi, prec }
}

/// Bracket of `erfc(x)` derived from the bracket of `erf(|x|)`.
///
/// Derivation: `erfc(a) = 1 - erf(a)` flips the bracket, while
/// `erfc(-a) = 1 + erf(a)` preserves its order. Each endpoint formation adds one
/// nearest rounding of a value in `[0, 2]`, i.e. at most half an ulp at `prec`;
/// the margin `2^(2-prec)` is four times that, and [`outward`] adds its own
/// margin, so the widened bracket still contains the exact value. The lower
/// endpoint is clamped at zero because `erfc >= 0` everywhere.
fn erfc_bracket(erf: &Bracket, negative: bool) -> Bracket {
    let prec = erf.prec;
    let one = exact_int(1, prec);
    let margin = pow2_below(prec, 2);
    let (lo, hi) = if negative {
        (
            outward(&(one.clone() + erf.lo.clone()), &margin, prec, false),
            outward(&(one + erf.hi.clone()), &margin, prec, true),
        )
    } else {
        (
            outward(&(one.clone() - erf.hi.clone()), &margin, prec, false),
            outward(&(one - erf.lo.clone()), &margin, prec, true),
        )
    };
    let lo = if lo.is_sign_negative() {
        BigFloat::zero().with_precision(prec)
    } else {
        lo
    };
    Bracket { lo, hi, prec }
}

/// Working-precision bracket of `erf(a)` for `a > 0` finite, targeting `p` bits.
///
/// Escalates the guard bits until both the `erf` and the (cancellation-sensitive)
/// `erfc` brackets prove correct rounding at `p`, or [`MAX_GUARD_RETRIES`] is
/// exhausted; the last attempt is returned either way, so an unproven case yields
/// a conservative value and an honestly larger bound (design doc section 4).
fn erf_core(a: &BigFloat, p: usize, negative: bool) -> Bracket {
    let mut prec = working_precision(a, p);
    let mut bracket = certify_at(a, prec);
    let mut retries = 0usize;
    while retries < MAX_GUARD_RETRIES {
        let erfc = erfc_bracket(&bracket, negative);
        if bracket.proves_rounding(p) && erfc.proves_rounding(p) {
            break;
        }
        retries += 1;
        prec = prec
            .saturating_add(GUARD_STEP_BITS)
            .min(MAX_WORKING_PRECISION);
        bracket = certify_at(a, prec);
    }
    bracket
}

/// Shared kernel of the four public entry points.
///
/// Returns `(erf, erf bound, erfc, erfc bound)` at the operand's working
/// precision, where each bound is a positive `BigFloat` with
/// `|true value - value| <= bound`.
fn certify_pair(x: &BigFloat) -> (BigFloat, BigFloat, BigFloat, BigFloat) {
    let p = work_prec(x);
    if x.is_nan() {
        let nan = BigFloat::NAN;
        return (nan.clone(), nan.clone(), nan.clone(), nan);
    }
    // Exact limits (design doc section 1): erf(+-inf) = +-1, erfc(+inf) = 0,
    // erfc(-inf) = 2. The bound 2^-p is positive and trivially valid because
    // these values are attained exactly.
    if x.is_infinite() {
        let b = pow2_neg(p);
        let (erf_v, erfc_v) = if x.is_sign_positive() {
            (exact_int(1, p), BigFloat::zero().with_precision(p))
        } else {
            (-exact_int(1, p), exact_int(2, p))
        };
        return (erf_v, b.clone(), erfc_v, b);
    }
    let neg = x.is_sign_negative();
    let a = x.abs();
    if a.is_zero() {
        // erf(+-0) = +-0 preserves the sign (odd symmetry); erfc(+-0) = 1.
        let z = if neg {
            -BigFloat::zero()
        } else {
            BigFloat::zero()
        };
        let b = pow2_neg(p);
        return (z.with_precision(p), b.clone(), exact_int(1, p), b);
    }
    // Large-argument threshold (design doc section 2). Chain of strict bounds for
    // a >= 1: erfc(a) <= exp(-a^2)/(a*sqrt(pi)) (design doc section 1), and
    // a*sqrt(pi) > 1 there, so erfc(a) < exp(-a^2) = 2^(-a^2 log2 e) <= 2^(-a^2)
    // because log2 e > 1. Hence a^2 >= p + 2 gives erfc(a) < 2^-(p+2). The p-bit
    // predecessor of 1 is 1 - 2^-p with midpoint 1 - 2^-(p+1), so erf necessarily
    // rounds to exactly 1, and 2 - erfc rounds to exactly 2. The predicate is
    // decisive on the exact dyadic parts, so the fast path cannot fire early; the
    // returned limits are the exact ones and the certificate is the proven bound
    // 2^-(p+2) on the omitted erfc.
    if square_at_least(&a, p.saturating_add(2)) {
        let b = pow2_neg(p.saturating_add(2));
        let one = exact_int(1, p);
        let erf_v = if neg { -one.clone() } else { one };
        let erfc_v = if neg {
            exact_int(2, p)
        } else {
            BigFloat::zero().with_precision(p)
        };
        return (erf_v, b.clone(), erfc_v, b);
    }
    let bracket = erf_core(&a, p, neg);
    // erfc is derived from the same bracket, so the two certificates are
    // consistent by construction (erf + erfc = 1 up to the reported bounds).
    let erfc_b = erfc_bracket(&bracket, neg);
    // erf is odd: the sign flip maps the bracket [-hi, -lo] exactly (flipping a
    // sign bit neither rounds nor changes magnitudes), so the same width bound
    // holds for negative operands.
    let signed = if neg {
        Bracket {
            lo: -bracket.hi.clone(),
            hi: -bracket.lo.clone(),
            prec: bracket.prec,
        }
    } else {
        bracket
    };
    let (erf_v, erf_b) = signed.value_and_bound(p);
    let (erfc_v, erfc_b) = erfc_b.value_and_bound(p);
    (erf_v, erf_b, erfc_v, erfc_b)
}

/// Endpoint image of a monotone `erf`/`erfc` mapping.
///
/// Derivation: for an increasing `f` the image of `[lo, hi]` contains
/// `[f(lo), f(hi)]`; each endpoint value is replaced by its proven bracket
/// `value +- err`, widened outward with [`outward`] and re-rounded with directed
/// rounding at the endpoints' working precision, and `Interval::new` rounds
/// outward again, so the result strictly contains the true image. For a
/// decreasing `f` the endpoints are swapped.
fn interval_image(
    iv: &Interval,
    decreasing: bool,
    f: impl Fn(&BigFloat) -> (BigFloat, BigFloat),
) -> Interval {
    if iv.is_nan() {
        return Interval::NAN;
    }
    if iv.is_empty() {
        return Interval::empty();
    }
    let (lo, hi) = match (iv.lower(), iv.upper()) {
        (Some(a), Some(b)) => (a, b),
        _ => return Interval::NAN,
    };
    let (flo, elo) = f(&lo);
    let (fhi, ehi) = f(&hi);
    if flo.is_nan() || fhi.is_nan() || elo.is_nan() || ehi.is_nan() {
        return Interval::NAN;
    }
    let prec = work_prec(&lo).max(work_prec(&hi));
    let (v_lo, e_lo, v_hi, e_hi) = if decreasing {
        (fhi, ehi, flo, elo)
    } else {
        (flo, elo, fhi, ehi)
    };
    let rlo = outward(&v_lo, &e_lo, prec, false).round_to(prec, Rounding::Down);
    let rhi = outward(&v_hi, &e_hi, prec, true).round_to(prec, Rounding::Up);
    Interval::new(rlo, rhi)
}

impl BigFloat {
    /// Error function `erf(x) = (2/sqrt(pi)) * integral_0^x exp(-t^2) dt`,
    /// rounded to nearest at the operand's working precision (`0` meaning
    /// [`DEFAULT_PRECISION`]).
    ///
    /// The value comes from the Taylor series at the working precision
    /// `P = p + 3*ceil(x^2) + guard` of design doc section 3, and the guard bits
    /// are escalated until correct rounding at `p` is proven for both `erf` and
    /// `erfc`. For `x^2 >= p + 2` the strict bound
    /// `erfc(x) <= exp(-x^2)/(x*sqrt(pi)) < 2^-(p+2)` proves that the correctly
    /// rounded result is the exact limit `+-1`, which is returned directly (the
    /// omitted part is reported by [`BigFloat::erf_checked`]).
    ///
    /// Special values: `NaN` -> `NaN`, `erf(+-infinity)` = `+-1`,
    /// `erf(+-0)` = `+-0` (sign preserved).
    ///
    /// # Examples
    ///
    /// ```
    /// use rhytido_backend::BigFloat;
    ///
    /// let e = BigFloat::from_f64(1.0).erf();
    /// assert!(e > BigFloat::from_f64(0.84));
    /// assert!(e < BigFloat::from_f64(0.85));
    /// ```
    ///
    /// # Panics
    ///
    /// Never.
    pub fn erf(&self) -> Self {
        certify_pair(self).0
    }

    /// Complementary error function `erfc(x) = 1 - erf(x)`, rounded to nearest at
    /// the operand's working precision.
    ///
    /// Special values: `NaN` -> `NaN`, `erfc(+infinity)` = `0`,
    /// `erfc(-infinity)` = `2`, `erfc(+-0)` = `1`. Beyond the large-argument
    /// threshold `x^2 >= p + 2` the value is the exact limit: design doc section 1
    /// proves `erfc(x) <= exp(-x^2)/(x*sqrt(pi)) < 2^-(p+2)`, i.e. the true value
    /// is smaller than the working precision can resolve, so per the design the
    /// exact limit `0` (resp. `2` for negative `x`) is returned; the omitted part
    /// is reported by [`BigFloat::erfc_checked`].
    ///
    /// # Panics
    ///
    /// Never.
    pub fn erfc(&self) -> Self {
        certify_pair(self).2
    }

    /// `erf(x)` together with a strict absolute error bound: `(value, bound)`
    /// with `|erf(x) - value| <= bound` and `bound > 0`.
    ///
    /// This is the certificate-ready API: the bound accounts for the series
    /// truncation (geometric remainder bound), the rounding accumulation at the
    /// working precision, the rigorous `pi`/`sqrt(pi)` bounds and the final
    /// re-rounding, so it can be consumed directly by the interval layer and the
    /// faithful evaluator. When correct rounding at the working precision is
    /// proven the bound is below one ulp of the value; otherwise the unproven
    /// bracket width is added, so the certificate degrades honestly instead of
    /// overstating accuracy. `NaN` propagates to both components; exact limits
    /// (special values) carry the positive bound `2^-p`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rhytido_backend::BigFloat;
    ///
    /// let (v, e) = BigFloat::from_f64(1.0).erf_checked();
    /// assert!(e > BigFloat::zero());
    /// assert!(v > BigFloat::from_f64(0.84));
    /// assert!(v < BigFloat::from_f64(0.85));
    /// ```
    ///
    /// # Panics
    ///
    /// Never.
    pub fn erf_checked(&self) -> (Self, Self) {
        let (v, e, _, _) = certify_pair(self);
        (v, e)
    }

    /// `erfc(x)` together with a strict absolute error bound, the same contract
    /// as [`BigFloat::erf_checked`].
    ///
    /// # Panics
    ///
    /// Never.
    pub fn erfc_checked(&self) -> (Self, Self) {
        let (_, _, v, e) = certify_pair(self);
        (v, e)
    }
}

impl Interval {
    /// Interval error function. `erf` is strictly increasing, so the image is the
    /// endpoint image widened by the checked error bounds and re-rounded outward;
    /// unbounded endpoints map to the corresponding exact limits, so the result
    /// always contains the true range.
    ///
    /// # Examples
    ///
    /// ```
    /// use rhytido_backend::{BigFloat, Interval};
    ///
    /// let iv = Interval::new(BigFloat::from_f64(-1.0), BigFloat::from_f64(1.0));
    /// let img = iv.erf();
    /// assert!(img.contains(&BigFloat::from_f64(0.0)));
    /// assert!(img.contains(&BigFloat::from_f64(0.84)));
    /// ```
    ///
    /// # Panics
    ///
    /// Never.
    pub fn erf(&self) -> Self {
        interval_image(self, false, |x| x.erf_checked())
    }

    /// Interval complementary error function. `erfc` is strictly decreasing, so
    /// the endpoint images are swapped; the containment argument is the one of
    /// [`Interval::erf`].
    ///
    /// # Panics
    ///
    /// Never.
    pub fn erfc(&self) -> Self {
        interval_image(self, true, |x| x.erfc_checked())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact series sum `S = (sqrt(pi)/2) erf(a)` from a much finer run of
    /// the same kernel. The module layer pins external reference values; this
    /// helper only checks the internal consistency of the bounds.
    fn reference_sum(a: &BigFloat, prec: usize) -> BigFloat {
        let x2_up = square_upper(a, prec);
        let (sum, _) = series_bound(a, prec, &x2_up, prec.saturating_add(64));
        sum
    }

    /// `erf(a)` reconstructed from [`reference_sum`] at `prec`.
    fn reference_erf(a: &BigFloat, prec: usize) -> BigFloat {
        let sum = reference_sum(a, prec);
        let root = BigFloat::pi_with_precision(prec.saturating_add(PI_GUARD_BITS)).sqrt();
        (exact_int(2, prec) * sum / root).with_precision(prec)
    }

    #[test]
    fn normal_next_term_matches_closed_form() {
        // Recurrence a_{n+1} = a_n * (-x^2)(2n+1)/((n+1)(2n+3)) versus the closed
        // form (-1)^n a^(2n+1)/(n!(2n+1)), implemented independently.
        let prec = 120usize;
        for a_f in [0.5f64, 1.0, 2.25, 3.5] {
            let a = BigFloat::from_f64(a_f).with_precision(prec);
            let neg_x2 = -(a.clone() * a.clone()).with_precision(prec);
            let mut term = a.clone();
            for n in 0..8usize {
                let direct = closed_form_term(&a, n, prec);
                let diff = (term.clone() - direct).abs();
                assert!(diff <= pow2_below(prec, 20), "a={a_f} n={n} diff={diff}");
                term = next_term(&term, &neg_x2, n, prec);
            }
        }
    }

    #[test]
    fn normal_geometric_remainder_requires_monotone_index() {
        // The precondition is n + 2 > x^2: below it the terms need not decrease,
        // so the geometric bound must be refused; above it, it is available.
        let prec = 80usize;
        let a = BigFloat::from_f64(3.0).with_precision(prec); // x^2 = 9
        let x2_up = square_upper(&a, prec);
        for n in 0..7usize {
            let next = closed_form_term(&a, n + 1, prec);
            assert!(
                geometric_remainder(&next, &x2_up, n, prec).is_none(),
                "n={n} must not use the geometric bound"
            );
        }
        for n in 8..12usize {
            let next = closed_form_term(&a, n + 1, prec);
            assert!(geometric_remainder(&next, &x2_up, n, prec).is_some());
        }
    }

    #[test]
    fn normal_geometric_remainder_covers_true_tail() {
        // The geometric bound must dominate any partial sum of the omitted
        // absolute terms, which is exactly the design-doc derivation.
        let prec = 220usize;
        for a_f in [0.75f64, 1.5, 2.5, 4.0] {
            let a = BigFloat::from_f64(a_f).with_precision(prec);
            let x2_up = square_upper(&a, prec);
            for n in [16usize, 24, 32, 48] {
                let first = closed_form_term(&a, n + 1, prec);
                let bound = match geometric_remainder(&first, &x2_up, n, prec) {
                    Some(b) => b,
                    None => continue,
                };
                let mut acc = BigFloat::zero().with_precision(prec);
                for k in (n + 1)..(n + 61) {
                    acc = (acc + closed_form_term(&a, k, prec).abs()).round_to(prec, Rounding::Up);
                }
                assert!(
                    acc <= bound,
                    "a={a_f} n={n}: tail sum {acc} > bound {bound}"
                );
            }
        }
    }

    #[test]
    fn error_geometric_remainder_refuses_non_contracting() {
        // n + 2 <= x^2 leaves r >= 1: the bound must be None, so the caller keeps
        // summing until the contraction actually holds.
        let prec = 80usize;
        let a = BigFloat::from_f64(6.0).with_precision(prec); // x^2 = 36
        let x2_up = square_upper(&a, prec);
        for n in 0..34usize {
            let next = closed_form_term(&a, n + 1, prec);
            assert!(geometric_remainder(&next, &x2_up, n, prec).is_none());
        }
    }

    #[test]
    fn normal_series_bound_covers_reference_sum() {
        // The reported error must dominate the true distance to a far finer
        // reference sum; this validates the rounding-accumulation budget.
        for a_f in [0.1f64, 0.5, 1.0, 2.0, 3.0, 5.0] {
            let prec = 160usize;
            let a = BigFloat::from_f64(a_f).with_precision(prec);
            let x2_up = square_upper(&a, prec);
            let (sum, err) = series_bound(&a, prec, &x2_up, prec + 32);
            let fine = reference_sum(&a, prec + 400);
            let diff = (fine - sum).abs();
            assert!(diff <= err, "a={a_f}: diff {diff} vs err {err}");
            assert!(err > BigFloat::zero());
        }
    }

    #[test]
    fn normal_pi_bounds_contain_reference() {
        for prec in [12usize, 53, 165, 300] {
            let (lo, hi) = pi_bounds(prec);
            let reference = BigFloat::pi_with_precision(prec + 90);
            assert!(lo < hi);
            assert!(lo <= reference, "prec={prec}: lo {lo} above reference");
            assert!(reference <= hi, "prec={prec}: hi {hi} below reference");
            assert!(lo > BigFloat::zero());
        }
    }

    #[test]
    fn normal_sqrt_pi_bounds_contain_reference() {
        for prec in [12usize, 53, 165, 300] {
            let (lo, hi) = sqrt_pi_bounds(prec);
            let reference = BigFloat::pi_with_precision(prec + 90).sqrt();
            assert!(lo < hi);
            assert!(lo <= reference, "prec={prec}: lo {lo} above reference");
            assert!(reference <= hi, "prec={prec}: hi {hi} below reference");
            assert!(lo > BigFloat::zero());
        }
    }

    #[test]
    fn normal_square_upper_bounds_product() {
        for a_f in [0.5f64, 1.0, 2.5, 7.0] {
            let prec = 100usize;
            let a = BigFloat::from_f64(a_f).with_precision(prec);
            let up = square_upper(&a, prec);
            let exact = (a.clone() * a.clone()).with_precision(4 * prec);
            assert!(up >= exact, "a={a_f}: upper {up} below exact {exact}");
        }
    }

    #[test]
    fn normal_square_at_least_matches_mathematics() {
        // Decisive on the exact dyadic parts, so the verdict must agree with the
        // true relation for squares around the tested thresholds.
        let prec = 80usize;
        for a_f in [2.0f64, 2.5, 3.0, 4.0, 0.5] {
            let a = BigFloat::from_f64(a_f).with_precision(prec);
            for t in [1usize, 4, 9, 10, 16] {
                let exact = (a.clone() * a.clone()).with_precision(4 * prec);
                let expect = exact >= BigFloat::from_i64(t as i64);
                assert_eq!(square_at_least(&a, t), expect, "a={a_f} t={t}");
            }
        }
    }

    #[test]
    fn normal_outward_and_div_bound_are_directed() {
        let prec = 60usize;
        let v = (BigFloat::one() / BigFloat::from_i64(3)).with_precision(prec);
        let d = (BigFloat::one() / BigFloat::from_i64(7)).with_precision(prec);
        assert!(outward(&v, &d, prec, true) >= v.clone() + d.clone());
        assert!(outward(&v, &d, prec, false) <= v.clone() - d.clone());
        let den = (BigFloat::one() + BigFloat::from_f64(0.1)).with_precision(prec);
        let exact_q = (v.clone() / den.clone()).with_precision(4 * prec);
        assert!(div_bound(&v, &den, prec, true) >= exact_q);
        assert!(div_bound(&v, &den, prec, false) <= exact_q);
    }

    #[test]
    fn normal_rounding_allowance_between_half_and_one_ulp() {
        // |v|*2^-p must be at least half an ulp and strictly below one ulp, which
        // is what keeps the reported certificate finer than the value.
        for v_f in [0.15729f64, 0.5, 0.8427, 1.0, 1.9999] {
            for p in [24usize, 53, 165] {
                let v = BigFloat::from_f64(v_f).with_precision(p);
                let allow = rounding_allowance(&v, p, p + 40);
                let exp = binade_exponent_for_test(&v);
                // half ulp = 2^(E-p), one ulp = 2^(E+1-p).
                assert!(allow >= pow2_shift(exp - p as i64), "v={v_f} p={p}");
                assert!(allow < pow2_shift(exp + 1 - p as i64), "v={v_f} p={p}");
            }
        }
        // A zero value cannot come from rounding (the backend has no underflow),
        // but the positive floor keeps the bound's sign contract.
        let zero_allow = rounding_allowance(&BigFloat::zero(), 64, 96);
        assert!(zero_allow > BigFloat::zero());
    }

    /// `floor(log2 |v|)` for the unit test above.
    fn binade_exponent_for_test(v: &BigFloat) -> i64 {
        let two = BigFloat::from_i64(2);
        let one = BigFloat::one();
        let mut m = v.abs();
        let mut e = 0i64;
        while m >= two {
            m = m / two.clone();
            e += 1;
        }
        while m < one {
            m = m * two.clone();
            e -= 1;
        }
        e
    }

    /// Exact `2^e` for the unit test above.
    fn pow2_shift(e: i64) -> BigFloat {
        let two = BigFloat::from_i64(2);
        let mut r = BigFloat::one();
        if e >= 0 {
            for _ in 0..e {
                r = r * two.clone();
            }
        } else {
            for _ in 0..(-e) {
                r = r / two.clone();
            }
        }
        r
    }

    #[test]
    fn normal_pow2_and_index_helpers() {
        assert_eq!(log2_ceil(1), 0);
        assert_eq!(log2_ceil(2), 1);
        assert_eq!(log2_ceil(3), 2);
        assert_eq!(log2_ceil(256), 8);
        // 2^(2-10) = 2^-8 = 0.00390625 and 2^(5-2) = 8, both exact.
        assert_eq!(pow2_below(10, 2), BigFloat::from_f64(0.00390625));
        assert_eq!(pow2_below(2, 5), BigFloat::from_i64(8));
        assert_eq!(pow2_neg(3), BigFloat::from_f64(0.125));
        assert_eq!(ceil_index(&BigFloat::from_f64(2.5)), 3);
        assert_eq!(ceil_index(&BigFloat::from_i64(4)), 4);
        assert_eq!(to_usize_exact(&BigFloat::from_f64(0.25)), None);
        assert_eq!(to_usize_exact(&-BigFloat::one()), None);
    }

    #[test]
    fn extreme_certify_at_brackets_reference() {
        // The certificate bracket must contain a far finer reference value at the
        // threshold boundary and across the range.
        for a_f in [0.0009765625f64, 0.25, 1.0, 4.0, 7.3, 12.9] {
            let p = 64usize;
            let a = BigFloat::from_f64(a_f).with_precision(p);
            let wp = working_precision(&a, p);
            let bracket = certify_at(&a, wp);
            let reference = reference_erf(&a, wp + 512);
            assert!(bracket.lo <= reference, "a={a_f}: lo above reference");
            assert!(
                reference <= bracket.hi.clone(),
                "a={a_f}: reference {reference} above hi {}",
                bracket.hi
            );
            assert!(bracket.width() > BigFloat::zero());
        }
    }

    #[test]
    fn extreme_erfc_bracket_contains_complement() {
        // erfc(x) = 1 - erf(x): the derived bracket must contain the complement of
        // a far finer reference, on both sides of zero.
        for a_f in [0.5f64, 1.0, 3.0, 6.0] {
            let p = 64usize;
            let a = BigFloat::from_f64(a_f).with_precision(p);
            let wp = working_precision(&a, p);
            let bracket = certify_at(&a, wp);
            for negative in [false, true] {
                let c = erfc_bracket(&bracket, negative);
                let reference = reference_erf(&a, wp + 512);
                let true_c = if negative {
                    BigFloat::one() + reference
                } else {
                    BigFloat::one() - reference
                };
                assert!(c.lo <= true_c, "a={a_f} neg={negative}: lo above true");
                assert!(true_c <= c.hi, "a={a_f} neg={negative}: true above hi");
                assert!(
                    c.lo >= BigFloat::zero(),
                    "erfc bracket must be non-negative"
                );
            }
        }
    }

    #[test]
    fn extreme_series_cap_falls_back_to_envelope() {
        // One term only: the bound must still dominate the true difference, which
        // is the guarantee the caller relies on past the iteration cap.
        let prec = 120usize;
        let a = BigFloat::from_f64(3.5).with_precision(prec);
        let x2_up = square_upper(&a, prec);
        let (sum, err) = series_bound(&a, prec, &x2_up, 1);
        let reference = reference_sum(&a, prec + 512);
        let diff = (reference - sum).abs();
        assert!(diff <= err, "capped sum diff {diff} vs err {err}");
    }

    #[test]
    fn extreme_value_and_bound_adds_width_when_unproven() {
        // The degraded certificate path: when the bracket does NOT prove correct
        // rounding at p (here a hand-made 8-ulp-wide bracket), the bound must
        // still dominate the width, which is how an exhausted retry budget shows
        // up honestly instead of overstating accuracy.
        let p = 64usize;
        let v = BigFloat::from_f64(0.75).with_precision(p);
        let hi = (v.clone() + pow2_below(p, 4)).with_precision(p);
        let b = Bracket {
            lo: v.clone(),
            hi,
            prec: p,
        };
        assert!(
            !b.proves_rounding(p),
            "8-ulp bracket must not prove rounding"
        );
        let (value, bound) = b.value_and_bound(p);
        assert_eq!(value, v.round_to(p, Rounding::Nearest));
        assert!(
            bound >= b.width(),
            "bound {bound} below width {}",
            b.width()
        );
        assert!(bound > BigFloat::zero());
    }

    #[test]
    fn extreme_erf_core_escalation_keeps_certificate_valid() {
        // The escalation loop must terminate and always return a bracket that
        // contains the reference value, even when the first attempt's precision
        // is far too small to prove rounding.
        let p = 53usize;
        for a_f in [1.0f64, 2.5, 4.0] {
            let a = BigFloat::from_f64(a_f).with_precision(p);
            let bracket = erf_core(&a, p, false);
            let reference = reference_erf(&a, 500);
            assert!(
                bracket.lo <= reference && reference <= bracket.hi,
                "a={a_f}: reference {reference} outside [{}, {}]",
                bracket.lo,
                bracket.hi
            );
            assert!(bracket.proves_rounding(p), "a={a_f}: rounding unproven");
        }
    }

    #[test]
    fn robustness_certify_at_terminates_for_wide_inputs() {
        // Deep operands and tiny precisions still terminate with a finite,
        // positive width (the escalation loop and the series cap are bounded).
        for a_f in [1e-12f64, 1e-6, 0.5, 6.5] {
            for p in [8usize, 24, 53] {
                let a = BigFloat::from_f64(a_f).with_precision(p);
                let bracket = certify_at(&a, working_precision(&a, p));
                assert!(bracket.lo.is_finite() && bracket.hi.is_finite());
                assert!(bracket.lo <= bracket.hi);
                assert!(bracket.width() > BigFloat::zero());
            }
        }
    }
}
