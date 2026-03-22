//! Impossibility proofs: prove that certain epistemic configurations
//! are unreachable in the current system.
//!
//! Syntax:
//!   [Refute NoLosslessSoundCollapse
//!     :forall [W1 W2]
//!     :impossible [
//!       [dominates W2 W1]
//!       [distance W1 W2 :max 0]
//!     ]
//!     :method model-check
//!   ]
//!
//! Like Law, distinguishes model-checking (search over registered worlds)
//! from structural proof (algebraic impossibility argument).
//!
//! Model-checking is honest: "no registered configuration satisfies this"
//! is NOT the same as "no configuration CAN satisfy this."

use std::collections::HashMap;

use apeiron::parser::Sexp;

use crate::assertion::{self, Assertion};
use crate::error::{MetacosmError, Result};
use crate::law::{parse_assertion_body, VerificationMethod};
use crate::metatheory::{Counterexample, ProofCertificate, ProofResult, Witness};
use crate::morphism::WorldMorphism;
use crate::transition::TransitionDef;
use crate::world::{FamilyDef, WorldDef};

/// An impossibility proof declaration.
#[derive(Debug, Clone)]
pub struct ImpossibilityProof {
    pub name: String,
    /// Quantified variable names
    pub quantified: Vec<String>,
    /// Typed variable bindings (for proof mode)
    pub variable_types: Vec<(String, crate::proof_engine::VarType)>,
    /// Conditions that must ALL hold simultaneously for a counterexample
    pub conditions: Vec<Assertion>,
    /// How to verify
    pub method: VerificationMethod,
    /// Tactic proof script (for :method proof)
    pub proof_script: Vec<crate::proof_engine::Tactic>,
}

/// Result of an impossibility check.
#[derive(Debug, Clone)]
pub struct RefuteResult {
    pub name: String,
    /// True if no satisfying assignment was found
    pub confirmed: bool,
    /// The proof or counterexample
    pub proof: ProofResult,
}

/// Parse a `[Refute Name :forall [...] :impossible [[...] ...] :method M]` block.
pub fn parse_refutation(items: &[Sexp]) -> Result<ImpossibilityProof> {
    if items.len() < 2 {
        return Err(MetacosmError::ParseError {
            block: "Refute".into(),
            detail: "missing refutation name".into(),
        });
    }

    let name = items[1].as_atom().ok_or_else(|| MetacosmError::ParseError {
        block: "Refute".into(),
        detail: "refutation name must be an atom".into(),
    })?.to_string();

    let mut quantified = Vec::new();
    let mut variable_types = Vec::new();
    let mut conditions = Vec::new();
    let mut method = VerificationMethod::ModelCheck;
    let mut proof_script = Vec::new();

    let mut i = 2;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":forall" => {
                i += 1;
                if let Some(vars) = items.get(i).and_then(|s| s.as_list()) {
                    let has_type = vars.iter().any(|v| v.as_atom() == Some(":type"));
                    if has_type {
                        variable_types = crate::proof_engine::parse_typed_forall(vars)?;
                        quantified = variable_types.iter().map(|(n, _)| n.clone()).collect();
                    } else {
                        for v in vars {
                            if let Some(name) = v.as_atom() {
                                quantified.push(name.to_string());
                            }
                        }
                    }
                }
            }
            ":impossible" | ":assume" => {
                i += 1;
                if let Some(cond_list) = items.get(i).and_then(|s| s.as_list()) {
                    for c in cond_list {
                        if let Some(body) = c.as_list() {
                            conditions.push(parse_assertion_body(body)?);
                        }
                    }
                }
            }
            ":method" => {
                i += 1;
                if let Some(m) = items.get(i).and_then(|s| s.as_atom()) {
                    method = match m {
                        "model-check" => VerificationMethod::ModelCheck,
                        "structural" => VerificationMethod::Structural,
                        "proof" => VerificationMethod::Proof,
                        _ => return Err(MetacosmError::ParseError {
                            block: "Refute".into(),
                            detail: format!("unknown method: '{}' (expected model-check, structural, or proof)", m),
                        }),
                    };
                }
            }
            ":proof" => {
                i += 1;
                if let Some(tac_list) = items.get(i).and_then(|s| s.as_list()) {
                    proof_script = crate::proof_engine::parse_tactics(tac_list)?;
                }
            }
            _ => {
                return Err(MetacosmError::ParseError {
                    block: "Refute".into(),
                    detail: format!("unknown keyword: {}", key),
                });
            }
        }
        i += 1;
    }

    if conditions.is_empty() {
        return Err(MetacosmError::ParseError {
            block: "Refute".into(),
            detail: "missing :impossible conditions".into(),
        });
    }

    Ok(ImpossibilityProof {
        name,
        quantified,
        variable_types,
        conditions,
        method,
        proof_script,
    })
}

/// Check an impossibility proof against the current session state.
pub fn check_impossibility(
    proof: &ImpossibilityProof,
    worlds: &HashMap<String, WorldDef>,
    families: &HashMap<String, FamilyDef>,
    transitions: &HashMap<String, TransitionDef>,
    morphisms: &HashMap<String, WorldMorphism>,
) -> RefuteResult {
    match proof.method {
        VerificationMethod::ModelCheck => {
            model_check_impossibility(proof, worlds, families, transitions, morphisms)
        }
        VerificationMethod::Structural => {
            structural_check_impossibility(proof)
        }
        VerificationMethod::Proof => {
            proof_check_impossibility(proof)
        }
    }
}

/// Tactic-based impossibility proof: the goal is Contradiction.
fn proof_check_impossibility(proof: &ImpossibilityProof) -> RefuteResult {
    use crate::proof_engine;

    let var_types = if !proof.variable_types.is_empty() {
        proof.variable_types.clone()
    } else {
        proof.quantified.iter()
            .map(|n| (n.clone(), proof_engine::VarType::World))
            .collect()
    };

    // All conditions become hypotheses; goal is Contradiction
    let premises: Vec<(String, proof_engine::Prop)> = proof.conditions.iter()
        .enumerate()
        .map(|(i, a)| (format!("h{}", i + 1), proof_engine::assertion_to_prop(a)))
        .collect();

    let state = proof_engine::ProofState::new(
        var_types,
        premises,
        proof_engine::Prop::Contradiction,
    );

    match proof_engine::evaluate_proof(&proof.name, state, &proof.proof_script) {
        Ok(cert) => RefuteResult {
            name: proof.name.clone(),
            confirmed: true,
            proof: ProofResult::Proved(ProofCertificate {
                theorem: cert.theorem,
                witness: Witness::ByConstruction(
                    format!("impossibility proved by tactic script ({} steps):\n{}",
                        cert.trace.len(),
                        cert.trace.join("\n")
                    ),
                ),
            }),
        },
        Err(e) => RefuteResult {
            name: proof.name.clone(),
            confirmed: false,
            proof: ProofResult::Refuted(Counterexample {
                theorem: proof.name.clone(),
                detail: format!("{}", e),
                witness: vec![],
            }),
        },
    }
}

/// Structural impossibility proof for known patterns.
///
/// Recognizes impossibility claims provable by algebraic argument:
/// - Contradictory dominance: dominates(W1, W2) ∧ dominates(W2, W1) ∧ distance(W1, W2, max=0)
///   with distinct W1, W2 — impossible only if distinctness is required, but reflexive pairs satisfy this.
///   So this is NOT structurally provable in general.
/// - Axis contradiction: conditions requiring incompatible axis values on the same world
///   (e.g., discover=complete AND discover=none simultaneously)
///
/// Unknown patterns are rejected honestly.
fn structural_check_impossibility(proof: &ImpossibilityProof) -> RefuteResult {
    // Check for axis contradictions: if conditions require the same world variable
    // to satisfy incompatible axis values, that's structurally impossible.
    // e.g., dominates(W, Lab) requires W.discover >= complete,
    //        dominates(Deploy, W) requires W.discover <= none (if Deploy has none)
    // This is the pattern used in CompleteDiscoveryWithCodegen.

    // For now, recognize the pattern: dominates(W, X) ∧ dominates(Y, W)
    // where X and Y have incompatible axis values. This requires actual world data,
    // which we don't have in structural mode.

    // Without world data, structural impossibility must rely purely on form.
    // The only structurally provable impossibility is a logical contradiction in the conditions.

    // Check for trivial contradiction: dominates(A, B) ∧ dominates(B, A) with A≠B
    // is NOT a contradiction (mutual dominance = equivalence).

    // Honest answer: structural impossibility proofs require algebraic reasoning
    // about specific axis values, which needs world data. Reject honestly.
    RefuteResult {
        name: proof.name.clone(),
        confirmed: false,
        proof: ProofResult::Refuted(Counterexample {
            theorem: proof.name.clone(),
            detail: "structural impossibility proof not available for this pattern — use :method model-check instead".into(),
            witness: vec![],
        }),
    }
}

/// Model-check: search for any assignment that satisfies ALL conditions.
fn model_check_impossibility(
    proof: &ImpossibilityProof,
    worlds: &HashMap<String, WorldDef>,
    families: &HashMap<String, FamilyDef>,
    transitions: &HashMap<String, TransitionDef>,
    morphisms: &HashMap<String, WorldMorphism>,
) -> RefuteResult {
    let world_names: Vec<&String> = worlds.keys().collect();
    let n = proof.quantified.len();

    let assignments = generate_assignments(&world_names, n);
    let total = assignments.len();

    for assignment in &assignments {
        let subst: HashMap<&str, &str> = proof.quantified.iter()
            .zip(assignment.iter())
            .map(|(var, world)| (var.as_str(), world.as_str()))
            .collect();

        // Check if ALL conditions hold for this assignment
        let all_hold = proof.conditions.iter().all(|cond| {
            let instantiated = substitute_assertion(cond, &subst);
            let result = assertion::check_assertion(&instantiated, worlds, families, transitions, morphisms);
            result.passed
        });

        if all_hold {
            // Found a satisfying assignment — impossibility is REFUTED
            let witness_pairs: Vec<(String, String)> = proof.quantified.iter()
                .zip(assignment.iter())
                .map(|(var, world)| (var.clone(), world.clone()))
                .collect();

            return RefuteResult {
                name: proof.name.clone(),
                confirmed: false,
                proof: ProofResult::Refuted(Counterexample {
                    theorem: proof.name.clone(),
                    detail: format!(
                        "configuration exists: all {} conditions satisfied simultaneously",
                        proof.conditions.len()
                    ),
                    witness: witness_pairs,
                }),
            };
        }
    }

    // No satisfying assignment found
    RefuteResult {
        name: proof.name.clone(),
        confirmed: true,
        proof: ProofResult::Proved(ProofCertificate {
            theorem: proof.name.clone(),
            witness: Witness::ForAll {
                domain: "registered worlds".into(),
                count: total,
                detail: format!(
                    "checked {} assignments, none satisfy all {} conditions (model-check, not universal proof)",
                    total, proof.conditions.len()
                ),
            },
        }),
    }
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

/// Substitute quantified variable names in an assertion.
fn substitute_assertion(assertion: &Assertion, subst: &HashMap<&str, &str>) -> Assertion {
    crate::law::substitute_assertion_pub(assertion, subst)
}
