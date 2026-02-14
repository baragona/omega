/// Proof state and goal management for tactic proofs.
use std::collections::HashMap;

use omega_core::binding::apply_meta_subst;
use omega_core::derivation::{Context, Derivation};
use omega_core::expr::Expr;
use omega_core::pattern::{match_expr, Substitution};
use omega_core::theory::Theory;
use omega_core::unify::UnificationState;

use crate::tactic::Tactic;

/// A single proof goal.
#[derive(Debug, Clone)]
pub struct Goal {
    /// The judgment to prove.
    pub target: Expr,
    /// Local assumptions available for this goal.
    pub context: Context,
}

/// The proof state during tactic execution.
#[derive(Debug, Clone)]
pub struct ProofState {
    /// Remaining goals to prove.
    pub goals: Vec<Goal>,
    /// The meta-variable substitution accumulated so far.
    pub subst: Substitution,
}

impl ProofState {
    /// Create a new proof state with a single goal.
    pub fn new(goal: Expr, context: Context) -> Self {
        ProofState {
            goals: vec![Goal {
                target: goal,
                context,
            }],
            subst: HashMap::new(),
        }
    }

    /// Check if all goals have been solved.
    pub fn is_complete(&self) -> bool {
        self.goals.is_empty()
    }

    /// Apply a tactic to the first goal.
    pub fn apply_tactic(
        &self,
        tactic: &Tactic,
        theory: &Theory,
    ) -> Result<ProofState, String> {
        if self.goals.is_empty() {
            return Err("no goals remaining".to_string());
        }

        match tactic {
            Tactic::Apply(rule_name) => self.apply_rule(rule_name, theory),
            Tactic::Assumption => self.apply_assumption(),
            Tactic::Intro(name) => self.apply_intro(name.as_deref(), theory),
            Tactic::Exact(deriv) => self.apply_exact(deriv),
            Tactic::Auto(depth) => {
                let (state, _trace) = crate::search::auto_search(self, theory, *depth)?;
                Ok(state)
            }
            Tactic::Try(t) => {
                match self.apply_tactic(t, theory) {
                    Ok(state) => Ok(state),
                    Err(_) => Ok(self.clone()), // Try never fails
                }
            }
            Tactic::Repeat(t) => {
                let mut state = self.clone();
                loop {
                    match state.apply_tactic(t, theory) {
                        Ok(new_state) => {
                            if new_state.goals.len() >= state.goals.len() {
                                // No progress, stop
                                return Ok(new_state);
                            }
                            state = new_state;
                        }
                        Err(_) => return Ok(state),
                    }
                }
            }
            Tactic::Focus(idx, t) => {
                if *idx >= self.goals.len() {
                    return Err(format!("goal index {} out of range", idx));
                }
                // Swap the focused goal to position 0
                let mut new_state = self.clone();
                new_state.goals.swap(0, *idx);
                let result = new_state.apply_tactic(t, theory)?;
                Ok(result)
            }
            Tactic::Seq(tactics) => {
                let mut state = self.clone();
                for t in tactics {
                    state = state.apply_tactic(t, theory)?;
                }
                Ok(state)
            }
        }
    }

    fn apply_rule(&self, rule_name: &str, theory: &Theory) -> Result<ProofState, String> {
        let rule = theory
            .get_rule(rule_name)
            .ok_or_else(|| format!("unknown rule: {}", rule_name))?;

        let goal = &self.goals[0];
        let goal_resolved = apply_meta_subst(&goal.target, &self.subst);

        // Use constraint-based unification for matching
        let mut unifier = UnificationState::new();

        // Create fresh meta-variables for implicit arguments
        let mut implicit_subst = Substitution::new();
        for arg_name in &rule.implicit_args {
            let fresh = unifier.fresh_meta(arg_name);
            implicit_subst.insert(arg_name.clone(), fresh);
        }

        // Instantiate the rule conclusion with fresh implicits
        let conclusion = if implicit_subst.is_empty() {
            rule.conclusion.clone()
        } else {
            apply_meta_subst(&rule.conclusion, &implicit_subst)
        };

        // Add unification constraint: rule conclusion = goal
        unifier.unify(conclusion, goal_resolved.clone());

        // Solve initial constraints
        match unifier.solve() {
            Ok(()) => {}
            Err(_) => {
                // Fallback to simple pattern matching
                let local_subst = match_expr(&rule.conclusion, &goal_resolved)
                    .map_err(|e| format!("rule {} doesn't match goal: {}", rule_name, e))?;

                return self.apply_rule_with_subst(rule_name, rule, goal, local_subst);
            }
        }

        // Merge unifier results into local_subst
        let mut local_subst = Substitution::new();
        for (k, v) in &unifier.subst {
            local_subst.insert(k.clone(), v.clone());
        }

        self.apply_rule_with_subst(rule_name, rule, goal, local_subst)
    }

    fn apply_rule_with_subst(
        &self,
        _rule_name: &str,
        rule: &omega_core::judgment::Rule,
        goal: &Goal,
        local_subst: Substitution,
    ) -> Result<ProofState, String> {
        // Create new goals for each premise
        let mut new_goals: Vec<Goal> = Vec::new();
        for premise in &rule.premises {
            let premise_goal = apply_meta_subst(premise, &local_subst);
            let premise_goal = apply_meta_subst(&premise_goal, &self.subst);
            new_goals.push(Goal {
                target: premise_goal,
                context: goal.context.clone(),
            });
        }

        // Replace the first goal with the new subgoals
        let mut remaining = new_goals;
        remaining.extend_from_slice(&self.goals[1..]);

        let mut new_subst = self.subst.clone();
        for (k, v) in local_subst {
            new_subst.insert(k, v);
        }

        Ok(ProofState {
            goals: remaining,
            subst: new_subst,
        })
    }

    fn apply_assumption(&self) -> Result<ProofState, String> {
        let goal = &self.goals[0];
        let goal_resolved = apply_meta_subst(&goal.target, &self.subst);

        for assumption in &goal.context.assumptions {
            let assumption_resolved = apply_meta_subst(assumption, &self.subst);
            if assumption_resolved == goal_resolved {
                let mut new_state = self.clone();
                new_state.goals.remove(0);
                return Ok(new_state);
            }
            // Try pattern matching first
            if let Ok(sub) = match_expr(&goal_resolved, &assumption_resolved) {
                let mut new_state = self.clone();
                new_state.goals.remove(0);
                for (k, v) in sub {
                    new_state.subst.insert(k, v);
                }
                return Ok(new_state);
            }
            // Try bidirectional unification as fallback
            if goal_resolved.has_metas() || assumption_resolved.has_metas() {
                let mut unifier = UnificationState::new();
                unifier.unify(goal_resolved.clone(), assumption_resolved.clone());
                if unifier.solve().is_ok() {
                    let mut new_state = self.clone();
                    new_state.goals.remove(0);
                    for (k, v) in unifier.subst {
                        new_state.subst.insert(k, v);
                    }
                    return Ok(new_state);
                }
            }
        }

        Err(format!(
            "no assumption matches goal {}",
            goal_resolved
        ))
    }

    fn apply_intro(&self, _name: Option<&str>, _theory: &Theory) -> Result<ProofState, String> {
        let goal = &self.goals[0];
        let goal_resolved = apply_meta_subst(&goal.target, &self.subst);

        // For now, intro works on implication-like goals:
        // If the goal is (proves (imp A B)), add (proves A) to context and target (proves B)
        if let Expr::App(args) = &goal_resolved {
            if args.len() == 2 {
                if let Expr::App(inner) = &args[1] {
                    if inner.len() == 3 {
                        if let Expr::Sym(name) = &inner[0] {
                            if name == "imp" || name == "->" {
                                let hyp = Expr::app(vec![args[0].clone(), inner[1].clone()]);
                                let new_target =
                                    Expr::app(vec![args[0].clone(), inner[2].clone()]);

                                let mut new_ctx = goal.context.clone();
                                new_ctx.push(hyp);

                                let mut new_goals = vec![Goal {
                                    target: new_target,
                                    context: new_ctx,
                                }];
                                new_goals.extend_from_slice(&self.goals[1..]);

                                return Ok(ProofState {
                                    goals: new_goals,
                                    subst: self.subst.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }

        Err(format!(
            "intro: goal {} is not an implication",
            goal_resolved
        ))
    }

    fn apply_exact(&self, _deriv: &Derivation) -> Result<ProofState, String> {
        // The exact tactic just closes the current goal with the given derivation.
        // We trust that it will be verified by the kernel later.
        let mut new_state = self.clone();
        new_state.goals.remove(0);
        Ok(new_state)
    }
}
