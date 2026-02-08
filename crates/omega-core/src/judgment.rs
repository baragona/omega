/// Judgment forms and inference rules.
///
/// A judgment form defines the shape of provable statements.
/// An inference rule has premises (judgments) and a conclusion (judgment).
use crate::expr::{Expr, Name};

/// A judgment form declaration, e.g. `(judgment (proves P) :where P : Prop)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgmentForm {
    /// The name of the judgment form, e.g. "proves"
    pub name: Name,
    /// The pattern for this judgment, e.g. `(proves ?P)`
    pub pattern: Expr,
    /// Sort constraints on meta-variables, e.g. `[("P", "Prop")]`
    pub constraints: Vec<(Name, Name)>,
}

/// An inference rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// The name of this rule, e.g. "and-intro"
    pub name: Name,
    /// The premises (each is a judgment expression)
    pub premises: Vec<Expr>,
    /// The conclusion (a judgment expression)
    pub conclusion: Expr,
    /// Whether this rule was produced by reflection
    pub reflected: bool,
    /// If reflected, the metatheorem that justified it
    pub provenance: Option<Name>,
    /// Meta-variables that are implicit (inferred by unification).
    /// When a user applies this rule, they don't need to specify these.
    pub implicit_args: Vec<Name>,
    /// Context extension: assumptions this rule adds for its premises.
    /// For rules like imp-intro, this says "add (proves ?A) to context for premise 0".
    pub context_extensions: Vec<(usize, Expr)>,
}

/// A sort declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortDecl {
    pub name: Name,
}

/// A constructor declaration, e.g. `(constructor and : (-> Prop Prop Prop))`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorDecl {
    pub name: Name,
    /// The type of this constructor. For simple constructors, this is just a sort name.
    /// For constructors with arguments, this is an arrow type.
    pub ty: Expr,
}

impl Rule {
    /// Collect all meta-variables mentioned in this rule.
    pub fn meta_vars(&self) -> Vec<Name> {
        let mut vars = Vec::new();
        for p in &self.premises {
            for m in p.meta_vars() {
                if !vars.contains(&m) {
                    vars.push(m);
                }
            }
        }
        for m in self.conclusion.meta_vars() {
            if !vars.contains(&m) {
                vars.push(m);
            }
        }
        vars
    }
}
