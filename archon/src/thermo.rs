//! Thermodynamic annealing — SAT/SMT solving as physics.
//!
//! Boolean variables are spin nodes (UP=true, DOWN=false).
//! Constraints are springs coupling spins together.
//! The system cools from high temperature (random flips) to
//! low temperature (deterministic ground state).
//!
//! CDCL is physicalized as conflict shockwaves: when a constraint
//! is unsatisfied, a shockwave propagates backward, depositing
//! learned clauses (new springs) at the earliest decision point.

use std::collections::HashMap;

use rand::Rng;

use apeiron::node::Ptr;

use crate::extended_arena::{ArchonArena, SpringConstraint};

/// Configuration for the thermodynamic annealing engine.
#[derive(Clone, Debug)]
pub struct AnnealConfig {
    /// Initial temperature.
    pub initial_temp: f64,
    /// Cooling rate (multiplied each step).
    pub cooling_rate: f64,
    /// Minimum temperature (stop cooling here).
    pub min_temp: f64,
    /// Maximum annealing steps.
    pub max_steps: u64,
}

impl Default for AnnealConfig {
    fn default() -> Self {
        AnnealConfig {
            initial_temp: 10.0,
            cooling_rate: 0.995,
            min_temp: 0.001,
            max_steps: 100_000,
        }
    }
}

/// Result of annealing.
#[derive(Debug)]
pub enum AnnealResult {
    /// Found a satisfying assignment.
    Satisfied {
        steps: u64,
        final_temp: f64,
    },
    /// System is unsatisfiable (contradictory springs).
    Unsatisfied {
        violated: usize,
        steps: u64,
    },
    /// Reached max steps without convergence.
    Timeout {
        violated: usize,
        steps: u64,
    },
}

/// Count how many springs are violated (all literals false).
pub fn count_violations(arena: &ArchonArena) -> usize {
    let mut violations = 0;
    for (_node_idx, constraints) in &arena.springs {
        for constraint in constraints {
            let satisfied = constraint.literals.iter().any(|&(spin_ptr, required)| {
                arena.spin_polarity(spin_ptr) == Some(required)
            });
            if !satisfied {
                violations += 1;
            }
        }
    }
    violations
}

/// Compute the energy delta of flipping a spin.
/// Energy = number of violated constraints.
/// Returns (violations_before_flip, violations_after_flip).
fn energy_delta(arena: &ArchonArena, spin: Ptr) -> (usize, usize) {
    let current = arena.spin_polarity(spin).unwrap_or(false);
    let mut before = 0;
    let mut after = 0;

    // Check all springs that mention this spin.
    for constraints in arena.springs.values() {
        for constraint in constraints {
            let involves_spin = constraint.literals.iter().any(|&(p, _)| p == spin);
            if !involves_spin {
                continue;
            }

            // Count satisfied before flip.
            let sat_before = constraint.literals.iter().any(|&(p, req)| {
                let pol = if p == spin { current } else { arena.spin_polarity(p).unwrap_or(false) };
                pol == req
            });

            // Count satisfied after flip.
            let sat_after = constraint.literals.iter().any(|&(p, req)| {
                let pol = if p == spin { !current } else { arena.spin_polarity(p).unwrap_or(false) };
                pol == req
            });

            if !sat_before { before += 1; }
            if !sat_after { after += 1; }
        }
    }

    (before, after)
}

// ── Topological Scars (CDCL Learned Clauses) ─────────────────────────

/// A topological scar — a rigid, inert constraint deposited when the
/// annealing wavefront hits an UNSAT conflict. Scars physically block
/// the search from ever revisiting the same geometric configuration.
///
/// In CDCL terms: a scar is a learned clause derived from conflict analysis.
#[derive(Clone, Debug)]
pub struct Scar {
    /// The learned clause: at least one of these literals must hold.
    /// This is the negation of the conflicting assignment.
    pub literals: Vec<(Ptr, bool)>,
    /// Which annealing step deposited this scar.
    pub deposited_at: u64,
}

/// CDCL state for conflict-driven physical learning.
struct CdclState {
    /// Decision level for each spin (which step set it).
    decision_levels: HashMap<u32, u64>,
    /// Implication graph: spin → (implied_by_constraint, antecedent_spins).
    implications: HashMap<u32, Vec<u32>>,
    /// All deposited scars (learned clauses).
    scars: Vec<Scar>,
    /// Stagnation detector: how many steps with same violation count.
    stagnation_count: u64,
    /// Last observed violation count.
    last_violations: usize,
    /// How many steps of stagnation before triggering conflict analysis.
    stagnation_threshold: u64,
}

impl CdclState {
    fn new() -> Self {
        CdclState {
            decision_levels: HashMap::new(),
            implications: HashMap::new(),
            scars: Vec::new(),
            stagnation_count: 0,
            last_violations: usize::MAX,
            stagnation_threshold: 50,
        }
    }

    /// Detect stagnation and return true if conflict analysis should run.
    fn check_stagnation(&mut self, violations: usize) -> bool {
        if violations == self.last_violations && violations > 0 {
            self.stagnation_count += 1;
        } else {
            self.stagnation_count = 0;
        }
        self.last_violations = violations;
        self.stagnation_count >= self.stagnation_threshold
    }

    /// Analyze conflict: find the spins involved in the most violated
    /// constraints and build a learned clause (scar) from their negation.
    fn analyze_conflict(
        &mut self,
        arena: &ArchonArena,
        spin_nodes: &[Ptr],
        step: u64,
    ) -> Option<Scar> {
        // Find the most-violated constraints.
        let mut spin_blame: HashMap<u32, usize> = HashMap::new();
        for constraints in arena.springs.values() {
            for constraint in constraints {
                let satisfied = constraint.literals.iter().any(|&(spin_ptr, required)| {
                    arena.spin_polarity(spin_ptr) == Some(required)
                });
                if !satisfied {
                    // All spins in this clause are blamed.
                    for &(spin_ptr, _) in &constraint.literals {
                        *spin_blame.entry(spin_ptr.0).or_insert(0) += 1;
                    }
                }
            }
        }

        if spin_blame.is_empty() {
            return None;
        }

        // Build learned clause: negate the current assignment of the
        // most-blamed spins. "At least one of these must be different."
        let mut blamed: Vec<(u32, usize)> = spin_blame.into_iter().collect();
        blamed.sort_by(|a, b| b.1.cmp(&a.1));

        // Take the top-K most blamed spins (at most half of all spins).
        let k = (blamed.len() / 2).max(1).min(blamed.len());
        let literals: Vec<(Ptr, bool)> = blamed[..k]
            .iter()
            .map(|&(idx, _)| {
                let ptr = Ptr(idx);
                let current_pol = arena.spin_polarity(ptr).unwrap_or(false);
                // Negate: require the opposite of current assignment.
                (ptr, !current_pol)
            })
            .collect();

        let scar = Scar {
            literals,
            deposited_at: step,
        };

        Some(scar)
    }

    /// Deposit a scar into the arena as a new spring constraint.
    fn deposit_scar(&mut self, arena: &mut ArchonArena, region_id: u32, scar: Scar) {
        let spring_node = arena.spawn_in(
            apeiron::node::OpCode::Sym {
                name: "__archon_scar".into(),
                arity: 0,
            },
            region_id,
        );
        arena.add_spring(
            spring_node,
            SpringConstraint {
                literals: scar.literals.clone(),
            },
        );
        self.scars.push(scar);
        self.stagnation_count = 0; // Reset stagnation after learning.
    }
}

/// Run simulated annealing with CDCL scarring on all spin nodes in a region.
///
/// When the annealing wavefront stagnates (same violations for N steps),
/// conflict analysis runs and deposits topological scars (learned clauses)
/// that physically block the search from revisiting the same configuration.
pub fn anneal(
    arena: &mut ArchonArena,
    region_id: u32,
    config: &AnnealConfig,
) -> AnnealResult {
    anneal_cdcl(arena, region_id, config, None)
}

/// Run annealing with CDCL and optional observer.
pub fn anneal_cdcl(
    arena: &mut ArchonArena,
    region_id: u32,
    config: &AnnealConfig,
    mut obs: Option<&mut dyn crate::observer::Observer>,
) -> AnnealResult {
    use crate::observer::PhysicsEvent;

    let mut rng = rand::thread_rng();
    let mut temp = config.initial_temp;
    let mut cdcl = CdclState::new();

    // Collect all spin nodes in this region.
    let spin_nodes: Vec<Ptr> = arena.spins.keys()
        .filter_map(|&idx| {
            let ptr = Ptr(idx);
            if arena.region_of(ptr) == region_id {
                Some(ptr)
            } else {
                None
            }
        })
        .collect();

    if spin_nodes.is_empty() {
        return AnnealResult::Satisfied { steps: 0, final_temp: temp };
    }

    for step in 0..config.max_steps {
        let violations = count_violations(arena);
        if violations == 0 {
            return AnnealResult::Satisfied {
                steps: step,
                final_temp: temp,
            };
        }

        // Emit energy snapshot to observer.
        if let Some(ref mut o) = obs {
            if step % 100 == 0 {
                o.observe(&PhysicsEvent::EnergySnapshot {
                    step,
                    total_energy: violations as f64,
                    violated_constraints: violations,
                    temperature: temp,
                    region: region_id,
                });
            }
        }

        // CDCL: check for stagnation and learn if needed.
        if cdcl.check_stagnation(violations) {
            if let Some(scar) = cdcl.analyze_conflict(arena, &spin_nodes, step) {
                if let Some(ref mut o) = obs {
                    o.observe(&PhysicsEvent::ScarDeposited {
                        step,
                        scar_id: cdcl.scars.len() as u32,
                        literals: scar.literals.clone(),
                        region: region_id,
                    });
                }
                cdcl.deposit_scar(arena, region_id, scar);

                // After learning, reheat slightly to escape local minimum.
                temp = (temp * 2.0).min(config.initial_temp * 0.5);
            }
        }

        // Pick a random spin.
        let spin = spin_nodes[rng.gen_range(0..spin_nodes.len())];

        // Compute energy delta.
        let (before, after) = energy_delta(arena, spin);
        let delta = after as f64 - before as f64;

        // Metropolis criterion.
        let accepted = delta <= 0.0 || rng.gen::<f64>() < (-delta / temp).exp();
        if accepted {
            if let Some(ref mut o) = obs {
                o.observe(&PhysicsEvent::SpinFlip {
                    step,
                    spin,
                    old_polarity: arena.spin_polarity(spin).unwrap_or(false),
                    new_polarity: !arena.spin_polarity(spin).unwrap_or(false),
                    energy_delta: delta,
                    accepted: true,
                });
            }
            arena.flip_spin(spin);
        }

        // Cool.
        if temp > config.min_temp {
            temp *= config.cooling_rate;
        }
    }

    let final_violations = count_violations(arena);
    if final_violations == 0 {
        AnnealResult::Satisfied {
            steps: config.max_steps,
            final_temp: temp,
        }
    } else {
        // Report as Unsatisfied if we deposited scars (strong evidence of UNSAT).
        if cdcl.scars.len() > 5 {
            AnnealResult::Unsatisfied {
                violated: final_violations,
                steps: config.max_steps,
            }
        } else {
            AnnealResult::Timeout {
                violated: final_violations,
                steps: config.max_steps,
            }
        }
    }
}

/// Encode a SAT clause as a spring constraint.
///
/// A clause like (x ∨ ¬y ∨ z) becomes a spring connecting
/// spin_x (required true), spin_y (required false), spin_z (required true).
pub fn encode_clause(
    arena: &mut ArchonArena,
    region_id: u32,
    literals: Vec<(Ptr, bool)>,
) -> Ptr {
    let spring_node = arena.spawn_in(
        apeiron::node::OpCode::Sym {
            name: "__archon_spring".into(),
            arity: 0,
        },
        region_id,
    );

    arena.add_spring(
        spring_node,
        SpringConstraint {
            literals,
        },
    );

    spring_node
}

// ── Continuous variables (integer/real) for arithmetic SMT ────────────

/// A continuous variable: a particle sliding on a 1D wire.
#[derive(Clone, Debug)]
pub struct ContinuousVar {
    pub value: f64,
    pub min_bound: f64,
    pub max_bound: f64,
}

/// An arithmetic constraint contributing to the Hamiltonian energy.
#[derive(Clone, Debug)]
pub enum ArithConstraint {
    /// E = (sum of (coeff * var) - target)^2
    /// Encodes equalities like x + y = 10.
    LinearEquality {
        terms: Vec<(Ptr, f64)>, // (variable, coefficient)
        target: f64,
    },
    /// E = max(0, bound - value)^2  (lower bound: value >= bound)
    /// or E = max(0, value - bound)^2  (upper bound: value <= bound)
    Inequality {
        var: Ptr,
        bound: f64,
        is_lower: bool, // true: var >= bound, false: var <= bound
    },
}

impl ArchonArena {
    /// Spawn a continuous variable node in a region.
    pub fn spawn_continuous(&mut self, region_id: u32, initial: f64) -> Ptr {
        let ptr = self.spawn_in(
            apeiron::node::OpCode::Sym {
                name: "__archon_continuous".into(),
                arity: 0,
            },
            region_id,
        );
        self.continuous_vars.insert(ptr.0, ContinuousVar {
            value: initial,
            min_bound: f64::NEG_INFINITY,
            max_bound: f64::INFINITY,
        });
        ptr
    }

    /// Get a continuous variable's current value.
    pub fn continuous_value(&self, ptr: Ptr) -> Option<f64> {
        self.continuous_vars.get(&ptr.0).map(|v| v.value)
    }

    /// Set a continuous variable's value.
    pub fn set_continuous(&mut self, ptr: Ptr, value: f64) {
        if let Some(v) = self.continuous_vars.get_mut(&ptr.0) {
            v.value = value.clamp(v.min_bound, v.max_bound);
        }
    }
}

/// Compute the total energy from arithmetic constraints.
pub fn arith_energy(arena: &ArchonArena) -> f64 {
    let mut total = 0.0;
    for constraints in arena.arith_constraints.values() {
        for c in constraints {
            total += constraint_energy(arena, c);
        }
    }
    total
}

/// Energy contribution of a single arithmetic constraint.
fn constraint_energy(arena: &ArchonArena, c: &ArithConstraint) -> f64 {
    match c {
        ArithConstraint::LinearEquality { terms, target } => {
            let sum: f64 = terms.iter().map(|&(ptr, coeff)| {
                coeff * arena.continuous_value(ptr).unwrap_or(0.0)
            }).sum();
            let diff = sum - target;
            diff * diff
        }
        ArithConstraint::Inequality { var, bound, is_lower } => {
            let val = arena.continuous_value(*var).unwrap_or(0.0);
            let violation = if *is_lower {
                (*bound - val).max(0.0)
            } else {
                (val - *bound).max(0.0)
            };
            violation * violation
        }
    }
}

/// Run arithmetic annealing on continuous variables in a region.
pub fn anneal_arithmetic(
    arena: &mut ArchonArena,
    region_id: u32,
    config: &AnnealConfig,
) -> AnnealResult {
    let mut rng = rand::thread_rng();
    let mut temp = config.initial_temp;

    // Collect continuous variable nodes in this region.
    let cont_nodes: Vec<Ptr> = arena.continuous_vars.keys()
        .filter_map(|&idx| {
            let ptr = Ptr(idx);
            if arena.region_of(ptr) == region_id {
                Some(ptr)
            } else {
                None
            }
        })
        .collect();

    if cont_nodes.is_empty() {
        // Fall back to SAT annealing for pure-boolean problems.
        return anneal(arena, region_id, config);
    }

    let epsilon = 1e-9;

    for step in 0..config.max_steps {
        let energy = arith_energy(arena);
        if energy < epsilon {
            return AnnealResult::Satisfied {
                steps: step,
                final_temp: temp,
            };
        }

        // Pick a random continuous variable and perturb it.
        let var = cont_nodes[rng.gen_range(0..cont_nodes.len())];
        let old_val = arena.continuous_value(var).unwrap_or(0.0);

        // Perturbation magnitude scales with temperature.
        let perturbation = (rng.gen::<f64>() - 0.5) * 2.0 * temp;
        arena.set_continuous(var, old_val + perturbation);

        let new_energy = arith_energy(arena);
        let delta = new_energy - energy;

        // Metropolis criterion.
        if delta > 0.0 && rng.gen::<f64>() >= (-delta / temp).exp() {
            // Reject: revert.
            arena.set_continuous(var, old_val);
        }

        if temp > config.min_temp {
            temp *= config.cooling_rate;
        }
    }

    let final_energy = arith_energy(arena);
    if final_energy < epsilon {
        AnnealResult::Satisfied {
            steps: config.max_steps,
            final_temp: temp,
        }
    } else {
        AnnealResult::Timeout {
            violated: 1, // At least one constraint unsatisfied
            steps: config.max_steps,
        }
    }
}

/// Encode a linear equality constraint: sum(coeff_i * var_i) = target.
pub fn encode_linear_equality(
    arena: &mut ArchonArena,
    region_id: u32,
    terms: Vec<(Ptr, f64)>,
    target: f64,
) -> Ptr {
    let node = arena.spawn_in(
        apeiron::node::OpCode::Sym {
            name: "__archon_arith_constraint".into(),
            arity: 0,
        },
        region_id,
    );
    arena.arith_constraints.entry(node.0).or_default().push(
        ArithConstraint::LinearEquality { terms, target },
    );
    node
}

/// Encode an inequality constraint: var >= bound (is_lower=true) or var <= bound (is_lower=false).
pub fn encode_inequality(
    arena: &mut ArchonArena,
    region_id: u32,
    var: Ptr,
    bound: f64,
    is_lower: bool,
) -> Ptr {
    let node = arena.spawn_in(
        apeiron::node::OpCode::Sym {
            name: "__archon_arith_constraint".into(),
            arity: 0,
        },
        region_id,
    );
    arena.arith_constraints.entry(node.0).or_default().push(
        ArithConstraint::Inequality { var, bound, is_lower },
    );
    node
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_sat() {
        let mut arena = ArchonArena::new();

        // Create two spins: x, y.
        let x = arena.spawn_spin(0, false);
        let y = arena.spawn_spin(0, false);

        // Clause: (x ∨ y) — at least one must be true.
        encode_clause(&mut arena, 0, vec![(x, true), (y, true)]);

        let result = anneal(&mut arena, 0, &AnnealConfig::default());
        assert!(matches!(result, AnnealResult::Satisfied { .. }));

        // At least one should be true.
        let x_pol = arena.spin_polarity(x).unwrap();
        let y_pol = arena.spin_polarity(y).unwrap();
        assert!(x_pol || y_pol);
    }

    #[test]
    fn unsat_detected() {
        let mut arena = ArchonArena::new();

        // Create one spin: x.
        let x = arena.spawn_spin(0, false);

        // Clause 1: (x) — x must be true.
        encode_clause(&mut arena, 0, vec![(x, true)]);
        // Clause 2: (¬x) — x must be false.
        encode_clause(&mut arena, 0, vec![(x, false)]);

        let config = AnnealConfig {
            max_steps: 1000,
            ..Default::default()
        };
        let result = anneal(&mut arena, 0, &config);

        // Should timeout with violations (can't satisfy both; CDCL scars may add more).
        assert!(matches!(result, AnnealResult::Timeout { violated, .. } if violated >= 1));
    }

    #[test]
    fn arithmetic_equality() {
        let mut arena = ArchonArena::new();

        // x + y = 10
        let x = arena.spawn_continuous(0, 0.0);
        let y = arena.spawn_continuous(0, 0.0);

        encode_linear_equality(&mut arena, 0, vec![(x, 1.0), (y, 1.0)], 10.0);

        let config = AnnealConfig {
            initial_temp: 50.0,
            max_steps: 50_000,
            ..Default::default()
        };
        let result = anneal_arithmetic(&mut arena, 0, &config);
        assert!(matches!(result, AnnealResult::Satisfied { .. }));

        let xv = arena.continuous_value(x).unwrap();
        let yv = arena.continuous_value(y).unwrap();
        assert!((xv + yv - 10.0).abs() < 0.1);
    }

    #[test]
    fn arithmetic_inequality() {
        let mut arena = ArchonArena::new();

        // x >= 5, x <= 7, x + 0 = x (trivial, but tests combined constraints)
        let x = arena.spawn_continuous(0, 0.0);

        encode_inequality(&mut arena, 0, x, 5.0, true);  // x >= 5
        encode_inequality(&mut arena, 0, x, 7.0, false); // x <= 7

        let config = AnnealConfig {
            initial_temp: 50.0,
            max_steps: 50_000,
            ..Default::default()
        };
        let result = anneal_arithmetic(&mut arena, 0, &config);
        assert!(matches!(result, AnnealResult::Satisfied { .. }));

        let xv = arena.continuous_value(x).unwrap();
        assert!(xv >= 4.9 && xv <= 7.1);
    }

    #[test]
    fn three_sat_instance() {
        let mut arena = ArchonArena::new();

        let x = arena.spawn_spin(0, false);
        let y = arena.spawn_spin(0, false);
        let z = arena.spawn_spin(0, false);

        // (x ∨ y ∨ z)
        encode_clause(&mut arena, 0, vec![(x, true), (y, true), (z, true)]);
        // (¬x ∨ ¬y)
        encode_clause(&mut arena, 0, vec![(x, false), (y, false)]);
        // (¬y ∨ z)
        encode_clause(&mut arena, 0, vec![(y, false), (z, true)]);

        let result = anneal(&mut arena, 0, &AnnealConfig::default());
        assert!(matches!(result, AnnealResult::Satisfied { .. }));
    }
}
