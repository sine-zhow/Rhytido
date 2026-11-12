//! Arbitrary-precision binary floating-point numbers.
//!
//! Facade over the trusted backend float representation (facade isolation,
//! ADR-2). Values are base-2 with precision measured in bits (matching
//! Sollya/MPFR). The backend has no NaN, so this facade adds NaN and handles
//! IEEE-754 special-value semantics explicitly; only finite operands are passed
//! to the backend.
//!
//! M0 uses round-to-nearest-even for arithmetic; [`BigFloat::round_to`] provides
//! directed rounding to any supported [`Rounding`] mode.

use core::cmp::Ordering;
use core::fmt;
use core::ops::{Add, Div, Mul, Neg, Sub};
use core::str::FromStr;

use dashu_float::round::mode;
use dashu_float::{DBig, FBig};
use dashu_int::{IBig, Sign};

use crate::{Error, Integer};

/// Default working precision in bits (Sollya's default is 165).
pub const DEFAULT_PRECISION: usize = 165;

/// Binary float with round-to-nearest-even, precision in bits.
type FBin = FBig<mode::HalfEven, 2>;

/// Backend predicate helpers. dashu exposes these on `Repr` / via `Sign` rather
/// than as inherent `FBig` methods; this private extension keeps the facade's
/// call sites uniform. `is_zero` excludes infinities (whose significand is also
/// zero); sign predicates include signed zero and signed infinity.
trait FBinExt {
    fn is_infinite(&self) -> bool;
    fn is_zero(&self) -> bool;
    fn is_sign_negative(&self) -> bool;
    fn is_sign_positive(&self) -> bool;
}

impl FBinExt for FBin {
    fn is_infinite(&self) -> bool {
        self.repr().is_infinite()
    }

    fn is_zero(&self) -> bool {
        self.repr().significand().is_zero() && !self.repr().is_infinite()
    }

    fn is_sign_negative(&self) -> bool {
        self.sign() == Sign::Negative
    }

    fn is_sign_positive(&self) -> bool {
        self.sign() == Sign::Positive
    }
}

/// Rounding modes supported by the facade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rounding {
    /// To nearest, ties to even (IEEE roundTiesToEven, Sollya RN).
    Nearest,
    /// Toward zero (truncation, Sollya RZ).
    Zero,
    /// Toward +infinity (Sollya RU).
    Up,
    /// Toward -infinity (Sollya RD).
    Down,
    /// Away from zero.
    Away,
    /// To nearest, ties away from zero.
    HalfAway,
}

#[derive(Clone)]
enum Value {
    Nan,
    Num(FBin),
}

/// Integer rounding selector for the private `integer_round` kernel.
#[derive(Clone, Copy)]
enum IntRounding {
    /// Toward -infinity.
    Floor,
    /// Toward +infinity.
    Ceil,
    /// To nearest, ties to even.
    NearestEven,
}

/// An arbitrary-precision binary floating-point number, including NaN and
/// signed infinities.
///
/// # Examples
///
/// ```
/// use rhytido_backend::BigFloat;
///
/// let a = BigFloat::from_i64(3);
/// let b = BigFloat::from_i64(2);
/// assert_eq!((a.clone() + b.clone()).to_string(), "5");
/// assert!((a.clone() / b).to_string().starts_with("1.5"));
/// assert!(BigFloat::from_i64(1).div_zero().is_infinite());
/// ```
#[derive(Clone)]
pub struct BigFloat {
    value: Value,
}

impl BigFloat {
    /// Not-a-number.
    pub const NAN: Self = Self { value: Value::Nan };

    /// Positive infinity.
    pub fn infinity() -> Self {
        Self::num(FBin::INFINITY)
    }

    /// Negative infinity.
    pub fn neg_infinity() -> Self {
        Self::num(-FBin::INFINITY)
    }

    /// Zero.
    pub fn zero() -> Self {
        Self::from_i64(0)
    }

    /// One.
    pub fn one() -> Self {
        Self::from_i64(1)
    }

    fn num(f: FBin) -> Self {
        Self {
            value: Value::Num(f),
        }
    }

    /// Constructs from a 64-bit signed integer (exact).
    pub fn from_i64(v: i64) -> Self {
        let f = FBin::from_parts(IBig::from(v), 0);
        Self::num(f.with_precision(DEFAULT_PRECISION).value())
    }

    /// Constructs from an `f64`. NaN and infinities map to the corresponding
    /// special values; finite values are represented exactly.
    pub fn from_f64(v: f64) -> Self {
        if v.is_nan() {
            return Self::NAN;
        }
        if v.is_infinite() {
            return if v > 0.0 {
                Self::infinity()
            } else {
                Self::neg_infinity()
            };
        }
        match FBin::try_from(v) {
            Ok(f) => Self::num(f.with_precision(DEFAULT_PRECISION).value()),
            Err(_) => Self::NAN,
        }
    }

    /// Constructs from an exact [`Integer`]. The value is stored exactly; the
    /// working precision is the significand's bit length (never rounded, so
    /// integers wider than [`DEFAULT_PRECISION`] survive intact).
    pub(crate) fn from_integer(i: Integer) -> Self {
        Self::num(FBin::from_parts(i.into_ibig(), 0))
    }

    /// Returns the precision in bits (0 means unlimited; NaN reports 0).
    pub fn precision(&self) -> usize {
        match &self.value {
            Value::Nan => 0,
            Value::Num(f) => f.precision(),
        }
    }

    /// Returns this value rounded to `prec` bits (nearest-even).
    pub fn with_precision(&self, prec: usize) -> Self {
        match &self.value {
            Value::Nan => Self::NAN,
            Value::Num(f) => Self::num(f.clone().with_precision(prec).value()),
        }
    }

    /// Returns this value re-rounded to `prec` bits under `mode`.
    ///
    /// Directed rounding takes effect when the precision shrinks; NaN is
    /// preserved. Infinities are unchanged.
    pub fn round_to(&self, prec: usize, mode: Rounding) -> Self {
        let f = match &self.value {
            Value::Nan => return Self::NAN,
            Value::Num(f) => f.clone(),
        };
        let rounded = match mode {
            Rounding::Nearest => round_mode!(f, prec, mode::HalfEven),
            Rounding::Zero => round_mode!(f, prec, mode::Zero),
            Rounding::Up => round_mode!(f, prec, mode::Up),
            Rounding::Down => round_mode!(f, prec, mode::Down),
            Rounding::Away => round_mode!(f, prec, mode::Away),
            Rounding::HalfAway => round_mode!(f, prec, mode::HalfAway),
        };
        Self::num(rounded)
    }

    /// Returns `true` if this is NaN.
    pub fn is_nan(&self) -> bool {
        matches!(self.value, Value::Nan)
    }

    /// Returns `true` if this is an infinity.
    pub fn is_infinite(&self) -> bool {
        match &self.value {
            Value::Num(f) => f.is_infinite(),
            Value::Nan => false,
        }
    }

    /// Returns `true` if this is finite (not NaN, not infinity).
    pub fn is_finite(&self) -> bool {
        match &self.value {
            Value::Num(f) => !f.is_infinite(),
            Value::Nan => false,
        }
    }

    /// Returns `true` if this is zero (either sign).
    pub fn is_zero(&self) -> bool {
        match &self.value {
            Value::Num(f) => f.is_zero(),
            Value::Nan => false,
        }
    }

    /// Returns `true` if the sign bit is negative (includes -0 and -infinity).
    pub fn is_sign_negative(&self) -> bool {
        match &self.value {
            Value::Num(f) => f.is_sign_negative(),
            Value::Nan => false,
        }
    }

    /// Returns `true` if the sign bit is positive (includes +0 and +infinity).
    pub fn is_sign_positive(&self) -> bool {
        match &self.value {
            Value::Num(f) => f.is_sign_positive(),
            Value::Nan => false,
        }
    }

    /// Returns the canonical `(significand, exponent)` representation for
    /// hashing / interning, or `None` for NaN. The significand is normalized
    /// (trailing zero digits folded into the exponent), so equal values at any
    /// precision yield the same parts; signed zero is distinguished by the
    /// exponent. Exposed as facade types (no backend type leaks).
    pub fn canonical_parts(&self) -> Option<(Integer, isize)> {
        match &self.value {
            Value::Nan => None,
            Value::Num(f) => Some((
                Integer::from_ibig(f.repr().significand().clone()),
                f.repr().exponent(),
            )),
        }
    }

    /// Square root. NaN for negative finite input and -infinity; +infinity for
    /// +infinity; preserves the sign of zero.
    pub fn sqrt(&self) -> Self {
        match &self.value {
            Value::Nan => Self::NAN,
            Value::Num(f) => {
                if f.is_infinite() {
                    if f.is_sign_negative() {
                        Self::NAN
                    } else {
                        Self::infinity()
                    }
                } else if f.is_sign_negative() && !f.is_zero() {
                    Self::NAN
                } else {
                    Self::num(f.sqrt())
                }
            }
        }
    }

    /// Exponential. `exp(-infinity) = 0`, `exp(+infinity) = +infinity`.
    pub fn exp(&self) -> Self {
        match &self.value {
            Value::Nan => Self::NAN,
            Value::Num(f) => {
                if f.is_infinite() {
                    if f.is_sign_negative() {
                        Self::zero()
                    } else {
                        Self::infinity()
                    }
                } else {
                    Self::num(f.exp())
                }
            }
        }
    }

    /// Natural logarithm. NaN for negative input; `-infinity` for zero;
    /// `+infinity` for `+infinity`.
    pub fn ln(&self) -> Self {
        match &self.value {
            Value::Nan => Self::NAN,
            Value::Num(f) => {
                if f.is_infinite() {
                    if f.is_sign_negative() {
                        Self::NAN
                    } else {
                        Self::infinity()
                    }
                } else if f.is_zero() {
                    Self::neg_infinity()
                } else if f.is_sign_negative() {
                    Self::NAN
                } else {
                    Self::num(f.ln())
                }
            }
        }
    }

    /// Sine. NaN for NaN or ±infinity input.
    pub fn sin(&self) -> Self {
        match &self.value {
            Value::Nan => Self::NAN,
            Value::Num(f) => {
                if f.is_infinite() {
                    Self::NAN
                } else {
                    Self::num(f.sin())
                }
            }
        }
    }

    /// Cosine. NaN for NaN or ±infinity input.
    pub fn cos(&self) -> Self {
        match &self.value {
            Value::Nan => Self::NAN,
            Value::Num(f) => {
                if f.is_infinite() {
                    Self::NAN
                } else {
                    Self::num(f.cos())
                }
            }
        }
    }

    /// Tangent. NaN for NaN or ±infinity input; infinity at odd multiples of
    /// pi/2 (returned as a value by the backend).
    pub fn tan(&self) -> Self {
        match &self.value {
            Value::Nan => Self::NAN,
            Value::Num(f) => {
                if f.is_infinite() {
                    Self::NAN
                } else {
                    Self::num(f.tan())
                }
            }
        }
    }

    /// Arcsine. NaN for NaN, ±infinity, or |x| > 1 (out of domain).
    pub fn asin(&self) -> Self {
        if self.is_nan() || self.is_infinite() {
            return Self::NAN;
        }
        let one = Self::one();
        let neg_one = -one.clone();
        if self > &one || self < &neg_one {
            return Self::NAN;
        }
        match &self.value {
            Value::Num(f) => Self::num(f.asin()),
            Value::Nan => Self::NAN,
        }
    }

    /// Arccosine. NaN for NaN, ±infinity, or |x| > 1 (out of domain).
    pub fn acos(&self) -> Self {
        if self.is_nan() || self.is_infinite() {
            return Self::NAN;
        }
        let one = Self::one();
        let neg_one = -one.clone();
        if self > &one || self < &neg_one {
            return Self::NAN;
        }
        match &self.value {
            Value::Num(f) => Self::num(f.acos()),
            Value::Nan => Self::NAN,
        }
    }

    /// Arctangent. NaN for NaN; `±pi/2` for `±infinity`.
    pub fn atan(&self) -> Self {
        match &self.value {
            Value::Nan => Self::NAN,
            Value::Num(f) => {
                if f.is_infinite() {
                    // The infinite operand carries no working precision, and the
                    // backend cannot compute pi at unlimited precision. Derive
                    // pi/2 = 2*atan(1) at a finite precision instead.
                    let half_pi = Self::one().atan() * Self::from_i64(2);
                    if f.is_sign_negative() {
                        -half_pi
                    } else {
                        half_pi
                    }
                } else {
                    Self::num(f.atan())
                }
            }
        }
    }

    /// Hyperbolic sine. `±infinity` for `±infinity`; NaN for NaN.
    pub fn sinh(&self) -> Self {
        match &self.value {
            Value::Nan => Self::NAN,
            Value::Num(f) => {
                if f.is_infinite() {
                    if f.is_sign_negative() {
                        Self::neg_infinity()
                    } else {
                        Self::infinity()
                    }
                } else {
                    Self::num(f.sinh())
                }
            }
        }
    }

    /// Hyperbolic cosine. `+infinity` for `±infinity`; NaN for NaN.
    pub fn cosh(&self) -> Self {
        match &self.value {
            Value::Nan => Self::NAN,
            Value::Num(f) => {
                if f.is_infinite() {
                    Self::infinity()
                } else {
                    Self::num(f.cosh())
                }
            }
        }
    }

    /// Hyperbolic tangent. `±1` for `±infinity`; NaN for NaN.
    pub fn tanh(&self) -> Self {
        match &self.value {
            Value::Nan => Self::NAN,
            Value::Num(f) => {
                if f.is_infinite() {
                    if f.is_sign_negative() {
                        -Self::one()
                    } else {
                        Self::one()
                    }
                } else {
                    Self::num(f.tanh())
                }
            }
        }
    }

    /// Inverse hyperbolic sine. `±infinity` for `±infinity`; NaN for NaN.
    pub fn asinh(&self) -> Self {
        match &self.value {
            Value::Nan => Self::NAN,
            Value::Num(f) => {
                if f.is_infinite() {
                    if f.is_sign_negative() {
                        Self::neg_infinity()
                    } else {
                        Self::infinity()
                    }
                } else {
                    Self::num(f.asinh())
                }
            }
        }
    }

    /// Inverse hyperbolic cosine. NaN for NaN, -infinity, or x < 1 (out of
    /// domain); `+infinity` for `+infinity`.
    pub fn acosh(&self) -> Self {
        if self.is_nan() {
            return Self::NAN;
        }
        if self.is_infinite() {
            return if self.is_sign_negative() {
                Self::NAN
            } else {
                Self::infinity()
            };
        }
        let one = Self::one();
        if self < &one {
            return Self::NAN;
        }
        match &self.value {
            Value::Num(f) => Self::num(f.acosh()),
            Value::Nan => Self::NAN,
        }
    }

    /// Inverse hyperbolic tangent. NaN for NaN, ±infinity, or |x| > 1;
    /// `±infinity` for `x = ±1`.
    pub fn atanh(&self) -> Self {
        if self.is_nan() || self.is_infinite() {
            return Self::NAN;
        }
        let one = Self::one();
        let neg_one = -one.clone();
        if self > &one || self < &neg_one {
            return Self::NAN;
        }
        if self == &one {
            return Self::infinity();
        }
        if self == &neg_one {
            return Self::neg_infinity();
        }
        match &self.value {
            Value::Num(f) => Self::num(f.atanh()),
            Value::Nan => Self::NAN,
        }
    }

    /// Base-2 logarithm. NaN for negative input or -infinity; `-infinity` for
    /// zero (either sign); `+infinity` for `+infinity`.
    pub fn log2(&self) -> Self {
        match &self.value {
            Value::Nan => Self::NAN,
            Value::Num(f) => {
                if f.is_infinite() {
                    if f.is_sign_negative() {
                        Self::NAN
                    } else {
                        Self::infinity()
                    }
                } else if f.is_zero() {
                    Self::neg_infinity()
                } else if f.is_sign_negative() {
                    Self::NAN
                } else {
                    Self::num(f.log2())
                }
            }
        }
    }

    /// Base-10 logarithm. NaN for negative input or -infinity; `-infinity`
    /// for zero (either sign); `+infinity` for `+infinity`.
    pub fn log10(&self) -> Self {
        match &self.value {
            Value::Nan => Self::NAN,
            Value::Num(f) => {
                if f.is_infinite() {
                    if f.is_sign_negative() {
                        Self::NAN
                    } else {
                        Self::infinity()
                    }
                } else if f.is_zero() {
                    Self::neg_infinity()
                } else if f.is_sign_negative() {
                    Self::NAN
                } else {
                    Self::num(f.log10())
                }
            }
        }
    }

    /// Natural logarithm of `1 + x`, accurate for small `x`. NaN for `x < -1`
    /// or -infinity; `-infinity` for `x = -1`; `+infinity` for `+infinity`;
    /// preserves the sign of zero.
    pub fn log1p(&self) -> Self {
        match &self.value {
            Value::Nan => Self::NAN,
            Value::Num(f) => {
                if f.is_infinite() {
                    if f.is_sign_negative() {
                        Self::NAN
                    } else {
                        Self::infinity()
                    }
                } else if f.is_sign_negative() && !f.is_zero() {
                    // The pole at -1 is compared exactly: a value that merely
                    // rounds onto -1 still has a finite logarithm.
                    let neg_one = -FBin::from_parts(IBig::from(1), 0);
                    if f < &neg_one {
                        Self::NAN
                    } else if f == &neg_one {
                        Self::neg_infinity()
                    } else {
                        Self::num(f.ln_1p())
                    }
                } else {
                    Self::num(f.ln_1p())
                }
            }
        }
    }

    /// Exponential minus one, accurate for small `x`. `+infinity` for
    /// `+infinity`; `-1` for -infinity; preserves the sign of zero.
    pub fn expm1(&self) -> Self {
        match &self.value {
            Value::Nan => Self::NAN,
            Value::Num(f) => {
                if f.is_infinite() {
                    if f.is_sign_negative() {
                        -Self::one()
                    } else {
                        Self::infinity()
                    }
                } else {
                    Self::num(f.exp_m1())
                }
            }
        }
    }

    /// Absolute value. NaN for NaN; `+infinity` for either infinity; `-0`
    /// becomes `+0`; other values keep their precision.
    pub fn abs(&self) -> Self {
        match &self.value {
            Value::Nan => Self::NAN,
            Value::Num(f) => {
                if f.is_infinite() {
                    Self::infinity()
                } else if f.is_sign_negative() {
                    Self::num(-f.clone())
                } else {
                    self.clone()
                }
            }
        }
    }

    /// Smallest integer not less than this value. NaN for NaN; infinities are
    /// unchanged; the result is exact.
    pub fn ceil(&self) -> Self {
        self.integer_round(IntRounding::Ceil)
    }

    /// Largest integer not greater than this value. NaN for NaN; infinities
    /// are unchanged; the result is exact.
    pub fn floor(&self) -> Self {
        self.integer_round(IntRounding::Floor)
    }

    /// Nearest integer, ties to even. NaN for NaN; infinities are unchanged;
    /// the result is exact.
    pub fn nearestint(&self) -> Self {
        self.integer_round(IntRounding::NearestEven)
    }

    /// Shared integer-rounding kernel: exact integer arithmetic on the
    /// canonical `(significand, exponent)` parts, so no backend rounding is
    /// involved and the result is the exact mathematical integer.
    fn integer_round(&self, mode: IntRounding) -> Self {
        if self.is_infinite() {
            return self.clone();
        }
        let (sig, exp) = match self.canonical_parts() {
            Some(parts) => parts,
            None => return Self::NAN,
        };
        if sig.is_zero() || exp >= 0 {
            // Already an integer (including signed zero): keep it verbatim.
            return self.clone();
        }
        // Below half in magnitude the result follows from the sign alone.
        // This also bounds the divisor below by the significand's own size,
        // so extreme negative exponents never materialize a huge power.
        let half = Self::from_f64(0.5);
        let neg_half = -half.clone();
        if self > &neg_half && self < &half {
            let rounded = match mode {
                IntRounding::Floor => {
                    if sig.is_negative() {
                        Integer::from(-1)
                    } else {
                        Integer::from(0)
                    }
                }
                IntRounding::Ceil => {
                    if sig.is_positive() {
                        Integer::from(1)
                    } else {
                        Integer::from(0)
                    }
                }
                IntRounding::NearestEven => Integer::from(0),
            };
            return Self::from_integer(rounded);
        }
        // The value is sig / 2^shift; truncating division splits off the
        // fractional remainder r (same sign as sig, |r| < divisor).
        let shift = exp.unsigned_abs();
        let divisor = Integer::from(2).pow(shift);
        let (q, r) = match sig.div_rem(&divisor) {
            Ok(parts) => parts,
            // The divisor is a positive power of two, so this cannot fail.
            Err(_) => return Self::NAN,
        };
        let step = if sig.is_negative() {
            Integer::from(-1)
        } else {
            Integer::from(1)
        };
        let rounded = match mode {
            IntRounding::Floor => {
                if sig.is_negative() && !r.is_zero() {
                    q + step
                } else {
                    q
                }
            }
            IntRounding::Ceil => {
                if sig.is_positive() && !r.is_zero() {
                    q + step
                } else {
                    q
                }
            }
            IntRounding::NearestEven => {
                let abs_r = if r.is_negative() { -&r } else { r.clone() };
                let twice = &abs_r + &abs_r;
                match twice.cmp(&divisor) {
                    Ordering::Less => q,
                    Ordering::Greater => q + step,
                    // Exact half: keep the even neighbor.
                    Ordering::Equal => {
                        let odd = q
                            .checked_rem(&Integer::from(2))
                            .is_some_and(|rem| !rem.is_zero());
                        if odd { q + step } else { q }
                    }
                }
            }
        };
        Self::from_integer(rounded)
    }

    /// Returns `true` if this value is a finite integer.
    pub fn is_integer(&self) -> bool {
        if self.is_nan() || self.is_infinite() {
            return false;
        }
        match self.canonical_parts() {
            Some((sig, exp)) => sig.is_zero() || exp >= 0,
            None => false,
        }
    }

    /// Computes pi at the given working precision, as `4*atan(1)`.
    pub fn pi_with_precision(prec: usize) -> Self {
        let one = Self::one().with_precision(prec);
        one.atan() * Self::from_i64(4)
    }

    /// Real power `self^exp` with IEEE special-value handling. NaN for a
    /// negative base raised to a non-integer exponent.
    pub fn pow(&self, exp: &Self) -> Self {
        let one = Self::one();
        // x^0 = 1 and 1^y = 1 for any operand (IEEE pow).
        if exp.is_zero() {
            return one;
        }
        if self == &one {
            return one;
        }
        if self.is_nan() || exp.is_nan() {
            return Self::NAN;
        }
        // 0^y: y>0 -> 0, y<0 -> +infinity.
        if self.is_zero() {
            return if exp.is_sign_negative() {
                Self::infinity()
            } else {
                Self::zero()
            };
        }
        // base = +/-infinity with finite exponent.
        if self.is_infinite() {
            let mag = if exp.is_sign_negative() {
                Self::zero()
            } else {
                Self::infinity()
            };
            if self.is_sign_negative() {
                if !exp.is_integer() {
                    return Self::NAN;
                }
                // Odd integer exponent keeps the sign; even makes it positive.
                let even = (exp.clone() / Self::from_i64(2)).is_integer();
                return if even { mag } else { -mag };
            }
            return mag;
        }
        // exponent = +/-infinity: depends on |base| vs 1.
        if exp.is_infinite() {
            let abs_base = if self.is_sign_negative() {
                -self.clone()
            } else {
                self.clone()
            };
            if abs_base > one {
                return if exp.is_sign_negative() {
                    Self::zero()
                } else {
                    Self::infinity()
                };
            } else if abs_base < one {
                return if exp.is_sign_negative() {
                    Self::infinity()
                } else {
                    Self::zero()
                };
            }
            // |base| == 1 with base != 1 (i.e. -1): indeterminate here.
            return Self::NAN;
        }
        // Negative base with non-integer exponent is not real.
        if self.is_sign_negative() && !exp.is_integer() {
            return Self::NAN;
        }
        match (&self.value, &exp.value) {
            (Value::Num(b), Value::Num(e)) => Self::num(b.powf(e)),
            _ => Self::NAN,
        }
    }

    /// Helper for tests/examples: divide by zero to produce an infinity.
    #[doc(hidden)]
    pub fn div_zero(&self) -> Self {
        self.clone() / Self::zero()
    }
}

/// Re-round under a concrete backend mode, then relabel to nearest-even without
/// further rounding (the value already carries the requested mode's rounding).
macro_rules! round_mode {
    ($f:expr, $prec:expr, $m:ty) => {
        $f.with_rounding::<$m>()
            .with_precision($prec)
            .value()
            .with_rounding::<mode::HalfEven>()
    };
}
use round_mode;

impl From<i64> for BigFloat {
    fn from(v: i64) -> Self {
        Self::from_i64(v)
    }
}

impl FromStr for BigFloat {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let t = s.trim();
        match t.to_ascii_lowercase().as_str() {
            "nan" | "+nan" | "-nan" => return Ok(Self::NAN),
            "inf" | "+inf" | "infty" | "+infty" | "infinity" | "+infinity" => {
                return Ok(Self::infinity());
            }
            "-inf" | "-infty" | "-infinity" => return Ok(Self::neg_infinity()),
            _ => {}
        }
        // Parse as decimal (base 10), then convert to binary at working precision.
        let dec = DBig::from_str(t).map_err(|e| Error::Parse(e.to_string()))?;
        let bin = dec
            .with_rounding::<mode::HalfEven>()
            .with_base_and_precision::<2>(DEFAULT_PRECISION)
            .value();
        Ok(Self::num(bin))
    }
}

impl fmt::Display for BigFloat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.value {
            Value::Nan => f.write_str("NaN"),
            Value::Num(v) => {
                if v.is_infinite() {
                    if v.is_sign_negative() {
                        f.write_str("-inf")
                    } else {
                        f.write_str("inf")
                    }
                } else {
                    // Internal representation is base-2; display in decimal.
                    fmt::Display::fmt(&v.to_decimal().value(), f)
                }
            }
        }
    }
}

impl fmt::Debug for BigFloat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl PartialEq for BigFloat {
    fn eq(&self, other: &Self) -> bool {
        match (&self.value, &other.value) {
            (Value::Num(a), Value::Num(b)) => a == b,
            _ => false,
        }
    }
}

impl PartialOrd for BigFloat {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (&self.value, &other.value) {
            (Value::Num(a), Value::Num(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}

impl Neg for BigFloat {
    type Output = Self;

    fn neg(self) -> Self {
        match self.value {
            Value::Nan => Self::NAN,
            Value::Num(f) => {
                if f.is_infinite() {
                    if f.is_sign_negative() {
                        Self::infinity()
                    } else {
                        Self::neg_infinity()
                    }
                } else {
                    Self::num(-f)
                }
            }
        }
    }
}

impl Add for BigFloat {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        match (self.value, rhs.value) {
            (Value::Nan, _) | (_, Value::Nan) => Self::NAN,
            (Value::Num(a), Value::Num(b)) => {
                let (ai, bi) = (a.is_infinite(), b.is_infinite());
                if ai && bi {
                    if a.is_sign_negative() != b.is_sign_negative() {
                        Self::NAN // inf + (-inf)
                    } else {
                        Self::num(a)
                    }
                } else if ai {
                    Self::num(a)
                } else if bi {
                    Self::num(b)
                } else {
                    Self::num(a + b)
                }
            }
        }
    }
}

impl Sub for BigFloat {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        self + (-rhs)
    }
}

impl Mul for BigFloat {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        match (self.value, rhs.value) {
            (Value::Nan, _) | (_, Value::Nan) => Self::NAN,
            (Value::Num(a), Value::Num(b)) => {
                let (ai, bi) = (a.is_infinite(), b.is_infinite());
                let (az, bz) = (a.is_zero(), b.is_zero());
                if (ai && bz) || (bi && az) {
                    Self::NAN // 0 * inf
                } else if ai || bi {
                    if a.is_sign_negative() != b.is_sign_negative() {
                        Self::neg_infinity()
                    } else {
                        Self::infinity()
                    }
                } else {
                    Self::num(a * b)
                }
            }
        }
    }
}

impl Div for BigFloat {
    type Output = Self;

    fn div(self, rhs: Self) -> Self {
        match (self.value, rhs.value) {
            (Value::Nan, _) | (_, Value::Nan) => Self::NAN,
            (Value::Num(a), Value::Num(b)) => {
                let (ai, bi) = (a.is_infinite(), b.is_infinite());
                let (az, bz) = (a.is_zero(), b.is_zero());
                if (ai && bi) || (az && bz) {
                    Self::NAN // inf/inf or 0/0
                } else if bz || ai {
                    // x/0 (x != 0) or inf/finite -> signed infinity.
                    if a.is_sign_negative() != b.is_sign_negative() {
                        Self::neg_infinity()
                    } else {
                        Self::infinity()
                    }
                } else if bi {
                    // finite / infinity -> signed zero
                    let z = FBin::from_parts(IBig::from(0), 0);
                    let z = if a.is_sign_negative() != b.is_sign_negative() {
                        -z
                    } else {
                        z
                    };
                    Self::num(z.with_precision(DEFAULT_PRECISION).value())
                } else {
                    Self::num(a / b)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finite(v: i64) -> FBin {
        FBin::from_parts(IBig::from(v), 0)
            .with_precision(DEFAULT_PRECISION)
            .value()
    }

    // ---- Normal cases -----------------------------------------------------

    #[test]
    fn normal_fbin_ext_finite_signs() {
        let pos = finite(5);
        assert!(!pos.is_zero());
        assert!(!pos.is_infinite());
        assert!(pos.is_sign_positive());
        assert!(!pos.is_sign_negative());
        let neg = finite(-5);
        assert!(!neg.is_zero());
        assert!(!neg.is_infinite());
        assert!(neg.is_sign_negative());
        assert!(!neg.is_sign_positive());
    }

    #[test]
    fn normal_fbin_ext_zero_signs() {
        let zero = finite(0);
        assert!(zero.is_zero());
        assert!(!zero.is_infinite());
        assert!(zero.is_sign_positive());
        assert!(!zero.is_sign_negative());
        // Signed zero keeps its sign bit.
        let neg_zero = -zero;
        assert!(neg_zero.is_zero());
        assert!(!neg_zero.is_infinite());
        assert!(neg_zero.is_sign_negative());
        assert!(!neg_zero.is_sign_positive());
    }

    // ---- Extreme cases ----------------------------------------------------

    #[test]
    fn extreme_fbin_ext_infinity_predicates() {
        // Infinities carry a zero significand but must not count as zero.
        let inf = FBin::INFINITY;
        assert!(inf.is_infinite());
        assert!(!inf.is_zero());
        assert!(inf.is_sign_positive());
        assert!(!inf.is_sign_negative());
        let ninf = -FBin::INFINITY;
        assert!(ninf.is_infinite());
        assert!(!ninf.is_zero());
        assert!(ninf.is_sign_negative());
        assert!(!ninf.is_sign_positive());
    }

    // ---- Private integer-rounding kernel (unit layer) ---------------------

    #[test]
    fn normal_integer_round_wide_significand_tie() {
        // (2^200 + 1) / 2 = 2^199 + 0.5: an exact tie far beyond the default
        // precision; the even neighbor 2^199 must win, exactly.
        let sig = (IBig::from(1) << 200) + IBig::from(1);
        let x = BigFloat::num(FBin::from_parts(sig, -1));
        let even = BigFloat::from_integer(Integer::from_ibig(IBig::from(1) << 199));
        assert_eq!(x.integer_round(IntRounding::NearestEven), even);
        assert_eq!(x.integer_round(IntRounding::Floor), even.clone());
        let up = BigFloat::from_integer(Integer::from_ibig((IBig::from(1) << 199) + IBig::from(1)));
        assert_eq!(x.integer_round(IntRounding::Ceil), up);
    }

    #[test]
    fn normal_integer_round_wide_significand_negative_tie() {
        // -(2^199 + 0.5): ties to even toward zero; floor goes one below.
        let sig = -((IBig::from(1) << 200) + IBig::from(1));
        let x = BigFloat::num(FBin::from_parts(sig, -1));
        let even = BigFloat::from_integer(Integer::from_ibig(-(IBig::from(1) << 199)));
        assert_eq!(x.integer_round(IntRounding::NearestEven), even);
        assert_eq!(x.integer_round(IntRounding::Ceil), even.clone());
        let down = BigFloat::from_integer(Integer::from_ibig(
            -((IBig::from(1) << 199) + IBig::from(1)),
        ));
        assert_eq!(x.integer_round(IntRounding::Floor), down);
    }

    #[test]
    fn normal_integer_round_wide_significand_non_tie() {
        // (2^200 + 3) / 4 = 2^198 + 0.75: above the half, all modes but floor
        // round up.
        let sig = (IBig::from(1) << 200) + IBig::from(3);
        let x = BigFloat::num(FBin::from_parts(sig, -2));
        let base = Integer::from_ibig(IBig::from(1) << 198);
        assert_eq!(
            x.integer_round(IntRounding::Floor),
            BigFloat::from_integer(base.clone())
        );
        let up = BigFloat::from_integer(base + Integer::from(1));
        assert_eq!(x.integer_round(IntRounding::Ceil), up.clone());
        assert_eq!(x.integer_round(IntRounding::NearestEven), up);
    }

    #[test]
    fn extreme_integer_round_tiny_magnitude_shortcut() {
        // 2^-1000 is far below half; the sign-only shortcut decides without
        // materializing the huge divisor 2^1000.
        let tiny = BigFloat::num(FBin::from_parts(IBig::from(1), -1000));
        assert_eq!(tiny.integer_round(IntRounding::Floor), BigFloat::zero());
        assert_eq!(tiny.integer_round(IntRounding::Ceil), BigFloat::one());
        assert_eq!(
            tiny.integer_round(IntRounding::NearestEven),
            BigFloat::zero()
        );
        let neg = -tiny;
        assert_eq!(neg.integer_round(IntRounding::Floor), -BigFloat::one());
        assert_eq!(neg.integer_round(IntRounding::Ceil), BigFloat::zero());
        assert_eq!(
            neg.integer_round(IntRounding::NearestEven),
            BigFloat::zero()
        );
    }

    #[test]
    fn extreme_integer_round_special_values() {
        for mode in [
            IntRounding::Floor,
            IntRounding::Ceil,
            IntRounding::NearestEven,
        ] {
            assert!(BigFloat::NAN.integer_round(mode).is_nan());
            assert_eq!(
                BigFloat::infinity().integer_round(mode),
                BigFloat::infinity()
            );
            assert_eq!(
                BigFloat::neg_infinity().integer_round(mode),
                BigFloat::neg_infinity()
            );
            let neg_zero = -BigFloat::zero();
            assert!(neg_zero.integer_round(mode).is_zero());
        }
    }

    #[test]
    fn normal_from_integer_wide_exact() {
        // A 300-bit integer must survive the conversion without rounding.
        let wide = Integer::from_ibig((IBig::from(1) << 299) + IBig::from(12345));
        let f = BigFloat::from_integer(wide.clone());
        assert!(f.is_integer());
        assert_eq!(f.canonical_parts().unwrap().0, wide);
    }
}
