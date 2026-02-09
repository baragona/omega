/// First-order pattern matching with Miller fragment extensions.
///
/// Patterns contain meta-variables (`?X`) that get bound during matching.
/// The matcher is eager and deterministic.
///
/// The Miller fragment handles higher-order patterns of the form `(?F ?x ?y ...)`:
/// when matching `App([Meta(f), a1, ..., an])` against a target with different arity,
/// it checks the "strict Miller condition" (all args must be distinct Free/Bound vars)
/// and solves `?f` to a lambda abstraction.
use std::collections::HashMap;

use crate::binding::{abstract_over, abstractable_vars, whnf};
use crate::expr::{BinderKind, Expr, Name};

/// Result of a successful pattern match: a mapping from meta-variable names to expressions.
pub type Substitution = HashMap<Name, Expr>;

/// Errors that can occur during pattern matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchError {
    /// A meta-variable was bound to two different expressions.
    Conflict {
        meta: Name,
        existing: Expr,
        new: Expr,
    },
    /// The pattern and expression have different structure.
    Mismatch { pattern: Expr, expr: Expr },
    /// Application length mismatch.
    ArityMismatch { expected: usize, got: usize },
    /// Miller constraint deferred — may become solvable after more metas are instantiated.
    Deferred { meta: Name, reason: String },
}

impl std::fmt::Display for MatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MatchError::Conflict {
                meta,
                existing,
                new,
            } => {
                write!(
                    f,
                    "meta-variable ?{} already bound to {} but also matches {}",
                    meta, existing, new
                )
            }
            MatchError::Mismatch { pattern, expr } => {
                write!(f, "pattern {} does not match {}", pattern, expr)
            }
            MatchError::ArityMismatch { expected, got } => {
                write!(f, "expected {} arguments, got {}", expected, got)
            }
            MatchError::Deferred { meta, reason } => {
                write!(f, "deferred Miller constraint for ?{}: {}", meta, reason)
            }
        }
    }
}

/// Match `pattern` against `expr`, producing a substitution for meta-variables.
///
/// This is first-order matching with Miller fragment extensions:
/// meta-variables match any expression, but once bound, must match the same
/// expression consistently. Meta-headed applications with arity mismatch
/// trigger the Miller pattern unification algorithm.
pub fn match_expr(pattern: &Expr, expr: &Expr) -> Result<Substitution, MatchError> {
    let mut subst = Substitution::new();
    match_inner(pattern, expr, &mut subst)?;
    Ok(subst)
}

/// Match a pattern against an expression, extending the given substitution.
pub fn match_extend(
    pattern: &Expr,
    expr: &Expr,
    subst: &mut Substitution,
) -> Result<(), MatchError> {
    match_inner(pattern, expr, subst)
}

fn match_inner(pattern: &Expr, expr: &Expr, subst: &mut Substitution) -> Result<(), MatchError> {
    // WHNF both sides so beta-redexes are transparent
    let pattern_whnf = whnf(pattern);
    let expr_whnf = whnf(expr);
    match_inner_core(&pattern_whnf, &expr_whnf, subst)
}

fn match_inner_core(pattern: &Expr, expr: &Expr, subst: &mut Substitution) -> Result<(), MatchError> {
    match pattern {
        // Meta-variable: bind or check consistency
        Expr::Meta(name) => {
            if let Some(existing) = subst.get(name) {
                if existing == expr {
                    Ok(())
                } else {
                    Err(MatchError::Conflict {
                        meta: name.clone(),
                        existing: existing.clone(),
                        new: expr.clone(),
                    })
                }
            } else {
                subst.insert(name.clone(), expr.clone());
                Ok(())
            }
        }

        // Symbols must match exactly
        Expr::Sym(n1) => match expr {
            Expr::Sym(n2) if n1 == n2 => Ok(()),
            _ => Err(MatchError::Mismatch {
                pattern: pattern.clone(),
                expr: expr.clone(),
            }),
        },

        // Free variables must match exactly
        Expr::Free(n1) => match expr {
            Expr::Free(n2) if n1 == n2 => Ok(()),
            _ => Err(MatchError::Mismatch {
                pattern: pattern.clone(),
                expr: expr.clone(),
            }),
        },

        // Bound variables must match exactly
        Expr::Bound(i1) => match expr {
            Expr::Bound(i2) if i1 == i2 => Ok(()),
            _ => Err(MatchError::Mismatch {
                pattern: pattern.clone(),
                expr: expr.clone(),
            }),
        },

        // Application: match head and all arguments
        Expr::App(pat_args) => match expr {
            Expr::App(expr_args) => {
                if pat_args.len() == expr_args.len() {
                    for (p, e) in pat_args.iter().zip(expr_args.iter()) {
                        match_inner(p, e, subst)?;
                    }
                    Ok(())
                } else {
                    // Arity mismatch: check for Miller pattern (meta-headed application)
                    if let Expr::Meta(meta_name) = &pat_args[0] {
                        return try_miller_match(meta_name, &pat_args[1..], expr, subst);
                    }
                    Err(MatchError::ArityMismatch {
                        expected: pat_args.len(),
                        got: expr_args.len(),
                    })
                }
            }
            // Pattern is App but expr is not: might be Miller if meta-headed
            _ => {
                if pat_args.len() >= 2 {
                    if let Expr::Meta(meta_name) = &pat_args[0] {
                        return try_miller_match(meta_name, &pat_args[1..], expr, subst);
                    }
                }
                Err(MatchError::Mismatch {
                    pattern: pattern.clone(),
                    expr: expr.clone(),
                })
            }
        },

        // Binders: match kind, type, and body
        Expr::Binder {
            kind: k1,
            ty: t1,
            body: b1,
            ..
        } => match expr {
            Expr::Binder {
                kind: k2,
                ty: t2,
                body: b2,
                ..
            } if k1 == k2 => {
                match_inner(t1, t2, subst)?;
                match_inner(b1, b2, subst)?;
                Ok(())
            }
            _ => Err(MatchError::Mismatch {
                pattern: pattern.clone(),
                expr: expr.clone(),
            }),
        },
    }
}

/// Miller pattern fragment: solve `?meta_name(arg1, ..., argN) = target`
/// by abstracting the argument values out of target and producing a lambda.
///
/// Strict Miller condition: each arg (after substitution) must be a distinct
/// Free or Bound variable. If not met, returns Deferred.
fn try_miller_match(
    meta_name: &str,
    args: &[Expr],
    target: &Expr,
    subst: &mut Substitution,
) -> Result<(), MatchError> {
    use crate::binding::apply_meta_subst;

    // Apply current substitution to each argument
    let resolved_args: Vec<Expr> = args
        .iter()
        .map(|a| {
            let applied = apply_meta_subst(a, subst);
            whnf(&applied)
        })
        .collect();

    // Strict Miller condition check: each resolved arg must be a distinct variable.
    // Free and Bound variables are accepted directly. Meta variables that are still
    // unsolved (not yet in subst) need to find a candidate from the target.
    let mut arg_values = Vec::new();
    for (i, arg) in resolved_args.iter().enumerate() {
        match arg {
            Expr::Free(_) | Expr::Bound(_) => {
                // Check linearity (no duplicate args)
                if arg_values.contains(arg) {
                    return Err(MatchError::Deferred {
                        meta: meta_name.to_string(),
                        reason: format!("duplicate argument at position {}", i),
                    });
                }
                arg_values.push(arg.clone());
            }
            Expr::Meta(m) => {
                // Unsolved meta as argument: try to find a candidate from target's
                // abstractable vars (both Free and Meta variables).
                let target_vars = abstractable_vars(target);
                if target_vars.len() == 1 && args.len() == 1 {
                    // Common single-arg case: one variable, one arg
                    let candidate = target_vars[0].clone();
                    subst.insert(m.clone(), candidate.clone());
                    arg_values.push(candidate);
                } else if !target_vars.is_empty() {
                    // Multiple vars: try each as candidate for this meta
                    // Pick the first one that doesn't conflict with existing arg values
                    let mut found = false;
                    for var in &target_vars {
                        if !arg_values.contains(var) {
                            subst.insert(m.clone(), var.clone());
                            arg_values.push(var.clone());
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return Err(MatchError::Deferred {
                            meta: meta_name.to_string(),
                            reason: format!("no variable candidate for ?{}", m),
                        });
                    }
                } else {
                    return Err(MatchError::Deferred {
                        meta: meta_name.to_string(),
                        reason: format!("unsolved meta ?{} with no candidates", m),
                    });
                }
            }
            _ => {
                // Compound expression: not in Miller fragment, defer
                return Err(MatchError::Deferred {
                    meta: meta_name.to_string(),
                    reason: format!("argument {} is compound, not a variable", arg),
                });
            }
        }
    }

    // Abstract each argument value from target (innermost-to-outermost)
    // to build a lambda body, then wrap in lambda binders.
    let mut body = target.clone();
    for (i, arg_val) in arg_values.iter().enumerate().rev() {
        body = abstract_over(&body, arg_val, 0);
        if i > 0 {
            // Wrap in lambda for all but the outermost (which we'll wrap below)
        }
    }

    // Wrap in lambdas
    let mut result = body;
    for (i, _arg_val) in arg_values.iter().enumerate().rev() {
        let hint = format!("x{}", i);
        result = Expr::Binder {
            kind: BinderKind::Lambda,
            hint,
            ty: Box::new(Expr::sym("_")),
            body: Box::new(result),
        };
    }

    // Check if meta already has a binding
    if let Some(existing) = subst.get(meta_name) {
        if existing == &result {
            Ok(())
        } else {
            Err(MatchError::Conflict {
                meta: meta_name.to_string(),
                existing: existing.clone(),
                new: result,
            })
        }
    } else {
        subst.insert(meta_name.to_string(), result);
        Ok(())
    }
}

/// Check if a pattern is well-formed: all meta-variables are consistently used.
pub fn pattern_meta_vars(pattern: &Expr) -> Vec<Name> {
    pattern.meta_vars()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Expr;

    #[test]
    fn match_simple_meta() {
        // ?A matches anything
        let pat = Expr::meta("A");
        let expr = Expr::sym("true");
        let subst = match_expr(&pat, &expr).unwrap();
        assert_eq!(subst.get("A"), Some(&Expr::sym("true")));
    }

    #[test]
    fn match_app_pattern() {
        // (proves ?A) matches (proves true)
        let pat = Expr::app(vec![Expr::sym("proves"), Expr::meta("A")]);
        let expr = Expr::app(vec![Expr::sym("proves"), Expr::sym("true")]);
        let subst = match_expr(&pat, &expr).unwrap();
        assert_eq!(subst.get("A"), Some(&Expr::sym("true")));
    }

    #[test]
    fn match_nested_pattern() {
        // (proves (and ?A ?B)) matches (proves (and p q))
        let pat = Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
        ]);
        let expr = Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::free("p"), Expr::free("q")]),
        ]);
        let subst = match_expr(&pat, &expr).unwrap();
        assert_eq!(subst.get("A"), Some(&Expr::free("p")));
        assert_eq!(subst.get("B"), Some(&Expr::free("q")));
    }

    #[test]
    fn match_consistent_binding() {
        // (and ?A ?A) matches (and p p) but not (and p q)
        let pat = Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("A")]);

        let ok = Expr::app(vec![Expr::sym("and"), Expr::free("p"), Expr::free("p")]);
        assert!(match_expr(&pat, &ok).is_ok());

        let bad = Expr::app(vec![Expr::sym("and"), Expr::free("p"), Expr::free("q")]);
        assert!(match_expr(&pat, &bad).is_err());
    }

    #[test]
    fn match_symbol_mismatch() {
        let pat = Expr::sym("and");
        let expr = Expr::sym("or");
        assert!(match_expr(&pat, &expr).is_err());
    }

    #[test]
    fn match_arity_mismatch_non_meta() {
        // Non-meta-headed arity mismatch is still an error
        let pat = Expr::app(vec![Expr::sym("f"), Expr::meta("A")]);
        let expr = Expr::app(vec![Expr::sym("f"), Expr::sym("x"), Expr::sym("y")]);
        assert!(match_expr(&pat, &expr).is_err());
    }

    // ── Miller pattern fragment tests ──

    #[test]
    fn miller_basic() {
        // (?P ?n) vs (eq (add n z) n) with ?n unsolved
        // Should solve: ?n → Free("n"), ?P → λx0.(eq (add #0 z) #0)
        let pat = Expr::app(vec![Expr::meta("P"), Expr::meta("n")]);
        let target = Expr::app(vec![
            Expr::sym("eq"),
            Expr::app(vec![Expr::sym("add"), Expr::free("n"), Expr::sym("z")]),
            Expr::free("n"),
        ]);
        let subst = match_expr(&pat, &target).unwrap();

        // ?n should be bound to Free("n")
        assert_eq!(subst.get("n"), Some(&Expr::free("n")));

        // ?P should be a lambda
        let p = subst.get("P").unwrap();
        if let Expr::Binder { kind: BinderKind::Lambda, body, .. } = p {
            // body should be (eq (add #0 z) #0)
            let expected_body = Expr::app(vec![
                Expr::sym("eq"),
                Expr::app(vec![Expr::sym("add"), Expr::Bound(0), Expr::sym("z")]),
                Expr::Bound(0),
            ]);
            assert_eq!(**body, expected_body);
        } else {
            panic!("expected lambda, got {:?}", p);
        }
    }

    #[test]
    fn miller_with_presolved_arg() {
        // (?P ?n) vs (eq (add n z) n) where ?n is already bound to Free("n")
        let pat = Expr::app(vec![Expr::meta("P"), Expr::meta("n")]);
        let target = Expr::app(vec![
            Expr::sym("eq"),
            Expr::app(vec![Expr::sym("add"), Expr::free("n"), Expr::sym("z")]),
            Expr::free("n"),
        ]);
        let mut subst = Substitution::new();
        subst.insert("n".to_string(), Expr::free("n"));
        match_extend(&pat, &target, &mut subst).unwrap();

        let p = subst.get("P").unwrap();
        assert!(matches!(p, Expr::Binder { kind: BinderKind::Lambda, .. }));
    }

    #[test]
    fn miller_compound_arg_deferred() {
        // (?P (s ?n)) — compound argument, should be deferred
        let pat = Expr::app(vec![
            Expr::meta("P"),
            Expr::app(vec![Expr::sym("s"), Expr::meta("n")]),
        ]);
        let target = Expr::app(vec![Expr::sym("eq"), Expr::free("x"), Expr::free("x")]);
        let result = match_expr(&pat, &target);
        assert!(matches!(result, Err(MatchError::Deferred { .. })));
    }

    #[test]
    fn miller_beta_reduces_correctly() {
        // After solving ?P → λx0.(eq (add #0 z) #0), applying (?P z)
        // should beta-reduce to (eq (add z z) z)
        use crate::binding::{apply_meta_subst, beta_normalize};

        let pat = Expr::app(vec![Expr::meta("P"), Expr::meta("n")]);
        let target = Expr::app(vec![
            Expr::sym("eq"),
            Expr::app(vec![Expr::sym("add"), Expr::free("n"), Expr::sym("z")]),
            Expr::free("n"),
        ]);
        let subst = match_expr(&pat, &target).unwrap();

        // Now apply ?P to z: (?P z)
        let applied = Expr::app(vec![
            apply_meta_subst(&Expr::meta("P"), &subst),
            Expr::sym("z"),
        ]);
        let reduced = beta_normalize(&applied);
        let expected = Expr::app(vec![
            Expr::sym("eq"),
            Expr::app(vec![Expr::sym("add"), Expr::sym("z"), Expr::sym("z")]),
            Expr::sym("z"),
        ]);
        assert_eq!(reduced, expected);
    }

    #[test]
    fn miller_with_meta_target() {
        // (?P$1 ?n$1) vs (eq (add ?n z) ?n) — target contains Meta, not Free
        // This is the real-world case: goal has metas like ?n from user's proof
        // The Miller matcher must abstract Meta("n") from the target.
        let pat = Expr::app(vec![Expr::meta("P$1"), Expr::meta("n$1")]);
        let target = Expr::app(vec![
            Expr::sym("eq"),
            Expr::app(vec![Expr::sym("add"), Expr::meta("n"), Expr::sym("z")]),
            Expr::meta("n"),
        ]);
        let subst = match_expr(&pat, &target).unwrap();

        // ?n$1 should be bound to Meta("n")
        assert_eq!(subst.get("n$1"), Some(&Expr::meta("n")));

        // ?P$1 should be a lambda that abstracts Meta("n") to Bound(0)
        let p = subst.get("P$1").unwrap();
        if let Expr::Binder { kind: BinderKind::Lambda, body, .. } = p {
            // body should be (eq (add #0 z) #0)
            let expected_body = Expr::app(vec![
                Expr::sym("eq"),
                Expr::app(vec![Expr::sym("add"), Expr::Bound(0), Expr::sym("z")]),
                Expr::Bound(0),
            ]);
            assert_eq!(**body, expected_body);
        } else {
            panic!("expected lambda, got {:?}", p);
        }
    }
}
