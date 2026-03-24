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

/// A declared judgment form in a category.
/// Judgments are the canonical way to declare derivation-checkable forms
/// (e.g., `typeof`, `proves`, `holds`) at the categorical level.
#[derive(Debug, Clone)]
pub struct JudgmentDecl {
    pub name: String,
    pub inputs: Vec<String>,
    pub output: Option<String>,
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
    /// Cubical interval sort with endpoints and kernel reduction rules.
    /// Enables kernel-level coe/hcomp computation along composite/inverse paths.
    IntervalSort {
        interval: String,
        i0: String,
        i1: String,
    },

    // === Phase 1: HOAS + Logic Programming + LCF Tactics ===

    /// Higher-Order Abstract Syntax: object-language binding via meta-language lambda.
    /// Enables LF-style representations where substitution is inherited from the meta-level.
    /// With StrictlyLinear resource mode, gives Linear LF (LLF).
    HOASBinding {
        binder: String,
        object_sort: String,
    },
    /// LCF-style tactic combinators for goal-directed proof search.
    /// Distinct from logic programming: operates on proof state trees with
    /// explicit validation functions, not Horn clause resolution.
    TacticCombinators {
        then: String,
        orelse: String,
        repeat: String,
        try_tac: String,
        focus: String,
    },

    // === Phase 1.5: AC-Matching + State Configurations ===

    /// State configuration cells (K Framework style).
    /// Nested multiset of named cells; matching is modulo multiset identity.
    /// Under the hood, shares the AC-matching algorithm with ACMatching equality mode.
    StateConfiguration {
        cell_sort: String,
        merge: String,
    },

    // === Phase 2: Contextual + Cohesive Modalities ===

    /// Contextual modal types (Beluga-style): contexts as first-class typed objects.
    /// `[Γ ⊢ t : A]` is a term, not just a judgment.
    ContextualType {
        context_sort: String,
        term_sort: String,
    },
    /// Cohesive modalities (Riehl-Shulman): shape ⊣ flat ⊣ sharp adjoint triple.
    /// These modalities restrict substitution: under flat, only discrete variables;
    /// under sharp, only codiscrete. For synthetic (∞,1)-category theory.
    CohesiveModality {
        shape: String,
        flat: String,
        sharp: String,
    },

    // === Phase 3: Full Cubical Type Theory ===

    /// Face lattice for cubical type theory: meet (∧), join (∨), negation (¬).
    /// Face formulas like `(i = 0) ∧ (j = 1)` form a distributive lattice.
    FaceLattice {
        meet: String,
        join: String,
        neg: String,
    },
    /// Glue types: the computational content of univalence.
    /// `Glue A [φ ↦ (B, f)]` looks like A globally but like B on face φ.
    GlueType {
        glue: String,
        unglue: String,
    },
    /// Kan operations: the computational heart of cubical type theory.
    /// comp (composition), fill (filling), hfill (homogeneous filling).
    KanOps {
        comp: String,
        fill: String,
        hfill: String,
    },

    // === Phase 4: SMT + Effectful Types ===

    /// Effect grading lattice (F*-style): types carry effect annotations.
    /// `Tot ≤ Dv ≤ ML ≤ All` — tracks state, exceptions, divergence in types.
    EffectGrading {
        effect_lattice: String,
        pure: String,
        total: String,
    },
}

/// A category definition: pure mathematical structure.
#[derive(Debug, Clone)]
pub struct CategoryDef {
    pub name: String,
    pub objects: Vec<ObjectDecl>,
    pub morphisms: Vec<MorphismDecl>,
    pub judgments: Vec<JudgmentDecl>,
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

    pub fn has_hoas(&self) -> bool {
        self.structure
            .iter()
            .any(|s| matches!(s, CategoricalStructure::HOASBinding { .. }))
    }

    pub fn has_tactic_combinators(&self) -> bool {
        self.structure
            .iter()
            .any(|s| matches!(s, CategoricalStructure::TacticCombinators { .. }))
    }

    pub fn has_state_configuration(&self) -> bool {
        self.structure
            .iter()
            .any(|s| matches!(s, CategoricalStructure::StateConfiguration { .. }))
    }

    pub fn has_contextual_type(&self) -> bool {
        self.structure
            .iter()
            .any(|s| matches!(s, CategoricalStructure::ContextualType { .. }))
    }

    pub fn has_cohesive_modality(&self) -> bool {
        self.structure
            .iter()
            .any(|s| matches!(s, CategoricalStructure::CohesiveModality { .. }))
    }

    pub fn has_face_lattice(&self) -> bool {
        self.structure
            .iter()
            .any(|s| matches!(s, CategoricalStructure::FaceLattice { .. }))
    }

    pub fn has_glue_type(&self) -> bool {
        self.structure
            .iter()
            .any(|s| matches!(s, CategoricalStructure::GlueType { .. }))
    }

    pub fn has_kan_ops(&self) -> bool {
        self.structure
            .iter()
            .any(|s| matches!(s, CategoricalStructure::KanOps { .. }))
    }

    pub fn has_effect_grading(&self) -> bool {
        self.structure
            .iter()
            .any(|s| matches!(s, CategoricalStructure::EffectGrading { .. }))
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
    let mut judgments = Vec::new();
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
            "Judgment" => {
                judgments.push(parse_judgment(inner)?);
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
            "IntervalSort" => {
                let interval = get_keyword_atom(inner, ":interval", "IntervalSort")
                    .unwrap_or_else(|_| get_required_atom(inner, 1, "IntervalSort", "name").unwrap_or_default());
                let endpoints = get_keyword_list(inner, ":endpoints", "IntervalSort").unwrap_or_default();
                let default_i0 = "i0".to_string();
                let default_i1 = "i1".to_string();
                let i0 = endpoints.first().unwrap_or(&default_i0).to_string();
                let i1 = endpoints.get(1).unwrap_or(&default_i1).to_string();
                structure.push(CategoricalStructure::IntervalSort {
                    interval,
                    i0,
                    i1,
                });
            }
            "HOASBinding" => {
                let binder = get_required_atom(inner, 1, "HOASBinding", "binder name")?;
                let object_sort = get_keyword_atom(inner, ":object", "HOASBinding")?;
                structure.push(CategoricalStructure::HOASBinding { binder, object_sort });
            }
            "TacticCombinators" => {
                let then = get_keyword_atom(inner, ":then", "TacticCombinators")?;
                let orelse = get_keyword_atom(inner, ":orelse", "TacticCombinators")?;
                let repeat = get_keyword_atom(inner, ":repeat", "TacticCombinators")?;
                let try_tac = get_keyword_atom(inner, ":try", "TacticCombinators")?;
                let focus = get_keyword_atom(inner, ":focus", "TacticCombinators")?;
                structure.push(CategoricalStructure::TacticCombinators {
                    then, orelse, repeat, try_tac, focus,
                });
            }
            "StateConfiguration" => {
                let cell_sort = get_keyword_atom(inner, ":cell", "StateConfiguration")?;
                let merge = get_keyword_atom(inner, ":merge", "StateConfiguration")?;
                structure.push(CategoricalStructure::StateConfiguration { cell_sort, merge });
            }
            "ContextualType" => {
                let context_sort = get_keyword_atom(inner, ":context", "ContextualType")?;
                let term_sort = get_keyword_atom(inner, ":term", "ContextualType")?;
                structure.push(CategoricalStructure::ContextualType { context_sort, term_sort });
            }
            "CohesiveModality" => {
                let shape = get_keyword_atom(inner, ":shape", "CohesiveModality")?;
                let flat = get_keyword_atom(inner, ":flat", "CohesiveModality")?;
                let sharp = get_keyword_atom(inner, ":sharp", "CohesiveModality")?;
                structure.push(CategoricalStructure::CohesiveModality { shape, flat, sharp });
            }
            "FaceLattice" => {
                let meet = get_keyword_atom(inner, ":meet", "FaceLattice")?;
                let join = get_keyword_atom(inner, ":join", "FaceLattice")?;
                let neg = get_keyword_atom(inner, ":neg", "FaceLattice")?;
                structure.push(CategoricalStructure::FaceLattice { meet, join, neg });
            }
            "GlueType" => {
                let glue = get_keyword_atom(inner, ":glue", "GlueType")?;
                let unglue = get_keyword_atom(inner, ":unglue", "GlueType")?;
                structure.push(CategoricalStructure::GlueType { glue, unglue });
            }
            "KanOps" => {
                let comp = get_keyword_atom(inner, ":comp", "KanOps")?;
                let fill = get_keyword_atom(inner, ":fill", "KanOps")?;
                let hfill = get_keyword_atom(inner, ":hfill", "KanOps")?;
                structure.push(CategoricalStructure::KanOps { comp, fill, hfill });
            }
            "EffectGrading" => {
                let effect_lattice = get_keyword_atom(inner, ":lattice", "EffectGrading")?;
                let pure = get_keyword_atom(inner, ":pure", "EffectGrading")?;
                let total = get_keyword_atom(inner, ":total", "EffectGrading")?;
                structure.push(CategoricalStructure::EffectGrading { effect_lattice, pure, total });
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
        judgments,
        structure,
    })
}

fn parse_judgment(inner: &[Sexp]) -> Result<JudgmentDecl> {
    let name = get_required_atom(inner, 1, "Judgment", "name")?;
    let inputs = get_keyword_list(inner, ":inputs", "Judgment")?;
    let output = get_keyword_atom(inner, ":output", "Judgment").ok();
    Ok(JudgmentDecl { name, inputs, output })
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
