/// Exhaustive refutation engine: bounded model checking for affine logic.
///
/// Given a set of assumptions and a goal, performs backward-chaining proof
/// search. If the search exhausts all possible derivation paths without
/// finding a proof, the impossibility is verified ("refuted").
///
/// Key insight: in affine logic (resources used at most once), the search
/// space is inherently finite — each derivation step consumes or transforms
/// assumptions, so paths cannot loop.
///
/// Soundness: expressions are normalized via the theory's rewrite rules
/// before comparison, so rules like `eq-refl` that depend on definitional
/// equality (e.g., `(energy (quark z)) → (s z)`) are handled correctly.
use omega_core::binding::subst_meta;
use omega_core::derivation::normalize_expr;
use omega_core::expr::Expr;
use omega_core::judgment::RewriteRule;
use omega_core::pattern;
use omega_core::pattern::Substitution;
use omega_core::theory::{ContextMode, Theory};

/// Result of an exhaustive refutation attempt.
pub enum RefuteResult {
    /// All derivation paths exhausted — no proof exists. Impossibility verified.
    Refuted,
    /// A proof was found — the goal IS derivable. Refutation fails.
    Derivable,
    /// Search budget exhausted before completing. Result is inconclusive.
    BudgetExhausted,
}

const BUDGET: usize = 1_000_000;
const NORM_FUEL: usize = 1_000;

/// Normalize an expression using the theory's rewrite rules.
fn normalize(expr: &Expr, rewrites: &[RewriteRule]) -> Expr {
    let mut fuel = NORM_FUEL;
    normalize_expr(expr, rewrites, &mut fuel)
}

/// Attempt to refute: prove that `goal` is NOT derivable from `assumptions`
/// in the given theory, searching up to `max_depth` rule applications.
pub fn exhaustive_refute(
    theory: &Theory,
    assumptions: &[Expr],
    goal: &Expr,
    max_depth: usize,
) -> RefuteResult {
    assert!(assumptions.len() <= 64, "refute supports at most 64 assumptions");
    let mode = theory.context_mode();
    let track_usage = matches!(mode, ContextMode::Affine | ContextMode::Linear);
    let linear = mode == ContextMode::Linear;
    let rewrites = theory.rewrites();
    let mut budget = BUDGET;

    // Pre-normalize assumptions and goal
    let norm_assumptions: Vec<Expr> = assumptions.iter().map(|a| normalize(a, rewrites)).collect();
    let norm_goal = normalize(goal, rewrites);

    // Bitmask of all base assumptions (for linear mode completeness check)
    let all_consumed = if norm_assumptions.len() < 64 {
        (1u64 << norm_assumptions.len()) - 1
    } else {
        u64::MAX
    };

    if can_derive(
        theory,
        &norm_assumptions,
        0u64,
        track_usage,
        linear,
        all_consumed,
        &norm_goal,
        max_depth,
        &mut budget,
    ) {
        RefuteResult::Derivable
    } else if budget == 0 {
        RefuteResult::BudgetExhausted
    } else {
        RefuteResult::Refuted
    }
}

/// Apply a substitution (HashMap<Name, Expr>) to an expression by iterating
/// over all bindings and calling subst_meta for each.
fn apply_subst(expr: &Expr, subst: &pattern::Substitution) -> Expr {
    let mut result = expr.clone();
    for (name, val) in subst {
        result = subst_meta(&result, name, val);
    }
    result
}

/// Apply substitution and then normalize.
fn apply_subst_norm(expr: &Expr, subst: &pattern::Substitution, rewrites: &[RewriteRule]) -> Expr {
    normalize(&apply_subst(expr, subst), rewrites)
}

/// Check if an expression contains any meta-variables.
fn has_metas(expr: &Expr) -> bool {
    match expr {
        Expr::Meta(_) => true,
        Expr::Free(_) | Expr::Bound(_) | Expr::Sym(_) => false,
        Expr::App(args) => args.iter().any(has_metas),
        Expr::Binder { ty, body, .. } => has_metas(ty) || has_metas(body),
    }
}

/// Merge two substitutions. Returns None if they conflict (same key, different value).
fn merge_subst(a: &Substitution, b: &Substitution) -> Option<Substitution> {
    let mut merged = a.clone();
    for (k, v) in b {
        if let Some(existing) = merged.get(k) {
            if existing != v {
                return None;
            }
        } else {
            merged.insert(k.clone(), v.clone());
        }
    }
    Some(merged)
}

/// Resolve meta-bearing premises by matching them against assumptions or zero-premise rules.
///
/// `raw_premises`: original premises from the rule (before any substitution).
/// `base_subst`: substitution from matching the rule's conclusion against the goal.
/// `meta_indices`: indices of premises that still have metas after applying base_subst.
/// `step`: current index into meta_indices.
/// `extra_subst`: bindings accumulated during resolution (merged with base_subst at the end).
/// `resolved`: bitmask of premise indices resolved by assumption matching.
///
/// When all meta-bearing premises are resolved, calls `on_done(extra_subst, consumed, resolved)`.
fn resolve_meta_step(
    theory: &Theory,
    raw_premises: &[Expr],
    base_subst: &Substitution,
    meta_indices: &[usize],
    step: usize,
    assumptions: &[Expr],
    consumed: u64,
    track_usage: bool,
    extra_subst: &Substitution,
    resolved: u64,
    rewrites: &[RewriteRule],
    budget: &mut usize,
    on_done: &dyn Fn(&Substitution, u64, u64, &mut usize) -> bool,
) -> bool {
    if *budget == 0 {
        return false;
    }
    *budget -= 1;

    if step >= meta_indices.len() {
        return on_done(extra_subst, consumed, resolved, budget);
    }

    let pidx = meta_indices[step];

    // Apply base_subst + extra_subst to get current premise
    let premise = apply_subst_norm(
        &apply_subst(&raw_premises[pidx], base_subst),
        extra_subst,
        rewrites,
    );

    if !has_metas(&premise) {
        // Already resolved by earlier substitutions — still needs derivation, not resolved by assumption
        return resolve_meta_step(
            theory, raw_premises, base_subst, meta_indices, step + 1,
            assumptions, consumed, track_usage, extra_subst, resolved,
            rewrites, budget, on_done,
        );
    }

    // Try matching against each unconsumed assumption
    for i in 0..assumptions.len().min(64) {
        if *budget == 0 {
            return false;
        }
        if track_usage && (consumed & (1u64 << i)) != 0 {
            continue;
        }

        if let Ok(new_bindings) = pattern::match_expr(&premise, &assumptions[i]) {
            if let Some(merged) = merge_subst(extra_subst, &new_bindings) {
                let new_consumed = if track_usage {
                    consumed | (1u64 << i)
                } else {
                    consumed
                };
                let new_resolved = resolved | (1u64 << pidx);

                if resolve_meta_step(
                    theory, raw_premises, base_subst, meta_indices, step + 1,
                    assumptions, new_consumed, track_usage, &merged, new_resolved,
                    rewrites, budget, on_done,
                ) {
                    return true;
                }
            }
        }
    }

    // Try zero-premise rules
    for rule in theory.rules() {
        if *budget == 0 {
            return false;
        }
        if !rule.premises().is_empty() {
            continue;
        }

        if let Ok(new_bindings) = pattern::match_expr(&premise, rule.conclusion()) {
            if let Some(merged) = merge_subst(extra_subst, &new_bindings) {
                let new_resolved = resolved | (1u64 << pidx);

                if resolve_meta_step(
                    theory, raw_premises, base_subst, meta_indices, step + 1,
                    assumptions, consumed, track_usage, &merged, new_resolved,
                    rewrites, budget, on_done,
                ) {
                    return true;
                }
            }
        }
    }

    false
}

/// Build resolved premises and extensions from a completed meta-resolution.
/// Returns (remaining_premises, remaining_extensions) excluding resolved premises.
fn build_resolved_premises(
    rule_premises: &[Expr],
    full_subst: &Substitution,
    resolved_mask: u64,
    context_extensions: &[(usize, Expr)],
    rewrites: &[RewriteRule],
) -> (Vec<Expr>, Vec<Vec<Expr>>) {
    let mut remaining_premises = Vec::new();
    let mut remaining_extensions = Vec::new();

    for (i, p) in rule_premises.iter().enumerate() {
        if resolved_mask & (1u64 << i) != 0 {
            continue; // Already satisfied by assumption matching
        }
        remaining_premises.push(apply_subst_norm(p, full_subst, rewrites));
        remaining_extensions.push(
            context_extensions
                .iter()
                .filter(|(ext_idx, _)| *ext_idx == i)
                .map(|(_, ext)| apply_subst_norm(ext, full_subst, rewrites))
                .collect(),
        );
    }

    (remaining_premises, remaining_extensions)
}

/// Try to derive `goal` from `assumptions` with the given consumed bitmask.
/// Returns true if ANY proof exists (the refutation fails).
///
/// Both `goal` and `assumptions` are expected to be pre-normalized.
///
/// In linear mode (`linear=true`), a proof is only valid if ALL base
/// assumptions are consumed. `all_consumed` is the bitmask of all base
/// assumption bits that must be set.
fn can_derive(
    theory: &Theory,
    assumptions: &[Expr],
    consumed: u64,
    track_usage: bool,
    linear: bool,
    all_consumed: u64,
    goal: &Expr,
    depth: usize,
    budget: &mut usize,
) -> bool {
    if *budget == 0 {
        return false;
    }
    *budget -= 1;

    // 1. Try matching each available assumption against the goal
    for i in 0..assumptions.len() {
        if track_usage && (consumed & (1u64 << i)) != 0 {
            continue;
        }
        if &assumptions[i] == goal {
            let new_consumed = if track_usage {
                consumed | (1u64 << i)
            } else {
                consumed
            };
            // In linear mode, all base assumptions must be consumed
            if linear && (new_consumed & all_consumed) != all_consumed {
                continue; // Not all consumed — not a valid linear proof
            }
            return true; // Proof found via assumption
        }
    }

    if depth == 0 {
        return false;
    }

    let rewrites = theory.rewrites();

    // 2. Try each rule whose conclusion matches the goal
    for rule in theory.rules() {
        if let Ok(subst) = pattern::match_expr(rule.conclusion(), goal) {
            // Compute concrete, normalized premises
            let premises: Vec<Expr> = rule
                .premises()
                .iter()
                .map(|p| apply_subst_norm(p, &subst, rewrites))
                .collect();

            if premises.iter().any(|p| has_metas(p)) {
                // Meta-resolution: try matching meta-bearing premises against assumptions
                let meta_indices: Vec<usize> = premises
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| has_metas(p))
                    .map(|(i, _)| i)
                    .collect();

                let empty_extra = Substitution::new();

                if resolve_meta_step(
                    theory,
                    rule.premises(),
                    &subst,
                    &meta_indices,
                    0,
                    assumptions,
                    consumed,
                    track_usage,
                    &empty_extra,
                    0u64,
                    rewrites,
                    budget,
                    &|extra_subst, resolved_consumed, resolved_mask, budget| {
                        let full_subst = merge_subst(&subst, extra_subst)
                            .unwrap_or_else(|| subst.clone());
                        let (rem_premises, rem_extensions) = build_resolved_premises(
                            rule.premises(),
                            &full_subst,
                            resolved_mask,
                            rule.context_extensions(),
                            rewrites,
                        );
                        if rem_premises.iter().any(|p| has_metas(p)) {
                            return false;
                        }
                        can_derive_all(
                            theory,
                            assumptions,
                            resolved_consumed,
                            track_usage,
                            linear,
                            all_consumed,
                            &rem_premises,
                            &rem_extensions,
                            0,
                            depth - 1,
                            budget,
                        )
                    },
                ) {
                    return true;
                }
                continue;
            }

            // Compute context extensions per premise (also normalized)
            let extensions: Vec<Vec<Expr>> = (0..premises.len())
                .map(|idx| {
                    rule.context_extensions()
                        .iter()
                        .filter(|(ext_idx, _)| *ext_idx == idx)
                        .map(|(_, ext)| apply_subst_norm(ext, &subst, rewrites))
                        .collect()
                })
                .collect();

            // Try to derive all premises (with backtracking for context splitting)
            if can_derive_all(
                theory,
                assumptions,
                consumed,
                track_usage,
                linear,
                all_consumed,
                &premises,
                &extensions,
                0,
                depth - 1,
                budget,
            ) {
                return true;
            }
        }
    }

    false
}

/// Try to derive premises[idx..] sequentially from assumptions.
///
/// For each way to derive premise[idx], try remaining premises with
/// the updated consumed mask. This handles affine context splitting
/// via backtracking: each premise "claims" some assumptions, and
/// remaining premises work with what's left.
fn can_derive_all(
    theory: &Theory,
    assumptions: &[Expr],
    consumed: u64,
    track_usage: bool,
    linear: bool,
    all_consumed: u64,
    premises: &[Expr],
    extensions: &[Vec<Expr>],
    idx: usize,
    depth: usize,
    budget: &mut usize,
) -> bool {
    if idx >= premises.len() {
        // In linear mode, check that all base assumptions are consumed
        if linear && (consumed & all_consumed) != all_consumed {
            return false;
        }
        return true; // All premises derived
    }

    let goal = &premises[idx];
    let exts = &extensions[idx];

    // Build extended assumptions for this premise (base + context extensions)
    if exts.is_empty() {
        enumerate_proofs(
            theory,
            assumptions,
            consumed,
            track_usage,
            goal,
            depth,
            budget,
            &|new_consumed, budget| {
                can_derive_all(
                    theory,
                    assumptions,
                    new_consumed,
                    track_usage,
                    linear,
                    all_consumed,
                    premises,
                    extensions,
                    idx + 1,
                    depth,
                    budget,
                )
            },
        )
    } else {
        let mut extended = assumptions.to_vec();
        extended.extend(exts.iter().cloned());
        enumerate_proofs(
            theory,
            &extended,
            consumed,
            track_usage,
            goal,
            depth,
            budget,
            &|new_consumed, budget| {
                let base_mask = if assumptions.len() < 64 {
                    (1u64 << assumptions.len()) - 1
                } else {
                    u64::MAX
                };
                let base_consumed = new_consumed & base_mask;
                can_derive_all(
                    theory,
                    assumptions,
                    base_consumed,
                    track_usage,
                    linear,
                    all_consumed,
                    premises,
                    extensions,
                    idx + 1,
                    depth,
                    budget,
                )
            },
        )
    }
}

/// Find all ways to derive `goal` from `assumptions`.
/// For each successful derivation, calls `on_success(new_consumed, budget)`.
/// If `on_success` returns true, stops early (short-circuit).
/// Returns true if `on_success` ever returned true.
fn enumerate_proofs(
    theory: &Theory,
    assumptions: &[Expr],
    consumed: u64,
    track_usage: bool,
    goal: &Expr,
    depth: usize,
    budget: &mut usize,
    on_success: &dyn Fn(u64, &mut usize) -> bool,
) -> bool {
    if *budget == 0 {
        return false;
    }
    *budget -= 1;

    // 1. Try matching each available assumption
    for i in 0..assumptions.len().min(64) {
        if track_usage && (consumed & (1u64 << i)) != 0 {
            continue;
        }
        if &assumptions[i] == goal {
            let new_consumed = if track_usage {
                consumed | (1u64 << i)
            } else {
                consumed
            };
            if on_success(new_consumed, budget) {
                return true;
            }
        }
    }

    if depth == 0 {
        return false;
    }

    let rewrites = theory.rewrites();

    // 2. Try each rule
    for rule in theory.rules() {
        if *budget == 0 {
            return false;
        }

        if let Ok(subst) = pattern::match_expr(rule.conclusion(), goal) {
            let premises: Vec<Expr> = rule
                .premises()
                .iter()
                .map(|p| apply_subst_norm(p, &subst, rewrites))
                .collect();

            if premises.iter().any(|p| has_metas(p)) {
                // Meta-resolution path
                let meta_indices: Vec<usize> = premises
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| has_metas(p))
                    .map(|(i, _)| i)
                    .collect();

                let empty_extra = Substitution::new();

                if resolve_meta_step(
                    theory,
                    rule.premises(),
                    &subst,
                    &meta_indices,
                    0,
                    assumptions,
                    consumed,
                    track_usage,
                    &empty_extra,
                    0u64,
                    rewrites,
                    budget,
                    &|extra_subst, resolved_consumed, resolved_mask, budget| {
                        let full_subst = merge_subst(&subst, extra_subst)
                            .unwrap_or_else(|| subst.clone());
                        let (rem_premises, rem_extensions) = build_resolved_premises(
                            rule.premises(),
                            &full_subst,
                            resolved_mask,
                            rule.context_extensions(),
                            rewrites,
                        );
                        if rem_premises.iter().any(|p| has_metas(p)) {
                            return false;
                        }
                        if rem_premises.is_empty() {
                            return on_success(resolved_consumed, budget);
                        }
                        enumerate_premises_then(
                            theory,
                            assumptions,
                            resolved_consumed,
                            track_usage,
                            &rem_premises,
                            &rem_extensions,
                            0,
                            depth - 1,
                            budget,
                            on_success,
                        )
                    },
                ) {
                    return true;
                }
                continue;
            }

            // Zero-premise rules: immediate success
            if premises.is_empty() {
                if on_success(consumed, budget) {
                    return true;
                }
                continue;
            }

            let extensions: Vec<Vec<Expr>> = (0..premises.len())
                .map(|idx| {
                    rule.context_extensions()
                        .iter()
                        .filter(|(ext_idx, _)| *ext_idx == idx)
                        .map(|(_, ext)| apply_subst_norm(ext, &subst, rewrites))
                        .collect()
                })
                .collect();

            if enumerate_premises_then(
                theory,
                assumptions,
                consumed,
                track_usage,
                &premises,
                &extensions,
                0,
                depth - 1,
                budget,
                on_success,
            ) {
                return true;
            }
        }
    }

    false
}

/// Derive premises[idx..] and then call `then_do` with the final consumed mask.
fn enumerate_premises_then(
    theory: &Theory,
    assumptions: &[Expr],
    consumed: u64,
    track_usage: bool,
    premises: &[Expr],
    extensions: &[Vec<Expr>],
    idx: usize,
    depth: usize,
    budget: &mut usize,
    then_do: &dyn Fn(u64, &mut usize) -> bool,
) -> bool {
    if idx >= premises.len() {
        return then_do(consumed, budget);
    }

    let goal = &premises[idx];
    let exts = &extensions[idx];

    if exts.is_empty() {
        enumerate_proofs(
            theory,
            assumptions,
            consumed,
            track_usage,
            goal,
            depth,
            budget,
            &|new_consumed, budget| {
                enumerate_premises_then(
                    theory,
                    assumptions,
                    new_consumed,
                    track_usage,
                    premises,
                    extensions,
                    idx + 1,
                    depth,
                    budget,
                    then_do,
                )
            },
        )
    } else {
        let mut extended = assumptions.to_vec();
        extended.extend(exts.iter().cloned());
        enumerate_proofs(
            theory,
            &extended,
            consumed,
            track_usage,
            goal,
            depth,
            budget,
            &|new_consumed, budget| {
                let base_mask = if assumptions.len() < 64 {
                    (1u64 << assumptions.len()) - 1
                } else {
                    u64::MAX
                };
                let base_consumed = new_consumed & base_mask;
                enumerate_premises_then(
                    theory,
                    assumptions,
                    base_consumed,
                    track_usage,
                    premises,
                    extensions,
                    idx + 1,
                    depth,
                    budget,
                    then_do,
                )
            },
        )
    }
}
