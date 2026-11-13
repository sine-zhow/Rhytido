// SPDX-License-Identifier: Apache-2.0
//! Composite values of the language (ADR-9).
//!
//! [`Value`] is the value the language manipulates. ADR-9 cuts the value model
//! in two: everything that carries no syntax tree — numbers, booleans, strings,
//! ranges, the first-class `error`, `void` — is the core layer's `Atom`, and
//! this type *wraps* it (`Value::Atom`) instead of restating it, so there is
//! exactly one definition of what it means for `2.0` to equal `2.00` at 300
//! bits, for a `NaN` to equal a `NaN`, or for signed zeros to stay apart
//! (ADR-10). The composite half added here is the list.
//!
//! Two further variants that ADR-9 plans for this enum — `Proc` and `Function`
//! — are intentionally absent: both are *captured syntax trees*, and the
//! language AST is the deliverable of M3-B1. Declaring them now would mean
//! inventing the tree shape that task is about to design, and the behaviour
//! that gives a body meaning belongs to M3-D. See the crate documentation.
//!
//! Lists are immutable and shared: elements live behind an [`Rc<[Value]>`] so a
//! clone is a reference-count bump rather than a deep copy
//! (`docs/research/rust-numerical-optimization.md`: share, do not copy). Every
//! apparent modification — [`Value::append`], [`Value::prepend`] — *rebuilds* a
//! new value and leaves the original untouched, per the architecture rule that
//! in-place list mutation becomes rebuilding (AGENTS.md §3). Sharing is visible
//! in equality but never in identity: two lists that hold equal elements are
//! equal, however they were built or however often their buffer is aliased.
//!
//! Presentation is out of scope again here: no user-facing `Display`, because
//! byte-level rendering is an L6 concern (AGENTS.md §3, 语义/表现分离; ADR-9
//! closes it out of A1/A2). The derived [`Debug`] exists for diagnostics and
//! carries no contract.
//!
//! # Examples
//!
//! ```
//! use rhytido_core::Atom;
//! use rhytido_lang::Value;
//!
//! let empty = Value::list_empty();
//! // Rebuilding, not mutating: `empty` is still the empty list.
//! let one = empty.append(Atom::boolean(true).into()).expect("list");
//! assert!(empty.as_list().is_some_and(|items| items.is_empty()));
//! assert_eq!(one.as_list().map(<[Value]>::len), Some(1));
//!
//! // Structural equality across independently built lists.
//! let same = Value::list_from_iter([Atom::boolean(true)]);
//! assert_eq!(one, same);
//! ```

use std::hash::{Hash, Hasher};
use std::rc::Rc;

use rhytido_core::Atom;

/// Tag bytes for hashing.
///
/// The constants keep a `Value` in a different bucket from an `Atom` that
/// happens to hash to the same byte sequence, which is what a discriminant
/// contributes to a hash. `0` mirrors the one-variant-wrapped case, `1` marks
/// a list; the length is folded in afterwards so that `[a, b]` and a lone
/// element whose own hash happens to repeat those bytes do not collide by
/// accident of construction.
const HASH_ATOM_TAG: u8 = 0;
const HASH_LIST_TAG: u8 = 1;

/// A value of the language: an atomic value, or a list of values (ADR-9).
///
/// Cloning is cheap — an `Atom` clone shares its string, a `List` clone bumps
/// the refcount of its element buffer — so values move into frames and out of
/// lookups by reference where possible: the accessors here borrow.
///
/// Equality is structural and hashing agrees with it: two values are equal iff
/// they hash alike. Numbers keep the semantics of the wrapped `Atom`, so the
/// precision label stays irrelevant in a list exactly as it is outside one.
///
/// # Examples
///
/// ```
/// use rhytido_core::Atom;
/// use rhytido_lang::Value;
///
/// // An atom of any kind lifts into a value.
/// assert_eq!(Value::from(Atom::void()), Value::atom(Atom::void()));
/// assert_eq!(Value::default(), Value::atom(Atom::void()));
/// ```
#[derive(Clone, Debug)]
pub enum Value {
    /// An atomic value owned by the core layer (ADR-10).
    Atom(Atom),
    /// An immutable, reference-counted list of values.
    List(Rc<[Value]>),
}

impl Value {
    /// Wraps an atomic value.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    pub fn atom(atom: Atom) -> Self {
        Self::Atom(atom)
    }

    /// The empty list.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    pub fn list_empty() -> Self {
        Self::List(Rc::from(Vec::new().into_boxed_slice()))
    }

    /// Builds a list from an iterator over values.
    ///
    /// Accepts `impl IntoIterator` so arrays, vectors, slices of references and
    /// iterator adaptors all work without the caller pre-collecting.
    ///
    /// # Panics
    ///
    /// Never: construction is total. The underlying iterator is consumed to
    /// completion; a panicking element source propagates that panic instead.
    pub fn list_from_iter(iter: impl IntoIterator<Item = impl Into<Value>>) -> Self {
        let items: Vec<Value> = iter.into_iter().map(Into::into).collect();
        Self::List(Rc::from(items.into_boxed_slice()))
    }

    /// Borrows the wrapped atom, or `None` for a list.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn as_atom(&self) -> Option<&Atom> {
        match self {
            Self::Atom(atom) => Some(atom),
            Self::List(_) => None,
        }
    }

    /// Borrows the elements, or `None` for an atom.
    ///
    /// A cheap view: the slice is the shared buffer itself, no copy.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Self::Atom(_) => None,
            Self::List(items) => Some(items),
        }
    }

    /// Returns a new list with `value` added at the end, leaving `self`
    /// unchanged.
    ///
    /// Returns `None` when `self` is not a list: there are no elements to
    /// rebuild. Coercing an atom into a one-element list instead would decide a
    /// type rule of the language — what `(append 3 4)` evaluates to — and that
    /// question belongs to the reference behaviour of the evaluator (M3-C), not
    /// to the representation.
    ///
    /// # Panics
    ///
    /// Never in this method; it allocates the new buffer, so an allocation
    /// failure follows the usual process-wide handling.
    pub fn append(&self, value: Value) -> Option<Value> {
        let items = self.as_list()?;
        let mut rebuilt = Vec::with_capacity(items.len() + 1);
        rebuilt.extend_from_slice(items);
        rebuilt.push(value);
        Some(Self::List(Rc::from(rebuilt.into_boxed_slice())))
    }

    /// Returns a new list with `value` added at the front, leaving `self`
    /// unchanged.
    ///
    /// `None` for a non-list, for the reason given on [`Value::append`].
    ///
    /// # Panics
    ///
    /// Never in this method; see [`Value::append`] about allocation.
    pub fn prepend(&self, value: Value) -> Option<Value> {
        let items = self.as_list()?;
        let mut rebuilt = Vec::with_capacity(items.len() + 1);
        rebuilt.push(value);
        rebuilt.extend_from_slice(items);
        Some(Self::List(Rc::from(rebuilt.into_boxed_slice())))
    }

    /// The number of elements, or `None` for an atom.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn len(&self) -> Option<usize> {
        self.as_list().map(<[Value]>::len)
    }

    /// Returns `true` if this is an empty list.
    ///
    /// An atom is never empty in this sense: it holds a value, and whether
    /// `Atom::void()` counts as "empty" is a language question.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn is_empty(&self) -> bool {
        self.as_list().is_some_and(<[Value]>::is_empty)
    }

    /// Returns `true` if this is the `Atom::Void` unit value.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn is_void(&self) -> bool {
        matches!(self, Self::Atom(atom) if atom.is_void())
    }

    /// Returns `true` if this is a first-class `Atom::Error` value.
    ///
    /// Existential only: an error is a value at this layer and propagates
    /// nowhere (AGENTS.md §3, error 一等值).
    ///
    /// # Panics
    ///
    /// Never.
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Atom(atom) if atom.is_error())
    }
}

impl Default for Value {
    /// The default value is `Atom::Void`, following the core layer, where
    /// `Default for Atom = Atom::Void` (ADR-10).
    ///
    /// Chosen over an empty list because `Void` is the *absence of a result* —
    /// the value a binding starts out with and a procedure with nothing to
    /// return produces — while an empty list is a perfectly specific value the
    /// language has to be asked for. Defaulting to it would make "nothing yet"
    /// and "a list with no elements" the same thing before any rule says so.
    fn default() -> Self {
        Self::Atom(Atom::Void)
    }
}

impl From<Atom> for Value {
    /// Lifts an atomic value; equivalent to [`Value::atom`].
    fn from(atom: Atom) -> Self {
        Value::Atom(atom)
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Atom(a), Self::Atom(b)) => a == b,
            (Self::List(a), Self::List(b)) => a.as_ref() == b.as_ref(),
            _ => false,
        }
    }
}

impl Eq for Value {}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Atom(atom) => {
                HASH_ATOM_TAG.hash(state);
                atom.hash(state);
            }
            Self::List(items) => {
                HASH_LIST_TAG.hash(state);
                // Length first, so a list and its elements cannot trade places
                // in the byte stream the hasher sees.
                items.len().hash(state);
                for item in items.iter() {
                    item.hash(state);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These unit tests cover the private hashing kernels only; every
    // observable behaviour of this module is tested from the outside in
    // `crates/rhytido-lang/tests/value.rs` (AGENTS.md §4).

    fn bytes_of(value: &Value) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn normal_hash_tags_separate_atoms_from_lists() {
        let atom = Value::atom(Atom::void());
        let list = Value::list_empty();
        assert_ne!(bytes_of(&atom), bytes_of(&list));
    }

    #[test]
    fn normal_hash_agrees_with_equality_on_nested_lists() {
        let inner = Value::list_from_iter([Value::atom(Atom::boolean(false))]);
        // Two ways of building the same `[[false], [false]]`.
        let outer = Value::list_empty()
            .append(inner.clone())
            .expect("list")
            .append(inner.clone())
            .expect("list");
        let rebuilt = Value::list_from_iter([inner.clone(), inner]);
        assert_eq!(outer, rebuilt);
        assert_eq!(bytes_of(&outer), bytes_of(&rebuilt));
    }

    #[test]
    fn extreme_hash_of_deeply_shared_list_is_stable() {
        // A buffer aliased many times hashes to the same bucket every round,
        // because sharing never enters the hash.
        let leaf = Value::list_from_iter([Value::atom(Atom::void())]);
        let mut nested = leaf.clone();
        for _ in 0..64 {
            nested = Value::list_from_iter([nested.clone(), leaf.clone()]);
        }
        assert_eq!(bytes_of(&nested), bytes_of(&nested.clone()));
    }
}
