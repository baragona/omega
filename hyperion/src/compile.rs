use apeiron::parser::{Sexp, Span};

use crate::category::{CategoricalStructure, CategoryDef};
use crate::error::{HyperionError, Result};
use crate::laws;
use crate::substrate::{BarrierMode, Engine, EqualityMode, ResourceMode, SubstrateDef};
use crate::universe::{system_name_for, CompiledUniverse};

/// Compile a Category + Substrate into a CompiledUniverse.
///
/// This is the heart of Hyperion: it verifies compatibility, then generates
/// the Apeiron SystemConfig that hosts the categorical structure.
pub fn compile_universe(
    universe_name: &str,
    cat: &CategoryDef,
    sub: &SubstrateDef,
) -> Result<CompiledUniverse> {
    // Step 1: Compatibility verification
    check_compatibility(cat, sub)?;

    // Step 2: Collect scope names from Context declarations
    let scope_names: Vec<String> = cat
        .structure
        .iter()
        .filter_map(|s| {
            if let CategoricalStructure::ContextDecl { name } = s {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();

    let sys_name = system_name_for(&cat.name, &sub.name);

    Ok(CompiledUniverse {
        name: universe_name.to_string(),
        system_name: sys_name,
        scope_names,
        category_name: cat.name.clone(),
        substrate_name: sub.name.clone(),
    })
}

/// Check if a substrate uses the Von Neumann engine.
pub fn is_von_neumann(sub: &SubstrateDef) -> bool {
    sub.engine == Engine::VonNeumann
}

/// Verify that the substrate's physics can host the category's math.
fn check_compatibility(cat: &CategoryDef, sub: &SubstrateDef) -> Result<()> {
    // Von Neumann rejects higher-order features
    if is_von_neumann(sub) {
        if cat.has_exponential() {
            return Err(HyperionError::Incompatible {
                category: cat.name.clone(),
                substrate: sub.name.clone(),
                detail: "Von Neumann engine does not support Exponential (no lambda at hardware level)".into(),
            });
        }
        if cat.has_modal_operator() || cat.has_context() {
            return Err(HyperionError::Incompatible {
                category: cat.name.clone(),
                substrate: sub.name.clone(),
                detail: "Von Neumann engine does not support ModalOperator/Context (no scope isolation)".into(),
            });
        }
        if cat.has_tensor() {
            return Err(HyperionError::Incompatible {
                category: cat.name.clone(),
                substrate: sub.name.clone(),
                detail: "Von Neumann engine does not support TensorProduct (no parallel composition in sequential model)".into(),
            });
        }
    }
    // Exponential + Evaluator requires lambda+beta capable engines
    if cat.has_exponential() || cat.has_evaluator() {
        let supports_lambda = matches!(
            sub.engine,
            Engine::InteractionGraph | Engine::TermTree | Engine::AbstractMachine
        );
        if !supports_lambda {
            return Err(HyperionError::Incompatible {
                category: cat.name.clone(),
                substrate: sub.name.clone(),
                detail: format!(
                    "Category '{}' requires Exponential support, but Substrate '{}' uses {:?} engine which has no lambda abstraction",
                    cat.name, sub.name, sub.engine
                ),
            });
        }
    }

    // ModalOperator + Context requires scope isolation
    if cat.has_modal_operator() || cat.has_context() {
        let supports_scopes = matches!(
            sub.barrier,
            BarrierMode::ContextualMembranes | BarrierMode::Cryptographic
        );
        if !supports_scopes {
            return Err(HyperionError::Incompatible {
                category: cat.name.clone(),
                substrate: sub.name.clone(),
                detail: format!(
                    "Category '{}' requires scope isolation (ModalOperator/Context), but Substrate '{}' uses {:?} barrier which provides no scope isolation",
                    cat.name, sub.name, sub.barrier
                ),
            });
        }
    }

    // TensorProduct requires parallel composition
    if cat.has_tensor() {
        let supports_tensor = matches!(
            sub.engine,
            Engine::InteractionGraph | Engine::SymmetricMonoidal
        );
        if !supports_tensor {
            return Err(HyperionError::Incompatible {
                category: cat.name.clone(),
                substrate: sub.name.clone(),
                detail: format!(
                    "Category '{}' requires TensorProduct support, but Substrate '{}' uses {:?} engine which has no parallel composition",
                    cat.name, sub.name, sub.engine
                ),
            });
        }
    }

    // StrictlyLinear + Exponential is impossible (linear can't duplicate closures)
    if sub.resource_mode == ResourceMode::StrictlyLinear && cat.has_exponential() {
        return Err(HyperionError::Incompatible {
            category: cat.name.clone(),
            substrate: sub.name.clone(),
            detail: format!(
                "Category '{}' uses Exponential (closures), but Substrate '{}' is strictly-linear (no duplication allowed)",
                cat.name, sub.name
            ),
        });
    }

    // PathType with Evaluator requires lambda-capable engine (ap-refl rule uses app).
    // PathType without Evaluator is purely first-order (refl, concat, inv, ap are just constructors).
    if cat.has_path_type() && cat.has_evaluator() {
        let supports_lambda = matches!(
            sub.engine,
            Engine::InteractionGraph | Engine::TermTree | Engine::AbstractMachine
        );
        if !supports_lambda {
            return Err(HyperionError::Incompatible {
                category: cat.name.clone(),
                substrate: sub.name.clone(),
                detail: format!(
                    "Category '{}' requires PathType+Evaluator support, but Substrate '{}' uses {:?} engine which has no lambda abstraction",
                    cat.name, sub.name, sub.engine
                ),
            });
        }
    }

    // TopologicalHomotopy requires lambda-capable engine (paths need higher-order structure)
    if sub.equality == EqualityMode::TopologicalHomotopy {
        let supports_lambda = matches!(
            sub.engine,
            Engine::InteractionGraph | Engine::TermTree | Engine::AbstractMachine
        );
        if !supports_lambda {
            return Err(HyperionError::Incompatible {
                category: cat.name.clone(),
                substrate: sub.name.clone(),
                detail: format!(
                    "Substrate '{}' uses topological-homotopy equality, but {:?} engine cannot represent path spaces (requires lambda-capable engine)",
                    sub.name, sub.engine
                ),
            });
        }
    }

    Ok(())
}

/// Determine the Apeiron binding mode from category + substrate.
fn binding_mode(cat: &CategoryDef, sub: &SubstrateDef) -> &'static str {
    if cat.has_modal_operator() && matches!(sub.barrier, BarrierMode::ContextualMembranes) {
        "contextual"
    } else if matches!(sub.resource_mode, ResourceMode::StrictlyLinear) {
        "linear-explicit"
    } else if matches!(sub.resource_mode, ResourceMode::Affine) {
        "linear-explicit"
    } else if cat.has_exponential() && matches!(sub.engine, Engine::InteractionGraph) {
        "implicit"
    } else if matches!(sub.engine, Engine::TermTree) {
        "exposed"
    } else {
        "implicit"
    }
}

/// Determine the Apeiron check modes from substrate.
fn check_modes(sub: &SubstrateDef) -> Vec<&'static str> {
    match sub.equality {
        EqualityMode::RewriteEquivalence => vec!["rewriting", "beta-reduction"],
        EqualityMode::TopologicalHash => vec!["oracle"],
        EqualityMode::Unification => vec!["pattern-unification"],
        EqualityMode::AlphaEquivalence => vec!["beta-reduction"],
        EqualityMode::Observational => vec!["rewriting", "beta-reduction"],
        EqualityMode::TopologicalHomotopy => vec!["rewriting", "beta-reduction", "eta"],
    }
}

/// Generate categorical laws for a category.
pub fn generate_category_laws(cat: &CategoryDef) -> Vec<laws::CategoricalLaw> {
    laws::generate_laws(cat)
}

/// Build a [Proofs] sexp for law checking.
pub fn build_law_proofs_sexp(
    theory_name: &str,
    cat: &CategoryDef,
) -> Option<Sexp> {
    let category_laws = laws::generate_laws(cat);
    if category_laws.is_empty() {
        return None;
    }
    // Use first object as witness sort
    let witness_sort = cat.objects.first().map(|o| o.name.as_str());
    laws::build_law_proofs(theory_name, &category_laws, witness_sort)
}

/// Generate the Apeiron [System ...] S-expression for a compiled universe.
pub fn emit_system_sexp(
    cat: &CategoryDef,
    sub: &SubstrateDef,
    compiled: &CompiledUniverse,
) -> Sexp {
    let sp = Span::default();
    let mut system_items: Vec<Sexp> = Vec::new();

    // [System __hyp_Cat_Sub ...]
    system_items.push(Sexp::Atom("System".into(), sp));
    system_items.push(Sexp::Atom(compiled.system_name.clone(), sp));

    // [@syntax ...] block
    let mut syntax_items: Vec<Sexp> = Vec::new();
    syntax_items.push(Sexp::Atom("@syntax".into(), sp));

    // Sorts from objects
    for obj in &cat.objects {
        syntax_items.push(Sexp::List(
            vec![
                Sexp::Atom("sort".into(), sp),
                Sexp::Atom(obj.name.clone(), sp),
            ],
            sp,
        ));
    }

    // Operators from morphisms
    for morph in &cat.morphisms {
        syntax_items.push(Sexp::List(
            vec![
                Sexp::Atom("op".into(), sp),
                Sexp::Atom(morph.name.clone(), sp),
            ],
            sp,
        ));
    }

    // Operators from structure
    for s in &cat.structure {
        match s {
            CategoricalStructure::Exponential { name, .. } => {
                syntax_items.push(Sexp::List(
                    vec![
                        Sexp::Atom("op".into(), sp),
                        Sexp::Atom(name.clone(), sp),
                    ],
                    sp,
                ));
            }
            CategoricalStructure::Evaluator { name } => {
                // Only add if not already a morphism with the same name
                let already = cat.morphisms.iter().any(|m| m.name == *name);
                if !already {
                    syntax_items.push(Sexp::List(
                        vec![
                            Sexp::Atom("op".into(), sp),
                            Sexp::Atom(name.clone(), sp),
                        ],
                        sp,
                    ));
                }
            }
            CategoricalStructure::ModalOperator { name } => {
                syntax_items.push(Sexp::List(
                    vec![
                        Sexp::Atom("op".into(), sp),
                        Sexp::Atom(name.clone(), sp),
                    ],
                    sp,
                ));
            }
            CategoricalStructure::TensorProduct { name } => {
                syntax_items.push(Sexp::List(
                    vec![
                        Sexp::Atom("op".into(), sp),
                        Sexp::Atom(name.clone(), sp),
                    ],
                    sp,
                ));
            }
            CategoricalStructure::Unit { name } => {
                syntax_items.push(Sexp::List(
                    vec![
                        Sexp::Atom("op".into(), sp),
                        Sexp::Atom(name.clone(), sp),
                    ],
                    sp,
                ));
            }
            CategoricalStructure::ContextDecl { .. } => {
                // Contexts become Scope declarations in Theory, not ops
            }
            CategoricalStructure::Preorder { relation: _ } => {
                // Inject `true` op for reflexivity result (if not already present)
                let true_name = "true";
                let already = cat.morphisms.iter().any(|m| m.name == true_name)
                    || syntax_items.iter().any(|s| {
                        s.as_list()
                            .and_then(|l| l.get(1))
                            .and_then(|s| s.as_atom())
                            .map(|a| a == true_name)
                            .unwrap_or(false)
                    });
                if !already {
                    syntax_items.push(Sexp::List(
                        vec![
                            Sexp::Atom("op".into(), sp),
                            Sexp::Atom(true_name.into(), sp),
                        ],
                        sp,
                    ));
                }
            }
            CategoricalStructure::PathType { refl, concat, inv, ap } => {
                // Inject path algebra ops (only if not already a morphism)
                for op_name in [refl, concat, inv, ap] {
                    let already = cat.morphisms.iter().any(|m| m.name == *op_name)
                        || syntax_items.iter().any(|s| {
                            s.as_list()
                                .and_then(|l| l.get(1))
                                .and_then(|s| s.as_atom())
                                .map(|a| a == op_name)
                                .unwrap_or(false)
                        });
                    if !already {
                        syntax_items.push(Sexp::List(
                            vec![
                                Sexp::Atom("op".into(), sp),
                                Sexp::Atom(op_name.clone(), sp),
                            ],
                            sp,
                        ));
                    }
                }
            }
        }
    }

    system_items.push(Sexp::List(syntax_items, sp));

    // [@binding ...] block
    let bmode = binding_mode(cat, sub);
    system_items.push(Sexp::List(
        vec![
            Sexp::Atom("@binding".into(), sp),
            Sexp::Atom(bmode.into(), sp),
        ],
        sp,
    ));

    // [@check ...] block
    let cmodes = check_modes(sub);
    let mut check_items: Vec<Sexp> = Vec::new();
    check_items.push(Sexp::Atom("@check".into(), sp));
    for mode in cmodes {
        check_items.push(Sexp::Atom(mode.into(), sp));
    }
    system_items.push(Sexp::List(check_items, sp));

    Sexp::List(system_items, sp)
}

/// Generate the morphism name for a functor applied to a specific category.
pub fn morphism_name_for(functor_name: &str, category_name: &str) -> String {
    format!("__fun_{}_{}", functor_name, category_name)
}

/// Generate an Apeiron `[AutoMorphism name source target [Map a b] ...]` S-expression.
pub fn emit_morphism_sexp(
    name: &str,
    source_system: &str,
    target_system: &str,
    op_maps: &[(String, String)],
) -> Sexp {
    let sp = Span::default();
    let mut items: Vec<Sexp> = Vec::new();

    items.push(Sexp::Atom("AutoMorphism".into(), sp));
    items.push(Sexp::Atom(name.into(), sp));
    items.push(Sexp::Atom(source_system.into(), sp));
    items.push(Sexp::Atom(target_system.into(), sp));

    for (src, tgt) in op_maps {
        items.push(Sexp::List(
            vec![
                Sexp::Atom("Map".into(), sp),
                Sexp::Atom(src.clone(), sp),
                Sexp::Atom(tgt.clone(), sp),
            ],
            sp,
        ));
    }

    Sexp::List(items, sp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::category::{MorphismDecl, ObjectDecl};
    use crate::substrate::{BarrierMode, Engine, EqualityMode, ResourceMode, SubstrateDef};

    fn make_ccc() -> CategoryDef {
        CategoryDef {
            name: "CartesianClosed".into(),
            objects: vec![
                ObjectDecl {
                    name: "Type".into(),
                },
                ObjectDecl {
                    name: "Term".into(),
                },
            ],
            morphisms: vec![
                MorphismDecl {
                    name: "arrow".into(),
                    domain: vec!["Type".into(), "Type".into()],
                    codomain: "Type".into(),
                },
                MorphismDecl {
                    name: "app".into(),
                    domain: vec!["Term".into(), "Term".into()],
                    codomain: "Term".into(),
                },
            ],
            structure: vec![
                CategoricalStructure::Exponential {
                    name: "lam".into(),
                    object: "Term".into(),
                },
                CategoricalStructure::Evaluator {
                    name: "app".into(),
                },
            ],
        }
    }

    fn make_inet() -> SubstrateDef {
        SubstrateDef {
            name: "InteractionNet".into(),
            engine: Engine::InteractionGraph,
            resource_mode: ResourceMode::OptimalSharing,
            barrier: BarrierMode::Transparent,
            equality: EqualityMode::TopologicalHash,
        }
    }

    #[test]
    fn ccc_on_inet_compiles() {
        let cat = make_ccc();
        let sub = make_inet();
        let result = compile_universe("WeakLF", &cat, &sub);
        assert!(result.is_ok());
        let compiled = result.unwrap();
        assert_eq!(compiled.system_name, "__hyp_CartesianClosed_InteractionNet");
    }

    #[test]
    fn ccc_on_cellular_automaton_fails() {
        let cat = make_ccc();
        let sub = SubstrateDef {
            name: "GridWorld".into(),
            engine: Engine::CellularAutomaton,
            resource_mode: ResourceMode::DeepCopy,
            barrier: BarrierMode::Transparent,
            equality: EqualityMode::RewriteEquivalence,
        };
        let result = compile_universe("Bad", &cat, &sub);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("Exponential support"));
    }

    #[test]
    fn strictly_linear_no_exponential() {
        let cat = make_ccc();
        let sub = SubstrateDef {
            name: "LinearNet".into(),
            engine: Engine::InteractionGraph,
            resource_mode: ResourceMode::StrictlyLinear,
            barrier: BarrierMode::Transparent,
            equality: EqualityMode::TopologicalHash,
        };
        let result = compile_universe("Bad", &cat, &sub);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("strictly-linear"));
    }

    #[test]
    fn modal_requires_membranes() {
        let cat = CategoryDef {
            name: "Modal".into(),
            objects: vec![ObjectDecl {
                name: "Prop".into(),
            }],
            morphisms: vec![],
            structure: vec![
                CategoricalStructure::ModalOperator {
                    name: "box".into(),
                },
                CategoricalStructure::ContextDecl {
                    name: "W".into(),
                },
            ],
        };
        let sub = make_inet(); // barrier = Transparent
        let result = compile_universe("Bad", &cat, &sub);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("scope isolation"));
    }

    #[test]
    fn emit_system_sexp_produces_valid_structure() {
        let cat = make_ccc();
        let sub = make_inet();
        let compiled = compile_universe("WeakLF", &cat, &sub).unwrap();
        let sexp = emit_system_sexp(&cat, &sub, &compiled);

        // Should be a list starting with "System"
        let items = sexp.as_list().unwrap();
        assert_eq!(items[0].as_atom().unwrap(), "System");
        assert_eq!(
            items[1].as_atom().unwrap(),
            "__hyp_CartesianClosed_InteractionNet"
        );
    }

    #[test]
    fn emit_morphism_sexp_no_maps() {
        let sexp = emit_morphism_sexp("__fun_F_Cat", "__hyp_Cat_A", "__hyp_Cat_B", &[]);
        let items = sexp.as_list().unwrap();
        assert_eq!(items[0].as_atom().unwrap(), "AutoMorphism");
        assert_eq!(items[1].as_atom().unwrap(), "__fun_F_Cat");
        assert_eq!(items[2].as_atom().unwrap(), "__hyp_Cat_A");
        assert_eq!(items[3].as_atom().unwrap(), "__hyp_Cat_B");
        assert_eq!(items.len(), 4); // No Map entries
    }

    #[test]
    fn emit_morphism_sexp_with_maps() {
        let maps = vec![
            ("z".to_string(), "zero".to_string()),
            ("s".to_string(), "succ".to_string()),
        ];
        let sexp = emit_morphism_sexp("morph", "src_sys", "tgt_sys", &maps);
        let items = sexp.as_list().unwrap();
        assert_eq!(items.len(), 6); // AutoMorphism + name + src + tgt + 2 Maps

        // First Map entry
        let map1 = items[4].as_list().unwrap();
        assert_eq!(map1[0].as_atom().unwrap(), "Map");
        assert_eq!(map1[1].as_atom().unwrap(), "z");
        assert_eq!(map1[2].as_atom().unwrap(), "zero");

        // Second Map entry
        let map2 = items[5].as_list().unwrap();
        assert_eq!(map2[0].as_atom().unwrap(), "Map");
        assert_eq!(map2[1].as_atom().unwrap(), "s");
        assert_eq!(map2[2].as_atom().unwrap(), "succ");
    }

    #[test]
    fn morphism_name_generation() {
        assert_eq!(morphism_name_for("F", "Cat"), "__fun_F_Cat");
        assert_eq!(
            morphism_name_for("NetToTree", "SimpleMath"),
            "__fun_NetToTree_SimpleMath"
        );
    }
}
