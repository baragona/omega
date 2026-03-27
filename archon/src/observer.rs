//! Topological Observability — the Archon microscope.
//!
//! When a traditional compiler fails, it gives a line number. When Z3 fails,
//! it gives an UNSAT core. When Archon fails, a graph shatters or boils
//! infinitely. The Observer trait hooks into the physics loop to emit
//! structured telemetry for debugging.

use apeiron::node::{OpCode, Ptr};

use crate::extended_arena::ArchonArena;

/// An observation event emitted during physics execution.
#[derive(Debug, Clone)]
pub enum PhysicsEvent {
    /// An interaction was dispatched.
    Interaction {
        step: u64,
        left: Ptr,
        left_kind: String,
        right: Ptr,
        right_kind: String,
        rule_name: String,
        left_region: u32,
        right_region: u32,
        crossed_boundary: bool,
    },
    /// A boundary crossing occurred.
    BoundaryCrossing {
        step: u64,
        node: Ptr,
        from_region: u32,
        to_region: u32,
        boundary_type: String,
        transform_name: String,
    },
    /// Radiation propagated to a new node.
    RadiationSpread {
        step: u64,
        marker_id: u32,
        source: Ptr,
        reached: Ptr,
        region: u32,
    },
    /// Radiation hit a boundary and was blocked or leaked.
    RadiationBoundaryHit {
        step: u64,
        marker_id: u32,
        node: Ptr,
        boundary_region: u32,
        leaked: bool,
    },
    /// Hamiltonian energy snapshot during annealing.
    EnergySnapshot {
        step: u64,
        total_energy: f64,
        violated_constraints: usize,
        temperature: f64,
        region: u32,
    },
    /// A spin was flipped during annealing.
    SpinFlip {
        step: u64,
        spin: Ptr,
        old_polarity: bool,
        new_polarity: bool,
        energy_delta: f64,
        accepted: bool,
    },
    /// A topological scar was deposited (CDCL learned clause).
    ScarDeposited {
        step: u64,
        scar_id: u32,
        literals: Vec<(Ptr, bool)>,
        region: u32,
    },
    /// Physics engine halted.
    Halted {
        step: u64,
        reason: String,
        live_nodes: usize,
        active_pairs_remaining: usize,
    },
    /// A node was created.
    NodeSpawned {
        step: u64,
        node: Ptr,
        kind: String,
        region: u32,
    },
    /// A node was freed (garbage collected).
    NodeFreed {
        step: u64,
        node: Ptr,
        region: u32,
    },
}

/// The Observer trait — implement this to receive structured telemetry
/// from the Archon physics engine.
///
/// All methods have default no-op implementations, so you only need to
/// override what you care about.
pub trait Observer {
    /// Called for every physics event.
    fn observe(&mut self, _event: &PhysicsEvent) {}

    /// Called at the start of physics execution.
    fn on_start(&mut self, _arena: &ArchonArena) {}

    /// Called at the end of physics execution.
    fn on_finish(&mut self, _arena: &ArchonArena) {}
}

/// A null observer that does nothing (default).
pub struct NullObserver;
impl Observer for NullObserver {}

/// A trace observer that collects all events into a Vec.
pub struct TraceObserver {
    pub events: Vec<PhysicsEvent>,
}

impl TraceObserver {
    pub fn new() -> Self {
        TraceObserver { events: Vec::new() }
    }

    /// Get all boundary crossing events.
    pub fn boundary_crossings(&self) -> Vec<&PhysicsEvent> {
        self.events.iter().filter(|e| matches!(e, PhysicsEvent::BoundaryCrossing { .. })).collect()
    }

    /// Get all energy snapshots.
    pub fn energy_snapshots(&self) -> Vec<(u64, f64, f64)> {
        self.events.iter().filter_map(|e| {
            if let PhysicsEvent::EnergySnapshot { step, total_energy, temperature, .. } = e {
                Some((*step, *total_energy, *temperature))
            } else {
                None
            }
        }).collect()
    }

    /// Get all radiation leak events.
    pub fn radiation_leaks(&self) -> Vec<&PhysicsEvent> {
        self.events.iter().filter(|e| {
            matches!(e, PhysicsEvent::RadiationBoundaryHit { leaked: true, .. })
        }).collect()
    }

    /// Get all scar deposits (learned clauses).
    pub fn scars(&self) -> Vec<&PhysicsEvent> {
        self.events.iter().filter(|e| matches!(e, PhysicsEvent::ScarDeposited { .. })).collect()
    }

    /// Count interactions by rule name.
    pub fn rule_histogram(&self) -> std::collections::HashMap<String, u64> {
        let mut hist = std::collections::HashMap::new();
        for e in &self.events {
            if let PhysicsEvent::Interaction { rule_name, .. } = e {
                *hist.entry(rule_name.clone()).or_insert(0) += 1;
            }
        }
        hist
    }

    /// Dump a human-readable summary.
    pub fn summary(&self) -> String {
        let interactions = self.events.iter().filter(|e| matches!(e, PhysicsEvent::Interaction { .. })).count();
        let crossings = self.events.iter().filter(|e| matches!(e, PhysicsEvent::BoundaryCrossing { .. })).count();
        let leaks = self.radiation_leaks().len();
        let scars = self.scars().len();
        let energy = self.energy_snapshots();
        let final_energy = energy.last().map(|(_, e, _)| *e).unwrap_or(0.0);

        format!(
            "Archon Observer Summary:\n  Interactions: {}\n  Boundary crossings: {}\n  Radiation leaks: {}\n  CDCL scars: {}\n  Final energy: {:.6}",
            interactions, crossings, leaks, scars, final_energy
        )
    }
}

impl Observer for TraceObserver {
    fn observe(&mut self, event: &PhysicsEvent) {
        self.events.push(event.clone());
    }
}

/// A stderr-printing observer for live debugging.
pub struct StderrObserver {
    /// Only emit events matching these filters. Empty = emit all.
    pub filters: Vec<EventFilter>,
}

/// Filters for the StderrObserver.
#[derive(Clone, Debug)]
pub enum EventFilter {
    Interactions,
    BoundaryCrossings,
    RadiationLeaks,
    EnergySnapshots,
    SpinFlips,
    Scars,
    All,
}

impl StderrObserver {
    pub fn all() -> Self {
        StderrObserver { filters: vec![EventFilter::All] }
    }

    pub fn boundaries_only() -> Self {
        StderrObserver { filters: vec![EventFilter::BoundaryCrossings, EventFilter::RadiationLeaks] }
    }

    fn should_emit(&self, event: &PhysicsEvent) -> bool {
        if self.filters.is_empty() || self.filters.iter().any(|f| matches!(f, EventFilter::All)) {
            return true;
        }
        for f in &self.filters {
            match (f, event) {
                (EventFilter::Interactions, PhysicsEvent::Interaction { .. }) => return true,
                (EventFilter::BoundaryCrossings, PhysicsEvent::BoundaryCrossing { .. }) => return true,
                (EventFilter::RadiationLeaks, PhysicsEvent::RadiationBoundaryHit { leaked: true, .. }) => return true,
                (EventFilter::EnergySnapshots, PhysicsEvent::EnergySnapshot { .. }) => return true,
                (EventFilter::SpinFlips, PhysicsEvent::SpinFlip { .. }) => return true,
                (EventFilter::Scars, PhysicsEvent::ScarDeposited { .. }) => return true,
                _ => {}
            }
        }
        false
    }
}

impl Observer for StderrObserver {
    fn observe(&mut self, event: &PhysicsEvent) {
        if !self.should_emit(event) {
            return;
        }
        match event {
            PhysicsEvent::Interaction { step, rule_name, left_region, right_region, crossed_boundary, .. } => {
                eprintln!("  [{}] {} (regions {},{}) {}", step, rule_name, left_region, right_region,
                    if *crossed_boundary { "BOUNDARY" } else { "" });
            }
            PhysicsEvent::BoundaryCrossing { step, boundary_type, from_region, to_region, transform_name, .. } => {
                eprintln!("  [{}] CROSSING {} -> {} via {} ({})", step, from_region, to_region, boundary_type, transform_name);
            }
            PhysicsEvent::RadiationBoundaryHit { step, marker_id, leaked, boundary_region, .. } => {
                eprintln!("  [{}] RADIATION marker={} at boundary region={} {}", step, marker_id, boundary_region,
                    if *leaked { "LEAKED!" } else { "blocked" });
            }
            PhysicsEvent::EnergySnapshot { step, total_energy, violated_constraints, temperature, region } => {
                eprintln!("  [{}] ENERGY region={} E={:.4} violations={} T={:.4}", step, region, total_energy, violated_constraints, temperature);
            }
            PhysicsEvent::SpinFlip { step, energy_delta, accepted, .. } => {
                eprintln!("  [{}] SPIN dE={:.4} {}", step, energy_delta, if *accepted { "accepted" } else { "rejected" });
            }
            PhysicsEvent::ScarDeposited { step, scar_id, literals, region } => {
                eprintln!("  [{}] SCAR #{} ({} literals) in region {}", step, scar_id, literals.len(), region);
            }
            PhysicsEvent::Halted { step, reason, live_nodes, active_pairs_remaining } => {
                eprintln!("  [{}] HALTED: {} (live={}, pending={})", step, reason, live_nodes, active_pairs_remaining);
            }
            _ => {}
        }
    }
}

/// Format an OpCode for display.
pub fn format_opcode(kind: &OpCode) -> String {
    match kind {
        OpCode::Lam => "Lam".into(),
        OpCode::App => "App".into(),
        OpCode::Erase => "Erase".into(),
        OpCode::Dup { label } => format!("Dup({})", label),
        OpCode::Sym { name, arity } => format!("{}[{}]", name, arity),
        OpCode::Barrier { scope, .. } => format!("Barrier({})", scope),
        OpCode::Future => "Future".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_observer_collects_events() {
        let mut obs = TraceObserver::new();
        obs.observe(&PhysicsEvent::Interaction {
            step: 0,
            left: Ptr(1),
            left_kind: "App".into(),
            right: Ptr(2),
            right_kind: "Lam".into(),
            rule_name: "Beta".into(),
            left_region: 0,
            right_region: 0,
            crossed_boundary: false,
        });
        assert_eq!(obs.events.len(), 1);
        let hist = obs.rule_histogram();
        assert_eq!(hist.get("Beta"), Some(&1));
    }

    #[test]
    fn null_observer_is_noop() {
        let mut obs = NullObserver;
        obs.observe(&PhysicsEvent::Halted {
            step: 0,
            reason: "test".into(),
            live_nodes: 0,
            active_pairs_remaining: 0,
        });
        // Just verifying it compiles and doesn't panic.
    }
}
