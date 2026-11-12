//! M0 micro-benchmark harness (std timing; no external benchmark dependency).
//!
//! Run with: `cargo run --release --example bench -p rhytido-backend`
//!
//! This is a coarse baseline for tracking performance across milestones, not a
//! statistically rigorous benchmark. It times a batch of representative
//! operations at the default working precision.

use std::time::Instant;

use rhytido_backend::{BigFloat, Format, Integer, Interval, Rational, Rounding};

fn time<F: FnMut()>(label: &str, iters: u32, mut f: F) {
    // Warm-up.
    for _ in 0..(iters / 10).max(1) {
        f();
    }
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    let elapsed = start.elapsed();
    let per = elapsed.as_secs_f64() / f64::from(iters);
    println!("{label:<32} {iters:>8} iters  {per:>12.9} s/op");
}

fn main() {
    println!(
        "Rhytido M0 micro-benchmark (default precision = {} bits)",
        BigFloat::one().precision()
    );
    println!("{:-<70}", "");

    let a = BigFloat::one() / BigFloat::from_i64(3);
    let b = BigFloat::one() / BigFloat::from_i64(7);

    time("BigFloat add", 20_000, || {
        let _ = a.clone() + b.clone();
    });
    time("BigFloat mul", 20_000, || {
        let _ = a.clone() * b.clone();
    });
    time("BigFloat div", 10_000, || {
        let _ = a.clone() / b.clone();
    });
    time("BigFloat sqrt", 5_000, || {
        let _ = a.sqrt();
    });
    time("BigFloat exp", 2_000, || {
        let _ = a.exp();
    });
    time("BigFloat ln", 2_000, || {
        let _ = a.ln();
    });
    time("BigFloat round_to_format(Double)", 20_000, || {
        let _ = a.round_to_format(Format::Double, Rounding::Nearest);
    });

    let ia = Integer::from(123_456_789);
    let ib = Integer::from(987_654_321);
    time("Integer mul", 50_000, || {
        let _ = ia.clone() * ib.clone();
    });

    let ra = Rational::new(123, 456).unwrap();
    let rb = Rational::new(789, 1011).unwrap();
    time("Rational add", 30_000, || {
        let _ = &ra + &rb;
    });

    let iv_a = Interval::new(a.clone(), b.clone());
    let iv_b = Interval::new(b.clone(), a.clone());
    time("Interval mul", 10_000, || {
        let _ = iv_a.mul(&iv_b);
    });

    println!("{:-<70}", "");
    println!("done");
}
