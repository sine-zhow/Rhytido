// SPDX-License-Identifier: Apache-2.0
//! Module-test layer (AGENTS.md §4): public API of `src/bigfloat.rs`.
//!
//! External-crate view of `BigFloat`, migrated from the former inline test
//! module with names and assertions preserved. Four categories: normal /
//! extreme / error / robustness. Test helpers are free functions: integration
//! tests are separate crates and cannot add inherent impls to `BigFloat`.

use rhytido_backend::{BigFloat, DEFAULT_PRECISION, Rounding};

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
