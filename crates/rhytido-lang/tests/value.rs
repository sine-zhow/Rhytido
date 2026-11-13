// SPDX-License-Identifier: Apache-2.0
//! Module tests for `value` (src/value.rs): the composite value type (ADR-9).
//!
//! Layer: module tests (AGENTS.md §4) — external view of the public API only.
//! Construction and rebuilding are total over lists, so the "error" category
//! pins the documented non-list behaviour (`append`/`prepend` yielding `None`)
//! rather than failure returns.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use rhytido_backend::{BigFloat, Integer, Rational};
use rhytido_core::{Atom, Bound, Range};
use rhytido_lang::Value;

// ---- Helpers ---------------------------------------------------------------

fn hash_of(value: &Value) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn integer_value(value: i64) -> Value {
    Value::atom(Atom::integer(Integer::from(value)))
}

/// One representative per atom shape plus the list shapes.
fn samples() -> Vec<Value> {
    vec![
        Value::atom(Atom::number(BigFloat::one())),
        Value::atom(Atom::number(BigFloat::NAN)),
        Value::atom(Atom::number(-BigFloat::zero())),
        Value::atom(Atom::integer(Integer::from(0))),
        Value::atom(Atom::rational(
            Rational::new(1, 2).expect("nonzero denominator"),
        )),
        Value::atom(Atom::boolean(true)),
        Value::atom(Atom::string("")),
        Value::atom(Atom::range(Range::new(
            Bound::closed(BigFloat::one()),
            Bound::pos_infinity(),
        ))),
        Value::atom(Atom::error("boom", Some(7))),
        Value::atom(Atom::Void),
        Value::list_empty(),
        Value::list_from_iter([Value::atom(Atom::Void)]),
        Value::list_from_iter([Value::atom(Atom::Void), Value::list_empty()]),
        Value::list_from_iter([Value::list_from_iter([Value::atom(Atom::Void)])]),
    ]
}

// ---- Normal cases: construction and views ----------------------------------

#[test]
fn normal_atom_constructor_wraps_the_atom() {
    let value = Value::atom(Atom::boolean(true));
    assert_eq!(value.as_atom(), Some(&Atom::boolean(true)));
    assert!(value.as_list().is_none());
}

#[test]
fn normal_from_atom_equals_atom_constructor() {
    let atom = Atom::string("lifted");
    assert_eq!(Value::from(atom.clone()), Value::atom(atom));
}

#[test]
fn normal_list_empty_is_an_empty_list() {
    let empty = Value::list_empty();
    assert_eq!(empty.as_list().map(<[Value]>::len), Some(0));
    assert_eq!(empty.len(), Some(0));
    assert!(empty.is_empty());
    assert!(empty.as_atom().is_none());
}

#[test]
fn normal_list_from_iter_preserves_order_and_length() {
    let values: Vec<Value> = (0..5)
        .map(|i| Value::atom(Atom::integer(Integer::from(i))))
        .collect();
    let list = Value::list_from_iter(values.clone());
    let items = list.as_list().expect("list");
    assert_eq!(items, values.as_slice());
    assert_eq!(list.len(), Some(5));
    assert!(!list.is_empty());
}

#[test]
fn normal_list_from_iter_accepts_atoms_and_borrowed_slices() {
    // `impl Into<Value>` lets an iterator of atoms build a list directly.
    let from_atoms = Value::list_from_iter([Atom::boolean(true), Atom::boolean(false)]);
    let from_values = Value::list_from_iter([
        Value::atom(Atom::boolean(true)),
        Value::atom(Atom::boolean(false)),
    ]);
    assert_eq!(from_atoms, from_values);
}

#[test]
fn normal_list_from_an_empty_iterator_is_the_empty_list() {
    let empty = Value::list_from_iter(std::iter::empty::<Value>());
    assert_eq!(empty, Value::list_empty());
}

#[test]
fn normal_default_is_the_void_atom() {
    assert_eq!(Value::default(), Value::atom(Atom::Void));
    assert!(Value::default().is_void());
}

#[test]
fn normal_predicates_report_the_wrapped_atom_kind() {
    assert!(Value::atom(Atom::error("x", None)).is_error());
    assert!(!Value::atom(Atom::void()).is_error());
    assert!(!Value::list_empty().is_void());
    assert!(!Value::list_from_iter([Value::atom(Atom::void())]).is_void());
}

// ---- Normal cases: rebuilding ----------------------------------------------

#[test]
fn normal_append_adds_at_the_end_and_leaves_the_original() {
    let original = Value::list_from_iter([Value::atom(Atom::integer(Integer::from(1)))]);
    let appended = original
        .append(Value::atom(Atom::integer(Integer::from(2))))
        .expect("list");
    assert_eq!(original.len(), Some(1));
    assert_eq!(appended.len(), Some(2));
    let items = appended.as_list().expect("list");
    assert_eq!(items[1], Value::atom(Atom::integer(Integer::from(2))));
}

#[test]
fn normal_prepend_adds_at_the_front_and_leaves_the_original() {
    let original = Value::list_from_iter([Value::atom(Atom::integer(Integer::from(1)))]);
    let prepended = original
        .prepend(Value::atom(Atom::integer(Integer::from(0))))
        .expect("list");
    assert_eq!(original.len(), Some(1));
    assert_eq!(prepended.len(), Some(2));
    assert_eq!(
        prepended.as_list().expect("list")[0],
        Value::atom(Atom::integer(Integer::from(0)))
    );
}

#[test]
fn normal_append_to_empty_singleton_matches_prepend_to_empty() {
    let item = Value::atom(Atom::boolean(true));
    let via_append = Value::list_empty().append(item.clone()).expect("list");
    let via_prepend = Value::list_empty().prepend(item).expect("list");
    assert_eq!(via_append, via_prepend);
}

#[test]
fn normal_rebuild_appends_are_associative_in_content() {
    let a = Value::atom(Atom::string("a"));
    let b = Value::atom(Atom::string("b"));
    let stepwise = Value::list_empty()
        .append(a.clone())
        .expect("list")
        .append(b.clone())
        .expect("list");
    let all_at_once = Value::list_from_iter([a, b]);
    assert_eq!(stepwise, all_at_once);
}

// ---- Normal cases: equality and hashing ------------------------------------

#[test]
fn normal_equal_lists_built_differently_compare_equal() {
    let left = Value::list_from_iter([Value::atom(Atom::void()), Value::list_empty()]);
    let right = Value::list_empty()
        .append(Value::list_empty())
        .expect("list")
        .prepend(Value::atom(Atom::void()))
        .expect("list");
    assert_eq!(left, right);
    assert_eq!(hash_of(&left), hash_of(&right));
}

#[test]
fn normal_atom_equality_is_the_core_semantics_verbatim() {
    // The precision label is irrelevant inside a value just as it is outside.
    let shallow = Value::atom(Atom::number(BigFloat::from_i64(3)));
    let deep = Value::atom(Atom::number(BigFloat::from_i64(3).with_precision(500)));
    assert_eq!(shallow, deep);
    assert_eq!(hash_of(&shallow), hash_of(&deep));
}

#[test]
fn normal_nan_atoms_are_equal_inside_a_list() {
    let a = Value::list_from_iter([Value::atom(Atom::number(BigFloat::NAN))]);
    let b = Value::list_from_iter([Value::atom(Atom::number(BigFloat::NAN))]);
    assert_eq!(a, b);
    assert_eq!(hash_of(&a), hash_of(&b));
}

// ---- Extreme cases ---------------------------------------------------------

#[test]
fn extreme_ten_thousand_element_list_rebuilds_without_loss() {
    let big = Value::list_from_iter((0..10_000).map(integer_value));
    assert_eq!(big.len(), Some(10_000));
    let appended = big.append(Value::atom(Atom::void())).expect("list");
    let prepended = big.prepend(Value::atom(Atom::void())).expect("list");
    assert_eq!(appended.len(), Some(10_001));
    assert_eq!(prepended.len(), Some(10_001));
    // Both keep the original content in place, shifted where prepended.
    assert_eq!(
        appended.as_list().expect("list")[9_999],
        integer_value(9_999)
    );
    assert_eq!(
        appended.as_list().expect("list")[10_000],
        Value::atom(Atom::void())
    );
    assert_eq!(
        prepended.as_list().expect("list")[5_000],
        integer_value(4_999)
    );
    assert_eq!(big.len(), Some(10_000), "original untouched");
}

#[test]
fn extreme_list_of_lists_of_lists_compares_structurally() {
    let leaf = Value::list_empty();
    let level1 = Value::list_from_iter([leaf.clone()]);
    let level2 = Value::list_from_iter([level1.clone()]);
    let rebuilt2 = Value::list_from_iter([Value::list_from_iter([Value::list_empty()])]);
    assert_eq!(level2, rebuilt2);
    assert_ne!(level2, level1);
    assert_ne!(level1, leaf);
    assert_eq!(hash_of(&level2), hash_of(&rebuilt2));
}

#[test]
fn extreme_signed_zeros_stay_distinct_inside_a_list() {
    let pos = Value::list_from_iter([Value::atom(Atom::number(BigFloat::zero()))]);
    let neg = Value::list_from_iter([Value::atom(Atom::number(-BigFloat::zero()))]);
    assert_ne!(pos, neg);
    assert_ne!(hash_of(&pos), hash_of(&neg));
}

#[test]
fn extreme_ordering_and_length_are_part_of_list_identity() {
    let a = Value::atom(Atom::string("a"));
    let b = Value::atom(Atom::string("b"));
    let ab = Value::list_from_iter([a.clone(), b.clone()]);
    let ba = Value::list_from_iter([b.clone(), a.clone()]);
    let shorter = Value::list_from_iter([a]);
    assert_ne!(ab, ba, "order is part of a list's identity");
    assert_ne!(ab, shorter, "length is part of a list's identity");
    assert_ne!(hash_of(&ab), hash_of(&shorter));
    // A rotation of the same elements is a different list of the same length.
    assert_eq!(ab.len(), ba.len());
    assert_ne!(ab, ba.clone().prepend(b).expect("list"));
}

#[test]
fn extreme_wide_rational_and_huge_integer_survive_equality() {
    let huge = Integer::from(2).pow(400) + Integer::from(1);
    let wide = {
        let mut r = Rational::new(1, 3).expect("nonzero denominator");
        for _ in 0..8 {
            r = &r * &r;
        }
        r
    };
    let values = [
        Value::atom(Atom::integer(huge.clone())),
        Value::atom(Atom::rational(wide.clone())),
    ];
    for value in &values {
        assert_eq!(value, &value.clone());
        assert_eq!(hash_of(value), hash_of(&value.clone()));
    }
    let list = Value::list_from_iter(values.iter().cloned());
    assert_eq!(list, Value::list_from_iter(values.iter().cloned()));
}

#[test]
fn extreme_error_value_is_an_ordinary_element() {
    // error 一等值: an error inside a list is data, not a control-flow event.
    let error = Atom::error("", None);
    let list = Value::list_from_iter([Value::atom(error.clone()), Value::atom(Atom::void())]);
    assert_eq!(list.len(), Some(2));
    assert!(list.as_list().expect("list")[0].is_error());
    assert_eq!(
        list.as_list().expect("list")[0].as_atom(),
        Some(&Atom::error("", None))
    );
}

// ---- Error cases: operations defined on lists only -------------------------

#[test]
fn error_append_on_an_atom_is_declined_not_guessed() {
    let atom = Value::atom(Atom::integer(Integer::from(3)));
    assert_eq!(atom.append(Value::atom(Atom::void())), None);
    assert_eq!(atom.prepend(Value::atom(Atom::void())), None);
    // And the atom is unchanged by the declined operation.
    assert_eq!(atom, Value::atom(Atom::integer(Integer::from(3))));
}

#[test]
fn error_views_of_the_wrong_shape_return_none() {
    let atom = Value::atom(Atom::void());
    let list = Value::list_empty();
    assert!(atom.as_list().is_none());
    assert!(list.as_atom().is_none());
    assert_eq!(atom.len(), None);
    assert!(!atom.is_empty(), "an atom is not an empty list");
}

#[test]
fn error_atom_and_list_variants_never_compare_equal() {
    // A list holding nothing and the unit value are different values.
    assert_ne!(Value::list_empty(), Value::atom(Atom::Void));
    assert_ne!(Value::atom(Atom::Void), Value::list_empty());
}

#[test]
fn error_nested_atom_versus_singleton_list_are_distinct() {
    let nested = Value::list_from_iter([Value::atom(Atom::Void)]);
    let deep = Value::list_from_iter([Value::list_from_iter([Value::atom(Atom::Void)])]);
    assert_ne!(nested, deep);
    assert_ne!(Value::atom(Atom::Void), nested);
}

// ---- Robustness ------------------------------------------------------------

#[test]
fn robustness_samples_are_reflexive_and_hash_stable() {
    for value in samples() {
        // Reflexive by way of a copy, so the comparison is not trivially true.
        assert!(value == value.clone());
        let once = hash_of(&value);
        for _ in 0..4 {
            assert_eq!(hash_of(&value), once);
        }
    }
}

#[test]
fn robustness_equal_values_collapse_in_hash_structures() {
    let built = Value::list_from_iter([Value::atom(Atom::boolean(true)), Value::list_empty()]);
    let rebuilt = Value::list_empty()
        .append(Value::atom(Atom::boolean(true)))
        .expect("list")
        .append(Value::list_empty())
        .expect("list");
    let mut set = HashSet::new();
    set.insert(built.clone());
    set.insert(rebuilt.clone());
    assert_eq!(set.len(), 1);
    let mut map = HashMap::new();
    map.insert(built.clone(), "first");
    map.insert(rebuilt, "second");
    assert_eq!(map.len(), 1);
    assert_eq!(map.get(&built), Some(&"second"));
}

#[test]
fn robustness_clone_aliases_the_buffer_and_never_shares_mutation() {
    let original = Value::list_from_iter([Value::atom(Atom::void())]);
    let clone = original.clone();
    // Cloning does not copy the element buffer.
    assert!(std::ptr::eq(
        original.as_list().expect("list").as_ptr(),
        clone.as_list().expect("list").as_ptr()
    ));
    // "Mutating" through a rebuild leaves every alias at its old length.
    let _rebuilt = clone.append(Value::atom(Atom::void())).expect("list");
    assert_eq!(original.len(), Some(1));
    assert_eq!(clone.len(), Some(1));
}

#[test]
fn robustness_debug_and_clone_are_total_over_every_shape() {
    for value in samples() {
        let _ = format!("{value:?}");
        let _ = value.clone();
    }
}

#[test]
fn robustness_deeply_nested_list_still_compares_and_hashes() {
    // A 200-deep nesting of single-element lists: recursion that a real
    // session could build, exercised to a bound.
    let mut value = Value::list_empty();
    for _ in 0..200 {
        value = Value::list_from_iter([value]);
    }
    let same = value.clone();
    assert_eq!(value, same);
    assert_eq!(hash_of(&value), hash_of(&same));
    assert_ne!(value, Value::list_empty());
}
