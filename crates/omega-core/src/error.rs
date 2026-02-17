/// Error types for the Omega kernel.
use std::fmt;

use crate::expr::{Expr, Name};
use crate::pattern::MatchError;

/// The kind of a named declaration (for error reporting).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclKind {
    Sort,
    Constructor,
    Rule,
    Judgment,
    BindingSpec,
    Rewrite,
    Theory,
}

impl fmt::Display for DeclKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeclKind::Sort => write!(f, "sort"),
            DeclKind::Constructor => write!(f, "constructor"),
            DeclKind::Rule => write!(f, "rule"),
            DeclKind::Judgment => write!(f, "judgment"),
            DeclKind::BindingSpec => write!(f, "binding-spec"),
            DeclKind::Rewrite => write!(f, "rewrite"),
            DeclKind::Theory => write!(f, "theory"),
        }
    }
}

/// All errors that can originate from the kernel.
#[derive(Debug, Clone)]
pub enum OmegaError {
    // --- Theory errors ---
    /// A named declaration was duplicated.
    DuplicateName { kind: DeclKind, name: Name },
    /// A named declaration was not found.
    UnknownName { kind: DeclKind, name: Name },

    // --- Derivation errors ---
    /// A rewrite rule's RHS references a meta-variable not present in its LHS.
    RewriteMetaEscape { rule: Name, meta: Name },
    /// A parameterized theory was instantiated with the wrong number of arguments.
    ParamCountMismatch { theory: Name, expected: usize, got: usize },
    /// Case analysis on a variable not in scope.
    UnknownScrutinee { var: Name },
    /// A derivation variable is not in scope.
    UnknownDerivationVar { var: Name },
    /// Metatheorem has no existential witness to reflect.
    NoExistential { metatheorem: Name },
    /// An assumption index is out of bounds.
    AssumptionIndexOutOfBounds { index: usize, count: usize },
    /// A premise sub-derivation failed during proof checking.
    PremiseCheckFailed { rule: Name, premise: usize, cause: Box<OmegaError> },
    /// A binder usage constraint was violated (linear or affine).
    BinderUsageViolation { rule: Name, detail: String },
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
    /// An assumption was not consumed (linear mode requires all assumptions used).
    LinearUnused { index: usize, expr: Expr },

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
            OmegaError::RewriteMetaEscape { rule, meta } => {
                write!(f, "rewrite rule {}: RHS meta-variable ?{} not in LHS", rule, meta)
            }
            OmegaError::ParamCountMismatch { theory, expected, got } => {
                write!(f, "theory {} expects {} parameters, got {}", theory, expected, got)
            }
            OmegaError::UnknownScrutinee { var } => {
                write!(f, "case analysis on unknown variable {}", var)
            }
            OmegaError::UnknownDerivationVar { var } => {
                write!(f, "unknown derivation variable {}", var)
            }
            OmegaError::NoExistential { metatheorem } => {
                write!(f, "metatheorem {} has no existential (nothing to reflect)", metatheorem)
            }
            OmegaError::AssumptionIndexOutOfBounds { index, count } => {
                write!(f, "assumption index {} out of bounds ({} assumptions)", index, count)
            }
            OmegaError::PremiseCheckFailed { rule, premise, cause } => {
                write!(f, "in premise {} of rule {}: {}", premise, rule, cause)
            }
            OmegaError::BinderUsageViolation { rule, detail } => {
                write!(f, "binder usage violation in rule {}: {}", rule, detail)
            }
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
                    "linear/affine violation: assumption {} ({}) already consumed",
                    index, expr
                )
            }
            OmegaError::LinearUnused { index, expr } => {
                write!(
                    f,
                    "linear violation: assumption {} ({}) was not consumed",
                    index, expr
                )
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
        }
    }
}

impl std::error::Error for OmegaError {}

pub type Result<T> = std::result::Result<T, OmegaError>;
