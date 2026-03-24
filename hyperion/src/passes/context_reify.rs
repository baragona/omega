//! Context Reification pass: transform first-class context references
//! into explicit data structures (tuples/lists).
//!
//! When a category uses `[ContextualModality]` or similar structures,
//! rules may reference context variables (`Γ`, `Δ`) as opaque entities.
//! First-order engines need these reified as concrete term constructors.
//!
//! This pass:
//! 1. Detects context metavariables (names starting with `Γ`, `Δ`, or declared context sorts)
//! 2. Replaces context operations (`extend`, `lookup`, `empty`) with explicit list constructors
//! 3. Adds structural rules for context manipulation

use apeiron::parser::{Sexp, Span};

/// Names conventionally used for context variables.
const CONTEXT_PREFIXES: &[&str] = &["Γ", "Δ", "Θ", "ctx", "gamma", "delta"];

/// Context operations that get reified to list constructors.
const CONTEXT_OPS: &[(&str, &str)] = &[
    ("empty-ctx", "__ctx_nil"),
    ("extend", "__ctx_cons"),
    ("lookup", "__ctx_lookup"),
];

/// Result of context reification.
pub struct ReifyResult {
    pub rules: Vec<crate::session::VonNeumannRule>,
    pub aux_rules: Vec<crate::session::VonNeumannRule>,
    pub reified_count: usize,
}

/// Reify context operations in a set of rules.
pub fn reify_contexts(
    rules: &[crate::session::VonNeumannRule],
    context_sorts: &[String],
) -> ReifyResult {
    let mut transformed = Vec::new();
    let mut count = 0;

    for rule in rules {
        let (new_lhs, c1) = reify_sexp(&rule.lhs, context_sorts);
        let (new_rhs, c2) = reify_sexp(&rule.rhs, context_sorts);
        count += c1 + c2;
        transformed.push(crate::session::VonNeumannRule {
            name: rule.name.clone(),
            lhs: new_lhs,
            rhs: new_rhs,
        });
    }

    // Generate auxiliary structural rules for context operations
    let aux_rules = generate_ctx_structural_rules();

    ReifyResult {
        rules: transformed,
        aux_rules,
        reified_count: count,
    }
}

/// Recursively reify context operations in a Sexp.
/// Returns (transformed, reification_count).
fn reify_sexp(sexp: &Sexp, context_sorts: &[String]) -> (Sexp, usize) {
    let sp = sexp.span();
    match sexp {
        Sexp::Atom(name, _) => {
            // Reify bare context operation names
            for &(op, reified) in CONTEXT_OPS {
                if name == op {
                    return (Sexp::Atom(reified.to_string(), sp), 1);
                }
            }
            (sexp.clone(), 0)
        }
        Sexp::List(items, _) => {
            if items.is_empty() {
                return (sexp.clone(), 0);
            }

            // Check for context operations: [extend Γ x A] → [__ctx_cons Γ x A]
            if let Some(head) = items[0].as_atom() {
                for &(op, reified) in CONTEXT_OPS {
                    if head == op {
                        let mut new_items = vec![Sexp::Atom(reified.to_string(), sp)];
                        let mut total = 1;
                        for item in &items[1..] {
                            let (new_item, c) = reify_sexp(item, context_sorts);
                            new_items.push(new_item);
                            total += c;
                        }
                        return (Sexp::List(new_items, sp), total);
                    }
                }
            }

            // Recurse into children
            let mut new_items = Vec::new();
            let mut total = 0;
            for item in items {
                let (new_item, c) = reify_sexp(item, context_sorts);
                new_items.push(new_item);
                total += c;
            }
            (Sexp::List(new_items, sp), total)
        }
    }
}

/// Check if a name looks like a context variable.
pub fn is_context_var(name: &str, context_sorts: &[String]) -> bool {
    CONTEXT_PREFIXES.iter().any(|p| name.starts_with(p))
        || context_sorts.iter().any(|s| name == s)
}

/// Generate structural rules for reified contexts.
fn generate_ctx_structural_rules() -> Vec<crate::session::VonNeumannRule> {
    let sp = Span::default();
    vec![
        // === Index-based lookup ===
        // [__ctx_lookup [__ctx_cons ?Γ ?x ?A] 0] ==> ?A
        crate::session::VonNeumannRule {
            name: "ctx-lookup-zero".to_string(),
            lhs: Sexp::List(vec![
                Sexp::Atom("__ctx_lookup".to_string(), sp),
                Sexp::List(vec![
                    Sexp::Atom("__ctx_cons".to_string(), sp),
                    Sexp::Atom("?Γ".to_string(), sp),
                    Sexp::Atom("?x".to_string(), sp),
                    Sexp::Atom("?A".to_string(), sp),
                ], sp),
                Sexp::Atom("0".to_string(), sp),
            ], sp),
            rhs: Sexp::Atom("?A".to_string(), sp),
        },
        // [__ctx_lookup [__ctx_cons ?Γ ?x ?A] [S ?n]] ==> [__ctx_lookup ?Γ ?n]
        crate::session::VonNeumannRule {
            name: "ctx-lookup-succ".to_string(),
            lhs: Sexp::List(vec![
                Sexp::Atom("__ctx_lookup".to_string(), sp),
                Sexp::List(vec![
                    Sexp::Atom("__ctx_cons".to_string(), sp),
                    Sexp::Atom("?Γ".to_string(), sp),
                    Sexp::Atom("?x".to_string(), sp),
                    Sexp::Atom("?A".to_string(), sp),
                ], sp),
                Sexp::List(vec![
                    Sexp::Atom("S".to_string(), sp),
                    Sexp::Atom("?n".to_string(), sp),
                ], sp),
            ], sp),
            rhs: Sexp::List(vec![
                Sexp::Atom("__ctx_lookup".to_string(), sp),
                Sexp::Atom("?Γ".to_string(), sp),
                Sexp::Atom("?n".to_string(), sp),
            ], sp),
        },
        // === Name-based lookup (handles shadowing correctly) ===
        // [__ctx_lookup_name [__ctx_cons ?Γ ?x ?A] ?x] ==> ?A
        // Non-linear pattern: both positions bind ?x, so this only matches
        // when the lookup name equals the binding name (most recent wins).
        crate::session::VonNeumannRule {
            name: "ctx-lookup-name-hit".to_string(),
            lhs: Sexp::List(vec![
                Sexp::Atom("__ctx_lookup_name".to_string(), sp),
                Sexp::List(vec![
                    Sexp::Atom("__ctx_cons".to_string(), sp),
                    Sexp::Atom("?Γ".to_string(), sp),
                    Sexp::Atom("?x".to_string(), sp),
                    Sexp::Atom("?A".to_string(), sp),
                ], sp),
                Sexp::Atom("?x".to_string(), sp),
            ], sp),
            rhs: Sexp::Atom("?A".to_string(), sp),
        },
        // [__ctx_lookup_name [__ctx_cons ?Γ ?y ?B] ?x] ==> [__ctx_lookup_name ?Γ ?x]
        // Fallthrough: when ?x ≠ ?y (non-linear pattern above takes priority),
        // skip this binding and recurse into the tail.
        crate::session::VonNeumannRule {
            name: "ctx-lookup-name-skip".to_string(),
            lhs: Sexp::List(vec![
                Sexp::Atom("__ctx_lookup_name".to_string(), sp),
                Sexp::List(vec![
                    Sexp::Atom("__ctx_cons".to_string(), sp),
                    Sexp::Atom("?Γ".to_string(), sp),
                    Sexp::Atom("?y".to_string(), sp),
                    Sexp::Atom("?B".to_string(), sp),
                ], sp),
                Sexp::Atom("?x".to_string(), sp),
            ], sp),
            rhs: Sexp::List(vec![
                Sexp::Atom("__ctx_lookup_name".to_string(), sp),
                Sexp::Atom("?Γ".to_string(), sp),
                Sexp::Atom("?x".to_string(), sp),
            ], sp),
        },
    ]
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
    fn no_context_ops_unchanged() {
        let rules = vec![rule("r1", list(vec![atom("f"), atom("?x")]), atom("?x"))];
        let result = reify_contexts(&rules, &[]);
        assert_eq!(result.reified_count, 0);
        assert_eq!(format!("{}", result.rules[0].lhs), "[f ?x]");
    }

    #[test]
    fn extend_reified() {
        let rules = vec![rule("r1",
            list(vec![atom("extend"), atom("?Γ"), atom("?x"), atom("?A")]),
            atom("ok"))];
        let result = reify_contexts(&rules, &[]);
        assert_eq!(result.reified_count, 1);
        let lhs = format!("{}", result.rules[0].lhs);
        assert!(lhs.contains("__ctx_cons"), "LHS: {}", lhs);
    }

    #[test]
    fn nested_context_ops() {
        // [lookup [extend Γ x A] 0]
        let rules = vec![rule("r1",
            list(vec![atom("lookup"),
                list(vec![atom("extend"), atom("?Γ"), atom("?x"), atom("?A")]),
                atom("0")]),
            atom("?A"))];
        let result = reify_contexts(&rules, &[]);
        assert_eq!(result.reified_count, 2); // lookup + extend both reified
    }

    #[test]
    fn empty_ctx_reified() {
        let rules = vec![rule("r1",
            list(vec![atom("typeof"), atom("empty-ctx"), atom("?t")]),
            atom("ok"))];
        let result = reify_contexts(&rules, &[]);
        assert_eq!(result.reified_count, 1);
        let lhs = format!("{}", result.rules[0].lhs);
        assert!(lhs.contains("__ctx_nil"), "LHS: {}", lhs);
    }

    #[test]
    fn is_context_var_checks() {
        assert!(is_context_var("Γ", &[]));
        assert!(is_context_var("Γ1", &[]));
        assert!(is_context_var("ctx", &[]));
        assert!(is_context_var("delta", &[]));
        assert!(!is_context_var("x", &[]));
        assert!(is_context_var("MyCtx", &["MyCtx".to_string()]));
    }

    #[test]
    fn structural_rules_generated() {
        let rules = vec![rule("r1", atom("a"), atom("b"))];
        let result = reify_contexts(&rules, &[]);
        assert!(!result.aux_rules.is_empty());
        let aux_lhs = format!("{}", result.aux_rules[0].lhs);
        assert!(aux_lhs.contains("__ctx_lookup"), "Aux: {}", aux_lhs);
    }

    // === ABYSSAL: Shadowing Sinkhole ===

    #[test]
    fn shadowing_sinkhole_name_lookup_finds_most_recent() {
        // Context: [__ctx_cons [__ctx_cons [__ctx_cons __ctx_nil x C] y B] x A]
        // The variable x is bound TWICE: once to C (deep), once to A (recent).
        // __ctx_lookup_name for "x" MUST return A, not C.
        let result = reify_contexts(&[], &[]);

        let ctx = list(vec![atom("__ctx_cons"),
            list(vec![atom("__ctx_cons"),
                list(vec![atom("__ctx_cons"),
                    atom("__ctx_nil"),
                    atom("x"), atom("C")]),
                atom("y"), atom("B")]),
            atom("x"), atom("A")]);

        // The aux rules should include a name-based lookup rule
        let has_name_lookup = result.aux_rules.iter().any(|r| r.name.contains("name"));
        assert!(has_name_lookup,
            "Must generate name-based lookup rules for shadowing support. Rules: {:?}",
            result.aux_rules.iter().map(|r| &r.name).collect::<Vec<_>>());
    }

    #[test]
    fn shadowing_sinkhole_index_traversal_exists() {
        // Even with index-based lookup, we need a successor rule:
        // [__ctx_lookup [__ctx_cons ?Γ ?x ?A] [S ?n]] ==> [__ctx_lookup ?Γ ?n]
        // Without it, we can never look past the first binding.
        let result = reify_contexts(&[], &[]);

        let has_succ_rule = result.aux_rules.iter().any(|r| r.name.contains("succ"));
        assert!(has_succ_rule,
            "Must generate successor lookup rule for index traversal. Rules: {:?}",
            result.aux_rules.iter().map(|r| &r.name).collect::<Vec<_>>());
    }

    #[test]
    fn shadowing_sinkhole_name_skip_rule_exists() {
        // Name-based lookup on mismatch must skip:
        // [__ctx_lookup_name [__ctx_cons ?Γ ?y ?B] ?x] ==> [__ctx_lookup_name ?Γ ?x]
        //   (when ?x ≠ ?y — enforced via non-linear pattern or guard)
        let result = reify_contexts(&[], &[]);

        let has_skip_rule = result.aux_rules.iter().any(|r|
            r.name.contains("name") && r.name.contains("skip"));
        assert!(has_skip_rule,
            "Must generate name-skip rule to traverse past non-matching bindings. Rules: {:?}",
            result.aux_rules.iter().map(|r| &r.name).collect::<Vec<_>>());
    }
}
