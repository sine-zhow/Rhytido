// SPDX-License-Identifier: Apache-2.0
//! Module-test layer (AGENTS.md §4): public API of `src/format.rs`.
//!
//! External-crate view of `Format` and `BigFloat::round_to_format`, migrated
//! from the former inline test module with names and assertions preserved.
//! Four categories: normal / extreme / error / robustness.

use rhytido_backend::{BigFloat, Format, Rounding};

// ---- Normal cases -----------------------------------------------------

#[test]
fn normal_format_precisions() {
    assert_eq!(Format::Double.precision(), 53);
    assert_eq!(Format::Single.precision(), 24);
    assert_eq!(Format::Half.precision(), 11);
    assert_eq!(Format::Quad.precision(), 113);
    assert_eq!(Format::DoubleExtended.precision(), 64);
}

#[test]
fn normal_round_to_double() {
    let third = BigFloat::one() / BigFloat::from_i64(3);
    let d = third.round_to_format(Format::Double, Rounding::Nearest);
    assert!(d.precision() <= 53);
    // Close to the original at double precision.
    assert!(d.with_precision(50) == third.with_precision(50));
}

#[test]
fn normal_round_to_single_and_half() {
    let x = BigFloat::from_f64(1.23456789);
    let s = x.round_to_format(Format::Single, Rounding::Nearest);
    assert!(s.precision() <= 24);
    let h = x.round_to_format(Format::Half, Rounding::Nearest);
    assert!(h.precision() <= 11);
}

#[test]
fn normal_exact_values_unchanged() {
    let two = BigFloat::from_i64(2);
    for fmt in [
        Format::Double,
        Format::Single,
        Format::Half,
        Format::Quad,
        Format::DoubleExtended,
    ] {
        assert_eq!(two.round_to_format(fmt, Rounding::Nearest), two);
    }
}

#[test]
fn normal_rounding_modes_differ() {
    let third = BigFloat::one() / BigFloat::from_i64(3);
    let up = third.round_to_format(Format::Half, Rounding::Up);
    let down = third.round_to_format(Format::Half, Rounding::Down);
    assert!(up >= down);
    assert!(up >= third.with_precision(11));
    assert!(down <= third.with_precision(11));
}

// ---- Extreme cases ----------------------------------------------------

#[test]
fn extreme_high_precision_to_single() {
    let x = (BigFloat::one() / BigFloat::from_i64(7)).with_precision(300);
    let s = x.round_to_format(Format::Single, Rounding::Nearest);
    assert!(s.precision() <= 24);
}

#[test]
fn extreme_quad_preserves_more() {
    let x = (BigFloat::one() / BigFloat::from_i64(7)).with_precision(200);
    let q = x.round_to_format(Format::Quad, Rounding::Nearest);
    let d = x.round_to_format(Format::Double, Rounding::Nearest);
    assert!(q.precision() <= 113);
    // Quad is at least as close as double.
    assert!(q.precision() >= d.precision() || q == d);
}

// ---- Error cases (异常报错) -------------------------------------------

#[test]
fn error_special_values_preserved() {
    assert!(
        BigFloat::NAN
            .round_to_format(Format::Double, Rounding::Nearest)
            .is_nan()
    );
    let inf = BigFloat::infinity();
    assert!(
        inf.round_to_format(Format::Single, Rounding::Nearest)
            .is_infinite()
    );
    let ninf = BigFloat::neg_infinity();
    assert!(
        ninf.round_to_format(Format::Half, Rounding::Nearest)
            .is_sign_negative()
    );
}

// ---- Interruption / robustness cases (异常中断) -----------------------

#[test]
fn robustness_all_formats_special_values_never_panic() {
    let vals = [
        BigFloat::NAN,
        BigFloat::infinity(),
        BigFloat::neg_infinity(),
        BigFloat::zero(),
        BigFloat::one(),
    ];
    let fmts = [
        Format::Double,
        Format::Single,
        Format::Half,
        Format::Quad,
        Format::DoubleExtended,
    ];
    let modes = [
        Rounding::Nearest,
        Rounding::Zero,
        Rounding::Up,
        Rounding::Down,
        Rounding::Away,
        Rounding::HalfAway,
    ];
    for v in &vals {
        for f in fmts {
            for m in modes {
                let _ = v.round_to_format(f, m);
            }
        }
    }
}

#[test]
fn robustness_zero_rounds_to_zero() {
    for fmt in [Format::Double, Format::Single, Format::Half] {
        assert!(
            BigFloat::zero()
                .round_to_format(fmt, Rounding::Nearest)
                .is_zero()
        );
    }
}
