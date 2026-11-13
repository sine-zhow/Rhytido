// SPDX-License-Identifier: Apache-2.0
//! Module tests for `symbol` (src/symbol.rs): per-session identifier interning.
//!
//! Layer: module tests (AGENTS.md §4) — external view of the public API only.
//! Interning is total by contract, so the "error" category asserts the meaning
//! of *unknown* identifiers (a lookup miss reported as `None`) rather than
//! failure returns, which the type does not have.

use std::collections::HashSet;

use rhytido_lang::{SymbolId, SymbolTable};

// ---- Helpers ---------------------------------------------------------------

/// Ids `0..count`, without going through the table under test.
fn synthetic_ids(count: u32) -> Vec<SymbolId> {
    (0..count).map(SymbolId::new).collect()
}

// ---- Normal cases ----------------------------------------------------------

#[test]
fn normal_intern_returns_text_through_lookup() {
    let mut table = SymbolTable::new();
    let id = table.intern("alpha");
    assert_eq!(table.text(id), Some("alpha"));
}

#[test]
fn normal_intern_is_stable_for_repeated_text() {
    let mut table = SymbolTable::new();
    let first = table.intern("beta");
    for _ in 0..16 {
        assert_eq!(table.intern("beta"), first);
    }
    assert_eq!(table.len(), 1);
}

#[test]
fn normal_distinct_texts_get_distinct_ids() {
    let mut table = SymbolTable::new();
    let names: Vec<&str> = vec!["gamma", "Gamma", "gamma ", " gamma", "gmma"];
    let ids: Vec<SymbolId> = names.iter().map(|n| table.intern(n)).collect();
    let unique: HashSet<SymbolId> = ids.iter().copied().collect();
    assert_eq!(unique.len(), names.len());
    for (id, name) in ids.iter().zip(&names) {
        assert_eq!(table.text(*id), Some(*name));
    }
}

#[test]
fn normal_ids_are_allocated_in_first_seeing_order() {
    let mut table = SymbolTable::new();
    let a = table.intern("a");
    let b = table.intern("b");
    let again = table.intern("a");
    assert_eq!(a, SymbolId::new(0));
    assert_eq!(b, SymbolId::new(1));
    assert_eq!(again, a);
    assert_eq!(u32::from(b), 1);
    assert_eq!(b.index(), 1);
}

#[test]
fn normal_new_and_default_are_both_empty() {
    let table = SymbolTable::new();
    assert!(table.is_empty());
    assert_eq!(table.len(), 0);
    assert_eq!(table, SymbolTable::default());
}

#[test]
fn normal_clone_keeps_ids_naming_the_same_text() {
    let mut table = SymbolTable::new();
    let id = table.intern("shared");
    let copy = table.clone();
    assert_eq!(copy, table);
    assert_eq!(copy.text(id), Some("shared"));
}

#[test]
fn normal_symbol_id_conversions_round_trip() {
    let id = SymbolId::new(4242);
    assert_eq!(SymbolId::from(4242u32), id);
    assert_eq!(u32::from(id), 4242);
    assert_eq!(id.index(), 4242);
}

// ---- Extreme cases ---------------------------------------------------------

#[test]
fn extreme_empty_string_is_a_valid_identifier() {
    let mut table = SymbolTable::new();
    let empty = table.intern("");
    assert_eq!(table.text(empty), Some(""));
    assert_eq!(table.intern(""), empty);
    assert_ne!(empty, table.intern(" "));
    assert_eq!(table.len(), 2);
}

#[test]
fn extreme_non_ascii_and_control_characters_round_trip() {
    let mut table = SymbolTable::new();
    for name in ["über", "\u{1f600}", "a\0b", "\u{200b}", "\n\t"] {
        let id = table.intern(name);
        assert_eq!(table.text(id), Some(name));
    }
}

#[test]
fn extreme_very_long_identifier_is_recalled_by_id() {
    let mut table = SymbolTable::new();
    let long = "s".repeat(1 << 16);
    let id = table.intern(&long);
    assert_eq!(table.text(id), Some(long.as_str()));
    // Interning the same 64 KiB spelling again is a lookup, not a second copy.
    assert_eq!(table.intern(&long), id);
    assert_eq!(table.len(), 1);
}

#[test]
fn extreme_many_symbols_get_unique_dense_ids() {
    let mut table = SymbolTable::new();
    let count = 20_000;
    let mut seen = HashSet::with_capacity(count);
    for index in 0..count {
        let id = table.intern(&format!("symbol_{index}"));
        assert!(seen.insert(id), "id {id:?} handed out twice");
    }
    assert_eq!(table.len(), count);
    for index in 0..count {
        let id = SymbolId::new(u32::try_from(index).expect("count fits u32"));
        assert_eq!(table.text(id), Some(format!("symbol_{index}").as_str()));
    }
}

#[test]
fn extreme_unknown_ids_are_rejected_not_misread() {
    let mut table = SymbolTable::new();
    table.intern("only");
    // Every id outside the allocated range reads back as unknown.
    for id in synthetic_ids(0).into_iter().chain([
        SymbolId::new(1),
        SymbolId::new(2),
        SymbolId::new(u32::MAX / 2),
        SymbolId::new(u32::MAX),
    ]) {
        assert_eq!(table.text(id), None);
    }
    assert_eq!(table.text(SymbolId::new(0)), Some("only"));
}

#[test]
fn extreme_ids_from_another_table_do_not_resolve() {
    let mut first = SymbolTable::new();
    let a = first.intern("a");
    let b = first.intern("b");
    let mut second = SymbolTable::new();
    // Nothing interned yet: both ids are unknown here.
    assert_eq!(second.text(a), None);
    assert_eq!(second.text(b), None);
    // The same spelling in the other table starts its own numbering, which
    // happens to agree — identity per table, not globally.
    assert_eq!(second.intern("b"), a);
    assert_eq!(second.text(a), Some("b"));
    assert_eq!(first.text(a), Some("a"));
}

// ---- Error cases -----------------------------------------------------------

#[test]
fn error_id_beyond_the_table_reads_as_none() {
    let table = SymbolTable::new();
    // An empty table resolves nothing, not even the id a fresh intern takes.
    assert_eq!(table.text(SymbolId::new(0)), None);
}

#[test]
fn error_partial_text_never_resolves_to_a_longer_interned_name() {
    let mut table = SymbolTable::new();
    let id = table.intern("prefix_long_name");
    assert_ne!(table.intern("prefix"), id);
    assert_eq!(table.text(id), Some("prefix_long_name"));
    assert_eq!(table.len(), 2);
}

#[test]
fn error_case_difference_is_a_different_identifier() {
    let mut table = SymbolTable::new();
    let lower = table.intern("Case");
    let upper = table.intern("case");
    assert_ne!(lower, upper);
    assert_eq!(table.text(lower), Some("Case"));
    assert_eq!(table.text(upper), Some("case"));
}

// ---- Robustness ------------------------------------------------------------

#[test]
fn robustness_intern_and_text_repeat_converges() {
    let mut table = SymbolTable::new();
    let mut ids = Vec::new();
    for round in 0..8 {
        let id = table.intern(&format!("round_{round}"));
        ids.push(id);
        for (index, existing) in ids.iter().enumerate() {
            assert_eq!(
                table.text(*existing),
                Some(format!("round_{index}").as_str())
            );
        }
    }
    assert_eq!(table.len(), 8);
}

#[test]
fn robustness_symbol_id_ordering_is_by_index_only() {
    let mut table = SymbolTable::new();
    let z = table.intern("zzzz");
    let a = table.intern("aaa");
    // Ordering follows allocation, not spelling: no text order is implied.
    assert!(z < a);
    let mut sorted = [a, z];
    sorted.sort_unstable();
    assert_eq!(sorted, [z, a]);
}

#[test]
fn robustness_debug_and_hash_are_total() {
    let mut table = SymbolTable::new();
    let id = table.intern("dbg");
    let _ = format!("{id:?}");
    let _ = format!("{table:?}");
    let mut set = HashSet::new();
    set.insert(id);
    set.insert(table.intern("dbg"));
    assert_eq!(set.len(), 1);
}

#[test]
fn robustness_dropped_table_invalidates_its_ids_for_others() {
    let dropped_ids: Vec<SymbolId> = {
        let mut temp = SymbolTable::new();
        (0..3)
            .map(|index| temp.intern(&format!("dropped_{index}")))
            .collect()
    };
    assert_eq!(dropped_ids.len(), 3);

    let mut survivor = SymbolTable::new();
    let live = survivor.intern("x");
    // Both tables number from zero, so a foreign id can alias a live entry:
    // reading one table's ids through another is meaningless by construction,
    // which is why text is only ever resolved by the table that minted the id.
    assert_eq!(live, dropped_ids[0]);
    assert_eq!(survivor.text(live), Some("x"));
    assert_eq!(survivor.text(dropped_ids[1]), None);
    assert_eq!(survivor.text(dropped_ids[2]), None);
    // And none of the dead table's spellings is reachable from the live one.
    for id in &dropped_ids {
        assert_ne!(survivor.text(*id), Some("dropped_1"));
    }
}
