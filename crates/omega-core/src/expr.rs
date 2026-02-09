/// Core expression type for the Omega logical framework.
///
/// Uses a locally nameless representation:
/// - Bound variables are de Bruijn indices (structural alpha-equivalence)
/// - Free variables are named (readable error messages)
/// - Meta-variables are named with `?` prefix (used in rule patterns)
use std::fmt;

/// A name for free variables and identifiers.
pub type Name = String;

/// Universe levels for the algebraic universe hierarchy.
/// Supports impredicative Prop via IMax.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Level {
    /// Level 0 (Prop)
    Zero,
    /// Successor level: lsuc(l)
    Succ(Box<Level>),
    /// Maximum of two levels: lmax(l1, l2)
    Max(Box<Level>, Box<Level>),
    /// Impredicative maximum: imax(l1, l2) = 0 when l2 = 0, else max(l1, l2)
    IMax(Box<Level>, Box<Level>),
    /// Level parameter (shares Meta namespace for substitution)
    Param(Name),
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

    /// A universe: `(Type 0)`, `(Type 1)`, `(Type (lmax ?u ?v))`, etc.
    Universe(Level),
}

/// Kinds of binders supported by the framework.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BinderKind {
    Lambda,
    Forall,
    /// Arrow type: `(-> A B)` desugars to a Forall with unused bound var
    Arrow,
}

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
            Expr::Universe(level) => level.has_params(),
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
            Expr::Universe(level) => level.collect_params(acc),
        }
    }

    /// Size of the expression tree (number of nodes).
    pub fn size(&self) -> usize {
        match self {
            Expr::Free(_) | Expr::Bound(_) | Expr::Meta(_) | Expr::Sym(_)
            | Expr::Universe(_) => 1,
            Expr::App(args) => 1 + args.iter().map(|a| a.size()).sum::<usize>(),
            Expr::Binder { ty, body, .. } => 1 + ty.size() + body.size(),
        }
    }
}

impl Level {
    /// Check if this level contains any Param (used as metas).
    pub fn has_params(&self) -> bool {
        match self {
            Level::Zero => false,
            Level::Succ(l) => l.has_params(),
            Level::Max(a, b) | Level::IMax(a, b) => a.has_params() || b.has_params(),
            Level::Param(_) => true,
        }
    }

    /// Collect Param names (treated as meta-variables for substitution).
    pub fn collect_params(&self, acc: &mut Vec<Name>) {
        match self {
            Level::Zero => {}
            Level::Succ(l) => l.collect_params(acc),
            Level::Max(a, b) | Level::IMax(a, b) => {
                a.collect_params(acc);
                b.collect_params(acc);
            }
            Level::Param(n) => acc.push(n.clone()),
        }
    }

    /// Convert a concrete level to a number, if possible.
    pub fn to_num(&self) -> Option<usize> {
        match self {
            Level::Zero => Some(0),
            Level::Succ(l) => l.to_num().map(|n| n + 1),
            _ => None,
        }
    }

    /// Create a level from a concrete number.
    pub fn from_num(n: usize) -> Level {
        let mut level = Level::Zero;
        for _ in 0..n {
            level = Level::Succ(Box::new(level));
        }
        level
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Level::Zero => write!(f, "0"),
            Level::Succ(l) => {
                if let Some(n) = self.to_num() {
                    write!(f, "{}", n)
                } else {
                    write!(f, "(lsuc {})", l)
                }
            }
            Level::Max(a, b) => write!(f, "(lmax {} {})", a, b),
            Level::IMax(a, b) => write!(f, "(imax {} {})", a, b),
            Level::Param(n) => write!(f, "?{}", n),
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
                write!(f, "({:?} ({} : {:?}) {:?})", kind, hint, ty, body)
            }
            Expr::Universe(level) => write!(f, "(Type {})", level),
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
                let kw = match kind {
                    BinderKind::Lambda => "lambda",
                    BinderKind::Forall => "forall",
                    BinderKind::Arrow => "->",
                };
                write!(f, "({} ({} : {}) {})", kw, hint, ty, body)
            }
            Expr::Universe(level) => write!(f, "(Type {})", level),
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
