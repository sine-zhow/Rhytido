// SPDX-License-Identifier: Apache-2.0
//! Expression parsing: concrete syntax to the interned core AST.
//!
//! [`parse_expr`] turns a mathematical-expression source string into a
//! [`NodeId`] in the caller's [`Interner`], so the result composes directly
//! with the evaluator and the canonical printer. The grammar lives in
//! `src/grammar.lalrpop` and is compiled at build time (see `build.rs`).
//!
//! ADR-9 places the *mathematical* AST in `rhytido-core`; this module is the
//! language-layer entry into it. Command / procedure syntax is a later task
//! (M3-B2/B3) and is intentionally absent here.

use rhytido_core::Interner;
use rhytido_core::NodeId;
use thiserror::Error;

// Pull in the parser generated from `grammar.lalrpop` into a private module.
// The generated code is machine-emitted, not authored, so it is exempted from
// the workspace doc and lint requirements that only apply to hand-written
// sources (the module is private, so none of its items are publicly reachable).
lalrpop_util::lalrpop_mod!(
    #[allow(missing_docs, clippy::all, unused_imports, dead_code)]
    grammar
);

// The generated parser compiles its lexer regexes once in `new()`, so a
// thread-local instance avoids rebuilding them on every call. The parser is
// stateless with respect to the interner (which is passed per call), so
// sharing one per thread is safe.
thread_local! {
    static PARSER: grammar::ExprParser = grammar::ExprParser::new();
}

/// A parse failure, carrying the byte span of the offending token when the
/// failure is tied to one.
#[derive(Debug, Error)]
pub enum ParseError {
    /// A character sequence that the lexer could not turn into any token —
    /// for example an unknown identifier such as `foo` (symbol fallback is a
    /// command-layer feature for B2+, so it is a syntax error here).
    #[error("unrecognized token at byte {location}")]
    InvalidToken {
        /// Byte offset of the first offending character.
        location: usize,
    },
    /// A token the grammar does not accept at this position (a dangling
    /// operator, a stray `)`, a misplaced literal).
    #[error("unexpected token {token:?} at bytes {start}..{end}; expected one of {expected:?}")]
    UnexpectedToken {
        /// Byte offset of the start of the offending token.
        start: usize,
        /// Byte offset just past the end of the offending token.
        end: usize,
        /// The lexeme that could not be used here (sliced from the source).
        token: String,
        /// Token names the grammar expected instead.
        expected: Vec<String>,
    },
    /// The grammar finished a complete expression but tokens remained (a value
    /// that started without an operator, a trailing `)`, …).
    #[error("unexpected trailing token {token:?} at bytes {start}..{end}")]
    ExtraToken {
        /// Byte offset of the start of the trailing token.
        start: usize,
        /// Byte offset just past the end of the trailing token.
        end: usize,
        /// The lexeme left over after a complete expression.
        token: String,
    },
    /// The source ended where the grammar still expected more input (an empty
    /// string, a trailing binary operator, an unclosed parenthesis).
    #[error("unexpected end of input at byte {location}; expected one of {expected:?}")]
    UnexpectedEof {
        /// Byte offset of the end of input.
        location: usize,
        /// Token names the grammar expected at the end of input.
        expected: Vec<String>,
    },
    /// A user-reported error from the grammar. Currently unreachable: the
    /// grammar emits no fallible (`=>?`) actions, so the built-in lexer's only
    /// failures surface as the variants above.
    #[error("internal parse error: {message}")]
    Internal {
        /// The message carried by the generated parser.
        message: String,
    },
}

impl ParseError {
    /// Converts a generated-parser error into a [`ParseError`], slicing the
    /// offending lexeme out of `source` (the generated error carries only byte
    /// spans, not text).
    fn from_generated<'input>(
        source: &str,
        err: lalrpop_util::ParseError<usize, lalrpop_util::lexer::Token<'input>, &'static str>,
    ) -> ParseError {
        use lalrpop_util::ParseError as G;
        match err {
            G::InvalidToken { location } => ParseError::InvalidToken { location },
            G::UnrecognizedEof { location, expected } => {
                ParseError::UnexpectedEof { location, expected }
            }
            G::UnrecognizedToken {
                token: (start, _, end),
                expected,
            } => ParseError::UnexpectedToken {
                start,
                end,
                token: lexeme(source, start, end),
                expected,
            },
            G::ExtraToken {
                token: (start, _, end),
            } => ParseError::ExtraToken {
                start,
                end,
                token: lexeme(source, start, end),
            },
            G::User { error } => ParseError::Internal {
                message: error.to_string(),
            },
        }
    }
}

/// Slices `source[start..end]` defensively: lalrpop offsets are byte indices
/// into `source`, but a malformed span would otherwise panic, and parsing must
/// never panic on adversarial input.
fn lexeme(source: &str, start: usize, end: usize) -> String {
    source.get(start..end).unwrap_or("").to_string()
}

/// Parses a mathematical expression into an interned [`NodeId`].
///
/// Sub-expressions are interned as they are built, so structurally identical
/// operands share a node (hash-consing, ADR-1) and the returned id is stable
/// across repeated parses of the same source into the same starting interner
/// state. Precedence and associativity match the canonical printer in
/// `rhytido_core::print` exactly, so `parse ∘ print` is a fixpoint on
/// printer-canonical strings (see `tests/integration_parse_print.rs`).
///
/// # Errors
///
/// Returns [`ParseError::InvalidToken`] when the source contains a character
/// sequence that is not a valid token (an unknown identifier, for example —
/// symbol fallback is a later command-layer feature and is a syntax error
/// here). Returns [`ParseError::UnexpectedToken`] for a token the grammar
/// rejects at a position (a dangling operator, a misplaced `)`). Returns
/// [`ParseError::ExtraToken`] when a complete expression is followed by extra
/// tokens. Returns [`ParseError::UnexpectedEof`] when the source ends where
/// more input was required (an empty string, a trailing binary operator, an
/// unclosed parenthesis). [`ParseError::Internal`] is reserved and currently
/// unreachable.
///
/// # Examples
///
/// ```
/// use rhytido_core::{Expr, Interner};
/// use rhytido_lang::{ParseError, parse_expr};
///
/// # fn main() -> Result<(), ParseError> {
/// let mut interner = Interner::new();
///
/// // "2 + 3" interns as an Add node.
/// let id = parse_expr("2 + 3", &mut interner)?;
/// assert!(matches!(interner.get(id), Expr::Add(_, _)));
///
/// // Precedence: multiplication binds tighter than addition.
/// let id = parse_expr("1 + 2 * 3", &mut interner)?;
/// assert!(matches!(interner.get(id), Expr::Add(_, _)));
///
/// // Unknown identifiers are syntax errors here (symbol fallback is B2+).
/// assert!(parse_expr("foo", &mut interner).is_err());
/// # Ok(())
/// # }
/// ```
pub fn parse_expr(source: &str, interner: &mut Interner) -> Result<NodeId, ParseError> {
    PARSER
        .with(|parser| parser.parse(interner, source))
        .map_err(|err| ParseError::from_generated(source, err))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The byte-span slicer must never panic and must degrade to "" on a span
    /// that does not line up with `source` (an internal invariant of the
    /// generated parser, but defended against adversarial offsets).
    #[test]
    fn unit_lexeme_slices_and_degrades() {
        assert_eq!(lexeme("abc", 0, 2), "ab");
        assert_eq!(lexeme("abc", 1, 3), "bc");
        // Out-of-range spans fall back to an empty string rather than panic.
        assert_eq!(lexeme("abc", 5, 9), "");
        assert_eq!(lexeme("abc", 1, 99), "");
    }
}
