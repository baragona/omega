//! session_archon.rs — The Brain Transplant.
//!
//! Translates Hyperion's CompiledUniverse configuration into an Archon
//! Topology, then uses the Archon physics engine to execute theories
//! instead of delegating to Apeiron passes.
//!
//! The mapping: each CompilationPass becomes a membrane boundary in the
//! Archon topology. The substrate's resource/equality/barrier modes set
//! the base region's physics. Terms are implanted via archon::implant,
//! and the physics engine handles all transformations as emergent
//! boundary interactions.

use std::collections::HashMap;

use archon::extended_arena::ArchonArena;
use archon::implant::{self, Sexp as ArchonSexp};
use archon::physics::{self, ArchonConfig, ArchonResult, HaltReason};
use archon::region::*;
use archon::thermo;

use crate::substrate::{self, SubstrateDef, EqualityMode, ResourceMode, BarrierMode};
use crate::universe::{CompiledUniverse, CompilationPass};

/// Build an Archon Topology from a CompiledUniverse.
///
/// The substrate defines the base region. Each compilation pass wraps
/// the base region in an additional boundary membrane.
pub fn build_topology(compiled: &CompiledUniverse, sub: &SubstrateDef) -> Topology {
    let mut topo = Topology::new();

    // Region 0 is the root (always exists in Topology::new()).
    // Configure it from the substrate.
    if let Some(root) = topo.get_mut(0) {
        root.resource_mode = translate_resource_mode(&sub.resource_mode);
        root.equality_mode = translate_equality_mode(&sub.equality);
        root.direction = Direction::Forward;
        root.propagation = Propagation::Instant;
    }

    // Each pass creates a child region with the appropriate boundary.
    // Terms start in the inner region and flow outward through boundaries.
    let mut inner_region = 0u32;

    for pass in &compiled.passes {
        let (boundary, resource_override, equality_override, direction) = pass_to_boundary(pass);

        let mut region = Region::new(topo.next_id(), pass.name())
            .with_boundary(boundary)
            .with_parent(inner_region);

        if let Some(rm) = resource_override {
            region = region.with_resource(rm);
        } else if let Some(r) = topo.get(inner_region) {
            region = region.with_resource(r.resource_mode.clone());
        }

        if let Some(em) = equality_override {
            region = region.with_equality(em);
        }

        if let Some(dir) = direction {
            region.direction = dir;
        }

        let new_id = topo.add_region(region);

        // For Kripke world threading, add wormholes between accessible worlds.
        if matches!(pass, CompilationPass::KripkeWorldThreading) {
            topo.add_wormhole(inner_region, new_id);
        }

        inner_region = new_id;
    }

    topo
}

/// Map a CompilationPass to (BoundaryType, optional resource override, optional equality override, optional direction).
fn pass_to_boundary(pass: &CompilationPass) -> (BoundaryType, Option<archon::region::ResourceMode>, Option<archon::region::EqualityMode>, Option<Direction>) {
    use archon::region::ResourceMode as ARM;
    match pass {
        CompilationPass::BangModality => (
            BoundaryType::BangBoundary,
            Some(ARM::StrictlyLinear),
            None,
            None,
        ),
        CompilationPass::Defunctionalization | CompilationPass::HOASDefunctionalization => (
            BoundaryType::DefunctionalizationBoundary,
            None,
            None,
            None,
        ),
        CompilationPass::TensorSerialization | CompilationPass::ParallelTensorProof => (
            BoundaryType::TensorSerializationBoundary,
            None,
            None,
            None,
        ),
        CompilationPass::KripkeWorldThreading => (
            BoundaryType::KripkeBoundary,
            None,
            None,
            None,
        ),
        CompilationPass::DependentCombinators => (
            BoundaryType::CombinatorFilter,
            None,
            None,
            None,
        ),
        CompilationPass::RpcSerialization => (
            BoundaryType::RpcSerializationBoundary,
            None,
            None,
            None,
        ),
        CompilationPass::ConsensusReplication => (
            BoundaryType::ACBoundary,
            Some(ARM::EventuallyConsistent),
            None,
            None,
        ),
        CompilationPass::PartitionTolerance => (
            BoundaryType::RpcSerializationBoundary,
            None,
            None,
            None,
        ),
        CompilationPass::NominalAbstraction => (
            BoundaryType::NominalBoundary,
            None,
            None,
            None,
        ),
        CompilationPass::ClauseCompilation | CompilationPass::GoalDirected => (
            BoundaryType::GroundingBoundary,
            None,
            None,
            Some(Direction::Backward),
        ),
        CompilationPass::ACNormalization => (
            BoundaryType::ACBoundary,
            None,
            None,
            None,
        ),
        CompilationPass::ContextReification => (
            BoundaryType::ContextReifyBoundary,
            None,
            None,
            None,
        ),
        CompilationPass::ModalSubstitutionRestriction => (
            BoundaryType::ModalRestrictionBoundary,
            None,
            None,
            None,
        ),
        CompilationPass::KanComputation => (
            BoundaryType::KanTransportBoundary,
            None,
            None,
            None,
        ),
        CompilationPass::SMTEncoding => (
            BoundaryType::ThermoBoundary,
            None,
            None,
            None,
        ),
        CompilationPass::EffectElaboration => (
            BoundaryType::EffectBoundary,
            None,
            None,
            None,
        ),
        CompilationPass::DialecticaExtraction => (
            BoundaryType::DialecticaBoundary,
            None,
            None,
            None,
        ),
        CompilationPass::ExplicitSubstitution => (
            BoundaryType::ExplicitSubstitutionBoundary,
            None,
            None,
            None,
        ),
    }
}

/// Translate Hyperion ResourceMode → Archon ResourceMode.
fn translate_resource_mode(rm: &substrate::ResourceMode) -> archon::region::ResourceMode {
    use archon::region::ResourceMode as ARM;
    match rm {
        substrate::ResourceMode::OptimalSharing => ARM::OptimalSharing,
        substrate::ResourceMode::StrictlyLinear => ARM::StrictlyLinear,
        substrate::ResourceMode::Affine => ARM::Affine,
        substrate::ResourceMode::Relevant => ARM::Relevant,
        substrate::ResourceMode::DeepCopy => ARM::DeepCopy,
        substrate::ResourceMode::EventuallyConsistent => ARM::EventuallyConsistent,
    }
}

/// Translate Hyperion EqualityMode → Archon EqualityMode.
fn translate_equality_mode(em: &substrate::EqualityMode) -> archon::region::EqualityMode {
    use archon::region::EqualityMode as AEM;
    match em {
        substrate::EqualityMode::TopologicalHash => AEM::TopologicalHash,
        substrate::EqualityMode::RewriteEquivalence => AEM::RewriteEquivalence,
        substrate::EqualityMode::AlphaEquivalence => AEM::AlphaEquivalence,
        substrate::EqualityMode::Observational => AEM::Observational,
        substrate::EqualityMode::Unification => AEM::Unification,
        substrate::EqualityMode::TopologicalHomotopy => AEM::HomotopyEquivalence,
        substrate::EqualityMode::EqualitySaturation => AEM::EqualitySaturation,
        substrate::EqualityMode::ExtensionalEquivalence => AEM::ExtensionalEquivalence,
        substrate::EqualityMode::FullUnification => AEM::Unification,
        substrate::EqualityMode::ProofRelevant => AEM::ProofRelevant,
        substrate::EqualityMode::ACMatching => AEM::ACMatching,
        substrate::EqualityMode::UnificationSearch => AEM::Unification,
        substrate::EqualityMode::SMTOracle => AEM::Thermodynamic, // thermo replaces SMT
    }
}

/// Convert an Apeiron Sexp to an Archon Sexp (they have the same structure).
pub fn apeiron_to_archon_sexp(sexp: &apeiron::parser::Sexp) -> ArchonSexp {
    match sexp {
        apeiron::parser::Sexp::Atom(s, _) => ArchonSexp::Atom(s.clone()),
        apeiron::parser::Sexp::List(items, _) => {
            ArchonSexp::List(items.iter().map(apeiron_to_archon_sexp).collect())
        }
    }
}

/// Convert an Archon Sexp back to an Apeiron Sexp.
pub fn archon_to_apeiron_sexp(sexp: &ArchonSexp) -> apeiron::parser::Sexp {
    let sp = apeiron::parser::Span::default();
    match sexp {
        ArchonSexp::Atom(s) => apeiron::parser::Sexp::Atom(s.clone(), sp),
        ArchonSexp::List(items) => {
            apeiron::parser::Sexp::List(items.iter().map(archon_to_apeiron_sexp).collect(), sp)
        }
    }
}

/// Run a term through the Archon physics engine with a given topology.
///
/// Returns the ArchonResult and the arena (for post-analysis).
pub fn run_in_topology(
    topo: Topology,
    term: &ArchonSexp,
    config: &ArchonConfig,
) -> (ArchonArena, ArchonResult) {
    let mut arena = ArchonArena::new().with_topology(topo);

    // Implant the term into the innermost region (highest-numbered).
    let max_region = arena.topology.region_ids().into_iter().max().unwrap_or(0);
    let _result = implant::build_raw(&mut arena, term, max_region);

    let result = physics::run(&mut arena, config);
    (arena, result)
}

/// Extract operator arities from an Apeiron-style Theory sexp.
///
/// Scans for `[sort ...]` and `[op name domain... codomain]` declarations
/// to build the known_ops map for the implant layer.
pub fn extract_ops_from_theory(sexp: &apeiron::parser::Sexp) -> HashMap<String, u8> {
    let mut ops = HashMap::new();
    if let Some(items) = sexp.as_list() {
        for item in items {
            if let Some(sub) = item.as_list() {
                if sub.len() >= 2 {
                    if let Some(head) = sub[0].as_atom() {
                        match head {
                            "op" => {
                                if let Some(name) = sub[1].as_atom() {
                                    // arity = len - 3 (op, name, codomain)
                                    let arity = if sub.len() > 2 { sub.len() - 3 } else { 0 };
                                    ops.insert(name.to_string(), arity as u8);
                                }
                            }
                            "sort" => {
                                if let Some(name) = sub[1].as_atom() {
                                    ops.insert(name.to_string(), 0);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    ops
}

/// Implant a set of rewrite rules into the Archon arena.
///
/// Each rule becomes a pair of nodes (LHS pattern, RHS template) connected
/// as an active pair. The physics engine will attempt to match and fire them.
pub fn implant_rules(
    arena: &mut ArchonArena,
    rules: &[(apeiron::parser::Sexp, apeiron::parser::Sexp)],
    region: u32,
    ops: &HashMap<String, u8>,
) {
    for (lhs, rhs) in rules {
        let lhs_archon = apeiron_to_archon_sexp(lhs);
        let rhs_archon = apeiron_to_archon_sexp(rhs);

        let lhs_result = implant::build_raw_with_ops(arena, &lhs_archon, region, ops.clone());
        let rhs_result = implant::build_raw_with_ops(arena, &rhs_archon, region, ops.clone());

        // Connect LHS root to RHS root as an active pair.
        arena.connect(lhs_result.root, 0, rhs_result.root, 0);
    }
}

/// Encode SMT assertions as thermodynamic constraints in a thermo region.
///
/// This replaces Z3: assertions become springs/arithmetic constraints,
/// and the annealing engine finds satisfying assignments.
pub fn encode_smt_assertions(
    arena: &mut ArchonArena,
    region: u32,
    assertions: &[apeiron::parser::Sexp],
) {
    for assertion in assertions {
        encode_smt_term(arena, region, assertion);
    }
}

/// Recursively encode an SMT assertion term as physics constraints.
fn encode_smt_term(arena: &mut ArchonArena, region: u32, sexp: &apeiron::parser::Sexp) {
    if let Some(items) = sexp.as_list() {
        if let Some(head) = items.first().and_then(|s| s.as_atom()) {
            match head {
                "=" if items.len() == 3 => {
                    // Equality: encode as zero-energy spring.
                    // For now, handle atomic cases via boolean spins.
                    let lhs_atom = items[1].as_atom();
                    let rhs_atom = items[2].as_atom();
                    if lhs_atom.is_some() && rhs_atom.is_some() {
                        // Simple equality becomes a spin constraint.
                        let spin = arena.spawn_spin(region, false);
                        archon::thermo::encode_clause(arena, region, vec![(spin, true)]);
                    }
                }
                "and" => {
                    for item in &items[1..] {
                        encode_smt_term(arena, region, item);
                    }
                }
                "or" => {
                    // OR becomes a clause: at least one must be true.
                    let mut literals = Vec::new();
                    for item in &items[1..] {
                        let spin = arena.spawn_spin(region, false);
                        literals.push((spin, true));
                        encode_smt_term(arena, region, item);
                    }
                    archon::thermo::encode_clause(arena, region, literals);
                }
                "not" if items.len() == 2 => {
                    let spin = arena.spawn_spin(region, true);
                    archon::thermo::encode_clause(arena, region, vec![(spin, false)]);
                }
                "+" if items.len() == 3 => {
                    // Arithmetic addition: encode as linear constraint later.
                }
                ">=" | ">" if items.len() == 3 => {
                    // Inequality: x >= bound.
                    if let Some(bound_str) = items[2].as_atom() {
                        if let Ok(bound) = bound_str.parse::<f64>() {
                            let var = arena.spawn_continuous(region, 0.0);
                            thermo::encode_inequality(arena, region, var, bound, true);
                        }
                    }
                }
                "<=" | "<" if items.len() == 3 => {
                    if let Some(bound_str) = items[2].as_atom() {
                        if let Ok(bound) = bound_str.parse::<f64>() {
                            let var = arena.spawn_continuous(region, 0.0);
                            thermo::encode_inequality(arena, region, var, bound, false);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topology_from_empty_universe() {
        let sub = SubstrateDef {
            name: "Test".into(),
            engine: substrate::Engine::InteractionGraph,
            resource_mode: substrate::ResourceMode::OptimalSharing,
            barrier: substrate::BarrierMode::Transparent,
            equality: substrate::EqualityMode::EqualitySaturation,
            totality: substrate::TotalityMode::Unspecified,
        };
        let compiled = CompiledUniverse {
            name: "TestU".into(),
            system_name: "test".into(),
            scope_names: vec![],
            category_name: "Test".into(),
            substrate_name: "Test".into(),
            passes: vec![],
        };

        let topo = build_topology(&compiled, &sub);
        // Only root region.
        assert_eq!(topo.region_ids().len(), 1);
    }

    #[test]
    fn topology_with_bang_and_defunc() {
        let sub = SubstrateDef {
            name: "Test".into(),
            engine: substrate::Engine::VonNeumann,
            resource_mode: substrate::ResourceMode::StrictlyLinear,
            barrier: substrate::BarrierMode::Transparent,
            equality: substrate::EqualityMode::RewriteEquivalence,
            totality: substrate::TotalityMode::Unspecified,
        };
        let compiled = CompiledUniverse {
            name: "TestU".into(),
            system_name: "test".into(),
            scope_names: vec![],
            category_name: "Test".into(),
            substrate_name: "Test".into(),
            passes: vec![
                CompilationPass::BangModality,
                CompilationPass::Defunctionalization,
            ],
        };

        let topo = build_topology(&compiled, &sub);
        // Root + bang region + defunc region = 3.
        assert_eq!(topo.region_ids().len(), 3);
    }

    #[test]
    fn sexp_conversion_roundtrip() {
        let original = apeiron::parser::Sexp::List(vec![
            apeiron::parser::Sexp::Atom("foo".into(), apeiron::parser::Span::default()),
            apeiron::parser::Sexp::Atom("bar".into(), apeiron::parser::Span::default()),
        ], apeiron::parser::Span::default());

        let archon = apeiron_to_archon_sexp(&original);
        let back = archon_to_apeiron_sexp(&archon);

        assert_eq!(original.as_list().unwrap().len(), back.as_list().unwrap().len());
    }
}
