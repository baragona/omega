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
    let mut tb = Theory::builder("PropLogic");

    tb.add_sort(SortDecl::new("Prop"));

    tb.add_constructor(ConstructorDecl::new("true", Expr::sym("Prop")));
    tb.add_constructor(ConstructorDecl::new(
        "and",
        Expr::app(vec![
            Expr::sym("->"),
            Expr::sym("Prop"),
            Expr::sym("Prop"),
            Expr::sym("Prop"),
        ]),
    ));
    tb.add_constructor(ConstructorDecl::new(
        "imp",
        Expr::app(vec![
            Expr::sym("->"),
            Expr::sym("Prop"),
            Expr::sym("Prop"),
            Expr::sym("Prop"),
        ]),
    ));

    tb.add_judgment(JudgmentForm::new(
        "proves",
        Expr::app(vec![Expr::sym("proves"), Expr::meta("P")]),
        vec![("P".into(), "Prop".into())],
    ));

    tb.push_rule(Rule::new(
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

    tb.push_rule(Rule::new(
        "and-elim-l",
        vec![Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
        ])],
        Expr::app(vec![Expr::sym("proves"), Expr::meta("A")]),
    ));

    tb.push_rule(Rule::new(
        "and-elim-r",
        vec![Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
        ])],
        Expr::app(vec![Expr::sym("proves"), Expr::meta("B")]),
    ));

    tb.push_rule(Rule::new(
        "imp-intro",
        vec![Expr::app(vec![Expr::sym("proves"), Expr::meta("B")])],
        Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("imp"), Expr::meta("A"), Expr::meta("B")]),
        ]),
    ));

    tb.push_rule(Rule::new(
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

    tb.build().unwrap()
}
