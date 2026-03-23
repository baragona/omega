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
