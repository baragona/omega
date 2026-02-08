/// Binding operations for locally nameless representation.
///
/// - `open`: replace bound var `index` with an expression (entering a binder scope)
/// - `close`: replace a free var with bound var `index` (leaving a scope)
/// - `shift`: adjust all bound var indices (for substitution under binders)
/// - `subst`: replace a free variable with an expression
use crate::expr::{Expr, Name};

/// Replace all occurrences of `Bound(index)` with `replacement` in `expr`.
/// This is used when "opening" a binder: we instantiate the bound variable.
pub fn open(expr: &Expr, index: usize, replacement: &Expr) -> Expr {
    match expr {
        Expr::Bound(i) => {
            if *i == index {
                replacement.clone()
            } else {
                expr.clone()
            }
        }
        Expr::Free(_) | Expr::Meta(_) | Expr::Sym(_) => expr.clone(),
        Expr::App(args) => {
            Expr::App(args.iter().map(|a| open(a, index, replacement)).collect())
        }
        Expr::Binder {
            kind,
            hint,
            ty,
            body,
        } => Expr::Binder {
            kind: kind.clone(),
            hint: hint.clone(),
            ty: Box::new(open(ty, index, replacement)),
            // Under a binder, the bound var's index shifts up by 1
            body: Box::new(open(body, index + 1, &shift(replacement, 0, 1))),
        },
    }
}

/// Replace all occurrences of `Free(name)` with `Bound(index)` in `expr`.
/// This is used when "closing" over a free variable to create a binder.
pub fn close(expr: &Expr, name: &str, index: usize) -> Expr {
    match expr {
        Expr::Free(n) if n == name => Expr::Bound(index),
        Expr::Free(_) | Expr::Bound(_) | Expr::Meta(_) | Expr::Sym(_) => expr.clone(),
        Expr::App(args) => {
            Expr::App(args.iter().map(|a| close(a, name, index)).collect())
        }
        Expr::Binder {
            kind,
            hint,
            ty,
            body,
        } => Expr::Binder {
            kind: kind.clone(),
            hint: hint.clone(),
            ty: Box::new(close(ty, name, index)),
            body: Box::new(close(body, name, index + 1)),
        },
    }
}

/// Shift all bound variable indices >= `cutoff` by `amount`.
/// This is needed when substituting under binders to avoid capture.
pub fn shift(expr: &Expr, cutoff: usize, amount: isize) -> Expr {
    match expr {
        Expr::Bound(i) => {
            if *i >= cutoff {
                let new_idx = (*i as isize + amount) as usize;
                Expr::Bound(new_idx)
            } else {
                expr.clone()
            }
        }
        Expr::Free(_) | Expr::Meta(_) | Expr::Sym(_) => expr.clone(),
        Expr::App(args) => {
            Expr::App(args.iter().map(|a| shift(a, cutoff, amount)).collect())
        }
        Expr::Binder {
            kind,
            hint,
            ty,
            body,
        } => Expr::Binder {
            kind: kind.clone(),
            hint: hint.clone(),
            ty: Box::new(shift(ty, cutoff, amount)),
            body: Box::new(shift(body, cutoff + 1, amount)),
        },
    }
}

/// Substitute all occurrences of `Free(name)` with `replacement` in `expr`.
pub fn subst(expr: &Expr, name: &str, replacement: &Expr) -> Expr {
    match expr {
        Expr::Free(n) if n == name => replacement.clone(),
        Expr::Free(_) | Expr::Bound(_) | Expr::Meta(_) | Expr::Sym(_) => expr.clone(),
        Expr::App(args) => {
            Expr::App(args.iter().map(|a| subst(a, name, replacement)).collect())
        }
        Expr::Binder {
            kind,
            hint,
            ty,
            body,
        } => Expr::Binder {
            kind: kind.clone(),
            hint: hint.clone(),
            ty: Box::new(subst(ty, name, replacement)),
            body: Box::new(subst(body, name, replacement)),
        },
    }
}

/// Apply a meta-substitution: replace `Meta(name)` with `replacement` everywhere.
pub fn subst_meta(expr: &Expr, name: &str, replacement: &Expr) -> Expr {
    match expr {
        Expr::Meta(n) if n == name => replacement.clone(),
        Expr::Free(_) | Expr::Bound(_) | Expr::Meta(_) | Expr::Sym(_) => expr.clone(),
        Expr::App(args) => {
            Expr::App(args.iter().map(|a| subst_meta(a, name, replacement)).collect())
        }
        Expr::Binder {
            kind,
            hint,
            ty,
            body,
        } => Expr::Binder {
            kind: kind.clone(),
            hint: hint.clone(),
            ty: Box::new(subst_meta(ty, name, replacement)),
            body: Box::new(subst_meta(body, name, replacement)),
        },
    }
}

/// Apply all meta-substitutions from a map.
pub fn apply_meta_subst(expr: &Expr, subst_map: &std::collections::HashMap<Name, Expr>) -> Expr {
    match expr {
        Expr::Meta(n) => {
            if let Some(replacement) = subst_map.get(n) {
                replacement.clone()
            } else {
                expr.clone()
            }
        }
        Expr::Free(_) | Expr::Bound(_) | Expr::Sym(_) => expr.clone(),
        Expr::App(args) => {
            Expr::App(args.iter().map(|a| apply_meta_subst(a, subst_map)).collect())
        }
        Expr::Binder {
            kind,
            hint,
            ty,
            body,
        } => Expr::Binder {
            kind: kind.clone(),
            hint: hint.clone(),
            ty: Box::new(apply_meta_subst(ty, subst_map)),
            body: Box::new(apply_meta_subst(body, subst_map)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Expr;

    #[test]
    fn open_simple() {
        // open(#0, 0, x) = x
        let e = Expr::Bound(0);
        let result = open(&e, 0, &Expr::free("x"));
        assert_eq!(result, Expr::free("x"));
    }

    #[test]
    fn open_nested() {
        // (f #0 #1) opened at 0 with x => (f x #1)
        let e = Expr::app(vec![Expr::sym("f"), Expr::Bound(0), Expr::Bound(1)]);
        let result = open(&e, 0, &Expr::free("x"));
        let expected = Expr::app(vec![Expr::sym("f"), Expr::free("x"), Expr::Bound(1)]);
        assert_eq!(result, expected);
    }

    #[test]
    fn close_then_open_roundtrip() {
        // close(x, "x", 0) then open at 0 with x should give back x
        let e = Expr::app(vec![Expr::sym("f"), Expr::free("x"), Expr::sym("y")]);
        let closed = close(&e, "x", 0);
        let opened = open(&closed, 0, &Expr::free("x"));
        assert_eq!(opened, e);
    }

    #[test]
    fn shift_bound_vars() {
        let e = Expr::app(vec![Expr::Bound(0), Expr::Bound(1), Expr::Bound(2)]);
        let shifted = shift(&e, 1, 2);
        let expected = Expr::app(vec![Expr::Bound(0), Expr::Bound(3), Expr::Bound(4)]);
        assert_eq!(shifted, expected);
    }

    #[test]
    fn subst_free_var() {
        let e = Expr::app(vec![Expr::sym("f"), Expr::free("x")]);
        let result = subst(&e, "x", &Expr::sym("true"));
        let expected = Expr::app(vec![Expr::sym("f"), Expr::sym("true")]);
        assert_eq!(result, expected);
    }

    #[test]
    fn meta_subst_application() {
        let e = Expr::app(vec![Expr::sym("proves"), Expr::meta("A")]);
        let result = subst_meta(&e, "A", &Expr::sym("true"));
        let expected = Expr::app(vec![Expr::sym("proves"), Expr::sym("true")]);
        assert_eq!(result, expected);
    }
}
