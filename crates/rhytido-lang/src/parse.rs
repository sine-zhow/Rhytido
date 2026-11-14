// SPDX-License-Identifier: Apache-2.0
//! Parsing: concrete syntax to the interned core AST and to the language AST.
//!
//! [`parse_expr`] turns a mathematical-expression source string into a
//! [`NodeId`] in the caller's [`Interner`], so the result composes directly
//! with the evaluator and the canonical printer. [`parse_script`] turns a
//! *statement* sequence into a [`Script`]: assignments, blocks, procedures,
//! calls and the literals the mathematics has no node for, with the
//! mathematical leaves interned into the same interner.
//!
//! The grammar lives in `src/grammar.lalrpop` and is compiled at build time
//! (see `build.rs`). ADR-9 places the *mathematical* AST in `rhytido-core` and
//! the command AST here; `src/ast.rs` documents where each construct landed and
//! why. Control flow and pattern matching (`if`/`for`/`while`/`match`) are
//! M3-B3 and are rejected by this grammar.

use rhytido_core::Interner;
use rhytido_core::NodeId;
use thiserror::Error;

use crate::ast::Script;
use crate::symbol::SymbolTable;

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
// stateless with respect to the interner and the symbol table (both are passed
// per call), so sharing one per thread is safe.
thread_local! {
    static PARSER: grammar::ExprParser = grammar::ExprParser::new();
    static SCRIPT_PARSER: grammar::StatementsParser = grammar::StatementsParser::new();
}

/// A parse failure, carrying the byte span of the offending token when the
/// failure is tied to one.
#[derive(Debug, Error)]
pub enum ParseError {
    /// A character sequence that the lexer could not turn into any token — a
    /// stray `@`, `$` or `?`, for example. Unknown identifiers are *not* in this
    /// class: since M3-B2 they lex as names, so the grammar rejects them where
    /// it cannot use them (see [`ParseError::UnexpectedToken`]).
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

/// Decodes the escape sequences of a string literal lexeme.
///
/// `text` is the raw token, surrounding double quotes included. The recognised
/// escapes are the set the public documentation lists — the C99 single-character
/// escapes (`\\ \" \? \' \a \b \f \n \r \t \v`), `\x` followed by hexadecimal
/// digits, and `\` followed by octal digits
/// (`docs/research/sollya-language-b2.md` §6.2). Numeric escapes are reduced to
/// one byte by keeping its low 8 bits, which is what a C reader of the same
/// source would get; the documentation does not pin the behaviour of an
/// out-of-range escape, so that choice is recorded as an open item rather than
/// presented as fact.
///
/// An escape outside that set is copied through verbatim, backslash included.
/// That keeps the function total — which is what lets the grammar stay free of
/// fallible actions, and `ParseError::Internal` unreachable — and, more to the
/// point, lossless: bytes the parser is not sure about are handed to the
/// evaluator unchanged instead of being silently dropped or replaced.
///
/// # Panics
///
/// Never: every input, including a lone `"` or an unterminated escape, yields a
/// string.
pub(crate) fn unescape(text: &str) -> String {
    let inner = text
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(text);
    let chars: Vec<char> = inner.chars().collect();
    let mut out = String::with_capacity(inner.len());
    let mut index = 0;
    while index < chars.len() {
        if chars[index] != '\\' {
            out.push(chars[index]);
            index += 1;
            continue;
        }
        // At a backslash: dispatch on the character that follows it. With no
        // character at all, the backslash is a trailing byte of the input and is
        // kept.
        let Some(&mark) = chars.get(index + 1) else {
            out.push('\\');
            index += 1;
            continue;
        };
        match mark {
            '\\' | '"' | '?' | '\'' => {
                out.push(mark);
                index += 2;
            }
            'a' => {
                out.push('\u{7}');
                index += 2;
            }
            'b' => {
                out.push('\u{8}');
                index += 2;
            }
            'f' => {
                out.push('\u{c}');
                index += 2;
            }
            'n' => {
                out.push('\n');
                index += 2;
            }
            'r' => {
                out.push('\r');
                index += 2;
            }
            't' => {
                out.push('\t');
                index += 2;
            }
            'v' => {
                out.push('\u{b}');
                index += 2;
            }
            'x' => match take_radix(&chars, index + 2, 16, |digit| digit.is_ascii_hexdigit()) {
                Some((byte, next)) => {
                    out.push(char::from(byte));
                    index = next;
                }
                // `\x` with no digits is not an escape; keep the backslash and
                // let the `x` be copied by the ordinary path.
                None => {
                    out.push('\\');
                    index += 1;
                }
            },
            digit if digit.is_digit(8) => {
                match take_radix(&chars, index + 1, 8, |digit| digit.is_digit(8)) {
                    Some((byte, next)) => {
                        out.push(char::from(byte));
                        index = next;
                    }
                    None => {
                        out.push('\\');
                        index += 1;
                    }
                }
            }
            // Unrecognised escape: keep the backslash, then copy the following
            // character normally.
            _ => {
                out.push('\\');
                index += 1;
            }
        }
    }
    out
}

/// Reads the run of `radix` digits starting at `from` and folds it into one
/// byte, wrapping past 8 bits (the documented behaviour of an out-of-range C
/// numeric escape). Returns `None` when the run is empty.
fn take_radix(
    chars: &[char],
    from: usize,
    radix: u32,
    is_digit: impl Fn(char) -> bool,
) -> Option<(u8, usize)> {
    let mut value: u32 = 0;
    let mut next = from;
    while let Some(&digit) = chars.get(next) {
        if !is_digit(digit) {
            break;
        }
        value = value
            .wrapping_mul(radix)
            .wrapping_add(digit.to_digit(radix)?);
        next += 1;
    }
    if next == from {
        return None;
    }
    Some((value as u8, next))
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
/// sequence that is not a valid token at all (a stray `@` or `$`). Returns
/// [`ParseError::UnexpectedToken`] for a token the grammar rejects at a position
/// which includes a name, because this entry point parses mathematics only and
/// an identifier has no mathematical meaning until the evaluator resolves it
/// (symbol fallback is M3-C; use [`parse_script`] for the language layer).
/// Returns [`ParseError::ExtraToken`] when a complete expression is followed by
/// extra tokens. Returns [`ParseError::UnexpectedEof`] when the source ends
/// where more input was required (an empty string, a trailing binary operator,
/// an unclosed parenthesis). [`ParseError::Internal`] is reserved and currently
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
/// // Names are not mathematics: rejected here, parsed by `parse_script`.
/// assert!(parse_expr("foo", &mut interner).is_err());
/// # Ok(())
/// # }
/// ```
pub fn parse_expr(source: &str, interner: &mut Interner) -> Result<NodeId, ParseError> {
    // The mathematics never mentions a name, so the symbol table this entry
    // point has to hand the shared lexer is only a placeholder: nothing is ever
    // interned into it, and an empty `SymbolTable` allocates nothing.
    let mut unused = SymbolTable::new();
    PARSER
        .with(|parser| parser.parse(interner, &mut unused, source))
        .map_err(|err| ParseError::from_generated(source, err))
}

/// Parses a statement sequence into a [`Script`].
///
/// The script is the language layer: assignments (`=` and `:=`, optionally
/// silenced with a trailing `!`), local `var` declarations, `print` statements
/// with optional file redirection, begin-end blocks, procedure literals (`proc`)
/// and named definitions (`procedure`), calls, string and list literals, and
/// bare expressions as statements. Mathematical subtrees are interned into
/// `interner` exactly as [`parse_expr`] would intern them, so an
/// identifier-free expression gets the identical [`NodeId`] from either entry
/// point; an expression that mentions a name stays in the language AST
/// ([`crate::ast::ProgramExpr`]) with the name kept as a [`crate::SymbolId`] in
/// the table the returned script carries.
///
/// Parsing is all this does. Nothing is evaluated, no scope is entered, no type
/// or arity check happens — a procedure body is stored as parsed and stays
/// inert until the evaluator runs it (M3-C/M3-D), which is the documented
/// behaviour of definition itself.
///
/// # Errors
///
/// The same failure classes as [`parse_expr`], for the statement grammar:
/// [`ParseError::InvalidToken`] for a character sequence that is no token (an
/// unterminated string, a stray `@`), [`ParseError::UnexpectedToken`] for a
/// token that cannot appear here (a missing `;` between statements, a `}` where
/// a statement belongs, a keyword used as a name), [`ParseError::ExtraToken`]
/// for input left over after a complete script, and
/// [`ParseError::UnexpectedEof`] for input that ended too early (an unclosed
/// block, a `return` without its expression, a truncated assignment).
/// [`ParseError::Internal`] is reserved and currently unreachable.
///
/// # Panics
///
/// Never: adversarial input yields an error, not a panic.
///
/// # Examples
///
/// ```
/// use rhytido_core::Interner;
/// use rhytido_lang::ast::{ProgramExpr, Stmt};
/// use rhytido_lang::{ParseError, parse_script};
///
/// # fn main() -> Result<(), ParseError> {
/// let mut interner = Interner::new();
/// let script = parse_script("succ = proc(n) { return n + 1; };", &mut interner)?;
///
/// assert_eq!(script.len(), 1);
/// let Stmt::Assign { target, value, .. } = &script.statements()[0] else {
///     panic!("the statement is an assignment");
/// };
/// assert_eq!(script.name_of(*target), Some("succ"));
/// let body = value.as_proc().expect("a procedure literal").body();
/// assert!(matches!(
///     &body[0],
///     Stmt::Return(Some(ProgramExpr::Binary(..)))
/// ));
/// # Ok(())
/// # }
/// ```
pub fn parse_script(source: &str, interner: &mut Interner) -> Result<Script, ParseError> {
    let mut symbols = SymbolTable::new();
    let statements = SCRIPT_PARSER
        .with(|parser| parser.parse(interner, &mut symbols, source))
        .map_err(|err| ParseError::from_generated(source, err))?;
    Ok(Script::new(statements, symbols))
}

#[cfg(test)]
mod tests {
    use super::*;

    // These unit tests cover the private helpers only: the byte-span slicer and
    // the string-literal decoder. Every observable behaviour of this module is
    // tested from the outside in `crates/rhytido-lang/tests/parse.rs`
    // (AGENTS.md §4).

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

    #[test]
    fn unit_unescape_drops_the_surrounding_quotes() {
        assert_eq!(unescape(r#""abc""#), "abc");
        assert_eq!(unescape(r#""""#), "");
        // With no quotes to drop, the input is returned as decoded text.
        assert_eq!(unescape("abc"), "abc");
    }

    #[test]
    fn unit_unescape_decodes_the_documented_single_character_escapes() {
        assert_eq!(unescape(r#""\\\"\' \?""#), "\\\"' ?");
        assert_eq!(unescape(r#""\n\t\r""#), "\n\t\r");
        assert_eq!(unescape(r#""\a\b\f\v""#), "\u{7}\u{8}\u{c}\u{b}");
    }

    #[test]
    fn unit_unescape_decodes_hexadecimal_and_octal_runs() {
        // `\x41` and `\101` are both the character A.
        assert_eq!(unescape(r#""\x41""#), "A");
        assert_eq!(unescape(r#""\101""#), "A");
        assert_eq!(unescape(r#""\0""#), "\0");
        // A run ends at the first non-digit of its radix.
        assert_eq!(unescape(r#""\x41z""#), "Az");
        assert_eq!(unescape(r#""\1018""#), "A8");
    }

    #[test]
    fn unit_unescape_wraps_numeric_escapes_past_one_byte() {
        // The documented behaviour of an out-of-range C escape is
        // implementation defined; this decoder keeps the low 8 bits and the
        // choice is recorded as an open distillation item.
        assert_eq!(unescape(r#""\x1FF""#), "\u{ff}");
        assert_eq!(unescape(r#""\777""#), "\u{ff}");
        // A run long enough to overflow the accumulator must not panic either:
        // the fold is congruent mod 256 at every step, so the low byte survives
        // however long the digit run is. One character out — the UTF-8 encoding
        // of `ÿ` spans two *bytes*, which is an encoding fact, not a second
        // character, hence `chars().count()`.
        let wide = unescape(r#""\xFFFFFFFFFFFF""#);
        assert_eq!(wide.chars().count(), 1);
        assert_eq!(wide, "\u{ff}");
    }

    #[test]
    fn unit_unescape_keeps_unrecognised_escapes_verbatim() {
        // Unknown escapes are not dropped: the parser is not the place to
        // decide what `\q` means, and dropping the backslash would lose
        // information the evaluator may still need.
        assert_eq!(unescape(r#""\q""#), r"\q");
        assert_eq!(unescape(r#""\x""#), r"\x", "no digits means no escape");
        assert_eq!(unescape(r#""a\\b""#), r"a\b");
        // A trailing backslash with nothing after it is kept, not dropped: the
        // lexer never produces one, so this is the defensive branch.
        assert_eq!(unescape("a\\"), "a\\");
    }

    #[test]
    fn unit_unescape_preserves_multibyte_characters() {
        // Escapes are decoded per character, so a non-ASCII payload survives
        // byte for byte and a multibyte character after a backslash is not
        // mangled by the byte-oriented decoding.
        assert_eq!(unescape(r#""héllo — ✓""#), "héllo — ✓");
        assert_eq!(unescape(r#""\✓""#), r"\✓");
        assert_eq!(unescape(r#""\né""#), "\né");
    }
}
