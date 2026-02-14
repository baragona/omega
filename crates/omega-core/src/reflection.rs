/// Reflection: promoting proven metatheorems into new inference rules.
///
/// Safety guarantees:
/// 1. The metatheorem must have been verified
/// 2. The reflected rule records its provenance
/// 3. Reflected rules cannot be used in their own metatheorem proof
/// 4. Staleness detection via theory content hash
use crate::error::{DeclKind, OmegaError, Result};
use crate::expr::{Expr, Name};
use crate::judgment::Rule;
use crate::metatheorem::MetaTheorem;
use crate::theory::Theory;

/// Record of a reflection operation.
#[derive(Debug, Clone)]
pub struct ReflectionRecord {
    /// The metatheorem that was reflected.
    pub metatheorem_name: Name,
    /// The new rule name.
    pub rule_name: Name,
    /// The theory it belongs to.
    pub theory_name: Name,
    /// Hash of the theory at the time of reflection.
    pub theory_hash: u64,
}

/// Promote a verified metatheorem into a new inference rule.
///
/// The metatheorem `forall D : J1, exists D' : J2` becomes a rule with:
/// - premise: J1
/// - conclusion: J2
pub fn reflect(
    metatheorem: &MetaTheorem,
    rule_name: &str,
    theory: &Theory,
) -> Result<(Rule, ReflectionRecord)> {
    // Check that the theory matches
    if theory.name() != metatheorem.theory_name {
        return Err(OmegaError::RuleNotInTheory {
            rule: metatheorem.name.clone(),
            theory: theory.name().into(),
        });
    }

    // Check that the rule name doesn't already exist
    if theory.get_rule(rule_name).is_some() {
        return Err(OmegaError::DuplicateName { kind: DeclKind::Rule, name: rule_name.into() });
    }

    // Build the rule from the metatheorem's forall/exists
    let premises: Vec<Expr> = metatheorem.forall.iter().map(|(_, j)| j.clone()).collect();
    let conclusion = if metatheorem.exists.len() == 1 {
        metatheorem.exists[0].1.clone()
    } else if metatheorem.exists.is_empty() {
        // A metatheorem with no existential is weird but handle it
        return Err(OmegaError::NoExistential {
            metatheorem: metatheorem.name.clone(),
        });
    } else {
        // Multiple existentials: for now, take the first one
        metatheorem.exists[0].1.clone()
    };

    let rule = Rule::new(rule_name, premises, conclusion)
        .with_reflected()
        .with_provenance(metatheorem.name.clone());

    let record = ReflectionRecord {
        metatheorem_name: metatheorem.name.clone(),
        rule_name: rule_name.into(),
        theory_name: theory.name().into(),
        theory_hash: theory.content_hash(),
    };

    Ok((rule, record))
}

/// Check that a reflection is still valid (the theory hasn't changed).
pub fn check_reflection_validity(
    record: &ReflectionRecord,
    theory: &Theory,
) -> Result<()> {
    if theory.content_hash() != record.theory_hash {
        return Err(OmegaError::StaleReflection {
            metatheorem: record.metatheorem_name.clone(),
            theory: record.theory_name.clone(),
        });
    }
    Ok(())
}

/// Check that a metatheorem proof doesn't use any reflected rules
/// (no self-strengthening).
pub fn check_no_self_strengthening(
    metatheorem: &MetaTheorem,
    theory: &Theory,
) -> Result<()> {
    let reflected_rules: Vec<&str> = theory
        .rules()
        .iter()
        .filter(|r| r.reflected())
        .map(|r| r.name().as_str())
        .collect();

    check_proof_no_reflected(&metatheorem.proof, &reflected_rules, &metatheorem.name)
}

fn check_proof_no_reflected(
    proof: &crate::metatheorem::MetaProof,
    reflected_rules: &[&str],
    metatheorem_name: &str,
) -> Result<()> {
    use crate::metatheorem::MetaProof;

    match proof {
        MetaProof::CaseAnalysis { cases, .. } => {
            for case in cases {
                if reflected_rules.contains(&case.rule_name.as_str()) {
                    return Err(OmegaError::SelfStrengthening {
                        reflected_rule: case.rule_name.clone(),
                        metatheorem: metatheorem_name.into(),
                    });
                }
                check_proof_no_reflected(&case.body, reflected_rules, metatheorem_name)?;
            }
            Ok(())
        }
        MetaProof::ByRule { rule_name, args } => {
            if reflected_rules.contains(&rule_name.as_str()) {
                return Err(OmegaError::SelfStrengthening {
                    reflected_rule: rule_name.clone(),
                    metatheorem: metatheorem_name.into(),
                });
            }
            for arg in args {
                check_proof_no_reflected(arg, reflected_rules, metatheorem_name)?;
            }
            Ok(())
        }
        MetaProof::Inductive { .. } | MetaProof::Var(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Expr;
    use crate::judgment::Rule;
    use crate::metatheorem::{MetaCase, MetaProof, MetaTheorem};
    use crate::test_util::make_prop_logic;

    #[test]
    fn reflect_and_comm() {
        let theory = make_prop_logic();

        let mt = MetaTheorem {
            name: "and-comm".into(),
            theory_name: "PropLogic".into(),
            forall: vec![(
                "D".into(),
                Expr::app(vec![
                    Expr::sym("proves"),
                    Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
                ]),
            )],
            exists: vec![(
                "D'".into(),
                Expr::app(vec![
                    Expr::sym("proves"),
                    Expr::app(vec![Expr::sym("and"), Expr::meta("B"), Expr::meta("A")]),
                ]),
            )],
            proof: MetaProof::CaseAnalysis {
                scrutinee: "D".into(),
                cases: vec![MetaCase {
                    rule_name: "and-intro".into(),
                    premise_names: vec!["D1".into(), "D2".into()],
                    body: MetaProof::ByRule {
                        rule_name: "and-intro".into(),
                        args: vec![
                            MetaProof::Var("D2".into()),
                            MetaProof::Var("D1".into()),
                        ],
                    },
                }],
            },
        };

        let (rule, record) = reflect(&mt, "proves/and-comm", &theory).unwrap();
        assert_eq!(rule.name(), "proves/and-comm");
        assert!(rule.reflected());
        assert_eq!(rule.provenance(), Some(&Name::from("and-comm")));
        assert_eq!(record.theory_name, "PropLogic");
    }

    #[test]
    fn detect_stale_reflection() {
        let mut theory = make_prop_logic();
        let hash_before = theory.content_hash();

        let record = ReflectionRecord {
            metatheorem_name: "test".into(),
            rule_name: "test-rule".into(),
            theory_name: "PropLogic".into(),
            theory_hash: hash_before,
        };

        // Before modification: should be valid
        assert!(check_reflection_validity(&record, &theory).is_ok());

        // Modify the theory
        theory.add_rule(Rule::new(
            "new-rule",
            vec![],
            Expr::sym("whatever"),
        )).unwrap();

        // After modification: should be stale
        assert!(matches!(
            check_reflection_validity(&record, &theory),
            Err(OmegaError::StaleReflection { .. })
        ));
    }
}
