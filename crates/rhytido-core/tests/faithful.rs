//! Module tests for `faithful` (src/faithful.rs): the Ziv-style faithful
//! evaluation engine and its eight-state quality certificates.
//!
//! Layer: module tests (AGENTS.md §4) — external view of the public API only.
//! The known-value table is the one in `docs/plan/M2-stage2-tasks.md` (task C).

use rhytido_backend::{BigFloat, Interval, Rounding};
use rhytido_core::{
    BaseFn, Context, EngineStats, Expr, FpResult, Interner, IntervalFnHook, NodeId, Quality,
    eval_point, evaluate_faithful,
};

/// Target precision of the task-sheet table: the binary64 significand width.
const P: usize = 53;

fn ctx(prec: usize) -> Context {
    Context::new().with_precision(prec)
}

fn c(it: &mut Interner, v: i64) -> NodeId {
    it.intern(Expr::Constant(BigFloat::from_i64(v)))
}

fn var(it: &mut Interner) -> NodeId {
    it.intern(Expr::Variable)
}

fn apply(it: &mut Interner, f: BaseFn, a: NodeId) -> NodeId {
    it.intern(Expr::Apply(f, a))
}

/// Exact power of two `2^n`, any sign of `n`.
fn pow2(n: i64) -> BigFloat {
    let k = BigFloat::from_i64(n.abs());
    let p = BigFloat::from_i64(2).pow(&k);
    if n >= 0 { p } else { BigFloat::one() / p }
}

/// The engine's answer without cutoff or hook.
fn eval(it: &Interner, id: NodeId, x: &BigFloat, prec: usize) -> (FpResult, EngineStats) {
    evaluate_faithful(it, id, x, &ctx(prec), None, None)
}

/// Reference value of the same tree from the point evaluator, `margin` bits
/// above the target (the task sheet's "p + 100 reference").
fn reference(it: &Interner, id: NodeId, x: &BigFloat, prec: usize, margin: usize) -> BigFloat {
    eval_point(it, id, x, prec + margin)
}

/// Local spacing of the `prec` grid at `v`: half of it is the largest error a
/// correctly rounded result can make, all of it the largest faithful error.
/// The larger neighbour distance is used so binade edges cannot undercut the
/// bound. `v` must be a nonzero finite value representable at `prec`.
fn spacing(v: &BigFloat, prec: usize) -> BigFloat {
    let up = v.next_above(prec) - v.clone();
    let down = v.clone() - v.next_below(prec);
    if up > down { up } else { down }
}

// ---- Normal cases: the task-sheet known-correct-rounding table ---------

#[test]
fn normal_known_double_table_values_and_certificates() {
    let mut it = Interner::new();
    let one = c(&mut it, 1);
    let two = c(&mut it, 2);
    let three = c(&mut it, 3);
    let sqrt2 = apply(&mut it, BaseFn::Sqrt, two);
    let exp1 = apply(&mut it, BaseFn::Exp, one);
    let ln2 = apply(&mut it, BaseFn::Ln, two);
    let sin1 = apply(&mut it, BaseFn::Sin, one);
    let cos1 = apply(&mut it, BaseFn::Cos, one);
    let atan1 = apply(&mut it, BaseFn::Atan, one);
    let erf1 = apply(&mut it, BaseFn::Erf, one);
    let third = it.intern(Expr::Div(one, three));
    let two_thirds = it.intern(Expr::Div(two, three));
    // Correctly-rounded binary64 reference values. The std constants are
    // value-identical to the decimal literals they replace (clippy
    // approx_constant), so the assertions are unchanged in strength.
    let cases = [
        ("sqrt(2)", sqrt2, std::f64::consts::SQRT_2),
        ("exp(1)", exp1, std::f64::consts::E),
        ("ln(2)", ln2, std::f64::consts::LN_2),
        ("sin(1)", sin1, 0.8414709848078965),
        ("cos(1)", cos1, 0.5403023058681398),
        ("atan(1)", atan1, std::f64::consts::FRAC_PI_4),
        ("erf(1)", erf1, 0.8427007929497149),
        ("1/3", third, 0.3333333333333333),
        ("2/3", two_thirds, 0.6666666666666666),
    ];
    for (name, id, want) in cases {
        let (result, stats) = eval(&it, id, &BigFloat::zero(), P);
        assert_eq!(
            result.value(),
            &BigFloat::from_f64(want),
            "{name}: certificate {:?}, stats {stats:?}",
            result.quality()
        );
        // Task C: certificate in {Exact, CorrectlyRounded}.
        assert!(
            matches!(result.quality(), Quality::Exact | Quality::CorrectlyRounded),
            "{name}: unexpected certificate {:?}",
            result.quality()
        );
        assert!(
            result.is_correctly_rounded(),
            "{name} must be correctly rounded"
        );
        assert!(stats.rounds() >= 1, "{name}: no round performed");
        assert!(
            stats.final_precision() <= 512 * P,
            "{name}: precision budget exceeded: {}",
            stats.final_precision()
        );
        // The certificate means the value is the target rounding of a
        // higher-precision reference of the same tree.
        let reference = reference(&it, id, &BigFloat::zero(), P, 100);
        assert_eq!(
            result.value(),
            &reference.round_to(P, Rounding::Nearest),
            "{name}: value is not the correctly rounded reference"
        );
    }
}

#[test]
fn normal_w0_starts_at_target_plus_ten_and_test1_hits_the_first_round() {
    // Task C: w0 = p + 10. When Test 1 already decides at w0 the engine stops
    // there, so the reported working precision pins that first-round width.
    let mut it = Interner::new();
    let two = c(&mut it, 2);
    let sqrt2 = apply(&mut it, BaseFn::Sqrt, two);
    let (result, stats) = eval(&it, sqrt2, &BigFloat::zero(), P);
    assert_eq!(stats.rounds(), 1, "Test 1 should decide on the first round");
    assert_eq!(stats.final_precision(), P + 10, "w0 must be p + 10");
    assert_eq!(result.quality(), Quality::CorrectlyRounded);
    assert_eq!(result.quality().code(), 6);
}

#[test]
fn normal_variable_input_keeps_the_guard_bits_and_claims_no_exactness() {
    // Regression for the input-label guard bug: sqrt(1.75) at a 24-bit target
    // through a Variable leaf. The engine's own input interval is labelled at
    // the working precision, which used to cap the kernel width, collapse the
    // enclosure to a point, and make the engine report a known-inexact (or even
    // exact) conclusion it cannot support.
    let mut it = Interner::new();
    let x = var(&mut it);
    let sqrt_x = apply(&mut it, BaseFn::Sqrt, x);
    let arg = BigFloat::from_f64(1.75);
    let (result, stats) = eval(&it, sqrt_x, &arg, 24);
    assert_eq!(stats.final_precision(), 34);
    assert_eq!(
        result.quality(),
        Quality::CorrectlyRounded,
        "an irrational must not be certified with exactness knowledge"
    );
    let reference = reference(&it, sqrt_x, &arg, 24, 200);
    assert_eq!(result.value(), &reference.round_to(24, Rounding::Nearest));
}

#[test]
fn normal_exact_case_two_plus_three_at_three_bits() {
    // Task C: "2+3@p=3 -> Exact(4)".
    let mut it = Interner::new();
    let two = c(&mut it, 2);
    let three = c(&mut it, 3);
    let sum = it.intern(Expr::Add(two, three));
    let (result, stats) = eval(&it, sum, &BigFloat::zero(), 3);
    assert_eq!(result.quality(), Quality::Exact);
    assert_eq!(result.quality().code(), 4);
    assert_eq!(result.value(), &BigFloat::from_i64(5));
    assert_eq!(stats.rounds(), 1);
    assert!(result.is_exact());
    assert!(result.is_faithful());
}

#[test]
fn normal_correctly_rounded_inexact_for_an_unrepresentable_point() {
    // M2-plan §2: a *point* enclosure of a value `p` cannot represent is
    // CorrectlyRoundedInexact(7) — exactness is known, the rounding is chosen.
    // 5/8 = 0.625 needs three bits; at p = 2 it is the tie between 0.5 and 0.75
    // and ties-to-even takes 0.5.
    let mut it = Interner::new();
    let x = var(&mut it);
    let value = BigFloat::from_i64(5) / pow2(3);
    let (result, stats) = eval(&it, x, &value, 2);
    assert_eq!(result.quality(), Quality::CorrectlyRoundedInexact);
    assert_eq!(result.quality().code(), 7);
    assert_eq!(result.value(), &pow2(-1));
    assert!(result.quality().is_known_inexact());
    assert!(result.is_correctly_rounded());
    assert_eq!(stats.rounds(), 1);
}

#[test]
fn normal_test2_straddling_tie_is_faithful_inexact() {
    // Task C Test 2: the nearest images differ but are adjacent target floats.
    // An input with more significant bits than the working precision gives the
    // enclosure its width; here it straddles the p = 2 tie at 0.625 and the
    // single retry round cannot resolve it either.
    let mut it = Interner::new();
    let x = var(&mut it);
    let value = BigFloat::from_i64(5) / pow2(3) + pow2(-40);
    let (result, stats) = eval(&it, x, &value, 2);
    assert_eq!(result.quality(), Quality::FaithfulInexact);
    assert_eq!(result.quality().code(), 5);
    assert!(result.quality().is_known_inexact());
    assert!(result.is_faithful());
    // Either adjacent float is a faithful rounding of the input; the engine
    // fixes the lower one so results are reproducible (module documentation).
    let lower = pow2(-1);
    let upper = BigFloat::from_i64(3) / pow2(2);
    assert!(
        result.value() == &lower || result.value() == &upper,
        "faithful result {} is neither adjacent float",
        result.value()
    );
    assert_eq!(
        result.value(),
        &lower,
        "the documented choice is the lower image"
    );
    // "One more unit in the last place": exactly one retry round at w0 + 10.
    assert_eq!(stats.rounds(), 2);
    assert_eq!(stats.final_precision(), 2 + 10 + 10);
    // Faithful means within one spacing of the true value.
    let exact = reference(&it, x, &value, 2, 200);
    assert!((result.value().clone() - exact).abs() <= spacing(result.value(), 2));
}

#[test]
fn normal_one_more_unit_retries_and_upgrades_the_certificate() {
    // Same shape, but the input becomes representable in the retry round: the
    // engine must adopt the stronger Test 1 conclusion from that round.
    let mut it = Interner::new();
    let x = var(&mut it);
    let value = BigFloat::from_i64(5) / pow2(3) + pow2(-15);
    let (result, stats) = eval(&it, x, &value, 2);
    assert!(
        result.is_correctly_rounded(),
        "the retry should upgrade the certificate, got {:?}",
        result.quality()
    );
    assert_eq!(result.quality(), Quality::CorrectlyRoundedInexact);
    let three_quarters = BigFloat::from_i64(3) / pow2(2);
    assert_eq!(result.value(), &three_quarters);
    assert_eq!(stats.rounds(), 2);
    assert_eq!(stats.final_precision(), 2 + 10 + 10);
}

/// Hook answering `exp` with the valid but loose enclosure [0.8, 1.4]: it
/// contains exp(0) = 1 and holds exactly one 2-bit float, so Test 3 applies.
struct ExpLoose;

impl IntervalFnHook for ExpLoose {
    fn apply_fn(&self, f: BaseFn, _arg: &Interval, _prec: usize) -> Option<Interval> {
        if f != BaseFn::Exp {
            return None;
        }
        Some(Interval::new(
            BigFloat::from_f64(0.8),
            BigFloat::from_f64(1.4),
        ))
    }
}

#[test]
fn normal_test3_unique_contained_float_is_faithful() {
    // Task C Test 3: round_up(lo) == round_down(hi) = c, so c is the only
    // target float inside the enclosure and is reported with Faithful(1).
    // Natural enclosures are about 2^-10 of a target ulp wide (the guard
    // model in interval_eval), so this shape is reached through a caller
    // enclosure; tests/integration_engine.rs carries the reachability audit.
    let mut it = Interner::new();
    let zero = c(&mut it, 0);
    let exp0 = apply(&mut it, BaseFn::Exp, zero);
    let (result, stats) =
        evaluate_faithful(&it, exp0, &BigFloat::zero(), &ctx(2), None, Some(&ExpLoose));
    assert_eq!(result.quality(), Quality::Faithful);
    assert_eq!(result.quality().code(), 1);
    assert!(result.is_faithful());
    assert!(!result.is_correctly_rounded());
    assert_eq!(
        result.value(),
        &BigFloat::one(),
        "the unique float in [0.8, 1.4]"
    );
    assert_eq!(
        stats.rounds(),
        2,
        "one more unit is spent on Test 3 as well"
    );
    assert_eq!(stats.final_precision(), 22);
}

#[test]
fn normal_test2_is_disabled_when_a_rounding_image_is_zero() {
    // The successor of zero at an unbounded exponent range is a documented
    // convention, not a true successor, so an enclosure whose images straddle
    // zero must not be certified faithful: the engine escalates and reports
    // NotFaithful(0) honestly instead.
    let mut it = Interner::new();
    let one = c(&mut it, 1);
    let exp1 = apply(&mut it, BaseFn::Exp, one);

    struct SignedZero;
    impl IntervalFnHook for SignedZero {
        fn apply_fn(&self, f: BaseFn, _arg: &Interval, _prec: usize) -> Option<Interval> {
            (f == BaseFn::Exp).then(|| Interval::new(-BigFloat::zero(), BigFloat::zero()))
        }
    }

    let (result, stats) = evaluate_faithful(
        &it,
        exp1,
        &BigFloat::zero(),
        &ctx(P),
        None,
        Some(&SignedZero),
    );
    assert_eq!(result.quality(), Quality::NotFaithful);
    assert!(!result.is_faithful());
    assert_eq!(
        stats.final_precision(),
        512 * P,
        "the budget stopped the loop"
    );
    // The conservative value is the nearest image of the lower endpoint, which
    // keeps the endpoint's sign.
    assert!(result.value().is_zero() && result.value().is_sign_negative());
}

/// Hook answering `sqrt` with [2, 4]: a valid enclosure of sqrt(4) = 2 whose
/// two-bit rounding images are two grid steps apart, so no test can conclude.
struct SqrtVeryLoose;

impl IntervalFnHook for SqrtVeryLoose {
    fn apply_fn(&self, f: BaseFn, _arg: &Interval, _prec: usize) -> Option<Interval> {
        if f != BaseFn::Sqrt {
            return None;
        }
        Some(Interval::new(BigFloat::from_i64(2), BigFloat::from_i64(4)))
    }
}

#[test]
fn error_not_faithful_reports_the_conservative_lower_image() {
    // Task C: past the precision budget the engine gives up honestly —
    // NotFaithful(0) with the nearest rounding of the lower endpoint, and the
    // budget clause "512 * p" is exactly what stopped the loop.
    let mut it = Interner::new();
    let four = c(&mut it, 4);
    let sqrt4 = apply(&mut it, BaseFn::Sqrt, four);
    let (result, stats) = evaluate_faithful(
        &it,
        sqrt4,
        &BigFloat::zero(),
        &ctx(2),
        None,
        Some(&SqrtVeryLoose),
    );
    assert_eq!(result.quality(), Quality::NotFaithful);
    assert_eq!(result.quality().code(), 0);
    assert!(!result.is_faithful());
    assert_eq!(result.value(), &BigFloat::from_i64(2), "nearest(lo@p)");
    assert_eq!(stats.final_precision(), 2 * 512, "the budget is 512 * p");
    assert!(stats.rounds() >= 3, "escalation should take several rounds");
    assert!(!result.value().is_nan());
}

#[test]
fn normal_cutoff_drives_a_negligible_result_belowcutoff() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let tiny = pow2(-60);
    let cutoff = pow2(-50);
    let (result, stats) = evaluate_faithful(&it, x, &tiny, &ctx(P), Some(&cutoff), None);
    assert_eq!(result.quality(), Quality::BelowCutoff);
    assert_eq!(result.quality().code(), 2);
    assert!(result.quality().is_below_cutoff());
    assert!(!result.is_faithful());
    assert_eq!(
        stats.rounds(),
        1,
        "the cutoff is decided in the first round"
    );
    // The documented convention is a signed zero, so everything knowable
    // beyond "negligible" survives. The certificate's meaning is that the
    // reported value is within the cutoff of the true one.
    assert!(result.value().is_zero());
    assert!(result.value().is_sign_positive());
    assert!(
        (result.value().clone() - tiny.clone()).abs() <= cutoff.clone(),
        "BelowCutoff must bound the error by the cutoff"
    );
    // A strictly negative enclosure flushes to negative zero.
    let neg_tiny = -tiny.clone();
    let (result, _) = evaluate_faithful(&it, x, &neg_tiny, &ctx(P), Some(&cutoff), None);
    assert_eq!(result.quality(), Quality::BelowCutoff);
    assert!(result.value().is_zero() && result.value().is_sign_negative());
    // An enclosure straddling zero has no knowable sign: positive zero.
    let (result, _) = evaluate_faithful(&it, x, &BigFloat::zero(), &ctx(P), Some(&cutoff), None);
    assert_eq!(result.quality(), Quality::BelowCutoff);
    assert!(result.value().is_sign_positive());
    // The cutoff is a magnitude: a negative one behaves as its absolute value,
    // and zero or non-finite cutoffs are ignored.
    let neg_cutoff = -cutoff.clone();
    let (result, _) = evaluate_faithful(&it, x, &tiny, &ctx(P), Some(&neg_cutoff), None);
    assert_eq!(result.quality(), Quality::BelowCutoff);
    let zero_cutoff = BigFloat::zero();
    let (result, _) = evaluate_faithful(&it, x, &tiny, &ctx(P), Some(&zero_cutoff), None);
    assert_eq!(
        result.quality(),
        Quality::Exact,
        "a zero cutoff must be ignored"
    );
    let inf_cutoff = BigFloat::infinity();
    let (result, _) = evaluate_faithful(&it, x, &tiny, &ctx(P), Some(&inf_cutoff), None);
    assert_eq!(
        result.quality(),
        Quality::Exact,
        "an infinite cutoff must be ignored"
    );
    // Not negligible: outside [-cutoff, cutoff] the normal certification runs.
    let bigger = pow2(-40);
    let (result, _) = evaluate_faithful(&it, x, &bigger, &ctx(P), Some(&cutoff), None);
    assert_eq!(result.quality(), Quality::Exact);
}

#[test]
fn normal_cutoff_boundary_is_inclusive() {
    // yI subset of [-c, c] with exact dyadic endpoints: equality counts.
    let mut it = Interner::new();
    let x = var(&mut it);
    let cutoff = pow2(-8);
    let (result, _) = evaluate_faithful(&it, x, &cutoff, &ctx(P), Some(&cutoff), None);
    assert_eq!(result.quality(), Quality::BelowCutoff);
    let neg_boundary = -cutoff.clone();
    let (result, _) = evaluate_faithful(&it, x, &neg_boundary, &ctx(P), Some(&cutoff), None);
    assert_eq!(result.quality(), Quality::BelowCutoff);
    let just_outside = cutoff.clone() + pow2(-20);
    let (result, _) = evaluate_faithful(&it, x, &just_outside, &ctx(P), Some(&cutoff), None);
    assert!(
        !result.quality().is_below_cutoff(),
        "{:?}",
        result.quality()
    );
    assert!(result.is_exact());
}

#[test]
fn error_nonreal_cases_report_no_real_conclusion() {
    let mut it = Interner::new();
    let zero = c(&mut it, 0);
    let one = c(&mut it, 1);
    let minus_one = c(&mut it, -1);
    let ln0 = apply(&mut it, BaseFn::Ln, zero);
    let zero_over_one = it.intern(Expr::Div(zero, one));
    let zero_over_zero = it.intern(Expr::Div(zero, zero));
    let one_over_zero = it.intern(Expr::Div(one, zero));
    let sqrt_neg = apply(&mut it, BaseFn::Sqrt, minus_one);
    let x = var(&mut it);

    let (result, stats) = eval(&it, ln0, &BigFloat::zero(), P);
    assert_eq!(result.quality(), Quality::NonReal);
    assert_eq!(result.quality().code(), 3);
    assert!(result.is_non_real());
    assert!(!result.is_faithful());
    // ln(0) = -infinity: the engine reports the offending endpoint. The task
    // sheet says NaN; the deviation is documented in src/faithful.rs and was
    // reported to the lead for adjudication.
    assert_eq!(result.value(), &BigFloat::neg_infinity());
    assert_eq!(
        stats.rounds(),
        1,
        "non-real is decided before any refinement"
    );

    let (result, _) = eval(&it, zero_over_one, &BigFloat::zero(), P);
    assert_eq!(result.quality(), Quality::Exact);
    assert_eq!(result.value(), &BigFloat::zero());

    // Division by a zero *interval* is NaN in the interval layer (the backend
    // treats any divisor containing zero as non-real), so 0/0 and 1/0 both end
    // up NonReal with no endpoint to report.
    let (result, _) = eval(&it, zero_over_zero, &BigFloat::zero(), P);
    assert_eq!(result.quality(), Quality::NonReal);
    assert!(result.value().is_nan(), "0/0: {result:?}");
    let (result, _) = eval(&it, one_over_zero, &BigFloat::zero(), P);
    assert_eq!(result.quality(), Quality::NonReal);
    assert!(result.value().is_nan(), "1/0: {result:?}");

    let (result, _) = eval(&it, sqrt_neg, &BigFloat::zero(), P);
    assert_eq!(result.quality(), Quality::NonReal);
    assert!(result.value().is_nan());

    for xv in [
        BigFloat::NAN,
        BigFloat::infinity(),
        BigFloat::neg_infinity(),
    ] {
        let (result, stats) = eval(&it, x, &xv, P);
        assert_eq!(result.quality(), Quality::NonReal, "input {xv}");
        assert!(!result.value().is_finite());
        assert_eq!(stats.rounds(), 1);
    }
}

#[test]
fn normal_determinism_same_inputs_give_identical_results() {
    let mut it = Interner::new();
    let one = c(&mut it, 1);
    let two = c(&mut it, 2);
    let sqrt2 = apply(&mut it, BaseFn::Sqrt, two);
    let erf1 = apply(&mut it, BaseFn::Erf, one);
    let atan1 = apply(&mut it, BaseFn::Atan, one);
    let half = it.intern(Expr::Div(one, two));
    for id in [sqrt2, erf1, atan1, half] {
        let first = eval(&it, id, &BigFloat::zero(), P);
        let second = eval(&it, id, &BigFloat::zero(), P);
        assert_eq!(first.0, second.0, "value or certificate drifted");
        assert_eq!(first.1, second.1, "engine statistics drifted");
    }
    // A fresh interner holding the same tree gives the same answer: nothing is
    // cached across calls or objects (AGENTS.md §3).
    let mut other = Interner::new();
    let o2 = c(&mut other, 2);
    let osqrt2 = apply(&mut other, BaseFn::Sqrt, o2);
    let rebuilt = eval(&other, osqrt2, &BigFloat::zero(), P);
    let original = eval(&it, sqrt2, &BigFloat::zero(), P);
    assert_eq!(original.0, rebuilt.0);
    assert_eq!(original.1, rebuilt.1);
}

#[test]
fn normal_statistics_stay_logarithmic_in_the_budget() {
    // The task sheet asks for "reasonable" rounds: growth is a constant factor
    // per round, so the count is logarithmic in the budget, never linear.
    let mut it = Interner::new();
    let one = c(&mut it, 1);
    let two = c(&mut it, 2);
    let four = c(&mut it, 4);
    let exp1 = apply(&mut it, BaseFn::Exp, one);
    let sin1 = apply(&mut it, BaseFn::Sin, one);
    let erfc1 = apply(&mut it, BaseFn::Erfc, one);
    let sqrt2 = apply(&mut it, BaseFn::Sqrt, two);
    let sqrt4 = apply(&mut it, BaseFn::Sqrt, four);
    for id in [exp1, sin1, erfc1, sqrt2] {
        for prec in [2usize, 24, 53, 165] {
            let (result, stats) = eval(&it, id, &BigFloat::zero(), prec);
            assert!(
                stats.rounds() <= 64,
                "{:?}@{prec}: {} rounds, certificate {:?}",
                it.get(id),
                stats.rounds(),
                result.quality()
            );
            assert!(stats.final_precision() >= prec + 10);
            assert!(stats.final_precision() <= 512 * prec);
        }
    }
    // EngineStats is a value type: reconstructing it from its own accessors
    // gives back the same statistics.
    let (result, stats) = eval(&it, sqrt4, &BigFloat::zero(), P);
    assert_eq!(result.quality(), Quality::Exact);
    let rebuilt = EngineStats::new(stats.rounds(), stats.final_precision());
    assert_eq!(rebuilt, stats);
    assert_eq!(rebuilt.final_precision(), P + 10);
    assert_eq!(rebuilt.rounds(), 1);
}

// ---- Extreme cases ----------------------------------------------------

#[test]
fn extreme_high_target_precision_still_certifies() {
    let mut it = Interner::new();
    let two = c(&mut it, 2);
    let sqrt2 = apply(&mut it, BaseFn::Sqrt, two);
    let prec = 1000;
    let (result, stats) = eval(&it, sqrt2, &BigFloat::zero(), prec);
    assert!(result.is_correctly_rounded(), "{:?}", result.quality());
    assert_eq!(stats.final_precision(), prec + 10);
    let reference = reference(&it, sqrt2, &BigFloat::zero(), prec, 100);
    assert_eq!(result.value(), &reference.round_to(prec, Rounding::Nearest));
    // The certified value is strictly sharper than the double one.
    let (coarse, _) = eval(&it, sqrt2, &BigFloat::zero(), P);
    assert_ne!(coarse.value(), result.value());
    let drift = result.value().clone() - coarse.value().clone();
    assert!(drift.abs() < BigFloat::from_f64(1e-15));
}

#[test]
fn extreme_minimum_target_precision() {
    let mut it = Interner::new();
    let two = c(&mut it, 2);
    let sqrt2 = apply(&mut it, BaseFn::Sqrt, two);
    let (result, stats) = eval(&it, sqrt2, &BigFloat::zero(), 2);
    assert!(result.is_faithful());
    assert_eq!(stats.final_precision(), 12);
    let reference = reference(&it, sqrt2, &BigFloat::zero(), 2, 200);
    assert_eq!(result.value(), &reference.round_to(2, Rounding::Nearest));
    // 1.4142... at two significant bits is 1.5.
    let one_and_half = BigFloat::from_i64(3) / pow2(1);
    assert_eq!(result.value(), &one_and_half);
}

#[test]
fn extreme_target_below_the_context_clamp_is_raised() {
    // Context clamps to MIN_PRECISION = 2 (documented, panic-free), so the
    // engine never runs with a one-bit target.
    let mut it = Interner::new();
    let two = c(&mut it, 2);
    let sqrt2 = apply(&mut it, BaseFn::Sqrt, two);
    let (result, stats) = evaluate_faithful(&it, sqrt2, &BigFloat::zero(), &ctx(1), None, None);
    assert_eq!(ctx(1).precision(), 2);
    assert_eq!(stats.final_precision(), 2 + 10);
    assert!(result.is_faithful());
}

#[test]
fn extreme_exact_zero_from_a_margined_kernel_escalates_to_not_faithful() {
    // erf(0) and sin(0) are exactly zero, but their interval kernels return a
    // symmetric bracket [-eps, +eps] (the checked error bound for erf, the
    // endpoint margin for sin). With the backend's unbounded exponent range
    // the bracket never collapses: both endpoints are representable at the
    // target, they are not adjacent, and Test 2 is disabled across zero, so
    // the engine escalates to the budget and reports NotFaithful(0).
    //
    // That is the contract's honest outcome — the enclosure alone really does
    // not identify the value — but it is a weaker certificate than the value
    // deserves. The fix belongs in the kernels (a zero bound at an exactly
    // representable special value), which is an M2-B refinement outside this
    // task's write scope; reported to the lead.
    let mut it = Interner::new();
    let zero = c(&mut it, 0);
    let x = var(&mut it);
    let erf0 = apply(&mut it, BaseFn::Erf, zero);
    let sin_x = apply(&mut it, BaseFn::Sin, x);
    for id in [erf0, sin_x] {
        let (result, stats) = eval(&it, id, &BigFloat::zero(), 2);
        assert_eq!(result.quality(), Quality::NotFaithful, "{:?}", it.get(id));
        assert!(!result.is_faithful());
        assert!(stats.rounds() >= 3, "escalation should have taken rounds");
        assert_eq!(stats.final_precision(), 2 * 512, "the budget stopped it");
        // The conservative value is the lower endpoint's image: negligible in
        // magnitude and signed like the enclosure's lower side.
        let value = result.value().clone();
        assert!(value.is_sign_negative(), "{value}");
        assert!(value.abs() < BigFloat::from_f64(1e-3), "{value}");
        // A cutoff covering the bracket turns the same run into BelowCutoff,
        // which is the documented way to flush such results.
        let cutoff = BigFloat::from_f64(0.5);
        let (result, stats) =
            evaluate_faithful(&it, id, &BigFloat::zero(), &ctx(2), Some(&cutoff), None);
        assert_eq!(result.quality(), Quality::BelowCutoff, "{:?}", it.get(id));
        assert_eq!(stats.rounds(), 1, "the cutoff is decided immediately");
    }
}

#[test]
fn extreme_arguments_across_the_binades() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let args = [
        BigFloat::zero(),
        BigFloat::one(),
        pow2(-60),
        pow2(-200),
        pow2(30),
        -BigFloat::one(),
        -pow2(-60),
    ];
    let sqrt_x = apply(&mut it, BaseFn::Sqrt, x);
    let exp_x = apply(&mut it, BaseFn::Exp, x);
    let erf_x = apply(&mut it, BaseFn::Erf, x);
    let erfc_x = apply(&mut it, BaseFn::Erfc, x);
    let atan_x = apply(&mut it, BaseFn::Atan, x);
    let exprs = [sqrt_x, exp_x, erf_x, erfc_x, atan_x];
    for &id in &exprs {
        for arg in &args {
            let (result, stats) = eval(&it, id, arg, P);
            assert!(
                result.is_faithful()
                    || result.is_non_real()
                    || result.quality().is_below_cutoff()
                    || result.quality() == Quality::NotFaithful,
                "{:?} at {arg}: certificate {:?}",
                it.get(id),
                result.quality()
            );
            assert!(stats.final_precision() <= 512 * P);
            if result.is_correctly_rounded() && !result.is_non_real() {
                let reference = reference(&it, id, arg, P, 100);
                if reference.is_finite() {
                    assert_eq!(
                        result.value(),
                        &reference.round_to(P, Rounding::Nearest),
                        "{:?} at {arg}",
                        it.get(id)
                    );
                }
            }
        }
    }
}

#[test]
fn extreme_shared_subtree_in_a_deep_tree_is_certified() {
    // Repeated sqrt of a shared subtree: a deeper DAG still terminates inside
    // the budget and stays faithful (sqrt^n(2) stays above 1 and tends to it).
    let mut it = Interner::new();
    let two = c(&mut it, 2);
    let mut node = two;
    for _ in 0..20 {
        node = apply(&mut it, BaseFn::Sqrt, node);
    }
    let doubled = it.intern(Expr::Add(node, node));
    let (result, stats) = eval(&it, doubled, &BigFloat::zero(), P);
    assert!(stats.final_precision() <= 512 * P);
    assert!(result.is_faithful(), "{:?}", result.quality());
    let reference = reference(&it, doubled, &BigFloat::zero(), P, 200);
    assert_eq!(result.value(), &reference.round_to(P, Rounding::Nearest));
    // sqrt^20(2) = 2^(1/2^20) = 1.00000066..., so twice that is just above 2.
    assert!(*result.value() > BigFloat::from_f64(2.0));
    assert!(*result.value() < BigFloat::from_f64(2.000002));
}

// ---- Error cases (异常报错) -------------------------------------------

#[test]
fn error_hook_can_make_the_engine_give_up_honestly() {
    // A hook is the caller's own enclosure; a permanently loose one must not
    // be dressed up as a correct rounding.
    let mut it = Interner::new();
    let one = c(&mut it, 1);
    let sin1 = apply(&mut it, BaseFn::Sin, one);

    struct Useless;
    impl IntervalFnHook for Useless {
        fn apply_fn(&self, f: BaseFn, _arg: &Interval, _prec: usize) -> Option<Interval> {
            (f == BaseFn::Sin)
                .then(|| Interval::new(BigFloat::from_i64(0), BigFloat::from_i64(1_000_000)))
        }
    }

    let (result, stats) =
        evaluate_faithful(&it, sin1, &BigFloat::zero(), &ctx(2), None, Some(&Useless));
    assert_eq!(result.quality(), Quality::NotFaithful);
    assert_eq!(stats.final_precision(), 1024);
    let (plain, _) = eval(&it, sin1, &BigFloat::zero(), 2);
    assert!(plain.is_correctly_rounded(), "without the hook: {plain:?}");
    assert_ne!(result.value(), plain.value());
}

#[test]
fn error_nan_operand_anywhere_in_the_tree_is_nonreal() {
    let mut it = Interner::new();
    let nan = it.intern(Expr::Constant(BigFloat::NAN));
    let one = c(&mut it, 1);
    let x = var(&mut it);
    let add = it.intern(Expr::Add(nan, one));
    let mul = it.intern(Expr::Mul(nan, x));
    let cos_nan = apply(&mut it, BaseFn::Cos, nan);
    let pow_nan = it.intern(Expr::Pow(nan, one));
    let sin_of_x = apply(&mut it, BaseFn::Sin, x);
    for id in [add, mul, cos_nan, pow_nan] {
        let (result, _) = eval(&it, id, &BigFloat::one(), P);
        assert_eq!(result.quality(), Quality::NonReal, "{:?}", it.get(id));
        assert!(result.value().is_nan());
    }
    // The same expression with a NaN *input* is non-real too.
    let (result, _) = eval(&it, sin_of_x, &BigFloat::NAN, P);
    assert_eq!(result.quality(), Quality::NonReal);
}

#[test]
fn error_pole_crossing_is_reported_not_silently_numbered() {
    let mut it = Interner::new();
    let one = c(&mut it, 1);
    let x = var(&mut it);
    let recip = it.intern(Expr::Div(one, x));
    let (result, _) = eval(&it, recip, &BigFloat::zero(), P);
    assert_eq!(result.quality(), Quality::NonReal);
    assert!(
        result.value().is_nan(),
        "1/0 over a zero interval: {result:?}"
    );
    // exp(+infinity) is an exact infinity: non-real, with that infinity as the
    // reported value.
    let exp_x = apply(&mut it, BaseFn::Exp, x);
    let (result, _) = eval(&it, exp_x, &BigFloat::infinity(), P);
    assert_eq!(result.quality(), Quality::NonReal);
    assert_eq!(result.value(), &BigFloat::infinity());
    // erf(+infinity) = 1 is a finite limit, so the engine certifies a real
    // value here rather than reporting a non-real conclusion. The backend
    // attaches a positive bound even to the exact limit, so the enclosure has
    // width and Exact is not claimed.
    let erf_x = apply(&mut it, BaseFn::Erf, x);
    let (result, _) = eval(&it, erf_x, &BigFloat::infinity(), P);
    assert!(
        !result.is_non_real(),
        "erf(+inf) = 1: {:?}",
        result.quality()
    );
    assert_eq!(result.value(), &BigFloat::one());
    assert!(result.is_faithful());
}

// ---- Interruption / robustness cases (异常中断) -----------------------

#[test]
fn robustness_special_value_matrix_never_panics_or_hangs() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let args = [
        BigFloat::NAN,
        BigFloat::infinity(),
        BigFloat::neg_infinity(),
        BigFloat::zero(),
        -BigFloat::zero(),
        BigFloat::one(),
        pow2(-500),
        pow2(30),
    ];
    let sqrt_x = apply(&mut it, BaseFn::Sqrt, x);
    let ln_x = apply(&mut it, BaseFn::Ln, x);
    let exp_x = apply(&mut it, BaseFn::Exp, x);
    let erf_x = apply(&mut it, BaseFn::Erf, x);
    let erfc_x = apply(&mut it, BaseFn::Erfc, x);
    let tan_x = apply(&mut it, BaseFn::Tan, x);
    let acosh_x = apply(&mut it, BaseFn::Acosh, x);
    let self_div = it.intern(Expr::Div(x, x));
    for &id in &[sqrt_x, ln_x, exp_x, erf_x, erfc_x, tan_x, acosh_x, self_div] {
        for arg in &args {
            for prec in [2usize, 53] {
                let (result, stats) = eval(&it, id, arg, prec);
                assert!(Quality::from_code(result.quality().code()).is_some());
                assert!(stats.rounds() >= 1);
                assert!(stats.final_precision() <= 512 * prec);
            }
        }
    }
}

#[test]
fn robustness_cutoff_and_hook_combined_is_still_deterministic() {
    let mut it = Interner::new();
    let zero = c(&mut it, 0);
    let exp0 = apply(&mut it, BaseFn::Exp, zero);
    let cutoff = pow2(-1);
    let first = evaluate_faithful(
        &it,
        exp0,
        &BigFloat::zero(),
        &ctx(2),
        Some(&cutoff),
        Some(&ExpLoose),
    );
    let second = evaluate_faithful(
        &it,
        exp0,
        &BigFloat::zero(),
        &ctx(2),
        Some(&cutoff),
        Some(&ExpLoose),
    );
    assert_eq!(first.0, second.0);
    assert_eq!(first.1, second.1);
    // exp(0) = 1 is not inside [-0.5, 0.5], so the cutoff does not fire; the
    // loose hook still yields Test 3 and the retry round agrees.
    assert_eq!(first.0.quality(), Quality::Faithful);
    // With a cutoff that does swallow the enclosure, BelowCutoff wins over the
    // faithful conclusion (its decision order is non-real, cutoff, tests).
    let wide = BigFloat::from_i64(2);
    let (result, _) = evaluate_faithful(
        &it,
        exp0,
        &BigFloat::zero(),
        &ctx(2),
        Some(&wide),
        Some(&ExpLoose),
    );
    assert_eq!(result.quality(), Quality::BelowCutoff);
    assert!(result.value().is_zero());
}

#[test]
fn robustness_certificate_error_bounds_hold_for_every_state() {
    // Faithful => within one target spacing of the true value; correctly
    // rounded => within half of it. This is the certificates' meaning (task C).
    let mut it = Interner::new();
    let x = var(&mut it);
    let one = c(&mut it, 1);
    let two = c(&mut it, 2);
    let three = c(&mut it, 3);
    let sqrt2 = apply(&mut it, BaseFn::Sqrt, two);
    let exp_x = apply(&mut it, BaseFn::Exp, x);
    let erf_x = apply(&mut it, BaseFn::Erf, x);
    let third = it.intern(Expr::Div(one, three));
    let pi = it.intern(Expr::Pi);
    for &id in &[sqrt2, exp_x, erf_x, third, pi] {
        for prec in [2usize, 10, 53] {
            let arg = BigFloat::one();
            let (result, _) = eval(&it, id, &arg, prec);
            let exact = eval_point(&it, id, &arg, prec + 200);
            let err = (result.value().clone() - exact).abs();
            let ulp = spacing(result.value(), prec);
            if result.is_correctly_rounded() {
                let doubled = err.clone() * BigFloat::from_i64(2);
                assert!(
                    doubled <= ulp.clone(),
                    "{:?}@{prec}: error {err} exceeds half a spacing {ulp}",
                    it.get(id)
                );
            } else if result.is_faithful() {
                assert!(err <= ulp.clone(), "{:?}@{prec}: not faithful", it.get(id));
            } else if result.quality().is_below_cutoff() {
                assert!(result.value().is_zero());
            } else {
                assert!(
                    result.is_non_real() || result.quality() == Quality::NotFaithful,
                    "{:?}@{prec}: unclassified certificate {:?}",
                    it.get(id),
                    result.quality()
                );
            }
        }
    }
}

// ---- Taylor intersection integration (M2-D) ---------------------------

#[test]
fn normal_taylor_intersection_collapses_a_dependency_zero() {
    // f = x - x is identically zero. At a two-bit target the naive enclosure
    // of a wide input straddles zero with representable endpoints, which no
    // certification test can resolve (without the refinement the engine would
    // escalate to the budget and report NotFaithful, the same shape as
    // `extreme_exact_zero_from_a_margined_kernel_escalates_to_not_faithful`).
    // The mean-value form evaluates f(m) = m - m = 0 exactly and f' = 1 - 1 = 0,
    // so the intersection collapses to the point zero and Test 1 certifies
    // Exact in the first round.
    let mut it = Interner::new();
    let x = var(&mut it);
    let f = it.intern(Expr::Sub(x, x));
    let third = BigFloat::one() / BigFloat::from_i64(3);
    let (result, stats) = eval(&it, f, &third, 2);
    assert_eq!(result.quality(), Quality::Exact);
    assert!(result.value().is_zero());
    assert!(result.value().is_sign_positive());
    assert_eq!(stats.rounds(), 1);
    assert_eq!(stats.final_precision(), 2 + 10);
}

#[test]
fn normal_taylor_recursions_from_context_collapse_the_pathology() {
    // f = x*x - (x-1)*(x+1) - 1 is identically zero but dependency-heavy.
    // With k = 0 the slope f'(xI) still carries dependency, so the early
    // rounds stay unresolved; with k = 1 (ctx.with_taylor_recursions) the
    // rule is re-applied to the derivative, whose own derivative is the
    // constant 2 - 2 = 0, and the enclosure collapses to the point zero at
    // once. The input 1/2 + 2^-40 has a dyadic midpoint (1/2 + 2^-13) at the
    // first working precision, so f(m) and f'(m) are exact at the guard width.
    let mut it = Interner::new();
    let x = var(&mut it);
    let one = c(&mut it, 1);
    let xx = it.intern(Expr::Mul(x, x));
    let xm1 = it.intern(Expr::Sub(x, one));
    let xp1 = it.intern(Expr::Add(x, one));
    let prod = it.intern(Expr::Mul(xm1, xp1));
    let diff = it.intern(Expr::Sub(xx, prod));
    let f = it.intern(Expr::Sub(diff, one));
    let arg = pow2(-1) + pow2(-40);

    let k1 = ctx(2).with_taylor_recursions(1);
    let (result, stats) = evaluate_faithful(&it, f, &arg, &k1, None, None);
    assert_eq!(result.quality(), Quality::Exact);
    assert!(result.value().is_zero());
    assert_eq!(stats.rounds(), 1, "k = 1 collapses in the first round");
    assert_eq!(stats.final_precision(), 12);

    // k = 0 needs the loop to grow the input into a point before the naive
    // walk itself collapses: same value, strictly more rounds.
    let (result0, stats0) = eval(&it, f, &arg, 2);
    assert_eq!(result0.quality(), Quality::Exact);
    assert!(result0.value().is_zero());
    assert!(stats0.rounds() > 1, "k = 0 cannot collapse immediately");
    assert!(stats0.final_precision() <= 512 * 2);
}

#[test]
fn normal_intersection_preserves_known_correct_roundings() {
    // Point inputs are the documented eval_taylor fallback, so wrapping the
    // known-table constants in the variable must not change value,
    // certificate, or statistics.
    let mut it = Interner::new();
    let x = var(&mut it);
    let two = c(&mut it, 2);
    let one = c(&mut it, 1);
    let sqrt_x = apply(&mut it, BaseFn::Sqrt, x);
    let sqrt2 = apply(&mut it, BaseFn::Sqrt, two);
    let exp_x = apply(&mut it, BaseFn::Exp, x);
    let exp1 = apply(&mut it, BaseFn::Exp, one);
    let erf_x = apply(&mut it, BaseFn::Erf, x);
    let erf1 = apply(&mut it, BaseFn::Erf, one);
    let cases = [
        (sqrt_x, sqrt2, BigFloat::from_i64(2)),
        (exp_x, exp1, BigFloat::one()),
        (erf_x, erf1, BigFloat::one()),
    ];
    for (vid, cid, arg) in cases {
        let via_var = eval(&it, vid, &arg, P);
        let via_const = eval(&it, cid, &BigFloat::zero(), P);
        assert_eq!(via_var.0, via_const.0, "value/certificate drifted");
        assert_eq!(via_var.1, via_const.1, "statistics drifted");
        assert!(via_var.0.is_correctly_rounded());
    }
}

#[test]
fn normal_wide_input_sqrt_still_correctly_rounds_with_the_intersection() {
    // A genuinely wide input (more significant bits than the working
    // precision) through a nontrivial expression: the intersection may only
    // narrow, so the first-round Test 1 conclusion is unchanged.
    let mut it = Interner::new();
    let x = var(&mut it);
    let sqrt_x = apply(&mut it, BaseFn::Sqrt, x);
    let arg = BigFloat::from_i64(2) + pow2(-70);
    let (result, stats) = eval(&it, sqrt_x, &arg, P);
    assert_eq!(result.quality(), Quality::CorrectlyRounded);
    assert_eq!(
        result.value(),
        &BigFloat::from_f64(std::f64::consts::SQRT_2)
    );
    assert_eq!(stats.rounds(), 1);
    let reference = reference(&it, sqrt_x, &arg, P, 100);
    assert_eq!(result.value(), &reference.round_to(P, Rounding::Nearest));
}

#[test]
fn error_nonreal_variable_expressions_stay_nonreal() {
    // The refinement must not resurrect a non-real conclusion: ln(0), 1/0
    // and sqrt(-1) through the variable keep the contract's decision order
    // (non-real is decided before any refinement).
    let mut it = Interner::new();
    let x = var(&mut it);
    let one = c(&mut it, 1);
    let ln_x = apply(&mut it, BaseFn::Ln, x);
    let recip = it.intern(Expr::Div(one, x));
    let sqrt_x = apply(&mut it, BaseFn::Sqrt, x);
    let (result, stats) = eval(&it, ln_x, &BigFloat::zero(), P);
    assert_eq!(result.quality(), Quality::NonReal);
    assert_eq!(result.value(), &BigFloat::neg_infinity());
    assert_eq!(stats.rounds(), 1);
    let (result, stats) = eval(&it, recip, &BigFloat::zero(), P);
    assert_eq!(result.quality(), Quality::NonReal);
    assert!(result.value().is_nan());
    assert_eq!(stats.rounds(), 1);
    let (result, stats) = eval(&it, sqrt_x, &BigFloat::from_i64(-1), P);
    assert_eq!(result.quality(), Quality::NonReal);
    assert!(result.value().is_nan());
    assert_eq!(stats.rounds(), 1);
}

#[test]
fn robustness_taylor_integration_is_deterministic() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let f = it.intern(Expr::Sub(x, x));
    let third = BigFloat::one() / BigFloat::from_i64(3);
    let first = eval(&it, f, &third, 2);
    let second = eval(&it, f, &third, 2);
    assert_eq!(first.0, second.0);
    assert_eq!(first.1, second.1);
    // The recursion setting is part of the input: same k, same answer.
    let kctx = ctx(2).with_taylor_recursions(2);
    let a = evaluate_faithful(&it, f, &third, &kctx, None, None);
    let b = evaluate_faithful(&it, f, &third, &kctx, None, None);
    assert_eq!(a.0, b.0);
    assert_eq!(a.1, b.1);
    assert_eq!(a.0.quality(), Quality::Exact);
}

// ---- M2-E2 additions: the new functions through the engine --------------

#[test]
fn normal_m2e2_exact_new_function_values_certify_exact() {
    let mut it = Interner::new();
    let eight = c(&mut it, 8);
    let thousand = c(&mut it, 1000);
    let zero = c(&mut it, 0);
    let log2_8 = apply(&mut it, BaseFn::Log2, eight);
    let log10_1000 = apply(&mut it, BaseFn::Log10, thousand);
    let log1p_0 = apply(&mut it, BaseFn::Log1p, zero);
    let expm1_0 = apply(&mut it, BaseFn::Expm1, zero);
    let cases = [
        ("log2(8)", log2_8, BigFloat::from_i64(3)),
        ("log10(1000)", log10_1000, BigFloat::from_i64(3)),
        ("log1p(0)", log1p_0, BigFloat::zero()),
        ("expm1(0)", expm1_0, BigFloat::zero()),
    ];
    for (name, id, want) in cases {
        let (result, stats) = eval(&it, id, &BigFloat::zero(), P);
        assert_eq!(result.value(), &want, "{name}: {result:?}");
        assert_eq!(result.quality(), Quality::Exact, "{name}: {result:?}");
        assert!(stats.rounds() >= 1);
    }
}

#[test]
fn normal_m2e2_rounding_functions_certify_exact_at_points() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let abs_x = apply(&mut it, BaseFn::Abs, x);
    let ceil_x = apply(&mut it, BaseFn::Ceil, x);
    let floor_x = apply(&mut it, BaseFn::Floor, x);
    let near_x = apply(&mut it, BaseFn::NearestInt, x);
    let cases = [
        (abs_x, bf_arg(-2.5), BigFloat::from_f64(2.5)),
        (ceil_x, bf_arg(2.5), BigFloat::from_i64(3)),
        (floor_x, bf_arg(2.5), BigFloat::from_i64(2)),
        // Ties-to-even through the whole engine.
        (near_x, bf_arg(2.5), BigFloat::from_i64(2)),
        (near_x, bf_arg(3.5), BigFloat::from_i64(4)),
        (near_x, bf_arg(-2.5), BigFloat::from_i64(-2)),
        (ceil_x, bf_arg(-2.5), BigFloat::from_i64(-2)),
        (floor_x, bf_arg(-2.5), BigFloat::from_i64(-3)),
    ];
    for (id, arg, want) in cases {
        let (result, _) = eval(&it, id, &arg, P);
        assert_eq!(result.value(), &want, "{result:?} at {arg}");
        assert_eq!(result.quality(), Quality::Exact, "{result:?} at {arg}");
    }
}

/// A binary64 argument helper (keeps the test table readable).
fn bf_arg(v: f64) -> BigFloat {
    BigFloat::from_f64(v)
}

#[test]
fn normal_m2e2_format_conversions_certify_correct_roundings() {
    let mut it = Interner::new();
    let three = c(&mut it, 3);
    let eight = c(&mut it, 8);
    let one = c(&mut it, 1);
    let third = it.intern(Expr::Div(one, three));
    // An exact dyadic (3/8) lies on every format grid: the directed endpoint
    // images coincide, the enclosure is a point, and the engine certifies
    // Exact for all five conversions.
    let exact_arg = it.intern(Expr::Div(three, eight));
    for f in [
        BaseFn::Double,
        BaseFn::Single,
        BaseFn::HalfPrecision,
        BaseFn::Quad,
        BaseFn::DoubleExtended,
    ] {
        let id = apply(&mut it, f, exact_arg);
        let (result, _) = eval(&it, id, &BigFloat::zero(), P);
        assert_eq!(
            result.value(),
            &BigFloat::from_f64(0.375),
            "{f:?}: {result:?}"
        );
        assert_eq!(result.quality(), Quality::Exact, "{f:?}: {result:?}");
    }
    // 1/3 is not on any format grid, but format conversion is round-to-
    // nearest: the nearest endpoint images of the argument point coincide,
    // so the enclosure collapses to the single nearest format float. At the
    // format's own target width that point is exactly representable and
    // Test 1 certifies Exact with the nearest image. (M2-E2b correction:
    // the former directed endpoints pinned Test 2's lower-neighbor choice,
    // a faithful but not nearest rounding — the wrong expectation for
    // Sollya's round-to-nearest format semantics.)
    let wide = (BigFloat::one() / BigFloat::from_i64(3)).with_precision(200);
    let double_x = apply(&mut it, BaseFn::Double, third);
    let single_x = apply(&mut it, BaseFn::Single, third);
    let half_x = apply(&mut it, BaseFn::HalfPrecision, third);
    let quad_x = apply(&mut it, BaseFn::Quad, third);
    let de_x = apply(&mut it, BaseFn::DoubleExtended, third);
    let cases = [
        (double_x, 53usize, wide.round_to(53, Rounding::Nearest)),
        (single_x, 24, wide.round_to(24, Rounding::Nearest)),
        (half_x, 11, wide.round_to(11, Rounding::Nearest)),
        (quad_x, 113, wide.round_to(113, Rounding::Nearest)),
        (de_x, 64, wide.round_to(64, Rounding::Nearest)),
    ];
    for (id, target, want) in cases {
        let (result, _) = eval(&it, id, &BigFloat::zero(), target);
        assert_eq!(result.value(), &want, "{result:?} at {target}");
        assert_eq!(result.quality(), Quality::Exact, "{result:?}");
    }
    // At the binary64 target the narrow images (double/single/half) are
    // exactly representable and stay Exact. The wide images (quad/de) exceed
    // the round's working precision (target + 10), so the final outward
    // rounding leaves a two-float enclosure: Test 1 still certifies the
    // correct rounding, without the point-exactness claim.
    for id in [double_x, single_x, half_x] {
        let (result, _) = eval(&it, id, &BigFloat::zero(), P);
        assert_eq!(result.quality(), Quality::Exact, "{result:?}");
    }
    for id in [quad_x, de_x] {
        let (result, _) = eval(&it, id, &BigFloat::zero(), P);
        assert_eq!(result.value(), &BigFloat::from_f64(1.0 / 3.0), "{result:?}");
        assert_eq!(result.quality(), Quality::CorrectlyRounded, "{result:?}");
        assert!(result.is_correctly_rounded(), "{result:?}");
    }
}

#[test]
fn error_m2e2_new_domain_errors_are_nonreal() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let log2_x = apply(&mut it, BaseFn::Log2, x);
    let log10_x = apply(&mut it, BaseFn::Log10, x);
    let log1p_x = apply(&mut it, BaseFn::Log1p, x);
    // Negative arguments: NaN.
    for id in [log2_x, log10_x] {
        let (result, stats) = eval(&it, id, &BigFloat::from_i64(-1), P);
        assert_eq!(result.quality(), Quality::NonReal, "{result:?}");
        assert!(result.value().is_nan());
        assert_eq!(stats.rounds(), 1);
    }
    let (result, _) = eval(&it, log1p_x, &BigFloat::from_i64(-2), P);
    assert_eq!(result.quality(), Quality::NonReal);
    assert!(result.value().is_nan());
    // The poles: -infinity, reported as the offending endpoint.
    for (id, arg) in [
        (log2_x, BigFloat::zero()),
        (log10_x, BigFloat::zero()),
        (log1p_x, -BigFloat::one()),
    ] {
        let (result, _) = eval(&it, id, &arg, P);
        assert_eq!(result.quality(), Quality::NonReal, "{result:?}");
        assert_eq!(result.value(), &BigFloat::neg_infinity());
    }
}

#[test]
fn normal_m2e2_non_smooth_pipeline_with_taylor_recursions() {
    // The engine's Taylor refinement must fall back for non-smooth
    // expressions and still certify the point value (the intersection with
    // the naive enclosure is a no-op, never a widening).
    let mut it = Interner::new();
    let x = var(&mut it);
    let two = c(&mut it, 2);
    let ceil_x = apply(&mut it, BaseFn::Ceil, x);
    let f = it.intern(Expr::Add(ceil_x, x));
    let kctx = ctx(P).with_taylor_recursions(1);
    let arg = BigFloat::from_f64(1.25);
    let (result, _) = evaluate_faithful(&it, f, &arg, &kctx, None, None);
    assert_eq!(result.value(), &BigFloat::from_f64(3.25));
    assert_eq!(result.quality(), Quality::Exact, "{result:?}");
    // double(x) * 2 at a binary64 argument: exact scaling of an exact image.
    let double_x = apply(&mut it, BaseFn::Double, x);
    let g = it.intern(Expr::Mul(double_x, two));
    let third = BigFloat::from_f64(1.0 / 3.0);
    let (result, _) = evaluate_faithful(&it, g, &third, &kctx, None, None);
    assert_eq!(result.value(), &BigFloat::from_f64(2.0 / 3.0));
    assert_eq!(result.quality(), Quality::Exact, "{result:?}");
    // A wide input makes the round's argument interval non-point, so the
    // Taylor refinement is actually consulted and must fall back: ceil(x)+x
    // at the stored 1/3 value is 4/3, correctly rounded.
    let wide_third = BigFloat::one() / BigFloat::from_i64(3);
    let (result, _) = evaluate_faithful(&it, f, &wide_third, &kctx, None, None);
    assert_eq!(result.value(), &BigFloat::from_f64(4.0 / 3.0));
    assert!(result.is_correctly_rounded(), "{result:?}");
    // Determinism with the recursion setting active.
    let a = evaluate_faithful(&it, f, &arg, &kctx, None, None);
    let b = evaluate_faithful(&it, f, &arg, &kctx, None, None);
    assert_eq!(a.0, b.0);
    assert_eq!(a.1, b.1);
}

#[test]
fn robustness_m2e2_new_functions_special_matrix_never_panics() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let fns = [
        BaseFn::Log2,
        BaseFn::Log10,
        BaseFn::Log1p,
        BaseFn::Expm1,
        BaseFn::Abs,
        BaseFn::Ceil,
        BaseFn::Floor,
        BaseFn::NearestInt,
        BaseFn::Double,
        BaseFn::Single,
        BaseFn::HalfPrecision,
        BaseFn::Quad,
        BaseFn::DoubleExtended,
    ];
    let args = [
        BigFloat::NAN,
        BigFloat::infinity(),
        BigFloat::neg_infinity(),
        BigFloat::zero(),
        -BigFloat::zero(),
        BigFloat::from_f64(2.5),
    ];
    for f in fns {
        let id = apply(&mut it, f, x);
        for arg in &args {
            let (first, _) = eval(&it, id, arg, P);
            let (second, _) = eval(&it, id, arg, P);
            // FpResult equality goes through BigFloat's IEEE-style PartialEq
            // (NaN != NaN), so determinism is compared by certificate and
            // canonical bits instead.
            assert_eq!(first.quality(), second.quality(), "{f:?} at {arg} drifted");
            assert_eq!(
                first.value().canonical_parts(),
                second.value().canonical_parts(),
                "{f:?} at {arg} drifted"
            );
        }
    }
}
