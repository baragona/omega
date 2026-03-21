//! Cosmological laws: universal properties about all admissible worlds.
//!
//! Syntax:
//!   [Law SoundnessMonotonicity
//!     :forall [W1 W2]
//!     :where [[dominates W1 W2]]
//!     :then [dominates W1 W2 :class Equational]
//!     :method model-check
//!   ]
//!
//! Critical distinction:
//!   - model-check: checks all registered worlds (search over examples)
//!   - structural: proves by algebraic argument (true metatheory)
//!
//! Model-checking is honest: it reports "holds for N registered worlds"
//! rather than claiming universal truth.

use std::collections::HashMap;

use apeiron::parser::Sexp;

use crate::assertion::{self, Assertion};
use crate::error::{MetacosmError, Result};
use crate::metatheory::{Counterexample, ProofCertificate, ProofResult, Witness};
use crate::morphism::WorldMorphism;
use crate::transition::TransitionDef;
use crate::world::{FamilyDef, WorldDef};

/// How the law is verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationMethod {
    /// Check all N-tuples of registered worlds. Honest: reports count.
    ModelCheck,
    /// Structural/algebraic proof. True metatheory.
    Structural,
}

impl std::fmt::Display for VerificationMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationMethod::ModelCheck => write!(f, "model-check"),
            VerificationMethod::Structural => write!(f, "structural"),
        }
    }
}

/// A cosmological law declaration.
#[derive(Debug, Clone)]
pub struct CosmologicalLaw {
    pub name: String,
    /// Quantified variable names (bound to worlds during checking)
    pub quantified: Vec<String>,
    /// Premises: assertions that must hold for the law to apply
    pub premises: Vec<Assertion>,
    /// Conclusion: assertion that should follow
    pub conclusion: Assertion,
    /// How to verify
    pub method: VerificationMethod,
}

/// Parse a `[Law Name :forall [...] :where [[...] ...] :then [...] :method M]` block.
pub fn parse_law(items: &[Sexp]) -> Result<CosmologicalLaw> {
    if items.len() < 2 {
        return Err(MetacosmError::ParseError {
            block: "Law".into(),
            detail: "missing law name".into(),
        });
    }

    let name = items[1].as_atom().ok_or_else(|| MetacosmError::ParseError {
        block: "Law".into(),
        detail: "law name must be an atom".into(),
    })?.to_string();

    let mut quantified = Vec::new();
    let mut premises = Vec::new();
    let mut conclusion: Option<Assertion> = None;
    let mut method = VerificationMethod::ModelCheck;

    let mut i = 2;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":forall" => {
                i += 1;
                if let Some(vars) = items.get(i).and_then(|s| s.as_list()) {
                    for v in vars {
                        if let Some(name) = v.as_atom() {
                            quantified.push(name.to_string());
                        }
                    }
                }
            }
            ":where" => {
                i += 1;
                if let Some(prem_list) = items.get(i).and_then(|s| s.as_list()) {
                    for p in prem_list {
                        if let Some(body) = p.as_list() {
                            premises.push(parse_assertion_body(body)?);
                        }
                    }
                }
            }
            ":then" => {
                i += 1;
                if let Some(body) = items.get(i).and_then(|s| s.as_list()) {
                    conclusion = Some(parse_assertion_body(body)?);
                }
            }
            ":method" => {
                i += 1;
                if let Some(m) = items.get(i).and_then(|s| s.as_atom()) {
                    method = match m {
                        "model-check" => VerificationMethod::ModelCheck,
                        "structural" => VerificationMethod::Structural,
                        _ => return Err(MetacosmError::ParseError {
                            block: "Law".into(),
                            detail: format!("unknown method: '{}' (expected model-check or structural)", m),
                        }),
                    };
                }
            }
            _ => {
                return Err(MetacosmError::ParseError {
                    block: "Law".into(),
                    detail: format!("unknown keyword: {}", key),
                });
            }
        }
        i += 1;
    }

    let conclusion = conclusion.ok_or_else(|| MetacosmError::ParseError {
        block: "Law".into(),
        detail: "missing :then".into(),
    })?;

    Ok(CosmologicalLaw {
        name,
        quantified,
        premises,
        conclusion,
        method,
    })
}

/// Check a cosmological law against the current session state.
pub fn check_law(
    law: &CosmologicalLaw,
    worlds: &HashMap<String, WorldDef>,
    families: &HashMap<String, FamilyDef>,
    transitions: &HashMap<String, TransitionDef>,
    morphisms: &HashMap<String, WorldMorphism>,
) -> ProofResult {
    match law.method {
        VerificationMethod::ModelCheck => {
            model_check_law(law, worlds, families, transitions, morphisms)
        }
        VerificationMethod::Structural => {
            structural_check_law(law)
        }
    }
}

/// Structural (algebraic) proof for known theorem families.
///
/// Recognizes laws that are true by the algebraic structure of epistemic profiles:
/// - Dominance reflexivity: ∀W. dominates(W, W) — true because >= is reflexive on all axes
/// - Dominance transitivity: ∀X Y Z. dominates(X,Y) ∧ dominates(Y,Z) → dominates(X,Z)
/// - Preserves reflexivity: ∀F. preserves(F, I) when I ∈ F.invariants (tautological)
///
/// Laws not matching a known family are rejected with an honest error.
fn structural_check_law(law: &CosmologicalLaw) -> ProofResult {
    // Dominance reflexivity: forall [W], then [dominates W W]
    if law.quantified.len() == 1 && law.premises.is_empty() {
        if let Assertion::Dominates { ref stronger, ref weaker, class: None } = law.conclusion {
            if stronger == &law.quantified[0] && weaker == &law.quantified[0] {
                return ProofResult::Proved(ProofCertificate {
                    theorem: law.name.clone(),
                    witness: Witness::ByConstruction(
                        "dominance is reflexive: every epistemic axis has a reflexive partial order (>= is reflexive)".into()
                    ),
                });
            }
        }
    }

    // Dominance transitivity: forall [X Y Z], where [dominates X Y] [dominates Y Z], then [dominates X Z]
    if law.quantified.len() == 3 && law.premises.len() == 2 {
        if let (
            Assertion::Dominates { stronger: ref s1, weaker: ref w1, class: None },
            Assertion::Dominates { stronger: ref s2, weaker: ref w2, class: None },
        ) = (&law.premises[0], &law.premises[1]) {
            if let Assertion::Dominates { stronger: ref sc, weaker: ref wc, class: None } = law.conclusion {
                // Check pattern: dom(X,Y) ∧ dom(Y,Z) → dom(X,Z)
                // s1=X, w1=Y, s2=Y, w2=Z, sc=X, wc=Z
                if s1 == sc && w1 == s2 && w2 == wc
                    && law.quantified.contains(s1)
                    && law.quantified.contains(w1)
                    && law.quantified.contains(w2)
                {
                    return ProofResult::Proved(ProofCertificate {
                        theorem: law.name.clone(),
                        witness: Witness::ByConstruction(
                            "dominance is transitive: each axis is a partial order, and >= is transitive on all partial orders".into()
                        ),
                    });
                }
            }
        }
    }

    // Unknown structural form — reject honestly
    ProofResult::Refuted(Counterexample {
        theorem: law.name.clone(),
        detail: "structural proof not available for this law form — use :method model-check instead".into(),
        witness: vec![],
    })
}

/// Model-check a law over all registered worlds.
fn model_check_law(
    law: &CosmologicalLaw,
    worlds: &HashMap<String, WorldDef>,
    families: &HashMap<String, FamilyDef>,
    transitions: &HashMap<String, TransitionDef>,
    morphisms: &HashMap<String, WorldMorphism>,
) -> ProofResult {
    let world_names: Vec<&String> = worlds.keys().collect();
    let n = law.quantified.len();

    if n == 0 {
        // No quantification — just check the conclusion directly
        let result = assertion::check_assertion(&law.conclusion, worlds, families, transitions, morphisms);
        if result.passed {
            return ProofResult::Proved(ProofCertificate {
                theorem: law.name.clone(),
                witness: Witness::ByConstruction("no quantification, conclusion holds".into()),
            });
        } else {
            return ProofResult::Refuted(Counterexample {
                theorem: law.name.clone(),
                detail: result.detail,
                witness: vec![],
            });
        }
    }

    // Generate all n-tuples of worlds
    let mut checked = 0usize;
    let mut satisfied = 0usize;

    let assignments = generate_assignments(&world_names, n);

    for assignment in &assignments {
        // Build a substitution map: quantified var -> world name
        let subst: HashMap<&str, &str> = law.quantified.iter()
            .zip(assignment.iter())
            .map(|(var, world)| (var.as_str(), world.as_str()))
            .collect();

        // Check all premises with this assignment
        let premises_hold = law.premises.iter().all(|prem| {
            let instantiated = substitute_assertion(prem, &subst);
            let result = assertion::check_assertion(&instantiated, worlds, families, transitions, morphisms);
            result.passed
        });

        if !premises_hold {
            continue; // Premises don't hold, this assignment is irrelevant
        }

        checked += 1;

        // Check conclusion
        let instantiated_conclusion = substitute_assertion(&law.conclusion, &subst);
        let result = assertion::check_assertion(&instantiated_conclusion, worlds, families, transitions, morphisms);

        if result.passed {
            satisfied += 1;
        } else {
            // Found a counterexample
            let witness_pairs: Vec<(String, String)> = law.quantified.iter()
                .zip(assignment.iter())
                .map(|(var, world)| (var.clone(), world.clone()))
                .collect();

            return ProofResult::Refuted(Counterexample {
                theorem: law.name.clone(),
                detail: format!("counterexample found: {} — {}", result.assertion, result.detail),
                witness: witness_pairs,
            });
        }
    }

    ProofResult::Proved(ProofCertificate {
        theorem: law.name.clone(),
        witness: Witness::ForAll {
            domain: "registered worlds".into(),
            count: checked,
            detail: format!(
                "checked {} relevant assignments ({} total), all {} satisfied (model-check, not universal proof)",
                checked, assignments.len(), satisfied
            ),
        },
    })
}

/// Generate all n-tuples from a list (with repetition).
fn generate_assignments(items: &[&String], n: usize) -> Vec<Vec<String>> {
    if n == 0 {
        return vec![vec![]];
    }
    let sub = generate_assignments(items, n - 1);
    let mut result = Vec::new();
    for item in items {
        for s in &sub {
            let mut v = vec![item.to_string()];
            v.extend(s.clone());
            result.push(v);
        }
    }
    result
}

/// Substitute quantified variable names in an assertion (public for reuse by refute.rs).
pub fn substitute_assertion_pub(assertion: &Assertion, subst: &HashMap<&str, &str>) -> Assertion {
    substitute_assertion(assertion, subst)
}

/// Substitute quantified variable names in an assertion.
fn substitute_assertion(assertion: &Assertion, subst: &HashMap<&str, &str>) -> Assertion {
    match assertion {
        Assertion::Dominates { stronger, weaker, class } => Assertion::Dominates {
            stronger: subst.get(stronger.as_str()).unwrap_or(&stronger.as_str()).to_string(),
            weaker: subst.get(weaker.as_str()).unwrap_or(&weaker.as_str()).to_string(),
            class: class.clone(),
        },
        Assertion::Preserves { family, invariant } => Assertion::Preserves {
            family: subst.get(family.as_str()).unwrap_or(&family.as_str()).to_string(),
            invariant: invariant.clone(),
        },
        Assertion::Distance { from, to, max } => Assertion::Distance {
            from: subst.get(from.as_str()).unwrap_or(&from.as_str()).to_string(),
            to: subst.get(to.as_str()).unwrap_or(&to.as_str()).to_string(),
            max: *max,
        },
        Assertion::Faithful { morphism } => Assertion::Faithful {
            morphism: subst.get(morphism.as_str()).unwrap_or(&morphism.as_str()).to_string(),
        },
        Assertion::Full { morphism } => Assertion::Full {
            morphism: subst.get(morphism.as_str()).unwrap_or(&morphism.as_str()).to_string(),
        },
        Assertion::StructurePreserving { morphism } => Assertion::StructurePreserving {
            morphism: subst.get(morphism.as_str()).unwrap_or(&morphism.as_str()).to_string(),
        },
        Assertion::PreservesTransition { transition, invariant } => Assertion::PreservesTransition {
            transition: subst.get(transition.as_str()).unwrap_or(&transition.as_str()).to_string(),
            invariant: invariant.clone(),
        },
        Assertion::TerminationDecidable { world } => Assertion::TerminationDecidable {
            world: subst.get(world.as_str()).unwrap_or(&world.as_str()).to_string(),
        },
    }
}

/// Parse an assertion body directly (shared with promote.rs).
pub fn parse_assertion_body(body: &[Sexp]) -> Result<Assertion> {
    if body.is_empty() {
        return Err(MetacosmError::ParseError {
            block: "Law".into(),
            detail: "empty assertion body".into(),
        });
    }

    let kind = body[0].as_atom().unwrap_or("");
    match kind {
        "dominates" => {
            if body.len() < 3 {
                return Err(MetacosmError::ParseError {
                    block: "Law".into(),
                    detail: "dominates requires two world names".into(),
                });
            }
            let stronger = body[1].as_atom().unwrap_or("").to_string();
            let weaker = body[2].as_atom().unwrap_or("").to_string();
            let mut class = None;
            let mut i = 3;
            while i < body.len() {
                if body[i].as_atom() == Some(":class") {
                    i += 1;
                    class = body.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
                }
                i += 1;
            }
            Ok(Assertion::Dominates { stronger, weaker, class })
        }
        "preserves" => {
            if body.len() < 3 {
                return Err(MetacosmError::ParseError {
                    block: "Law".into(),
                    detail: "preserves requires family and invariant".into(),
                });
            }
            Ok(Assertion::Preserves {
                family: body[1].as_atom().unwrap_or("").to_string(),
                invariant: body[2].as_atom().unwrap_or("").to_string(),
            })
        }
        "distance" => {
            if body.len() < 3 {
                return Err(MetacosmError::ParseError {
                    block: "Law".into(),
                    detail: "distance requires two world names".into(),
                });
            }
            let from = body[1].as_atom().unwrap_or("").to_string();
            let to = body[2].as_atom().unwrap_or("").to_string();
            let mut max = u32::MAX;
            let mut i = 3;
            while i < body.len() {
                if body[i].as_atom() == Some(":max") {
                    i += 1;
                    if let Some(v) = body.get(i).and_then(|s| s.as_atom()) {
                        max = v.parse().unwrap_or(u32::MAX);
                    }
                }
                i += 1;
            }
            Ok(Assertion::Distance { from, to, max })
        }
        "preserves-transition" => {
            if body.len() < 3 {
                return Err(MetacosmError::ParseError {
                    block: "Law".into(),
                    detail: "preserves-transition requires transition and invariant".into(),
                });
            }
            Ok(Assertion::PreservesTransition {
                transition: body[1].as_atom().unwrap_or("").to_string(),
                invariant: body[2].as_atom().unwrap_or("").to_string(),
            })
        }
        "termination-decidable" => {
            if body.len() < 2 {
                return Err(MetacosmError::ParseError {
                    block: "Law".into(),
                    detail: "termination-decidable requires world name".into(),
                });
            }
            Ok(Assertion::TerminationDecidable {
                world: body[1].as_atom().unwrap_or("").to_string(),
            })
        }
        "faithful" | "full" | "structure-preserving" => {
            if body.len() < 2 {
                return Err(MetacosmError::ParseError {
                    block: "Law".into(),
                    detail: format!("{} requires morphism name", kind),
                });
            }
            let morphism = body[1].as_atom().unwrap_or("").to_string();
            match kind {
                "faithful" => Ok(Assertion::Faithful { morphism }),
                "full" => Ok(Assertion::Full { morphism }),
                "structure-preserving" => Ok(Assertion::StructurePreserving { morphism }),
                _ => unreachable!(),
            }
        }
        _ => Err(MetacosmError::ParseError {
            block: "Law".into(),
            detail: format!("unknown assertion kind: '{}'", kind),
        }),
    }
}
