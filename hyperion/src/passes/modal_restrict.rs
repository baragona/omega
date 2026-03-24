//! Modal Substitution Restriction pass: validate and enforce cohesive
//! variable class constraints.
//!
//! In cohesive HoTT, variables belong to modality classes (crisp, flat, sharp).
//! The substitution restriction ensures that terms from one class don't flow
//! into positions reserved for another class.
//!
//! This pass:
//! 1. Assigns modality classes to metavariables based on annotations
//! 2. Validates that no forbidden cross-modal substitutions occur
//! 3. Inserts coercion markers where needed

use apeiron::parser::{Sexp, Span};
use std::collections::HashMap;

/// Modality classes for cohesive type theory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Modality {
    /// Standard variable — no restriction
    Crisp,
    /// Flat modality (♭) — discrete/constant, can be used anywhere
    Flat,
    /// Sharp modality (♯) — codiscrete, restricted substitution
    Sharp,
}

/// A restriction violation found during analysis.
#[derive(Debug, Clone)]
pub struct Violation {
    pub var_name: String,
    pub var_modality: Modality,
    pub context_modality: Modality,
    pub message: String,
}

/// Result of modal restriction checking.
pub struct ModalResult {
    pub rules: Vec<crate::session::VonNeumannRule>,
    pub violations: Vec<Violation>,
    pub coercions_inserted: usize,
}

/// Check and enforce modal restrictions on a set of rules.
/// `modal_annotations` maps metavar names to their declared modality.
pub fn check_modal_restrictions(
    rules: &[crate::session::VonNeumannRule],
    modal_annotations: &HashMap<String, Modality>,
) -> ModalResult {
    let mut transformed = Vec::new();
    let mut violations = Vec::new();
    let mut coercions = 0;

    for rule in rules {
        let (new_lhs, mut lhs_viols, c1) = restrict_sexp(&rule.lhs, modal_annotations, Modality::Crisp);
        let (new_rhs, mut rhs_viols, c2) = restrict_sexp(&rule.rhs, modal_annotations, Modality::Crisp);
        violations.append(&mut lhs_viols);
        violations.append(&mut rhs_viols);
        coercions += c1 + c2;

        transformed.push(crate::session::VonNeumannRule {
            name: rule.name.clone(),
            lhs: new_lhs,
            rhs: new_rhs,
        });
    }

    ModalResult {
        rules: transformed,
        violations,
        coercions_inserted: coercions,
    }
}

/// Parse modality annotations from category morphisms.
/// Looks for patterns like `[flat ?x]` or `[sharp ?x]` in rules.
pub fn extract_modal_annotations(rules: &[crate::session::VonNeumannRule]) -> HashMap<String, Modality> {
    let mut annotations = HashMap::new();
    for rule in rules {
        extract_from_sexp(&rule.lhs, &mut annotations);
        extract_from_sexp(&rule.rhs, &mut annotations);
    }
    annotations
}

fn extract_from_sexp(sexp: &Sexp, annotations: &mut HashMap<String, Modality>) {
    match sexp {
        Sexp::Atom(_, _) => {}
        Sexp::List(items, _) => {
            // Detect [flat ?x] or [sharp ?x]
            if items.len() == 2 {
                if let (Some(head), Some(var)) = (items[0].as_atom(), items[1].as_atom()) {
                    if var.starts_with('?') {
                        let modality = match head {
                            "flat" | "♭" => Some(Modality::Flat),
                            "sharp" | "♯" => Some(Modality::Sharp),
                            "crisp" => Some(Modality::Crisp),
                            _ => None,
                        };
                        if let Some(m) = modality {
                            annotations.insert(var.to_string(), m);
                        }
                    }
                }
            }
            for item in items {
                extract_from_sexp(item, annotations);
            }
        }
    }
}

/// Recursively check modal restrictions, inserting coercions where needed.
fn restrict_sexp(
    sexp: &Sexp,
    annotations: &HashMap<String, Modality>,
    context_modality: Modality,
) -> (Sexp, Vec<Violation>, usize) {
    let sp = sexp.span();
    match sexp {
        Sexp::Atom(name, _) if name.starts_with('?') => {
            if let Some(&var_mod) = annotations.get(name.as_str()) {
                if !is_substitution_allowed(var_mod, context_modality) {
                    let v = Violation {
                        var_name: name.clone(),
                        var_modality: var_mod,
                        context_modality,
                        message: format!(
                            "{:?} variable {} cannot appear in {:?} context",
                            var_mod, name, context_modality
                        ),
                    };
                    // Insert coercion marker instead of rejecting
                    let coerced = Sexp::List(vec![
                        Sexp::Atom("__modal_coerce".to_string(), sp),
                        Sexp::Atom(format!("{:?}", context_modality), sp),
                        sexp.clone(),
                    ], sp);
                    return (coerced, vec![v], 1);
                }
            }
            (sexp.clone(), vec![], 0)
        }
        Sexp::Atom(_, _) => (sexp.clone(), vec![], 0),
        Sexp::List(items, _) => {
            if items.is_empty() {
                return (sexp.clone(), vec![], 0);
            }

            // Detect modality context switches: [flat E] means E is in flat context
            let inner_modality = if let Some(head) = items[0].as_atom() {
                match head {
                    "flat" | "♭" => Modality::Flat,
                    "sharp" | "♯" => Modality::Sharp,
                    _ => context_modality,
                }
            } else {
                context_modality
            };

            let mut new_items = Vec::new();
            let mut all_viols = Vec::new();
            let mut total_coercions = 0;

            for (i, item) in items.iter().enumerate() {
                let ctx = if i == 0 { context_modality } else { inner_modality };
                let (new_item, mut viols, c) = restrict_sexp(item, annotations, ctx);
                new_items.push(new_item);
                all_viols.append(&mut viols);
                total_coercions += c;
            }
            (Sexp::List(new_items, sp), all_viols, total_coercions)
        }
    }
}

/// Check if a variable of `var_mod` can appear in a `context_mod` position.
/// Sharp variables CANNOT be substituted into flat/crisp contexts.
/// Flat variables CAN appear anywhere (they're discrete).
fn is_substitution_allowed(var_mod: Modality, context_mod: Modality) -> bool {
    match (var_mod, context_mod) {
        (_, Modality::Crisp) => true,          // crisp context accepts all
        (Modality::Flat, _) => true,           // flat vars are unrestricted
        (Modality::Crisp, Modality::Flat) => true,
        (Modality::Sharp, Modality::Flat) => false, // sharp can't go to flat
        (Modality::Sharp, Modality::Sharp) => true,
        (Modality::Crisp, Modality::Sharp) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(s: &str) -> Sexp { Sexp::Atom(s.to_string(), Span::default()) }
    fn list(items: Vec<Sexp>) -> Sexp { Sexp::List(items, Span::default()) }
    fn rule(name: &str, lhs: Sexp, rhs: Sexp) -> crate::session::VonNeumannRule {
        crate::session::VonNeumannRule { name: name.to_string(), lhs, rhs }
    }

    #[test]
    fn no_annotations_no_violations() {
        let rules = vec![rule("r1", list(vec![atom("f"), atom("?x")]), atom("?x"))];
        let result = check_modal_restrictions(&rules, &HashMap::new());
        assert!(result.violations.is_empty());
        assert_eq!(result.coercions_inserted, 0);
    }

    #[test]
    fn flat_var_in_crisp_ok() {
        let mut ann = HashMap::new();
        ann.insert("?x".to_string(), Modality::Flat);
        let rules = vec![rule("r1", list(vec![atom("f"), atom("?x")]), atom("?x"))];
        let result = check_modal_restrictions(&rules, &ann);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn sharp_var_in_flat_context_violation() {
        let mut ann = HashMap::new();
        ann.insert("?x".to_string(), Modality::Sharp);
        let rules = vec![rule("r1",
            list(vec![atom("flat"), atom("?x")]),
            atom("ok"))];
        let result = check_modal_restrictions(&rules, &ann);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.coercions_inserted, 1);
        let lhs = format!("{}", result.rules[0].lhs);
        assert!(lhs.contains("__modal_coerce"), "LHS: {}", lhs);
    }

    #[test]
    fn extract_annotations_from_rules() {
        let rules = vec![
            rule("r1",
                list(vec![atom("typeof"), list(vec![atom("flat"), atom("?a")]), atom("?T")]),
                atom("ok")),
            rule("r2",
                list(vec![atom("typeof"), list(vec![atom("sharp"), atom("?b")]), atom("?T")]),
                atom("ok")),
        ];
        let ann = extract_modal_annotations(&rules);
        assert_eq!(ann.get("?a"), Some(&Modality::Flat));
        assert_eq!(ann.get("?b"), Some(&Modality::Sharp));
    }

    // === ABYSSAL: Trojan Horse Closure ===

    #[test]
    fn trojan_horse_sharp_captured_in_closure() {
        // ?x is sharp. [lam ?y [op ?x ?y]] is passed into a flat context.
        // The surface term [lam ...] is not itself annotated, but it CAPTURES
        // a sharp variable. The pass must detect this deep violation.
        let mut ann = HashMap::new();
        ann.insert("?x".to_string(), Modality::Sharp);

        // Rule: [flat [lam ?y [op ?x ?y]]] ==> ok
        // The closure [lam ?y [op ?x ?y]] is in flat context, and ?x is sharp inside.
        let rules = vec![rule("r1",
            list(vec![atom("flat"),
                list(vec![atom("lam"), atom("?y"),
                    list(vec![atom("op"), atom("?x"), atom("?y")])])]),
            atom("ok"))];

        let result = check_modal_restrictions(&rules, &ann);
        assert!(!result.violations.is_empty(),
            "Must detect sharp ?x captured inside closure in flat context. Got 0 violations.");
        assert!(result.violations.iter().any(|v| v.var_name == "?x"),
            "Violation must name ?x as the offending captured variable");
    }

    #[test]
    fn trojan_horse_nested_closure_depth() {
        // Sharp var buried 3 levels deep inside nested lambdas in flat context
        let mut ann = HashMap::new();
        ann.insert("?s".to_string(), Modality::Sharp);

        // [flat [f [g [h ?s]]]]
        let rules = vec![rule("r1",
            list(vec![atom("flat"),
                list(vec![atom("f"),
                    list(vec![atom("g"),
                        list(vec![atom("h"), atom("?s")])])])]),
            atom("ok"))];

        let result = check_modal_restrictions(&rules, &ann);
        assert!(!result.violations.is_empty(),
            "Must detect sharp ?s buried 3 levels deep in flat context. Got 0 violations.");
    }

    #[test]
    fn substitution_rules() {
        assert!(is_substitution_allowed(Modality::Flat, Modality::Sharp));
        assert!(is_substitution_allowed(Modality::Flat, Modality::Flat));
        assert!(is_substitution_allowed(Modality::Flat, Modality::Crisp));
        assert!(!is_substitution_allowed(Modality::Sharp, Modality::Flat));
        assert!(is_substitution_allowed(Modality::Sharp, Modality::Sharp));
        assert!(is_substitution_allowed(Modality::Sharp, Modality::Crisp));
    }
}
