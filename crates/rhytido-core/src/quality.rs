//! The 8-state floating-point evaluation quality certificate.
//!
//! Every faithfully-evaluated numeric result carries a certificate describing
//! the strength of the rounding guarantee and what is known about exactness.
//! This is a core semantic contract (AGENTS.md §3): it must never be optimized
//! away. The eight states and their numeric codes follow the public Sollya
//! behavior; this is an independent clean-room design from that understood
//! semantics, not a copy of any reference expression.
//!
//! Two orthogonal dimensions are encoded:
//! - Rounding strength: not-faithful < faithful < correctly-rounded < exact.
//! - Exactness knowledge: unknown / known-exact / known-inexact.
//!
//! Two special states sit outside that ladder: `BelowCutoff` (the proof
//! interval was driven under a cutoff threshold) and `NonReal` (the value
//! evaluated to NaN or infinity).

/// The quality certificate of a floating-point evaluation result.
///
/// The discriminant values are the stable numeric codes (0–7) used for
/// compatibility and serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Quality {
    /// No faithful rounding was achieved, but the value is a bounded real (the
    /// proof interval was a closed subset of the reals, or an exact infinity).
    NotFaithful = 0,
    /// A faithful rounding was obtained; it is unknown whether it is exact.
    Faithful = 1,
    /// The proof interval was driven below the cutoff threshold.
    BelowCutoff = 2,
    /// The evaluation yielded NaN or infinity, so no real conclusion is possible.
    NonReal = 3,
    /// A faithfully / correctly rounded result that is known to be exact.
    Exact = 4,
    /// A faithful rounding that is known to be inexact.
    FaithfulInexact = 5,
    /// A correctly rounded result; it is unknown whether it is inexact.
    CorrectlyRounded = 6,
    /// A correctly rounded result that is known to be inexact.
    CorrectlyRoundedInexact = 7,
}

impl Quality {
    /// Returns the stable numeric code (0–7).
    pub fn code(self) -> u8 {
        self as u8
    }

    /// Reconstructs a quality from its numeric code, or `None` if out of range.
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::NotFaithful),
            1 => Some(Self::Faithful),
            2 => Some(Self::BelowCutoff),
            3 => Some(Self::NonReal),
            4 => Some(Self::Exact),
            5 => Some(Self::FaithfulInexact),
            6 => Some(Self::CorrectlyRounded),
            7 => Some(Self::CorrectlyRoundedInexact),
            _ => None,
        }
    }

    /// Returns `true` if the result is at least faithfully rounded. Correctly
    /// rounded and exact results are also faithful.
    pub fn is_faithful(self) -> bool {
        matches!(
            self,
            Self::Faithful
                | Self::Exact
                | Self::FaithfulInexact
                | Self::CorrectlyRounded
                | Self::CorrectlyRoundedInexact
        )
    }

    /// Returns `true` if the result is correctly rounded (or exact).
    pub fn is_correctly_rounded(self) -> bool {
        matches!(
            self,
            Self::Exact | Self::CorrectlyRounded | Self::CorrectlyRoundedInexact
        )
    }

    /// Returns `true` if the result is known to be exact.
    pub fn is_exact(self) -> bool {
        matches!(self, Self::Exact)
    }

    /// Returns `true` if the result is known to be inexact.
    pub fn is_known_inexact(self) -> bool {
        matches!(self, Self::FaithfulInexact | Self::CorrectlyRoundedInexact)
    }

    /// Returns `true` if the value evaluated to NaN or infinity.
    pub fn is_non_real(self) -> bool {
        matches!(self, Self::NonReal)
    }

    /// Returns `true` if the proof interval fell below the cutoff.
    pub fn is_below_cutoff(self) -> bool {
        matches!(self, Self::BelowCutoff)
    }
}
