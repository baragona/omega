/// Raw S-expression type.
use crate::span::Span;

/// A raw S-expression with source location.
#[derive(Debug, Clone)]
pub enum Sexp {
    /// An atom: symbol, number, string, etc.
    Atom(String, Span),
    /// A list: `(...)`.
    List(Vec<Sexp>, Span),
}

impl Sexp {
    pub fn span(&self) -> Span {
        match self {
            Sexp::Atom(_, s) => *s,
            Sexp::List(_, s) => *s,
        }
    }

    pub fn as_atom(&self) -> Option<&str> {
        match self {
            Sexp::Atom(s, _) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Sexp]> {
        match self {
            Sexp::List(items, _) => Some(items),
            _ => None,
        }
    }

    pub fn is_keyword(&self, kw: &str) -> bool {
        matches!(self, Sexp::Atom(s, _) if s == kw)
    }
}

impl std::fmt::Display for Sexp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Sexp::Atom(s, _) => write!(f, "{}", s),
            Sexp::List(items, _) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, ")")
            }
        }
    }
}
