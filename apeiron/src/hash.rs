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

fn opcode_hash(op: &OpCode) -> u64 {
    match op {
        OpCode::Lam => 0x1111111111111111,
        OpCode::App => 0x2222222222222222,
        OpCode::Erase => 0x3333333333333333,
        OpCode::Dup { label } => hash_mix(0x4444444444444444, *label as u64),
        OpCode::Barrier { scope } => hash_mix(0x5555555555555555, *scope as u64),
        OpCode::Lens { shift } => hash_mix(0x6666666666666666, *shift as u64),
        OpCode::Future { constraint_id } => hash_mix(0x7777777777777777, *constraint_id as u64),
        OpCode::Sym { name, arity } => {
            let h = hash_str(name);
            hash_mix(h, *arity as u64)
        }
    }
}

/// Compute a topological hash of the subgraph reachable from `root`.
///
/// Two graphs that are structurally identical (up to internal node ordering)
/// produce the same hash. Uses a two-pass BFS:
/// 1. Discover all reachable nodes and assign ordinals.
/// 2. Hash each node's opcode + port target ordinals.
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

    // Pass 2: hash with all ordinals known
    let mut hash = FNV_OFFSET;
    for &ptr in &order {
        if let Some(node) = arena.get(ptr) {
            hash = hash_mix(hash, opcode_hash(&node.kind));
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
}
