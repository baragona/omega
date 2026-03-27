//! Kripke Z-axis — modal operators extrude nodes across stacked
//! world-membranes connected by wormholes.
//!
//! - Box (□): necessity. A must hold in ALL accessible worlds.
//!   When Box hits a wormhole, it duplicates the subgraph into the target world.
//!
//! - Diamond (◇): possibility. A holds in SOME accessible world.
//!   When Diamond hits a wormhole, it searches the target world for a witness.

use std::collections::HashMap;

use apeiron::node::{OpCode, Ptr};

use crate::extended_arena::ArchonArena;

/// Result of a modal interaction.
#[derive(Debug)]
pub enum ModalResult {
    /// Box: duplicated subgraph into N accessible worlds.
    Necessitated { worlds: Vec<u32> },
    /// Diamond: found witness in target world.
    Witnessed { world: u32 },
    /// Diamond: no witness found (all accessible worlds checked).
    NoWitness,
    /// Not a modal interaction.
    NotModal,
}

/// Handle a Box (necessity) node meeting a wormhole connection.
///
/// The Box node's inner content gets duplicated into every accessible world.
/// Each copy lives in its target world's region.
pub fn box_extrude(
    arena: &mut ArchonArena,
    box_node: Ptr,
    source_region: u32,
) -> ModalResult {
    let accessible = arena.topology.accessible_from(source_region);
    if accessible.is_empty() {
        return ModalResult::NotModal;
    }

    // The box node has: principal (port 0), inner content (port 1).
    let inner_port = arena.port(box_node, 1);
    if !inner_port.is_connected() {
        return ModalResult::NotModal;
    }

    let inner_node = inner_port.target;
    let mut worlds = Vec::new();

    for (i, &target_world) in accessible.iter().enumerate() {
        if i == 0 {
            // First world: move the original inner node there.
            arena.move_to_region(inner_node, target_world);
        } else {
            // Additional worlds: deep-copy the subgraph into the target world.
            let copy_root = deep_copy_subgraph(arena, inner_node, target_world);
            // Wire: the copy's principal connects to... nothing for now.
            // The box's principal reconnection (below) only wires the first copy.
            // Additional copies are available in their target worlds.
            let _ = copy_root;
        }
        worlds.push(target_world);
    }

    // Free the box node (it has been "dissolved" by the extrusion).
    let box_principal = arena.port(box_node, 0);
    if box_principal.is_connected() {
        // Reconnect: whatever was connected to box's principal
        // now connects to the inner content directly.
        arena.connect(inner_node, 0, box_principal.target, box_principal.slot);
    }
    arena.free(box_node);

    ModalResult::Necessitated { worlds }
}

/// Handle a Diamond (possibility) node.
///
/// The Diamond node searches accessible worlds for a witness.
/// Returns the first world where the inner content can be satisfied.
pub fn diamond_search(
    arena: &mut ArchonArena,
    diamond_node: Ptr,
    source_region: u32,
) -> ModalResult {
    let accessible = arena.topology.accessible_from(source_region);
    if accessible.is_empty() {
        return ModalResult::NoWitness;
    }

    // For now, pick the first accessible world as the witness world.
    // A full implementation would search for an actual witness.
    let target_world = accessible[0];

    let inner_port = arena.port(diamond_node, 1);
    if !inner_port.is_connected() {
        return ModalResult::NotModal;
    }

    // Move the inner content to the witness world.
    let inner_node = inner_port.target;
    arena.move_to_region(inner_node, target_world);

    // Dissolve the diamond node.
    let diamond_principal = arena.port(diamond_node, 0);
    if diamond_principal.is_connected() {
        arena.connect(inner_node, 0, diamond_principal.target, diamond_principal.slot);
    }
    arena.free(diamond_node);

    ModalResult::Witnessed { world: target_world }
}

/// Check if a node is a modal operator (Box or Diamond).
pub fn is_modal(arena: &ArchonArena, ptr: Ptr) -> Option<ModalKind> {
    let node = arena.get(ptr)?;
    match &node.kind {
        OpCode::Sym { name, .. } if name == "__archon_box" || name == "box" => {
            Some(ModalKind::Box)
        }
        OpCode::Sym { name, .. } if name == "__archon_diamond" || name == "diamond" => {
            Some(ModalKind::Diamond)
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalKind {
    Box,
    Diamond,
}

/// Deep-copy a subgraph rooted at `root` into `target_region`.
///
/// Walks the subgraph via ALL ports (DFS), clones each node with the same
/// opcode, and re-wires all internal connections. Returns the copy of `root`.
///
/// The copy's principal port (port 0) is left disconnected UNLESS it connects
/// to another node within the subgraph (e.g., active pairs within the subgraph).
///
/// Handles: self-loops, shared subgraphs (diamonds), Dup fans, internal
/// principal-to-principal connections, and cycles.
pub fn deep_copy_subgraph(
    arena: &mut ArchonArena,
    root: Ptr,
    target_region: u32,
) -> Ptr {
    let mut old_to_new: HashMap<u32, Ptr> = HashMap::new();
    let mut visited = std::collections::HashSet::new();

    // Phase 1: Discover all nodes reachable via aux ports from root.
    // We start from aux ports only to define the "subgraph boundary" —
    // principal ports pointing outside the subgraph are NOT followed.
    let mut stack = vec![root];
    while let Some(ptr) = stack.pop() {
        if !visited.insert(ptr.0) {
            continue;
        }
        let node = match arena.get(ptr) {
            Some(n) => n.kind.clone(),
            None => continue,
        };

        let port_count = node.port_count();
        // Walk aux ports to discover children.
        for slot in 1..port_count {
            let port = arena.port(ptr, slot as u8);
            if port.is_connected() && arena.get(port.target).is_some() {
                stack.push(port.target);
            }
        }

        // Also follow port 0 IF the target was already discovered
        // (handles internal active pairs). We'll do a second pass for this.
    }

    // Second discovery pass: follow principal ports if target is in the subgraph.
    // This catches internal active pairs and back-edges.
    let mut changed = true;
    while changed {
        changed = false;
        let current: Vec<u32> = visited.iter().copied().collect();
        for &id in &current {
            let ptr = Ptr(id);
            let node = match arena.get(ptr) {
                Some(n) => n.kind.clone(),
                None => continue,
            };
            let port_count = node.port_count();
            for slot in 0..port_count {
                let port = arena.port(ptr, slot as u8);
                if port.is_connected() && arena.get(port.target).is_some() {
                    // For port 0: only include if target is already in visited.
                    // For other ports: always include.
                    if slot > 0 || visited.contains(&port.target.0) {
                        if visited.insert(port.target.0) {
                            changed = true;
                        }
                    }
                }
            }
        }
    }

    // Phase 2: Clone all discovered nodes.
    for &id in &visited {
        let ptr = Ptr(id);
        let node = match arena.get(ptr) {
            Some(n) => n.kind.clone(),
            None => continue,
        };
        let copy = arena.spawn_in(node, target_region);
        old_to_new.insert(id, copy);
    }

    // Phase 3: Re-wire ALL internal connections (including port 0 if internal).
    for &old_id in &visited {
        let old_ptr = Ptr(old_id);
        let new_ptr = match old_to_new.get(&old_id) {
            Some(&p) => p,
            None => continue,
        };
        let kind = match arena.get(old_ptr) {
            Some(n) => n.kind.clone(),
            None => continue,
        };
        let port_count = kind.port_count();

        for slot in 0..port_count {
            let port = arena.port(old_ptr, slot as u8);
            if port.is_connected() {
                if let Some(&target_copy) = old_to_new.get(&port.target.0) {
                    // Internal edge: connect copy-to-copy.
                    let target_slot = port.slot;
                    // Avoid double-connecting (each edge is bidirectional).
                    // Only connect if this side hasn't been wired yet.
                    let new_port = arena.port(new_ptr, slot as u8);
                    if !new_port.is_connected() || new_port.target != target_copy {
                        arena.connect(new_ptr, slot as u8, target_copy, target_slot);
                    }
                }
                // External edges: leave disconnected (boundary of the copy).
            }
        }
    }

    *old_to_new.get(&root.0).unwrap_or(&Ptr::NONE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::*;

    #[test]
    fn box_extrudes_to_accessible_worlds() {
        let mut topo = Topology::new();
        let w1 = topo.add_region(Region::new(0, "world-1").with_parent(0));
        let w2 = topo.add_region(Region::new(0, "world-2").with_parent(0));
        topo.add_wormhole(w1, w2);

        let mut arena = ArchonArena::new().with_topology(topo);

        // Create: box(A) in world-1.
        let box_node = arena.spawn_in(
            OpCode::Sym { name: "__archon_box".into(), arity: 1 },
            w1,
        );
        let content = arena.spawn_in(
            OpCode::Sym { name: "A".into(), arity: 0 },
            w1,
        );
        let root = arena.spawn_in(
            OpCode::Sym { name: "root".into(), arity: 1 },
            w1,
        );

        arena.connect(box_node, 1, content, 0);
        arena.connect(box_node, 0, root, 1);

        let result = box_extrude(&mut arena, box_node, w1);
        assert!(matches!(result, ModalResult::Necessitated { ref worlds } if worlds == &[w2]));

        // Content should now be in world-2.
        assert_eq!(arena.region_of(content), w2);
        // Box node should be freed.
        assert!(arena.get(box_node).is_none());
    }

    #[test]
    fn diamond_finds_witness() {
        let mut topo = Topology::new();
        let w1 = topo.add_region(Region::new(0, "world-1").with_parent(0));
        let w2 = topo.add_region(Region::new(0, "world-2").with_parent(0));
        topo.add_wormhole(w1, w2);

        let mut arena = ArchonArena::new().with_topology(topo);

        let diamond = arena.spawn_in(
            OpCode::Sym { name: "__archon_diamond".into(), arity: 1 },
            w1,
        );
        let content = arena.spawn_in(
            OpCode::Sym { name: "witness".into(), arity: 0 },
            w1,
        );
        let root = arena.spawn_in(
            OpCode::Sym { name: "root".into(), arity: 1 },
            w1,
        );

        arena.connect(diamond, 1, content, 0);
        arena.connect(diamond, 0, root, 1);

        let result = diamond_search(&mut arena, diamond, w1);
        assert!(matches!(result, ModalResult::Witnessed { world } if world == w2));
        assert_eq!(arena.region_of(content), w2);
    }
}
