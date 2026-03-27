//! Quantum Superposition — e-graph equality saturation via physics.
//!
//! In a standard interaction net, a wire connects one port to one port.
//! In an e-graph, an e-class is a hub where many equivalent nodes coexist.
//!
//! We bridge this gap with **Superposition particles**: when a `@law` asserts
//! LHS ≡ RHS, the physics engine doesn't destroy the LHS. Instead, it unplugs
//! the LHS from its parent, inserts a `__archon_super` node, and plugs both
//! LHS and the newly materialized RHS into it. The Superposition node acts as
//! a quantum multiplexer — signals split and travel down both realities.
//!
//! **Saturation** emerges from floating Law catalysts that emit pattern radiation,
//! bind to matching subgraphs, trigger quantum fluctuations (materializing new
//! alternatives inside Superposition nodes), and detach to float again.
//!
//! **Congruence closure** propagates upward: when children become superposed,
//! their parent nodes experience constructive interference and merge into
//! higher-level Superpositions.
//!
//! **Extraction** is thermodynamic collapse: lowering temperature forces each
//! Superposition to pick its lowest-energy (smallest AST) child.

use apeiron::node::{OpCode, Ptr};

use crate::extended_arena::ArchonArena;

// ── Constants ──────────────────────────────────────────────────────────

/// The reserved name for superposition nodes.
pub const SUPER_NAME: &str = "__archon_super";

/// The reserved name for law catalyst nodes.
pub const LAW_CATALYST_NAME: &str = "__archon_law";

// ── Predicates ─────────────────────────────────────────────────────────

/// Check if a node is a Superposition particle.
pub fn is_superposition(arena: &ArchonArena, ptr: Ptr) -> bool {
    arena.get(ptr).map_or(false, |n| {
        matches!(&n.kind, OpCode::Sym { name, .. } if name == SUPER_NAME)
    })
}

/// Check if a node is a Law catalyst.
pub fn is_law_catalyst(arena: &ArchonArena, ptr: Ptr) -> bool {
    arena.get(ptr).map_or(false, |n| {
        matches!(&n.kind, OpCode::Sym { name, .. } if name == LAW_CATALYST_NAME)
    })
}

/// Get the arity (number of alternatives) of a Superposition node.
pub fn super_arity(arena: &ArchonArena, ptr: Ptr) -> u8 {
    arena.get(ptr).map_or(0, |n| {
        match &n.kind {
            OpCode::Sym { name, arity } if name == SUPER_NAME => *arity,
            _ => 0,
        }
    })
}

// ── Superposition creation ─────────────────────────────────────────────

/// Spawn a Superposition node with the given number of alternatives.
///
/// Port layout:
/// - Port 0 (principal): connects upward to the parent/consumer
/// - Ports 1..N: each connects to one alternative child
pub fn spawn_super(arena: &mut ArchonArena, region: u32, alternatives: u8) -> Ptr {
    arena.spawn_in(
        OpCode::Sym {
            name: SUPER_NAME.into(),
            arity: alternatives,
        },
        region,
    )
}

/// Insert a Superposition between a node and its parent, adding a new
/// alternative alongside the original.
///
/// Before: parent.slot ←→ original.port
/// After:  parent.slot ←→ super.0, super.1 ←→ original.port, super.2 ←→ new_alt.port
///
/// Returns the Superposition node.
pub fn superpose(
    arena: &mut ArchonArena,
    original: Ptr,
    original_slot: u8,
    new_alt: Ptr,
    new_alt_slot: u8,
    region: u32,
) -> Ptr {
    // Record who original was connected to on `original_slot`.
    let parent_port = arena.port(original, original_slot);

    // Create a 2-alternative superposition.
    let sup = spawn_super(arena, region, 2);

    // Wire: super.0 ←→ parent (where original used to connect)
    if parent_port.is_connected() {
        arena.connect(sup, 0, parent_port.target, parent_port.slot);
    }

    // Wire: super.1 ←→ original
    arena.connect(sup, 1, original, original_slot);

    // Wire: super.2 ←→ new alternative
    arena.connect(sup, 2, new_alt, new_alt_slot);

    // Track the e-class membership.
    arena.add_to_eclass(sup, original);
    arena.add_to_eclass(sup, new_alt);

    sup
}

/// Extend an existing Superposition with one more alternative.
///
/// This "grows" the e-class by replacing the old super node with a wider one.
/// Returns the new Superposition node (old one is freed).
pub fn extend_super(
    arena: &mut ArchonArena,
    old_super: Ptr,
    new_alt: Ptr,
    new_alt_slot: u8,
) -> Ptr {
    let region = arena.region_of(old_super);
    let old_arity = super_arity(arena, old_super);

    // Collect existing connections.
    let parent_port = arena.port(old_super, 0);
    let mut child_ports: Vec<(Ptr, u8)> = Vec::new();
    for i in 1..=old_arity {
        let p = arena.port(old_super, i);
        if p.is_connected() {
            child_ports.push((p.target, p.slot));
        }
    }

    // Spawn a wider superposition.
    let new_super = spawn_super(arena, region, old_arity + 1);

    // Reconnect parent.
    if parent_port.is_connected() {
        arena.connect(new_super, 0, parent_port.target, parent_port.slot);
    }

    // Reconnect existing children.
    for (i, (target, slot)) in child_ports.iter().enumerate() {
        arena.connect(new_super, (i + 1) as u8, *target, *slot);
    }

    // Connect new alternative.
    arena.connect(new_super, old_arity + 1, new_alt, new_alt_slot);

    // Transfer e-class tracking.
    let old_members = arena.eclass_members(old_super);
    for m in old_members {
        arena.add_to_eclass(new_super, m);
    }
    arena.add_to_eclass(new_super, new_alt);
    arena.remove_eclass(old_super);

    arena.free(old_super);
    new_super
}

// ── Active pair dispatch ───────────────────────────────────────────────

/// Handle an active pair where one side is a Superposition node.
///
/// The Superposition acts as a multiplexer: the interaction is replicated
/// across all alternatives. Each alternative gets its own copy of the
/// interacting node (via Dup), maintaining the invariant that all
/// alternatives remain live.
///
/// Returns the rule name, or None if this isn't a super interaction.
pub fn dispatch_super(
    arena: &mut ArchonArena,
    left: Ptr,
    left_kind: &OpCode,
    right: Ptr,
    right_kind: &OpCode,
) -> Option<String> {
    let (sup, sup_kind, other, other_kind) =
        if matches!(left_kind, OpCode::Sym { name, .. } if name == SUPER_NAME) {
            (left, left_kind, right, right_kind)
        } else if matches!(right_kind, OpCode::Sym { name, .. } if name == SUPER_NAME) {
            (right, right_kind, left, left_kind)
        } else {
            return None;
        };

    let arity = match sup_kind {
        OpCode::Sym { arity, .. } => *arity,
        _ => return None,
    };

    // The other node interacted with the super's principal port (port 0).
    // We need to fan out: duplicate the other node and push active pairs
    // with each alternative.
    //
    // For arity=2: dup the other node, connect copies to alternatives.
    // For arity>2: chain of dups.

    if arity == 0 {
        // Degenerate: empty superposition. Just free it.
        arena.free(sup);
        return Some("Super-Empty".into());
    }

    if arity == 1 {
        // Singleton superposition: collapse to the single alternative.
        let child_port = arena.port(sup, 1);
        if child_port.is_connected() {
            let other_port = arena.port(other, 0);
            // Connect the alternative directly to whatever the other node connects to.
            arena.inner.active_pairs.push((other, child_port.target));
        }
        arena.free(sup);
        return Some("Super-Collapse-Singleton".into());
    }

    // General case: fan out via Dup nodes.
    let region = arena.region_of(sup);
    let label = sup.0; // unique label per superposition

    // We need (arity - 1) Dup nodes to split `other` into `arity` copies.
    // Build a binary tree of Dups.
    let mut targets: Vec<Ptr> = Vec::new();
    for i in 1..=arity {
        let p = arena.port(sup, i);
        if p.is_connected() {
            targets.push(p.target);
        }
    }

    if targets.len() <= 1 {
        // Only one real alternative.
        if let Some(&t) = targets.first() {
            arena.inner.active_pairs.push((other, t));
        }
        arena.free(sup);
        return Some("Super-Collapse-Single".into());
    }

    // Create dup chain: other fans into copies, each paired with an alternative.
    let mut current = other;
    for (i, &target) in targets.iter().enumerate() {
        if i == targets.len() - 1 {
            // Last one: connect directly (no more duping needed).
            arena.inner.active_pairs.push((current, target));
        } else {
            // Dup current: copy_a goes to this target, copy_b continues.
            let dup = arena.spawn_in(OpCode::Dup { label }, region);
            let copy_a = arena.spawn_in(other_kind.clone(), region);
            let copy_b = arena.spawn_in(other_kind.clone(), region);

            arena.connect(dup, 0, current, 0);
            arena.connect(dup, 1, copy_a, 0);
            arena.connect(dup, 2, copy_b, 0);

            arena.inner.active_pairs.push((copy_a, target));
            current = copy_b;
        }
    }

    arena.free(sup);
    Some(format!("Super-Fanout({})", arity))
}

// ── Congruence closure ─────────────────────────────────────────────────

/// Propagate congruence closure via the shockwave queue.
///
/// When two nodes merge into a Superposition, every parent node's structural
/// signature changes (one of its children now points to a different e-class root).
/// If two parents now have identical signatures, they are congruent and must
/// also be merged — this is the cascade.
///
/// This implements egg's `rebuild()` loop as a physical worklist in the arena.
/// Returns the number of congruence merges performed.
pub fn propagate_congruence(arena: &mut ArchonArena) -> usize {
    let mut merges = 0;

    while let Some(merged_hub) = arena.shockwave_queue.pop() {
        // Skip if this hub was already freed (superseded by a later merge).
        if arena.get(merged_hub).is_none() {
            continue;
        }
        // Find all parent nodes that reference this merged hub (or its members).
        let mut parents_to_check: Vec<Ptr> = Vec::new();

        // Collect parents of all e-class members (including the hub itself).
        let members = arena.eclass_members(merged_hub);
        for member in &members {
            parents_to_check.extend(arena.get_parents(*member));
        }
        parents_to_check.extend(arena.get_parents(merged_hub));
        parents_to_check.sort_unstable_by_key(|p| p.0);
        parents_to_check.dedup();

        for parent in parents_to_check {
            if arena.get(parent).is_none() {
                continue;
            }
            if is_superposition(arena, parent) {
                continue; // don't re-index superposition nodes themselves
            }

            // Recompute this parent's signature (children resolved through e-classes).
            let sig = match arena.compute_signature(parent) {
                Some(s) => s,
                None => continue,
            };

            // Check the spatial index for a collision.
            if let Some(&existing) = arena.spatial_index.get(&sig) {
                if existing != parent && arena.get(existing).is_some()
                    && !arena.uf_same(parent.0, existing.0)
                {
                    // Congruence! Two parents have identical structure.
                    // Merge them into a superposition.
                    let region = arena.region_of(parent);
                    let sup = superpose(arena, parent, 0, existing, 0, region);
                    arena.uf_union(parent.0, existing.0);
                    arena.uf_union(parent.0, sup.0); // hub must be in the class
                    // The new superposition is itself a shockwave source.
                    arena.shockwave_queue.push(sup);
                    merges += 1;
                    continue;
                }
            }

            // No collision — register (or update) in the spatial index.
            arena.spatial_index.insert(sig, parent);
        }
    }

    merges
}

// ── Extraction (thermodynamic collapse) ────────────────────────────────

/// Collapse all Superposition nodes by selecting the lowest-energy alternative.
///
/// Energy is measured by AST size (number of reachable nodes from each
/// alternative's root). The alternative with the smallest subgraph wins.
///
/// This is the "wavefunction collapse" — temperature drops to zero and
/// each Superposition must pick one classical reality.
pub fn collapse_all(arena: &mut ArchonArena) -> usize {
    let capacity = arena.inner.node_capacity();
    let mut supers: Vec<Ptr> = Vec::new();

    for idx in 0..capacity {
        let ptr = Ptr(idx as u32);
        if is_superposition(arena, ptr) {
            supers.push(ptr);
        }
    }

    let mut collapsed = 0;
    for sup in supers {
        if arena.get(sup).is_none() {
            continue; // already freed by a previous collapse
        }
        if collapse_one(arena, sup) {
            collapsed += 1;
        }
    }
    collapsed
}

/// Collapse a single Superposition node, picking the smallest alternative.
fn collapse_one(arena: &mut ArchonArena, sup: Ptr) -> bool {
    let arity = super_arity(arena, sup);
    if arity == 0 {
        arena.free(sup);
        return true;
    }

    // Measure energy (AST size) of each alternative.
    let mut best_slot: u8 = 1;
    let mut best_energy = u64::MAX;

    for i in 1..=arity {
        let port = arena.port(sup, i);
        if !port.is_connected() {
            continue;
        }
        let energy = measure_energy(arena, port.target, 100);
        if energy < best_energy {
            best_energy = energy;
            best_slot = i;
        }
    }

    // Reconnect: winner takes the parent wire, losers get freed.
    let parent_port = arena.port(sup, 0);
    let winner_port = arena.port(sup, best_slot);

    if parent_port.is_connected() && winner_port.is_connected() {
        arena.connect(
            parent_port.target, parent_port.slot,
            winner_port.target, winner_port.slot,
        );
    }

    // Free loser alternatives (shallow — only the root node of each subtree).
    for i in 1..=arity {
        if i == best_slot {
            continue;
        }
        let port = arena.port(sup, i);
        if port.is_connected() {
            // Don't deeply free — other parts of the graph may reference these.
            // Just disconnect.
        }
    }

    arena.remove_eclass(sup);
    arena.free(sup);
    true
}

/// Measure the "energy" (AST size) of a subgraph rooted at `root`.
/// Uses BFS with a fuel limit to avoid infinite loops on cyclic graphs.
fn measure_energy(arena: &ArchonArena, root: Ptr, fuel: usize) -> u64 {
    use std::collections::VecDeque;

    let mut visited = std::collections::HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(root);
    visited.insert(root.0);

    let mut count: u64 = 0;

    while let Some(ptr) = queue.pop_front() {
        if count as usize >= fuel {
            break;
        }
        let node = match arena.get(ptr) {
            Some(n) => n,
            None => continue,
        };
        count += 1;

        // Traverse auxiliary ports (skip principal / port 0 to avoid going up).
        let port_count = node.kind.port_count();
        for slot in 1..port_count {
            let port = arena.port(ptr, slot as u8);
            if port.is_connected() && !visited.contains(&port.target.0) {
                // Don't traverse into other superpositions (they're separate e-classes).
                if !is_superposition(arena, port.target) {
                    visited.insert(port.target.0);
                    queue.push_back(port.target);
                }
            }
        }
    }

    count
}

// ── Law catalysts ──────────────────────────────────────────────────────

/// Spawn a Law catalyst node.
///
/// A Law catalyst is a floating node that carries a rewrite pattern.
/// It emits pattern radiation, looking for matching subgraphs.
/// When it finds one, it triggers a quantum fluctuation (superposition).
///
/// Port layout:
/// - Port 0: principal (floats free or connects to matched node)
/// - Port 1: LHS pattern root
/// - Port 2: RHS pattern root
pub fn spawn_law_catalyst(
    arena: &mut ArchonArena,
    region: u32,
    lhs_root: Ptr,
    rhs_root: Ptr,
) -> Ptr {
    let catalyst = arena.spawn_in(
        OpCode::Sym {
            name: LAW_CATALYST_NAME.into(),
            arity: 2,
        },
        region,
    );
    arena.connect(catalyst, 1, lhs_root, 0);
    arena.connect(catalyst, 2, rhs_root, 0);
    catalyst
}

// ── Equality check ─────────────────────────────────────────────────────

/// Check if two nodes are in the same e-class (connected via Superpositions).
///
/// This is the physics equivalent of `egg::EGraph::find(a) == find(b)`.
pub fn same_eclass(arena: &ArchonArena, a: Ptr, b: Ptr) -> bool {
    if a == b {
        return true;
    }
    // Walk up from both nodes, looking for a common Superposition ancestor.
    let a_supers = find_super_ancestors(arena, a, 50);
    let b_supers = find_super_ancestors(arena, b, 50);
    // If they share any superposition ancestor, they're in the same e-class.
    for sa in &a_supers {
        if b_supers.contains(sa) {
            return true;
        }
    }
    // Also check the e-class side table.
    arena.same_eclass_table(a, b)
}

/// Find all Superposition ancestors of a node (walk up principal ports).
fn find_super_ancestors(arena: &ArchonArena, start: Ptr, fuel: usize) -> Vec<Ptr> {
    let mut result = Vec::new();
    let mut current = start;
    for _ in 0..fuel {
        let port = arena.port(current, 0);
        if !port.is_connected() {
            break;
        }
        let parent = port.target;
        if is_superposition(arena, parent) {
            result.push(parent);
        }
        current = parent;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym(arena: &mut ArchonArena, name: &str, arity: u8) -> Ptr {
        arena.spawn(OpCode::Sym { name: name.into(), arity })
    }

    #[test]
    fn superpose_creates_super_node() {
        let mut arena = ArchonArena::new();

        let parent = sym(&mut arena, "root", 1);
        let original = sym(&mut arena, "a", 0);
        let alt = sym(&mut arena, "b", 0);

        arena.connect(parent, 1, original, 0);

        let sup = superpose(&mut arena, original, 0, alt, 0, 0);

        // Super node should exist with arity 2.
        assert!(is_superposition(&arena, sup));
        assert_eq!(super_arity(&arena, sup), 2);

        // Parent should now connect to super.
        let parent_child = arena.port(parent, 1);
        assert_eq!(parent_child.target, sup);

        // Super's children should be original and alt.
        let child1 = arena.port(sup, 1);
        let child2 = arena.port(sup, 2);
        assert_eq!(child1.target, original);
        assert_eq!(child2.target, alt);
    }

    #[test]
    fn same_eclass_via_superposition() {
        let mut arena = ArchonArena::new();

        let a = sym(&mut arena, "a", 0);
        let b = sym(&mut arena, "b", 0);
        let parent = sym(&mut arena, "root", 1);

        arena.connect(parent, 1, a, 0);

        // Before superposition: not same e-class.
        assert!(!same_eclass(&arena, a, b));

        // Create superposition.
        let _sup = superpose(&mut arena, a, 0, b, 0, 0);

        // Now they should be in the same e-class.
        assert!(same_eclass(&arena, a, b));
    }

    #[test]
    fn collapse_picks_smallest() {
        let mut arena = ArchonArena::new();

        // "a" is a leaf (energy=1), "f(g(x))" has energy=3.
        let small = sym(&mut arena, "a", 0);
        let f = sym(&mut arena, "f", 1);
        let g = sym(&mut arena, "g", 1);
        let x = sym(&mut arena, "x", 0);
        arena.connect(f, 1, g, 0);
        arena.connect(g, 1, x, 0);

        let parent = sym(&mut arena, "root", 1);
        arena.connect(parent, 1, small, 0);

        let _sup = superpose(&mut arena, small, 0, f, 0, 0);

        // Collapse should pick "a" (energy=1) over "f(g(x))" (energy=3).
        let collapsed = collapse_all(&mut arena);
        assert_eq!(collapsed, 1);

        // Parent should now connect to "a".
        let child = arena.port(parent, 1);
        assert!(child.is_connected());
        assert_eq!(
            arena.get(child.target).unwrap().kind,
            OpCode::Sym { name: "a".into(), arity: 0 }
        );
    }

    #[test]
    fn extend_super_grows_eclass() {
        let mut arena = ArchonArena::new();

        let a = sym(&mut arena, "a", 0);
        let b = sym(&mut arena, "b", 0);
        let c = sym(&mut arena, "c", 0);
        let parent = sym(&mut arena, "root", 1);

        arena.connect(parent, 1, a, 0);

        // Create {a, b}.
        let sup = superpose(&mut arena, a, 0, b, 0, 0);
        assert_eq!(super_arity(&arena, sup), 2);

        // Extend to {a, b, c}.
        let sup2 = extend_super(&mut arena, sup, c, 0);
        assert_eq!(super_arity(&arena, sup2), 3);

        // Old super should be freed.
        assert!(arena.get(sup).is_none());
    }

    #[test]
    fn dispatch_super_fans_out() {
        let mut arena = ArchonArena::new();

        let a = sym(&mut arena, "a", 0);
        let b = sym(&mut arena, "b", 0);
        let consumer = sym(&mut arena, "f", 1);

        // Create superposition {a, b}.
        let sup = spawn_super(&mut arena, 0, 2);
        arena.connect(sup, 1, a, 0);
        arena.connect(sup, 2, b, 0);
        arena.connect(sup, 0, consumer, 0);

        let sup_kind = arena.get(sup).unwrap().kind.clone();
        let consumer_kind = arena.get(consumer).unwrap().kind.clone();

        let result = dispatch_super(&mut arena, sup, &sup_kind, consumer, &consumer_kind);
        assert!(result.is_some());
        assert!(result.unwrap().starts_with("Super-Fanout"));

        // Super should be freed.
        assert!(arena.get(sup).is_none());

        // Active pairs should have been enqueued for each alternative.
        assert!(!arena.inner.active_pairs.is_empty());
    }

    #[test]
    fn measure_energy_counts_nodes() {
        let mut arena = ArchonArena::new();

        let a = sym(&mut arena, "a", 0);
        assert_eq!(measure_energy(&arena, a, 100), 1);

        let f = sym(&mut arena, "f", 2);
        let x = sym(&mut arena, "x", 0);
        let y = sym(&mut arena, "y", 0);
        arena.connect(f, 1, x, 0);
        arena.connect(f, 2, y, 0);

        assert_eq!(measure_energy(&arena, f, 100), 3);
    }
}
