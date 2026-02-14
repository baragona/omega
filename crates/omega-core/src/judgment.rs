/// Judgment forms and inference rules.
///
/// A judgment form defines the shape of provable statements.
/// An inference rule has premises (judgments) and a conclusion (judgment).
use crate::expr::{Expr, Name};

/// A judgment form declaration, e.g. `(judgment (proves P) :where P : Prop)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgmentForm {
    name: Name,
    pattern: Expr,
    constraints: Vec<(Name, Name)>,
}

impl JudgmentForm {
    pub fn new(name: impl Into<Name>, pattern: Expr, constraints: Vec<(Name, Name)>) -> Self {
        JudgmentForm { name: name.into(), pattern, constraints }
    }
    pub fn name(&self) -> &Name { &self.name }
    pub fn pattern(&self) -> &Expr { &self.pattern }
    pub fn constraints(&self) -> &[(Name, Name)] { &self.constraints }
}

/// An inference rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    name: Name,
    premises: Vec<Expr>,
    conclusion: Expr,
    reflected: bool,
    provenance: Option<Name>,
    implicit_args: Vec<Name>,
    context_extensions: Vec<(usize, Expr)>,
}

/// A sort declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortDecl {
    name: Name,
}

impl SortDecl {
    pub fn new(name: impl Into<Name>) -> Self {
        SortDecl { name: name.into() }
    }
    pub fn name(&self) -> &Name { &self.name }
}

/// A constructor declaration, e.g. `(constructor and : (-> Prop Prop Prop))`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorDecl {
    name: Name,
    ty: Expr,
}

impl ConstructorDecl {
    pub fn new(name: impl Into<Name>, ty: Expr) -> Self {
        ConstructorDecl { name: name.into(), ty }
    }
    pub fn name(&self) -> &Name { &self.name }
    pub fn ty(&self) -> &Expr { &self.ty }
}

/// A rewrite rule for definitional equality via delta reduction.
/// The kernel normalizes terms using these rules before equality checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteRule {
    name: Name,
    lhs: Expr,
    rhs: Expr,
}

impl RewriteRule {
    pub fn new(name: impl Into<Name>, lhs: Expr, rhs: Expr) -> Self {
        RewriteRule { name: name.into(), lhs, rhs }
    }
    pub fn name(&self) -> &Name { &self.name }
    pub fn lhs(&self) -> &Expr { &self.lhs }
    pub fn rhs(&self) -> &Expr { &self.rhs }
}

impl Rule {
    /// Create a new rule with the minimum required fields.
    /// Optional fields default to: reflected=false, provenance=None,
    /// implicit_args=[], context_extensions=[].
    pub fn new(name: impl Into<Name>, premises: Vec<Expr>, conclusion: Expr) -> Self {
        Rule {
            name: name.into(),
            premises,
            conclusion,
            reflected: false,
            provenance: None,
            implicit_args: vec![],
            context_extensions: vec![],
        }
    }

    // --- Accessors ---
    pub fn name(&self) -> &Name { &self.name }
    pub fn premises(&self) -> &[Expr] { &self.premises }
    pub fn conclusion(&self) -> &Expr { &self.conclusion }
    pub fn reflected(&self) -> bool { self.reflected }
    pub fn provenance(&self) -> Option<&Name> { self.provenance.as_ref() }
    pub fn implicit_args(&self) -> &[Name] { &self.implicit_args }
    pub fn context_extensions(&self) -> &[(usize, Expr)] { &self.context_extensions }

    // --- Builders ---

    /// Builder: set implicit arguments.
    pub fn with_implicit(mut self, args: Vec<Name>) -> Self {
        self.implicit_args = args;
        self
    }

    /// Builder: set context extensions.
    pub fn with_context(mut self, extensions: Vec<(usize, Expr)>) -> Self {
        self.context_extensions = extensions;
        self
    }

    /// Builder: mark as reflected.
    pub fn with_reflected(mut self) -> Self {
        self.reflected = true;
        self
    }

    /// Builder: set provenance.
    pub fn with_provenance(mut self, prov: impl Into<Name>) -> Self {
        self.provenance = Some(prov.into());
        self
    }

    /// Collect all meta-variables mentioned in this rule (insertion order, deduplicated).
    pub fn meta_vars(&self) -> Vec<Name> {
        let mut seen = std::collections::HashSet::new();
        let mut vars = Vec::new();
        for p in &self.premises {
            for m in p.meta_vars() {
                if seen.insert(m.clone()) {
                    vars.push(m);
                }
            }
        }
        for m in self.conclusion.meta_vars() {
            if seen.insert(m.clone()) {
                vars.push(m);
            }
        }
        vars
    }
}
