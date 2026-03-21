use apeiron::parser::Sexp;

use crate::epistemic::EpistemicProfile;
use crate::error::{MetacosmError, Result};
use crate::transition::TransitionKind;

/// A Metacosm world: the core meta-IR object.
///
/// Omega = one World, no transitions, trivial epistemic profile
/// Hyperion = static World family with explicit category/substrate
/// Full Metacosm = dynamic worlds with transitions + epistemic observables
#[derive(Debug, Clone)]
pub struct WorldDef {
    pub name: String,
    /// Mathematical content (Hyperion category name, or "Implicit" for Omega mode)
    pub category: String,
    /// Computational physics (Hyperion substrate name, or "Default" for Omega mode)
    pub substrate: String,
    /// Epistemic signature
    pub epistemic: EpistemicProfile,
    /// Admissible transition kinds
    pub admissible_transitions: Vec<TransitionKind>,
}

impl WorldDef {
    /// Create an Omega-mode world: single fixed world, trivial epistemic profile.
    pub fn omega_default(name: &str) -> Self {
        WorldDef {
            name: name.to_string(),
            category: "Implicit".to_string(),
            substrate: "Default".to_string(),
            epistemic: EpistemicProfile::trivial(),
            admissible_transitions: vec![],
        }
    }

    /// Create a Hyperion-mode world: explicit category + substrate, no cosmology.
    pub fn hyperion(name: &str, category: &str, substrate: &str) -> Self {
        WorldDef {
            name: name.to_string(),
            category: category.to_string(),
            substrate: substrate.to_string(),
            epistemic: EpistemicProfile::trivial(),
            admissible_transitions: vec![],
        }
    }

    /// Is this a trivial (Omega-mode) world?
    pub fn is_omega_mode(&self) -> bool {
        self.admissible_transitions.is_empty()
            && self.epistemic == EpistemicProfile::trivial()
            && self.category == "Implicit"
    }

    /// Is this a Hyperion-mode world? (explicit category+substrate, no cosmology)
    pub fn is_hyperion_mode(&self) -> bool {
        self.admissible_transitions.is_empty()
            && self.category != "Implicit"
    }
}

/// Parse `[World Name :category C :substrate S :epistemic [...] :admits [...]]`
pub fn parse_world(items: &[Sexp]) -> Result<WorldDef> {
    if items.len() < 2 {
        return Err(MetacosmError::ParseError {
            block: "World".into(),
            detail: "missing world name".into(),
        });
    }

    let name = items[1]
        .as_atom()
        .ok_or_else(|| MetacosmError::ParseError {
            block: "World".into(),
            detail: "world name must be an atom".into(),
        })?
        .to_string();

    let mut category = "Implicit".to_string();
    let mut substrate = "Default".to_string();
    let mut epistemic = EpistemicProfile::trivial();
    let mut admissible = Vec::new();

    let mut i = 2;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":category" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    category = v.to_string();
                }
            }
            ":substrate" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    substrate = v.to_string();
                }
            }
            ":epistemic" => {
                i += 1;
                if let Some(list) = items.get(i).and_then(|s| s.as_list()) {
                    epistemic = crate::epistemic::parse_epistemic_profile(list)?;
                }
            }
            ":admits" => {
                i += 1;
                if let Some(list) = items.get(i).and_then(|s| s.as_list()) {
                    for item in list {
                        if let Some(name) = item.as_atom() {
                            admissible.push(crate::transition::parse_transition_kind(name)?);
                        }
                    }
                }
            }
            _ => {
                return Err(MetacosmError::ParseError {
                    block: "World".into(),
                    detail: format!("unknown keyword: {}", key),
                });
            }
        }
        i += 1;
    }

    Ok(WorldDef {
        name,
        category,
        substrate,
        epistemic,
        admissible_transitions: admissible,
    })
}

/// A universe family: a named collection of worlds with shared invariants.
#[derive(Debug, Clone)]
pub struct FamilyDef {
    pub name: String,
    pub worlds: Vec<String>,
    pub invariants: Vec<crate::transition::Invariant>,
}

pub fn parse_family(items: &[Sexp]) -> Result<FamilyDef> {
    if items.len() < 2 {
        return Err(MetacosmError::ParseError {
            block: "Family".into(),
            detail: "missing family name".into(),
        });
    }

    let name = items[1]
        .as_atom()
        .ok_or_else(|| MetacosmError::ParseError {
            block: "Family".into(),
            detail: "family name must be an atom".into(),
        })?
        .to_string();

    let mut worlds = Vec::new();
    let mut invariants = Vec::new();

    let mut i = 2;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":worlds" => {
                i += 1;
                if let Some(list) = items.get(i).and_then(|s| s.as_list()) {
                    for item in list {
                        if let Some(w) = item.as_atom() {
                            worlds.push(w.to_string());
                        }
                    }
                }
            }
            ":invariants" => {
                i += 1;
                if let Some(list) = items.get(i).and_then(|s| s.as_list()) {
                    for item in list {
                        if let Some(inv) = item.as_atom() {
                            invariants.push(crate::transition::parse_invariant(inv));
                        }
                    }
                }
            }
            _ => {
                return Err(MetacosmError::ParseError {
                    block: "Family".into(),
                    detail: format!("unknown keyword: {}", key),
                });
            }
        }
        i += 1;
    }

    Ok(FamilyDef {
        name,
        worlds,
        invariants,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use apeiron::parser::parse;

    #[test]
    fn omega_mode_world() {
        let w = WorldDef::omega_default("OmegaWorld");
        assert!(w.is_omega_mode());
        assert!(!w.is_hyperion_mode());
    }

    #[test]
    fn hyperion_mode_world() {
        let w = WorldDef::hyperion("HypWorld", "CartesianClosed", "ApeironStandard");
        assert!(!w.is_omega_mode());
        assert!(w.is_hyperion_mode());
    }

    #[test]
    fn parse_world_minimal() {
        let input = "[World MyWorld :category CartesianClosed :substrate ApeironStandard]";
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let w = parse_world(items).unwrap();
        assert_eq!(w.name, "MyWorld");
        assert_eq!(w.category, "CartesianClosed");
        assert!(w.is_hyperion_mode());
    }

    #[test]
    fn parse_world_full() {
        let input = r#"[World Explorer
            :category CartesianClosed
            :substrate ApeironStandard
            :epistemic [:discovery high :verification high :canonicality low :transportability medium :compression low]
            :admits [Split Merge Tunnel]
        ]"#;
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let w = parse_world(items).unwrap();
        assert_eq!(w.name, "Explorer");
        assert_eq!(w.admissible_transitions.len(), 3);
        assert!(!w.is_omega_mode());
        assert!(!w.is_hyperion_mode()); // has transitions
    }

    #[test]
    fn parse_family_basic() {
        let input = "[Family ExploratoryWorlds :worlds [Explorer Certifier Executor] :invariants [Soundness]]";
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let fam = parse_family(items).unwrap();
        assert_eq!(fam.name, "ExploratoryWorlds");
        assert_eq!(fam.worlds, vec!["Explorer", "Certifier", "Executor"]);
    }
}
