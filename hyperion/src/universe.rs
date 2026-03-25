use apeiron::parser::Sexp;

use crate::error::{HyperionError, Result};

/// A universe definition: binds a category to a substrate.
#[derive(Debug, Clone)]
pub struct UniverseDef {
    pub name: String,
    pub category: String,
    pub substrate: String,
}

/// A compilation pass that bridges an otherwise-incompatible category+substrate pair.
///
/// Each pass represents a well-known theoretical construction that makes the
/// combination sound by inserting an intermediate representation or translation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompilationPass {
    /// Girard's ! modality: `A → B` becomes `!A ⊸ B` in a linear substrate.
    /// Exponentials are available only under explicit `!` promotion.
    BangModality,
    /// Gabbay-Pitts nominal abstraction: lambda replaced by name-abstraction
    /// with explicit swapping and freshness side-conditions.
    NominalAbstraction,
    /// Reynolds' defunctionalization: closures compiled to a first-order ADT
    /// with a global `apply` dispatch function.
    Defunctionalization,
    /// Tensor products serialized left-to-right on sequential engines.
    /// Sound for pure (effect-free) tensor operands.
    TensorSerialization,
    /// Kripke semantics compilation: □A compiled to ∀w.A(w) with explicit
    /// world-parameter threading, eliminating the need for physical barriers.
    KripkeWorldThreading,
    /// Dependent combinatory logic: Π-types and path spaces compiled to
    /// a dependent SKI combinator calculus (extremely expensive).
    DependentCombinators,
    /// RPC serialization: all data crossing network partition barriers must be
    /// serialized to a wire format. Generates proof obligation that every
    /// cross-partition type is an instance of a Serializable category.
    RpcSerialization,
    /// Consensus replication: eventually-consistent resources require conflict
    /// resolution. Operations must be commutative (CRDT-compatible) or
    /// totally ordered (Raft/Paxos). Generates commutativity proof obligations.
    ConsensusReplication,
    /// Partition tolerance: modal operators on network-partition barriers
    /// become availability/consistency trade-off points. Each □A is compiled
    /// to "A is available at all non-partitioned replicas" (AP) or
    /// "A is consistent across all replicas before proceeding" (CP).
    PartitionTolerance,
    /// Proof-carrying parallel tensor: tensor products compiled to parallel
    /// execution (rayon::join) after verifying free-variable disjointness.
    /// Strictly-linear resource mode forbids the diagonal map (A → A ⊗ A),
    /// guaranteeing data-race freedom at the categorical level.
    ParallelTensorProof,

    // === Phase 1: HOAS + Logic Programming + LCF Tactics ===

    /// HOAS defunctionalization: higher-order binders compiled to first-order
    /// explicit substitution calculus. Object-language binding becomes ADTs
    /// with shift/subst operations instead of meta-language lambda.
    HOASDefunctionalization,
    /// Clause compilation: higher-order Horn clauses compiled to first-order
    /// resolution-ready clauses with explicit unification variables.
    ClauseCompilation,
    /// Goal-directed proof search: LCF tactic trees compiled to iterative
    /// proof state manipulation with explicit validation and backtracking.
    GoalDirected,

    // === Phase 1.5: AC-Matching ===

    /// AC normalization: terms with associative-commutative operators are
    /// flattened (removing nesting) and sorted (canonical permutation)
    /// before pattern matching. Also handles K Framework cell multiset matching.
    ACNormalization,

    // === Phase 2: Contextual + Cohesive ===

    /// Context reification: first-class contexts compiled to explicit data
    /// structures (context stacks/telescopes) on substrates with transparent barriers.
    ContextReification,
    /// Modal substitution restriction: cohesive variable-class guards inserted
    /// at runtime. Under flat, only discrete variables substitutable; under sharp,
    /// only codiscrete. Enforces Riehl-Shulman substitution discipline.
    ModalSubstitutionRestriction,

    // === Phase 3: Full Cubical ===

    /// Kan computation: generates reduction rules for transport through each
    /// type former (Σ, Π, Glue, Path). Makes cubical operations compute
    /// instead of being merely provable.
    KanComputation,

    // === Phase 4: SMT + Effects ===

    /// SMT encoding: categorical terms translated to SMT-LIB2 format for
    /// external solver dispatch. Theory-specific axioms become SMT assertions.
    SMTEncoding,
    /// Effect elaboration: effect grading compiled to monadic encoding or CPS
    /// on substrates without native effect tracking.
    EffectElaboration,

    // === Phase 5: Dialectica ===

    /// Dialectica extraction: Gödel's Dialectica interpretation applied to
    /// classical proofs. Extracts witness functions (realizers) from
    /// ∀x.∃y.A(x,y) as concrete code: f: X → Y with proof of A(x, f(x)).
    DialecticaExtraction,

    /// Explicit substitution calculus (λσ): lowers lambda binders into
    /// first-order Closure/Env nodes so the e-graph can safely rewrite
    /// under binders without variable capture.
    ExplicitSubstitution,
}

impl CompilationPass {
    /// Human-readable name for diagnostics.
    pub fn name(&self) -> &'static str {
        match self {
            Self::BangModality => "bang-modality",
            Self::NominalAbstraction => "nominal-abstraction",
            Self::Defunctionalization => "defunctionalization",
            Self::TensorSerialization => "tensor-serialization",
            Self::KripkeWorldThreading => "kripke-world-threading",
            Self::DependentCombinators => "dependent-combinators",
            Self::RpcSerialization => "rpc-serialization",
            Self::ConsensusReplication => "consensus-replication",
            Self::PartitionTolerance => "partition-tolerance",
            Self::ParallelTensorProof => "parallel-tensor-proof",
            Self::HOASDefunctionalization => "hoas-defunctionalization",
            Self::ClauseCompilation => "clause-compilation",
            Self::GoalDirected => "goal-directed",
            Self::ACNormalization => "ac-normalization",
            Self::ContextReification => "context-reification",
            Self::ModalSubstitutionRestriction => "modal-substitution-restriction",
            Self::KanComputation => "kan-computation",
            Self::SMTEncoding => "smt-encoding",
            Self::EffectElaboration => "effect-elaboration",
            Self::DialecticaExtraction => "dialectica-extraction",
            Self::ExplicitSubstitution => "explicit-substitution",
        }
    }

    /// Short description of what the pass does.
    pub fn description(&self) -> &'static str {
        match self {
            Self::BangModality => "Girard's !-modality: closures require explicit promotion from linear to unrestricted",
            Self::NominalAbstraction => "Gabbay-Pitts nominal abstraction: lambda via name-swapping + freshness",
            Self::Defunctionalization => "Reynolds defunctionalization: closures compiled to first-order ADT + apply dispatch",
            Self::TensorSerialization => "tensor products serialized left-to-right (sound for pure operands)",
            Self::KripkeWorldThreading => "Kripke compilation: modal scopes threaded as explicit world parameters",
            Self::DependentCombinators => "dependent SKI combinator translation for path spaces (expensive)",
            Self::RpcSerialization => "RPC serialization: cross-partition data must be wire-serializable",
            Self::ConsensusReplication => "consensus replication: operations must be commutative (CRDT) or totally ordered (Raft/Paxos)",
            Self::PartitionTolerance => "partition tolerance: modal operators compiled to AP/CP availability trade-offs",
            Self::ParallelTensorProof => "proof-carrying parallel tensor: disjoint linear resources compiled to rayon::join",
            Self::HOASDefunctionalization => "HOAS defunctionalization: higher-order binders compiled to explicit substitution calculus",
            Self::ClauseCompilation => "clause compilation: higher-order clauses compiled to first-order resolution-ready form",
            Self::GoalDirected => "goal-directed search: LCF tactic trees compiled to iterative proof state manipulation",
            Self::ACNormalization => "AC normalization: flatten + sort associative-commutative terms before pattern matching",
            Self::ContextReification => "context reification: first-class contexts compiled to explicit data structures",
            Self::ModalSubstitutionRestriction => "modal substitution restriction: cohesive variable-class guards (shape/flat/sharp)",
            Self::KanComputation => "Kan computation: transport reduction rules generated for each type former",
            Self::SMTEncoding => "SMT encoding: categorical terms translated to SMT-LIB2 for external solver",
            Self::EffectElaboration => "effect elaboration: effect grading compiled to monadic/CPS encoding",
            Self::DialecticaExtraction => "Dialectica extraction: classical proofs compiled to witness programs via Gödel's interpretation",
            Self::ExplicitSubstitution => "explicit substitution calculus (λσ): binders lowered to first-order Closure/Env for safe e-graph rewriting",
        }
    }
}

/// A compiled universe: the result of compilation.
#[derive(Debug, Clone)]
pub struct CompiledUniverse {
    pub name: String,
    pub system_name: String,
    pub scope_names: Vec<String>,
    pub category_name: String,
    pub substrate_name: String,
    /// Compilation passes required to bridge the category+substrate gap.
    /// Empty when the pair is natively compatible.
    pub passes: Vec<CompilationPass>,
}

/// Parse a `[Universe Name :category C :substrate S]` S-expression.
pub fn parse_universe(items: &[Sexp]) -> Result<UniverseDef> {
    if items.len() < 2 {
        return Err(HyperionError::ParseError {
            block: "Universe".into(),
            detail: "missing universe name".into(),
        });
    }

    let name = items[1]
        .as_atom()
        .ok_or_else(|| HyperionError::ParseError {
            block: "Universe".into(),
            detail: "universe name must be an atom".into(),
        })?
        .to_string();

    let mut category: Option<String> = None;
    let mut substrate: Option<String> = None;

    let mut i = 2;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":category" => {
                i += 1;
                category = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
            }
            ":substrate" => {
                i += 1;
                substrate = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
            }
            _ => {
                return Err(HyperionError::ParseError {
                    block: "Universe".into(),
                    detail: format!("unknown keyword: {}", key),
                });
            }
        }
        i += 1;
    }

    let category = category.ok_or_else(|| HyperionError::ParseError {
        block: "Universe".into(),
        detail: format!("Universe '{}' is missing :category", name),
    })?;
    let substrate = substrate.ok_or_else(|| HyperionError::ParseError {
        block: "Universe".into(),
        detail: format!("Universe '{}' is missing :substrate", name),
    })?;

    Ok(UniverseDef {
        name,
        category,
        substrate,
    })
}

/// Generate the deterministic Apeiron system name for a universe.
pub fn system_name_for(category: &str, substrate: &str) -> String {
    format!("__hyp_{}_{}", category, substrate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use apeiron::parser::parse;

    #[test]
    fn parse_universe_def() {
        let input = "[Universe WeakLF :category CartesianClosed :substrate InteractionNet]";
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let uni = parse_universe(items).unwrap();

        assert_eq!(uni.name, "WeakLF");
        assert_eq!(uni.category, "CartesianClosed");
        assert_eq!(uni.substrate, "InteractionNet");
    }

    #[test]
    fn system_name_generation() {
        assert_eq!(
            system_name_for("CartesianClosed", "InteractionNet"),
            "__hyp_CartesianClosed_InteractionNet"
        );
    }
}
