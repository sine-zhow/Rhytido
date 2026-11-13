// SPDX-License-Identifier: Apache-2.0
//! Module-test layer (AGENTS.md §4 / 05-testing.md §1): the public API of
//! `src/erf.rs` — `BigFloat::{erf, erfc, erf_checked, erfc_checked}` and
//! `Interval::{erf, erfc}`. External-crate view, four categories (normal /
//! extreme / error / robustness). Helpers are free functions: integration tests
//! are separate crates and cannot add inherent impls to foreign types.

use rhytido_backend::{BigFloat, DEFAULT_PRECISION, Interval, Rounding};

// ---- Helpers ----------------------------------------------------------

/// Parses a decimal literal at the default working precision.
fn decimal(s: &str) -> BigFloat {
    s.parse::<BigFloat>().unwrap()
}

/// Exact `2^e` by repeated doubling/halving, so no `exp`/`pow` is involved.
fn pow2_exact(e: i64) -> BigFloat {
    let two = BigFloat::from_i64(2);
    let mut r = BigFloat::one();
    if e >= 0 {
        for _ in 0..e {
            r = r * two.clone();
        }
    } else {
        for _ in 0..(-e) {
            r = r / two.clone();
        }
    }
    r
}

/// `floor(log2 |v|)` for a non-zero finite `v`.
fn binade_exponent(v: &BigFloat) -> i64 {
    let two = BigFloat::from_i64(2);
    let one = BigFloat::one();
    let mut m = v.abs();
    let mut e = 0i64;
    while m >= two {
        m = m / two.clone();
        e += 1;
    }
    while m < one {
        m = m * two.clone();
        e -= 1;
    }
    e
}

/// One ulp of `v` at its own working precision (non-zero finite `v`).
fn ulp_p(v: &BigFloat) -> BigFloat {
    let p = v.precision() as i64;
    pow2_exact(binade_exponent(v) + 1 - p)
}

/// Asserts agreement with a 15-significant-digit public constant.
///
/// The constants below are table values rounded to 15 digits, so the exact
/// value differs from the constant by up to half an ulp of the 15th digit
/// (`5e-16`). They are therefore NOT expected to fall inside the far tighter
/// certified bracket of `*_checked`; this helper checks the value instead.
fn assert_15(v: &BigFloat, s: &str) {
    let c = decimal(s);
    let d = (v.clone() - c).abs();
    let tol = BigFloat::from_f64(1e-15);
    assert!(d <= tol, "value {v} vs constant {s}: diff {d} > {tol}");
}

/// Linear sample points across `[lo, hi]` inclusive.
fn samples_between(lo: &BigFloat, hi: &BigFloat, n: i64) -> Vec<BigFloat> {
    let span = hi.clone() - lo.clone();
    (0..=n)
        .map(|k| {
            let frac = BigFloat::from_i64(k) / BigFloat::from_i64(n);
            lo.clone() + span.clone() * frac
        })
        .collect()
}

/// Rigorous containment of a sample's certified bracket inside an interval
/// image.
///
/// The image of `[lo, hi]` covers the true `f(x)` for every `x` inside, and
/// `[v-e, v+e]` contains that true value, so the two must overlap: the image
/// cannot start above `v+e` nor end below `v-e`.
fn assert_image_covers(img: &Interval, x: &BigFloat, erfc: bool) {
    let (v, e) = if erfc {
        x.erfc_checked()
    } else {
        x.erf_checked()
    };
    let lo = img.lower().expect("bounded image");
    let hi = img.upper().expect("bounded image");
    let name = if erfc { "erfc" } else { "erf" };
    assert!(
        lo <= v.clone() + e.clone(),
        "{name} image low {lo} above sample bracket {v} +/- {e} at {x}"
    );
    assert!(
        v - e <= hi,
        "{name} image high {hi} below sample bracket at {x}"
    );
}

// ---- Normal: public constants (DLMF / A&S tables, 15 digits) ----------

#[test]
fn normal_erf_public_constants() {
    assert_15(&BigFloat::from_f64(1.0).erf(), "0.842700792949715");
    assert_15(&BigFloat::from_f64(0.5).erf(), "0.520499877813047");
    assert_15(&BigFloat::from_f64(2.0).erf(), "0.995322265018953");
    assert_15(&BigFloat::from_f64(3.0).erf(), "0.999977909503001");
    assert_15(&BigFloat::from_f64(4.0).erf(), "0.999999984582742");
}

#[test]
fn normal_erfc_public_constant() {
    assert_15(&BigFloat::from_f64(1.0).erfc(), "0.157299207050285");
}

#[test]
fn normal_erf_odd_symmetry() {
    // erf(-x) == -erf(x) bitwise: the kernel brackets erf(|x|) and flips the
    // sign exactly, and both sides are correctly rounded.
    for x in [0.25f64, 0.5, 1.0, 1.7, 2.0, 3.5] {
        let pos = BigFloat::from_f64(x);
        let neg = -pos.clone();
        let p = pos.erf();
        assert_eq!(neg.erf(), -p.clone(), "odd symmetry at {x}");
        assert_eq!(
            neg.erf().canonical_parts(),
            (-p).canonical_parts(),
            "bitwise odd symmetry at {x}"
        );
    }
}

#[test]
fn normal_erf_signed_zero() {
    let pz = BigFloat::zero();
    let nz = -BigFloat::zero();
    let ep = pz.erf();
    let en = nz.erf();
    assert!(ep.is_zero() && ep.is_sign_positive(), "erf(+0) must be +0");
    assert!(en.is_zero() && en.is_sign_negative(), "erf(-0) must be -0");
    assert_eq!(pz.erfc(), BigFloat::one());
    assert_eq!(nz.erfc(), BigFloat::one());
}

#[test]
fn normal_erf_plus_erfc_is_one_within_bounds() {
    for x in [0.1f64, 0.5, 1.0, 2.0, 3.0, 5.0, -1.0, -2.5] {
        let xv = BigFloat::from_f64(x);
        let (e, eb) = xv.erf_checked();
        let (c, cb) = xv.erfc_checked();
        let dev = (e + c - BigFloat::one()).abs();
        let budget = eb + cb;
        assert!(dev <= budget, "x={x}: |erf+erfc-1|={dev} exceeds {budget}");
    }
}

#[test]
fn normal_monotone_sampling() {
    // Dense sweep across zero, well inside the p=165 threshold (x^2 >= 167).
    let xs = samples_between(&BigFloat::from_f64(-8.0), &BigFloat::from_f64(8.0), 48);
    let mut prev: Option<(BigFloat, BigFloat)> = None;
    for x in &xs {
        let pair = (x.erf(), x.erfc());
        if let Some((pe, pc)) = &prev {
            assert!(*pe <= pair.0, "erf not monotone at {x}");
            assert!(*pc >= pair.1, "erfc not monotone at {x}");
        }
        prev = Some(pair);
    }
}

#[test]
fn normal_monotone_across_threshold() {
    // Both sides of the p=165 large-argument threshold x^2 >= 167 (x ~ 12.92):
    // below it the series runs, at/above it the exact limits are returned, and
    // monotonicity must survive the switch of code path.
    let vals = [12.0f64, 12.5, 12.9, 12.92, 12.93, 13.0, 14.0, 20.0];
    let mut prev: Option<(BigFloat, BigFloat)> = None;
    for x in vals {
        let xv = BigFloat::from_f64(x);
        let pair = (xv.erf(), xv.erfc());
        if let Some((pe, pc)) = &prev {
            assert!(*pe <= pair.0, "erf not monotone across threshold at {x}");
            assert!(*pc >= pair.1, "erfc not monotone across threshold at {x}");
        }
        prev = Some(pair);
    }
}

#[test]
fn normal_checked_bound_is_below_one_ulp() {
    // Certificate self-consistency: whenever correct rounding at the working
    // precision is proven (the escalation loop's exit condition), the reported
    // bound must be finer than one ulp of the returned value.
    for x in [0.5f64, 1.0, 2.0, 3.0, 5.0, 8.0, -1.0, -4.0] {
        let xv = BigFloat::from_f64(x);
        let (e, eb) = xv.erf_checked();
        let (c, cb) = xv.erfc_checked();
        assert!(eb > BigFloat::zero() && cb > BigFloat::zero());
        assert!(
            eb < ulp_p(&e),
            "erf bound {eb} >= 1 ulp {} at x={x}",
            ulp_p(&e)
        );
        assert!(
            cb < ulp_p(&c),
            "erfc bound {cb} >= 1 ulp {} at x={x}",
            ulp_p(&c)
        );
    }
}

#[test]
fn normal_checked_value_stable_across_working_precisions() {
    // Two independently rounded results must agree at the FULL coarser
    // resolution (both are correctly rounded), which cross-checks the
    // certificate against a backend high-precision reference rather than an
    // external table. After the Display round-trip fix this is no longer
    // bounded by the 15-digit public constants: it verifies every working-
    // precision bit across the whole public-constant value range (a
    // strengthening of the former ≤15-digit cross-check, not a relaxation).
    for x in [0.5f64, 1.0, 2.0, 3.0, 4.0, -1.0, -2.5] {
        let coarse = BigFloat::from_f64(x).with_precision(53).erf();
        let fine = BigFloat::from_f64(x).with_precision(300).erf();
        assert_eq!(
            coarse,
            fine.round_to(53, Rounding::Nearest),
            "erf({x}) inconsistent between p=53 and p=300"
        );
        let ccoarse = BigFloat::from_f64(x).with_precision(53).erfc();
        let cfine = BigFloat::from_f64(x).with_precision(300).erfc();
        assert_eq!(
            ccoarse,
            cfine.round_to(53, Rounding::Nearest),
            "erfc({x}) inconsistent between p=53 and p=300"
        );
    }
}

#[test]
fn normal_interval_erf_dense_containment() {
    for (lo, hi) in [
        (-2.0f64, 2.0),
        (0.0, 1.0),
        (-1.0, 0.0),
        (0.5, 0.5000001),
        (3.0, 4.0),
    ] {
        let a = BigFloat::from_f64(lo);
        let b = BigFloat::from_f64(hi);
        let img = Interval::new(a.clone(), b.clone()).erf();
        assert!(img.is_bounded());
        for x in samples_between(&a, &b, 12) {
            assert_image_covers(&img, &x, false);
        }
    }
}

#[test]
fn normal_interval_erfc_dense_containment() {
    for (lo, hi) in [(-2.0f64, 2.0), (0.0, 1.0), (1.0, 3.0)] {
        let a = BigFloat::from_f64(lo);
        let b = BigFloat::from_f64(hi);
        let img = Interval::new(a.clone(), b.clone()).erfc();
        assert!(img.is_bounded());
        for x in samples_between(&a, &b, 12) {
            assert_image_covers(&img, &x, true);
        }
    }
}

#[test]
fn normal_interval_point_matches_scalar() {
    for x in [-1.5f64, 0.0, 0.75, 2.5] {
        let xv = BigFloat::from_f64(x);
        let pt = Interval::point(xv.clone());
        assert!(
            pt.erf().contains(&xv.erf()),
            "point image must contain erf({x})"
        );
        assert!(
            pt.erfc().contains(&xv.erfc()),
            "point image must contain erfc({x})"
        );
    }
}

// ---- Extreme ----------------------------------------------------------

#[test]
fn extreme_infinity_limits() {
    assert_eq!(BigFloat::infinity().erf(), BigFloat::one());
    assert_eq!(BigFloat::neg_infinity().erf(), -BigFloat::one());
    assert_eq!(BigFloat::infinity().erfc(), BigFloat::zero());
    assert_eq!(BigFloat::neg_infinity().erfc(), BigFloat::from_i64(2));
    // Checked variants: exact limits still carry a positive bound.
    let (v, b) = BigFloat::infinity().erf_checked();
    assert_eq!(v, BigFloat::one());
    assert!(b > BigFloat::zero());
    let (v, b) = BigFloat::neg_infinity().erfc_checked();
    assert_eq!(v, BigFloat::from_i64(2));
    assert!(b > BigFloat::zero());
}

#[test]
fn extreme_large_argument_threshold() {
    // At p=53 the threshold is x^2 >= 55 (x >= 7.42): x=8 returns the exact
    // limits, x=7 stays on the series path with a tiny but representable erfc.
    let p = 53usize;
    let eight = BigFloat::from_f64(8.0).with_precision(p);
    let seven = BigFloat::from_f64(7.0).with_precision(p);
    assert_eq!(eight.erf(), BigFloat::one().with_precision(p));
    assert!(
        eight.erfc().is_zero(),
        "erfc(8) must hit the exact-zero limit"
    );
    assert_eq!(seven.erf(), BigFloat::one().with_precision(p));
    assert!(!seven.erfc().is_zero(), "erfc(7) is tiny but not the limit");
    assert!(seven.erfc() > BigFloat::zero());
    // Symmetric negative side: erfc(-large) rounds to exactly 2.
    assert_eq!((-eight.clone()).erf(), -BigFloat::one().with_precision(p));
    assert_eq!((-eight).erfc(), BigFloat::from_i64(2).with_precision(p));
}

#[test]
fn extreme_threshold_checked_bound() {
    // Beyond the threshold the certificate is the proven strict bound
    // 2^-(p+2) on the omitted erfc.
    let p = 53usize;
    let big = BigFloat::from_f64(9.0).with_precision(p);
    let (_, eb) = big.erf_checked();
    let (_, cb) = big.erfc_checked();
    let limit = pow2_exact(-(p as i64 + 2));
    assert!(
        eb > BigFloat::zero() && eb <= limit,
        "erf bound {eb} > {limit}"
    );
    assert!(
        cb > BigFloat::zero() && cb <= limit,
        "erfc bound {cb} > {limit}"
    );
}

#[test]
fn extreme_tiny_argument_matches_linear_term() {
    // erf(x) ~ 2x/sqrt(pi) as x -> 0, so erfc(1e-8) = 1 - 1.13e-8 and is NOT
    // 1 to 15 digits: check it relationally instead of against a constant.
    let x = BigFloat::from_f64(1e-8);
    let root_pi = BigFloat::pi_with_precision(DEFAULT_PRECISION + 40).sqrt();
    let approx = (BigFloat::from_i64(2) * x.clone()) / root_pi;
    let d = (x.erf() - approx.clone()).abs();
    // The next series term is 2x^3/(3 sqrt(pi)) ~ 4e-25.
    assert!(
        d < BigFloat::from_f64(1e-20),
        "tiny-arg linear term off by {d}"
    );
    let c = x.erfc();
    assert!(c < BigFloat::one() && c > BigFloat::one() - approx * BigFloat::from_i64(2));
}

#[test]
fn extreme_precision_independence() {
    // The 15-digit value is stable across working precisions.
    let c = "0.842700792949715";
    assert_15(&BigFloat::from_f64(1.0).with_precision(53).erf(), c);
    assert_15(&BigFloat::from_f64(1.0).with_precision(200).erf(), c);
    assert_15(&BigFloat::from_f64(1.0).with_precision(500).erf(), c);
    assert_15(
        &BigFloat::from_f64(1.0).with_precision(64).erfc(),
        "0.157299207050285",
    );
}

#[test]
fn extreme_unlimited_precision_operand() {
    // precision() == 0 falls back to DEFAULT_PRECISION without panicking.
    let u = (BigFloat::one() / BigFloat::from_i64(3)).with_precision(0);
    assert_eq!(u.precision(), 0);
    let e = u.erf();
    assert!(e.is_finite() && e > BigFloat::zero());
    // erf(1/3) ~ 0.3626481.
    assert!(
        e > BigFloat::from_f64(0.3626) && e < BigFloat::from_f64(0.3627),
        "erf(1/3) = {e}"
    );
}

#[test]
fn extreme_interval_unbounded_and_cross_zero() {
    let full = Interval::new(BigFloat::neg_infinity(), BigFloat::infinity());
    let img = full.erf();
    assert!(img.contains(&-BigFloat::one()));
    assert!(img.contains(&BigFloat::zero()));
    assert!(img.contains(&BigFloat::one()));
    let cimg = full.erfc();
    assert!(cimg.contains(&BigFloat::zero()));
    assert!(cimg.contains(&BigFloat::from_i64(2)));
    // Cross-zero narrow interval contains erf(0) = 0 and erfc(0) = 1.
    let narrow = Interval::new(BigFloat::from_f64(-1e-9), BigFloat::from_f64(1e-9));
    assert!(narrow.erf().contains(&BigFloat::zero()));
    assert!(narrow.erfc().contains(&BigFloat::one()));
}

#[test]
fn extreme_interval_straddles_threshold() {
    // Interval crossing the p=53 threshold (x ~ 7.42): the image must still
    // cover every sample's certified bracket, including the exact-limit side.
    let p = 53usize;
    let a = BigFloat::from_f64(6.0).with_precision(p);
    let b = BigFloat::from_f64(9.0).with_precision(p);
    let iv = Interval::new(a.clone(), b.clone());
    let eimg = iv.erf();
    let cimg = iv.erfc();
    for x in samples_between(&a, &b, 12) {
        assert_image_covers(&eimg, &x, false);
        assert_image_covers(&cimg, &x, true);
    }
}

// ---- Error (NaN / invalid-domain propagation) -------------------------

#[test]
fn error_nan_propagates_all_apis() {
    let nan = BigFloat::NAN;
    assert!(nan.erf().is_nan());
    assert!(nan.erfc().is_nan());
    let (v, e) = nan.erf_checked();
    assert!(v.is_nan() && e.is_nan());
    let (v, e) = nan.erfc_checked();
    assert!(v.is_nan() && e.is_nan());
    // An interval with a NaN endpoint is NaN, and maps to NaN.
    let iv = Interval::new(nan.clone(), BigFloat::one());
    assert!(iv.is_nan());
    assert!(iv.erf().is_nan());
    assert!(iv.erfc().is_nan());
    assert!(Interval::NAN.erf().is_nan());
    assert!(Interval::NAN.erfc().is_nan());
}

#[test]
fn error_checked_bounds_strictly_positive_for_finite() {
    // Every finite, non-NaN input yields a strictly positive, non-NaN bound:
    // the invariant |true - value| <= bound needs a non-degenerate certificate.
    for x in [0.0f64, 1e-30, 0.5, 1.0, 7.0, 50.0, -3.0, -50.0] {
        let xv = BigFloat::from_f64(x);
        let (_, eb) = xv.erf_checked();
        let (_, cb) = xv.erfc_checked();
        assert!(eb.is_finite() && eb > BigFloat::zero(), "erf bound at {x}");
        assert!(cb.is_finite() && cb > BigFloat::zero(), "erfc bound at {x}");
    }
}

#[test]
fn error_empty_interval_stays_empty() {
    let e = Interval::empty();
    assert!(e.erf().is_empty());
    assert!(e.erfc().is_empty());
    // lo > hi collapses to empty, and maps to empty (not NaN, not a range).
    let bad = Interval::new(BigFloat::from_i64(3), BigFloat::from_i64(1));
    assert!(bad.is_empty());
    assert!(bad.erf().is_empty());
}

// ---- Robustness (异常中断: no panic, no unbounded work) ---------------

#[test]
fn robustness_special_value_matrix() {
    let specials = [
        BigFloat::NAN,
        BigFloat::infinity(),
        BigFloat::neg_infinity(),
        BigFloat::zero(),
        -BigFloat::zero(),
        BigFloat::one(),
        -BigFloat::one(),
    ];
    // Every combination across all entry points must terminate without panic.
    for s in &specials {
        let _ = s.erf();
        let _ = s.erfc();
        let (v, b) = s.erf_checked();
        assert!(!v.is_nan() || b.is_nan());
        let (v, b) = s.erfc_checked();
        assert!(!v.is_nan() || b.is_nan());
        let pt = Interval::point(s.clone());
        let _ = pt.erf();
        let _ = pt.erfc();
    }
}

#[test]
fn robustness_wide_sweep_terminates() {
    // A broad sweep including the threshold neighbourhood and both signs. The
    // guard-escalation loop and the series iteration cap are bounded, so every
    // call returns with a finite value and a positive bound.
    let mut x = BigFloat::from_f64(-14.0);
    let step = BigFloat::from_f64(0.42);
    for _ in 0..50 {
        let (e, eb) = x.erf_checked();
        let (c, cb) = x.erfc_checked();
        assert!(e.is_finite() && c.is_finite(), "non-finite at {x}");
        assert!(
            eb > BigFloat::zero() && cb > BigFloat::zero(),
            "bad bound at {x}"
        );
        x = x + step.clone();
    }
}

#[test]
fn robustness_interval_degenerate_and_extreme() {
    let cases = [
        Interval::point(BigFloat::infinity()),
        Interval::point(BigFloat::neg_infinity()),
        Interval::point(BigFloat::zero()),
        Interval::new(BigFloat::from_f64(-1e6), BigFloat::from_f64(1e6)),
        Interval::new(BigFloat::from_f64(1e-30), BigFloat::from_f64(2e-30)),
    ];
    for iv in &cases {
        let _ = iv.erf();
        let _ = iv.erfc();
    }
    // A very wide interval maps into the saturated range.
    let wide = Interval::new(BigFloat::from_f64(-1e6), BigFloat::from_f64(1e6));
    let img = wide.erf();
    assert!(img.contains(&-BigFloat::one()));
    assert!(img.contains(&BigFloat::one()));
}

#[test]
fn robustness_deterministic_repeated_calls() {
    // Zero global state: repeated calls must return bitwise identical results,
    // including the certified bound.
    let xv = BigFloat::from_f64(1.3);
    assert_eq!(xv.erf().canonical_parts(), xv.erf().canonical_parts());
    let a = xv.erf_checked();
    let b = xv.erf_checked();
    assert_eq!(a.0.canonical_parts(), b.0.canonical_parts());
    assert_eq!(a.1.canonical_parts(), b.1.canonical_parts());
    let c = xv.erfc_checked();
    let d = xv.erfc_checked();
    assert_eq!(c.1.canonical_parts(), d.1.canonical_parts());
}
