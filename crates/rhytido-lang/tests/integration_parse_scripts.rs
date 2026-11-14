// SPDX-License-Identifier: Apache-2.0
//! Integration tests for `parse_script`: whole scripts, written from the
//! behaviour the public examples demonstrate, parsed end to end and asserted as
//! AST shape.
//!
//! These are cross-module by construction — the grammar (`grammar.lalrpop`), the
//! syntax tree (`ast.rs`), the core interner (`rhytido_core::expr`) and the
//! per-session symbol table (`symbol.rs`) all have to agree for an assertion
//! here to hold. Two contracts are checked over and above the individual
//! shapes:
//!
//! * **Mathematical fidelity.** A statement whose expression mentions no name
//!   must intern to *exactly* the node `parse_expr` produces for the same text
//!   from the same interner (the fusion rule of
//!   `docs/research/sollya-language-b2.md` §10). That is what makes M3-B2 an
//!   extension of M3-B1 rather than a second mathematics parser.
//! * **Names survive parsing.** Where the mathematics cannot express a
//!   construct — `n + 1` with a formal parameter `n` — the name must still be in
//!   the tree, readable back through the script's own symbol table.
//!
//! The scenarios paraphrase documented *behaviour* (procedure definition and
//! application, the two assignment operators, silent settings assignment,
//! printing, blocks and shadowing, lists of procedures, higher-order
//! procedures). No wording or production of the reference implementation is
//! reproduced, and nothing here evaluates a script: interpretation is M3-C/D.

use std::rc::Rc;
use std::str::FromStr;

use rhytido_backend::BigFloat;
use rhytido_core::{BaseFn, Expr, Interner, NodeId};
use rhytido_lang::ast::{
    AssignOp, BinaryOp, Formal, MessagePolicy, Proc, ProgramExpr, Redirect, Script, Stmt, UnaryOp,
};
use rhytido_lang::{SymbolId, Value, parse_expr, parse_script};

// ---- Shared helpers -------------------------------------------------------

/// Parses `src` into a fresh interner, panicking with the error text on failure.
fn ok(src: &str) -> (Interner, Script) {
    let mut it = Interner::new();
    match parse_script(src, &mut it) {
        Ok(script) => (it, script),
        Err(e) => panic!("expected `{src}` to parse, got {e:?}"),
    }
}

/// Parses `src` and hands back its single statement plus the script that
/// produced it, asserting that there is exactly one statement.
fn one(src: &str) -> (Interner, Script, Stmt) {
    let (it, script) = ok(src);
    assert_eq!(
        script.statements().len(),
        1,
        "`{src}` must be exactly one statement"
    );
    let statement = script.statements()[0].clone();
    (it, script, statement)
}

/// Borrows the four fields of an assignment, panicking with the node's shape
/// otherwise; every assignment scenario reads its statement back this way.
fn assign(stmt: &Stmt) -> (SymbolId, AssignOp, ProgramExpr, MessagePolicy) {
    match stmt {
        Stmt::Assign {
            target,
            op,
            value,
            messages,
        } => (*target, *op, value.clone(), *messages),
        other => panic!("expected an assignment, got {other:?}"),
    }
}

/// The procedure literal assigned by `name = proc(...) { ... };`.
fn assigned_proc(stmt: &Stmt) -> Proc {
    match assign(stmt).2 {
        ProgramExpr::ProcLit(proc) => (*proc).clone(),
        other => panic!("expected an assigned procedure literal, got {other:?}"),
    }
}

/// Wraps a hand-built procedure as a program expression.
fn proc_lit(proc: Proc) -> ProgramExpr {
    ProgramExpr::proc_lit(Rc::new(proc))
}

/// Reads back a name from the script's own table.
///
/// The table offers `intern` (which inserts) and `text` (which reads), so
/// finding an id by spelling is a scan over the ids the parse actually produced
/// — which doubles as the assertion that the parse interned only what it used.
fn sym(script: &Script, name: &str) -> SymbolId {
    (0..script.symbols().len() as u32)
        .map(SymbolId::new)
        .find(|&id| script.name_of(id) == Some(name))
        .unwrap_or_else(|| panic!("`{name}` was never interned by this parse"))
}

/// Asserts that `script` interned exactly `names`, in first-use order.
fn assert_names(script: &Script, names: &[&str]) {
    let got: Vec<&str> = (0..script.symbols().len() as u32)
        .map(SymbolId::new)
        .filter_map(|id| script.name_of(id))
        .collect();
    assert_eq!(got, names, "interned names");
}

fn cst(it: &mut Interner, s: &str) -> NodeId {
    it.intern(Expr::Constant(
        BigFloat::from_str(s).expect("test literal must be a valid decimal"),
    ))
}

// ---- Scenario 1: the successor procedure ----------------------------------
//
// `succ = proc(n) { return n + 1; }; succ(5); 3 + succ(0);` — the canonical
// procedure: define by assignment, apply, apply inside a larger sum.

#[test]
fn integration_proc_definition_keeps_parameter_and_body_shape() {
    let (mut it, script, stmt) = one("succ = proc(n) { return n + 1; };");
    let succ = sym(&script, "succ");
    let n = sym(&script, "n");

    let expected = Stmt::assign(
        succ,
        AssignOp::Plain,
        proc_lit(Proc::new(
            [Formal::single(n)],
            [Stmt::returning(ProgramExpr::binary(
                BinaryOp::Add,
                ProgramExpr::var(n),
                ProgramExpr::math(cst(&mut it, "1")),
            ))],
        )),
        MessagePolicy::Report,
    );
    assert_eq!(stmt, expected);
    assert_names(&script, &["succ", "n"]);

    // The name is why this is not a core node: core's variable carries no
    // spelling, so `n + 1` cannot be interned.
    let body = assigned_proc(&stmt);
    let Stmt::Return(Some(value)) = &body.body()[0] else {
        panic!("the body statement is a return");
    };
    let ProgramExpr::Binary(op, lhs, rhs) = value else {
        panic!("the returned expression is a binary node, got {value:?}");
    };
    assert_eq!(*op, BinaryOp::Add);
    assert_eq!(lhs.as_var(), Some(n));
    assert_eq!(rhs.as_math(), Some(cst(&mut it, "1")));
}

#[test]
fn integration_proc_application_is_a_named_call() {
    let (_it, script) = ok("succ = proc(n) { return n + 1; };\nsucc(5);\n3 + succ(0);\n");
    assert_eq!(script.statements().len(), 3);
    let succ = sym(&script, "succ");

    let Stmt::ExprStmt(ProgramExpr::Call { callee, args }) = &script.statements()[1] else {
        panic!(
            "the second statement is an application, got {:?}",
            script.statements()[1]
        )
    };
    assert_eq!(*callee, succ);
    assert_eq!(args.len(), 1);
    assert!(args[0].is_math(), "the actual argument is mathematics");

    // `3 + succ(0)` mixes a constant with a call, so the addition stays at
    // program level with both operands intact.
    let Stmt::ExprStmt(ProgramExpr::Binary(op, lhs, rhs)) = &script.statements()[2] else {
        panic!("the third statement is a binary expression");
    };
    assert_eq!(*op, BinaryOp::Add);
    assert!(lhs.is_math());
    assert!(rhs.as_call().is_some());
}

// ---- Scenario 2: local declarations and the two assignment operators ------
//
// `add = proc(m,n) { var res; res := m + n; return res; };` — declare,
// evaluate-and-assign, return what was assigned.

#[test]
fn integration_var_then_evaluating_assignment_then_return() {
    let (_it, script, stmt) = one("add = proc(m,n) { var res; res := m + n; return res; };");
    let body = assigned_proc(&stmt);

    assert_eq!(
        body.formals(),
        [
            Formal::single(sym(&script, "m")),
            Formal::single(sym(&script, "n"))
        ]
    );
    assert_eq!(body.arity(), Some(2));
    assert_eq!(body.body().len(), 3);

    // Declaration first, exactly as the block grammar requires...
    assert_eq!(
        body.body()[0],
        Stmt::declare([sym(&script, "res")]),
        "the `var` statement leads the body"
    );
    // ...then the evaluating assignment...
    let (target, op, value, messages) = assign(&body.body()[1]);
    assert_eq!(target, sym(&script, "res"));
    assert_eq!(op, AssignOp::Evaluated, "`:=` is the evaluating operator");
    assert_eq!(messages, MessagePolicy::Report);
    assert!(
        matches!(value, ProgramExpr::Binary(BinaryOp::Add, ..)),
        "`m + n` has two names, so it stays at program level"
    );
    // ...and the return closes the body.
    assert!(body.body()[2].is_return());
    assert_eq!(
        body.returned(),
        body.body()[2].as_return(),
        "`returned` reports the trailing return's value"
    );
    assert!(body.returned().is_some_and(|value| value.is_some()));
}

#[test]
fn integration_plain_and_evaluating_assignment_are_distinguishable() {
    // The two operators differ only in whether the right-hand side is evaluated,
    // so the tree has to keep them apart or the evaluator cannot implement either.
    let (_it, _script, plain) = one("i = i + 1;");
    let (_it, _script, eager) = one("i := i + 1;");
    assert_eq!(assign(&plain).1, AssignOp::Plain);
    assert_eq!(assign(&eager).1, AssignOp::Evaluated);
    // Same target, same value; only the operator differs.
    assert_eq!(assign(&plain).0, assign(&eager).0);
    assert_eq!(assign(&plain).2, assign(&eager).2);
}

// ---- Scenario 3: settings are assignments ---------------------------------
//
// `prec = 30;` announces itself, `prec = 30!;` is the silent form. Both are
// ordinary assignments to a global name; what a setting *does* is the
// evaluator's business.

#[test]
fn integration_settings_assignment_records_the_silent_marker() {
    let (_it, script) = ok("prec=30;\nprec=30!;\nverbosity = 1!;\n");
    assert_eq!(script.statements().len(), 3);
    assert_eq!(assign(&script.statements()[0]).3, MessagePolicy::Report);
    assert!(!assign(&script.statements()[0]).3.is_suppressing());
    assert_eq!(
        assign(&script.statements()[1]).3,
        MessagePolicy::Suppress,
        "a trailing `!` silences the assignment"
    );
    assert!(assign(&script.statements()[1]).3.is_suppressing());
    assert!(assign(&script.statements()[2]).3.is_suppressing());
    // `prec` is interned once for both statements.
    assert_names(&script, &["prec", "verbosity"]);
}

#[test]
fn integration_settings_read_as_names_not_reserved_words() {
    // `display = decimal;` assigns a *name*: only the syntactic keywords are
    // reserved, so the particular values are variable references rather than a
    // fresh node kind per value.
    let (_it, script, stmt) = one("display = decimal;");
    let expected = Stmt::assign(
        sym(&script, "display"),
        AssignOp::Plain,
        ProgramExpr::var(sym(&script, "decimal")),
        MessagePolicy::Report,
    );
    assert_eq!(stmt, expected);
}

#[test]
fn integration_bare_setting_read_is_an_expression_statement() {
    // Reading a setting is just a name used as a statement (`verbosity;`).
    let (_it, script, stmt) = one("verbosity;");
    assert_eq!(
        stmt,
        Stmt::expr_stmt(ProgramExpr::var(sym(&script, "verbosity")))
    );
}

// ---- Scenario 4: printing -------------------------------------------------

#[test]
fn integration_print_takes_a_sequence_of_any_expressions() {
    let (_it, script, stmt) = one(r#"print("Hello", "you", 4 + 3, "other persons.");"#);
    let (args, redirect) = stmt.as_print().expect("a print statement");
    assert_eq!(args.len(), 4);
    assert_eq!(args[0].as_string(), Some("Hello"));
    assert_eq!(args[1].as_string(), Some("you"));
    assert!(args[2].is_math(), "`4 + 3` mentions no name, so it interns");
    assert_eq!(args[3].as_string(), Some("other persons."));
    assert_eq!(redirect, None, "no redirection was written");
    assert_names(&script, &[]);
}

#[test]
fn integration_print_can_target_a_file_for_creation_or_appending() {
    let (_it, _script, stmt) = one(r#"print(_x_ + 2) > "foo.sol";"#);
    let (_, redirect) = stmt.as_print().expect("a print statement");
    assert_eq!(
        redirect,
        Some(&Redirect::create("foo.sol")),
        "`>` creates or overwrites"
    );
    assert!(!redirect.expect("a redirect").is_append());

    let (_it, _script, stmt) = one(r#"print(_x_ + 2) >> "foo.sol";"#);
    let (_, redirect) = stmt.as_print().expect("a print statement");
    assert_eq!(redirect, Some(&Redirect::append("foo.sol")));
    assert!(redirect.expect("a redirect").is_append());
    assert_eq!(redirect.expect("a redirect").file(), "foo.sol");
}

#[test]
fn integration_print_escapes_are_resolved_at_parse_time() {
    // The documentation states escapes are interpreted when the string is
    // defined, not when it is printed, so the tree holds the characters.
    let (_it, _script, stmt) = one(r#"print("Hello\tyou\n");"#);
    let (args, _) = stmt.as_print().expect("a print statement");
    assert_eq!(args[0].as_string(), Some("Hello\tyou\n"));

    let (_it, _script, stmt) = one(r#"print("65 is \x41, octal \101, quote \"");"#);
    let (args, _) = stmt.as_print().expect("a print statement");
    assert_eq!(args[0].as_string(), Some("65 is A, octal A, quote \""));
}

// ---- Scenario 5: blocks, scopes and shadowing -----------------------------
//
// A block is one statement; declarations lead it; a nested block re-declares.
// Which binding a lookup finds is M3-C — this asserts only the nesting.

#[test]
fn integration_nested_blocks_keep_declarations_and_statements_apart() {
    let (_it, script, stmt) =
        one("{var a, b; a = 5; b = 3; {var a; var b; b = true; a = 1; a; b;}; a; b;};");
    let block = stmt.as_block().expect("a block statement");
    // 1 declaration + 2 assignments + 1 nested block + 2 expression statements.
    assert_eq!(block.len(), 6, "block contents: {block:?}");
    assert_eq!(
        block[0],
        Stmt::declare([sym(&script, "a"), sym(&script, "b")]),
        "declarations lead the block"
    );
    let inner = block[3].as_block().expect("the nested block");
    assert_eq!(inner.len(), 6);
    assert_eq!(
        inner[0],
        Stmt::declare([sym(&script, "a")]),
        "the inner block re-declares `a`, which is what makes it a shadow"
    );
    assert_eq!(inner[1], Stmt::declare([sym(&script, "b")]));
    // Only two names exist in the table however often they are declared.
    assert_names(&script, &["a", "b", "true"]);
    // `b = true;` assigns the *name* `true`: booleans are values, not syntax.
    assert_eq!(assign(&inner[2]).2.as_var(), Some(sym(&script, "true")));
    // The trailing `a; b;` are expression statements, i.e. value uses.
    assert_eq!(
        inner[4],
        Stmt::expr_stmt(ProgramExpr::var(sym(&script, "a")))
    );
}

#[test]
fn integration_empty_block_and_empty_script_are_statements() {
    let (_it, script) = ok("");
    assert!(script.is_empty(), "a script with no statements parses");
    assert_eq!(script.len(), 0);

    let (_it, _script, stmt) = one("{};");
    assert_eq!(stmt, Stmt::block([]), "an empty block is still a block");
    assert_eq!(stmt.as_block().map(<[Stmt]>::len), Some(0));
}

#[test]
fn integration_long_statement_sequence_keeps_every_statement_in_order() {
    // 200 statements of the same shape, then one different one at the end.
    let mut source = String::new();
    for index in 0..200 {
        source.push_str(&format!("acc = acc + {index};\n"));
    }
    source.push_str("print(acc);\n");
    let (_it, script) = ok(&source);
    assert_eq!(script.statements().len(), 201);
    assert!(script.statements()[199].as_print().is_none());
    assert!(script.statements()[200].as_print().is_some());
    assert_names(&script, &["acc"]);
}

// ---- Scenario 6: procedures as first-class objects ------------------------

#[test]
fn integration_list_of_procedure_literals() {
    let (_it, _script, stmt) =
        one("myprocs = [| proc(m,n) { return m + n; }, proc(m,n) { return m - n; } |];");
    let value = assign(&stmt).2;
    let elements = value.as_list().expect("a list literal");
    assert_eq!(elements.len(), 2);
    let first = elements[0].as_proc().expect("a procedure literal");
    let second = elements[1].as_proc().expect("a procedure literal");
    assert_eq!(first.formals(), second.formals());
    assert_ne!(
        first.body(),
        second.body(),
        "the two bodies differ in their operator"
    );
    assert!(
        first
            .returned()
            .flatten()
            .is_some_and(|value| !value.is_math()),
        "neither body interns: both mention names"
    );
}

#[test]
fn integration_higher_order_procedure_applies_its_parameter() {
    // `applier = proc(p,n) { return p(n); };` — applying a procedure that
    // arrived as an argument is a call whose callee is a name.
    let (_it, script, stmt) = one("applier = proc(p,n) { return p(n); };");
    let body = assigned_proc(&stmt);
    let Stmt::Return(Some(value)) = &body.body()[0] else {
        panic!("the body returns");
    };
    let (callee, args) = value.as_call().expect("an application of a name");
    assert_eq!(callee, sym(&script, "p"));
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].as_var(), Some(sym(&script, "n")));
}

#[test]
fn integration_variadic_formal_is_the_list_shape() {
    // `sumall = proc(L = ...) { var acc, i; acc = 0; return acc; };` — the
    // variadic parameter is a distinct shape, not an arity plus a flag.
    let (_it, script, stmt) = one("sumall = proc(L = ...) { var acc, i; acc = 0; return acc; };");
    let body = assigned_proc(&stmt);
    assert_eq!(body.formals(), [Formal::list(sym(&script, "L"))]);
    assert!(body.is_variadic());
    assert_eq!(
        body.arity(),
        None,
        "a variadic procedure is applied to any number of arguments"
    );
    assert_eq!(
        body.body()[0],
        Stmt::declare([sym(&script, "acc"), sym(&script, "i")]),
        "one statement declares both names"
    );
    assert!(
        assign(&body.body()[1]).2.is_math(),
        "`acc = 0` has identifier-free mathematics on the right, so it interns"
    );
}

#[test]
fn integration_zero_parameter_procedure_and_nullary_call() {
    let (_it, script, stmt) = one(r#"hey = proc() { print("Hello world."); };"#);
    let body = assigned_proc(&stmt);
    assert!(body.formals().is_empty());
    assert_eq!(body.arity(), Some(0));
    assert_eq!(
        body.returned(),
        None,
        "no `return` was written, and the parser invents none"
    );
    assert_eq!(body.body().len(), 1);
    let (args, redirect) = body.body()[0].as_print().expect("the body prints");
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].as_string(), Some("Hello world."));
    assert_eq!(redirect, None);
    assert_names(&script, &["hey"]);

    let (_it, script, stmt) = one("hey();");
    let (callee, args) = stmt
        .as_expr()
        .and_then(ProgramExpr::as_call)
        .expect("a nullary application");
    assert_eq!(callee, sym(&script, "hey"));
    assert!(args.is_empty());
}

#[test]
fn integration_procedure_returning_a_procedure() {
    // `makeadd = proc(n) { return proc(m,k) { return m + k; }; };` — a procedure
    // value nested inside another procedure's return.
    let (_it, script, stmt) = one("makeadd = proc(n) { return proc(m,k) { return m + k; }; };");
    let outer = assigned_proc(&stmt);
    let Stmt::Return(Some(value)) = &outer.body()[0] else {
        panic!("the outer body returns");
    };
    let inner = value.as_proc().expect("a nested procedure literal");
    assert_eq!(
        inner.formals(),
        [
            Formal::single(sym(&script, "m")),
            Formal::single(sym(&script, "k"))
        ]
    );
    assert_eq!(outer.formals(), [Formal::single(sym(&script, "n"))]);
}

// ---- Scenario 7: `procedure` is its own statement -------------------------

#[test]
fn integration_procedure_keyword_defines_and_names() {
    let (_it, script, stmt) = one("procedure succ(n) { return n + 1; };");
    let Stmt::Procedure { name, proc } = &stmt else {
        panic!("the statement is a procedure definition, got {stmt:?}");
    };
    assert_eq!(*name, sym(&script, "succ"));
    assert_eq!(proc.formals(), [Formal::single(sym(&script, "n"))]);
    assert!(proc.body()[0].is_return());
    // It is *not* silently rewritten into an assignment: that equivalence is an
    // evaluator decision, not a parsing one.
    assert!(!matches!(stmt, Stmt::Assign { .. }));
}

#[test]
fn integration_procedure_keyword_with_variadic_formals() {
    let (_it, script, stmt) = one("procedure myprint(L = ...) { var i; };");
    let Stmt::Procedure { name, proc } = &stmt else {
        panic!("the statement is a procedure definition");
    };
    assert_eq!(*name, sym(&script, "myprint"));
    assert_eq!(proc.formals(), [Formal::list(sym(&script, "L"))]);
    assert_eq!(proc.body()[0], Stmt::declare([sym(&script, "i")]));
}

// ---- Scenario 8: mathematical fidelity across the two entry points --------

#[test]
fn integration_identifier_free_expressions_intern_identically() {
    // The load-bearing regression guard for M3-B1: for identifier-free text the
    // statement layer must produce the very node the expression entry point
    // produces, from the same interner, and must add no node of its own.
    let cases = [
        "2 + 3",
        "1 + 2 * 3",
        "2 / 3 / 4",
        "2^3^4",
        "-2^3",
        "-2*3",
        "1 - (2 - 3)",
        "sqrt(2)^2 + pi/3",
        "sqrt(1 + 2)",
        "exp(sin(_x_))",
        "1 + 2 + 3 + 4 + 5",
        "NaN",
        "1e-300",
        "(((((1)))))",
    ];
    for src in cases {
        let mut it = Interner::new();
        let expected = parse_expr(src, &mut it)
            .unwrap_or_else(|e| panic!("`{src}` must parse as maths, got {e:?}"));
        let nodes_after_expr = it.len();
        let statement = format!("x = {src};");
        let script = parse_script(&statement, &mut it)
            .unwrap_or_else(|e| panic!("`{statement}` must parse, got {e:?}"));
        let got = assign(&script.statements()[0])
            .2
            .as_math()
            .unwrap_or_else(|| panic!("`{src}` inside a script must stay mathematical"));
        assert_eq!(
            got, expected,
            "`{src}` must intern to the same node through both entry points"
        );
        assert_eq!(
            it.len(),
            nodes_after_expr,
            "`{src}` via the statement layer must create no extra node"
        );
    }
}

#[test]
fn integration_repeated_mathematics_in_a_script_shares_nodes() {
    let (mut it, script) = ok("a = sqrt(2) ^ 2;\nb = sqrt(2) ^ 2;\n");
    let first = assign(&script.statements()[0]).2;
    let second = assign(&script.statements()[1]).2;
    assert_eq!(first, second, "identical mathematics shares one node");
    let before = it.len();
    let again = parse_expr("sqrt(2) ^ 2", &mut it).expect("maths");
    assert_eq!(it.len(), before, "a repeat parse adds no node");
    assert_eq!(again, first.as_math().expect("mathematics"));
}

#[test]
fn integration_base_function_of_a_name_stays_at_program_level() {
    // `sqrt(x)` inside a procedure body: a base function applied to a name
    // cannot be interned, so the application node lives here and the name
    // survives.
    let (_it, script, stmt) = one("f = proc(x) { return sqrt(x); };");
    let body = assigned_proc(&stmt);
    let Stmt::Return(Some(value)) = &body.body()[0] else {
        panic!("the body returns");
    };
    let ProgramExpr::Unary(UnaryOp::Apply(BaseFn::Sqrt), inner) = value else {
        panic!("a base function applied to a name, got {value:?}");
    };
    assert_eq!(inner.as_var(), Some(sym(&script, "x")));
}

// ---- Scenario 9: a whole script, end to end -------------------------------

#[test]
fn integration_multi_statement_script_in_source_order() {
    let source = "\
autosimplify = off;
i = 1000;
i = i + 1;
print(i);
i := i + 1;
print(i);
L = [|1,2,3|];
print(L);
";
    let (_it, script) = ok(source);
    let kinds: Vec<&str> = script
        .statements()
        .iter()
        .map(|stmt| match stmt {
            Stmt::Assign { .. } => "assign",
            Stmt::Declare(_) => "declare",
            Stmt::Print { .. } => "print",
            Stmt::Block(_) => "block",
            Stmt::Procedure { .. } => "procedure",
            Stmt::ExprStmt(_) => "expr",
            Stmt::Return(_) => "return",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "assign", "assign", "assign", "print", "assign", "print", "assign", "print"
        ]
    );
    // `print` is a keyword, never a name; the rest is interned on first use.
    assert_names(&script, &["autosimplify", "off", "i", "L"]);
    let list = assign(&script.statements()[6]).2;
    assert_eq!(list.as_list().map(<[ProgramExpr]>::len), Some(3));
}

#[test]
fn integration_a_parsed_procedure_is_what_the_value_wraps() {
    // The AST half of `Value::Proc`: a parsed procedure literal is exactly what
    // the composite value wraps, so parse and representation agree (running it
    // is M3-D).
    let (_it, _script, stmt) = one("succ = proc(n) { return n + 1; };");
    let parsed = assigned_proc(&stmt);
    let value = Value::proc(parsed.clone());
    assert_eq!(value.as_proc(), Some(&parsed));
    assert!(value.is_proc());
    assert_eq!(
        value,
        Value::proc(Proc::new(parsed.formals().to_vec(), parsed.body().to_vec())),
        "two independently built procedure values with equal bodies are equal"
    );
}
