use std::fmt;

#[derive(Debug, Clone)]
pub enum ApeironError {
    // Graph errors
    NullPointer { context: String },
    PortOutOfBounds { ptr: u32, slot: u8, num_ports: usize },
    NodeFreed { ptr: u32 },

    // Parse errors
    ParseError { message: String, line: usize, col: usize },
    UnexpectedToken { expected: String, got: String },
    UnexpectedEof,

    // System/Theory errors
    UnknownSystem { name: String },
    UnknownOperator { name: String },
    DuplicateName { kind: String, name: String },
    InvalidConfig { block: String, detail: String },

    // Physics errors
    FuelExhausted { interactions: u64 },

    // Assertion errors
    AssertionFailed { name: String, detail: String },

    // Linearity errors
    LinearityViolation { detail: String },

    // Morphism errors
    MorphismError { name: String, detail: String },
    UnknownMorphism { name: String },
    OperatorNotInTarget { source_op: String, target_system: String },

    // Judgment / derivation errors
    DerivationFailed { name: String, detail: String },
    JudgmentMismatch { name: String, detail: String },

    // Refutation errors
    RefutationFailed { name: String, detail: String },
    RefutationInconclusive { name: String, detail: String },
}

impl fmt::Display for ApeironError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NullPointer { context } => write!(f, "null pointer in {}", context),
            Self::PortOutOfBounds { ptr, slot, num_ports } => {
                write!(f, "port {} out of bounds on node {} (has {} ports)", slot, ptr, num_ports)
            }
            Self::NodeFreed { ptr } => write!(f, "node {} already freed", ptr),
            Self::ParseError { message, line, col } => {
                write!(f, "parse error at {}:{}: {}", line, col, message)
            }
            Self::UnexpectedToken { expected, got } => {
                write!(f, "expected {}, got {}", expected, got)
            }
            Self::UnexpectedEof => write!(f, "unexpected end of input"),
            Self::UnknownSystem { name } => write!(f, "unknown system: {}", name),
            Self::UnknownOperator { name } => write!(f, "unknown operator: {}", name),
            Self::DuplicateName { kind, name } => {
                write!(f, "duplicate {} name: {}", kind, name)
            }
            Self::InvalidConfig { block, detail } => {
                write!(f, "invalid config in {}: {}", block, detail)
            }
            Self::FuelExhausted { interactions } => {
                write!(f, "fuel exhausted after {} interactions", interactions)
            }
            Self::AssertionFailed { name, detail } => {
                write!(f, "assertion '{}' failed: {}", name, detail)
            }
            Self::LinearityViolation { detail } => {
                write!(f, "linearity violation: {}", detail)
            }
            Self::MorphismError { name, detail } => {
                write!(f, "morphism '{}' error: {}", name, detail)
            }
            Self::UnknownMorphism { name } => {
                write!(f, "unknown morphism: {}", name)
            }
            Self::OperatorNotInTarget { source_op, target_system } => {
                write!(f, "source operator '{}' has no mapping in target system '{}'. Add [Map {} ...] to the AutoMorphism.", source_op, target_system, source_op)
            }
            Self::DerivationFailed { name, detail } => {
                write!(f, "derivation '{}' failed: {}", name, detail)
            }
            Self::JudgmentMismatch { name, detail } => {
                write!(f, "judgment '{}' mismatch: {}", name, detail)
            }
            Self::RefutationFailed { name, detail } => {
                write!(f, "refutation '{}' failed: {}", name, detail)
            }
            Self::RefutationInconclusive { name, detail } => {
                write!(f, "refutation '{}' inconclusive: {}", name, detail)
            }
        }
    }
}

impl std::error::Error for ApeironError {}

pub type Result<T> = std::result::Result<T, ApeironError>;
