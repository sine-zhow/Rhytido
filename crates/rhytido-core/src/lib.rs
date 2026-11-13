//! Rhytido core: interned expression AST, structural hashing, byte-exact
//! printing, and the message catalog.
//!
//! This layer builds on the numerical backend facade (`rhytido-backend`) and is
//! the foundation for the evaluation engine and language. See
//! `docs/plan/02-architecture.md` (L1) and ADR-1 (interning).

pub mod context;
pub mod diff;
pub mod eval;
pub mod expr;
pub mod faithful;
pub mod fpresult;
pub mod interval_eval;
pub mod message;
pub mod print;
pub mod quality;
pub mod taylor;
pub mod value;

pub use context::{Context, MIN_PRECISION};
pub use diff::differentiate;
pub use eval::eval_point;
pub use expr::{BaseFn, Expr, Interner, NodeId};
pub use faithful::{EngineStats, evaluate_faithful};
pub use fpresult::FpResult;
pub use interval_eval::{IntervalFnHook, eval_interval};
pub use message::{Message, MessageCatalog};
pub use print::print_expr;
pub use quality::Quality;
pub use taylor::eval_taylor;
pub use value::{Atom, Bound, LangError, Range};
