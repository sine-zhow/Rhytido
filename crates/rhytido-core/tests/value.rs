// SPDX-License-Identifier: Apache-2.0
//! Module tests for `value` (src/value.rs): the atomic value type (ADR-10).
//!
//! Layer: module tests (AGENTS.md §4) — external view of the public API only.
//! Construction is total by contract, so the "error" category asserts the
//! *storage* semantics of ill-formed inputs (degenerate ranges, empty messages)
//! rather than failure returns.

use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use rhytido_backend::{BigFloat, Integer, Rational};
use rhytido_core::{Atom, Bound, LangError, Range};

// ---- Helpers ---------------------------------------------------------------

fn hash_of(atom: &Atom) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    atom.hash(&mut h);
    h.finish()
}

fn bound_hash_of(bound: &Bound) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bound.hash(&mut h);
    h.finish()
}

fn range_of(low: Bound, high: Bound) -> Atom {
    Atom::range(Range::new(low, high))
}

fn third() -> BigFloat {
    BigFloat::one() / BigFloat::from_i64(3)
}

/// One representative per variant plus the special values ADR-10 calls out.
///
/// `Atom::Void` is deliberately present twice (`void()` and the variant):
/// `normal_void_constructor_and_predicates` pins that they are the same value,
/// and `robustness_equal_atoms_collapse_in_a_hashset` needs a duplicate key to
/// show that equal atoms share one slot.
fn samples() -> Vec<Atom> {
    let long = "x".repeat(4096);
    vec![
        Atom::number(BigFloat::one()),
        Atom::number(BigFloat::zero()),
        Atom::number(-BigFloat::zero()),
        Atom::number(BigFloat::NAN),
        Atom::number(BigFloat::infinity()),
        Atom::number(BigFloat::neg_infinity()),
        Atom::number(third()),
        Atom::number(BigFloat::pi_with_precision(1000)),
        Atom::integer(Integer::from(0)),
        Atom::integer(Integer::from(2).pow(200) + Integer::from(1)),
        Atom::rational(Rational::from_i64(5)),
        Atom::rational(Rational::new(1, 2).expect("nonzero denominator")),
        Atom::boolean(true),
        Atom::boolean(false),
        Atom::string(""),
        Atom::string("abc"),
        Atom::string(long),
        Atom::string("a\0b\u{1f600}"),
        range_of(
            Bound::closed(BigFloat::one()),
            Bound::closed(BigFloat::from_i64(2)),
        ),
        range_of(
            Bound::closed(BigFloat::from_i64(3)),
            Bound::closed(BigFloat::one()),
        ),
        range_of(Bound::neg_infinity(), Bound::pos_infinity()),
        range_of(Bound::open(BigFloat::NAN), Bound::open(BigFloat::NAN)),
        range_of(
            Bound::closed(BigFloat::neg_infinity()),
            Bound::open(BigFloat::infinity()),
        ),
        Atom::error("boom", Some(7)),
        Atom::error("", None),
        Atom::void(),
        Atom::Void,
    ]
}

// ---- Normal cases ----------------------------------------------------------

#[test]
fn normal_number_accessor_returns_stored_float() {
    let a = Atom::number(BigFloat::from_i64(11));
    assert_eq!(a.as_number(), Some(&BigFloat::from_i64(11)));
    assert!(matches!(a, Atom::Number(_)));
}

#[test]
fn normal_integer_accessor_and_equality() {
    let a = Atom::integer(Integer::from(-7));
    assert_eq!(a.as_integer(), Some(&Integer::from(-7)));
    assert_eq!(a, Atom::integer(Integer::from(-7)));
}

#[test]
fn normal_rational_accessor_and_equality() {
    let half = Rational::new(1, 2).expect("nonzero denominator");
    let a = Atom::rational(half.clone());
    assert_eq!(a.as_rational(), Some(&half));
    assert_eq!(a, Atom::rational(half));
}

#[test]
fn normal_bool_accessor() {
    assert_eq!(Atom::boolean(true).as_bool(), Some(true));
    assert_eq!(Atom::boolean(false).as_bool(), Some(false));
    assert_ne!(Atom::boolean(true), Atom::boolean(false));
}

#[test]
fn normal_string_compares_by_content_not_identity() {
    let owned: Rc<str> = Rc::from("shared");
    let a = Atom::string(owned.clone());
    let b = Atom::string("shared");
    assert_eq!(a, b);
    assert_eq!(hash_of(&a), hash_of(&b));
    assert_eq!(a.as_string(), Some("shared"));
    assert_eq!(b.as_string(), Some("shared"));
    // Equality is by content; an already-shared string is additionally aliased
    // rather than copied, while a borrowed one gets its own allocation.
    let Atom::String(ra) = &a else {
        panic!("expected a string atom, got {a:?}");
    };
    let Atom::String(rb) = &b else {
        panic!("expected a string atom, got {b:?}");
    };
    assert!(Rc::ptr_eq(ra, &owned));
    assert!(!Rc::ptr_eq(ra, rb));
}

#[test]
fn normal_range_accessors_expose_both_bounds() {
    let r = Range::new(Bound::open(BigFloat::zero()), Bound::closed(third()));
    assert_eq!(r.lower(), &Bound::open(BigFloat::zero()));
    assert_eq!(r.upper(), &Bound::closed(third()));
    assert!(r.lower().is_open());
    assert!(r.upper().is_closed());
}

#[test]
fn normal_bound_value_and_predicates() {
    assert_eq!(
        Bound::closed(BigFloat::one()).value(),
        Some(&BigFloat::one())
    );
    assert_eq!(Bound::open(BigFloat::one()).value(), Some(&BigFloat::one()));
    assert_eq!(Bound::neg_infinity().value(), None);
    assert_eq!(Bound::pos_infinity().value(), None);
    assert!(Bound::neg_infinity().is_infinite() && Bound::pos_infinity().is_infinite());
    assert!(!Bound::closed(BigFloat::one()).is_infinite());
}

#[test]
fn normal_error_accessors_and_carrier() {
    let e = LangError::new("bad thing", Some(42));
    assert_eq!(e.message(), "bad thing");
    assert_eq!(e.code(), Some(42));
    let a = Atom::Error(e.clone());
    assert_eq!(a.as_error(), Some(&e));
    assert!(a.is_error());
}

#[test]
fn normal_error_constructor_matches_carrier() {
    assert_eq!(
        Atom::error("boom", Some(1)),
        Atom::Error(LangError::new("boom", Some(1)))
    );
    assert_eq!(
        Atom::error("boom", None).as_error().map(LangError::code),
        Some(None)
    );
}

#[test]
fn normal_void_constructor_and_predicates() {
    assert_eq!(Atom::void(), Atom::Void);
    assert!(Atom::void().is_void());
    assert!(!Atom::void().is_error());
    assert_eq!(Atom::default(), Atom::Void);
}

#[test]
fn normal_default_is_shared_by_all_uses() {
    let a: Atom = Atom::default();
    let b = Atom::void();
    assert_eq!(a, b);
    assert_eq!(hash_of(&a), hash_of(&b));
}

#[test]
fn normal_constructors_reach_every_variant() {
    let built = [
        Atom::number(BigFloat::one()),
        Atom::integer(Integer::from(1)),
        Atom::rational(Rational::from_i64(1)),
        Atom::boolean(true),
        Atom::string("s"),
        range_of(Bound::neg_infinity(), Bound::pos_infinity()),
        Atom::error("m", None),
        Atom::void(),
    ];
    let patterns: Vec<fn(&Atom) -> bool> = vec![
        |a| matches!(a, Atom::Number(_)),
        |a| matches!(a, Atom::Integer(_)),
        |a| matches!(a, Atom::Rational(_)),
        |a| matches!(a, Atom::Bool(_)),
        |a| matches!(a, Atom::String(_)),
        |a| matches!(a, Atom::Range(_)),
        |a| matches!(a, Atom::Error(_)),
        |a| matches!(a, Atom::Void),
    ];
    for (atom, test) in built.iter().zip(patterns) {
        assert!(test(atom), "wrong variant for {atom:?}");
    }
}

#[test]
fn normal_equality_implies_equal_hash_across_samples() {
    // Every pair that compares equal must hash identically, and the converse
    // must not be assumed.
    let s = samples();
    for a in &s {
        for b in &s {
            if a == b {
                assert_eq!(hash_of(a), hash_of(b), "{a:?} vs {b:?}");
            }
        }
    }
}

#[test]
fn normal_distinct_variants_are_never_equal() {
    let s = samples();
    for (i, a) in s.iter().enumerate() {
        for b in s.iter().skip(i + 1) {
            let variant_switch = std::mem::discriminant(a) != std::mem::discriminant(b);
            if variant_switch {
                assert_ne!(a, b, "{a:?} vs {b:?}");
            }
        }
    }
}

#[test]
fn normal_zero_valued_variants_do_not_collapse() {
    let zero_number = Atom::number(BigFloat::zero());
    let zero_integer = Atom::integer(Integer::from(0));
    let zero_rational = Atom::rational(Rational::from_i64(0));
    assert_ne!(zero_number, zero_integer);
    assert_ne!(zero_number, zero_rational);
    assert_ne!(zero_integer, zero_rational);
}

#[test]
fn normal_clone_preserves_equality_and_hash() {
    for a in samples() {
        let b = a.clone();
        assert_eq!(a, b);
        assert_eq!(hash_of(&a), hash_of(&b));
    }
}

#[test]
fn normal_debug_names_every_variant() {
    let cases: [(Atom, &str); 8] = [
        (Atom::number(BigFloat::one()), "Number"),
        (Atom::integer(Integer::from(1)), "Integer"),
        (Atom::rational(Rational::from_i64(1)), "Rational"),
        (Atom::boolean(true), "Bool"),
        (Atom::string("q"), "String"),
        (
            range_of(Bound::neg_infinity(), Bound::pos_infinity()),
            "Range",
        ),
        (Atom::error("m", None), "Error"),
        (Atom::Void, "Void"),
    ];
    for (atom, tag) in cases {
        assert!(format!("{atom:?}").contains(tag), "{atom:?} lacks {tag}");
    }
}

#[test]
fn normal_number_precision_label_is_not_part_of_identity() {
    let narrow = BigFloat::from_i64(5);
    let wide = narrow.clone().with_precision(300);
    assert_eq!(Atom::number(narrow), Atom::number(wide));
}

#[test]
fn normal_rational_canonical_form_equals_unreduced() {
    let reduced = Rational::new(1, 2).expect("nonzero denominator");
    let unreduced = Rational::new(2, 4).expect("nonzero denominator");
    assert_eq!(
        Atom::rational(reduced.clone()),
        Atom::rational(unreduced.clone())
    );
    assert_eq!(
        hash_of(&Atom::rational(reduced)),
        hash_of(&Atom::rational(unreduced))
    );
}

#[test]
fn normal_string_accepts_shared_and_owned_input() {
    let shared: Rc<str> = Rc::from("same");
    assert_eq!(
        Atom::string(shared.clone()),
        Atom::string(String::from("same"))
    );
    assert_eq!(Atom::string(shared), Atom::string("same"));
}

#[test]
fn normal_range_point_and_punctured_point_differ() {
    let point = range_of(
        Bound::closed(BigFloat::zero()),
        Bound::closed(BigFloat::zero()),
    );
    let punctured = range_of(Bound::open(BigFloat::zero()), Bound::open(BigFloat::zero()));
    assert_ne!(point, punctured);
    assert_ne!(hash_of(&point), hash_of(&punctured));
}

#[test]
fn normal_same_value_different_openness_is_different() {
    assert_ne!(Bound::closed(BigFloat::one()), Bound::open(BigFloat::one()));
    assert_ne!(
        bound_hash_of(&Bound::closed(BigFloat::one())),
        bound_hash_of(&Bound::open(BigFloat::one()))
    );
}

#[test]
fn normal_atom_range_equality_follows_bounds() {
    let a = range_of(Bound::closed(third()), Bound::open(BigFloat::one()));
    let b = range_of(Bound::closed(third()), Bound::open(BigFloat::one()));
    assert_eq!(a, b);
    assert_eq!(hash_of(&a), hash_of(&b));
}

// ---- Extreme cases ---------------------------------------------------------

#[test]
fn extreme_nan_numbers_are_equal_and_hash_alike() {
    let a = Atom::number(BigFloat::NAN);
    let b = Atom::number(BigFloat::NAN);
    assert_eq!(a, b, "NaN is structurally equal to NaN (ADR-10)");
    assert_eq!(hash_of(&a), hash_of(&b));
    assert_ne!(a, Atom::number(BigFloat::zero()));
}

#[test]
fn extreme_signed_zero_is_distinguished() {
    let pos = Atom::number(BigFloat::zero());
    let neg = Atom::number(-BigFloat::zero());
    assert_ne!(pos, neg);
    assert_ne!(hash_of(&pos), hash_of(&neg));
    assert_eq!(pos, Atom::number(BigFloat::zero()));
}

#[test]
fn extreme_infinities_are_distinguished_from_each_other_and_nan() {
    let inf = Atom::number(BigFloat::infinity());
    let ninf = Atom::number(BigFloat::neg_infinity());
    let nan = Atom::number(BigFloat::NAN);
    assert_ne!(inf, ninf);
    assert_ne!(inf, nan);
    assert_ne!(ninf, nan);
    assert_eq!(inf, Atom::number(BigFloat::infinity()));
    assert_eq!(hash_of(&inf), hash_of(&Atom::number(BigFloat::infinity())));
}

#[test]
fn extreme_nan_endpoints_equal_in_closed_and_open_form() {
    let closed = range_of(Bound::closed(BigFloat::NAN), Bound::closed(BigFloat::one()));
    let same = range_of(Bound::closed(BigFloat::NAN), Bound::closed(BigFloat::one()));
    assert_eq!(closed, same);
    assert_eq!(hash_of(&closed), hash_of(&same));
    assert_ne!(
        closed,
        range_of(Bound::open(BigFloat::NAN), Bound::closed(BigFloat::one()))
    );
}

#[test]
fn extreme_bound_combination_matrix_is_preserved() {
    let kinds = [
        Bound::closed(BigFloat::one()),
        Bound::open(BigFloat::one()),
        Bound::neg_infinity(),
        Bound::pos_infinity(),
    ];
    let mut ranges = Vec::new();
    for low in kinds.iter() {
        for high in kinds.iter() {
            let r = Range::new(low.clone(), high.clone());
            assert_eq!(r.lower(), low);
            assert_eq!(r.upper(), high);
            assert_eq!(
                r,
                Range::new(low.clone(), high.clone()),
                "combination {low:?}..{high:?} not stable"
            );
            ranges.push(r);
        }
    }
    assert_eq!(ranges.len(), 16);
    // All 16 combinations are distinct values.
    for (i, a) in ranges.iter().enumerate() {
        for b in ranges.iter().skip(i + 1) {
            assert_ne!(a, b);
        }
    }
}

#[test]
fn extreme_every_infinity_pair_survives_atom_roundtrip() {
    let lows = [Bound::neg_infinity(), Bound::pos_infinity()];
    let highs = [Bound::neg_infinity(), Bound::pos_infinity()];
    for low in &lows {
        for high in &highs {
            let a = range_of(low.clone(), high.clone());
            let got = a.as_range().unwrap_or_else(|| panic!("not a range: {a:?}"));
            assert_eq!(got.lower(), low);
            assert_eq!(got.upper(), high);
        }
    }
}

#[test]
fn extreme_empty_string_equals_only_empty_string() {
    let empty = Atom::string("");
    assert_eq!(empty, Atom::string(""));
    assert_eq!(empty.as_string(), Some(""));
    assert_ne!(empty, Atom::string(" "));
    assert_ne!(empty, Atom::Void);
    assert_eq!(hash_of(&empty), hash_of(&Atom::string("")));
}

#[test]
fn extreme_long_string_equality_and_hash() {
    let big = "y".repeat(100_000);
    let a = Atom::string(big.clone());
    let b = Atom::string(big.clone());
    assert_eq!(a, b);
    assert_eq!(hash_of(&a), hash_of(&b));
    assert_ne!(a, Atom::string("y".repeat(99_999)));
    assert_eq!(a.as_string().map(str::len), Some(100_000));
}

#[test]
fn extreme_degenerate_reversed_range_is_stored_verbatim() {
    // lower > upper is legal to *represent*: the evaluator decides (M3-C).
    let reversed = range_of(
        Bound::closed(BigFloat::from_i64(3)),
        Bound::closed(BigFloat::from_i64(1)),
    );
    let r = reversed.as_range().expect("range atom");
    assert_eq!(
        *r.lower().value().expect("closed bound"),
        BigFloat::from_i64(3)
    );
    assert_eq!(
        *r.upper().value().expect("closed bound"),
        BigFloat::from_i64(1)
    );
    assert_eq!(reversed, reversed.clone());
    assert_ne!(
        reversed,
        range_of(
            Bound::closed(BigFloat::from_i64(1)),
            Bound::closed(BigFloat::from_i64(3)),
        )
    );
}

#[test]
fn extreme_inverted_infinite_range_is_stored_verbatim() {
    let inverted = range_of(Bound::pos_infinity(), Bound::neg_infinity());
    let r = inverted.as_range().expect("range atom");
    assert!(matches!(r.lower(), Bound::PosInfinity));
    assert!(matches!(r.upper(), Bound::NegInfinity));
    assert_ne!(
        inverted,
        range_of(Bound::neg_infinity(), Bound::pos_infinity())
    );
}

#[test]
fn extreme_wide_precision_label_still_irrelevant() {
    let one = BigFloat::one();
    assert_eq!(
        Atom::number(one.clone()),
        Atom::number(one.with_precision(4000))
    );
}

#[test]
fn extreme_precision_change_that_changes_the_value_changes_identity() {
    let exact = third();
    let coarser = exact
        .clone()
        .round_to(20, rhytido_backend::Rounding::Nearest);
    assert_ne!(Atom::number(exact.clone()), Atom::number(coarser.clone()));
    assert_ne!(
        hash_of(&Atom::number(exact)),
        hash_of(&Atom::number(coarser))
    );
}

#[test]
fn extreme_tiny_magnitude_number_is_not_zero() {
    let tiny = BigFloat::from_i64(2).pow(&BigFloat::from_i64(-1000));
    let a = Atom::number(tiny.clone());
    assert_ne!(a, Atom::number(BigFloat::zero()));
    assert_eq!(a, Atom::number(tiny));
}

#[test]
fn extreme_huge_integer_keeps_exact_identity() {
    let wide = Integer::from(2).pow(4096) + Integer::from(1);
    let a = Atom::integer(wide.clone());
    assert_eq!(a.as_integer(), Some(&wide));
    assert_eq!(a, Atom::integer(wide.clone()));
    assert_ne!(a, Atom::integer(Integer::from(2).pow(4096)));
    assert_eq!(hash_of(&a), hash_of(&Atom::integer(wide)));
}

#[test]
fn extreme_huge_rational_hashes_stably() {
    let mut wide = Rational::new(1, 3).expect("nonzero denominator");
    for _ in 0..8 {
        wide = wide.clone() * wide.clone();
    }
    let a = Atom::rational(wide.clone());
    assert_eq!(hash_of(&a), hash_of(&a.clone()));
    assert_eq!(a.as_rational(), Some(&wide));
    assert_ne!(a, Atom::rational(Rational::new(1, 3).expect("valid")));
}

#[test]
fn extreme_nan_bound_is_distinct_from_infinite_bound() {
    let nan = Bound::closed(BigFloat::NAN);
    assert_ne!(nan, Bound::neg_infinity());
    assert_ne!(nan, Bound::pos_infinity());
    assert_eq!(nan, Bound::closed(BigFloat::NAN));
    assert_eq!(
        bound_hash_of(&nan),
        bound_hash_of(&Bound::closed(BigFloat::NAN))
    );
}

#[test]
fn extreme_error_message_at_both_length_extremes() {
    let empty = Atom::error("", Some(0));
    let huge = Atom::error("z".repeat(50_000), Some(u32::MAX));
    assert_eq!(empty.as_error().map(|e| e.message()), Some(""));
    assert_eq!(huge.as_error().and_then(|e| e.code()), Some(u32::MAX));
    assert_eq!(huge, huge.clone());
    assert_ne!(empty, huge);
}

#[test]
fn extreme_non_ascii_and_control_characters_are_preserved() {
    let odd = "\u{0}mid\u{7}\u{1f600}\u{e0001}";
    let a = Atom::string(odd);
    assert_eq!(a.as_string(), Some(odd));
    assert_eq!(a, Atom::string(odd));
    assert_ne!(a, Atom::string("mid"));
    let e = Atom::error(odd, None);
    assert_eq!(e.as_error().map(LangError::message), Some(odd));
}

#[test]
fn extreme_very_high_precision_number_keeps_its_identity() {
    // Identity is the value, not the precision label: widening a label never
    // changes the number, while a coarser rounding of a transcendental does.
    let pi = BigFloat::pi_with_precision(2000);
    let a = Atom::number(pi.clone());
    assert_eq!(a, Atom::number(pi.clone()));
    assert_eq!(hash_of(&a), hash_of(&Atom::number(pi.clone())));
    assert_eq!(a, Atom::number(pi.with_precision(4000)));
    assert_ne!(a, Atom::number(BigFloat::pi_with_precision(64)));
}

#[test]
fn extreme_bool_never_equals_number_zero_or_one() {
    assert_ne!(Atom::boolean(false), Atom::number(BigFloat::zero()));
    assert_ne!(Atom::boolean(true), Atom::number(BigFloat::one()));
}

// ---- Error-shaped inputs: total storage semantics ---------------------------

#[test]
fn error_ill_formed_range_is_a_value_not_a_failure() {
    // Nothing about `Atom::range` can fail, so an invalid interval must simply
    // be retained for the evaluator to judge.
    let nonsense = range_of(
        Bound::open(BigFloat::infinity()),
        Bound::closed(BigFloat::neg_infinity()),
    );
    assert!(nonsense.as_range().is_some());
    let r = nonsense.as_range().expect("range atom");
    assert!(matches!(r.lower(), Bound::Open(_)));
    assert!(matches!(r.upper(), Bound::Closed(_)));
    assert_eq!(r.lower().value().map(BigFloat::is_infinite), Some(true));
}

#[test]
fn error_bound_holding_infinity_is_not_promoted_to_infinite_kind() {
    let closed_inf = Bound::closed(BigFloat::infinity());
    assert!(
        !closed_inf.is_infinite(),
        "kind is what was written, not implied"
    );
    assert_ne!(closed_inf, Bound::pos_infinity());
    assert_eq!(closed_inf.value(), Some(&BigFloat::infinity()));
}

#[test]
fn error_range_with_both_endpoints_nan_is_retained() {
    let both_nan = range_of(Bound::open(BigFloat::NAN), Bound::closed(BigFloat::NAN));
    let r = both_nan.as_range().expect("range atom");
    assert_eq!(r.lower().value().map(BigFloat::is_nan), Some(true));
    assert_eq!(r.upper().value().map(BigFloat::is_nan), Some(true));
    assert_eq!(both_nan, both_nan.clone());
}

#[test]
fn error_error_atom_without_code_differs_from_code_zero() {
    assert_ne!(Atom::error("m", None), Atom::error("m", Some(0)));
    assert_eq!(
        Atom::error("m", None).as_error().and_then(LangError::code),
        None
    );
    assert_eq!(
        Atom::error("m", Some(0))
            .as_error()
            .and_then(LangError::code),
        Some(0)
    );
}

#[test]
fn error_error_equality_requires_message_and_code() {
    let base = Atom::error("same", Some(1));
    assert_ne!(base, Atom::error("other", Some(1)));
    assert_ne!(base, Atom::error("same", Some(2)));
    assert_eq!(base, Atom::error("same", Some(1)));
}

#[test]
fn error_error_is_distinct_from_every_other_variant() {
    let e = Atom::error("1", Some(1));
    for a in samples() {
        if !a.is_error() {
            assert_ne!(e, a, "error must not equal {a:?}");
        }
    }
}

#[test]
fn error_string_and_error_with_same_text_are_different() {
    assert_ne!(Atom::string("boom"), Atom::error("boom", None));
}

#[test]
fn error_void_differs_from_empty_string_and_false() {
    assert_ne!(Atom::Void, Atom::string(""));
    assert_ne!(Atom::Void, Atom::boolean(false));
    assert_ne!(Atom::string(""), Atom::boolean(false));
}

#[test]
fn error_accessors_reject_the_wrong_variant() {
    for a in samples() {
        let ok = match &a {
            Atom::Number(_) => a.as_number().is_some(),
            Atom::Integer(_) => a.as_integer().is_some(),
            Atom::Rational(_) => a.as_rational().is_some(),
            Atom::Bool(_) => a.as_bool().is_some(),
            Atom::String(_) => a.as_string().is_some(),
            Atom::Range(_) => a.as_range().is_some(),
            Atom::Error(_) => a.as_error().is_some(),
            Atom::Void => a.is_void() && !a.is_error(),
        };
        assert!(ok, "accessor disagrees with variant: {a:?}");
    }
}

#[test]
fn error_wrong_variant_accessors_are_all_none() {
    let number = Atom::number(BigFloat::one());
    assert_eq!(number.as_integer(), None);
    assert_eq!(number.as_rational(), None);
    assert_eq!(number.as_bool(), None);
    assert_eq!(number.as_string(), None);
    assert_eq!(number.as_range(), None);
    assert_eq!(number.as_error(), None);
    assert!(!number.is_void());
    assert!(!number.is_error());
}

// ---- Interruption / robustness cases --------------------------------------

#[test]
fn robustness_clone_hash_debug_never_panic_on_all_samples() {
    for a in samples() {
        let cloned = a.clone();
        let _ = hash_of(&cloned);
        let _ = format!("{cloned:?}");
        assert_eq!(a, cloned);
    }
}

#[test]
fn robustness_equality_is_reflexive_and_symmetric() {
    let s = samples();
    for a in &s {
        assert_eq!(a, a);
        for b in &s {
            assert_eq!(a == b, b == a, "{a:?} vs {b:?}");
        }
    }
}

#[test]
fn robustness_equality_is_transitive_across_samples() {
    let s = samples();
    for a in &s {
        for b in &s {
            if a != b {
                continue;
            }
            for c in &s {
                if b == c {
                    assert_eq!(a, c, "transitivity broken: {a:?} {b:?} {c:?}");
                }
            }
        }
    }
}

#[test]
fn robustness_hash_is_deterministic_across_repeats() {
    for a in samples() {
        let first = hash_of(&a);
        for _ in 0..8 {
            assert_eq!(first, hash_of(&a), "unstable hash for {a:?}");
        }
    }
}

#[test]
fn robustness_atoms_work_as_hashmap_keys() {
    // Keys must be usable, stable, and consistent between `get` and `contains`.
    let all = samples();
    let mut map: HashMap<Atom, String> = HashMap::new();
    for a in &all {
        map.insert(a.clone(), format!("{a:?}"));
    }
    for a in &all {
        let found = map.get(a).unwrap_or_else(|| panic!("key missing: {a:?}"));
        assert_eq!(found, &format!("{a:?}"));
        assert!(map.contains_key(a));
    }
    assert!(map.len() < all.len(), "equal atoms must share one slot");
}

#[test]
fn robustness_equal_atoms_collapse_in_a_hashset() {
    let s = samples();
    let set: HashSet<Atom> = s.iter().cloned().collect();
    assert!(set.len() < s.len(), "duplicate variants did not collapse");
    for a in &s {
        assert!(set.contains(a), "missing {a:?}");
    }
    // Zero, signed zero, NaN and the two infinities are five distinct values.
    let special: Vec<&Atom> = s
        .iter()
        .filter(|a| {
            a.as_number()
                .is_some_and(|v| v.is_nan() || v.is_zero() || v.is_infinite())
        })
        .collect();
    assert_eq!(special.len(), 5);
}

#[test]
fn robustness_range_rebuilt_from_accessors_is_equal() {
    for a in samples() {
        if let Some(r) = a.as_range() {
            let rebuilt = Range::new(r.lower().clone(), r.upper().clone());
            assert_eq!(&rebuilt, r);
            assert_eq!(Atom::range(rebuilt), a);
        }
    }
}

#[test]
fn robustness_bound_value_accessor_is_total() {
    let bounds = [
        Bound::closed(BigFloat::NAN),
        Bound::open(BigFloat::infinity()),
        Bound::closed(-BigFloat::zero()),
        Bound::neg_infinity(),
        Bound::pos_infinity(),
    ];
    for b in &bounds {
        let _ = b.value();
        let _ = b.is_closed();
        let _ = b.is_open();
        let _ = b.is_infinite();
        let _ = format!("{b:?}");
        assert_eq!(b, b);
        assert_eq!(bound_hash_of(b), bound_hash_of(&b.clone()));
    }
}

#[test]
fn robustness_lang_error_clone_and_hash_on_extremes() {
    let cases = [
        LangError::new("", None),
        LangError::new("", Some(u32::MAX)),
        LangError::new("m".repeat(10_000), Some(1)),
        LangError::new("\u{0}", Some(0)),
    ];
    for e in &cases {
        let c = e.clone();
        assert_eq!(&c, e);
        let mut h = std::collections::hash_map::DefaultHasher::new();
        c.hash(&mut h);
        let mut h2 = std::collections::hash_map::DefaultHasher::new();
        e.hash(&mut h2);
        assert_eq!(h.finish(), h2.finish());
        assert!(!format!("{c:?}").is_empty());
    }
}

#[test]
fn robustness_default_atom_is_usable_everywhere() {
    let a = Atom::default();
    assert!(a.is_void());
    assert_eq!(a, Atom::default());
    let mut set = HashSet::new();
    set.insert(a.clone());
    set.insert(Atom::void());
    assert_eq!(set.len(), 1);
    let _ = format!("{a:?}");
}

#[test]
fn robustness_string_aliasing_survives_clone_and_drop() {
    let shared: Rc<str> = Rc::from("alias");
    let a = Atom::string(Rc::clone(&shared));
    let b = a.clone();
    let extra = Atom::string("alias");
    assert_eq!(a, b);
    assert_eq!(a, extra);
    drop(shared);
    assert_eq!(b.as_string(), Some("alias"));
    assert_eq!(extra.as_string(), Some("alias"));
}
