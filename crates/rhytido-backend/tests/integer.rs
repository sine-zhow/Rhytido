// SPDX-License-Identifier: Apache-2.0
//! Module-test layer (AGENTS.md §4): public API of `src/integer.rs`.
//!
//! External-crate view of `Integer`, migrated from the former inline test
//! module with names and assertions preserved. Four categories: normal /
//! extreme / error / robustness.

use rhytido_backend::{Error, Integer};

// ---- Normal cases -----------------------------------------------------

#[test]
fn normal_add_sub_mul() {
    let a = Integer::from(21);
    let b = Integer::from(2);
    assert_eq!((a.clone() + b.clone()).to_string(), "23");
    assert_eq!((a.clone() - b.clone()).to_string(), "19");
    assert_eq!((a.clone() * b.clone()).to_string(), "42");
    assert_eq!((-a).to_string(), "-21");
}

#[test]
fn normal_div_rem_truncating() {
    let a = Integer::from(21);
    let b = Integer::from(2);
    assert_eq!(a.checked_div(&b).unwrap().to_string(), "10");
    assert_eq!(a.checked_rem(&b).unwrap().to_string(), "1");
    let (q, r) = a.div_rem(&b).unwrap();
    assert_eq!(q.to_string(), "10");
    assert_eq!(r.to_string(), "1");
    // Truncating toward zero for negatives.
    let neg = Integer::from(-21);
    assert_eq!(neg.checked_div(&b).unwrap().to_string(), "-10");
    assert_eq!(neg.checked_rem(&b).unwrap().to_string(), "-1");
}

#[test]
fn normal_pow() {
    let two = Integer::from(2);
    assert_eq!(two.pow(10).to_string(), "1024");
    assert_eq!(two.pow(0).to_string(), "1");
    assert_eq!(Integer::from(0).pow(0).to_string(), "1");
    assert_eq!(Integer::from(-3).pow(3).to_string(), "-27");
    assert_eq!(Integer::from(-3).pow(2).to_string(), "9");
}

#[test]
fn normal_parse_display_roundtrip() {
    for s in ["0", "1", "-1", "123456789", "-987654321"] {
        let n: Integer = s.parse().unwrap();
        assert_eq!(n.to_string(), s);
    }
    assert_eq!(
        Integer::from_str_radix("ff", 16).unwrap().to_string(),
        "255"
    );
    assert_eq!(
        Integer::from_str_radix("-101", 2).unwrap().to_string(),
        "-5"
    );
}

#[test]
fn normal_ordering_and_predicates() {
    let neg = Integer::from(-5);
    let zero = Integer::from(0);
    let pos = Integer::from(5);
    assert!(neg < zero && zero < pos);
    assert!(neg.is_negative() && !neg.is_zero());
    assert!(zero.is_zero() && !zero.is_positive() && !zero.is_negative());
    assert!(pos.is_positive());
}

// ---- Extreme cases ----------------------------------------------------

#[test]
fn extreme_beyond_64_bit() {
    // 2^128, far beyond u64.
    let big = Integer::from(2).pow(128);
    assert_eq!(big.to_string(), "340282366920938463463374607431768211456");
    let bigger = &big * &big; // 2^256
    assert_eq!(bigger.checked_div(&big).unwrap(), big);
}

#[test]
fn extreme_i64_boundaries() {
    let min = Integer::from(i64::MIN);
    let max = Integer::from(i64::MAX);
    assert_eq!(min.to_string(), "-9223372036854775808");
    assert_eq!(max.to_string(), "9223372036854775807");
    // No overflow: max + 1 is representable.
    assert_eq!((max + Integer::from(1)).to_string(), "9223372036854775808");
    assert_eq!((min - Integer::from(1)).to_string(), "-9223372036854775809");
}

#[test]
fn extreme_large_pow_and_div() {
    let ten = Integer::from(10);
    let googol = ten.pow(100);
    assert_eq!(googol.to_string().len(), 101); // 1 followed by 100 zeros
    assert_eq!(googol.checked_div(&ten.pow(99)).unwrap().to_string(), "10");
    assert_eq!(
        googol.checked_rem(&Integer::from(7)).unwrap().to_string(),
        "4"
    );
}

#[test]
fn extreme_zero_and_sign_edges() {
    let zero = Integer::from(0);
    assert_eq!((zero.clone() * Integer::from(123)).to_string(), "0");
    assert_eq!((zero.clone() - zero.clone()).to_string(), "0");
    assert_eq!((-zero.clone()).to_string(), "0");
    assert_eq!(
        zero.checked_div(&Integer::from(5)).unwrap().to_string(),
        "0"
    );
}

// ---- Error cases (异常报错) -------------------------------------------

#[test]
fn error_division_by_zero() {
    let a = Integer::from(21);
    let zero = Integer::from(0);
    assert!(a.checked_div(&zero).is_none());
    assert!(a.checked_rem(&zero).is_none());
    assert_eq!(a.div_rem(&zero), Err(Error::DivisionByZero));
    assert_eq!(zero.div_rem(&zero), Err(Error::DivisionByZero));
}

#[test]
fn error_invalid_parse() {
    assert!("".parse::<Integer>().is_err());
    assert!("abc".parse::<Integer>().is_err());
    assert!("12.5".parse::<Integer>().is_err());
    assert!("1 2".parse::<Integer>().is_err());
    assert!("--1".parse::<Integer>().is_err());
    assert!(Integer::from_str_radix("123", 1).is_err()); // radix too small
    assert!(Integer::from_str_radix("123", 37).is_err()); // radix too large
    assert!(Integer::from_str_radix("g", 16).is_err()); // invalid digit
}

#[test]
fn error_display_is_meaningful() {
    let e = "not_a_number".parse::<Integer>().unwrap_err();
    assert!(matches!(e, Error::Parse(_)));
    assert!(e.to_string().contains("invalid numeric literal"));
    assert_eq!(Error::DivisionByZero.to_string(), "division by zero");
}

// ---- Interruption / robustness cases (异常中断) -----------------------
// Integer operations are exact and total (no panics on valid inputs); these
// ensure edge computations complete without panicking or hanging and that
// fallible paths never panic on a zero divisor.

#[test]
fn robustness_zero_divisor_never_panics() {
    let zero = Integer::from(0);
    for v in [i64::MIN, -1, 0, 1, i64::MAX] {
        let a = Integer::from(v);
        assert!(a.checked_div(&zero).is_none());
        assert!(a.checked_rem(&zero).is_none());
        assert!(a.div_rem(&zero).is_err());
    }
}

#[test]
fn robustness_large_computation_completes() {
    // A large but bounded computation must terminate with the right answer.
    let base = Integer::from(3);
    let p = base.pow(200);
    let q = p.checked_div(&base.pow(199)).unwrap();
    assert_eq!(q.to_string(), "3");
    // Repeated squaring stays consistent.
    let mut acc = Integer::from(1);
    for _ in 0..64 {
        acc = &acc * &acc; // 1 stays 1; exercises the mul path repeatedly
    }
    assert_eq!(acc.to_string(), "1");
}
