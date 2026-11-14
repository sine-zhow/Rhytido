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
use rhytido_core::{Atom, Bound, Expr, Interner, NodeId, Range};
use rhytido_lang::SymbolTable;
use rhytido_lang::ast::{Formal, Proc, ProgramExpr, Stmt};
use rhytido_lang::{Fun, SymbolId, Value};

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

// ---- Composite syntax values: `Proc` and `Function` (M3-B2) ----------------
//
// These two variants carry captured syntax rather than data, so the questions
// here are the same three that matter for any value: does it round-trip through
// its constructor and views, does equality mean *structure* rather than
// identity, and does every list-shaped operation decline it politely instead of
// inventing an answer.

/// Interned nodes for this section.
///
/// A [`rhytido_core::NodeId`] only means something together with the arena that
/// produced it, so these come from one fixed sequence interned into a fresh
/// arena on every call: the ids are deterministic across calls, and "a
/// different expression" really is a different id rather than an artefact of
/// two arenas each starting at zero.
fn nodes() -> [NodeId; 3] {
    let mut it = Interner::new();
    [
        it.intern(Expr::Constant(BigFloat::from_i64(1))),
        it.intern(Expr::Constant(BigFloat::from_i64(2))),
        it.intern(Expr::Constant(BigFloat::from_i64(3))),
    ]
}

fn first() -> NodeId {
    nodes()[0]
}

fn second() -> NodeId {
    nodes()[1]
}

fn third() -> NodeId {
    nodes()[2]
}

fn name(text: &str) -> SymbolId {
    let mut table = SymbolTable::new();
    table.intern(text)
}

/// `proc(n) { return n; }` — the smallest body with a shape worth comparing.
fn successor() -> Proc {
    let n = name("n");
    Proc::new(
        [Formal::single(n)],
        [Stmt::returning(Some(ProgramExpr::var(n)))],
    )
}

/// `proc(L = ...) {}` — the variadic shape, which no fixed-arity procedure is.
fn variadic() -> Proc {
    Proc::new([Formal::list(name("L"))], [])
}

/// One representative per value shape, including the composite syntax kinds.
fn composite_samples() -> Vec<Value> {
    vec![
        Value::proc(successor()),
        Value::proc(variadic()),
        Value::proc(Proc::new([], [])),
        Value::function(Fun::new(first())),
        Value::function(Fun::new(first()).with_name(name("f"))),
        Value::function(Fun::new(second())),
        Value::function(Fun::new(first()).with_range(Range::new(
            Bound::closed(BigFloat::one()),
            Bound::pos_infinity(),
        ))),
        Value::list_from_iter([Value::proc(successor())]),
    ]
}

#[test]
fn normal_function_new_is_bare_and_keeps_the_interned_expression() {
    let expr = first();
    let fun = Fun::new(expr);
    assert_eq!(fun.expr(), expr);
    assert_eq!(fun.range(), None);
    assert_eq!(fun.name(), None);
    assert!(fun.is_bare());
}

#[test]
fn normal_function_builders_attach_range_and_name() {
    let expr = third();
    let range = Range::new(Bound::neg_infinity(), Bound::open(BigFloat::one()));
    let fun = Fun::new(expr)
        .with_range(range.clone())
        .with_name(name("f"));
    assert_eq!(fun.expr(), expr, "the expression is shared, never copied");
    assert_eq!(fun.range(), Some(&range));
    assert_eq!(fun.name(), Some(name("f")));
    assert!(!fun.is_bare());
}

#[test]
fn normal_function_range_is_stored_verbatim_even_when_degenerate() {
    // Following the core `Range` contract (ADR-10): the language layer keeps
    // what it was given, and well-formedness is decided by the evaluator.
    let upside_down = Range::new(Bound::pos_infinity(), Bound::neg_infinity());
    let fun = Fun::new(first()).with_range(upside_down.clone());
    assert_eq!(fun.range(), Some(&upside_down));
    assert_ne!(
        fun,
        Fun::new(first()),
        "a claimed range is a different value"
    );
}

#[test]
fn normal_procedure_value_round_trips_through_constructor_and_view() {
    let proc = successor();
    let by_value = Value::proc(proc.clone());
    let by_handle = Value::proc_shared(std::rc::Rc::new(proc.clone()));
    assert_eq!(by_value, by_handle);
    assert_eq!(by_value.as_proc(), Some(&proc));
    assert!(by_value.is_proc());
    assert!(!by_value.is_function());
}

#[test]
fn normal_function_value_round_trips_through_constructor_and_view() {
    let fun = Fun::new(second()).with_name(name("g"));
    let by_value = Value::function(fun.clone());
    let by_handle = Value::function_shared(std::rc::Rc::new(fun.clone()));
    assert_eq!(by_value, by_handle);
    assert_eq!(by_value.as_function(), Some(&fun));
    assert!(by_value.is_function());
    assert!(!by_value.is_proc());
}

#[test]
fn normal_procedure_equality_is_structural_not_identity() {
    // Two independently built procedures with the same shape are equal, however
    // many times their buffers are aliased.
    assert_eq!(Value::proc(successor()), Value::proc(successor()));
    assert_eq!(
        hash_of(&Value::proc(successor())),
        hash_of(&Value::proc(successor()))
    );
    assert_ne!(
        Value::proc(successor()),
        Value::proc(variadic()),
        "arity is part of the value"
    );
    assert_ne!(
        Value::proc(Proc::new([], [])),
        Value::proc(successor()),
        "a body of one statement is not a body of none"
    );
}

#[test]
fn normal_function_equality_compares_expression_range_and_name() {
    assert_eq!(
        Value::function(Fun::new(first())),
        Value::function(Fun::new(first()))
    );
    assert_ne!(
        Value::function(Fun::new(first())),
        Value::function(Fun::new(second()))
    );
    assert_ne!(
        Value::function(Fun::new(first())),
        Value::function(Fun::new(first()).with_name(name("f"))),
        "a named function differs from an anonymous one"
    );
}

#[test]
fn error_list_operations_decline_procedure_and_function_values() {
    // There are no elements to rebuild, and coercing a procedure into a
    // one-element list would decide a language rule this layer does not own.
    for value in composite_samples() {
        if matches!(value, Value::List(_)) {
            continue;
        }
        assert_eq!(value.len(), None);
        assert!(!value.is_empty());
        assert!(value.append(Value::default()).is_none());
        assert!(value.prepend(Value::default()).is_none());
        assert!(value.as_list().is_none());
        assert!(value.as_atom().is_none());
    }
}

#[test]
fn error_the_four_kinds_never_compare_equal_across_shapes() {
    let by_kind = [
        Value::atom(Atom::Void),
        Value::list_empty(),
        Value::proc(successor()),
        Value::function(Fun::new(first())),
    ];
    for (index, left) in by_kind.iter().enumerate() {
        for right in by_kind.iter().skip(index + 1) {
            assert_ne!(
                left, right,
                "distinct kinds stay distinct: {left:?} vs {right:?}"
            );
            assert_ne!(hash_of(left), hash_of(right));
        }
    }
}

#[test]
fn error_views_of_the_wrong_composite_kind_return_none() {
    let proc_value = Value::proc(successor());
    assert!(proc_value.as_function().is_none());
    let fun_value = Value::function(Fun::new(first()));
    assert!(fun_value.as_proc().is_none());
    // And the atom predicates are not fooled by either.
    assert!(!proc_value.is_void());
    assert!(!proc_value.is_error());
    assert!(!fun_value.is_void());
    assert!(!fun_value.is_error());
}

#[test]
fn extreme_procedure_value_with_a_large_body_compares_and_hashes() {
    let n = name("n");
    let mut statements = Vec::new();
    for index in 0..500u32 {
        statements.push(Stmt::expr_stmt(ProgramExpr::var(SymbolId::new(index))));
    }
    statements.push(Stmt::returning(Some(ProgramExpr::var(n))));
    let big = Proc::new([Formal::single(n)], statements.iter().cloned());
    let same = Proc::new([Formal::single(n)], statements.iter().cloned());
    let left = Value::proc(big);
    let right = Value::proc(same);
    assert_eq!(left, right);
    assert_eq!(hash_of(&left), hash_of(&right));
    // Dropping one statement changes the value: length is part of the shape.
    let shorter = Proc::new(
        [Formal::single(n)],
        statements.iter().take(499).cloned().collect::<Vec<_>>(),
    );
    assert_ne!(left, Value::proc(shorter));
}

#[test]
fn extreme_a_procedure_held_in_a_list_keeps_its_identity_by_structure() {
    // A list of procedures is equal to another such list built the same way,
    // which is what lets the evaluator pass procedures around inside lists.
    let one = Value::list_from_iter([Value::proc(successor()), Value::proc(variadic())]);
    let other = Value::list_from_iter([Value::proc(successor()), Value::proc(variadic())]);
    assert_eq!(one, other);
    assert_eq!(hash_of(&one), hash_of(&other));
    let reordered = Value::list_from_iter([Value::proc(variadic()), Value::proc(successor())]);
    assert_ne!(one, reordered, "list order matters, as everywhere else");
}

#[test]
fn robustness_composite_samples_are_reflexive_and_hash_stable() {
    let samples = composite_samples();
    for value in &samples {
        assert_eq!(value, value, "every value equals itself");
        assert_eq!(hash_of(value), hash_of(&value.clone()));
    }
    let mut set: HashSet<Value> = HashSet::new();
    for value in &samples {
        set.insert(value.clone());
    }
    assert_eq!(
        set.len(),
        samples.len(),
        "distinct samples occupy distinct buckets"
    );
    // Re-inserting a clone adds nothing: hashing agrees with equality over
    // captured syntax, so caches keyed on values behave.
    for value in &samples {
        assert!(
            !set.insert(value.clone()),
            "a clone must land in the bucket its original already holds"
        );
    }
    assert_eq!(set.len(), samples.len());
}

#[test]
fn robustness_composite_values_collapse_in_hash_structures() {
    let mut map = HashMap::new();
    for value in composite_samples() {
        map.insert(value, ());
    }
    // Equal values collapse, and independently built equal ones land in the
    // same bucket: equality and hashing agree over captured syntax too.
    assert!(map.contains_key(&Value::proc(successor())));
    assert!(map.contains_key(&Value::function(Fun::new(first()))));
    assert!(!map.contains_key(&Value::function(Fun::new(third()))));
    assert!(map.contains_key(&Value::proc(successor())));
}

#[test]
fn robustness_debug_and_clone_are_total_over_composite_shapes() {
    for value in composite_samples() {
        let text = format!("{value:?}");
        assert!(!text.is_empty());
        let _ = value.clone();
    }
}

#[test]
fn robustness_default_is_still_the_void_atom_with_the_new_variants_present() {
    // Adding variants must not move the default: `Void` remains "no result"
    // (ADR-10), and is neither a procedure nor a function.
    let value = Value::default();
    assert_eq!(value, Value::atom(Atom::Void));
    assert!(value.is_void());
    assert!(!value.is_proc());
    assert!(!value.is_function());
}
