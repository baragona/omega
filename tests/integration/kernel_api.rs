/// Kernel API tests: construct theories and proofs programmatically.
use omega_core::derivation::{Context, Derivation};
use omega_core::expr::Expr;
use omega_core::judgment::{ConstructorDecl, JudgmentForm, Rule, SortDecl};
use omega_core::kernel::Kernel;
use omega_core::metatheorem::{MetaCase, MetaProof, MetaTheorem};
use omega_core::theory::Theory;

fn make_minimal_logic() -> Theory {
    let mut theory = Theory::new("MinLogic");

    theory.sorts.push(SortDecl {
        name: "Prop".to_string(),
    });
    theory.constructors.push(ConstructorDecl {
        name: "and".to_string(),
        ty: Expr::app(vec![
            Expr::sym("->"),
            Expr::sym("Prop"),
            Expr::sym("Prop"),
            Expr::sym("Prop"),
        ]),
    });
    theory.judgments.push(JudgmentForm {
        name: "proves".to_string(),
        pattern: Expr::app(vec![Expr::sym("proves"), Expr::meta("P")]),
        constraints: vec![("P".to_string(), "Prop".to_string())],
    });

    theory.rules.push(Rule {
        name: "and-intro".to_string(),
        premises: vec![
            Expr::app(vec![Expr::sym("proves"), Expr::meta("A")]),
            Expr::app(vec![Expr::sym("proves"), Expr::meta("B")]),
        ],
        conclusion: Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
        ]),
        reflected: false,
        provenance: None,
        implicit_args: vec![],
        context_extensions: vec![],
    });

    theory.rules.push(Rule {
        name: "and-elim-l".to_string(),
        premises: vec![Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
        ])],
        conclusion: Expr::app(vec![Expr::sym("proves"), Expr::meta("A")]),
        reflected: false,
        provenance: None,
        implicit_args: vec![],
        context_extensions: vec![],
    });

    theory.rules.push(Rule {
        name: "and-elim-r".to_string(),
        premises: vec![Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
        ])],
        conclusion: Expr::app(vec![Expr::sym("proves"), Expr::meta("B")]),
        reflected: false,
        provenance: None,
        implicit_args: vec![],
        context_extensions: vec![],
    });

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

    // 2. Reflect it
    kernel.reflect("and-comm", "proves/and-comm").unwrap();

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
