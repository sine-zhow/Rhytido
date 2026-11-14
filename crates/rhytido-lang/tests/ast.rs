// SPDX-License-Identifier: Apache-2.0
//! Module tests for the `rhytido_lang::ast` public API.
//!
//! These tests the syntax tree as *representation*, from outside the crate:
//! construction, views, structural equality and hashing, sharing, and the
//! operator-fusion rule that decides whether a node lands in the core interner
//! or stays at program level. Four case classes as ever — normal, extreme,
//! error, and interruption/robustness.
//!
//! Parsing is deliberately absent: a parser is one way to *fill* these types,
//! and `tests/parse.rs` plus `tests/integration_parse_scripts.rs` cover that.
//! Keeping the two separate is what lets a change in the grammar be told apart
//! from a change in the tree.

use std::rc::Rc;

use rhytido_backend::BigFloat;
use rhytido_core::{BaseFn, Expr, Interner, NodeId};
use rhytido_lang::Value;
use rhytido_lang::ast::{
    AssignOp, BinaryOp, Formal, MessagePolicy, Proc, ProgramExpr, Redirect, Script, Stmt, UnaryOp,
};
use rhytido_lang::{SymbolId, SymbolTable};

// ---- Shared helpers -------------------------------------------------------

fn name(table: &mut SymbolTable, text: &str) -> SymbolId {
    table.intern(text)
}

fn constant(it: &mut Interner, value: i64) -> NodeId {
    it.intern(Expr::Constant(BigFloat::from_i64(value)))
}

/// A one-statement procedure used by several equality scenarios.
fn successor(n: SymbolId, one: NodeId) -> Proc {
    Proc::new(
        [Formal::single(n)],
        [Stmt::returning(ProgramExpr::binary(
            BinaryOp::Add,
            ProgramExpr::var(n),
            ProgramExpr::math(one),
        ))],
    )
}

fn hash_of<T: std::hash::Hash>(value: &T) -> u64 {
    use std::hash::Hasher;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

// ---- Normal: construction and views round-trip ----------------------------

#[test]
fn normal_assign_keeps_all_four_parts() {
    let mut table = SymbolTable::new();
    let target = name(&mut table, "i");
    let mut it = Interner::new();
    let value = ProgramExpr::math(constant(&mut it, 1000));
    let stmt = Stmt::assign(
        target,
        AssignOp::Evaluated,
        value.clone(),
        MessagePolicy::Suppress,
    );
    match stmt {
        Stmt::Assign {
            target: got_target,
            op,
            value: got_value,
            messages,
        } => {
            assert_eq!(got_target, target);
            assert_eq!(op, AssignOp::Evaluated);
            assert_eq!(got_value, value);
            assert_eq!(messages, MessagePolicy::Suppress);
        }
        other => panic!("an assignment was built, got {other:?}"),
    }
}

#[test]
fn normal_message_policy_defaults_to_report_and_suppress_is_queryable() {
    assert_eq!(MessagePolicy::default(), MessagePolicy::Report);
    assert!(!MessagePolicy::Report.is_suppressing());
    assert!(MessagePolicy::Suppress.is_suppressing());
}

#[test]
fn normal_declare_round_trips_its_names() {
    let mut table = SymbolTable::new();
    let a = name(&mut table, "a");
    let b = name(&mut table, "b");
    let stmt = Stmt::declare([a, b]);
    assert_eq!(stmt.as_declare(), Some(&[a, b][..]));
    assert_eq!(stmt.as_block(), None, "a declaration is not a block");
    // Repeated names are kept as written: de-duplicating them would be a
    // decision about the language, not about representation.
    let twice = Stmt::declare([a, a]);
    assert_eq!(twice.as_declare().map(<[SymbolId]>::len), Some(2));
}

#[test]
fn normal_print_views_report_arguments_and_redirect() {
    let stmt = Stmt::print([ProgramExpr::string_lit("Hello")]);
    let (args, redirect) = stmt.as_print().expect("a print statement");
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].as_string(), Some("Hello"));
    assert_eq!(redirect, None);

    let to_file = Stmt::print_to([ProgramExpr::string_lit("x")], Redirect::append("out.sol"));
    let (_, redirect) = to_file.as_print().expect("a print statement");
    let redirect = redirect.expect("a redirect");
    assert_eq!(redirect.file(), "out.sol");
    assert_eq!(redirect.mode(), rhytido_lang::ast::FileMode::Append);
    assert!(redirect.is_append());
}

#[test]
fn normal_redirect_distinguishes_create_from_append() {
    let create = Redirect::create("f.sol");
    let append = Redirect::append("f.sol");
    assert_eq!(create.file(), append.file());
    assert_ne!(create, append, "the mode is part of the value");
    assert!(!create.is_append());
    assert_eq!(
        Redirect::new("f.sol", rhytido_lang::ast::FileMode::Create),
        create
    );
}

#[test]
fn normal_block_is_a_statement_and_nests() {
    let inner = Stmt::block([Stmt::declare([])]);
    let outer = Stmt::block([inner.clone()]);
    assert_eq!(outer.as_block().map(<[Stmt]>::len), Some(1));
    assert_eq!(outer.as_block(), Some(&[inner][..]));
    assert_eq!(Stmt::block([]).as_block().map(<[Stmt]>::len), Some(0));
}

#[test]
fn normal_expr_stmt_borrows_its_expression() {
    let expr = ProgramExpr::string_lit("hi");
    let stmt = Stmt::expr_stmt(expr.clone());
    assert_eq!(stmt.as_expr(), Some(&expr));
    assert_eq!(stmt.as_return(), None);
    assert!(!stmt.is_return());
}

#[test]
fn normal_return_views_separate_bare_from_valued() {
    let mut it = Interner::new();
    let valued = Stmt::returning(ProgramExpr::math(constant(&mut it, 1)));
    assert_eq!(valued.as_return().map(|inner| inner.is_some()), Some(true));
    assert!(valued.is_return());
    let bare = Stmt::returning(None);
    assert_eq!(bare.as_return(), Some(None));
    assert!(bare.is_return());
}

#[test]
fn normal_formal_name_accessor_covers_both_shapes() {
    let mut table = SymbolTable::new();
    let x = name(&mut table, "x");
    let single = Formal::single(x);
    let list = Formal::list(x);
    assert_eq!(single.name(), x);
    assert_eq!(list.name(), x, "the same name, a different binding shape");
    assert!(!single.is_list());
    assert!(list.is_list());
    assert_ne!(single, list);
}

#[test]
fn normal_proc_views_report_formals_body_and_return() {
    let mut table = SymbolTable::new();
    let n = name(&mut table, "n");
    let mut it = Interner::new();
    let one = constant(&mut it, 1);
    let proc = successor(n, one);
    assert_eq!(proc.formals(), [Formal::single(n)]);
    assert_eq!(proc.body().len(), 1);
    assert_eq!(proc.arity(), Some(1));
    assert!(!proc.is_variadic());
    assert!(matches!(proc.returned(), Some(Some(_))));
}

#[test]
fn normal_proc_without_return_or_formals_reports_both_absent() {
    let proc = Proc::new([], [Stmt::print([])]);
    assert!(proc.formals().is_empty());
    assert_eq!(proc.arity(), Some(0));
    assert_eq!(proc.returned(), None, "the body does not end with a return");
    assert!(!proc.is_variadic());
}

#[test]
fn normal_proc_variadic_has_no_checked_arity() {
    let mut table = SymbolTable::new();
    let l = name(&mut table, "L");
    let proc = Proc::new([Formal::list(l)], []);
    assert!(proc.is_variadic());
    assert_eq!(proc.arity(), None);
    assert_eq!(proc.returned(), None);
    assert!(proc.body().is_empty());
}

#[test]
fn normal_proc_returned_borrows_the_same_expression_as_the_body() {
    let mut table = SymbolTable::new();
    let n = name(&mut table, "n");
    let mut it = Interner::new();
    let proc = successor(n, constant(&mut it, 1));
    let from_body = proc.body()[0].as_return();
    assert_eq!(proc.returned(), from_body);
    // Same node, not a copy: the view borrows the body rather than rebuilding it.
    let outer = std::ptr::from_ref(from_body.flatten().expect("a value"));
    let inner = std::ptr::from_ref(proc.returned().flatten().expect("a value"));
    assert!(std::ptr::eq(outer, inner));
}

#[test]
fn normal_program_expr_accessors_pick_their_own_variant() {
    let mut table = SymbolTable::new();
    let x = name(&mut table, "x");
    let mut it = Interner::new();
    let math = ProgramExpr::math(constant(&mut it, 2));
    assert_eq!(math.as_math(), Some(constant(&mut it, 2)));
    assert_eq!(math.as_var(), None);
    assert!(math.is_math());

    let var = ProgramExpr::var(x);
    assert_eq!(var.as_var(), Some(x));
    assert_eq!(var.as_math(), None);
    assert!(!var.is_math());

    let call = ProgramExpr::call(x, [var.clone(), math.clone()]);
    let (callee, args) = call.as_call().expect("a call");
    assert_eq!(callee, x);
    assert_eq!(args.len(), 2);
    assert_eq!(call.as_string(), None);

    let list = ProgramExpr::list_lit([call.clone(), math.clone()]);
    assert_eq!(list.as_list().map(<[ProgramExpr]>::len), Some(2));
    assert_eq!(list.as_call(), None);

    let string = ProgramExpr::string_lit("text");
    assert_eq!(string.as_string(), Some("text"));

    let proc = ProgramExpr::proc_lit(Rc::new(Proc::new([], [])));
    assert_eq!(proc.as_proc().map(Proc::arity), Some(Some(0)));
}

#[test]
fn normal_binary_and_unary_constructors_keep_operands() {
    let lhs = ProgramExpr::string_lit("a");
    let rhs = ProgramExpr::string_lit("b");
    let binary = ProgramExpr::binary(BinaryOp::Mul, lhs.clone(), rhs.clone());
    match &binary {
        ProgramExpr::Binary(op, l, r) => {
            assert_eq!(*op, BinaryOp::Mul);
            assert_eq!(**l, lhs);
            assert_eq!(**r, rhs);
        }
        other => panic!("a binary node was built, got {other:?}"),
    }
    let unary = ProgramExpr::unary(UnaryOp::Neg, lhs.clone());
    match &unary {
        ProgramExpr::Unary(op, operand) => {
            assert_eq!(*op, UnaryOp::Neg);
            assert_eq!(**operand, lhs);
        }
        other => panic!("a unary node was built, got {other:?}"),
    }
}

#[test]
fn normal_ops_build_the_matching_core_nodes() {
    let mut it = Interner::new();
    let a = constant(&mut it, 2);
    let b = constant(&mut it, 3);
    let expected = [
        (BinaryOp::Add.into_expr(a, b), it.intern(Expr::Add(a, b))),
        (BinaryOp::Sub.into_expr(a, b), it.intern(Expr::Sub(a, b))),
        (BinaryOp::Mul.into_expr(a, b), it.intern(Expr::Mul(a, b))),
        (BinaryOp::Div.into_expr(a, b), it.intern(Expr::Div(a, b))),
        (BinaryOp::Pow.into_expr(a, b), it.intern(Expr::Pow(a, b))),
    ];
    for (built, want) in expected {
        assert_eq!(it.intern(built), want);
    }
    assert_eq!(
        it.intern(UnaryOp::Neg.into_expr(a)),
        it.intern(Expr::Neg(a))
    );
    assert_eq!(
        it.intern(UnaryOp::Apply(BaseFn::Sqrt).into_expr(a)),
        it.intern(Expr::Apply(BaseFn::Sqrt, a))
    );
}

#[test]
fn normal_fuse_interns_two_mathematical_operands() {
    let mut it = Interner::new();
    let a = constant(&mut it, 2);
    let b = constant(&mut it, 3);
    let fused = BinaryOp::Add.fuse(ProgramExpr::math(a), ProgramExpr::math(b), &mut it);
    assert_eq!(fused.as_math(), Some(it.intern(Expr::Add(a, b))));
    // Interning the same structure again reuses the node: fusion goes through
    // the interner, not around it.
    let again = BinaryOp::Add.fuse(ProgramExpr::math(a), ProgramExpr::math(b), &mut it);
    assert_eq!(fused, again);
}

#[test]
fn normal_fuse_keeps_a_named_operand_at_program_level() {
    let mut table = SymbolTable::new();
    let n = name(&mut table, "n");
    let mut it = Interner::new();
    let b = constant(&mut it, 1);
    let fused = BinaryOp::Add.fuse(ProgramExpr::var(n), ProgramExpr::math(b), &mut it);
    assert_eq!(fused.as_math(), None);
    assert!(matches!(fused, ProgramExpr::Binary(BinaryOp::Add, ..)));

    // Symmetrically on the right, and for both unary shapes.
    let flipped = BinaryOp::Add.fuse(ProgramExpr::math(b), ProgramExpr::var(n), &mut it);
    assert!(matches!(flipped, ProgramExpr::Binary(..)));
    let negated = UnaryOp::Neg.fuse(ProgramExpr::var(n), &mut it);
    assert!(matches!(negated, ProgramExpr::Unary(UnaryOp::Neg, ..)));
    let applied = UnaryOp::Apply(BaseFn::Exp).fuse(ProgramExpr::var(n), &mut it);
    assert!(matches!(
        applied,
        ProgramExpr::Unary(UnaryOp::Apply(BaseFn::Exp), ..)
    ));
    // A mathematical operand still interns through the unary path.
    let pure = UnaryOp::Apply(BaseFn::Exp).fuse(ProgramExpr::math(b), &mut it);
    assert!(pure.is_math());
    assert_eq!(pure.as_math(), Some(it.intern(Expr::Apply(BaseFn::Exp, b))));
}

#[test]
fn normal_script_carries_its_statements_and_its_names() {
    let mut table = SymbolTable::new();
    let x = name(&mut table, "x");
    let script = Script::new([Stmt::expr_stmt(ProgramExpr::var(x))], table);
    assert_eq!(script.len(), 1);
    assert!(!script.is_empty());
    assert_eq!(script.name_of(x), Some("x"));
    assert_eq!(script.statements().len(), 1);
    assert_eq!(script.symbols().len(), 1);
}

#[test]
fn normal_empty_script_is_representable() {
    let script = Script::new([], SymbolTable::new());
    assert!(script.is_empty());
    assert_eq!(script.len(), 0);
    assert!(script.statements().is_empty());
    assert!(script.symbols().is_empty());
}

#[test]
fn normal_value_wraps_a_proc_and_a_function() {
    let mut table = SymbolTable::new();
    let n = name(&mut table, "n");
    let mut it = Interner::new();
    let proc = successor(n, constant(&mut it, 1));

    let shared = Rc::new(proc.clone());
    let by_value = Value::proc(proc.clone());
    let by_rc = Value::proc_shared(Rc::clone(&shared));
    assert_eq!(by_value, by_rc);
    assert_eq!(by_value.as_proc(), Some(&proc));
    assert!(by_value.is_proc());
    assert!(!by_value.is_function());
    assert_eq!(by_value.as_atom(), None, "a procedure is not an atom");
    assert_eq!(by_value.as_list(), None, "nor a list");

    let expr = constant(&mut it, 7);
    let fun = rhytido_lang::Fun::new(expr).with_name(n);
    let function = Value::function(fun.clone());
    assert!(function.is_function());
    assert_eq!(function.as_function(), Some(&fun));
    assert_eq!(function.as_proc(), None);
    assert_ne!(function, by_value, "a function is not a procedure");
    assert_ne!(function, Value::default());
}

// ---- Equality and hashing -------------------------------------------------

#[test]
fn normal_structurally_equal_trees_are_equal_and_hash_alike() {
    let mut table = SymbolTable::new();
    let n = name(&mut table, "n");
    let mut it = Interner::new();
    let one = constant(&mut it, 1);
    let a = successor(n, one);
    let b = successor(n, one);
    assert_eq!(a, b);
    assert_eq!(hash_of(&a), hash_of(&b));
    assert_eq!(Value::proc(a), Value::proc(b));
}

#[test]
fn normal_sharing_a_subtree_does_not_change_equality() {
    let mut table = SymbolTable::new();
    let n = name(&mut table, "n");
    let mut it = Interner::new();
    let one = ProgramExpr::math(constant(&mut it, 1));
    // Two ways to build `n + 1`: one aliases a buffer, the other copies it.
    let one_way = ProgramExpr::binary(BinaryOp::Add, ProgramExpr::var(n), one.clone());
    let other_way = ProgramExpr::binary(
        BinaryOp::Add,
        ProgramExpr::var(n),
        ProgramExpr::math(constant(&mut it, 1)),
    );
    assert_eq!(one_way, other_way, "aliasing is invisible to equality");
    assert_eq!(hash_of(&one_way), hash_of(&other_way));

    // And the alias is real: cloning the left operand shares its buffer.
    let ProgramExpr::Binary(_, lhs, _) = &one_way else {
        panic!("a binary node was built above");
    };
    let clone = one_way.clone();
    let ProgramExpr::Binary(_, cloned_lhs, _) = &clone else {
        panic!("cloning keeps the shape");
    };
    assert!(std::ptr::eq(
        std::ptr::from_ref(&**lhs),
        std::ptr::from_ref(&**cloned_lhs)
    ));
}

#[test]
fn normal_different_node_kinds_never_compare_equal() {
    let mut table = SymbolTable::new();
    let n = name(&mut table, "n");
    let mut it = Interner::new();
    let id = constant(&mut it, 1);
    let candidates = [
        ProgramExpr::math(id),
        ProgramExpr::var(n),
        ProgramExpr::call(n, []),
        ProgramExpr::proc_lit(Rc::new(Proc::new([], []))),
        ProgramExpr::string_lit("1"),
        ProgramExpr::list_lit([]),
        ProgramExpr::binary(BinaryOp::Add, ProgramExpr::math(id), ProgramExpr::math(id)),
        ProgramExpr::unary(UnaryOp::Neg, ProgramExpr::math(id)),
    ];
    for (index, left) in candidates.iter().enumerate() {
        for right in candidates.iter().skip(index + 1) {
            assert_ne!(left, right, "distinct shapes must not compare equal");
            assert_ne!(hash_of(left), hash_of(right), "nor hash alike");
        }
    }
}

#[test]
fn normal_statement_variants_are_pairwise_distinct() {
    let mut table = SymbolTable::new();
    let x = name(&mut table, "x");
    let expr = ProgramExpr::var(x);
    let statements = [
        Stmt::assign(x, AssignOp::Plain, expr.clone(), MessagePolicy::Report),
        Stmt::declare([x]),
        Stmt::print([expr.clone()]),
        Stmt::block([]),
        Stmt::procedure(x, Rc::new(Proc::new([], []))),
        Stmt::expr_stmt(expr.clone()),
        Stmt::returning(Some(expr.clone())),
        Stmt::returning(None),
    ];
    for (index, left) in statements.iter().enumerate() {
        for right in statements.iter().skip(index + 1) {
            assert_ne!(left, right, "distinct statements must not compare equal");
        }
    }
    // The two bare-return statements are equal to themselves only.
    assert_eq!(statements[6], statements[6]);
    assert_ne!(statements[6], statements[7]);
}

#[test]
fn normal_scripts_compare_equal_only_when_names_agree() {
    let mut table = SymbolTable::new();
    let x = name(&mut table, "x");
    let left = Script::new([Stmt::expr_stmt(ProgramExpr::var(x))], table.clone());
    let same = Script::new([Stmt::expr_stmt(ProgramExpr::var(x))], table);
    assert_eq!(left, same);
    assert_eq!(hash_of(&left.statements()), hash_of(&same.statements()));

    // A different interned spelling changes the statement, so the scripts differ.
    let mut other_table = SymbolTable::new();
    let y = name(&mut other_table, "y");
    let different = Script::new([Stmt::expr_stmt(ProgramExpr::var(y))], other_table);
    assert_ne!(left, different);
}

// ---- Extreme --------------------------------------------------------------

#[test]
fn extreme_deeply_nested_program_expression_does_not_panic() {
    // 500 levels of `Binary`, built iteratively: the tree is deep, and every
    // operation below (clone, eq, hash, Debug) walks it without recursion
    // problems in the representation itself.
    let mut it = Interner::new();
    let one = ProgramExpr::math(constant(&mut it, 1));
    let two = ProgramExpr::math(constant(&mut it, 2));
    let chain = |leaf: &ProgramExpr| {
        let mut acc = leaf.clone();
        for _ in 0..500 {
            acc = ProgramExpr::binary(BinaryOp::Add, acc, leaf.clone());
        }
        acc
    };
    let deep = chain(&one);
    let clone = deep.clone();
    assert_eq!(deep, clone);
    assert_eq!(hash_of(&deep), hash_of(&clone));

    // Same shape, different mathematics: the two trees are distinguishable, and
    // so is the same chain built twice.
    assert_ne!(deep, chain(&two));
    assert_eq!(deep, chain(&one));
    // Left-nesting depth is visible: 499 levels is not 500.
    let mut shorter = one.clone();
    for _ in 0..499 {
        shorter = ProgramExpr::binary(BinaryOp::Add, shorter, one.clone());
    }
    assert_ne!(deep, shorter);
}

#[test]
fn extreme_long_body_and_formal_lists_are_representable() {
    let mut table = SymbolTable::new();
    let formals: Vec<Formal> = (0..200)
        .map(|index| Formal::single(name(&mut table, &format!("p{index}"))))
        .collect();
    let body: Vec<Stmt> = (0..200)
        .map(|index| Stmt::expr_stmt(ProgramExpr::var(formals[index].name())))
        .collect();
    let proc = Proc::new(formals.iter().copied(), body.iter().cloned());
    assert_eq!(proc.formals().len(), 200);
    assert_eq!(proc.body().len(), 200);
    assert_eq!(proc.arity(), Some(200));
    assert_eq!(proc, Proc::new(formals, body));
}

#[test]
fn extreme_clone_of_a_shared_tree_aliases_instead_of_copying() {
    // The whole point of `Rc` here: cloning a statement must not duplicate its
    // children's buffers.
    let mut it = Interner::new();
    let leaf = ProgramExpr::math(constant(&mut it, 5));
    let list = ProgramExpr::list_lit(std::iter::repeat_n(leaf.clone(), 32));
    let stmt = Stmt::expr_stmt(list.clone());
    let cloned = stmt.clone();
    let original = stmt.as_expr().and_then(ProgramExpr::as_list).expect("list");
    let copy = cloned
        .as_expr()
        .and_then(ProgramExpr::as_list)
        .expect("list");
    assert_eq!(original.len(), copy.len());
    assert!(
        std::ptr::eq(original.as_ptr(), copy.as_ptr()),
        "the element buffer is shared, not copied"
    );
    // A nested procedure is shared the same way.
    let mut table = SymbolTable::new();
    let n = name(&mut table, "n");
    let one = constant(&mut it, 1);
    let shared = Rc::new(successor(n, one));
    let a = ProgramExpr::proc_lit(Rc::clone(&shared));
    let b = ProgramExpr::proc_lit(shared);
    assert_eq!(a, b);
    assert!(std::ptr::eq(
        a.as_proc().expect("a procedure"),
        b.as_proc().expect("a procedure")
    ));
}

#[test]
fn extreme_nested_proc_literals_nest_to_any_depth() {
    let mut table = SymbolTable::new();
    let n = name(&mut table, "n");
    let mut nested = Proc::new([Formal::single(n)], [Stmt::returning(ProgramExpr::var(n))]);
    for _ in 0..50 {
        nested = Proc::new(
            [Formal::single(n)],
            [Stmt::returning(ProgramExpr::proc_lit(Rc::new(
                nested.clone(),
            )))],
        );
    }
    assert_eq!(hash_of(&nested), hash_of(&nested.clone()));
    assert_eq!(nested, nested.clone());
}

#[test]
fn extreme_symbol_ids_from_another_table_are_unknown() {
    let mut table = SymbolTable::new();
    let x = name(&mut table, "x");
    let script = Script::new([Stmt::expr_stmt(ProgramExpr::var(x))], table);
    // An id this script never interned reads back as no name, and an id beyond
    // the table does too.
    assert_eq!(script.name_of(SymbolId::new(999)), None);
    assert_eq!(script.name_of(x), Some("x"));
}

// ---- Error-shaped inputs: representation is total -------------------------

#[test]
fn error_constructors_are_total_and_never_reject() {
    // Nothing at this layer can fail: a body with a misplaced `return`, an
    // empty call, a duplicated formal are all representable, and it is the
    // evaluator that gets to complain (AGENTS.md §2.6, C-VALIDATE at the
    // boundary that actually has a policy).
    let mut table = SymbolTable::new();
    let a = name(&mut table, "a");
    let b = name(&mut table, "b");
    let misplaced = Proc::new(
        [Formal::single(a), Formal::single(a)],
        [
            Stmt::returning(None),
            Stmt::expr_stmt(ProgramExpr::var(b)),
            Stmt::declare([a, a]),
        ],
    );
    assert_eq!(misplaced.formals().len(), 2, "the duplicate name is kept");
    assert_eq!(
        misplaced.returned(),
        None,
        "a `return` that is not last is not the body's return"
    );
    assert_eq!(misplaced.body().len(), 3);
    assert_eq!(
        ProgramExpr::call(a, [])
            .as_call()
            .map(|(_, args)| args.len()),
        Some(0)
    );
}

#[test]
fn error_empty_names_and_strings_are_values_of_the_language() {
    let mut table = SymbolTable::new();
    let empty = table.intern("");
    let script = Script::new(
        [
            Stmt::expr_stmt(ProgramExpr::var(empty)),
            Stmt::print([ProgramExpr::string_lit("")]),
        ],
        table,
    );
    assert_eq!(script.name_of(empty), Some(""));
    let (args, _) = script.statements()[1].as_print().expect("print");
    assert_eq!(args[0].as_string(), Some(""));
}

#[test]
fn error_foreign_core_node_compares_by_id_only() {
    // `ProgramExpr::Math` stores the id; an id from an unrelated interner is not
    // detectable here (and must not be: checking it would mean copying arenas).
    let mut first = Interner::new();
    let mut second = Interner::new();
    let a = first.intern(Expr::Constant(BigFloat::from_i64(1)));
    let b = second.intern(Expr::Pi);
    assert_eq!(a, b, "both are node 0 of their own arena");
    assert_eq!(
        ProgramExpr::math(a),
        ProgramExpr::math(b),
        "and so the tree cannot tell them apart"
    );
}

// ---- Interruption / robustness -------------------------------------------

#[test]
fn robustness_debug_clone_hash_are_total_over_a_whole_script() {
    let mut table = SymbolTable::new();
    let succ = name(&mut table, "succ");
    let n = name(&mut table, "n");
    let mut it = Interner::new();
    let one = constant(&mut it, 1);
    let statements = vec![
        Stmt::assign(
            succ,
            AssignOp::Plain,
            ProgramExpr::proc_lit(Rc::new(successor(n, one))),
            MessagePolicy::Suppress,
        ),
        Stmt::declare([succ, n]),
        Stmt::print_to([ProgramExpr::var(succ)], Redirect::create("out.sol")),
        Stmt::block([Stmt::expr_stmt(ProgramExpr::list_lit([
            ProgramExpr::string_lit("a"),
            ProgramExpr::binary(BinaryOp::Div, ProgramExpr::var(n), ProgramExpr::math(one)),
        ]))]),
        Stmt::procedure(succ, Rc::new(successor(n, one))),
        Stmt::returning(None),
    ];
    let script = Script::new(statements, table);
    let text = format!("{script:?}");
    assert!(text.contains("Assign"), "Debug names the variants: {text}");
    assert_eq!(script.clone(), script);
    assert_eq!(
        hash_of(&script.statements()),
        hash_of(&script.clone().statements())
    );
}

#[test]
fn robustness_panicking_source_never_reaches_the_tree() {
    // The tree has no parser in it, so the only panic surface is a caller
    // handing over an iterator that panics; construction must leave no
    // half-built node behind in that case, which it cannot because nothing is
    // mutated in place.
    struct PanicOnThird;
    impl Iterator for PanicOnThird {
        type Item = Stmt;
        fn next(&mut self) -> Option<Stmt> {
            panic!("the caller's iterator panicked")
        }
    }
    let before = Stmt::block([]);
    let result = std::panic::catch_unwind(|| Stmt::block(PanicOnThird));
    assert!(result.is_err(), "the panic propagates, as documented");
    // Unaffected values stay valid.
    assert_eq!(before, Stmt::block([]));
}

#[test]
fn robustness_hash_is_stable_across_repeated_construction() {
    let mut table = SymbolTable::new();
    let x = name(&mut table, "x");
    let mut it = Interner::new();
    let id = constant(&mut it, 3);
    let build = || {
        Stmt::assign(
            x,
            AssignOp::Plain,
            ProgramExpr::binary(BinaryOp::Pow, ProgramExpr::var(x), ProgramExpr::math(id)),
            MessagePolicy::Report,
        )
    };
    let mut seen = Vec::new();
    for _ in 0..16 {
        let stmt = build();
        seen.push(hash_of(&stmt));
    }
    assert!(seen.windows(2).all(|pair| pair[0] == pair[1]));
    assert_eq!(build(), build());
}
