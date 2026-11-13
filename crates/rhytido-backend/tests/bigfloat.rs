// SPDX-License-Identifier: Apache-2.0
//! Module-test layer (AGENTS.md §4): public API of `src/bigfloat.rs`.
//!
//! External-crate view of `BigFloat`, migrated from the former inline test
//! module with names and assertions preserved. Four categories: normal /
//! extreme / error / robustness. Test helpers are free functions: integration
//! tests are separate crates and cannot add inherent impls to `BigFloat`.

use rhytido_backend::{BigFloat, DEFAULT_PRECISION, Integer, Rounding};

fn close(a: &BigFloat, b: &BigFloat, bits: usize) -> bool {
    // Compare at a reduced precision to tolerate last-bit rounding.
    let (ar, br) = (a.with_precision(bits), b.with_precision(bits));
    ar == br
}

// Helper: integer power via repeated multiplication. A free function because
// integration tests are separate crates and cannot add inherent impls to
// `BigFloat` (orphan rule, E0116).
fn powi_helper(x: &BigFloat, n: u32) -> BigFloat {
    let mut acc = BigFloat::one();
    for _ in 0..n {
        acc = acc * x.clone();
    }
    acc
}

// ---- Normal cases -----------------------------------------------------

#[test]
fn normal_construct_and_display() {
    assert_eq!(BigFloat::from_i64(5).to_string(), "5");
    assert_eq!(BigFloat::from_i64(-3).to_string(), "-3");
    assert_eq!(BigFloat::from_f64(0.5).to_string(), "0.5");
    assert_eq!("2.5".parse::<BigFloat>().unwrap().to_string(), "2.5");
    assert_eq!(BigFloat::from_i64(0).to_string(), "0");
}

#[test]
fn normal_arithmetic() {
    let a = BigFloat::from_f64(1.5);
    let b = BigFloat::from_f64(2.25);
    assert_eq!((a.clone() + b.clone()).to_string(), "3.75");
    assert_eq!((b.clone() - a.clone()).to_string(), "0.75");
    assert_eq!((a.clone() * b.clone()).to_string(), "3.375");
    assert!(close(&(a.clone() / b), &BigFloat::from_f64(1.5 / 2.25), 50));
    assert_eq!((-a).to_string(), "-1.5");
}

#[test]
fn normal_sqrt_exp_ln() {
    assert_eq!(BigFloat::from_i64(4).sqrt().to_string(), "2");
    assert_eq!(BigFloat::from_i64(0).exp().to_string(), "1");
    assert_eq!(BigFloat::from_i64(1).ln().to_string(), "0");
    assert!(close(
        &BigFloat::from_i64(2).sqrt(),
        &BigFloat::from_f64(core::f64::consts::SQRT_2),
        50
    ));
}

#[test]
fn normal_ordering() {
    let a = BigFloat::from_f64(1.5);
    let b = BigFloat::from_f64(2.5);
    assert!(a < b);
    assert!(b > a);
    assert_eq!(a, BigFloat::from_f64(1.5));
    assert!(BigFloat::zero() == -BigFloat::zero()); // -0 == +0
}

#[test]
fn normal_precision() {
    let x = BigFloat::from_i64(1).with_precision(100);
    assert_eq!(x.precision(), 100);
    let third = BigFloat::from_i64(1) / BigFloat::from_i64(3);
    assert!(third.precision() <= DEFAULT_PRECISION);
}

// ---- Extreme cases ----------------------------------------------------

#[test]
fn extreme_large_and_small() {
    let big = powi_helper(&BigFloat::from_i64(2), 100);
    assert!(big > BigFloat::from_i64(1_000_000));
    let tiny = BigFloat::from_i64(1) / big.clone();
    assert!(tiny > BigFloat::zero());
    assert!(tiny < BigFloat::from_f64(1e-20));
}

#[test]
fn extreme_high_precision_rounding() {
    // 1/3 at high precision, then re-round to fewer bits.
    let third = (BigFloat::from_i64(1) / BigFloat::from_i64(3)).with_precision(200);
    let r = third.with_precision(10);
    assert_eq!(r.precision(), 10);
    assert!(close(&r, &third, 10));
}

#[test]
fn extreme_directed_rounding() {
    let third = (BigFloat::from_i64(1) / BigFloat::from_i64(3)).with_precision(200);
    let up = third.round_to(8, Rounding::Up);
    let down = third.round_to(8, Rounding::Down);
    let zero = third.round_to(8, Rounding::Zero);
    assert!(up >= down);
    assert!(up >= third.with_precision(8));
    assert!(down <= third.with_precision(8));
    assert_eq!(zero, down); // positive value: toward-zero == down
    // Negative value: toward-zero == up.
    let neg = -third;
    assert_eq!(
        neg.round_to(8, Rounding::Zero),
        neg.round_to(8, Rounding::Up)
    );
}

#[test]
fn extreme_i64_boundaries_as_float() {
    let min = BigFloat::from_i64(i64::MIN);
    let max = BigFloat::from_i64(i64::MAX);
    assert!(min < BigFloat::zero());
    assert!(max > BigFloat::zero());
    assert_eq!(min.to_string(), "-9223372036854775808");
    assert_eq!(max.to_string(), "9223372036854775807");
}

// ---- Error cases (异常报错) -------------------------------------------

#[test]
fn error_nan_producing_operations() {
    let zero = BigFloat::zero();
    let one = BigFloat::one();
    let inf = BigFloat::infinity();
    assert!((zero.clone() / zero.clone()).is_nan()); // 0/0
    assert!((inf.clone() / inf.clone()).is_nan()); // inf/inf
    assert!((inf.clone() - inf.clone()).is_nan()); // inf-inf
    assert!((inf.clone() + BigFloat::neg_infinity()).is_nan());
    assert!((zero * inf).is_nan()); // 0*inf
    assert!(one.div_zero().is_infinite()); // 1/0
    assert!((-one.clone()).div_zero().is_infinite());
    assert!((-one).div_zero().is_sign_negative());
}

#[test]
fn error_domain_errors() {
    assert!(BigFloat::from_i64(-1).sqrt().is_nan());
    assert!(BigFloat::from_i64(-1).ln().is_nan());
    assert!(BigFloat::zero().ln().is_infinite());
    assert!(BigFloat::zero().ln().is_sign_negative());
    assert!(BigFloat::neg_infinity().sqrt().is_nan());
    assert!(BigFloat::neg_infinity().ln().is_nan());
}

#[test]
fn error_invalid_parse() {
    assert!("".parse::<BigFloat>().is_err());
    assert!("abc".parse::<BigFloat>().is_err());
    assert!("1.2.3".parse::<BigFloat>().is_err());
    assert!("nan".parse::<BigFloat>().unwrap().is_nan());
    assert!("inf".parse::<BigFloat>().unwrap().is_infinite());
    assert!("-infty".parse::<BigFloat>().unwrap().is_sign_negative());
}

#[test]
fn error_nan_comparison_unordered() {
    let nan = BigFloat::NAN;
    let one = BigFloat::one();
    assert!(nan != one);
    assert!(nan != nan.clone());
    assert!(nan.partial_cmp(&one).is_none());
    let nan_lt = nan < one;
    let nan_gt = nan > one;
    assert!(!nan_lt && !nan_gt);
}

// ---- Interruption / robustness cases (异常中断) -----------------------

#[test]
fn robustness_nan_propagates() {
    let nan = BigFloat::NAN;
    let one = BigFloat::one();
    assert!((nan.clone() + one.clone()).is_nan());
    assert!((one.clone() + nan.clone()).is_nan());
    assert!((nan.clone() * one.clone()).is_nan());
    assert!((one.clone() / nan.clone()).is_nan());
    assert!((-nan.clone()).is_nan());
    assert!(nan.sqrt().is_nan());
    assert!(nan.exp().is_nan());
    assert!(nan.ln().is_nan());
    assert!(nan.round_to(10, Rounding::Up).is_nan());
}

#[test]
fn robustness_special_values_never_panic() {
    let vals = [
        BigFloat::NAN,
        BigFloat::infinity(),
        BigFloat::neg_infinity(),
        BigFloat::zero(),
        -BigFloat::zero(),
        BigFloat::one(),
    ];
    for a in &vals {
        for b in &vals {
            let _ = a.clone() + b.clone();
            let _ = a.clone() - b.clone();
            let _ = a.clone() * b.clone();
            let _ = a.clone() / b.clone();
        }
        let _ = a.sqrt();
        let _ = a.exp();
        let _ = a.ln();
        let _ = a.round_to(5, Rounding::Zero);
    }
}

#[test]
fn robustness_infinity_arithmetic() {
    let inf = BigFloat::infinity();
    let ninf = BigFloat::neg_infinity();
    let one = BigFloat::one();
    assert!((inf.clone() + one.clone()).is_infinite());
    assert!((inf.clone() * one.clone()).is_infinite());
    assert!((inf.clone() * -one.clone()).is_sign_negative());
    assert!((one.clone() / inf.clone()).is_zero());
    assert!((inf.clone() / one.clone()).is_infinite());
    assert!(ninf.exp().is_zero());
    assert!(inf.exp().is_infinite());
    assert!(inf.ln().is_infinite());
    assert!(inf.sqrt().is_infinite());
}

// ---- Elementary functions: normal cases -------------------------------

#[test]
fn normal_trig_at_zero() {
    let zero = BigFloat::zero();
    assert!(zero.sin().is_zero());
    assert!(close(&zero.cos(), &BigFloat::one(), 60));
    assert!(zero.tan().is_zero());
    assert!(zero.asin().is_zero());
    assert!(close(&BigFloat::one().acos(), &zero, 60));
    assert!(zero.atan().is_zero());
}

#[test]
fn normal_hyperbolic_at_zero() {
    let zero = BigFloat::zero();
    assert!(zero.sinh().is_zero());
    assert!(close(&zero.cosh(), &BigFloat::one(), 60));
    assert!(zero.tanh().is_zero());
    assert!(zero.asinh().is_zero());
    assert!(close(&BigFloat::one().acosh(), &zero, 60));
    assert!(zero.atanh().is_zero());
}

#[test]
fn normal_trig_identities() {
    let x = BigFloat::from_f64(0.7);
    let s = x.sin();
    let c = x.cos();
    let sum = s.clone() * s.clone() + c.clone() * c.clone();
    assert!(close(&sum, &BigFloat::one(), 50));
    let t = x.tan();
    assert!(close(&t, &(s / c), 50));
}

#[test]
fn normal_hyperbolic_identity() {
    let x = BigFloat::from_f64(0.7);
    let s = x.sinh();
    let c = x.cosh();
    let diff = c.clone() * c - s.clone() * s;
    assert!(close(&diff, &BigFloat::one(), 50));
}

#[test]
fn normal_atan_one_is_pi_over_4() {
    let quarter_pi = BigFloat::one().atan();
    let half_pi = BigFloat::one().asin();
    assert!(close(&half_pi, &(quarter_pi * BigFloat::from_i64(2)), 50));
}

// ---- Elementary functions: extreme cases ------------------------------

#[test]
fn extreme_trig_special_values() {
    let inf = BigFloat::infinity();
    let ninf = BigFloat::neg_infinity();
    assert!(inf.sin().is_nan());
    assert!(ninf.sin().is_nan());
    assert!(inf.cos().is_nan());
    assert!(inf.tan().is_nan());
    assert!(inf.asin().is_nan());
    assert!(inf.acos().is_nan());
    let pi_over_2 = BigFloat::one().asin();
    assert!(close(&inf.atan(), &pi_over_2, 50));
    assert!(close(&ninf.atan(), &-pi_over_2, 50));
}

#[test]
fn extreme_hyperbolic_special_values() {
    let inf = BigFloat::infinity();
    let ninf = BigFloat::neg_infinity();
    assert!(inf.sinh().is_infinite());
    assert!(inf.sinh().is_sign_positive());
    assert!(ninf.sinh().is_sign_negative());
    assert!(inf.cosh().is_infinite());
    assert!(ninf.cosh().is_infinite());
    assert!(ninf.cosh().is_sign_positive());
    assert!(close(&inf.tanh(), &BigFloat::one(), 60));
    assert!(close(&ninf.tanh(), &-BigFloat::one(), 60));
    assert!(inf.asinh().is_infinite());
    assert!(inf.acosh().is_infinite());
    assert!(ninf.acosh().is_nan());
    assert!(inf.atanh().is_nan());
}

#[test]
fn extreme_domain_boundaries() {
    let one = BigFloat::one();
    let neg_one = -one.clone();
    let pi_over_2 = one.asin();
    assert!(close(&one.asin(), &pi_over_2, 60));
    assert!(close(&neg_one.asin(), &-pi_over_2, 60));
    assert!(one.acos().is_zero());
    assert!(one.atanh().is_infinite());
    assert!(one.atanh().is_sign_positive());
    assert!(neg_one.atanh().is_infinite());
    assert!(neg_one.atanh().is_sign_negative());
    assert!(one.acosh().is_zero());
}

// ---- Elementary functions: error cases (异常报错) ---------------------

#[test]
fn error_out_of_domain() {
    let two = BigFloat::from_i64(2);
    let neg_two = -two.clone();
    assert!(two.asin().is_nan());
    assert!(neg_two.asin().is_nan());
    assert!(two.acos().is_nan());
    assert!(BigFloat::zero().acosh().is_nan());
    assert!(neg_two.acosh().is_nan());
    assert!(two.atanh().is_nan());
    assert!(neg_two.atanh().is_nan());
}

#[test]
fn error_nan_input_all_functions() {
    let nan = BigFloat::NAN;
    assert!(nan.sin().is_nan());
    assert!(nan.cos().is_nan());
    assert!(nan.tan().is_nan());
    assert!(nan.asin().is_nan());
    assert!(nan.acos().is_nan());
    assert!(nan.atan().is_nan());
    assert!(nan.sinh().is_nan());
    assert!(nan.cosh().is_nan());
    assert!(nan.tanh().is_nan());
    assert!(nan.asinh().is_nan());
    assert!(nan.acosh().is_nan());
    assert!(nan.atanh().is_nan());
}

// ---- Elementary functions: robustness (异常中断) ----------------------

#[test]
fn robustness_all_functions_special_values_never_panic() {
    let vals = [
        BigFloat::NAN,
        BigFloat::infinity(),
        BigFloat::neg_infinity(),
        BigFloat::zero(),
        BigFloat::one(),
        -BigFloat::one(),
        BigFloat::from_i64(2),
        BigFloat::from_f64(0.5),
    ];
    for v in &vals {
        let _ = v.sin();
        let _ = v.cos();
        let _ = v.tan();
        let _ = v.asin();
        let _ = v.acos();
        let _ = v.atan();
        let _ = v.sinh();
        let _ = v.cosh();
        let _ = v.tanh();
        let _ = v.asinh();
        let _ = v.acosh();
        let _ = v.atanh();
    }
}

// ---- is_integer / pi / pow: normal cases ------------------------------

#[test]
fn normal_is_integer() {
    assert!(BigFloat::zero().is_integer());
    assert!(BigFloat::from_i64(5).is_integer());
    assert!(BigFloat::from_i64(-7).is_integer());
    assert!(!BigFloat::from_f64(0.5).is_integer());
    assert!(!BigFloat::from_f64(1.5).is_integer());
    assert!(!BigFloat::from_f64(-2.25).is_integer());
}

#[test]
fn normal_pi_value() {
    let pi = BigFloat::pi_with_precision(165);
    assert!(close(&pi, &BigFloat::from_f64(std::f64::consts::PI), 50));
    let four_atan1 = BigFloat::one().atan() * BigFloat::from_i64(4);
    assert!(close(&pi, &four_atan1, 60));
}

#[test]
fn normal_pow_integers() {
    let two = BigFloat::from_i64(2);
    let three = BigFloat::from_i64(3);
    assert!(close(&two.pow(&three), &BigFloat::from_i64(8), 60));
    assert!(close(
        &two.pow(&BigFloat::from_i64(-1)),
        &BigFloat::from_f64(0.5),
        60
    ));
    assert!(close(&two.pow(&BigFloat::zero()), &BigFloat::one(), 60));
    assert!(close(&BigFloat::one().pow(&two), &BigFloat::one(), 60));
}

#[test]
fn normal_pow_fractional() {
    let four = BigFloat::from_i64(4);
    let half = BigFloat::from_f64(0.5);
    assert!(close(&four.pow(&half), &BigFloat::from_i64(2), 50));
    let two = BigFloat::from_i64(2);
    assert!(close(&two.pow(&half), &two.sqrt(), 50));
}

// ---- is_integer / pi / pow: extreme cases -----------------------------

#[test]
fn extreme_pow_zero_and_infinity() {
    let zero = BigFloat::zero();
    let two = BigFloat::from_i64(2);
    let neg_two = BigFloat::from_i64(-2);
    let inf = BigFloat::infinity();
    assert!(zero.pow(&two).is_zero());
    assert!(zero.pow(&neg_two).is_infinite());
    assert!(inf.pow(&two).is_infinite());
    assert!(inf.pow(&neg_two).is_zero());
    assert!(two.pow(&inf).is_infinite());
    assert!(two.pow(&-inf.clone()).is_zero());
    assert!(BigFloat::from_f64(0.5).pow(&inf).is_zero());
}

#[test]
fn extreme_pow_negative_base_integer() {
    let neg_two = BigFloat::from_i64(-2);
    assert!(close(
        &neg_two.pow(&BigFloat::from_i64(3)),
        &BigFloat::from_i64(-8),
        60
    ));
    assert!(close(
        &neg_two.pow(&BigFloat::from_i64(2)),
        &BigFloat::from_i64(4),
        60
    ));
}

#[test]
fn extreme_is_integer_special() {
    assert!(!BigFloat::NAN.is_integer());
    assert!(!BigFloat::infinity().is_integer());
    assert!(!BigFloat::neg_infinity().is_integer());
    assert!(BigFloat::from_i64(i64::MAX).is_integer());
}

// ---- is_integer / pi / pow: error cases (异常报错) --------------------

#[test]
fn error_pow_negative_base_non_integer() {
    let neg_two = BigFloat::from_i64(-2);
    assert!(neg_two.pow(&BigFloat::from_f64(0.5)).is_nan());
}

#[test]
fn error_pow_nan() {
    let nan = BigFloat::NAN;
    let two = BigFloat::from_i64(2);
    assert!(nan.pow(&two).is_nan());
    assert!(two.pow(&nan).is_nan());
    assert!(close(&nan.pow(&BigFloat::zero()), &BigFloat::one(), 60));
    assert!(close(&BigFloat::one().pow(&nan), &BigFloat::one(), 60));
}

// ---- is_integer / pi / pow: robustness (异常中断) ---------------------

#[test]
fn robustness_pow_special_values_never_panic() {
    let vals = [
        BigFloat::NAN,
        BigFloat::infinity(),
        BigFloat::neg_infinity(),
        BigFloat::zero(),
        BigFloat::one(),
        -BigFloat::one(),
        BigFloat::from_i64(2),
        BigFloat::from_f64(0.5),
        BigFloat::from_i64(-2),
    ];
    for a in &vals {
        for b in &vals {
            let _ = a.pow(b);
        }
    }
}

// ---- M2-A function family: normal cases ---------------------------------

#[test]
fn normal_log2_powers_of_two() {
    assert_eq!(BigFloat::from_i64(8).log2(), BigFloat::from_i64(3));
    assert_eq!(BigFloat::from_i64(1).log2(), BigFloat::zero());
    assert_eq!(BigFloat::from_f64(0.5).log2(), BigFloat::from_i64(-1));
    assert_eq!(BigFloat::from_i64(1024).log2(), BigFloat::from_i64(10));
    let big = powi_helper(&BigFloat::from_i64(2), 100);
    assert_eq!(big.log2(), BigFloat::from_i64(100));
}

#[test]
fn normal_log10_powers_of_ten() {
    assert_eq!(BigFloat::from_i64(1000).log10(), BigFloat::from_i64(3));
    assert_eq!(BigFloat::from_i64(1).log10(), BigFloat::zero());
    assert_eq!(BigFloat::from_i64(100).log10(), BigFloat::from_i64(2));
    // 0.01 is not dyadic: the logarithm is close to -2, not exactly -2.
    assert!(close(
        &BigFloat::from_f64(0.01).log10(),
        &BigFloat::from_i64(-2),
        50
    ));
}

#[test]
fn normal_log_base_change() {
    let x = BigFloat::from_f64(7.3);
    let ln = x.ln();
    assert!(close(
        &x.log2(),
        &(ln.clone() / BigFloat::from_i64(2).ln()),
        50
    ));
    assert!(close(&x.log10(), &(ln / BigFloat::from_i64(10).ln()), 50));
}

#[test]
fn normal_log1p_values() {
    assert!(BigFloat::zero().log1p().is_zero());
    let e_minus_1 = BigFloat::one().exp() - BigFloat::one();
    assert!(close(&e_minus_1.log1p(), &BigFloat::one(), 50));
    let half = BigFloat::from_f64(0.5);
    assert!(close(&half.log1p(), &BigFloat::from_f64(1.5).ln(), 50));
    let neg_half = BigFloat::from_f64(-0.5);
    assert!(close(&neg_half.log1p(), &BigFloat::from_f64(0.5).ln(), 50));
}

#[test]
fn normal_expm1_values() {
    assert!(BigFloat::zero().expm1().is_zero());
    let ln2 = BigFloat::from_i64(2).ln();
    assert!(close(&ln2.expm1(), &BigFloat::one(), 50));
    let one = BigFloat::one();
    assert!(close(&one.expm1(), &(one.exp() - one), 50));
}

#[test]
fn normal_log1p_expm1_small_x_accuracy() {
    // For tiny x both functions must track x itself (the whole point of the
    // log1p/expm1 pair): agreement far below the working precision.
    let tiny = BigFloat::one() / powi_helper(&BigFloat::from_i64(10), 20);
    assert!(close(&tiny.log1p(), &tiny, 60));
    assert!(close(&tiny.expm1(), &tiny, 60));
    let neg_tiny = -tiny;
    assert!(close(&neg_tiny.log1p(), &neg_tiny, 60));
    assert!(close(&neg_tiny.expm1(), &neg_tiny, 60));
}

#[test]
fn normal_log1p_expm1_roundtrip_identities() {
    // expm1(log1p(x)) = x and log1p(expm1(x)) = x across the domain.
    for v in [0.7f64, -0.3, 0.001, 2.0, -0.999, 10.0] {
        let x = BigFloat::from_f64(v);
        assert!(close(&x.log1p().expm1(), &x, 50));
        assert!(close(&x.expm1().log1p(), &x, 50));
    }
}

#[test]
fn normal_abs_values() {
    assert_eq!(BigFloat::from_i64(-5).abs(), BigFloat::from_i64(5));
    assert_eq!(BigFloat::from_i64(5).abs(), BigFloat::from_i64(5));
    assert_eq!(BigFloat::from_f64(-2.5).abs(), BigFloat::from_f64(2.5));
    assert!(BigFloat::zero().abs().is_zero());
}

#[test]
fn normal_ceil_floor_values() {
    let cases = [
        (2.1f64, 3i64, 2i64),
        (2.9, 3, 2),
        (-2.1, -2, -3),
        (-2.9, -2, -3),
        (5.0, 5, 5),
        (-7.0, -7, -7),
    ];
    for (v, c, f) in cases {
        let x = BigFloat::from_f64(v);
        assert_eq!(x.ceil(), BigFloat::from_i64(c));
        assert_eq!(x.floor(), BigFloat::from_i64(f));
    }
}

#[test]
fn normal_nearestint_values() {
    let cases = [
        (2.4f64, 2i64),
        (2.6, 3),
        (-2.4, -2),
        (-2.6, -3),
        (7.0, 7),
        (-7.0, -7),
    ];
    for (v, n) in cases {
        assert_eq!(BigFloat::from_f64(v).nearestint(), BigFloat::from_i64(n));
    }
}

#[test]
fn normal_nearestint_ties_to_even() {
    let cases = [
        (0.5f64, 0i64),
        (1.5, 2),
        (2.5, 2),
        (3.5, 4),
        (-0.5, 0),
        (-1.5, -2),
        (-2.5, -2),
        (-3.5, -4),
    ];
    for (v, n) in cases {
        assert_eq!(BigFloat::from_f64(v).nearestint(), BigFloat::from_i64(n));
    }
}

// ---- M2-A function family: extreme cases --------------------------------

#[test]
fn extreme_log2_log10_special_values() {
    for f in [BigFloat::log2, BigFloat::log10] {
        assert!(f(&BigFloat::NAN).is_nan());
        assert!(f(&BigFloat::from_i64(-1)).is_nan());
        assert!(f(&BigFloat::neg_infinity()).is_nan());
        let z = f(&BigFloat::zero());
        assert!(z.is_infinite() && z.is_sign_negative());
        let nz = f(&-BigFloat::zero());
        assert!(nz.is_infinite() && nz.is_sign_negative());
        let p = f(&BigFloat::infinity());
        assert!(p.is_infinite() && p.is_sign_positive());
    }
}

#[test]
fn extreme_log1p_special_values() {
    let f = BigFloat::log1p;
    assert!(f(&BigFloat::NAN).is_nan());
    assert!(f(&BigFloat::from_i64(-2)).is_nan());
    assert!(f(&BigFloat::neg_infinity()).is_nan());
    let pole = f(&BigFloat::from_i64(-1));
    assert!(pole.is_infinite() && pole.is_sign_negative());
    let p = f(&BigFloat::infinity());
    assert!(p.is_infinite() && p.is_sign_positive());
    let z = f(&BigFloat::zero());
    assert!(z.is_zero() && z.is_sign_positive());
    let nz = f(&-BigFloat::zero());
    assert!(nz.is_zero() && nz.is_sign_negative());
}

#[test]
fn extreme_expm1_special_values() {
    let f = BigFloat::expm1;
    assert!(f(&BigFloat::NAN).is_nan());
    let p = f(&BigFloat::infinity());
    assert!(p.is_infinite() && p.is_sign_positive());
    assert_eq!(f(&BigFloat::neg_infinity()), -BigFloat::one());
    let z = f(&BigFloat::zero());
    assert!(z.is_zero() && z.is_sign_positive());
    let nz = f(&-BigFloat::zero());
    assert!(nz.is_zero() && nz.is_sign_negative());
    // Large negative argument rounds to exactly -1 at working precision.
    assert_eq!(BigFloat::from_i64(-1000).expm1(), -BigFloat::one());
}

#[test]
fn extreme_abs_special_values() {
    assert!(BigFloat::NAN.abs().is_nan());
    let p = BigFloat::infinity().abs();
    assert!(p.is_infinite() && p.is_sign_positive());
    let n = BigFloat::neg_infinity().abs();
    assert!(n.is_infinite() && n.is_sign_positive());
    let neg_zero = -BigFloat::zero();
    assert!(neg_zero.is_sign_negative());
    let a = neg_zero.abs();
    assert!(a.is_zero());
    assert!(a.is_sign_positive());
    assert!(BigFloat::zero().abs().is_sign_positive());
}

#[test]
fn extreme_rounding_special_values() {
    for f in [BigFloat::ceil, BigFloat::floor, BigFloat::nearestint] {
        assert!(f(&BigFloat::NAN).is_nan());
        assert_eq!(f(&BigFloat::infinity()), BigFloat::infinity());
        assert_eq!(f(&BigFloat::neg_infinity()), BigFloat::neg_infinity());
        assert!(f(&BigFloat::zero()).is_zero());
        assert!(f(&-BigFloat::zero()).is_zero());
    }
}

#[test]
fn extreme_rounding_half_integers() {
    let cases = [
        (0.5f64, 1i64, 0i64),
        (-0.5, 0, -1),
        (1.5, 2, 1),
        (-1.5, -1, -2),
        (2.5, 3, 2),
        (-2.5, -2, -3),
    ];
    for (v, c, fl) in cases {
        let x = BigFloat::from_f64(v);
        assert_eq!(x.ceil(), BigFloat::from_i64(c));
        assert_eq!(x.floor(), BigFloat::from_i64(fl));
    }
}

#[test]
fn extreme_rounding_large_and_tiny() {
    // 2^100 + 0.5: exact at working precision, tie to even.
    let big = powi_helper(&BigFloat::from_i64(2), 100);
    let x = big.clone() + BigFloat::from_f64(0.5);
    assert_eq!(x.floor(), big.clone());
    assert_eq!(x.ceil(), big.clone() + BigFloat::one());
    assert_eq!(x.nearestint(), big.clone()); // 2^100 is even.
    let odd = big.clone() + BigFloat::one();
    let y = odd.clone() + BigFloat::from_f64(0.5);
    assert_eq!(y.nearestint(), odd + BigFloat::one()); // Tie up to even.
    // Tiny magnitudes.
    let tiny = BigFloat::one() / powi_helper(&BigFloat::from_i64(2), 100);
    assert_eq!(tiny.ceil(), BigFloat::one());
    assert_eq!(tiny.floor(), BigFloat::zero());
    assert_eq!(tiny.nearestint(), BigFloat::zero());
    let neg_tiny = -tiny;
    assert_eq!(neg_tiny.ceil(), BigFloat::zero());
    assert_eq!(neg_tiny.floor(), -BigFloat::one());
    assert_eq!(neg_tiny.nearestint(), BigFloat::zero());
}

#[test]
fn extreme_log2_large_power_exact() {
    let big = powi_helper(&BigFloat::from_i64(2), 1000);
    assert_eq!(big.log2(), BigFloat::from_i64(1000));
    let small = BigFloat::one() / big;
    assert_eq!(small.log2(), BigFloat::from_i64(-1000));
}

// ---- M2-A function family: error cases (异常报错) ------------------------

#[test]
fn error_new_function_domain() {
    assert!(BigFloat::from_i64(-8).log2().is_nan());
    assert!(BigFloat::from_i64(-100).log10().is_nan());
    assert!(BigFloat::from_f64(-1.5).log1p().is_nan());
    assert!(BigFloat::from_i64(-100).log1p().is_nan());
    // Just above the pole stays finite: the -1 comparison is exact, so a
    // value that would round onto -1 at low precision keeps its logarithm.
    let near_pole =
        BigFloat::from_i64(-1) + BigFloat::one() / powi_helper(&BigFloat::from_i64(2), 100);
    let r = near_pole.log1p();
    assert!(r.is_finite());
    assert!(close(
        &r,
        &(BigFloat::from_i64(-100) * BigFloat::from_i64(2).ln()),
        50
    ));
}

#[test]
fn error_new_functions_nan_input() {
    let nan = BigFloat::NAN;
    assert!(nan.log2().is_nan());
    assert!(nan.log10().is_nan());
    assert!(nan.log1p().is_nan());
    assert!(nan.expm1().is_nan());
    assert!(nan.abs().is_nan());
    assert!(nan.ceil().is_nan());
    assert!(nan.floor().is_nan());
    assert!(nan.nearestint().is_nan());
}

// ---- M2-A function family: robustness (异常中断) -------------------------

#[test]
fn robustness_new_functions_special_values_never_panic() {
    let vals = [
        BigFloat::NAN,
        BigFloat::infinity(),
        BigFloat::neg_infinity(),
        BigFloat::zero(),
        -BigFloat::zero(),
        BigFloat::one(),
        -BigFloat::one(),
        BigFloat::from_f64(0.5),
        BigFloat::from_f64(-0.5),
        BigFloat::from_f64(-1.5),
        BigFloat::from_i64(2),
        BigFloat::from_i64(-2),
    ];
    for v in &vals {
        let _ = v.log2();
        let _ = v.log10();
        let _ = v.log1p();
        let _ = v.expm1();
        let _ = v.abs();
        let _ = v.ceil();
        let _ = v.floor();
        let _ = v.nearestint();
    }
}

#[test]
fn robustness_roundtrip_extreme_arguments() {
    // Round-trip identities at the edges of the domain must not panic and
    // must stay close to the identity.
    let tiny = BigFloat::one() / powi_helper(&BigFloat::from_i64(10), 30);
    let near_pole = BigFloat::from_i64(-1) + tiny.clone();
    let large = BigFloat::from_i64(50);
    for x in [tiny, near_pole, large] {
        assert!(close(&x.log1p().expm1(), &x, 40));
        assert!(close(&x.expm1().log1p(), &x, 40));
    }
}

#[test]
fn robustness_rounding_high_precision() {
    let third = (BigFloat::one() / BigFloat::from_i64(3)).with_precision(200);
    assert_eq!(third.ceil(), BigFloat::one());
    assert_eq!(third.floor(), BigFloat::zero());
    assert_eq!(third.nearestint(), BigFloat::zero());
    let neg = -third;
    assert_eq!(neg.ceil(), BigFloat::zero());
    assert_eq!(neg.floor(), -BigFloat::one());
    assert_eq!(neg.nearestint(), BigFloat::zero());
}

// ---- M2-C next_above / next_below: normal cases -------------------------

/// Exact `(significand, exponent)` parts of a finite value, so assertions are
/// about the represented number rather than about decimal display.
fn parts(v: &BigFloat) -> (Integer, isize) {
    v.canonical_parts().expect("finite neighbor")
}

/// `2^k` as a BigFloat for any integer `k`, exact. Binary exponentiation keeps
/// every intermediate a one-bit significand, so no rounding can occur; the
/// negative case divides the exact power and is likewise exact.
fn pow2_of(k: isize) -> BigFloat {
    let mut acc = BigFloat::one();
    let mut base = BigFloat::from_i64(2);
    let mut n = k.unsigned_abs();
    while n > 0 {
        if n & 1 == 1 {
            acc = acc * base.clone();
        }
        base = base.clone() * base;
        n >>= 1;
    }
    if k >= 0 { acc } else { BigFloat::one() / acc }
}

#[test]
fn normal_next_neighbors_on_the_binary64_grid() {
    let one = BigFloat::one();
    // Above 1 the ulp is 2^-52; the binade below 1 is twice as fine.
    assert_eq!(one.next_above(53), BigFloat::from_f64(1.0000000000000002));
    assert_eq!(one.next_below(53), BigFloat::from_f64(0.9999999999999999));
    // At the default working precision the double 0.1 is an exact dyadic.
    let tenth = BigFloat::from_f64(0.1);
    assert!(tenth.next_below(53) < tenth);
    assert!(tenth < tenth.next_above(53));
    assert_eq!(tenth.next_above(53).next_below(53), tenth);
}

#[test]
fn normal_next_companion_identity_for_representable_values() {
    // A value is representable at p bits exactly when both directed roundings
    // fix it; for those values the two neighbor steps must invert each other.
    let vals = [
        BigFloat::from_f64(0.1),
        BigFloat::from_i64(3) / BigFloat::from_i64(8),
        -BigFloat::from_i64(21) / BigFloat::from_i64(4),
        BigFloat::from_f64(1.0000000000000002),
        BigFloat::one() / pow2_of(60),
    ];
    for p in [4usize, 11, 24, 53, 64, 113, DEFAULT_PRECISION] {
        for v in &vals {
            let up_fixed = v.round_to(p, Rounding::Up) == *v;
            let down_fixed = v.round_to(p, Rounding::Down) == *v;
            assert_eq!(up_fixed, down_fixed, "representability p={p}");
            if up_fixed {
                assert_eq!(v.next_above(p).next_below(p), *v, "up-then-down p={p}");
                assert_eq!(v.next_below(p).next_above(p), *v, "down-then-up p={p}");
            }
        }
    }
}

#[test]
fn normal_next_neighbors_are_adjacent_and_ordered() {
    // No p-bit float may sit strictly between a value and its neighbor: the
    // exact midpoint must round back onto the lower side.
    let vals = [
        BigFloat::from_f64(1.5),
        BigFloat::from_i64(-7) / BigFloat::from_i64(5),
        BigFloat::from_f64(12345.6789),
    ];
    for p in [4usize, 24, 53] {
        for v in &vals {
            let r = v.round_to(p, Rounding::Nearest);
            let up = r.next_above(p);
            let down = r.next_below(p);
            assert!(down < r && r < up);
            let wide = (r.clone().with_precision(p + 4) + up.clone().with_precision(p + 4))
                / BigFloat::from_i64(2);
            assert_eq!(wide.round_to(p, Rounding::Down), r);
            let wide2 = (down.clone().with_precision(p + 4) + r.clone().with_precision(p + 4))
                / BigFloat::from_i64(2);
            assert_eq!(wide2.round_to(p, Rounding::Up), r);
        }
    }
}

#[test]
fn normal_next_above_below_cross_sign_symmetry() {
    // The grid is symmetric about zero: next_above(-x) = -next_below(x).
    let vals = [
        BigFloat::one(),
        BigFloat::from_i64(8),
        BigFloat::from_f64(0.1),
        BigFloat::from_i64(2).sqrt(),
    ];
    for p in [4usize, 24, 53, 165] {
        for v in &vals {
            let neg = -v.clone();
            assert_eq!(neg.next_above(p), -v.next_below(p));
            assert_eq!(neg.next_below(p), -v.next_above(p));
        }
    }
}

#[test]
fn normal_next_non_representable_matches_directed_rounding() {
    // When the value does not fit in p bits, the neighbor toward +infinity is
    // exactly the round-up image and the neighbor toward -infinity the
    // round-down image (the minimal p-float >= / <= decision).
    let third = BigFloat::one() / BigFloat::from_i64(3);
    let pi = BigFloat::pi_with_precision(120);
    for v in [third.clone(), -third.clone(), pi.clone(), -pi.clone()] {
        for p in [4usize, 10, 53] {
            assert_eq!(v.next_above(p), v.round_to(p, Rounding::Up));
            assert_eq!(v.next_below(p), v.round_to(p, Rounding::Down));
            // Strictly past the operand in both directions.
            assert!(v.next_below(p) < v && v < v.next_above(p));
        }
    }
}

// ---- M2-C next_above / next_below: extreme cases ------------------------

#[test]
fn extreme_next_power_of_two_boundaries() {
    // At 2^k the grid above is coarser than the grid below by a factor of two:
    // next_above = 2^k + 2^(k+1-p), next_below = 2^k - 2^(k-p). Asserted on the
    // exact canonical parts, including binade crossings.
    for p in [4usize, 24, 53] {
        for k in [-40isize, -1, 0, 1, 7, 60] {
            let x = pow2_of(k);
            // The successors are (2^(p-1) + 1) * 2^(k+1-p) upward and
            // (2^p - 1) * 2^(k-p) downward: the coarse and fine grid sides.
            let up_sig = Integer::from(2).pow(p - 1) + Integer::from(1);
            let down_sig = Integer::from(2).pow(p) - Integer::from(1);
            assert_eq!(
                parts(&x.next_above(p)),
                (up_sig, k + 1 - p as isize),
                "next_above(2^{k}, {p})"
            );
            assert_eq!(
                parts(&x.next_below(p)),
                (down_sig.clone(), k - p as isize),
                "next_below(2^{k}, {p})"
            );
            assert_eq!(
                parts(&(-x.clone()).next_above(p)),
                (-down_sig, k - p as isize),
                "next_above(-(2^{k}), {p})"
            );
            // The steps invert each other across the boundary.
            assert_eq!(x.next_above(p).next_below(p), x);
            assert_eq!(x.next_below(p).next_above(p), x);
        }
    }
}

#[test]
fn extreme_next_zero_convention() {
    // Documented convention: zero steps to +-(2^(1-p)) (there is no smallest
    // positive float with an unbounded exponent range).
    for p in [4usize, 24, 53, 113] {
        let zero = BigFloat::zero();
        // 2^(1-p) is the one-bit significand 1 at exponent 1-p.
        assert_eq!(
            parts(&zero.next_above(p)),
            (Integer::from(1), 1 - p as isize)
        );
        assert_eq!(
            parts(&zero.next_below(p)),
            (-Integer::from(1), 1 - p as isize)
        );
        // Signed zero follows the same convention (the value is zero).
        assert_eq!(zero.next_above(p), (-zero.clone()).next_above(p));
        assert_eq!(zero.next_below(p), (-zero).next_below(p));
    }
}

#[test]
fn extreme_next_infinities_and_nan() {
    for p in [4usize, 53, DEFAULT_PRECISION] {
        assert!(BigFloat::NAN.next_above(p).is_nan());
        assert!(BigFloat::NAN.next_below(p).is_nan());
        let inf = BigFloat::infinity();
        assert!(inf.next_above(p).is_infinite() && inf.next_above(p).is_sign_positive());
        assert!(inf.next_below(p).is_infinite() && inf.next_below(p).is_sign_positive());
        let ninf = BigFloat::neg_infinity();
        assert!(ninf.next_above(p).is_infinite() && ninf.next_above(p).is_sign_negative());
        assert!(ninf.next_below(p).is_infinite() && ninf.next_below(p).is_sign_negative());
    }
}

#[test]
fn extreme_next_wide_and_narrow_precisions() {
    // Very wide queries stay exact and round-trip; the two-bit grid is the
    // narrowest accepted format.
    let one = BigFloat::one();
    for p in [2usize, 3, 1000, 4096] {
        let up = one.next_above(p);
        let down = one.next_below(p);
        assert!(down < one && one < up, "ordering at p={p}");
        assert_eq!(up.next_below(p), one, "round trip up at p={p}");
        assert_eq!(down.next_above(p), one, "round trip down at p={p}");
    }
    // precisions below two bits are clamped to the two-bit grid.
    assert_eq!(one.next_above(0), one.next_above(2));
    assert_eq!(one.next_above(1), one.next_above(2));
    assert_eq!(one.next_below(0), one.next_below(2));
    assert_eq!(one.next_below(1), one.next_below(2));
}

#[test]
fn extreme_next_all_ones_significand_carries() {
    // (2^p - 1) * 2^0 is the largest p-float below 2^p; its successor is the
    // power of two itself (a carry that normalization must fold).
    for p in [4usize, 24, 53] {
        let top = pow2_of(p as isize) - BigFloat::one();
        let up = top.next_above(p);
        assert!(up.is_integer(), "carry stays integral");
        assert_eq!(parts(&up), (Integer::from(1), p as isize), "carry p={p}");
        assert_eq!(up.next_below(p), top);
        let neg = -top;
        assert_eq!(
            parts(&neg.next_below(p)),
            (-Integer::from(1), p as isize),
            "negative carry p={p}"
        );
    }
}

#[test]
fn extreme_next_tiny_and_huge_exponents() {
    // Exponent extremes must not materialize an absurdly wide significand.
    for k in [-100_000isize, -1000, 1000, 100_000] {
        let x = pow2_of(k);
        let up = x.next_above(53);
        let down = x.next_below(53);
        assert!(down < x && x < up, "ordering 2^{k}");
        assert_eq!(up.next_below(53), x);
        assert_eq!(down.next_above(53), x);
        assert_eq!(parts(&up).1, k + 1 - 53, "up exponent 2^{k}");
        assert_eq!(parts(&down).1, k - 53, "down exponent 2^{k}");
    }
}

// ---- M2-C next_above / next_below: error cases (异常报错) ---------------

#[test]
fn error_next_special_operands_report_by_value() {
    // This API is infallible: out-of-contract operands (NaN, infinities) are
    // reported by value per the documented mapping instead of an error.
    let cases = [
        BigFloat::NAN,
        BigFloat::infinity(),
        BigFloat::neg_infinity(),
    ];
    for v in &cases {
        for p in [0usize, 2, 53, 1000] {
            let up = v.next_above(p);
            let down = v.next_below(p);
            assert_eq!(up.is_nan(), v.is_nan());
            assert_eq!(down.is_nan(), v.is_nan());
            if v.is_nan() {
                assert!(up.is_nan() && down.is_nan());
            } else {
                // Infinities have no finite successor: they are unchanged.
                assert_eq!(up, *v);
                assert_eq!(down, *v);
            }
        }
    }
}

#[test]
fn error_next_of_nan_propagates_through_comparisons() {
    // Neighbors of NaN stay NaN and comparisons stay undefined (None).
    let nan = BigFloat::NAN.next_above(53);
    assert!(nan.is_nan());
    assert!(nan.partial_cmp(&BigFloat::one()).is_none());
    assert!(nan != BigFloat::NAN);
}

// ---- M2-C next_above / next_below: robustness (异常中断) ----------------

#[test]
fn robustness_next_is_deterministic_and_labelled() {
    // Two calls on the same operand must agree bit-for-bit, and the result must
    // carry the requested working precision (the interval layer rounds outward
    // at the widest endpoint precision).
    let vals = [
        BigFloat::from_f64(0.1),
        BigFloat::one() / BigFloat::from_i64(3),
        BigFloat::from_i64(-5) / BigFloat::from_i64(7),
    ];
    for p in [24usize, 53, DEFAULT_PRECISION] {
        for v in &vals {
            let a = v.next_above(p);
            let b = v.next_above(p);
            assert_eq!(a, b);
            assert_eq!(a.precision(), p);
            assert_eq!(v.next_below(p).precision(), p);
        }
    }
}

#[test]
fn robustness_next_never_panics_over_a_sweep() {
    // A sweep over values, precisions and both directions must stay panic-free,
    // including operands wider than the requested precision.
    let mut v = BigFloat::from_f64(1.0 / 3.0);
    for i in 0..200 {
        let p = 2 + (i % 97);
        let _ = v.next_above(p);
        let _ = v.next_below(p);
        v = v * BigFloat::from_f64(1.0009765625);
    }
    let _ = BigFloat::zero().next_above(2);
    let _ = (-BigFloat::zero()).next_below(3);
}

// ---- M2-E1 Display decimal round-trip ----------------------------------
//
// These tests are the empirical check on dashu's digit control: `Display` must
// emit at least ceil(p*log10(2))+1 significant decimal digits so that parsing
// the string and re-rounding to p bits recovers the exact value. They replace
// the former "≤15 significant digits" Display limitation (05-testing.md §4b).

/// Asserts the Display round-trip contract for a `p`-bit value: rendering to a
/// string, parsing it back, and re-rounding to `p` bits is the identity.
fn assert_roundtrip(v: &BigFloat, p: usize) {
    let s = v.to_string();
    let parsed: BigFloat = s.parse().expect("round-trip string must parse");
    assert_eq!(
        parsed.with_precision(p),
        *v,
        "round-trip p={p}: '{s}' did not parse back to the same value"
    );
}

#[test]
fn normal_display_roundtrip_across_precisions() {
    for p in [11usize, 24, 53, 64, 113, 165, 300] {
        // Full-width non-dyadic significands exercise every rendered digit.
        let third = (BigFloat::one() / BigFloat::from_i64(3)).with_precision(p);
        let seventh = (BigFloat::from_i64(2) / BigFloat::from_i64(7)).with_precision(p);
        let pi = BigFloat::pi_with_precision(p + 20).with_precision(p);
        for v in [third, seventh, pi] {
            assert_roundtrip(&v, p);
            assert_roundtrip(&-v.clone(), p);
        }
        // The historic quirk value 2^53-1, at every precision (it rounds when
        // p < 53 and must still round-trip at the rounded width).
        let mersenne = BigFloat::from_i64((1i64 << 53) - 1).with_precision(p);
        assert_roundtrip(&mersenne, p);
        assert_roundtrip(&-mersenne.clone(), p);
    }
}

#[test]
fn normal_display_53bit_quirk_fixed() {
    // The documented quirk: a 53-bit value used to render only ~15 digits, so
    // (2^53-1) printed as 9007199254740990. It must now print all 16 digits.
    let v = BigFloat::from_i64((1i64 << 53) - 1).round_to(53, Rounding::Nearest);
    assert_eq!(v.to_string(), "9007199254740991");
    assert_roundtrip(&v, 53);
    // The negative side renders the full digits too.
    assert_eq!((-v.clone()).to_string(), "-9007199254740991");
    assert_roundtrip(&-v, 53);
}

#[test]
fn normal_display_signed_zero_roundtrip() {
    for p in [11usize, 53, 165, 300] {
        let pz = BigFloat::zero().with_precision(p);
        let nz = -(BigFloat::zero().with_precision(p));
        assert!(nz.is_sign_negative(), "construction must yield -0");
        assert_eq!(pz.to_string(), "0");
        // The ±0 display rule is unchanged by the round-trip fix: the decimal
        // conversion renders both signs as "0" (the preserved baseline). The
        // value still round-trips because +0 and -0 compare equal.
        assert_eq!(nz.to_string(), "0");
        assert_roundtrip(&pz, p);
        assert_roundtrip(&nz, p);
    }
}

#[test]
fn extreme_display_roundtrip_tiny_huge_and_subnormal() {
    // binary64 subnormal boundary and magnitude extremes. These have short
    // significands but extreme exponents, so the decimal point placement (not
    // the digit count) is what must survive.
    let smallest_subnormal = BigFloat::from_f64(2f64.powi(-1074));
    let smallest_normal = BigFloat::from_f64(2f64.powi(-1022));
    let huge = BigFloat::from_f64(2f64.powi(1023));
    let small_dec = BigFloat::from_f64(1e-30);
    let big_dec = BigFloat::from_f64(1e30);
    for p in [24usize, 53, 113, 165, 300] {
        for v in [
            smallest_subnormal.clone(),
            smallest_normal.clone(),
            huge.clone(),
            small_dec.clone(),
            big_dec.clone(),
        ] {
            let vp = v.with_precision(p);
            assert_roundtrip(&vp, p);
            assert_roundtrip(&-vp.clone(), p);
        }
    }
}

#[test]
fn extreme_display_roundtrip_all_ones_significands() {
    // (2^p - 1) is the widest p-bit significand and the hardest to round-trip:
    // every decimal digit matters. Check it at each precision, both signs.
    for p in [11usize, 24, 53, 64, 113, 165, 300] {
        let top = (pow2_of(p as isize) - BigFloat::one()).with_precision(p);
        assert_roundtrip(&top, p);
        assert_roundtrip(&-top.clone(), p);
        // One ulp below a power of two, the other all-ones boundary.
        let below = pow2_of(p as isize).next_below(p);
        assert_roundtrip(&below, p);
        assert_roundtrip(&-below.clone(), p);
    }
}

#[test]
fn robustness_display_roundtrip_sweep() {
    // A broad deterministic sweep: many precisions and value shapes must all
    // round-trip without panic, including values wider than the default.
    let mut v = BigFloat::one() / BigFloat::from_i64(7);
    for i in 0..60 {
        let p = 11 + (i % 290);
        let vp = v.with_precision(p);
        assert_roundtrip(&vp, p);
        assert_roundtrip(&-vp.clone(), p);
        v = v * BigFloat::from_f64(1.0009765625) + BigFloat::from_f64(0.25);
    }
}
