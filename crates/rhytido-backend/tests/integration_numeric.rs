// SPDX-License-Identifier: Apache-2.0
//! Integration-test layer (AGENTS.md §4): cross-type numeric scenarios.
//!
//! End-to-end consistency across `Integer`, `BigFloat`, `Rational`,
//! `Interval`, and `Format` through the public facade API only. Helpers are
//! free functions (separate test crates cannot add inherent impls, E0116).

use rhytido_backend::{BigFloat, Format, Integer, Interval, Rational, Rounding};

fn close(a: &BigFloat, b: &BigFloat, bits: usize) -> bool {
    // Compare at a reduced precision to tolerate last-bit rounding.
    let (ar, br) = (a.with_precision(bits), b.with_precision(bits));
    ar == br
}

// Helper: integer power via repeated multiplication.
fn powi_helper(x: &BigFloat, n: u32) -> BigFloat {
    let mut acc = BigFloat::one();
    for _ in 0..n {
        acc = acc * x.clone();
    }
    acc
}

// Helper: every format must round x into [down, up] with at most one ulp
// between the directed results; ulp = 2^(ulp_exp - precision).
fn check_format_bounds(x: &BigFloat, ulp_exp: i32) {
    for fmt in [
        Format::Double,
        Format::Single,
        Format::Half,
        Format::Quad,
        Format::DoubleExtended,
    ] {
        let p = fmt.precision();
        let down = x.round_to_format(fmt, Rounding::Down);
        let up = x.round_to_format(fmt, Rounding::Up);
        let nearest = x.round_to_format(fmt, Rounding::Nearest);
        assert!(down.precision() <= p);
        assert!(up.precision() <= p);
        assert!(nearest.precision() <= p);
        assert!(down <= *x);
        assert!(*x <= up);
        assert!(nearest == down || nearest == up);
        let ulp = BigFloat::from_f64(2f64.powi(ulp_exp - p as i32));
        assert!(up - down <= ulp);
    }
}

// ---- Exactness across Integer / BigFloat / Rational -------------------

#[test]
fn normal_integer_ops_match_bigfloat_and_rational() {
    // Exact integer arithmetic must agree across all three numeric types,
    // including results that overflow i64.
    for (a, b) in [
        (21i64, 2i64),
        (-7, 3),
        (0, 5),
        (i64::MAX, 1),
        (i64::MIN, -1),
    ] {
        let ia = Integer::from(a);
        let ib = Integer::from(b);
        let fa = BigFloat::from_i64(a);
        let fb = BigFloat::from_i64(b);
        let ra = Rational::from_i64(a);
        let rb = Rational::from_i64(b);
        let sum_i = (ia.clone() + ib.clone()).to_string();
        let sum_f = (fa.clone() + fb.clone()).to_string();
        let sum_r = (ra.clone() + rb.clone()).to_string();
        assert_eq!(sum_i, sum_f);
        assert_eq!(sum_i, sum_r);
        let diff_i = (ia.clone() - ib.clone()).to_string();
        let diff_f = (fa.clone() - fb.clone()).to_string();
        let diff_r = (ra.clone() - rb.clone()).to_string();
        assert_eq!(diff_i, diff_f);
        assert_eq!(diff_i, diff_r);
        let prod_i = (ia * ib).to_string();
        let prod_f = (fa * fb).to_string();
        let prod_r = (ra * rb).to_string();
        assert_eq!(prod_i, prod_f);
        assert_eq!(prod_i, prod_r);
    }
}

#[test]
fn normal_integer_pow_matches_float_and_rational() {
    // 2^100 and 3^40 exceed i64 but stay exact in all three types.
    for (base, exp) in [(2i64, 100u32), (3, 40), (-2, 61)] {
        let ip = Integer::from(base).pow(exp as usize);
        let fp = powi_helper(&BigFloat::from_i64(base), exp);
        let mut rp = Rational::from_i64(1);
        let rb = Rational::from_i64(base);
        for _ in 0..exp {
            rp = &rp * &rb;
        }
        assert_eq!(ip.to_string(), fp.to_string());
        assert_eq!(ip.to_string(), rp.to_string());
    }
}

// ---- Interval enclosure of BigFloat results ---------------------------

#[test]
fn normal_point_interval_contains_bigfloat_results() {
    // Point-interval arithmetic must contain the matching BigFloat result,
    // including operands that are not exactly representable (1/3).
    let third = BigFloat::one() / BigFloat::from_i64(3);
    let pairs = [
        (BigFloat::from_f64(1.5), BigFloat::from_f64(2.25)),
        (third.clone(), third.clone()),
        (BigFloat::from_f64(-0.5), BigFloat::from_i64(4)),
    ];
    for (a, b) in &pairs {
        let pa = Interval::point(a.clone());
        let pb = Interval::point(b.clone());
        assert!(pa.add(&pb).contains(&(a.clone() + b.clone())));
        assert!(pa.sub(&pb).contains(&(a.clone() - b.clone())));
        assert!(pa.mul(&pb).contains(&(a.clone() * b.clone())));
    }
}

// ---- Format rounding bounds on computed values ------------------------

#[test]
fn extreme_round_to_format_precision_bounds() {
    // 1/3 lies in [1/4, 1/2): one ulp at p bits is 2^-(p+1).
    let third = BigFloat::one().with_precision(200) / BigFloat::from_i64(3).with_precision(200);
    check_format_bounds(&third, -1);
    // 1/7 lies in [1/8, 1/4): one ulp at p bits is 2^-(p+2).
    let seventh = BigFloat::one().with_precision(200) / BigFloat::from_i64(7).with_precision(200);
    check_format_bounds(&seventh, -2);
}

// ---- Canonical parts contracts ----------------------------------------

#[test]
fn normal_canonical_parts_is_integer_consistency() {
    // canonical_parts and is_integer must agree, and the parts must
    // reconstruct the value exactly (Integer significand times 2^exponent).
    let vals = [
        BigFloat::zero(),
        -BigFloat::zero(),
        BigFloat::from_i64(5),
        BigFloat::from_i64(-7),
        BigFloat::from_f64(0.5),
        BigFloat::from_f64(1.5),
        BigFloat::from_f64(-2.25),
        BigFloat::one() / BigFloat::from_i64(3),
        BigFloat::from_i64(i64::MAX),
    ];
    for v in &vals {
        let (sig, exp) = v.canonical_parts().unwrap();
        let expected = sig.is_zero() || exp >= 0;
        assert_eq!(v.is_integer(), expected);
        // Reconstruct sig * 2^exp through the public facade types.
        let sig_f: BigFloat = sig.to_string().parse().unwrap();
        let two = BigFloat::from_i64(2);
        let reconstructed = if exp >= 0 {
            sig_f * powi_helper(&two, exp as u32)
        } else {
            sig_f / powi_helper(&two, (-exp) as u32)
        };
        assert_eq!(&reconstructed, v);
    }
}

#[test]
fn extreme_canonical_parts_special_values() {
    // NaN has no canonical parts; infinities are never integers.
    assert!(BigFloat::NAN.canonical_parts().is_none());
    assert!(!BigFloat::NAN.is_integer());
    for v in [BigFloat::infinity(), BigFloat::neg_infinity()] {
        assert!(!v.is_integer());
        assert!(v.is_infinite());
        assert!(!v.is_finite());
    }
    for v in [BigFloat::zero(), -BigFloat::zero()] {
        let (sig, _exp) = v.canonical_parts().unwrap();
        assert!(sig.is_zero());
        assert!(v.is_integer());
    }
}

// ---- Rational numerator/denominator vs BigFloat division --------------

#[test]
fn normal_rational_division_matches_bigfloat() {
    // Dividing a Rational's canonical numerator by its denominator must
    // agree with the direct BigFloat quotient at high precision.
    for (n, d) in [(1i64, 3i64), (22, 7), (-355, 113), (6, 4), (8, 4), (0, 5)] {
        let r = Rational::new(n, d).unwrap();
        let num: BigFloat = r.numerator().to_string().parse().unwrap();
        let den: BigFloat = r.denominator().to_string().parse().unwrap();
        let quotient = num / den;
        let direct = BigFloat::from_i64(n) / BigFloat::from_i64(d);
        assert!(close(&quotient, &direct, 120));
        assert_eq!(quotient.is_zero(), r.is_zero());
    }
    // Integer-valued rationals display identically in both types.
    let two = Rational::new(8, 4).unwrap();
    let num: BigFloat = two.numerator().to_string().parse().unwrap();
    let den: BigFloat = two.denominator().to_string().parse().unwrap();
    assert_eq!((num / den).to_string(), two.to_string());
}

#[test]
fn normal_double_format_matches_f64_for_small_integers() {
    // Integers up to 2^53 are exact in binary64: facade rounding must agree
    // with the native f64 conversion. Since the Display round-trip fix the
    // decimal cross-check spans the whole exact range as well: it was formerly
    // limited to ≤15 significant digits because the 53-bit rendering truncated
    // beyond that (05-testing.md §4b). Lifting that limit is a strengthening.
    for v in [
        0i64,
        1,
        -1,
        42,
        -1000,
        1 << 20,
        123456789012345,
        (1 << 53) - 1,
        -((1 << 53) - 1),
        1 << 53,
    ] {
        let f = BigFloat::from_i64(v);
        let d = f.round_to_format(Format::Double, Rounding::Nearest);
        assert_eq!(d, BigFloat::from_f64(v as f64));
        // Full-precision Display cross-check against the exact integer.
        let i = Integer::from(v);
        assert_eq!(d.to_string(), i.to_string());
    }
}
