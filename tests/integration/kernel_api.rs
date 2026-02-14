/// Kernel API tests: construct theories and proofs programmatically.
use omega_core::derivation::{Context, Derivation};
use omega_core::expr::Expr;
use omega_core::judgment::{ConstructorDecl, JudgmentForm, Rule, SortDecl};
use omega_core::kernel::Kernel;
use omega_core::metatheorem::{MetaCase, MetaProof, MetaTheorem};
use omega_core::reflection;
use omega_core::theory::Theory;

fn make_minimal_logic() -> Theory {
    let mut theory = Theory::new("MinLogic");

    theory.add_sort(SortDecl {
        name: "Prop".to_string(),
    });
    theory.add_constructor(ConstructorDecl {
        name: "and".to_string(),
        ty: Expr::app(vec![
            Expr::sym("->"),
            Expr::sym("Prop"),
            Expr::sym("Prop"),
            Expr::sym("Prop"),
        ]),

    });
    theory.add_judgment(JudgmentForm {
        name: "proves".to_string(),
        pattern: Expr::app(vec![Expr::sym("proves"), Expr::meta("P")]),
        constraints: vec![("P".to_string(), "Prop".to_string())],
    });

    theory.push_rule(Rule::new(
        "and-intro",
        vec![
            Expr::app(vec![Expr::sym("proves"), Expr::meta("A")]),
            Expr::app(vec![Expr::sym("proves"), Expr::meta("B")]),
        ],
        Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
        ]),
    ));

    theory.push_rule(Rule::new(
        "and-elim-l",
        vec![Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
        ])],
        Expr::app(vec![Expr::sym("proves"), Expr::meta("A")]),
    ));

    theory.push_rule(Rule::new(
        "and-elim-r",
        vec![Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
        ])],
        Expr::app(vec![Expr::sym("proves"), Expr::meta("B")]),
    ));

    theory.compute_hash();
    theory
}

#[test]
fn kernel_register_and_check() {
    let mut kernel = Kernel::new();
    kernel.register_theory(make_minimal_logic()).unwrap();

    // Prove: (and p q) from assumptions
    let goal = Expr::app(vec![
        Expr::sym("proves"),
        Expr::app(vec![Expr::sym("and"), Expr::free("p"), Expr::free("q")]),
    ]);
    let ctx = Context::with_assumptions(vec![
        Expr::app(vec![Expr::sym("proves"), Expr::free("p")]),
        Expr::app(vec![Expr::sym("proves"), Expr::free("q")]),
    ]);
    let deriv = Derivation::RuleApp {
        rule_name: "and-intro".to_string(),
        premises: vec![Derivation::Assumption, Derivation::Assumption],
    };
    kernel
        .check_derivation("MinLogic", &goal, &deriv, &ctx)
        .unwrap();
}

#[test]
fn kernel_full_reflection_pipeline() {
    let mut kernel = Kernel::new();
    kernel.register_theory(make_minimal_logic()).unwrap();

    // 1. Prove a metatheorem: and-comm
    let mt = MetaTheorem {
        name: "and-comm".to_string(),
        theory_name: "MinLogic".to_string(),
        forall: vec![(
            "D".to_string(),
            Expr::app(vec![
                Expr::sym("proves"),
                Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
            ]),
        )],
        exists: vec![(
            "D'".to_string(),
            Expr::app(vec![
                Expr::sym("proves"),
                Expr::app(vec![Expr::sym("and"), Expr::meta("B"), Expr::meta("A")]),
            ]),
        )],
        proof: MetaProof::CaseAnalysis {
            scrutinee: "D".to_string(),
            cases: vec![MetaCase {
                rule_name: "and-intro".to_string(),
                premise_names: vec!["D1".to_string(), "D2".to_string()],
                body: MetaProof::ByRule {
                    rule_name: "and-intro".to_string(),
                    args: vec![
                        MetaProof::Var("D2".to_string()),
                        MetaProof::Var("D1".to_string()),
                    ],
                },
            }],
        },
    };
    kernel.check_metatheorem(mt).unwrap();

    // 2. Reflect it (driver-level operation)
    let mt = kernel.get_verified_metatheorem("and-comm").unwrap().clone();
    let theory = kernel.get_theory("MinLogic").unwrap();
    let (rule, _record) = reflection::reflect(&mt, "proves/and-comm", theory).unwrap();
    kernel.add_rule("MinLogic", rule).unwrap();

    // 3. Use the reflected rule
    let goal = Expr::app(vec![
        Expr::sym("proves"),
        Expr::app(vec![Expr::sym("and"), Expr::free("b"), Expr::free("a")]),
    ]);
    let ctx = Context::with_assumptions(vec![Expr::app(vec![
        Expr::sym("proves"),
        Expr::app(vec![Expr::sym("and"), Expr::free("a"), Expr::free("b")]),
    ])]);
    let deriv = Derivation::RuleApp {
        rule_name: "proves/and-comm".to_string(),
        premises: vec![Derivation::Assumption],
    };
    kernel
        .check_derivation("MinLogic", &goal, &deriv, &ctx)
        .unwrap();
}

#[test]
fn kernel_reject_invalid_derivation() {
    let mut kernel = Kernel::new();
    kernel.register_theory(make_minimal_logic()).unwrap();

    // Try to prove (and p q) with only one assumption
    let goal = Expr::app(vec![
        Expr::sym("proves"),
        Expr::app(vec![Expr::sym("and"), Expr::free("p"), Expr::free("q")]),
    ]);
    let ctx = Context::with_assumptions(vec![
        Expr::app(vec![Expr::sym("proves"), Expr::free("p")]),
        // Missing (proves q)!
    ]);
    let deriv = Derivation::RuleApp {
        rule_name: "and-intro".to_string(),
        premises: vec![Derivation::Assumption, Derivation::Assumption],
    };
    assert!(kernel
        .check_derivation("MinLogic", &goal, &deriv, &ctx)
        .is_err());
}

#[test]
fn kernel_multi_step_derivation() {
    let mut kernel = Kernel::new();
    kernel.register_theory(make_minimal_logic()).unwrap();

    // Prove (and q p) from (and p q) using elim + intro
    let goal = Expr::app(vec![
        Expr::sym("proves"),
        Expr::app(vec![Expr::sym("and"), Expr::free("q"), Expr::free("p")]),
    ]);
    let ctx = Context::with_assumptions(vec![Expr::app(vec![
        Expr::sym("proves"),
        Expr::app(vec![Expr::sym("and"), Expr::free("p"), Expr::free("q")]),
    ])]);
    let deriv = Derivation::RuleApp {
        rule_name: "and-intro".to_string(),
        premises: vec![
            Derivation::RuleApp {
                rule_name: "and-elim-r".to_string(),
                premises: vec![Derivation::Assumption],
            },
            Derivation::RuleApp {
                rule_name: "and-elim-l".to_string(),
                premises: vec![Derivation::Assumption],
            },
        ],
    };
    kernel
        .check_derivation("MinLogic", &goal, &deriv, &ctx)
        .unwrap();
}
