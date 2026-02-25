use apeiron::parser::Sexp;

use crate::error::{HyperionError, Result};

/// A functor definition: cross-substrate translation declaration.
#[derive(Debug, Clone)]
pub struct FunctorDef {
    pub name: String,
    pub source: String,
    pub target: String,
    pub object_map: Vec<(String, String)>,
    pub morphism_map: Vec<(String, String)>,
    /// If true, VerifyFunctor can check equational theory preservation
    pub verify: bool,
}

/// Parse a `[Functor Name :from A :to B ...]` S-expression.
pub fn parse_functor(items: &[Sexp]) -> Result<FunctorDef> {
    if items.len() < 2 {
        return Err(HyperionError::ParseError {
            block: "Functor".into(),
            detail: "missing functor name".into(),
        });
    }

    let name = items[1]
        .as_atom()
        .ok_or_else(|| HyperionError::ParseError {
            block: "Functor".into(),
            detail: "functor name must be an atom".into(),
        })?
        .to_string();

    let mut source: Option<String> = None;
    let mut target: Option<String> = None;
    let mut object_map = Vec::new();
    let mut morphism_map = Vec::new();
    let mut verify = false;

    let mut i = 2;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":from" => {
                i += 1;
                source = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
            }
            ":to" => {
                i += 1;
                target = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
            }
            ":map-object" => {
                // :map-object [src tgt]
                i += 1;
                if let Some(pair) = items.get(i).and_then(|s| s.as_list()) {
                    if pair.len() == 2 {
                        let a = pair[0].as_atom().unwrap_or("").to_string();
                        let b = pair[1].as_atom().unwrap_or("").to_string();
                        object_map.push((a, b));
                    }
                }
            }
            ":map-morphism" => {
                // :map-morphism [src tgt]
                i += 1;
                if let Some(pair) = items.get(i).and_then(|s| s.as_list()) {
                    if pair.len() == 2 {
                        let a = pair[0].as_atom().unwrap_or("").to_string();
                        let b = pair[1].as_atom().unwrap_or("").to_string();
                        morphism_map.push((a, b));
                    }
                }
            }
            ":verify" => {
                verify = true;
            }
            _ => {
                return Err(HyperionError::ParseError {
                    block: "Functor".into(),
                    detail: format!("unknown keyword: {}", key),
                });
            }
        }
        i += 1;
    }

    let source = source.ok_or_else(|| HyperionError::ParseError {
        block: "Functor".into(),
        detail: format!("Functor '{}' is missing :from", name),
    })?;
    let target = target.ok_or_else(|| HyperionError::ParseError {
        block: "Functor".into(),
        detail: format!("Functor '{}' is missing :to", name),
    })?;

    Ok(FunctorDef {
        name,
        source,
        target,
        object_map,
        morphism_map,
        verify,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use apeiron::parser::parse;

    #[test]
    fn parse_simple_functor() {
        let input = "[Functor F :from InteractionNet :to TermTree]";
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let fun = parse_functor(items).unwrap();

        assert_eq!(fun.name, "F");
        assert_eq!(fun.source, "InteractionNet");
        assert_eq!(fun.target, "TermTree");
        assert!(fun.object_map.is_empty());
    }

    #[test]
    fn parse_functor_with_maps() {
        let input = r#"[Functor Embed
            :from InteractionNet
            :to TermTree
            :map-object [Type Ty]
            :map-morphism [lam abs]
        ]"#;
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let fun = parse_functor(items).unwrap();

        assert_eq!(fun.object_map.len(), 1);
        assert_eq!(fun.object_map[0], ("Type".into(), "Ty".into()));
        assert_eq!(fun.morphism_map.len(), 1);
        assert_eq!(fun.morphism_map[0], ("lam".into(), "abs".into()));
    }
}
