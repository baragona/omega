//! The Archon physics loop — extends Apeiron's physics with
//! region-aware dispatch, boundary crossing, radiation propagation,
//! modal extrusion, catalyst wavefronts, and thermodynamic annealing.
//!
//! The main loop:
//! 1. Pop an active pair
//! 2. If same region: delegate to Apeiron's standard interaction rules
//! 3. If different regions: dispatch to boundary physics
//! 4. After each batch: propagate radiation one hop
//! 5. If thermodynamic regions exist: run one annealing step

use apeiron::node::{OpCode, Ptr};

use crate::boundary;
use crate::crystallize;
use crate::extended_arena::ArchonArena;
use crate::kripke;
use crate::observer::{self, Observer, NullObserver, PhysicsEvent};
use crate::radiation;
use crate::region::Direction;
use crate::superposition;

/// Configuration for the Archon physics engine.
pub struct ArchonConfig {
    /// Maximum interactions before halting.
    pub max_interactions: u64,
    /// Whether to collect a trace.
    pub trace: bool,
    /// How many radiation propagation hops per physics tick.
    pub radiation_hops_per_tick: u32,
}

impl Default for ArchonConfig {
    fn default() -> Self {
        ArchonConfig {
            max_interactions: 100_000,
            trace: false,
            radiation_hops_per_tick: 1,
        }
    }
}

/// Why the Archon engine halted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HaltReason {
    NormalForm,
    FuelExhausted,
    Error(String),
}

/// A trace record.
#[derive(Debug, Clone)]
pub struct TraceRecord {
    pub rule_name: String,
    pub crossed_boundary: bool,
}

/// Result of running the Archon physics engine.
pub struct ArchonResult {
    pub interactions: u64,
    pub boundary_crossings: u64,
    pub radiation_hops: u32,
    pub trace: Vec<TraceRecord>,
    pub halted_reason: HaltReason,
}

/// Run the Archon physics engine (convenience wrapper with no observer).
pub fn run(arena: &mut ArchonArena, config: &ArchonConfig) -> ArchonResult {
    run_with_observer(arena, config, &mut NullObserver)
}

/// Run the Archon physics engine with a pluggable observer for telemetry.
pub fn run_with_observer(
    arena: &mut ArchonArena,
    config: &ArchonConfig,
    obs: &mut dyn Observer,
) -> ArchonResult {
    let mut result = ArchonResult {
        interactions: 0,
        boundary_crossings: 0,
        radiation_hops: 0,
        trace: Vec::new(),
        halted_reason: HaltReason::NormalForm,
    };

    obs.on_start(arena);

    // Pre-propagate radiation to fixpoint before physics starts.
    result.radiation_hops = radiation::propagate_to_fixpoint(arena, 100);

    while let Some((left, right)) = arena.inner.active_pairs.pop() {
        if result.interactions >= config.max_interactions {
            arena.inner.active_pairs.push((left, right));
            result.halted_reason = HaltReason::FuelExhausted;
            obs.observe(&PhysicsEvent::Halted {
                step: result.interactions,
                reason: "FuelExhausted".into(),
                live_nodes: arena.node_count(),
                active_pairs_remaining: arena.inner.active_pairs.len() + 1,
            });
            break;
        }

        // Both nodes must still be alive.
        let (left_kind, right_kind) = match (arena.get(left), arena.get(right)) {
            (Some(l), Some(r)) => (l.kind.clone(), r.kind.clone()),
            _ => continue,
        };

        if config.trace {
            eprintln!(
                "  ARCHON PAIR: {:?}({}) × {:?}({}) [regions {}, {}]",
                left_kind, left.0, right_kind, right.0,
                arena.region_of(left), arena.region_of(right),
            );
        }

        let left_region = arena.region_of(left);
        let right_region = arena.region_of(right);

        let (rule_name, crossed) = if arena.same_region(left, right) {
            let name = dispatch_same_region(arena, left, &left_kind, right, &right_kind);
            (name, false)
        } else {
            let br = boundary::dispatch(arena, left, &left_kind, right, &right_kind);
            match br {
                boundary::BoundaryResult::Handled(name) => {
                    result.boundary_crossings += 1;
                    obs.observe(&PhysicsEvent::BoundaryCrossing {
                        step: result.interactions,
                        node: left,
                        from_region: left_region,
                        to_region: right_region,
                        boundary_type: format!("{:?}",
                            arena.topology.boundary_between(left_region, right_region)
                                .cloned()
                                .unwrap_or(crate::region::BoundaryType::Transparent)),
                        transform_name: name.clone(),
                    });
                    (name, true)
                }
                boundary::BoundaryResult::PassThrough => {
                    let name = dispatch_standard(arena, left, &left_kind, right, &right_kind);
                    (name, false)
                }
                boundary::BoundaryResult::Rejected(msg) => {
                    result.halted_reason = HaltReason::Error(msg.clone());
                    obs.observe(&PhysicsEvent::Halted {
                        step: result.interactions,
                        reason: format!("Rejected: {}", msg),
                        live_nodes: arena.node_count(),
                        active_pairs_remaining: arena.inner.active_pairs.len(),
                    });
                    break;
                }
            }
        };

        result.interactions += 1;

        // Emit interaction event to observer.
        obs.observe(&PhysicsEvent::Interaction {
            step: result.interactions,
            left,
            left_kind: observer::format_opcode(&left_kind),
            right,
            right_kind: observer::format_opcode(&right_kind),
            rule_name: rule_name.clone(),
            left_region,
            right_region,
            crossed_boundary: crossed,
        });

        if config.trace {
            let record = TraceRecord {
                rule_name: rule_name.clone(),
                crossed_boundary: crossed,
            };
            eprintln!("  -> {} {}", rule_name, if crossed { "(boundary)" } else { "" });
            result.trace.push(record);
        }

        // Periodic radiation propagation.
        if result.interactions % 10 == 0 {
            let hops = radiation::propagate_to_fixpoint(arena, config.radiation_hops_per_tick);
            result.radiation_hops += hops;
            // Propagate congruence closure (upward wave from superpositions).
            superposition::propagate_congruence(arena);
        }
    }

    // Emit final halt event.
    if result.halted_reason == HaltReason::NormalForm {
        obs.observe(&PhysicsEvent::Halted {
            step: result.interactions,
            reason: "NormalForm".into(),
            live_nodes: arena.node_count(),
            active_pairs_remaining: 0,
        });
    }

    obs.on_finish(arena);
    result
}

/// Dispatch within the same region — handles Archon-specific interactions
/// (catalysts, modals, resource violations) before falling through to standard.
fn dispatch_same_region(
    arena: &mut ArchonArena,
    left: Ptr,
    left_kind: &OpCode,
    right: Ptr,
    right_kind: &OpCode,
) -> String {
    let region_id = arena.region_of(left);
    let direction = arena
        .topology
        .get(region_id)
        .map(|r| r.direction.clone())
        .unwrap_or(Direction::Forward);

    // Check for catalyst interactions (CPS wavefront).
    if crystallize::is_catalyst(arena, left) {
        match right_kind {
            OpCode::App => {
                crystallize::catalyst_meets_app(arena, left, right);
                return "Catalyst-App".into();
            }
            OpCode::Lam => {
                crystallize::catalyst_meets_lam(arena, left, right);
                return "Catalyst-Lam".into();
            }
            OpCode::Sym { name, arity } if *arity == 0 && !name.starts_with("__") => {
                crystallize::catalyst_meets_value(arena, left, right);
                return "Catalyst-Value".into();
            }
            // Non-zero arity Sym: treat as a value (constructor applied to args).
            OpCode::Sym { name, .. } if !name.starts_with("__") => {
                crystallize::catalyst_meets_value(arena, left, right);
                return "Catalyst-Value".into();
            }
            _ => {}
        }
    }
    if crystallize::is_catalyst(arena, right) {
        match left_kind {
            OpCode::App => {
                crystallize::catalyst_meets_app(arena, right, left);
                return "Catalyst-App".into();
            }
            OpCode::Lam => {
                crystallize::catalyst_meets_lam(arena, right, left);
                return "Catalyst-Lam".into();
            }
            OpCode::Sym { name, arity } if *arity == 0 && !name.starts_with("__") => {
                crystallize::catalyst_meets_value(arena, right, left);
                return "Catalyst-Value".into();
            }
            OpCode::Sym { name, .. } if !name.starts_with("__") => {
                crystallize::catalyst_meets_value(arena, right, left);
                return "Catalyst-Value".into();
            }
            _ => {}
        }
    }

    // Check for superposition interactions.
    if let Some(rule) = superposition::dispatch_super(arena, left, left_kind, right, right_kind) {
        return rule;
    }

    // Check for modal interactions.
    if let Some(modal_kind) = kripke::is_modal(arena, left) {
        match modal_kind {
            kripke::ModalKind::Box => {
                kripke::box_extrude(arena, left, region_id);
                return "Box-Extrude".into();
            }
            kripke::ModalKind::Diamond => {
                kripke::diamond_search(arena, left, region_id);
                return "Diamond-Search".into();
            }
        }
    }
    if let Some(modal_kind) = kripke::is_modal(arena, right) {
        match modal_kind {
            kripke::ModalKind::Box => {
                kripke::box_extrude(arena, right, region_id);
                return "Box-Extrude".into();
            }
            kripke::ModalKind::Diamond => {
                kripke::diamond_search(arena, right, region_id);
                return "Diamond-Search".into();
            }
        }
    }

    // Check resource mode violations.
    match (left_kind, right_kind) {
        (OpCode::Dup { .. }, _) | (_, OpCode::Dup { .. }) => {
            let dup_node = if matches!(left_kind, OpCode::Dup { .. }) { left } else { right };
            let target = if dup_node == left { right } else { left };
            if !arena.dup_allowed(target) {
                // In a strictly linear region, Dup is forbidden.
                // Instead of duplicating, this is a type error / resource violation.
                // For now, we erase the dup and keep the original.
                arena.free(dup_node);
                return "Linear-Dup-Rejected".into();
            }
        }
        (OpCode::Erase, _) | (_, OpCode::Erase) => {
            let target = if matches!(left_kind, OpCode::Erase) { right } else { left };
            if !arena.erase_allowed(target) {
                // In a relevant region, erasure is forbidden.
                let eraser = if matches!(left_kind, OpCode::Erase) { left } else { right };
                arena.free(eraser);
                return "Relevant-Erase-Rejected".into();
            }
        }
        _ => {}
    }

    // Backward direction: in GoalDirected regions, reverse interaction.
    if direction == Direction::Backward {
        return dispatch_backward(arena, left, left_kind, right, right_kind);
    }

    // Fall through to standard Apeiron physics.
    dispatch_standard(arena, left, left_kind, right, right_kind)
}

/// Standard Apeiron dispatch (delegates to apeiron::interact).
fn dispatch_standard(
    arena: &mut ArchonArena,
    left: Ptr,
    left_kind: &OpCode,
    right: Ptr,
    right_kind: &OpCode,
) -> String {
    use apeiron::interact;
    use OpCode::*;

    match (left_kind, right_kind) {
        (App, Lam) => {
            interact::beta(&mut arena.inner, left, right);
            "Beta".into()
        }
        (Lam, App) => {
            interact::beta(&mut arena.inner, right, left);
            "Beta".into()
        }
        (Erase, _) => {
            interact::erase_node(&mut arena.inner, left, right);
            "Erase".into()
        }
        (_, Erase) => {
            interact::erase_node(&mut arena.inner, right, left);
            "Erase".into()
        }
        (Dup { label: l1 }, Dup { label: l2 }) => {
            if l1 == l2 {
                interact::dup_dup_annihilate(&mut arena.inner, left, right);
                "Dup-Annihilate".into()
            } else {
                interact::dup_dup_commute(&mut arena.inner, left, right);
                "Dup-Commute".into()
            }
        }
        (Dup { .. }, Lam) => {
            interact::dup_lam(&mut arena.inner, left, right);
            "Dup-Lam".into()
        }
        (Lam, Dup { .. }) => {
            interact::dup_lam(&mut arena.inner, right, left);
            "Dup-Lam".into()
        }
        (Dup { .. }, App) => {
            interact::dup_app(&mut arena.inner, left, right);
            "Dup-App".into()
        }
        (App, Dup { .. }) => {
            interact::dup_app(&mut arena.inner, right, left);
            "Dup-App".into()
        }
        (Dup { .. }, Sym { .. }) => {
            interact::dup_sym(&mut arena.inner, left, right);
            "Dup-Sym".into()
        }
        (Sym { .. }, Dup { .. }) => {
            interact::dup_sym(&mut arena.inner, right, left);
            "Dup-Sym".into()
        }
        (Barrier { .. }, _) => {
            interact::barrier_check(&mut arena.inner, left, right);
            "Barrier".into()
        }
        (_, Barrier { .. }) => {
            interact::barrier_check(&mut arena.inner, right, left);
            "Barrier".into()
        }
        (Future, _) | (_, Future) => "Future-Suspend".into(),
        _ => "Inert".into(),
    }
}

/// Backward-direction dispatch for GoalDirected regions.
/// Instead of reducing, we expand: a goal node spawns its premises.
///
/// When a goal (any Sym) meets a rule node (__rule with arity N: port 1 = conclusion,
/// ports 2..N = premises), the rule fires backward: the conclusion unifies with the
/// goal, and the premises become new active goals.
///
/// When two non-rule nodes meet, the pair is inert (no reduction in backward mode).
fn dispatch_backward(
    arena: &mut ArchonArena,
    left: Ptr,
    left_kind: &OpCode,
    right: Ptr,
    right_kind: &OpCode,
) -> String {
    let is_rule = |kind: &OpCode| matches!(kind, OpCode::Sym { name, .. } if name == "__rule");

    let (rule, goal) = if is_rule(left_kind) {
        (left, right)
    } else if is_rule(right_kind) {
        (right, left)
    } else {
        // Two non-rule nodes: inert in backward mode.
        return "Backward-Inert".into();
    };

    let region = arena.region_of(rule);

    // Rule structure: port 0 = principal, port 1 = conclusion, ports 2.. = premises.
    let rule_arity = match arena.get(rule).map(|n| n.kind.clone()) {
        Some(OpCode::Sym { arity, .. }) => arity,
        _ => return "Backward-Error".into(),
    };

    // Connect the conclusion (port 1) to the goal — this "unifies" them
    // by physically wiring them together. If they're Sym nodes with the same
    // opcode, Apeiron's standard dispatch will annihilate them.
    let conclusion_port = arena.port(rule, 1);
    if conclusion_port.is_connected() {
        let conclusion = conclusion_port.target;
        // Wire conclusion to the goal's principal port.
        let goal_port = arena.port(goal, 0);
        if goal_port.is_connected() {
            arena.connect(conclusion, 0, goal_port.target, goal_port.slot);
        }
    }

    // Each premise (ports 2..arity) becomes a new active goal.
    // Spawn a fresh goal-marker for each and push as active pairs
    // so they'll be picked up in subsequent physics steps.
    for slot in 2..=rule_arity {
        let premise_port = arena.port(rule, slot);
        if premise_port.is_connected() {
            let premise = premise_port.target;
            // Create a demand node that will seek a matching rule.
            let demand = arena.spawn_in(
                OpCode::Sym {
                    name: "__goal_demand".into(),
                    arity: 1,
                },
                region,
            );
            arena.connect(demand, 1, premise, 0);
            // The demand's principal port is free — it will form active pairs
            // with any rule node whose conclusion matches.
        }
    }

    // Free the rule node (it has been "consumed" by backward expansion).
    arena.free(rule);

    "Backward-Expand".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::*;

    #[test]
    fn standard_beta_in_same_region() {
        let mut arena = ArchonArena::new();

        let lam = arena.spawn(OpCode::Lam);
        let app = arena.spawn(OpCode::App);
        let y = arena.spawn(OpCode::Sym { name: "y".into(), arity: 0 });
        let root = arena.spawn(OpCode::Sym { name: "root".into(), arity: 1 });

        // Identity lambda: var ↔ body.
        arena.connect(lam, 1, lam, 2);
        arena.connect(app, 1, y, 0);
        arena.connect(app, 2, root, 1);
        arena.connect(app, 0, lam, 0);

        let result = run(&mut arena, &ArchonConfig::default());
        assert_eq!(result.halted_reason, HaltReason::NormalForm);
        assert_eq!(result.interactions, 1);

        // root.1 should be connected to y.
        let root_child = arena.port(root, 1);
        assert!(root_child.is_connected());
        assert_eq!(
            arena.get(root_child.target).unwrap().kind,
            OpCode::Sym { name: "y".into(), arity: 0 }
        );
    }

    #[test]
    fn boundary_crossing_counted() {
        let mut topo = Topology::new();
        let linear = topo.add_region(
            Region::new(0, "linear")
                .with_resource(ResourceMode::StrictlyLinear)
                .with_boundary(BoundaryType::BangBoundary)
                .with_parent(0),
        );

        let mut arena = ArchonArena::new().with_topology(topo);

        let lin_node = arena.spawn_in(
            OpCode::Sym { name: "x".into(), arity: 0 },
            linear,
        );
        let share_node = arena.spawn_in(
            OpCode::Sym { name: "consumer".into(), arity: 1 },
            0,
        );
        arena.connect(lin_node, 0, share_node, 0);

        let result = run(&mut arena, &ArchonConfig::default());
        assert!(result.boundary_crossings > 0);
    }

    #[test]
    fn linear_region_rejects_dup() {
        let mut topo = Topology::new();
        let linear = topo.add_region(
            Region::new(0, "linear")
                .with_resource(ResourceMode::StrictlyLinear)
                .with_parent(0),
        );

        let mut arena = ArchonArena::new().with_topology(topo);

        let node = arena.spawn_in(
            OpCode::Sym { name: "x".into(), arity: 0 },
            linear,
        );
        let dup = arena.spawn_in(
            OpCode::Dup { label: 0 },
            linear,
        );
        let a = arena.spawn_in(
            OpCode::Sym { name: "a".into(), arity: 0 },
            linear,
        );
        let b = arena.spawn_in(
            OpCode::Sym { name: "b".into(), arity: 0 },
            linear,
        );

        arena.connect(dup, 1, a, 0);
        arena.connect(dup, 2, b, 0);
        arena.connect(dup, 0, node, 0);

        let result = run(&mut arena, &ArchonConfig::default());
        assert_eq!(result.halted_reason, HaltReason::NormalForm);

        // The dup should have been rejected (freed), not duplicated.
        assert!(arena.get(dup).is_none());
    }
}
