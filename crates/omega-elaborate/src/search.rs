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
    let initial_budget = budget;
    let result = auto_search_inner(state, theory, max_depth, &mut budget);
    let nodes_explored = initial_budget - budget;
    if nodes_explored > 100 {
        eprintln!("[auto] explored {} nodes (depth {})", nodes_explored, max_depth);
    }
    result
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

    let trace_enabled = std::env::var("AUTO_TRACE").is_ok();
    let trace_verbose = std::env::var("AUTO_TRACE_VERBOSE").is_ok();
    let initial_goal = if trace_enabled && !state.goals.is_empty() {
        use omega_core::binding::apply_meta_subst;
        let g = &state.goals[0];
        let resolved = apply_meta_subst(&g.target, &state.subst);
        if trace_verbose {
            eprintln!("[auto d={}] goal_raw={} subst_keys={:?} resolved={}",
                max_depth, g.target,
                state.subst.keys().collect::<Vec<_>>(),
                resolved);
        }
        Some(format!("{}", resolved))
    } else {
        None
    };

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
            if trace_enabled {
                eprintln!("[auto d={}] {} matched goal {}", max_depth, rule.name(),
                    initial_goal.as_deref().unwrap_or("?"));
            }
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
