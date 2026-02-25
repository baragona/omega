//! Categorical law verification: auto-generate [Proofs] blocks with assert-eq
//! for foundational categorical laws, and verify them via Apeiron.
//!
//! Laws are tested on irreducible witness atoms (__law_a, etc.) plus structural
//! witnesses that exercise rule interactions (nested tensor+unit, etc.).
//! The output honestly reports the number of witness tests performed.

use apeiron::parser::{Sexp, Span};

use crate::category::{CategoricalStructure, CategoryDef};

/// A categorical law to verify: an assert-eq between two expressions.
pub struct CategoricalLaw {
    pub name: String,
    pub lhs: Sexp,
    pub rhs: Sexp,
}

/// Generate categorical laws for a given category definition.
/// Returns laws that should hold for any theory in a universe with this category.
/// Includes both flat witness tests and structural witness tests that exercise
/// rule interactions.
pub fn generate_laws(cat: &CategoryDef) -> Vec<CategoricalLaw> {
    let mut laws = Vec::new();

    // Collect structure names for law generation
    let mut tensor_name: Option<&str> = None;
    let mut unit_name: Option<&str> = None;
    let mut exp_name: Option<&str> = None;
    let mut exp_object: Option<&str> = None;
    let mut eval_name: Option<&str> = None;

    for s in &cat.structure {
        match s {
            CategoricalStructure::TensorProduct { name } => tensor_name = Some(name),
            CategoricalStructure::Unit { name } => unit_name = Some(name),
            CategoricalStructure::Exponential { name, object } => {
                exp_name = Some(name);
                exp_object = Some(object);
            }
            CategoricalStructure::Evaluator { name } => eval_name = Some(name),
            _ => {}
        }
    }

    // SymmetricMonoidal laws: associativity + unit laws
    if let (Some(t), Some(u)) = (tensor_name, unit_name) {
        laws.extend(monoidal_laws(t, u));
        laws.extend(monoidal_structural_witnesses(t, u));
    }

    // CartesianClosed laws: beta reduction
    if let (Some(lam), Some(_obj), Some(app)) = (exp_name, exp_object, eval_name) {
        laws.extend(ccc_laws(lam, app));
        laws.extend(ccc_structural_witnesses(lam, app));
    }

    // PathType laws (if present)
    for s in &cat.structure {
        if let CategoricalStructure::PathType { refl, concat, inv, ap } = s {
            laws.extend(path_type_laws(refl, concat, inv, ap, eval_name));
        }
    }

    // Preorder laws (if present)
    for s in &cat.structure {
        if let CategoricalStructure::Preorder { relation } = s {
            laws.extend(preorder_laws(relation));
        }
    }

    laws
}

/// Monoidal category laws using witness constants a, b, c.
fn monoidal_laws(tensor: &str, unit: &str) -> Vec<CategoricalLaw> {
    let sp = Span::default();

    let a = || Sexp::Atom("__law_a".into(), sp);
    let b = || Sexp::Atom("__law_b".into(), sp);
    let c = || Sexp::Atom("__law_c".into(), sp);

    let t = |x: Sexp, y: Sexp| -> Sexp {
        Sexp::List(vec![Sexp::Atom(tensor.into(), sp), x, y], sp)
    };
    let u = || Sexp::Atom(unit.into(), sp);

    vec![
        // Associativity: tensor(tensor(a, b), c) = tensor(a, tensor(b, c))
        CategoricalLaw {
            name: "monoidal-assoc".into(),
            lhs: t(t(a(), b()), c()),
            rhs: t(a(), t(b(), c())),
        },
        // Left unit: tensor(unit, a) = a
        CategoricalLaw {
            name: "monoidal-left-unit".into(),
            lhs: t(u(), a()),
            rhs: a(),
        },
        // Right unit: tensor(a, unit) = a
        CategoricalLaw {
            name: "monoidal-right-unit".into(),
            lhs: t(a(), u()),
            rhs: a(),
        },
    ]
}

/// Structural witnesses for monoidal laws: exercise rule interactions.
fn monoidal_structural_witnesses(tensor: &str, unit: &str) -> Vec<CategoricalLaw> {
    let sp = Span::default();

    let a = || Sexp::Atom("__law_a".into(), sp);
    let b = || Sexp::Atom("__law_b".into(), sp);

    let t = |x: Sexp, y: Sexp| -> Sexp {
        Sexp::List(vec![Sexp::Atom(tensor.into(), sp), x, y], sp)
    };
    let u = || Sexp::Atom(unit.into(), sp);

    vec![
        // Nested: tensor(tensor(a, b), unit) = tensor(a, b) — tests assoc + unit interaction
        CategoricalLaw {
            name: "monoidal-nested-right-unit".into(),
            lhs: t(t(a(), b()), u()),
            rhs: t(a(), b()),
        },
        // Mixed: tensor(unit, tensor(a, unit)) = a — tests left-unit + right-unit interaction
        CategoricalLaw {
            name: "monoidal-mixed-unit".into(),
            lhs: t(u(), t(a(), u())),
            rhs: a(),
        },
        // Double unit: tensor(unit, unit) = unit
        CategoricalLaw {
            name: "monoidal-double-unit".into(),
            lhs: t(u(), u()),
            rhs: u(),
        },
    ]
}

/// Cartesian closed category laws: beta reduction.
fn ccc_laws(lam: &str, app: &str) -> Vec<CategoricalLaw> {
    let sp = Span::default();

    let a = || Sexp::Atom("__law_a".into(), sp);

    // Beta: app(lam(x, x), a) = a  (identity function applied)
    vec![CategoricalLaw {
        name: "ccc-beta".into(),
        lhs: Sexp::List(
            vec![
                Sexp::Atom(app.into(), sp),
                Sexp::List(
                    vec![
                        Sexp::Atom(lam.into(), sp),
                        Sexp::Atom("__law_x".into(), sp),
                        Sexp::Atom("__law_x".into(), sp),
                    ],
                    sp,
                ),
                a(),
            ],
            sp,
        ),
        rhs: a(),
    }]
}

/// Structural witnesses for CCC laws: exercise nested beta interactions.
fn ccc_structural_witnesses(lam: &str, app: &str) -> Vec<CategoricalLaw> {
    let sp = Span::default();

    let a = || Sexp::Atom("__law_a".into(), sp);
    let b = || Sexp::Atom("__law_b".into(), sp);

    vec![
        // Constant function: app(lam(x, a), b) = a
        CategoricalLaw {
            name: "ccc-beta-const".into(),
            lhs: Sexp::List(
                vec![
                    Sexp::Atom(app.into(), sp),
                    Sexp::List(
                        vec![
                            Sexp::Atom(lam.into(), sp),
                            Sexp::Atom("__law_x".into(), sp),
                            a(),
                        ],
                        sp,
                    ),
                    b(),
                ],
                sp,
            ),
            rhs: a(),
        },
        // Nested beta: app(lam(x, app(lam(y, y), x)), a) = a
        CategoricalLaw {
            name: "ccc-beta-nested".into(),
            lhs: Sexp::List(
                vec![
                    Sexp::Atom(app.into(), sp),
                    Sexp::List(
                        vec![
                            Sexp::Atom(lam.into(), sp),
                            Sexp::Atom("__law_x".into(), sp),
                            Sexp::List(
                                vec![
                                    Sexp::Atom(app.into(), sp),
                                    Sexp::List(
                                        vec![
                                            Sexp::Atom(lam.into(), sp),
                                            Sexp::Atom("__law_y".into(), sp),
                                            Sexp::Atom("__law_y".into(), sp),
                                        ],
                                        sp,
                                    ),
                                    Sexp::Atom("__law_x".into(), sp),
                                ],
                                sp,
                            ),
                        ],
                        sp,
                    ),
                    a(),
                ],
                sp,
            ),
            rhs: a(),
        },
    ]
}

/// Path algebra laws for PathType categories.
fn path_type_laws(refl: &str, concat: &str, inv: &str, ap: &str, eval_name: Option<&str>) -> Vec<CategoricalLaw> {
    let sp = Span::default();

    let a = || Sexp::Atom("__law_a".into(), sp);
    let p = || Sexp::Atom("__law_p".into(), sp);
    let q = || Sexp::Atom("__law_q".into(), sp);
    let r = || Sexp::Atom("__law_r".into(), sp);

    let mk_refl = |x: Sexp| -> Sexp {
        Sexp::List(vec![Sexp::Atom(refl.into(), sp), x], sp)
    };
    let mk_concat = |x: Sexp, y: Sexp| -> Sexp {
        Sexp::List(vec![Sexp::Atom(concat.into(), sp), x, y], sp)
    };
    let mk_inv = |x: Sexp| -> Sexp {
        Sexp::List(vec![Sexp::Atom(inv.into(), sp), x], sp)
    };

    let mut laws = vec![
        // Left unit: concat(refl(a), p) = p
        CategoricalLaw {
            name: "path-left-unit".into(),
            lhs: mk_concat(mk_refl(a()), p()),
            rhs: p(),
        },
        // Right unit: concat(p, refl(a)) = p
        CategoricalLaw {
            name: "path-right-unit".into(),
            lhs: mk_concat(p(), mk_refl(a())),
            rhs: p(),
        },
        // Inverse of refl: inv(refl(a)) = refl(a)
        CategoricalLaw {
            name: "path-inv-refl".into(),
            lhs: mk_inv(mk_refl(a())),
            rhs: mk_refl(a()),
        },
        // Associativity: concat(concat(p,q), r) = concat(p, concat(q,r))
        CategoricalLaw {
            name: "path-assoc".into(),
            lhs: mk_concat(mk_concat(p(), q()), r()),
            rhs: mk_concat(p(), mk_concat(q(), r())),
        },
    ];

    // ap(f, refl(a)) = refl(app(f, a)) — only if category also has Evaluator
    if let Some(app) = eval_name {
        let f = || Sexp::Atom("__law_f".into(), sp);
        laws.push(CategoricalLaw {
            name: "path-ap-refl".into(),
            lhs: Sexp::List(vec![Sexp::Atom(ap.into(), sp), f(), mk_refl(a())], sp),
            rhs: mk_refl(Sexp::List(vec![Sexp::Atom(app.into(), sp), f(), a()], sp)),
        });
        // ap(f, concat(p, q)) = concat(ap(f, p), ap(f, q)) — functoriality of ap over concat
        let mk_ap = |f: Sexp, x: Sexp| -> Sexp {
            Sexp::List(vec![Sexp::Atom(ap.into(), sp), f, x], sp)
        };
        laws.push(CategoricalLaw {
            name: "path-ap-concat".into(),
            lhs: mk_ap(f(), mk_concat(p(), q())),
            rhs: mk_concat(mk_ap(f(), p()), mk_ap(f(), q())),
        });
    }

    laws
}

/// Preorder laws: reflexivity.
fn preorder_laws(relation: &str) -> Vec<CategoricalLaw> {
    let sp = Span::default();

    let a = || Sexp::Atom("__law_a".into(), sp);
    let b = || Sexp::Atom("__law_b".into(), sp);
    let tr = || Sexp::Atom("true".into(), sp);

    let mk_rel = |x: Sexp, y: Sexp| -> Sexp {
        Sexp::List(vec![Sexp::Atom(relation.into(), sp), x, y], sp)
    };

    vec![
        // Reflexivity: rel(a, a) = true
        CategoricalLaw {
            name: "preorder-refl".into(),
            lhs: mk_rel(a(), a()),
            rhs: tr(),
        },
        // Reflexivity on structured witness: rel(rel(a,b), rel(a,b)) = true
        CategoricalLaw {
            name: "preorder-refl-structured".into(),
            lhs: mk_rel(mk_rel(a(), b()), mk_rel(a(), b())),
            rhs: tr(),
        },
    ]
}

/// Build a [Proofs] Sexp that checks all laws for a theory.
/// The `:in` target is the Apeiron theory name (not the system name).
/// Witness atoms (__law_a, etc.) are used as irreducible constants.
/// Returns None if there are no laws to check.
pub fn build_law_proofs(
    theory_name: &str,
    laws: &[CategoricalLaw],
    _witness_sort: Option<&str>,
) -> Option<Sexp> {
    if laws.is_empty() {
        return None;
    }

    let sp = Span::default();
    let proofs_name = format!("__laws_{}", theory_name);

    let mut items: Vec<Sexp> = Vec::new();
    items.push(Sexp::Atom("Proofs".into(), sp));
    items.push(Sexp::Atom(proofs_name, sp));
    items.push(Sexp::Atom(":in".into(), sp));
    items.push(Sexp::Atom(theory_name.into(), sp));

    // Assert-eq for each law (witness atoms are irreducible — no declaration needed)
    for law in laws {
        items.push(Sexp::List(
            vec![
                Sexp::Atom("assert-eq".into(), sp),
                Sexp::Atom(law.name.clone(), sp),
                law.lhs.clone(),
                law.rhs.clone(),
            ],
            sp,
        ));
    }

    Some(Sexp::List(items, sp))
}
