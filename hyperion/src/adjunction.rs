use apeiron::parser::Sexp;

use crate::error::{HyperionError, Result};

/// An adjunction between two functors.
#[derive(Debug, Clone)]
pub struct AdjunctionDef {
    pub name: String,
    pub left: String,
    pub right: String,
    pub unit: String,
    pub counit: String,
    pub verify: bool,
}

/// Parse a `[Adjunction name :left F :right G :unit eta :counit eps :verify]` S-expression.
pub fn parse_adjunction(items: &[Sexp]) -> Result<AdjunctionDef> {
    if items.len() < 2 {
        return Err(HyperionError::ParseError {
            block: "Adjunction".into(),
            detail: "missing adjunction name".into(),
        });
    }

    let name = items[1]
        .as_atom()
        .ok_or_else(|| HyperionError::ParseError {
            block: "Adjunction".into(),
            detail: "name must be an atom".into(),
        })?
        .to_string();

    let mut left: Option<String> = None;
    let mut right: Option<String> = None;
    let mut unit: Option<String> = None;
    let mut counit: Option<String> = None;
    let mut verify = false;

    let mut i = 2;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":left" => {
                i += 1;
                left = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
            }
            ":right" => {
                i += 1;
                right = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
            }
            ":unit" => {
                i += 1;
                unit = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
            }
            ":counit" => {
                i += 1;
                counit = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
            }
            ":verify" => {
                verify = true;
            }
            _ => {
                return Err(HyperionError::ParseError {
                    block: "Adjunction".into(),
                    detail: format!("unknown keyword: {}", key),
                });
            }
        }
        i += 1;
    }

    let left = left.ok_or_else(|| HyperionError::ParseError {
        block: "Adjunction".into(),
        detail: format!("Adjunction '{}' is missing :left", name),
    })?;
    let right = right.ok_or_else(|| HyperionError::ParseError {
        block: "Adjunction".into(),
        detail: format!("Adjunction '{}' is missing :right", name),
    })?;
    let unit = unit.ok_or_else(|| HyperionError::ParseError {
        block: "Adjunction".into(),
        detail: format!("Adjunction '{}' is missing :unit", name),
    })?;
    let counit = counit.ok_or_else(|| HyperionError::ParseError {
        block: "Adjunction".into(),
        detail: format!("Adjunction '{}' is missing :counit", name),
    })?;

    Ok(AdjunctionDef {
        name,
        left,
        right,
        unit,
        counit,
        verify,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use apeiron::parser::parse;

    #[test]
    fn parse_simple_adjunction() {
        let input = "[Adjunction FreeForget :left Free :right Forget :unit eta :counit eps]";
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let adj = parse_adjunction(items).unwrap();

        assert_eq!(adj.name, "FreeForget");
        assert_eq!(adj.left, "Free");
        assert_eq!(adj.right, "Forget");
        assert_eq!(adj.unit, "eta");
        assert_eq!(adj.counit, "eps");
        assert!(!adj.verify);
    }

    #[test]
    fn parse_adjunction_with_verify() {
        let input =
            "[Adjunction A :left F :right G :unit eta :counit eps :verify]";
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let adj = parse_adjunction(items).unwrap();
        assert!(adj.verify);
    }
}
