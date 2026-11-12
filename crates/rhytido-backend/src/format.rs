//! Rounding to standard IEEE floating-point formats.
//!
//! M0 implements precision rounding to each format's significand width with a
//! chosen [`Rounding`] mode. Overflow/underflow and subnormal handling, and the
//! double-double / triple-double expansion formats, are refined in M2.

use crate::{BigFloat, Rounding};

/// A target floating-point format (single-component formats in M0).
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
    /// x87 80-bit double-extended, 64-bit significand.
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
}

impl BigFloat {
    /// Rounds this value to the significand precision of `format` using `mode`.
    ///
    /// NaN and infinities are preserved. M0 performs precision rounding;
    /// overflow/underflow/subnormal behavior is refined in M2.
    ///
    /// # Examples
    ///
    /// ```
    /// use rhytido_backend::{BigFloat, Format, Rounding};
    ///
    /// let x = BigFloat::one() / BigFloat::from_i64(3);
    /// let d = x.round_to_format(Format::Double, Rounding::Nearest);
    /// assert!(d.precision() <= 53);
    /// ```
    pub fn round_to_format(&self, format: Format, mode: Rounding) -> BigFloat {
        if self.is_nan() || self.is_infinite() {
            return self.clone();
        }
        self.round_to(format.precision(), mode)
    }
}
