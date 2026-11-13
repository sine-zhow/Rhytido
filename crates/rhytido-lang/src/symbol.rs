// SPDX-License-Identifier: Apache-2.0
//! Per-session identifier interning (ADR-11).
//!
//! Language identifiers are compared constantly during frame lookups, so they
//! are interned once into a [`SymbolId`] and compared as small integers
//! afterwards. Interning is *owned by a session*, never by a global table: two
//! sessions may intern `"x"` and get the same id by coincidence, but neither
//! can observe, or be disturbed by, the other's table (AGENTS.md §3, 零全局状态;
//! ADR-10 explicitly rejects a global intern table).
//!
//! The contract is only identity and recall — `intern` maps identical text to
//! one stable id, `text` reads it back. Nothing here orders, pools, or garbage
//! collects symbols; those would be policy decisions about the language, not
//! about representation.
//!
//! # Examples
//!
//! ```
//! use rhytido_lang::SymbolTable;
//!
//! let mut table = SymbolTable::new();
//! let a = table.intern("foo");
//! let b = table.intern("bar");
//! // Identical text always interns to the identical id.
//! assert_eq!(a, table.intern("foo"));
//! assert_ne!(a, b);
//! assert_eq!(table.text(a), Some("foo"));
//! // An id from another table is simply unknown here.
//! let mut other = SymbolTable::new();
//! assert_eq!(other.text(a), None);
//! assert_eq!(other.intern("foo"), a);
//! ```

use std::collections::HashMap;

/// An identifier interned in some [`SymbolTable`].
///
/// Newtype over `u32` so a symbol cannot be confused with any other integer
/// the surrounding code carries around (AGENTS.md §2.4 C-NEWTYPE). Ids are only
/// meaningful together with the table that produced them: they say nothing
/// about scope, and hold no value.
///
/// # Examples
///
/// ```
/// use rhytido_lang::SymbolId;
///
/// // Debug shows the numeric id; useful in diagnostics only, not a printing
/// // contract (presentation belongs to L6).
/// let id = SymbolId::from(7);
/// assert_eq!(format!("{id:?}"), "SymbolId(7)");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolId(u32);

impl SymbolId {
    /// The identifier with index `index`.
    ///
    /// Prefer [`SymbolTable::intern`]: a hand-made id is only valid in a table
    /// that already holds that many symbols.
    ///
    /// # Panics
    ///
    /// Never: the index is stored as given.
    pub fn new(index: u32) -> Self {
        Self(index)
    }

    /// The zero-based index of this identifier.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn index(self) -> u32 {
        self.0
    }
}

impl From<u32> for SymbolId {
    /// Equivalent to [`SymbolId::new`].
    fn from(index: u32) -> Self {
        Self(index)
    }
}

impl From<SymbolId> for u32 {
    /// Equivalent to [`SymbolId::index`].
    fn from(id: SymbolId) -> Self {
        id.0
    }
}

/// The identifier table of one session: text in, stable id out, and back.
///
/// Interning is case sensitive and content based; the text is stored once and
/// borrowed, so repeated interning of the same spelling is a hash lookup and
/// allocates nothing. [`Clone`] and [`PartialEq`] are structural, which is what
/// makes a *copied* session keep working: its ids still name the same text, and
/// ids taken from an unrelated table are rejected as unknown rather than
/// misread.
///
/// # Examples
///
/// ```
/// use rhytido_lang::SymbolTable;
///
/// let mut table = SymbolTable::new();
/// assert!(table.is_empty());
/// let x = table.intern("x");
/// assert_eq!(table.len(), 1);
/// assert!(!table.is_empty());
/// let copy = table.clone();
/// assert_eq!(copy, table);
/// assert_eq!(copy.text(x), Some("x"));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SymbolTable {
    /// Interned text, indexed by id.
    texts: Vec<String>,
    /// Reverse index, text to id.
    ids: HashMap<String, SymbolId>,
}

impl SymbolTable {
    /// Creates an empty table.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn new() -> Self {
        Self::default()
    }

    /// Interns `text`, returning its identifier.
    ///
    /// The first call for a spelling allocates the entry and hands out the next
    /// id; every later call with the same contents returns that id again. Text
    /// is kept verbatim, including the empty string, which is a perfectly good
    /// identifier as far as representation is concerned.
    ///
    /// # Panics
    ///
    /// Panics only if the table already holds `u32::MAX` distinct symbols and
    /// a new one is interned (id space exhausted). This is a documented
    /// invariant breach, not an expected error: 4 billion live identifiers in
    /// one session means the caller has stopped interning.
    pub fn intern(&mut self, text: &str) -> SymbolId {
        if let Some(&id) = self.ids.get(text) {
            return id;
        }
        let next = u32::try_from(self.texts.len())
            .expect("symbol table exhausted the u32 identifier space");
        let id = SymbolId(next);
        self.texts.push(text.to_owned());
        self.ids.insert(text.to_owned(), id);
        id
    }

    /// Borrows the text interned under `id`, or `None` if this table never
    /// interned it.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn text(&self, id: SymbolId) -> Option<&str> {
        self.texts.get(id.index() as usize).map(String::as_str)
    }

    /// Returns the number of distinct interned identifiers.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn len(&self) -> usize {
        self.texts.len()
    }

    /// Returns `true` if nothing has been interned yet.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn is_empty(&self) -> bool {
        self.texts.is_empty()
    }
}
