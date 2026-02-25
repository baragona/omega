use apeiron::parser::Sexp;

use crate::error::{HyperionError, Result};

/// A universe definition: binds a category to a substrate.
#[derive(Debug, Clone)]
pub struct UniverseDef {
    pub name: String,
    pub category: String,
    pub substrate: String,
}

/// A compiled universe: the result of compilation.
#[derive(Debug, Clone)]
pub struct CompiledUniverse {
    pub name: String,
    pub system_name: String,
    pub scope_names: Vec<String>,
    pub category_name: String,
    pub substrate_name: String,
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
