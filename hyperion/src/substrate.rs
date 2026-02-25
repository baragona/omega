use apeiron::parser::Sexp;

use crate::error::{HyperionError, Result};

/// Computational engine type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Engine {
    InteractionGraph,
    TermTree,
    SymmetricMonoidal,
    CellularAutomaton,
    AbstractMachine,
    VonNeumann,
    ReversibleGraph,
    ConcurrentGraph,
}

/// Resource management mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceMode {
    OptimalSharing,
    StrictlyLinear,
    Affine,
    Relevant,
    DeepCopy,
}

/// Barrier/scoping mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BarrierMode {
    Transparent,
    ContextualMembranes,
    OneWayValve,
    TemporalPhase,
    Cryptographic,
    NominalScoping,
}

/// Equality checking mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EqualityMode {
    TopologicalHash,
    RewriteEquivalence,
    AlphaEquivalence,
    Observational,
    Unification,
    /// HoTT: equality is a space (paths between points, paths between paths)
    TopologicalHomotopy,
    /// Equality saturation via e-graphs: bidirectional rule application.
    EqualitySaturation,
    /// Extensional equivalence: functions equal iff they agree on all inputs.
    ExtensionalEquivalence,
    /// Full (higher-order) unification, not just Miller pattern fragment.
    FullUnification,
}

/// A substrate definition: the physical laws of computation.
#[derive(Debug, Clone)]
pub struct SubstrateDef {
    pub name: String,
    pub engine: Engine,
    pub resource_mode: ResourceMode,
    pub barrier: BarrierMode,
    pub equality: EqualityMode,
}

/// Parse a `[Substrate Name @engine ... @resource-mode ... @barrier ... @equality ...]` S-expression.
pub fn parse_substrate(items: &[Sexp]) -> Result<SubstrateDef> {
    if items.len() < 2 {
        return Err(HyperionError::ParseError {
            block: "Substrate".into(),
            detail: "missing substrate name".into(),
        });
    }

    let name = items[1]
        .as_atom()
        .ok_or_else(|| HyperionError::ParseError {
            block: "Substrate".into(),
            detail: "substrate name must be an atom".into(),
        })?
        .to_string();

    let mut engine: Option<Engine> = None;
    let mut resource_mode: Option<ResourceMode> = None;
    let mut barrier: Option<BarrierMode> = None;
    let mut equality: Option<EqualityMode> = None;

    let mut i = 2;
    while i < items.len() {
        let key = items[i]
            .as_atom()
            .ok_or_else(|| HyperionError::ParseError {
                block: "Substrate".into(),
                detail: format!("expected keyword at position {}", i),
            })?;

        let val_sexp = items.get(i + 1).ok_or_else(|| HyperionError::ParseError {
            block: "Substrate".into(),
            detail: format!("missing value after {}", key),
        })?;
        let val = val_sexp
            .as_atom()
            .ok_or_else(|| HyperionError::ParseError {
                block: "Substrate".into(),
                detail: format!("expected atom value after {}", key),
            })?;

        match key {
            "@engine" => {
                engine = Some(parse_engine(val, &name)?);
            }
            "@resource-mode" => {
                resource_mode = Some(parse_resource_mode(val, &name)?);
            }
            "@barrier" => {
                barrier = Some(parse_barrier_mode(val, &name)?);
            }
            "@equality" => {
                equality = Some(parse_equality_mode(val, &name)?);
            }
            _ => {
                return Err(HyperionError::ParseError {
                    block: "Substrate".into(),
                    detail: format!("unknown field: {}", key),
                });
            }
        }

        i += 2;
    }

    let engine = engine.ok_or_else(|| HyperionError::ParseError {
        block: "Substrate".into(),
        detail: format!("Substrate '{}' is missing required field @engine", name),
    })?;
    let resource_mode = resource_mode.ok_or_else(|| HyperionError::ParseError {
        block: "Substrate".into(),
        detail: format!(
            "Substrate '{}' is missing required field @resource-mode",
            name
        ),
    })?;
    let barrier = barrier.ok_or_else(|| HyperionError::ParseError {
        block: "Substrate".into(),
        detail: format!("Substrate '{}' is missing required field @barrier", name),
    })?;
    let equality = equality.ok_or_else(|| HyperionError::ParseError {
        block: "Substrate".into(),
        detail: format!("Substrate '{}' is missing required field @equality", name),
    })?;

    Ok(SubstrateDef {
        name,
        engine,
        resource_mode,
        barrier,
        equality,
    })
}

fn parse_engine(val: &str, substrate: &str) -> Result<Engine> {
    match val {
        "interaction-graph" => Ok(Engine::InteractionGraph),
        "term-tree" => Ok(Engine::TermTree),
        "symmetric-monoidal" => Ok(Engine::SymmetricMonoidal),
        "cellular-automaton" => Ok(Engine::CellularAutomaton),
        "abstract-machine" => Ok(Engine::AbstractMachine),
        "von-neumann" => Ok(Engine::VonNeumann),
        "reversible-graph" => Ok(Engine::ReversibleGraph),
        "concurrent-graph" => Ok(Engine::ConcurrentGraph),
        _ => Err(HyperionError::ParseError {
            block: "Substrate".into(),
            detail: format!(
                "Substrate '{}': unknown engine '{}'. Expected one of: interaction-graph, term-tree, symmetric-monoidal, cellular-automaton, abstract-machine, von-neumann, reversible-graph, concurrent-graph",
                substrate, val
            ),
        }),
    }
}

fn parse_resource_mode(val: &str, substrate: &str) -> Result<ResourceMode> {
    match val {
        "optimal-sharing" => Ok(ResourceMode::OptimalSharing),
        "strictly-linear" => Ok(ResourceMode::StrictlyLinear),
        "affine" => Ok(ResourceMode::Affine),
        "relevant" => Ok(ResourceMode::Relevant),
        "deep-copy" => Ok(ResourceMode::DeepCopy),
        _ => Err(HyperionError::ParseError {
            block: "Substrate".into(),
            detail: format!(
                "Substrate '{}': unknown resource-mode '{}'. Expected one of: optimal-sharing, strictly-linear, affine, relevant, deep-copy",
                substrate, val
            ),
        }),
    }
}

fn parse_barrier_mode(val: &str, substrate: &str) -> Result<BarrierMode> {
    match val {
        "transparent" => Ok(BarrierMode::Transparent),
        "contextual-membranes" => Ok(BarrierMode::ContextualMembranes),
        "one-way-valve" => Ok(BarrierMode::OneWayValve),
        "temporal-phase" => Ok(BarrierMode::TemporalPhase),
        "cryptographic" => Ok(BarrierMode::Cryptographic),
        "nominal-scoping" => Ok(BarrierMode::NominalScoping),
        _ => Err(HyperionError::ParseError {
            block: "Substrate".into(),
            detail: format!(
                "Substrate '{}': unknown barrier '{}'. Expected one of: transparent, contextual-membranes, one-way-valve, temporal-phase, cryptographic, nominal-scoping",
                substrate, val
            ),
        }),
    }
}

fn parse_equality_mode(val: &str, substrate: &str) -> Result<EqualityMode> {
    match val {
        "topological-hash" => Ok(EqualityMode::TopologicalHash),
        "rewrite-equivalence" => Ok(EqualityMode::RewriteEquivalence),
        "alpha-equivalence" => Ok(EqualityMode::AlphaEquivalence),
        "observational" => Ok(EqualityMode::Observational),
        "unification" => Ok(EqualityMode::Unification),
        "topological-homotopy" => Ok(EqualityMode::TopologicalHomotopy),
        "equality-saturation" | "e-graph" | "egraph" => Ok(EqualityMode::EqualitySaturation),
        "extensional-equivalence" | "extensional" => Ok(EqualityMode::ExtensionalEquivalence),
        "full-unification" => Ok(EqualityMode::FullUnification),
        _ => Err(HyperionError::ParseError {
            block: "Substrate".into(),
            detail: format!(
                "Substrate '{}': unknown equality '{}'. Expected one of: topological-hash, rewrite-equivalence, alpha-equivalence, observational, unification, topological-homotopy, equality-saturation, extensional-equivalence, full-unification",
                substrate, val
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apeiron::parser::parse;

    #[test]
    fn parse_interaction_net_substrate() {
        let input = r#"[Substrate InteractionNet
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-hash
        ]"#;
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let sub = parse_substrate(items).unwrap();

        assert_eq!(sub.name, "InteractionNet");
        assert_eq!(sub.engine, Engine::InteractionGraph);
        assert_eq!(sub.resource_mode, ResourceMode::OptimalSharing);
        assert_eq!(sub.barrier, BarrierMode::Transparent);
        assert_eq!(sub.equality, EqualityMode::TopologicalHash);
    }

    #[test]
    fn parse_compartment_net_substrate() {
        let input = r#"[Substrate CompartmentNet
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier contextual-membranes
            @equality rewrite-equivalence
        ]"#;
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let sub = parse_substrate(items).unwrap();

        assert_eq!(sub.name, "CompartmentNet");
        assert_eq!(sub.barrier, BarrierMode::ContextualMembranes);
        assert_eq!(sub.equality, EqualityMode::RewriteEquivalence);
    }

    #[test]
    fn parse_nominal_scoping_substrate() {
        let input = r#"[Substrate NominalNet
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier nominal-scoping
            @equality topological-hash
        ]"#;
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let sub = parse_substrate(items).unwrap();
        assert_eq!(sub.barrier, BarrierMode::NominalScoping);
    }

    #[test]
    fn parse_reversible_graph_engine() {
        let input = r#"[Substrate RevNet
            @engine reversible-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-hash
        ]"#;
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let sub = parse_substrate(items).unwrap();
        assert_eq!(sub.engine, Engine::ReversibleGraph);
    }

    #[test]
    fn parse_concurrent_graph_engine() {
        let input = r#"[Substrate ConcNet
            @engine concurrent-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-hash
        ]"#;
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let sub = parse_substrate(items).unwrap();
        assert_eq!(sub.engine, Engine::ConcurrentGraph);
    }

    #[test]
    fn parse_extensional_equivalence() {
        let input = r#"[Substrate ExtNet
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality extensional-equivalence
        ]"#;
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let sub = parse_substrate(items).unwrap();
        assert_eq!(sub.equality, EqualityMode::ExtensionalEquivalence);
    }

    #[test]
    fn parse_extensional_shorthand() {
        let input = r#"[Substrate ExtNet
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality extensional
        ]"#;
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let sub = parse_substrate(items).unwrap();
        assert_eq!(sub.equality, EqualityMode::ExtensionalEquivalence);
    }

    #[test]
    fn parse_full_unification() {
        let input = r#"[Substrate FullUnifNet
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality full-unification
        ]"#;
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let sub = parse_substrate(items).unwrap();
        assert_eq!(sub.equality, EqualityMode::FullUnification);
    }

    #[test]
    fn missing_field_error() {
        let input = r#"[Substrate Bad
            @engine interaction-graph
            @resource-mode optimal-sharing
        ]"#;
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let err = parse_substrate(items).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("missing required field @barrier"));
    }
}
