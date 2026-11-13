//! Expression printing.
//!
//! Renders an interned expression to a string with precedence-aware
//! parenthesization. M1 provides a canonical decimal rendering; exact
//! byte-for-byte Sollya display modes (dyadic/binary/hex, midpoint, canonical
//! form, full parentheses) are layered on in the presentation refinement using
//! the reference test corpus.

use std::fmt::Write;

use crate::expr::{BaseFn, Expr, Interner, NodeId};

/// Precedence levels (higher binds tighter).
fn prec(expr: &Expr) -> u8 {
    match expr {
        Expr::Add(_, _) | Expr::Sub(_, _) => 1,
        Expr::Mul(_, _) | Expr::Div(_, _) => 2,
        Expr::Neg(_) => 3,
        Expr::Pow(_, _) => 4,
        _ => 5, // atoms and function application
    }
}

fn base_fn_name(f: BaseFn) -> &'static str {
    match f {
        BaseFn::Sqrt => "sqrt",
        BaseFn::Exp => "exp",
        // The natural logarithm prints as "ln"; the "log" alias belongs to the
        // M3 parser layer, not to the canonical printer.
        BaseFn::Ln => "ln",
        BaseFn::Sin => "sin",
        BaseFn::Cos => "cos",
        BaseFn::Tan => "tan",
        BaseFn::Asin => "asin",
        BaseFn::Acos => "acos",
        BaseFn::Atan => "atan",
        BaseFn::Sinh => "sinh",
        BaseFn::Cosh => "cosh",
        BaseFn::Tanh => "tanh",
        BaseFn::Asinh => "asinh",
        BaseFn::Acosh => "acosh",
        BaseFn::Atanh => "atanh",
        BaseFn::Erf => "erf",
        BaseFn::Erfc => "erfc",
        BaseFn::Log2 => "log2",
        BaseFn::Log10 => "log10",
        BaseFn::Log1p => "log1p",
        BaseFn::Expm1 => "expm1",
        BaseFn::Abs => "abs",
        BaseFn::Ceil => "ceil",
        BaseFn::Floor => "floor",
        BaseFn::NearestInt => "nearestint",
        BaseFn::Double => "double",
        BaseFn::Single => "single",
        BaseFn::HalfPrecision => "halfprecision",
        BaseFn::Quad => "quad",
        BaseFn::DoubleExtended => "doubleextended",
    }
}

/// Renders the expression `id` to a string.
pub fn print_expr(it: &Interner, id: NodeId) -> String {
    let mut out = String::new();
    write_expr(&mut out, it, id);
    out
}

fn write_expr(out: &mut String, it: &Interner, id: NodeId) {
    let expr = it.get(id);
    match expr {
        Expr::Constant(c) => {
            let _ = write!(out, "{c}");
        }
        Expr::Pi => out.push_str("pi"),
        Expr::Variable => out.push_str("_x_"),
        Expr::Neg(a) => {
            out.push('-');
            write_child(out, it, *a, prec(expr), true);
        }
        Expr::Apply(f, a) => {
            out.push_str(base_fn_name(*f));
            out.push('(');
            write_expr(out, it, *a);
            out.push(')');
        }
        Expr::Add(a, b) => write_bin(out, it, expr, *a, *b, " + "),
        Expr::Sub(a, b) => write_bin(out, it, expr, *a, *b, " - "),
        Expr::Mul(a, b) => write_bin(out, it, expr, *a, *b, "*"),
        Expr::Div(a, b) => write_bin(out, it, expr, *a, *b, "/"),
        Expr::Pow(a, b) => write_bin(out, it, expr, *a, *b, "^"),
    }
}

fn write_bin(out: &mut String, it: &Interner, parent: &Expr, a: NodeId, b: NodeId, op: &str) {
    let p = prec(parent);
    write_child(out, it, a, p, false);
    out.push_str(op);
    // Right operand of a right-associative-sensitive op needs care: for `-` and
    // `/`, a same-precedence right child must be parenthesized.
    let right_same_needs_paren = matches!(parent, Expr::Sub(_, _) | Expr::Div(_, _));
    write_child_right(out, it, b, p, right_same_needs_paren);
}

fn write_child(out: &mut String, it: &Interner, id: NodeId, parent_prec: u8, _is_neg: bool) {
    let child = it.get(id);
    let need_paren = prec(child) < parent_prec;
    if need_paren {
        out.push('(');
    }
    write_expr(out, it, id);
    if need_paren {
        out.push(')');
    }
}

fn write_child_right(
    out: &mut String,
    it: &Interner,
    id: NodeId,
    parent_prec: u8,
    same_needs_paren: bool,
) {
    let child = it.get(id);
    let cp = prec(child);
    let need_paren = cp < parent_prec || (cp == parent_prec && same_needs_paren);
    if need_paren {
        out.push('(');
    }
    write_expr(out, it, id);
    if need_paren {
        out.push(')');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhytido_backend::BigFloat;

    // ---- Normal cases -----------------------------------------------------

    #[test]
    fn normal_precedence_ordering() {
        let mut it = Interner::new();
        let a = it.intern(Expr::Constant(BigFloat::from_i64(2)));
        let b = it.intern(Expr::Constant(BigFloat::from_i64(3)));
        let add = it.intern(Expr::Add(a, b));
        let sub = it.intern(Expr::Sub(a, b));
        let mul = it.intern(Expr::Mul(a, b));
        let div = it.intern(Expr::Div(a, b));
        let neg = it.intern(Expr::Neg(a));
        let pow = it.intern(Expr::Pow(a, b));
        let apply = it.intern(Expr::Apply(BaseFn::Sqrt, a));
        // add/sub < mul/div < neg < pow < atoms/application.
        assert_eq!(prec(it.get(add)), 1);
        assert_eq!(prec(it.get(sub)), 1);
        assert_eq!(prec(it.get(mul)), 2);
        assert_eq!(prec(it.get(div)), 2);
        assert_eq!(prec(it.get(neg)), 3);
        assert_eq!(prec(it.get(pow)), 4);
        assert_eq!(prec(it.get(apply)), 5);
        assert!(prec(it.get(add)) < prec(it.get(mul)));
        assert!(prec(it.get(mul)) < prec(it.get(neg)));
        assert!(prec(it.get(neg)) < prec(it.get(pow)));
        assert!(prec(it.get(pow)) < prec(it.get(apply)));
    }

    #[test]
    fn normal_atoms_and_application_share_top_precedence() {
        let mut it = Interner::new();
        let c = it.intern(Expr::Constant(BigFloat::from_i64(1)));
        let pi = it.intern(Expr::Pi);
        let x = it.intern(Expr::Variable);
        let apply = it.intern(Expr::Apply(BaseFn::Exp, x));
        assert_eq!(prec(it.get(c)), 5);
        assert_eq!(prec(it.get(pi)), prec(it.get(c)));
        assert_eq!(prec(it.get(x)), prec(it.get(c)));
        assert_eq!(prec(it.get(apply)), prec(it.get(c)));
    }
}
