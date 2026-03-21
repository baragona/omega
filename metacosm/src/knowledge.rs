/// Knowledge species: the fundamental distinction between meta-theoretic facts
/// and operational measurements.
///
/// These are different species of knowledge:
/// - Semantic: follows from the structure of the logic (soundness, confluence, conservativity)
/// - Empirical: measured from running the system (proof size, search cost, runtime)

/// The species of an epistemic claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KnowledgeSpecies {
    /// Meta-theoretic: follows from the structure of the logic.
    Semantic,
    /// Operational: measured from running the system.
    Empirical,
}

impl Default for KnowledgeSpecies {
    fn default() -> Self {
        KnowledgeSpecies::Semantic
    }
}

impl std::fmt::Display for KnowledgeSpecies {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KnowledgeSpecies::Semantic => write!(f, "semantic"),
            KnowledgeSpecies::Empirical => write!(f, "empirical"),
        }
    }
}

/// The status of a semantic property claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyStatus {
    /// User asserted
    Claimed,
    /// Inferred by derivation rules
    Derived { rule: String },
    /// Validated by metacosm checks
    Checked,
}

impl std::fmt::Display for PropertyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropertyStatus::Claimed => write!(f, "claimed"),
            PropertyStatus::Derived { rule } => write!(f, "derived({})", rule),
            PropertyStatus::Checked => write!(f, "checked"),
        }
    }
}

/// A semantic property: a meta-theoretic fact about a world or transition.
#[derive(Debug, Clone)]
pub struct SemanticProperty {
    pub name: String,
    pub holder: String,
    pub status: PropertyStatus,
}

/// Unit of an empirical metric.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MetricUnit {
    ProofSize,
    SearchCost,
    Runtime,
    MemoryUsage,
    WitnessSize,
    Custom(String),
}

impl std::fmt::Display for MetricUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetricUnit::ProofSize => write!(f, "proof-size"),
            MetricUnit::SearchCost => write!(f, "search-cost"),
            MetricUnit::Runtime => write!(f, "runtime"),
            MetricUnit::MemoryUsage => write!(f, "memory"),
            MetricUnit::WitnessSize => write!(f, "witness-size"),
            MetricUnit::Custom(s) => write!(f, "{}", s),
        }
    }
}

pub fn parse_metric_unit(s: &str) -> MetricUnit {
    match s {
        "proof-size" => MetricUnit::ProofSize,
        "search-cost" => MetricUnit::SearchCost,
        "runtime" => MetricUnit::Runtime,
        "memory" => MetricUnit::MemoryUsage,
        "witness-size" => MetricUnit::WitnessSize,
        other => MetricUnit::Custom(other.to_string()),
    }
}
