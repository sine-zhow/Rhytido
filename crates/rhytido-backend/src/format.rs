// SPDX-License-Identifier: Apache-2.0
//! Rounding to standard IEEE floating-point formats.
//!
//! Each [`Format`] models a complete IEEE-754 binary format: significand
//! width, normal exponent range, and the subnormal grid below the smallest
//! normal. [`BigFloat::round_to_format`] rounds the significand in the normal
//! range, quantizes onto the subnormal grid under `emin`, and applies the
//! rounding mode's overflow rule (signed infinity or the largest finite
//! value) above `emax`. Every boundary decision is exact integer arithmetic
//! on the canonical parts, so no floating-point comparison can misjudge an
//! exponent boundary.
//!
//! The double-double / triple-double expansion formats are deferred to M6.

use core::cmp::Ordering;

use dashu_int::IBig;
use dashu_int::ops::{BitTest, DivRem};

use crate::{BigFloat, Integer, Rounding};

/// A target floating-point format (single-component formats).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    /// binary64 (double), 53-bit significand.
    Double,
    /// binary32 (single), 24-bit significand.
    Single,
    /// binary16 (half), 11-bit significand.
    Half,
    /// binary128 (quad), 113-bit significand.
    Quad,
    /// Double-extended with a 64-bit significand and a 15-bit exponent field:
    /// IEEE-754 binary79-like rounding including subnormals (the Itanium
    /// behavior), not the Intel x87 pseudo-denormal behavior.
    DoubleExtended,
}

impl Format {
    /// Returns the significand precision in bits.
    pub fn precision(self) -> usize {
        match self {
            Format::Double => 53,
            Format::Single => 24,
            Format::Half => 11,
            Format::Quad => 113,
            Format::DoubleExtended => 64,
        }
    }

    /// Returns the normal exponent range `(emin, emax)`, unbiased.
    ///
    /// The smallest normal is `2^emin` and the largest finite value is
    /// `(2 - 2^(1-p)) * 2^emax` for precision `p`. Below `emin` the format
    /// keeps a subnormal grid of spacing `2^(emin - p + 1)` down to zero;
    /// the smallest subnormal is `2^(emin - p + 1)` itself.
    ///
    /// # Examples
    ///
    /// ```
    /// use rhytido_backend::Format;
    ///
    /// assert_eq!(Format::Double.exponent_range(), (-1022, 1023));
    /// assert_eq!(Format::Half.exponent_range(), (-14, 15));
    /// ```
    pub fn exponent_range(self) -> (isize, isize) {
        match self {
            Format::Double => (-1022, 1023),
            Format::Single => (-126, 127),
            Format::Half => (-14, 15),
            Format::Quad => (-16382, 16383),
            Format::DoubleExtended => (-16382, 16383),
        }
    }
}

impl BigFloat {
    /// Rounds this value to `format` under `mode` with full IEEE-754
    /// semantics: significand rounding in the normal range, subnormal-grid
    /// quantization below the smallest normal, and the mode's overflow rule
    /// above the largest finite value.
    ///
    /// NaN and infinities are preserved; signed zero keeps its sign. Overflow
    /// yields signed infinity under `Nearest`/`HalfAway`/`Away`, the largest
    /// finite value with the input's sign under `Zero`, and the finite extreme
    /// on the rounded-toward side under `Up`/`Down` (so a positive value under
    /// `Down` stays finite, a negative one under `Up` stays finite). Underflow
    /// rounds onto the subnormal grid, possibly to signed zero.
    ///
    /// The result carries the format's precision label and is exactly
    /// representable in the format (its significand fits the width and its
    /// exponent lies in the format's range, subnormals included).
    ///
    /// # Examples
    ///
    /// ```
    /// use rhytido_backend::{BigFloat, Format, Rounding};
    ///
    /// let x = BigFloat::one() / BigFloat::from_i64(3);
    /// let d = x.round_to_format(Format::Double, Rounding::Nearest);
    /// assert!(d.precision() <= 53);
    /// // The half-precision overflow tie rounds to infinity, not 65536.
    /// let h = BigFloat::from_i64(65520).round_to_format(Format::Half, Rounding::Nearest);
    /// assert!(h.is_infinite());
    /// ```
    ///
    /// # Panics
    ///
    /// Never.
    pub fn round_to_format(&self, format: Format, mode: Rounding) -> BigFloat {
        if self.is_nan() || self.is_infinite() {
            return self.clone();
        }
        let p = format.precision();
        let (emin, emax) = format.exponent_range();
        let Some((sig, exp)) = self.canonical_parts() else {
            // Unreachable: NaN was handled above.
            return Self::NAN;
        };
        if sig.is_zero() {
            // Signed zero is representable in every format; keep the sign.
            return self.with_precision(p);
        }
        let sig = sig.into_ibig();
        let negative = sig < IBig::from(0);
        // Leading-bit exponent: the magnitude lies in [2^e, 2^(e+1)).
        let e = exp + sig.bit_len() as isize - 1;
        // Rounding grid exponent: p significand bits in the normal range,
        // the fixed subnormal spacing below emin. The max folds underflow
        // into the same quantization, and a carry out of the subnormal grid
        // lands exactly on the smallest normal.
        let min_exp = emin - p as isize + 1;
        let grid = (e - p as isize + 1).max(min_exp);
        let q = quantize(&sig, exp, grid, negative, mode);
        if q.is_zero() {
            let zero = BigFloat::zero().with_precision(p);
            return if negative { -zero } else { zero };
        }
        // Overflow test after rounding: a carry out of the emax binade
        // (halfprecision(65520) -> 2^16) overflows just like a far-out value.
        if grid + q.bit_len() as isize - 1 > emax {
            return overflow_image(format, negative, mode);
        }
        // The quantized value has at most p significand bits (a carry to
        // 2^p normalizes to one bit), so relabeling never changes it.
        BigFloat::from_parts(Integer::from_ibig(q), grid).with_precision(p)
    }
}

/// Quantizes `sig * 2^exp` onto the grid `2^grid`, returning the grid-unit
/// integer. Exact integer arithmetic only: the divisor is a power of two
/// built by shifting, and the truncating quotient/remainder split carries the
/// dividend's sign, so the mode adjustment below is sign-correct by cases.
fn quantize(sig: &IBig, exp: isize, grid: isize, negative: bool, mode: Rounding) -> IBig {
    if exp >= grid {
        // The value already sits on the grid: an exact left shift. The shift
        // is bounded by p-1 (normal range) or the subnormal gap, never huge.
        return sig.clone() << (exp - grid) as usize;
    }
    let shift = (grid - exp) as usize;
    if shift > sig.bit_len() + 1 {
        // The magnitude is strictly below half a grid unit, so the quotient
        // is zero and the mode alone decides between zero and one grid step.
        // This also avoids materializing a divisor larger than the operand
        // for extremely tiny inputs.
        let one = IBig::from(1);
        return match mode {
            Rounding::Zero | Rounding::Nearest | Rounding::HalfAway => IBig::from(0),
            Rounding::Away => {
                if negative {
                    -one
                } else {
                    one
                }
            }
            Rounding::Up => {
                if negative {
                    IBig::from(0)
                } else {
                    one
                }
            }
            Rounding::Down => {
                if negative {
                    -one
                } else {
                    IBig::from(0)
                }
            }
        };
    }
    let divisor = IBig::from(1) << shift;
    let (q, r) = sig.div_rem(&divisor);
    round_quotient(q, &r, &divisor, negative, mode)
}

/// Adjusts the truncating quotient `q` to `mode` given the remainder `r`
/// (same sign as the dividend) and the positive divisor. `step` moves one
/// grid unit away from zero.
fn round_quotient(q: IBig, r: &IBig, divisor: &IBig, negative: bool, mode: Rounding) -> IBig {
    if r.is_zero() {
        return q;
    }
    let one = IBig::from(1);
    let step = if negative { -one.clone() } else { one.clone() };
    match mode {
        Rounding::Zero => q,
        Rounding::Up => {
            if negative {
                q
            } else {
                q + step
            }
        }
        Rounding::Down => {
            if negative {
                q + step
            } else {
                q
            }
        }
        Rounding::Away => q + step,
        Rounding::Nearest | Rounding::HalfAway => {
            let twice = r + r;
            let mag = if twice < IBig::from(0) { -twice } else { twice };
            match mag.cmp(divisor) {
                Ordering::Less => q,
                Ordering::Greater => q + step,
                Ordering::Equal => {
                    if mode == Rounding::HalfAway {
                        q + step
                    } else {
                        // Exact half: keep the even grid neighbor.
                        let odd = (&q & &one) != IBig::from(0);
                        if odd { q + step } else { q }
                    }
                }
            }
        }
    }
}

/// The mode's overflow image: signed infinity, or the largest finite value
/// `(2^p - 1) * 2^(emax - p + 1)` with the input's sign.
fn overflow_image(format: Format, negative: bool, mode: Rounding) -> BigFloat {
    let to_infinity = match mode {
        Rounding::Nearest | Rounding::HalfAway | Rounding::Away => true,
        Rounding::Zero => false,
        Rounding::Up => !negative,
        Rounding::Down => negative,
    };
    if to_infinity {
        return if negative {
            BigFloat::neg_infinity()
        } else {
            BigFloat::infinity()
        };
    }
    let p = format.precision();
    let (_, emax) = format.exponent_range();
    let sig = (IBig::from(1) << p) - IBig::from(1);
    let max =
        BigFloat::from_parts(Integer::from_ibig(sig), emax - p as isize + 1).with_precision(p);
    if negative { -max } else { max }
}
