//! Module tests for `quality` (src/quality.rs): the 8-state quality certificate.
//!
//! Layer: module tests (AGENTS.md §4) — external view of the public API only.

use rhytido_core::Quality;

const ALL: [Quality; 8] = [
    Quality::NotFaithful,
    Quality::Faithful,
    Quality::BelowCutoff,
    Quality::NonReal,
    Quality::Exact,
    Quality::FaithfulInexact,
    Quality::CorrectlyRounded,
    Quality::CorrectlyRoundedInexact,
];

// ---- Normal cases -----------------------------------------------------

#[test]
fn normal_codes_are_stable() {
    assert_eq!(Quality::NotFaithful.code(), 0);
    assert_eq!(Quality::Faithful.code(), 1);
    assert_eq!(Quality::BelowCutoff.code(), 2);
    assert_eq!(Quality::NonReal.code(), 3);
    assert_eq!(Quality::Exact.code(), 4);
    assert_eq!(Quality::FaithfulInexact.code(), 5);
    assert_eq!(Quality::CorrectlyRounded.code(), 6);
    assert_eq!(Quality::CorrectlyRoundedInexact.code(), 7);
}

#[test]
fn normal_code_roundtrip() {
    for q in ALL {
        assert_eq!(Quality::from_code(q.code()), Some(q));
    }
}

#[test]
fn normal_faithful_ladder() {
    // Faithful-or-better: 1,4,5,6,7.
    for q in ALL {
        let expected = matches!(q.code(), 1 | 4 | 5 | 6 | 7);
        assert_eq!(q.is_faithful(), expected, "code {}", q.code());
    }
}

#[test]
fn normal_correctly_rounded_ladder() {
    // Correctly-rounded-or-better: 4,6,7.
    for q in ALL {
        let expected = matches!(q.code(), 4 | 6 | 7);
        assert_eq!(q.is_correctly_rounded(), expected, "code {}", q.code());
    }
}

#[test]
fn normal_exactness_knowledge() {
    assert!(Quality::Exact.is_exact());
    for q in ALL {
        if q != Quality::Exact {
            assert!(!q.is_exact());
        }
    }
    assert!(Quality::FaithfulInexact.is_known_inexact());
    assert!(Quality::CorrectlyRoundedInexact.is_known_inexact());
    assert!(!Quality::Faithful.is_known_inexact());
    assert!(!Quality::CorrectlyRounded.is_known_inexact());
    assert!(!Quality::Exact.is_known_inexact());
}

// ---- Extreme cases ----------------------------------------------------

#[test]
fn extreme_special_states() {
    assert!(Quality::NonReal.is_non_real());
    assert!(Quality::BelowCutoff.is_below_cutoff());
    // Special states are not faithful and not correctly rounded.
    assert!(!Quality::NonReal.is_faithful());
    assert!(!Quality::BelowCutoff.is_faithful());
    assert!(!Quality::NonReal.is_correctly_rounded());
    assert!(!Quality::BelowCutoff.is_correctly_rounded());
    assert!(!Quality::NotFaithful.is_faithful());
}

#[test]
fn extreme_all_eight_distinct() {
    for (i, a) in ALL.iter().enumerate() {
        for b in ALL.iter().skip(i + 1) {
            assert_ne!(a, b);
            assert_ne!(a.code(), b.code());
        }
    }
}

// ---- Error cases (异常报错) -------------------------------------------

#[test]
fn error_from_code_out_of_range() {
    assert_eq!(Quality::from_code(8), None);
    assert_eq!(Quality::from_code(255), None);
}

// ---- Interruption / robustness cases (异常中断) -----------------------

#[test]
fn robustness_predicates_total_over_all_states() {
    // Every predicate is defined (no panic) for every state.
    for q in ALL {
        let _ = q.is_faithful();
        let _ = q.is_correctly_rounded();
        let _ = q.is_exact();
        let _ = q.is_known_inexact();
        let _ = q.is_non_real();
        let _ = q.is_below_cutoff();
        let _ = q.code();
    }
}

#[test]
fn robustness_implications_hold() {
    // exact => correctly rounded => faithful, for every state.
    for q in ALL {
        if q.is_exact() {
            assert!(q.is_correctly_rounded());
        }
        if q.is_correctly_rounded() {
            assert!(q.is_faithful());
        }
    }
}
