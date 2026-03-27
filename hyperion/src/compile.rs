use apeiron::parser::{Sexp, Span};

use crate::category::{CategoricalStructure, CategoryDef};
use crate::error::Result;
use crate::laws;
use crate::substrate::{BarrierMode, Engine, EqualityMode, ResourceMode, SubstrateDef, TotalityMode};
use crate::universe::{system_name_for, CompilationPass, CompiledUniverse};

/// Compile a Category + Substrate into a CompiledUniverse.
///
/// This is the heart of Hyperion: it verifies compatibility, then generates
/// the Apeiron SystemConfig that hosts the categorical structure.
///
/// When the category and substrate are not natively compatible, the compiler
/// inserts compilation passes that bridge the gap via well-known theoretical
/// constructions (bang modality, defunctionalization, Kripke threading, etc.).
pub fn compile_universe(
    universe_name: &str,
    cat: &CategoryDef,
    sub: &SubstrateDef,
) -> Result<CompiledUniverse> {
    // Step 1: Compatibility analysis — returns required bridging passes
    let passes = check_compatibility(cat, sub)?;

    // Step 2: Collect scope names from Context declarations
    let scope_names: Vec<String> = cat
        .structure
        .iter()
        .filter_map(|s| {
            if let CategoricalStructure::ContextDecl { name } = s {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();

    let sys_name = system_name_for(&cat.name, &sub.name);

    Ok(CompiledUniverse {
        name: universe_name.to_string(),
        system_name: sys_name,
        scope_names,
        category_name: cat.name.clone(),
        substrate_name: sub.name.clone(),
        passes,
    })
}

/// Check if a substrate uses a first-order engine (VonNeumann, NetworkRpc, SystemIO, or Compiler).
/// These engines bypass Apeiron and use direct rewrite-rule theories.
pub fn is_first_order_engine(sub: &SubstrateDef) -> bool {
    matches!(sub.engine, Engine::VonNeumann | Engine::NetworkRpc | Engine::SystemIO | Engine::ConcurrentGraph | Engine::ConcurrentIO | Engine::Compiler | Engine::LogicProgramming | Engine::SMTAssisted | Engine::ArchonPhysics)
}

/// Check if a substrate uses the Archon physics engine.
pub fn is_archon(sub: &SubstrateDef) -> bool {
    sub.engine == Engine::ArchonPhysics
}

/// Check if a substrate uses the Von Neumann engine.
pub fn is_von_neumann(sub: &SubstrateDef) -> bool {
    sub.engine == Engine::VonNeumann
}

/// Check if a substrate uses the SystemIO engine.
pub fn is_system_io(sub: &SubstrateDef) -> bool {
    matches!(sub.engine, Engine::SystemIO | Engine::ConcurrentIO)
}

/// Analyze compatibility between category and substrate, returning any
/// compilation passes needed to bridge gaps.
///
/// Instead of rejecting incompatible pairs outright, Hyperion inserts
/// well-known theoretical constructions as compilation passes:
///
/// - **BangModality** (Girard): strictly-linear + Exponential → `!A ⊸ B`
/// - **NominalAbstraction** (Gabbay-Pitts): nominal-scoping + Exponential → name-abstraction
/// - **Defunctionalization** (Reynolds): von-neumann/cellular-automaton + Exponential → first-order ADT
/// - **TensorSerialization**: sequential engine + TensorProduct → left-to-right evaluation
/// - **KripkeWorldThreading**: transparent/one-way-valve barrier + Modal → explicit world parameters
/// - **DependentCombinators**: non-lambda engine + topological-homotopy → dependent SKI
fn check_compatibility(cat: &CategoryDef, sub: &SubstrateDef) -> Result<Vec<CompilationPass>> {
    let mut passes = Vec::new();

    // Archon physics engine: all passes are physicalized as membrane boundaries.
    // We still compute the pass list (for diagnostics and topology construction),
    // but the engine handles everything natively.
    let is_archon = sub.engine == Engine::ArchonPhysics;

    let needs_exponential = cat.has_exponential() || cat.has_evaluator();
    let needs_modal = cat.has_modal_operator() || cat.has_context();

    let supports_lambda = matches!(
        sub.engine,
        Engine::InteractionGraph | Engine::TermTree | Engine::AbstractMachine
        | Engine::ConcurrentGraph | Engine::Compiler
    );

    let supports_tensor = matches!(
        sub.engine,
        Engine::InteractionGraph | Engine::SymmetricMonoidal
        | Engine::ReversibleGraph | Engine::ConcurrentGraph | Engine::ConcurrentIO
    );

    let supports_scopes = matches!(
        sub.barrier,
        BarrierMode::ContextualMembranes | BarrierMode::Cryptographic
        | BarrierMode::NominalScoping | BarrierMode::NetworkPartition
    );

    // --- Exponential bridging ---

    // StrictlyLinear + Exponential → Bang modality (Girard's !)
    if sub.resource_mode == ResourceMode::StrictlyLinear && cat.has_exponential() {
        passes.push(CompilationPass::BangModality);
    }

    // NominalScoping + Exponential → Nominal abstraction (Gabbay-Pitts)
    if matches!(sub.barrier, BarrierMode::NominalScoping) && cat.has_exponential() {
        passes.push(CompilationPass::NominalAbstraction);
    }

    // Non-lambda engine + Exponential → Defunctionalization (Reynolds)
    // Von Neumann and CellularAutomaton are first-order; SymmetricMonoidal lacks closures.
    // ReversibleGraph also lacks lambda — but reversible defunctionalization is sound.
    if needs_exponential && !supports_lambda {
        passes.push(CompilationPass::Defunctionalization);
    }

    // --- TensorProduct bridging ---

    // Concurrent engine + TensorProduct + StrictlyLinear → Parallel tensor (proof-carrying)
    // The strictly-linear resource mode forbids the diagonal map (A → A ⊗ A),
    // so tensor factors are guaranteed to have disjoint free variables.
    // This is verified at kompile time via AST-level variable partitioning.
    if cat.has_tensor() && supports_tensor
        && matches!(sub.engine, Engine::ConcurrentGraph | Engine::ConcurrentIO)
        && matches!(sub.resource_mode, ResourceMode::StrictlyLinear)
    {
        passes.push(CompilationPass::ParallelTensorProof);
    }

    // Sequential engine + TensorProduct → Serialization (fallback for non-parallel)
    if cat.has_tensor() && !supports_tensor {
        passes.push(CompilationPass::TensorSerialization);
    }

    // Von Neumann + TensorProduct is also covered by TensorSerialization above.

    // --- Modal bridging ---

    // Non-isolating barrier + Modal → Kripke world threading
    if needs_modal && !supports_scopes {
        passes.push(CompilationPass::KripkeWorldThreading);
    }

    // Von Neumann + Modal is covered by both Kripke threading and (if exponential)
    // defunctionalization — both passes compose.

    // --- HoTT bridging ---

    // PathType + Evaluator on non-lambda engine → needs Defunctionalization (already covered above)

    // TopologicalHomotopy on non-lambda engine → Dependent combinators (extremely expensive)
    if sub.equality == EqualityMode::TopologicalHomotopy && !supports_lambda {
        passes.push(CompilationPass::DependentCombinators);
    }

    // --- HOAS bridging (Phase 1) ---

    // HOAS on a first-order engine → explicit substitution calculus
    if cat.has_hoas() && !supports_lambda {
        passes.push(CompilationPass::HOASDefunctionalization);
    }

    // Equality-saturation + binders (Exponential/HOAS) without nominal scoping →
    // explicit substitution calculus (λσ) so the e-graph can rewrite under binders safely
    if matches!(sub.equality, EqualityMode::EqualitySaturation)
        && (cat.has_exponential() || cat.has_hoas())
        && !matches!(sub.barrier, BarrierMode::NominalScoping)
    {
        passes.push(CompilationPass::ExplicitSubstitution);
    }

    // Logic programming engine + Exponential → clause compilation
    if matches!(sub.engine, Engine::LogicProgramming) && needs_exponential {
        passes.push(CompilationPass::ClauseCompilation);
    }

    // LCF tactic combinators on a forward-only engine → goal-directed compilation
    if cat.has_tactic_combinators() && !matches!(sub.engine, Engine::LogicProgramming) {
        passes.push(CompilationPass::GoalDirected);
    }

    // --- AC-matching bridging (Phase 1.5) ---

    // AC-matching or StateConfiguration on non-AC engine → normalization pass
    if (matches!(sub.equality, EqualityMode::ACMatching) || cat.has_state_configuration())
        && !matches!(sub.engine, Engine::InteractionGraph | Engine::SymmetricMonoidal)
    {
        passes.push(CompilationPass::ACNormalization);
    }

    // --- Contextual + Cohesive bridging (Phase 2) ---

    // Contextual types on transparent barrier → reify contexts as data
    if cat.has_contextual_type() && matches!(sub.barrier, BarrierMode::Transparent) {
        passes.push(CompilationPass::ContextReification);
    }

    // Cohesive modalities on non-restricting barrier → substitution guards
    if cat.has_cohesive_modality() && !matches!(sub.barrier, BarrierMode::ContextualMembranes | BarrierMode::Cryptographic) {
        passes.push(CompilationPass::ModalSubstitutionRestriction);
    }

    // --- Cubical bridging (Phase 3) ---

    // Kan operations need computation rules on rewriting substrates
    if cat.has_kan_ops() && matches!(sub.equality, EqualityMode::RewriteEquivalence | EqualityMode::Observational) {
        passes.push(CompilationPass::KanComputation);
    }

    // --- SMT bridging (Phase 4) ---

    // SMT oracle → encoding pass (terms must be serialized to SMT-LIB2)
    if matches!(sub.equality, EqualityMode::SMTOracle) {
        passes.push(CompilationPass::SMTEncoding);
    }

    // Effect grading on substrates without native effect tracking → elaboration
    if cat.has_effect_grading() && matches!(sub.barrier, BarrierMode::Transparent) {
        passes.push(CompilationPass::EffectElaboration);
    }

    // --- Distributed systems bridging ---

    // NetworkPartition barrier → RPC serialization for any data crossing the partition
    if matches!(sub.barrier, BarrierMode::NetworkPartition) {
        passes.push(CompilationPass::RpcSerialization);
    }

    // EventuallyConsistent resource mode → consensus replication
    // Operations must be commutative (CRDT) or totally ordered (Raft/Paxos)
    if matches!(sub.resource_mode, ResourceMode::EventuallyConsistent) {
        passes.push(CompilationPass::ConsensusReplication);
    }

    // Modal operators on network-partition → partition tolerance (CAP theorem)
    // □A becomes an availability/consistency trade-off point
    if needs_modal && matches!(sub.barrier, BarrierMode::NetworkPartition) {
        passes.push(CompilationPass::PartitionTolerance);
    }

    Ok(passes)
}

/// Determine the Apeiron binding mode from category + substrate + compilation passes.
fn binding_mode(cat: &CategoryDef, sub: &SubstrateDef, passes: &[CompilationPass]) -> &'static str {
    // HOAS binding: meta-level substitution handles object-level binding
    if cat.has_hoas() && !passes.contains(&CompilationPass::HOASDefunctionalization) {
        return "implicit"; // HOAS uses implicit binding (meta-level substitution)
    }
    // Cohesive: variable discreteness tracking (shape/flat/sharp)
    if cat.has_cohesive_modality() {
        return "contextual"; // Cohesive modalities restrict variable substitution
    }
    // Nominal abstraction pass overrides to nominal even if the barrier isn't NominalScoping
    if matches!(sub.barrier, BarrierMode::NominalScoping)
        || passes.contains(&CompilationPass::NominalAbstraction)
    {
        "nominal"
    } else if cat.has_contextual_type() && matches!(sub.barrier, BarrierMode::ContextualMembranes) {
        "contextual"
    } else if cat.has_modal_operator() && matches!(sub.barrier, BarrierMode::ContextualMembranes) {
        "contextual"
    } else if passes.contains(&CompilationPass::Defunctionalization)
        || passes.contains(&CompilationPass::HOASDefunctionalization)
    {
        // Defunctionalized code is first-order — use exposed binding
        "exposed"
    } else if matches!(sub.resource_mode, ResourceMode::StrictlyLinear) {
        "linear-explicit"
    } else if matches!(sub.resource_mode, ResourceMode::Affine) {
        "linear-explicit"
    } else if cat.has_exponential() && matches!(sub.engine, Engine::InteractionGraph) {
        "implicit"
    } else if matches!(sub.engine, Engine::TermTree) {
        "exposed"
    } else {
        "implicit"
    }
}

/// Determine the Apeiron check modes from substrate.
fn check_modes(sub: &SubstrateDef) -> Vec<&'static str> {
    let mut modes = match sub.equality {
        EqualityMode::RewriteEquivalence => vec!["rewriting", "beta-reduction"],
        EqualityMode::TopologicalHash => vec!["oracle"],
        EqualityMode::Unification => vec!["pattern-unification"],
        EqualityMode::AlphaEquivalence => vec!["beta-reduction"],
        EqualityMode::Observational => vec!["rewriting", "beta-reduction"],
        EqualityMode::TopologicalHomotopy => vec!["rewriting", "beta-reduction", "eta"],
        EqualityMode::EqualitySaturation => {
            vec!["rewriting", "beta-reduction", "equality-saturation"]
        }
        EqualityMode::ExtensionalEquivalence => vec!["rewriting", "beta-reduction", "extensional"],
        EqualityMode::FullUnification => vec!["unification"],
        EqualityMode::ProofRelevant => {
            vec!["rewriting", "beta-reduction", "equality-saturation"]
        }
        EqualityMode::ACMatching => {
            // AC normalization (flatten+sort) before rewriting; Apeiron sees rewriting
            vec!["rewriting", "beta-reduction"]
        }
        EqualityMode::UnificationSearch => {
            // Backward-chaining = unification-driven search; Apeiron sees unification
            vec!["unification", "pattern-unification"]
        }
        EqualityMode::SMTOracle => {
            // Terminal SMT decision procedure; Apeiron sees oracle
            vec!["oracle"]
        }
    };

    // Engine-driven check modes (appended to equality-driven modes)
    if matches!(sub.engine, Engine::ReversibleGraph) {
        modes.push("reversible");
    }
    if matches!(sub.engine, Engine::ConcurrentGraph) {
        modes.push("confluent-race");
    }
    // LogicProgramming and SMTAssisted add unification/oracle at the Apeiron level;
    // the actual backward-chaining/SMT-cooperation is handled by Hyperion's compilation passes
    if matches!(sub.engine, Engine::LogicProgramming) {
        modes.push("unification");
    }
    if matches!(sub.engine, Engine::SMTAssisted) {
        modes.push("oracle");
    }

    modes
}

/// Generate categorical laws for a category.
pub fn generate_category_laws(cat: &CategoryDef) -> Vec<laws::CategoricalLaw> {
    laws::generate_laws(cat)
}

/// Build a [Proofs] sexp for law checking.
pub fn build_law_proofs_sexp(
    theory_name: &str,
    cat: &CategoryDef,
) -> Option<Sexp> {
    let category_laws = laws::generate_laws(cat);
    if category_laws.is_empty() {
        return None;
    }
    // Use first object as witness sort
    let witness_sort = cat.objects.first().map(|o| o.name.as_str());
    laws::build_law_proofs(theory_name, &category_laws, witness_sort)
}

/// Generate an Apeiron [Signature ...] S-expression with typed operator declarations.
pub fn emit_signature_sexp(cat: &CategoryDef) -> Sexp {
    let sp = Span::default();
    let mut items: Vec<Sexp> = Vec::new();

    let sig_name = format!("__hyp_sig_{}", cat.name);
    items.push(Sexp::Atom("Signature".into(), sp));
    items.push(Sexp::Atom(sig_name, sp));

    // [sort ObjName] for each object
    for obj in &cat.objects {
        items.push(Sexp::List(
            vec![
                Sexp::Atom("sort".into(), sp),
                Sexp::Atom(obj.name.clone(), sp),
            ],
            sp,
        ));
    }

    // [op name domain... codomain] for each morphism (typed!)
    for morph in &cat.morphisms {
        let mut op_items: Vec<Sexp> = Vec::new();
        op_items.push(Sexp::Atom("op".into(), sp));
        op_items.push(Sexp::Atom(morph.name.clone(), sp));
        for d in &morph.domain {
            op_items.push(Sexp::Atom(d.clone(), sp));
        }
        op_items.push(Sexp::Atom(morph.codomain.clone(), sp));
        items.push(Sexp::List(op_items, sp));
    }

    Sexp::List(items, sp)
}

/// Inject operator declarations for structure-provided names, skipping duplicates.
fn inject_ops(syntax_items: &mut Vec<Sexp>, names: &[&String], cat: &CategoryDef, sp: Span) {
    for op_name in names {
        let already = cat.morphisms.iter().any(|m| m.name == **op_name)
            || syntax_items.iter().any(|s| {
                s.as_list()
                    .and_then(|l| l.get(1))
                    .and_then(|s| s.as_atom())
                    .map(|a| a == *op_name)
                    .unwrap_or(false)
            });
        if !already {
            syntax_items.push(Sexp::List(
                vec![
                    Sexp::Atom("op".into(), sp),
                    Sexp::Atom((*op_name).clone(), sp),
                ],
                sp,
            ));
        }
    }
}

/// Generate the Apeiron [System ...] S-expression for a compiled universe.
pub fn emit_system_sexp(
    cat: &CategoryDef,
    sub: &SubstrateDef,
    compiled: &CompiledUniverse,
    signature_name: Option<&str>,
) -> Sexp {
    let sp = Span::default();
    let mut system_items: Vec<Sexp> = Vec::new();

    // [System __hyp_Cat_Sub ...]
    system_items.push(Sexp::Atom("System".into(), sp));
    system_items.push(Sexp::Atom(compiled.system_name.clone(), sp));

    // :signature ref (if Signature was registered)
    if let Some(sig) = signature_name {
        system_items.push(Sexp::Atom(":signature".into(), sp));
        system_items.push(Sexp::Atom(sig.to_string(), sp));
    }

    // [@syntax ...] block
    let mut syntax_items: Vec<Sexp> = Vec::new();
    syntax_items.push(Sexp::Atom("@syntax".into(), sp));

    if signature_name.is_none() {
        // Sorts from objects (only if no signature — signature carries them)
        for obj in &cat.objects {
            syntax_items.push(Sexp::List(
                vec![
                    Sexp::Atom("sort".into(), sp),
                    Sexp::Atom(obj.name.clone(), sp),
                ],
                sp,
            ));
        }

        // Operators from morphisms (only if no signature)
        for morph in &cat.morphisms {
            let mut op_items = vec![
                Sexp::Atom("op".into(), sp),
                Sexp::Atom(morph.name.clone(), sp),
            ];
            // Emit :arity for arity enforcement
            if !morph.domain.is_empty() {
                op_items.push(Sexp::Atom(":arity".into(), sp));
                op_items.push(Sexp::Atom(morph.domain.len().to_string(), sp));
            }
            syntax_items.push(Sexp::List(op_items, sp));
        }
    }

    // Judgment forms as operators
    for j in &cat.judgments {
        syntax_items.push(Sexp::List(
            vec![
                Sexp::Atom("op".into(), sp),
                Sexp::Atom(j.name.clone(), sp),
            ],
            sp,
        ));
    }

    // Operators from structure
    for s in &cat.structure {
        match s {
            CategoricalStructure::Exponential { name, .. } => {
                syntax_items.push(Sexp::List(
                    vec![
                        Sexp::Atom("op".into(), sp),
                        Sexp::Atom(name.clone(), sp),
                    ],
                    sp,
                ));
            }
            CategoricalStructure::Evaluator { name } => {
                // Only add if not already a morphism with the same name
                let already = cat.morphisms.iter().any(|m| m.name == *name);
                if !already {
                    syntax_items.push(Sexp::List(
                        vec![
                            Sexp::Atom("op".into(), sp),
                            Sexp::Atom(name.clone(), sp),
                        ],
                        sp,
                    ));
                }
            }
            CategoricalStructure::ModalOperator { name } => {
                syntax_items.push(Sexp::List(
                    vec![
                        Sexp::Atom("op".into(), sp),
                        Sexp::Atom(name.clone(), sp),
                    ],
                    sp,
                ));
            }
            CategoricalStructure::TensorProduct { name } => {
                syntax_items.push(Sexp::List(
                    vec![
                        Sexp::Atom("op".into(), sp),
                        Sexp::Atom(name.clone(), sp),
                    ],
                    sp,
                ));
            }
            CategoricalStructure::Unit { name } => {
                syntax_items.push(Sexp::List(
                    vec![
                        Sexp::Atom("op".into(), sp),
                        Sexp::Atom(name.clone(), sp),
                    ],
                    sp,
                ));
            }
            CategoricalStructure::ContextDecl { .. } => {
                // Contexts become Scope declarations in Theory, not ops
            }
            CategoricalStructure::Preorder { relation: _ } => {
                // Inject `true` op for reflexivity result (if not already present)
                let true_name = "true";
                let already = cat.morphisms.iter().any(|m| m.name == true_name)
                    || syntax_items.iter().any(|s| {
                        s.as_list()
                            .and_then(|l| l.get(1))
                            .and_then(|s| s.as_atom())
                            .map(|a| a == true_name)
                            .unwrap_or(false)
                    });
                if !already {
                    syntax_items.push(Sexp::List(
                        vec![
                            Sexp::Atom("op".into(), sp),
                            Sexp::Atom(true_name.into(), sp),
                        ],
                        sp,
                    ));
                }
            }
            CategoricalStructure::PathType { refl, concat, inv, ap } => {
                // Inject path algebra ops (only if not already a morphism)
                for op_name in [refl, concat, inv, ap] {
                    let already = cat.morphisms.iter().any(|m| m.name == *op_name)
                        || syntax_items.iter().any(|s| {
                            s.as_list()
                                .and_then(|l| l.get(1))
                                .and_then(|s| s.as_atom())
                                .map(|a| a == op_name)
                                .unwrap_or(false)
                        });
                    if !already {
                        syntax_items.push(Sexp::List(
                            vec![
                                Sexp::Atom("op".into(), sp),
                                Sexp::Atom(op_name.clone(), sp),
                            ],
                            sp,
                        ));
                    }
                }
            }
            CategoricalStructure::JType { j_elim, transport } => {
                for op_name in [j_elim, transport] {
                    let already = cat.morphisms.iter().any(|m| m.name == *op_name)
                        || syntax_items.iter().any(|s| {
                            s.as_list()
                                .and_then(|l| l.get(1))
                                .and_then(|s| s.as_atom())
                                .map(|a| a == op_name)
                                .unwrap_or(false)
                        });
                    if !already {
                        syntax_items.push(Sexp::List(
                            vec![
                                Sexp::Atom("op".into(), sp),
                                Sexp::Atom(op_name.clone(), sp),
                            ],
                            sp,
                        ));
                    }
                }
            }
            CategoricalStructure::PartialElement { hcomp, coe } => {
                for op_name in [hcomp, coe] {
                    let already = cat.morphisms.iter().any(|m| m.name == *op_name)
                        || syntax_items.iter().any(|s| {
                            s.as_list()
                                .and_then(|l| l.get(1))
                                .and_then(|s| s.as_atom())
                                .map(|a| a == op_name)
                                .unwrap_or(false)
                        });
                    if !already {
                        syntax_items.push(Sexp::List(
                            vec![
                                Sexp::Atom("op".into(), sp),
                                Sexp::Atom(op_name.clone(), sp),
                            ],
                            sp,
                        ));
                    }
                }
            }
            CategoricalStructure::IntervalSort { interval, i0, i1 } => {
                inject_ops(&mut syntax_items, &[interval, i0, i1], cat, sp);
            }
            CategoricalStructure::HOASBinding { binder, .. } => {
                inject_ops(&mut syntax_items, &[binder], cat, sp);
            }
            CategoricalStructure::TacticCombinators { then, orelse, repeat, try_tac, focus } => {
                inject_ops(&mut syntax_items, &[then, orelse, repeat, try_tac, focus], cat, sp);
            }
            CategoricalStructure::StateConfiguration { cell_sort, merge } => {
                inject_ops(&mut syntax_items, &[cell_sort, merge], cat, sp);
            }
            CategoricalStructure::ContextualType { context_sort, term_sort } => {
                inject_ops(&mut syntax_items, &[context_sort, term_sort], cat, sp);
            }
            CategoricalStructure::CohesiveModality { shape, flat, sharp } => {
                inject_ops(&mut syntax_items, &[shape, flat, sharp], cat, sp);
            }
            CategoricalStructure::FaceLattice { meet, join, neg } => {
                inject_ops(&mut syntax_items, &[meet, join, neg], cat, sp);
            }
            CategoricalStructure::GlueType { glue, unglue } => {
                inject_ops(&mut syntax_items, &[glue, unglue], cat, sp);
            }
            CategoricalStructure::KanOps { comp, fill, hfill } => {
                inject_ops(&mut syntax_items, &[comp, fill, hfill], cat, sp);
            }
            CategoricalStructure::EffectGrading { effect_lattice, pure, total } => {
                inject_ops(&mut syntax_items, &[effect_lattice, pure, total], cat, sp);
            }
        }
    }

    system_items.push(Sexp::List(syntax_items, sp));

    // [@binding ...] block
    let bmode = binding_mode(cat, sub, &compiled.passes);
    system_items.push(Sexp::List(
        vec![
            Sexp::Atom("@binding".into(), sp),
            Sexp::Atom(bmode.into(), sp),
        ],
        sp,
    ));

    // [@check ...] block
    let cmodes = check_modes(sub);
    let mut check_items: Vec<Sexp> = Vec::new();
    check_items.push(Sexp::Atom("@check".into(), sp));
    for mode in cmodes {
        check_items.push(Sexp::Atom(mode.into(), sp));
    }
    system_items.push(Sexp::List(check_items, sp));

    // [@compilation-passes ...] block — bridging passes needed for this universe
    if !compiled.passes.is_empty() {
        let mut pass_items = vec![Sexp::Atom("@compilation-passes".into(), sp)];
        for pass in &compiled.passes {
            pass_items.push(Sexp::List(
                vec![
                    Sexp::Atom(pass.name().into(), sp),
                    Sexp::Atom(pass.description().into(), sp),
                ],
                sp,
            ));
        }
        system_items.push(Sexp::List(pass_items, sp));
    }

    // [@barrier-ops ...] block — inform Apeiron which ops are scope-significant
    let modal_ops: Vec<&str> = cat.structure.iter().filter_map(|s| match s {
        CategoricalStructure::ModalOperator { name } => Some(name.as_str()),
        _ => None,
    }).collect();
    if !modal_ops.is_empty() && sub.barrier == BarrierMode::ContextualMembranes {
        let mut barrier_items = vec![Sexp::Atom("@barrier-ops".into(), sp)];
        for op in &modal_ops {
            barrier_items.push(Sexp::Atom((*op).to_string(), sp));
        }
        system_items.push(Sexp::List(barrier_items, sp));
    }

    Sexp::List(system_items, sp)
}

/// Generate the morphism name for a functor applied to a specific category.
pub fn morphism_name_for(functor_name: &str, category_name: &str) -> String {
    format!("__fun_{}_{}", functor_name, category_name)
}

/// Generate an Apeiron `[AutoMorphism name source target [Map a b] ...]` S-expression.
pub fn emit_morphism_sexp(
    name: &str,
    source_system: &str,
    target_system: &str,
    op_maps: &[(String, String)],
) -> Sexp {
    let sp = Span::default();
    let mut items: Vec<Sexp> = Vec::new();

    items.push(Sexp::Atom("AutoMorphism".into(), sp));
    items.push(Sexp::Atom(name.into(), sp));
    items.push(Sexp::Atom(source_system.into(), sp));
    items.push(Sexp::Atom(target_system.into(), sp));

    for (src, tgt) in op_maps {
        items.push(Sexp::List(
            vec![
                Sexp::Atom("Map".into(), sp),
                Sexp::Atom(src.clone(), sp),
                Sexp::Atom(tgt.clone(), sp),
            ],
            sp,
        ));
    }

    Sexp::List(items, sp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::category::{MorphismDecl, ObjectDecl};
    use crate::substrate::{BarrierMode, Engine, EqualityMode, ResourceMode, SubstrateDef, TotalityMode};
    use crate::universe::CompilationPass;

    fn make_ccc() -> CategoryDef {
        CategoryDef {
            name: "CartesianClosed".into(),
            objects: vec![
                ObjectDecl {
                    name: "Type".into(),
                },
                ObjectDecl {
                    name: "Term".into(),
                },
            ],
            morphisms: vec![
                MorphismDecl {
                    name: "arrow".into(),
                    domain: vec!["Type".into(), "Type".into()],
                    codomain: "Type".into(),
                },
                MorphismDecl {
                    name: "app".into(),
                    domain: vec!["Term".into(), "Term".into()],
                    codomain: "Term".into(),
                },
            ],
            judgments: vec![],
            structure: vec![
                CategoricalStructure::Exponential {
                    name: "lam".into(),
                    object: "Term".into(),
                },
                CategoricalStructure::Evaluator {
                    name: "app".into(),
                },
            ],
        }
    }

    fn make_inet() -> SubstrateDef {
        SubstrateDef {
            name: "InteractionNet".into(),
            engine: Engine::InteractionGraph,
            resource_mode: ResourceMode::OptimalSharing,
            barrier: BarrierMode::Transparent,
            equality: EqualityMode::TopologicalHash,
            totality: TotalityMode::Unspecified,
        }
    }

    #[test]
    fn ccc_on_inet_compiles_natively() {
        let cat = make_ccc();
        let sub = make_inet();
        let compiled = compile_universe("WeakLF", &cat, &sub).unwrap();
        assert_eq!(compiled.system_name, "__hyp_CartesianClosed_InteractionNet");
        assert!(compiled.passes.is_empty(), "native pair needs no passes");
    }

    #[test]
    fn ccc_on_cellular_automaton_defunctionalizes() {
        let cat = make_ccc();
        let sub = SubstrateDef {
            name: "GridWorld".into(),
            engine: Engine::CellularAutomaton,
            resource_mode: ResourceMode::DeepCopy,
            barrier: BarrierMode::Transparent,
            equality: EqualityMode::RewriteEquivalence,
            totality: TotalityMode::Unspecified,
        };
        let compiled = compile_universe("Defunc", &cat, &sub).unwrap();
        assert!(compiled.passes.contains(&CompilationPass::Defunctionalization));
    }

    #[test]
    fn strictly_linear_exponential_gets_bang_modality() {
        let cat = make_ccc();
        let sub = SubstrateDef {
            name: "LinearNet".into(),
            engine: Engine::InteractionGraph,
            resource_mode: ResourceMode::StrictlyLinear,
            barrier: BarrierMode::Transparent,
            equality: EqualityMode::TopologicalHash,
            totality: TotalityMode::Unspecified,
        };
        let compiled = compile_universe("LinearCCC", &cat, &sub).unwrap();
        assert!(compiled.passes.contains(&CompilationPass::BangModality));
    }

    #[test]
    fn modal_on_transparent_gets_kripke_threading() {
        let cat = CategoryDef {
            name: "Modal".into(),
            objects: vec![ObjectDecl {
                name: "Prop".into(),
            }],
            morphisms: vec![],
            judgments: vec![],
            structure: vec![
                CategoricalStructure::ModalOperator {
                    name: "box".into(),
                },
                CategoricalStructure::ContextDecl {
                    name: "W".into(),
                },
            ],
        };
        let sub = make_inet(); // barrier = Transparent
        let compiled = compile_universe("KripkeModal", &cat, &sub).unwrap();
        assert!(compiled.passes.contains(&CompilationPass::KripkeWorldThreading));
    }

    #[test]
    fn emit_system_sexp_produces_valid_structure() {
        let cat = make_ccc();
        let sub = make_inet();
        let compiled = compile_universe("WeakLF", &cat, &sub).unwrap();
        let sexp = emit_system_sexp(&cat, &sub, &compiled, None);

        // Should be a list starting with "System"
        let items = sexp.as_list().unwrap();
        assert_eq!(items[0].as_atom().unwrap(), "System");
        assert_eq!(
            items[1].as_atom().unwrap(),
            "__hyp_CartesianClosed_InteractionNet"
        );
    }

    #[test]
    fn emit_morphism_sexp_no_maps() {
        let sexp = emit_morphism_sexp("__fun_F_Cat", "__hyp_Cat_A", "__hyp_Cat_B", &[]);
        let items = sexp.as_list().unwrap();
        assert_eq!(items[0].as_atom().unwrap(), "AutoMorphism");
        assert_eq!(items[1].as_atom().unwrap(), "__fun_F_Cat");
        assert_eq!(items[2].as_atom().unwrap(), "__hyp_Cat_A");
        assert_eq!(items[3].as_atom().unwrap(), "__hyp_Cat_B");
        assert_eq!(items.len(), 4); // No Map entries
    }

    #[test]
    fn emit_morphism_sexp_with_maps() {
        let maps = vec![
            ("z".to_string(), "zero".to_string()),
            ("s".to_string(), "succ".to_string()),
        ];
        let sexp = emit_morphism_sexp("morph", "src_sys", "tgt_sys", &maps);
        let items = sexp.as_list().unwrap();
        assert_eq!(items.len(), 6); // AutoMorphism + name + src + tgt + 2 Maps

        // First Map entry
        let map1 = items[4].as_list().unwrap();
        assert_eq!(map1[0].as_atom().unwrap(), "Map");
        assert_eq!(map1[1].as_atom().unwrap(), "z");
        assert_eq!(map1[2].as_atom().unwrap(), "zero");

        // Second Map entry
        let map2 = items[5].as_list().unwrap();
        assert_eq!(map2[0].as_atom().unwrap(), "Map");
        assert_eq!(map2[1].as_atom().unwrap(), "s");
        assert_eq!(map2[2].as_atom().unwrap(), "succ");
    }

    #[test]
    fn morphism_name_generation() {
        assert_eq!(morphism_name_for("F", "Cat"), "__fun_F_Cat");
        assert_eq!(
            morphism_name_for("NetToTree", "SimpleMath"),
            "__fun_NetToTree_SimpleMath"
        );
    }

    #[test]
    fn nominal_binding_mode() {
        let cat = CategoryDef {
            name: "Simple".into(),
            objects: vec![ObjectDecl { name: "T".into() }],
            morphisms: vec![],
            judgments: vec![],
            structure: vec![],
        };
        let sub = SubstrateDef {
            name: "Nom".into(),
            engine: Engine::InteractionGraph,
            resource_mode: ResourceMode::OptimalSharing,
            barrier: BarrierMode::NominalScoping,
            equality: EqualityMode::TopologicalHash,
            totality: TotalityMode::Unspecified,
        };
        assert_eq!(binding_mode(&cat, &sub, &[]), "nominal");
    }

    #[test]
    fn nominal_exponential_gets_nominal_abstraction() {
        let cat = make_ccc(); // has Exponential
        let sub = SubstrateDef {
            name: "Nom".into(),
            engine: Engine::InteractionGraph,
            resource_mode: ResourceMode::OptimalSharing,
            barrier: BarrierMode::NominalScoping,
            equality: EqualityMode::TopologicalHash,
            totality: TotalityMode::Unspecified,
        };
        let compiled = compile_universe("NomCCC", &cat, &sub).unwrap();
        assert!(compiled.passes.contains(&CompilationPass::NominalAbstraction));
    }

    #[test]
    fn reversible_check_mode() {
        let sub = SubstrateDef {
            name: "Rev".into(),
            engine: Engine::ReversibleGraph,
            resource_mode: ResourceMode::OptimalSharing,
            barrier: BarrierMode::Transparent,
            equality: EqualityMode::RewriteEquivalence,
            totality: TotalityMode::Unspecified,
        };
        let modes = check_modes(&sub);
        assert!(modes.contains(&"reversible"));
        assert!(modes.contains(&"rewriting"));
    }

    #[test]
    fn concurrent_check_mode() {
        let sub = SubstrateDef {
            name: "Conc".into(),
            engine: Engine::ConcurrentGraph,
            resource_mode: ResourceMode::OptimalSharing,
            barrier: BarrierMode::Transparent,
            equality: EqualityMode::RewriteEquivalence,
            totality: TotalityMode::Unspecified,
        };
        let modes = check_modes(&sub);
        assert!(modes.contains(&"confluent-race"));
        assert!(modes.contains(&"rewriting"));
    }

    #[test]
    fn extensional_check_mode() {
        let sub = SubstrateDef {
            name: "Ext".into(),
            engine: Engine::InteractionGraph,
            resource_mode: ResourceMode::OptimalSharing,
            barrier: BarrierMode::Transparent,
            equality: EqualityMode::ExtensionalEquivalence,
            totality: TotalityMode::Unspecified,
        };
        let modes = check_modes(&sub);
        assert!(modes.contains(&"extensional"));
        assert!(modes.contains(&"rewriting"));
        assert!(modes.contains(&"beta-reduction"));
    }

    #[test]
    fn full_unification_check_mode() {
        let sub = SubstrateDef {
            name: "FullU".into(),
            engine: Engine::InteractionGraph,
            resource_mode: ResourceMode::OptimalSharing,
            barrier: BarrierMode::Transparent,
            equality: EqualityMode::FullUnification,
            totality: TotalityMode::Unspecified,
        };
        let modes = check_modes(&sub);
        assert_eq!(modes, vec!["unification"]);
    }

    #[test]
    fn pattern_unification_unchanged() {
        let sub = SubstrateDef {
            name: "PatU".into(),
            engine: Engine::InteractionGraph,
            resource_mode: ResourceMode::OptimalSharing,
            barrier: BarrierMode::Transparent,
            equality: EqualityMode::Unification,
            totality: TotalityMode::Unspecified,
        };
        let modes = check_modes(&sub);
        assert_eq!(modes, vec!["pattern-unification"]);
    }

    #[test]
    fn von_neumann_exponential_defunctionalizes() {
        let cat = make_ccc();
        let sub = SubstrateDef {
            name: "VN".into(),
            engine: Engine::VonNeumann,
            resource_mode: ResourceMode::DeepCopy,
            barrier: BarrierMode::Transparent,
            equality: EqualityMode::RewriteEquivalence,
            totality: TotalityMode::Unspecified,
        };
        let compiled = compile_universe("VN_CCC", &cat, &sub).unwrap();
        assert!(compiled.passes.contains(&CompilationPass::Defunctionalization));
    }

    #[test]
    fn von_neumann_modal_gets_kripke_and_defunc() {
        let cat = CategoryDef {
            name: "ModalCCC".into(),
            objects: vec![ObjectDecl { name: "T".into() }],
            morphisms: vec![],
            judgments: vec![],
            structure: vec![
                CategoricalStructure::Exponential { name: "lam".into(), object: "T".into() },
                CategoricalStructure::Evaluator { name: "app".into() },
                CategoricalStructure::ModalOperator { name: "box".into() },
                CategoricalStructure::ContextDecl { name: "W".into() },
            ],
        };
        let sub = SubstrateDef {
            name: "VN".into(),
            engine: Engine::VonNeumann,
            resource_mode: ResourceMode::DeepCopy,
            barrier: BarrierMode::Transparent,
            equality: EqualityMode::RewriteEquivalence,
            totality: TotalityMode::Unspecified,
        };
        let compiled = compile_universe("VN_Modal", &cat, &sub).unwrap();
        assert!(compiled.passes.contains(&CompilationPass::Defunctionalization));
        assert!(compiled.passes.contains(&CompilationPass::KripkeWorldThreading));
    }

    #[test]
    fn tensor_on_term_tree_serializes() {
        let cat = CategoryDef {
            name: "Monoidal".into(),
            objects: vec![ObjectDecl { name: "Obj".into() }],
            morphisms: vec![],
            judgments: vec![],
            structure: vec![
                CategoricalStructure::TensorProduct { name: "tensor".into() },
                CategoricalStructure::Unit { name: "I".into() },
            ],
        };
        let sub = SubstrateDef {
            name: "Tree".into(),
            engine: Engine::TermTree,
            resource_mode: ResourceMode::OptimalSharing,
            barrier: BarrierMode::Transparent,
            equality: EqualityMode::RewriteEquivalence,
            totality: TotalityMode::Unspecified,
        };
        let compiled = compile_universe("SerialMonoidal", &cat, &sub).unwrap();
        assert!(compiled.passes.contains(&CompilationPass::TensorSerialization));
    }

    #[test]
    fn homotopy_on_von_neumann_gets_dependent_combinators_and_defunc() {
        let cat = CategoryDef {
            name: "HoTT".into(),
            objects: vec![ObjectDecl { name: "Type".into() }],
            morphisms: vec![],
            judgments: vec![],
            structure: vec![
                CategoricalStructure::Exponential { name: "lam".into(), object: "Type".into() },
                CategoricalStructure::Evaluator { name: "app".into() },
                CategoricalStructure::PathType {
                    refl: "refl".into(), concat: "concat".into(),
                    inv: "inv".into(), ap: "ap".into(),
                },
            ],
        };
        let sub = SubstrateDef {
            name: "VN".into(),
            engine: Engine::VonNeumann,
            resource_mode: ResourceMode::DeepCopy,
            barrier: BarrierMode::Transparent,
            equality: EqualityMode::TopologicalHomotopy,
            totality: TotalityMode::Unspecified,
        };
        let compiled = compile_universe("VN_HoTT", &cat, &sub).unwrap();
        assert!(compiled.passes.contains(&CompilationPass::DependentCombinators));
        assert!(compiled.passes.contains(&CompilationPass::Defunctionalization));
    }

    #[test]
    fn compilation_passes_emitted_in_system_sexp() {
        let cat = make_ccc();
        let sub = SubstrateDef {
            name: "LinearNet".into(),
            engine: Engine::InteractionGraph,
            resource_mode: ResourceMode::StrictlyLinear,
            barrier: BarrierMode::Transparent,
            equality: EqualityMode::TopologicalHash,
            totality: TotalityMode::Unspecified,
        };
        let compiled = compile_universe("LinearCCC", &cat, &sub).unwrap();
        let sexp = emit_system_sexp(&cat, &sub, &compiled, None);
        let items = sexp.as_list().unwrap();
        // Find the @compilation-passes block
        let passes_block = items.iter().find(|s| {
            s.as_list().and_then(|l| l.first()).and_then(|s| s.as_atom()) == Some("@compilation-passes")
        });
        assert!(passes_block.is_some(), "system sexp should contain @compilation-passes");
        let pass_items = passes_block.unwrap().as_list().unwrap();
        // @compilation-passes + at least one pass entry
        assert!(pass_items.len() >= 2);
    }

    #[test]
    fn no_passes_block_when_native() {
        let cat = make_ccc();
        let sub = make_inet();
        let compiled = compile_universe("WeakLF", &cat, &sub).unwrap();
        let sexp = emit_system_sexp(&cat, &sub, &compiled, None);
        let items = sexp.as_list().unwrap();
        let passes_block = items.iter().find(|s| {
            s.as_list().and_then(|l| l.first()).and_then(|s| s.as_atom()) == Some("@compilation-passes")
        });
        assert!(passes_block.is_none(), "native pair should not have @compilation-passes");
    }

    #[test]
    fn emit_signature_sexp_produces_typed_ops() {
        let cat = make_ccc();
        let sig = emit_signature_sexp(&cat);
        let items = sig.as_list().unwrap();
        assert_eq!(items[0].as_atom().unwrap(), "Signature");
        assert_eq!(items[1].as_atom().unwrap(), "__hyp_sig_CartesianClosed");

        // Should have 2 sort declarations + 2 typed op declarations = 6 items total (Signature + name + 4)
        // sorts: Type, Term
        // ops: [op arrow Type Type Type], [op app Term Term Term]
        assert_eq!(items.len(), 6);

        // Check a typed op
        let arrow_op = items[4].as_list().unwrap();
        assert_eq!(arrow_op[0].as_atom().unwrap(), "op");
        assert_eq!(arrow_op[1].as_atom().unwrap(), "arrow");
        assert_eq!(arrow_op[2].as_atom().unwrap(), "Type");
        assert_eq!(arrow_op[3].as_atom().unwrap(), "Type");
        assert_eq!(arrow_op[4].as_atom().unwrap(), "Type"); // codomain
    }

    #[test]
    fn emit_system_sexp_with_signature_reference() {
        let cat = make_ccc();
        let sub = make_inet();
        let compiled = compile_universe("WeakLF", &cat, &sub).unwrap();
        let sexp = emit_system_sexp(&cat, &sub, &compiled, Some("__hyp_sig_CartesianClosed"));
        let items = sexp.as_list().unwrap();

        // Should have :signature reference
        let sig_idx = items.iter().position(|s| s.as_atom() == Some(":signature")).unwrap();
        assert_eq!(items[sig_idx + 1].as_atom().unwrap(), "__hyp_sig_CartesianClosed");

        // @syntax should NOT contain sorts/morphisms (they come from Signature)
        let syntax = items.iter().find(|s| {
            s.as_list().and_then(|l| l.first()).and_then(|s| s.as_atom()) == Some("@syntax")
        }).unwrap();
        let syntax_items = syntax.as_list().unwrap();
        // Should only have @syntax head + structure-derived ops (lam from Exponential)
        // Not the sorts (Type, Term) or morphisms (arrow, app)
        let has_sort = syntax_items.iter().any(|s| {
            s.as_list().and_then(|l| l.first()).and_then(|s| s.as_atom()) == Some("sort")
        });
        assert!(!has_sort, "Signature-referenced system should not have sorts in @syntax");
    }
}
