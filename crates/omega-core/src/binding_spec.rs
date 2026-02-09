/// Generic binding specifications: user-definable scoping rules.
///
/// Instead of hardcoding lambda/forall as the only binders, theories can
/// declare custom binding forms. The kernel understands these generically:
///
/// ```omega
/// (theory STLC
///   (sort Ty) (sort Tm)
///   (binding-spec lam-bind
///     :binds 1            ;; binds one variable
///     :scope body         ;; the "body" position is under the binder
///     :display (lam ?ty ?body))
///
///   ;; Now (lam-bind Nat #0) means "lambda x:Nat. x"
///   ;; The kernel handles alpha-equivalence, substitution, etc.
/// )
/// ```
///
/// This lets the user define:
/// - Lambda abstractions, Pi types, Sigma types
/// - Let bindings (bind + definition)
/// - Pattern matching arms with binders
/// - Linear/affine binders (with usage tracking)
/// - Dynamic scope (where the body doesn't actually capture)
use crate::expr::Name;

/// A user-defined binding specification.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BindingSpec {
    /// Name of this binding form, e.g. "lam-bind"
    pub name: Name,
    /// How many variables this binder introduces.
    pub arity: usize,
    /// Which positions in the constructor are "body" positions
    /// (where the bound variables are in scope).
    /// Indices refer to positions in the `params` list of UserBind.
    pub body_positions: Vec<usize>,
    /// Whether this binding is linear (each bound var used exactly once).
    pub linear: bool,
    /// Whether this binding is affine (each bound var used at most once).
    pub affine: bool,
    /// An optional display pattern for pretty-printing.
    pub display: Option<String>,
}

impl BindingSpec {
    /// Create a standard binding spec (one variable, one body).
    pub fn standard(name: &str) -> Self {
        BindingSpec {
            name: name.to_string(),
            arity: 1,
            body_positions: vec![],
            linear: false,
            affine: false,
            display: None,
        }
    }

    /// Create a binding spec for a let-binding (binds one var, has a definition and a body).
    pub fn let_bind(name: &str) -> Self {
        BindingSpec {
            name: name.to_string(),
            arity: 1,
            body_positions: vec![1], // second param is the body
            linear: false,
            affine: false,
            display: None,
        }
    }

    /// Create a linear binding spec.
    pub fn linear(name: &str) -> Self {
        BindingSpec {
            name: name.to_string(),
            arity: 1,
            body_positions: vec![],
            linear: true,
            affine: false,
            display: None,
        }
    }

    /// Create an affine binding spec.
    pub fn affine(name: &str) -> Self {
        BindingSpec {
            name: name.to_string(),
            arity: 1,
            body_positions: vec![],
            linear: false,
            affine: true,
            display: None,
        }
    }
}

/// Binding mode: how the kernel should handle scope for a particular
/// binding form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingMode {
    /// Standard lexical scoping (the default for most logics).
    Lexical,
    /// Linear: each bound variable must be used exactly once.
    Linear,
    /// Affine: each bound variable may be used at most once.
    Affine,
    /// Relevant: each bound variable must be used at least once.
    Relevant,
    /// No scope: the binder form exists but doesn't actually capture variables.
    /// Useful for defining "let" in certain logics where it's just sugar.
    NoCapture,
}

/// Check that a binding form is used correctly according to its spec.
///
/// For linear/affine bindings, this counts occurrences of the bound variable
/// in the body and ensures the constraint is satisfied.
pub fn check_binding_usage(
    spec: &BindingSpec,
    body: &crate::expr::Expr,
    bound_index: usize,
) -> Result<(), BindingViolation> {
    if !spec.linear && !spec.affine {
        return Ok(()); // No usage constraints
    }

    let count = count_bound_occurrences(body, bound_index);

    if spec.linear && count != 1 {
        return Err(BindingViolation::LinearityViolation {
            spec: spec.name.clone(),
            expected: 1,
            actual: count,
        });
    }

    if spec.affine && count > 1 {
        return Err(BindingViolation::AffinityViolation {
            spec: spec.name.clone(),
            max: 1,
            actual: count,
        });
    }

    Ok(())
}

/// Count how many times `Bound(index)` appears in an expression.
fn count_bound_occurrences(expr: &crate::expr::Expr, index: usize) -> usize {
    use crate::expr::Expr;
    match expr {
        Expr::Bound(i) if *i == index => 1,
        Expr::Bound(_) | Expr::Free(_) | Expr::Meta(_) | Expr::Sym(_)
        | Expr::Universe(_) => 0,
        Expr::App(args) => args.iter().map(|a| count_bound_occurrences(a, index)).sum(),
        Expr::Binder { ty, body, .. } => {
            count_bound_occurrences(ty, index)
                + count_bound_occurrences(body, index + 1)
        }
    }
}

/// Binding usage violations.
#[derive(Debug, Clone)]
pub enum BindingViolation {
    LinearityViolation {
        spec: Name,
        expected: usize,
        actual: usize,
    },
    AffinityViolation {
        spec: Name,
        max: usize,
        actual: usize,
    },
}

impl std::fmt::Display for BindingViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BindingViolation::LinearityViolation {
                spec,
                expected,
                actual,
            } => write!(
                f,
                "linearity violation in {}: expected {} use(s) of bound variable, got {}",
                spec, expected, actual
            ),
            BindingViolation::AffinityViolation { spec, max, actual } => write!(
                f,
                "affinity violation in {}: at most {} use(s) of bound variable allowed, got {}",
                spec, max, actual
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Expr;

    #[test]
    fn standard_binding_no_constraints() {
        let spec = BindingSpec::standard("lambda");
        // No constraints, so any usage is fine
        let body = Expr::app(vec![Expr::Bound(0), Expr::Bound(0)]);
        assert!(check_binding_usage(&spec, &body, 0).is_ok());
    }

    #[test]
    fn linear_binding_exactly_one() {
        let spec = BindingSpec::linear("lin-lam");

        // Exactly one use: OK
        let body_ok = Expr::app(vec![Expr::sym("f"), Expr::Bound(0)]);
        assert!(check_binding_usage(&spec, &body_ok, 0).is_ok());

        // Zero uses: violation
        let body_zero = Expr::sym("constant");
        assert!(check_binding_usage(&spec, &body_zero, 0).is_err());

        // Two uses: violation
        let body_two = Expr::app(vec![Expr::Bound(0), Expr::Bound(0)]);
        assert!(check_binding_usage(&spec, &body_two, 0).is_err());
    }

    #[test]
    fn affine_binding_at_most_one() {
        let spec = BindingSpec::affine("aff-lam");

        // Zero uses: OK
        let body_zero = Expr::sym("constant");
        assert!(check_binding_usage(&spec, &body_zero, 0).is_ok());

        // One use: OK
        let body_one = Expr::app(vec![Expr::sym("f"), Expr::Bound(0)]);
        assert!(check_binding_usage(&spec, &body_one, 0).is_ok());

        // Two uses: violation
        let body_two = Expr::app(vec![Expr::Bound(0), Expr::Bound(0)]);
        assert!(check_binding_usage(&spec, &body_two, 0).is_err());
    }
}
