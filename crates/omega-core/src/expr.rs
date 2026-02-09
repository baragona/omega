/// Core expression type for the Omega logical framework.
///
/// Uses a locally nameless representation:
/// - Bound variables are de Bruijn indices (structural alpha-equivalence)
/// - Free variables are named (readable error messages)
/// - Meta-variables are named with `?` prefix (used in rule patterns)
use std::fmt;

/// A name for free variables and identifiers.
pub type Name = String;

/// De Bruijn index for bound variables (0-based, counting from the innermost binder).
pub type DeBruijnIndex = usize;

/// The core expression type. Everything in Omega is an Expr.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Expr {
    /// A free variable: `x`, `A`, `foo`
    Free(Name),

    /// A bound variable (de Bruijn index)
    Bound(DeBruijnIndex),

    /// A meta-variable used in patterns: `?X`, `?A`
    Meta(Name),

    /// A symbol/constructor: `true`, `and`, `proves`
    Sym(Name),

    /// Application / compound expression: `(f a b c)`
    App(Vec<Expr>),

    /// A binder: `(lambda (x : T) body)` or `(forall (x : T) body)`
    /// The body uses de Bruijn index 0 for the bound variable.
    Binder {
        kind: BinderKind,
        /// Hint name for pretty-printing (not semantically relevant)
        hint: Name,
        /// The type annotation (if any)
        ty: Box<Expr>,
        /// The body (bound var 0 refers to this binder)
        body: Box<Expr>,
    },
}

/// Binder kind: a string name identifying the binder type.
/// The kernel is binder-agnostic — theories declare which kinds trigger
/// beta-reduction (substitution on application) via `substitutive_binders`.
pub type BinderKind = Name;

/// Standard binder kind constants.
pub const LAMBDA: &str = "lambda";
pub const FORALL: &str = "forall";
pub const ARROW: &str = "->";

impl Expr {
    /// Convenience: create a symbol.
    pub fn sym(s: &str) -> Expr {
        Expr::Sym(s.to_string())
    }

    /// Convenience: create a free variable.
    pub fn free(s: &str) -> Expr {
        Expr::Free(s.to_string())
    }

    /// Convenience: create a meta-variable.
    pub fn meta(s: &str) -> Expr {
        Expr::Meta(s.to_string())
    }

    /// Convenience: create an application.
    pub fn app(exprs: Vec<Expr>) -> Expr {
        Expr::App(exprs)
    }

    /// Check if this expression is a meta-variable.
    pub fn is_meta(&self) -> bool {
        matches!(self, Expr::Meta(_))
    }

    /// Check if this expression contains any meta-variables.
    pub fn has_metas(&self) -> bool {
        match self {
            Expr::Meta(_) => true,
            Expr::Free(_) | Expr::Bound(_) | Expr::Sym(_) => false,
            Expr::App(args) => args.iter().any(|a| a.has_metas()),
            Expr::Binder { ty, body, .. } => ty.has_metas() || body.has_metas(),
        }
    }

    /// Collect all meta-variable names in this expression.
    pub fn meta_vars(&self) -> Vec<Name> {
        let mut vars = Vec::new();
        self.collect_metas(&mut vars);
        vars.sort();
        vars.dedup();
        vars
    }

    fn collect_metas(&self, acc: &mut Vec<Name>) {
        match self {
            Expr::Meta(n) => acc.push(n.clone()),
            Expr::Free(_) | Expr::Bound(_) | Expr::Sym(_) => {}
            Expr::App(args) => {
                for a in args {
                    a.collect_metas(acc);
                }
            }
            Expr::Binder { ty, body, .. } => {
                ty.collect_metas(acc);
                body.collect_metas(acc);
            }
        }
    }

    /// Size of the expression tree (number of nodes).
    pub fn size(&self) -> usize {
        match self {
            Expr::Free(_) | Expr::Bound(_) | Expr::Meta(_) | Expr::Sym(_) => 1,
            Expr::App(args) => 1 + args.iter().map(|a| a.size()).sum::<usize>(),
            Expr::Binder { ty, body, .. } => 1 + ty.size() + body.size(),
        }
    }
}

impl fmt::Debug for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Free(n) => write!(f, "{}", n),
            Expr::Bound(i) => write!(f, "#{}", i),
            Expr::Meta(n) => write!(f, "?{}", n),
            Expr::Sym(n) => write!(f, "'{}", n),
            Expr::App(args) => {
                write!(f, "(")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{:?}", a)?;
                }
                write!(f, ")")
            }
            Expr::Binder {
                kind, hint, ty, body,
            } => {
                write!(f, "({} ({} : {:?}) {:?})", kind, hint, ty, body)
            }
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Free(n) => write!(f, "{}", n),
            Expr::Bound(i) => write!(f, "#{}", i),
            Expr::Meta(n) => write!(f, "?{}", n),
            Expr::Sym(n) => write!(f, "{}", n),
            Expr::App(args) => {
                write!(f, "(")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", a)?;
                }
                write!(f, ")")
            }
            Expr::Binder {
                kind, hint, ty, body,
            } => {
                write!(f, "({} ({} : {}) {})", kind, hint, ty, body)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expr_equality() {
        assert_eq!(Expr::sym("and"), Expr::sym("and"));
        assert_ne!(Expr::sym("and"), Expr::sym("or"));
        assert_eq!(Expr::Bound(0), Expr::Bound(0));
        assert_ne!(Expr::Bound(0), Expr::Bound(1));
    }

    #[test]
    fn meta_collection() {
        let e = Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
        ]);
        let metas = e.meta_vars();
        assert_eq!(metas, vec!["A", "B"]);
    }

    #[test]
    fn size_counting() {
        let e = Expr::app(vec![Expr::sym("f"), Expr::sym("x"), Expr::sym("y")]);
        assert_eq!(e.size(), 4); // app node + 3 syms
    }
}
