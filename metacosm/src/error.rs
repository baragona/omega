use std::fmt;

#[derive(Debug, Clone)]
pub enum MetacosmError {
    /// Unknown top-level block
    UnknownBlock { name: String },
    /// Parse error in a declaration
    ParseError { block: String, detail: String },
    /// Duplicate name
    DuplicateName { kind: String, name: String },
    /// Reference to undefined name
    Undefined { kind: String, name: String },
    /// Invalid transition between worlds
    InvalidTransition { from: String, to: String, detail: String },
    /// Invariant violated by a transition
    InvariantViolation { transition: String, invariant: String, detail: String },
    /// Epistemic constraint failure
    EpistemicError { universe: String, detail: String },
    /// Universe family error
    FamilyError { family: String, detail: String },
    /// Pipeline step failure
    PipelineError { pipeline: String, step: String, detail: String },
    /// Transition composition error
    CompositionError { detail: String },
    /// Embedding property violation
    EmbeddingViolation { embedding: String, property: String, detail: String },
    /// Assertion failed
    AssertionFailed { assertion: String, detail: String },
    /// Proof engine error
    ProofError { theorem: String, detail: String },
    /// Hyperion pass-through
    HyperionError(hyperion::error::HyperionError),
}

impl fmt::Display for MetacosmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetacosmError::UnknownBlock { name } => {
                write!(f, "unknown top-level block: {}", name)
            }
            MetacosmError::ParseError { block, detail } => {
                write!(f, "parse error in {}: {}", block, detail)
            }
            MetacosmError::DuplicateName { kind, name } => {
                write!(f, "duplicate {} name: {}", kind, name)
            }
            MetacosmError::Undefined { kind, name } => {
                write!(f, "undefined {}: {}", kind, name)
            }
            MetacosmError::InvalidTransition { from, to, detail } => {
                write!(f, "invalid transition {} → {}: {}", from, to, detail)
            }
            MetacosmError::InvariantViolation { transition, invariant, detail } => {
                write!(
                    f,
                    "invariant '{}' violated by transition '{}': {}",
                    invariant, transition, detail
                )
            }
            MetacosmError::EpistemicError { universe, detail } => {
                write!(f, "epistemic error in world '{}': {}", universe, detail)
            }
            MetacosmError::FamilyError { family, detail } => {
                write!(f, "world family '{}': {}", family, detail)
            }
            MetacosmError::PipelineError { pipeline, step, detail } => {
                write!(f, "pipeline '{}' failed at step '{}': {}", pipeline, step, detail)
            }
            MetacosmError::CompositionError { detail } => {
                write!(f, "transition composition error: {}", detail)
            }
            MetacosmError::EmbeddingViolation { embedding, property, detail } => {
                write!(
                    f,
                    "embedding '{}' violates '{}': {}",
                    embedding, property, detail
                )
            }
            MetacosmError::AssertionFailed { assertion, detail } => {
                write!(f, "assertion failed: {} — {}", assertion, detail)
            }
            MetacosmError::ProofError { theorem, detail } => {
                write!(f, "proof error in '{}': {}", theorem, detail)
            }
            MetacosmError::HyperionError(e) => write!(f, "{}", e),
        }
    }
}

impl From<hyperion::error::HyperionError> for MetacosmError {
    fn from(e: hyperion::error::HyperionError) -> Self {
        MetacosmError::HyperionError(e)
    }
}

pub type Result<T> = std::result::Result<T, MetacosmError>;
