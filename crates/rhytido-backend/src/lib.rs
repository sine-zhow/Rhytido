//! Rhytido numerical backend facade.
//!
//! This crate is the single place that touches the underlying arbitrary-precision
//! library. The rest of Rhytido depends only on the abstract types exposed here,
//! never on the concrete backend. See `docs/plan/02-architecture.md` (facade
//! isolation) and ADR-8 (backend selection).
//!
//! M0 builds the numerical foundation incrementally; each type ships with full
//! four-category test coverage (normal / extreme / error / interruption) per
//! AGENTS.md before it is used or merged.

mod bigfloat;
mod erf;
mod error;
mod format;
mod integer;
mod interval;
mod rational;

pub use bigfloat::{BigFloat, DEFAULT_PRECISION, Rounding};
pub use error::Error;
pub use format::Format;
pub use integer::Integer;
pub use interval::Interval;
pub use rational::Rational;
