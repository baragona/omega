//! ArchonArena — wraps Apeiron's Arena with region membership,
//! radiation state, and thermodynamic metadata.
//!
//! Uses composition (not inheritance): Apeiron's Arena is untouched.
//! New node types use OpCode::Sym with reserved `__archon_` prefixes
//! and store metadata in side tables.

use std::collections::{HashMap, HashSet};

use apeiron::arena::Arena;
use apeiron::node::{OpCode, Port, Ptr};

use crate::region::{Topology, ResourceMode};

// ── Marker IDs for radiation / gauge fields ──────────────────────────

/// A unique marker identifying a topological radiation source (e.g., a variable).
pub type MarkerId = u32;

// ── ArchonArena ──────────────────────────────────────────────────────

pub struct ArchonArena {
    /// The underlying Apeiron arena (all graph operations delegate here).
    pub inner: Arena,
    /// The topology of regions (the sheaf skeleton).
    pub topology: Topology,
    /// Region membership: node index → region ID.
    /// Parallel to inner.nodes; grows with spawns.
    node_region: Vec<u32>,
    /// Radiation state: which markers are "glowing" on each node.
    /// node index → set of active marker IDs.
    radiation: HashMap<u32, HashSet<MarkerId>>,
    /// Radiation sources: marker ID → source node Ptr.
    radiation_sources: HashMap<MarkerId, Ptr>,
    /// Next marker ID for auto-allocation.
    next_marker: MarkerId,
    /// Per-region temperature for thermodynamic annealing.
    pub temperatures: HashMap<u32, f64>,
    /// Spin polarities: node index → bool (true = UP, false = DOWN).
    pub spins: HashMap<u32, bool>,
    /// Spring constraints: node index → list of (connected spin nodes, polarity requirement).
    pub springs: HashMap<u32, Vec<SpringConstraint>>,
    /// Continuous variables for arithmetic annealing.
    pub continuous_vars: HashMap<u32, crate::thermo::ContinuousVar>,
    /// Arithmetic constraints for Hamiltonian energy.
    pub arith_constraints: HashMap<u32, Vec<crate::thermo::ArithConstraint>>,
    /// E-class membership: superposition node → set of member nodes.
    eclass_members: HashMap<u32, HashSet<u32>>,
    /// Reverse e-class lookup: member node → superposition node.
    eclass_of: HashMap<u32, u32>,
    /// Union-find for e-class equivalence: node_id → parent_id.
    /// This is the canonical source of truth for "are two nodes equivalent?"
    /// The superposition nodes are physical representations in the net, but
    /// this union-find handles the transitive closure correctly.
    pub uf_parent: HashMap<u32, u32>,
    /// Physical hashcons: maps a node's structural fingerprint to the canonical node.
    /// When two nodes have the same signature, they are congruent and must be merged.
    pub spatial_index: HashMap<ENodeSignature, Ptr>,
    /// Shockwave queue: superposition hubs that just merged. Their parents need
    /// re-canonicalization (the rebuild/congruence cascade).
    pub shockwave_queue: Vec<Ptr>,
    /// Parent index: node → set of nodes that reference it as a child (via aux ports).
    /// Enables upward traversal for congruence propagation.
    pub parent_index: HashMap<u32, HashSet<u32>>,
}

/// A structural fingerprint of a node based on its opcode and child e-class roots.
/// Two nodes with the same signature are congruent and must be merged.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct ENodeSignature {
    /// The opcode of the node (serialized for hashing).
    pub opcode: String,
    /// The e-class root pointers for each child (aux port targets, resolved through superpositions).
    pub child_classes: Vec<u32>,
}

/// A constraint coupling between spin nodes.
#[derive(Clone, Debug)]
pub struct SpringConstraint {
    /// The spin nodes involved in this constraint.
    pub literals: Vec<(Ptr, bool)>, // (spin_node, required_polarity)
}

impl ArchonArena {
    pub fn new() -> Self {
        ArchonArena {
            inner: Arena::new(),
            topology: Topology::new(),
            node_region: Vec::new(),
            radiation: HashMap::new(),
            radiation_sources: HashMap::new(),
            next_marker: 0,
            temperatures: HashMap::new(),
            spins: HashMap::new(),
            springs: HashMap::new(),
            continuous_vars: HashMap::new(),
            arith_constraints: HashMap::new(),
            eclass_members: HashMap::new(),
            eclass_of: HashMap::new(),
            uf_parent: HashMap::new(),
            spatial_index: HashMap::new(),
            shockwave_queue: Vec::new(),
            parent_index: HashMap::new(),
        }
    }

    pub fn with_topology(mut self, topology: Topology) -> Self {
        self.topology = topology;
        self
    }

    // ── Spawning (region-aware) ──────────────────────────────────────

    /// Spawn a node in a specific region.
    pub fn spawn_in(&mut self, kind: OpCode, region_id: u32) -> Ptr {
        let ptr = self.inner.spawn(kind);
        // Ensure node_region is large enough.
        while self.node_region.len() <= ptr.index() {
            self.node_region.push(0); // default to root region
        }
        self.node_region[ptr.index()] = region_id;
        ptr
    }

    /// Spawn a node in the root region (region 0).
    pub fn spawn(&mut self, kind: OpCode) -> Ptr {
        self.spawn_in(kind, 0)
    }

    /// Get the region ID of a node.
    pub fn region_of(&self, ptr: Ptr) -> u32 {
        if ptr.is_none() || ptr.index() >= self.node_region.len() {
            return 0;
        }
        self.node_region[ptr.index()]
    }

    /// Check if two nodes are in the same region.
    pub fn same_region(&self, a: Ptr, b: Ptr) -> bool {
        self.region_of(a) == self.region_of(b)
    }

    /// Move a node to a different region.
    pub fn move_to_region(&mut self, ptr: Ptr, region_id: u32) {
        while self.node_region.len() <= ptr.index() {
            self.node_region.push(0);
        }
        self.node_region[ptr.index()] = region_id;
    }

    // ── Connection (delegates to inner) ──────────────────────────────

    pub fn connect(&mut self, a: Ptr, slot_a: u8, b: Ptr, slot_b: u8) {
        self.inner.connect(a, slot_a, b, slot_b);
    }

    pub fn port(&self, ptr: Ptr, slot: u8) -> Port {
        self.inner.port(ptr, slot)
    }

    pub fn get(&self, ptr: Ptr) -> Option<&apeiron::node::Node> {
        self.inner.get(ptr)
    }

    /// Number of node slots allocated (includes freed slots).
    pub fn node_count(&self) -> usize {
        self.node_region.len()
    }

    pub fn free(&mut self, ptr: Ptr) {
        // Clean up side tables.
        if !ptr.is_none() {
            self.radiation.remove(&(ptr.0));
            self.spins.remove(&(ptr.0));
            self.springs.remove(&(ptr.0));
            self.continuous_vars.remove(&(ptr.0));
            self.arith_constraints.remove(&(ptr.0));
            self.eclass_members.remove(&(ptr.0));
            self.eclass_of.remove(&(ptr.0));
        }
        self.inner.free(ptr);
    }

    // ── Resource mode queries ────────────────────────────────────────

    /// Check if duplication is allowed for a node (based on its region's resource mode).
    pub fn dup_allowed(&self, ptr: Ptr) -> bool {
        let region_id = self.region_of(ptr);
        match self.topology.get(region_id) {
            Some(r) => !matches!(r.resource_mode, ResourceMode::StrictlyLinear),
            None => true,
        }
    }

    /// Check if erasure is allowed for a node.
    pub fn erase_allowed(&self, ptr: Ptr) -> bool {
        let region_id = self.region_of(ptr);
        match self.topology.get(region_id) {
            Some(r) => !matches!(r.resource_mode, ResourceMode::StrictlyLinear | ResourceMode::Relevant),
            None => true,
        }
    }

    // ── Radiation / gauge fields ─────────────────────────────────────

    /// Create a new radiation source at a node, returning its marker ID.
    pub fn add_radiation_source(&mut self, ptr: Ptr) -> MarkerId {
        let marker = self.next_marker;
        self.next_marker += 1;
        self.radiation_sources.insert(marker, ptr);
        // The source node itself is always glowing.
        self.radiation.entry(ptr.0).or_default().insert(marker);
        marker
    }

    /// Check if a node is glowing with a specific marker.
    pub fn is_glowing(&self, ptr: Ptr, marker: MarkerId) -> bool {
        self.radiation
            .get(&ptr.0)
            .map_or(false, |markers| markers.contains(&marker))
    }

    /// Check if a node is glowing with any marker.
    pub fn is_glowing_any(&self, ptr: Ptr) -> bool {
        self.radiation
            .get(&ptr.0)
            .map_or(false, |markers| !markers.is_empty())
    }

    /// Set a node as glowing with a marker.
    pub fn set_glowing(&mut self, ptr: Ptr, marker: MarkerId) {
        self.radiation.entry(ptr.0).or_default().insert(marker);
    }

    /// Get all markers glowing on a node.
    pub fn markers_on(&self, ptr: Ptr) -> Vec<MarkerId> {
        self.radiation
            .get(&ptr.0)
            .map(|m| m.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Get the radiation source node for a marker.
    pub fn radiation_source(&self, marker: MarkerId) -> Option<Ptr> {
        self.radiation_sources.get(&marker).copied()
    }

    // ── Spin / thermodynamic ─────────────────────────────────────────

    /// Spawn a spin node (boolean SAT variable) in a region.
    pub fn spawn_spin(&mut self, region_id: u32, initial_polarity: bool) -> Ptr {
        let ptr = self.spawn_in(
            OpCode::Sym {
                name: "__archon_spin".into(),
                arity: 0,
            },
            region_id,
        );
        self.spins.insert(ptr.0, initial_polarity);
        ptr
    }

    /// Get spin polarity.
    pub fn spin_polarity(&self, ptr: Ptr) -> Option<bool> {
        self.spins.get(&ptr.0).copied()
    }

    /// Flip a spin's polarity.
    pub fn flip_spin(&mut self, ptr: Ptr) {
        if let Some(pol) = self.spins.get_mut(&ptr.0) {
            *pol = !*pol;
        }
    }

    /// Add a spring constraint.
    pub fn add_spring(&mut self, node: Ptr, constraint: SpringConstraint) {
        self.springs.entry(node.0).or_default().push(constraint);
    }

    // ── E-class (superposition) tracking ────────────────────────────────

    /// Record that `member` belongs to the e-class represented by `super_node`.
    pub fn add_to_eclass(&mut self, super_node: Ptr, member: Ptr) {
        self.eclass_members.entry(super_node.0).or_default().insert(member.0);
        self.eclass_of.insert(member.0, super_node.0);
    }

    /// Get all members of the e-class represented by a superposition node.
    pub fn eclass_members(&self, super_node: Ptr) -> Vec<Ptr> {
        self.eclass_members
            .get(&super_node.0)
            .map(|s| s.iter().map(|&id| Ptr(id)).collect())
            .unwrap_or_default()
    }

    /// Check if two nodes are in the same e-class via the side table.
    pub fn same_eclass_table(&self, a: Ptr, b: Ptr) -> bool {
        match (self.eclass_of.get(&a.0), self.eclass_of.get(&b.0)) {
            (Some(sa), Some(sb)) => sa == sb,
            _ => false,
        }
    }

    /// Remove e-class tracking for a superposition node (when it's freed/collapsed).
    pub fn remove_eclass(&mut self, super_node: Ptr) {
        if let Some(members) = self.eclass_members.remove(&super_node.0) {
            for m in members {
                self.eclass_of.remove(&m);
            }
        }
    }

    // ── Union-find for e-class equivalence ─────────────────────────────

    /// Find the canonical representative of a node's equivalence class.
    pub fn uf_find(&mut self, x: u32) -> u32 {
        let parent = *self.uf_parent.get(&x).unwrap_or(&x);
        if parent == x {
            return x;
        }
        let root = self.uf_find(parent);
        self.uf_parent.insert(x, root); // path compression
        root
    }

    /// Immutable find (no path compression).
    pub fn uf_find_immut(&self, mut x: u32) -> u32 {
        let mut depth = 0;
        loop {
            let parent = *self.uf_parent.get(&x).unwrap_or(&x);
            if parent == x || depth > 200 { return x; }
            x = parent;
            depth += 1;
        }
    }

    /// Union two nodes into the same equivalence class.
    /// Returns true if they were previously in different classes.
    pub fn uf_union(&mut self, a: u32, b: u32) -> bool {
        let ra = self.uf_find(a);
        let rb = self.uf_find(b);
        if ra == rb { return false; }
        // Smaller ID becomes root for determinism.
        let (root, child) = if ra < rb { (ra, rb) } else { (rb, ra) };
        self.uf_parent.insert(child, root);
        true
    }

    /// Check if two nodes are in the same equivalence class.
    pub fn uf_same(&self, a: u32, b: u32) -> bool {
        self.uf_find_immut(a) == self.uf_find_immut(b)
    }

    // ── Spatial index (physical hashcons) ────────────────────────────────

    /// Compute the structural signature of a node: its opcode + the e-class
    /// roots of all its children (aux port targets).
    pub fn compute_signature(&self, ptr: Ptr) -> Option<ENodeSignature> {
        let node = self.inner.get(ptr)?;
        let opcode = format!("{:?}", node.kind);
        let port_count = node.kind.port_count();
        let mut child_classes = Vec::new();
        for slot in 1..port_count {
            let port = self.inner.port(ptr, slot as u8);
            if port.is_connected() {
                // Resolve through union-find to get the canonical e-class root.
                let canonical = self.uf_find_immut(port.target.0);
                child_classes.push(canonical);
            } else {
                child_classes.push(u32::MAX); // disconnected sentinel
            }
        }
        Some(ENodeSignature { opcode, child_classes })
    }

    /// Register a node in the spatial index. Returns the existing node if
    /// a congruent node already exists (the hashcons hit).
    pub fn register_in_spatial_index(&mut self, ptr: Ptr) -> Option<Ptr> {
        if let Some(sig) = self.compute_signature(ptr) {
            if let Some(&existing) = self.spatial_index.get(&sig) {
                if existing != ptr && self.inner.get(existing).is_some() {
                    return Some(existing); // congruence collision
                }
            }
            self.spatial_index.insert(sig, ptr);
        }
        None
    }

    /// Record that `parent` has `child` as one of its children (for upward traversal).
    pub fn record_parent(&mut self, child: Ptr, parent: Ptr) {
        self.parent_index.entry(child.0).or_default().insert(parent.0);
    }

    /// Get all parents of a node (nodes whose aux ports point to it).
    pub fn get_parents(&self, ptr: Ptr) -> Vec<Ptr> {
        self.parent_index
            .get(&ptr.0)
            .map(|s| s.iter().map(|&id| Ptr(id)).collect())
            .unwrap_or_default()
    }

    /// Find the e-class root for a node via the union-find.
    pub fn find_eclass_root(&self, ptr: Ptr) -> Ptr {
        Ptr(self.uf_find_immut(ptr.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::*;

    #[test]
    fn spawn_in_region() {
        let mut topo = Topology::new();
        let linear = topo.add_region(
            Region::new(0, "linear")
                .with_resource(ResourceMode::StrictlyLinear)
                .with_parent(0),
        );

        let mut arena = ArchonArena::new().with_topology(topo);
        let n1 = arena.spawn_in(OpCode::Lam, linear);
        let n2 = arena.spawn(OpCode::App); // root region

        assert_eq!(arena.region_of(n1), linear);
        assert_eq!(arena.region_of(n2), 0);
        assert!(!arena.same_region(n1, n2));
    }

    #[test]
    fn resource_mode_enforcement() {
        let mut topo = Topology::new();
        let linear = topo.add_region(
            Region::new(0, "linear")
                .with_resource(ResourceMode::StrictlyLinear)
                .with_parent(0),
        );

        let mut arena = ArchonArena::new().with_topology(topo);
        let n = arena.spawn_in(OpCode::Lam, linear);

        assert!(!arena.dup_allowed(n));
        assert!(!arena.erase_allowed(n));

        let n2 = arena.spawn(OpCode::Lam); // root (OptimalSharing)
        assert!(arena.dup_allowed(n2));
        assert!(arena.erase_allowed(n2));
    }

    #[test]
    fn radiation_basics() {
        let mut arena = ArchonArena::new();
        let var_node = arena.spawn(OpCode::Sym {
            name: "x".into(),
            arity: 0,
        });
        let parent_node = arena.spawn(OpCode::App);

        let marker = arena.add_radiation_source(var_node);
        assert!(arena.is_glowing(var_node, marker));
        assert!(!arena.is_glowing(parent_node, marker));

        // Propagate manually (radiation.rs will automate this).
        arena.set_glowing(parent_node, marker);
        assert!(arena.is_glowing(parent_node, marker));
    }

    #[test]
    fn spin_basics() {
        let mut arena = ArchonArena::new();
        let spin = arena.spawn_spin(0, true);

        assert_eq!(arena.spin_polarity(spin), Some(true));
        arena.flip_spin(spin);
        assert_eq!(arena.spin_polarity(spin), Some(false));
    }
}
