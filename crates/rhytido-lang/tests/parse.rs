// SPDX-License-Identifier: Apache-2.0
//! Module tests for the `rhytido_lang::parse` public API.
//!
//! Tests the `parse_expr` surface from an external crate viewpoint, covering the
//! four mandated case classes: normal (operator/function/constant/precedence
//! combinations asserted as AST *shape*), extreme (deep grouping, long chains,
//! tiny/huge literals, every function name), error (illegal input asserted by
//! `ParseError` variant *and* byte position), and interruption/robustness (large
//! input, deterministic re-parse, no panic on adversarial text).
//!
//! AST shape is asserted by re-interning the expected tree into the *same*
//! interner the parser used: hash-consing means a matching structure resolves to
//! the identical [`NodeId`], while any mismatch creates a fresh node and fails
//! the equality — so this is a genuine structural check, not a tautology.

use std::str::FromStr;

use rhytido_backend::BigFloat;
use rhytido_core::BaseFn;
use rhytido_core::Expr;
use rhytido_core::Interner;
use rhytido_core::NodeId;
use rhytido_lang::ParseError;
use rhytido_lang::parse_expr;

// ---- Shared helpers -------------------------------------------------------

/// Parses `src` into a fresh interner, panicking with the error message on any
/// failure — used by the positive tests where a parse is expected.
fn ok(src: &str) -> (Interner, NodeId) {
    let mut it = Interner::new();
    let id =
        parse_expr(src, &mut it).unwrap_or_else(|e| panic!("expected `{src}` to parse, got {e:?}"));
    (it, id)
}

fn cst(it: &mut Interner, s: &str) -> NodeId {
    it.intern(Expr::Constant(
        BigFloat::from_str(s).expect("test literal must be a valid decimal"),
    ))
}

#[allow(clippy::needless_pass_by_value)]
fn node(it: &mut Interner, e: Expr) -> NodeId {
    it.intern(e)
}

// ---- Normal cases ---------------------------------------------------------

#[test]
fn normal_decimal_literal_forms_are_constants() {
    let mut it = Interner::new();
    // Each spelling interns to the value it denotes.
    let cases = [
        ("2", "2"),
        ("1.5", "1.5"),
        (".5", "0.5"),
        ("2e3", "2000"),
        ("1.5e-7", "1.5e-7"),
        ("2E-3", "0.002"),
        ("2.", "2"),
    ];
    for (src, canonical) in cases {
        let got = parse_expr(src, &mut it).unwrap();
        let want = cst(&mut it, canonical);
        assert_eq!(got, want, "`{src}` must intern to the constant {canonical}");
        assert!(
            matches!(it.get(got), Expr::Constant(_)),
            "`{src}` is a constant"
        );
    }
}

#[test]
fn normal_binary_operators_shape() {
    let cases = [
        ("1 + 2", 1u8),
        ("1 - 2", 2),
        ("2 * 3", 3),
        ("2 / 3", 4),
        ("2 ^ 3", 5),
    ];
    for (src, tag) in cases {
        let (mut it, got) = ok(src);
        let (l, r) = split_two(src);
        let a = cst(&mut it, l);
        let b = cst(&mut it, r);
        let want = match tag {
            1 => Expr::Add(a, b),
            2 => Expr::Sub(a, b),
            3 => Expr::Mul(a, b),
            4 => Expr::Div(a, b),
            _ => Expr::Pow(a, b),
        };
        assert_eq!(got, node(&mut it, want), "`{src}` shape");
    }
}

/// Splits a `"<l> <op> <r>"` three-token source into its operand lexemes.
fn split_two(src: &str) -> (&str, &str) {
    let mut parts = src.split_whitespace();
    let l = parts.next().expect("left operand");
    let _op = parts.next().expect("operator");
    let r = parts.next().expect("right operand");
    (l, r)
}

#[test]
fn normal_addition_left_associative() {
    // "1 - 2 - 3" == Sub(Sub(1,2),3).
    let (mut it, got) = ok("1 - 2 - 3");
    let a = cst(&mut it, "1");
    let b = cst(&mut it, "2");
    let c = cst(&mut it, "3");
    let left = node(&mut it, Expr::Sub(a, b));
    let want = node(&mut it, Expr::Sub(left, c));
    assert_eq!(got, want);
}

#[test]
fn normal_multiplication_division_left_associative() {
    // "2 / 3 / 4" == Div(Div(2,3),4).
    let (mut it, got) = ok("2 / 3 / 4");
    let a = cst(&mut it, "2");
    let b = cst(&mut it, "3");
    let c = cst(&mut it, "4");
    let left = node(&mut it, Expr::Div(a, b));
    let want = node(&mut it, Expr::Div(left, c));
    assert_eq!(got, want);
}

#[test]
fn normal_power_right_associative() {
    // "2^3^4" == Pow(2, Pow(3,4)).
    let (mut it, got) = ok("2^3^4");
    let a = cst(&mut it, "2");
    let b = cst(&mut it, "3");
    let c = cst(&mut it, "4");
    let right = node(&mut it, Expr::Pow(b, c));
    let want = node(&mut it, Expr::Pow(a, right));
    assert_eq!(got, want);
}

#[test]
fn normal_precedence_mul_binds_tighter_than_add() {
    // "1 + 2 * 3" == Add(1, Mul(2,3)).
    let (mut it, got) = ok("1 + 2 * 3");
    let one = cst(&mut it, "1");
    let two = cst(&mut it, "2");
    let three = cst(&mut it, "3");
    let mul = node(&mut it, Expr::Mul(two, three));
    let want = node(&mut it, Expr::Add(one, mul));
    assert_eq!(got, want);
}

#[test]
fn normal_precedence_pow_binds_tighter_than_mul() {
    // "2 * 3 ^ 4" == Mul(2, Pow(3,4)).
    let (mut it, got) = ok("2 * 3 ^ 4");
    let two = cst(&mut it, "2");
    let three = cst(&mut it, "3");
    let four = cst(&mut it, "4");
    let pow = node(&mut it, Expr::Pow(three, four));
    let want = node(&mut it, Expr::Mul(two, pow));
    assert_eq!(got, want);
}

#[test]
fn normal_unary_minus_binds_tighter_than_mul() {
    // "-2*3" == Mul(Neg(2), 3) — the printer renders "-2*3" for exactly this tree.
    let (mut it, got) = ok("-2*3");
    let two = cst(&mut it, "2");
    let three = cst(&mut it, "3");
    let neg = node(&mut it, Expr::Neg(two));
    let want = node(&mut it, Expr::Mul(neg, three));
    assert_eq!(got, want);
}

#[test]
fn normal_unary_minus_looser_than_power() {
    // "-2^3" == Neg(Pow(2,3)) — power binds tighter than unary minus.
    let (mut it, got) = ok("-2^3");
    let two = cst(&mut it, "2");
    let three = cst(&mut it, "3");
    let pow = node(&mut it, Expr::Pow(two, three));
    let want = node(&mut it, Expr::Neg(pow));
    assert_eq!(got, want);
}

#[test]
fn normal_double_negation() {
    // "- -5" == Neg(Neg(5)).
    let (mut it, got) = ok("- -5");
    let five = cst(&mut it, "5");
    let inner = node(&mut it, Expr::Neg(five));
    let want = node(&mut it, Expr::Neg(inner));
    assert_eq!(got, want);
}

#[test]
fn normal_parens_override_associativity() {
    let cases = [
        // ("input", expected builder).
        "1 - (2 - 3)",
        "1/(2/3)",
        "(2 + 3)^3",
    ];
    // "1 - (2 - 3)" == Sub(1, Sub(2,3)).
    let (mut it, got) = ok(cases[0]);
    let a = cst(&mut it, "1");
    let b = cst(&mut it, "2");
    let c = cst(&mut it, "3");
    let inner = node(&mut it, Expr::Sub(b, c));
    assert_eq!(got, node(&mut it, Expr::Sub(a, inner)));

    // "1/(2/3)" == Div(1, Div(2,3)).
    let (mut it, got) = ok(cases[1]);
    let a = cst(&mut it, "1");
    let b = cst(&mut it, "2");
    let c = cst(&mut it, "3");
    let inner = node(&mut it, Expr::Div(b, c));
    assert_eq!(got, node(&mut it, Expr::Div(a, inner)));

    // "(2 + 3)^3" == Pow(Add(2,3), 3).
    let (mut it, got) = ok(cases[2]);
    let a = cst(&mut it, "2");
    let b = cst(&mut it, "3");
    let sum = node(&mut it, Expr::Add(a, b));
    assert_eq!(got, node(&mut it, Expr::Pow(sum, b)));
}

#[test]
fn normal_reserved_atoms() {
    let mut it = Interner::new();
    let pi = parse_expr("pi", &mut it).unwrap();
    assert!(matches!(it.get(pi), Expr::Pi));
    let x = parse_expr("_x_", &mut it).unwrap();
    assert!(matches!(it.get(x), Expr::Variable));
    let nan = parse_expr("NaN", &mut it).unwrap();
    let got_nan = match it.get(nan) {
        Expr::Constant(c) => c.is_nan(),
        other => panic!("NaN must be a constant, got {other:?}"),
    };
    assert!(got_nan, "`NaN` interns to a NaN constant");
}

#[test]
fn normal_function_application_single_arg() {
    let (mut it, got) = ok("sqrt(2)");
    let two = cst(&mut it, "2");
    let want = node(&mut it, Expr::Apply(BaseFn::Sqrt, two));
    assert_eq!(got, want);
}

#[test]
fn normal_log_alias_is_natural_log() {
    // `log` and `ln` must intern to the identical Apply(Ln, _) node.
    let mut it = Interner::new();
    let via_log = parse_expr("log(_x_)", &mut it).unwrap();
    let via_ln = parse_expr("ln(_x_)", &mut it).unwrap();
    assert_eq!(via_log, via_ln, "`log` must alias `ln` (BaseFn::Ln)");
    let x = cst_var(&mut it);
    assert_eq!(via_log, node(&mut it, Expr::Apply(BaseFn::Ln, x)));
}

fn cst_var(it: &mut Interner) -> NodeId {
    it.intern(Expr::Variable)
}

#[test]
fn normal_function_argument_is_full_expression() {
    // "sqrt(1 + 2)" == Apply(Sqrt, Add(1,2)).
    let (mut it, got) = ok("sqrt(1 + 2)");
    let a = cst(&mut it, "1");
    let b = cst(&mut it, "2");
    let add = node(&mut it, Expr::Add(a, b));
    assert_eq!(got, node(&mut it, Expr::Apply(BaseFn::Sqrt, add)));
}

#[test]
fn normal_whitespace_is_insignificant() {
    let mut it = Interner::new();
    let a = parse_expr("1 + 2", &mut it).unwrap();
    let b = parse_expr("  1\n+\t2  ", &mut it).unwrap();
    let c = parse_expr("1+2", &mut it).unwrap();
    assert_eq!(a, b);
    assert_eq!(a, c);
}

#[test]
fn normal_sample_expression_shape() {
    // Spot check from the task spec: "sqrt(2)^2 + pi/3".
    // == Add(Pow(Apply(Sqrt,2), 2), Div(Pi, 3)).
    let (mut it, got) = ok("sqrt(2)^2 + pi/3");
    let two = cst(&mut it, "2");
    let three = cst(&mut it, "3");
    let sqrt = node(&mut it, Expr::Apply(BaseFn::Sqrt, two));
    let pow = node(&mut it, Expr::Pow(sqrt, two));
    let pi = it.intern(Expr::Pi);
    let div = node(&mut it, Expr::Div(pi, three));
    assert_eq!(got, node(&mut it, Expr::Add(pow, div)));
}

// ---- Extreme cases --------------------------------------------------------

#[test]
fn extreme_deep_grouping_collapses_to_atom() {
    // 200 nested parentheses: grouping adds no AST node.
    let src = format!("{}1{}", "(".repeat(200), ")".repeat(200));
    let (mut it, got) = ok(&src);
    assert_eq!(
        got,
        cst(&mut it, "1"),
        "200 nested parens must yield constant 1"
    );
}

#[test]
fn extreme_long_addition_chain() {
    // 200 terms of "1 +" — left-associative chain of 199 Add nodes.
    let n = 200;
    let src = std::iter::repeat_n("1", n).collect::<Vec<_>>().join(" + ");
    let (mut it, got) = ok(&src);
    let mut acc = cst(&mut it, "1");
    let one = cst(&mut it, "1");
    for _ in 1..n {
        acc = node(&mut it, Expr::Add(acc, one));
    }
    assert_eq!(got, acc, "long left-associative chain shape");
}

#[test]
fn extreme_tiny_and_huge_literals_parse_as_constants() {
    // Tiny/huge magnitudes must parse to a constant without panic (arbitrary
    // precision, no overflow). Dashu stores the decimal compactly (significand +
    // exponent), so `1e300` / `1e-300` are cheap.
    let mut it = Interner::new();
    for src in ["1e-300", "1e300", "1e1000000"] {
        let got = parse_expr(src, &mut it)
            .unwrap_or_else(|e| panic!("`{src}` must parse without panic: {e:?}"));
        assert!(
            matches!(it.get(got), Expr::Constant(_)),
            "`{src}` is a constant"
        );
    }
    // Same string, same node (hash-consing), and the node count stays small.
    let a = parse_expr("1e-300", &mut it).unwrap();
    let b = parse_expr("1e-300", &mut it).unwrap();
    assert_eq!(a, b, "identical literal reuses the node");
}

#[test]
fn extreme_leading_and_trailing_dot_forms() {
    // ".5" == 0.5 and "5." == 5, matching the constant node from a plain spelling.
    let mut it = Interner::new();
    let lead = parse_expr(".5", &mut it).unwrap();
    assert_eq!(lead, cst(&mut it, "0.5"));
    let trail = parse_expr("5.", &mut it).unwrap();
    assert_eq!(trail, cst(&mut it, "5"));
}

#[test]
fn extreme_nested_function_application() {
    // 30 nested sqrt calls produce a 30-deep Apply chain (parse is iterative).
    let depth = 30;
    let mut src = String::new();
    for _ in 0..depth {
        src.push_str("sqrt(");
    }
    src.push_str("_x_");
    for _ in 0..depth {
        src.push(')');
    }
    let (mut it, got) = ok(&src);
    let mut acc = cst_var(&mut it);
    for _ in 0..depth {
        acc = node(&mut it, Expr::Apply(BaseFn::Sqrt, acc));
    }
    assert_eq!(got, acc);
}

#[test]
fn extreme_all_function_names_parse() {
    // Every one of the 30 base-function names plus the `log` alias.
    let names: [(&str, BaseFn); 31] = [
        ("sqrt", BaseFn::Sqrt),
        ("exp", BaseFn::Exp),
        ("ln", BaseFn::Ln),
        ("log", BaseFn::Ln),
        ("sin", BaseFn::Sin),
        ("cos", BaseFn::Cos),
        ("tan", BaseFn::Tan),
        ("asin", BaseFn::Asin),
        ("acos", BaseFn::Acos),
        ("atan", BaseFn::Atan),
        ("sinh", BaseFn::Sinh),
        ("cosh", BaseFn::Cosh),
        ("tanh", BaseFn::Tanh),
        ("asinh", BaseFn::Asinh),
        ("acosh", BaseFn::Acosh),
        ("atanh", BaseFn::Atanh),
        ("erf", BaseFn::Erf),
        ("erfc", BaseFn::Erfc),
        ("log2", BaseFn::Log2),
        ("log10", BaseFn::Log10),
        ("log1p", BaseFn::Log1p),
        ("expm1", BaseFn::Expm1),
        ("abs", BaseFn::Abs),
        ("ceil", BaseFn::Ceil),
        ("floor", BaseFn::Floor),
        ("nearestint", BaseFn::NearestInt),
        ("double", BaseFn::Double),
        ("single", BaseFn::Single),
        ("halfprecision", BaseFn::HalfPrecision),
        ("quad", BaseFn::Quad),
        ("doubleextended", BaseFn::DoubleExtended),
    ];
    for (name, variant) in names {
        let src = format!("{name}(_x_)");
        let (mut it, got) = ok(&src);
        let x = cst_var(&mut it);
        let want = node(&mut it, Expr::Apply(variant, x));
        assert_eq!(got, want, "`{src}` must apply {variant:?}");
    }
}

// ---- Error cases ----------------------------------------------------------

#[track_caller]
fn err(src: &str) -> ParseError {
    let mut it = Interner::new();
    parse_expr(src, &mut it).expect_err("expected a parse error")
}

#[test]
fn error_empty_and_whitespace_only() {
    assert!(matches!(
        err(""),
        ParseError::UnexpectedEof { location: 0, .. }
    ));
    // The built-in lexer skips whitespace, so the EOF location is the end of the
    // last *token* — with none present that is the origin (offset 0), not the
    // raw source length. Only the variant is pinned here.
    assert!(matches!(err("   \t\n "), ParseError::UnexpectedEof { .. }));
}

#[test]
fn error_trailing_binary_operator() {
    let e = err("1 +");
    match e {
        ParseError::UnexpectedEof { location, .. } => assert_eq!(location, 3),
        other => panic!("trailing operator must be UnexpectedEof, got {other:?}"),
    }
}

#[test]
fn error_missing_operand_before_operator() {
    // "* 2" — the leading operator is unexpected at byte 0.
    let e = err("* 2");
    match e {
        ParseError::UnexpectedToken { start, token, .. } => {
            assert_eq!(start, 0);
            assert_eq!(token, "*");
        }
        other => panic!("leading `*` must be UnexpectedToken, got {other:?}"),
    }
}

#[test]
fn error_operator_sequence() {
    // "1 + * 2" — the second operator is unexpected at byte 4.
    let e = err("1 + * 2");
    match e {
        ParseError::UnexpectedToken {
            start, end, token, ..
        } => {
            assert_eq!(start, 4);
            assert_eq!(end, 5);
            assert_eq!(token, "*");
        }
        other => panic!("`+ *` must be UnexpectedToken, got {other:?}"),
    }
}

#[test]
fn error_unclosed_parenthesis() {
    let e = err("(1 + 2");
    match e {
        ParseError::UnexpectedEof { location, .. } => assert_eq!(location, 6),
        other => panic!("unclosed `(` must be UnexpectedEof, got {other:?}"),
    }
}

#[test]
fn error_extra_closing_parenthesis() {
    let e = err("1 + 2)");
    match e {
        ParseError::UnexpectedToken { start, token, .. } => {
            assert_eq!(start, 5);
            assert_eq!(token, ")");
        }
        other => panic!("extra `)` must be UnexpectedToken, got {other:?}"),
    }
}

#[test]
fn error_unknown_identifier_is_invalid_token() {
    // Symbol fallback is a later (B2+) command-layer feature, so an identifier
    // lexes as no token at all.
    let e = err("foo");
    match e {
        ParseError::InvalidToken { location } => assert_eq!(location, 0),
        other => panic!("unknown identifier must be InvalidToken, got {other:?}"),
    }
    let e = err("1 + foo");
    match e {
        ParseError::InvalidToken { location } => assert_eq!(location, 4),
        other => panic!("trailing unknown identifier must be InvalidToken, got {other:?}"),
    }
}

#[test]
fn error_malformed_literals() {
    // Dangling exponent marker.
    assert!(matches!(
        err("1e"),
        ParseError::InvalidToken { location: 1 }
    ));
    // Hex is not part of this grammar.
    assert!(matches!(
        err("0x10"),
        ParseError::InvalidToken { location: 1 }
    ));
    // Two dots: "1.2" then a stray ".3".
    let e = err("1.2.3");
    match e {
        ParseError::UnexpectedToken { start, token, .. } => {
            assert_eq!(start, 3);
            assert_eq!(token, ".3");
        }
        other => panic!("`1.2.3` must be UnexpectedToken, got {other:?}"),
    }
    // A lone dot is not a valid number or token.
    assert!(matches!(err("."), ParseError::InvalidToken { location: 0 }));
}

#[test]
fn error_special_value_spellings_not_implemented() {
    // Only `NaN` is accepted; lowercase `nan` and `inf`/`infinity` are future
    // distillation items and must not parse.
    assert!(matches!(err("nan"), ParseError::InvalidToken { .. }));
    assert!(matches!(err("inf"), ParseError::InvalidToken { .. }));
    assert!(matches!(err("infinity"), ParseError::InvalidToken { .. }));
}

#[test]
fn error_function_name_without_argument() {
    // A function name is a reserved keyword and requires an argument in parens.
    let e = err("sqrt");
    match e {
        ParseError::UnexpectedEof { .. } => {}
        other => panic!("bare `sqrt` must be UnexpectedEof, got {other:?}"),
    }
    // Empty argument list is also rejected.
    assert!(matches!(err("sqrt()"), ParseError::UnexpectedToken { .. }));
}

#[test]
fn error_juxtaposition_of_two_operands() {
    let e = err("1 2");
    match e {
        ParseError::UnexpectedToken { start, token, .. } => {
            assert_eq!(start, 2);
            assert_eq!(token, "2");
        }
        other => panic!("juxtaposition must be UnexpectedToken, got {other:?}"),
    }
}

#[test]
fn error_message_reports_position_text() {
    // The Display of an error embeds the byte offset, so callers can surface it.
    let msg = err("1 + foo").to_string();
    assert!(
        msg.contains("4"),
        "message must carry the offending byte offset: {msg}"
    );
}

// ---- Interruption / robustness cases --------------------------------------

#[test]
fn robustness_large_input_does_not_panic() {
    // ~12 KB left-associative chain; LR parsing is iterative (no stack growth).
    let n = 3000;
    let src = std::iter::repeat_n("1", n).collect::<Vec<_>>().join(" + ");
    assert!(src.len() >= 10 * 1024, "test input must exceed 10 KB");
    let (it, got) = ok(&src);
    assert!(matches!(it.get(got), Expr::Add(_, _)));
    // Interning shared the `1` constant, so the node count is O(chain) not O(1).
    assert_eq!(it.len(), n); // 1 constant + (n-1) sums
}

#[test]
fn robustness_repeated_parse_is_deterministic() {
    let src = "sqrt(1 + _x_ * 2) ^ (pi / 3) - NaN";
    let mut it = Interner::new();
    let first = parse_expr(src, &mut it).unwrap();
    let len_after_first = it.len();
    let second = parse_expr(src, &mut it).unwrap();
    assert_eq!(
        first, second,
        "re-parsing the same source must reuse the node"
    );
    assert_eq!(
        it.len(),
        len_after_first,
        "re-parse must not intern any new nodes (hash-consing)"
    );
}

#[test]
fn robustness_adversarial_inputs_never_panic() {
    // A spread of malformed / hostile inputs: each must return Ok or Err and
    // never panic, and the interner must stay self-consistent.
    let corpus = [
        "",
        "(",
        ")",
        "()",
        "((",
        "))",
        "1 +",
        "+ 1",
        "*",
        "/",
        "^",
        "-",
        "--",
        "---",
        "1 2 3 4",
        "sqrt(",
        "sqrt(_x_",
        "(((1)))",
        "1e999999",
        "1.1.1.1",
        "pi pi",
        "_x__x_",
        "@",
        "$",
        "a",
        "1 + 2 +",
        "(1))",
        "((1)",
        "2^2^2^2^2",
        "- - - -1",
        "\u{0}",
        "2e+e3",
        "..",
        "1..2",
        "sqrt(((((1))))",
        "(((((((((((_x_))))))))))",
    ];
    for src in corpus {
        let mut it = Interner::new();
        let result = parse_expr(src, &mut it);
        // No panic reaching here; also confirm the interner is readable.
        if let Ok(id) = result {
            let _ = it.get(id);
        }
    }
}

#[test]
fn robustness_interner_shared_across_parses() {
    let mut it = Interner::new();
    let a = parse_expr("1 + 2", &mut it).unwrap();
    let b = parse_expr("2 + 1", &mut it).unwrap();
    // Constants are shared, but Add(1,2) != Add(2,1) (operand order matters).
    assert_ne!(a, b);
    let c = parse_expr("1 + 2", &mut it).unwrap();
    assert_eq!(a, c, "same structure reuses the same node id");
}
