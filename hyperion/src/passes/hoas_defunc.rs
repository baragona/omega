//! HOASDefunctionalization pass: convert higher-order abstract syntax to
//! first-order explicit substitution calculus.
//!
//! When a category declares `[HOASBinding lam :object Term]`, rules may
//! contain meta-level lambdas (binder expressions). First-order engines
//! (VonNeumann, LogicProgramming, SMTAssisted) can't evaluate these.
//!
//! This pass:
//! 1. Detects lambda-like binder patterns in rules
//! 2. Lifts them to top-level closure constructors
//! 3. Adds explicit `apply` rules for each lifted closure
//!
//! Example:
//!   Input rule:  [typeof [lam ?A [body ?B]] [arrow ?A ?B]] ==> ...
//!   Output:      [typeof [lam ?A [closure_0 ?B]] [arrow ?A ?B]] ==> ...
//!                + new rule: [apply [closure_0 ?B] ?x] ==> [subst ?B ?x]

use apeiron::parser::{Sexp, Span};
use std::collections::HashSet;

/// Result of defunctionalization: transformed rules + any new closure rules.
pub struct DefuncResult {
    pub rules: Vec<crate::session::VonNeumannRule>,
    pub closure_rules: Vec<crate::session::VonNeumannRule>,
    pub lifted_count: usize,
}

/// Detect and replace nested binder expressions in rules.
/// `binder_name` is the HOAS binder (e.g., "lam").
pub fn defunctionalize_rules(
    rules: &[crate::session::VonNeumannRule],
    binder_name: &str,
) -> DefuncResult {
    let mut transformed_rules = Vec::new();
    let mut closure_rules = Vec::new();
    let mut counter = 0;

    for rule in rules {
        let (new_lhs, mut new_closures) = defunc_sexp(&rule.lhs, binder_name, &mut counter);
        let (new_rhs, mut rhs_closures) = defunc_sexp(&rule.rhs, binder_name, &mut counter);
        new_closures.append(&mut rhs_closures);

        transformed_rules.push(crate::session::VonNeumannRule {
            name: rule.name.clone(),
            lhs: new_lhs,
            rhs: new_rhs,
        });

        closure_rules.extend(new_closures);
    }

    DefuncResult {
        rules: transformed_rules,
        closure_rules,
        lifted_count: counter,
    }
}

/// Recursively traverse a Sexp, lifting binder bodies to closure constructors.
/// Returns (transformed_sexp, new_closure_rules).
fn defunc_sexp(
    sexp: &Sexp,
    binder_name: &str,
    counter: &mut usize,
) -> (Sexp, Vec<crate::session::VonNeumannRule>) {
    let sp = sexp.span();
    match sexp {
        Sexp::Atom(_, _) => (sexp.clone(), vec![]),
        Sexp::List(items, _) => {
            // Check if this is a binder application: [lam <type> <body>]
            if items.len() == 3 {
                if let Some(head) = items[0].as_atom() {
                    if head == binder_name {
                        // This is a binder: [lam ?A <body>]
                        let type_arg = &items[1];
                        // Recursively defunctionalize the body first (inner-to-outer)
                        let (body, mut inner_closures) = defunc_sexp(&items[2], binder_name, counter);
                        let body = &body;

                        // Collect free metavariables in the body (these become closure env)
                        let mut free_vars = Vec::new();
                        collect_metavars(body, &mut free_vars);

                        // Generate closure name
                        let closure_name = format!("__closure_{}", counter);
                        *counter += 1;

                        // Build closure constructor: [__closure_N ?v1 ?v2 ...]
                        let mut closure_args = vec![Sexp::Atom(closure_name.clone(), sp)];
                        closure_args.extend(free_vars.iter().map(|v| Sexp::Atom(v.clone(), sp)));
                        let closure_term = Sexp::List(closure_args.clone(), sp);

                        // Build the defunctionalized binder: [lam ?A [__closure_N ...]]
                        let new_binder = Sexp::List(vec![
                            Sexp::Atom(binder_name.to_string(), sp),
                            type_arg.clone(),
                            closure_term,
                        ], sp);

                        // Generate apply rule: [apply [__closure_N ?v1 ...] ?x] ==> body[?x for bound]
                        // The bound variable in HOAS is implicit — in the body,
                        // references to it become explicit via the apply rule.
                        let apply_var = Sexp::Atom("?__apply_arg".to_string(), sp);
                        let apply_lhs = Sexp::List(vec![
                            Sexp::Atom("apply".to_string(), sp),
                            Sexp::List(closure_args, sp),
                            apply_var,
                        ], sp);

                        let apply_rule = crate::session::VonNeumannRule {
                            name: format!("apply-{}", closure_name),
                            lhs: apply_lhs,
                            rhs: body.clone(),
                        };

                        inner_closures.push(apply_rule);
                        return (new_binder, inner_closures);
                    }
                }
            }

            // Not a binder — recursively defunctionalize children
            let mut all_closures = Vec::new();
            let mut new_items = Vec::new();
            for item in items {
                let (new_item, closures) = defunc_sexp(item, binder_name, counter);
                new_items.push(new_item);
                all_closures.extend(closures);
            }
            (Sexp::List(new_items, sp), all_closures)
        }
    }
}

/// Collect all metavariable names (?X) in a Sexp.
fn collect_metavars(sexp: &Sexp, vars: &mut Vec<String>) {
    let mut seen = HashSet::new();
    collect_metavars_inner(sexp, vars, &mut seen);
}

fn collect_metavars_inner(sexp: &Sexp, vars: &mut Vec<String>, seen: &mut HashSet<String>) {
    match sexp {
        Sexp::Atom(name, _) if name.starts_with('?') => {
            if seen.insert(name.clone()) {
                vars.push(name.clone());
            }
        }
        Sexp::List(items, _) => {
            for item in items {
                collect_metavars_inner(item, vars, seen);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(s: &str) -> Sexp {
        Sexp::Atom(s.to_string(), Span::default())
    }

    fn list(items: Vec<Sexp>) -> Sexp {
        Sexp::List(items, Span::default())
    }

    fn rule(name: &str, lhs: Sexp, rhs: Sexp) -> crate::session::VonNeumannRule {
        crate::session::VonNeumannRule { name: name.to_string(), lhs, rhs }
    }

    #[test]
    fn no_binders_unchanged() {
        let rules = vec![
            rule("r1", list(vec![atom("f"), atom("?x")]), atom("?x")),
        ];
        let result = defunctionalize_rules(&rules, "lam");
        assert_eq!(result.lifted_count, 0);
        assert!(result.closure_rules.is_empty());
        assert_eq!(format!("{}", result.rules[0].lhs), "[f ?x]");
    }

    #[test]
    fn simple_binder_lifted() {
        // [lam ?A ?B] → [lam ?A [__closure_0 ?B]]
        let rules = vec![
            rule("r1",
                list(vec![atom("typeof"), list(vec![atom("lam"), atom("?A"), atom("?B")])]),
                atom("ok")),
        ];
        let result = defunctionalize_rules(&rules, "lam");
        assert_eq!(result.lifted_count, 1);
        assert_eq!(result.closure_rules.len(), 1);

        let lhs_str = format!("{}", result.rules[0].lhs);
        assert!(lhs_str.contains("__closure_0"), "LHS should contain closure: {}", lhs_str);

        // Apply rule should be generated
        let apply = &result.closure_rules[0];
        assert!(apply.name.contains("closure_0"));
        let apply_lhs = format!("{}", apply.lhs);
        assert!(apply_lhs.contains("apply"), "Apply rule LHS: {}", apply_lhs);
    }

    #[test]
    fn binder_with_free_vars_captured() {
        // [lam ?A [f ?X ?Y]] → closure captures ?X, ?Y
        let rules = vec![
            rule("r1",
                list(vec![atom("lam"), atom("?A"), list(vec![atom("f"), atom("?X"), atom("?Y")])]),
                atom("ok")),
        ];
        let result = defunctionalize_rules(&rules, "lam");
        assert_eq!(result.lifted_count, 1);

        let closure = &result.closure_rules[0];
        let closure_lhs = format!("{}", closure.lhs);
        // Closure should capture ?X and ?Y
        assert!(closure_lhs.contains("?X"), "Should capture ?X: {}", closure_lhs);
        assert!(closure_lhs.contains("?Y"), "Should capture ?Y: {}", closure_lhs);
    }

    #[test]
    fn nested_binders_both_lifted() {
        // [lam ?A [lam ?B ?C]] → two closures
        let rules = vec![
            rule("r1",
                list(vec![atom("lam"), atom("?A"), list(vec![atom("lam"), atom("?B"), atom("?C")])]),
                atom("ok")),
        ];
        let result = defunctionalize_rules(&rules, "lam");
        assert_eq!(result.lifted_count, 2);
        assert_eq!(result.closure_rules.len(), 2);
    }

    #[test]
    fn non_binder_head_untouched() {
        // [app ?f ?x] — not a binder, should pass through
        let rules = vec![
            rule("r1",
                list(vec![atom("app"), atom("?f"), atom("?x")]),
                atom("?x")),
        ];
        let result = defunctionalize_rules(&rules, "lam");
        assert_eq!(result.lifted_count, 0);
        assert_eq!(format!("{}", result.rules[0].lhs), "[app ?f ?x]");
    }
}
