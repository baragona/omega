//! Tactic engine for backward-reasoning proof construction.
//!
//! Tactics transform a **goal stack** — a list of open proof obligations.
//! Each tactic step consumes the top goal and may produce zero or more subgoals.
//! A proof is complete when the goal stack is empty.

use std::collections::HashMap;

use crate::egraph;
use crate::judgment::DerivRule;
use crate::parser::Sexp;
use crate::refute;
use crate::system::RewriteRule;

/// A single tactic step.
#[derive(Debug, Clone)]
pub enum TacticStep {
    /// Apply a named derive rule: match conclusion against goal, push premises as subgoals.
    Apply { rule_name: String },
    /// Automatic backward-chaining search up to given depth.
    Auto { depth: usize },
    /// Discharge goal from assumptions (exact match).
    Assumption,
    /// Provide an exact term matching the goal.
    Exact { term: Sexp },
    /// Use a previously proved lemma (by name) to discharge the goal.
    Cut { lemma_name: String },
    /// Intro: if goal is a judgment with universally quantified structure,
    /// move the outermost binder into assumptions.
    Intro { name: Option<String> },
    /// Try to close the top goal via e-graph equality saturation.
    /// The goal must be of the form `[head lhs rhs]` where lhs ≡ rhs.
    EGraph,
}

/// The state of a tactic proof.
#[derive(Debug, Clone)]
pub struct ProofState {
    /// Open goals remaining to be proved.
    pub goals: Vec<Goal>,
    /// Accumulated assumptions (from intro, cut, etc.).
    pub assumptions: Vec<Sexp>,
}

/// A single goal (proof obligation).
#[derive(Debug, Clone)]
pub struct Goal {
    pub judgment: Sexp,
}

/// Result of running a tactic.
#[derive(Debug)]
pub enum TacticResult {
    /// All goals discharged — proof complete.
    Complete,
    /// Goals remain after all steps.
    Incomplete {
        remaining: Vec<Goal>,
    },
    /// A tactic step failed.
    Failed {
        step_index: usize,
        step: String,
        detail: String,
    },
}

/// Run a sequence of tactic steps against a goal.
/// `graph_rules` enables e-graph tactics when the system uses equality-saturation.
pub fn run_tactics(
    initial_goal: Sexp,
    steps: &[TacticStep],
    derive_rules: &[DerivRule],
    assumptions: &[Sexp],
    graph_rules: Option<&[RewriteRule]>,
) -> TacticResult {
    let mut state = ProofState {
        goals: vec![Goal { judgment: initial_goal }],
        assumptions: assumptions.to_vec(),
    };

    for (i, step) in steps.iter().enumerate() {
        if state.goals.is_empty() {
            return TacticResult::Complete;
        }

        match apply_step(&mut state, step, derive_rules, graph_rules) {
            Ok(()) => {}
            Err(detail) => {
                return TacticResult::Failed {
                    step_index: i,
                    step: format!("{:?}", step),
                    detail,
                };
            }
        }
    }

    if state.goals.is_empty() {
        TacticResult::Complete
    } else {
        TacticResult::Incomplete {
            remaining: state.goals,
        }
    }
}

/// Apply a single tactic step, mutating the proof state.
fn apply_step(
    state: &mut ProofState,
    step: &TacticStep,
    derive_rules: &[DerivRule],
    graph_rules: Option<&[RewriteRule]>,
) -> Result<(), String> {
    match step {
        TacticStep::Apply { rule_name } => apply_rule(state, rule_name, derive_rules),
        TacticStep::Auto { depth } => apply_auto(state, *depth, derive_rules, graph_rules),
        TacticStep::Assumption => apply_assumption(state),
        TacticStep::Exact { term } => apply_exact(state, term),
        TacticStep::Cut { lemma_name } => apply_cut(state, lemma_name, derive_rules),
        TacticStep::Intro { name } => apply_intro(state, name.as_deref()),
        TacticStep::EGraph => apply_egraph(state, graph_rules),
    }
}

/// `apply rule-name`: Match the named rule's conclusion against the top goal.
/// On success, replace the goal with the rule's premises.
fn apply_rule(
    state: &mut ProofState,
    rule_name: &str,
    derive_rules: &[DerivRule],
) -> Result<(), String> {
    let rule = derive_rules
        .iter()
        .find(|r| r.name == rule_name)
        .ok_or_else(|| format!("unknown rule '{}'", rule_name))?;

    let goal = state.goals.remove(0);

    let mut bindings = HashMap::new();
    if !match_sexp_pattern(&rule.conclusion, &goal.judgment, &mut bindings) {
        let msg = format!(
            "rule '{}' conclusion {} does not match goal {}",
            rule_name, rule.conclusion, goal.judgment
        );
        state.goals.insert(0, goal);
        return Err(msg);
    }

    // Push premises as new subgoals (in order, at the front)
    let new_goals: Vec<Goal> = rule
        .premises
        .iter()
        .map(|p| Goal {
            judgment: substitute_sexp(p, &bindings),
        })
        .collect();

    // Insert new goals at front (leftmost premise first)
    for (j, g) in new_goals.into_iter().enumerate() {
        state.goals.insert(j, g);
    }

    Ok(())
}

/// `auto N`: Try to discharge the top goal via backward-chaining search.
/// Falls back to e-graph equality saturation if graph_rules are available.
fn apply_auto(
    state: &mut ProofState,
    depth: usize,
    derive_rules: &[DerivRule],
    graph_rules: Option<&[RewriteRule]>,
) -> Result<(), String> {
    if state.goals.is_empty() {
        return Ok(());
    }

    let result = refute::exhaustive_refute(
        derive_rules,
        &state.assumptions,
        &state.goals[0].judgment,
        depth,
        100_000,
        false,
    );

    match result {
        refute::RefuteResult::Derivable => {
            state.goals.remove(0);
            Ok(())
        }
        refute::RefuteResult::Refuted { .. } | refute::RefuteResult::Inconclusive { .. } => {
            // E-graph fallback: if the goal is a 3-element list [head lhs rhs],
            // try to prove lhs ≡ rhs via equality saturation.
            if let Some(rules) = graph_rules {
                if try_egraph_equality(state, rules) {
                    return Ok(());
                }
            }
            let goal_str = format!("{}", state.goals[0].judgment);
            Err(format!("auto: goal {} is not derivable at depth {}", goal_str, depth))
        }
    }
}

/// Try to close the top goal via e-graph equality saturation.
/// The goal must be of the form `[head lhs rhs]` where lhs and rhs
/// can be proved equal via the e-graph.
fn try_egraph_equality(state: &mut ProofState, graph_rules: &[RewriteRule]) -> bool {
    if state.goals.is_empty() {
        return false;
    }
    let goal = &state.goals[0];
    if let Some(items) = goal.judgment.as_list() {
        if items.len() == 3 {
            let lhs = &items[1];
            let rhs = &items[2];
            let empty: Vec<String> = vec![];
            let filtered = egraph::filter_barrier_rules(graph_rules, &empty);
            let result = egraph::check_equal_egraph(lhs, rhs, &filtered, egraph::EGraphFuel::default());
            if result == egraph::EGraphResult::Equal {
                state.goals.remove(0);
                return true;
            }
        }
    }
    false
}

/// `egraph`: Explicitly try to close the top goal via e-graph equality saturation.
fn apply_egraph(
    state: &mut ProofState,
    graph_rules: Option<&[RewriteRule]>,
) -> Result<(), String> {
    if state.goals.is_empty() {
        return Ok(());
    }
    let rules = graph_rules.ok_or_else(|| {
        "egraph: no equality-saturation rules available (system does not use equality-saturation mode)".to_string()
    })?;
    if try_egraph_equality(state, rules) {
        Ok(())
    } else {
        let goal = &state.goals[0];
        Err(format!("egraph: could not prove goal {} via equality saturation", goal.judgment))
    }
}

/// `assumption`: Discharge top goal if it matches an assumption.
fn apply_assumption(state: &mut ProofState) -> Result<(), String> {
    if state.goals.is_empty() {
        return Ok(());
    }

    let goal = &state.goals[0];
    for assumption in &state.assumptions {
        if sexp_eq(assumption, &goal.judgment) {
            state.goals.remove(0);
            return Ok(());
        }
    }

    Err(format!(
        "assumption: goal {} does not match any assumption",
        goal.judgment
    ))
}

/// `exact term`: Discharge top goal if the term matches it exactly.
fn apply_exact(state: &mut ProofState, term: &Sexp) -> Result<(), String> {
    if state.goals.is_empty() {
        return Ok(());
    }

    let goal = &state.goals[0];
    if sexp_eq(term, &goal.judgment) {
        state.goals.remove(0);
        Ok(())
    } else {
        Err(format!(
            "exact: term {} does not match goal {}",
            term, goal.judgment
        ))
    }
}

/// `cut lemma-name`: Find a 0-premise derive rule (lemma) and add its conclusion
/// as an assumption, then try to discharge the top goal from it.
fn apply_cut(
    state: &mut ProofState,
    lemma_name: &str,
    derive_rules: &[DerivRule],
) -> Result<(), String> {
    let rule = derive_rules
        .iter()
        .find(|r| r.name == lemma_name || r.name == format!("__lemma_{}", lemma_name))
        .ok_or_else(|| format!("unknown lemma '{}'", lemma_name))?;

    if !rule.premises.is_empty() {
        return Err(format!(
            "cut: '{}' has premises — only 0-premise lemmas can be cut",
            lemma_name
        ));
    }

    // Add the lemma's conclusion as an assumption
    state.assumptions.push(rule.conclusion.clone());
    Ok(())
}

/// `intro name`: Move the top-level structure of a judgment goal into assumptions.
/// For a goal like [J [f x] result], intro adds [f x] as an assumption
/// and replaces the goal with [J result].
fn apply_intro(
    state: &mut ProofState,
    _name: Option<&str>,
) -> Result<(), String> {
    if state.goals.is_empty() {
        return Ok(());
    }

    let goal = &state.goals[0];
    if let Some(items) = goal.judgment.as_list() {
        if items.len() >= 3 {
            // Move the second element (first argument after judgment name) into assumptions
            let assumption = items[1].clone();
            state.assumptions.push(assumption);

            // Reconstruct goal without the intro'd argument
            let s = crate::parser::Span::default();
            let mut new_items = vec![items[0].clone()];
            new_items.extend_from_slice(&items[2..]);
            let new_goal = if new_items.len() == 1 {
                new_items.into_iter().next().unwrap()
            } else {
                Sexp::List(new_items, s)
            };

            state.goals[0] = Goal { judgment: new_goal };
            return Ok(());
        }
    }

    Err("intro: goal is not a multi-argument judgment".into())
}

// ── Pattern matching (reuse refute's logic) ──

fn match_sexp_pattern(
    pattern: &Sexp,
    concrete: &Sexp,
    bindings: &mut HashMap<String, Sexp>,
) -> bool {
    match pattern {
        Sexp::Atom(name, _) if name.starts_with('?') => {
            if let Some(existing) = bindings.get(name) {
                format!("{}", existing) == format!("{}", concrete)
            } else {
                bindings.insert(name.clone(), concrete.clone());
                true
            }
        }
        Sexp::Atom(name, _) => concrete.as_atom().map_or(false, |c| c == name),
        Sexp::List(pitems, _) => {
            if let Some(citems) = concrete.as_list() {
                if pitems.len() != citems.len() {
                    return false;
                }
                pitems.iter().zip(citems.iter()).all(|(p, c)| match_sexp_pattern(p, c, bindings))
            } else {
                false
            }
        }
    }
}

fn substitute_sexp(sexp: &Sexp, bindings: &HashMap<String, Sexp>) -> Sexp {
    match sexp {
        Sexp::Atom(name, _) if name.starts_with('?') => {
            bindings.get(name).cloned().unwrap_or_else(|| sexp.clone())
        }
        Sexp::Atom(_, _) => sexp.clone(),
        Sexp::List(items, span) => {
            let new_items: Vec<Sexp> = items.iter().map(|i| substitute_sexp(i, bindings)).collect();
            Sexp::List(new_items, *span)
        }
    }
}

fn sexp_eq(a: &Sexp, b: &Sexp) -> bool {
    format!("{}", a) == format!("{}", b)
}

/// Parse tactic steps from a list of Sexps.
/// Each step is a bracketed command like [apply rule-name] or [auto 3].
pub fn parse_tactic_steps(items: &[Sexp]) -> Result<Vec<TacticStep>, String> {
    let mut steps = Vec::new();
    for item in items {
        if let Some(parts) = item.as_list() {
            if parts.is_empty() {
                continue;
            }
            let head = parts[0].as_atom().unwrap_or("");
            match head {
                "apply" => {
                    let name = parts
                        .get(1)
                        .and_then(|s| s.as_atom())
                        .ok_or("apply: missing rule name")?
                        .to_string();
                    steps.push(TacticStep::Apply { rule_name: name });
                }
                "auto" => {
                    let depth = parts
                        .get(1)
                        .and_then(|s| s.as_atom())
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(5);
                    steps.push(TacticStep::Auto { depth });
                }
                "assumption" => {
                    steps.push(TacticStep::Assumption);
                }
                "exact" => {
                    let term = parts
                        .get(1)
                        .ok_or("exact: missing term")?
                        .clone();
                    steps.push(TacticStep::Exact { term });
                }
                "cut" => {
                    let name = parts
                        .get(1)
                        .and_then(|s| s.as_atom())
                        .ok_or("cut: missing lemma name")?
                        .to_string();
                    steps.push(TacticStep::Cut { lemma_name: name });
                }
                "intro" => {
                    let name = parts.get(1).and_then(|s| s.as_atom()).map(|s| s.to_string());
                    steps.push(TacticStep::Intro { name });
                }
                "egraph" => {
                    steps.push(TacticStep::EGraph);
                }
                _ => {
                    return Err(format!(
                        "unknown tactic step '{}' — expected: apply, auto, assumption, exact, cut, intro, egraph",
                        head
                    ));
                }
            }
        }
    }
    Ok(steps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn parse_sexp(s: &str) -> Sexp {
        parser::parse(s).unwrap().into_iter().next().unwrap()
    }

    fn make_rule(name: &str, premises: &[&str], conclusion: &str) -> DerivRule {
        DerivRule {
            name: name.to_string(),
            premises: premises.iter().map(|p| parse_sexp(p)).collect(),
            conclusion: parse_sexp(conclusion),
            absurd: false,
        }
    }

    #[test]
    fn test_apply_rule_no_premises() {
        let rule = make_rule("ax-true", &[], "[holds true true]");
        let goal = parse_sexp("[holds true true]");
        let result = run_tactics(goal, &[TacticStep::Apply { rule_name: "ax-true".into() }], &[rule], &[], None);
        assert!(matches!(result, TacticResult::Complete));
    }

    #[test]
    fn test_apply_rule_with_premises() {
        let rules = vec![
            make_rule("mp", &["[holds ?A [implies ?A ?B]]", "[holds ?A ?A]"], "[holds ?A ?B]"),
            make_rule("ax-p", &[], "[holds p p]"),
            make_rule("ax-impl", &[], "[holds p [implies p q]]"),
        ];
        let goal = parse_sexp("[holds p q]");
        let steps = vec![
            TacticStep::Apply { rule_name: "mp".into() },
            TacticStep::Auto { depth: 3 },
            TacticStep::Auto { depth: 3 },
        ];
        let result = run_tactics(goal, &steps, &rules, &[], None);
        assert!(matches!(result, TacticResult::Complete));
    }

    #[test]
    fn test_assumption() {
        let goal = parse_sexp("[holds a a]");
        let assumptions = vec![parse_sexp("[holds a a]")];
        let result = run_tactics(
            goal,
            &[TacticStep::Assumption],
            &[],
            &assumptions,
            None,
        );
        assert!(matches!(result, TacticResult::Complete));
    }

    #[test]
    fn test_auto() {
        let rules = vec![
            make_rule("ax-a", &[], "[holds a a]"),
        ];
        let goal = parse_sexp("[holds a a]");
        let result = run_tactics(goal, &[TacticStep::Auto { depth: 3 }], &rules, &[], None);
        assert!(matches!(result, TacticResult::Complete));
    }

    #[test]
    fn test_failed_tactic() {
        let goal = parse_sexp("[holds a a]");
        let result = run_tactics(goal, &[TacticStep::Assumption], &[], &[], None);
        assert!(matches!(result, TacticResult::Failed { .. }));
    }

    #[test]
    fn test_parse_tactic_steps() {
        let source = "[[apply mp] [auto 5] [assumption] [intro x] [cut lemma1] [exact [holds a a]]]";
        let sexp = parse_sexp(source);
        let items = sexp.as_list().unwrap();
        let steps = parse_tactic_steps(items).unwrap();
        assert_eq!(steps.len(), 6);
        assert!(matches!(&steps[0], TacticStep::Apply { rule_name } if rule_name == "mp"));
        assert!(matches!(&steps[1], TacticStep::Auto { depth: 5 }));
        assert!(matches!(&steps[2], TacticStep::Assumption));
        assert!(matches!(&steps[3], TacticStep::Intro { name: Some(n) } if n == "x"));
        assert!(matches!(&steps[4], TacticStep::Cut { lemma_name } if lemma_name == "lemma1"));
        assert!(matches!(&steps[5], TacticStep::Exact { .. }));
    }
}
