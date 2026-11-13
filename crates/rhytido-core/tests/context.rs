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

// ---- Taylor recursion setting (M2-D, additive) ------------------------

#[test]
fn normal_taylor_recursions_default_and_setter() {
    assert_eq!(Context::new().taylor_recursions(), 0);
    assert_eq!(Context::default().taylor_recursions(), 0);
    let ctx = Context::new().with_taylor_recursions(3);
    assert_eq!(ctx.taylor_recursions(), 3);
    // Setting it does not disturb the other settings.
    assert_eq!(ctx.precision(), DEFAULT_PRECISION);
    assert_eq!(ctx.rounding(), Rounding::Nearest);
}

#[test]
fn normal_taylor_builder_chains_with_other_settings() {
    let ctx = Context::new()
        .with_precision(100)
        .with_rounding(Rounding::Up)
        .with_taylor_recursions(2);
    assert_eq!(ctx.precision(), 100);
    assert_eq!(ctx.rounding(), Rounding::Up);
    assert_eq!(ctx.taylor_recursions(), 2);
    // Builder order does not matter.
    let other = Context::new()
        .with_taylor_recursions(2)
        .with_rounding(Rounding::Up)
        .with_precision(100);
    assert_eq!(ctx, other);
}

#[test]
fn extreme_taylor_recursions_bounds() {
    assert_eq!(
        Context::new().with_taylor_recursions(0).taylor_recursions(),
        0
    );
    assert_eq!(
        Context::new()
            .with_taylor_recursions(u32::MAX)
            .taylor_recursions(),
        u32::MAX
    );
}

#[test]
fn error_taylor_setting_preserves_precision_clamp() {
    // The taylor setting is independent of precision validation: an
    // under-minimum precision is still clamped, and no u32 value panics.
    let ctx = Context::new().with_precision(0).with_taylor_recursions(7);
    assert_eq!(ctx.precision(), MIN_PRECISION);
    assert_eq!(ctx.taylor_recursions(), 7);
}

#[test]
fn robustness_taylor_copy_and_hash_consistent() {
    let ctx = Context::new().with_precision(64).with_taylor_recursions(5);
    let copied = ctx; // Copy
    assert_eq!(ctx, copied);
    assert_eq!(ctx.taylor_recursions(), copied.taylor_recursions());
    let hash = |c: &Context| {
        let mut h = DefaultHasher::new();
        c.hash(&mut h);
        h.finish()
    };
    assert_eq!(hash(&ctx), hash(&copied));
    // The new field participates in equality (a different k is a different ctx).
    let different = ctx.with_taylor_recursions(6);
    assert_ne!(ctx, different);
}

#[test]
fn robustness_taylor_repeated_setters_idempotent() {
    let ctx = Context::new()
        .with_taylor_recursions(4)
        .with_taylor_recursions(4);
    assert_eq!(ctx.taylor_recursions(), 4);
    // Last write wins.
    let ctx = ctx.with_taylor_recursions(9);
    assert_eq!(ctx.taylor_recursions(), 9);
}
