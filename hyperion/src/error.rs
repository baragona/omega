use std::fmt;

#[derive(Debug, Clone)]
pub enum HyperionError {
    /// Unknown top-level block
    UnknownBlock { name: String },
    /// Parse error in a declaration
    ParseError { block: String, detail: String },
    /// Duplicate name
    DuplicateName { kind: String, name: String },
    /// Reference to undefined name
    Undefined { kind: String, name: String },
    /// Category/Substrate incompatibility
    Incompatible { category: String, substrate: String, detail: String },
    /// Functor has no matching universe pairs
    NoMatchingUniverses { functor: String, source: String, target: String },
    /// Prelude loading error
    PreludeError { detail: String },
    /// Categorical law violation: theory fails to satisfy category axioms
    LawViolation { theory: String, law: String, detail: String },
    /// Categorical law inconclusive: normalization ran out of fuel or timed out
    LawInconclusive { theory: String, law: String, detail: String },
    /// Resource mode violation: rule violates substrate's resource constraints
    ResourceViolation {
        theory: String,
        rule_name: Option<String>,
        detail: String,
    },
    /// Proof assertion failure (logic engine)
    ProofFailure { name: String, detail: String },
    /// Binder safety violation: e-graph rewrite descends into a binder without scoping pass
    BinderSafety {
        theory: String,
        rule_name: Option<String>,
        detail: String,
    },
    /// Theory sealing error
    SealError { theory: String, detail: String },
    /// Totality violation: rewrite rule fails structural termination check
    TotalityViolation {
        theory: String,
        rule_name: Option<String>,
        detail: String,
    },
    /// Apeiron error pass-through
    ApeironError(apeiron::error::ApeironError),
}

impl fmt::Display for HyperionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HyperionError::UnknownBlock { name } => {
                write!(f, "unknown top-level block: {}", name)
            }
            HyperionError::ParseError { block, detail } => {
                write!(f, "parse error in {}: {}", block, detail)
            }
            HyperionError::DuplicateName { kind, name } => {
                write!(f, "duplicate {} name: {}", kind, name)
            }
            HyperionError::Undefined { kind, name } => {
                write!(f, "undefined {}: {}", kind, name)
            }
            HyperionError::Incompatible {
                category,
                substrate,
                detail,
            } => {
                write!(
                    f,
                    "Category '{}' incompatible with Substrate '{}': {}",
                    category, substrate, detail
                )
            }
            HyperionError::NoMatchingUniverses { functor, source, target } => {
                write!(
                    f,
                    "Functor '{}' from '{}' to '{}' has no matching universe pairs (no categories shared between substrates)",
                    functor, source, target
                )
            }
            HyperionError::PreludeError { detail } => {
                write!(f, "prelude error: {}", detail)
            }
            HyperionError::LawViolation { theory, law, detail } => {
                write!(
                    f,
                    "categorical law violation in Theory '{}': law '{}' failed — {}",
                    theory, law, detail
                )
            }
            HyperionError::LawInconclusive { theory, law, detail } => {
                write!(
                    f,
                    "categorical law INCONCLUSIVE in Theory '{}': law '{}' — {}",
                    theory, law, detail
                )
            }
            HyperionError::ResourceViolation { theory, rule_name, detail } => {
                if let Some(rn) = rule_name {
                    write!(
                        f,
                        "resource violation in Theory '{}' (rule '{}'): {}",
                        theory, rn, detail
                    )
                } else {
                    write!(
                        f,
                        "resource violation in Theory '{}': {}",
                        theory, detail
                    )
                }
            }
            HyperionError::ProofFailure { name, detail } => {
                write!(f, "proof '{}' failed: {}", name, detail)
            }
            HyperionError::BinderSafety { theory, rule_name, detail } => {
                if let Some(rn) = rule_name {
                    write!(f, "binder safety violation in Theory '{}' (rule '{}'): {}", theory, rn, detail)
                } else {
                    write!(f, "binder safety violation in Theory '{}': {}", theory, detail)
                }
            }
            HyperionError::SealError { theory, detail } => {
                write!(f, "seal error for Theory '{}': {}", theory, detail)
            }
            HyperionError::TotalityViolation { theory, rule_name, detail } => {
                if let Some(rn) = rule_name {
                    write!(f, "totality violation in Theory '{}' (rule '{}'): {}", theory, rn, detail)
                } else {
                    write!(f, "totality violation in Theory '{}': {}", theory, detail)
                }
            }
            HyperionError::ApeironError(e) => {
                write!(f, "Apeiron error: {}", e)
            }
        }
    }
}

impl From<apeiron::error::ApeironError> for HyperionError {
    fn from(e: apeiron::error::ApeironError) -> Self {
        HyperionError::ApeironError(e)
    }
}

pub type Result<T> = std::result::Result<T, HyperionError>;
