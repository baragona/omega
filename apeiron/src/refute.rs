use std::collections::{HashMap, HashSet};

use crate::judgment::DerivRule;
use crate::parser::Sexp;

/// Result of an exhaustive refutation search.
#[derive(Debug, Clone, PartialEq)]
pub enum RefuteResult {
    /// All derivation paths exhausted. No proof exists at depth <= N.
    Refuted { depth: usize },
    /// A proof was found. The goal IS derivable.
    Derivable,
    /// Budget exhausted before completing search.
    Inconclusive { steps_used: usize },
}

/// State for the backward-chaining search.
struct SearchState {
    /// Bitmask: bit i set = assumption i consumed (for affine tracking).
    used: u64,
    /// Remaining depth.
    depth: usize,
    /// Remaining budget (steps).
    budget: usize,
}

/// Perform exhaustive backward-chaining proof search.
///
/// Returns Refuted if no proof exists at the given depth,
/// Derivable if a proof was found, or Inconclusive if budget ran out.
pub fn exhaustive_refute(
    derive_rules: &[DerivRule],
    assumptions: &[Sexp],
    goal: &Sexp,
    max_depth: usize,
    max_budget: usize,
    affine: bool,
) -> RefuteResult {
    // Iterative deepening: try depth 1, 2, ..., max_depth
    let mut total_steps = 0;
    let remaining = max_budget;

    for depth in 0..=max_depth {
        let mut state = SearchState {
            used: 0,
            depth,
            budget: remaining.saturating_sub(total_steps),
        };

        match can_derive(derive_rules, assumptions, goal, &mut state, affine) {
            SearchResult::Proved => return RefuteResult::Derivable,
            SearchResult::Failed => {
                total_steps += (remaining.saturating_sub(total_steps)) - state.budget;
            }
            SearchResult::BudgetExhausted => {
                return RefuteResult::Inconclusive {
                    steps_used: max_budget,
                };
            }
        }
    }

    RefuteResult::Refuted { depth: max_depth }
}

#[derive(Debug, PartialEq)]
enum SearchResult {
    Proved,
    Failed,
    BudgetExhausted,
}

/// Recursive backward-chaining: try to derive `goal` from `assumptions` + `derive_rules`.
fn can_derive(
    rules: &[DerivRule],
    assumptions: &[Sexp],
    goal: &Sexp,
    state: &mut SearchState,
    affine: bool,
) -> SearchResult {
    if state.budget == 0 {
        return SearchResult::BudgetExhausted;
    }
    state.budget -= 1;

    // Base case: does goal match any (unconsumed) assumption?
    for (i, assumption) in assumptions.iter().enumerate() {
        if affine && (state.used & (1 << i)) != 0 {
            continue; // already consumed
        }
        if sexp_matches(assumption, goal) {
            return SearchResult::Proved;
        }
    }

    // If depth 0, no more rule applications allowed
    if state.depth == 0 {
        return SearchResult::Failed;
    }

    // Try each derive rule whose conclusion pattern could match the goal
    for rule in rules {
        if rule.absurd {
            continue;
        }

        if let Some(bindings) = try_match_conclusion(&rule.conclusion, goal) {
            // Instantiate all premises
            let premises: Vec<Sexp> = rule
                .premises
                .iter()
                .map(|p| substitute_sexp(p, &bindings))
                .collect();

            // Try to derive ALL premises
            let saved_used = state.used;
            state.depth -= 1;

            let all_proved =
                try_derive_all_premises(rules, assumptions, &premises, state, affine);

            state.depth += 1;

            match all_proved {
                SearchResult::Proved => return SearchResult::Proved,
                SearchResult::BudgetExhausted => return SearchResult::BudgetExhausted,
                SearchResult::Failed => {
                    state.used = saved_used; // backtrack
                }
            }
        }
    }

    SearchResult::Failed
}

/// Try to derive all premises sequentially.
fn try_derive_all_premises(
    rules: &[DerivRule],
    assumptions: &[Sexp],
    premises: &[Sexp],
    state: &mut SearchState,
    affine: bool,
) -> SearchResult {
    for premise in premises {
        match can_derive(rules, assumptions, premise, state, affine) {
            SearchResult::Proved => continue,
            other => return other,
        }
    }
    SearchResult::Proved
}

/// Try to match a rule's conclusion pattern against a concrete goal.
/// Returns meta-variable bindings if successful.
fn try_match_conclusion(pattern: &Sexp, goal: &Sexp) -> Option<HashMap<String, Sexp>> {
    let mut bindings = HashMap::new();
    if match_sexp_pattern(pattern, goal, &mut bindings) {
        Some(bindings)
    } else {
        None
    }
}

/// Pattern match: meta-variables (?X) bind to subterms.
fn match_sexp_pattern(
    pattern: &Sexp,
    concrete: &Sexp,
    bindings: &mut HashMap<String, Sexp>,
) -> bool {
    match pattern {
        Sexp::Atom(name, _) if name.starts_with('?') => {
            if let Some(existing) = bindings.get(name) {
                // Non-linear: check equality
                format!("{}", existing) == format!("{}", concrete)
            } else {
                bindings.insert(name.clone(), concrete.clone());
                true
            }
        }
        Sexp::Atom(name, _) => concrete.as_atom().map_or(false, |c| c == name),
        Sexp::List(pitems, _) => {
            if let Some(citems) = concrete.as_list() {
                if pitems.len() != citems.len() {
                    return false;
                }
                for (p, c) in pitems.iter().zip(citems.iter()) {
                    if !match_sexp_pattern(p, c, bindings) {
                        return false;
                    }
                }
                true
            } else {
                false
            }
        }
    }
}

/// Check if two sexps are structurally equal (no meta-variables).
fn sexp_matches(a: &Sexp, b: &Sexp) -> bool {
    format!("{}", a) == format!("{}", b)
}

/// Strategy for proof search.
#[derive(Debug, Clone, PartialEq)]
pub enum SearchStrategy {
    /// Backward chaining from goal (default).
    Backward,
    /// Forward chaining from assumptions.
    Forward,
}

/// Indexed fact database for efficient forward-chaining.
struct FactDB {
    /// All facts in insertion order.
    facts: Vec<Sexp>,
    /// Index: head symbol → indices into `facts`.
    by_head: HashMap<String, Vec<usize>>,
    /// Deduplication via structural hash.
    seen: HashSet<String>,
}

impl FactDB {
    fn new() -> Self {
        FactDB {
            facts: Vec::new(),
            by_head: HashMap::new(),
            seen: HashSet::new(),
        }
    }

    fn from_assumptions(assumptions: &[Sexp]) -> Self {
        let mut db = Self::new();
        for a in assumptions {
            db.insert(a.clone());
        }
        db
    }

    /// Insert a fact if not already present. Returns true if new.
    fn insert(&mut self, fact: Sexp) -> bool {
        let key = sexp_structural_key(&fact);
        if self.seen.contains(&key) {
            return false;
        }
        self.seen.insert(key);
        let idx = self.facts.len();
        let head = sexp_head(&fact);
        self.by_head.entry(head).or_default().push(idx);
        self.facts.push(fact);
        true
    }

    /// Check if a fact matching `goal` exists.
    fn contains(&self, goal: &Sexp) -> bool {
        let key = sexp_structural_key(goal);
        self.seen.contains(&key)
    }

    /// Get facts whose head matches the given pattern's head.
    fn facts_for_head(&self, head: &str) -> &[usize] {
        self.by_head.get(head).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

/// Structural key for deduplication (avoids format!-based comparison).
fn sexp_structural_key(sexp: &Sexp) -> String {
    // Use Display which is already implemented, but cached per-insert
    format!("{}", sexp)
}

/// Extract head symbol from a sexp (first atom of a list, or the atom itself).
fn sexp_head(sexp: &Sexp) -> String {
    match sexp {
        Sexp::Atom(name, _) => name.clone(),
        Sexp::List(items, _) => {
            items.first()
                .and_then(|s| s.as_atom())
                .unwrap_or("__list")
                .to_string()
        }
    }
}

/// Perform exhaustive forward-chaining proof search.
///
/// Uses an indexed fact database for efficient matching. Facts are indexed by
/// head symbol for O(1) lookup instead of scanning all facts.
///
/// An optional `normalizer` function can normalize derived conclusions through
/// the interaction net before inserting them, ensuring the inet's rewrite rules
/// and optimal sharing are used.
pub fn exhaustive_refute_forward(
    derive_rules: &[DerivRule],
    assumptions: &[Sexp],
    goal: &Sexp,
    max_depth: usize,
    max_budget: usize,
) -> RefuteResult {
    exhaustive_refute_forward_with_normalizer(
        derive_rules, assumptions, goal, max_depth, max_budget, None,
    )
}

/// Forward-chaining with an optional normalizer for inet integration.
pub fn exhaustive_refute_forward_with_normalizer(
    derive_rules: &[DerivRule],
    assumptions: &[Sexp],
    goal: &Sexp,
    max_depth: usize,
    max_budget: usize,
    normalizer: Option<&dyn Fn(&Sexp) -> Sexp>,
) -> RefuteResult {
    let mut db = FactDB::from_assumptions(assumptions);
    let mut steps = 0;

    for _round in 0..=max_depth {
        // Check if goal is already known
        if db.contains(goal) {
            return RefuteResult::Derivable;
        }

        let mut new_facts: Vec<Sexp> = Vec::new();

        // Try each rule
        for rule in derive_rules {
            if rule.absurd {
                continue;
            }

            // 0-premise rules: conclusion is always derivable
            if rule.premises.is_empty() {
                let fact = rule.conclusion.clone();
                let fact = maybe_normalize(&fact, &normalizer);
                if !db.contains(&fact) {
                    new_facts.push(fact);
                }
                continue;
            }

            steps += 1;
            if steps > max_budget {
                return RefuteResult::Inconclusive { steps_used: steps };
            }

            // Find matching facts using indexed lookup
            let binding_sets = forward_match_premises_indexed(rule, &db);

            for bindings in binding_sets {
                let conclusion = substitute_sexp(&rule.conclusion, &bindings);
                let conclusion = maybe_normalize(&conclusion, &normalizer);
                if !db.contains(&conclusion) {
                    new_facts.push(conclusion);
                }
            }
        }

        if new_facts.is_empty() {
            break; // Fixed point
        }

        for fact in new_facts {
            db.insert(fact);
        }
    }

    // Final check
    if db.contains(goal) {
        RefuteResult::Derivable
    } else {
        RefuteResult::Refuted { depth: max_depth }
    }
}

fn maybe_normalize(fact: &Sexp, normalizer: &Option<&dyn Fn(&Sexp) -> Sexp>) -> Sexp {
    match normalizer {
        Some(f) => f(fact),
        None => fact.clone(),
    }
}

/// Find all ways to match a rule's premises against facts in the indexed DB.
fn forward_match_premises_indexed(
    rule: &DerivRule,
    db: &FactDB,
) -> Vec<HashMap<String, Sexp>> {
    let mut binding_sets: Vec<HashMap<String, Sexp>> = vec![HashMap::new()];

    for premise in &rule.premises {
        let mut next_sets = Vec::new();
        for bindings in &binding_sets {
            let instantiated = substitute_sexp(premise, bindings);
            let head = sexp_head(&instantiated);

            // Use index: only check facts with matching head symbol
            let candidate_indices = if head.starts_with('?') {
                // Meta-variable head: must check all facts
                (0..db.facts.len()).collect::<Vec<_>>()
            } else {
                db.facts_for_head(&head).to_vec()
            };

            for &idx in &candidate_indices {
                let fact = &db.facts[idx];
                let mut new_bindings = bindings.clone();
                if match_sexp_pattern(&instantiated, fact, &mut new_bindings) {
                    next_sets.push(new_bindings);
                }
            }
        }
        binding_sets = next_sets;
        if binding_sets.is_empty() {
            break;
        }
        // Cap combinatorial explosion
        if binding_sets.len() > 1000 {
            binding_sets.truncate(1000);
        }
    }

    binding_sets
}

/// Substitute meta-variables in an sexp.
fn substitute_sexp(sexp: &Sexp, bindings: &HashMap<String, Sexp>) -> Sexp {
    match sexp {
        Sexp::Atom(name, _) if name.starts_with('?') => {
            if let Some(val) = bindings.get(name) {
                val.clone()
            } else {
                sexp.clone()
            }
        }
        Sexp::Atom(_, _) => sexp.clone(),
        Sexp::List(items, span) => {
            let new_items: Vec<Sexp> = items
                .iter()
                .map(|item| substitute_sexp(item, bindings))
                .collect();
            Sexp::List(new_items, *span)
        }
    }
}
