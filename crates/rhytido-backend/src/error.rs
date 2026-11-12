//! Error types for the numerical backend facade.

use thiserror::Error;

/// Errors produced by the numerical backend facade.
///
/// Each variant is meaningful and user-facing (AGENTS.md C-GOOD-ERR); the
/// presentation layer maps these to Sollya-compatible diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Error {
    /// A numeric literal could not be parsed.
    #[error("invalid numeric literal: {0}")]
    Parse(String),
    /// A division by zero was attempted.
    #[error("division by zero")]
    DivisionByZero,
}
