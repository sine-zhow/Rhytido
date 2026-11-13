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

// ---- M2-E2b: full IEEE semantics (normal / overflow / subnormal) --------

/// Exact power of two `2^n`, any sign of `n` (the quotient terminates, so the
/// division is exact at the default working precision).
fn pow2(n: isize) -> BigFloat {
    let k = BigFloat::from_i64(n.abs() as i64);
    let p = BigFloat::from_i64(2).pow(&k);
    if n >= 0 { p } else { BigFloat::one() / p }
}

/// Largest finite value of `fmt`: `2^(emax+1) - 2^(emax-p+1)`. The exact
/// difference needs `p <= 113` bits, below the default working precision.
fn max_finite(fmt: Format) -> BigFloat {
    let p = fmt.precision() as isize;
    let (_, emax) = fmt.exponent_range();
    pow2(emax + 1) - pow2(emax - p + 1)
}

const ALL_FORMATS: [Format; 5] = [
    Format::Double,
    Format::Single,
    Format::Half,
    Format::Quad,
    Format::DoubleExtended,
];

const ALL_MODES: [Rounding; 6] = [
    Rounding::Nearest,
    Rounding::Zero,
    Rounding::Up,
    Rounding::Down,
    Rounding::Away,
    Rounding::HalfAway,
];

#[test]
fn normal_format_exponent_ranges() {
    // IEEE-754 binary16/32/64/128; DoubleExtended is binary79-like with a
    // 15-bit exponent field (bias 16383), per the public format description.
    assert_eq!(Format::Double.exponent_range(), (-1022, 1023));
    assert_eq!(Format::Single.exponent_range(), (-126, 127));
    assert_eq!(Format::Half.exponent_range(), (-14, 15));
    assert_eq!(Format::Quad.exponent_range(), (-16382, 16383));
    assert_eq!(Format::DoubleExtended.exponent_range(), (-16382, 16383));
}

#[test]
fn normal_round_to_format_matches_round_to_in_normal_range() {
    // Well inside every format's normal range, format rounding must agree
    // with the plain significand rounder in every mode (cross-check against
    // the backend's own directed rounding).
    let vals = [
        BigFloat::one() / BigFloat::from_i64(3),
        BigFloat::from_i64(2) / BigFloat::from_i64(7),
        BigFloat::from_i64(-5) / BigFloat::from_i64(9),
    ];
    for v in &vals {
        let wide = v.with_precision(200);
        for fmt in ALL_FORMATS {
            for mode in ALL_MODES {
                assert_eq!(
                    wide.round_to_format(fmt, mode),
                    wide.round_to(fmt.precision(), mode),
                    "{fmt:?} {mode:?} at {wide}"
                );
            }
        }
    }
}

#[test]
fn normal_format_boundary_values_are_fixed_points() {
    // max_finite, min_normal and min_subnormal are representable, so every
    // mode must keep them verbatim.
    for fmt in ALL_FORMATS {
        let p = fmt.precision() as isize;
        let (emin, _) = fmt.exponent_range();
        for v in [
            max_finite(fmt),
            pow2(emin),
            pow2(emin - p + 1),
            BigFloat::one(),
        ] {
            for mode in ALL_MODES {
                let r = v.round_to_format(fmt, mode);
                assert_eq!(r, v, "{fmt:?} {mode:?} at {v}");
                assert!(r.precision() <= fmt.precision());
            }
        }
    }
}

#[test]
fn normal_subnormal_grid_precision() {
    // Half subnormal grid spacing is 2^-24. On-grid values are fixed points;
    // the half-grid tie 3*2^-25 goes to the even neighbor 2*2^-24 = 2^-23;
    // perturbations off the tie resolve to the nearer grid point.
    let grid = pow2(-24);
    let on_grid = BigFloat::from_i64(3) * grid.clone();
    assert_eq!(
        on_grid.round_to_format(Format::Half, Rounding::Nearest),
        on_grid
    );
    let tie = BigFloat::from_i64(3) * pow2(-25);
    assert_eq!(
        tie.round_to_format(Format::Half, Rounding::Nearest),
        pow2(-23)
    );
    let above = tie.clone() + pow2(-30);
    assert_eq!(
        above.round_to_format(Format::Half, Rounding::Nearest),
        pow2(-23)
    );
    let below = tie - pow2(-30);
    assert_eq!(below.round_to_format(Format::Half, Rounding::Nearest), grid);
    // Double: the largest subnormal (2^52-1)*2^-1074 is a fixed point, and
    // half a grid below the smallest normal ties to the (even) normal.
    let largest_sub = (pow2(52) - BigFloat::one()) * pow2(-1074);
    assert_eq!(
        largest_sub.round_to_format(Format::Double, Rounding::Nearest),
        largest_sub
    );
    let carry_tie = pow2(-1022) - pow2(-1075);
    assert_eq!(
        carry_tie.round_to_format(Format::Double, Rounding::Nearest),
        pow2(-1022)
    );
}

#[test]
fn extreme_overflow_to_infinity() {
    // The half-precision overflow tie 65520 = 65504 + 16 rounds to 2^16,
    // which is out of range: infinity, not 65536.
    let h = BigFloat::from_i64(65520).round_to_format(Format::Half, Rounding::Nearest);
    assert!(h.is_infinite() && h.is_sign_positive());
    let nh = BigFloat::from_i64(-65520).round_to_format(Format::Half, Rounding::Nearest);
    assert!(nh.is_infinite() && nh.is_sign_negative());
    // Just below the tie stays finite; far-out values overflow too.
    assert_eq!(
        BigFloat::from_i64(65519).round_to_format(Format::Half, Rounding::Nearest),
        BigFloat::from_i64(65504)
    );
    assert_eq!(
        BigFloat::from_i64(65512).round_to_format(Format::Half, Rounding::Nearest),
        BigFloat::from_i64(65504)
    );
    for (fmt, e) in [
        (Format::Single, 128isize),
        (Format::Double, 1024),
        (Format::Quad, 16384),
        (Format::DoubleExtended, 16384),
    ] {
        let r = pow2(e).round_to_format(fmt, Rounding::Nearest);
        assert!(r.is_infinite() && r.is_sign_positive(), "{fmt:?}");
        let nr = (-pow2(e)).round_to_format(fmt, Rounding::Nearest);
        assert!(nr.is_infinite() && nr.is_sign_negative(), "{fmt:?}");
    }
    // 10^40 exceeds binary32's range as well.
    let ten40 = BigFloat::from_i64(10).pow(&BigFloat::from_i64(40));
    assert!(
        ten40
            .round_to_format(Format::Single, Rounding::Nearest)
            .is_infinite()
    );
}

#[test]
fn extreme_overflow_directed_modes() {
    let x = BigFloat::from_i64(65520);
    let max = BigFloat::from_i64(65504);
    let cases = [
        (Rounding::Nearest, BigFloat::infinity()),
        (Rounding::HalfAway, BigFloat::infinity()),
        (Rounding::Away, BigFloat::infinity()),
        (Rounding::Up, BigFloat::infinity()),
        (Rounding::Zero, max.clone()),
        (Rounding::Down, max.clone()),
    ];
    for (mode, want) in cases {
        assert_eq!(x.round_to_format(Format::Half, mode), want, "{mode:?}");
    }
    // Negative overflow mirrors: Up keeps the finite extreme, Down goes to
    // -infinity.
    let nx = -x;
    let nmax = -max.clone();
    let ncases = [
        (Rounding::Nearest, BigFloat::neg_infinity()),
        (Rounding::HalfAway, BigFloat::neg_infinity()),
        (Rounding::Away, BigFloat::neg_infinity()),
        (Rounding::Down, BigFloat::neg_infinity()),
        (Rounding::Zero, nmax.clone()),
        (Rounding::Up, nmax.clone()),
    ];
    for (mode, want) in ncases {
        assert_eq!(nx.round_to_format(Format::Half, mode), want, "{mode:?}");
    }
    // Just above max but below the nearest tie: only the upward modes
    // overflow (they carry into 2^16).
    let above = BigFloat::from_i64(65505);
    assert_eq!(above.round_to_format(Format::Half, Rounding::Nearest), max);
    assert!(
        above
            .round_to_format(Format::Half, Rounding::Up)
            .is_infinite()
    );
    assert_eq!(above.round_to_format(Format::Half, Rounding::Zero), max);
    // Far overflow in every format: Zero/Down keep max_finite.
    for fmt in ALL_FORMATS {
        let far = pow2(20000);
        assert_eq!(far.round_to_format(fmt, Rounding::Zero), max_finite(fmt));
        assert_eq!(far.round_to_format(fmt, Rounding::Down), max_finite(fmt));
        assert!(far.round_to_format(fmt, Rounding::Nearest).is_infinite());
    }
}

#[test]
fn extreme_underflow_to_subnormal_and_zero() {
    // The smallest subnormal of each format is a fixed point; half of it is
    // an exact tie between 0 (even) and the grid step, so nearest keeps 0.
    for fmt in ALL_FORMATS {
        let p = fmt.precision() as isize;
        let (emin, _) = fmt.exponent_range();
        let min_sub = pow2(emin - p + 1);
        assert_eq!(
            min_sub.round_to_format(fmt, Rounding::Nearest),
            min_sub,
            "{fmt:?}"
        );
        let half = pow2(emin - p);
        let r = half.round_to_format(fmt, Rounding::Nearest);
        assert!(r.is_zero() && r.is_sign_positive(), "{fmt:?}");
        let nr = (-half.clone()).round_to_format(fmt, Rounding::Nearest);
        assert!(nr.is_zero() && nr.is_sign_negative(), "{fmt:?}");
        // Far below the grid: zero with the input's sign.
        let tiny = pow2(emin - p - 100);
        assert!(tiny.round_to_format(fmt, Rounding::Nearest).is_zero());
    }
    // Named binary64/binary32 landmarks.
    assert_eq!(
        pow2(-1074).round_to_format(Format::Double, Rounding::Nearest),
        pow2(-1074)
    );
    assert!(
        pow2(-1075)
            .round_to_format(Format::Double, Rounding::Nearest)
            .is_zero()
    );
    assert_eq!(
        pow2(-149).round_to_format(Format::Single, Rounding::Nearest),
        pow2(-149)
    );
    assert!(
        pow2(-150)
            .round_to_format(Format::Single, Rounding::Nearest)
            .is_zero()
    );
    // 1.5 grid steps tie to the even neighbor (2 steps).
    let tie = BigFloat::from_i64(3) * pow2(-1075);
    assert_eq!(
        tie.round_to_format(Format::Double, Rounding::Nearest),
        pow2(-1073)
    );
}

#[test]
fn extreme_underflow_directed_modes() {
    // Half a double grid step: the mode decides between 0 and 2^-1074.
    let half = pow2(-1075);
    let step = pow2(-1074);
    let cases = [
        (Rounding::Nearest, BigFloat::zero()),
        (Rounding::Zero, BigFloat::zero()),
        (Rounding::Down, BigFloat::zero()),
        (Rounding::Up, step.clone()),
        (Rounding::Away, step.clone()),
        (Rounding::HalfAway, step.clone()),
    ];
    for (mode, want) in cases {
        let r = half.round_to_format(Format::Double, mode);
        assert_eq!(r, want, "{mode:?}");
    }
    let nhalf = -half;
    let nstep = -step.clone();
    let ncases = [
        (Rounding::Nearest, -BigFloat::zero()),
        (Rounding::Zero, -BigFloat::zero()),
        (Rounding::Up, -BigFloat::zero()),
        (Rounding::Down, nstep.clone()),
        (Rounding::Away, nstep.clone()),
        (Rounding::HalfAway, nstep.clone()),
    ];
    for (mode, want) in ncases {
        let r = nhalf.round_to_format(Format::Double, mode);
        assert_eq!(r, want, "{mode:?}");
        if want.is_zero() {
            assert!(r.is_sign_negative(), "{mode:?} sign");
        }
    }
    // Far below the grid: directed-away modes still emit one grid step.
    let tiny = pow2(-2000);
    assert!(
        tiny.round_to_format(Format::Double, Rounding::Nearest)
            .is_zero()
    );
    assert_eq!(tiny.round_to_format(Format::Double, Rounding::Up), step);
    assert_eq!(tiny.round_to_format(Format::Double, Rounding::Away), step);
    assert!(
        tiny.round_to_format(Format::Double, Rounding::Zero)
            .is_zero()
    );
}

#[test]
fn extreme_subnormal_carry_into_normal() {
    // Half: 2^-14 - 2^-25 is 1023.5 grid units, an exact tie between the
    // largest subnormal (1023, odd) and the smallest normal (1024, even).
    let tie = pow2(-14) - pow2(-25);
    assert_eq!(
        tie.round_to_format(Format::Half, Rounding::Nearest),
        pow2(-14)
    );
    assert_eq!(tie.round_to_format(Format::Half, Rounding::Up), pow2(-14));
    assert_eq!(
        tie.round_to_format(Format::Half, Rounding::Down),
        BigFloat::from_i64(1023) * pow2(-24)
    );
}

#[test]
fn error_special_values_propagate_all_formats_modes() {
    for fmt in ALL_FORMATS {
        for mode in ALL_MODES {
            assert!(BigFloat::NAN.round_to_format(fmt, mode).is_nan());
            let inf = BigFloat::infinity().round_to_format(fmt, mode);
            assert!(inf.is_infinite() && inf.is_sign_positive());
            let ninf = BigFloat::neg_infinity().round_to_format(fmt, mode);
            assert!(ninf.is_infinite() && ninf.is_sign_negative());
            let z = BigFloat::zero().round_to_format(fmt, mode);
            assert!(z.is_zero() && z.is_sign_positive());
            let nz = (-BigFloat::zero()).round_to_format(fmt, mode);
            assert!(nz.is_zero() && nz.is_sign_negative());
        }
    }
}

#[test]
fn robustness_format_rounding_encloses_and_is_idempotent() {
    // Across formats and modes: the image is representable (width bound),
    // re-rounding is a fixed point, the directed images enclose the input,
    // and nearest lies between them.
    let vals = [
        pow2(-2000),
        pow2(-1075),
        pow2(-1074),
        pow2(-1022),
        BigFloat::from_f64(-0.3),
        BigFloat::from_f64(0.3),
        BigFloat::one() / BigFloat::from_i64(3),
        BigFloat::from_i64(65504),
        BigFloat::from_i64(65520),
        pow2(128),
        pow2(1024),
        pow2(16384),
    ];
    for fmt in ALL_FORMATS {
        for v in &vals {
            let down = v.round_to_format(fmt, Rounding::Down);
            let up = v.round_to_format(fmt, Rounding::Up);
            assert!(down <= *v && *v <= up, "{fmt:?} at {v}");
            for mode in ALL_MODES {
                let r = v.round_to_format(fmt, mode);
                assert!(r.precision() <= fmt.precision(), "{fmt:?} {mode:?}");
                assert_eq!(r.round_to_format(fmt, mode), r, "{fmt:?} {mode:?}");
                if mode == Rounding::Nearest {
                    assert!(down <= r && r <= up, "{fmt:?} at {v}");
                }
            }
        }
    }
}

#[test]
fn robustness_nearest_format_rounding_is_monotone() {
    // Round-to-nearest-format is monotone (the property the core interval
    // kernel relies on), including across zero and the overflow threshold.
    let sorted = [
        -pow2(16),
        BigFloat::from_f64(-0.3),
        -pow2(-1075),
        BigFloat::zero(),
        pow2(-2000),
        pow2(-1075),
        pow2(-1074),
        pow2(-1022),
        BigFloat::from_f64(0.3),
        BigFloat::from_i64(65504),
        BigFloat::from_i64(65520),
        pow2(128),
        pow2(1024),
        pow2(16384),
    ];
    for fmt in ALL_FORMATS {
        for pair in sorted.windows(2) {
            let a = pair[0].round_to_format(fmt, Rounding::Nearest);
            let b = pair[1].round_to_format(fmt, Rounding::Nearest);
            assert!(a <= b, "{fmt:?}: {a} > {b}");
        }
    }
}

#[test]
fn robustness_extreme_exponents_never_panic() {
    // Exponents far outside every format's range exercise both overflow and
    // the tiny-input fast path without materializing huge divisors.
    for e in [-100000isize, -20000, 20000, 100000] {
        let v = pow2(e);
        for fmt in ALL_FORMATS {
            for mode in ALL_MODES {
                let r = v.round_to_format(fmt, mode);
                assert!(r.precision() <= fmt.precision());
            }
        }
    }
}
