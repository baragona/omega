/// First-order pattern matching with Miller fragment extensions.
///
/// Patterns contain meta-variables (`?X`) that get bound during matching.
/// The matcher is eager and deterministic.
use std::collections::HashMap;

use crate::expr::{Expr, Name};

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
        }
    }
}

/// Match `pattern` against `expr`, producing a substitution for meta-variables.
///
/// This is first-order matching: meta-variables match any expression, but
/// once bound, must match the same expression consistently.
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
                if pat_args.len() != expr_args.len() {
                    return Err(MatchError::ArityMismatch {
                        expected: pat_args.len(),
                        got: expr_args.len(),
                    });
                }
                for (p, e) in pat_args.iter().zip(expr_args.iter()) {
                    match_inner(p, e, subst)?;
                }
                Ok(())
            }
            _ => Err(MatchError::Mismatch {
                pattern: pattern.clone(),
                expr: expr.clone(),
            }),
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
    fn match_arity_mismatch() {
        let pat = Expr::app(vec![Expr::sym("f"), Expr::meta("A")]);
        let expr = Expr::app(vec![Expr::sym("f"), Expr::sym("x"), Expr::sym("y")]);
        assert!(match_expr(&pat, &expr).is_err());
    }
}
