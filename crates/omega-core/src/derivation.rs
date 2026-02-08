/// Derivation trees and proof checking.
///
/// A derivation tree records the proof structure: each node is a rule application
/// with sub-derivations for the premises. The kernel walks the tree and verifies
/// that each step is valid.
use std::collections::{HashMap, HashSet};

use crate::binding::apply_meta_subst;
use crate::error::{OmegaError, Result};
use crate::expr::{Expr, Name};
use crate::judgment::RewriteRule;
use crate::pattern::{match_expr, Substitution};
use crate::theory::{ContextMode, Theory};

/// A derivation tree node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Derivation {
    /// Apply a rule with sub-derivations for each premise.
    RuleApp {
        rule_name: Name,
        /// Sub-derivations, one for each premise of the rule.
        premises: Vec<Derivation>,
    },
    /// An assumption: the goal must appear in the current context.
    Assumption,
    /// An assumption identified by index in the context.
    AssumptionIdx(usize),
}

/// Context for proof checking: a list of assumed judgments.
#[derive(Debug, Clone)]
pub struct Context {
    pub assumptions: Vec<Expr>,
}

impl Context {
    pub fn new() -> Self {
        Context {
            assumptions: Vec::new(),
        }
    }

    pub fn with_assumptions(assumptions: Vec<Expr>) -> Self {
        Context { assumptions }
    }

    pub fn push(&mut self, assumption: Expr) {
        self.assumptions.push(assumption);
    }
}

/// Normalize an expression by exhaustively applying rewrite rules (innermost strategy).
fn normalize_expr(expr: &Expr, rewrites: &[RewriteRule], fuel: &mut usize) -> Expr {
    if rewrites.is_empty() || *fuel == 0 {
        return expr.clone();
    }

    // Step 1: Normalize children
    let children_normalized = match expr {
        Expr::App(args) => {
            let new_args: Vec<Expr> = args
                .iter()
                .map(|a| normalize_expr(a, rewrites, fuel))
                .collect();
            if new_args == *args {
                expr.clone()
            } else {
                Expr::App(new_args)
            }
        }
        _ => expr.clone(),
    };

    // Step 2: Try rewrite rules at the head
    let mut current = children_normalized;
    loop {
        if *fuel == 0 {
            break;
        }
        let mut matched = false;
        for rw in rewrites {
            if let Ok(subst) = match_expr(&rw.lhs, &current) {
                *fuel = fuel.saturating_sub(1);
                let replaced = apply_meta_subst(&rw.rhs, &subst);
                current = normalize_expr(&replaced, rewrites, fuel);
                matched = true;
                break;
            }
        }
        if !matched {
            break;
        }
    }

    current
}

/// Check that a derivation proves the given goal in the given theory.
///
/// This is the main verification function. It walks the derivation tree
/// top-down, checking that each rule application is valid.
pub fn check_derivation(
    theory: &Theory,
    goal: &Expr,
    derivation: &Derivation,
    ctx: &Context,
) -> Result<()> {
    let mut fresh_counter = 0usize;
    let mut consumed = HashSet::new();
    check_derivation_inner(
        theory,
        goal,
        derivation,
        ctx,
        &mut HashMap::new(),
        &mut fresh_counter,
        &mut consumed,
    )
}

/// Freshen all meta-variables in a rule by appending a unique suffix.
/// This prevents collisions when the same rule is used multiple times
/// (e.g., nested eq-trans calls).
fn freshen_rule(rule: &crate::judgment::Rule, counter: &mut usize) -> crate::judgment::Rule {
    *counter += 1;
    let suffix = format!("${}", counter);

    // Collect all meta-variable names used in the rule
    let mut metas = rule.conclusion.meta_vars();
    for p in &rule.premises {
        for m in p.meta_vars() {
            if !metas.contains(&m) {
                metas.push(m);
            }
        }
    }

    if metas.is_empty() {
        return rule.clone();
    }

    // Build a renaming substitution
    let mut rename = HashMap::new();
    for m in &metas {
        rename.insert(m.clone(), Expr::Meta(format!("{}{}", m, suffix)));
    }

    crate::judgment::Rule {
        name: rule.name.clone(),
        premises: rule.premises.iter().map(|p| apply_meta_subst(p, &rename)).collect(),
        conclusion: apply_meta_subst(&rule.conclusion, &rename),
        reflected: rule.reflected,
        provenance: rule.provenance.clone(),
        implicit_args: rule.implicit_args.iter().map(|a| format!("{}{}", a, suffix)).collect(),
        context_extensions: rule.context_extensions.iter()
            .map(|(idx, expr)| (*idx, apply_meta_subst(expr, &rename)))
            .collect(),
    }
}

/// Simple bidirectional unification: try to make `a` and `b` equal by
/// solving meta-variables in both sides.
fn unify_exprs(a: &Expr, b: &Expr) -> Option<Substitution> {
    let mut subst = Substitution::new();
    if unify_inner(a, b, &mut subst) {
        Some(subst)
    } else {
        None
    }
}

fn unify_inner(a: &Expr, b: &Expr, subst: &mut Substitution) -> bool {
    if a == b {
        return true;
    }
    match (a, b) {
        (Expr::Meta(name), _) => {
            if let Some(existing) = subst.get(name) {
                existing == b
            } else {
                // Occurs check: prevent circular bindings like ?n → (s ?n)
                if b.meta_vars().contains(name) {
                    return false;
                }
                subst.insert(name.clone(), b.clone());
                true
            }
        }
        (_, Expr::Meta(name)) => {
            if let Some(existing) = subst.get(name) {
                existing == a
            } else {
                if a.meta_vars().contains(name) {
                    return false;
                }
                subst.insert(name.clone(), a.clone());
                true
            }
        }
        (Expr::Sym(a_name), Expr::Sym(b_name)) => a_name == b_name,
        (Expr::Free(a_name), Expr::Free(b_name)) => a_name == b_name,
        (Expr::Bound(a_idx), Expr::Bound(b_idx)) => a_idx == b_idx,
        (Expr::App(args_a), Expr::App(args_b)) => {
            if args_a.len() != args_b.len() {
                return false;
            }
            args_a
                .iter()
                .zip(args_b.iter())
                .all(|(x, y)| unify_inner(x, y, subst))
        }
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
        ) => k1 == k2 && unify_inner(t1, t2, subst) && unify_inner(b1, b2, subst),
        _ => false,
    }
}

/// Infer what a derivation proves (bottom-up), returning the conclusion.
/// This is used when we need to determine what a sub-derivation produces
/// so we can solve unification variables.
fn infer_conclusion(
    theory: &Theory,
    derivation: &Derivation,
    ctx: &Context,
    subst: &Substitution,
    fresh_counter: &mut usize,
) -> Option<Expr> {
    match derivation {
        Derivation::Assumption => {
            // We can't infer from an assumption alone without knowing which one
            None
        }
        Derivation::AssumptionIdx(idx) => {
            ctx.assumptions.get(*idx).map(|a| apply_meta_subst(a, subst))
        }
        Derivation::RuleApp { rule_name, premises } => {
            let rule_orig = theory.get_rule(rule_name)?;
            // Freshen the rule to avoid meta-variable name collisions
            // with the goal's metas (which could cause circular bindings).
            let rule = freshen_rule(rule_orig, fresh_counter);
            // Match the premises recursively to fill in the rule's metas
            let mut rule_subst = Substitution::new();
            for (premise_deriv, premise_pattern) in premises.iter().zip(rule.premises.iter()) {
                if let Some(inferred) = infer_conclusion(theory, premise_deriv, ctx, subst, fresh_counter) {
                    // Try to match the premise pattern against what we inferred
                    if let Ok(s) = match_expr(premise_pattern, &inferred) {
                        for (k, v) in s {
                            rule_subst.insert(k, v);
                        }
                    }
                }
            }
            Some(apply_meta_subst(&rule.conclusion, &rule_subst))
        }
    }
}

fn check_derivation_inner(
    theory: &Theory,
    goal: &Expr,
    derivation: &Derivation,
    ctx: &Context,
    global_subst: &mut Substitution,
    fresh_counter: &mut usize,
    consumed: &mut HashSet<usize>,
) -> Result<()> {
    let affine = theory.context_mode == ContextMode::Affine;
    let mut fuel = 10_000usize;
    match derivation {
        Derivation::Assumption => {
            // The goal must match one of the assumptions in the context.
            // In affine mode, iterate from the end (most recent first),
            // skipping consumed entries — this gives shadowing for free.
            let goal_resolved = apply_meta_subst(goal, global_subst);
            let goal_norm = normalize_expr(&goal_resolved, &theory.rewrites, &mut fuel);
            for idx in (0..ctx.assumptions.len()).rev() {
                if affine && consumed.contains(&idx) {
                    continue;
                }
                let assumption = &ctx.assumptions[idx];
                let assumption_resolved = apply_meta_subst(assumption, global_subst);
                let assumption_norm = normalize_expr(&assumption_resolved, &theory.rewrites, &mut fuel);
                if assumption_norm == goal_norm {
                    if affine {
                        consumed.insert(idx);
                    }
                    return Ok(());
                }
                if let Ok(sub) = match_expr(&assumption_norm, &goal_norm) {
                    for (k, v) in sub {
                        global_subst.insert(k, v);
                    }
                    if affine {
                        consumed.insert(idx);
                    }
                    return Ok(());
                }
                if let Ok(sub) = match_expr(&goal_norm, &assumption_norm) {
                    for (k, v) in sub {
                        global_subst.insert(k, v);
                    }
                    if affine {
                        consumed.insert(idx);
                    }
                    return Ok(());
                }
            }
            Err(OmegaError::AssumptionMismatch {
                goal: goal_norm,
            })
        }

        Derivation::AssumptionIdx(idx) => {
            if *idx >= ctx.assumptions.len() {
                return Err(OmegaError::MalformedDerivation(format!(
                    "assumption index {} out of bounds (context has {} assumptions)",
                    idx,
                    ctx.assumptions.len()
                )));
            }
            if affine && consumed.contains(idx) {
                return Err(OmegaError::UseAfterMove {
                    index: *idx,
                    expr: ctx.assumptions[*idx].clone(),
                });
            }
            let assumption = apply_meta_subst(&ctx.assumptions[*idx], global_subst);
            let assumption_norm = normalize_expr(&assumption, &theory.rewrites, &mut fuel);
            let goal_resolved = apply_meta_subst(goal, global_subst);
            let goal_norm = normalize_expr(&goal_resolved, &theory.rewrites, &mut fuel);
            if assumption_norm == goal_norm {
                if affine {
                    consumed.insert(*idx);
                }
                return Ok(());
            }
            if let Ok(sub) = match_expr(&goal_norm, &assumption_norm) {
                for (k, v) in sub {
                    global_subst.insert(k, v);
                }
                if affine {
                    consumed.insert(*idx);
                }
                return Ok(());
            }
            if let Ok(sub) = match_expr(&assumption_norm, &goal_norm) {
                for (k, v) in sub {
                    global_subst.insert(k, v);
                }
                if affine {
                    consumed.insert(*idx);
                }
                return Ok(());
            }
            Err(OmegaError::GoalMismatch {
                expected: goal_norm,
                got: assumption_norm,
            })
        }

        Derivation::RuleApp {
            rule_name,
            premises,
        } => {
            // Look up the rule
            let rule_orig = theory
                .get_rule(rule_name)
                .ok_or_else(|| OmegaError::UnknownRule(rule_name.clone()))?;

            // Freshen the rule's meta-variables to avoid collisions
            // when the same rule is used multiple times (e.g., nested eq-trans)
            let rule = freshen_rule(rule_orig, fresh_counter);

            // Check premise count
            if premises.len() != rule.premises.len() {
                return Err(OmegaError::PremiseCountMismatch {
                    rule: rule_name.clone(),
                    expected: rule.premises.len(),
                    got: premises.len(),
                });
            }

            // Match the rule's conclusion against the goal to determine meta-variable bindings.
            // Some metas may remain unsolved (e.g., the "middle" term in eq-trans).
            let goal_resolved = apply_meta_subst(goal, global_subst);
            let goal_norm = normalize_expr(&goal_resolved, &theory.rewrites, &mut fuel);
            let mut local_subst = match match_expr(&rule.conclusion, &goal_norm) {
                Ok(s) => s,
                Err(cause) => {
                    // If the goal itself has metas, try matching the other direction too
                    if goal_norm.has_metas() {
                        match match_expr(&goal_norm, &rule.conclusion) {
                            Ok(s) => s,
                            Err(_) => {
                                return Err(OmegaError::PatternMatchFailed {
                                    rule: rule_name.clone(),
                                    expected: rule.conclusion.clone(),
                                    got: goal_norm,
                                    cause,
                                });
                            }
                        }
                    } else {
                        return Err(OmegaError::PatternMatchFailed {
                            rule: rule_name.clone(),
                            expected: rule.conclusion.clone(),
                            got: goal_norm,
                            cause,
                        });
                    }
                }
            };

            // Now check each premise recursively.
            // If a premise goal still has unsolved metas, we first try to infer
            // the conclusion of the sub-derivation and use it to solve those metas.
            for (i, (premise_derivation, premise_pattern)) in
                premises.iter().zip(rule.premises.iter()).enumerate()
            {
                let mut premise_goal = apply_meta_subst(premise_pattern, &local_subst);
                premise_goal = apply_meta_subst(&premise_goal, global_subst);

                // If the premise goal has unsolved metas, infer the derivation's
                // conclusion first, then unify it with the goal to solve metas,
                // and finally verify the derivation against the now-concrete goal.
                if premise_goal.has_metas() {
                    if let Some(inferred) = infer_conclusion(theory, premise_derivation, ctx, global_subst, fresh_counter) {
                        // Use unification to solve metas in the premise goal
                        let solved = unify_exprs(&premise_goal, &inferred);
                        if let Some(s) = solved {
                            for (k, v) in &s {
                                local_subst.insert(k.clone(), v.clone());
                                global_subst.insert(k.clone(), v.clone());
                            }
                            premise_goal = apply_meta_subst(&premise_goal, &s);
                        }
                    }
                }

                // Build extended context if this premise has context extensions
                let ext_ctx: Context;
                let premise_ctx = {
                    let extensions: Vec<&Expr> = rule.context_extensions.iter()
                        .filter(|(idx, _)| *idx == i)
                        .map(|(_, expr)| expr)
                        .collect();
                    if extensions.is_empty() {
                        ctx
                    } else {
                        let mut new_assumptions = ctx.assumptions.clone();
                        for ext in extensions {
                            let mut resolved = apply_meta_subst(ext, &local_subst);
                            resolved = apply_meta_subst(&resolved, global_subst);
                            new_assumptions.push(resolved);
                        }
                        ext_ctx = Context::with_assumptions(new_assumptions);
                        &ext_ctx
                    }
                };

                check_derivation_inner(
                    theory,
                    &premise_goal,
                    premise_derivation,
                    premise_ctx,
                    global_subst,
                    fresh_counter,
                    consumed,
                )
                .map_err(|e| {
                    OmegaError::MalformedDerivation(format!(
                        "in premise {} of rule {}: {}",
                        i, rule_name, e
                    ))
                })?;

                // After checking a premise, pick up any new meta bindings discovered
                for (k, v) in global_subst.iter() {
                    if !local_subst.contains_key(k) {
                        local_subst.insert(k.clone(), v.clone());
                    }
                }
            }

            // Merge local bindings back into global
            for (k, v) in local_subst {
                global_subst.insert(k, v);
            }

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Expr;
    use crate::judgment::{ConstructorDecl, JudgmentForm, Rule, SortDecl};
    use crate::theory::Theory;

    fn make_prop_logic() -> Theory {
        let mut theory = Theory::new("PropLogic");

        theory.sorts.push(SortDecl {
            name: "Prop".to_string(),
        });

        theory.constructors.push(ConstructorDecl {
            name: "true".to_string(),
            ty: Expr::sym("Prop"),
        });
        theory.constructors.push(ConstructorDecl {
            name: "and".to_string(),
            ty: Expr::app(vec![
                Expr::sym("->"),
                Expr::sym("Prop"),
                Expr::sym("Prop"),
                Expr::sym("Prop"),
            ]),
        });
        theory.constructors.push(ConstructorDecl {
            name: "imp".to_string(),
            ty: Expr::app(vec![
                Expr::sym("->"),
                Expr::sym("Prop"),
                Expr::sym("Prop"),
                Expr::sym("Prop"),
            ]),
        });

        theory.judgments.push(JudgmentForm {
            name: "proves".to_string(),
            pattern: Expr::app(vec![Expr::sym("proves"), Expr::meta("P")]),
            constraints: vec![("P".to_string(), "Prop".to_string())],
        });

        theory.rules.push(Rule {
            name: "and-intro".to_string(),
            premises: vec![
                Expr::app(vec![Expr::sym("proves"), Expr::meta("A")]),
                Expr::app(vec![Expr::sym("proves"), Expr::meta("B")]),
            ],
            conclusion: Expr::app(vec![
                Expr::sym("proves"),
                Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
            ]),
            reflected: false,
            provenance: None,
            implicit_args: vec![],
            context_extensions: vec![],
        });

        theory.rules.push(Rule {
            name: "and-elim-l".to_string(),
            premises: vec![Expr::app(vec![
                Expr::sym("proves"),
                Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
            ])],
            conclusion: Expr::app(vec![Expr::sym("proves"), Expr::meta("A")]),
            reflected: false,
            provenance: None,
            implicit_args: vec![],
            context_extensions: vec![],
        });

        theory.rules.push(Rule {
            name: "and-elim-r".to_string(),
            premises: vec![Expr::app(vec![
                Expr::sym("proves"),
                Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
            ])],
            conclusion: Expr::app(vec![Expr::sym("proves"), Expr::meta("B")]),
            reflected: false,
            provenance: None,
            implicit_args: vec![],
            context_extensions: vec![],
        });

        theory.rules.push(Rule {
            name: "imp-intro".to_string(),
            premises: vec![
                // For imp-intro: if assuming A we can prove B, then we prove A -> B.
                // This rule uses a contextual premise represented as:
                // we add (proves ?A) to context and must derive (proves ?B)
                Expr::app(vec![Expr::sym("proves"), Expr::meta("B")]),
            ],
            conclusion: Expr::app(vec![
                Expr::sym("proves"),
                Expr::app(vec![Expr::sym("imp"), Expr::meta("A"), Expr::meta("B")]),
            ]),
            reflected: false,
            provenance: None,
            implicit_args: vec![],
            context_extensions: vec![],
        });

        theory.rules.push(Rule {
            name: "imp-elim".to_string(),
            premises: vec![
                Expr::app(vec![
                    Expr::sym("proves"),
                    Expr::app(vec![Expr::sym("imp"), Expr::meta("A"), Expr::meta("B")]),
                ]),
                Expr::app(vec![Expr::sym("proves"), Expr::meta("A")]),
            ],
            conclusion: Expr::app(vec![Expr::sym("proves"), Expr::meta("B")]),
            reflected: false,
            provenance: None,
            implicit_args: vec![],
            context_extensions: vec![],
        });

        theory.compute_hash();
        theory
    }

    #[test]
    fn check_and_intro_simple() {
        let theory = make_prop_logic();

        // Prove: (proves (and p q)) from assumptions (proves p) and (proves q)
        let goal = Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::free("p"), Expr::free("q")]),
        ]);

        let derivation = Derivation::RuleApp {
            rule_name: "and-intro".to_string(),
            premises: vec![Derivation::Assumption, Derivation::Assumption],
        };

        let ctx = Context::with_assumptions(vec![
            Expr::app(vec![Expr::sym("proves"), Expr::free("p")]),
            Expr::app(vec![Expr::sym("proves"), Expr::free("q")]),
        ]);

        assert!(check_derivation(&theory, &goal, &derivation, &ctx).is_ok());
    }

    #[test]
    fn check_and_comm() {
        let theory = make_prop_logic();

        // Prove: (proves (and q p)) from assumption (proves (and p q))
        let goal = Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::free("q"), Expr::free("p")]),
        ]);

        let ctx = Context::with_assumptions(vec![Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::free("p"), Expr::free("q")]),
        ])]);

        // Derivation: and-intro(and-elim-r(assumption), and-elim-l(assumption))
        let derivation = Derivation::RuleApp {
            rule_name: "and-intro".to_string(),
            premises: vec![
                Derivation::RuleApp {
                    rule_name: "and-elim-r".to_string(),
                    premises: vec![Derivation::Assumption],
                },
                Derivation::RuleApp {
                    rule_name: "and-elim-l".to_string(),
                    premises: vec![Derivation::Assumption],
                },
            ],
        };

        assert!(check_derivation(&theory, &goal, &derivation, &ctx).is_ok());
    }

    #[test]
    fn reject_wrong_rule() {
        let theory = make_prop_logic();

        // Try to use and-intro with wrong number of premises
        let goal = Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::free("p"), Expr::free("q")]),
        ]);

        let derivation = Derivation::RuleApp {
            rule_name: "and-intro".to_string(),
            premises: vec![Derivation::Assumption], // Only 1 premise, need 2
        };

        let ctx = Context::with_assumptions(vec![
            Expr::app(vec![Expr::sym("proves"), Expr::free("p")]),
        ]);

        assert!(check_derivation(&theory, &goal, &derivation, &ctx).is_err());
    }

    #[test]
    fn reject_unknown_rule() {
        let theory = make_prop_logic();
        let goal = Expr::app(vec![Expr::sym("proves"), Expr::free("p")]);
        let derivation = Derivation::RuleApp {
            rule_name: "nonexistent".to_string(),
            premises: vec![],
        };
        let ctx = Context::new();
        assert!(check_derivation(&theory, &goal, &derivation, &ctx).is_err());
    }
}
