//! Evaluation context: explicitly-threaded settings (zero global state).
//!
//! Per the architecture (ADR / AGENTS.md §3), settings are carried by a
//! [`Context`] value passed through the engine rather than stored globally, so
//! multiple independent evaluations can coexist. M1 carries the working
//! precision and rounding mode; M2 adds the Taylor recursion setting; further
//! settings (cutoff, display, recursion limits) are added as the engine grows.

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
    taylor_recursions: u32,
}

impl Context {
    /// Creates a context with the default working precision, nearest-even
    /// rounding, and no extra Taylor recursions.
    pub fn new() -> Self {
        Self {
            precision: DEFAULT_PRECISION,
            rounding: Rounding::Nearest,
            taylor_recursions: 0,
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

    /// Sets the Taylor recursion setting `k` for the interval refiner.
    ///
    /// The mean-value refinement rule is applied **at least once**: `k = 0`
    /// applies it once, and `k` applies it `k + 1` times (each extra recursion
    /// re-applies the rule to the derivative). Larger `k` can tighten enclosures
    /// of dependency-heavy expressions at the cost of more differentiation and
    /// interval evaluations. The default is `0`.
    pub fn with_taylor_recursions(mut self, k: u32) -> Self {
        self.taylor_recursions = k;
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

    /// Returns the Taylor recursion setting `k` (see
    /// [`Context::with_taylor_recursions`]). The refinement rule is applied
    /// `k + 1` times; the default is `0` (applied once).
    pub fn taylor_recursions(&self) -> u32 {
        self.taylor_recursions
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
