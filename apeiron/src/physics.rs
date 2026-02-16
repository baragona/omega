use crate::arena::Arena;
use crate::interact;
use crate::node::OpCode;

/// Configuration for the physics engine.
pub struct PhysicsConfig {
    /// Maximum interactions before halting (fuel).
    pub max_interactions: u64,
    /// Whether to collect a step-by-step trace.
    pub trace: bool,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        PhysicsConfig {
            max_interactions: 100_000,
            trace: false,
        }
    }
}

/// Why the physics engine halted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HaltReason {
    /// No active pairs remain — normal form reached.
    NormalForm,
    /// Fuel exhausted.
    FuelExhausted,
    /// Error during interaction.
    Error(String),
}

/// A single interaction step record (for tracing/debugging).
#[derive(Debug, Clone)]
pub struct InteractionRecord {
    pub rule_name: String,
}

/// Result of running the physics engine.
pub struct PhysicsResult {
    pub interactions: u64,
    pub trace: Vec<InteractionRecord>,
    pub halted_reason: HaltReason,
}

/// Run the physics engine: consume active pairs until empty or fuel runs out.
pub fn run(arena: &mut Arena, config: &PhysicsConfig) -> PhysicsResult {
    let mut result = PhysicsResult {
        interactions: 0,
        trace: Vec::new(),
        halted_reason: HaltReason::NormalForm,
    };

    while let Some((left, right)) = arena.active_pairs.pop() {
        if result.interactions >= config.max_interactions {
            arena.active_pairs.push((left, right));
            result.halted_reason = HaltReason::FuelExhausted;
            break;
        }

        // Both nodes must still be alive
        let (left_kind, right_kind) = match (arena.get(left), arena.get(right)) {
            (Some(l), Some(r)) => (l.kind.clone(), r.kind.clone()),
            _ => continue, // one was freed by a previous interaction
        };

        if config.trace {
            eprintln!(
                "  PAIR: {:?}({}) × {:?}({})",
                left_kind, left.0, right_kind, right.0
            );
        }

        let rule_name = match dispatch(arena, left, &left_kind, right, &right_kind) {
            Ok(name) => name,
            Err(e) => {
                result.halted_reason = HaltReason::Error(format!("{}", e));
                break;
            }
        };

        result.interactions += 1;
        arena.stats.interactions += 1;

        if config.trace {
            eprintln!("  -> {}", rule_name);
            result.trace.push(InteractionRecord {
                rule_name: rule_name.clone(),
            });
        }
    }

    result
}

/// Dispatch an active pair to the appropriate interaction handler.
/// The pair is symmetric — we normalize the order inside each match arm.
fn dispatch(
    arena: &mut Arena,
    left: crate::node::Ptr,
    left_kind: &OpCode,
    right: crate::node::Ptr,
    right_kind: &OpCode,
) -> crate::error::Result<String> {
    use OpCode::*;

    match (left_kind, right_kind) {
        // Beta: App × Lam
        (App, Lam) => {
            interact::beta(arena, left, right);
            Ok("Beta".into())
        }
        (Lam, App) => {
            interact::beta(arena, right, left);
            Ok("Beta".into())
        }

        // Erase × anything
        (Erase, _) => {
            interact::erase_node(arena, left, right);
            Ok("Erase".into())
        }
        (_, Erase) => {
            interact::erase_node(arena, right, left);
            Ok("Erase".into())
        }

        // Dup × Dup (must check before Dup × others)
        (Dup { label: l1 }, Dup { label: l2 }) => {
            if l1 == l2 {
                interact::dup_dup_annihilate(arena, left, right);
                Ok("Dup-Annihilate".into())
            } else {
                interact::dup_dup_commute(arena, left, right);
                Ok("Dup-Commute".into())
            }
        }

        // Dup × Lam
        (Dup { .. }, Lam) => {
            interact::dup_lam(arena, left, right);
            Ok("Dup-Lam".into())
        }
        (Lam, Dup { .. }) => {
            interact::dup_lam(arena, right, left);
            Ok("Dup-Lam".into())
        }

        // Dup × App
        (Dup { .. }, App) => {
            interact::dup_app(arena, left, right);
            Ok("Dup-App".into())
        }
        (App, Dup { .. }) => {
            interact::dup_app(arena, right, left);
            Ok("Dup-App".into())
        }

        // Dup × Sym
        (Dup { .. }, Sym { .. }) => {
            interact::dup_sym(arena, left, right);
            Ok("Dup-Sym".into())
        }
        (Sym { .. }, Dup { .. }) => {
            interact::dup_sym(arena, right, left);
            Ok("Dup-Sym".into())
        }

        // Barrier × anything
        (Barrier { .. }, _) => {
            interact::barrier_check(arena, left, right);
            Ok("Barrier".into())
        }
        (_, Barrier { .. }) => {
            interact::barrier_check(arena, right, left);
            Ok("Barrier".into())
        }

        // Future × anything → suspend (do nothing, don't re-enqueue)
        (Future, _) | (_, Future) => Ok("Future-Suspend".into()),

        // Inert pair — skip silently
        _ => Ok("Inert".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::Arena;
    use crate::node::OpCode;

    #[test]
    fn empty_arena_halts_immediately() {
        let mut arena = Arena::new();
        let result = run(&mut arena, &PhysicsConfig::default());
        assert_eq!(result.halted_reason, HaltReason::NormalForm);
        assert_eq!(result.interactions, 0);
    }

    #[test]
    fn fuel_exhaustion() {
        let mut arena = Arena::new();
        // Create a self-duplicating pair that won't terminate
        // Just test that fuel works by setting it to 0
        let app = arena.spawn(OpCode::App);
        let lam = arena.spawn(OpCode::Lam);
        arena.connect(app, 0, lam, 0);

        let config = PhysicsConfig {
            max_interactions: 0,
            trace: false,
        };
        let result = run(&mut arena, &config);
        assert_eq!(result.halted_reason, HaltReason::FuelExhausted);
    }
}
