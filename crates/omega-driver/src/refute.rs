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
    let affine = theory.context_mode() == ContextMode::Affine;
    let rewrites = theory.rewrites();
    let mut budget = BUDGET;

    // Pre-normalize assumptions and goal
    let norm_assumptions: Vec<Expr> = assumptions.iter().map(|a| normalize(a, rewrites)).collect();
    let norm_goal = normalize(goal, rewrites);

    if can_derive(
        theory,
        &norm_assumptions,
        0u64,
        affine,
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

/// Try to derive `goal` from `assumptions` with the given consumed bitmask.
/// Returns true if ANY proof exists (the refutation fails).
///
/// Both `goal` and `assumptions` are expected to be pre-normalized.
fn can_derive(
    theory: &Theory,
    assumptions: &[Expr],
    consumed: u64,
    affine: bool,
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
        if affine && (consumed & (1u64 << i)) != 0 {
            continue;
        }
        if &assumptions[i] == goal {
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

            // Skip if any premise still has unresolved metas
            if premises.iter().any(|p| has_metas(p)) {
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
                affine,
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
    affine: bool,
    premises: &[Expr],
    extensions: &[Vec<Expr>],
    idx: usize,
    depth: usize,
    budget: &mut usize,
) -> bool {
    if idx >= premises.len() {
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
            affine,
            goal,
            depth,
            budget,
            &|new_consumed, budget| {
                can_derive_all(
                    theory,
                    assumptions,
                    new_consumed,
                    affine,
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
            affine,
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
                    affine,
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
    affine: bool,
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
        if affine && (consumed & (1u64 << i)) != 0 {
            continue;
        }
        if &assumptions[i] == goal {
            let new_consumed = if affine {
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
                affine,
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
    affine: bool,
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
            affine,
            goal,
            depth,
            budget,
            &|new_consumed, budget| {
                enumerate_premises_then(
                    theory,
                    assumptions,
                    new_consumed,
                    affine,
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
            affine,
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
                    affine,
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
