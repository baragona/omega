//! World morphisms: transitions promoted to functorial maps between worlds.
//!
//! A WorldMorphism wraps a TransitionDef with an optional functor reference
//! that connects the source world's categorical structure to the target's.
//! When a functor is present, additional invariant checks become possible:
//! - Functor source/target must match world categories
//! - Faithful functors imply witness-preserving transport
//! - Structure preservation is derived from the functor's object/morphism maps
//! - Composition chains functor maps with real functorial semantics

use std::collections::HashMap;

use crate::error::{MetacosmError, Result};
use crate::transition::{TransitionDef, TransportMode, compose_transitions};

// ── Functor reference ───────────────────────────────────────────────

/// How a morphism references its categorical functor.
#[derive(Debug, Clone)]
pub enum FunctorRef {
    /// Reference to a named Hyperion FunctorDef.
    Named(String),
    /// Identity functor (same-category transitions).
    Identity,
    /// Composed from sub-morphism functors (left-to-right).
    Composite(Vec<String>),
}

// ── Morphism properties ─────────────────────────────────────────────

/// Properties derived from the functor + epistemic analysis.
#[derive(Debug, Clone)]
pub struct MorphismProperties {
    /// Injective on morphism sets (no collapsing of distinct proofs).
    pub faithful: bool,
    /// Surjective on morphism sets (every target proof has a preimage).
    pub full: bool,
    /// Hits all target objects up to isomorphism.
    pub essentially_surjective: bool,
    /// Which categorical structures survive the map.
    pub preserves_structure: Vec<String>,
}

impl Default for MorphismProperties {
    fn default() -> Self {
        MorphismProperties {
            faithful: false,
            full: false,
            essentially_surjective: false,
            preserves_structure: Vec::new(),
        }
    }
}

impl MorphismProperties {
    /// Identity morphism properties.
    pub fn identity() -> Self {
        MorphismProperties {
            faithful: true,
            full: true,
            essentially_surjective: true,
            preserves_structure: Vec::new(), // filled in from category
        }
    }

    /// Compose properties: faithful/full are conjunctive, structure intersects.
    pub fn compose(&self, other: &MorphismProperties) -> MorphismProperties {
        let structure: Vec<String> = self.preserves_structure.iter()
            .filter(|s| other.preserves_structure.contains(s))
            .cloned()
            .collect();

        MorphismProperties {
            faithful: self.faithful && other.faithful,
            full: self.full && other.full,
            essentially_surjective: self.essentially_surjective && other.essentially_surjective,
            preserves_structure: structure,
        }
    }
}

impl std::fmt::Display for MorphismProperties {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut tags = Vec::new();
        if self.faithful { tags.push("faithful"); }
        if self.full { tags.push("full"); }
        if self.essentially_surjective { tags.push("ess-surj"); }
        if self.faithful && self.full && self.essentially_surjective {
            tags.clear();
            tags.push("equivalence");
        }
        if !self.preserves_structure.is_empty() {
            tags.push("structure-preserving");
        }
        if tags.is_empty() {
            write!(f, "opaque")
        } else {
            write!(f, "{}", tags.join("+"))
        }
    }
}

// ── World morphism ──────────────────────────────────────────────────

/// A world morphism: a transition with functorial structure.
#[derive(Debug, Clone)]
pub struct WorldMorphism {
    /// The underlying transition data.
    pub transition: TransitionDef,
    /// Optional functor connecting source/target categories.
    pub functor: Option<FunctorRef>,
    /// Derived morphism properties.
    pub properties: MorphismProperties,
}

impl WorldMorphism {
    /// Create an opaque morphism from a plain transition (backward compat).
    pub fn opaque(transition: TransitionDef) -> Self {
        WorldMorphism {
            transition,
            functor: None,
            properties: MorphismProperties::default(),
        }
    }

    /// Create a morphism with a named functor reference.
    pub fn with_functor(transition: TransitionDef, functor_name: String) -> Self {
        WorldMorphism {
            transition,
            functor: Some(FunctorRef::Named(functor_name)),
            properties: MorphismProperties::default(),
        }
    }

    /// Create an identity morphism for a world.
    pub fn identity(world_name: &str, category_structures: Vec<String>) -> Self {
        WorldMorphism {
            transition: TransitionDef {
                name: format!("id_{}", world_name),
                kind: crate::transition::TransitionKind::Transport,
                source: world_name.to_string(),
                target: world_name.to_string(),
                preserves: Vec::new(),
                breaks: Vec::new(),
                transport: crate::transition::TransportEpistemics {
                    mode: TransportMode::Conservative,
                    loss: Vec::new(),
                },
                functor: None,
            },
            functor: Some(FunctorRef::Identity),
            properties: MorphismProperties {
                preserves_structure: category_structures,
                ..MorphismProperties::identity()
            },
        }
    }
}

/// Compose two world morphisms.
///
/// The transition layer uses existing set-algebra (preserves∩, breaks∪, loss∪).
/// The functor layer chains: Named(f) ; Named(g) = Composite([f, g]).
/// Properties compose conjunctively.
pub fn compose_morphisms(
    f: &WorldMorphism,
    g: &WorldMorphism,
    name: &str,
) -> Result<WorldMorphism> {
    let transition = compose_transitions(&f.transition, &g.transition, name)?;

    let functor = match (&f.functor, &g.functor) {
        (Some(FunctorRef::Identity), other) => other.clone(),
        (other, Some(FunctorRef::Identity)) => other.clone(),
        (Some(FunctorRef::Named(a)), Some(FunctorRef::Named(b))) => {
            Some(FunctorRef::Composite(vec![a.clone(), b.clone()]))
        }
        (Some(FunctorRef::Named(a)), Some(FunctorRef::Composite(bs))) => {
            let mut names = vec![a.clone()];
            names.extend(bs.iter().cloned());
            Some(FunctorRef::Composite(names))
        }
        (Some(FunctorRef::Composite(as_)), Some(FunctorRef::Named(b))) => {
            let mut names = as_.clone();
            names.push(b.clone());
            Some(FunctorRef::Composite(names))
        }
        (Some(FunctorRef::Composite(as_)), Some(FunctorRef::Composite(bs))) => {
            let mut names = as_.clone();
            names.extend(bs.iter().cloned());
            Some(FunctorRef::Composite(names))
        }
        // Opaque absorbs: if either side has no functor, result is opaque
        _ => None,
    };

    let properties = f.properties.compose(&g.properties);

    Ok(WorldMorphism {
        transition,
        functor,
        properties,
    })
}

// ── Validation ──────────────────────────────────────────────────────

/// Validate functor-transition consistency.
///
/// Checks:
/// 1. Functor source category matches source world's category
/// 2. Functor target category matches target world's category
/// 3. Faithful functor is consistent with witness-preserving transport
/// 4. Structure preservation derived from object/morphism maps
pub fn validate_morphism(
    morphism: &mut WorldMorphism,
    worlds: &HashMap<String, crate::world::WorldDef>,
    hyperion: &hyperion::session::HyperionSession,
) -> Result<Vec<String>> {
    let mut warnings = Vec::new();

    let functor_name = match &morphism.functor {
        Some(FunctorRef::Named(name)) => name.clone(),
        Some(FunctorRef::Identity) => {
            // Identity: derive structure preservation from category
            if let Some(src_world) = worlds.get(&morphism.transition.source) {
                let structures = category_structure_names(hyperion, &src_world.category);
                morphism.properties = MorphismProperties {
                    preserves_structure: structures,
                    ..MorphismProperties::identity()
                };
            }
            return Ok(warnings);
        }
        _ => return Ok(warnings), // Opaque or composite — no validation
    };

    // Look up functor in Hyperion
    let functor = hyperion.functors.get(&functor_name).ok_or_else(|| {
        MetacosmError::Undefined {
            kind: "Functor".into(),
            name: functor_name.clone(),
        }
    })?;

    // Check 1 & 2: Category consistency
    let src_world = worlds.get(&morphism.transition.source).ok_or_else(|| {
        MetacosmError::Undefined {
            kind: "World".into(),
            name: morphism.transition.source.clone(),
        }
    })?;
    let tgt_world = worlds.get(&morphism.transition.target).ok_or_else(|| {
        MetacosmError::Undefined {
            kind: "World".into(),
            name: morphism.transition.target.clone(),
        }
    })?;

    // Hyperion functors map between substrates, so validate against world substrates
    if functor.source != src_world.substrate {
        return Err(MetacosmError::InvalidTransition {
            from: morphism.transition.source.clone(),
            to: morphism.transition.target.clone(),
            detail: format!(
                "functor {} source '{}' doesn't match world '{}' substrate '{}'",
                functor_name, functor.source, morphism.transition.source, src_world.substrate,
            ),
        });
    }
    if functor.target != tgt_world.substrate {
        return Err(MetacosmError::InvalidTransition {
            from: morphism.transition.source.clone(),
            to: morphism.transition.target.clone(),
            detail: format!(
                "functor {} target '{}' doesn't match world '{}' substrate '{}'",
                functor_name, functor.target, morphism.transition.target, tgt_world.substrate,
            ),
        });
    }

    // Derive properties from functor maps
    let src_cat = hyperion.categories.get(&src_world.category);
    let tgt_cat = hyperion.categories.get(&tgt_world.category);

    if let (Some(src_cat), Some(tgt_cat)) = (src_cat, tgt_cat) {
        // Faithful: object map is injective (no two source objects map to same target)
        let mapped_targets: Vec<&str> = functor.object_map.iter().map(|(_, t)| t.as_str()).collect();
        let unique_targets: std::collections::HashSet<&str> = mapped_targets.iter().copied().collect();
        let faithful = mapped_targets.len() == unique_targets.len() && !mapped_targets.is_empty();

        // Full: all target objects are in the image (for matching-category case)
        let full = if src_cat.name == tgt_cat.name {
            // Same category: identity-like, check all objects mapped
            src_cat.objects.iter().all(|obj| {
                functor.object_map.iter().any(|(s, _)| s == &obj.name)
            })
        } else {
            // Different categories: check all target objects have a preimage
            tgt_cat.objects.iter().all(|obj| {
                functor.object_map.iter().any(|(_, t)| t == &obj.name)
            })
        };

        // Essentially surjective: all target objects hit
        let ess_surj = tgt_cat.objects.iter().all(|obj| {
            functor.object_map.iter().any(|(_, t)| t == &obj.name)
        });

        // Structure preservation: check which structures survive
        let mut preserved = Vec::new();
        for structure in &src_cat.structure {
            let name = structure_name(structure);
            // A structure is preserved if its constituent objects/morphisms
            // are all in the functor's maps
            let obj_names = structure_objects(structure);
            let morph_names = structure_morphisms(structure);

            let objs_mapped = obj_names.iter().all(|o| {
                functor.object_map.iter().any(|(s, _)| s == o)
            });
            let morphs_mapped = morph_names.iter().all(|m| {
                functor.morphism_map.iter().any(|(s, _)| s == m)
            });

            if objs_mapped && morphs_mapped {
                preserved.push(name);
            }
        }

        morphism.properties = MorphismProperties {
            faithful,
            full,
            essentially_surjective: ess_surj,
            preserves_structure: preserved,
        };

        // Check 3: Epistemic-functorial coherence
        if faithful && morphism.transition.transport.mode == TransportMode::Lossy {
            warnings.push(format!(
                "functor {} is faithful but transport mode is lossy — consider witness transport",
                functor_name,
            ));
        }
        if !faithful && matches!(morphism.transition.transport.mode, TransportMode::Witness | TransportMode::Conservative) {
            warnings.push(format!(
                "functor {} is not faithful but transport claims witness-preservation — proofs may collapse",
                functor_name,
            ));
        }
    }

    Ok(warnings)
}

// ── Helpers ─────────────────────────────────────────────────────────

fn category_structure_names(
    hyperion: &hyperion::session::HyperionSession,
    category_name: &str,
) -> Vec<String> {
    hyperion.categories.get(category_name)
        .map(|cat| cat.structure.iter().map(|s| structure_name(s)).collect())
        .unwrap_or_default()
}

fn structure_name(s: &hyperion::category::CategoricalStructure) -> String {
    use hyperion::category::CategoricalStructure::*;
    match s {
        Exponential { name, .. } => format!("Exponential({})", name),
        Evaluator { name } => format!("Evaluator({})", name),
        ModalOperator { name } => format!("Modal({})", name),
        ContextDecl { name } => format!("Context({})", name),
        TensorProduct { name } => format!("Tensor({})", name),
        Unit { name } => format!("Unit({})", name),
        Preorder { relation } => format!("Preorder({})", relation),
        PathType { .. } => "PathType".to_string(),
        JType { .. } => "JType".to_string(),
        PartialElement { .. } => "PartialElement".to_string(),
        IntervalSort { .. } => "IntervalSort".to_string(),
    }
}

fn structure_objects(s: &hyperion::category::CategoricalStructure) -> Vec<String> {
    use hyperion::category::CategoricalStructure::*;
    match s {
        Exponential { object, .. } => vec![object.clone()],
        Unit { name } => vec![name.clone()],
        _ => vec![],
    }
}

fn structure_morphisms(s: &hyperion::category::CategoricalStructure) -> Vec<String> {
    use hyperion::category::CategoricalStructure::*;
    match s {
        Evaluator { name } => vec![name.clone()],
        ModalOperator { name } => vec![name.clone()],
        TensorProduct { name } => vec![name.clone()],
        Preorder { relation } => vec![relation.clone()],
        PathType { refl, concat, inv, ap } => vec![
            refl.clone(), concat.clone(), inv.clone(), ap.clone(),
        ],
        JType { j_elim, transport } => vec![j_elim.clone(), transport.clone()],
        PartialElement { hcomp, coe } => vec![hcomp.clone(), coe.clone()],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transition::*;

    #[test]
    fn opaque_morphism_from_transition() {
        let t = TransitionDef {
            name: "T".into(),
            kind: TransitionKind::Tunnel,
            source: "A".into(),
            target: "B".into(),
            preserves: vec![Invariant::Soundness],
            breaks: vec![],
            transport: TransportEpistemics::default(),
            functor: None,
        };
        let m = WorldMorphism::opaque(t);
        assert!(m.functor.is_none());
        assert!(!m.properties.faithful);
    }

    #[test]
    fn identity_morphism() {
        let m = WorldMorphism::identity("Explorer", vec!["Exponential(lam)".into()]);
        assert!(m.properties.faithful);
        assert!(m.properties.full);
        assert!(m.properties.essentially_surjective);
        assert_eq!(m.transition.source, "Explorer");
        assert_eq!(m.transition.target, "Explorer");
        assert_eq!(m.transition.transport.mode, TransportMode::Conservative);
    }

    #[test]
    fn compose_morphisms_identity_left() {
        let id = WorldMorphism::identity("A", vec!["Exp".into()]);
        let f = WorldMorphism::with_functor(
            TransitionDef {
                name: "F".into(),
                kind: TransitionKind::Tunnel,
                source: "A".into(),
                target: "B".into(),
                preserves: vec![Invariant::Soundness],
                breaks: vec![],
                transport: TransportEpistemics::default(),
                functor: None,
            },
            "MyFunctor".into(),
        );
        let composed = compose_morphisms(&id, &f, "id_F").unwrap();
        // Identity absorbed: result has F's functor
        match &composed.functor {
            Some(FunctorRef::Named(n)) => assert_eq!(n, "MyFunctor"),
            other => panic!("expected Named, got {:?}", other),
        }
    }

    #[test]
    fn compose_morphisms_named() {
        let f = WorldMorphism::with_functor(
            TransitionDef {
                name: "F".into(),
                kind: TransitionKind::Tunnel,
                source: "A".into(),
                target: "B".into(),
                preserves: vec![Invariant::Soundness],
                breaks: vec![],
                transport: TransportEpistemics::default(),
                functor: None,
            },
            "F1".into(),
        );
        let g = WorldMorphism::with_functor(
            TransitionDef {
                name: "G".into(),
                kind: TransitionKind::CoarseGrain,
                source: "B".into(),
                target: "C".into(),
                preserves: vec![Invariant::Soundness],
                breaks: vec![Invariant::PathStructure],
                transport: TransportEpistemics {
                    mode: TransportMode::Lossy,
                    loss: vec![Invariant::PathStructure],
                },
                functor: None,
            },
            "F2".into(),
        );
        let composed = compose_morphisms(&f, &g, "FG").unwrap();
        match &composed.functor {
            Some(FunctorRef::Composite(names)) => {
                assert_eq!(names, &vec!["F1".to_string(), "F2".to_string()]);
            }
            other => panic!("expected Composite, got {:?}", other),
        }
        // preserves = intersection
        assert_eq!(composed.transition.preserves, vec![Invariant::Soundness]);
        // breaks = union
        assert_eq!(composed.transition.breaks, vec![Invariant::PathStructure]);
    }

    #[test]
    fn opaque_absorbs_in_composition() {
        let f = WorldMorphism::opaque(TransitionDef {
            name: "F".into(),
            kind: TransitionKind::Tunnel,
            source: "A".into(),
            target: "B".into(),
            preserves: vec![],
            breaks: vec![],
            transport: TransportEpistemics::default(),
            functor: None,
        });
        let g = WorldMorphism::with_functor(
            TransitionDef {
                name: "G".into(),
                kind: TransitionKind::Tunnel,
                source: "B".into(),
                target: "C".into(),
                preserves: vec![],
                breaks: vec![],
                transport: TransportEpistemics::default(),
                functor: None,
            },
            "F2".into(),
        );
        let composed = compose_morphisms(&f, &g, "FG").unwrap();
        assert!(composed.functor.is_none(), "opaque should absorb");
    }

    #[test]
    fn properties_compose_conjunctively() {
        let a = MorphismProperties {
            faithful: true,
            full: true,
            essentially_surjective: false,
            preserves_structure: vec!["Exp".into(), "Eval".into()],
        };
        let b = MorphismProperties {
            faithful: true,
            full: false,
            essentially_surjective: true,
            preserves_structure: vec!["Exp".into(), "Modal".into()],
        };
        let c = a.compose(&b);
        assert!(c.faithful);
        assert!(!c.full);
        assert!(!c.essentially_surjective);
        assert_eq!(c.preserves_structure, vec!["Exp".to_string()]);
    }
}
