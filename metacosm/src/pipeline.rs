use apeiron::parser::Sexp;

use crate::error::{MetacosmError, Result};

/// A pipeline step: one phase in a theorem cosmology pipeline.
#[derive(Debug, Clone)]
pub struct PipelineStep {
    pub name: String,
    pub action: PipelineAction,
    pub world: String,
    /// Optional target world (for transitions)
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineAction {
    /// Discover theorems in this world (uses its search substrate)
    Discover,
    /// Verify a theorem in this world
    Verify,
    /// Tunnel a result from current world to target world
    Tunnel,
    /// Coarse-grain into a simpler effective world
    CoarseGrain,
    /// Measure an observable
    Measure,
    /// Split into descendant worlds
    Split,
    /// Merge results from multiple worlds
    Merge,
}

impl std::fmt::Display for PipelineAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineAction::Discover => write!(f, "Discover"),
            PipelineAction::Verify => write!(f, "Verify"),
            PipelineAction::Tunnel => write!(f, "Tunnel"),
            PipelineAction::CoarseGrain => write!(f, "CoarseGrain"),
            PipelineAction::Measure => write!(f, "Measure"),
            PipelineAction::Split => write!(f, "Split"),
            PipelineAction::Merge => write!(f, "Merge"),
        }
    }
}

fn parse_action(s: &str) -> Result<PipelineAction> {
    match s {
        "Discover" => Ok(PipelineAction::Discover),
        "Verify" => Ok(PipelineAction::Verify),
        "Tunnel" => Ok(PipelineAction::Tunnel),
        "CoarseGrain" => Ok(PipelineAction::CoarseGrain),
        "Measure" => Ok(PipelineAction::Measure),
        "Split" => Ok(PipelineAction::Split),
        "Merge" => Ok(PipelineAction::Merge),
        _ => Err(MetacosmError::ParseError {
            block: "Pipeline".into(),
            detail: format!("unknown pipeline action: '{}'", s),
        }),
    }
}

/// A theorem cosmology pipeline: a sequence of steps through world-space.
#[derive(Debug, Clone)]
pub struct PipelineDef {
    pub name: String,
    pub steps: Vec<PipelineStep>,
}

/// Parse `[Pipeline Name [Step ...] [Step ...] ...]`
pub fn parse_pipeline(items: &[Sexp]) -> Result<PipelineDef> {
    if items.len() < 2 {
        return Err(MetacosmError::ParseError {
            block: "Pipeline".into(),
            detail: "missing pipeline name".into(),
        });
    }

    let name = items[1]
        .as_atom()
        .ok_or_else(|| MetacosmError::ParseError {
            block: "Pipeline".into(),
            detail: "pipeline name must be an atom".into(),
        })?
        .to_string();

    let mut steps = Vec::new();

    for item in &items[2..] {
        if let Some(step_items) = item.as_list() {
            steps.push(parse_step(step_items)?);
        }
    }

    Ok(PipelineDef { name, steps })
}

fn parse_step(items: &[Sexp]) -> Result<PipelineStep> {
    // [Step name :action A :world W [:target T]]
    if items.len() < 2 {
        return Err(MetacosmError::ParseError {
            block: "Pipeline/Step".into(),
            detail: "missing step name".into(),
        });
    }

    let head = items[0].as_atom().unwrap_or("");
    if head != "Step" {
        return Err(MetacosmError::ParseError {
            block: "Pipeline".into(),
            detail: format!("expected Step, got '{}'", head),
        });
    }

    let name = items[1]
        .as_atom()
        .ok_or_else(|| MetacosmError::ParseError {
            block: "Pipeline/Step".into(),
            detail: "step name must be an atom".into(),
        })?
        .to_string();

    let mut action: Option<PipelineAction> = None;
    let mut world: Option<String> = None;
    let mut target: Option<String> = None;

    let mut i = 2;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":action" => {
                i += 1;
                if let Some(a) = items.get(i).and_then(|s| s.as_atom()) {
                    action = Some(parse_action(a)?);
                }
            }
            ":world" => {
                i += 1;
                world = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
            }
            ":target" => {
                i += 1;
                target = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
            }
            _ => {
                return Err(MetacosmError::ParseError {
                    block: "Pipeline/Step".into(),
                    detail: format!("unknown keyword: {}", key),
                });
            }
        }
        i += 1;
    }

    let action = action.ok_or_else(|| MetacosmError::ParseError {
        block: "Pipeline/Step".into(),
        detail: format!("Step '{}' is missing :action", name),
    })?;
    let world = world.ok_or_else(|| MetacosmError::ParseError {
        block: "Pipeline/Step".into(),
        detail: format!("Step '{}' is missing :world", name),
    })?;

    Ok(PipelineStep {
        name,
        action,
        world,
        target,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use apeiron::parser::parse;

    #[test]
    fn parse_pipeline_basic() {
        let input = r#"[Pipeline TheoremCosmo
            [Step discover-equiv :action Discover :world Explorer]
            [Step transport-proof :action Tunnel :world Explorer :target Certifier]
            [Step verify-result :action Verify :world Certifier]
            [Step compile-out :action CoarseGrain :world Certifier :target Executor]
            [Step measure :action Measure :world Executor]
        ]"#;
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let p = parse_pipeline(items).unwrap();
        assert_eq!(p.name, "TheoremCosmo");
        assert_eq!(p.steps.len(), 5);
        assert_eq!(p.steps[0].action, PipelineAction::Discover);
        assert_eq!(p.steps[1].action, PipelineAction::Tunnel);
        assert_eq!(p.steps[1].target, Some("Certifier".to_string()));
    }
}
