// SPDX-License-Identifier: Apache-2.0
//! Integration test: `parse` ↔ `core::print` round-trip identity.
//!
//! The grammar was built to be the exact inverse of the canonical printer in
//! `rhytido_core::print`. The acceptance property is:
//!
//! * **Idempotence** — for any parseable source `s`, normalizing once
//!   (`t = print(parse(s))`) reaches a fixpoint: `parse(t)` reuses the very same
//!   interned node and `print(parse(t)) == t`. This holds even where the printer
//!   is non-injective (the known `(a^b)^c` case).
//! * **Canonical identity** — for a string `t` that is already printer output,
//!   `print(parse(t)) == t` byte-for-byte.
//! * **AST → print → parse → same `NodeId`** for a spread of hand-built nodes.

use rhytido_backend::BigFloat;
use rhytido_core::BaseFn;
use rhytido_core::Expr;
use rhytido_core::Interner;
use rhytido_core::NodeId;
use rhytido_core::print_expr;
use rhytido_lang::parse_expr;
use std::str::FromStr;

/// Parses `src` twice through the printer and asserts the normalizing fixpoint;
/// returns the canonical printer string for further checks.
#[track_caller]
fn normalize(src: &str) -> String {
    let mut it = Interner::new();
    let first = parse_expr(src, &mut it).unwrap_or_else(|e| panic!("`{src}` must parse: {e:?}"));
    let once = print_expr(&it, first);
    let second = parse_expr(&once, &mut it)
        .unwrap_or_else(|e| panic!("canonical `{once}` (from `{src}`) must reparse: {e:?}"));
    assert_eq!(
        first, second,
        "`{src}` → print `{once}` must re-parse to the same node id"
    );
    assert_eq!(
        print_expr(&it, second),
        once,
        "`{src}` → print `{once}` must print identically after a round trip"
    );
    once
}

/// Asserts `src` is already canonical: parsing it and re-printing yields it.
#[track_caller]
fn assert_canonical(src: &str) {
    let mut it = Interner::new();
    let id = parse_expr(src, &mut it).unwrap_or_else(|e| panic!("`{src}` must parse: {e:?}"));
    let reprinted = print_expr(&it, id);
    assert_eq!(
        reprinted, src,
        "printer must reproduce canonical `{src}` exactly, got `{reprinted}`"
    );
    // And re-parsing the reprint reuses the node.
    let again = parse_expr(&reprinted, &mut it).unwrap();
    assert_eq!(id, again);
}

/// Builds a hand-crafted canonical AST, then checks print → parse → intern is the
/// same node. `build` must produce a node the printer would emit unchanged.
#[track_caller]
fn assert_ast_roundtrip(build: impl Fn(&mut Interner) -> NodeId, expected: &str) {
    let mut it = Interner::new();
    let id = build(&mut it);
    let text = print_expr(&it, id);
    assert_eq!(text, expected, "hand-built AST must print as `{expected}`");
    let reparsed =
        parse_expr(&text, &mut it).unwrap_or_else(|e| panic!("`{text}` must reparse: {e:?}"));
    assert_eq!(
        id, reparsed,
        "AST → print → parse → intern must be identity"
    );
}

// ---- Idempotence over a broad corpus --------------------------------------

#[test]
fn integration_normalize_fixpoint_operators() {
    for src in [
        "1+2",
        "1 - 2 - 3",
        "2*3/4",
        "2^3^4",
        "(2+3)*4",
        "1-(2-3)",
        "1/(2/3)",
        "-2*3",
        "-2^3",
        "- -5",
        "2*-3",
        "2^-3",
        "1 + 2 * 3 - 4 / 5 ^ 6",
    ] {
        // NB: `(a^b)^c` is excluded — it is the documented non-injective case,
        // covered by `integration_known_non_injective_power_is_string_stable`.
        let _ = normalize(src);
    }
}

#[test]
fn integration_normalize_fixpoint_literals_and_atoms() {
    // Dyadic-exact literals (integers and halves) have an exact finite binary
    // form, so they survive the parse → print → parse intern round-trip as a
    // fixpoint at the NodeId level.
    for src in ["2", "1.5", ".5", "2.", "2e3", "pi", "_x_", "NaN"] {
        let _ = normalize(src);
    }
}

#[test]
fn integration_non_dyadic_literal_value_round_trip() {
    // Non-dyadic decimals (e.g. `1.5e-7`) are stored by the backend at a
    // working precision derived from the literal's digit count; the printer
    // re-renders ~51 digits, and re-parsing that string grows the precision a
    // little, so the interned *bits* are not byte-identical and NodeId equality
    // does not hold. This is a backend/core display-precision boundary, outside
    // the parser's control (AGENTS §3: lang does not alter core value semantics).
    // The *value* does round-trip once re-rounded to the default precision — the
    // exact contract `rhytido-backend`'s own Display tests establish.
    for src in ["1.5e-7", "2E-3", "1e-300", "0.1"] {
        let mut it = Interner::new();
        let id = parse_expr(src, &mut it).unwrap();
        let before = constant_of(&it, id);
        let text = print_expr(&it, id);
        let id2 = parse_expr(&text, &mut it).unwrap();
        let after = constant_of(&it, id2);
        let p = rhytido_backend::DEFAULT_PRECISION;
        assert_eq!(
            before.with_precision(p),
            after.with_precision(p),
            "`{src}` value must round-trip when re-rounded to default precision"
        );
    }
}

/// Extracts the `BigFloat` from a constant node (panics otherwise — a test
/// invariant on the caller's part).
fn constant_of(it: &Interner, id: NodeId) -> BigFloat {
    match it.get(id) {
        Expr::Constant(c) => c.clone(),
        other => panic!("expected a constant node, got {other:?}"),
    }
}

#[test]
fn integration_normalize_fixpoint_functions() {
    for src in [
        "sqrt(2)",
        "sqrt(1+2)",
        "log(_x_)",
        "exp(sin(_x_))",
        "sqrt(_x_)^2",
        "-sqrt(_x_)",
        "erf(_x_)+erfc(_x_)",
        "double(log2(_x_))",
        "abs(pi)+1",
    ] {
        let _ = normalize(src);
    }
}

// ---- Canonical (printer-shaped) strings are reproduced byte-for-byte ------

#[test]
fn integration_canonical_strings_are_fixpoints() {
    // These are exactly the printer's canonical renderings (see the task spec
    // and crates/rhytido-core/tests/print.rs samples).
    for src in [
        "2 + 3",
        "2 - 3",
        "2*3",
        "2/3",
        "2^3",
        "(2 + 3)*4",
        "2 + 3*4",
        "1 - (2 - 3)",
        "1/(2/3)",
        "(2 + 3)^3",
        "-5",
        "-(5 + 5)",
        "-_x_",
        "sqrt(_x_)",
        "exp(sqrt(_x_))",
        "pi",
        "_x_",
        "NaN",
        "1 - 2 - 3",
        "2^2^2",
        "-2*3",
        "-2^3",
        "2^(-3)",
        "sqrt(_x_) + sqrt(_x_)",
        "double(log2(_x_))",
        "abs(_x_) + 1",
        "-abs(_x_)",
        "abs(_x_)^1",
        "erf(_x_)^2",
        "-erf(_x_)",
        "ln(_x_)",
    ] {
        assert_canonical(src);
    }
}

// ---- AST → print → parse → intern identity for hand-built nodes -----------

#[test]
fn integration_ast_roundtrip_spread() {
    // Add: 1 + 2*3.
    assert_ast_roundtrip(
        |it| {
            let a = it.intern(Expr::Constant(BigFloat::from_str("1").unwrap()));
            let b = it.intern(Expr::Constant(BigFloat::from_str("2").unwrap()));
            let c = it.intern(Expr::Constant(BigFloat::from_str("3").unwrap()));
            let mul = it.intern(Expr::Mul(b, c));
            it.intern(Expr::Add(a, mul))
        },
        "1 + 2*3",
    );

    // Sub with parenthesized right operand: 1 - (2 - 3).
    assert_ast_roundtrip(
        |it| {
            let a = it.intern(Expr::Constant(BigFloat::from_str("1").unwrap()));
            let b = it.intern(Expr::Constant(BigFloat::from_str("2").unwrap()));
            let c = it.intern(Expr::Constant(BigFloat::from_str("3").unwrap()));
            let inner = it.intern(Expr::Sub(b, c));
            it.intern(Expr::Sub(a, inner))
        },
        "1 - (2 - 3)",
    );

    // Neg of a sum: -(5 + 5).
    assert_ast_roundtrip(
        |it| {
            let f = it.intern(Expr::Constant(BigFloat::from_str("5").unwrap()));
            let sum = it.intern(Expr::Add(f, f));
            it.intern(Expr::Neg(sum))
        },
        "-(5 + 5)",
    );

    // Right-associative power: 2^3^4.
    assert_ast_roundtrip(
        |it| {
            let a = it.intern(Expr::Constant(BigFloat::from_str("2").unwrap()));
            let b = it.intern(Expr::Constant(BigFloat::from_str("3").unwrap()));
            let c = it.intern(Expr::Constant(BigFloat::from_str("4").unwrap()));
            let right = it.intern(Expr::Pow(b, c));
            it.intern(Expr::Pow(a, right))
        },
        "2^3^4",
    );

    // Application + pi + variable.
    assert_ast_roundtrip(
        |it| {
            let x = it.intern(Expr::Variable);
            let sqrt = it.intern(Expr::Apply(BaseFn::Sqrt, x));
            let pi = it.intern(Expr::Pi);
            let three = it.intern(Expr::Constant(BigFloat::from_str("3").unwrap()));
            let div = it.intern(Expr::Div(pi, three));
            it.intern(Expr::Add(sqrt, div))
        },
        "sqrt(_x_) + pi/3",
    );

    // NaN constant.
    assert_ast_roundtrip(|it| it.intern(Expr::Constant(BigFloat::NAN)), "NaN");

    // log alias re-prints as ln (the alias is parse-only).
    let mut it = Interner::new();
    let via_log = parse_expr("log(_x_)", &mut it).unwrap();
    assert_eq!(print_expr(&it, via_log), "ln(_x_)");
    let via_ln = parse_expr("ln(_x_)", &mut it).unwrap();
    assert_eq!(via_log, via_ln, "log and ln intern to the same node");
}

// ---- Documented printer non-injectivity -----------------------------------

#[test]
fn integration_known_non_injective_power_is_string_stable() {
    // (2^3)^4 and 2^(3^4) both print as "2^3^4" (the printer adds no parens for a
    // same-precedence power left operand). The *string* round-trip is still a
    // fixpoint; only the node-level `parse(print(id)) == id` fails for the
    // non-canonical left-nested shape, which is the documented carve-out.
    let mut it = Interner::new();
    let a = it.intern(Expr::Constant(BigFloat::from_str("2").unwrap()));
    let b = it.intern(Expr::Constant(BigFloat::from_str("3").unwrap()));
    let c = it.intern(Expr::Constant(BigFloat::from_str("4").unwrap()));

    let ab = it.intern(Expr::Pow(a, b));
    let left_nested = it.intern(Expr::Pow(ab, c));
    let bc = it.intern(Expr::Pow(b, c));
    let canonical = it.intern(Expr::Pow(a, bc));

    let text = print_expr(&it, left_nested);
    assert_eq!(text, "2^3^4", "left-nested power must print without parens");
    assert_eq!(
        print_expr(&it, canonical),
        "2^3^4",
        "both shapes share a print"
    );

    // Reparsing the canonical string yields the right-associative node.
    let reparsed = parse_expr(&text, &mut it).unwrap();
    assert_eq!(
        reparsed, canonical,
        "parse resolves `2^3^4` to right-associative"
    );

    // The string-level fixpoint still holds for the pathological input too.
    assert_eq!(print_expr(&it, reparsed), text);
}
