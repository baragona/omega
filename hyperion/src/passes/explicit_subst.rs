//! Explicit Substitution Calculus (λσ) pass.
//!
//! Lowers standard lambda binders into a first-order explicit substitution
//! calculus so the e-graph can safely rewrite under binders using purely
//! first-order rules — no variable capture possible.
//!
//! The calculus introduces these first-order AST nodes:
//!   - `[Closure term env]`     — a term paired with its pending substitution
//!   - `[Shift n]`              — de Bruijn shift by n levels
//!   - `[Cons val env]`         — extend an environment: val · env
//!   - `[IdEnv]`                — identity environment (no pending substitutions)
//!   - `[Compose env1 env2]`    — sequential composition of environments
//!
//! And generates rewrite rules implementing the σ-calculus:
//!   - `[Closure [Var 0] [Cons ?v ?e]]` ==> `?v`                     (var-zero)
//!   - `[Closure [Var [S ?n]] [Cons ?v ?e]]` ==> `[Closure [Var ?n] ?e]` (var-succ)
//!   - `[Closure ?t [IdEnv]]` ==> `?t`                               (id-elim)
//!   - `[Closure [App ?f ?a] ?e]` ==> `[App [Closure ?f ?e] [Closure ?a ?e]]` (app-push)
//!   - `[Closure [binder ?body] ?e]` ==> `[binder [Closure ?body [Cons [Var 0] [Compose ?e [Shift 1]]]]]` (binder-push)
//!   - `[Compose [IdEnv] ?e]` ==> `?e`                               (compose-id-l)
//!   - `[Compose ?e [IdEnv]]` ==> `?e`                               (compose-id-r)

use apeiron::parser::{Sexp, Span};

/// Result of the explicit substitution lowering pass.
pub struct ExplicitSubstResult {
    /// Original rules, with binder bodies wrapped in Closure nodes.
    pub rules: Vec<crate::session::VonNeumannRule>,
    /// New σ-calculus rewrite rules to add to the system.
    pub sigma_rules: Vec<crate::session::VonNeumannRule>,
    /// How many binder occurrences were lowered.
    pub lowered_count: usize,
}

/// Lower binder expressions to explicit substitution calculus.
///
/// `binder_name` is the HOAS binder (e.g., "lam").
/// Returns transformed rules plus the σ-calculus machinery rules.
pub fn lower_to_explicit_subst(
    rules: &[crate::session::VonNeumannRule],
    binder_name: &str,
) -> ExplicitSubstResult {
    let mut transformed = Vec::new();
    let mut lowered = 0;

    for rule in rules {
        let (new_lhs, n1) = lower_sexp(&rule.lhs, binder_name);
        let (new_rhs, n2) = lower_sexp(&rule.rhs, binder_name);
        lowered += n1 + n2;
        transformed.push(crate::session::VonNeumannRule {
            name: rule.name.clone(),
            lhs: new_lhs,
            rhs: new_rhs,
        });
    }

    let sigma_rules = generate_sigma_rules(binder_name);

    ExplicitSubstResult {
        rules: transformed,
        sigma_rules,
        lowered_count: lowered,
    }
}

/// Recursively traverse sexp, wrapping binder bodies with Closure nodes.
///
/// `[lam ?A ?body]` → `[lam ?A [Closure ?body [Cons [Var 0] [Compose ?e [Shift 1]]]]]`
/// is NOT done here — that's handled by the sigma rewrite rules at runtime.
/// Instead, this pass wraps top-level free terms in `[Closure term [IdEnv]]`
/// when they appear as arguments to binder-containing expressions.
fn lower_sexp(sexp: &Sexp, binder_name: &str) -> (Sexp, usize) {
    let sp = sexp.span();
    match sexp {
        Sexp::Atom(_, _) => (sexp.clone(), 0),
        Sexp::List(items, _) => {
            // Check for binder pattern: [lam <body>] or [lam <type> <body>]
            if items.len() >= 2 {
                if let Some(head) = items[0].as_atom() {
                    if head == binder_name {
                        // Last element is the body, everything in between is type args
                        let body_idx = items.len() - 1;
                        let mut new_items = vec![atom(sp, binder_name)];
                        let mut n_total = 0;

                        // Lower type arguments
                        for i in 1..body_idx {
                            let (new_arg, n) = lower_sexp(&items[i], binder_name);
                            new_items.push(new_arg);
                            n_total += n;
                        }

                        // Lower and wrap the body
                        let (new_body, n_body) = lower_sexp(&items[body_idx], binder_name);
                        let closure_body = mk_list(sp, vec![
                            atom(sp, "Closure"),
                            new_body,
                            atom(sp, "IdEnv"),
                        ]);
                        new_items.push(closure_body);

                        return (Sexp::List(new_items, sp), n_total + n_body + 1);
                    }
                }
            }

            // Recurse into children
            let mut total = 0;
            let mut new_items = Vec::new();
            for item in items {
                let (new_item, n) = lower_sexp(item, binder_name);
                new_items.push(new_item);
                total += n;
            }
            (Sexp::List(new_items, sp), total)
        }
    }
}

/// Generate the σ-calculus rewrite rules.
fn generate_sigma_rules(binder_name: &str) -> Vec<crate::session::VonNeumannRule> {
    let sp = Span::default();
    let mut rules = Vec::new();

    // var-zero: [Closure [Var 0] [Cons ?v ?e]] ==> ?v
    rules.push(mk_rule("sigma-var-zero",
        mk_list(sp, vec![
            atom(sp, "Closure"),
            mk_list(sp, vec![atom(sp, "Var"), atom(sp, "0")]),
            mk_list(sp, vec![atom(sp, "Cons"), atom(sp, "?v"), atom(sp, "?e")]),
        ]),
        atom(sp, "?v"),
    ));

    // var-succ: [Closure [Var [S ?n]] [Cons ?v ?e]] ==> [Closure [Var ?n] ?e]
    rules.push(mk_rule("sigma-var-succ",
        mk_list(sp, vec![
            atom(sp, "Closure"),
            mk_list(sp, vec![atom(sp, "Var"), mk_list(sp, vec![atom(sp, "S"), atom(sp, "?n")])]),
            mk_list(sp, vec![atom(sp, "Cons"), atom(sp, "?v"), atom(sp, "?e")]),
        ]),
        mk_list(sp, vec![
            atom(sp, "Closure"),
            mk_list(sp, vec![atom(sp, "Var"), atom(sp, "?n")]),
            atom(sp, "?e"),
        ]),
    ));

    // id-elim: [Closure ?t [IdEnv]] ==> ?t
    rules.push(mk_rule("sigma-id-elim",
        mk_list(sp, vec![
            atom(sp, "Closure"),
            atom(sp, "?t"),
            atom(sp, "IdEnv"),
        ]),
        atom(sp, "?t"),
    ));

    // app-push: [Closure [App ?f ?a] ?e] ==> [App [Closure ?f ?e] [Closure ?a ?e]]
    rules.push(mk_rule("sigma-app-push",
        mk_list(sp, vec![
            atom(sp, "Closure"),
            mk_list(sp, vec![atom(sp, "App"), atom(sp, "?f"), atom(sp, "?a")]),
            atom(sp, "?e"),
        ]),
        mk_list(sp, vec![
            atom(sp, "App"),
            mk_list(sp, vec![atom(sp, "Closure"), atom(sp, "?f"), atom(sp, "?e")]),
            mk_list(sp, vec![atom(sp, "Closure"), atom(sp, "?a"), atom(sp, "?e")]),
        ]),
    ));

    // binder-push: [Closure [lam ?body] ?e] ==> [lam [Closure ?body [Cons [Var 0] [Compose ?e [Shift 1]]]]]
    rules.push(mk_rule(&format!("sigma-{}-push", binder_name),
        mk_list(sp, vec![
            atom(sp, "Closure"),
            mk_list(sp, vec![atom(sp, binder_name), atom(sp, "?body")]),
            atom(sp, "?e"),
        ]),
        mk_list(sp, vec![
            atom(sp, binder_name),
            mk_list(sp, vec![
                atom(sp, "Closure"),
                atom(sp, "?body"),
                mk_list(sp, vec![
                    atom(sp, "Cons"),
                    mk_list(sp, vec![atom(sp, "Var"), atom(sp, "0")]),
                    mk_list(sp, vec![atom(sp, "Compose"), atom(sp, "?e"), mk_list(sp, vec![atom(sp, "Shift"), atom(sp, "1")])]),
                ]),
            ]),
        ]),
    ));

    // compose-id-l: [Compose [IdEnv] ?e] ==> ?e
    rules.push(mk_rule("sigma-compose-id-l",
        mk_list(sp, vec![atom(sp, "Compose"), atom(sp, "IdEnv"), atom(sp, "?e")]),
        atom(sp, "?e"),
    ));

    // compose-id-r: [Compose ?e [IdEnv]] ==> ?e
    rules.push(mk_rule("sigma-compose-id-r",
        mk_list(sp, vec![atom(sp, "Compose"), atom(sp, "?e"), atom(sp, "IdEnv")]),
        atom(sp, "?e"),
    ));

    rules
}

/// Lower binder expressions within a Theory sexp (for e-graph path).
/// Transforms `@rule` LHS/RHS pairs within the theory declaration.
/// Theory structure: `[Theory Name :in Uni ... [@rule name LHS ==> RHS] ...]`
/// Returns `Some((new_sexp, count))` if any lowering happened.
pub fn lower_theory_sexp(sexp: &Sexp, binder_name: &str) -> Option<(Sexp, usize)> {
    let items = sexp.as_list()?;
    let sp = sexp.span();
    let mut new_items = Vec::new();
    let mut total_lowered = 0;

    for item in items {
        if let Some(sub_items) = item.as_list() {
            // Check for [@rule name LHS ==> RHS]
            if sub_items.len() >= 5 {
                if let Some(head) = sub_items[0].as_atom() {
                    if head == "@rule" {
                        if let Some(sep) = sub_items.get(3).and_then(|s| s.as_atom()) {
                            if sep == "==>" {
                                let (new_lhs, n1) = lower_sexp(&sub_items[2], binder_name);
                                let (new_rhs, n2) = lower_sexp(&sub_items[4], binder_name);
                                let mut new_sub = vec![
                                    sub_items[0].clone(), // @rule
                                    sub_items[1].clone(), // name
                                    new_lhs,
                                    sub_items[3].clone(), // ==>
                                    new_rhs,
                                ];
                                // Preserve any trailing items (e.g., conditions)
                                new_sub.extend(sub_items[5..].iter().cloned());
                                total_lowered += n1 + n2;
                                new_items.push(Sexp::List(new_sub, item.span()));
                                continue;
                            }
                        }
                    }
                }
            }
        }
        new_items.push(item.clone());
    }

    if total_lowered > 0 {
        Some((Sexp::List(new_items, sp), total_lowered))
    } else {
        None
    }
}

fn atom(sp: Span, s: &str) -> Sexp {
    Sexp::Atom(s.to_string(), sp)
}

fn mk_list(sp: Span, items: Vec<Sexp>) -> Sexp {
    Sexp::List(items, sp)
}

fn mk_rule(name: &str, lhs: Sexp, rhs: Sexp) -> crate::session::VonNeumannRule {
    crate::session::VonNeumannRule {
        name: name.to_string(),
        lhs,
        rhs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &str) -> Sexp {
        Sexp::Atom(s.to_string(), Span::default())
    }

    fn l(items: Vec<Sexp>) -> Sexp {
        Sexp::List(items, Span::default())
    }

    fn rule(name: &str, lhs: Sexp, rhs: Sexp) -> crate::session::VonNeumannRule {
        crate::session::VonNeumannRule { name: name.to_string(), lhs, rhs }
    }

    #[test]
    fn no_binders_passes_through() {
        let rules = vec![rule("r1", l(vec![a("f"), a("?x")]), a("?x"))];
        let result = lower_to_explicit_subst(&rules, "lam");
        assert_eq!(result.lowered_count, 0);
        assert_eq!(format!("{}", result.rules[0].lhs), "[f ?x]");
        // Sigma rules are always generated
        assert!(!result.sigma_rules.is_empty());
    }

    #[test]
    fn binder_wrapped_in_closure() {
        // [lam ?A ?B] → [lam ?A [Closure ?B IdEnv]]
        let rules = vec![
            rule("r1", l(vec![a("lam"), a("?A"), a("?B")]), a("ok")),
        ];
        let result = lower_to_explicit_subst(&rules, "lam");
        assert_eq!(result.lowered_count, 1);
        let lhs_str = format!("{}", result.rules[0].lhs);
        assert!(lhs_str.contains("Closure"), "Expected Closure in: {}", lhs_str);
        assert!(lhs_str.contains("IdEnv"), "Expected IdEnv in: {}", lhs_str);
    }

    #[test]
    fn nested_binders_both_lowered() {
        let rules = vec![
            rule("r1",
                l(vec![a("lam"), a("?A"), l(vec![a("lam"), a("?B"), a("?C")])]),
                a("ok")),
        ];
        let result = lower_to_explicit_subst(&rules, "lam");
        assert_eq!(result.lowered_count, 2);
    }

    #[test]
    fn sigma_rules_generated() {
        let result = lower_to_explicit_subst(&[], "lam");
        let names: Vec<_> = result.sigma_rules.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"sigma-var-zero"));
        assert!(names.contains(&"sigma-var-succ"));
        assert!(names.contains(&"sigma-id-elim"));
        assert!(names.contains(&"sigma-app-push"));
        assert!(names.contains(&"sigma-lam-push"));
        assert!(names.contains(&"sigma-compose-id-l"));
        assert!(names.contains(&"sigma-compose-id-r"));
    }

    #[test]
    fn rhs_binders_also_lowered() {
        let rules = vec![
            rule("r1", a("?x"), l(vec![a("lam"), a("?A"), a("?B")])),
        ];
        let result = lower_to_explicit_subst(&rules, "lam");
        assert_eq!(result.lowered_count, 1);
        let rhs_str = format!("{}", result.rules[0].rhs);
        assert!(rhs_str.contains("Closure"), "Expected Closure in RHS: {}", rhs_str);
    }
}
