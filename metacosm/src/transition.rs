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
    Soundness,
    Normalization,
    Transportability,
    PathStructure,
    ResourceSensitivity,
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

/// Transport mode: how a transition moves theorems across worlds.
/// This is a relational property — it belongs on transitions, not worlds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportMode {
    /// Transport the full witness/proof
    Witness,
    /// Transport only the theorem statement
    TheoremOnly,
    /// Conservative: everything transfers
    Conservative,
    /// Lossy: some information is lost
    Lossy,
}

impl Default for TransportMode {
    fn default() -> Self {
        TransportMode::Witness
    }
}

impl std::fmt::Display for TransportMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportMode::Witness => write!(f, "witness"),
            TransportMode::TheoremOnly => write!(f, "theorem-only"),
            TransportMode::Conservative => write!(f, "conservative"),
            TransportMode::Lossy => write!(f, "lossy"),
        }
    }
}

fn parse_transport_mode(s: &str) -> Result<TransportMode> {
    match s {
        "witness" => Ok(TransportMode::Witness),
        "theorem-only" => Ok(TransportMode::TheoremOnly),
        "conservative" => Ok(TransportMode::Conservative),
        "lossy" => Ok(TransportMode::Lossy),
        _ => Err(MetacosmError::ParseError {
            block: "Transition".into(),
            detail: format!("unknown transport mode: '{}'", s),
        }),
    }
}

/// Epistemic data attached to a transition (relational properties).
#[derive(Debug, Clone)]
pub struct TransportEpistemics {
    pub mode: TransportMode,
    pub loss: Vec<Invariant>,
}

impl Default for TransportEpistemics {
    fn default() -> Self {
        TransportEpistemics {
            mode: TransportMode::Witness,
            loss: Vec::new(),
        }
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
    /// Relational epistemic data (transport mode, information loss)
    pub transport: TransportEpistemics,
}

/// Parse `[Transition Name :kind K :from S :to T :preserves [...] :breaks [...] :transport [...]]`
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
    let mut transport = TransportEpistemics::default();

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
            ":transport" => {
                i += 1;
                if let Some(list) = items.get(i).and_then(|s| s.as_list()) {
                    transport = parse_transport_epistemics(list)?;
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
        transport,
    })
}

fn parse_transport_epistemics(items: &[Sexp]) -> Result<TransportEpistemics> {
    let mut te = TransportEpistemics::default();
    let mut i = 0;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":mode" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    te.mode = parse_transport_mode(v)?;
                }
            }
            ":loss" => {
                i += 1;
                if let Some(list) = items.get(i).and_then(|s| s.as_list()) {
                    for item in list {
                        if let Some(inv) = item.as_atom() {
                            te.loss.push(parse_invariant(inv));
                        }
                    }
                }
            }
            _ => {
                return Err(MetacosmError::ParseError {
                    block: "Transition/transport".into(),
                    detail: format!("unknown keyword: {}", key),
                });
            }
        }
        i += 1;
    }
    Ok(te)
}

/// Check that a transition is valid given the epistemic profiles of its worlds.
///
/// Transportability is no longer a unary world property. Instead:
/// - Tunnel: target must have verification capability (can check presented proof)
/// - ConservativeExtension: target must dominate source
/// - Collapse: warns if target gains discovery (unusual)
/// - CoarseGrain: warns if target loses verification
pub fn check_transition_epistemic(
    transition: &TransitionDef,
    source_ep: &EpistemicProfile,
    target_ep: &EpistemicProfile,
) -> Result<Vec<String>> {
    let mut warnings = Vec::new();

    match transition.kind {
        TransitionKind::Tunnel => {
            // Tunnel target must be able to verify
            if !target_ep.can_verify() {
                return Err(MetacosmError::InvalidTransition {
                    from: transition.source.clone(),
                    to: transition.target.clone(),
                    detail: "tunnel target cannot verify (verification = none)".into(),
                });
            }
            // Tunnel source should have some discovery capability
            if !source_ep.can_discover() {
                warnings.push(format!(
                    "tunnel source {} has no discovery capability",
                    transition.source
                ));
            }
        }
        TransitionKind::ConservativeExtension => {
            if !target_ep.dominates(source_ep) {
                warnings.push(format!(
                    "conservative extension {} → {} loses epistemic capability",
                    transition.source, transition.target
                ));
            }
        }
        TransitionKind::Collapse => {
            if target_ep.discover > source_ep.discover {
                warnings.push(format!(
                    "collapse {} → {} gains discovery power (unusual)",
                    transition.source, transition.target
                ));
            }
        }
        TransitionKind::CoarseGrain => {
            if target_ep.verify < source_ep.verify {
                warnings.push(format!(
                    "coarse-grain {} → {} loses verification strength",
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
    use crate::epistemic::*;

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
    fn parse_transition_with_transport() {
        let input = r#"[Transition T
            :kind Tunnel
            :from A :to B
            :transport [:mode witness :loss [PathStructure]]
        ]"#;
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let t = parse_transition(items).unwrap();
        assert_eq!(t.transport.mode, TransportMode::Witness);
        assert_eq!(t.transport.loss, vec![Invariant::PathStructure]);
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
            transport: TransportEpistemics::default(),
        };
        let src = EpistemicProfile {
            discover: DiscoveryStrength::Complete,
            ..Default::default()
        };
        let tgt = EpistemicProfile {
            verify: VerificationStrength::None,
            ..Default::default()
        };
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
            transport: TransportEpistemics::default(),
        };
        let ep = EpistemicProfile::default();
        assert!(check_transition_epistemic(&t, &ep, &ep).is_err());
    }
}
