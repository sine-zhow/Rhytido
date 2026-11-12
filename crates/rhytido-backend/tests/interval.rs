// SPDX-License-Identifier: Apache-2.0
//! Module-test layer (AGENTS.md §4): public API of `src/interval.rs`.
//!
//! External-crate view of `Interval`, migrated from the former inline test
//! module with names and assertions preserved. Four categories: normal /
//! extreme / error / robustness.

use rhytido_backend::{BigFloat, Interval};

fn bf(v: i64) -> BigFloat {
    BigFloat::from_i64(v)
}

// ---- Normal cases -----------------------------------------------------

#[test]
fn normal_construct_and_accessors() {
    let i = Interval::new(bf(1), bf(3));
    assert!(i.is_bounded());
    assert!(!i.is_nan());
    assert!(!i.is_empty());
    assert_eq!(i.lower().unwrap(), bf(1));
    assert_eq!(i.upper().unwrap(), bf(3));
    assert!(i.contains(&bf(2)));
    assert!(!i.contains(&bf(4)));
    assert_eq!(i.to_string(), "[1, 3]");
}

#[test]
fn normal_point() {
    let p = Interval::point(bf(5));
    assert!(p.is_point());
    assert!(p.contains(&bf(5)));
    assert!(!p.contains(&bf(6)));
}

#[test]
fn normal_add_sub() {
    let a = Interval::new(bf(1), bf(2));
    let b = Interval::new(bf(3), bf(4));
    let s = a.add(&b); // [4, 6]
    assert_eq!(s.lower().unwrap(), bf(4));
    assert_eq!(s.upper().unwrap(), bf(6));
    let d = b.sub(&a); // [1, 3]
    assert_eq!(d.lower().unwrap(), bf(1));
    assert_eq!(d.upper().unwrap(), bf(3));
}

#[test]
fn normal_mul() {
    let a = Interval::new(bf(1), bf(2));
    let b = Interval::new(bf(3), bf(4));
    let m = a.mul(&b); // [3, 8]
    assert_eq!(m.lower().unwrap(), bf(3));
    assert_eq!(m.upper().unwrap(), bf(8));
    // Negative range crosses sign.
    let c = Interval::new(bf(-2), bf(3));
    let m2 = c.mul(&b); // [-8, 12]
    assert_eq!(m2.lower().unwrap(), bf(-8));
    assert_eq!(m2.upper().unwrap(), bf(12));
}

#[test]
fn normal_div() {
    let a = Interval::new(bf(6), bf(8));
    let b = Interval::new(bf(2), bf(4));
    let q = a.div(&b); // [6/4, 8/2] = [1.5, 4]
    assert!(q.contains(&BigFloat::from_f64(1.5)));
    assert!(q.contains(&bf(4)));
    assert!(!q.contains(&bf(5)));
}

// ---- Extreme cases ----------------------------------------------------

#[test]
fn extreme_outward_rounding_contains() {
    // 1/3 endpoints must round outward so the true value is contained.
    let third = BigFloat::one() / bf(3);
    let i = Interval::new(third.clone(), third.clone());
    assert!(i.contains(&third));
    assert!(i.lower().unwrap() <= third);
    assert!(i.upper().unwrap() >= third);
}

#[test]
fn extreme_unbounded_endpoints() {
    let i = Interval::new(BigFloat::neg_infinity(), bf(0));
    assert!(i.is_bounded());
    assert!(i.contains(&bf(-1_000_000)));
    assert!(i.contains(&bf(0)));
    assert!(!i.contains(&bf(1)));
    let j = Interval::new(bf(0), BigFloat::infinity());
    assert!(j.contains(&bf(1_000_000)));
}

#[test]
fn extreme_large_interval_arithmetic() {
    let big = bf(1_000_000_000);
    let a = Interval::new(big.clone(), big.clone());
    let s = a.add(&a);
    assert!(s.contains(&bf(2_000_000_000)));
}

// ---- Error cases (异常报错) -------------------------------------------

#[test]
fn error_nan_and_empty() {
    assert!(Interval::new(BigFloat::NAN, bf(1)).is_nan());
    assert!(Interval::new(bf(1), BigFloat::NAN).is_nan());
    assert!(Interval::new(bf(3), bf(1)).is_empty()); // lo > hi
    assert!(Interval::empty().is_empty());
    assert!(!Interval::empty().contains(&bf(0)));
    assert!(Interval::NAN.lower().is_none());
    assert!(Interval::empty().upper().is_none());
}

#[test]
fn error_div_by_zero_containing() {
    let a = Interval::new(bf(1), bf(2));
    let z = Interval::new(bf(-1), bf(1)); // contains 0
    assert!(a.div(&z).is_nan());
    let zp = Interval::point(bf(0));
    assert!(a.div(&zp).is_nan());
}

#[test]
fn error_nan_propagation() {
    let a = Interval::new(bf(1), bf(2));
    let nan = Interval::NAN;
    assert!(a.add(&nan).is_nan());
    assert!(nan.sub(&a).is_nan());
    assert!(a.mul(&nan).is_nan());
    assert!(nan.div(&a).is_nan());
    assert!(a.add(&Interval::empty()).is_empty());
}

// ---- Interruption / robustness cases (异常中断) -----------------------

#[test]
fn robustness_special_combinations_never_panic() {
    let vals = [
        Interval::NAN,
        Interval::empty(),
        Interval::point(bf(0)),
        Interval::new(bf(-1), bf(1)),
        Interval::new(bf(2), bf(3)),
        Interval::new(BigFloat::neg_infinity(), BigFloat::infinity()),
    ];
    for a in &vals {
        for b in &vals {
            let _ = a.add(b);
            let _ = a.sub(b);
            let _ = a.mul(b);
            let _ = a.div(b);
        }
    }
}

#[test]
fn robustness_contains_nan_false() {
    let i = Interval::new(bf(0), bf(1));
    assert!(!i.contains(&BigFloat::NAN));
    assert!(!Interval::NAN.contains(&bf(0)));
}

// ---- M2-A helpers --------------------------------------------------------

fn bff(v: f64) -> BigFloat {
    BigFloat::from_f64(v)
}

fn close(a: &BigFloat, b: &BigFloat, bits: usize) -> bool {
    // Compare at a reduced precision to tolerate last-bit rounding.
    a.with_precision(bits) == b.with_precision(bits)
}

fn pow2(n: u32) -> BigFloat {
    let mut acc = BigFloat::one();
    for _ in 0..n {
        acc = acc * bf(2);
    }
    acc
}

/// Dense-sampling containment property: every correctly-rounded point value
/// at the samples must lie inside the interval result. Samples are clamped to
/// the query interval so arithmetic rounding cannot push one outside.
fn assert_contains_samples(
    interval: &Interval,
    result: &Interval,
    f: impl Fn(&BigFloat) -> BigFloat,
    n: usize,
) {
    let lo = interval.lower().unwrap();
    let hi = interval.upper().unwrap();
    let span = hi.clone() - lo.clone();
    for i in 0..=n {
        let t = bf(i as i64) / bf(n as i64);
        let mut x = lo.clone() + span.clone() * t;
        if x < lo {
            x = lo.clone();
        }
        if x > hi {
            x = hi.clone();
        }
        let y = f(&x);
        assert!(result.contains(&y), "sample {i}: {y} outside {result}");
    }
}

// ---- M2-A function family: normal cases ---------------------------------

#[test]
fn normal_interval_log2_log10() {
    let p = Interval::point(bf(8));
    let r = p.log2();
    assert_eq!(r.lower().unwrap(), bf(3));
    assert_eq!(r.upper().unwrap(), bf(3));
    let i = Interval::new(bf(1), bf(1000));
    let r = i.log10();
    assert!(r.contains(&bf(0)));
    assert!(r.contains(&bf(3)));
    assert!(!r.contains(&bff(3.5)));
    assert_contains_samples(&i, &r, |x| x.log10(), 32);
    let i2 = Interval::new(bf(2), bf(16));
    let r2 = i2.log2();
    assert!(r2.contains(&bf(1)));
    assert!(r2.contains(&bf(4)));
    assert_contains_samples(&i2, &r2, |x| x.log2(), 32);
}

#[test]
fn normal_interval_log1p_expm1() {
    let i = Interval::new(bff(0.0), bff(1.0));
    let r = i.log1p();
    assert!(r.contains(&bff(0.0)));
    assert!(r.contains(&bff(2.0).ln()));
    assert_contains_samples(&i, &r, |x| x.log1p(), 32);
    let j = Interval::new(bff(-0.5), bff(0.5));
    let s = j.expm1();
    assert_contains_samples(&j, &s, |x| x.expm1(), 32);
    // Composition: expm1(log1p([0,1])) tracks [0,1]. Endpoint agreement is
    // checked at reduced precision: the M0 endpoint paradigm rounds nearest
    // before outward rounding, so a composition can lose a last bit at the
    // exact endpoint (fully directed kernels arrive in M2-C). Single-function
    // containment above remains exact at full precision.
    let rt = r.expm1();
    assert!(rt.contains(&bff(0.0)));
    assert!(rt.contains(&bff(0.5)));
    assert!(close(&rt.lower().unwrap(), &BigFloat::zero(), 150));
    assert!(close(&rt.upper().unwrap(), &BigFloat::one(), 150));
}

#[test]
fn normal_interval_abs() {
    let pos = Interval::new(bf(2), bf(5));
    let r = pos.abs();
    assert_eq!(r.lower().unwrap(), bf(2));
    assert_eq!(r.upper().unwrap(), bf(5));
    let neg = Interval::new(bf(-5), bf(-2));
    let r = neg.abs();
    assert_eq!(r.lower().unwrap(), bf(2));
    assert_eq!(r.upper().unwrap(), bf(5));
    let cross = Interval::new(bf(-3), bf(5));
    let r = cross.abs();
    assert_eq!(r.lower().unwrap(), bf(0));
    assert_eq!(r.upper().unwrap(), bf(5));
    let cross2 = Interval::new(bf(-7), bf(3));
    let r2 = cross2.abs();
    assert_eq!(r2.lower().unwrap(), bf(0));
    assert_eq!(r2.upper().unwrap(), bf(7));
    assert_contains_samples(&cross, &r, |x| x.abs(), 32);
    assert_contains_samples(&cross2, &r2, |x| x.abs(), 32);
    // -0 as the lower endpoint normalizes to +0.
    let nz = Interval::new(-BigFloat::zero(), bf(5));
    assert!(nz.abs().lower().unwrap().is_sign_positive());
}

#[test]
fn normal_interval_ceil_floor_nearestint() {
    let i = Interval::new(bff(2.1), bff(3.7));
    let c = i.ceil();
    assert_eq!(c.lower().unwrap(), bf(3));
    assert_eq!(c.upper().unwrap(), bf(4));
    let f = i.floor();
    assert_eq!(f.lower().unwrap(), bf(2));
    assert_eq!(f.upper().unwrap(), bf(3));
    let n = i.nearestint();
    assert_eq!(n.lower().unwrap(), bf(2));
    assert_eq!(n.upper().unwrap(), bf(4));
    let neg = Interval::new(bff(-3.7), bff(-2.1));
    assert_eq!(neg.ceil().lower().unwrap(), bf(-3));
    assert_eq!(neg.ceil().upper().unwrap(), bf(-2));
    assert_eq!(neg.floor().lower().unwrap(), bf(-4));
    assert_eq!(neg.floor().upper().unwrap(), bf(-3));
    assert_eq!(neg.nearestint().lower().unwrap(), bf(-4));
    assert_eq!(neg.nearestint().upper().unwrap(), bf(-2));
    // Integer intervals are fixed points.
    let ints = Interval::new(bf(2), bf(5));
    assert_eq!(ints.ceil().lower().unwrap(), bf(2));
    assert_eq!(ints.floor().upper().unwrap(), bf(5));
    assert_contains_samples(&i, &c, |x| x.ceil(), 32);
    assert_contains_samples(&i, &f, |x| x.floor(), 32);
    assert_contains_samples(&i, &n, |x| x.nearestint(), 32);
}

#[test]
fn normal_interval_sin_cos_monotone_branch() {
    let i = Interval::new(bff(0.0), bff(1.0));
    let r = i.sin();
    assert!(r.contains(&bff(0.0)));
    assert!(r.contains(&bff(1.0).sin()));
    assert!(!r.contains(&bff(1.0))); // sin(1) < 1.
    assert_contains_samples(&i, &r, |x| x.sin(), 32);
    let j = Interval::new(bff(-1.0), bff(0.0));
    let c = j.cos();
    assert!(c.contains(&BigFloat::one())); // cos(0) = 1 at the endpoint.
    assert_contains_samples(&j, &c, |x| x.cos(), 32);
}

// ---- M2-A function family: extreme cases --------------------------------

#[test]
fn extreme_interval_sin_extrema() {
    let pi = BigFloat::pi_with_precision(165);
    let half_pi = pi.clone() / bf(2);
    // Neighborhood of the maximum at pi/2.
    let i = Interval::new(half_pi.clone() - bff(0.01), half_pi.clone() + bff(0.01));
    let r = i.sin();
    assert_eq!(r.upper().unwrap(), BigFloat::one());
    assert!(r.lower().unwrap() < BigFloat::one());
    assert_contains_samples(&i, &r, |x| x.sin(), 48);
    // Neighborhood of the minimum at -pi/2.
    let neg_half_pi = -half_pi.clone();
    let j = Interval::new(neg_half_pi.clone() - bff(0.01), neg_half_pi + bff(0.01));
    let s = j.sin();
    assert_eq!(s.lower().unwrap(), -BigFloat::one());
    assert_contains_samples(&j, &s, |x| x.sin(), 48);
    // Point interval at the (dyadic) maximum: sin rounds to 1.
    let p = Interval::point(half_pi.clone());
    assert!(p.sin().contains(&BigFloat::one()));
    // Large-k extremum: pi/2 + 20*pi.
    let far = half_pi + pi * bf(20);
    let k = Interval::new(far.clone() - bff(0.01), far + bff(0.01));
    let rk = k.sin();
    assert_eq!(rk.upper().unwrap(), BigFloat::one());
    assert_contains_samples(&k, &rk, |x| x.sin(), 48);
}

#[test]
fn extreme_interval_cos_extrema() {
    let pi = BigFloat::pi_with_precision(165);
    // Maximum at 0.
    let i = Interval::new(bff(-0.1), bff(0.1));
    let r = i.cos();
    assert_eq!(r.upper().unwrap(), BigFloat::one());
    assert_contains_samples(&i, &r, |x| x.cos(), 48);
    // Minimum at pi.
    let j = Interval::new(pi.clone() - bff(0.1), pi.clone() + bff(0.1));
    let s = j.cos();
    assert_eq!(s.lower().unwrap(), -BigFloat::one());
    assert_contains_samples(&j, &s, |x| x.cos(), 48);
    // Maximum at 2*pi (k = 1), minimum at -pi (k = -1).
    let two_pi = pi.clone() * bf(2);
    let k = Interval::new(two_pi.clone() - bff(0.1), two_pi + bff(0.1));
    assert_eq!(k.cos().upper().unwrap(), BigFloat::one());
    let m = Interval::new(-pi.clone() - bff(0.1), -pi + bff(0.1));
    assert_eq!(m.cos().lower().unwrap(), -BigFloat::one());
    assert_contains_samples(&k, &k.cos(), |x| x.cos(), 48);
    assert_contains_samples(&m, &m.cos(), |x| x.cos(), 48);
}

#[test]
fn extreme_interval_sin_cos_full_period() {
    let cases = [
        Interval::new(bf(0), bff(7.0)),
        Interval::new(bf(-4), bf(4)),
        Interval::new(bf(-1000), bf(1000)),
        Interval::new(BigFloat::neg_infinity(), bf(5)),
        Interval::new(bf(0), BigFloat::infinity()),
        Interval::new(BigFloat::neg_infinity(), BigFloat::infinity()),
    ];
    for i in &cases {
        let s = i.sin();
        assert_eq!(s.lower().unwrap(), -BigFloat::one());
        assert_eq!(s.upper().unwrap(), BigFloat::one());
        let c = i.cos();
        assert_eq!(c.lower().unwrap(), -BigFloat::one());
        assert_eq!(c.upper().unwrap(), BigFloat::one());
    }
    // Just over a full period, anchored at pi/2.
    let pi = BigFloat::pi_with_precision(165);
    let half_pi = pi.clone() / bf(2);
    let i = Interval::new(half_pi.clone(), half_pi + pi * bf(2) + bff(0.1));
    assert_eq!(i.sin().lower().unwrap(), -BigFloat::one());
    assert_eq!(i.sin().upper().unwrap(), BigFloat::one());
}

#[test]
fn extreme_interval_tan_poles() {
    let pi = BigFloat::pi_with_precision(165);
    let half_pi = pi.clone() / bf(2);
    let crossing = Interval::new(half_pi.clone() - bff(0.1), half_pi.clone() + bff(0.1));
    let r = crossing.tan();
    assert!(r.lower().unwrap().is_infinite());
    assert!(r.lower().unwrap().is_sign_negative());
    assert!(r.upper().unwrap().is_infinite());
    assert!(r.upper().unwrap().is_sign_positive());
    // Pole at -pi/2 (k = -1) and at pi/2 + pi (k = 1).
    let neg = Interval::new(-half_pi.clone() - bff(0.1), -half_pi.clone() + bff(0.1));
    assert!(neg.tan().lower().unwrap().is_infinite());
    let far = Interval::new(
        half_pi.clone() + pi.clone() - bff(0.1),
        half_pi + pi + bff(0.1),
    );
    assert!(far.tan().upper().unwrap().is_infinite());
    // Unbounded endpoints always contain poles.
    let unb = Interval::new(bf(0), BigFloat::infinity());
    assert!(unb.tan().lower().unwrap().is_infinite());
}

#[test]
fn extreme_interval_tan_monotone() {
    let i = Interval::new(bff(-1.0), bff(1.0));
    let r = i.tan();
    assert!(r.lower().unwrap().is_finite());
    assert!(r.upper().unwrap().is_finite());
    assert!(r.contains(&bff(1.0).tan()));
    assert!(r.contains(&bff(-1.0).tan()));
    assert_contains_samples(&i, &r, |x| x.tan(), 48);
    // Branch just below the pole: large but finite values.
    let pi = BigFloat::pi_with_precision(165);
    let half_pi = pi / bf(2);
    let near = Interval::new(half_pi.clone() - bff(0.001), half_pi - bff(0.0005));
    let rn = near.tan();
    assert!(rn.lower().unwrap().is_finite());
    assert!(rn.upper().unwrap().is_finite());
    assert!(rn.upper().unwrap() > bf(100));
    assert_contains_samples(&near, &rn, |x| x.tan(), 48);
}

#[test]
fn extreme_interval_unbounded_new_functions() {
    let i = Interval::new(BigFloat::neg_infinity(), BigFloat::infinity());
    let a = i.abs();
    assert!(a.lower().unwrap().is_zero());
    assert!(a.upper().unwrap().is_infinite());
    let l = Interval::new(bf(0), BigFloat::infinity()).log2();
    assert!(l.lower().unwrap().is_infinite());
    assert!(l.lower().unwrap().is_sign_negative());
    assert!(l.upper().unwrap().is_infinite());
    let e = Interval::new(BigFloat::neg_infinity(), bf(0)).expm1();
    assert_eq!(e.lower().unwrap(), -BigFloat::one());
    assert!(e.upper().unwrap().is_zero());
    let lp = Interval::new(bf(-1), BigFloat::infinity()).log1p();
    assert!(lp.lower().unwrap().is_infinite());
    assert!(lp.lower().unwrap().is_sign_negative());
}

#[test]
fn extreme_interval_point_intervals() {
    for v in [0.25f64, 0.5, 1.0, 2.5, 7.0] {
        let x = bff(v);
        let p = Interval::point(x.clone());
        assert!(p.log2().contains(&x.log2()));
        assert!(p.log10().contains(&x.log10()));
        assert!(p.log1p().contains(&x.log1p()));
        assert!(p.expm1().contains(&x.expm1()));
        assert!(p.sin().contains(&x.sin()));
        assert!(p.cos().contains(&x.cos()));
        assert!(p.tan().contains(&x.tan()));
        assert!(p.abs().contains(&x.abs()));
        assert!(p.ceil().contains(&x.ceil()));
        assert!(p.floor().contains(&x.floor()));
        assert!(p.nearestint().contains(&x.nearestint()));
    }
    // Negative point for the sign-sensitive functions.
    let x = bff(-2.5);
    let p = Interval::point(x.clone());
    assert!(p.abs().contains(&x.abs()));
    assert!(p.ceil().contains(&x.ceil()));
    assert!(p.floor().contains(&x.floor()));
    assert!(p.nearestint().contains(&x.nearestint()));
    assert!(p.sin().contains(&x.sin()));
    assert!(p.cos().contains(&x.cos()));
    assert!(p.tan().contains(&x.tan()));
}

#[test]
fn extreme_interval_narrow_near_extrema() {
    // Ultra-narrow intervals stress the containment margins: sample values
    // sit within a few ulps of the endpoint images.
    let pi = BigFloat::pi_with_precision(165);
    let half_pi = pi / bf(2);
    let delta = BigFloat::one() / pow2(160);
    let i = Interval::new(half_pi.clone() - delta.clone(), half_pi + delta.clone());
    let r = i.sin();
    assert_eq!(r.upper().unwrap(), BigFloat::one());
    assert_contains_samples(&i, &r, |x| x.sin(), 32);
    let j = Interval::new(bff(0.3) - delta.clone(), bff(0.3) + delta.clone());
    assert_contains_samples(&j, &j.sin(), |x| x.sin(), 32);
    assert_contains_samples(&j, &j.cos(), |x| x.cos(), 32);
    let k = Interval::new(bff(1.2) - delta.clone(), bff(1.2) + delta);
    assert_contains_samples(&k, &k.tan(), |x| x.tan(), 32);
}

// ---- M2-A function family: error cases (异常报错) ------------------------

#[test]
fn error_interval_new_functions_nan_empty() {
    for f in [
        Interval::sin,
        Interval::cos,
        Interval::tan,
        Interval::log2,
        Interval::log10,
        Interval::log1p,
        Interval::expm1,
        Interval::abs,
        Interval::ceil,
        Interval::floor,
        Interval::nearestint,
    ] {
        assert!(f(&Interval::NAN).is_nan());
        assert!(f(&Interval::empty()).is_empty());
    }
}

#[test]
fn error_interval_log_domain() {
    assert!(Interval::new(bf(-2), bf(-1)).log2().is_nan());
    assert!(Interval::new(bf(-2), bf(-1)).log10().is_nan());
    assert!(Interval::new(bf(-1), bf(1)).log2().is_nan()); // Crosses zero.
    assert!(Interval::new(bf(-2), bf(0)).log1p().is_nan());
    // The pole itself maps to -infinity, not NaN.
    let r = Interval::new(bf(-1), bf(0)).log1p();
    assert!(r.lower().unwrap().is_infinite());
    assert!(r.lower().unwrap().is_sign_negative());
    assert!(r.upper().unwrap().is_zero());
    // expm1 is entire: no domain errors.
    assert!(Interval::new(bf(-100), bf(100)).expm1().is_bounded());
}

// ---- M2-A function family: robustness (异常中断) -------------------------

#[test]
fn robustness_interval_new_functions_never_panic() {
    let pi = BigFloat::pi_with_precision(165);
    let half_pi = pi / bf(2);
    let vals = [
        Interval::NAN,
        Interval::empty(),
        Interval::point(bf(0)),
        Interval::point(-bf(1)),
        Interval::new(bf(-1), bf(1)),
        Interval::new(bf(2), bf(3)),
        Interval::new(BigFloat::neg_infinity(), BigFloat::infinity()),
        Interval::new(half_pi.clone() - bff(0.1), half_pi + bff(0.1)),
    ];
    for a in &vals {
        let _ = a.sin();
        let _ = a.cos();
        let _ = a.tan();
        let _ = a.log2();
        let _ = a.log10();
        let _ = a.log1p();
        let _ = a.expm1();
        let _ = a.abs();
        let _ = a.ceil();
        let _ = a.floor();
        let _ = a.nearestint();
    }
}

#[test]
fn robustness_interval_sin_containment_property() {
    let pi = BigFloat::pi_with_precision(165);
    let half_pi = pi.clone() / bf(2);
    let far = half_pi.clone() + bf(20) * pi.clone();
    let intervals = [
        Interval::new(bff(0.0), bff(1.0)),
        Interval::new(bff(-1.0), bff(1.0)),
        Interval::new(half_pi.clone() - bff(0.2), half_pi.clone() + bff(0.2)),
        Interval::new(-half_pi.clone() - bff(0.2), -half_pi + bff(0.2)),
        Interval::new(pi.clone() - bff(0.5), pi.clone() + bff(0.5)),
        Interval::new(bf(0), bff(7.0)),
        Interval::new(bf(-20), bf(20)),
        Interval::new(far.clone() - bff(0.05), far + bff(0.05)),
        Interval::new(bff(0.3), bff(0.3000001)),
    ];
    for i in &intervals {
        let r = i.sin();
        assert_contains_samples(i, &r, |x| x.sin(), 48);
    }
}

#[test]
fn robustness_interval_cos_containment_property() {
    let pi = BigFloat::pi_with_precision(165);
    let intervals = [
        Interval::new(bff(-0.2), bff(0.2)),
        Interval::new(pi.clone() - bff(0.2), pi.clone() + bff(0.2)),
        Interval::new(-pi.clone() - bff(0.2), -pi.clone() + bff(0.2)),
        Interval::new(bf(2) * pi.clone() - bff(0.2), bf(2) * pi.clone() + bff(0.2)),
        Interval::new(bff(0.0), bff(6.5)),
        Interval::new(bf(-10), bf(10)),
        Interval::new(bf(50), bff(50.5)),
        Interval::new(bff(-0.3), bff(-0.2999999)),
    ];
    for i in &intervals {
        let r = i.cos();
        assert_contains_samples(i, &r, |x| x.cos(), 48);
    }
}

#[test]
fn robustness_interval_tan_containment_property() {
    let pi = BigFloat::pi_with_precision(165);
    let half_pi = pi.clone() / bf(2);
    // Monotone branches: dense sampling must stay inside.
    let branches = [
        Interval::new(bff(-1.0), bff(1.0)),
        Interval::new(bff(0.0), half_pi.clone() - bff(0.01)),
        Interval::new(half_pi.clone() + bff(0.01), half_pi.clone() + bff(1.0)),
        Interval::new(bf(3), bf(4)),
        Interval::new(
            half_pi.clone() - bff(0.0001),
            half_pi.clone() - bff(0.00005),
        ),
    ];
    for i in &branches {
        let r = i.tan();
        assert!(r.lower().unwrap().is_finite());
        assert!(r.upper().unwrap().is_finite());
        assert_contains_samples(i, &r, |x| x.tan(), 48);
    }
    // Pole crossings: the result must be the whole line.
    let crossings = [
        Interval::new(half_pi.clone() - bff(0.1), half_pi.clone() + bff(0.1)),
        Interval::new(-half_pi.clone() - bff(0.1), -half_pi.clone() + bff(0.1)),
        Interval::new(bf(0), bff(4.0)),
        Interval::new(
            half_pi.clone() + pi.clone() - bff(0.1),
            half_pi + pi + bff(0.1),
        ),
    ];
    for i in &crossings {
        let r = i.tan();
        assert!(r.lower().unwrap().is_infinite());
        assert!(r.upper().unwrap().is_infinite());
    }
}

#[test]
fn robustness_interval_abs_containment_property() {
    let intervals = [
        Interval::new(bf(-5), bf(3)),
        Interval::new(bf(-3), bf(5)),
        Interval::new(bf(-7), bf(-2)),
        Interval::new(bf(2), bf(7)),
        Interval::new(-BigFloat::zero(), bf(5)),
        Interval::new(bf(-5), -BigFloat::zero()),
    ];
    for i in &intervals {
        let r = i.abs();
        assert_contains_samples(i, &r, |x| x.abs(), 32);
    }
    // Unbounded: probe finite points directly (sampling cannot span infinity).
    let u = Interval::new(BigFloat::neg_infinity(), bf(2)).abs();
    assert!(u.contains(&bf(0)));
    assert!(u.contains(&bf(100)));
    assert!(!u.contains(&bf(-1)));
}

#[test]
fn robustness_interval_monotone_containment_property() {
    type Ifn = fn(&Interval) -> Interval;
    type Bfn = fn(&BigFloat) -> BigFloat;
    let cases: [(Interval, Ifn, Bfn); 7] = [
        (
            Interval::new(bff(0.1), bff(10.0)),
            Interval::log2,
            BigFloat::log2,
        ),
        (
            Interval::new(bff(0.1), bff(10.0)),
            Interval::log10,
            BigFloat::log10,
        ),
        (
            Interval::new(bff(-0.9), bff(5.0)),
            Interval::log1p,
            BigFloat::log1p,
        ),
        (
            Interval::new(bff(-5.0), bff(5.0)),
            Interval::expm1,
            BigFloat::expm1,
        ),
        (
            Interval::new(bff(-3.7), bff(3.7)),
            Interval::ceil,
            BigFloat::ceil,
        ),
        (
            Interval::new(bff(-3.7), bff(3.7)),
            Interval::floor,
            BigFloat::floor,
        ),
        (
            Interval::new(bff(-3.7), bff(3.7)),
            Interval::nearestint,
            BigFloat::nearestint,
        ),
    ];
    for (i, ifn, bfn) in &cases {
        let r = ifn(i);
        assert_contains_samples(i, &r, *bfn, 32);
    }
}

#[test]
fn robustness_interval_trig_coarse_pi_anchor() {
    // Intervals anchored at a coarse pi must still detect the true extrema:
    // the detection runs at the endpoint precision plus guard, not the
    // anchor's precision.
    for prec in [10usize, 20, 53] {
        let pi = BigFloat::pi_with_precision(prec);
        let half_pi = pi.clone() / bf(2);
        let i = Interval::new(half_pi.clone() - bff(0.1), half_pi.clone() + bff(0.1));
        let r = i.sin();
        assert_eq!(r.upper().unwrap(), BigFloat::one());
        assert_contains_samples(&i, &r, |x| x.sin(), 32);
        let j = Interval::new(pi.clone() - bff(0.1), pi.clone() + bff(0.1));
        let c = j.cos();
        assert_eq!(c.lower().unwrap(), -BigFloat::one());
        assert_contains_samples(&j, &c, |x| x.cos(), 32);
    }
}
