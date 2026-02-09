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

/// Replace all occurrences of `target` in `expr` with `Bound(depth)`.
/// Shifts existing bound vars >= depth up by 1 to avoid capture.
/// This is the inverse of `open` — used for higher-order pattern unification
/// to abstract a free variable out of an expression.
pub fn abstract_over(expr: &Expr, target: &Expr, depth: usize) -> Expr {
    // If the expression exactly matches the target, replace with Bound(depth)
    if expr == target {
        return Expr::Bound(depth);
    }
    match expr {
        Expr::Free(_) | Expr::Meta(_) | Expr::Sym(_) => expr.clone(),
        Expr::Bound(i) => {
            // Shift existing bound vars >= depth up by 1 to make room
            if *i >= depth {
                Expr::Bound(*i + 1)
            } else {
                expr.clone()
            }
        }
        Expr::App(args) => {
            let new_args: Vec<Expr> = args.iter().map(|a| abstract_over(a, target, depth)).collect();
            Expr::App(new_args)
        }
        Expr::Binder { kind, hint, ty, body } => Expr::Binder {
            kind: kind.clone(),
            hint: hint.clone(),
            ty: Box::new(abstract_over(ty, target, depth)),
            body: Box::new(abstract_over(body, target, depth + 1)),
        },
    }
}

/// Weak Head Normal Form: reduce head beta-redexes only.
/// `(lambda (x : T) body)(arg)(rest...)` → `body[#0 := arg](rest...)`
/// Stops when head is not a lambda application. Used in the unifier/matcher.
pub fn whnf(expr: &Expr) -> Expr {
    match expr {
        Expr::App(args) if args.len() >= 2 => {
            // Check if head is a lambda
            let head = whnf(&args[0]);
            if let Expr::Binder { kind: crate::expr::BinderKind::Lambda, body, .. } = &head {
                // Beta-reduce: open body with first argument
                let reduced = open(body, 0, &args[1]);
                if args.len() == 2 {
                    // Exactly one arg consumed, recurse on result
                    whnf(&reduced)
                } else {
                    // More args remain: re-apply remaining args
                    let mut new_args = vec![reduced];
                    new_args.extend_from_slice(&args[2..]);
                    whnf(&Expr::App(new_args))
                }
            } else if head != args[0] {
                // Head reduced but is not a lambda; rebuild with reduced head
                let mut new_args = vec![head];
                new_args.extend_from_slice(&args[1..]);
                Expr::App(new_args)
            } else {
                expr.clone()
            }
        }
        _ => expr.clone(),
    }
}

/// Full beta-normalize: reduce ALL beta-redexes everywhere (innermost strategy).
/// Only used for definitional equality and display, NOT in the unifier.
/// Fuel-limited to prevent non-termination.
pub fn beta_normalize(expr: &Expr) -> Expr {
    beta_normalize_fuel(expr, &mut 1000)
}

fn beta_normalize_fuel(expr: &Expr, fuel: &mut usize) -> Expr {
    if *fuel == 0 {
        return expr.clone();
    }
    match expr {
        Expr::Free(_) | Expr::Bound(_) | Expr::Meta(_) | Expr::Sym(_) => expr.clone(),
        Expr::App(args) => {
            // First, normalize all children
            let normalized: Vec<Expr> = args.iter().map(|a| beta_normalize_fuel(a, fuel)).collect();
            // Then try head reduction (only consume fuel on actual beta-reduction)
            if normalized.len() >= 2 {
                if let Expr::Binder { kind: crate::expr::BinderKind::Lambda, body, .. } = &normalized[0] {
                    *fuel = fuel.saturating_sub(1);
                    let reduced = open(body, 0, &normalized[1]);
                    if normalized.len() == 2 {
                        return beta_normalize_fuel(&reduced, fuel);
                    } else {
                        let mut new_args = vec![reduced];
                        new_args.extend_from_slice(&normalized[2..]);
                        return beta_normalize_fuel(&Expr::App(new_args), fuel);
                    }
                }
            }
            if normalized == *args { expr.clone() } else { Expr::App(normalized) }
        }
        Expr::Binder { kind, hint, ty, body } => {
            let new_ty = beta_normalize_fuel(ty, fuel);
            let new_body = beta_normalize_fuel(body, fuel);
            if &new_ty == ty.as_ref() && &new_body == body.as_ref() {
                expr.clone()
            } else {
                Expr::Binder {
                    kind: kind.clone(),
                    hint: hint.clone(),
                    ty: Box::new(new_ty),
                    body: Box::new(new_body),
                }
            }
        }
    }
}

/// Collect all free variable names in an expression.
pub fn free_vars(expr: &Expr) -> Vec<Name> {
    let mut vars = Vec::new();
    collect_free_vars(expr, &mut vars);
    vars.sort();
    vars.dedup();
    vars
}

fn collect_free_vars(expr: &Expr, acc: &mut Vec<Name>) {
    match expr {
        Expr::Free(n) => {
            if !acc.contains(n) {
                acc.push(n.clone());
            }
        }
        Expr::Bound(_) | Expr::Meta(_) | Expr::Sym(_) => {}
        Expr::App(args) => {
            for a in args {
                collect_free_vars(a, acc);
            }
        }
        Expr::Binder { ty, body, .. } => {
            collect_free_vars(ty, acc);
            collect_free_vars(body, acc);
        }
    }
}

/// Collect all variable-like names (Free and Meta) from an expression.
/// Used by Miller matching where Meta variables in the target can serve
/// as abstraction targets (since goal metas represent universally-quantified variables).
pub fn abstractable_vars(expr: &Expr) -> Vec<Expr> {
    let mut vars = Vec::new();
    collect_abstractable(expr, &mut vars);
    vars
}

fn collect_abstractable(expr: &Expr, acc: &mut Vec<Expr>) {
    match expr {
        Expr::Free(_) | Expr::Meta(_) => {
            if !acc.contains(expr) {
                acc.push(expr.clone());
            }
        }
        Expr::Bound(_) | Expr::Sym(_) => {}
        Expr::App(args) => {
            for a in args {
                collect_abstractable(a, acc);
            }
        }
        Expr::Binder { ty, body, .. } => {
            collect_abstractable(ty, acc);
            collect_abstractable(body, acc);
        }
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

    #[test]
    fn abstract_over_simple() {
        // abstract Free("n") from (eq (add n z) n) at depth 0
        // => (eq (add #0 z) #0)
        let expr = Expr::app(vec![
            Expr::sym("eq"),
            Expr::app(vec![Expr::sym("add"), Expr::free("n"), Expr::sym("z")]),
            Expr::free("n"),
        ]);
        let result = abstract_over(&expr, &Expr::free("n"), 0);
        let expected = Expr::app(vec![
            Expr::sym("eq"),
            Expr::app(vec![Expr::sym("add"), Expr::Bound(0), Expr::sym("z")]),
            Expr::Bound(0),
        ]);
        assert_eq!(result, expected);
    }

    #[test]
    fn abstract_over_shifts_existing_bound() {
        // abstract Free("x") from (f #0 x) at depth 0
        // => (f #1 #0) — existing #0 shifts to #1
        let expr = Expr::app(vec![Expr::sym("f"), Expr::Bound(0), Expr::free("x")]);
        let result = abstract_over(&expr, &Expr::free("x"), 0);
        let expected = Expr::app(vec![Expr::sym("f"), Expr::Bound(1), Expr::Bound(0)]);
        assert_eq!(result, expected);
    }

    #[test]
    fn whnf_beta_redex() {
        // (lambda (x : _) (eq x z)) applied to n => (eq n z)
        let lam = Expr::Binder {
            kind: crate::expr::BinderKind::Lambda,
            hint: "x".to_string(),
            ty: Box::new(Expr::sym("_")),
            body: Box::new(Expr::app(vec![Expr::sym("eq"), Expr::Bound(0), Expr::sym("z")])),
        };
        let app = Expr::app(vec![lam, Expr::free("n")]);
        let result = whnf(&app);
        let expected = Expr::app(vec![Expr::sym("eq"), Expr::free("n"), Expr::sym("z")]);
        assert_eq!(result, expected);
    }

    #[test]
    fn whnf_no_redex() {
        // (f x y) stays as-is
        let e = Expr::app(vec![Expr::sym("f"), Expr::free("x"), Expr::free("y")]);
        assert_eq!(whnf(&e), e);
    }

    #[test]
    fn whnf_nested_lambda_application() {
        // (lambda (x:_) (lambda (y:_) (f #1 #0))) a b => (f a b)
        let inner = Expr::Binder {
            kind: crate::expr::BinderKind::Lambda,
            hint: "y".to_string(),
            ty: Box::new(Expr::sym("_")),
            body: Box::new(Expr::app(vec![Expr::sym("f"), Expr::Bound(1), Expr::Bound(0)])),
        };
        let outer = Expr::Binder {
            kind: crate::expr::BinderKind::Lambda,
            hint: "x".to_string(),
            ty: Box::new(Expr::sym("_")),
            body: Box::new(inner),
        };
        let app = Expr::app(vec![outer, Expr::sym("a"), Expr::sym("b")]);
        let result = whnf(&app);
        let expected = Expr::app(vec![Expr::sym("f"), Expr::sym("a"), Expr::sym("b")]);
        assert_eq!(result, expected);
    }

    #[test]
    fn beta_normalize_deep() {
        // (lambda (x:_) x) applied to a => a (identity function)
        let id = Expr::Binder {
            kind: crate::expr::BinderKind::Lambda,
            hint: "x".to_string(),
            ty: Box::new(Expr::sym("_")),
            body: Box::new(Expr::Bound(0)),
        };
        let app = Expr::app(vec![id, Expr::sym("a")]);
        assert_eq!(beta_normalize(&app), Expr::sym("a"));
    }

    #[test]
    fn beta_normalize_inside_app() {
        // (f ((lambda (x:_) x) a)) => (f a)
        let id = Expr::Binder {
            kind: crate::expr::BinderKind::Lambda,
            hint: "x".to_string(),
            ty: Box::new(Expr::sym("_")),
            body: Box::new(Expr::Bound(0)),
        };
        let e = Expr::app(vec![Expr::sym("f"), Expr::app(vec![id, Expr::sym("a")])]);
        let result = beta_normalize(&e);
        let expected = Expr::app(vec![Expr::sym("f"), Expr::sym("a")]);
        assert_eq!(result, expected);
    }

    #[test]
    fn free_vars_collection() {
        let e = Expr::app(vec![
            Expr::sym("eq"),
            Expr::app(vec![Expr::sym("add"), Expr::free("n"), Expr::sym("z")]),
            Expr::free("n"),
        ]);
        assert_eq!(free_vars(&e), vec!["n".to_string()]);
    }
}
