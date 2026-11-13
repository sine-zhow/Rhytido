// SPDX-License-Identifier: Apache-2.0
//! Atomic value representation for the Rhytido language (ADR-10).
//!
//! [`Atom`] is the immutable, self-contained half of the language value model:
//! numeric scalars, booleans, strings, ranges, the first-class `error` value,
//! and `void`. It references no syntax tree, which is exactly the cut that
//! keeps these values in this layer while composite values (lists, procedures,
//! functions) live above it (ADR-9).
//!
//! Design contract, restated from ADR-10:
//!
//! - **Construction is total.** Every constructor and every variant accepts
//!   every input of its declared type and stores it verbatim: no validation, no
//!   normalization, no failure, no panic. A degenerate range such as
//!   `[3, 1]`, a `NaN` endpoint, or an empty error message are all preserved
//!   values, not errors. Deciding what is *legal* is a question about the
//!   reference behaviour of the evaluator and therefore belongs to the
//!   evaluation layer (M3-C/D), where it can be pinned by behavioural fixtures.
//! - **Equality follows the interning precedent.** Numbers compare by
//!   [`BigFloat::canonical_parts`], so NaN is structurally equal to NaN,
//!   signed zeros stay distinguished, and the precision label is irrelevant —
//!   the same equivalence classes `crate::expr` builds for constants. One
//!   mathematical value is one equivalence class in both the expression world
//!   and the value world.
//! - **Semantics, not presentation.** This layer defines no byte-level
//!   rendering; user-facing printing is a presentation concern (L6), and the
//!   deliberately non-contractual [`Debug`] output must not be mistaken for it.
//!
//! Values are cheap to share: strings are reference-counted [`Rc<str>`] so
//! copies alias instead of reallocating (there is no global intern table —
//! AGENTS.md §3, zero global state), and accessors hand out borrows so callers
//! never clone an arbitrary-precision number they only want to read.
//!
//! # Examples
//!
//! ```
//! use rhytido_backend::BigFloat;
//! use rhytido_core::{Atom, Bound, Range};
//!
//! let a = Atom::number(BigFloat::one());
//! // The precision label is not part of a number's identity.
//! let b = Atom::number(BigFloat::one().with_precision(300));
//! assert_eq!(a, b);
//!
//! // Construction is total: a reversed range is stored, not rejected.
//! let reversed = Range::new(
//!     Bound::closed(BigFloat::from_i64(3)),
//!     Bound::closed(BigFloat::from_i64(1)),
//! );
//! assert_eq!(reversed.lower(), &Bound::closed(BigFloat::from_i64(3)));
//! assert_eq!(Atom::range(reversed.clone()), Atom::range(reversed));
//! ```

use std::hash::{Hash, Hasher};
use std::rc::Rc;

use rhytido_backend::{BigFloat, Integer, Rational};

/// Compares two floats by canonical value, the interning convention of
/// [`crate::expr`]: `NaN` equals `NaN`, signed zeros differ, and precision is
/// irrelevant.
fn float_eq(a: &BigFloat, b: &BigFloat) -> bool {
    match (a.canonical_parts(), b.canonical_parts()) {
        (None, None) => true,
        (Some(p), Some(q)) => p == q,
        _ => false,
    }
}

/// Hashes a float consistently with [`float_eq`]: one bucket for `NaN`, and
/// otherwise the canonical `(significand, exponent)` pair.
fn float_hash<H: Hasher>(value: &BigFloat, state: &mut H) {
    match value.canonical_parts() {
        None => 0u8.hash(state),
        Some((sig, exp)) => {
            1u8.hash(state);
            sig.hash(state);
            exp.hash(state);
        }
    }
}

/// Hashes a rational by its canonical numerator and denominator, matching the
/// structural equality `Rational` derives over its reduced form.
fn rational_hash<H: Hasher>(value: &Rational, state: &mut H) {
    value.numerator().hash(state);
    value.denominator().hash(state);
}

/// One endpoint of a [`Range`].
///
/// Open/closed-ness and infinity are encoded in the variant rather than in a
/// flag, so no combination of inputs can produce a state the type cannot
/// represent (AGENTS.md §2.4 C-CUSTOM-TYPE). Like every other item here, the
/// variant set is a *representation*: whether an endpoint makes sense for a
/// given operation is decided by the evaluation layer.
///
/// # Examples
///
/// ```
/// use rhytido_backend::BigFloat;
/// use rhytido_core::Bound;
///
/// let closed = Bound::closed(BigFloat::one());
/// let open = Bound::open(BigFloat::one());
/// assert_ne!(closed, open, "openness is part of the value");
/// assert_eq!(closed.value(), Some(&BigFloat::one()));
/// assert_eq!(Bound::neg_infinity().value(), None);
/// ```
#[derive(Clone, Debug)]
pub enum Bound {
    /// A finite endpoint included in the range.
    Closed(BigFloat),
    /// A finite endpoint excluded from the range.
    Open(BigFloat),
    /// Negative infinity, always an open endpoint in effect.
    NegInfinity,
    /// Positive infinity, always an open endpoint in effect.
    PosInfinity,
}

impl Bound {
    /// Creates the closed finite endpoint `value`.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    pub fn closed(value: BigFloat) -> Self {
        Self::Closed(value)
    }

    /// Creates the open finite endpoint `value`.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    pub fn open(value: BigFloat) -> Self {
        Self::Open(value)
    }

    /// Creates the negative-infinity endpoint.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    pub fn neg_infinity() -> Self {
        Self::NegInfinity
    }

    /// Creates the positive-infinity endpoint.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    pub fn pos_infinity() -> Self {
        Self::PosInfinity
    }

    /// Borrows the finite endpoint of a [`Bound::Closed`] or [`Bound::Open`]
    /// bound, and `None` for either infinity.
    ///
    /// A cheap view over the stored value; it says nothing about whether the
    /// endpoint is usable for a particular operation.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn value(&self) -> Option<&BigFloat> {
        match self {
            Self::Closed(v) | Self::Open(v) => Some(v),
            Self::NegInfinity | Self::PosInfinity => None,
        }
    }

    /// Returns `true` if the endpoint includes its value ([`Bound::Closed`]).
    ///
    /// # Panics
    ///
    /// Never.
    pub fn is_closed(&self) -> bool {
        matches!(self, Self::Closed(_))
    }

    /// Returns `true` if the endpoint excludes its value ([`Bound::Open`]).
    ///
    /// # Panics
    ///
    /// Never.
    pub fn is_open(&self) -> bool {
        matches!(self, Self::Open(_))
    }

    /// Returns `true` for either infinite endpoint.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn is_infinite(&self) -> bool {
        matches!(self, Self::NegInfinity | Self::PosInfinity)
    }
}

impl PartialEq for Bound {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Closed(a), Self::Closed(b)) | (Self::Open(a), Self::Open(b)) => float_eq(a, b),
            (Self::NegInfinity, Self::NegInfinity) | (Self::PosInfinity, Self::PosInfinity) => true,
            _ => false,
        }
    }
}

impl Eq for Bound {}

impl Hash for Bound {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Closed(v) => {
                0u8.hash(state);
                float_hash(v, state);
            }
            Self::Open(v) => {
                1u8.hash(state);
                float_hash(v, state);
            }
            Self::NegInfinity => 2u8.hash(state),
            Self::PosInfinity => 3u8.hash(state),
        }
    }
}

/// A numeric range: a lower and an upper [`Bound`], each independently open,
/// closed or infinite.
///
/// The pair is stored **exactly as given**. Nothing here checks that
/// `lower <= upper`, that a closed lower bound is not `+infinity`, or that the
/// endpoints are not `NaN`: those are well-formedness questions whose answers
/// are part of the evaluator's observable behaviour, which ADR-10 defers to the
/// evaluation layer (M3-C/D). Preserving a degenerate range verbatim is what
/// lets that layer decide, and report, as the reference does.
///
/// # Examples
///
/// ```
/// use rhytido_backend::BigFloat;
/// use rhytido_core::Range;
/// use rhytido_core::Bound;
///
/// let whole_line = Range::new(Bound::neg_infinity(), Bound::pos_infinity());
/// assert!(whole_line.lower().is_infinite());
/// assert!(whole_line.upper().is_infinite());
///
/// // A singleton and a degenerate interval are different, both representable.
/// let point = Range::new(
///     Bound::closed(BigFloat::zero()),
///     Bound::closed(BigFloat::zero()),
/// );
/// let empty = Range::new(
///     Bound::open(BigFloat::zero()),
///     Bound::open(BigFloat::zero()),
/// );
/// assert_ne!(point, empty);
/// ```
#[derive(Clone, Debug)]
pub struct Range {
    lower: Bound,
    upper: Bound,
}

impl Range {
    /// Creates the range from a `lower` and an `upper` bound.
    ///
    /// Total: any combination is accepted and kept unchanged, including
    /// `lower > upper`, infinite bounds in either position, and `NaN`
    /// endpoints.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    pub fn new(lower: Bound, upper: Bound) -> Self {
        Self { lower, upper }
    }

    /// Borrows the lower endpoint bound.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn lower(&self) -> &Bound {
        &self.lower
    }

    /// Borrows the upper endpoint bound.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn upper(&self) -> &Bound {
        &self.upper
    }
}

impl PartialEq for Range {
    fn eq(&self, other: &Self) -> bool {
        self.lower == other.lower && self.upper == other.upper
    }
}

impl Eq for Range {}

impl Hash for Range {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.lower.hash(state);
        self.upper.hash(state);
    }
}

/// An error value: the payload of [`Atom::Error`].
///
/// Error is a *first-class value* in this language (AGENTS.md §3), so an error
/// is plain data that flows through variables, lists and results like any other
/// value. This type is that carrier only: it is not a Rust error enum, it is
/// never returned from a constructor, and it defines no propagation behaviour.
/// Propagation rules belong to the evaluator (M3-C); the wording of `message`
/// and the numbering of `code` (aligned with [`crate::message`]) are refined
/// when the reference behaviour is distilled in M3-B/C.
///
/// # Examples
///
/// ```
/// use rhytido_core::{Atom, LangError};
///
/// let with_code = LangError::new("cannot evaluate", Some(7));
/// assert_eq!(with_code.message(), "cannot evaluate");
/// assert_eq!(with_code.code(), Some(7));
///
/// // A missing code is distinct from code 0, and both are valid.
/// assert_ne!(with_code, LangError::new("cannot evaluate", None));
/// assert_ne!(with_code, LangError::new("cannot evaluate", Some(0)));
///
/// assert_eq!(Atom::error("cannot evaluate", Some(7)), Atom::Error(with_code));
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LangError {
    message: Rc<str>,
    code: Option<u32>,
}

impl LangError {
    /// Creates an error value from `message` and an optional catalog code.
    ///
    /// Total: an empty message and any `code` are stored verbatim.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    pub fn new(message: impl Into<Rc<str>>, code: Option<u32>) -> Self {
        Self {
            message: message.into(),
            code,
        }
    }

    /// Borrows the message text.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the message-catalog code, if the error carries one.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn code(&self) -> Option<u32> {
        self.code
    }
}

/// An immutable atomic value of the language (ADR-10).
///
/// `Atom` covers the values that carry no syntax tree; composite values are
/// layered on top of it by the language crate (ADR-9). Cloning an `Atom` is
/// shallow for strings ([`Rc<str>`] aliases) and deep for the numbers behind
/// them, so pass values by reference where possible (accessors borrow).
///
/// Equality and hashing implement the semantics described at the top of this
/// module: numbers by canonical parts, integers and rationals by their
/// canonical values, strings by content, ranges and errors structurally, `Void`
/// only equal to itself. Distinct variants are never equal even when they
/// denote the same mathematical value — `Atom::integer(0)` and
/// `Atom::number(0)` are different values, because choosing otherwise would
/// silently collapse type distinctions the evaluator still has to see.
///
/// `Ord` is deliberately **not** implemented: cross-variant ordering has no
/// defined meaning until the reference behaviour is distilled, and a partial
/// order over numbers alone would promise what the evaluator cannot keep.
///
/// # Examples
///
/// ```
/// use rhytido_backend::{BigFloat, Integer, Rational};
/// use rhytido_core::Atom;
///
/// // Numeric variants do not collapse: the variant is part of the identity.
/// assert_ne!(
///     Atom::number(BigFloat::from_i64(2)),
///     Atom::integer(Integer::from(2))
/// );
///
/// assert_eq!(Atom::string("abc"), Atom::string(String::from("abc")));
/// assert_eq!(
///     Atom::rational(Rational::from_i64(5)),
///     Atom::rational(Rational::from(5i64))
/// );
/// assert_eq!(Atom::default(), Atom::Void);
/// assert_eq!(Atom::boolean(true).as_bool(), Some(true));
/// // `BigFloat::NAN` never compares equal to itself as a float, but two NaN
/// // numbers are equal as values.
/// let nan = Atom::number(BigFloat::NAN);
/// assert!(nan.as_number().is_some_and(|v| v.is_nan()));
/// assert_eq!(nan, Atom::number(BigFloat::NAN));
/// ```
#[derive(Clone, Debug)]
pub enum Atom {
    /// An arbitrary-precision binary float.
    Number(BigFloat),
    /// An exact integer of unbounded width.
    Integer(Integer),
    /// An exact rational in canonical (reduced) form.
    Rational(Rational),
    /// A boolean.
    Bool(bool),
    /// An immutable string, shared by reference count and compared by content.
    String(Rc<str>),
    /// A numeric range with independently open, closed or infinite endpoints.
    Range(Range),
    /// A first-class error value.
    Error(LangError),
    /// The unit value: the result of a procedure with nothing to return.
    Void,
}

impl Atom {
    /// Creates a [`Atom::Number`] from `value`.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    pub fn number(value: BigFloat) -> Self {
        Self::Number(value)
    }

    /// Creates an [`Atom::Integer`] from `value`.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    pub fn integer(value: Integer) -> Self {
        Self::Integer(value)
    }

    /// Creates an [`Atom::Rational`] from `value`.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    pub fn rational(value: Rational) -> Self {
        Self::Rational(value)
    }

    /// Creates an [`Atom::Bool`] from `value`.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    pub fn boolean(value: bool) -> Self {
        Self::Bool(value)
    }

    /// Creates an [`Atom::String`] holding the contents of `value`.
    ///
    /// Accepts anything convertible into `Rc<str>` so a caller that already
    /// owns a shared string passes it by value instead of copying it again.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    pub fn string(value: impl Into<Rc<str>>) -> Self {
        Self::String(value.into())
    }

    /// Creates an [`Atom::Range`] from `value`.
    ///
    /// # Panics
    ///
    /// Never: construction is total, and the range's own constructor is total.
    pub fn range(value: Range) -> Self {
        Self::Range(value)
    }

    /// Creates an [`Atom::Error`] carrying a message and an optional
    /// message-catalog `code` (see [`LangError`]).
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    pub fn error(message: impl Into<Rc<str>>, code: Option<u32>) -> Self {
        Self::Error(LangError::new(message, code))
    }

    /// Returns the [`Atom::Void`] unit value.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    pub fn void() -> Self {
        Self::Void
    }

    /// Borrows the float if this is an [`Atom::Number`].
    ///
    /// # Panics
    ///
    /// Never.
    pub fn as_number(&self) -> Option<&BigFloat> {
        match self {
            Self::Number(v) => Some(v),
            _ => None,
        }
    }

    /// Borrows the integer if this is an [`Atom::Integer`].
    ///
    /// # Panics
    ///
    /// Never.
    pub fn as_integer(&self) -> Option<&Integer> {
        match self {
            Self::Integer(v) => Some(v),
            _ => None,
        }
    }

    /// Borrows the rational if this is an [`Atom::Rational`].
    ///
    /// # Panics
    ///
    /// Never.
    pub fn as_rational(&self) -> Option<&Rational> {
        match self {
            Self::Rational(v) => Some(v),
            _ => None,
        }
    }

    /// Returns the flag if this is an [`Atom::Bool`].
    ///
    /// # Panics
    ///
    /// Never.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// Borrows the text if this is an [`Atom::String`].
    ///
    /// # Panics
    ///
    /// Never.
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(v) => Some(v),
            _ => None,
        }
    }

    /// Borrows the range if this is an [`Atom::Range`].
    ///
    /// # Panics
    ///
    /// Never.
    pub fn as_range(&self) -> Option<&Range> {
        match self {
            Self::Range(v) => Some(v),
            _ => None,
        }
    }

    /// Borrows the payload if this is an [`Atom::Error`].
    ///
    /// # Panics
    ///
    /// Never.
    pub fn as_error(&self) -> Option<&LangError> {
        match self {
            Self::Error(v) => Some(v),
            _ => None,
        }
    }

    /// Returns `true` if this is the [`Atom::Void`] unit value.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn is_void(&self) -> bool {
        matches!(self, Self::Void)
    }

    /// Returns `true` if this is an [`Atom::Error`] value.
    ///
    /// Existential only: it never unwinds or propagates anything, because an
    /// error is a value at this layer.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

impl Default for Atom {
    /// `Void` is the default value (ADR-10): the result of computing nothing.
    fn default() -> Self {
        Self::Void
    }
}

impl PartialEq for Atom {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => float_eq(a, b),
            (Self::Integer(a), Self::Integer(b)) => a == b,
            (Self::Rational(a), Self::Rational(b)) => a == b,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::String(a), Self::String(b)) => a.as_ref() == b.as_ref(),
            (Self::Range(a), Self::Range(b)) => a == b,
            (Self::Error(a), Self::Error(b)) => a == b,
            (Self::Void, Self::Void) => true,
            _ => false,
        }
    }
}

impl Eq for Atom {}

impl Hash for Atom {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Number(v) => {
                0u8.hash(state);
                float_hash(v, state);
            }
            Self::Integer(v) => {
                1u8.hash(state);
                v.hash(state);
            }
            Self::Rational(v) => {
                2u8.hash(state);
                rational_hash(v, state);
            }
            Self::Bool(v) => {
                3u8.hash(state);
                v.hash(state);
            }
            Self::String(v) => {
                4u8.hash(state);
                v.as_ref().hash(state);
            }
            Self::Range(v) => {
                5u8.hash(state);
                v.hash(state);
            }
            Self::Error(v) => {
                6u8.hash(state);
                v.hash(state);
            }
            Self::Void => {
                7u8.hash(state);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These unit tests cover the private kernels only; every observable
    // behaviour of this module is tested from the outside in
    // `crates/rhytido-core/tests/value.rs` (AGENTS.md §4).

    // ---- float_eq / float_hash ----------------------------------------------

    #[test]
    fn normal_float_eq_ignores_precision_label() {
        let a = BigFloat::from_i64(7);
        let b = BigFloat::from_i64(7).with_precision(300);
        assert!(float_eq(&a, &b));
        assert_eq!(hash_of(&a), hash_of(&b));
        assert!(!float_eq(&a, &BigFloat::from_i64(8)));
    }

    #[test]
    fn normal_float_eq_nan_is_structurally_equal() {
        assert!(float_eq(&BigFloat::NAN, &BigFloat::NAN));
        assert!(!float_eq(&BigFloat::NAN, &BigFloat::zero()));
        assert!(!float_eq(&BigFloat::zero(), &BigFloat::NAN));
        assert_eq!(hash_of(&BigFloat::NAN), hash_of(&BigFloat::NAN));
    }

    #[test]
    fn extreme_float_hash_distinguishes_signed_zero_and_infinities() {
        let pos = BigFloat::zero();
        let neg = -BigFloat::zero();
        assert!(!float_eq(&pos, &neg));
        assert_ne!(hash_of(&pos), hash_of(&neg));
        assert!(!float_eq(&BigFloat::infinity(), &BigFloat::neg_infinity()));
        assert_ne!(
            hash_of(&BigFloat::infinity()),
            hash_of(&BigFloat::neg_infinity())
        );
        // An infinity is not NaN, and so takes the canonical-parts branch.
        assert_ne!(hash_of(&BigFloat::infinity()), hash_of(&BigFloat::NAN));
    }

    #[test]
    fn extreme_float_hash_kernel_total_on_special_values() {
        for v in [
            BigFloat::NAN,
            BigFloat::infinity(),
            BigFloat::neg_infinity(),
            -BigFloat::zero(),
            BigFloat::pi_with_precision(1000),
        ] {
            let _ = hash_of(&v);
            assert!(float_eq(&v, &v));
        }
    }

    // ---- rational_hash ------------------------------------------------------

    #[test]
    fn normal_rational_hash_follows_canonical_form() {
        let half = Rational::new(1, 2).expect("valid denominator");
        let unreduced = Rational::new(2, 4).expect("valid denominator");
        assert_eq!(half, unreduced);
        assert_eq!(rational_hash_of(&half), rational_hash_of(&unreduced));
        let neg = Rational::new(-1, 2).expect("valid denominator");
        assert_ne!(rational_hash_of(&half), rational_hash_of(&neg));
    }

    #[test]
    fn extreme_rational_hash_of_wide_fraction() {
        let mut wide = Rational::new(1, 3).expect("valid denominator");
        for _ in 0..8 {
            wide = &wide * &wide;
        }
        // Denominator 3^256: far beyond a machine word, still hashable.
        assert_eq!(rational_hash_of(&wide), rational_hash_of(&wide.clone()));
        assert!(!wide.numerator().is_zero());
    }

    #[test]
    fn robustness_rational_hash_stable_under_repetition() {
        let r = Rational::new(31, 97).expect("valid denominator");
        let first = rational_hash_of(&r);
        for _ in 0..16 {
            assert_eq!(first, rational_hash_of(&r));
        }
    }

    // ---- private helpers used by both kernels and tests ---------------------

    fn hash_of(v: &BigFloat) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        float_hash(v, &mut h);
        h.finish()
    }

    fn rational_hash_of(v: &Rational) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        rational_hash(v, &mut h);
        h.finish()
    }
}
