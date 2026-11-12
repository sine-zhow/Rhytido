//! Multi-precision intervals with outward rounding.
//!
//! Facade interval type (facade isolation, ADR-2). An interval is NaN, empty, or
//! a bounded range `[lo, hi]` with `lo <= hi`; endpoints may be infinite
//! (unbounded). Construction and arithmetic round outward (lo down, hi up) so
//! the result contains the true range.
//!
//! M0 note: endpoint arithmetic uses nearest-even BigFloat ops followed by
//! outward rounding (conservative). Fully directed-rounding interval kernels
//! and division by zero-containing intervals are refined in M2.

use core::fmt;

use crate::{BigFloat, DEFAULT_PRECISION, Integer, Rounding};

/// Guard bits added to the endpoint precision when computing pi for
/// extremum/pole detection. The pi error bound (half an ulp of the correctly
/// rounded atan) is absorbed by the explicit margin in `pi_bounds`.
const PI_GUARD_BITS: usize = 10;

#[derive(Clone)]
enum Repr {
    Nan,
    Empty,
    Range { lo: BigFloat, hi: BigFloat },
}

/// Selector for the shared sine/cosine range kernel.
enum Trig {
    Sin,
    Cos,
}

/// A period lattice with rigorous bounds: the points `offset + k * period`
/// for integer `k`, where the true offset and period are known to lie within
/// `[offset_lo, offset_hi]` and `[period_lo, period_hi]` (period positive).
/// Used for extremum/pole detection with a finite-precision pi.
struct Lattice {
    offset_lo: BigFloat,
    offset_hi: BigFloat,
    period_lo: BigFloat,
    period_hi: BigFloat,
}

impl Lattice {
    /// Conservative test for whether some lattice point may lie in `[lo, hi]`.
    ///
    /// Point `k` spans `[A(k), B(k)]` with `A(k) = offset_lo + k*period_lo`
    /// and `B(k) = offset_hi + k*period_hi` for `k >= 0` (the period bounds
    /// swap for `k < 0`). Both bounds increase with `k`, so an intersection
    /// exists iff `min{k : B(k) >= lo} <= max{k : A(k) <= hi}`. Both indices
    /// are computed with exact integer arithmetic on the canonical dyadic
    /// parts, so no rounding can drop a point; combined with the rigorous
    /// offset/period bounds the test never misses a true extremum (it may
    /// include one just outside when the bounds are uncertain).
    fn intersects(&self, lo: &BigFloat, hi: &BigFloat) -> bool {
        // The branch denominator follows the sign of the numerator, which
        // selects the k >= 0 vs k < 0 half of the lattice.
        let period_b = if lo >= &self.offset_hi {
            &self.period_hi
        } else {
            &self.period_lo
        };
        let k_b = ceil_ratio(lo, &self.offset_hi, period_b);
        let period_a = if hi >= &self.offset_lo {
            &self.period_lo
        } else {
            &self.period_hi
        };
        let k_a = floor_ratio(hi, &self.offset_lo, period_a);
        k_b <= k_a
    }
}

/// A multi-precision interval: NaN, empty, or `[lo, hi]`.
///
/// # Examples
///
/// ```
/// use rhytido_backend::{BigFloat, Interval};
///
/// let a = Interval::point(BigFloat::from_i64(1));
/// let b = Interval::new(BigFloat::from_i64(2), BigFloat::from_i64(3));
/// let s = a.add(&b);
/// assert!(s.contains(&BigFloat::from_i64(3)));
/// assert!(s.contains(&BigFloat::from_i64(4)));
/// assert!(Interval::new(BigFloat::from_i64(3), BigFloat::from_i64(1)).is_empty());
/// ```
#[derive(Clone)]
pub struct Interval {
    repr: Repr,
}

impl Interval {
    /// The NaN interval.
    pub const NAN: Self = Self { repr: Repr::Nan };

    /// The empty interval.
    pub fn empty() -> Self {
        Self { repr: Repr::Empty }
    }

    /// Constructs `[lo, hi]` with outward rounding. NaN if either endpoint is
    /// NaN; empty if `lo > hi`.
    pub fn new(lo: BigFloat, hi: BigFloat) -> Self {
        make_range(lo, hi)
    }

    /// Constructs the point interval `[x, x]`.
    pub fn point(x: BigFloat) -> Self {
        make_range(x.clone(), x)
    }

    /// Returns `true` if this is the NaN interval.
    pub fn is_nan(&self) -> bool {
        matches!(self.repr, Repr::Nan)
    }

    /// Returns `true` if this is the empty interval.
    pub fn is_empty(&self) -> bool {
        matches!(self.repr, Repr::Empty)
    }

    /// Returns `true` if this is a bounded range (not NaN, not empty).
    pub fn is_bounded(&self) -> bool {
        matches!(self.repr, Repr::Range { .. })
    }

    /// Returns `true` if this is a single point (`lo == hi`, finite).
    pub fn is_point(&self) -> bool {
        match &self.repr {
            Repr::Range { lo, hi } => lo == hi && lo.is_finite(),
            _ => false,
        }
    }

    /// Returns the lower endpoint, or `None` for NaN/empty.
    pub fn lower(&self) -> Option<BigFloat> {
        match &self.repr {
            Repr::Range { lo, .. } => Some(lo.clone()),
            _ => None,
        }
    }

    /// Returns the upper endpoint, or `None` for NaN/empty.
    pub fn upper(&self) -> Option<BigFloat> {
        match &self.repr {
            Repr::Range { hi, .. } => Some(hi.clone()),
            _ => None,
        }
    }

    /// Returns `true` if `x` lies in this interval (false for NaN/empty).
    pub fn contains(&self, x: &BigFloat) -> bool {
        match &self.repr {
            Repr::Range { lo, hi } => {
                if x.is_nan() {
                    false
                } else {
                    lo <= x && x <= hi
                }
            }
            _ => false,
        }
    }

    /// Returns `true` if this interval contains zero.
    pub fn contains_zero(&self) -> bool {
        self.contains(&BigFloat::zero())
    }

    /// Interval addition.
    pub fn add(&self, other: &Self) -> Self {
        match (&self.repr, &other.repr) {
            (Repr::Nan, _) | (_, Repr::Nan) => Self::NAN,
            (Repr::Empty, _) | (_, Repr::Empty) => Self::empty(),
            (Repr::Range { lo: a, hi: b }, Repr::Range { lo: c, hi: d }) => {
                make_range(a.clone() + c.clone(), b.clone() + d.clone())
            }
        }
    }

    /// Interval subtraction.
    pub fn sub(&self, other: &Self) -> Self {
        match (&self.repr, &other.repr) {
            (Repr::Nan, _) | (_, Repr::Nan) => Self::NAN,
            (Repr::Empty, _) | (_, Repr::Empty) => Self::empty(),
            (Repr::Range { lo: a, hi: b }, Repr::Range { lo: c, hi: d }) => {
                make_range(a.clone() - d.clone(), b.clone() - c.clone())
            }
        }
    }

    /// Interval multiplication.
    pub fn mul(&self, other: &Self) -> Self {
        match (&self.repr, &other.repr) {
            (Repr::Nan, _) | (_, Repr::Nan) => Self::NAN,
            (Repr::Empty, _) | (_, Repr::Empty) => Self::empty(),
            (Repr::Range { lo: a, hi: b }, Repr::Range { lo: c, hi: d }) => {
                let prods = [
                    a.clone() * c.clone(),
                    a.clone() * d.clone(),
                    b.clone() * c.clone(),
                    b.clone() * d.clone(),
                ];
                if prods.iter().any(|p| p.is_nan()) {
                    return Self::NAN;
                }
                let mut lo = prods[0].clone();
                let mut hi = prods[0].clone();
                for p in &prods[1..] {
                    if p < &lo {
                        lo = p.clone();
                    }
                    if p > &hi {
                        hi = p.clone();
                    }
                }
                make_range(lo, hi)
            }
        }
    }

    /// Interval division. M0 returns NaN if the divisor contains zero.
    pub fn div(&self, other: &Self) -> Self {
        match (&self.repr, &other.repr) {
            (Repr::Nan, _) | (_, Repr::Nan) => Self::NAN,
            (Repr::Empty, _) | (_, Repr::Empty) => Self::empty(),
            (_, Repr::Range { .. }) if other.contains_zero() => Self::NAN,
            (Repr::Range { .. }, Repr::Range { lo: c, hi: d }) => {
                // 1/[c,d] = [1/d, 1/c] when 0 not in [c,d].
                let inv_lo = BigFloat::one() / d.clone();
                let inv_hi = BigFloat::one() / c.clone();
                let recip = make_range(inv_lo, inv_hi);
                self.mul(&recip)
            }
        }
    }

    /// Applies a monotonic function by evaluating both endpoints with outward
    /// rounding. Conservative on domain: returns NaN if either endpoint maps to
    /// NaN (sufficient for the point/narrow intervals used by the faithful
    /// engine; partial-domain clipping is a refinement).
    fn map_mono(&self, f: impl Fn(&BigFloat) -> BigFloat, decreasing: bool) -> Self {
        match &self.repr {
            Repr::Nan => Self::NAN,
            Repr::Empty => Self::empty(),
            Repr::Range { lo, hi } => {
                let flo = f(lo);
                let fhi = f(hi);
                if flo.is_nan() || fhi.is_nan() {
                    return Self::NAN;
                }
                if decreasing {
                    make_range(fhi, flo)
                } else {
                    make_range(flo, fhi)
                }
            }
        }
    }

    /// Interval exponential (monotonic increasing).
    pub fn exp(&self) -> Self {
        self.map_mono(|x| x.exp(), false)
    }

    /// Interval natural logarithm (monotonic increasing on (0, inf)).
    pub fn ln(&self) -> Self {
        self.map_mono(|x| x.ln(), false)
    }

    /// Interval square root (monotonic increasing on [0, inf)).
    pub fn sqrt(&self) -> Self {
        self.map_mono(|x| x.sqrt(), false)
    }

    /// Interval hyperbolic sine (monotonic increasing).
    pub fn sinh(&self) -> Self {
        self.map_mono(|x| x.sinh(), false)
    }

    /// Interval inverse hyperbolic sine (monotonic increasing).
    pub fn asinh(&self) -> Self {
        self.map_mono(|x| x.asinh(), false)
    }

    /// Interval arctangent (monotonic increasing).
    pub fn atan(&self) -> Self {
        self.map_mono(|x| x.atan(), false)
    }

    /// Interval arcsine (monotonic increasing on [-1, 1]).
    pub fn asin(&self) -> Self {
        self.map_mono(|x| x.asin(), false)
    }

    /// Interval arccosine (monotonic decreasing on [-1, 1]).
    pub fn acos(&self) -> Self {
        self.map_mono(|x| x.acos(), true)
    }

    /// Interval hyperbolic tangent (monotonic increasing).
    pub fn tanh(&self) -> Self {
        self.map_mono(|x| x.tanh(), false)
    }

    /// Interval inverse hyperbolic tangent (monotonic increasing on (-1, 1)).
    pub fn atanh(&self) -> Self {
        self.map_mono(|x| x.atanh(), false)
    }

    /// Interval inverse hyperbolic cosine (monotonic increasing on [1, inf)).
    pub fn acosh(&self) -> Self {
        self.map_mono(|x| x.acosh(), false)
    }

    /// Interval hyperbolic cosine. Even with a minimum of 1 at 0; handles
    /// intervals crossing zero explicitly.
    pub fn cosh(&self) -> Self {
        match &self.repr {
            Repr::Nan => Self::NAN,
            Repr::Empty => Self::empty(),
            Repr::Range { lo, hi } => {
                let zero = BigFloat::zero();
                if lo >= &zero {
                    make_range(lo.cosh(), hi.cosh())
                } else if hi <= &zero {
                    make_range(hi.cosh(), lo.cosh())
                } else {
                    let clo = lo.cosh();
                    let chi = hi.cosh();
                    let mx = if clo > chi { clo } else { chi };
                    make_range(BigFloat::one(), mx)
                }
            }
        }
    }

    /// Interval sine. The endpoint range is widened by any extremum the
    /// interval contains (maxima at pi/2 + 2k*pi, minima at -pi/2 + 2k*pi);
    /// an interval spanning a full period yields exactly [-1, 1].
    pub fn sin(&self) -> Self {
        self.trig_range(Trig::Sin)
    }

    /// Interval cosine. The endpoint range is widened by any extremum the
    /// interval contains (maxima at 2k*pi, minima at pi + 2k*pi); an interval
    /// spanning a full period yields exactly [-1, 1].
    pub fn cos(&self) -> Self {
        self.trig_range(Trig::Cos)
    }

    /// Interval tangent. An interval containing a pole pi/2 + k*pi yields
    /// (-infinity, +infinity); otherwise tan is increasing on the branch and
    /// the endpoints map directly.
    pub fn tan(&self) -> Self {
        let (lo, hi) = match &self.repr {
            Repr::Nan => return Self::NAN,
            Repr::Empty => return Self::empty(),
            Repr::Range { lo, hi } => (lo, hi),
        };
        if !lo.is_finite() || !hi.is_finite() {
            // An unbounded interval always contains poles.
            return make_range(BigFloat::neg_infinity(), BigFloat::infinity());
        }
        let prec = working_prec(lo, hi);
        let w = prec + PI_GUARD_BITS;
        let (pi_lo, pi_hi) = pi_bounds(prec);
        let half = BigFloat::from_f64(0.5);
        let poles = Lattice {
            offset_lo: pi_lo.clone() * half.clone(),
            offset_hi: pi_hi.clone() * half,
            period_lo: pi_lo,
            period_hi: pi_hi,
        };
        if poles.intersects(lo, hi) {
            return make_range(BigFloat::neg_infinity(), BigFloat::infinity());
        }
        // No pole: increasing on the branch. A relative margin covers the
        // unbounded magnitude near poles (see `endpoint_image`).
        let eps_rel = pow2_frac(prec, 3);
        let vlo = lo.with_precision(w).tan();
        let vhi = hi.with_precision(w).tan();
        let rlo = (vlo.clone() - vlo.abs() * eps_rel.clone()).round_to(prec, Rounding::Down);
        let rhi = (vhi.clone() + vhi.abs() * eps_rel).round_to(prec, Rounding::Up);
        make_range(rlo, rhi)
    }

    /// Interval base-2 logarithm (monotonic increasing on (0, inf)).
    pub fn log2(&self) -> Self {
        self.map_mono(|x| x.log2(), false)
    }

    /// Interval base-10 logarithm (monotonic increasing on (0, inf)).
    pub fn log10(&self) -> Self {
        self.map_mono(|x| x.log10(), false)
    }

    /// Interval log(1 + x) (monotonic increasing on (-1, inf)).
    pub fn log1p(&self) -> Self {
        self.map_mono(|x| x.log1p(), false)
    }

    /// Interval exp(x) - 1 (monotonic increasing).
    pub fn expm1(&self) -> Self {
        self.map_mono(|x| x.expm1(), false)
    }

    /// Interval absolute value. Even with a minimum of 0 at 0; handles
    /// intervals crossing zero explicitly (like `cosh`).
    pub fn abs(&self) -> Self {
        match &self.repr {
            Repr::Nan => Self::NAN,
            Repr::Empty => Self::empty(),
            Repr::Range { lo, hi } => {
                let zero = BigFloat::zero();
                if lo >= &zero {
                    make_range(lo.abs(), hi.abs())
                } else if hi <= &zero {
                    make_range(hi.abs(), lo.abs())
                } else {
                    let alo = lo.abs();
                    let ahi = hi.abs();
                    let mx = if alo > ahi { alo } else { ahi };
                    make_range(zero, mx)
                }
            }
        }
    }

    /// Interval ceiling (monotonic non-decreasing, exact integer endpoints).
    pub fn ceil(&self) -> Self {
        self.map_mono(|x| x.ceil(), false)
    }

    /// Interval floor (monotonic non-decreasing, exact integer endpoints).
    pub fn floor(&self) -> Self {
        self.map_mono(|x| x.floor(), false)
    }

    /// Interval nearest integer, ties to even (monotonic non-decreasing,
    /// exact integer endpoints).
    pub fn nearestint(&self) -> Self {
        self.map_mono(|x| x.nearestint(), false)
    }

    /// Shared range kernel for sine and cosine: outward-rounded endpoint
    /// images plus rigorous extremum detection over the period lattice.
    fn trig_range(&self, kind: Trig) -> Self {
        let (lo, hi) = match &self.repr {
            Repr::Nan => return Self::NAN,
            Repr::Empty => return Self::empty(),
            Repr::Range { lo, hi } => (lo, hi),
        };
        if !lo.is_finite() || !hi.is_finite() {
            // An unbounded interval spans full periods.
            return make_range(-BigFloat::one(), BigFloat::one());
        }
        let prec = working_prec(lo, hi);
        let w = prec + PI_GUARD_BITS;
        let (pi_lo, pi_hi) = pi_bounds(prec);
        let two = BigFloat::from_i64(2);
        let half = BigFloat::from_f64(0.5);
        let period_lo = pi_lo.clone() * two.clone();
        let period_hi = pi_hi.clone() * two;
        let (max_lat, min_lat) = match kind {
            Trig::Sin => (
                Lattice {
                    offset_lo: pi_lo.clone() * half.clone(),
                    offset_hi: pi_hi.clone() * half.clone(),
                    period_lo: period_lo.clone(),
                    period_hi: period_hi.clone(),
                },
                Lattice {
                    offset_lo: -(pi_hi.clone() * half.clone()),
                    offset_hi: -(pi_lo.clone() * half),
                    period_lo,
                    period_hi,
                },
            ),
            Trig::Cos => (
                Lattice {
                    offset_lo: BigFloat::zero(),
                    offset_hi: BigFloat::zero(),
                    period_lo: period_lo.clone(),
                    period_hi: period_hi.clone(),
                },
                Lattice {
                    offset_lo: pi_lo,
                    offset_hi: pi_hi,
                    period_lo,
                    period_hi,
                },
            ),
        };
        let (vlo, vhi) = match kind {
            Trig::Sin => (lo.with_precision(w).sin(), hi.with_precision(w).sin()),
            Trig::Cos => (lo.with_precision(w).cos(), hi.with_precision(w).cos()),
        };
        // |sin|, |cos| <= 1, so an absolute margin of 2^(2-prec) absorbs both
        // the guard-precision endpoint error and the sample-side rounding.
        let eps = pow2_frac(prec, 2);
        let ilo = endpoint_image(&vlo, &eps, prec);
        let ihi = endpoint_image(&vhi, &eps, prec);
        let mut rlo = if ilo.0 < ihi.0 { ilo.0 } else { ihi.0 };
        let mut rhi = if ilo.1 > ihi.1 { ilo.1 } else { ihi.1 };
        // A full period (width >= 2*pi) necessarily triggers both detections.
        if max_lat.intersects(lo, hi) {
            rhi = BigFloat::one();
        }
        if min_lat.intersects(lo, hi) {
            rlo = -BigFloat::one();
        }
        make_range(rlo, rhi)
    }
}

/// Builds a range with outward rounding; NaN/empty handled per endpoints.
fn make_range(lo: BigFloat, hi: BigFloat) -> Interval {
    if lo.is_nan() || hi.is_nan() {
        return Interval::NAN;
    }
    if lo > hi {
        return Interval::empty();
    }
    let prec = lo.precision().max(hi.precision());
    let prec = if prec == 0 { DEFAULT_PRECISION } else { prec };
    let lo = lo.round_to(prec, Rounding::Down);
    let hi = hi.round_to(prec, Rounding::Up);
    Interval {
        repr: Repr::Range { lo, hi },
    }
}

/// Shared working precision of two endpoints, with the same unlimited (0)
/// fallback as `make_range`.
fn working_prec(lo: &BigFloat, hi: &BigFloat) -> usize {
    let prec = lo.precision().max(hi.precision());
    if prec == 0 { DEFAULT_PRECISION } else { prec }
}

/// Exact power of two `2^(k - prec)` as a BigFloat (k small).
fn pow2_frac(prec: usize, k: usize) -> BigFloat {
    let pow2 = BigFloat::from_integer(Integer::from(2).pow(prec));
    BigFloat::from_i64(1i64 << k) / pow2
}

/// Rigorous lower/upper bounds on pi for `prec`-bit endpoint arithmetic.
///
/// pi is computed at `prec + PI_GUARD_BITS` bits via the correctly rounded
/// atan (error <= half an ulp). Since pi lies in [2, 4), that error is at most
/// 2^(1-w); the explicit margin 2^(3-w) is four times larger, so it survives
/// the nearest-rounded offset, and the final directed rounding only widens.
fn pi_bounds(prec: usize) -> (BigFloat, BigFloat) {
    let w = prec + PI_GUARD_BITS;
    let center = BigFloat::pi_with_precision(w);
    let eps = pow2_frac(w, 3);
    let lo = (center.clone() - eps.clone()).round_to(w, Rounding::Down);
    let hi = (center + eps).round_to(w, Rounding::Up);
    (lo, hi)
}

/// Outward-rounded image of one endpoint value with an explicit containment
/// margin: the point value was computed at guard precision, and `eps` absorbs
/// both its rounding error and the rounding of any correctly-rounded sample
/// inside the interval, so the directed result contains every such sample.
fn endpoint_image(v: &BigFloat, eps: &BigFloat, prec: usize) -> (BigFloat, BigFloat) {
    let lo = (v.clone() - eps.clone()).round_to(prec, Rounding::Down);
    let hi = (v.clone() + eps.clone()).round_to(prec, Rounding::Up);
    (lo, hi)
}

/// Exact integer numerator/denominator of `(x - offset) / period` for finite
/// dyadic BigFloats with positive period. All arithmetic is on the canonical
/// integer parts, so the quotient is represented without any rounding.
fn ratio_parts(x: &BigFloat, offset: &BigFloat, period: &BigFloat) -> (Integer, Integer) {
    // The callers guarantee finite operands (endpoints are checked; the
    // lattice bounds derive from finite pi bounds).
    let (sx, ex) = x.canonical_parts().expect("finite dyadic endpoint");
    let (so, eo) = offset.canonical_parts().expect("finite dyadic bound");
    let (sp, ep) = period.canonical_parts().expect("finite positive bound");
    let e_min = ex.min(eo);
    let two = Integer::from(2);
    let nx = sx * two.pow((ex - e_min) as usize);
    let no = so * two.pow((eo - e_min) as usize);
    let n = nx - no;
    if ep >= e_min {
        (n, sp * two.pow((ep - e_min) as usize))
    } else {
        (n * two.pow((e_min - ep) as usize), sp)
    }
}

/// Exact `ceil((x - offset) / period)` for finite dyadics, period > 0.
fn ceil_ratio(x: &BigFloat, offset: &BigFloat, period: &BigFloat) -> Integer {
    let (n, d) = ratio_parts(x, offset, period);
    ceil_div(&n, &d)
}

/// Exact `floor((x - offset) / period)` for finite dyadics, period > 0.
fn floor_ratio(x: &BigFloat, offset: &BigFloat, period: &BigFloat) -> Integer {
    let (n, d) = ratio_parts(x, offset, period);
    floor_div(&n, &d)
}

/// Exact ceiling of `a / b` for a positive divisor `b`.
fn ceil_div(a: &Integer, b: &Integer) -> Integer {
    // The divisor is a positively scaled period significand, never zero.
    let (q, r) = a.div_rem(b).expect("positive divisor");
    if r.is_positive() {
        q + Integer::from(1)
    } else {
        q
    }
}

/// Exact floor of `a / b` for a positive divisor `b`.
fn floor_div(a: &Integer, b: &Integer) -> Integer {
    // The divisor is a positively scaled period significand, never zero.
    let (q, r) = a.div_rem(b).expect("positive divisor");
    if r.is_negative() {
        q - Integer::from(1)
    } else {
        q
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.repr {
            Repr::Nan => f.write_str("NaN"),
            Repr::Empty => f.write_str("[empty]"),
            Repr::Range { lo, hi } => write!(f, "[{lo}, {hi}]"),
        }
    }
}

impl fmt::Debug for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Normal cases -----------------------------------------------------

    #[test]
    fn normal_make_range_rounds_outward() {
        // Endpoints round away from the true value: lo down, hi up, at the
        // shared working precision.
        let third = BigFloat::one() / BigFloat::from_i64(3);
        let r = make_range(third.clone(), third.clone());
        let lo = r.lower().unwrap();
        let hi = r.upper().unwrap();
        assert!(lo <= third);
        assert!(hi >= third);
        assert_eq!(lo, third.round_to(DEFAULT_PRECISION, Rounding::Down));
        assert_eq!(hi, third.round_to(DEFAULT_PRECISION, Rounding::Up));
    }

    #[test]
    fn normal_make_range_uses_max_precision() {
        // The wider endpoint precision wins for both endpoints; directed
        // rounding keeps lo below hi around the true 1/3.
        let third = BigFloat::one() / BigFloat::from_i64(3);
        let lo = third.round_to(20, Rounding::Down);
        let hi = third.round_to(100, Rounding::Up);
        let r = make_range(lo.clone(), hi.clone());
        assert_eq!(r.lower().unwrap(), lo.round_to(100, Rounding::Down));
        assert_eq!(r.upper().unwrap(), hi.round_to(100, Rounding::Up));
        assert_eq!(r.lower().unwrap().precision(), 100);
        assert_eq!(r.upper().unwrap().precision(), 100);
    }

    #[test]
    fn normal_make_range_equal_endpoints_is_point() {
        let r = make_range(BigFloat::from_i64(2), BigFloat::from_i64(2));
        assert!(r.is_point());
        assert!(r.contains(&BigFloat::from_i64(2)));
    }

    // ---- Extreme cases ----------------------------------------------------

    #[test]
    fn extreme_make_range_unlimited_precision_rounds_outward() {
        // Unlimited-precision endpoints fall back to DEFAULT_PRECISION; a
        // significand wider than that must round strictly outward.
        let x = (BigFloat::one() / BigFloat::from_i64(3)).with_precision(0);
        let quarter = BigFloat::from_f64(0.25).with_precision(0);
        let wide = x.clone() + x * quarter;
        assert_eq!(wide.precision(), 0);
        let r = make_range(wide.clone(), wide.clone());
        let lo = r.lower().unwrap();
        let hi = r.upper().unwrap();
        assert_eq!(lo.precision(), DEFAULT_PRECISION);
        assert_eq!(hi.precision(), DEFAULT_PRECISION);
        assert!(lo < wide);
        assert!(wide < hi);
        assert_eq!(lo, wide.round_to(DEFAULT_PRECISION, Rounding::Down));
        assert_eq!(hi, wide.round_to(DEFAULT_PRECISION, Rounding::Up));
    }

    #[test]
    fn extreme_make_range_lo_above_hi_is_empty() {
        let r = make_range(BigFloat::from_i64(3), BigFloat::from_i64(1));
        assert!(r.is_empty());
        assert!(!r.is_nan());
        assert!(r.lower().is_none());
    }

    // ---- Error cases (异常报错) -------------------------------------------

    #[test]
    fn error_make_range_nan_endpoints() {
        assert!(make_range(BigFloat::NAN, BigFloat::one()).is_nan());
        assert!(make_range(BigFloat::one(), BigFloat::NAN).is_nan());
        assert!(make_range(BigFloat::NAN, BigFloat::NAN).is_nan());
    }

    // ---- Private helpers (unit layer) --------------------------------------

    #[test]
    fn normal_ceil_div_floor_div_signs() {
        let two = Integer::from(2);
        let three = Integer::from(3);
        assert_eq!(ceil_div(&Integer::from(7), &two), Integer::from(4));
        assert_eq!(floor_div(&Integer::from(7), &two), Integer::from(3));
        assert_eq!(ceil_div(&Integer::from(-7), &two), Integer::from(-3));
        assert_eq!(floor_div(&Integer::from(-7), &two), Integer::from(-4));
        assert_eq!(ceil_div(&Integer::from(6), &two), Integer::from(3));
        assert_eq!(floor_div(&Integer::from(6), &two), Integer::from(3));
        assert_eq!(ceil_div(&Integer::from(-6), &two), Integer::from(-3));
        assert_eq!(floor_div(&Integer::from(-6), &two), Integer::from(-3));
        assert_eq!(ceil_div(&Integer::from(0), &three), Integer::from(0));
        assert_eq!(floor_div(&Integer::from(0), &three), Integer::from(0));
    }

    #[test]
    fn extreme_ceil_div_floor_div_huge_divisor() {
        let huge = Integer::from(2).pow(100);
        assert_eq!(ceil_div(&Integer::from(1), &huge), Integer::from(1));
        assert_eq!(floor_div(&Integer::from(1), &huge), Integer::from(0));
        assert_eq!(ceil_div(&Integer::from(-1), &huge), Integer::from(0));
        assert_eq!(floor_div(&Integer::from(-1), &huge), Integer::from(-1));
    }

    #[test]
    fn normal_ratio_parts_exact_quotients() {
        let bf = |v: f64| BigFloat::from_f64(v);
        // (2.5 - 0.5) / 2 = 1.
        let (n, d) = ratio_parts(&bf(2.5), &bf(0.5), &bf(2.0));
        assert_eq!(n, d);
        // (0.5 - 0) / 2 = 1/4.
        let (n, d) = ratio_parts(&bf(0.5), &bf(0.0), &bf(2.0));
        assert_eq!(n, Integer::from(1));
        assert_eq!(d, Integer::from(4));
        // (-0.5 - 0) / 2 = -1/4.
        let (n, d) = ratio_parts(&bf(-0.5), &bf(0.0), &bf(2.0));
        assert_eq!(n, Integer::from(-1));
        assert_eq!(d, Integer::from(4));
        // (1 - 0) / 0.5 = 2: the period exponent below the alignment moves
        // the power into the numerator.
        let (n, d) = ratio_parts(&bf(1.0), &bf(0.0), &bf(0.5));
        assert_eq!(n, Integer::from(2));
        assert_eq!(d, Integer::from(1));
    }

    fn exact_lattice() -> Lattice {
        // Points at the even integers, with exact (degenerate) bounds.
        Lattice {
            offset_lo: BigFloat::zero(),
            offset_hi: BigFloat::zero(),
            period_lo: BigFloat::from_i64(2),
            period_hi: BigFloat::from_i64(2),
        }
    }

    #[test]
    fn normal_lattice_exact_integer_points() {
        let lat = exact_lattice();
        let iv = |a: f64, b: f64| (BigFloat::from_f64(a), BigFloat::from_f64(b));
        let (lo, hi) = iv(0.0, 0.0);
        assert!(lat.intersects(&lo, &hi));
        let (lo, hi) = iv(2.0, 2.0);
        assert!(lat.intersects(&lo, &hi));
        let (lo, hi) = iv(1.0, 1.0);
        assert!(!lat.intersects(&lo, &hi));
        let (lo, hi) = iv(0.5, 1.5);
        assert!(!lat.intersects(&lo, &hi));
        let (lo, hi) = iv(-0.5, 0.5);
        assert!(lat.intersects(&lo, &hi));
        let (lo, hi) = iv(1.0, 3.0);
        assert!(lat.intersects(&lo, &hi));
        let (lo, hi) = iv(0.1, 0.9);
        assert!(!lat.intersects(&lo, &hi));
    }

    #[test]
    fn extreme_lattice_negative_indices() {
        let lat = exact_lattice();
        let (lo, hi) = (BigFloat::from_i64(-2), BigFloat::from_i64(-2));
        assert!(lat.intersects(&lo, &hi));
        let (lo, hi) = (BigFloat::from_i64(-4), BigFloat::from_i64(-4));
        assert!(lat.intersects(&lo, &hi));
        let (lo, hi) = (BigFloat::from_i64(-3), BigFloat::from_i64(-1));
        assert!(lat.intersects(&lo, &hi));
        let (lo, hi) = (BigFloat::from_f64(-1.5), BigFloat::from_f64(-0.5));
        assert!(!lat.intersects(&lo, &hi));
        // Offset near zero with uncertain bounds: k = -1 spans [-2.2, -1.8].
        let lat = Lattice {
            offset_lo: BigFloat::from_f64(-0.1),
            offset_hi: BigFloat::from_f64(0.1),
            period_lo: BigFloat::from_f64(1.9),
            period_hi: BigFloat::from_f64(2.1),
        };
        let (lo, hi) = (BigFloat::from_f64(-2.0), BigFloat::from_f64(-2.0));
        assert!(lat.intersects(&lo, &hi));
    }

    #[test]
    fn normal_lattice_uncertain_bounds_conservative() {
        // Offset in [0.9, 1.1], period in [1.9, 2.1]: a point query inside
        // the offset uncertainty must be included (never missed).
        let lat = Lattice {
            offset_lo: BigFloat::from_f64(0.9),
            offset_hi: BigFloat::from_f64(1.1),
            period_lo: BigFloat::from_f64(1.9),
            period_hi: BigFloat::from_f64(2.1),
        };
        let (lo, hi) = (BigFloat::from_f64(0.95), BigFloat::from_f64(0.95));
        assert!(lat.intersects(&lo, &hi));
        let (lo, hi) = (BigFloat::from_f64(1.05), BigFloat::from_f64(1.05));
        assert!(lat.intersects(&lo, &hi));
        // k = 1 spans [2.8, 3.2].
        let (lo, hi) = (BigFloat::from_f64(2.85), BigFloat::from_f64(2.9));
        assert!(lat.intersects(&lo, &hi));
        // Rigorously outside every possible point.
        let (lo, hi) = (BigFloat::from_f64(0.8), BigFloat::from_f64(0.85));
        assert!(!lat.intersects(&lo, &hi));
        let (lo, hi) = (BigFloat::from_f64(1.2), BigFloat::from_f64(1.8));
        assert!(!lat.intersects(&lo, &hi));
    }

    #[test]
    fn normal_pi_bounds_rigorous() {
        for prec in [10usize, 53, 165] {
            let (lo, hi) = pi_bounds(prec);
            // A much finer pi sits strictly inside the rigorous bounds.
            let reference = BigFloat::pi_with_precision(prec + 60);
            assert!(lo < hi);
            assert!(lo <= reference);
            assert!(reference <= hi);
        }
    }

    #[test]
    fn normal_working_prec_fallback() {
        let a = BigFloat::from_i64(1).with_precision(20);
        let b = BigFloat::from_i64(2).with_precision(100);
        assert_eq!(working_prec(&a, &b), 100);
        let u = a.with_precision(0);
        let v = b.with_precision(0);
        assert_eq!(working_prec(&u, &v), DEFAULT_PRECISION);
        assert_eq!(working_prec(&u, &b), 100);
    }

    #[test]
    fn normal_pow2_frac_exact() {
        // 2^(2-10) = 2^-8 = 0.00390625, exactly representable.
        let e = pow2_frac(10, 2);
        assert_eq!(e, BigFloat::from_f64(0.00390625));
        let e = pow2_frac(53, 3);
        assert_eq!(
            e,
            BigFloat::one() / BigFloat::from_i64(2).pow(&BigFloat::from_i64(50))
        );
    }
}
