//! Anti-matter annihilation — Dialectica witness extraction.
//!
//! The Dialectica membrane flips quantifier polarity:
//! ∀ (matter) ↔ ∃ (anti-matter).
//!
//! When a flipped ∃ (now demanding a witness) collides with an axiom
//! that provides one, they annihilate — the witness term is the residual.
//!
//! The A-translation (preprocessing membrane) first normalizes classical
//! proofs to the negative fragment, then the anti-matter boundary extracts
//! computational witnesses.

use apeiron::node::{OpCode, Ptr};

use crate::extended_arena::ArchonArena;

/// Polarity of a quantifier node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Polarity {
    /// Positive: ∀ (universal, matter).
    Positive,
    /// Negative: ∃ (existential, anti-matter).
    Negative,
}

/// Check if a node is a quantifier and return its polarity.
pub fn quantifier_polarity(arena: &ArchonArena, ptr: Ptr) -> Option<Polarity> {
    let node = arena.get(ptr)?;
    match &node.kind {
        OpCode::Sym { name, .. } if name == "forall" => Some(Polarity::Positive),
        OpCode::Sym { name, .. } if name == "exists" => Some(Polarity::Negative),
        _ => None,
    }
}

/// Result of an annihilation event.
#[derive(Debug)]
pub enum AnnihilationResult {
    /// Matter met anti-matter: both consumed, witness extracted.
    Annihilated { witness: Ptr },
    /// Polarity flip performed (preprocessing).
    Flipped,
    /// No annihilation occurred.
    NoReaction,
}

/// Attempt annihilation: when ∀ meets ∃ on the same predicate,
/// they annihilate and produce a witness term.
///
/// ∀x.P(x) collides with ∃x.¬P(x) → witness is the x that
/// distinguishes them.
pub fn try_annihilate(
    arena: &mut ArchonArena,
    matter: Ptr,
    antimatter: Ptr,
) -> AnnihilationResult {
    let m_pol = quantifier_polarity(arena, matter);
    let a_pol = quantifier_polarity(arena, antimatter);

    match (m_pol, a_pol) {
        (Some(Polarity::Positive), Some(Polarity::Negative))
        | (Some(Polarity::Negative), Some(Polarity::Positive)) => {
            // Annihilation! Extract the witness from the existential's body.
            let (forall_node, exists_node) = if m_pol == Some(Polarity::Positive) {
                (matter, antimatter)
            } else {
                (antimatter, matter)
            };

            let region = arena.region_of(exists_node);

            // The existential's body (port 1) contains the witness.
            let witness_port = arena.port(exists_node, 1);
            let forall_body = arena.port(forall_node, 1);

            // Create a witness extraction node.
            let witness = arena.spawn_in(
                OpCode::Sym {
                    name: "__witness".into(),
                    arity: 2,
                },
                region,
            );

            // The witness captures both the existential's body and
            // the universal's body for verification.
            if witness_port.is_connected() {
                arena.connect(witness, 1, witness_port.target, witness_port.slot);
            }
            if forall_body.is_connected() {
                arena.connect(witness, 2, forall_body.target, forall_body.slot);
            }

            // Connect the witness to wherever the forall's principal was going.
            let forall_principal = arena.port(forall_node, 0);
            if forall_principal.is_connected() {
                arena.connect(witness, 0, forall_principal.target, forall_principal.slot);
            }

            // Both quantifiers are consumed.
            arena.free(forall_node);
            arena.free(exists_node);

            AnnihilationResult::Annihilated { witness }
        }
        _ => AnnihilationResult::NoReaction,
    }
}

/// Perform the A-translation: transform a classical proof into
/// the negative fragment (suitable for Dialectica extraction).
///
/// This is a preprocessing step that adds double-negation translations
/// to classical axioms (LEM, DNE, etc.).
pub fn a_translate(
    arena: &mut ArchonArena,
    node: Ptr,
) -> Option<Ptr> {
    let kind = arena.get(node)?.kind.clone();
    let region = arena.region_of(node);

    match &kind {
        // LEM: P ∨ ¬P → ¬¬(P ∨ ¬P) (double-negate)
        OpCode::Sym { name, arity } if name == "lem" || name == "dne" => {
            let double_neg = arena.spawn_in(
                OpCode::Sym {
                    name: "__not_not".into(),
                    arity: *arity,
                },
                region,
            );

            // Rewire: double_neg wraps the original.
            let node_port = arena.port(node, 0);
            if node_port.is_connected() {
                arena.connect(double_neg, 0, node_port.target, node_port.slot);
            }
            arena.connect(double_neg, 1, node, 0);

            Some(double_neg)
        }
        _ => None, // Already in negative fragment.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polarity_detection() {
        let mut arena = ArchonArena::new();

        let forall = arena.spawn(OpCode::Sym {
            name: "forall".into(),
            arity: 1,
        });
        let exists = arena.spawn(OpCode::Sym {
            name: "exists".into(),
            arity: 1,
        });
        let other = arena.spawn(OpCode::Lam);

        assert_eq!(quantifier_polarity(&arena, forall), Some(Polarity::Positive));
        assert_eq!(quantifier_polarity(&arena, exists), Some(Polarity::Negative));
        assert_eq!(quantifier_polarity(&arena, other), None);
    }

    #[test]
    fn annihilation_produces_witness() {
        let mut arena = ArchonArena::new();

        let forall = arena.spawn(OpCode::Sym {
            name: "forall".into(),
            arity: 1,
        });
        let exists = arena.spawn(OpCode::Sym {
            name: "exists".into(),
            arity: 1,
        });
        let forall_body = arena.spawn(OpCode::Sym {
            name: "P".into(),
            arity: 0,
        });
        let exists_body = arena.spawn(OpCode::Sym {
            name: "witness_term".into(),
            arity: 0,
        });
        let root = arena.spawn(OpCode::Sym {
            name: "root".into(),
            arity: 1,
        });

        arena.connect(forall, 1, forall_body, 0);
        arena.connect(exists, 1, exists_body, 0);
        arena.connect(forall, 0, root, 1);

        let result = try_annihilate(&mut arena, forall, exists);
        assert!(matches!(result, AnnihilationResult::Annihilated { .. }));

        // Both quantifiers should be freed.
        assert!(arena.get(forall).is_none());
        assert!(arena.get(exists).is_none());
    }
}
