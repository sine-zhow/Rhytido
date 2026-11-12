//! Floating-point evaluation result: a value paired with its quality certificate.
//!
//! An [`FpResult`] is what the faithful evaluation engine produces: the rounded
//! [`BigFloat`] value together with a [`Quality`] certificate describing the
//! rounding guarantee. The certificate is a first-class part of the result and
//! must never be discarded (AGENTS.md §3 semantic contract).

use rhytido_backend::BigFloat;

use crate::quality::Quality;

/// A floating-point evaluation result: value + quality certificate.
#[derive(Debug, Clone, PartialEq)]
pub struct FpResult {
    value: BigFloat,
    quality: Quality,
}

impl FpResult {
    /// Creates a result from a value and its quality certificate.
    pub fn new(value: BigFloat, quality: Quality) -> Self {
        Self { value, quality }
    }

    /// Creates a result known to be exact.
    pub fn exact(value: BigFloat) -> Self {
        Self::new(value, Quality::Exact)
    }

    /// Returns a reference to the value.
    pub fn value(&self) -> &BigFloat {
        &self.value
    }

    /// Returns the quality certificate.
    pub fn quality(&self) -> Quality {
        self.quality
    }

    /// Consumes the result and returns the value.
    pub fn into_value(self) -> BigFloat {
        self.value
    }

    /// Returns `true` if the result is known to be exact.
    pub fn is_exact(&self) -> bool {
        self.quality.is_exact()
    }

    /// Returns `true` if the result is at least faithfully rounded.
    pub fn is_faithful(&self) -> bool {
        self.quality.is_faithful()
    }

    /// Returns `true` if the result is correctly rounded (or exact).
    pub fn is_correctly_rounded(&self) -> bool {
        self.quality.is_correctly_rounded()
    }

    /// Returns `true` if the value is NaN or infinity.
    pub fn is_non_real(&self) -> bool {
        self.quality.is_non_real()
    }
}
