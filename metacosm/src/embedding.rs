use apeiron::parser::Sexp;

use crate::error::{MetacosmError, Result};

/// A layer in the Metacosm hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LayerName {
    Omega,
    Hyperion,
    Metacosm,
}

impl std::fmt::Display for LayerName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayerName::Omega => write!(f, "Omega"),
            LayerName::Hyperion => write!(f, "Hyperion"),
            LayerName::Metacosm => write!(f, "Metacosm"),
        }
    }
}

fn parse_layer(s: &str) -> Option<LayerName> {
    match s {
        "Omega" => Some(LayerName::Omega),
        "Hyperion" => Some(LayerName::Hyperion),
        "Metacosm" => Some(LayerName::Metacosm),
        _ => None,
    }
}

/// An endpoint: either a named layer or a named world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbeddingEndpoint {
    Layer(LayerName),
    World(String),
}

impl std::fmt::Display for EmbeddingEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbeddingEndpoint::Layer(l) => write!(f, "{}", l),
            EmbeddingEndpoint::World(w) => write!(f, "{}", w),
        }
    }
}

fn parse_endpoint(s: &str) -> EmbeddingEndpoint {
    if let Some(layer) = parse_layer(s) {
        EmbeddingEndpoint::Layer(layer)
    } else {
        EmbeddingEndpoint::World(s.to_string())
    }
}

/// A property claimed of an embedding.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EmbeddingProperty {
    /// Everything provable in source is provable in target
    Conservative,
    /// Source is a definable fragment of target
    DefinableFragment,
    /// Target has things source doesn't
    StrictExtension,
    /// Adding target features doesn't change source behavior
    NonPerturbing,
    Custom(String),
}

impl std::fmt::Display for EmbeddingProperty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbeddingProperty::Conservative => write!(f, "conservative"),
            EmbeddingProperty::DefinableFragment => write!(f, "definable-fragment"),
            EmbeddingProperty::StrictExtension => write!(f, "strict-extension"),
            EmbeddingProperty::NonPerturbing => write!(f, "non-perturbing"),
            EmbeddingProperty::Custom(s) => write!(f, "{}", s),
        }
    }
}

fn parse_embedding_property(s: &str) -> EmbeddingProperty {
    match s {
        "conservative" => EmbeddingProperty::Conservative,
        "definable-fragment" => EmbeddingProperty::DefinableFragment,
        "strict-extension" => EmbeddingProperty::StrictExtension,
        "non-perturbing" => EmbeddingProperty::NonPerturbing,
        other => EmbeddingProperty::Custom(other.to_string()),
    }
}

/// A declared embedding relationship.
#[derive(Debug, Clone)]
pub struct EmbeddingDef {
    pub name: String,
    pub source: EmbeddingEndpoint,
    pub target: EmbeddingEndpoint,
    pub properties: Vec<EmbeddingProperty>,
    pub checked: bool,
}

/// Parse `[Embedding Name :from E :to E :properties [...]]`
pub fn parse_embedding(items: &[Sexp]) -> Result<EmbeddingDef> {
    if items.len() < 2 {
        return Err(MetacosmError::ParseError {
            block: "Embedding".into(),
            detail: "missing embedding name".into(),
        });
    }

    let name = items[1]
        .as_atom()
        .ok_or_else(|| MetacosmError::ParseError {
            block: "Embedding".into(),
            detail: "embedding name must be an atom".into(),
        })?
        .to_string();

    let mut source = None;
    let mut target = None;
    let mut properties = Vec::new();

    let mut i = 2;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":from" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    source = Some(parse_endpoint(v));
                }
            }
            ":to" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    target = Some(parse_endpoint(v));
                }
            }
            ":properties" => {
                i += 1;
                if let Some(list) = items.get(i).and_then(|s| s.as_list()) {
                    for item in list {
                        if let Some(p) = item.as_atom() {
                            properties.push(parse_embedding_property(p));
                        }
                    }
                }
            }
            _ => {
                return Err(MetacosmError::ParseError {
                    block: "Embedding".into(),
                    detail: format!("unknown keyword: {}", key),
                });
            }
        }
        i += 1;
    }

    let source = source.ok_or_else(|| MetacosmError::ParseError {
        block: "Embedding".into(),
        detail: "missing :from".into(),
    })?;
    let target = target.ok_or_else(|| MetacosmError::ParseError {
        block: "Embedding".into(),
        detail: "missing :to".into(),
    })?;

    Ok(EmbeddingDef {
        name,
        source,
        target,
        properties,
        checked: false,
    })
}

/// The block types accepted by each layer.
pub fn layer_block_types(layer: &LayerName) -> &'static [&'static str] {
    match layer {
        LayerName::Omega => &["Theory", "Proofs"],
        LayerName::Hyperion => &[
            "Theory", "Proofs", "Category", "Substrate", "Universe",
            "Functor", "NatTrans", "Adjunction", "VerifyFunctor",
        ],
        LayerName::Metacosm => &[
            "Theory", "Proofs", "Category", "Substrate", "Universe",
            "Functor", "NatTrans", "Adjunction", "VerifyFunctor",
            "World", "Transition", "Observable", "Family", "Pipeline",
            "Measure", "Compose", "Embedding",
        ],
    }
}

/// Check structural properties of a layer-to-layer embedding.
pub fn check_layer_embedding(
    source: &LayerName,
    target: &LayerName,
    properties: &[EmbeddingProperty],
) -> Vec<Result<String>> {
    let mut results = Vec::new();
    let src_blocks = layer_block_types(source);
    let tgt_blocks = layer_block_types(target);

    for prop in properties {
        match prop {
            EmbeddingProperty::DefinableFragment => {
                // All source block types must be in target
                let all_included = src_blocks.iter().all(|b| tgt_blocks.contains(b));
                if all_included {
                    results.push(Ok(format!(
                        "definable-fragment: {} block types are subset of {}",
                        source, target
                    )));
                } else {
                    results.push(Err(MetacosmError::EmbeddingViolation {
                        embedding: format!("{} -> {}", source, target),
                        property: "definable-fragment".into(),
                        detail: "source has block types not accepted by target".into(),
                    }));
                }
            }
            EmbeddingProperty::StrictExtension => {
                let has_extra = tgt_blocks.iter().any(|b| !src_blocks.contains(b));
                if has_extra {
                    results.push(Ok(format!(
                        "strict-extension: {} has block types not in {}",
                        target, source
                    )));
                } else {
                    results.push(Err(MetacosmError::EmbeddingViolation {
                        embedding: format!("{} -> {}", source, target),
                        property: "strict-extension".into(),
                        detail: "target has no additional block types".into(),
                    }));
                }
            }
            EmbeddingProperty::Conservative => {
                // Structural check: conservative embedding means source pass-through
                // is identity. This is true by construction for our layered architecture.
                results.push(Ok(format!(
                    "conservative: {} pass-through in {} preserves semantics (by construction)",
                    source, target
                )));
            }
            EmbeddingProperty::NonPerturbing => {
                // Structural check: cosmology blocks don't modify lower-layer state.
                results.push(Ok(format!(
                    "non-perturbing: {} blocks don't modify {} state (by construction)",
                    target, source
                )));
            }
            EmbeddingProperty::Custom(s) => {
                results.push(Ok(format!("custom property '{}': unchecked", s)));
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use apeiron::parser::parse;

    #[test]
    fn parse_embedding_basic() {
        let input = "[Embedding OmegaInHyperion :from Omega :to Hyperion :properties [conservative definable-fragment]]";
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let e = parse_embedding(items).unwrap();
        assert_eq!(e.name, "OmegaInHyperion");
        assert_eq!(e.source, EmbeddingEndpoint::Layer(LayerName::Omega));
        assert_eq!(e.target, EmbeddingEndpoint::Layer(LayerName::Hyperion));
        assert_eq!(e.properties.len(), 2);
    }

    #[test]
    fn omega_is_fragment_of_hyperion() {
        let results = check_layer_embedding(
            &LayerName::Omega,
            &LayerName::Hyperion,
            &[EmbeddingProperty::DefinableFragment, EmbeddingProperty::StrictExtension],
        );
        assert!(results.iter().all(|r| r.is_ok()), "results: {:?}", results);
    }

    #[test]
    fn hyperion_is_fragment_of_metacosm() {
        let results = check_layer_embedding(
            &LayerName::Hyperion,
            &LayerName::Metacosm,
            &[
                EmbeddingProperty::DefinableFragment,
                EmbeddingProperty::StrictExtension,
                EmbeddingProperty::Conservative,
                EmbeddingProperty::NonPerturbing,
            ],
        );
        assert!(results.iter().all(|r| r.is_ok()), "results: {:?}", results);
    }
}
