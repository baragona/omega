use apeiron::parser::Sexp;

use crate::error::{MetacosmError, Result};
use crate::epistemic::EpistemicProfile;

/// The kinds of cosmological transitions between worlds.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TransitionKind {
    /// Conservative extension: target strictly extends source
    ConservativeExtension,
    /// Split: one world branches into specialized descendants
    Split,
    /// Merge: two worlds recombine (possible information loss)
    Merge,
    /// Tunnel: theorem moves to a world where it wasn't discoverable but is verifiable
    Tunnel,
    /// Collapse: higher structure contracts (e.g., homotopical → extensional)
    Collapse,
    /// Refinement: world gains precision without losing content
    Refinement,
    /// Quotient: world collapses distinctions
    Quotient,
    /// Transport: structure-preserving map (functorial)
    Transport,
    /// PhaseTransition: substrate changes qualitatively (e.g., e-graph → rewriting)
    PhaseTransition,
    /// CoarseGrain: compress a rich world into a simpler effective world
    CoarseGrain,
}

impl std::fmt::Display for TransitionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransitionKind::ConservativeExtension => write!(f, "ConservativeExtension"),
            TransitionKind::Split => write!(f, "Split"),
            TransitionKind::Merge => write!(f, "Merge"),
            TransitionKind::Tunnel => write!(f, "Tunnel"),
            TransitionKind::Collapse => write!(f, "Collapse"),
            TransitionKind::Refinement => write!(f, "Refinement"),
            TransitionKind::Quotient => write!(f, "Quotient"),
            TransitionKind::Transport => write!(f, "Transport"),
            TransitionKind::PhaseTransition => write!(f, "PhaseTransition"),
            TransitionKind::CoarseGrain => write!(f, "CoarseGrain"),
        }
    }
}

pub fn parse_transition_kind(s: &str) -> Result<TransitionKind> {
    match s {
        "ConservativeExtension" => Ok(TransitionKind::ConservativeExtension),
        "Split" => Ok(TransitionKind::Split),
        "Merge" => Ok(TransitionKind::Merge),
        "Tunnel" => Ok(TransitionKind::Tunnel),
        "Collapse" => Ok(TransitionKind::Collapse),
        "Refinement" => Ok(TransitionKind::Refinement),
        "Quotient" => Ok(TransitionKind::Quotient),
        "Transport" => Ok(TransitionKind::Transport),
        "PhaseTransition" => Ok(TransitionKind::PhaseTransition),
        "CoarseGrain" => Ok(TransitionKind::CoarseGrain),
        _ => Err(MetacosmError::ParseError {
            block: "Transition".into(),
            detail: format!("unknown transition kind: '{}'", s),
        }),
    }
}

/// A named invariant that may or may not survive a transition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Invariant {
    /// All provable statements remain provable
    Soundness,
    /// All terms have normal forms
    Normalization,
    /// Transport between worlds is possible
    Transportability,
    /// Higher path structure is preserved
    PathStructure,
    /// Resource sensitivity is preserved
    ResourceSensitivity,
    /// Custom invariant
    Custom(String),
}

impl std::fmt::Display for Invariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Invariant::Soundness => write!(f, "Soundness"),
            Invariant::Normalization => write!(f, "Normalization"),
            Invariant::Transportability => write!(f, "Transportability"),
            Invariant::PathStructure => write!(f, "PathStructure"),
            Invariant::ResourceSensitivity => write!(f, "ResourceSensitivity"),
            Invariant::Custom(s) => write!(f, "{}", s),
        }
    }
}

pub fn parse_invariant(s: &str) -> Invariant {
    match s {
        "Soundness" => Invariant::Soundness,
        "Normalization" => Invariant::Normalization,
        "Transportability" => Invariant::Transportability,
        "PathStructure" => Invariant::PathStructure,
        "ResourceSensitivity" => Invariant::ResourceSensitivity,
        other => Invariant::Custom(other.to_string()),
    }
}

/// A declared transition between two worlds.
#[derive(Debug, Clone)]
pub struct TransitionDef {
    pub name: String,
    pub kind: TransitionKind,
    pub source: String,
    pub target: String,
    /// Invariants that this transition claims to preserve
    pub preserves: Vec<Invariant>,
    /// Invariants that this transition may violate
    pub breaks: Vec<Invariant>,
}

/// Parse `[Transition Name :kind K :from S :to T :preserves [...] :breaks [...]]`
pub fn parse_transition(items: &[Sexp]) -> Result<TransitionDef> {
    if items.len() < 2 {
        return Err(MetacosmError::ParseError {
            block: "Transition".into(),
            detail: "missing transition name".into(),
        });
    }

    let name = items[1]
        .as_atom()
        .ok_or_else(|| MetacosmError::ParseError {
            block: "Transition".into(),
            detail: "transition name must be an atom".into(),
        })?
        .to_string();

    let mut kind: Option<TransitionKind> = None;
    let mut source: Option<String> = None;
    let mut target: Option<String> = None;
    let mut preserves = Vec::new();
    let mut breaks = Vec::new();

    let mut i = 2;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":kind" => {
                i += 1;
                if let Some(k) = items.get(i).and_then(|s| s.as_atom()) {
                    kind = Some(parse_transition_kind(k)?);
                }
            }
            ":from" => {
                i += 1;
                source = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
            }
            ":to" => {
                i += 1;
                target = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
            }
            ":preserves" => {
                i += 1;
                if let Some(list) = items.get(i).and_then(|s| s.as_list()) {
                    for item in list {
                        if let Some(inv) = item.as_atom() {
                            preserves.push(parse_invariant(inv));
                        }
                    }
                }
            }
            ":breaks" => {
                i += 1;
                if let Some(list) = items.get(i).and_then(|s| s.as_list()) {
                    for item in list {
                        if let Some(inv) = item.as_atom() {
                            breaks.push(parse_invariant(inv));
                        }
                    }
                }
            }
            _ => {
                return Err(MetacosmError::ParseError {
                    block: "Transition".into(),
                    detail: format!("unknown keyword: {}", key),
                });
            }
        }
        i += 1;
    }

    let kind = kind.ok_or_else(|| MetacosmError::ParseError {
        block: "Transition".into(),
        detail: format!("Transition '{}' is missing :kind", name),
    })?;
    let source = source.ok_or_else(|| MetacosmError::ParseError {
        block: "Transition".into(),
        detail: format!("Transition '{}' is missing :from", name),
    })?;
    let target = target.ok_or_else(|| MetacosmError::ParseError {
        block: "Transition".into(),
        detail: format!("Transition '{}' is missing :to", name),
    })?;

    Ok(TransitionDef {
        name,
        kind,
        source,
        target,
        preserves,
        breaks,
    })
}

/// Check that a transition is valid given the epistemic profiles of its worlds.
pub fn check_transition_epistemic(
    transition: &TransitionDef,
    source_ep: &EpistemicProfile,
    target_ep: &EpistemicProfile,
) -> Result<Vec<String>> {
    let mut warnings = Vec::new();

    match transition.kind {
        TransitionKind::Tunnel => {
            // Tunnel: target must be able to verify but need not discover
            if !target_ep.can_verify() {
                return Err(MetacosmError::InvalidTransition {
                    from: transition.source.clone(),
                    to: transition.target.clone(),
                    detail: "tunnel target cannot verify (verification = none)".into(),
                });
            }
            if !source_ep.can_transport() {
                return Err(MetacosmError::InvalidTransition {
                    from: transition.source.clone(),
                    to: transition.target.clone(),
                    detail: "tunnel source cannot transport (transportability = none)".into(),
                });
            }
        }
        TransitionKind::ConservativeExtension => {
            // Target must dominate source epistemically (no capabilities lost)
            if !target_ep.dominates(source_ep) {
                warnings.push(format!(
                    "conservative extension {} → {} loses epistemic capability",
                    transition.source, transition.target
                ));
            }
        }
        TransitionKind::Collapse => {
            // Collapse typically loses path structure / discovery
            if target_ep.discovery > source_ep.discovery {
                warnings.push(format!(
                    "collapse {} → {} gains discovery power (unusual)",
                    transition.source, transition.target
                ));
            }
        }
        TransitionKind::CoarseGrain => {
            // Coarse-graining should improve verification/compression at cost of richness
            if target_ep.verification < source_ep.verification {
                warnings.push(format!(
                    "coarse-grain {} → {} loses verification power",
                    transition.source, transition.target
                ));
            }
        }
        _ => {}
    }

    // Check for invariant conflicts
    for inv in &transition.preserves {
        if transition.breaks.contains(inv) {
            return Err(MetacosmError::InvariantViolation {
                transition: transition.name.clone(),
                invariant: inv.to_string(),
                detail: "invariant appears in both :preserves and :breaks".into(),
            });
        }
    }

    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use apeiron::parser::parse;
    use crate::epistemic::Capacity;

    #[test]
    fn parse_transition_basic() {
        let input = r#"[Transition Discover
            :kind Tunnel
            :from Explorer
            :to Certifier
            :preserves [Soundness]
            :breaks [PathStructure]
        ]"#;
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let t = parse_transition(items).unwrap();
        assert_eq!(t.name, "Discover");
        assert_eq!(t.kind, TransitionKind::Tunnel);
        assert_eq!(t.source, "Explorer");
        assert_eq!(t.target, "Certifier");
        assert_eq!(t.preserves, vec![Invariant::Soundness]);
        assert_eq!(t.breaks, vec![Invariant::PathStructure]);
    }

    #[test]
    fn tunnel_requires_verification() {
        let t = TransitionDef {
            name: "T".into(),
            kind: TransitionKind::Tunnel,
            source: "A".into(),
            target: "B".into(),
            preserves: vec![],
            breaks: vec![],
        };
        let src = EpistemicProfile { transportability: Capacity::High, ..Default::default() };
        let tgt = EpistemicProfile { verification: Capacity::None, ..Default::default() };
        assert!(check_transition_epistemic(&t, &src, &tgt).is_err());
    }

    #[test]
    fn invariant_conflict_rejected() {
        let t = TransitionDef {
            name: "T".into(),
            kind: TransitionKind::Split,
            source: "A".into(),
            target: "B".into(),
            preserves: vec![Invariant::Soundness],
            breaks: vec![Invariant::Soundness],
        };
        let ep = EpistemicProfile::default();
        assert!(check_transition_epistemic(&t, &ep, &ep).is_err());
    }
}
