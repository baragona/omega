//! Topological radiation — gauge fields that encode global structural
//! information as locally-propagating markers.
//!
//! A variable node emits "X-radiation" that flows backward up the wires.
//! Any node it passes through becomes "glowing" with that marker.
//! This replaces global structural recursion with local field propagation.
//!
//! Used for:
//! - DependentCombinators: bracket abstraction via S/K/I crystallization
//! - HOASDefunctionalization: closure conversion via occurrence markers
//! - ClauseCompilation: grounding field for first-order resolution

use apeiron::node::Ptr;

use crate::extended_arena::{ArchonArena, MarkerId};

/// Propagate radiation one hop from each glowing node to its neighbors.
/// Returns the number of new nodes that became glowing (0 = fixpoint reached).
pub fn propagate_one_hop(arena: &ArchonArena, updates: &mut Vec<(Ptr, MarkerId)>) -> usize {
    updates.clear();
    let capacity = arena.inner.node_capacity();

    for idx in 0..capacity {
        let ptr = Ptr(idx as u32);
        let markers = arena.markers_on(ptr);
        if markers.is_empty() {
            continue;
        }

        // Get port count for this node.
        let node = match arena.get(ptr) {
            Some(n) => n,
            None => continue,
        };
        let port_count = node.kind.port_count();

        // Radiation flows backward: from aux ports toward whoever is connected.
        // For each port, if the connected node is NOT glowing with this marker,
        // schedule it to become glowing.
        //
        // Region-aware: radiation does NOT cross non-transparent boundaries.
        // This ensures radiation fields are scoped to their region, which is
        // critical for combinator filters, grounding, and modal restriction.
        let src_region = arena.region_of(ptr);

        for slot in 0..port_count {
            let port = arena.port(ptr, slot as u8);
            if !port.is_connected() {
                continue;
            }
            let neighbor = port.target;
            if arena.get(neighbor).is_none() {
                continue;
            }

            // Block radiation at non-transparent boundaries.
            let neighbor_region = arena.region_of(neighbor);
            if src_region != neighbor_region {
                use crate::region::BoundaryType;
                let boundary = arena.topology.boundary_between(src_region, neighbor_region)
                    .or_else(|| arena.topology.boundary_between(neighbor_region, src_region));
                match boundary {
                    Some(BoundaryType::Transparent) | None => {
                        // Transparent or no boundary: radiation passes.
                    }
                    // Boundaries that are permeable to radiation:
                    // These don't fundamentally change term identity, so
                    // variable occurrence info should propagate through.
                    Some(BoundaryType::EffectBoundary)
                    | Some(BoundaryType::ACBoundary)
                    | Some(BoundaryType::ContextReifyBoundary) => {
                        // Permeable: radiation passes through.
                    }
                    Some(_) => {
                        // Opaque boundary: radiation is blocked.
                        continue;
                    }
                }
            }

            for &marker in &markers {
                if !arena.is_glowing(neighbor, marker) {
                    updates.push((neighbor, marker));
                }
            }
        }
    }

    let count = updates.len();
    count
}

/// Apply pending radiation updates.
pub fn apply_updates(arena: &mut ArchonArena, updates: &[(Ptr, MarkerId)]) {
    for &(ptr, marker) in updates {
        arena.set_glowing(ptr, marker);
    }
}

/// Propagate radiation to fixpoint (all reachable nodes glow).
/// Returns the total number of hops taken.
pub fn propagate_to_fixpoint(arena: &mut ArchonArena, fuel: u32) -> u32 {
    let mut updates = Vec::new();
    let mut hops = 0;

    for _ in 0..fuel {
        let new = propagate_one_hop(arena, &mut updates);
        if new == 0 {
            break;
        }
        apply_updates(arena, &updates);
        hops += 1;
    }

    hops
}

/// Set up radiation sources for all variable-like nodes in a subgraph.
/// A "variable-like" node is one that represents a bound variable
/// (used for bracket abstraction / combinator conversion).
///
/// Returns the list of (node, marker_id) pairs created.
pub fn mark_variables(arena: &mut ArchonArena, var_ptrs: &[Ptr]) -> Vec<(Ptr, MarkerId)> {
    var_ptrs
        .iter()
        .map(|&ptr| {
            let marker = arena.add_radiation_source(ptr);
            (ptr, marker)
        })
        .collect()
}

/// Check if a specific wire (port on a node) is glowing with a marker.
/// This is what the combinator boundary checks to decide S vs K.
pub fn wire_is_glowing(arena: &ArchonArena, node: Ptr, slot: u8, marker: MarkerId) -> bool {
    let port = arena.port(node, slot);
    if !port.is_connected() {
        return false;
    }
    // The wire is "glowing" if the node on the other end is glowing,
    // OR if the node itself (this side) is glowing on that port's direction.
    arena.is_glowing(port.target, marker) || arena.is_glowing(node, marker)
}

#[cfg(test)]
mod tests {
    use super::*;
    use apeiron::node::OpCode;

    #[test]
    fn radiation_propagates() {
        let mut arena = ArchonArena::new();

        // Build a chain: A -- B -- C
        // A is the radiation source.
        let a = arena.spawn(OpCode::Sym { name: "a".into(), arity: 1 });
        let b = arena.spawn(OpCode::Sym { name: "b".into(), arity: 2 });
        let c = arena.spawn(OpCode::Sym { name: "c".into(), arity: 1 });

        arena.connect(a, 1, b, 0);  // a.aux1 -- b.principal
        arena.connect(b, 1, c, 0);  // b.aux1 -- c.principal

        let marker = arena.add_radiation_source(a);
        assert!(arena.is_glowing(a, marker));
        assert!(!arena.is_glowing(b, marker));
        assert!(!arena.is_glowing(c, marker));

        // One hop: A → B
        let hops = propagate_to_fixpoint(&mut arena, 1);
        assert_eq!(hops, 1);
        assert!(arena.is_glowing(b, marker));
        assert!(!arena.is_glowing(c, marker)); // not yet

        // Second hop: B → C
        let hops2 = propagate_to_fixpoint(&mut arena, 1);
        assert_eq!(hops2, 1);
        assert!(arena.is_glowing(c, marker));
    }

    #[test]
    fn radiation_fixpoint() {
        let mut arena = ArchonArena::new();

        let a = arena.spawn(OpCode::Sym { name: "a".into(), arity: 1 });
        let b = arena.spawn(OpCode::Sym { name: "b".into(), arity: 1 });
        let c = arena.spawn(OpCode::Sym { name: "c".into(), arity: 1 });

        arena.connect(a, 1, b, 0);
        arena.connect(b, 1, c, 0);

        let marker = arena.add_radiation_source(a);
        let hops = propagate_to_fixpoint(&mut arena, 100);
        assert_eq!(hops, 2); // 2 hops to reach fixpoint

        assert!(arena.is_glowing(a, marker));
        assert!(arena.is_glowing(b, marker));
        assert!(arena.is_glowing(c, marker));
    }

    #[test]
    fn disconnected_nodes_dont_glow() {
        let mut arena = ArchonArena::new();

        let a = arena.spawn(OpCode::Sym { name: "a".into(), arity: 0 });
        let b = arena.spawn(OpCode::Sym { name: "b".into(), arity: 0 });
        // No connection between a and b.

        let marker = arena.add_radiation_source(a);
        propagate_to_fixpoint(&mut arena, 100);

        assert!(arena.is_glowing(a, marker));
        assert!(!arena.is_glowing(b, marker)); // unreachable
    }
}
