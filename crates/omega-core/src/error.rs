/// Error types for the Omega kernel.
use std::fmt;

use crate::expr::{Expr, Name};
use crate::pattern::MatchError;

/// All errors that can originate from the kernel.
#[derive(Debug, Clone)]
pub enum OmegaError {
    // --- Theory errors ---
    /// A named declaration was duplicated.
    DuplicateName { kind: String, name: Name },
    /// A named declaration was not found.
    UnknownName { kind: String, name: Name },
    /// Theory has no rules.
    EmptyTheory(Name),

    // --- Derivation errors ---
    /// The derivation tree is malformed.
    MalformedDerivation(String),
    /// Pattern matching failed during proof checking.
    PatternMatchFailed {
        rule: Name,
        expected: Expr,
        got: Expr,
        cause: MatchError,
    },
    /// A rule requires N premises but the derivation provides a different number.
    PremiseCountMismatch {
        rule: Name,
        expected: usize,
        got: usize,
    },
    /// The conclusion of a derivation doesn't match the goal.
    GoalMismatch { expected: Expr, got: Expr },
    /// An assumption was used but doesn't match the goal.
    AssumptionMismatch { goal: Expr },
    /// An assumption was already consumed (affine mode).
    UseAfterMove { index: usize, expr: Expr },
    /// Unresolved meta-variables remain after checking.
    UnresolvedMetas(Vec<Name>),

    // --- Metatheorem errors ---
    /// Case analysis is not exhaustive.
    NonExhaustiveCases {
        missing_rules: Vec<Name>,
    },
    /// Inductive call is not on a structural sub-derivation.
    NonStructuralRecursion {
        metatheorem: Name,
        detail: String,
    },
    /// Metatheorem proof uses a rule not in the theory.
    RuleNotInTheory {
        rule: Name,
        theory: Name,
    },

    // --- Reflection errors ---
    /// Attempted to reflect a metatheorem that hasn't been proven.
    UnprovenMetatheorem(Name),
    /// Attempted self-strengthening (using reflected rules in their own proof).
    SelfStrengthening {
        reflected_rule: Name,
        metatheorem: Name,
    },
    /// Theory has been modified since the metatheorem was proven.
    StaleReflection {
        metatheorem: Name,
        theory: Name,
    },

    // --- General ---
    /// Internal error (should never happen in correct code).
    Internal(String),
}

impl fmt::Display for OmegaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OmegaError::DuplicateName { kind, name } => {
                write!(f, "duplicate {}: {}", kind, name)
            }
            OmegaError::UnknownName { kind, name } => {
                write!(f, "unknown {}: {}", kind, name)
            }
            OmegaError::EmptyTheory(n) => write!(f, "theory {} has no rules", n),
            OmegaError::MalformedDerivation(s) => write!(f, "malformed derivation: {}", s),
            OmegaError::PatternMatchFailed {
                rule,
                expected,
                got,
                cause,
            } => {
                write!(
                    f,
                    "pattern match failed for rule {}: expected {}, got {} ({})",
                    rule, expected, got, cause
                )
            }
            OmegaError::PremiseCountMismatch {
                rule,
                expected,
                got,
            } => {
                write!(
                    f,
                    "rule {} expects {} premises, but {} were provided",
                    rule, expected, got
                )
            }
            OmegaError::GoalMismatch { expected, got } => {
                write!(f, "goal mismatch: expected {}, got {}", expected, got)
            }
            OmegaError::AssumptionMismatch { goal } => {
                write!(f, "no matching assumption for goal {}", goal)
            }
            OmegaError::UseAfterMove { index, expr } => {
                write!(
                    f,
                    "affine violation: assumption {} ({}) already consumed",
                    index, expr
                )
            }
            OmegaError::UnresolvedMetas(ms) => {
                write!(f, "unresolved meta-variables: ")?;
                for (i, m) in ms.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "?{}", m)?;
                }
                Ok(())
            }
            OmegaError::NonExhaustiveCases { missing_rules } => {
                write!(f, "non-exhaustive case analysis, missing: ")?;
                for (i, r) in missing_rules.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", r)?;
                }
                Ok(())
            }
            OmegaError::NonStructuralRecursion {
                metatheorem,
                detail,
            } => {
                write!(
                    f,
                    "non-structural recursion in metatheorem {}: {}",
                    metatheorem, detail
                )
            }
            OmegaError::RuleNotInTheory { rule, theory } => {
                write!(f, "rule {} is not in theory {}", rule, theory)
            }
            OmegaError::UnprovenMetatheorem(n) => write!(f, "unproven metatheorem: {}", n),
            OmegaError::SelfStrengthening {
                reflected_rule,
                metatheorem,
            } => {
                write!(
                    f,
                    "self-strengthening: reflected rule {} cannot be used in proof of {}",
                    reflected_rule, metatheorem
                )
            }
            OmegaError::StaleReflection {
                metatheorem,
                theory,
            } => {
                write!(
                    f,
                    "stale reflection: theory {} modified since metatheorem {} was proven",
                    theory, metatheorem
                )
            }
            OmegaError::Internal(s) => write!(f, "internal error: {}", s),
        }
    }
}

impl std::error::Error for OmegaError {}

pub type Result<T> = std::result::Result<T, OmegaError>;
