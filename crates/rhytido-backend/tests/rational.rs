// SPDX-License-Identifier: Apache-2.0
//! Module-test layer (AGENTS.md §4): public API of `src/rational.rs`.
//!
//! External-crate view of `Rational`, migrated from the former inline test
//! module with names and assertions preserved. Four categories: normal /
//! extreme / error / robustness.

use rhytido_backend::{Error, Rational};

// ---- Normal cases -----------------------------------------------------

#[test]
fn normal_construct_and_reduce() {
    assert_eq!(Rational::new(1, 2).unwrap().to_string(), "1/2");
    assert_eq!(Rational::new(2, 4).unwrap().to_string(), "1/2");
    assert_eq!(Rational::new(6, 3).unwrap().to_string(), "2");
    assert_eq!(Rational::from_i64(5).to_string(), "5");
    assert_eq!(Rational::new(0, 5).unwrap().to_string(), "0");
}

#[test]
fn normal_sign_normalization() {
    assert_eq!(Rational::new(1, -2).unwrap().to_string(), "-1/2");
    assert_eq!(Rational::new(-1, -2).unwrap().to_string(), "1/2");
    assert_eq!(Rational::new(-1, 2).unwrap().to_string(), "-1/2");
    assert!(Rational::new(1, -2).unwrap().denominator().is_positive());
}

#[test]
fn normal_arithmetic() {
    let half = Rational::new(1, 2).unwrap();
    let third = Rational::new(1, 3).unwrap();
    assert_eq!((half.clone() + third.clone()).to_string(), "5/6");
    assert_eq!((half.clone() - third.clone()).to_string(), "1/6");
    assert_eq!((half.clone() * third.clone()).to_string(), "1/6");
    assert_eq!(half.checked_div(&third).unwrap().to_string(), "3/2");
    assert_eq!((-half).to_string(), "-1/2");
}

#[test]
fn normal_accessors_and_predicates() {
    let r = Rational::new(6, 4).unwrap(); // 3/2
    assert_eq!(r.numerator().to_string(), "3");
    assert_eq!(r.denominator().to_string(), "2");
    assert!(!r.is_integer());
    assert!(r.is_positive());
    assert!(!r.is_negative());
    assert!(!r.is_zero());
    assert!(Rational::from_i64(7).is_integer());
    assert!(Rational::new(0, 3).unwrap().is_zero());
    assert!(Rational::new(-3, 2).unwrap().is_negative());
}

#[test]
fn normal_parse_and_ordering() {
    assert_eq!("3/4".parse::<Rational>().unwrap().to_string(), "3/4");
    assert_eq!("5".parse::<Rational>().unwrap().to_string(), "5");
    assert_eq!("-2/3".parse::<Rational>().unwrap().to_string(), "-2/3");
    let a = Rational::new(1, 2).unwrap();
    let b = Rational::new(2, 3).unwrap();
    assert!(a < b);
    assert_eq!(a, Rational::new(2, 4).unwrap());
}

#[test]
fn normal_recip() {
    let r = Rational::new(2, 3).unwrap();
    assert_eq!(r.checked_recip().unwrap().to_string(), "3/2");
    assert_eq!(
        Rational::from_i64(4).checked_recip().unwrap().to_string(),
        "1/4"
    );
}

// ---- Extreme cases ----------------------------------------------------

#[test]
fn extreme_i64_boundaries() {
    let r = Rational::new(i64::MAX, 1).unwrap();
    assert_eq!(r.to_string(), "9223372036854775807");
    // i64::MIN / -1 = 2^63, which overflows i64 but not the big rational.
    let r2 = Rational::new(i64::MIN, -1).unwrap();
    assert_eq!(r2.to_string(), "9223372036854775808");
    let r3 = Rational::new(i64::MIN, i64::MAX).unwrap();
    assert!(r3.is_negative());
}

#[test]
fn extreme_large_reduction() {
    // Large common factor reduces exactly.
    let r = Rational::new(1_000_000_000, 2_000_000_000).unwrap();
    assert_eq!(r.to_string(), "1/2");
    let big = Rational::new(i64::MAX, 2).unwrap();
    let doubled = &big * &Rational::from_i64(2);
    assert_eq!(doubled.to_string(), "9223372036854775807");
}

#[test]
fn extreme_ref_ops_consistency() {
    let a = Rational::new(1, 2).unwrap();
    let b = Rational::new(1, 3).unwrap();
    assert_eq!(&a + &b, a.clone() + b.clone());
    assert_eq!(&a - &b, a.clone() - b.clone());
    assert_eq!(&a * &b, a.clone() * b.clone());
    assert_eq!(-&a, -a.clone());
}

// ---- Error cases (异常报错) -------------------------------------------

#[test]
fn error_zero_denominator() {
    assert_eq!(Rational::new(1, 0), Err(Error::DivisionByZero));
    assert_eq!(Rational::new(0, 0), Err(Error::DivisionByZero));
}

#[test]
fn error_div_by_zero_rational() {
    let a = Rational::new(1, 2).unwrap();
    let zero = Rational::new(0, 1).unwrap();
    assert!(a.checked_div(&zero).is_none());
    assert!(zero.checked_recip().is_none());
}

#[test]
fn error_invalid_parse() {
    assert!("".parse::<Rational>().is_err());
    assert!("abc".parse::<Rational>().is_err());
    assert!("1/0".parse::<Rational>().is_err());
    assert!("1/2/3".parse::<Rational>().is_err());
    assert!("1.5".parse::<Rational>().is_err());
}

// ---- Interruption / robustness cases (异常中断) -----------------------

#[test]
fn robustness_canonical_form_stable() {
    // Repeated operations stay canonical and never panic.
    let mut acc = Rational::from_i64(1);
    for k in 1..=20i64 {
        acc = &acc + &Rational::new(1, k).unwrap();
        assert!(acc.denominator().is_positive());
    }
    assert!(acc.is_positive());
}

#[test]
fn robustness_zero_and_sign_edges() {
    let zero = Rational::new(0, 1).unwrap();
    let one = Rational::from_i64(1);
    assert_eq!((&zero + &one).to_string(), "1");
    assert_eq!((&zero * &one).to_string(), "0");
    assert_eq!((-zero.clone()).to_string(), "0");
    assert!(zero.checked_div(&one).unwrap().is_zero());
}
