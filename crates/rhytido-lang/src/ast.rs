// SPDX-License-Identifier: Apache-2.0
//! Statement and procedure syntax trees of the language layer (ADR-9, M3-B2).
//!
//! ADR-9 cuts the syntax model in two along the line "does it reference a
//! syntax tree?". Everything *mathematical* — constants, the free variable,
//! arithmetic, base-function application — is the core layer's interned
//! [`Expr`], referenced from here by [`NodeId`]. Everything the language adds on
//! top — statements, blocks, procedure literals, calls, string and list
//! literals — lives in this module as an ordinary immutable tree.
//!
//! ## Why no hash-consing and no arena here
//!
//! The core interner buys structural sharing *because* mathematical
//! sub-expressions are looked up, compared and cached by shape all the time.
//! Statements get none of that benefit: a command is evaluated for its effect,
//! once, in the order it was written, and nothing asks "have I seen this
//! assignment before?". ADR-9 §3 records exactly this trade-off, and adds the
//! cost side — routing commands through the core arena would have meant
//! re-opening a value/AST model whose numeric behaviour is already accepted. So
//! this tree is plain [`Rc`]-shared and immutable: cloning a [`Stmt`] or a
//! [`ProgramExpr`] bumps refcounts on the children it has, never copies them,
//! and no node is ever mutated after construction.
//!
//! ## Program expressions versus mathematical ones
//!
//! [`ProgramExpr::Math`] is a leaf holding a core [`NodeId`]: the mathematics
//! that needed no language-level name. The remaining variants are the language's
//! own — a name reference, a call, a procedure literal, a string, a list. The
//! two meet in the operator nodes [`ProgramExpr::Binary`] and
//! [`ProgramExpr::Unary`], which exist for one reason only: a procedure body may
//! write `n + 1` where `n` is a formal parameter, and the core `Expr` has no
//! node that carries a *name* (its [`Expr::Variable`] is the anonymous free
//! variable). Rather than lose the name or change core, an operator whose
//! operands are not all mathematical stays here.
//!
//! [`BinaryOp::fuse`] and [`UnaryOp::fuse`] implement that rule and are the only
//! place it is decided: when both operands are already [`ProgramExpr::Math`],
//! the operator is interned into core and the result is mathematical again, so
//! an identifier-free expression parses to exactly the [`NodeId`] the core
//! interner would have produced on its own (which is what keeps `parse_expr`'s
//! canonical inputs unchanged). See `docs/research/sollya-language-b2.md` §10.
//!
//! ## What this module deliberately does not decide
//!
//! Every type here is a *shape*. Whether a name refers to a program variable or
//! fixes the free variable, whether `=` stores a right-hand side as-is while
//! `:=` evaluates constant expressions first, whether an assignment prints a
//! message, what a procedure application checks, when `error` propagates — all
//! of that is evaluator behaviour (M3-C/D) pinned by reference fixtures. The
//! tree keeps the distinction visible (two [`AssignOp`] variants, a
//! [`MessagePolicy`], a `Formal::List`) so that the evaluator can implement it
//! instead of guessing it back out of a lossy encoding.
//!
//! # Examples
//!
//! ```
//! use rhytido_core::Interner;
//! use rhytido_lang::ast::{AssignOp, MessagePolicy, Proc, ProgramExpr, Stmt};
//! use rhytido_lang::{SymbolId, parse_expr};
//!
//! # fn main() -> Result<(), rhytido_lang::ParseError> {
//! let mut interner = Interner::new();
//! let mut symbols = rhytido_lang::SymbolTable::new();
//!
//! // `n + 1`, as the body of a procedure, keeps the name: the addition cannot
//! // be interned into core because one operand is a reference.
//! let n = symbols.intern("n");
//! let one = parse_expr("1", &mut interner)?;
//! let sum = ProgramExpr::binary(
//!     rhytido_lang::ast::BinaryOp::Add,
//!     ProgramExpr::var(n),
//!     ProgramExpr::math(one),
//! );
//! assert!(sum.as_math().is_none(), "a named operand stays at program level");
//!
//! // `proc(n) { return n + 1; }` — the return statement closes the body.
//! let body = [Stmt::returning(Some(sum))];
//! let proc = Proc::new(
//!     [rhytido_lang::ast::Formal::single(n)],
//!     body,
//! );
//! assert_eq!(proc.arity(), Some(1));
//!
//! // Binding it: `succ = proc(n) { ... };`
//! let succ = symbols.intern("succ");
//! let stmt = Stmt::assign(
//!     succ,
//!     AssignOp::Plain,
//!     ProgramExpr::proc_lit(proc),
//!     MessagePolicy::Report,
//! );
//! assert!(matches!(stmt, Stmt::Assign { .. }));
//! # Ok(())
//! # }
//! ```

use std::rc::Rc;

use rhytido_core::{BaseFn, Expr, Interner, NodeId};

use crate::symbol::{SymbolId, SymbolTable};

/// Which of the language's two assignment operators was written.
///
/// The reference distinguishes them behaviourally: `=` stores the right-hand
/// side *as is*, while `:=` first evaluates constant functional expressions and
/// behaves like `=` otherwise (`docs/research/sollya-language-b2.md` §2.1). The
/// difference is invisible to a printer and decisive to an evaluator, so the
/// tree records which one was parsed rather than normalizing them together.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AssignOp {
    /// `=` — store the right-hand side as it is written.
    Plain,
    /// `:=` — evaluate constant functional expressions before storing.
    Evaluated,
}

/// Whether an assignment announces itself with a message.
///
/// Assigning a global variable normally prints a confirmation, and a trailing
/// `!` on the right-hand side suppresses it (`verbosity = 1!;`,
/// `prec=30!;`). Modelled as a type rather than a flag so the two policies
/// cannot be confused with any other boolean an assignment might grow
/// (AGENTS.md §2.4, C-CUSTOM-TYPE); which message is printed, and whether
/// suppression interacts with the session's catalog, belongs to the evaluator
/// and the presentation layer (M3-C/E).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MessagePolicy {
    /// The assignment reports itself, the ordinary case.
    #[default]
    Report,
    /// The assignment is silent: the source carried a trailing `!`.
    Suppress,
}

impl MessagePolicy {
    /// Returns `true` for [`MessagePolicy::Suppress`].
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn is_suppressing(self) -> bool {
        matches!(self, Self::Suppress)
    }
}

/// Where a `print` statement sends its output.
///
/// Both modes name a file given as a string literal in the source; creating the
/// file, truncating or appending to it is an I/O concern (M3-E and the L6
/// presentation layer), not something this representation performs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Redirect {
    file: Rc<str>,
    mode: FileMode,
}

/// How a [`Redirect`] treats the file it names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileMode {
    /// `>` — create the file, or overwrite it if it exists.
    Create,
    /// `>>` — append to the file.
    Append,
}

impl Redirect {
    /// Creates a redirect to `file` in `mode`.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    #[must_use]
    pub fn new(file: impl Into<Rc<str>>, mode: FileMode) -> Self {
        Self {
            file: file.into(),
            mode,
        }
    }

    /// Creates a `>` redirect: create or overwrite `file`.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    #[must_use]
    pub fn create(file: impl Into<Rc<str>>) -> Self {
        Self::new(file, FileMode::Create)
    }

    /// Creates a `>>` redirect: append to `file`.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    #[must_use]
    pub fn append(file: impl Into<Rc<str>>) -> Self {
        Self::new(file, FileMode::Append)
    }

    /// Borrows the file name.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Returns the file mode.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn mode(&self) -> FileMode {
        self.mode
    }

    /// Returns `true` if this redirect appends.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn is_append(&self) -> bool {
        self.mode == FileMode::Append
    }
}

/// One formal parameter of a procedure.
///
/// The reference offers two shapes, and they are not interchangeable: a list of
/// named parameters is checked for arity at application time, while a single
/// *formal list parameter* (written `L = ...`) collects any number of actual
/// arguments into a list and performs no arity check
/// (`docs/research/sollya-language-b2.md` §4.2). The two shapes are therefore
/// variants, not a count plus a flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Formal {
    /// One named parameter, written as a bare identifier.
    Single(SymbolId),
    /// The variadic parameter, written `identifier = ...`, bound to the list of
    /// all actual arguments.
    List(SymbolId),
}

impl Formal {
    /// Creates a named (single) formal parameter.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    #[must_use]
    pub fn single(name: SymbolId) -> Self {
        Self::Single(name)
    }

    /// Creates the variadic list formal parameter.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    #[must_use]
    pub fn list(name: SymbolId) -> Self {
        Self::List(name)
    }

    /// Returns the name of this parameter, in either shape.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn name(&self) -> SymbolId {
        match self {
            Self::Single(name) | Self::List(name) => *name,
        }
    }

    /// Returns `true` for the variadic [`Formal::List`] shape.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn is_list(&self) -> bool {
        matches!(self, Self::List(_))
    }
}

/// A procedure literal: formal parameters plus a body, captured unevaluated.
///
/// Procedures are anonymous, first-class objects — bound to a name by an
/// assignment, stored in lists, passed as arguments, returned from other
/// procedures — and defining one *only parses* the body: no interpretation and
/// no type checking happens until it is applied
/// (`docs/research/sollya-language-b2.md` §4.4). This type is exactly that
/// parsed, inert body.
///
/// The body may end with a single [`Stmt::Return`], and only there: `return` is
/// permitted at the end of a procedure body and nowhere else
/// (`docs/research/sollya-language-b2.md` §1.7). The parser enforces the
/// placement; this type stores the result, so a hand-built value with a
/// misplaced `return` is representable and simply means nothing until an
/// evaluator defines it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Proc {
    formals: Rc<[Formal]>,
    body: Rc<[Stmt]>,
}

impl Proc {
    /// Creates a procedure from its formal parameters and body statements.
    ///
    /// # Panics
    ///
    /// Never: construction is total. Both iterators are consumed to completion.
    #[must_use]
    pub fn new(
        formals: impl IntoIterator<Item = Formal>,
        body: impl IntoIterator<Item = Stmt>,
    ) -> Self {
        Self {
            formals: Rc::from(formals.into_iter().collect::<Vec<_>>()),
            body: Rc::from(body.into_iter().collect::<Vec<_>>()),
        }
    }

    /// Borrows the formal parameters.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn formals(&self) -> &[Formal] {
        &self.formals
    }

    /// Borrows the body statements, in source order.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn body(&self) -> &[Stmt] {
        &self.body
    }

    /// Returns the trailing `return` expression, if the body has one.
    ///
    /// A cheap view over the last statement; `None` both for a body that ends
    /// with an ordinary statement and for an empty body. The implicit
    /// `return void` the reference supplies for a body without one is evaluator
    /// behaviour (M3-C), so it is *not* materialized here.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn returned(&self) -> Option<Option<&ProgramExpr>> {
        match self.body.last()? {
            Stmt::Return(value) => Some(value.as_ref()),
            _ => None,
        }
    }

    /// Returns `true` if any formal parameter is the variadic list form.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn is_variadic(&self) -> bool {
        self.formals.iter().any(Formal::is_list)
    }

    /// Returns the number of actual arguments an application must supply, or
    /// `None` for a variadic procedure, whose argument count is not checked.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn arity(&self) -> Option<usize> {
        if self.is_variadic() {
            return None;
        }
        Some(self.formals.len())
    }
}

/// A mathematical operator written over program-level operands.
///
/// See the module documentation for why this exists at all: the core `Expr`
/// cannot name a variable, so an operator with a named operand has to stay in
/// the language layer. [`BinaryOp::fuse`] decides, per node, whether the result
/// can go back into core.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
    /// Division.
    Div,
    /// Exponentiation.
    Pow,
}

impl BinaryOp {
    /// Builds the core expression node for this operator over two interned
    /// operands.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    #[must_use]
    pub fn into_expr(self, lhs: NodeId, rhs: NodeId) -> Expr {
        match self {
            Self::Add => Expr::Add(lhs, rhs),
            Self::Sub => Expr::Sub(lhs, rhs),
            Self::Mul => Expr::Mul(lhs, rhs),
            Self::Div => Expr::Div(lhs, rhs),
            Self::Pow => Expr::Pow(lhs, rhs),
        }
    }

    /// Combines `lhs` and `rhs` under this operator.
    ///
    /// When both operands are [`ProgramExpr::Math`], the node is interned into
    /// `interner` and the result is mathematical again — the same [`NodeId`] the
    /// core interner gives an identifier-free expression. Otherwise the result
    /// is a [`ProgramExpr::Binary`] that keeps both operands, and their names,
    /// intact.
    ///
    /// # Panics
    ///
    /// Never: interning is total and the operands are moved, not copied.
    #[must_use]
    pub fn fuse(self, lhs: ProgramExpr, rhs: ProgramExpr, interner: &mut Interner) -> ProgramExpr {
        match (lhs, rhs) {
            (ProgramExpr::Math(lhs), ProgramExpr::Math(rhs)) => {
                ProgramExpr::Math(interner.intern(self.into_expr(lhs, rhs)))
            }
            (lhs, rhs) => ProgramExpr::Binary(self, Rc::new(lhs), Rc::new(rhs)),
        }
    }
}

/// A mathematical operator of one operand, written over program-level operands.
///
/// Base-function application shares this node with negation because both take a
/// single operand and both have a core counterpart; the distinction the core
/// makes between them ([`Expr::Neg`] and [`Expr::Apply`]) is preserved by the
/// variant payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    /// Prefix negation.
    Neg,
    /// Application of a base mathematical function.
    Apply(BaseFn),
}

impl UnaryOp {
    /// Builds the core expression node for this operator over one interned
    /// operand.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    #[must_use]
    pub fn into_expr(self, operand: NodeId) -> Expr {
        match self {
            Self::Neg => Expr::Neg(operand),
            Self::Apply(function) => Expr::Apply(function, operand),
        }
    }

    /// Applies this operator to `operand`, interning into `interner` when
    /// `operand` is mathematical and keeping it at program level otherwise.
    ///
    /// The mirror of [`BinaryOp::fuse`]; see that method for the rule.
    ///
    /// # Panics
    ///
    /// Never: interning is total and the operand is moved, not copied.
    #[must_use]
    pub fn fuse(self, operand: ProgramExpr, interner: &mut Interner) -> ProgramExpr {
        match operand {
            ProgramExpr::Math(operand) => {
                ProgramExpr::Math(interner.intern(self.into_expr(operand)))
            }
            operand => ProgramExpr::Unary(self, Rc::new(operand)),
        }
    }
}

/// An expression in the language layer: mathematics, or something the language
/// adds around it.
///
/// Cloning is shallow — the mathematical leaf is a `Copy` [`NodeId`], a string
/// is an [`Rc<str>`], a list or an argument sequence is an [`Rc<[T]>`], a
/// procedure literal is an `Rc<Proc>` — so a subtree can be shared between
/// statements without copying it. Equality is structural: two expressions are
/// equal iff they have the same shape, the same names and the same interned
/// mathematics, however often their buffers are aliased.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProgramExpr {
    /// A mathematical subtree interned in core.
    ///
    /// The identifier-free part of the language's expression syntax: exactly
    /// what `parse_expr` produces.
    Math(NodeId),
    /// A reference to a name.
    ///
    /// The tree keeps the name and nothing else. Whether it denotes a program
    /// variable, a setting, a value keyword such as `void` or `decimal`, or the
    /// free variable of a mathematical expression is decided by the evaluator
    /// against the session (M3-C, symbol fallback).
    Var(SymbolId),
    /// An application of a named procedure or command to arguments.
    Call {
        /// The callee's name.
        callee: SymbolId,
        /// The actual arguments, in source order; empty for a nullary call.
        args: Rc<[ProgramExpr]>,
    },
    /// A procedure literal, the value `proc(...) { ... }` denotes.
    ProcLit(Rc<Proc>),
    /// A string literal, with its escape sequences already resolved.
    StringLit(Rc<str>),
    /// A list literal `[| a, b, ... |]`.
    ListLit(Rc<[ProgramExpr]>),
    /// A binary mathematical operator over at least one non-mathematical
    /// operand. See [`BinaryOp::fuse`].
    Binary(BinaryOp, Rc<ProgramExpr>, Rc<ProgramExpr>),
    /// A unary mathematical operator over a non-mathematical operand. See
    /// [`UnaryOp::fuse`].
    Unary(UnaryOp, Rc<ProgramExpr>),
}

impl ProgramExpr {
    /// Wraps an interned mathematical subtree.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    #[must_use]
    pub fn math(id: NodeId) -> Self {
        Self::Math(id)
    }

    /// Creates a reference to the name `symbol`.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    #[must_use]
    pub fn var(symbol: SymbolId) -> Self {
        Self::Var(symbol)
    }

    /// Creates an application of `callee` to `args`.
    ///
    /// # Panics
    ///
    /// Never: construction is total. The iterator is consumed to completion.
    #[must_use]
    pub fn call(callee: SymbolId, args: impl IntoIterator<Item = ProgramExpr>) -> Self {
        Self::Call {
            callee,
            args: Rc::from(args.into_iter().collect::<Vec<_>>()),
        }
    }

    /// Creates a procedure literal, sharing `proc` (an owned `Proc` gets its
    /// reference count started here; an existing [`std::rc::Rc`] is reused).
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    #[must_use]
    pub fn proc_lit(proc: impl Into<Rc<Proc>>) -> Self {
        Self::ProcLit(proc.into())
    }

    /// Creates a string literal from `text`, which must already be unescaped.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    #[must_use]
    pub fn string_lit(text: impl Into<Rc<str>>) -> Self {
        Self::StringLit(text.into())
    }

    /// Creates a list literal from `elements`.
    ///
    /// # Panics
    ///
    /// Never: construction is total. The iterator is consumed to completion.
    #[must_use]
    pub fn list_lit(elements: impl IntoIterator<Item = ProgramExpr>) -> Self {
        Self::ListLit(Rc::from(elements.into_iter().collect::<Vec<_>>()))
    }

    /// Creates `lhs op rhs` without interning, keeping both operands at program
    /// level even when they are both mathematical.
    ///
    /// Prefer [`BinaryOp::fuse`], which interns the mathematical case; this
    /// constructor exists for callers that build a tree by hand and want the
    /// node they asked for.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    #[must_use]
    pub fn binary(op: BinaryOp, lhs: ProgramExpr, rhs: ProgramExpr) -> Self {
        Self::Binary(op, Rc::new(lhs), Rc::new(rhs))
    }

    /// Creates `op operand` without interning.
    ///
    /// The unary counterpart of [`ProgramExpr::binary`]; prefer
    /// [`UnaryOp::fuse`].
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    #[must_use]
    pub fn unary(op: UnaryOp, operand: ProgramExpr) -> Self {
        Self::Unary(op, Rc::new(operand))
    }

    /// Returns the interned node of a [`ProgramExpr::Math`], and `None` for
    /// every other shape.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn as_math(&self) -> Option<NodeId> {
        match self {
            Self::Math(id) => Some(*id),
            _ => None,
        }
    }

    /// Returns the name of a [`ProgramExpr::Var`], and `None` otherwise.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn as_var(&self) -> Option<SymbolId> {
        match self {
            Self::Var(symbol) => Some(*symbol),
            _ => None,
        }
    }

    /// Borrows the callee and arguments of a [`ProgramExpr::Call`], and `None`
    /// otherwise.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn as_call(&self) -> Option<(SymbolId, &[ProgramExpr])> {
        match self {
            Self::Call { callee, args } => Some((*callee, args)),
            _ => None,
        }
    }

    /// Borrows the procedure of a [`ProgramExpr::ProcLit`], and `None`
    /// otherwise.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn as_proc(&self) -> Option<&Proc> {
        match self {
            Self::ProcLit(proc) => Some(proc),
            _ => None,
        }
    }

    /// Borrows the contents of a [`ProgramExpr::StringLit`], and `None`
    /// otherwise.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::StringLit(text) => Some(text),
            _ => None,
        }
    }

    /// Borrows the elements of a [`ProgramExpr::ListLit`], and `None`
    /// otherwise.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn as_list(&self) -> Option<&[ProgramExpr]> {
        match self {
            Self::ListLit(elements) => Some(elements),
            _ => None,
        }
    }

    /// Returns `true` if this expression is an interned mathematical subtree,
    /// i.e. if it mentions no language-level name.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn is_math(&self) -> bool {
        matches!(self, Self::Math(_))
    }
}

/// One statement of the language.
///
/// The variants are the statement forms the reference documents at this level
/// (`docs/research/sollya-language-b2.md` §1, §2, §4, §5): assignment, local
/// declaration, printing, a block, a named procedure definition, a bare
/// expression, and the `return` that closes a procedure body. Control flow
/// (`if`/`for`/`while`/`match`) is M3-B3 and is intentionally absent.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Stmt {
    /// An assignment of `value` to the name `target`.
    Assign {
        /// The assigned name. Indexed targets (`L[3] = ...`) are M3-B3.
        target: SymbolId,
        /// Which assignment operator was written.
        op: AssignOp,
        /// The right-hand side, unevaluated.
        value: ProgramExpr,
        /// Whether the assignment announces itself.
        messages: MessagePolicy,
    },
    /// A local declaration `var a, b, ...;`, valid only at the start of a block.
    ///
    /// Declaring a name does not give it a value: the reference dereferences an
    /// unassigned declared variable as `error`, which is evaluator behaviour
    /// (M3-C).
    Declare(Rc<[SymbolId]>),
    /// `print(args)`, optionally redirected to a file.
    Print {
        /// The expressions to print, in source order.
        args: Rc<[ProgramExpr]>,
        /// `None` for standard output.
        redirect: Option<Redirect>,
    },
    /// A begin-end block `{ ... }`, which is itself a single statement.
    ///
    /// A block may open with declarations and may not contain a `return`.
    Block(Rc<[Stmt]>),
    /// `procedure name(formals) { body }` — define *and* assign in one
    /// statement, the documented abbreviation for `name = proc(...) { ... };`.
    ///
    /// Kept as its own statement rather than rewritten into an assignment: the
    /// two are equivalent in effect, and deciding that they are the same object
    /// is an evaluator question, not a parsing one.
    Procedure {
        /// The name the procedure is assigned to.
        name: SymbolId,
        /// The procedure itself.
        proc: Rc<Proc>,
    },
    /// A bare expression used as a statement, for its value or its effect.
    ExprStmt(ProgramExpr),
    /// The `return` that ends a procedure body.
    ///
    /// `None` is a bare `return;`, which the reference documents only with an
    /// expression; the shape is kept total so that a body's implicit
    /// `return void` has somewhere to live once the evaluator decides what it
    /// means.
    Return(Option<ProgramExpr>),
}

impl Stmt {
    /// Creates an assignment of `value` to `target`.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    #[must_use]
    pub fn assign(
        target: SymbolId,
        op: AssignOp,
        value: ProgramExpr,
        messages: MessagePolicy,
    ) -> Self {
        Self::Assign {
            target,
            op,
            value,
            messages,
        }
    }

    /// Creates a local declaration of `names`.
    ///
    /// # Panics
    ///
    /// Never: construction is total. The iterator is consumed to completion.
    #[must_use]
    pub fn declare(names: impl IntoIterator<Item = SymbolId>) -> Self {
        Self::Declare(Rc::from(names.into_iter().collect::<Vec<_>>()))
    }

    /// Creates a `print` of `args` to standard output.
    ///
    /// # Panics
    ///
    /// Never: construction is total. The iterator is consumed to completion.
    #[must_use]
    pub fn print(args: impl IntoIterator<Item = ProgramExpr>) -> Self {
        Self::Print {
            args: Rc::from(args.into_iter().collect::<Vec<_>>()),
            redirect: None,
        }
    }

    /// Creates a `print` of `args` redirected to `redirect`.
    ///
    /// # Panics
    ///
    /// Never: construction is total. The iterator is consumed to completion.
    #[must_use]
    pub fn print_to(args: impl IntoIterator<Item = ProgramExpr>, redirect: Redirect) -> Self {
        Self::Print {
            args: Rc::from(args.into_iter().collect::<Vec<_>>()),
            redirect: Some(redirect),
        }
    }

    /// Creates a block of `statements`.
    ///
    /// # Panics
    ///
    /// Never: construction is total. The iterator is consumed to completion.
    #[must_use]
    pub fn block(statements: impl IntoIterator<Item = Stmt>) -> Self {
        Self::Block(Rc::from(statements.into_iter().collect::<Vec<_>>()))
    }

    /// Creates a `procedure name(formals) { body }` definition.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    #[must_use]
    pub fn procedure(name: SymbolId, proc: Rc<Proc>) -> Self {
        Self::Procedure { name, proc }
    }

    /// Creates a statement that evaluates `expr` for its value or effect.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    #[must_use]
    pub fn expr_stmt(expr: ProgramExpr) -> Self {
        Self::ExprStmt(expr)
    }

    /// Creates a `return` statement, with or without a value.
    ///
    /// Accepts `impl Into<Option<_>>` so both `Stmt::returning(expr)` and
    /// `Stmt::returning(None)` read naturally.
    ///
    /// # Panics
    ///
    /// Never: construction is total.
    #[must_use]
    pub fn returning(value: impl Into<Option<ProgramExpr>>) -> Self {
        Self::Return(value.into())
    }

    /// Returns `true` if this is a [`Stmt::Return`].
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn is_return(&self) -> bool {
        matches!(self, Self::Return(_))
    }

    /// Borrows the value of a [`Stmt::Return`], and `None` for any other
    /// statement.
    ///
    /// The outer `Option` says whether this is a return at all; the inner one
    /// distinguishes `return;` from `return expr;`.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn as_return(&self) -> Option<Option<&ProgramExpr>> {
        match self {
            Self::Return(value) => Some(value.as_ref()),
            _ => None,
        }
    }

    /// Borrows the statements of a [`Stmt::Block`], and `None` otherwise.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn as_block(&self) -> Option<&[Stmt]> {
        match self {
            Self::Block(statements) => Some(statements),
            _ => None,
        }
    }

    /// Borrows the expression of a [`Stmt::ExprStmt`], and `None` otherwise.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn as_expr(&self) -> Option<&ProgramExpr> {
        match self {
            Self::ExprStmt(expr) => Some(expr),
            _ => None,
        }
    }

    /// Borrows the declared names of a [`Stmt::Declare`], and `None` otherwise.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn as_declare(&self) -> Option<&[SymbolId]> {
        match self {
            Self::Declare(names) => Some(names),
            _ => None,
        }
    }

    /// Borrows the arguments and redirect of a [`Stmt::Print`], and `None`
    /// otherwise.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn as_print(&self) -> Option<(&[ProgramExpr], Option<&Redirect>)> {
        match self {
            Self::Print { args, redirect } => Some((args, redirect.as_ref())),
            _ => None,
        }
    }
}

/// A parsed script: its statements and the names they mention.
///
/// The symbol table travels with the statements because a [`Stmt`] refers to
/// names by [`SymbolId`], and an id is meaningless without the table that
/// interned it (ADR-11: interning belongs to a session, never to a global). A
/// parse therefore owns a table of its own; the caller that has a
/// [`crate::Session`] re-interns the texts it reads back through
/// [`Script::name_of`] so the resulting ids are the session's.
///
/// [`Script::statements`] borrows the statements without copying them.
/// Equality is structural over both halves: two scripts are equal iff their
/// statements *and* their symbol tables agree, because a [`SymbolId`] only
/// means anything together with the table that produced it.
///
/// `Hash` is deliberately not implemented: a script owns a [`SymbolTable`]
/// whose reverse index is a `HashMap`, which has no hash of its own, and a
/// hashing kernel over an unordered index would either be order-dependent (and
/// therefore not agree with [`PartialEq`] after a re-interning) or throw the
/// index away (and therefore duplicate [`Stmt`]'s own hash for no gain).
/// Structural identity lives in [`Script::statements`], which is `Hash`.
///
/// # Examples
///
/// ```
/// use rhytido_lang::ast::{AssignOp, MessagePolicy, ProgramExpr, Stmt};
/// use rhytido_lang::SymbolTable;
/// use rhytido_lang::ast::Script;
///
/// let mut symbols = SymbolTable::new();
/// let name = symbols.intern("x");
/// let script = Script::new([Stmt::expr_stmt(ProgramExpr::var(name))], symbols);
/// assert_eq!(script.len(), 1);
/// assert_eq!(script.name_of(name), Some("x"));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Script {
    statements: Rc<[Stmt]>,
    symbols: SymbolTable,
}

impl Script {
    /// Creates a script from its statements and the table their names came
    /// from.
    ///
    /// # Panics
    ///
    /// Never: construction is total. The iterator is consumed to completion.
    #[must_use]
    pub fn new(statements: impl IntoIterator<Item = Stmt>, symbols: SymbolTable) -> Self {
        Self {
            statements: Rc::from(statements.into_iter().collect::<Vec<_>>()),
            symbols,
        }
    }

    /// Borrows the statements, in source order.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn statements(&self) -> &[Stmt] {
        &self.statements
    }

    /// Borrows the symbol table holding the names this script mentions.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn symbols(&self) -> &SymbolTable {
        &self.symbols
    }

    /// Reads back the text a [`SymbolId`] of this script was interned from.
    ///
    /// Returns `None` for an id that did not come from [`Script::symbols`].
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn name_of(&self, name: SymbolId) -> Option<&str> {
        self.symbols.text(name)
    }

    /// Returns the number of statements.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn len(&self) -> usize {
        self.statements.len()
    }

    /// Returns `true` if the script has no statements.
    ///
    /// # Panics
    ///
    /// Never.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.statements.is_empty()
    }
}
