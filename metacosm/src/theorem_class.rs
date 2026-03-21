use crate::error::Result;

/// A theorem class: a named fragment of reasoning.
///
/// Epistemic profiles can vary by theorem class — a world might be
/// excellent at equational discovery but weak at resource-sensitive reasoning.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TheoremClass {
    /// Equational reasoning (rewrite-based)
    Equational,
    /// Resource-sensitive reasoning (linear/affine logic)
    ResourceSensitive,
    /// Higher-order reasoning (lambda calculus, Pi types)
    HigherOrder,
    /// Inductive reasoning (induction, recursion, W-types)
    Inductive,
    /// Classical reasoning (DNE, LEM, Peirce)
    Classical,
    /// Computational reasoning (normalization, evaluation)
    Computational,
    /// User-defined class
    Custom(String),
}

impl std::fmt::Display for TheoremClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TheoremClass::Equational => write!(f, "Equational"),
            TheoremClass::ResourceSensitive => write!(f, "ResourceSensitive"),
            TheoremClass::HigherOrder => write!(f, "HigherOrder"),
            TheoremClass::Inductive => write!(f, "Inductive"),
            TheoremClass::Classical => write!(f, "Classical"),
            TheoremClass::Computational => write!(f, "Computational"),
            TheoremClass::Custom(s) => write!(f, "{}", s),
        }
    }
}

pub fn parse_theorem_class(s: &str) -> Result<TheoremClass> {
    match s {
        "Equational" => Ok(TheoremClass::Equational),
        "ResourceSensitive" => Ok(TheoremClass::ResourceSensitive),
        "HigherOrder" => Ok(TheoremClass::HigherOrder),
        "Inductive" => Ok(TheoremClass::Inductive),
        "Classical" => Ok(TheoremClass::Classical),
        "Computational" => Ok(TheoremClass::Computational),
        _ => Ok(TheoremClass::Custom(s.to_string())),
    }
}
