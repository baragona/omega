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
        _ => Err(HyperionError::ParseError {
            block: "Substrate".into(),
            detail: format!(
                "Substrate '{}': unknown engine '{}'. Expected one of: interaction-graph, term-tree, symmetric-monoidal, cellular-automaton, abstract-machine, von-neumann",
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
        _ => Err(HyperionError::ParseError {
            block: "Substrate".into(),
            detail: format!(
                "Substrate '{}': unknown barrier '{}'. Expected one of: transparent, contextual-membranes, one-way-valve, temporal-phase, cryptographic",
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
        _ => Err(HyperionError::ParseError {
            block: "Substrate".into(),
            detail: format!(
                "Substrate '{}': unknown equality '{}'. Expected one of: topological-hash, rewrite-equivalence, alpha-equivalence, observational, unification, topological-homotopy",
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
