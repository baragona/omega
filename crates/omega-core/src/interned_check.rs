/// Interned derivation checker: wraps the proof checking hot path
/// with hash-consing so that equality is O(1) and substitution
/// produces maximally shared results.
///
/// Performance wins:
/// - O(1) structural equality via handle comparison (vs O(n) tree walk)
/// - `has_metas` cached per node (vs O(n) recursive check)
/// - Substitution with maximal sharing (unchanged subtrees aren't cloned)
/// - Pre-interned theory rules (no re-interning on each check)
use std::collections::{HashMap, HashSet};

use crate::derivation::{Context, Derivation};
use crate::error::{OmegaError, Result};
use crate::expr::{Expr, Name};
use crate::intern::{Arena, HExpr};
use crate::theory::{ContextMode, Theory};

type HSubst = HashMap<Name, HExpr>;

/// A pre-interned rewrite rule for normalization.
struct InternedRewrite {
    lhs: HExpr,
    rhs: HExpr,
}

/// A cached interned representation of a theory.
/// Build once, reuse for many proof checks to avoid re-interning overhead.
pub struct InternedTheory {
    arena: Arena,
    rule_cache: HashMap<String, InternedRule>,
    rewrites: Vec<InternedRewrite>,
    reduce_cache: HashMap<HExpr, HExpr>,
    pub reduce_fuel: usize,
    fresh_counter: usize,
    context_mode: ContextMode,
}

impl InternedTheory {
    /// Build an interned theory from a regular theory.
    pub fn new(theory: &Theory) -> Self {
        let mut arena = Arena::new();
        // Thread AC/ACI symbols from theory attributes into arena
        for (name, attrs) in theory.attributes() {
            use crate::theory::Attribute;
            if attrs.contains(&Attribute::ACI) {
                arena.aci_symbols.insert(name.clone());
            } else if attrs.contains(&Attribute::AC) {
                arena.ac_symbols.insert(name.clone());
            }
        }
        // Thread binder behavior flags from theory into arena
        arena.substitutive_binders = theory.substitutive_binders().clone();
        arena.eta_binders = theory.eta_binders().clone();
        arena.linear_binders = theory.linear_binders().clone();
        arena.affine_binders = theory.affine_binders().clone();

        let mut rule_cache: HashMap<String, InternedRule> = HashMap::new();
        for rule in theory.rules() {
            rule_cache.insert(rule.name.clone(), intern_rule(&mut arena, rule));
        }
        let mut rewrites = Vec::new();
        for rw in theory.rewrites() {
            let lhs = arena.from_expr(&rw.lhs);
            let rhs = arena.from_expr(&rw.rhs);
            rewrites.push(InternedRewrite { lhs, rhs });
        }

        InternedTheory {
            arena,
            rule_cache,
            rewrites,
            reduce_cache: HashMap::new(),
            reduce_fuel: 10_000,
            fresh_counter: 0,
            context_mode: theory.context_mode(),
        }
    }

    /// Add a new rule (e.g., from reflection) to the cached theory.
    pub fn add_rule(&mut self, rule: &crate::judgment::Rule) {
        let ir = intern_rule(&mut self.arena, rule);
        self.rule_cache.insert(rule.name.clone(), ir);
    }

    /// Check a derivation using the cached arena and rules.
    pub fn check(
        &mut self,
        goal: &Expr,
        derivation: &Derivation,
        ctx: &Context,
    ) -> Result<()> {
        let h_goal = self.arena.from_expr(goal);
        let h_assumptions: Vec<HExpr> =
            ctx.assumptions.iter().map(|a| self.arena.from_expr(a)).collect();
        let mut global_subst = HashMap::new();
        let mut consumed = HashSet::new();
        let mut state = CheckState {
            arena: &mut self.arena,
            rule_cache: &self.rule_cache,
            rewrites: &self.rewrites,
            reduce_cache: &mut self.reduce_cache,
            reduce_fuel: self.reduce_fuel,
            context_mode: self.context_mode,
            global_subst: &mut global_subst,
            fresh_counter: &mut self.fresh_counter,
            consumed: &mut consumed,
        };
        check_inner(&mut state, h_goal, derivation, &h_assumptions)
    }

    /// Check a derivation with a pre-interned goal and assumptions.
    /// Bypasses tree construction entirely — the term never exists as an Expr.
    pub fn check_h(
        &mut self,
        h_goal: HExpr,
        derivation: &Derivation,
        h_assumptions: &[HExpr],
    ) -> Result<()> {
        let mut global_subst = HashMap::new();
        let mut consumed = HashSet::new();
        let mut state = CheckState {
            arena: &mut self.arena,
            rule_cache: &self.rule_cache,
            rewrites: &self.rewrites,
            reduce_cache: &mut self.reduce_cache,
            reduce_fuel: self.reduce_fuel,
            context_mode: self.context_mode,
            global_subst: &mut global_subst,
            fresh_counter: &mut self.fresh_counter,
            consumed: &mut consumed,
        };
        check_inner(&mut state, h_goal, derivation, h_assumptions)
    }

    /// Mutable access to the arena for direct term construction.
    pub fn arena_mut(&mut self) -> &mut Arena {
        &mut self.arena
    }
}

/// Check a derivation using the interned (hash-consed) checker.
/// Creates a fresh arena per call — use `InternedTheory` for repeated checks.
pub fn check_derivation_interned(
    theory: &Theory,
    goal: &Expr,
    derivation: &Derivation,
    ctx: &Context,
) -> Result<()> {
    let mut cached = InternedTheory::new(theory);
    cached.check(goal, derivation, ctx)
}

struct InternedRule {
    name: Name,
    conclusion: HExpr,
    premises: Vec<HExpr>,
    implicit_args: Vec<Name>,
    /// Pre-cached meta-variable names (avoids tree conversion during freshening).
    meta_names: Vec<Name>,
    /// Context extensions: (premise_index, assumption_to_add).
    context_extensions: Vec<(usize, HExpr)>,
}

/// Intern a single rule into the arena, collecting all meta-variable names.
fn intern_rule(arena: &mut Arena, rule: &crate::judgment::Rule) -> InternedRule {
    let h_conclusion = arena.from_expr(&rule.conclusion);
    let h_premises: Vec<HExpr> = rule.premises.iter().map(|p| arena.from_expr(p)).collect();
    let h_context_extensions: Vec<(usize, HExpr)> = rule
        .context_extensions
        .iter()
        .map(|(idx, expr)| (*idx, arena.from_expr(expr)))
        .collect();
    let mut meta_names = arena.meta_vars(h_conclusion);
    for hp in &h_premises {
        for m in arena.meta_vars(*hp) {
            if !meta_names.contains(&m) {
                meta_names.push(m);
            }
        }
    }
    for (_, hce) in &h_context_extensions {
        for m in arena.meta_vars(*hce) {
            if !meta_names.contains(&m) {
                meta_names.push(m);
            }
        }
    }
    InternedRule {
        name: rule.name.clone(),
        conclusion: h_conclusion,
        premises: h_premises,
        implicit_args: rule.implicit_args.clone(),
        meta_names,
        context_extensions: h_context_extensions,
    }
}

fn freshen_interned_rule(
    arena: &mut Arena,
    rule: &InternedRule,
    counter: &mut usize,
) -> InternedRule {
    if rule.meta_names.is_empty() {
        return InternedRule {
            name: rule.name.clone(),
            conclusion: rule.conclusion,
            premises: rule.premises.clone(),
            implicit_args: rule.implicit_args.clone(),
            meta_names: rule.meta_names.clone(),
            context_extensions: rule.context_extensions.clone(),
        };
    }

    *counter += 1;
    let suffix = format!("${}", counter);

    let mut rename = HSubst::new();
    for m in &rule.meta_names {
        rename.insert(m.clone(), arena.meta(&format!("{}{}", m, suffix)));
    }

    InternedRule {
        name: rule.name.clone(),
        conclusion: arena.apply_meta_subst(rule.conclusion, &rename),
        premises: rule
            .premises
            .iter()
            .map(|p| arena.apply_meta_subst(*p, &rename))
            .collect(),
        implicit_args: rule
            .implicit_args
            .iter()
            .map(|a| format!("{}{}", a, suffix))
            .collect(),
        meta_names: rule.meta_names.iter().map(|m| format!("{}{}", m, suffix)).collect(),
        context_extensions: rule.context_extensions
            .iter()
            .map(|(idx, h)| (*idx, arena.apply_meta_subst(*h, &rename)))
            .collect(),
    }
}

/// Apply substitution with fixpoint iteration for transitive chains.
/// Capped at 100 iterations to prevent divergence from cross-proof meta bindings.
fn apply_fixpoint(arena: &mut Arena, h: HExpr, subst: &HSubst) -> HExpr {
    let mut result = arena.apply_meta_subst(h, subst);
    for _ in 0..100 {
        let next = arena.apply_meta_subst(result, subst);
        if next == result {
            return result;
        }
        result = next;
    }
    result
}

/// Bidirectional unification on HExprs using native Arena method.
fn unify_h(arena: &mut Arena, a: HExpr, b: HExpr) -> Option<HSubst> {
    let mut subst = HSubst::new();
    if arena.unify_exprs(a, b, &mut subst) {
        Some(subst)
    } else {
        None
    }
}

/// Normalize an HExpr by exhaustively applying beta-reduction and
/// rewrite rules (innermost strategy).
/// Memoized via reduce_cache. Returns early if fuel is exhausted.
fn normalize(
    arena: &mut Arena,
    rewrites: &[InternedRewrite],
    cache: &mut HashMap<HExpr, HExpr>,
    h: HExpr,
    fuel: &mut usize,
) -> HExpr {
    if *fuel == 0 {
        return h;
    }
    if let Some(&cached) = cache.get(&h) {
        return cached;
    }

    // Step 0: WHNF first to reduce any head beta-redexes
    let whnf_ed = arena.whnf(h);

    if rewrites.is_empty() {
        return whnf_ed;
    }

    // Step 1: Normalize children
    let children_normalized = if let Some(args) = arena.app_args(whnf_ed) {
        let new_args: Vec<HExpr> = args
            .iter()
            .map(|&a| normalize(arena, rewrites, cache, a, fuel))
            .collect();
        if new_args == args {
            whnf_ed
        } else {
            arena.app(new_args)
        }
    } else {
        whnf_ed
    };

    // Step 1.5: WHNF again — child normalization may have exposed
    // beta-redexes (e.g. a rewrite expanded a constructor into a lambda)
    let after_whnf = arena.whnf(children_normalized);
    if after_whnf != children_normalized {
        let result = normalize(arena, rewrites, cache, after_whnf, fuel);
        cache.insert(h, result);
        return result;
    }

    // Step 2: Try rewrite rules at the head
    let mut current = children_normalized;
    loop {
        if *fuel == 0 {
            break;
        }
        let mut matched = false;
        for rw in rewrites {
            if let Ok(subst) = arena.match_expr(rw.lhs, current) {
                *fuel = fuel.saturating_sub(1);
                let replaced = arena.apply_meta_subst(rw.rhs, &subst);
                // Normalize the result recursively
                current = normalize(arena, rewrites, cache, replaced, fuel);
                matched = true;
                break;
            }
        }
        if !matched {
            break;
        }
    }

    cache.insert(h, current);
    current
}

/// Mutable state threaded through derivation checking.
struct CheckState<'a> {
    arena: &'a mut Arena,
    rule_cache: &'a HashMap<String, InternedRule>,
    rewrites: &'a [InternedRewrite],
    reduce_cache: &'a mut HashMap<HExpr, HExpr>,
    reduce_fuel: usize,
    context_mode: ContextMode,
    global_subst: &'a mut HSubst,
    fresh_counter: &'a mut usize,
    consumed: &'a mut HashSet<usize>,
}

fn check_inner(
    state: &mut CheckState,
    goal: HExpr,
    derivation: &Derivation,
    assumptions: &[HExpr],
) -> Result<()> {
    let affine = state.context_mode == ContextMode::Affine;
    let mut fuel = state.reduce_fuel;
    match derivation {
        Derivation::Assumption => {
            let goal_resolved = apply_fixpoint(state.arena, goal, state.global_subst);
            let goal_norm = normalize(state.arena, state.rewrites, state.reduce_cache, goal_resolved, &mut fuel);

            // In affine mode, iterate from the end (most recent first),
            // skipping consumed entries — this gives shadowing for free.
            for idx in (0..assumptions.len()).rev() {
                if affine && state.consumed.contains(&idx) {
                    continue;
                }
                let assumption_resolved = apply_fixpoint(state.arena, assumptions[idx], state.global_subst);
                let assumption_norm = normalize(state.arena, state.rewrites, state.reduce_cache, assumption_resolved, &mut fuel);

                // O(1) equality check!
                if assumption_norm == goal_norm {
                    if affine {
                        state.consumed.insert(idx);
                    }
                    return Ok(());
                }

                if let Ok(sub) = state.arena.match_expr(assumption_norm, goal_norm) {
                    for (k, v) in sub {
                        state.global_subst.insert(k, v);
                    }
                    if affine {
                        state.consumed.insert(idx);
                    }
                    return Ok(());
                }
                if let Ok(sub) = state.arena.match_expr(goal_norm, assumption_norm) {
                    for (k, v) in sub {
                        state.global_subst.insert(k, v);
                    }
                    if affine {
                        state.consumed.insert(idx);
                    }
                    return Ok(());
                }
            }
            Err(OmegaError::AssumptionMismatch {
                goal: state.arena.to_expr(goal_norm),
            })
        }

        Derivation::AssumptionIdx(idx) => {
            if *idx >= assumptions.len() {
                return Err(OmegaError::AssumptionIndexOutOfBounds {
                    index: *idx,
                    count: assumptions.len(),
                });
            }
            if affine && state.consumed.contains(idx) {
                return Err(OmegaError::UseAfterMove {
                    index: *idx,
                    expr: state.arena.to_expr(assumptions[*idx]),
                });
            }
            let assumption = apply_fixpoint(state.arena, assumptions[*idx], state.global_subst);
            let assumption_norm = normalize(state.arena, state.rewrites, state.reduce_cache, assumption, &mut fuel);
            let goal_resolved = apply_fixpoint(state.arena, goal, state.global_subst);
            let goal_norm = normalize(state.arena, state.rewrites, state.reduce_cache, goal_resolved, &mut fuel);

            if assumption_norm == goal_norm {
                if affine {
                    state.consumed.insert(*idx);
                }
                return Ok(());
            }
            if let Ok(sub) = state.arena.match_expr(goal_norm, assumption_norm) {
                for (k, v) in sub {
                    state.global_subst.insert(k, v);
                }
                if affine {
                    state.consumed.insert(*idx);
                }
                return Ok(());
            }
            if let Ok(sub) = state.arena.match_expr(assumption_norm, goal_norm) {
                for (k, v) in sub {
                    state.global_subst.insert(k, v);
                }
                if affine {
                    state.consumed.insert(*idx);
                }
                return Ok(());
            }
            Err(OmegaError::GoalMismatch {
                expected: state.arena.to_expr(goal_norm),
                got: state.arena.to_expr(assumption_norm),
            })
        }

        Derivation::RuleApp {
            rule_name,
            premises,
        } => {
            let orig_rule = state.rule_cache
                .get(rule_name)
                .ok_or_else(|| OmegaError::UnknownName { kind: "rule".into(), name: rule_name.clone() })?;

            if premises.len() != orig_rule.premises.len() {
                return Err(OmegaError::PremiseCountMismatch {
                    rule: rule_name.clone(),
                    expected: orig_rule.premises.len(),
                    got: premises.len(),
                });
            }

            let rule = freshen_interned_rule(state.arena, orig_rule, state.fresh_counter);
            let goal_resolved = apply_fixpoint(state.arena, goal, state.global_subst);
            let goal_norm = normalize(state.arena, state.rewrites, state.reduce_cache, goal_resolved, &mut fuel);

            let mut local_subst: HSubst =
                match state.arena.match_expr(rule.conclusion, goal_norm) {
                    Ok(s) => s,
                    Err(_cause) => {
                        if state.arena.has_metas(goal_norm) {
                            state.arena
                                .match_expr(goal_norm, rule.conclusion)
                                .map_err(|_| OmegaError::PatternMatchFailed {
                                    rule: rule_name.clone(),
                                    expected: state.arena.to_expr(rule.conclusion),
                                    got: state.arena.to_expr(goal_norm),
                                    cause: crate::pattern::MatchError::Mismatch {
                                        pattern: state.arena.to_expr(rule.conclusion),
                                        expr: state.arena.to_expr(goal_norm),
                                    },
                                })?
                        } else {
                            return Err(OmegaError::PatternMatchFailed {
                                rule: rule_name.clone(),
                                expected: state.arena.to_expr(rule.conclusion),
                                got: state.arena.to_expr(goal_norm),
                                cause: crate::pattern::MatchError::Mismatch {
                                    pattern: state.arena.to_expr(rule.conclusion),
                                    expr: state.arena.to_expr(goal_norm),
                                },
                            });
                        }
                    }
                };

            for (i, (premise_derivation, &premise_pattern)) in
                premises.iter().zip(rule.premises.iter()).enumerate()
            {
                let mut premise_goal =
                    state.arena.apply_meta_subst(premise_pattern, &local_subst);
                premise_goal = state.arena.apply_meta_subst(premise_goal, state.global_subst);
                // Beta-normalize premise goals to reduce any beta-redexes
                // created by meta substitution (e.g., (?B ?e2) -> ((lx.T) e2) -> T[e2/x])
                premise_goal = state.arena.beta_normalize(premise_goal, &mut fuel);

                // Bidirectional: infer conclusion and unify when metas remain
                if state.arena.has_metas(premise_goal) {
                    if let Some(inferred) = infer_conclusion_h(
                        state.arena,
                        state.rule_cache,
                        premise_derivation,
                        assumptions,
                        state.global_subst,
                        state.fresh_counter,
                    ) {
                        if let Some(s) = unify_h(state.arena, premise_goal, inferred) {
                            for (k, v) in &s {
                                local_subst.insert(k.clone(), *v);
                                state.global_subst.insert(k.clone(), *v);
                            }
                            premise_goal = state.arena.apply_meta_subst(premise_goal, &s);
                            premise_goal = state.arena.beta_normalize(premise_goal, &mut fuel);
                        }
                    }
                }

                // Build extended assumptions if this premise has context extensions
                let ext_assumptions: Vec<HExpr>;
                let premise_assumptions = {
                    let extensions: Vec<HExpr> = rule.context_extensions.iter()
                        .filter(|(idx, _)| *idx == i)
                        .map(|(_, h)| {
                            let resolved = state.arena.apply_meta_subst(*h, &local_subst);
                            let resolved = state.arena.apply_meta_subst(resolved, state.global_subst);
                            state.arena.whnf(resolved)
                        })
                        .collect();
                    if extensions.is_empty() {
                        assumptions
                    } else {
                        ext_assumptions = assumptions.iter().copied()
                            .chain(extensions)
                            .collect();
                        &ext_assumptions
                    }
                };

                check_inner(state, premise_goal, premise_derivation, premise_assumptions)
                    .map_err(|e| {
                        OmegaError::PremiseCheckFailed {
                            rule: rule_name.to_string(),
                            premise: i,
                            cause: Box::new(e),
                        }
                    })?;

                for (k, v) in state.global_subst.iter() {
                    if !local_subst.contains_key(k) {
                        local_subst.insert(k.clone(), *v);
                    }
                }
            }

            // Validate linear/affine binder usage in the goal
            if !state.arena.linear_binders.is_empty() || !state.arena.affine_binders.is_empty() {
                let resolved_goal = apply_fixpoint(state.arena, goal, state.global_subst);
                state.arena.validate_binder_usage(resolved_goal).map_err(|msg| {
                    OmegaError::BinderUsageViolation {
                        rule: rule_name.to_string(),
                        detail: msg,
                    }
                })?;
            }

            for (k, v) in local_subst {
                state.global_subst.insert(k, v);
            }

            Ok(())
        }
    }
}

fn infer_conclusion_h(
    arena: &mut Arena,
    rule_cache: &HashMap<String, InternedRule>,
    derivation: &Derivation,
    assumptions: &[HExpr],
    subst: &HSubst,
    fresh_counter: &mut usize,
) -> Option<HExpr> {
    match derivation {
        Derivation::Assumption => None,
        Derivation::AssumptionIdx(idx) => {
            assumptions
                .get(*idx)
                .map(|&a| arena.apply_meta_subst(a, subst))
        }
        Derivation::RuleApp {
            rule_name,
            premises,
        } => {
            let orig_rule = rule_cache.get(rule_name)?;
            // Freshen to avoid meta-variable collisions with the goal's metas
            let rule = freshen_interned_rule(arena, orig_rule, fresh_counter);
            let mut rule_subst = HSubst::new();
            for (premise_deriv, &premise_pattern) in
                premises.iter().zip(rule.premises.iter())
            {
                if let Some(inferred) =
                    infer_conclusion_h(arena, rule_cache, premise_deriv, assumptions, subst, fresh_counter)
                {
                    if let Ok(s) = arena.match_expr(premise_pattern, inferred) {
                        for (k, v) in s {
                            rule_subst.insert(k, v);
                        }
                    }
                }
            }
            Some(arena.apply_meta_subst(rule.conclusion, &rule_subst))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::derivation::{Context, Derivation};
    use crate::expr::Expr;
    use crate::test_util::make_prop_logic;

    #[test]
    fn interned_and_intro() {
        let theory = make_prop_logic();
        let goal = Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::free("p"), Expr::free("q")]),
        ]);
        let derivation = Derivation::RuleApp {
            rule_name: "and-intro".to_string(),
            premises: vec![Derivation::Assumption, Derivation::Assumption],
        };
        let ctx = Context::with_assumptions(vec![
            Expr::app(vec![Expr::sym("proves"), Expr::free("p")]),
            Expr::app(vec![Expr::sym("proves"), Expr::free("q")]),
        ]);

        assert!(check_derivation_interned(&theory, &goal, &derivation, &ctx).is_ok());
    }

    #[test]
    fn interned_and_comm() {
        let theory = make_prop_logic();
        let goal = Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::free("q"), Expr::free("p")]),
        ]);
        let ctx = Context::with_assumptions(vec![Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::free("p"), Expr::free("q")]),
        ])]);
        let derivation = Derivation::RuleApp {
            rule_name: "and-intro".to_string(),
            premises: vec![
                Derivation::RuleApp {
                    rule_name: "and-elim-r".to_string(),
                    premises: vec![Derivation::Assumption],
                },
                Derivation::RuleApp {
                    rule_name: "and-elim-l".to_string(),
                    premises: vec![Derivation::Assumption],
                },
            ],
        };

        assert!(check_derivation_interned(&theory, &goal, &derivation, &ctx).is_ok());
    }

    #[test]
    fn interned_reject_invalid() {
        let theory = make_prop_logic();
        let goal = Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::free("p"), Expr::free("q")]),
        ]);
        // Wrong number of premises
        let derivation = Derivation::RuleApp {
            rule_name: "and-intro".to_string(),
            premises: vec![Derivation::Assumption],
        };
        let ctx = Context::with_assumptions(vec![
            Expr::app(vec![Expr::sym("proves"), Expr::free("p")]),
        ]);

        assert!(check_derivation_interned(&theory, &goal, &derivation, &ctx).is_err());
    }
}
