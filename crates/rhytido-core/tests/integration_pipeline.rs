//! Integration tests: cross-module pipelines through rhytido-core.
//!
//! Layer: integration tests (AGENTS.md §4) — expr → print → eval →
//! FpResult/Quality, with Context and MessageCatalog cooperation.

use rhytido_backend::{BigFloat, Rounding};
use rhytido_core::{
    BaseFn, Context, Expr, FpResult, Interner, MIN_PRECISION, MessageCatalog, NodeId, Quality,
    eval_point,
    message::{MSG_ROUNDING_OCCURRED, MSG_UNDEFINED_OR_UNSTABLE},
    print_expr,
};

const PREC: usize = 165;

fn constant(it: &mut Interner, v: i64) -> NodeId {
    it.intern(Expr::Constant(BigFloat::from_i64(v)))
}

fn close(a: &BigFloat, b: &BigFloat, bits: usize) -> bool {
    a.with_precision(bits) == b.with_precision(bits)
}

// ---- Normal cases -----------------------------------------------------

#[test]
fn normal_pipeline_intern_print_eval_certificate() {
    // Intern x^2 + 1, render it, evaluate at x=2, and certify the result.
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let two = constant(&mut it, 2);
    let one = constant(&mut it, 1);
    let x2 = it.intern(Expr::Pow(x, two));
    let poly = it.intern(Expr::Add(x2, one));
    assert_eq!(print_expr(&it, poly), "_x_^2 + 1");
    let ctx = Context::new();
    let v = eval_point(&it, poly, &BigFloat::from_i64(2), ctx.precision());
    assert_eq!(v, BigFloat::from_i64(5));
    let r = FpResult::exact(v);
    assert_eq!(r.value(), &BigFloat::from_i64(5));
    assert_eq!(r.quality(), Quality::Exact);
    assert!(r.is_exact());
    assert!(r.is_correctly_rounded());
    assert!(r.is_faithful());
    // Certificate queries agree with the stable numeric code.
    assert_eq!(r.quality().code(), 4);
    assert_eq!(Quality::from_code(r.quality().code()), Some(Quality::Exact));
}

#[test]
fn normal_pipeline_transcendental_faithful_certificate() {
    // sqrt(_x_) at x=2 is irrational: the pipeline yields a faithful value.
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let sqrt_x = it.intern(Expr::Apply(BaseFn::Sqrt, x));
    assert_eq!(print_expr(&it, sqrt_x), "sqrt(_x_)");
    let v = eval_point(&it, sqrt_x, &BigFloat::from_i64(2), PREC);
    assert_eq!(v, BigFloat::from_i64(2).sqrt().with_precision(PREC));
    let r = FpResult::new(v, Quality::Faithful);
    assert!(r.is_faithful());
    assert!(!r.is_exact());
    assert!(!r.is_correctly_rounded());
    assert_eq!(
        Quality::from_code(r.quality().code()),
        Some(Quality::Faithful)
    );
}

#[test]
fn normal_context_precision_drives_eval() {
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let one = constant(&mut it, 1);
    let sum = it.intern(Expr::Add(x, one));
    let diff = it.intern(Expr::Sub(x, one));
    // (x+1)*(x-1) at x=3 is exactly 8 at every working precision.
    let poly = it.intern(Expr::Mul(sum, diff));
    let point = BigFloat::from_i64(3);
    for prec in [MIN_PRECISION, 8, 64, 165, 400] {
        let ctx = Context::new().with_precision(prec);
        let v = eval_point(&it, poly, &point, ctx.precision());
        assert_eq!(v, BigFloat::from_i64(8), "precision {}", ctx.precision());
    }
    // Under-minimum precision is clamped, and evaluation still works there.
    let clamped = Context::new().with_precision(0);
    assert_eq!(clamped.precision(), MIN_PRECISION);
    // An irrational result agrees across precisions at the lower precision.
    let two = constant(&mut it, 2);
    let sqrt2 = it.intern(Expr::Apply(BaseFn::Sqrt, two));
    let low = Context::new().with_precision(64);
    let high = Context::new().with_precision(300);
    let v_low = eval_point(&it, sqrt2, &BigFloat::zero(), low.precision());
    let v_high = eval_point(&it, sqrt2, &BigFloat::zero(), high.precision());
    assert!(close(&v_high.with_precision(low.precision()), &v_low, 60));
    assert!(close(&v_low, &BigFloat::from_i64(2).sqrt(), 60));
}

#[test]
fn normal_context_rounding_postprocesses_eval_result() {
    // Evaluate 1/3, then re-round under the Context's directed modes.
    let mut it = Interner::new();
    let one = constant(&mut it, 1);
    let three = constant(&mut it, 3);
    let third = it.intern(Expr::Div(one, three));
    let v = eval_point(&it, third, &BigFloat::zero(), PREC);
    let up = Context::new().with_precision(8).with_rounding(Rounding::Up);
    let down = Context::new()
        .with_precision(8)
        .with_rounding(Rounding::Down);
    let r_up = v.round_to(up.precision(), up.rounding());
    let r_down = v.round_to(down.precision(), down.rounding());
    // 1/3 is inexact at 8 bits, so the directed roundings straddle it.
    assert!(r_up > r_down);
    assert!(r_up > v);
    assert!(r_down < v);
    // Rounding occurred: the catalog emits the warning at default verbosity.
    let cat = MessageCatalog::new();
    assert!(cat.should_emit(MSG_ROUNDING_OCCURRED.code, MSG_ROUNDING_OCCURRED.level));
    assert_eq!(
        cat.render(&MSG_ROUNDING_OCCURRED).unwrap(),
        MSG_ROUNDING_OCCURRED.text
    );
    // The re-rounded value carries a known-inexact certificate.
    let cert = FpResult::new(r_up, Quality::FaithfulInexact);
    assert!(cert.is_faithful());
    assert!(cert.quality().is_known_inexact());
}

// ---- Extreme cases ----------------------------------------------------

#[test]
fn extreme_shared_subtree_intern_eval_invariant() {
    // Sharing a subtree via hash-consing must not change evaluation.
    let mut it = Interner::new();
    let x = it.intern(Expr::Variable);
    let sq = it.intern(Expr::Apply(BaseFn::Sqrt, x));
    let shared = it.intern(Expr::Add(sq, sq));
    // A structurally identical tree in a fresh interner.
    let mut other = Interner::new();
    let x2 = other.intern(Expr::Variable);
    let s1 = other.intern(Expr::Apply(BaseFn::Sqrt, x2));
    let s2 = other.intern(Expr::Apply(BaseFn::Sqrt, x2));
    let duplicated = other.intern(Expr::Add(s1, s2));
    assert_eq!(s1, s2); // the subtree is shared
    let point = BigFloat::from_i64(9);
    let v_shared = eval_point(&it, shared, &point, PREC);
    let v_dup = eval_point(&other, duplicated, &point, PREC);
    assert_eq!(v_shared, v_dup);
    assert_eq!(v_shared, BigFloat::from_i64(6)); // 2*sqrt(9)
    assert_eq!(print_expr(&it, shared), print_expr(&other, duplicated));
}

// ---- Error cases (异常报错) -------------------------------------------

#[test]
fn error_pipeline_domain_error_certificate_and_message() {
    // sqrt(-1) flows through print/eval to a NonReal certificate, and the
    // catalog gates the accompanying diagnostic.
    let mut it = Interner::new();
    let neg = constant(&mut it, -1);
    let sqrt_neg = it.intern(Expr::Apply(BaseFn::Sqrt, neg));
    assert_eq!(print_expr(&it, sqrt_neg), "sqrt(-1)");
    let v = eval_point(&it, sqrt_neg, &BigFloat::zero(), PREC);
    assert!(v.is_nan());
    let r = FpResult::new(v, Quality::NonReal);
    assert!(r.is_non_real());
    assert!(!r.is_faithful());
    assert!(!r.is_correctly_rounded());
    let mut cat = MessageCatalog::new();
    assert_eq!(
        cat.render(&MSG_UNDEFINED_OR_UNSTABLE).unwrap(),
        MSG_UNDEFINED_OR_UNSTABLE.text
    );
    cat.suppress(MSG_UNDEFINED_OR_UNSTABLE.code);
    assert!(cat.render(&MSG_UNDEFINED_OR_UNSTABLE).is_none());
    cat.unsuppress(MSG_UNDEFINED_OR_UNSTABLE.code);
    assert!(cat.render(&MSG_UNDEFINED_OR_UNSTABLE).is_some());
    // Zero verbosity silences the diagnostic even without suppression.
    let mut quiet = MessageCatalog::new();
    quiet.set_verbosity(0);
    assert!(quiet.render(&MSG_UNDEFINED_OR_UNSTABLE).is_none());
}

// ---- Interruption / robustness cases (异常中断) -----------------------

#[test]
fn robustness_quality_delegation_across_pipeline() {
    // All eight certificate states delegate consistently through FpResult
    // when the value comes from the evaluation pipeline.
    let mut it = Interner::new();
    let two = constant(&mut it, 2);
    let three = constant(&mut it, 3);
    let sum = it.intern(Expr::Add(two, three));
    assert_eq!(print_expr(&it, sum), "2 + 3");
    let v = eval_point(&it, sum, &BigFloat::zero(), PREC);
    for code in 0..=7u8 {
        let q = Quality::from_code(code).unwrap();
        let r = FpResult::new(v.clone(), q);
        assert_eq!(r.quality(), q);
        assert_eq!(Quality::from_code(r.quality().code()), Some(q));
        assert_eq!(r.is_exact(), q.is_exact());
        assert_eq!(r.is_faithful(), q.is_faithful());
        assert_eq!(r.is_correctly_rounded(), q.is_correctly_rounded());
        assert_eq!(r.is_non_real(), q.is_non_real());
    }
}
