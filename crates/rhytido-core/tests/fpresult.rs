//! Module tests for `fpresult` (src/fpresult.rs): value + quality certificate.
//!
//! Layer: module tests (AGENTS.md §4) — external view of the public API only.

use rhytido_backend::BigFloat;
use rhytido_core::{FpResult, Quality};

// ---- Normal cases -----------------------------------------------------

#[test]
fn normal_new_and_accessors() {
    let v = BigFloat::from_i64(5);
    let r = FpResult::new(v.clone(), Quality::Faithful);
    assert_eq!(r.value(), &v);
    assert_eq!(r.quality(), Quality::Faithful);
    assert!(r.is_faithful());
    assert!(!r.is_exact());
}

#[test]
fn normal_exact_constructor() {
    let v = BigFloat::from_i64(2);
    let r = FpResult::exact(v.clone());
    assert_eq!(r.quality(), Quality::Exact);
    assert!(r.is_exact());
    assert!(r.is_correctly_rounded());
    assert!(r.is_faithful());
    assert_eq!(r.value(), &v);
}

#[test]
fn normal_into_value() {
    let v = BigFloat::from_i64(7);
    let r = FpResult::exact(v.clone());
    assert_eq!(r.into_value(), v);
}

#[test]
fn normal_clone_equality() {
    let r = FpResult::new(BigFloat::from_i64(3), Quality::CorrectlyRounded);
    assert_eq!(r.clone(), r);
}

// ---- Extreme cases ----------------------------------------------------

#[test]
fn extreme_nan_non_real() {
    let r = FpResult::new(BigFloat::NAN, Quality::NonReal);
    assert!(r.is_non_real());
    assert!(!r.is_faithful());
    assert!(r.value().is_nan());
}

#[test]
fn extreme_infinity_non_real() {
    let r = FpResult::new(BigFloat::infinity(), Quality::NonReal);
    assert!(r.is_non_real());
    assert!(r.value().is_infinite());
    let rn = FpResult::new(BigFloat::neg_infinity(), Quality::NonReal);
    assert!(rn.is_non_real());
}

#[test]
fn extreme_all_qualities_constructible() {
    let v = BigFloat::from_i64(1);
    for code in 0..=7u8 {
        let q = Quality::from_code(code).unwrap();
        let r = FpResult::new(v.clone(), q);
        assert_eq!(r.quality(), q);
    }
}

// ---- Error cases (异常报错) -------------------------------------------

#[test]
fn error_not_faithful_result() {
    let r = FpResult::new(BigFloat::from_i64(1), Quality::NotFaithful);
    assert!(!r.is_faithful());
    assert!(!r.is_correctly_rounded());
    assert!(!r.is_exact());
}

#[test]
fn error_below_cutoff_not_faithful() {
    let r = FpResult::new(BigFloat::zero(), Quality::BelowCutoff);
    assert!(!r.is_faithful());
    assert!(r.quality().is_below_cutoff());
}

// ---- Interruption / robustness cases (异常中断) -----------------------

#[test]
fn robustness_quality_delegation_consistent() {
    let v = BigFloat::from_i64(1);
    for code in 0..=7u8 {
        let q = Quality::from_code(code).unwrap();
        let r = FpResult::new(v.clone(), q);
        assert_eq!(r.is_exact(), q.is_exact());
        assert_eq!(r.is_faithful(), q.is_faithful());
        assert_eq!(r.is_correctly_rounded(), q.is_correctly_rounded());
        assert_eq!(r.is_non_real(), q.is_non_real());
    }
}

#[test]
fn robustness_special_values_never_panic() {
    for v in [
        BigFloat::NAN,
        BigFloat::infinity(),
        BigFloat::neg_infinity(),
        BigFloat::zero(),
    ] {
        let r = FpResult::new(v, Quality::NonReal);
        let _ = r.is_exact();
        let _ = r.is_faithful();
        let _ = r.is_correctly_rounded();
        let _ = r.is_non_real();
        let _ = r.clone();
    }
}
