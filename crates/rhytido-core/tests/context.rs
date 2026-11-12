//! Module tests for `context` (src/context.rs): threaded evaluation settings.
//!
//! Layer: module tests (AGENTS.md §4) — external view of the public API only.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use rhytido_backend::{DEFAULT_PRECISION, Rounding};
use rhytido_core::{Context, MIN_PRECISION};

// ---- Normal cases -----------------------------------------------------

#[test]
fn normal_defaults() {
    let ctx = Context::new();
    assert_eq!(ctx.precision(), DEFAULT_PRECISION);
    assert_eq!(ctx.rounding(), Rounding::Nearest);
}

#[test]
fn normal_with_precision_and_rounding() {
    let ctx = Context::new()
        .with_precision(100)
        .with_rounding(Rounding::Up);
    assert_eq!(ctx.precision(), 100);
    assert_eq!(ctx.rounding(), Rounding::Up);
}

#[test]
fn normal_builder_chaining_order_independent() {
    let a = Context::new()
        .with_precision(80)
        .with_rounding(Rounding::Down);
    let b = Context::new()
        .with_rounding(Rounding::Down)
        .with_precision(80);
    assert_eq!(a, b);
}

#[test]
fn normal_default_equals_new() {
    assert_eq!(Context::default(), Context::new());
}

// ---- Extreme cases ----------------------------------------------------

#[test]
fn extreme_precision_clamped_to_minimum() {
    assert_eq!(Context::new().with_precision(0).precision(), MIN_PRECISION);
    assert_eq!(Context::new().with_precision(1).precision(), MIN_PRECISION);
    assert_eq!(Context::new().with_precision(2).precision(), 2);
}

#[test]
fn extreme_large_precision_preserved() {
    let ctx = Context::new().with_precision(10_000);
    assert_eq!(ctx.precision(), 10_000);
}

#[test]
fn extreme_all_rounding_modes() {
    for r in [
        Rounding::Nearest,
        Rounding::Zero,
        Rounding::Up,
        Rounding::Down,
        Rounding::Away,
        Rounding::HalfAway,
    ] {
        assert_eq!(Context::new().with_rounding(r).rounding(), r);
    }
}

// ---- Error cases (异常报错) -------------------------------------------

#[test]
fn error_invalid_precision_does_not_panic() {
    // Under-minimum precision is clamped, not an error/panic.
    let ctx = Context::new().with_precision(0);
    assert!(ctx.precision() >= MIN_PRECISION);
}

// ---- Interruption / robustness cases (异常中断) -----------------------

#[test]
fn robustness_copy_and_hash_consistent() {
    let ctx = Context::new()
        .with_precision(64)
        .with_rounding(Rounding::Zero);
    let copied = ctx; // Copy
    assert_eq!(ctx, copied);

    let hash = |c: &Context| {
        let mut h = DefaultHasher::new();
        c.hash(&mut h);
        h.finish()
    };
    assert_eq!(hash(&ctx), hash(&copied));
}

#[test]
fn robustness_repeated_setters_idempotent() {
    let ctx = Context::new()
        .with_precision(53)
        .with_precision(53)
        .with_rounding(Rounding::Nearest)
        .with_rounding(Rounding::Nearest);
    assert_eq!(ctx.precision(), 53);
    assert_eq!(ctx.rounding(), Rounding::Nearest);
}
