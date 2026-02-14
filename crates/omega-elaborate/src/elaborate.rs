/// Elaboration: tactic scripts → derivation trees.
///
/// Takes a sequence of tactics, runs them on a proof state, and then
/// reconstructs a Derivation tree that can be verified by the kernel.
use omega_core::binding::apply_meta_subst;
use omega_core::derivation::{Context, Derivation};
use omega_core::expr::Expr;
use omega_core::pattern::match_expr;
use omega_core::theory::Theory;

use crate::engine::ProofState;
use crate::tactic::Tactic;

/// Elaborate a sequence of tactics into a derivation tree.
///
/// This is a two-phase process:
/// 1. Run the tactics to solve all goals (and record the steps)
/// 2. Reconstruct a Derivation tree from the recorded steps
pub fn elaborate(
    tactics: &[Tactic],
    goal: Expr,
    context: Context,
    theory: &Theory,
) -> Result<Derivation, String> {
    let mut state = ProofState::new(goal.clone(), context.clone());
    let mut steps: Vec<TacticStep> = Vec::new();

    for tactic in tactics {
        if state.is_complete() {
            break;
        }

        // Option B desugaring: Auto is expanded into its primitive trace
        // so reconstruction never sees Tactic::Auto — only Apply/Assumption.
        if let Tactic::Auto(depth) = tactic {
            let (new_state, trace) = crate::search::auto_search(&state, theory, *depth)?;
            state = new_state;
            for t in trace {
                steps.push(TacticStep { tactic: t });
            }
        } else {
            state = state.apply_tactic(tactic, theory)?;
            steps.push(TacticStep {
                tactic: tactic.clone(),
            });
        }
    }

    if !state.is_complete() {
        return Err(format!(
            "proof incomplete: {} goals remaining",
            state.goals.len()
        ));
    }

    // Reconstruct derivation from recorded steps
    reconstruct(&steps, &goal, &context, theory, &state.subst)
}

#[derive(Debug)]
struct TacticStep {
    tactic: Tactic,
}

/// Reconstruct a derivation tree from tactic steps.
fn reconstruct(
    steps: &[TacticStep],
    goal: &Expr,
    context: &Context,
    theory: &Theory,
    subst: &std::collections::HashMap<String, Expr>,
) -> Result<Derivation, String> {
    // Simple reconstruction: walk the steps and build the tree
    let mut step_iter = steps.iter().peekable();
    reconstruct_goal(&mut step_iter, goal, context, theory, subst)
}

fn reconstruct_goal<'a>(
    steps: &mut std::iter::Peekable<std::slice::Iter<'a, TacticStep>>,
    goal: &Expr,
    context: &Context,
    theory: &Theory,
    subst: &std::collections::HashMap<String, Expr>,
) -> Result<Derivation, String> {
    let step = steps.next().ok_or("ran out of tactic steps during reconstruction")?;

    match &step.tactic {
        Tactic::Assumption => Ok(Derivation::Assumption),
        Tactic::Apply(rule_name) => {
            let rule = theory
                .get_rule(rule_name)
                .ok_or_else(|| format!("unknown rule: {}", rule_name))?;

            let goal_resolved = apply_meta_subst(goal, subst);
            let local_subst = match_expr(&rule.conclusion, &goal_resolved)
                .map_err(|e| format!("reconstruction: rule {} doesn't match: {}", rule_name, e))?;

            let mut merged_subst = subst.clone();
            for (k, v) in &local_subst {
                merged_subst.insert(k.clone(), v.clone());
            }

            let mut premises = Vec::new();
            for premise_pattern in &rule.premises {
                let premise_goal = apply_meta_subst(premise_pattern, &merged_subst);
                let sub_deriv =
                    reconstruct_goal(steps, &premise_goal, context, theory, &merged_subst)?;
                premises.push(sub_deriv);
            }

            Ok(Derivation::RuleApp {
                rule_name: rule_name.clone(),
                premises,
            })
        }
        Tactic::Exact(deriv) => Ok(deriv.clone()),
        Tactic::Intro(_) => {
            // Intro modifies the context; the sub-derivation proves the new goal
            // For reconstruction, we need to wrap in the appropriate rule
            reconstruct_goal(steps, goal, context, theory, subst)
        }
        _ => Err(format!("reconstruction not implemented for {:?}", step.tactic)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_core::expr::Expr;
    use omega_core::judgment::{ConstructorDecl, JudgmentForm, Rule, SortDecl};
    use omega_core::test_util::make_prop_logic;
    use omega_core::theory::Theory;

    #[test]
    fn elaborate_with_implicit_args() {
        // Test that the tactic engine can handle rules with implicit arguments
        let mut theory = Theory::new("EqLogic");
        theory.sorts.push(SortDecl {
            name: "Tm".to_string(),
        });
        theory.constructors.push(ConstructorDecl {
            name: "eq".to_string(),
            ty: Expr::sym("Tm"),

        });
        theory.judgments.push(JudgmentForm {
            name: "proves".to_string(),
            pattern: Expr::app(vec![Expr::sym("proves"), Expr::meta("P")]),
            constraints: vec![],
        });

        // eq-refl: proves (eq ?a ?a) with ?a implicit
        theory.rules.push(Rule::new(
            "eq-refl",
            vec![],
            Expr::app(vec![
                Expr::sym("proves"),
                Expr::app(vec![Expr::sym("eq"), Expr::meta("a"), Expr::meta("a")]),
            ]),
        ).with_implicit(vec!["a".to_string()]));
        theory.compute_hash();

        let goal = Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("eq"), Expr::free("x"), Expr::free("x")]),
        ]);

        let ctx = Context::new();
        let tactics = vec![Tactic::apply("eq-refl")];

        let deriv = elaborate(&tactics, goal, ctx, &theory).unwrap();
        match &deriv {
            Derivation::RuleApp { rule_name, premises } => {
                assert_eq!(rule_name, "eq-refl");
                assert_eq!(premises.len(), 0);
            }
            _ => panic!("expected RuleApp"),
        }
    }

    #[test]
    fn elaborate_auto_generates_kernel_checkable_proof() {
        // Auto should find a multi-step proof and produce a derivation
        // that the kernel can verify — not a placeholder.
        let theory = make_prop_logic();

        // Goal: prove (and p q) from assumptions (proves p) and (proves q)
        // Requires: and-intro(assumption, assumption) — depth 2
        let goal = Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::free("p"), Expr::free("q")]),
        ]);

        let ctx = Context::with_assumptions(vec![
            Expr::app(vec![Expr::sym("proves"), Expr::free("p")]),
            Expr::app(vec![Expr::sym("proves"), Expr::free("q")]),
        ]);

        let tactics = vec![Tactic::Auto(5)];
        let deriv = elaborate(&tactics, goal.clone(), ctx.clone(), &theory).unwrap();

        // The derivation must be a RuleApp (and-intro), not a bare Assumption
        match &deriv {
            Derivation::RuleApp { rule_name, premises } => {
                assert_eq!(rule_name, "and-intro");
                assert_eq!(premises.len(), 2);
            }
            _ => panic!("expected RuleApp from auto, got {:?}", deriv),
        }

        // Verify the kernel accepts this derivation
        let mut kernel = omega_core::kernel::Kernel::new();
        kernel.register_theory(theory).unwrap();
        kernel
            .check_derivation("PropLogic", &goal, &deriv, &ctx)
            .expect("kernel should accept auto-elaborated proof");
    }

    #[test]
    fn elaborate_simple_proof() {
        let theory = make_prop_logic();

        let goal = Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::free("p"), Expr::free("q")]),
        ]);

        let ctx = Context::with_assumptions(vec![
            Expr::app(vec![Expr::sym("proves"), Expr::free("p")]),
            Expr::app(vec![Expr::sym("proves"), Expr::free("q")]),
        ]);

        let tactics = vec![
            Tactic::apply("and-intro"),
            Tactic::Assumption,
            Tactic::Assumption,
        ];

        let deriv = elaborate(&tactics, goal, ctx, &theory).unwrap();

        // Should produce: (and-intro assumption assumption)
        match &deriv {
            Derivation::RuleApp {
                rule_name,
                premises,
            } => {
                assert_eq!(rule_name, "and-intro");
                assert_eq!(premises.len(), 2);
            }
            _ => panic!("expected RuleApp"),
        }
    }
}
