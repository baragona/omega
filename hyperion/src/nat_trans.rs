use apeiron::parser::Sexp;

use crate::error::{HyperionError, Result};

/// A component of a natural transformation: maps a category object to a morphism name.
#[derive(Debug, Clone)]
pub struct NatTransComponent {
    pub object: String,
    pub morphism: String,
}

/// A natural transformation between two parallel functors.
#[derive(Debug, Clone)]
pub struct NatTransDef {
    pub name: String,
    pub source_functor: String,
    pub target_functor: String,
    pub components: Vec<NatTransComponent>,
    pub verify: bool,
}

/// Parse a `[NatTrans name :from F :to G :component [Obj morph] ... :verify]` S-expression.
pub fn parse_nat_trans(items: &[Sexp]) -> Result<NatTransDef> {
    if items.len() < 2 {
        return Err(HyperionError::ParseError {
            block: "NatTrans".into(),
            detail: "missing natural transformation name".into(),
        });
    }

    let name = items[1]
        .as_atom()
        .ok_or_else(|| HyperionError::ParseError {
            block: "NatTrans".into(),
            detail: "name must be an atom".into(),
        })?
        .to_string();

    let mut source_functor: Option<String> = None;
    let mut target_functor: Option<String> = None;
    let mut components = Vec::new();
    let mut verify = false;

    let mut i = 2;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":from" => {
                i += 1;
                source_functor = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
            }
            ":to" => {
                i += 1;
                target_functor = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
            }
            ":component" => {
                i += 1;
                if let Some(pair) = items.get(i).and_then(|s| s.as_list()) {
                    if pair.len() == 2 {
                        let obj = pair[0].as_atom().unwrap_or("").to_string();
                        let morph = pair[1].as_atom().unwrap_or("").to_string();
                        components.push(NatTransComponent {
                            object: obj,
                            morphism: morph,
                        });
                    }
                }
            }
            ":verify" => {
                verify = true;
            }
            _ => {
                return Err(HyperionError::ParseError {
                    block: "NatTrans".into(),
                    detail: format!("unknown keyword: {}", key),
                });
            }
        }
        i += 1;
    }

    let source_functor = source_functor.ok_or_else(|| HyperionError::ParseError {
        block: "NatTrans".into(),
        detail: format!("NatTrans '{}' is missing :from", name),
    })?;
    let target_functor = target_functor.ok_or_else(|| HyperionError::ParseError {
        block: "NatTrans".into(),
        detail: format!("NatTrans '{}' is missing :to", name),
    })?;

    Ok(NatTransDef {
        name,
        source_functor,
        target_functor,
        components,
        verify,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use apeiron::parser::parse;

    #[test]
    fn parse_simple_nat_trans() {
        let input = r#"[NatTrans eta :from F :to G
            :component [Nat tau_nat]
            :component [Bool tau_bool]
        ]"#;
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let nt = parse_nat_trans(items).unwrap();

        assert_eq!(nt.name, "eta");
        assert_eq!(nt.source_functor, "F");
        assert_eq!(nt.target_functor, "G");
        assert_eq!(nt.components.len(), 2);
        assert_eq!(nt.components[0].object, "Nat");
        assert_eq!(nt.components[0].morphism, "tau_nat");
        assert!(!nt.verify);
    }

    #[test]
    fn parse_nat_trans_with_verify() {
        let input = "[NatTrans eta :from F :to G :component [X tau] :verify]";
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let nt = parse_nat_trans(items).unwrap();
        assert!(nt.verify);
    }
}
