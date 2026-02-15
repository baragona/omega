use std::collections::HashMap;

use crate::arena::Arena;
use crate::node::{OpCode, Ptr};

/// FNV-1a offset basis.
const FNV_OFFSET: u64 = 0xcbf29ce484222325;
/// FNV-1a prime.
const FNV_PRIME: u64 = 0x100000001b3;

fn hash_mix(h: u64, val: u64) -> u64 {
    (h ^ val).wrapping_mul(FNV_PRIME)
}

fn hash_str(s: &str) -> u64 {
    let mut h = FNV_OFFSET;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

/// Canonicalization state for labeled opcodes during hashing.
///
/// Instead of hashing absolute IDs (scope IDs, dup labels, future constraint IDs),
/// we assign each ID a canonical index based on first-encounter order. This
/// implements **Relative Topological Hashing**: two graphs that differ only in
/// the naming of their scopes/labels/futures hash identically, achieving
/// alpha-equivalence for contexts (de Bruijn levels for scopes).
struct CanonicalState {
    scope_map: HashMap<u32, u32>,
    dup_map: HashMap<u32, u32>,
    future_map: HashMap<u32, u32>,
}

impl CanonicalState {
    fn new() -> Self {
        CanonicalState {
            scope_map: HashMap::new(),
            dup_map: HashMap::new(),
            future_map: HashMap::new(),
        }
    }

    fn canonical_scope(&mut self, scope: u32) -> u32 {
        let next = self.scope_map.len() as u32;
        *self.scope_map.entry(scope).or_insert(next)
    }

    fn canonical_dup(&mut self, label: u32) -> u32 {
        let next = self.dup_map.len() as u32;
        *self.dup_map.entry(label).or_insert(next)
    }

    fn canonical_future(&mut self, id: u32) -> u32 {
        let next = self.future_map.len() as u32;
        *self.future_map.entry(id).or_insert(next)
    }
}

fn opcode_hash_canonical(op: &OpCode, state: &mut CanonicalState) -> u64 {
    match op {
        OpCode::Lam => 0x1111111111111111,
        OpCode::App => 0x2222222222222222,
        OpCode::Erase => 0x3333333333333333,
        OpCode::Dup { label } => hash_mix(0x4444444444444444, state.canonical_dup(*label) as u64),
        OpCode::Barrier { scope } => {
            hash_mix(0x5555555555555555, state.canonical_scope(*scope) as u64)
        }
        OpCode::Lens { shift } => hash_mix(0x6666666666666666, *shift as u64),
        OpCode::Future { constraint_id } => {
            hash_mix(0x7777777777777777, state.canonical_future(*constraint_id) as u64)
        }
        OpCode::Sym { name, arity } => {
            let h = hash_str(name);
            hash_mix(h, *arity as u64)
        }
    }
}

/// Compute a topological hash of the subgraph reachable from `root`.
///
/// Two graphs that are structurally identical (up to internal node ordering
/// and label/scope/future naming) produce the same hash. This implements
/// **Relative Topological Hashing**: Barrier scope IDs, Dup labels, and
/// Future constraint IDs are canonicalized via first-encounter ordinals
/// (de Bruijn levels for scopes), achieving alpha-equivalence for contexts.
///
/// Uses a two-pass traversal:
/// 1. Discover all reachable nodes and assign ordinals.
/// 2. Hash each node's opcode (with canonical labels) + port target ordinals.
pub fn topological_hash(arena: &Arena, root: Ptr) -> u64 {
    let mut visited: HashMap<Ptr, u32> = HashMap::new();
    let mut order: Vec<Ptr> = Vec::new();
    let mut queue: Vec<Ptr> = vec![root];
    let mut ordinal: u32 = 0;

    // Pass 1: BFS to discover all nodes
    while let Some(ptr) = queue.pop() {
        if ptr.is_none() || visited.contains_key(&ptr) {
            continue;
        }
        visited.insert(ptr, ordinal);
        order.push(ptr);
        ordinal += 1;

        if let Some(node) = arena.get(ptr) {
            for port in &node.ports {
                if port.is_connected() && !visited.contains_key(&port.target) {
                    queue.push(port.target);
                }
            }
        }
    }

    // Pass 2: hash with canonical labels and all ordinals known
    let mut hash = FNV_OFFSET;
    let mut canonical = CanonicalState::new();
    for &ptr in &order {
        if let Some(node) = arena.get(ptr) {
            hash = hash_mix(hash, opcode_hash_canonical(&node.kind, &mut canonical));
            hash = hash_mix(hash, node.ports.len() as u64);

            for port in &node.ports {
                if port.is_connected() {
                    let target_ord = visited.get(&port.target).copied().unwrap_or(u32::MAX);
                    hash = hash_mix(hash, target_ord as u64);
                    hash = hash_mix(hash, port.slot as u64);
                    hash = hash_mix(hash, port.color as u64);
                } else {
                    hash = hash_mix(hash, u64::MAX);
                }
            }
        }
    }

    hash
}

/// Check structural equality of two subgraphs via topological hashing.
pub fn structurally_equal(arena: &Arena, a: Ptr, b: Ptr) -> bool {
    topological_hash(arena, a) == topological_hash(arena, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::Arena;
    use crate::node::{OpCode, WireColor};

    #[test]
    fn identical_constants_same_hash() {
        let mut arena = Arena::new();
        let a = arena.spawn(OpCode::Sym {
            name: "x".into(),
            arity: 0,
        });
        let b = arena.spawn(OpCode::Sym {
            name: "x".into(),
            arity: 0,
        });
        assert_eq!(topological_hash(&arena, a), topological_hash(&arena, b));
    }

    #[test]
    fn different_constants_different_hash() {
        let mut arena = Arena::new();
        let a = arena.spawn(OpCode::Sym {
            name: "x".into(),
            arity: 0,
        });
        let b = arena.spawn(OpCode::Sym {
            name: "y".into(),
            arity: 0,
        });
        assert_ne!(topological_hash(&arena, a), topological_hash(&arena, b));
    }

    #[test]
    fn structurally_equal_graphs() {
        let mut arena = Arena::new();

        // Build two copies of [lam x x] (identity)
        let lam1 = arena.spawn(OpCode::Lam);
        arena.connect(lam1, 1, lam1, 2, WireColor::Green);

        let lam2 = arena.spawn(OpCode::Lam);
        arena.connect(lam2, 1, lam2, 2, WireColor::Green);

        assert!(structurally_equal(&arena, lam1, lam2));
    }

    #[test]
    fn contextual_alpha_barriers_same_structure() {
        let mut arena = Arena::new();

        // Barrier(scope=42, inner=Lam identity)
        let lam1 = arena.spawn(OpCode::Lam);
        arena.connect(lam1, 1, lam1, 2, WireColor::Green);
        let bar1 = arena.spawn(OpCode::Barrier { scope: 42 });
        arena.connect(bar1, 1, lam1, 0, WireColor::Blue);

        // Barrier(scope=99, inner=Lam identity) — different scope ID, same structure
        let lam2 = arena.spawn(OpCode::Lam);
        arena.connect(lam2, 1, lam2, 2, WireColor::Green);
        let bar2 = arena.spawn(OpCode::Barrier { scope: 99 });
        arena.connect(bar2, 1, lam2, 0, WireColor::Blue);

        // Relative hashing: scope IDs are canonicalized → structurally equal
        assert!(structurally_equal(&arena, bar1, bar2));
    }

    #[test]
    fn contextual_alpha_dup_labels() {
        let mut arena = Arena::new();

        // Dup#0 with two Sym("x") children
        let x1a = arena.spawn(OpCode::Sym { name: "x".into(), arity: 0 });
        let x1b = arena.spawn(OpCode::Sym { name: "x".into(), arity: 0 });
        let dup1 = arena.spawn(OpCode::Dup { label: 0 });
        arena.connect(dup1, 1, x1a, 0, WireColor::Blue);
        arena.connect(dup1, 2, x1b, 0, WireColor::Blue);

        // Dup#7 with two Sym("x") children — different label, same structure
        let x2a = arena.spawn(OpCode::Sym { name: "x".into(), arity: 0 });
        let x2b = arena.spawn(OpCode::Sym { name: "x".into(), arity: 0 });
        let dup2 = arena.spawn(OpCode::Dup { label: 7 });
        arena.connect(dup2, 1, x2a, 0, WireColor::Blue);
        arena.connect(dup2, 2, x2b, 0, WireColor::Blue);

        assert!(structurally_equal(&arena, dup1, dup2));
    }

    #[test]
    fn shared_scope_preserved() {
        let mut arena = Arena::new();

        // Two barriers sharing scope A, inner=Sym("x")
        let x1 = arena.spawn(OpCode::Sym { name: "x".into(), arity: 0 });
        let bar1a = arena.spawn(OpCode::Barrier { scope: 10 });
        arena.connect(bar1a, 1, x1, 0, WireColor::Blue);
        let x2 = arena.spawn(OpCode::Sym { name: "x".into(), arity: 0 });
        let bar1b = arena.spawn(OpCode::Barrier { scope: 10 }); // same scope
        arena.connect(bar1b, 1, x2, 0, WireColor::Blue);
        let pair1 = arena.spawn(OpCode::Sym { name: "pair".into(), arity: 2 });
        arena.connect(pair1, 1, bar1a, 0, WireColor::Blue);
        arena.connect(pair1, 2, bar1b, 0, WireColor::Blue);

        // Two barriers with DIFFERENT scopes — structurally different!
        let x3 = arena.spawn(OpCode::Sym { name: "x".into(), arity: 0 });
        let bar2a = arena.spawn(OpCode::Barrier { scope: 20 });
        arena.connect(bar2a, 1, x3, 0, WireColor::Blue);
        let x4 = arena.spawn(OpCode::Sym { name: "x".into(), arity: 0 });
        let bar2b = arena.spawn(OpCode::Barrier { scope: 30 }); // different scope
        arena.connect(bar2b, 1, x4, 0, WireColor::Blue);
        let pair2 = arena.spawn(OpCode::Sym { name: "pair".into(), arity: 2 });
        arena.connect(pair2, 1, bar2a, 0, WireColor::Blue);
        arena.connect(pair2, 2, bar2b, 0, WireColor::Blue);

        // Same-scope pair vs different-scope pair → NOT equal
        assert!(!structurally_equal(&arena, pair1, pair2));
    }

    #[test]
    fn contextual_alpha_futures() {
        let mut arena = Arena::new();

        // Two Future nodes with different constraint IDs
        let f1 = arena.spawn(OpCode::Future { constraint_id: 0 });
        let f2 = arena.spawn(OpCode::Future { constraint_id: 42 });

        // Single futures are structurally identical (both canonicalize to index 0)
        assert!(structurally_equal(&arena, f1, f2));
    }
}
