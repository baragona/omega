/// Core expression type for the Omega logical framework.
///
/// Uses a locally nameless representation:
/// - Bound variables are de Bruijn indices (structural alpha-equivalence)
/// - Free variables are named (readable error messages)
/// - Meta-variables are named with `?` prefix (used in rule patterns)
use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

/// A name for free variables and identifiers.
/// Wraps `Arc<str>` for O(1) cloning.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Name(Arc<str>);

impl Name {
    /// View as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for Name {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for Name {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Name {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Name {
    fn from(s: &str) -> Self {
        Name(Arc::from(s))
    }
}

impl From<String> for Name {
    fn from(s: String) -> Self {
        Name(Arc::from(s.as_str()))
    }
}

impl PartialEq<str> for Name {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Name {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<String> for Name {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<Name> for str {
    fn eq(&self, other: &Name) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<Name> for &str {
    fn eq(&self, other: &Name) -> bool {
        *self == other.as_str()
    }
}

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
        Expr::Sym(s.into())
    }

    /// Convenience: create a free variable.
    pub fn free(s: &str) -> Expr {
        Expr::Free(s.into())
    }

    /// Convenience: create a meta-variable.
    pub fn meta(s: &str) -> Expr {
        Expr::Meta(s.into())
    }

    /// Convenience: create an application.
    pub fn app(exprs: Vec<Expr>) -> Expr {
        Expr::App(exprs)
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

    /// Collect all meta-variable names in this expression (insertion order, deduplicated).
    pub fn meta_vars(&self) -> Vec<Name> {
        let mut seen = std::collections::HashSet::new();
        let mut vars = Vec::new();
        self.collect_metas(&mut vars, &mut seen);
        vars
    }

    fn collect_metas(&self, acc: &mut Vec<Name>, seen: &mut std::collections::HashSet<Name>) {
        match self {
            Expr::Meta(n) => {
                if seen.insert(n.clone()) {
                    acc.push(n.clone());
                }
            }
            Expr::Free(_) | Expr::Bound(_) | Expr::Sym(_) => {}
            Expr::App(args) => {
                for a in args {
                    a.collect_metas(acc, seen);
                }
            }
            Expr::Binder { ty, body, .. } => {
                ty.collect_metas(acc, seen);
                body.collect_metas(acc, seen);
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
