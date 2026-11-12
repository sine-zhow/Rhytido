//! Module tests for `message` (src/message.rs): the message catalog.
//!
//! Layer: module tests (AGENTS.md §4) — external view of the public API only.

use rhytido_core::{
    MessageCatalog,
    message::{MSG_PRECISION_INCREASED, MSG_ROUNDING_OCCURRED, MSG_UNDEFINED_OR_UNSTABLE},
};

// ---- Normal cases -----------------------------------------------------

#[test]
fn normal_emit_at_default_verbosity() {
    let cat = MessageCatalog::new();
    assert!(cat.should_emit(MSG_ROUNDING_OCCURRED.code, MSG_ROUNDING_OCCURRED.level));
    assert_eq!(
        cat.render(&MSG_ROUNDING_OCCURRED).unwrap(),
        "Warning: rounding has happened."
    );
}

#[test]
fn normal_verbosity_gating() {
    let mut cat = MessageCatalog::new();
    // Level-4 message not emitted at verbosity 1.
    assert!(!cat.should_emit(MSG_PRECISION_INCREASED.code, MSG_PRECISION_INCREASED.level));
    cat.set_verbosity(4);
    assert!(cat.should_emit(MSG_PRECISION_INCREASED.code, MSG_PRECISION_INCREASED.level));
    assert_eq!(cat.verbosity(), 4);
}

#[test]
fn normal_show_numbers() {
    let mut cat = MessageCatalog::new();
    cat.set_show_numbers(true);
    assert_eq!(
        cat.render(&MSG_ROUNDING_OCCURRED).unwrap(),
        "[1] Warning: rounding has happened."
    );
}

// ---- Extreme cases ----------------------------------------------------

#[test]
fn extreme_zero_verbosity_emits_nothing() {
    let mut cat = MessageCatalog::new();
    cat.set_verbosity(0);
    assert!(!cat.should_emit(MSG_ROUNDING_OCCURRED.code, MSG_ROUNDING_OCCURRED.level));
    assert!(cat.render(&MSG_ROUNDING_OCCURRED).is_none());
}

#[test]
fn extreme_high_verbosity_emits_all_levels() {
    let mut cat = MessageCatalog::new();
    cat.set_verbosity(100);
    assert!(cat.should_emit(MSG_PRECISION_INCREASED.code, MSG_PRECISION_INCREASED.level));
    assert!(cat.should_emit(MSG_ROUNDING_OCCURRED.code, MSG_ROUNDING_OCCURRED.level));
}

// ---- Error cases (异常报错) -------------------------------------------

#[test]
fn error_suppression_blocks_emission() {
    let mut cat = MessageCatalog::new();
    cat.suppress(MSG_ROUNDING_OCCURRED.code);
    assert!(cat.is_suppressed(MSG_ROUNDING_OCCURRED.code));
    assert!(!cat.should_emit(MSG_ROUNDING_OCCURRED.code, MSG_ROUNDING_OCCURRED.level));
    assert!(cat.render(&MSG_ROUNDING_OCCURRED).is_none());
    // Other messages unaffected.
    assert!(cat.should_emit(
        MSG_UNDEFINED_OR_UNSTABLE.code,
        MSG_UNDEFINED_OR_UNSTABLE.level
    ));
}

#[test]
fn error_unsuppress_restores() {
    let mut cat = MessageCatalog::new();
    cat.suppress(MSG_ROUNDING_OCCURRED.code);
    cat.unsuppress(MSG_ROUNDING_OCCURRED.code);
    assert!(!cat.is_suppressed(MSG_ROUNDING_OCCURRED.code));
    assert!(cat.render(&MSG_ROUNDING_OCCURRED).is_some());
}

// ---- Interruption / robustness cases (异常中断) -----------------------

#[test]
fn robustness_suppress_unsuppress_idempotent() {
    let mut cat = MessageCatalog::new();
    for _ in 0..10 {
        cat.suppress(MSG_ROUNDING_OCCURRED.code);
    }
    assert!(cat.is_suppressed(MSG_ROUNDING_OCCURRED.code));
    for _ in 0..10 {
        cat.unsuppress(MSG_ROUNDING_OCCURRED.code);
    }
    assert!(!cat.is_suppressed(MSG_ROUNDING_OCCURRED.code));
}

#[test]
fn robustness_unknown_code_not_suppressed() {
    let cat = MessageCatalog::new();
    assert!(!cat.is_suppressed(999_999));
    // should_emit for an unknown code still follows verbosity rules.
    assert!(cat.should_emit(999_999, 1));
}
