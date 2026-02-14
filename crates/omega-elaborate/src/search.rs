/// Automated proof search via iterative deepening.
///
/// Returns the solved proof state AND the trace of primitive tactics
/// (Apply/Assumption) that achieved it. On backtrack, partial traces
/// are naturally discarded.
use omega_core::theory::Theory;

use crate::engine::ProofState;
use crate::tactic::Tactic;

/// Perform automated proof search up to a given depth.
///
/// On success, returns `(final_state, trace)` where `trace` is the sequence
/// of primitive tactics (Apply, Assumption) that solve all goals. The caller
/// can feed these directly to reconstruction without knowing Auto was involved.
pub fn auto_search(
    state: &ProofState,
    theory: &Theory,
    max_depth: usize,
) -> Result<(ProofState, Vec<Tactic>), String> {
    let mut budget: usize = 500_000;
    auto_search_inner(state, theory, max_depth, &mut budget)
}

fn auto_search_inner(
    state: &ProofState,
    theory: &Theory,
    max_depth: usize,
    budget: &mut usize,
) -> Result<(ProofState, Vec<Tactic>), String> {
    if *budget == 0 {
        return Err("auto: node budget exhausted".to_string());
    }
    *budget -= 1;

    if state.is_complete() {
        return Ok((state.clone(), vec![]));
    }

    // Try assumption first
    if let Ok(new_state) = state.apply_tactic(&Tactic::Assumption, theory) {
        if new_state.is_complete() {
            return Ok((new_state, vec![Tactic::Assumption]));
        }
        if max_depth > 0 {
            if let Ok((result, mut trace)) = auto_search_inner(&new_state, theory, max_depth - 1, budget) {
                trace.insert(0, Tactic::Assumption);
                return Ok((result, trace));
            }
        }
    }

    if max_depth == 0 {
        return Err("auto: search depth exhausted".to_string());
    }

    // Try each rule
    for rule in theory.rules() {
        if *budget == 0 {
            return Err("auto: node budget exhausted".to_string());
        }
        let tactic = Tactic::Apply(rule.name().clone());
        if let Ok(new_state) = state.apply_tactic(&tactic, theory) {
            if new_state.is_complete() {
                return Ok((new_state, vec![tactic]));
            }
            if let Ok((result, mut trace)) = auto_search_inner(&new_state, theory, max_depth - 1, budget) {
                trace.insert(0, tactic);
                return Ok((result, trace));
            }
        }
    }

    Err("auto: no proof found".to_string())
}
