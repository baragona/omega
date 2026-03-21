use apeiron::parser::Sexp;

use crate::error::{HyperionError, Result};

/// A declared object in a category.
#[derive(Debug, Clone)]
pub struct ObjectDecl {
    pub name: String,
}

/// A declared morphism in a category.
#[derive(Debug, Clone)]
pub struct MorphismDecl {
    pub name: String,
    pub domain: Vec<String>,
    pub codomain: String,
}

/// Categorical gadgets that constrain what substrates are compatible.
#[derive(Debug, Clone)]
pub enum CategoricalStructure {
    /// CCC: lambda abstraction (exponential object)
    Exponential { name: String, object: String },
    /// CCC: function application (evaluation morphism)
    Evaluator { name: String },
    /// Topos: modal necessity operator
    ModalOperator { name: String },
    /// Modal: named world/scope
    ContextDecl { name: String },
    /// Monoidal: tensor product
    TensorProduct { name: String },
    /// Monoidal: unit object
    Unit { name: String },
    /// Preorder: reflexive relation with auto-injected reflexivity rule
    Preorder { relation: String },
    /// HoTT: path algebra (refl, concat, inv, ap)
    PathType {
        refl: String,
        concat: String,
        inv: String,
        ap: String,
    },
    /// Dependent elimination for identity types (J-eliminator)
    JType {
        j_elim: String,
        transport: String,
    },
    /// Partial element support (cubical face constraints)
    PartialElement {
        hcomp: String,
        coe: String,
    },
}

/// A category definition: pure mathematical structure.
#[derive(Debug, Clone)]
pub struct CategoryDef {
    pub name: String,
    pub objects: Vec<ObjectDecl>,
    pub morphisms: Vec<MorphismDecl>,
    pub structure: Vec<CategoricalStructure>,
}

impl CategoryDef {
    /// Check whether this category has any Exponential structure.
    pub fn has_exponential(&self) -> bool {
        self.structure
            .iter()
            .any(|s| matches!(s, CategoricalStructure::Exponential { .. }))
    }

    /// Check whether this category has any Evaluator structure.
    pub fn has_evaluator(&self) -> bool {
        self.structure
            .iter()
            .any(|s| matches!(s, CategoricalStructure::Evaluator { .. }))
    }

    /// Check whether this category has any ModalOperator structure.
    pub fn has_modal_operator(&self) -> bool {
        self.structure
            .iter()
            .any(|s| matches!(s, CategoricalStructure::ModalOperator { .. }))
    }

    /// Check whether this category has any Context declarations.
    pub fn has_context(&self) -> bool {
        self.structure
            .iter()
            .any(|s| matches!(s, CategoricalStructure::ContextDecl { .. }))
    }

    /// Check whether this category has any TensorProduct structure.
    pub fn has_tensor(&self) -> bool {
        self.structure
            .iter()
            .any(|s| matches!(s, CategoricalStructure::TensorProduct { .. }))
    }

    /// Check whether this category has any Preorder structure.
    pub fn has_preorder(&self) -> bool {
        self.structure
            .iter()
            .any(|s| matches!(s, CategoricalStructure::Preorder { .. }))
    }

    /// Check whether this category has any PathType structure.
    pub fn has_path_type(&self) -> bool {
        self.structure
            .iter()
            .any(|s| matches!(s, CategoricalStructure::PathType { .. }))
    }

    /// Check whether this category has any JType structure.
    pub fn has_j_type(&self) -> bool {
        self.structure
            .iter()
            .any(|s| matches!(s, CategoricalStructure::JType { .. }))
    }

    /// Check whether this category has any PartialElement structure.
    pub fn has_partial_element(&self) -> bool {
        self.structure
            .iter()
            .any(|s| matches!(s, CategoricalStructure::PartialElement { .. }))
    }
}

/// Parse a `[Category Name ...]` S-expression into a CategoryDef.
pub fn parse_category(items: &[Sexp]) -> Result<CategoryDef> {
    if items.len() < 2 {
        return Err(HyperionError::ParseError {
            block: "Category".into(),
            detail: "missing category name".into(),
        });
    }

    let name = items[1]
        .as_atom()
        .ok_or_else(|| HyperionError::ParseError {
            block: "Category".into(),
            detail: "category name must be an atom".into(),
        })?
        .to_string();

    let mut objects = Vec::new();
    let mut morphisms = Vec::new();
    let mut structure = Vec::new();

    for item in &items[2..] {
        let inner = item.as_list().ok_or_else(|| HyperionError::ParseError {
            block: "Category".into(),
            detail: "expected list inside Category".into(),
        })?;

        if inner.is_empty() {
            continue;
        }

        let head = inner[0].as_atom().unwrap_or("");
        match head {
            "Object" => {
                let obj_name = inner
                    .get(1)
                    .and_then(|s| s.as_atom())
                    .ok_or_else(|| HyperionError::ParseError {
                        block: "Category".into(),
                        detail: "Object requires a name".into(),
                    })?
                    .to_string();
                objects.push(ObjectDecl { name: obj_name });
            }
            "Morphism" => {
                morphisms.push(parse_morphism(inner)?);
            }
            "Exponential" => {
                let exp_name = get_required_atom(inner, 1, "Exponential", "name")?;
                let obj = get_keyword_atom(inner, ":object", "Exponential")?;
                structure.push(CategoricalStructure::Exponential {
                    name: exp_name,
                    object: obj,
                });
            }
            "Evaluator" => {
                let eval_name = get_required_atom(inner, 1, "Evaluator", "name")?;
                structure.push(CategoricalStructure::Evaluator { name: eval_name });
            }
            "ModalOperator" => {
                let op_name = get_required_atom(inner, 1, "ModalOperator", "name")?;
                structure.push(CategoricalStructure::ModalOperator { name: op_name });
            }
            "Context" => {
                let ctx_name = get_required_atom(inner, 1, "Context", "name")?;
                structure.push(CategoricalStructure::ContextDecl { name: ctx_name });
            }
            "TensorProduct" => {
                let tp_name = get_required_atom(inner, 1, "TensorProduct", "name")?;
                structure.push(CategoricalStructure::TensorProduct { name: tp_name });
            }
            "Unit" => {
                let u_name = get_required_atom(inner, 1, "Unit", "name")?;
                structure.push(CategoricalStructure::Unit { name: u_name });
            }
            "Preorder" => {
                let rel_name = get_required_atom(inner, 1, "Preorder", "relation name")?;
                structure.push(CategoricalStructure::Preorder { relation: rel_name });
            }
            "SymmetricMonoidal" => {
                // Compound syntax: [SymmetricMonoidal tensor_name unit_name]
                // Decomposes into TensorProduct + Unit
                let tp_name = get_required_atom(inner, 1, "SymmetricMonoidal", "tensor product name")?;
                let u_name = get_required_atom(inner, 2, "SymmetricMonoidal", "unit name")?;
                structure.push(CategoricalStructure::TensorProduct { name: tp_name });
                structure.push(CategoricalStructure::Unit { name: u_name });
            }
            "PathType" => {
                let refl = get_keyword_atom(inner, ":refl", "PathType")?;
                let concat = get_keyword_atom(inner, ":concat", "PathType")?;
                let inv = get_keyword_atom(inner, ":inv", "PathType")?;
                let ap = get_keyword_atom(inner, ":ap", "PathType")?;
                structure.push(CategoricalStructure::PathType {
                    refl,
                    concat,
                    inv,
                    ap,
                });
            }
            "JType" => {
                let j_elim = get_keyword_atom(inner, ":elim", "JType")?;
                let transport = get_keyword_atom(inner, ":transport", "JType")?;
                structure.push(CategoricalStructure::JType { j_elim, transport });
            }
            "PartialElement" => {
                let hcomp = get_keyword_atom(inner, ":hcomp", "PartialElement")?;
                let coe = get_keyword_atom(inner, ":coe", "PartialElement")?;
                structure.push(CategoricalStructure::PartialElement { hcomp, coe });
            }
            _ => {
                return Err(HyperionError::ParseError {
                    block: "Category".into(),
                    detail: format!("unknown declaration: {}", head),
                });
            }
        }
    }

    Ok(CategoryDef {
        name,
        objects,
        morphisms,
        structure,
    })
}

fn parse_morphism(inner: &[Sexp]) -> Result<MorphismDecl> {
    let name = get_required_atom(inner, 1, "Morphism", "name")?;

    let domain = get_keyword_list(inner, ":domain", "Morphism")?;
    let codomain = get_keyword_atom(inner, ":codomain", "Morphism")?;

    Ok(MorphismDecl {
        name,
        domain,
        codomain,
    })
}

/// Get a required atom at a specific index.
fn get_required_atom(items: &[Sexp], idx: usize, block: &str, field: &str) -> Result<String> {
    items
        .get(idx)
        .and_then(|s| s.as_atom())
        .map(|s| s.to_string())
        .ok_or_else(|| HyperionError::ParseError {
            block: block.into(),
            detail: format!("{} requires a {}", block, field),
        })
}

/// Find a keyword in items and return the atom after it.
fn get_keyword_atom(items: &[Sexp], keyword: &str, block: &str) -> Result<String> {
    for (i, item) in items.iter().enumerate() {
        if item.is_atom(keyword) {
            return items
                .get(i + 1)
                .and_then(|s| s.as_atom())
                .map(|s| s.to_string())
                .ok_or_else(|| HyperionError::ParseError {
                    block: block.into(),
                    detail: format!("expected atom after {}", keyword),
                });
        }
    }
    Err(HyperionError::ParseError {
        block: block.into(),
        detail: format!("missing keyword {}", keyword),
    })
}

/// Find a keyword in items and return the list of atoms after it.
fn get_keyword_list(items: &[Sexp], keyword: &str, block: &str) -> Result<Vec<String>> {
    for (i, item) in items.iter().enumerate() {
        if item.is_atom(keyword) {
            let list_sexp = items.get(i + 1).ok_or_else(|| HyperionError::ParseError {
                block: block.into(),
                detail: format!("expected list after {}", keyword),
            })?;
            let list = list_sexp
                .as_list()
                .ok_or_else(|| HyperionError::ParseError {
                    block: block.into(),
                    detail: format!("expected list after {}", keyword),
                })?;
            return list
                .iter()
                .map(|s| {
                    s.as_atom()
                        .map(|a| a.to_string())
                        .ok_or_else(|| HyperionError::ParseError {
                            block: block.into(),
                            detail: "expected atom in list".into(),
                        })
                })
                .collect();
        }
    }
    Err(HyperionError::ParseError {
        block: block.into(),
        detail: format!("missing keyword {}", keyword),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use apeiron::parser::parse;

    #[test]
    fn parse_simple_category() {
        let input = r#"[Category CartesianClosed
            [Object Type]
            [Object Term]
            [Morphism arrow :domain [Type Type] :codomain Type]
            [Morphism app :domain [Term Term] :codomain Term]
            [Exponential lam :object Term]
            [Evaluator app]
        ]"#;
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let cat = parse_category(items).unwrap();

        assert_eq!(cat.name, "CartesianClosed");
        assert_eq!(cat.objects.len(), 2);
        assert_eq!(cat.morphisms.len(), 2);
        assert!(cat.has_exponential());
        assert!(cat.has_evaluator());
        assert!(!cat.has_modal_operator());
    }

    #[test]
    fn parse_modal_category() {
        let input = r#"[Category ModalSpace
            [Object Prop]
            [ModalOperator box]
            [Context WorldA]
            [Context WorldB]
        ]"#;
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let cat = parse_category(items).unwrap();

        assert_eq!(cat.name, "ModalSpace");
        assert!(cat.has_modal_operator());
        assert!(cat.has_context());
        assert!(!cat.has_exponential());
    }
}
