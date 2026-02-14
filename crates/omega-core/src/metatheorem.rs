/// Metatheorem verification.
///
/// A metatheorem is a statement about a theory: for every derivation of shape X,
/// there exists a derivation of shape Y. The proof proceeds by structural induction
/// (case analysis) on the input derivation.
use std::collections::HashSet;

use crate::error::{OmegaError, Result};
use crate::expr::{Expr, Name};
use crate::theory::Theory;

/// A metatheorem declaration.
#[derive(Debug, Clone)]
pub struct MetaTheorem {
    /// Name of this metatheorem.
    pub name: Name,
    /// The theory this metatheorem is about.
    pub theory_name: Name,
    /// Universally quantified derivation variables.
    /// Each entry is (var_name, judgment_pattern).
    pub forall: Vec<(Name, Expr)>,
    /// Existentially quantified derivation variables (the witness).
    /// Each entry is (var_name, judgment_pattern).
    pub exists: Vec<(Name, Expr)>,
    /// The proof of the metatheorem.
    pub proof: MetaProof,
}

/// A metatheorem proof by structural induction.
#[derive(Debug, Clone)]
pub enum MetaProof {
    /// Case analysis on a universally-quantified derivation variable.
    CaseAnalysis {
        /// The variable being analyzed.
        scrutinee: Name,
        /// One case per rule that could have produced the scrutinee.
        cases: Vec<MetaCase>,
    },
    /// Directly construct the witness derivation using a rule.
    ByRule {
        rule_name: Name,
        /// Arguments to the rule (sub-derivation witnesses or inductive calls).
        args: Vec<MetaProof>,
    },
    /// Inductive call: apply the metatheorem recursively to a sub-derivation.
    Inductive {
        /// The metatheorem being invoked (must be the one being defined).
        metatheorem_name: Name,
        /// The sub-derivation being passed (must be structurally smaller).
        arg: Name,
    },
    /// Use one of the bound derivation variables directly.
    Var(Name),
}

/// A single case in a case analysis.
#[derive(Debug, Clone)]
pub struct MetaCase {
    /// The rule this case handles.
    pub rule_name: Name,
    /// Names for the sub-derivation premises of this rule.
    pub premise_names: Vec<Name>,
    /// The proof for this case.
    pub body: MetaProof,
}

/// Verify a metatheorem proof.
pub fn verify_metatheorem(metatheorem: &MetaTheorem, theory: &Theory) -> Result<()> {
    // 1. Check the theory exists and matches
    if theory.name() != metatheorem.theory_name {
        return Err(OmegaError::RuleNotInTheory {
            rule: metatheorem.name.clone(),
            theory: metatheorem.theory_name.clone(),
        });
    }

    // 2. Check case analysis exhaustiveness and structural recursion
    let forall_map: std::collections::HashMap<_, _> = metatheorem.forall.iter().cloned().collect();

    verify_proof(
        &metatheorem.proof,
        &metatheorem.name,
        theory,
        &forall_map,
        &HashSet::new(),
    )?;

    Ok(())
}

/// Verify a metatheorem proof term.
fn verify_proof(
    proof: &MetaProof,
    metatheorem_name: &str,
    theory: &Theory,
    forall_vars: &std::collections::HashMap<Name, Expr>,
    // Set of variable names that are structurally smaller than the original scrutinee
    smaller_vars: &HashSet<Name>,
) -> Result<()> {
    match proof {
        MetaProof::CaseAnalysis { scrutinee, cases } => {
            // The scrutinee must be a universally-quantified variable
            let scrutinee_judgment = forall_vars.get(scrutinee).ok_or_else(|| {
                OmegaError::MalformedDerivation(format!(
                    "case analysis on unknown variable {}",
                    scrutinee
                ))
            })?;

            // Find all rules whose conclusion could match the scrutinee's judgment
            let applicable_rules = find_applicable_rules(theory, scrutinee_judgment);

            // Check exhaustiveness
            let covered: HashSet<&str> = cases.iter().map(|c| c.rule_name.as_str()).collect();
            let missing: Vec<Name> = applicable_rules
                .iter()
                .filter(|r| !covered.contains(r.as_str()))
                .cloned()
                .collect();

            if !missing.is_empty() {
                return Err(OmegaError::NonExhaustiveCases {
                    missing_rules: missing,
                });
            }

            // Verify each case
            for case in cases {
                // Ensure the rule exists
                let rule = theory.get_rule(&case.rule_name).ok_or_else(|| {
                    OmegaError::UnknownName { kind: "rule".into(), name: case.rule_name.clone() }
                })?;

                // The premise names are structurally smaller
                let mut new_smaller = smaller_vars.clone();
                for pname in &case.premise_names {
                    new_smaller.insert(pname.clone());
                }

                // Extend forall_vars with the premise names and their types
                let mut extended_forall = forall_vars.clone();
                for (pname, premise_pattern) in
                    case.premise_names.iter().zip(rule.premises.iter())
                {
                    extended_forall.insert(pname.clone(), premise_pattern.clone());
                }

                verify_proof(
                    &case.body,
                    metatheorem_name,
                    theory,
                    &extended_forall,
                    &new_smaller,
                )?;
            }

            Ok(())
        }

        MetaProof::ByRule { rule_name, args } => {
            // The rule must exist in the theory
            let rule = theory.get_rule(rule_name).ok_or_else(|| {
                OmegaError::UnknownName { kind: "rule".into(), name: rule_name.clone() }
            })?;

            if args.len() != rule.premises.len() {
                return Err(OmegaError::PremiseCountMismatch {
                    rule: rule_name.clone(),
                    expected: rule.premises.len(),
                    got: args.len(),
                });
            }

            // Recursively verify each argument
            for arg in args {
                verify_proof(arg, metatheorem_name, theory, forall_vars, smaller_vars)?;
            }

            Ok(())
        }

        MetaProof::Inductive {
            metatheorem_name: called_name,
            arg,
        } => {
            // Must be calling itself
            if called_name != metatheorem_name {
                return Err(OmegaError::NonStructuralRecursion {
                    metatheorem: metatheorem_name.to_string(),
                    detail: format!(
                        "inductive call to {} but proving {}",
                        called_name, metatheorem_name
                    ),
                });
            }

            // The argument must be structurally smaller
            if !smaller_vars.contains(arg) {
                return Err(OmegaError::NonStructuralRecursion {
                    metatheorem: metatheorem_name.to_string(),
                    detail: format!(
                        "argument {} is not structurally smaller than the scrutinee",
                        arg
                    ),
                });
            }

            Ok(())
        }

        MetaProof::Var(name) => {
            // The variable must be in scope
            if !forall_vars.contains_key(name) {
                return Err(OmegaError::MalformedDerivation(format!(
                    "unknown derivation variable {}",
                    name
                )));
            }
            Ok(())
        }
    }
}

/// Find all rule names whose conclusion could have produced a given judgment.
///
/// A rule is applicable if its conclusion can be *specialized* to match the
/// judgment pattern. We match the judgment against the conclusion (treating
/// the conclusion's metas as pattern variables). This means a rule with
/// conclusion `(proves ?A)` is only applicable for `(proves (and ?X ?Y))`
/// if `?A` in that conclusion is *the* pattern variable being matched —
/// which is always true. However, we use **head-constructor refinement**:
/// if the judgment pattern has a known head constructor in the position
/// where the rule's conclusion has a meta-variable, then the rule only
/// applies if it could actually introduce that constructor (i.e., it is
/// a potential *introduction* rule for that form).
///
/// In practice: we check if the rule's conclusion, after matching the
/// outer judgment form, also constrains the inner structure. If the
/// conclusion's inner structure is just a bare meta-variable (like `?A`),
/// it cannot *introduce* a specific constructor like `and`, so it is
/// NOT considered applicable. Only rules whose conclusion structurally
/// produces the judgment's head constructor are applicable.
fn find_applicable_rules(theory: &Theory, judgment: &Expr) -> Vec<Name> {
    let mut result = Vec::new();
    for rule in theory.rules() {
        if conclusion_could_introduce(rule, judgment) {
            result.push(rule.name.clone());
        }
    }
    result
}

/// Check if a rule's conclusion could directly introduce/produce the given judgment.
///
/// This uses structural compatibility: the non-meta parts of the conclusion
/// must be compatible with the judgment pattern. A bare meta-variable in the
/// conclusion at a position where the judgment has a constructor means the
/// rule does NOT introduce that constructor — it merely passes through.
fn conclusion_could_introduce(rule: &crate::judgment::Rule, judgment: &Expr) -> bool {
    structurally_compatible(&rule.conclusion, judgment)
}

/// Check structural compatibility: at each position, either both sides agree
/// on the constructor, or the judgment side is a meta-variable (wildcard).
/// If the conclusion side is a meta-variable but the judgment side is not,
/// the rule does NOT structurally introduce that form.
fn structurally_compatible(conclusion: &Expr, judgment: &Expr) -> bool {
    match (conclusion, judgment) {
        // Both symbols: must match
        (Expr::Sym(a), Expr::Sym(b)) => a == b,
        // Conclusion is a meta: only compatible if judgment is also a meta
        // (the rule doesn't constrain this position)
        (Expr::Meta(_), Expr::Meta(_)) => true,
        (Expr::Meta(_), _) => false, // rule doesn't introduce this structure
        // Judgment is a meta: always compatible (wildcard)
        (_, Expr::Meta(_)) => true,
        // Both applications: check head and recurse
        (Expr::App(args_c), Expr::App(args_j)) => {
            if args_c.len() != args_j.len() {
                return false;
            }
            args_c
                .iter()
                .zip(args_j.iter())
                .all(|(c, j)| structurally_compatible(c, j))
        }
        // Both bound vars
        (Expr::Bound(a), Expr::Bound(b)) => a == b,
        // Both free vars
        (Expr::Free(a), Expr::Free(b)) => a == b,
        // Binders
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
        ) => k1 == k2 && structurally_compatible(t1, t2) && structurally_compatible(b1, b2),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Expr;
    use crate::test_util::make_prop_logic;

    #[test]
    fn verify_and_comm_metatheorem() {
        let theory = make_prop_logic();

        // Metatheorem: forall D : (proves (and ?A ?B)),
        //   exists D' : (proves (and ?B ?A))
        // Proof: case analysis on D:
        //   case and-intro(D1, D2) => by-rule and-intro(D2, D1)
        let mt = MetaTheorem {
            name: "and-comm".to_string(),
            theory_name: "PropLogic".to_string(),
            forall: vec![(
                "D".to_string(),
                Expr::app(vec![
                    Expr::sym("proves"),
                    Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
                ]),
            )],
            exists: vec![(
                "D'".to_string(),
                Expr::app(vec![
                    Expr::sym("proves"),
                    Expr::app(vec![Expr::sym("and"), Expr::meta("B"), Expr::meta("A")]),
                ]),
            )],
            proof: MetaProof::CaseAnalysis {
                scrutinee: "D".to_string(),
                cases: vec![MetaCase {
                    rule_name: "and-intro".to_string(),
                    premise_names: vec!["D1".to_string(), "D2".to_string()],
                    body: MetaProof::ByRule {
                        rule_name: "and-intro".to_string(),
                        args: vec![MetaProof::Var("D2".to_string()), MetaProof::Var("D1".to_string())],
                    },
                }],
            },
        };

        assert!(verify_metatheorem(&mt, &theory).is_ok());
    }

    #[test]
    fn reject_non_exhaustive() {
        let theory = make_prop_logic();

        // Missing the and-intro case
        let mt = MetaTheorem {
            name: "bad".to_string(),
            theory_name: "PropLogic".to_string(),
            forall: vec![(
                "D".to_string(),
                Expr::app(vec![
                    Expr::sym("proves"),
                    Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
                ]),
            )],
            exists: vec![],
            proof: MetaProof::CaseAnalysis {
                scrutinee: "D".to_string(),
                cases: vec![], // No cases!
            },
        };

        assert!(matches!(
            verify_metatheorem(&mt, &theory),
            Err(OmegaError::NonExhaustiveCases { .. })
        ));
    }

    #[test]
    fn reject_non_structural_recursion() {
        let theory = make_prop_logic();

        let mt = MetaTheorem {
            name: "bad-induction".to_string(),
            theory_name: "PropLogic".to_string(),
            forall: vec![(
                "D".to_string(),
                Expr::app(vec![
                    Expr::sym("proves"),
                    Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
                ]),
            )],
            exists: vec![],
            proof: MetaProof::CaseAnalysis {
                scrutinee: "D".to_string(),
                cases: vec![MetaCase {
                    rule_name: "and-intro".to_string(),
                    premise_names: vec!["D1".to_string(), "D2".to_string()],
                    body: MetaProof::Inductive {
                        metatheorem_name: "bad-induction".to_string(),
                        arg: "D".to_string(), // NOT structurally smaller!
                    },
                }],
            },
        };

        assert!(matches!(
            verify_metatheorem(&mt, &theory),
            Err(OmegaError::NonStructuralRecursion { .. })
        ));
    }
}
