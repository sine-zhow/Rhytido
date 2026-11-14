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
use rhytido_lang::SymbolId;
use rhytido_lang::ast::{ProgramExpr, Script, Stmt};
use rhytido_lang::parse_expr;
use rhytido_lang::parse_script;

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
fn error_unknown_identifier_is_rejected_by_the_grammar() {
    // M3-B2 adaptation (same strength, different variant): a name is a token now
    // that the statement grammar needs it, so `parse_expr` no longer fails in
    // the lexer but at the parse — symbol fallback itself is still M3-C. The
    // failure class and the byte position are both still asserted.
    let e = err("foo");
    match e {
        ParseError::UnexpectedToken {
            start, end, token, ..
        } => {
            assert_eq!(start, 0);
            assert_eq!(end, 3);
            assert_eq!(token, "foo");
        }
        other => panic!("unknown identifier must be UnexpectedToken, got {other:?}"),
    }
    let e = err("1 + foo");
    match e {
        ParseError::UnexpectedToken { start, token, .. } => {
            assert_eq!(start, 4);
            assert_eq!(token, "foo");
        }
        other => panic!("trailing unknown identifier must be UnexpectedToken, got {other:?}"),
    }
}

#[test]
fn error_malformed_literals() {
    // M3-B2 adaptation (same strength, different variant): a letter run that
    // used to be unlexable is now an identifier token, so the malformed-number
    // spellings fail at the parse instead of at the lexer. Position, lexeme and
    // failure are still pinned, and the two cases the new token does not touch
    // (`.3`, `.`) keep their original variants.
    // Dangling exponent marker: `1` then the name `e`.
    let e = err("1e");
    match e {
        ParseError::UnexpectedToken { start, token, .. } => {
            assert_eq!(start, 1);
            assert_eq!(token, "e");
        }
        other => panic!("`1e` must be UnexpectedToken, got {other:?}"),
    }
    // Hex is not part of this grammar: `0` then the name `x10`.
    let e = err("0x10");
    match e {
        ParseError::UnexpectedToken { start, token, .. } => {
            assert_eq!(start, 1);
            assert_eq!(token, "x10");
        }
        other => panic!("`0x10` must be UnexpectedToken, got {other:?}"),
    }
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
    // distillation items. M3-B2 adaptation: they are names now, so the grammar
    // refuses them (a name is not mathematics) rather than the lexer.
    for src in ["nan", "inf", "infinity"] {
        let e = err(src);
        match e {
            ParseError::UnexpectedToken { start, token, .. } => {
                assert_eq!(start, 0, "`{src}` must be rejected at the origin");
                assert_eq!(token, src, "the rejected lexeme must be reported");
            }
            other => panic!("`{src}` must be UnexpectedToken, got {other:?}"),
        }
    }
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

// ===========================================================================
// Statement grammar (M3-B2).
//
// `parse_script` is a second entry point over a second precedence chain, so the
// four case classes are repeated for it here at the *surface* level (what parses,
// what is refused, what position is reported). The shape of the trees it builds is
// the business of `tests/ast.rs` and `tests/integration_parse_scripts.rs`.
// ===========================================================================

/// Parses `src` as a script, panicking with the error text on failure.
fn script(src: &str) -> Script {
    let mut it = Interner::new();
    parse_script(src, &mut it).unwrap_or_else(|e| panic!("expected `{src}` to parse, got {e:?}"))
}

/// The statement count a script parsed into.
fn count(src: &str) -> usize {
    script(src).len()
}

/// Parses `src` as a script expecting failure, with a fresh interner.
fn script_err(src: &str) -> ParseError {
    let mut it = Interner::new();
    parse_script(src, &mut it).expect_err("expected a parse error")
}

/// Asserts that `src` is refused. The refusal itself is the assertion: the
/// helper panics if the source parses.
fn refuses(src: &str) {
    let _ = script_err(src);
}

/// The byte offset a failure points at, whatever its class.
fn offset(err: &ParseError) -> usize {
    match err {
        ParseError::InvalidToken { location } => *location,
        ParseError::UnexpectedToken { start, .. } => *start,
        ParseError::ExtraToken { start, .. } => *start,
        ParseError::UnexpectedEof { location, .. } => *location,
        ParseError::Internal { .. } => panic!("the grammar emits no fallible actions"),
    }
}

/// Finds the identifier a script interned under `text`.
fn sym(script: &Script, text: &str) -> SymbolId {
    (0..script.symbols().len() as u32)
        .map(SymbolId::new)
        .find(|&id| script.name_of(id) == Some(text))
        .unwrap_or_else(|| panic!("`{text}` was never interned by this parse"))
}

// ---- Normal: statement forms ----------------------------------------------

#[test]
fn normal_statement_terminators_separate_every_form() {
    // One of each legal statement shape, in one script.
    assert_eq!(count("x = 1;"), 1);
    assert_eq!(count("x := 1;"), 1);
    assert_eq!(count("x = 1!;"), 1);
    assert_eq!(count("print(1);"), 1);
    assert_eq!(count("{};"), 1);
    assert_eq!(count("x;"), 1);
    assert_eq!(count("f(1, 2);"), 1);
    assert_eq!(count("x = proc() {} ;"), 1);
    assert_eq!(count("procedure f() {};"), 1);
    assert_eq!(count("{ var x; };"), 1);
    assert_eq!(count("x = 1;\ny = 2;\nz = 3;\n"), 3);
}

#[test]
fn normal_whitespace_and_newlines_are_insignificant_between_statements() {
    let one = script("x=1;y=2;");
    let spaced = script("  x  =  1 ;\n\n  y = 2 ;  ");
    assert_eq!(one.statements(), spaced.statements());
    assert_eq!(one.symbols(), spaced.symbols());
}

#[test]
fn normal_var_leads_a_block_and_multiple_declarations_are_allowed() {
    // `var` may appear several times, but only before any other statement.
    assert_eq!(count("{var a; var b; a = 1;};"), 1);
    assert_eq!(count("{var a, b, c;};"), 1);
    let block = script("{var a; a = 1;};");
    let Stmt::Block(inner) = &block.statements()[0] else {
        panic!("a block was parsed");
    };
    assert_eq!(inner.len(), 2);
    assert!(matches!(inner[0], Stmt::Declare(_)));
    assert!(matches!(inner[1], Stmt::Assign { .. }));
}

#[test]
fn normal_return_is_accepted_at_the_end_of_a_procedure_body() {
    assert_eq!(count("f = proc() { return 1; };"), 1);
    assert_eq!(count("f = proc() { return; };"), 1);
    assert_eq!(count("f = proc() { x = 1; return x; };"), 1);
    assert_eq!(count("procedure f() { return 1; };"), 1);
}

#[test]
fn normal_call_accepts_any_arity_and_nesting() {
    assert_eq!(count("f();"), 1);
    assert_eq!(count("f(1);"), 1);
    assert_eq!(count("f(1, 2, 3, 4, 5);"), 1);
    assert_eq!(count("f(g(h(1)));"), 1);
    assert_eq!(count("f(1, g(2), [|3|], \"four\");"), 1);
    // A call is an expression, so it composes with mathematics.
    assert_eq!(count("f(1) + g(2);"), 1);
    assert_eq!(count("1 + f(2) * 3;"), 1);
}

#[test]
fn normal_list_and_string_literals_take_any_elements() {
    assert_eq!(count("L = [||];"), 1);
    assert_eq!(count("L = [|1|];"), 1);
    assert_eq!(count("L = [|1, 2, true, \"three\", x|];"), 1);
    assert_eq!(count("L = [|[|1|], [|2|]|];"), 1);
    assert_eq!(count("L = [|proc(a) { return a; }|];"), 1);
    assert_eq!(count("s = \"\";"), 1);
    assert_eq!(count("s = \"Hello world\";"), 1);
    assert_eq!(count("s = \"tab\\there\";"), 1);
    // List elements are full expressions, so arithmetic inside them interns.
    let list = script("L = [|1 + 2, 3 * 4|];");
    let value = match &list.statements()[0] {
        Stmt::Assign { value, .. } => value,
        other => panic!("an assignment was parsed, got {other:?}"),
    };
    let elements = value.as_list().expect("a list literal");
    assert_eq!(elements.len(), 2);
    assert!(elements.iter().all(ProgramExpr::is_math));
}

#[test]
fn normal_keywords_win_over_the_identifier_matcher() {
    // The lexer prefers the longest match and breaks a tie in favour of the
    // keyword. So a keyword stays a keyword, while a name that merely starts
    // with one is a name — and names that shadow base-function spellings are
    // names too, exactly as `log2` vs `log` already works in the maths grammar.
    assert!(matches!(
        script("printer = 3;").statements()[0],
        Stmt::Assign { .. }
    ));
    assert!(matches!(
        script("print(3);").statements()[0],
        Stmt::Print { .. }
    ));
    assert!(matches!(
        script("procxy = 3;").statements()[0],
        Stmt::Assign { .. }
    ));
    assert!(matches!(
        script("returnee = 3;").statements()[0],
        Stmt::Assign { .. }
    ));
    assert!(matches!(
        script("various = 3;").statements()[0],
        Stmt::Assign { .. }
    ));
    // A name that is also a base-function spelling is still a function in the
    // mathematics, and `sine` (longer) is still a name in a call position.
    assert!(matches!(
        script("sine(3);").statements()[0],
        Stmt::ExprStmt(ProgramExpr::Call { .. })
    ));
}

#[test]
fn normal_print_redirect_takes_a_string_literal() {
    assert_eq!(count(r#"print(1) > "f.sol";"#), 1);
    assert_eq!(count(r#"print(1) >> "f.sol";"#), 1);
    assert_eq!(count(r#"print(1, 2) > "f.sol";"#), 1);
}

#[test]
fn normal_procedure_literal_takes_every_documented_formal_shape() {
    assert_eq!(count("f = proc() {};"), 1);
    assert_eq!(count("f = proc(a) {};"), 1);
    assert_eq!(count("f = proc(a, b, c) {};"), 1);
    assert_eq!(count("f = proc(L = ...) {};"), 1);
    // Mixing is accepted by the grammar; whether it is meaningful is the
    // evaluator's question (open distillation item, sollya-language-b2 §4.2).
    assert_eq!(count("f = proc(a, L = ...) {};"), 1);
}

#[test]
fn normal_identifiers_accept_the_documented_spellings() {
    for src in [
        "x = 1;",
        "_ = 1;",
        "_x1 = 1;",
        "__argv = 1;",
        "__unique_id = 1;",
        "a1b2c3 = 1;",
        "X = 1;",
        "x_ = 1;",
    ] {
        assert_eq!(count(src), 1, "`{src}` must parse");
    }
}

// ---- Extreme --------------------------------------------------------------

#[test]
fn extreme_nested_blocks_parse_without_recursion_in_the_parser() {
    // LR parsing is iterative, so nesting depth costs stack in the *tree*, not
    // in the parser.
    let depth = 60;
    let src = format!("{}x = 1;{}", "{".repeat(depth), "};".repeat(depth));
    let parsed = script(&src);
    assert_eq!(parsed.len(), 1);
    let mut cursor = &parsed.statements()[0];
    let mut walked = 0;
    while let Some(inner) = cursor.as_block() {
        assert_eq!(inner.len(), 1, "each block holds exactly one statement");
        cursor = &inner[0];
        walked += 1;
    }
    assert_eq!(walked, depth, "the nesting is as deep as it was written");
    assert!(matches!(cursor, Stmt::Assign { .. }));
}

#[test]
fn extreme_deeply_nested_procedure_literals_parse() {
    let depth = 40;
    // `return proc(a) { return proc(a) { ... return 1; }; };` nested 40 deep:
    // every level is a procedure whose body ends in the one `return` it allows.
    let mut inner = "return 1;".to_string();
    for _ in 0..depth {
        inner = format!("return proc(a) {{ {inner} }};");
    }
    let src = format!("f = proc() {{ {inner} }};");
    let parsed = script(&src);
    assert_eq!(parsed.len(), 1, "the nesting is one assignment");

    let Stmt::Assign { value, .. } = &parsed.statements()[0] else {
        panic!("an assignment was parsed");
    };
    let mut current = value.as_proc().expect("a procedure literal");
    for level in 0..depth {
        let Stmt::Return(Some(value)) = &current.body()[0] else {
            panic!("level {level} does not end in a valued return");
        };
        current = value.as_proc().expect("a nested procedure literal");
        assert_eq!(current.formals().len(), 1);
    }
    assert_eq!(
        current.returned().flatten().map(ProgramExpr::is_math),
        Some(true),
        "the innermost body returns the constant"
    );
}

#[test]
fn extreme_long_statement_sequence_parses() {
    let n = 1000;
    let src: String = (0..n).map(|i| format!("v{i} = {i};\n")).collect();
    let parsed = script(&src);
    assert_eq!(parsed.len(), n);
    assert_eq!(parsed.symbols().len(), n);
}

#[test]
fn extreme_empty_containers_and_nullary_forms_parse() {
    // Each of these is the zero case of a list-shaped rule.
    assert_eq!(count(""), 0);
    assert_eq!(count("{};"), 1);
    assert_eq!(count("L = [||];"), 1);
    assert_eq!(count("f();"), 1);
    assert_eq!(count("f = proc() {};"), 1);
    assert_eq!(count("procedure f() {};"), 1);
    assert_eq!(count("print();"), 1);
    assert_eq!(count("s = \"\";"), 1);
}

#[test]
fn extreme_long_call_argument_list_and_deep_call_chain() {
    let args = (0..200)
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    assert_eq!(count(&format!("f({args});")), 1);
    let mut chain = "1".to_string();
    for _ in 0..100 {
        chain = format!("f({chain})");
    }
    assert_eq!(count(&format!("{chain};")), 1);
}

#[test]
fn extreme_mathematical_precedence_is_unchanged_at_statement_level() {
    // The program chain mirrors the mathematical chain level for level, so the
    // grouping the maths grammar gives must be reproduced inside a statement.
    let (mut it, script) = {
        let mut it = Interner::new();
        let s = parse_script("x = 1 + 2 * 3 ^ -4;", &mut it).expect("parses");
        (it, s)
    };
    let one = cst(&mut it, "1");
    let two = cst(&mut it, "2");
    let three = cst(&mut it, "3");
    let four = cst(&mut it, "4");
    let neg = node(&mut it, Expr::Neg(four));
    let pow = node(&mut it, Expr::Pow(three, neg));
    let mul = node(&mut it, Expr::Mul(two, pow));
    let want = node(&mut it, Expr::Add(one, mul));
    let got = match &script.statements()[0] {
        Stmt::Assign { value, .. } => value.as_math().expect("identifier-free maths"),
        other => panic!("an assignment was parsed, got {other:?}"),
    };
    assert_eq!(got, want);
    // And it is the very node `parse_expr` builds.
    let mut fresh = Interner::new();
    assert_eq!(
        got,
        parse_expr("1 + 2 * 3 ^ -4", &mut fresh).expect("maths")
    );
}

#[test]
fn extreme_string_escapes_all_documented_forms() {
    // Every escape the public documentation lists must resolve.
    let src = r#"s = "\\A\"B\?C\'D\E\F\G\H\Ia\j\k\l\m\n\o\p\q\r\s\t\u\v\w\x\y\z";"#;
    // Only the documented subset is decoded; the rest must survive verbatim
    // rather than being dropped, and the statement must still parse.
    let parsed = script(src);
    let value = match &parsed.statements()[0] {
        Stmt::Assign { value, .. } => value,
        other => panic!("an assignment was parsed, got {other:?}"),
    };
    let text = value.as_string().expect("a string literal");
    assert!(text.contains("A"), "undocumented escapes keep their letter");
    assert!(text.contains('\n'), "\\n resolves to a newline");
    assert!(text.contains('\t'), "\\t resolves to a tab");
    assert!(text.contains('"'), "\\\" resolves to a quote");
    assert!(text.contains('\\'), "\\\\ resolves to one backslash");

    let numeric = script(r#"s = "\x41\101\0";"#);
    let value = match &numeric.statements()[0] {
        Stmt::Assign { value, .. } => value,
        other => panic!("an assignment was parsed, got {other:?}"),
    };
    assert_eq!(value.as_string(), Some("AA\0"));
}

// ---- Error: malformed scripts ---------------------------------------------

#[test]
fn error_missing_statement_terminator_is_reported_at_the_next_statement() {
    let e = script_err("x = 1\ny = 2;");
    assert!(offset(&e) > 0, "the failure must point somewhere: {e:?}");
    assert!(
        matches!(
            e,
            ParseError::UnexpectedToken { .. } | ParseError::ExtraToken { .. }
        ),
        "a missing `;` is a token-position failure, got {e:?}"
    );
    // The same holds between a print and the next statement.
    assert!(matches!(
        script_err("print(1) print(2);"),
        ParseError::UnexpectedToken { .. } | ParseError::ExtraToken { .. }
    ));
}

#[test]
fn error_unclosed_block_is_unexpected_eof() {
    let e = script_err("{x = 1;");
    match e {
        ParseError::UnexpectedEof { location, .. } => assert_eq!(location, 7),
        other => panic!("an unclosed block must be UnexpectedEof, got {other:?}"),
    }
    assert!(matches!(
        script_err("f = proc(a) { return a;"),
        ParseError::UnexpectedEof { .. }
    ));
    assert!(matches!(
        script_err("L = [|1"),
        ParseError::UnexpectedEof { .. }
    ));
    assert!(matches!(
        script_err("f(1"),
        ParseError::UnexpectedEof { .. }
    ));
}

#[test]
fn error_return_outside_a_procedure_body_is_refused() {
    // `return` belongs to the end of a procedure body and nowhere else.
    assert!(matches!(
        script_err("return 1;"),
        ParseError::UnexpectedToken { start: 0, .. }
    ));
    assert!(matches!(
        script_err("{ return 1; };"),
        ParseError::UnexpectedToken { .. }
    ));
    // Nor anywhere but the end of the body.
    refuses("f = proc(a) { return a; print(a); };");
    // Nor twice.
    refuses("f = proc(a) { return a; return a; };");
}

#[test]
fn error_var_after_another_statement_in_a_block_is_refused() {
    // Declarations lead the block: once a statement has appeared, `var` is over.
    assert!(matches!(
        script_err("{ x = 1; var a; };"),
        ParseError::UnexpectedToken { .. }
    ));
    assert!(matches!(
        script_err("{ x = 1; var a; y = 2; };"),
        ParseError::UnexpectedToken { .. }
    ));
    refuses("var");
    refuses("var ;");
    refuses("var 1;");
}

#[test]
fn error_illegal_formal_parameters_are_refused() {
    for src in [
        "f = proc(1) {};",
        "f = proc(a b) {};",
        "f = proc(= ...) {};",
        "f = proc(a =) {};",
        "f = proc(a = 1) {};",
        "f = proc(,a) {};",
        "f = proc(*a) {};",
    ] {
        let e = script_err(src);
        assert!(
            offset(&e) > 0 || matches!(e, ParseError::InvalidToken { .. }),
            "`{src}` must be refused with a position, got {e:?}"
        );
    }
}

#[test]
fn error_bare_keyword_misuse_is_refused() {
    // A *reserved* word is not an expression, so these cannot form a statement.
    for src in ["proc;", "return;", "var;", "print;", "procedure;"] {
        let e = script_err(src);
        assert!(
            !matches!(e, ParseError::Internal { .. }),
            "`{src}` must produce a real parse error, got {e:?}"
        );
    }
    // Reserved words cannot be used as names either.
    refuses("print = 3;");
    refuses("proc = 3;");
    refuses("var = 3;");
    refuses("return = 3;");
    refuses("procedure = 3;");
    // Nor as a callee.
    refuses("proc();");
    refuses("return(1);");
}

#[test]
fn normal_command_like_names_are_ordinary_identifiers() {
    // Only the five syntactic keywords are reserved. Every other documented
    // command name is an identifier here, and its meaning (including the
    // session lifecycle of `quit`/`restart`, §8.4 of the distillation) is
    // decided by the evaluator, not by the parser.
    for src in [
        "nop;",
        "nop();",
        "quit;",
        "restart;",
        "display = decimal;",
        "verbosity = 1!;",
        "autosimplify = off;",
        "x = error;",
        "return_of_a_name = 1;",
    ] {
        assert_eq!(count(src), 1, "`{src}` must parse");
    }
    let parsed = script("nop;");
    assert_eq!(
        parsed.statements()[0],
        Stmt::expr_stmt(ProgramExpr::var(sym(&parsed, "nop")))
    );
}

#[test]
fn error_incomplete_assignment_and_dangling_operator_are_refused() {
    assert!(matches!(
        script_err("x = ;"),
        ParseError::UnexpectedToken { .. } | ParseError::UnexpectedEof { .. }
    ));
    refuses("x =");
    refuses("x"); // a statement needs its terminator
    refuses("x +");
    refuses("= 3;"); // an assignment needs a name
    refuses("1 = 2;"); // a number is not a target
    refuses("x == 2;"); // comparison is M3-B3, not here
}

#[test]
fn error_unterminated_string_is_a_lexical_failure() {
    // No token matcher accepts an unterminated string, so the failure is lexical
    // and points at the opening quote's line, not at a parse position.
    let e = script_err("s = \"abc;");
    assert!(
        matches!(
            e,
            ParseError::InvalidToken { .. } | ParseError::UnexpectedEof { .. }
        ),
        "an unterminated string must be a lexical or EOF failure, got {e:?}"
    );
    // A raw newline inside the quotes is likewise not a token.
    refuses("s = \"a\nb\";");
    // Nor is a complete string with no statement terminator.
    refuses("s = \"abc\"");
}

#[test]
fn error_unsupported_notations_are_refused_not_misread() {
    // Constructs deliberately left out of M3-B2 (open distillation items) must be
    // refused rather than silently parsed into something else.
    for src in [
        "L[2] = 1;",                // indexed target: §8.1
        "print(L[2]);",             // indexing: §8.1
        "r = [1; 2];",              // interval literal: §8.1
        "L = [|1, ..., 5|];",       // end-elliptic list: §8.5
        "sumall @ [||];",           // list-splicing operator: §8.7
        "if (x) then y = 1;",       // control flow: M3-B3
        "for i in [||] do i;",      // control flow: M3-B3
        "match x with default: 1;", // patterns: M3-B3
        "while (x) do nop;",        // control flow: M3-B3
        "a.b = 3;",                 // structures: §8.7
        "x = 0x1p3;",               // base-prefix literals: §8.6
    ] {
        let e = script_err(src);
        assert!(
            !matches!(e, ParseError::Internal { .. }),
            "`{src}` must be refused cleanly, got {e:?}"
        );
    }
}

#[test]
fn error_message_reports_position_text_for_scripts() {
    let msg = script_err("{ x = 1;").to_string();
    assert!(
        msg.contains("8"),
        "the message must carry the byte offset: {msg}"
    );
    let msg = script_err("x = 1; }").to_string();
    assert!(msg.contains('7'), "a stray brace must be located: {msg}");
}

// ---- Interruption / robustness -------------------------------------------

#[test]
fn robustness_adversarial_scripts_never_panic() {
    let corpus = [
        "",
        ";",
        ";;;",
        "{",
        "}",
        "{}",
        "{};",
        "x =",
        "x = ;",
        "= x;",
        "proc",
        "proc(",
        "proc()",
        "proc(){}",
        "proc(){};",
        "return",
        "var",
        "print(",
        "print()",
        "print(,)",
        "[|",
        "|]",
        "[||]",
        "\"",
        "\"\\",
        "\"\\x\"",
        "x[",
        "x!;",
        "x = 1 !",
        "@",
        "$",
        "?",
        "\u{0}",
        "\u{1f600}",
        "x = 1;;y = 2;",
        "f(g(h(",
        "{{{{{{{{{{x;};};};};};};};};};};",
        "proc(a,a){return a;};",
        "1 + + 1;",
        "x := := 1;",
        "print(1) >",
        "print(1) > ",
        "x = \"unterminated",
        "procedure 1() {};",
    ];
    for src in corpus {
        let mut it = Interner::new();
        let result = parse_script(src, &mut it);
        // Reaching here is the assertion: no panic, and any accepted tree must be
        // readable back out of the interner.
        if let Ok(script) = result {
            for stmt in script.statements() {
                let _ = format!("{stmt:?}");
            }
        }
    }
}

#[test]
fn robustness_repeated_script_parse_is_deterministic() {
    let src = "add = proc(m, n) { var res; res := m + n; return res; };";
    let mut it = Interner::new();
    let first = parse_script(src, &mut it).expect("parses");
    let nodes_after_first = it.len();
    let symbols_after_first = first.symbols().len();
    let second = parse_script(src, &mut it).expect("re-parses");
    assert_eq!(
        it.len(),
        nodes_after_first,
        "a re-parse must intern no new node"
    );
    assert_eq!(
        first.symbols(),
        second.symbols(),
        "the same source must intern the same names"
    );
    assert_eq!(first.statements(), second.statements());
    assert_eq!(symbols_after_first, 4);
}

#[test]
fn robustness_oversized_script_stays_linear_and_does_not_panic() {
    // ~40 KB of statements: LR is iterative and the tree is built bottom-up, so
    // neither the parser nor the Drop of the result can overflow the stack here.
    let src: String = (0..4000)
        .map(|i| format!("v{i} = proc(a, b) {{ return a + {i}; }};\n"))
        .collect();
    assert!(src.len() >= 40 * 1024, "test input must exceed 40 KB");
    let parsed = script(&src);
    assert_eq!(parsed.len(), 4000);
    // Names are interned once per spelling, not once per occurrence.
    assert_eq!(parsed.symbols().len(), 4002, "4000 targets plus a and b");
}

#[test]
fn robustness_script_and_expr_share_one_interner_without_quarrelling() {
    // Interleaving the two entry points on one interner must not disturb either.
    let mut it = Interner::new();
    let via_script = match &parse_script("x = sqrt(2);\n", &mut it)
        .expect("parses")
        .statements()[0]
    {
        Stmt::Assign { value, .. } => value.as_math().expect("maths"),
        other => panic!("an assignment was parsed, got {other:?}"),
    };
    let before = it.len();
    let via_expr = parse_expr("sqrt(2)", &mut it).expect("maths");
    assert_eq!(via_script, via_expr, "one node for one structure");
    assert_eq!(it.len(), before, "the expression parse added nothing");
    // And a named expression from the script layer never claims a maths node.
    let named = parse_script("f = proc(x) { return sqrt(x); };", &mut it).expect("parses");
    let value = match &named.statements()[0] {
        Stmt::Assign { value, .. } => value,
        other => panic!("an assignment was parsed, got {other:?}"),
    };
    assert!(!value.is_math());
}
