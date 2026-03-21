//! User-declared assertions: formal claims about epistemic configurations.
//!
//! Syntax:
//!   [Assert [dominates Explorer Certifier]]
//!   [Assert [dominates Explorer Certifier :class Equational]]
//!   [Assert [preserves TheoremPipeline Soundness]]
//!   [Assert [distance Explorer Executor :max 10]]
//!   [Assert [faithful DiscoverTunnel]]
//!
//! Each assertion is checked against the current session state and either
//! passes or fails with a structured explanation.

use std::collections::HashMap;

use apeiron::parser::Sexp;

use crate::error::{MetacosmError, Result};
use crate::morphism::WorldMorphism;
use crate::world::{FamilyDef, WorldDef};

/// A parsed assertion.
#[derive(Debug, Clone)]
pub enum Assertion {
    /// World A's epistemic profile dominates World B's.
    Dominates {
        stronger: String,
        weaker: String,
        class: Option<String>,
    },
    /// A family/pipeline preserves an invariant through all its transitions.
    Preserves {
        family: String,
        invariant: String,
    },
    /// Epistemic distance between two worlds is at most N.
    Distance {
        from: String,
        to: String,
        max: u32,
    },
    /// A morphism/transition is faithful.
    Faithful {
        morphism: String,
    },
    /// A morphism/transition is full.
    Full {
        morphism: String,
    },
    /// A morphism/transition preserves categorical structure.
    StructurePreserving {
        morphism: String,
    },
    /// A transition preserves a specific invariant.
    PreservesTransition {
        transition: String,
        invariant: String,
    },
    /// A world has decidable termination.
    TerminationDecidable {
        world: String,
    },
}

/// Result of checking an assertion.
#[derive(Debug, Clone)]
pub struct AssertionResult {
    pub assertion: String,
    pub passed: bool,
    pub detail: String,
}

impl std::fmt::Display for AssertionResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.passed {
            write!(f, "PASS: {} — {}", self.assertion, self.detail)
        } else {
            write!(f, "FAIL: {} — {}", self.assertion, self.detail)
        }
    }
}

/// Parse an `[Assert (kind args...)]` block.
pub fn parse_assertion(items: &[Sexp]) -> Result<Assertion> {
    if items.len() < 2 {
        return Err(MetacosmError::ParseError {
            block: "Assert".into(),
            detail: "expected assertion body".into(),
        });
    }

    let body = items[1].as_list().ok_or_else(|| MetacosmError::ParseError {
        block: "Assert".into(),
        detail: "assertion body must be a list".into(),
    })?;

    if body.is_empty() {
        return Err(MetacosmError::ParseError {
            block: "Assert".into(),
            detail: "empty assertion".into(),
        });
    }

    let kind = body[0].as_atom().unwrap_or("");
    match kind {
        "dominates" => {
            if body.len() < 3 {
                return Err(MetacosmError::ParseError {
                    block: "Assert".into(),
                    detail: "dominates requires two world names".into(),
                });
            }
            let stronger = body[1].as_atom().unwrap_or("").to_string();
            let weaker = body[2].as_atom().unwrap_or("").to_string();

            // Optional :class
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
                    block: "Assert".into(),
                    detail: "preserves requires family and invariant names".into(),
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
                    block: "Assert".into(),
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
        "faithful" => {
            if body.len() < 2 {
                return Err(MetacosmError::ParseError {
                    block: "Assert".into(),
                    detail: "faithful requires morphism name".into(),
                });
            }
            Ok(Assertion::Faithful {
                morphism: body[1].as_atom().unwrap_or("").to_string(),
            })
        }
        "full" => {
            if body.len() < 2 {
                return Err(MetacosmError::ParseError {
                    block: "Assert".into(),
                    detail: "full requires morphism name".into(),
                });
            }
            Ok(Assertion::Full {
                morphism: body[1].as_atom().unwrap_or("").to_string(),
            })
        }
        "structure-preserving" => {
            if body.len() < 2 {
                return Err(MetacosmError::ParseError {
                    block: "Assert".into(),
                    detail: "structure-preserving requires morphism name".into(),
                });
            }
            Ok(Assertion::StructurePreserving {
                morphism: body[1].as_atom().unwrap_or("").to_string(),
            })
        }
        "preserves-transition" => {
            if body.len() < 3 {
                return Err(MetacosmError::ParseError {
                    block: "Assert".into(),
                    detail: "preserves-transition requires transition and invariant names".into(),
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
                    block: "Assert".into(),
                    detail: "termination-decidable requires world name".into(),
                });
            }
            Ok(Assertion::TerminationDecidable {
                world: body[1].as_atom().unwrap_or("").to_string(),
            })
        }
        _ => Err(MetacosmError::ParseError {
            block: "Assert".into(),
            detail: format!("unknown assertion kind: '{}'", kind),
        }),
    }
}

/// Check an assertion against session state.
pub fn check_assertion(
    assertion: &Assertion,
    worlds: &HashMap<String, WorldDef>,
    families: &HashMap<String, FamilyDef>,
    transitions: &HashMap<String, crate::transition::TransitionDef>,
    morphisms: &HashMap<String, WorldMorphism>,
) -> AssertionResult {
    match assertion {
        Assertion::Dominates { stronger, weaker, class } => {
            let (Some(s), Some(w)) = (worlds.get(stronger), worlds.get(weaker)) else {
                return AssertionResult {
                    assertion: format!("dominates({}, {})", stronger, weaker),
                    passed: false,
                    detail: "world not found".into(),
                };
            };

            let (s_ep, w_ep) = if let Some(class_name) = class {
                let tc = crate::theorem_class::parse_theorem_class(class_name).unwrap();
                (s.epistemic.for_class(&tc), w.epistemic.for_class(&tc))
            } else {
                (s.epistemic.clone(), w.epistemic.clone())
            };

            let class_str = class.as_ref().map(|c| format!(" [class={}]", c)).unwrap_or_default();
            if s_ep.dominates(&w_ep) {
                AssertionResult {
                    assertion: format!("dominates({}, {}{})", stronger, weaker, class_str),
                    passed: true,
                    detail: format!("{} epistemically dominates {}", stronger, weaker),
                }
            } else {
                AssertionResult {
                    assertion: format!("dominates({}, {}{})", stronger, weaker, class_str),
                    passed: false,
                    detail: format!("{} does not dominate {} on all axes", stronger, weaker),
                }
            }
        }

        Assertion::Preserves { family, invariant } => {
            let fam = match families.get(family) {
                Some(f) => f,
                None => return AssertionResult {
                    assertion: format!("preserves({}, {})", family, invariant),
                    passed: false,
                    detail: format!("family '{}' not found", family),
                },
            };

            let inv = crate::transition::parse_invariant(invariant);

            // Check all transitions within the family
            for t in transitions.values() {
                if fam.worlds.contains(&t.source) && fam.worlds.contains(&t.target) {
                    if t.breaks.contains(&inv) {
                        return AssertionResult {
                            assertion: format!("preserves({}, {})", family, invariant),
                            passed: false,
                            detail: format!("transition {} breaks {}", t.name, invariant),
                        };
                    }
                }
            }

            AssertionResult {
                assertion: format!("preserves({}, {})", family, invariant),
                passed: true,
                detail: format!("{} preserved through all transitions in {}", invariant, family),
            }
        }

        Assertion::Distance { from, to, max } => {
            let (Some(f), Some(t)) = (worlds.get(from), worlds.get(to)) else {
                return AssertionResult {
                    assertion: format!("distance({}, {}, max={})", from, to, max),
                    passed: false,
                    detail: "world not found".into(),
                };
            };

            let dist = f.epistemic.distance(&t.epistemic);
            if dist <= *max {
                AssertionResult {
                    assertion: format!("distance({}, {}, max={})", from, to, max),
                    passed: true,
                    detail: format!("distance = {} ≤ {}", dist, max),
                }
            } else {
                AssertionResult {
                    assertion: format!("distance({}, {}, max={})", from, to, max),
                    passed: false,
                    detail: format!("distance = {} > {}", dist, max),
                }
            }
        }

        Assertion::Faithful { morphism } => {
            check_morphism_property(morphism, morphisms, "faithful", |m| m.properties.faithful)
        }

        Assertion::Full { morphism } => {
            check_morphism_property(morphism, morphisms, "full", |m| m.properties.full)
        }

        Assertion::StructurePreserving { morphism } => {
            check_morphism_property(morphism, morphisms, "structure-preserving", |m| {
                !m.properties.preserves_structure.is_empty()
            })
        }

        Assertion::PreservesTransition { transition, invariant } => {
            let inv = crate::transition::parse_invariant(invariant);
            match transitions.get(transition) {
                Some(t) => {
                    if t.breaks.contains(&inv) {
                        AssertionResult {
                            assertion: format!("preserves-transition({}, {})", transition, invariant),
                            passed: false,
                            detail: format!("transition {} breaks {}", transition, invariant),
                        }
                    } else if t.preserves.contains(&inv) {
                        AssertionResult {
                            assertion: format!("preserves-transition({}, {})", transition, invariant),
                            passed: true,
                            detail: format!("transition {} preserves {}", transition, invariant),
                        }
                    } else {
                        AssertionResult {
                            assertion: format!("preserves-transition({}, {})", transition, invariant),
                            passed: false,
                            detail: format!("transition {} does not declare preserving {}", transition, invariant),
                        }
                    }
                }
                None => AssertionResult {
                    assertion: format!("preserves-transition({}, {})", transition, invariant),
                    passed: false,
                    detail: format!("transition '{}' not found", transition),
                },
            }
        }

        Assertion::TerminationDecidable { world } => {
            match worlds.get(world) {
                Some(w) => {
                    let decidable = w.epistemic.verify.termination == crate::epistemic::Termination::Decidable;
                    AssertionResult {
                        assertion: format!("termination-decidable({})", world),
                        passed: decidable,
                        detail: if decidable {
                            format!("{} has decidable termination", world)
                        } else {
                            format!("{} does not have decidable termination (termination={:?})", world, w.epistemic.verify.termination)
                        },
                    }
                }
                None => AssertionResult {
                    assertion: format!("termination-decidable({})", world),
                    passed: false,
                    detail: format!("world '{}' not found", world),
                },
            }
        }
    }
}

fn check_morphism_property(
    name: &str,
    morphisms: &HashMap<String, WorldMorphism>,
    prop: &str,
    check: impl Fn(&WorldMorphism) -> bool,
) -> AssertionResult {
    match morphisms.get(name) {
        Some(m) => {
            if check(m) {
                AssertionResult {
                    assertion: format!("{}({})", prop, name),
                    passed: true,
                    detail: format!("{} is {}", name, prop),
                }
            } else {
                AssertionResult {
                    assertion: format!("{}({})", prop, name),
                    passed: false,
                    detail: format!("{} is not {}", name, prop),
                }
            }
        }
        None => AssertionResult {
            assertion: format!("{}({})", prop, name),
            passed: false,
            detail: format!("morphism '{}' not found", name),
        },
    }
}
