use std::collections::{HashMap, HashSet};

use crate::node::{Node, OpCode, Port, Ptr};

/// Statistics for the arena.
#[derive(Default, Debug, Clone)]
pub struct ArenaStats {
    pub nodes_spawned: u64,
    pub nodes_freed: u64,
    pub interactions: u64,
}

/// The interaction net graph store.
pub struct Arena {
    /// All nodes, indexed by Ptr. Freed slots contain None.
    nodes: Vec<Option<Node>>,
    /// Recycled indices for reuse.
    free_list: Vec<u32>,
    /// Active pairs: nodes whose principal ports face each other.
    pub active_pairs: Vec<(Ptr, Ptr)>,
    /// Nodes waiting for a scope to become active.
    pub listeners: HashMap<u32, Vec<Ptr>>,
    /// Currently active scopes.
    pub active_scopes: HashSet<u32>,
    /// Running statistics.
    pub stats: ArenaStats,
    /// Next dup label (auto-increment for builder).
    pub next_dup_label: u32,
    /// Active pairs suspended inside inactive barriers, keyed by scope ID.
    /// When a scope is activated, its suspended pairs are moved to `active_pairs`.
    pub suspended_pairs: HashMap<u32, Vec<(Ptr, Ptr)>>,
    /// When true, freed indices are deferred (not recycled immediately).
    /// Used during building to prevent index aliasing.
    building: bool,
    /// Indices freed during building, added to free_list when building ends.
    deferred_free: Vec<u32>,
}

impl Arena {
    pub fn new() -> Self {
        Arena {
            nodes: Vec::new(),
            free_list: Vec::new(),
            active_pairs: Vec::new(),
            listeners: HashMap::new(),
            active_scopes: HashSet::new(),
            stats: ArenaStats::default(),
            next_dup_label: 0,
            suspended_pairs: HashMap::new(),
            building: false,
            deferred_free: Vec::new(),
        }
    }

    /// Allocate a new node, returning its Ptr.
    pub fn spawn(&mut self, kind: OpCode) -> Ptr {
        let node_id = if let Some(recycled) = self.free_list.pop() {
            let ptr = Ptr(recycled);
            self.nodes[ptr.index()] = Some(Node::new(ptr, kind));
            ptr
        } else {
            let idx = self.nodes.len() as u32;
            let ptr = Ptr(idx);
            self.nodes.push(Some(Node::new(ptr, kind)));
            ptr
        };
        self.stats.nodes_spawned += 1;
        node_id
    }

    /// Free a node, returning its slot to the free list.
    /// During building mode, freed indices are deferred to prevent aliasing.
    pub fn free(&mut self, ptr: Ptr) {
        if ptr.is_none() {
            return;
        }
        if let Some(slot) = self.nodes.get_mut(ptr.index()) {
            if slot.is_some() {
                *slot = None;
                if self.building {
                    self.deferred_free.push(ptr.0);
                } else {
                    self.free_list.push(ptr.0);
                }
                self.stats.nodes_freed += 1;
            }
        }
    }

    /// Enter building mode: prevents freed indices from being recycled.
    pub fn begin_building(&mut self) {
        self.building = true;
    }

    /// Exit building mode: flush deferred frees to the free list and
    /// purge stale active pairs (where one or both nodes are freed).
    pub fn end_building(&mut self) {
        self.building = false;
        self.free_list.extend(self.deferred_free.drain(..));
        // Purge stale active pairs created during building
        self.active_pairs.retain(|(a, b)| {
            self.nodes.get(a.index()).map_or(false, |s| s.is_some())
                && self.nodes.get(b.index()).map_or(false, |s| s.is_some())
        });
        // Purge stale suspended pairs
        for pairs in self.suspended_pairs.values_mut() {
            pairs.retain(|(a, b)| {
                self.nodes.get(a.index()).map_or(false, |s| s.is_some())
                    && self.nodes.get(b.index()).map_or(false, |s| s.is_some())
            });
        }
    }

    /// Get a reference to a node.
    pub fn get(&self, ptr: Ptr) -> Option<&Node> {
        if ptr.is_none() {
            return None;
        }
        self.nodes.get(ptr.index()).and_then(|slot| slot.as_ref())
    }

    /// Get a mutable reference to a node.
    pub fn get_mut(&mut self, ptr: Ptr) -> Option<&mut Node> {
        if ptr.is_none() {
            return None;
        }
        self.nodes.get_mut(ptr.index()).and_then(|slot| slot.as_mut())
    }

    /// Connect two ports bidirectionally.
    ///
    /// Sets `a.ports[slot_a]` to point at `(b, slot_b)` and vice versa.
    /// If both slots are 0 (principal ports), auto-enqueues as an active pair.
    pub fn connect(&mut self, a: Ptr, slot_a: u8, b: Ptr, slot_b: u8) {
        // Set a -> b
        if let Some(node_a) = self.nodes.get_mut(a.index()).and_then(|s| s.as_mut()) {
            if (slot_a as usize) < node_a.ports.len() {
                node_a.ports[slot_a as usize] = Port::new(b, slot_b);
            }
        }
        // Set b -> a
        if let Some(node_b) = self.nodes.get_mut(b.index()).and_then(|s| s.as_mut()) {
            if (slot_b as usize) < node_b.ports.len() {
                node_b.ports[slot_b as usize] = Port::new(a, slot_a);
            }
        }
        // Auto-detect active pair: both principal ports connected
        if slot_a == 0 && slot_b == 0 {
            self.active_pairs.push((a, b));
        }
    }

    /// Disconnect a port (set it and its back-reference to NONE).
    pub fn disconnect(&mut self, ptr: Ptr, slot: u8) {
        let port = if let Some(node) = self.get(ptr) {
            if (slot as usize) < node.ports.len() {
                node.ports[slot as usize]
            } else {
                return;
            }
        } else {
            return;
        };

        // Clear the back-reference
        if port.is_connected() {
            if let Some(target_node) = self.get_mut(port.target) {
                if (port.slot as usize) < target_node.ports.len() {
                    target_node.ports[port.slot as usize] = Port::disconnected();
                }
            }
        }

        // Clear our port
        if let Some(node) = self.get_mut(ptr) {
            if (slot as usize) < node.ports.len() {
                node.ports[slot as usize] = Port::disconnected();
            }
        }
    }

    /// Read a port value (cloned to avoid borrow issues).
    pub fn port(&self, ptr: Ptr, slot: u8) -> Port {
        self.get(ptr)
            .and_then(|n| n.ports.get(slot as usize).copied())
            .unwrap_or(Port::disconnected())
    }

    /// Number of live nodes.
    pub fn live_count(&self) -> usize {
        self.nodes.iter().filter(|s| s.is_some()).count()
    }

    /// Total capacity (for iterating all possible node indices).
    pub fn node_capacity(&self) -> usize {
        self.nodes.len()
    }

    /// Allocate a fresh dup label.
    pub fn fresh_dup_label(&mut self) -> u32 {
        let label = self.next_dup_label;
        self.next_dup_label += 1;
        label
    }

    /// Activate a scope, waking up any listeners and releasing suspended pairs.
    pub fn activate_scope(&mut self, scope: u32) {
        self.active_scopes.insert(scope);
        // Wake barrier listeners
        if let Some(waiters) = self.listeners.remove(&scope) {
            for ptr in waiters {
                // Re-check if the barrier node is still alive
                if let Some(node) = self.get(ptr) {
                    let principal = node.ports[0];
                    if principal.is_connected() {
                        self.active_pairs.push((ptr, principal.target));
                    }
                }
            }
        }
        // Release suspended active pairs that were inside this scope's barriers
        if let Some(pairs) = self.suspended_pairs.remove(&scope) {
            for (a, b) in pairs {
                if self.get(a).is_some() && self.get(b).is_some() {
                    self.active_pairs.push((a, b));
                }
            }
        }
    }

    /// Deactivate a scope.
    pub fn deactivate_scope(&mut self, scope: u32) {
        self.active_scopes.remove(&scope);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_and_get() {
        let mut arena = Arena::new();
        let ptr = arena.spawn(OpCode::Lam);
        assert!(arena.get(ptr).is_some());
        assert_eq!(arena.get(ptr).unwrap().kind, OpCode::Lam);
        assert_eq!(arena.get(ptr).unwrap().ports.len(), 3);
        assert_eq!(arena.live_count(), 1);
    }

    #[test]
    fn free_and_recycle() {
        let mut arena = Arena::new();
        let p1 = arena.spawn(OpCode::Lam);
        arena.free(p1);
        assert!(arena.get(p1).is_none());
        assert_eq!(arena.live_count(), 0);

        // Recycled slot
        let p2 = arena.spawn(OpCode::App);
        assert_eq!(p1.0, p2.0); // same index reused
        assert_eq!(arena.get(p2).unwrap().kind, OpCode::App);
    }

    #[test]
    fn connect_bidirectional() {
        let mut arena = Arena::new();
        let a = arena.spawn(OpCode::Lam);
        let b = arena.spawn(OpCode::App);

        // Connect aux ports (not principal → no active pair)
        arena.connect(a, 2, b, 1);

        let port_a2 = arena.get(a).unwrap().ports[2];
        assert_eq!(port_a2.target, b);
        assert_eq!(port_a2.slot, 1);

        let port_b1 = arena.get(b).unwrap().ports[1];
        assert_eq!(port_b1.target, a);
        assert_eq!(port_b1.slot, 2);

        assert!(arena.active_pairs.is_empty());
    }

    #[test]
    fn principal_port_auto_schedule() {
        let mut arena = Arena::new();
        let app = arena.spawn(OpCode::App);
        let lam = arena.spawn(OpCode::Lam);

        // Connect principal ports → should auto-enqueue
        arena.connect(app, 0, lam, 0);
        assert_eq!(arena.active_pairs.len(), 1);
        assert_eq!(arena.active_pairs[0], (app, lam));
    }

    #[test]
    fn disconnect() {
        let mut arena = Arena::new();
        let a = arena.spawn(OpCode::Lam);
        let b = arena.spawn(OpCode::App);
        arena.connect(a, 1, b, 2);

        arena.disconnect(a, 1);
        assert!(!arena.get(a).unwrap().ports[1].is_connected());
        assert!(!arena.get(b).unwrap().ports[2].is_connected());
    }

    #[test]
    fn scope_activation() {
        let mut arena = Arena::new();
        let barrier = arena.spawn(OpCode::Barrier { scope: 42 });
        let other = arena.spawn(OpCode::Sym {
            name: "x".into(),
            arity: 0,
        });

        // Wire principal ports
        arena.connect(barrier, 0, other, 0);
        // Clear the auto-enqueued pair (we'll test listener wakeup)
        arena.active_pairs.clear();

        // Register listener
        arena.listeners.entry(42).or_default().push(barrier);

        // Activate scope → should wake up the barrier
        arena.activate_scope(42);
        assert_eq!(arena.active_pairs.len(), 1);
    }
}
