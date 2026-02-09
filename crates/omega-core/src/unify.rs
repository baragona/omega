/// Constraint-based unification for the Omega elaborator.
///
/// Instead of eager pattern matching that fails immediately on a mismatch,
/// this module collects constraints and solves them incrementally. This
/// enables:
/// - Implicit argument inference
/// - Bidirectional type checking
/// - Deferred constraint solving (when not enough info is available yet)
/// - Higher-order pattern unification (Miller patterns)
use crate::binding::apply_meta_subst;
use crate::expr::{Expr, Name};
use crate::pattern::Substitution;

/// A unification variable (existential meta-variable created by the elaborator).
pub type UVar = Name;

/// A unification constraint.
#[derive(Debug, Clone)]
pub enum Constraint {
    /// Two expressions must be equal: `lhs = rhs`.
    Unify(Expr, Expr),
    /// A meta-variable must be solved to some expression: `?X = e`.
    Assign(UVar, Expr),
    /// An expression must match a pattern (one-directional).
    Match { pattern: Expr, expr: Expr },
    /// A constraint that is deferred (not enough info to solve yet).
    Deferred(Box<Constraint>),
}

/// The state of the constraint solver.
#[derive(Debug, Clone)]
pub struct UnificationState {
    /// Current substitution (solved variables).
    pub subst: Substitution,
    /// Unsolved constraints.
    pub pending: Vec<Constraint>,
    /// Deferred constraints (tried but couldn't solve).
    pub deferred: Vec<Constraint>,
    /// Fresh variable counter.
    fresh_counter: usize,
}

/// Result of a unification step.
#[derive(Debug)]
pub enum UnifyResult {
    /// Constraint solved, substitution updated.
    Solved,
    /// Constraint deferred (not enough info).
    Deferred,
    /// Constraint is unsolvable.
    Failed(String),
}

impl UnificationState {
    pub fn new() -> Self {
        UnificationState {
            subst: Substitution::new(),
            pending: Vec::new(),
            deferred: Vec::new(),
            fresh_counter: 0,
        }
    }

    /// Create a fresh unification variable.
    pub fn fresh_uvar(&mut self, prefix: &str) -> UVar {
        self.fresh_counter += 1;
        format!("{}${}", prefix, self.fresh_counter)
    }

    /// Create a fresh meta-expression.
    pub fn fresh_meta(&mut self, prefix: &str) -> Expr {
        Expr::Meta(self.fresh_uvar(prefix))
    }

    /// Add a unification constraint.
    pub fn add_constraint(&mut self, c: Constraint) {
        self.pending.push(c);
    }

    /// Constrain two expressions to be equal.
    pub fn unify(&mut self, lhs: Expr, rhs: Expr) {
        self.pending.push(Constraint::Unify(lhs, rhs));
    }

    /// Solve all pending constraints, iterating until fixpoint.
    pub fn solve(&mut self) -> Result<(), String> {
        let mut progress = true;

        while progress {
            progress = false;

            // Move deferred back to pending for retry
            let mut retry = Vec::new();
            std::mem::swap(&mut retry, &mut self.deferred);
            self.pending.extend(retry);

            let constraints = std::mem::take(&mut self.pending);

            for c in constraints {
                match self.solve_one(c)? {
                    UnifyResult::Solved => {
                        progress = true;
                    }
                    UnifyResult::Deferred => {
                        // Already pushed to deferred by solve_one
                    }
                    UnifyResult::Failed(msg) => {
                        return Err(msg);
                    }
                }
            }
        }

        if !self.deferred.is_empty() {
            return Err(format!(
                "{} unsolved constraints remain",
                self.deferred.len()
            ));
        }

        Ok(())
    }

    /// Try to solve a single constraint.
    fn solve_one(&mut self, constraint: Constraint) -> Result<UnifyResult, String> {
        match constraint {
            Constraint::Unify(lhs, rhs) => self.solve_unify(lhs, rhs),
            Constraint::Assign(var, expr) => {
                let expr = apply_meta_subst(&expr, &self.subst);
                if let Some(existing) = self.subst.get(&var) {
                    // Already assigned — unify with existing
                    self.solve_unify(existing.clone(), expr)
                } else {
                    // Occurs check
                    if occurs(&var, &expr) {
                        return Err(format!(
                            "occurs check failed: ?{} appears in {}",
                            var, expr
                        ));
                    }
                    self.subst.insert(var, expr);
                    Ok(UnifyResult::Solved)
                }
            }
            Constraint::Match { pattern, expr } => {
                let pattern = apply_meta_subst(&pattern, &self.subst);
                let expr = apply_meta_subst(&expr, &self.subst);
                match crate::pattern::match_expr(&pattern, &expr) {
                    Ok(sub) => {
                        for (k, v) in sub {
                            self.subst.insert(k, v);
                        }
                        Ok(UnifyResult::Solved)
                    }
                    Err(_) => {
                        // Maybe the pattern has unsolved metas — defer
                        if pattern.has_metas() || expr.has_metas() {
                            self.deferred.push(Constraint::Match { pattern, expr });
                            Ok(UnifyResult::Deferred)
                        } else {
                            Err(format!("pattern {} does not match {}", pattern, expr))
                        }
                    }
                }
            }
            Constraint::Deferred(inner) => self.solve_one(*inner),
        }
    }

    /// Solve a unification constraint: `lhs = rhs`.
    fn solve_unify(&mut self, lhs: Expr, rhs: Expr) -> Result<UnifyResult, String> {
        let lhs = apply_meta_subst(&lhs, &self.subst);
        let rhs = apply_meta_subst(&rhs, &self.subst);

        // Identical after substitution
        if lhs == rhs {
            return Ok(UnifyResult::Solved);
        }

        match (&lhs, &rhs) {
            // Meta on the left: assign
            (Expr::Meta(name), _) => {
                if occurs(name, &rhs) {
                    return Err(format!("occurs check: ?{} in {}", name, rhs));
                }
                self.subst.insert(name.clone(), rhs);
                Ok(UnifyResult::Solved)
            }

            // Meta on the right: assign
            (_, Expr::Meta(name)) => {
                if occurs(name, &lhs) {
                    return Err(format!("occurs check: ?{} in {}", name, lhs));
                }
                self.subst.insert(name.clone(), lhs);
                Ok(UnifyResult::Solved)
            }

            // Symbols
            (Expr::Sym(a), Expr::Sym(b)) => {
                if a == b {
                    Ok(UnifyResult::Solved)
                } else {
                    Err(format!("cannot unify symbols {} and {}", a, b))
                }
            }

            // Free variables
            (Expr::Free(a), Expr::Free(b)) => {
                if a == b {
                    Ok(UnifyResult::Solved)
                } else {
                    Err(format!("cannot unify variables {} and {}", a, b))
                }
            }

            // Bound variables
            (Expr::Bound(a), Expr::Bound(b)) => {
                if a == b {
                    Ok(UnifyResult::Solved)
                } else {
                    Err(format!("cannot unify bound vars #{} and #{}", a, b))
                }
            }

            // Applications: decompose
            (Expr::App(args_l), Expr::App(args_r)) => {
                if args_l.len() != args_r.len() {
                    return Err(format!(
                        "arity mismatch: {} vs {} args",
                        args_l.len(),
                        args_r.len()
                    ));
                }
                for (l, r) in args_l.iter().zip(args_r.iter()) {
                    self.pending.push(Constraint::Unify(l.clone(), r.clone()));
                }
                Ok(UnifyResult::Solved)
            }

            // Binders: decompose if same kind
            (
                Expr::Binder {
                    kind: k1,
                    ty: t1,
                    body: b1,
                    ..
                },
                Expr::Binder {
                    kind: k2,
                    ty: t2,
                    body: b2,
                    ..
                },
            ) => {
                if k1 != k2 {
                    return Err(format!("binder kind mismatch: {:?} vs {:?}", k1, k2));
                }
                self.pending
                    .push(Constraint::Unify(t1.as_ref().clone(), t2.as_ref().clone()));
                self.pending
                    .push(Constraint::Unify(b1.as_ref().clone(), b2.as_ref().clone()));
                Ok(UnifyResult::Solved)
            }

            // Can't unify yet — defer if metas remain
            _ => {
                if lhs.has_metas() || rhs.has_metas() {
                    self.deferred.push(Constraint::Unify(lhs, rhs));
                    Ok(UnifyResult::Deferred)
                } else {
                    Err(format!("cannot unify {} and {}", lhs, rhs))
                }
            }
        }
    }

    /// Apply the current substitution to an expression transitively.
    /// Iterates until fixpoint so chains like ?A→?B→?C→hello resolve fully.
    pub fn apply(&self, expr: &Expr) -> Expr {
        let mut result = apply_meta_subst(expr, &self.subst);
        // Iterate until fixpoint (for transitive chains)
        loop {
            let next = apply_meta_subst(&result, &self.subst);
            if next == result {
                break;
            }
            result = next;
        }
        result
    }

    /// Check if the state has any unsolved constraints.
    pub fn is_solved(&self) -> bool {
        self.pending.is_empty() && self.deferred.is_empty()
    }
}

impl Default for UnificationState {
    fn default() -> Self {
        Self::new()
    }
}

/// Occurs check: does the variable `name` appear in `expr`?
fn occurs(name: &str, expr: &Expr) -> bool {
    match expr {
        Expr::Meta(n) => n == name,
        Expr::Free(_) | Expr::Bound(_) | Expr::Sym(_) => false,
        Expr::App(args) => args.iter().any(|a| occurs(name, a)),
        Expr::Binder { ty, body, .. } => occurs(name, ty) || occurs(name, body),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Expr;

    #[test]
    fn simple_unification() {
        let mut state = UnificationState::new();
        state.unify(Expr::meta("X"), Expr::sym("true"));
        state.solve().unwrap();
        assert_eq!(state.subst.get("X"), Some(&Expr::sym("true")));
    }

    #[test]
    fn bidirectional_unification() {
        let mut state = UnificationState::new();
        // ?X = (and ?Y true) and ?Y = false
        state.unify(
            Expr::meta("X"),
            Expr::app(vec![
                Expr::sym("and"),
                Expr::meta("Y"),
                Expr::sym("true"),
            ]),
        );
        state.unify(Expr::meta("Y"), Expr::sym("false"));
        state.solve().unwrap();

        assert_eq!(state.subst.get("Y"), Some(&Expr::sym("false")));
        // X should be (and false true)
        let x = state.apply(&Expr::meta("X"));
        assert_eq!(
            x,
            Expr::app(vec![
                Expr::sym("and"),
                Expr::sym("false"),
                Expr::sym("true"),
            ])
        );
    }

    #[test]
    fn decomposition() {
        let mut state = UnificationState::new();
        // (f ?A ?B) = (f p q)
        state.unify(
            Expr::app(vec![Expr::sym("f"), Expr::meta("A"), Expr::meta("B")]),
            Expr::app(vec![Expr::sym("f"), Expr::free("p"), Expr::free("q")]),
        );
        state.solve().unwrap();
        assert_eq!(state.subst.get("A"), Some(&Expr::free("p")));
        assert_eq!(state.subst.get("B"), Some(&Expr::free("q")));
    }

    #[test]
    fn occurs_check() {
        let mut state = UnificationState::new();
        // ?X = (f ?X) — should fail
        state.unify(
            Expr::meta("X"),
            Expr::app(vec![Expr::sym("f"), Expr::meta("X")]),
        );
        assert!(state.solve().is_err());
    }

    #[test]
    fn deferred_constraints() {
        let mut state = UnificationState::new();
        // First add a constraint that can't be solved yet
        // (f ?X) = (f (g ?Y))
        // Then add ?Y = a, which should let the first one solve
        state.unify(
            Expr::app(vec![Expr::sym("f"), Expr::meta("X")]),
            Expr::app(vec![
                Expr::sym("f"),
                Expr::app(vec![Expr::sym("g"), Expr::meta("Y")]),
            ]),
        );
        state.unify(Expr::meta("Y"), Expr::free("a"));
        state.solve().unwrap();

        assert_eq!(state.subst.get("Y"), Some(&Expr::free("a")));
        let x = state.apply(&Expr::meta("X"));
        assert_eq!(
            x,
            Expr::app(vec![Expr::sym("g"), Expr::free("a")])
        );
    }

    #[test]
    fn fresh_variables() {
        let mut state = UnificationState::new();
        let v1 = state.fresh_uvar("T");
        let v2 = state.fresh_uvar("T");
        assert_ne!(v1, v2);
        assert!(v1.starts_with("T$"));
    }

    #[test]
    fn implicit_argument_inference() {
        // Simulate: a rule says (proves (and ?A ?B)) with implicit ?A, ?B
        // The user writes (and-intro p1 p2) and the elaborator must infer A and B
        let mut state = UnificationState::new();

        // Rule conclusion: (proves (and ?A ?B))
        // Goal: (proves (and p q))
        state.unify(
            Expr::app(vec![
                Expr::sym("proves"),
                Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
            ]),
            Expr::app(vec![
                Expr::sym("proves"),
                Expr::app(vec![
                    Expr::sym("and"),
                    Expr::free("p"),
                    Expr::free("q"),
                ]),
            ]),
        );

        state.solve().unwrap();
        assert_eq!(state.subst.get("A"), Some(&Expr::free("p")));
        assert_eq!(state.subst.get("B"), Some(&Expr::free("q")));
    }

    #[test]
    fn constraint_chain() {
        let mut state = UnificationState::new();
        // ?A = ?B, ?B = ?C, ?C = hello
        state.unify(Expr::meta("A"), Expr::meta("B"));
        state.unify(Expr::meta("B"), Expr::meta("C"));
        state.unify(Expr::meta("C"), Expr::sym("hello"));
        state.solve().unwrap();

        let a = state.apply(&Expr::meta("A"));
        assert_eq!(a, Expr::sym("hello"));
    }
}
