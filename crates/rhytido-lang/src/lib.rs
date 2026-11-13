// SPDX-License-Identifier: Apache-2.0
//! Rhytido language layer (L4): composite values and session state.
//!
//! This crate is the home, decided in ADR-9, for everything the language needs
//! above the core value model but below the parser and evaluator:
//!
//! - [`value::Value`] — the composite value type. An atom of the core layer is
//!   *wrapped*, never re-implemented (`Value::Atom(rhytido_core::Atom)`), so
//!   numeric, string, range and error semantics keep a single definition.
//!   Lists are reference-counted and immutable; modification rebuilds a value
//!   instead of mutating one in place (AGENTS.md §3, "原地列表修改改为重建").
//! - [`symbol::SymbolTable`] — per-session identifier interning. Identifiers
//!   become [`symbol::SymbolId`] values so frame lookups compare integers;
//!   the table belongs to a [`context::Session`], never to a global (ADR-11).
//! - [`context::Session`] — the one carrier of mutable language state:
//!   settings, the message-suppression catalog, the symbol table, and the
//!   frame stack that gives the language its dynamic scope. All mutation goes
//!   through `&mut Session`, so any number of sessions coexist independently.
//!
//! ## What is deliberately *not* here yet
//!
//! ADR-9 reserves two further `Value` variants — `Proc` (anonymous, first-class
//! procedure) and `Function` (a mathematical body plus its ranges) — and both
//! are deferred. Neither can be represented without a syntax tree: a procedure
//! value *is* a captured body, and a body is only meaningful as parsed
//! structure. That structure is the product of M3-B1 (the AST), and the rules
//! for *running* it belong to M3-D (the behavioural interpreter). Adding
//! placeholder variants now would invent an AST shape for those tasks to
//! retrofit, which is the kind of premature commitment the ADRs avoid. So this
//! milestone carries the two variants the spec fixes — atom and list — and the
//! session machinery they will be threaded through.
//!
//! Printing is likewise absent: byte-level rendering of a `Value` is a
//! presentation concern of L6 (AGENTS.md §3, 语义/表现分离; ADR-9 explicitly
//! closes user-facing `Display` out of A1/A2). Derived [`Debug`] output is a
//! diagnostic aid and carries no contract.
//!
//! # Examples
//!
//! ```
//! use rhytido_core::Atom;
//! use rhytido_lang::{Session, Value};
//!
//! let mut session = Session::new();
//! // Dynamic scope: the inner frame shadows, then vanishes.
//! let name = session.intern("x");
//! session.assign(name, Value::atom(Atom::boolean(true)));
//! session.push_frame();
//! let inner = session.intern("y");
//! session.assign(inner, Value::list_empty());
//! assert!(session.lookup(inner).is_some());
//! session.pop_frame();
//! assert!(session.lookup(inner).is_none());
//! // Interning is per session and stable for identical text.
//! assert_eq!(name, session.intern("x"));
//! assert_eq!(session.text(name), Some("x"));
//! ```

pub mod context;
pub mod parse;
pub mod symbol;
pub mod value;

pub use context::{DisplayMode, Frame, Session, Settings};
pub use parse::{ParseError, parse_expr};
pub use symbol::{SymbolId, SymbolTable};
pub use value::Value;
