/// The Omega kernel: the trusted core with exactly four operations.
///
/// 1. `register_theory` — validate and load a theory
/// 2. `check_derivation` — verify a proof tree
/// 3. `check_metatheorem` — verify a structural-induction proof
/// 4. `reflect` — promote a proven metatheorem into a rule
use std::collections::HashMap;

use crate::derivation::{self, Context, Derivation};
use crate::error::{OmegaError, Result};
use crate::expr::Name;
use crate::expr::Expr;
use crate::interned_check;
use crate::metatheorem::{self, MetaTheorem};
use crate::reflection::{self, ReflectionRecord};
use crate::theory::Theory;

/// The kernel state.
pub struct Kernel {
    /// Registered theories, keyed by name.
    theories: HashMap<Name, Theory>,
    /// Verified metatheorems, keyed by name.
    verified_metatheorems: HashMap<Name, MetaTheorem>,
    /// Reflection records for audit trail.
    reflections: Vec<ReflectionRecord>,
    /// Use the interned (hash-consed) derivation checker for O(1) equality.
    pub use_interned: bool,
    /// Cached interned theories for O(1) equality during proof checking.
    interned_cache: HashMap<Name, interned_check::InternedTheory>,
}

impl Kernel {
    /// Create a new empty kernel.
    pub fn new() -> Self {
        Kernel {
            theories: HashMap::new(),
            verified_metatheorems: HashMap::new(),
            reflections: Vec::new(),
            use_interned: true,
            interned_cache: HashMap::new(),
        }
    }

    /// Operation 1: Register and validate a theory.
    pub fn register_theory(&mut self, theory: Theory) -> Result<()> {
        theory.validate()?;

        let name = theory.name.clone();
        if self.use_interned {
            self.interned_cache
                .insert(name.clone(), interned_check::InternedTheory::new(&theory));
        }
        self.theories.insert(name, theory);
        Ok(())
    }

    /// Operation 2: Check a derivation against a goal in a theory.
    pub fn check_derivation(
        &mut self,
        theory_name: &str,
        goal: &Expr,
        deriv: &Derivation,
        ctx: &Context,
    ) -> Result<()> {
        let theory = self
            .theories
            .get(theory_name)
            .ok_or_else(|| OmegaError::UnknownTheory(theory_name.to_string()))?;

        if self.use_interned {
            // Use cached InternedTheory for amortized O(1) rule interning
            if let Some(cached) = self.interned_cache.get_mut(theory_name) {
                cached.check(goal, deriv, ctx)
            } else {
                // Fallback: create a fresh one (shouldn't happen if registered properly)
                interned_check::check_derivation_interned(theory, goal, deriv, ctx)
            }
        } else {
            derivation::check_derivation(theory, goal, deriv, ctx)
        }
    }

    /// Operation 3: Verify a metatheorem.
    pub fn check_metatheorem(&mut self, mt: MetaTheorem) -> Result<()> {
        let theory = self
            .theories
            .get(&mt.theory_name)
            .ok_or_else(|| OmegaError::UnknownTheory(mt.theory_name.clone()))?;

        // Check no self-strengthening
        reflection::check_no_self_strengthening(&mt, theory)?;

        // Verify the proof
        metatheorem::verify_metatheorem(&mt, theory)?;

        // Store as verified
        let name = mt.name.clone();
        self.verified_metatheorems.insert(name, mt);
        Ok(())
    }

    /// Operation 4: Reflect a proven metatheorem as a new rule.
    pub fn reflect(&mut self, metatheorem_name: &str, rule_name: &str) -> Result<()> {
        let mt = self
            .verified_metatheorems
            .get(metatheorem_name)
            .ok_or_else(|| OmegaError::UnprovenMetatheorem(metatheorem_name.to_string()))?
            .clone();

        let theory = self
            .theories
            .get(&mt.theory_name)
            .ok_or_else(|| OmegaError::UnknownTheory(mt.theory_name.clone()))?;

        let (rule, record) = reflection::reflect(&mt, rule_name, theory)?;

        // Add the rule to the theory
        let theory = self
            .theories
            .get_mut(&mt.theory_name)
            .ok_or_else(|| OmegaError::UnknownTheory(mt.theory_name.clone()))?;

        if self.use_interned {
            if let Some(cached) = self.interned_cache.get_mut(&mt.theory_name) {
                cached.add_rule(&rule);
            }
        }
        theory.add_rule(rule)?;
        self.reflections.push(record);

        Ok(())
    }

    /// Get a reference to a registered theory.
    pub fn get_theory(&self, name: &str) -> Option<&Theory> {
        self.theories.get(name)
    }

    /// Get a mutable reference to a registered theory.
    pub fn get_theory_mut(&mut self, name: &str) -> Option<&mut Theory> {
        self.theories.get_mut(name)
    }

    /// List all registered theory names.
    pub fn theory_names(&self) -> Vec<&str> {
        self.theories.keys().map(|s| s.as_str()).collect()
    }

    /// List all verified metatheorem names.
    pub fn metatheorem_names(&self) -> Vec<&str> {
        self.verified_metatheorems.keys().map(|s| s.as_str()).collect()
    }

    /// Get reflection records for audit.
    pub fn reflections(&self) -> &[ReflectionRecord] {
        &self.reflections
    }
}

impl Default for Kernel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Expr;
    use crate::judgment::{ConstructorDecl, JudgmentForm, Rule, SortDecl};
    use crate::metatheorem::{MetaCase, MetaProof, MetaTheorem};

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
            premises: vec![Expr::app(vec![Expr::sym("proves"), Expr::meta("B")])],
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
    fn full_kernel_workflow() {
        let mut kernel = Kernel::new();

        // 1. Register theory
        let theory = make_prop_logic();
        kernel.register_theory(theory).unwrap();

        // 2. Check a derivation: prove (and p q) from (proves p) and (proves q)
        let goal = Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::free("p"), Expr::free("q")]),
        ]);
        let deriv = Derivation::RuleApp {
            rule_name: "and-intro".to_string(),
            premises: vec![Derivation::Assumption, Derivation::Assumption],
        };
        let ctx = Context::with_assumptions(vec![
            Expr::app(vec![Expr::sym("proves"), Expr::free("p")]),
            Expr::app(vec![Expr::sym("proves"), Expr::free("q")]),
        ]);
        kernel
            .check_derivation("PropLogic", &goal, &deriv, &ctx)
            .unwrap();

        // 3. Verify a metatheorem: and-comm
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
                        args: vec![
                            MetaProof::Var("D2".to_string()),
                            MetaProof::Var("D1".to_string()),
                        ],
                    },
                }],
            },
        };
        kernel.check_metatheorem(mt).unwrap();

        // 4. Reflect the metatheorem as a new rule
        kernel.reflect("and-comm", "proves/and-comm").unwrap();

        // Verify the rule exists
        let theory = kernel.get_theory("PropLogic").unwrap();
        let rule = theory.get_rule("proves/and-comm").unwrap();
        assert!(rule.reflected);
        assert_eq!(rule.provenance, Some("and-comm".to_string()));

        // 5. Use the reflected rule in a proof
        let goal2 = Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::free("q"), Expr::free("p")]),
        ]);
        let deriv2 = Derivation::RuleApp {
            rule_name: "proves/and-comm".to_string(),
            premises: vec![Derivation::Assumption],
        };
        let ctx2 = Context::with_assumptions(vec![Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::free("p"), Expr::free("q")]),
        ])]);
        kernel
            .check_derivation("PropLogic", &goal2, &deriv2, &ctx2)
            .unwrap();
    }

    #[test]
    fn reject_unregistered_theory() {
        let mut kernel = Kernel::new();
        let goal = Expr::sym("whatever");
        let deriv = Derivation::Assumption;
        let ctx = Context::new();
        assert!(matches!(
            kernel.check_derivation("Nonexistent", &goal, &deriv, &ctx),
            Err(OmegaError::UnknownTheory(_))
        ));
    }
}
