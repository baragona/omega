/// Tactic definitions.
use omega_core::expr::Name;

/// A tactic that can be applied to a proof state.
#[derive(Debug, Clone)]
pub enum Tactic {
    /// Apply a rule by name (backward reasoning).
    Apply(Name),
    /// Close a goal that matches an assumption.
    Assumption,
    /// Introduce a hypothesis (for rules like imp-intro).
    Intro(Option<Name>),
    /// Provide an explicit derivation.
    Exact(omega_core::derivation::Derivation),
    /// Automated proof search up to a given depth.
    Auto(usize),

    // --- Combinators ---
    /// Try a tactic; if it fails, the goal remains unchanged.
    Try(Box<Tactic>),
    /// Repeat a tactic until it fails.
    Repeat(Box<Tactic>),
    /// Focus on a specific subgoal by index.
    Focus(usize, Box<Tactic>),
    /// Apply a sequence of tactics.
    Seq(Vec<Tactic>),
}

impl Tactic {
    pub fn apply(rule: &str) -> Self {
        Tactic::Apply(rule.into())
    }
}
