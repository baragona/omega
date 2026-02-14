/// Shared test fixtures for omega-core and downstream crates.
///
/// Contains canonical theory definitions used across test suites,
/// eliminating duplication of `make_prop_logic()` across 6+ files.
use crate::expr::Expr;
use crate::judgment::{ConstructorDecl, JudgmentForm, Rule, SortDecl};
use crate::theory::Theory;

/// Build the canonical PropLogic theory used by most test suites.
///
/// Contains:
/// - Sort: Prop
/// - Constructors: true, and, imp
/// - Judgment: proves (with Prop constraint)
/// - Rules: and-intro, and-elim-l, and-elim-r, imp-intro, imp-elim
pub fn make_prop_logic() -> Theory {
    let mut theory = Theory::new("PropLogic");

    theory.sorts.push(SortDecl {
        name: "Prop".to_string(),
    });

    theory.constructors.push(ConstructorDecl {
        name: "true".to_string(),
        ty: Expr::sym("Prop"),
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
    theory.constructors.push(ConstructorDecl {
        name: "imp".to_string(),
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

    theory.rules.push(Rule::new(
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

    theory.rules.push(Rule::new(
        "and-elim-l",
        vec![Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
        ])],
        Expr::app(vec![Expr::sym("proves"), Expr::meta("A")]),
    ));

    theory.rules.push(Rule::new(
        "and-elim-r",
        vec![Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
        ])],
        Expr::app(vec![Expr::sym("proves"), Expr::meta("B")]),
    ));

    theory.rules.push(Rule::new(
        "imp-intro",
        vec![Expr::app(vec![Expr::sym("proves"), Expr::meta("B")])],
        Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("imp"), Expr::meta("A"), Expr::meta("B")]),
        ]),
    ));

    theory.rules.push(Rule::new(
        "imp-elim",
        vec![
            Expr::app(vec![
                Expr::sym("proves"),
                Expr::app(vec![Expr::sym("imp"), Expr::meta("A"), Expr::meta("B")]),
            ]),
            Expr::app(vec![Expr::sym("proves"), Expr::meta("A")]),
        ],
        Expr::app(vec![Expr::sym("proves"), Expr::meta("B")]),
    ));

    theory.compute_hash();
    theory
}
