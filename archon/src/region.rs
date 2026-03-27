//! Regions and membranes — the topology of computation.
//!
//! A Region is a volume of the interaction net with local physics:
//! its own resource mode (how duplication/erasure works), equality mode
//! (how terms are compared), and boundary type (what happens at the edge).
//!
//! Regions nest: a linear region inside a sharing region means the outer
//! liquid can't leak into the inner solid.

use std::collections::HashMap;

// ── Resource modes ("states of matter") ──────────────────────────────

/// How duplication and erasure behave inside this region.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResourceMode {
    /// Solid: wires cannot branch. Dup creation is rejected.
    StrictlyLinear,
    /// Liquid: identical subgraphs coalesce via hash-consing.
    OptimalSharing,
    /// Jelly: each resource used at most once, but erasure is allowed.
    Affine,
    /// Elastic: each resource used at least once, but duplication is allowed.
    Relevant,
    /// Gas: whenever a graph is referenced, it spontaneously replicates.
    DeepCopy,
    /// Plasma: replicas propagate as wavefronts, merge on collision via AC rules.
    EventuallyConsistent,
}

// ── Equality modes ("topological rigidity") ──────────────────────────

/// How equality/matching works inside this region.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EqualityMode {
    /// Rigid: graphs match only if structurally identical (pointer/hash).
    TopologicalHash,
    /// Elastic wires, fixed nodes: equal up to variable renaming.
    AlphaEquivalence,
    /// Directed rewriting + beta-reduction.
    RewriteEquivalence,
    /// Observational: equal if observable behavior matches.
    Observational,
    /// Quantum superposition: e-graph equality saturation.
    EqualitySaturation,
    /// Extensional: functions equal iff agree on all inputs.
    ExtensionalEquivalence,
    /// Miller pattern unification.
    Unification,
    /// HoTT: equality as path spaces.
    HomotopyEquivalence,
    /// Path-labeled e-graph edges (proof-relevant).
    ProofRelevant,
    /// Associativity-commutativity matching (flatten + sort).
    ACMatching,
    /// Backward-chaining resolution.
    UnificationSearch,
    /// CDCL as damped oscillation toward ground state.
    Thermodynamic,
}

// ── Boundary types ("membrane material") ─────────────────────────────

/// What kind of boundary separates this region from its parent/neighbors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoundaryType {
    /// Empty space: no boundary effects.
    Transparent,
    /// Compartment-style membranes (K Framework cells).
    ContextualMembrane,
    /// Asymmetric one-way information flow.
    OneWayValve,
    /// Phase-based temporal barriers.
    TemporalPhase,
    /// Opaque: data can only pass through with a key node.
    Cryptographic,
    /// Nominal name-abstraction scoping.
    NominalScoping,
    /// Restricted wormhole: graphs are extruded into token streams.
    NetworkPartition,
    /// Tight pores: complex closures shatter into S/K/I combinators.
    CombinatorFilter,
    /// Crystallization front: continuations propagate as catalysts.
    EffectBoundary,
    /// Anti-matter: quantifier polarity flips on crossing.
    DialecticaBoundary,
    /// Linear-to-unrestricted: wraps nodes in ! (bang) bubbles.
    BangBoundary,
    /// Higher-order to first-order: closures crystallize into ADTs.
    DefunctionalizationBoundary,
    /// Binders become explicit substitution closures.
    ExplicitSubstitutionBoundary,
    /// AC terms are flattened and sorted at crossing.
    ACBoundary,
    /// Tensor products serialized left-to-right at crossing.
    TensorSerializationBoundary,
    /// Kripke world boundary: modal operators thread world parameters.
    KripkeBoundary,
    /// RPC: data serialized to wire format at crossing.
    RpcSerializationBoundary,
    /// Nominal: name-abstraction scoping at boundary.
    NominalBoundary,
    /// Grounding: higher-order terms compiled to first-order at crossing.
    GroundingBoundary,
    /// Context reification: first-class contexts become data structures.
    ContextReifyBoundary,
    /// Modal restriction: variable-class guards at boundary.
    ModalRestrictionBoundary,
    /// Kan transport: reduction rules for path transport at boundary.
    KanTransportBoundary,
    /// Thermodynamic: terms crossing become spin/spring constraints.
    ThermoBoundary,
}

// ── Direction ("arrow of time") ──────────────────────────────────────

/// Which direction interactions run in this region.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Normal: reduce redexes (beta, rewrite, etc.)
    Forward,
    /// Inverted: expand goals into premises (backward chaining).
    Backward,
}

// ── Propagation speed ────────────────────────────────────────────────

/// How fast interactions propagate within this region.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Propagation {
    /// Standard: interactions fire instantly.
    Instant,
    /// Relativistic: changes propagate as wavefronts with finite speed.
    Delayed { speed: u32 },
}

// ── Region ───────────────────────────────────────────────────────────

/// A region of the interaction net with local physics.
#[derive(Clone, Debug)]
pub struct Region {
    pub id: u32,
    /// Parent region (None = root/global).
    pub parent: Option<u32>,
    /// Child regions nested inside this one.
    pub children: Vec<u32>,
    /// How duplication/erasure work here.
    pub resource_mode: ResourceMode,
    /// How equality/matching works here.
    pub equality_mode: EqualityMode,
    /// What happens at this region's boundary.
    pub boundary_type: BoundaryType,
    /// Which direction interactions run.
    pub direction: Direction,
    /// How fast interactions propagate.
    pub propagation: Propagation,
    /// Human-readable label.
    pub label: String,
}

impl Region {
    pub fn new(id: u32, label: impl Into<String>) -> Self {
        Region {
            id,
            parent: None,
            children: Vec::new(),
            resource_mode: ResourceMode::OptimalSharing,
            equality_mode: EqualityMode::RewriteEquivalence,
            boundary_type: BoundaryType::Transparent,
            direction: Direction::Forward,
            propagation: Propagation::Instant,
            label: label.into(),
        }
    }

    pub fn with_resource(mut self, mode: ResourceMode) -> Self {
        self.resource_mode = mode;
        self
    }

    pub fn with_equality(mut self, mode: EqualityMode) -> Self {
        self.equality_mode = mode;
        self
    }

    pub fn with_boundary(mut self, boundary: BoundaryType) -> Self {
        self.boundary_type = boundary;
        self
    }

    pub fn with_direction(mut self, dir: Direction) -> Self {
        self.direction = dir;
        self
    }

    pub fn with_propagation(mut self, prop: Propagation) -> Self {
        self.propagation = prop;
        self
    }

    pub fn with_parent(mut self, parent: u32) -> Self {
        self.parent = Some(parent);
        self
    }
}

// ── Topology ─────────────────────────────────────────────────────────

/// The topology of regions — a directed graph of regions with boundary types.
/// This is the "sheaf skeleton" that Hyperion (the Architect) computes.
#[derive(Clone, Debug)]
pub struct Topology {
    /// All regions, keyed by ID.
    pub regions: HashMap<u32, Region>,
    /// Wormholes connecting regions across the Kripke Z-axis.
    /// (from_region, to_region) — directed accessibility relation.
    pub wormholes: Vec<(u32, u32)>,
    /// Next region ID for auto-allocation.
    next_id: u32,
}

impl Topology {
    pub fn new() -> Self {
        // Create a default root region (region 0).
        let mut regions = HashMap::new();
        let root = Region::new(0, "root");
        regions.insert(0, root);

        Topology {
            regions,
            wormholes: Vec::new(),
            next_id: 1,
        }
    }

    /// Add a new region, returning its ID.
    pub fn add_region(&mut self, mut region: Region) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        region.id = id;

        // Register as child of parent.
        if let Some(parent_id) = region.parent {
            if let Some(parent) = self.regions.get_mut(&parent_id) {
                parent.children.push(id);
            }
        }

        self.regions.insert(id, region);
        id
    }

    /// Add a Kripke wormhole (directed accessibility between regions).
    pub fn add_wormhole(&mut self, from: u32, to: u32) {
        self.wormholes.push((from, to));
    }

    /// Get accessible worlds from a given region (outgoing wormholes).
    pub fn accessible_from(&self, region_id: u32) -> Vec<u32> {
        self.wormholes
            .iter()
            .filter(|(from, _)| *from == region_id)
            .map(|(_, to)| *to)
            .collect()
    }

    /// Get the next auto-allocated ID (without consuming it).
    pub fn next_id(&self) -> u32 {
        self.next_id
    }

    /// Get all region IDs.
    pub fn region_ids(&self) -> Vec<u32> {
        self.regions.keys().copied().collect()
    }

    /// Get the region by ID.
    pub fn get(&self, id: u32) -> Option<&Region> {
        self.regions.get(&id)
    }

    /// Get a mutable reference to a region by ID.
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Region> {
        self.regions.get_mut(&id)
    }

    /// Get the boundary type between two regions.
    /// Uses the child's boundary type (the boundary "belongs" to the inner region).
    pub fn boundary_between(&self, from: u32, to: u32) -> Option<&BoundaryType> {
        // If to is a child of from, use to's boundary.
        // If from is a child of to, use from's boundary.
        // If they're siblings or unrelated, check for wormhole.
        if let Some(region_to) = self.regions.get(&to) {
            if region_to.parent == Some(from) {
                return Some(&region_to.boundary_type);
            }
        }
        if let Some(region_from) = self.regions.get(&from) {
            if region_from.parent == Some(to) {
                return Some(&region_from.boundary_type);
            }
        }
        // For wormhole connections, use target's boundary.
        if self.wormholes.contains(&(from, to)) {
            return self.regions.get(&to).map(|r| &r.boundary_type);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topology_basics() {
        let mut topo = Topology::new();
        assert!(topo.get(0).is_some()); // root exists

        let linear = Region::new(0, "linear-zone")
            .with_resource(ResourceMode::StrictlyLinear)
            .with_boundary(BoundaryType::BangBoundary)
            .with_parent(0);
        let id = topo.add_region(linear);

        assert_eq!(id, 1);
        assert_eq!(topo.get(id).unwrap().resource_mode, ResourceMode::StrictlyLinear);
        assert_eq!(topo.get(0).unwrap().children, vec![1]);
    }

    #[test]
    fn wormholes() {
        let mut topo = Topology::new();
        let w1 = topo.add_region(Region::new(0, "world-1").with_parent(0));
        let w2 = topo.add_region(Region::new(0, "world-2").with_parent(0));
        topo.add_wormhole(w1, w2);
        topo.add_wormhole(w2, w1); // symmetric accessibility (S5)

        assert_eq!(topo.accessible_from(w1), vec![w2]);
        assert_eq!(topo.accessible_from(w2), vec![w1]);
    }

    #[test]
    fn boundary_lookup() {
        let mut topo = Topology::new();
        let child = Region::new(0, "combinator-zone")
            .with_boundary(BoundaryType::CombinatorFilter)
            .with_parent(0);
        let child_id = topo.add_region(child);

        let boundary = topo.boundary_between(0, child_id);
        assert_eq!(boundary, Some(&BoundaryType::CombinatorFilter));
    }

    #[test]
    fn nesting() {
        let mut topo = Topology::new();
        let outer = topo.add_region(
            Region::new(0, "sharing")
                .with_resource(ResourceMode::OptimalSharing)
                .with_parent(0),
        );
        let inner = topo.add_region(
            Region::new(0, "linear-inside-sharing")
                .with_resource(ResourceMode::StrictlyLinear)
                .with_boundary(BoundaryType::BangBoundary)
                .with_parent(outer),
        );

        assert_eq!(topo.get(outer).unwrap().children, vec![inner]);
        assert_eq!(topo.get(inner).unwrap().parent, Some(outer));
    }
}
