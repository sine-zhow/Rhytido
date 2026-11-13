//! Integration tests for the evaluation engine (AGENTS.md §4, third layer):
//! `expr` interning + `interval_eval` enclosures + `faithful` certification +
//! `eval` point evaluation + `context` threading, cross-checked against each
//! other and against the backend facade.
//!
//! This file is the certificate-reachability audit for the eight states of
//! `Quality` (task C acceptance: "8 态可达性"), the eval_point/eval_interval
//! cross-check, the hook injection pipeline, and the Context-driven semantics.

use std::sync::atomic::{AtomicUsize, Ordering};

use rhytido_backend::{BigFloat, Interval, Rounding};
use rhytido_core::{
    BaseFn, Context, Expr, IntervalFnHook, Quality, eval_interval, eval_point, eval_taylor,
    evaluate_faithful, print_expr,
};
use rhytido_core::{Interner, NodeId};

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

fn point(x: &BigFloat) -> Interval {
    Interval::point(x.clone())
}

/// Exact power of two `2^n`, any sign of `n`.
fn pow2(n: i64) -> BigFloat {
    let k = BigFloat::from_i64(n.abs());
    let p = BigFloat::from_i64(2).pow(&k);
    if n >= 0 { p } else { BigFloat::one() / p }
}

/// Local spacing of the `prec` grid at `v`: the larger neighbour distance, so
/// binade edges cannot undercut a bound expressed in "one ulp". `v` must be a
/// finite, nonzero value representable at `prec`.
fn ulp(v: &BigFloat, prec: usize) -> BigFloat {
    let up = v.next_above(prec) - v.clone();
    let down = v.clone() - v.next_below(prec);
    if up > down { up } else { down }
}

// ---- Certificate reachability (task C: all eight states) --------------

/// Hook answering `exp` with [0.8, 1.4]: a valid enclosure of exp(0) = 1 that
/// holds exactly one 2-bit float, which is the shape Test 3 needs.
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

/// Hook supplying a rigorous *point* enclosure for erf at exactly zero, which
/// the kernel can only report together with a positive error bound.
struct ErfExactZero;

impl IntervalFnHook for ErfExactZero {
    fn apply_fn(&self, f: BaseFn, arg: &Interval, _prec: usize) -> Option<Interval> {
        if f != BaseFn::Erf || !arg.is_point() {
            return None;
        }
        let value = arg.lower().expect("point");
        if !value.is_zero() {
            return None;
        }
        Some(Interval::point(value))
    }
}

#[test]
fn normal_all_eight_certificate_states_are_reachable() {
    let mut it = Interner::new();
    let zero = c(&mut it, 0);
    let two = c(&mut it, 2);
    let three = c(&mut it, 3);
    let x = var(&mut it);

    let sum = it.intern(Expr::Add(two, three));
    let ln0 = apply(&mut it, BaseFn::Ln, zero);
    let sqrt2 = apply(&mut it, BaseFn::Sqrt, two);
    let exp0 = apply(&mut it, BaseFn::Exp, zero);
    let erf0 = apply(&mut it, BaseFn::Erf, zero);

    // 4 Exact: an exactly representable point at p = 3.
    let exact = evaluate_faithful(&it, sum, &BigFloat::zero(), &ctx(3), None, None).0;
    // 6 CorrectlyRounded: an irrational value at double precision.
    let cr = evaluate_faithful(&it, sqrt2, &BigFloat::zero(), &ctx(53), None, None).0;
    // 7 CorrectlyRoundedInexact: a tie point the target cannot represent.
    let tie_point = BigFloat::from_i64(5) / pow2(3);
    let cri = evaluate_faithful(&it, x, &tie_point, &ctx(2), None, None).0;
    // 5 FaithfulInexact: the same tie crossed by an enclosure of real width.
    let wide_tie = BigFloat::from_i64(5) / pow2(3) + pow2(-40);
    let fi = evaluate_faithful(&it, x, &wide_tie, &ctx(2), None, None).0;
    // 1 Faithful: Test 3, reached through a caller enclosure (audit below).
    let faithful =
        evaluate_faithful(&it, exp0, &BigFloat::zero(), &ctx(2), None, Some(&ExpLoose)).0;
    // 2 BelowCutoff: a negligible enclosure inside the cutoff band.
    let cutoff = pow2(-50);
    let tiny = pow2(-60);
    let below = evaluate_faithful(&it, x, &tiny, &ctx(53), Some(&cutoff), None).0;
    // 3 NonReal: ln(0) is -infinity.
    let nonreal = evaluate_faithful(&it, ln0, &BigFloat::zero(), &ctx(53), None, None).0;
    // 0 NotFaithful: the precision budget runs out (natural pipeline).
    let nf = evaluate_faithful(&it, erf0, &BigFloat::zero(), &ctx(53), None, None).0;

    let cases = [
        (Quality::Exact, "2+3@p=3", &exact),
        (Quality::CorrectlyRounded, "sqrt(2)@p=53", &cr),
        (Quality::CorrectlyRoundedInexact, "5/8@p=2", &cri),
        (Quality::FaithfulInexact, "tie crossed", &fi),
        (Quality::Faithful, "unique float", &faithful),
        (Quality::BelowCutoff, "cutoff", &below),
        (Quality::NonReal, "ln(0)", &nonreal),
        (Quality::NotFaithful, "budget spent", &nf),
    ];
    let mut codes = Vec::new();
    for (want, label, result) in cases {
        assert_eq!(result.quality(), want, "state {label}");
        codes.push(result.quality().code());
    }
    codes.sort_unstable();
    assert_eq!(
        codes,
        vec![0, 1, 2, 3, 4, 5, 6, 7],
        "all eight states reached"
    );
}

#[test]
fn normal_state_one_needs_a_caller_enclosure_reachability_audit() {
    // Why Test 3 (certificate Faithful(1)) cannot fire in the natural pipeline:
    // eval_interval evaluates the DAG at prec + guard bits and rounds only the
    // result outward to `prec`, so an enclosure is at most a couple of ulps of
    // the *working* precision wide. Test 3 needs an enclosure spanning more
    // than half a *target* ulp on both sides of the single contained float. The
    // faithful loop always calls with prec >= p + 10, so the natural width is
    // at most 2^-9 of a target ulp and Test 1 or Test 2 always decides first.
    // The substitute coverage is therefore a caller-supplied valid enclosure.
    let mut it = Interner::new();
    let zero = c(&mut it, 0);
    let exp0 = apply(&mut it, BaseFn::Exp, zero);
    let pi = it.intern(Expr::Pi);

    // Natural: exp(0) is the exact point 1, so Test 1 concludes with Exact.
    let (natural, stats) = evaluate_faithful(&it, exp0, &BigFloat::zero(), &ctx(53), None, None);
    assert_eq!(natural.quality(), Quality::Exact);
    assert_eq!(stats.rounds(), 1);

    // With the loose caller enclosure at a two-bit target, Test 3 fires.
    let (hooked, hstats) =
        evaluate_faithful(&it, exp0, &BigFloat::zero(), &ctx(2), None, Some(&ExpLoose));
    assert_eq!(hooked.quality(), Quality::Faithful);
    assert_eq!(hooked.quality().code(), 1);
    assert_eq!(hooked.value(), &BigFloat::one());
    assert_eq!(hstats.rounds(), 2, "the retry cannot improve on Test 3");

    // Even the deliberately loose rigorous pi bracket at a coarse target lands
    // on Test 1 or Test 2, never on Test 3: the natural width argument holds
    // for the widest enclosure the layer produces.
    let (pi_result, pi_stats) = evaluate_faithful(&it, pi, &BigFloat::zero(), &ctx(2), None, None);
    assert_ne!(pi_result.quality(), Quality::Faithful);
    assert!(pi_result.is_faithful());
    assert!(pi_stats.final_precision() <= 2 * 512);
    assert_eq!(
        pi_result.value(),
        &BigFloat::pi_with_precision(400).round_to(2, Rounding::Nearest)
    );
}

#[test]
fn normal_not_faithful_is_reachable_naturally_and_through_a_loose_hook() {
    // Two independent constructions of certificate 0.
    // (a) natural: erf(0) is exactly 0, but the kernel's checked bound makes
    //     the enclosure [-2^-w, +2^-w]. With the backend's unbounded exponent
    //     range both endpoints stay representable at the target and are not
    //     adjacent, and Test 2 is disabled across zero, so the loop escalates
    //     to 512 * p and reports NotFaithful honestly.
    // (b) caller: a permanently loose hook enclosure.
    let mut it = Interner::new();
    let zero = c(&mut it, 0);
    let one = c(&mut it, 1);
    let erf0 = apply(&mut it, BaseFn::Erf, zero);
    let erf1 = apply(&mut it, BaseFn::Erf, one);

    let (natural, stats) = evaluate_faithful(&it, erf0, &BigFloat::zero(), &ctx(53), None, None);
    assert_eq!(natural.quality(), Quality::NotFaithful);
    assert_eq!(
        stats.final_precision(),
        512 * 53,
        "the budget stopped the loop"
    );
    assert!(!natural.is_faithful());
    // The conservative value is the lower endpoint's target image: tiny and
    // signed like the enclosure's lower side, never a fabricated plausible one.
    assert!(natural.value().is_sign_negative());
    assert!(natural.value().abs() < pow2(-50));

    struct AlwaysWide;
    impl IntervalFnHook for AlwaysWide {
        fn apply_fn(&self, f: BaseFn, _arg: &Interval, _prec: usize) -> Option<Interval> {
            (f == BaseFn::Erf).then(|| Interval::new(BigFloat::from_f64(-1.0), BigFloat::one()))
        }
    }
    let (hooked, hstats) = evaluate_faithful(
        &it,
        erf1,
        &BigFloat::zero(),
        &ctx(2),
        None,
        Some(&AlwaysWide),
    );
    assert_eq!(hooked.quality(), Quality::NotFaithful);
    assert_eq!(hstats.final_precision(), 2 * 512);
    assert_eq!(hooked.value(), &(-BigFloat::one()), "nearest(lo@p)");
}

// ---- Cross-module consistency: eval_point vs evaluate_faithful --------

#[test]
fn normal_faithful_values_match_the_high_precision_point_evaluator() {
    // Task C acceptance: a CR/Exact result equals the p+100 reference rounded
    // to the target; a merely faithful one is within one target ulp of it.
    let mut it = Interner::new();
    let x = var(&mut it);
    let one = c(&mut it, 1);
    let two = c(&mut it, 2);
    let three = c(&mut it, 3);
    let sqrt2 = apply(&mut it, BaseFn::Sqrt, two);
    let exp_x = apply(&mut it, BaseFn::Exp, x);
    let ln_x = apply(&mut it, BaseFn::Ln, x);
    let sin_x = apply(&mut it, BaseFn::Sin, x);
    let atan_x = apply(&mut it, BaseFn::Atan, x);
    let erf_x = apply(&mut it, BaseFn::Erf, x);
    let erfc_x = apply(&mut it, BaseFn::Erfc, x);
    let third = it.intern(Expr::Div(one, three));
    let composite = it.intern(Expr::Mul(sin_x, erf_x));
    let exprs = [
        sqrt2, exp_x, ln_x, sin_x, atan_x, erf_x, erfc_x, third, composite,
    ];
    let args = [BigFloat::one(), pow2(-3), pow2(3), BigFloat::from_f64(0.25)];
    for &id in &exprs {
        for arg in &args {
            for prec in [24usize, 53, 113] {
                let (result, stats) = evaluate_faithful(&it, id, arg, &ctx(prec), None, None);
                assert!(stats.final_precision() <= 512 * prec);
                if !result.is_faithful() {
                    // NotFaithful / NonReal are audited in their own tests.
                    continue;
                }
                let reference = eval_point(&it, id, arg, prec + 100);
                if !reference.is_finite() {
                    continue;
                }
                let err = (result.value().clone() - reference.clone()).abs();
                let spacing = ulp(result.value(), prec);
                if result.is_correctly_rounded() {
                    assert_eq!(
                        result.value(),
                        &reference.round_to(prec, Rounding::Nearest),
                        "{:?} at {arg}@{prec}: not the reference's rounding",
                        it.get(id)
                    );
                    let doubled = err.clone() * BigFloat::from_i64(2);
                    assert!(
                        doubled <= spacing.clone(),
                        "{:?} at {arg}@{prec}: CR error {err} exceeds half a spacing {spacing}",
                        it.get(id)
                    );
                } else {
                    assert!(
                        err <= spacing,
                        "{:?} at {arg}@{prec}: faithful error {err} exceeds {spacing}",
                        it.get(id)
                    );
                }
            }
        }
    }
}

#[test]
fn normal_enclosure_holds_the_point_value_and_the_certified_value() {
    // Three-way consistency at the engine's first-round width: the interval
    // layer's enclosure holds the point evaluator's value, and the certified
    // value is that enclosure's target rounding.
    let mut it = Interner::new();
    let x = var(&mut it);
    let two = c(&mut it, 2);
    let sqrt2_x = apply(&mut it, BaseFn::Sqrt, x);
    let exp2 = apply(&mut it, BaseFn::Exp, two);
    let erf2 = apply(&mut it, BaseFn::Erf, two);
    let div = it.intern(Expr::Div(two, x));
    let arg = BigFloat::one();
    for &id in &[sqrt2_x, exp2, erf2, div] {
        for prec in [8usize, 53, 165] {
            let w = prec + 10;
            let near = eval_point(&it, id, &arg, w);
            // The enclosure is taken over the *argument* interval, not over the
            // value the point evaluator produced.
            let enc = eval_interval(&it, id, &point(&arg), w, None);
            assert!(
                enc.contains(&near),
                "{:?}@{prec}: enclosure {enc} misses the point value {near}",
                it.get(id)
            );
            let (result, _) = evaluate_faithful(&it, id, &arg, &ctx(prec), None, None);
            if result.is_correctly_rounded() {
                assert_eq!(
                    result.value(),
                    &near.round_to(prec, Rounding::Nearest),
                    "{:?}@{prec} disagrees with the point evaluator",
                    it.get(id)
                );
            }
        }
    }
}

// ---- Hook injection pipeline (roadmap hooks deliverable, minimal form) -

/// Hook that counts consultations and answers ln(2) with a rigorous bracket
/// built from the caller's own high-precision value.
struct LnBracket {
    calls: AtomicUsize,
    ln_calls: AtomicUsize,
}

impl IntervalFnHook for LnBracket {
    fn apply_fn(&self, f: BaseFn, arg: &Interval, _prec: usize) -> Option<Interval> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if f != BaseFn::Ln || !arg.is_point() {
            return None;
        }
        let value = arg.lower().expect("point");
        if value != BigFloat::from_i64(2) {
            return None;
        }
        self.ln_calls.fetch_add(1, Ordering::SeqCst);
        // ln(2) is irrational: the honest custom answer is a tight bracket
        // around a 300-bit value, not a point.
        let exact = BigFloat::from_i64(2).with_precision(300).ln();
        Some(Interval::new(
            exact.clone() - pow2(-200),
            exact + pow2(-200),
        ))
    }
}

#[test]
fn normal_hook_pipeline_injects_an_enclosure_and_is_observable() {
    let mut it = Interner::new();
    let two = c(&mut it, 2);
    let ln2 = apply(&mut it, BaseFn::Ln, two);
    let hook = LnBracket {
        calls: AtomicUsize::new(0),
        ln_calls: AtomicUsize::new(0),
    };
    let (result, stats) =
        evaluate_faithful(&it, ln2, &BigFloat::zero(), &ctx(53), None, Some(&hook));
    assert_eq!(result.quality(), Quality::CorrectlyRounded);
    assert_eq!(result.value(), &BigFloat::from_f64(std::f64::consts::LN_2));
    assert_eq!(hook.calls.load(Ordering::SeqCst), 1, "one function node");
    assert_eq!(hook.ln_calls.load(Ordering::SeqCst), 1);
    assert_eq!(stats.rounds(), 1, "the injected bracket decides at w0");
}

#[test]
fn normal_hook_can_restore_a_certificate_the_kernel_cannot_prove() {
    // erf(0) = 0 exactly. The kernel's positive bound makes the natural
    // enclosure straddle zero, so the engine can only report NotFaithful(0);
    // a caller that knows better can inject the point enclosure and get
    // Exact(4). This is the purpose of the hooks delivery item.
    let mut it = Interner::new();
    let zero = c(&mut it, 0);
    let erf0 = apply(&mut it, BaseFn::Erf, zero);
    let natural = evaluate_faithful(&it, erf0, &BigFloat::zero(), &ctx(53), None, None);
    assert_eq!(natural.0.quality(), Quality::NotFaithful);
    let fixed = evaluate_faithful(
        &it,
        erf0,
        &BigFloat::zero(),
        &ctx(53),
        None,
        Some(&ErfExactZero),
    );
    assert_eq!(fixed.0.quality(), Quality::Exact);
    assert!(fixed.0.value().is_zero());
    assert_eq!(fixed.1.rounds(), 1);
}

#[test]
fn normal_hook_none_falls_through_and_the_pipeline_is_pure() {
    // Same expression through the three entry points, three times each: no
    // hidden state anywhere in the pipeline (AGENTS.md §3 zero global state).
    let mut it = Interner::new();
    let x = var(&mut it);
    let sqrt_x = apply(&mut it, BaseFn::Sqrt, x);
    let exp_x = apply(&mut it, BaseFn::Exp, x);
    let tree = it.intern(Expr::Add(sqrt_x, exp_x));
    let arg = BigFloat::from_f64(0.5);

    struct Silent;
    impl IntervalFnHook for Silent {
        fn apply_fn(&self, _f: BaseFn, _arg: &Interval, _prec: usize) -> Option<Interval> {
            None
        }
    }
    let silent = Silent;

    let mut enclosures = Vec::new();
    let mut results = Vec::new();
    for _ in 0..3 {
        enclosures.push(eval_interval(&it, tree, &point(&arg), 63, None));
        results.push(evaluate_faithful(
            &it,
            tree,
            &arg,
            &ctx(53),
            None,
            Some(&silent),
        ));
    }
    for pair in enclosures.windows(2) {
        assert_eq!(pair[0].lower(), pair[1].lower());
        assert_eq!(pair[0].upper(), pair[1].upper());
    }
    for pair in results.windows(2) {
        assert_eq!(pair[0].0, pair[1].0);
        assert_eq!(pair[0].1, pair[1].1);
    }
    // A no-op hook must not change the enclosure at all, and a second interner
    // holding the same tree must reach the same certificate.
    let with_silent = eval_interval(&it, tree, &point(&arg), 63, Some(&silent));
    assert_eq!(with_silent.lower(), enclosures[0].lower());
    assert_eq!(with_silent.upper(), enclosures[0].upper());

    let mut other = Interner::new();
    let ox = var(&mut other);
    let os = apply(&mut other, BaseFn::Sqrt, ox);
    let oe = apply(&mut other, BaseFn::Exp, ox);
    let otree = other.intern(Expr::Add(os, oe));
    let rebuilt = evaluate_faithful(&other, otree, &arg, &ctx(53), None, None);
    assert_eq!(results[0].0, rebuilt.0);
    assert_eq!(results[0].1, rebuilt.1);
}

// ---- Context-driven semantics -----------------------------------------

#[test]
fn normal_context_precision_drives_resolution_consistently() {
    // Raising the target precision refines the value, and rounding the finer
    // result back to the coarser grid reproduces it exactly: the certificates
    // are mutually consistent across Context settings.
    let mut it = Interner::new();
    let two = c(&mut it, 2);
    let sqrt2 = apply(&mut it, BaseFn::Sqrt, two);
    let mut previous = None::<(usize, BigFloat)>;
    for prec in [8usize, 24, 53, 113, 212] {
        let (result, stats) =
            evaluate_faithful(&it, sqrt2, &BigFloat::zero(), &ctx(prec), None, None);
        assert!(result.is_correctly_rounded(), "p={prec}");
        assert_eq!(stats.final_precision(), prec + 10, "p={prec}");
        if let Some((q, value)) = &previous {
            let refined = result.value().round_to(*q, Rounding::Nearest);
            assert_eq!(&refined, value, "p={prec} is inconsistent with p={q}");
        }
        previous = Some((prec, result.value().clone()));
    }
    let (result, _) = evaluate_faithful(&it, sqrt2, &BigFloat::zero(), &ctx(53), None, None);
    assert_eq!(
        result.value(),
        &BigFloat::from_f64(std::f64::consts::SQRT_2)
    );
}

#[test]
fn normal_context_rounding_mode_is_not_part_of_certification() {
    // The certification tests are stated for round-to-nearest (M2-plan §2), so
    // Context's rounding mode does not enter the engine: the caller applies it
    // to the certified value, exactly as the point-evaluation pipeline does.
    let mut it = Interner::new();
    let one = c(&mut it, 1);
    let three = c(&mut it, 3);
    let third = it.intern(Expr::Div(one, three));
    let nearest = evaluate_faithful(&it, third, &BigFloat::zero(), &ctx(53), None, None);
    let upward = evaluate_faithful(
        &it,
        third,
        &BigFloat::zero(),
        &ctx(53).with_rounding(Rounding::Up),
        None,
        None,
    );
    assert_eq!(nearest.0, upward.0, "the engine must ignore ctx.rounding");
    assert_eq!(nearest.0.value(), &BigFloat::from_f64(0.3333333333333333));
    // A directed rounding of the *reference* differs from the certified value,
    // so the post-processing step is a real choice the caller owns.
    let reference = eval_point(&it, third, &BigFloat::zero(), 200);
    let up = reference.round_to(53, Rounding::Up);
    let down = reference.round_to(53, Rounding::Down);
    assert_eq!(nearest.0.value(), &down, "1/3 rounds down at 53 bits");
    assert_ne!(up, down, "the directed neighbours must be distinct");
    let with_up_ctx = evaluate_faithful(
        &it,
        third,
        &BigFloat::zero(),
        &ctx(53).with_rounding(Rounding::Up),
        None,
        None,
    );
    assert_eq!(with_up_ctx.0.value(), &down, "ctx.rounding is not applied");
}

#[test]
fn normal_two_contexts_and_two_interners_do_not_interfere() {
    // Interleaved evaluations with different Contexts and different interners:
    // each result depends only on its own arguments.
    let mut a = Interner::new();
    let mut b = Interner::new();
    let a_two = c(&mut a, 2);
    let b_two = c(&mut b, 2);
    let a_sqrt = apply(&mut a, BaseFn::Sqrt, a_two);
    let b_sqrt = apply(&mut b, BaseFn::Sqrt, b_two);
    let coarse = ctx(8);
    let fine = ctx(113);
    let coarse_reference =
        eval_point(&a, a_sqrt, &BigFloat::zero(), 400).round_to(8, Rounding::Nearest);
    let fine_reference =
        eval_point(&b, b_sqrt, &BigFloat::zero(), 400).round_to(113, Rounding::Nearest);
    for _ in 0..3 {
        let first = evaluate_faithful(&a, a_sqrt, &BigFloat::zero(), &coarse, None, None);
        let second = evaluate_faithful(&b, b_sqrt, &BigFloat::zero(), &fine, None, None);
        assert_eq!(first.0.value(), &coarse_reference, "coarse context");
        assert_eq!(second.0.value(), &fine_reference, "fine context");
        assert_ne!(first.0.value(), second.0.value());
        assert_eq!(first.1.final_precision(), 18);
        assert_eq!(second.1.final_precision(), 123);
    }
}

// ---- Extreme and error paths across modules ---------------------------

#[test]
fn extreme_cutoff_is_the_documented_tool_for_negligible_kernel_results() {
    // The large-argument threshold proves erfc(x) < 2^-(p+2) and reports the
    // value 0 with that bound, so the enclosure straddles zero. Two ways out:
    // the cutoff flushes it to zero with BelowCutoff in the first round, or the
    // engine escalates until the kernel leaves the threshold branch and computes
    // the genuine tiny value, which it then correctly rounds. Both are honest;
    // the cutoff is the cheap one and is what table-making wants.
    let mut it = Interner::new();
    let ten = c(&mut it, 10);
    let erfc10 = apply(&mut it, BaseFn::Erfc, ten);

    // Without a cutoff: the zero-with-bound enclosure is unresolvable, so the
    // loop escalates past the threshold branch and certifies the real value.
    let (no_cutoff, stats) =
        evaluate_faithful(&it, erfc10, &BigFloat::zero(), &ctx(53), None, None);
    assert!(stats.rounds() > 1, "escalation is expected, got {stats:?}");
    assert_eq!(no_cutoff.quality(), Quality::CorrectlyRounded);
    assert!(
        *no_cutoff.value() > BigFloat::zero(),
        "{}",
        no_cutoff.value()
    );
    let reference = eval_point(&it, erfc10, &BigFloat::zero(), 300);
    assert_eq!(
        no_cutoff.value(),
        &reference.round_to(53, Rounding::Nearest)
    );
    assert!(no_cutoff.value().abs() < BigFloat::from_f64(1e-40));

    // With a cutoff covering the bracket: BelowCutoff, flushed to zero, and the
    // decision is made in the first round instead of after escalation.
    let cutoff = pow2(-30);
    let (with_cutoff, cstats) = evaluate_faithful(
        &it,
        erfc10,
        &BigFloat::zero(),
        &ctx(53),
        Some(&cutoff),
        None,
    );
    assert_eq!(with_cutoff.quality(), Quality::BelowCutoff);
    assert!(with_cutoff.value().is_zero());
    assert_eq!(cstats.rounds(), 1, "decided in the first round");

    // A cutoff below the true value (~2^-148) never fires and the run is
    // indistinguishable from the no-cutoff one.
    let tight = pow2(-160);
    let (result, _) =
        evaluate_faithful(&it, erfc10, &BigFloat::zero(), &ctx(53), Some(&tight), None);
    assert!(!result.quality().is_below_cutoff());
    assert_eq!(result, no_cutoff, "an inactive cutoff changes nothing");
}

#[test]
fn extreme_nan_input_ends_the_loop_immediately_and_zero_is_exact() {
    let mut it = Interner::new();
    let x = var(&mut it);
    let one = c(&mut it, 1);
    let expr = it.intern(Expr::Mul(x, one));
    let (result, stats) = evaluate_faithful(&it, expr, &BigFloat::NAN, &ctx(53), None, None);
    assert_eq!(result.quality(), Quality::NonReal);
    assert_eq!(stats.rounds(), 1);
    assert!(result.value().is_nan());

    // An exact zero input gives a point enclosure and an Exact certificate.
    let (result, _) = evaluate_faithful(&it, expr, &BigFloat::zero(), &ctx(53), None, None);
    assert_eq!(result.quality(), Quality::Exact);
    assert!(result.value().is_zero());

    // The interval layer's empty range propagates through the same tree.
    let enc = eval_interval(&it, expr, &Interval::empty(), 53, None);
    assert!(enc.is_empty());
}

#[test]
fn error_pole_neighbourhoods_terminate_within_the_budget() {
    // Poles and underflow-sized arguments: every run finishes inside the
    // budget with one of the eight certificates and never panics.
    let mut it = Interner::new();
    let x = var(&mut it);
    let one = c(&mut it, 1);
    let tan_x = apply(&mut it, BaseFn::Tan, x);
    let atan_x = apply(&mut it, BaseFn::Atan, x);
    let recip = it.intern(Expr::Div(one, x));
    let half_pi = BigFloat::pi_with_precision(200) / BigFloat::from_i64(2);
    let just_below = half_pi.clone() - pow2(-60);
    let args = [
        half_pi,
        just_below,
        pow2(-1000),
        -pow2(-1000),
        BigFloat::from_f64(1e-300),
    ];
    for &id in &[tan_x, atan_x, recip] {
        for arg in &args {
            for prec in [2usize, 53] {
                let (result, stats) = evaluate_faithful(&it, id, arg, &ctx(prec), None, None);
                assert!(
                    Quality::from_code(result.quality().code()).is_some(),
                    "certificate out of range: {:?}",
                    result.quality()
                );
                assert!(stats.final_precision() <= 512 * prec);
                assert!(stats.rounds() >= 1);
            }
        }
    }
}

#[test]
fn robustness_certified_value_matches_the_engine_own_enclosure() {
    // The engine's certificate must be consistent with the enclosure it was
    // certified from, at the precision it actually used. Note the enclosure is
    // at the *working* precision while the certificate is at the target, so the
    // certified value is that enclosure's target rounding — it need not lie
    // inside the enclosure itself (a working-precision point enclosure is
    // narrower than one target ulp).
    let mut it = Interner::new();
    let x = var(&mut it);
    let sqrt_x = apply(&mut it, BaseFn::Sqrt, x);
    let erf_x = apply(&mut it, BaseFn::Erf, x);
    let sum = it.intern(Expr::Add(sqrt_x, erf_x));
    let one = c(&mut it, 1);
    let prod = it.intern(Expr::Mul(sum, one));
    let arg = BigFloat::from_f64(1.75);
    for &id in &[sqrt_x, erf_x, sum, prod] {
        for prec in [24usize, 53] {
            let (result, stats) = evaluate_faithful(&it, id, &arg, &ctx(prec), None, None);
            if result.is_non_real() || result.quality().is_below_cutoff() {
                continue;
            }
            let w = stats.final_precision();
            let enc = eval_interval(
                &it,
                id,
                &Interval::new(
                    arg.round_to(w, Rounding::Down),
                    arg.round_to(w, Rounding::Up),
                ),
                w,
                None,
            );
            // The enclosure brackets the true value of the same tree.
            let reference = eval_point(&it, id, &arg, w + 100);
            if reference.is_finite() {
                assert!(
                    enc.contains(&reference),
                    "{:?}@{prec}: reference {reference} outside its enclosure {enc}",
                    it.get(id)
                );
            }
            // A correct-rounding certificate is the target rounding of that
            // enclosure, and equals the reference's rounding.
            if result.is_correctly_rounded() {
                assert_eq!(
                    result.value(),
                    &enc.lower()
                        .expect("bounded")
                        .round_to(prec, Rounding::Nearest),
                    "{:?}@{prec}: certificate disagrees with the enclosure {enc}",
                    it.get(id)
                );
                if reference.is_finite() {
                    assert_eq!(
                        result.value(),
                        &reference.round_to(prec, Rounding::Nearest),
                        "{:?}@{prec}: disagrees with the reference",
                        it.get(id)
                    );
                }
            }
        }
    }
}

// ---- M2-E2: the new function family through the whole pipeline ----------

#[test]
fn normal_m2e2_new_function_pipeline_cross_checks() {
    // interning -> printing -> point evaluation -> interval enclosure ->
    // faithful certification, cross-checked against each other for every new
    // function on an in-domain argument.
    let mut it = Interner::new();
    let x = var(&mut it);
    let third = BigFloat::one() / BigFloat::from_i64(3);
    let cases = [
        (BaseFn::Log2, BigFloat::from_i64(8), "log2"),
        (BaseFn::Log10, BigFloat::from_i64(1000), "log10"),
        (BaseFn::Log1p, BigFloat::from_f64(0.5), "log1p"),
        (BaseFn::Expm1, BigFloat::one(), "expm1"),
        (BaseFn::Abs, BigFloat::from_f64(-2.5), "abs"),
        (BaseFn::Ceil, BigFloat::from_f64(2.5), "ceil"),
        (BaseFn::Floor, BigFloat::from_f64(2.5), "floor"),
        (BaseFn::NearestInt, BigFloat::from_f64(2.5), "nearestint"),
        (BaseFn::Double, third.clone(), "double"),
        (BaseFn::Single, third.clone(), "single"),
        (BaseFn::HalfPrecision, third.clone(), "halfprecision"),
        (BaseFn::Quad, third.clone(), "quad"),
        (BaseFn::DoubleExtended, third, "doubleextended"),
    ];
    for (f, arg, name) in cases {
        let id = apply(&mut it, f, x);
        // Printing: the canonical name with the free variable.
        assert_eq!(print_expr(&it, id), format!("{name}(_x_)"));
        // The interval enclosure at the target contains the wide point truth.
        let reference = eval_point(&it, id, &arg, 153);
        let enc = eval_interval(&it, id, &point(&arg), 53, None);
        assert!(
            enc.contains(&reference),
            "{name}: enclosure {enc} misses the reference {reference}"
        );
        // The faithful engine agrees with the reference's target rounding
        // whenever it certifies a correct rounding. With nearest endpoint
        // rounding (M2-E2b) a point argument collapses the format enclosure
        // to the nearest format float, so the conversions certify a correct
        // rounding here; the other branches remain as defensive checks (a
        // faithful certificate stays within one target ulp of the reference).
        let (result, stats) = evaluate_faithful(&it, id, &arg, &ctx(53), None, None);
        assert!(stats.rounds() >= 1);
        if result.is_correctly_rounded() {
            assert_eq!(
                result.value(),
                &reference.round_to(53, Rounding::Nearest),
                "{name}: certificate {:?} disagrees with the reference",
                result.quality()
            );
        } else if result.quality() == Quality::NotFaithful {
            assert_eq!(
                result.value(),
                &enc.lower()
                    .expect("bounded")
                    .round_to(53, Rounding::Nearest),
                "{name}: give-up value is not the lower endpoint's image"
            );
        } else {
            assert!(result.is_faithful(), "{name}: {:?}", result.quality());
            let err = (result.value().clone() - reference.clone()).abs();
            assert!(
                err <= ulp(result.value(), 53),
                "{name}: error {err} exceeds one ulp (certificate {:?})",
                result.quality()
            );
        }
    }
}

#[test]
fn normal_m2e2_non_smooth_pipeline_inclusion_across_breakpoints() {
    // A composite mixing smooth and non-smooth functions: over a span that
    // crosses integer and rounding breakpoints, both the naive and the
    // Taylor-refined enclosures contain every sampled point truth, and the
    // engine certifies the exact value at an interior point.
    let mut it = Interner::new();
    let x = var(&mut it);
    let ceil_x = apply(&mut it, BaseFn::Ceil, x);
    let floor_x = apply(&mut it, BaseFn::Floor, x);
    let double_x = apply(&mut it, BaseFn::Double, x);
    let sin_x = apply(&mut it, BaseFn::Sin, x);
    // f = sin(x) + ceil(x) - floor(x) + double(x).
    let t1 = it.intern(Expr::Add(sin_x, ceil_x));
    let t2 = it.intern(Expr::Sub(t1, floor_x));
    let f = it.intern(Expr::Add(t2, double_x));
    let span = Interval::new(BigFloat::from_f64(1.25), BigFloat::from_f64(2.75));
    let naive = eval_interval(&it, f, &span, 53, None);
    let refined = eval_taylor(&it, f, &span, 53, 1, None);
    // An exact dyadic grid (step 3/32): every sample is represented exactly,
    // so no grid drift can place a probe outside the span.
    let lo = BigFloat::from_f64(1.25);
    let step = BigFloat::from_i64(3) / BigFloat::from_i64(32);
    for i in 0..=16i64 {
        let xv = lo.clone() + step.clone() * BigFloat::from_i64(i);
        let truth = eval_point(&it, f, &xv, 200);
        assert!(naive.contains(&truth), "naive {naive} misses sample {i}");
        assert!(
            refined.contains(&truth),
            "refined {refined} misses sample {i}"
        );
    }
    // The non-smooth fallback keeps the refined enclosure identical to naive.
    assert_eq!(naive.lower(), refined.lower());
    assert_eq!(naive.upper(), refined.upper());
    // At an interior dyadic point the engine certifies the exact value.
    let arg = BigFloat::from_f64(1.25);
    let kctx = ctx(53).with_taylor_recursions(1);
    let (result, _) = evaluate_faithful(&it, f, &arg, &kctx, None, None);
    let truth = eval_point(&it, f, &arg, 200);
    assert_eq!(result.value(), &truth.round_to(53, Rounding::Nearest));
    assert!(result.is_correctly_rounded(), "{result:?}");
}

#[test]
fn normal_m2e2_format_conversion_composition_pipeline() {
    // Conversions compose and are idempotent through the whole pipeline, and
    // the printed composition matches the canonical names.
    let mut it = Interner::new();
    let x = var(&mut it);
    let double_x = apply(&mut it, BaseFn::Double, x);
    let single_x = apply(&mut it, BaseFn::Single, x);
    let single_of_double = apply(&mut it, BaseFn::Single, double_x);
    let double_of_double = apply(&mut it, BaseFn::Double, double_x);
    assert_eq!(print_expr(&it, single_of_double), "single(double(_x_))");
    let third = BigFloat::one() / BigFloat::from_i64(3);
    // single(double(1/3)) == single(1/3): double rounding through binary64
    // into binary32 is safe for 1/3 (no double-rounding tie).
    let a = eval_point(&it, single_of_double, &third, 165);
    let b = eval_point(&it, single_x, &third, 165);
    assert_eq!(a, b);
    // Idempotence of double.
    let c1 = eval_point(&it, double_of_double, &third, 165);
    let c2 = eval_point(&it, double_x, &third, 165);
    assert_eq!(c1, c2);
    // The engine certifies the composed conversion at the binary32 width:
    // nearest endpoint rounding collapses the point argument to the single
    // nearest binary32 float, so Test 1 certifies Exact with the nearest
    // image `b`. (M2-E2b correction: the former directed endpoints pinned
    // Test 2's lower neighbor `b.next_below(24)`, a faithful but not nearest
    // rounding — the wrong expectation for round-to-nearest semantics.)
    let (result, _) = evaluate_faithful(&it, single_of_double, &third, &ctx(24), None, None);
    assert_eq!(result.value(), &b);
    assert_eq!(result.quality(), Quality::Exact, "{result:?}");
}
