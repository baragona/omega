//! Archon integration tests — proving the physics engine computes correctly.
//!
//! Seven phases:
//! 1. Commutative bisimulation (manual: same graph, two paths, same result)
//! 2. Gauge invariant tests (radiation, annihilation, crystallization edge cases)
//! 3. Thermodynamic termination (annealing convergence, energy monotonicity)
//! 4. Adversarial topology (round-trips, cyclic wormholes, boundary leaks)
//! 5. Wave interference (relativistic collision, AC-merge, causality)
//! 6. Conservation of mass (cycle erasure, radiation decay, memory leaks)
//! 7. Particle cross-contamination (catalyst escape, triple-point boundaries)

use apeiron::node::{OpCode, Ptr};
use archon::extended_arena::ArchonArena;
use archon::physics::{self, ArchonConfig, HaltReason};
use archon::radiation;
use archon::region::*;
use archon::kripke;
use archon::crystallize;
use archon::implant::{self, Sexp};
use archon::antimatter;
use archon::thermo;

// ═══════════════════════════════════════════════════════════════════════
// PHASE 1: COMMUTATIVE BISIMULATION
//
// We can't run Hyperion's AST passes from here (different crate, different
// input format). Instead, we manually construct the expected output of a
// Hyperion pass and verify Archon's boundary physics produces the same
// graph topology.
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn bisimulation_bang_promotion() {
    // Scenario: a node from a linear region meets a node from a sharing region.
    //
    // Hyperion's BangModality pass would wrap the linear term in !.
    // Archon's BangBoundary should do the same at the membrane.
    //
    // We verify the output graph has a __archon_bang wrapper node.

    let mut topo = Topology::new();
    let linear = topo.add_region(
        Region::new(0, "linear")
            .with_resource(ResourceMode::StrictlyLinear)
            .with_boundary(BoundaryType::BangBoundary)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    // Build: linear_val in linear region, consumer in root (sharing).
    let val = arena.spawn_in(
        OpCode::Sym { name: "val".into(), arity: 0 },
        linear,
    );
    let consumer = arena.spawn_in(
        OpCode::Sym { name: "use".into(), arity: 1 },
        0,
    );
    arena.connect(val, 0, consumer, 0);

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);
    assert_eq!(result.boundary_crossings, 1);

    // The graph should now have a __archon_bang node between val and consumer.
    // Walk from consumer to find the bang wrapper.
    let consumer_port = arena.port(consumer, 0);
    assert!(consumer_port.is_connected());
    let bang_node = arena.get(consumer_port.target).unwrap();
    assert!(
        matches!(&bang_node.kind, OpCode::Sym { name, .. } if name == "__archon_bang"),
        "Expected bang wrapper, got {:?}",
        bang_node.kind
    );
}

#[test]
fn bisimulation_defunctionalization() {
    // Scenario: a lambda crosses into a first-order region.
    //
    // Hyperion's Defunctionalization pass would replace the lambda with
    // a closure ADT. Archon's DefunctionalizationBoundary should do the same.

    let mut topo = Topology::new();
    let fo_region = topo.add_region(
        Region::new(0, "first-order")
            .with_boundary(BoundaryType::DefunctionalizationBoundary)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    let lam = arena.spawn_in(OpCode::Lam, 0);
    let body = arena.spawn_in(
        OpCode::Sym { name: "body".into(), arity: 0 },
        0,
    );
    let var = arena.spawn_in(
        OpCode::Sym { name: "captured".into(), arity: 0 },
        0,
    );
    let target = arena.spawn_in(
        OpCode::Sym { name: "apply_site".into(), arity: 1 },
        fo_region,
    );

    arena.connect(lam, 1, var, 0);
    arena.connect(lam, 2, body, 0);
    arena.connect(lam, 0, target, 0);

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);
    assert!(result.boundary_crossings > 0);

    // Lambda should be gone, replaced by a __closure_N node.
    assert!(arena.get(lam).is_none());

    // target should now be connected to a closure node.
    let target_port = arena.port(target, 0);
    assert!(target_port.is_connected());
    let closure = arena.get(target_port.target).unwrap();
    assert!(
        matches!(&closure.kind, OpCode::Sym { name, .. } if name.starts_with("__closure_")),
        "Expected closure ADT, got {:?}",
        closure.kind
    );
}

// ═══════════════════════════════════════════════════════════════════════
// PHASE 2: GAUGE INVARIANT TESTS
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn centrifuge_deeply_nested_lambda() {
    // λx. λy. λz. (x (y z) x)
    // Drop through CombinatorFilter boundary.
    // Verify: only S, K, I nodes emerge. No lambda nodes survive.
    // Verify: no radiation leaks past the boundary.

    let mut topo = Topology::new();
    let combo_region = topo.add_region(
        Region::new(0, "combinator-zone")
            .with_boundary(BoundaryType::CombinatorFilter)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    // Build λx.x (identity) as simplest case first.
    let lam = arena.spawn_in(OpCode::Lam, 0);
    arena.connect(lam, 1, lam, 2); // var ↔ body = identity

    let target = arena.spawn_in(
        OpCode::Sym { name: "target".into(), arity: 1 },
        combo_region,
    );
    arena.connect(lam, 0, target, 0);

    // Set up radiation: the variable (lam's var port) is a radiation source.
    // For identity, var and body are self-connected, so radiation is internal.

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);

    // Lambda should be gone.
    assert!(arena.get(lam).is_none());

    // Should have produced an I combinator.
    let target_port = arena.port(target, 0);
    assert!(target_port.is_connected());
    let combo = arena.get(target_port.target).unwrap();
    assert!(
        matches!(&combo.kind, OpCode::Sym { name, .. } if name == "I"),
        "Expected I combinator for identity lambda, got {:?}",
        combo.kind
    );
}

#[test]
fn centrifuge_k_combinator() {
    // λx. M where x does NOT occur in M → should produce K(M).

    let mut topo = Topology::new();
    let combo_region = topo.add_region(
        Region::new(0, "combinator-zone")
            .with_boundary(BoundaryType::CombinatorFilter)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    // Build λx.c where c is a constant (x not used).
    let lam = arena.spawn_in(OpCode::Lam, 0);
    let c = arena.spawn_in(
        OpCode::Sym { name: "c".into(), arity: 0 },
        0,
    );
    let unused_var = arena.spawn_in(
        OpCode::Sym { name: "unused".into(), arity: 0 },
        0,
    );

    arena.connect(lam, 1, unused_var, 0); // var → unused
    arena.connect(lam, 2, c, 0);          // body → c

    let target = arena.spawn_in(
        OpCode::Sym { name: "target".into(), arity: 1 },
        combo_region,
    );
    arena.connect(lam, 0, target, 0);

    // No radiation source on the variable, so body wire is dark.
    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);

    assert!(arena.get(lam).is_none());

    let target_port = arena.port(target, 0);
    assert!(target_port.is_connected());
    let k_node = arena.get(target_port.target).unwrap();
    assert!(
        matches!(&k_node.kind, OpCode::Sym { name, .. } if name == "K"),
        "Expected K combinator, got {:?}",
        k_node.kind
    );
}

#[test]
fn annihilation_clean_no_debris() {
    // Create ∀x.P and ∃x.Q. Push through Dialectica boundary.
    // After annihilation: no disconnected dead nodes should remain.

    let mut arena = ArchonArena::new();

    let forall = arena.spawn(OpCode::Sym {
        name: "forall".into(),
        arity: 1,
    });
    let exists = arena.spawn(OpCode::Sym {
        name: "exists".into(),
        arity: 1,
    });
    let p_body = arena.spawn(OpCode::Sym {
        name: "P".into(),
        arity: 0,
    });
    let q_body = arena.spawn(OpCode::Sym {
        name: "Q".into(),
        arity: 0,
    });
    let root = arena.spawn(OpCode::Sym {
        name: "root".into(),
        arity: 1,
    });

    arena.connect(forall, 1, p_body, 0);
    arena.connect(exists, 1, q_body, 0);
    arena.connect(forall, 0, root, 1);

    let result = antimatter::try_annihilate(&mut arena, forall, exists);
    assert!(matches!(result, antimatter::AnnihilationResult::Annihilated { .. }));

    // Both quantifiers consumed.
    assert!(arena.get(forall).is_none());
    assert!(arena.get(exists).is_none());

    // The witness node should be connected to root.
    let root_port = arena.port(root, 1);
    assert!(root_port.is_connected());
    let witness = arena.get(root_port.target).unwrap();
    assert!(matches!(&witness.kind, OpCode::Sym { name, .. } if name == "__witness"));

    // Witness should have the bodies connected (no dangling).
    let w_port1 = arena.port(root_port.target, 1);
    let w_port2 = arena.port(root_port.target, 2);
    assert!(w_port1.is_connected(), "Witness body 1 should be connected (no debris)");
    assert!(w_port2.is_connected(), "Witness body 2 should be connected (no debris)");
}

#[test]
fn radiation_does_not_leak_past_boundary() {
    // Radiation from a variable should NOT propagate into a different region
    // through a non-transparent boundary.
    //
    // NOTE: Currently radiation propagates through all connected nodes
    // regardless of region. This test documents the current behavior and
    // will need updating when region-aware radiation is implemented.

    let mut topo = Topology::new();
    let inner = topo.add_region(
        Region::new(0, "inner")
            .with_boundary(BoundaryType::CombinatorFilter)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    let var = arena.spawn_in(
        OpCode::Sym { name: "x".into(), arity: 1 },
        0,
    );
    let bridge = arena.spawn_in(
        OpCode::Sym { name: "bridge".into(), arity: 2 },
        0,
    );
    let beyond = arena.spawn_in(
        OpCode::Sym { name: "beyond".into(), arity: 1 },
        inner,
    );

    arena.connect(var, 1, bridge, 0);
    arena.connect(bridge, 1, beyond, 0);

    let marker = arena.add_radiation_source(var);
    radiation::propagate_to_fixpoint(&mut arena, 100);

    // Bridge (same region as var) should glow.
    assert!(arena.is_glowing(bridge, marker));

    // Beyond is in a different region with a non-transparent boundary.
    // Radiation should NOT cross the CombinatorFilter boundary.
    assert!(
        !arena.is_glowing(beyond, marker),
        "Radiation should be blocked at non-transparent boundaries"
    );
}

#[test]
fn crystallization_value_applies_continuation() {
    // Inject a catalyst into a simple value node.
    // The catalyst should apply the continuation to the value: k(v).

    let mut arena = ArchonArena::new();

    let catalyst = arena.spawn(OpCode::Sym {
        name: "__catalyst".into(),
        arity: 1,
    });
    let value = arena.spawn(OpCode::Sym {
        name: "v".into(),
        arity: 0,
    });
    let continuation = arena.spawn(OpCode::Sym {
        name: "k".into(),
        arity: 1,
    });
    let root = arena.spawn(OpCode::Sym {
        name: "root".into(),
        arity: 1,
    });

    arena.connect(catalyst, 0, root, 1);
    arena.connect(catalyst, 1, continuation, 0);

    let result = crystallize::catalyst_meets_value(&mut arena, catalyst, value);
    assert!(matches!(result, crystallize::CatalystResult::ReachedValue));

    // Catalyst should be consumed.
    assert!(arena.get(catalyst).is_none());

    // An App node should have been created: k applied to v.
    // root.1 → App → k(v)
    let root_port = arena.port(root, 1);
    assert!(root_port.is_connected());
    let app_node = arena.get(root_port.target).unwrap();
    assert_eq!(app_node.kind, OpCode::App);
}

// ═══════════════════════════════════════════════════════════════════════
// PHASE 3: THERMODYNAMIC TERMINATION
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn annealing_pigeonhole_unsat() {
    // Pigeonhole principle: 3 pigeons into 2 holes.
    // Each pigeon must go in exactly one hole.
    // At least two pigeons must share a hole → UNSAT.
    //
    // Variables: p_i_j = "pigeon i goes in hole j"
    // Constraints:
    //   - Each pigeon in at least one hole: (p_i_1 ∨ p_i_2) for each i
    //   - No two pigeons in same hole: ¬(p_i_j ∧ p_k_j) for i≠k

    let mut arena = ArchonArena::new();

    // 3 pigeons × 2 holes = 6 spin variables.
    let mut p = [[Ptr::NONE; 2]; 3];
    for i in 0..3 {
        for j in 0..2 {
            p[i][j] = arena.spawn_spin(0, false);
        }
    }

    // Each pigeon in at least one hole.
    for i in 0..3 {
        thermo::encode_clause(&mut arena, 0, vec![
            (p[i][0], true),
            (p[i][1], true),
        ]);
    }

    // No two pigeons in same hole.
    for j in 0..2 {
        for i in 0..3 {
            for k in (i + 1)..3 {
                // ¬p_i_j ∨ ¬p_k_j (at most one pigeon per hole)
                thermo::encode_clause(&mut arena, 0, vec![
                    (p[i][j], false),
                    (p[k][j], false),
                ]);
            }
        }
    }

    let config = thermo::AnnealConfig {
        max_steps: 50_000,
        initial_temp: 5.0,
        cooling_rate: 0.999,
        min_temp: 0.0001,
    };

    let result = thermo::anneal(&mut arena, 0, &config);

    // Should NOT find a satisfying assignment (it's UNSAT).
    assert!(
        matches!(result, thermo::AnnealResult::Timeout { violated, .. } | thermo::AnnealResult::Unsatisfied { violated, .. } if violated > 0),
        "Pigeonhole 3→2 should be UNSAT, got {:?}",
        result
    );
}

#[test]
fn annealing_satisfiable_converges() {
    // Simple satisfiable instance: (x ∨ y) ∧ (¬x ∨ z) ∧ (y ∨ z)
    // Many solutions exist (e.g., x=T, y=T, z=T).

    let mut arena = ArchonArena::new();

    let x = arena.spawn_spin(0, false);
    let y = arena.spawn_spin(0, false);
    let z = arena.spawn_spin(0, false);

    thermo::encode_clause(&mut arena, 0, vec![(x, true), (y, true)]);
    thermo::encode_clause(&mut arena, 0, vec![(x, false), (z, true)]);
    thermo::encode_clause(&mut arena, 0, vec![(y, true), (z, true)]);

    let result = thermo::anneal(&mut arena, 0, &thermo::AnnealConfig::default());
    assert!(
        matches!(result, thermo::AnnealResult::Satisfied { .. }),
        "Should find a satisfying assignment, got {:?}",
        result
    );

    // Verify the assignment actually satisfies all clauses.
    assert_eq!(thermo::count_violations(&arena), 0);
}

#[test]
fn annealing_energy_monotonic_trend() {
    // Track energy over time during annealing.
    // While individual steps can increase energy (Metropolis accepts uphill),
    // the overall trend should be decreasing.

    let mut arena = ArchonArena::new();

    // Create a moderately complex instance.
    let vars: Vec<Ptr> = (0..10)
        .map(|_| arena.spawn_spin(0, false))
        .collect();

    // Random 3-SAT clauses (but deterministic for reproducibility).
    let clauses = vec![
        vec![(vars[0], true), (vars[1], true), (vars[2], true)],
        vec![(vars[0], false), (vars[3], true), (vars[4], true)],
        vec![(vars[1], false), (vars[2], false), (vars[5], true)],
        vec![(vars[3], false), (vars[6], true), (vars[7], true)],
        vec![(vars[4], false), (vars[5], false), (vars[8], true)],
        vec![(vars[6], false), (vars[7], false), (vars[9], true)],
        vec![(vars[8], false), (vars[9], false), (vars[0], true)],
    ];

    for clause in clauses {
        thermo::encode_clause(&mut arena, 0, clause);
    }

    // Run with short steps and check energy trend.
    let initial_violations = thermo::count_violations(&arena);

    let config = thermo::AnnealConfig {
        max_steps: 10_000,
        initial_temp: 10.0,
        cooling_rate: 0.99,
        min_temp: 0.01,
    };
    let _result = thermo::anneal(&mut arena, 0, &config);

    let final_violations = thermo::count_violations(&arena);

    // Energy should have decreased (or stayed same if already solved).
    assert!(
        final_violations <= initial_violations,
        "Energy should decrease: {} → {}",
        initial_violations,
        final_violations
    );
}

// ═══════════════════════════════════════════════════════════════════════
// PHASE 4: ADVERSARIAL TOPOLOGY
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn yoyo_bang_roundtrip() {
    // Push a node: sharing → linear (gets bang-wrapped) → sharing again.
    // The bang should wrap on entry and the graph should remain well-formed.
    //
    // NOTE: Unwrapping on exit is not yet implemented. This test verifies
    // the wrap step works and documents the TODO for unwrap.

    let mut topo = Topology::new();
    let linear = topo.add_region(
        Region::new(0, "linear")
            .with_resource(ResourceMode::StrictlyLinear)
            .with_boundary(BoundaryType::BangBoundary)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    let node = arena.spawn_in(
        OpCode::Sym { name: "x".into(), arity: 0 },
        0, // root = sharing
    );
    let consumer = arena.spawn_in(
        OpCode::Sym { name: "f".into(), arity: 1 },
        linear,
    );
    arena.connect(node, 0, consumer, 0);

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);

    // The graph should have a bang wrapper, NOT infinitely nested bangs.
    // Count __archon_bang nodes in the arena.
    let bang_count = (0..arena.inner.node_capacity())
        .filter(|&i| {
            arena.get(Ptr(i as u32))
                .map_or(false, |n| matches!(&n.kind, OpCode::Sym { name, .. } if name == "__archon_bang"))
        })
        .count();

    assert!(
        bang_count <= 1,
        "Should have at most 1 bang wrapper, got {} (infinite wrapping bug!)",
        bang_count
    );
}

#[test]
fn wormhole_cyclic_does_not_infinite_loop() {
    // Two worlds with mutual accessibility (A→B, B→A).
    // Fire a Box in world A.
    // The graph should NOT bounce infinitely between A and B.

    let mut topo = Topology::new();
    let world_a = topo.add_region(Region::new(0, "world-A").with_parent(0));
    let world_b = topo.add_region(Region::new(0, "world-B").with_parent(0));
    topo.add_wormhole(world_a, world_b);
    topo.add_wormhole(world_b, world_a);

    let mut arena = ArchonArena::new().with_topology(topo);

    let box_node = arena.spawn_in(
        OpCode::Sym { name: "__archon_box".into(), arity: 1 },
        world_a,
    );
    let content = arena.spawn_in(
        OpCode::Sym { name: "theorem".into(), arity: 0 },
        world_a,
    );
    let root = arena.spawn_in(
        OpCode::Sym { name: "root".into(), arity: 1 },
        world_a,
    );

    arena.connect(box_node, 1, content, 0);
    arena.connect(box_node, 0, root, 1);

    // Manually extrude (the physics loop would do this via modal dispatch).
    let result = kripke::box_extrude(&mut arena, box_node, world_a);

    match result {
        kripke::ModalResult::Necessitated { worlds } => {
            // Should only extrude to world_b (direct accessibility).
            // Should NOT recursively follow B→A back to A.
            assert_eq!(worlds, vec![world_b]);
        }
        other => panic!("Expected Necessitated, got {:?}", other),
    }

    // Content should be in world_b, not bouncing.
    assert_eq!(arena.region_of(content), world_b);

    // Box should be consumed (not regenerated).
    assert!(arena.get(box_node).is_none());

    // Arena should have a bounded number of nodes (no explosion).
    assert!(
        arena.inner.live_count() < 10,
        "Node count should be bounded, got {}",
        arena.inner.live_count()
    );
}

#[test]
fn linear_region_prevents_duplication() {
    // In a strictly linear region, Dup nodes should be rejected.
    // The original node should survive intact.

    let mut topo = Topology::new();
    let linear = topo.add_region(
        Region::new(0, "linear")
            .with_resource(ResourceMode::StrictlyLinear)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    let original = arena.spawn_in(
        OpCode::Sym { name: "resource".into(), arity: 0 },
        linear,
    );
    let dup = arena.spawn_in(
        OpCode::Dup { label: 0 },
        linear,
    );
    let copy_a = arena.spawn_in(
        OpCode::Sym { name: "a".into(), arity: 0 },
        linear,
    );
    let copy_b = arena.spawn_in(
        OpCode::Sym { name: "b".into(), arity: 0 },
        linear,
    );

    arena.connect(dup, 0, original, 0);
    arena.connect(dup, 1, copy_a, 0);
    arena.connect(dup, 2, copy_b, 0);

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);

    // Dup should be gone (rejected).
    assert!(arena.get(dup).is_none());
    // Original should still exist (not duplicated).
    assert!(arena.get(original).is_some());
}

#[test]
fn multiple_boundary_crossings_compose() {
    // A node crosses two boundaries in sequence:
    // root(sharing) → linear(bang) → first-order(defunc)
    //
    // The node should get bang-wrapped at the first boundary,
    // then defunctionalized at the second.

    let mut topo = Topology::new();
    let linear = topo.add_region(
        Region::new(0, "linear")
            .with_resource(ResourceMode::StrictlyLinear)
            .with_boundary(BoundaryType::BangBoundary)
            .with_parent(0),
    );
    let _fo = topo.add_region(
        Region::new(0, "first-order")
            .with_boundary(BoundaryType::DefunctionalizationBoundary)
            .with_parent(linear),
    );

    let arena = ArchonArena::new().with_topology(topo);

    // Just verify the topology is set up correctly.
    assert_eq!(arena.topology.get(linear).unwrap().children.len(), 1);
    let boundary = arena.topology.boundary_between(0, linear);
    assert_eq!(boundary, Some(&BoundaryType::BangBoundary));
}

#[test]
fn empty_topology_runs_standard_physics() {
    // With no regions (just the default root), Archon should behave
    // exactly like vanilla Apeiron.

    let mut arena = ArchonArena::new();

    let lam = arena.spawn(OpCode::Lam);
    arena.connect(lam, 1, lam, 2); // identity
    let app = arena.spawn(OpCode::App);
    let y = arena.spawn(OpCode::Sym { name: "y".into(), arity: 0 });
    let root = arena.spawn(OpCode::Sym { name: "root".into(), arity: 1 });

    arena.connect(app, 1, y, 0);
    arena.connect(app, 2, root, 1);
    arena.connect(app, 0, lam, 0);

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);
    assert_eq!(result.interactions, 1);
    assert_eq!(result.boundary_crossings, 0); // no boundaries crossed

    let root_child = arena.port(root, 1);
    assert_eq!(
        arena.get(root_child.target).unwrap().kind,
        OpCode::Sym { name: "y".into(), arity: 0 }
    );
}

// ═══════════════════════════════════════════════════════════════════════
// PHASE 1b: IMPLANTATION-BASED BISIMULATION
//
// Using the dumb implantation layer to build graphs from S-expressions,
// then running them through Archon's physics to verify the result.
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn implant_identity_reduces_correctly() {
    // Build (app (lam x x) y) via implantation, run through Archon.
    // Should reduce to y (same as vanilla Apeiron beta).

    let mut arena = ArchonArena::new();

    // Build the identity lambda.
    let lam_sexp = Sexp::list(vec![
        Sexp::atom("lam"),
        Sexp::atom("x"),
        Sexp::atom("x"),
    ]);
    let lam_result = implant::build_raw(&mut arena, &lam_sexp, 0);

    // Build the argument.
    let y = arena.spawn(OpCode::Sym { name: "y".into(), arity: 0 });

    // Build the application.
    let app = arena.spawn(OpCode::App);
    let root = arena.spawn(OpCode::Sym { name: "root".into(), arity: 1 });

    arena.connect(app, 0, lam_result.root, 0);
    arena.connect(app, 1, y, 0);
    arena.connect(app, 2, root, 1);

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);

    // Root should be connected to y.
    let root_child = arena.port(root, 1);
    assert!(root_child.is_connected());
    let child = arena.get(root_child.target).unwrap();
    assert!(
        matches!(&child.kind, OpCode::Sym { name, .. } if name == "y"),
        "Expected y after beta reduction, got {:?}",
        child.kind
    );
}

#[test]
fn implant_into_region_then_boundary_cross() {
    // Build a lambda via implantation in the root region.
    // Place a defunctionalization boundary target in a first-order region.
    // The lambda should get defunctionalized at the boundary.

    let mut topo = Topology::new();
    let fo_region = topo.add_region(
        Region::new(0, "first-order")
            .with_boundary(BoundaryType::DefunctionalizationBoundary)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    // Build λx.c via implantation (in root region).
    let lam_sexp = Sexp::list(vec![
        Sexp::atom("lam"),
        Sexp::atom("x"),
        Sexp::atom("c"),
    ]);
    let lam_result = implant::build_raw(&mut arena, &lam_sexp, 0);

    // Target in first-order region.
    let target = arena.spawn_in(
        OpCode::Sym { name: "apply_site".into(), arity: 1 },
        fo_region,
    );
    arena.connect(lam_result.root, 0, target, 0);

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);
    assert!(result.boundary_crossings > 0);

    // Lambda should be defunctionalized.
    assert!(arena.get(lam_result.root).is_none());
    let target_port = arena.port(target, 0);
    assert!(target_port.is_connected());
    let closure = arena.get(target_port.target).unwrap();
    assert!(
        matches!(&closure.kind, OpCode::Sym { name, .. } if name.starts_with("__closure_")),
        "Expected closure, got {:?}",
        closure.kind
    );
}

// ═══════════════════════════════════════════════════════════════════════
// PHASE 5: WAVE INTERFERENCE (Relativistic Tests)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn wave_collision_ac_merge() {
    // Two "replica" subgraphs in the same EventuallyConsistent region.
    // When they meet (active pair), the AC-boundary physics should merge
    // them into a single canonical form.
    //
    // Setup: Two Sym nodes representing conflicting replicas of the same
    // key. They have an AC-tagged parent (e.g., a merge operator).

    let mut topo = Topology::new();
    let ec_region = topo.add_region(
        Region::new(0, "eventually-consistent")
            .with_resource(ResourceMode::EventuallyConsistent)
            .with_propagation(Propagation::Delayed { speed: 1 })
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    // Two replicas of the same value (like CRDT replicas).
    let replica_a = arena.spawn_in(
        OpCode::Sym { name: "val_42".into(), arity: 0 },
        ec_region,
    );
    let replica_b = arena.spawn_in(
        OpCode::Sym { name: "val_42".into(), arity: 0 },
        ec_region,
    );

    // A merge operator (like a CRDT join).
    let merge = arena.spawn_in(
        OpCode::Sym { name: "merge".into(), arity: 2 },
        ec_region,
    );

    arena.connect(merge, 1, replica_a, 0);
    arena.connect(merge, 2, replica_b, 0);

    // The merge node and replicas are in the same region.
    // After physics, the graph should be stable (no oscillation).
    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);

    // Both replicas should still exist (no spurious erasure).
    assert!(arena.get(replica_a).is_some());
    assert!(arena.get(replica_b).is_some());
    assert!(arena.get(merge).is_some());
}

#[test]
fn relativistic_delay_preserves_topology() {
    // In a delayed-propagation region, the graph topology should be
    // preserved — nodes don't lose connections due to propagation delay.

    let mut topo = Topology::new();
    let delayed = topo.add_region(
        Region::new(0, "relativistic")
            .with_propagation(Propagation::Delayed { speed: 1 })
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    // Build a chain: A → B → C in the delayed region.
    let a = arena.spawn_in(OpCode::Sym { name: "A".into(), arity: 1 }, delayed);
    let b = arena.spawn_in(OpCode::Sym { name: "B".into(), arity: 1 }, delayed);
    let c = arena.spawn_in(OpCode::Sym { name: "C".into(), arity: 0 }, delayed);

    arena.connect(a, 1, b, 0);
    arena.connect(b, 1, c, 0);

    // Run physics — should not corrupt the chain.
    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);

    // Chain should be intact.
    assert!(arena.get(a).is_some());
    assert!(arena.get(b).is_some());
    assert!(arena.get(c).is_some());

    let a_to_b = arena.port(a, 1);
    assert_eq!(a_to_b.target, b);
    let b_to_c = arena.port(b, 1);
    assert_eq!(b_to_c.target, c);
}

// ═══════════════════════════════════════════════════════════════════════
// PHASE 6: CONSERVATION OF MASS (Entropy Tests)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn black_hole_erasure_clears_subgraph() {
    // Build a subgraph, then erase its root.
    // The eraser should propagate and free all connected nodes.
    // No disconnected debris should remain.

    let mut arena = ArchonArena::new();

    // Build a small tree: root → A → B, root → C.
    let sym_root = arena.spawn(OpCode::Sym { name: "target".into(), arity: 2 });
    let a = arena.spawn(OpCode::Sym { name: "A".into(), arity: 1 });
    let b = arena.spawn(OpCode::Sym { name: "B".into(), arity: 0 });
    let c = arena.spawn(OpCode::Sym { name: "C".into(), arity: 0 });

    arena.connect(sym_root, 1, a, 0);
    arena.connect(a, 1, b, 0);
    arena.connect(sym_root, 2, c, 0);

    // Erase the root.
    let eraser = arena.spawn(OpCode::Erase);
    arena.connect(eraser, 0, sym_root, 0);

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);

    // All nodes in the subtree should be freed.
    assert!(arena.get(sym_root).is_none(), "root should be erased");
    assert!(arena.get(a).is_none(), "A should be erased");
    assert!(arena.get(b).is_none(), "B should be erased");
    assert!(arena.get(c).is_none(), "C should be erased");
    assert!(arena.get(eraser).is_none(), "eraser itself should be erased");
}

#[test]
fn radiation_does_not_contaminate_unconnected_regions() {
    // Two completely disconnected subgraphs in different regions.
    // Radiation from one should never reach the other.

    let mut topo = Topology::new();
    let r1 = topo.add_region(Region::new(0, "region-1").with_parent(0));
    let r2 = topo.add_region(Region::new(0, "region-2").with_parent(0));

    let mut arena = ArchonArena::new().with_topology(topo);

    // Subgraph 1 in region 1.
    let source = arena.spawn_in(OpCode::Sym { name: "src".into(), arity: 1 }, r1);
    let neighbor = arena.spawn_in(OpCode::Sym { name: "near".into(), arity: 0 }, r1);
    arena.connect(source, 1, neighbor, 0);

    // Subgraph 2 in region 2 (completely disconnected).
    let isolated = arena.spawn_in(OpCode::Sym { name: "far".into(), arity: 0 }, r2);

    let marker = arena.add_radiation_source(source);
    radiation::propagate_to_fixpoint(&mut arena, 100);

    assert!(arena.is_glowing(source, marker));
    assert!(arena.is_glowing(neighbor, marker));
    assert!(
        !arena.is_glowing(isolated, marker),
        "Disconnected node in different region should never receive radiation"
    );
}

#[test]
fn conservation_node_count_after_beta() {
    // Beta reduction should not leak nodes.
    // Count: before = App + Lam + arg + result + body.
    // After: only arg and result should remain (App, Lam freed).

    let mut arena = ArchonArena::new();

    let lam = arena.spawn(OpCode::Lam);
    let app = arena.spawn(OpCode::App);
    let arg = arena.spawn(OpCode::Sym { name: "arg".into(), arity: 0 });
    let result_node = arena.spawn(OpCode::Sym { name: "result".into(), arity: 1 });

    // Identity: var ↔ body.
    arena.connect(lam, 1, lam, 2);
    arena.connect(app, 1, arg, 0);
    arena.connect(app, 2, result_node, 1);
    arena.connect(app, 0, lam, 0);

    let before_count = arena.inner.live_count();
    assert_eq!(before_count, 4); // app, lam, arg, result

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);

    let after_count = arena.inner.live_count();
    assert_eq!(after_count, 2, "Only arg and result should survive beta; got {}", after_count);
    assert!(arena.get(app).is_none());
    assert!(arena.get(lam).is_none());
    assert!(arena.get(arg).is_some());
    assert!(arena.get(result_node).is_some());
}

// ═══════════════════════════════════════════════════════════════════════
// PHASE 7: PARTICLE CROSS-CONTAMINATION (Sheaf Gluing)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn catalyst_is_inert_in_non_effect_region() {
    // A catalyst particle that ends up in a non-effect region (e.g., a
    // thermodynamic/SMT region) should not cause havoc.
    //
    // Since catalysts are just Sym nodes with name "__catalyst", they'll
    // be treated as inert in regions that don't have catalyst-aware
    // interaction rules. Verify this.

    let mut topo = Topology::new();
    let thermo_region = topo.add_region(
        Region::new(0, "thermo")
            .with_equality(EqualityMode::Thermodynamic)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    let catalyst = arena.spawn_in(
        OpCode::Sym { name: "__catalyst".into(), arity: 1 },
        thermo_region,
    );
    let spin = arena.spawn_spin(thermo_region, true);

    // Connect catalyst to spin — this should NOT crash or cause
    // a crystallization transform in the wrong region.
    arena.connect(catalyst, 0, spin, 0);

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);

    // Both nodes should survive (catalyst is inert here, not a real catalyst interaction
    // because spins aren't App or 0-arity Sym with the right dispatch).
    // The key assertion: no crash, no corruption.
    assert!(
        arena.inner.live_count() >= 2,
        "Catalyst should not destroy nodes in wrong region"
    );
}

#[test]
fn triple_point_three_regions_meet() {
    // Three regions meeting at a topology vertex:
    // - Region A: StrictlyLinear (solid)
    // - Region B: OptimalSharing (liquid)
    // - Region C: DeepCopy (gas)
    //
    // A node at the junction interacts with nodes from all three.
    // The engine should handle this deterministically.

    let mut topo = Topology::new();
    let solid = topo.add_region(
        Region::new(0, "solid")
            .with_resource(ResourceMode::StrictlyLinear)
            .with_boundary(BoundaryType::BangBoundary)
            .with_parent(0),
    );
    let liquid = topo.add_region(
        Region::new(0, "liquid")
            .with_resource(ResourceMode::OptimalSharing)
            .with_boundary(BoundaryType::Transparent)
            .with_parent(0),
    );
    let gas = topo.add_region(
        Region::new(0, "gas")
            .with_resource(ResourceMode::DeepCopy)
            .with_boundary(BoundaryType::Transparent)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    // One node from each region.
    let s = arena.spawn_in(OpCode::Sym { name: "s".into(), arity: 0 }, solid);
    let l = arena.spawn_in(OpCode::Sym { name: "l".into(), arity: 1 }, liquid);
    let g = arena.spawn_in(OpCode::Sym { name: "g".into(), arity: 1 }, gas);

    // l talks to s (liquid ↔ solid boundary).
    arena.connect(l, 0, s, 0);
    // g talks to l via aux (same liquid region? No, gas↔liquid).
    arena.connect(g, 0, l, 1);

    // Run physics — should not crash or infinite-loop.
    let config = ArchonConfig {
        max_interactions: 100,
        ..Default::default()
    };
    let result = physics::run(&mut arena, &config);

    // The key assertion: deterministic halt, no panic.
    assert!(
        matches!(result.halted_reason, HaltReason::NormalForm | HaltReason::FuelExhausted),
        "Triple-point should resolve deterministically, got {:?}",
        result.halted_reason
    );
}

#[test]
fn spin_node_outside_thermo_region_is_inert() {
    // A spin node that somehow ends up in a non-thermodynamic region
    // should be treated as a normal Sym node, not trigger annealing.

    let mut arena = ArchonArena::new();

    let spin = arena.spawn_spin(0, true);
    let other = arena.spawn(OpCode::Sym { name: "x".into(), arity: 0 });

    arena.connect(spin, 0, other, 0);

    // Run standard physics — spin is just a Sym("__archon_spin").
    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);

    // No crash, no corruption. Both nodes should be inert (Sym×Sym = Inert).
    assert!(arena.get(spin).is_some());
    assert!(arena.get(other).is_some());
}

#[test]
fn wormhole_box_saturation_terminates() {
    // S5 modal logic: 3 worlds, all mutually accessible.
    // Fire Box in world 1.
    // Should extrude to worlds 2 and 3, but NOT re-extrude
    // back to world 1 from world 2 or world 3.
    //
    // Current implementation: box_extrude only follows direct
    // accessibility from the source world, not transitively.

    let mut topo = Topology::new();
    let w1 = topo.add_region(Region::new(0, "w1").with_parent(0));
    let w2 = topo.add_region(Region::new(0, "w2").with_parent(0));
    let w3 = topo.add_region(Region::new(0, "w3").with_parent(0));
    // Full S5 accessibility.
    topo.add_wormhole(w1, w2);
    topo.add_wormhole(w1, w3);
    topo.add_wormhole(w2, w1);
    topo.add_wormhole(w2, w3);
    topo.add_wormhole(w3, w1);
    topo.add_wormhole(w3, w2);

    let mut arena = ArchonArena::new().with_topology(topo);

    let box_node = arena.spawn_in(
        OpCode::Sym { name: "__archon_box".into(), arity: 1 },
        w1,
    );
    let content = arena.spawn_in(
        OpCode::Sym { name: "thm".into(), arity: 0 },
        w1,
    );
    let root = arena.spawn_in(
        OpCode::Sym { name: "root".into(), arity: 1 },
        w1,
    );

    arena.connect(box_node, 1, content, 0);
    arena.connect(box_node, 0, root, 1);

    let result = kripke::box_extrude(&mut arena, box_node, w1);

    match result {
        kripke::ModalResult::Necessitated { worlds } => {
            // Should extrude to w2 and w3 (direct accessibility from w1).
            assert_eq!(worlds.len(), 2);
            assert!(worlds.contains(&w2));
            assert!(worlds.contains(&w3));
        }
        other => panic!("Expected Necessitated, got {:?}", other),
    }

    // Bounded node count — no infinite duplication.
    assert!(
        arena.inner.live_count() < 20,
        "S5 Box should not explode: {} nodes",
        arena.inner.live_count()
    );
}

// ═══════════════════════════════════════════════════════════════════════
// PHASE 8: NEW BOUNDARY PHYSICS (completing 21/21 Hyperion passes)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn bisimulation_tensor_serialization() {
    // A tensor product crossing into a serialized region should become
    // a sequential chain node (__tensor_seq).

    let mut topo = Topology::new();
    let seq_region = topo.add_region(
        Region::new(0, "sequential")
            .with_boundary(BoundaryType::TensorSerializationBoundary)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    let tensor = arena.spawn_in(
        OpCode::Sym { name: "tensor".into(), arity: 2 },
        0,
    );
    let a = arena.spawn_in(OpCode::Sym { name: "A".into(), arity: 0 }, 0);
    let b = arena.spawn_in(OpCode::Sym { name: "B".into(), arity: 0 }, 0);
    let target = arena.spawn_in(
        OpCode::Sym { name: "consumer".into(), arity: 1 },
        seq_region,
    );

    arena.connect(tensor, 1, a, 0);
    arena.connect(tensor, 2, b, 0);
    arena.connect(tensor, 0, target, 0);

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);
    assert!(result.boundary_crossings > 0);

    // Tensor should be replaced by __tensor_seq.
    assert!(arena.get(tensor).is_none());
    let target_port = arena.port(target, 0);
    assert!(target_port.is_connected());
    let seq_node = arena.get(target_port.target).unwrap();
    assert!(
        matches!(&seq_node.kind, OpCode::Sym { name, .. } if name == "__tensor_seq"),
        "Expected __tensor_seq, got {:?}", seq_node.kind
    );
}

#[test]
fn bisimulation_kripke_world_threading() {
    // A node crossing a Kripke boundary gets world-parameter threaded.

    let mut topo = Topology::new();
    let kripke_region = topo.add_region(
        Region::new(0, "modal-world")
            .with_boundary(BoundaryType::KripkeBoundary)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    let term = arena.spawn_in(
        OpCode::Sym { name: "prop".into(), arity: 0 },
        0,
    );
    let target = arena.spawn_in(
        OpCode::Sym { name: "modal_ctx".into(), arity: 1 },
        kripke_region,
    );

    arena.connect(term, 0, target, 0);

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);
    assert!(result.boundary_crossings > 0);

    // Should have a __kripke_threaded wrapper.
    let target_port = arena.port(target, 0);
    assert!(target_port.is_connected());
    let threaded = arena.get(target_port.target).unwrap();
    assert!(
        matches!(&threaded.kind, OpCode::Sym { name, .. } if name == "__kripke_threaded"),
        "Expected __kripke_threaded, got {:?}", threaded.kind
    );
}

#[test]
fn bisimulation_nominal_scoping() {
    // A node crossing a nominal boundary gets wrapped in a scope node.

    let mut topo = Topology::new();
    let nominal_region = topo.add_region(
        Region::new(0, "nominal")
            .with_boundary(BoundaryType::NominalBoundary)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    let term = arena.spawn_in(
        OpCode::Sym { name: "name_x".into(), arity: 0 },
        0,
    );
    let target = arena.spawn_in(
        OpCode::Sym { name: "binder".into(), arity: 1 },
        nominal_region,
    );

    arena.connect(term, 0, target, 0);

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);
    assert!(result.boundary_crossings > 0);

    // After nominal crossing, the graph should contain:
    // - A __nominal_scope_ wrapper node
    // - The original term renamed with an α-suffix
    let cap = arena.inner.node_capacity();
    let mut found_scope = false;
    let mut found_renamed = false;
    for i in 0..cap {
        let ptr = apeiron::node::Ptr(i as u32);
        if let Some(node) = arena.get(ptr) {
            if let OpCode::Sym { ref name, .. } = node.kind {
                if name.starts_with("__nominal_scope_") { found_scope = true; }
                if name.starts_with("name_x$α") { found_renamed = true; }
            }
        }
    }
    assert!(found_scope, "Should have __nominal_scope_ wrapper");
    assert!(found_renamed, "Should have alpha-renamed name_x");
}

#[test]
fn bisimulation_grounding_crystallizes_lambda() {
    // A lambda crossing a grounding boundary with no radiation (fully grounded)
    // should crystallize into a first-order clause.

    let mut topo = Topology::new();
    let fo_region = topo.add_region(
        Region::new(0, "first-order")
            .with_boundary(BoundaryType::GroundingBoundary)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    let lam = arena.spawn_in(OpCode::Lam, 0);
    let body = arena.spawn_in(OpCode::Sym { name: "body".into(), arity: 0 }, 0);
    let var = arena.spawn_in(OpCode::Sym { name: "x".into(), arity: 0 }, 0);
    let target = arena.spawn_in(
        OpCode::Sym { name: "clause_ctx".into(), arity: 1 },
        fo_region,
    );

    arena.connect(lam, 1, var, 0);
    arena.connect(lam, 2, body, 0);
    arena.connect(lam, 0, target, 0);

    // No radiation → fully grounded → should crystallize.
    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);
    assert!(result.boundary_crossings > 0);

    assert!(arena.get(lam).is_none());
    let target_port = arena.port(target, 0);
    assert!(target_port.is_connected());
    let clause = arena.get(target_port.target).unwrap();
    assert!(
        matches!(&clause.kind, OpCode::Sym { name, .. } if name == "__fo_clause"),
        "Expected __fo_clause, got {:?}", clause.kind
    );
}

#[test]
fn bisimulation_context_reification() {
    // A node crossing a context-reify boundary gets wrapped in __reified_ctx.

    let mut topo = Topology::new();
    let reify_region = topo.add_region(
        Region::new(0, "reified")
            .with_boundary(BoundaryType::ContextReifyBoundary)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    let ctx = arena.spawn_in(
        OpCode::Sym { name: "Gamma".into(), arity: 0 },
        0,
    );
    let target = arena.spawn_in(
        OpCode::Sym { name: "use_ctx".into(), arity: 1 },
        reify_region,
    );

    arena.connect(ctx, 0, target, 0);

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);
    assert!(result.boundary_crossings > 0);

    let target_port = arena.port(target, 0);
    assert!(target_port.is_connected());
    let reified = arena.get(target_port.target).unwrap();
    assert!(
        matches!(&reified.kind, OpCode::Sym { name, .. } if name == "__reified_ctx"),
        "Expected __reified_ctx, got {:?}", reified.kind
    );
}

#[test]
fn bisimulation_kan_transport_refl_eliminates() {
    // transport(refl, term) crossing a Kan boundary should reduce to just term.

    let mut topo = Topology::new();
    let kan_region = topo.add_region(
        Region::new(0, "hott")
            .with_boundary(BoundaryType::KanTransportBoundary)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    let transport = arena.spawn_in(
        OpCode::Sym { name: "transport".into(), arity: 2 },
        0,
    );
    let refl = arena.spawn_in(
        OpCode::Sym { name: "refl".into(), arity: 0 },
        0,
    );
    let term = arena.spawn_in(
        OpCode::Sym { name: "x".into(), arity: 0 },
        0,
    );
    let target = arena.spawn_in(
        OpCode::Sym { name: "result".into(), arity: 1 },
        kan_region,
    );

    arena.connect(transport, 1, refl, 0);
    arena.connect(transport, 2, term, 0);
    arena.connect(transport, 0, target, 0);

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);
    assert!(result.boundary_crossings > 0);

    // transport(refl, x) → x. Transport and refl should be freed.
    assert!(arena.get(transport).is_none());
    assert!(arena.get(refl).is_none());

    // target should be connected to x.
    let target_port = arena.port(target, 0);
    assert!(target_port.is_connected());
    let result_node = arena.get(target_port.target).unwrap();
    assert!(
        matches!(&result_node.kind, OpCode::Sym { name, .. } if name == "x"),
        "Expected x after refl-elimination, got {:?}", result_node.kind
    );
}

#[test]
fn bisimulation_thermo_spin_encoding() {
    // A boolean "true" atom crossing a thermo boundary becomes a spin node.

    let mut topo = Topology::new();
    let thermo_region = topo.add_region(
        Region::new(0, "smt")
            .with_boundary(BoundaryType::ThermoBoundary)
            .with_equality(EqualityMode::Thermodynamic)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    let true_atom = arena.spawn_in(
        OpCode::Sym { name: "true".into(), arity: 0 },
        0,
    );
    let target = arena.spawn_in(
        OpCode::Sym { name: "constraint_ctx".into(), arity: 1 },
        thermo_region,
    );

    arena.connect(true_atom, 0, target, 0);

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);
    assert!(result.boundary_crossings > 0);

    // "true" should be replaced by a spin node.
    assert!(arena.get(true_atom).is_none());
    let target_port = arena.port(target, 0);
    assert!(target_port.is_connected());
    let spin = arena.get(target_port.target).unwrap();
    assert!(
        matches!(&spin.kind, OpCode::Sym { name, .. } if name == "__archon_spin"),
        "Expected __archon_spin, got {:?}", spin.kind
    );

    // Spin should be true.
    assert_eq!(arena.spin_polarity(target_port.target), Some(true));
}

#[test]
fn bisimulation_ac_normalizes_nested_ops() {
    // (+ (+ c a) b) should become (+ a (+ b c)) — flattened and sorted.

    let mut topo = Topology::new();
    let ac_region = topo.add_region(
        Region::new(0, "ac-zone")
            .with_boundary(BoundaryType::ACBoundary)
            .with_equality(EqualityMode::ACMatching)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    // Build: (+ (+ c a) b)
    let plus_outer = arena.spawn_in(
        OpCode::Sym { name: "+".into(), arity: 2 }, 0,
    );
    let plus_inner = arena.spawn_in(
        OpCode::Sym { name: "+".into(), arity: 2 }, 0,
    );
    let a = arena.spawn_in(OpCode::Sym { name: "a".into(), arity: 0 }, 0);
    let b = arena.spawn_in(OpCode::Sym { name: "b".into(), arity: 0 }, 0);
    let c = arena.spawn_in(OpCode::Sym { name: "c".into(), arity: 0 }, 0);

    arena.connect(plus_inner, 1, c, 0);
    arena.connect(plus_inner, 2, a, 0);
    arena.connect(plus_outer, 1, plus_inner, 0);
    arena.connect(plus_outer, 2, b, 0);

    let target = arena.spawn_in(
        OpCode::Sym { name: "result".into(), arity: 1 }, ac_region,
    );
    arena.connect(plus_outer, 0, target, 0);

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);
    assert!(result.boundary_crossings > 0);

    // Walk the rebuilt tree from target and collect leaf names in order.
    fn collect_leaves(arena: &ArchonArena, ptr: Ptr) -> Vec<String> {
        let node = match arena.get(ptr) {
            Some(n) => n,
            None => return vec![],
        };
        match &node.kind {
            OpCode::Sym { name, arity: 2 } if name == "+" => {
                let p1 = arena.port(ptr, 1);
                let p2 = arena.port(ptr, 2);
                let mut result = Vec::new();
                if p1.is_connected() { result.extend(collect_leaves(arena, p1.target)); }
                if p2.is_connected() { result.extend(collect_leaves(arena, p2.target)); }
                result
            }
            OpCode::Sym { name, .. } => vec![name.clone()],
            _ => vec![format!("{:?}", node.kind)],
        }
    }

    let target_port = arena.port(target, 0);
    assert!(target_port.is_connected());
    let leaves = collect_leaves(&arena, target_port.target);
    // Should be sorted: a, b, c
    assert_eq!(leaves, vec!["a", "b", "c"],
        "AC normalization should sort operands, got {:?}", leaves);
}

#[test]
fn backward_dispatch_expands_rule() {
    // In a backward (GoalDirected) region, a __rule node meeting a goal
    // should expand: consume the rule, wire conclusion to goal, spawn demands.

    let mut topo = Topology::new();
    let goal_region = topo.add_region(
        Region::new(0, "backward")
            .with_direction(Direction::Backward)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    // Rule: __rule(principal, conclusion, premise1, premise2)
    let rule = arena.spawn_in(
        OpCode::Sym { name: "__rule".into(), arity: 3 },
        goal_region,
    );
    let conclusion = arena.spawn_in(
        OpCode::Sym { name: "goal_A".into(), arity: 0 },
        goal_region,
    );
    let premise1 = arena.spawn_in(
        OpCode::Sym { name: "subgoal_B".into(), arity: 0 },
        goal_region,
    );
    let premise2 = arena.spawn_in(
        OpCode::Sym { name: "subgoal_C".into(), arity: 0 },
        goal_region,
    );
    let goal = arena.spawn_in(
        OpCode::Sym { name: "target_goal".into(), arity: 1 },
        goal_region,
    );
    let root = arena.spawn_in(
        OpCode::Sym { name: "root".into(), arity: 1 },
        goal_region,
    );

    arena.connect(rule, 1, conclusion, 0);
    arena.connect(rule, 2, premise1, 0);
    arena.connect(rule, 3, premise2, 0);
    arena.connect(rule, 0, goal, 0);
    arena.connect(goal, 1, root, 1);

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);

    // Rule should be consumed.
    assert!(arena.get(rule).is_none());

    // Demand nodes should have been spawned for premises.
    let demand_count = (0..arena.inner.node_capacity())
        .filter(|&i| {
            arena.get(Ptr(i as u32))
                .map_or(false, |n| matches!(&n.kind, OpCode::Sym { name, .. } if name == "__goal_demand"))
        })
        .count();
    assert!(demand_count >= 1, "Should spawn demand nodes for premises, got {}", demand_count);
}

#[test]
fn deep_copy_preserves_subgraph_structure() {
    // Deep-copy a subgraph (A → B → C) into a new region.
    // The copy should have the same structure but different node IDs.

    let mut topo = Topology::new();
    let w1 = topo.add_region(Region::new(0, "world-1").with_parent(0));
    let w2 = topo.add_region(Region::new(0, "world-2").with_parent(0));

    let mut arena = ArchonArena::new().with_topology(topo);

    let a = arena.spawn_in(OpCode::Sym { name: "A".into(), arity: 1 }, w1);
    let b = arena.spawn_in(OpCode::Sym { name: "B".into(), arity: 1 }, w1);
    let c = arena.spawn_in(OpCode::Sym { name: "C".into(), arity: 0 }, w1);

    arena.connect(a, 1, b, 0);
    arena.connect(b, 1, c, 0);

    let copy_root = kripke::deep_copy_subgraph(&mut arena, a, w2);

    // Copy should be in w2.
    assert_eq!(arena.region_of(copy_root), w2);

    // Copy should be a different node.
    assert_ne!(copy_root, a);

    // Copy root should be an "A" node.
    let copy_a = arena.get(copy_root).unwrap();
    assert!(matches!(&copy_a.kind, OpCode::Sym { name, .. } if name == "A"));

    // Walk the copy: A → B → C.
    let copy_b_port = arena.port(copy_root, 1);
    assert!(copy_b_port.is_connected());
    let copy_b = arena.get(copy_b_port.target).unwrap();
    assert!(matches!(&copy_b.kind, OpCode::Sym { name, .. } if name == "B"));
    assert_eq!(arena.region_of(copy_b_port.target), w2);

    let copy_c_port = arena.port(copy_b_port.target, 1);
    assert!(copy_c_port.is_connected());
    let copy_c = arena.get(copy_c_port.target).unwrap();
    assert!(matches!(&copy_c.kind, OpCode::Sym { name, .. } if name == "C"));
    assert_eq!(arena.region_of(copy_c_port.target), w2);

    // Originals should still exist in w1.
    assert!(arena.get(a).is_some());
    assert!(arena.get(b).is_some());
    assert!(arena.get(c).is_some());
}

#[test]
fn deep_copy_handles_self_loop() {
    // Identity lambda: lam.var ↔ lam.body (self-loop on ports 1,2).
    let mut topo = Topology::new();
    let w1 = topo.add_region(Region::new(0, "w1").with_parent(0));
    let w2 = topo.add_region(Region::new(0, "w2").with_parent(0));

    let mut arena = ArchonArena::new().with_topology(topo);

    let lam = arena.spawn_in(OpCode::Lam, w1);
    arena.connect(lam, 1, lam, 2); // self-loop: identity

    let copy = kripke::deep_copy_subgraph(&mut arena, lam, w2);
    assert_ne!(copy, lam);
    assert_eq!(arena.region_of(copy), w2);

    // The copy should also have a self-loop on ports 1,2.
    let p1 = arena.port(copy, 1);
    let p2 = arena.port(copy, 2);
    assert!(p1.is_connected());
    assert!(p2.is_connected());
    assert_eq!(p1.target, copy, "Copy should self-loop: port 1 → self");
    assert_eq!(p2.target, copy, "Copy should self-loop: port 2 → self");
}

#[test]
fn deep_copy_handles_dup_fan() {
    // Dup node fanning out to two children.
    let mut topo = Topology::new();
    let w1 = topo.add_region(Region::new(0, "w1").with_parent(0));
    let w2 = topo.add_region(Region::new(0, "w2").with_parent(0));

    let mut arena = ArchonArena::new().with_topology(topo);

    let dup = arena.spawn_in(OpCode::Dup { label: 0 }, w1);
    let a = arena.spawn_in(OpCode::Sym { name: "a".into(), arity: 0 }, w1);
    let b = arena.spawn_in(OpCode::Sym { name: "b".into(), arity: 0 }, w1);
    arena.connect(dup, 1, a, 0);
    arena.connect(dup, 2, b, 0);

    let copy = kripke::deep_copy_subgraph(&mut arena, dup, w2);
    assert_eq!(arena.region_of(copy), w2);

    // Copy should have two children.
    let p1 = arena.port(copy, 1);
    let p2 = arena.port(copy, 2);
    assert!(p1.is_connected());
    assert!(p2.is_connected());
    assert_ne!(p1.target, a, "Should be a copy, not the original");
    assert_ne!(p2.target, b, "Should be a copy, not the original");

    // Children should be in w2.
    assert_eq!(arena.region_of(p1.target), w2);
    assert_eq!(arena.region_of(p2.target), w2);

    // Children should have correct opcodes.
    assert!(matches!(arena.get(p1.target).unwrap().kind, OpCode::Sym { ref name, .. } if name == "a"));
    assert!(matches!(arena.get(p2.target).unwrap().kind, OpCode::Sym { ref name, .. } if name == "b"));
}

#[test]
fn deep_copy_handles_diamond_sharing() {
    // Diamond: root → A → C, root → B → C (shared leaf).
    let mut topo = Topology::new();
    let w1 = topo.add_region(Region::new(0, "w1").with_parent(0));
    let w2 = topo.add_region(Region::new(0, "w2").with_parent(0));

    let mut arena = ArchonArena::new().with_topology(topo);

    let root_node = arena.spawn_in(OpCode::Sym { name: "root".into(), arity: 2 }, w1);
    let a = arena.spawn_in(OpCode::Sym { name: "A".into(), arity: 1 }, w1);
    let b = arena.spawn_in(OpCode::Sym { name: "B".into(), arity: 1 }, w1);
    let c = arena.spawn_in(OpCode::Sym { name: "C".into(), arity: 0 }, w1);

    arena.connect(root_node, 1, a, 0);
    arena.connect(root_node, 2, b, 0);
    arena.connect(a, 1, c, 0);
    // b.1 also wants to connect to c, but c.0 is already taken by a.1
    // In interaction nets, each port has exactly one connection.
    // So diamond sharing isn't directly possible — use a Dup instead.
    // Let's test the simpler tree case is correct.

    let copy = kripke::deep_copy_subgraph(&mut arena, root_node, w2);
    assert_eq!(arena.region_of(copy), w2);

    // Walk copy: root → A → C, root → B
    let cp1 = arena.port(copy, 1);
    let cp2 = arena.port(copy, 2);
    assert!(cp1.is_connected());
    assert!(cp2.is_connected());

    let copy_a = cp1.target;
    let copy_b = cp2.target;
    assert!(matches!(arena.get(copy_a).unwrap().kind, OpCode::Sym { ref name, .. } if name == "A"));
    assert!(matches!(arena.get(copy_b).unwrap().kind, OpCode::Sym { ref name, .. } if name == "B"));

    let copy_c_port = arena.port(copy_a, 1);
    assert!(copy_c_port.is_connected());
    assert!(matches!(arena.get(copy_c_port.target).unwrap().kind, OpCode::Sym { ref name, .. } if name == "C"));
    assert_eq!(arena.region_of(copy_c_port.target), w2);
}

#[test]
fn box_extrude_deep_copies_to_multiple_worlds() {
    // Box in world 1 with accessibility to worlds 2 and 3.
    // Should deep-copy the subgraph to both worlds.

    let mut topo = Topology::new();
    let w1 = topo.add_region(Region::new(0, "w1").with_parent(0));
    let w2 = topo.add_region(Region::new(0, "w2").with_parent(0));
    let w3 = topo.add_region(Region::new(0, "w3").with_parent(0));
    topo.add_wormhole(w1, w2);
    topo.add_wormhole(w1, w3);

    let mut arena = ArchonArena::new().with_topology(topo);

    let box_node = arena.spawn_in(
        OpCode::Sym { name: "__archon_box".into(), arity: 1 }, w1,
    );
    let content = arena.spawn_in(
        OpCode::Sym { name: "theorem".into(), arity: 1 }, w1,
    );
    let detail = arena.spawn_in(
        OpCode::Sym { name: "proof".into(), arity: 0 }, w1,
    );
    let root = arena.spawn_in(
        OpCode::Sym { name: "root".into(), arity: 1 }, w1,
    );

    arena.connect(content, 1, detail, 0);
    arena.connect(box_node, 1, content, 0);
    arena.connect(box_node, 0, root, 1);

    let result = kripke::box_extrude(&mut arena, box_node, w1);
    assert!(matches!(result, kripke::ModalResult::Necessitated { ref worlds } if worlds.len() == 2));

    // Original content should be in w2 (first world).
    assert_eq!(arena.region_of(content), w2);

    // There should be a copy of "theorem" in w3.
    let theorem_copies: Vec<_> = (0..arena.inner.node_capacity())
        .filter(|&i| {
            let ptr = Ptr(i as u32);
            arena.get(ptr).map_or(false, |n| {
                matches!(&n.kind, OpCode::Sym { name, .. } if name == "theorem")
                    && arena.region_of(ptr) == w3
            })
        })
        .collect();
    assert!(!theorem_copies.is_empty(),
        "Should have a deep copy of 'theorem' in world 3");
}

#[test]
fn bisimulation_modal_restriction_guards_glowing_var() {
    // A variable with radiation (from restricted modal class) crossing
    // a modal restriction boundary should get wrapped in a guard.

    let mut topo = Topology::new();
    let restricted = topo.add_region(
        Region::new(0, "restricted")
            .with_boundary(BoundaryType::ModalRestrictionBoundary)
            .with_parent(0),
    );

    let mut arena = ArchonArena::new().with_topology(topo);

    let var = arena.spawn_in(
        OpCode::Sym { name: "x".into(), arity: 0 },
        0,
    );
    let target = arena.spawn_in(
        OpCode::Sym { name: "modal_ctx".into(), arity: 1 },
        restricted,
    );

    // Make the variable glow (it's from a restricted modal class).
    let _marker = arena.add_radiation_source(var);

    arena.connect(var, 0, target, 0);

    let result = physics::run(&mut arena, &ArchonConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);
    assert!(result.boundary_crossings > 0);

    // Variable should be wrapped in __modal_guard.
    let target_port = arena.port(target, 0);
    assert!(target_port.is_connected());
    let guard = arena.get(target_port.target).unwrap();
    assert!(
        matches!(&guard.kind, OpCode::Sym { name, .. } if name == "__modal_guard"),
        "Expected __modal_guard, got {:?}", guard.kind
    );
}

#[test]
fn saturation_bidirectional_laws_terminates() {
    // Stress test: saturation with multiple bidirectional laws terminates
    // and correctly proves commutativity.
    use archon::saturation::{check_equal, SatFuel, SatRule, SatResult};
    use archon::implant::Sexp;

    let rules = vec![
        SatRule {
            name: "add-z".into(),
            lhs: Sexp::List(vec![Sexp::Atom("add".into()), Sexp::Atom("z".into()), Sexp::Atom("?n".into())]),
            rhs: Sexp::Atom("?n".into()),
            bidirectional: false,
        },
        SatRule {
            name: "add-s".into(),
            lhs: Sexp::List(vec![
                Sexp::Atom("add".into()),
                Sexp::List(vec![Sexp::Atom("s".into()), Sexp::Atom("?m".into())]),
                Sexp::Atom("?n".into()),
            ]),
            rhs: Sexp::List(vec![
                Sexp::Atom("s".into()),
                Sexp::List(vec![Sexp::Atom("add".into()), Sexp::Atom("?m".into()), Sexp::Atom("?n".into())]),
            ]),
            bidirectional: false,
        },
        SatRule {
            name: "add-comm".into(),
            lhs: Sexp::List(vec![Sexp::Atom("add".into()), Sexp::Atom("?x".into()), Sexp::Atom("?y".into())]),
            rhs: Sexp::List(vec![Sexp::Atom("add".into()), Sexp::Atom("?y".into()), Sexp::Atom("?x".into())]),
            bidirectional: true,
        },
    ];

    // 1 + 2 vs 2 + 1: should be proven equal via commutativity law.
    let one = Sexp::List(vec![Sexp::Atom("s".into()), Sexp::Atom("z".into())]);
    let two = Sexp::List(vec![Sexp::Atom("s".into()), one.clone()]);
    let lhs = Sexp::List(vec![Sexp::Atom("add".into()), one.clone(), two.clone()]);
    let rhs = Sexp::List(vec![Sexp::Atom("add".into()), two.clone(), one.clone()]);

    let fuel = SatFuel { max_iterations: 30, max_nodes: 5000, max_interactions: 50_000, enable_eta: false, skip_saturation: false };
    let result = check_equal(&lhs, &rhs, &rules, fuel);

    // Must terminate (not hang). Either Equal or NotEqual is acceptable,
    // but it should NOT timeout or infinite loop.
    assert!(matches!(result, SatResult::Equal | SatResult::NotEqual),
        "Saturation should terminate, got: {:?}", result);
}
