//! Evaluation context: explicitly-threaded settings (zero global state).
//!
//! Per the architecture (ADR / AGENTS.md §3), settings are carried by a
//! [`Context`] value passed through the engine rather than stored globally, so
//! multiple independent evaluations can coexist. M1 carries the working
//! precision and rounding mode; further settings (cutoff, display, recursion
//! limits) are added as the engine grows.

use rhytido_backend::{DEFAULT_PRECISION, Rounding};

/// The minimum working precision, in bits. Precisions below this are clamped up
/// to it, since a meaningful binary floating-point format needs at least two
/// significand bits.
pub const MIN_PRECISION: usize = 2;

/// Explicitly-threaded evaluation settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Context {
    precision: usize,
    rounding: Rounding,
}

impl Context {
    /// Creates a context with the default working precision and nearest-even
    /// rounding.
    pub fn new() -> Self {
        Self {
            precision: DEFAULT_PRECISION,
            rounding: Rounding::Nearest,
        }
    }

    /// Sets the working precision in bits. Values below [`MIN_PRECISION`] are
    /// clamped up to it (documented, panic-free).
    pub fn with_precision(mut self, precision: usize) -> Self {
        self.precision = precision.max(MIN_PRECISION);
        self
    }

    /// Sets the rounding mode.
    pub fn with_rounding(mut self, rounding: Rounding) -> Self {
        self.rounding = rounding;
        self
    }

    /// Returns the working precision in bits.
    pub fn precision(&self) -> usize {
        self.precision
    }

    /// Returns the rounding mode.
    pub fn rounding(&self) -> Rounding {
        self.rounding
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
