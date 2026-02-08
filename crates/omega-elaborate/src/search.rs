/// Automated proof search via iterative deepening.
use omega_core::theory::Theory;

use crate::engine::ProofState;
use crate::tactic::Tactic;

/// Perform automated proof search up to a given depth.
pub fn auto_search(
    state: &ProofState,
    theory: &Theory,
    max_depth: usize,
) -> Result<ProofState, String> {
    if state.is_complete() {
        return Ok(state.clone());
    }

    // Try assumption first
    if let Ok(new_state) = state.apply_tactic(&Tactic::Assumption, theory) {
        if new_state.is_complete() {
            return Ok(new_state);
        }
        if max_depth > 0 {
            if let Ok(result) = auto_search(&new_state, theory, max_depth - 1) {
                return Ok(result);
            }
        }
    }

    if max_depth == 0 {
        return Err("auto: search depth exhausted".to_string());
    }

    // Try each rule
    for rule in &theory.rules {
        if let Ok(new_state) = state.apply_tactic(&Tactic::Apply(rule.name.clone()), theory) {
            if new_state.is_complete() {
                return Ok(new_state);
            }
            if let Ok(result) = auto_search(&new_state, theory, max_depth - 1) {
                return Ok(result);
            }
        }
    }

    Err("auto: no proof found".to_string())
}
