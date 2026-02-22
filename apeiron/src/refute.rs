use std::collections::HashMap;

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
