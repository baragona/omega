//! Equality saturation via graph physics.
//!
//! This module replaces the `egg` crate's equality saturation with Archon's
//! native interaction net physics + superposition particles. **No S-expression
//! manipulation** — all semantic operations happen at the graph level.
//!
//! ## Architecture
//!
//! 1. **Implant** both LHS and RHS into an Archon arena as interaction net graphs
//! 2. **Run physics** — Apeiron's interaction rules handle beta reduction, eta
//!    contraction, dup/erase propagation natively via pointer rewiring
//! 3. **Apply compiled graph rewrite rules** via `apeiron::rewrite::try_rewrite_scan`
//!    interleaved with physics until fixpoint
//! 4. **Compare** normal forms via topological hash
//! 5. If laws exist: **superposition-based saturation** creates quantum alternatives
//!    and propagates congruence closure through the graph
//! 6. **Check e-class connectivity** between LHS and RHS roots

use std::collections::{BTreeSet, HashMap, HashSet};

use apeiron::node::{OpCode, Ptr};
use apeiron::rewrite;

use crate::extended_arena::ArchonArena;
use crate::implant::Sexp;
use crate::physics;
use crate::superposition;

// ── Public types ───────────────────────────────────────────────────────

/// Result of a saturation-based equality check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SatResult {
    Equal,
    NotEqual,
    Timeout,
}

/// Fuel limits for the saturation engine.
#[derive(Debug, Clone, Copy)]
pub struct SatFuel {
    /// Maximum saturation iterations (rule application rounds).
    pub max_iterations: usize,
    /// Maximum nodes before stopping.
    pub max_nodes: usize,
    /// Maximum physics interactions for directed reduction.
    pub max_interactions: u64,
    /// Whether to perform eta reduction during normalization.
    pub enable_eta: bool,
}

impl Default for SatFuel {
    fn default() -> Self {
        SatFuel {
            max_iterations: 30,
            max_nodes: 10_000,
            max_interactions: 100_000,
            enable_eta: true,
        }
    }
}

/// A rewrite rule for the saturation engine.
#[derive(Debug, Clone)]
pub struct SatRule {
    pub name: String,
    pub lhs: Sexp,
    pub rhs: Sexp,
    /// If true, apply in both directions (law). If false, only LHS → RHS (rule).
    pub bidirectional: bool,
}

// ── Core equality check ────────────────────────────────────────────────

/// Check if two S-expressions are equal under the given rewrite rules.
///
/// All reduction happens at the graph level:
/// 1. Implant both sides, run Apeiron physics (beta/eta via interaction net rewiring)
/// 2. Apply compiled graph rewrite rules via `try_rewrite_scan` until fixpoint
/// 3. Compare normal forms via topological hash
/// 4. If laws exist, run superposition saturation on the graph
pub fn check_equal(
    lhs: &Sexp,
    rhs: &Sexp,
    rules: &[SatRule],
    fuel: SatFuel,
) -> SatResult {
    // Fast path: structural Sexp equality (avoids building a graph).
    if lhs == rhs {
        return SatResult::Equal;
    }

    let mut ops = extract_all_ops(rules);
    // Also extract ops from the expressions themselves (operators may not appear in rules).
    extract_ops_from_sexp(lhs, &mut ops);
    extract_ops_from_sexp(rhs, &mut ops);

    // Compile directed rules into Apeiron GraphRules for graph-level rewriting.
    let graph_rules = compile_directed_rules(rules);

    // Phase 1: Reduce both sides via graph physics + directed rewrite rules.
    let lhs_nf = reduce_via_graph(lhs, &graph_rules, &ops, fuel.max_interactions, fuel.enable_eta);
    let rhs_nf = reduce_via_graph(rhs, &graph_rules, &ops, fuel.max_interactions, fuel.enable_eta);

    if lhs_nf == rhs_nf {
        return SatResult::Equal;
    }

    // Phase 2: Physical equality saturation via graph-level superposition.
    // Implant both normal forms into one arena, run law application +
    // physics + congruence closure until the roots merge or fuel runs out.
    let has_laws = rules.iter().any(|r| r.bidirectional);
    if !has_laws {
        return SatResult::NotEqual;
    }

    let laws: Vec<&SatRule> = rules.iter().filter(|r| r.bidirectional).collect();
    physical_equality_saturation(&lhs_nf, &rhs_nf, &laws, &ops, &fuel)
}

/// Physical equality saturation: implant both terms into one arena,
/// apply laws via graph-level pattern matching + superposition,
/// propagate congruence closure, and check if roots merge.
fn physical_equality_saturation(
    lhs: &Sexp,
    rhs: &Sexp,
    laws: &[&SatRule],
    ops: &HashMap<String, u8>,
    fuel: &SatFuel,
) -> SatResult {
    let mut arena = ArchonArena::new();

    // Implant both terms into the same arena.
    let lhs_ap = to_apeiron_sexp(lhs);
    let rhs_ap = to_apeiron_sexp(rhs);
    let mut env = apeiron::builder::BuildEnv::new();
    env.known_ops = ops.keys().cloned().collect();
    for (name, arity) in ops {
        env.op_arities.insert(name.clone(), *arity);
    }
    let lhs_root = apeiron::builder::build_rooted(&mut arena.inner, &mut env, &lhs_ap);
    let rhs_root = apeiron::builder::build_rooted(&mut arena.inner, &mut env, &rhs_ap);

    // The actual term roots are on aux port 1 of the ROOT anchors.
    let lhs_term_port = arena.inner.port(lhs_root, 1);
    let rhs_term_port = arena.inner.port(rhs_root, 1);
    if !lhs_term_port.is_connected() || !rhs_term_port.is_connected() {
        return SatResult::NotEqual;
    }
    let lhs_term = lhs_term_port.target;
    let rhs_term = rhs_term_port.target;

    // Build initial parent index for all nodes in the arena.
    build_parent_index(&mut arena);

    // Saturation loop: apply laws, run physics, propagate congruence.
    for _round in 0..fuel.max_iterations {
        // Check if LHS and RHS are in the same e-class (via union-find).
        if arena.uf_same(lhs_term.0, rhs_term.0) {
            return SatResult::Equal;
        }

        // Apply all laws one round (graph-level pattern matching + superposition).
        let new_supers = apply_laws_one_round_saturating(&mut arena, laws, ops);
        // Round completed.

        // Register newly created nodes in spatial index (merges duplicate atoms).
        register_new_nodes_in_spatial_index(&mut arena);

        // Propagate congruence closure (the shockwave cascade).
        let congruence_merges = superposition::propagate_congruence(&mut arena);

        // Run a short burst of physics (beta, dup/erase propagation).
        let config = physics::ArchonConfig {
            max_interactions: 1000,
            trace: false,
            radiation_hops_per_tick: 1,
        };
        let phys = physics::run(&mut arena, &config);

        // If nothing happened, we've saturated — check one final time.
        if new_supers == 0 && congruence_merges == 0 && phys.interactions == 0 {
            break;
        }

        // Node count guard.
        if arena.node_count() > fuel.max_nodes {
            break;
        }
    }

    // Final check.
    // Final check via union-find.
    if arena.uf_same(lhs_term.0, rhs_term.0) {
        return SatResult::Equal;
    }

    SatResult::NotEqual
}

/// Apply all bidirectional laws one round, WITHOUT the polarization filter.
/// In equality saturation, we must explore both directions unconditionally —
/// the congruence cascade handles convergence, not hash ordering.
///
/// Deduplication: uses a set of (matched_readback, law_index, direction) to
/// avoid re-applying the same law to structurally identical subgraphs.
fn apply_laws_one_round_saturating(
    arena: &mut ArchonArena,
    laws: &[&SatRule],
    ops: &HashMap<String, u8>,
) -> usize {
    let mut new_supers = 0;
    let capacity = arena.inner.node_capacity();
    // Track which (binding_e-class_roots, law_name, direction) combos we've already done.
    // Using binding e-class roots instead of matched node readback means that
    // two e-class members with the same bindings (up to UF) won't both get law applied.
    let mut applied: HashSet<(Vec<u32>, usize, bool)> = HashSet::new();

    // Match against all non-super nodes. Using BTreeSet for deterministic iteration.
    let existing_nodes: BTreeSet<u32> = (0..capacity)
        .filter(|&idx| {
            let ptr = Ptr(idx as u32);
            arena.get(ptr).is_some()
                && !superposition::is_superposition(arena, ptr)
        })
        .map(|idx| idx as u32)
        .collect();

    for (law_idx, law) in laws.iter().enumerate() {
        // Forward: match LHS, materialize RHS, superpose.
        let matches_fwd = find_pattern_matches_filtered(arena, &law.lhs, &existing_nodes);
        for (matched_root, bindings) in &matches_fwd {
            // Dedup by binding e-class roots: if another match bound the same
            // variables to the same e-classes, the materialized result would be
            // structurally identical — skip it.
            let binding_key = binding_eclass_key(arena, bindings);
            if !applied.insert((binding_key, law_idx, true)) {
                continue;
            }

            if let Some(mat_root) = materialize_with_bindings(arena, &law.rhs, bindings, ops, 0) {
                if try_superpose_new(arena, *matched_root, mat_root) {
                    new_supers += 1;
                }
            }
        }

        // Reverse: match RHS, materialize LHS, superpose.
        let matches_rev = find_pattern_matches_filtered(arena, &law.rhs, &existing_nodes);
        for (matched_root, bindings) in &matches_rev {
            let binding_key = binding_eclass_key(arena, bindings);
            if !applied.insert((binding_key, law_idx, false)) {
                continue;
            }

            if let Some(mat_root) = materialize_with_bindings(arena, &law.lhs, bindings, ops, 0) {
                if try_superpose_new(arena, *matched_root, mat_root) {
                    new_supers += 1;
                }
            }
        }
    }

    new_supers
}

/// Build the parent index for all existing nodes in the arena.
fn build_parent_index(arena: &mut ArchonArena) {
    let capacity = arena.inner.node_capacity();
    for idx in 0..capacity {
        let ptr = Ptr(idx as u32);
        let node = match arena.inner.get(ptr) {
            Some(n) => n,
            None => continue,
        };
        let port_count = node.kind.port_count();
        for slot in 1..port_count {
            let port = arena.inner.port(ptr, slot as u8);
            if port.is_connected() {
                arena.parent_index.entry(port.target.0).or_default().insert(ptr.0);
            }
        }
    }
}

/// Register all nodes in the spatial index with fresh signatures (clearing stale
/// entries first), merge collisions, and propagate congruence. Processes bottom-up
/// by arity so atom merges update UF before parent signatures are computed.
fn register_new_nodes_in_spatial_index(arena: &mut ArchonArena) {
    // Clear stale spatial index — UF changes invalidate old signatures.
    arena.spatial_index.clear();

    let capacity = arena.inner.node_capacity();
    let mut by_arity: HashMap<u8, Vec<Ptr>> = HashMap::new();
    for idx in 0..capacity {
        let ptr = Ptr(idx as u32);
        if let Some(node) = arena.inner.get(ptr) {
            if !superposition::is_superposition(arena, ptr) {
                let arity = match &node.kind {
                    OpCode::Sym { arity, .. } => *arity,
                    _ => 0,
                };
                by_arity.entry(arity).or_default().push(ptr);
            }
        }
    }

    let mut arities: Vec<u8> = by_arity.keys().cloned().collect();
    arities.sort();

    for arity in arities {
        let ptrs = match by_arity.get(&arity) {
            Some(v) => v.clone(),
            None => continue,
        };

        for ptr in ptrs {
            if arena.get(ptr).is_none() {
                continue;
            }
            if let Some(existing) = arena.register_in_spatial_index(ptr) {
                if arena.uf_same(existing.0, ptr.0) {
                    continue;
                }
                if arena.get(existing).is_none() {
                    continue;
                }
                let region = arena.region_of(existing);
                let sup = superposition::superpose(arena, existing, 0, ptr, 0, region);
                arena.uf_union(existing.0, ptr.0);
                arena.uf_union(existing.0, sup.0);
                arena.record_parent(existing, sup);
                arena.record_parent(ptr, sup);
                arena.shockwave_queue.push(sup);
            }
        }
        // After each arity layer, propagate congruence so UF is up-to-date
        // for the next layer's signature computations.
        superposition::propagate_congruence(arena);
    }
}

/// Update parent index and spatial index after creating a superposition.
fn update_indices_after_superpose(
    arena: &mut ArchonArena,
    sup: Ptr,
    original: Ptr,
    new_alt: Ptr,
) {
    // The superposition node is now a parent of both children.
    arena.record_parent(original, sup);
    arena.record_parent(new_alt, sup);

    // Record parent relationships for the materialized subtree.
    record_subtree_parents(arena, new_alt);
}

/// Recursively record parent relationships for a newly materialized subtree.
fn record_subtree_parents(arena: &mut ArchonArena, root: Ptr) {
    let node = match arena.inner.get(root) {
        Some(n) => n,
        None => return,
    };
    let port_count = node.kind.port_count();
    let mut children = Vec::new();
    for slot in 1..port_count {
        let port = arena.inner.port(root, slot as u8);
        if port.is_connected() {
            arena.parent_index.entry(port.target.0).or_default().insert(root.0);
            children.push(port.target);
        }
    }
    for child in children {
        record_subtree_parents(arena, child);
    }
}

/// Simplify an expression by reducing to normal form via graph physics.
pub fn extract_simplest(
    expr: &Sexp,
    rules: &[SatRule],
    fuel: SatFuel,
) -> Sexp {
    let mut ops = extract_all_ops(rules);
    extract_ops_from_sexp(expr, &mut ops);
    let graph_rules = compile_directed_rules(rules);
    reduce_via_graph(expr, &graph_rules, &ops, fuel.max_interactions, fuel.enable_eta)
}

/// Extract near-miss diagnostics: reduce both sides and return their normal forms.
pub fn extract_near_miss(
    lhs: &Sexp,
    rhs: &Sexp,
    rules: &[SatRule],
    fuel: SatFuel,
) -> (Sexp, Sexp) {
    let mut ops = extract_all_ops(rules);
    extract_ops_from_sexp(lhs, &mut ops);
    extract_ops_from_sexp(rhs, &mut ops);
    let graph_rules = compile_directed_rules(rules);
    let lhs_nf = reduce_via_graph(lhs, &graph_rules, &ops, fuel.max_interactions, fuel.enable_eta);
    let rhs_nf = reduce_via_graph(rhs, &graph_rules, &ops, fuel.max_interactions, fuel.enable_eta);
    (lhs_nf, rhs_nf)
}

// ── Graph-level reduction ──────────────────────────────────────────────

/// Compile directed rules (non-bidirectional) into Apeiron GraphRules.
fn compile_directed_rules(rules: &[SatRule]) -> Vec<rewrite::GraphRule> {
    rules.iter()
        .filter(|r| !r.bidirectional)
        .filter_map(|r| {
            let lhs_ap = to_apeiron_sexp(&r.lhs);
            let rhs_ap = to_apeiron_sexp(&r.rhs);
            rewrite::compile_rule(&r.name, &lhs_ap, &rhs_ap)
        })
        .collect()
}

/// Reduce an S-expression to normal form via graph physics.
///
/// This is the core: implant the term as an interaction net, run Apeiron's
/// physics engine (which handles beta reduction, dup/erase propagation natively
/// via pointer rewiring), then apply compiled graph rewrite rules, and readback.
fn reduce_via_graph(
    expr: &Sexp,
    graph_rules: &[rewrite::GraphRule],
    ops: &HashMap<String, u8>,
    max_interactions: u64,
    enable_eta: bool,
) -> Sexp {
    let mut arena = ArchonArena::new();

    // Use Apeiron's builder directly — it creates a ROOT anchor and properly
    // wires the term's output port (e.g., App slot 2) to ROOT slot 1,
    // without disturbing principal-port active pairs.
    let ap_sexp = to_apeiron_sexp(expr);
    let mut env = apeiron::builder::BuildEnv::new();
    env.known_ops = ops.keys().cloned().collect();
    // Set arities for proper curried application
    for (name, arity) in ops {
        env.op_arities.insert(name.clone(), *arity);
    }
    let root = apeiron::builder::build_rooted(&mut arena.inner, &mut env, &ap_sexp);

    // Run the physics + rewrite loop on the Archon arena.
    run_physics_rewrite_loop(&mut arena, graph_rules, max_interactions, enable_eta);

    // Readback from the ROOT anchor's aux port (slot 1), skipping the ROOT wrapper.
    let result_port = arena.inner.port(root, 1);
    if result_port.is_connected() {
        let term = apeiron::readback::readback(&arena.inner, result_port.target);
        let result_sexp = rewrite::term_to_sexp(&term);
        from_apeiron_sexp(&result_sexp)
    } else {
        Sexp::atom("_")
    }
}

/// Run the physics + graph rewrite interleaving loop until fixpoint.
///
/// 1. Run Apeiron physics (beta/eta/dup/erase — all via pointer rewiring)
/// 2. Scan for graph rewrite matches and apply one
/// 3. If either made progress, loop back to step 1
fn run_physics_rewrite_loop(
    arena: &mut ArchonArena,
    graph_rules: &[rewrite::GraphRule],
    max_interactions: u64,
    enable_eta: bool,
) {
    let mut total_interactions = 0u64;

    loop {
        if total_interactions >= max_interactions {
            break;
        }

        // Run Apeiron physics on the inner arena.
        let config = physics::ArchonConfig {
            max_interactions: max_interactions.saturating_sub(total_interactions),
            trace: false,
            radiation_hops_per_tick: 1,
        };
        let result = physics::run(arena, &config);
        total_interactions += result.interactions;
        let physics_did_work = result.interactions > 0;

        // Apply one graph rewrite rule.
        let rewrite_fired = if !graph_rules.is_empty() {
            rewrite::try_rewrite_scan(&mut arena.inner, graph_rules)
        } else {
            false
        };

        // Eta reduction scan (only when enabled, e.g., topological-homotopy).
        let eta_fired = if enable_eta {
            try_eta_scan(&mut arena.inner)
        } else {
            false
        };

        // If nothing made progress, we've hit fixpoint.
        if !physics_did_work && !rewrite_fired && !eta_fired {
            break;
        }
    }
}

/// Eta reduction scan: find λx.(f x) patterns and annihilate.
///
/// Two patterns:
/// 1. App-based: Lam.var(1)↔App.arg(1), Lam.body(2)↔App.result(2)
///    → connect Lam.principal_peer ↔ App.function_peer, free both
/// 2. Sym-based: Lam.var(1)↔Sym.last_aux(N), Lam.body(2)↔Sym.principal(0)
///    where Sym has arity N and the var is its last argument
///    → This is NOT a simple eta (Sym applies f to multiple args), skip for now
///    Instead: Lam.body(2)↔Sym.principal(0) means the body IS the Sym node.
///    If the Sym node's ONLY aux port connects to the Lam's var, eta-reduce.
fn try_eta_scan(arena: &mut apeiron::arena::Arena) -> bool {
    use apeiron::node::OpCode;

    let mut fired = false;
    let capacity = arena.node_capacity();

    for idx in 0..capacity {
        let ptr = apeiron::node::Ptr(idx as u32);
        let node = match arena.get(ptr) {
            Some(n) => n.clone(),
            None => continue,
        };
        if !matches!(node.kind, OpCode::Lam) {
            continue;
        }

        let var_port = node.ports[1]; // Lam.var
        let body_port = node.ports[2]; // Lam.body

        if !var_port.is_connected() || !body_port.is_connected() {
            continue;
        }

        let body_target = body_port.target;
        let body_node = match arena.get(body_target) {
            Some(n) => n.clone(),
            None => continue,
        };

        // Pattern 1: App-based eta — λx. (app f x)
        // Lam.var(1)↔App.arg(1), Lam.body(2)↔App.result(2)
        if matches!(body_node.kind, OpCode::App)
            && var_port.target == body_target
            && var_port.slot == 1
            && body_port.slot == 2
        {
            let lam_principal = node.ports[0];
            let app_function = body_node.ports[0];

            if lam_principal.is_connected() && app_function.is_connected() {
                arena.connect(
                    lam_principal.target, lam_principal.slot,
                    app_function.target, app_function.slot,
                );
            }
            arena.free(ptr);
            arena.free(body_target);
            fired = true;
            continue;
        }

        // Pattern 2: Sym-based eta — λx. (f x) where f is Sym with arity 1
        // Lam.body(2)↔Sym.principal(0), Lam.var(1)↔Sym.aux(1)
        if let OpCode::Sym { arity, name: sym_name, .. } = &body_node.kind {
            if *arity == 1
                && body_port.slot == 0  // body connects to Sym's principal
                && var_port.target == body_target
                && var_port.slot == 1   // var connects to Sym's only aux port
            {
                // η: λx.(f x) → f. The Sym IS f applied to x.
                // After eta, the Sym stays but loses its argument.
                // We need to replace the Sym(arity=1) with Sym(arity=0)
                // and connect it where the Lam was.
                let lam_principal = node.ports[0];
                let sym_name_owned = sym_name.clone();
                if lam_principal.is_connected() {
                    // Spawn a new arity-0 Sym for the bare name `f`
                    let bare = arena.spawn(OpCode::Sym {
                        name: sym_name_owned,
                        arity: 0,
                    });
                    arena.connect(
                        lam_principal.target, lam_principal.slot,
                        bare, 0,
                    );
                }
                arena.free(body_target); // free old Sym(f, 1)
                arena.free(ptr);         // free Lam
                fired = true;
                continue;
            }
        }
    }

    fired
}

/// Try to superpose a materialized node with its matched root.
/// First registers the new subtree in the spatial index (merging atoms and
/// structurally identical subterms), then checks if they're already in the
/// same e-class. Returns true if a new superposition was created.
fn try_superpose_new(
    arena: &mut ArchonArena,
    matched_root: Ptr,
    mat_root: Ptr,
) -> bool {
    // Register the materialized subtree's parents.
    record_subtree_parents(arena, mat_root);

    // Register in spatial index bottom-up to merge duplicate atoms/ops.
    // This may merge mat_root into matched_root's e-class via congruence.
    register_new_nodes_in_spatial_index(arena);

    // After registration + congruence, check if already merged.
    if arena.uf_same(matched_root.0, mat_root.0) {
        return false;
    }

    // Also check readback identity (handles cases spatial index misses).
    let match_sexp = readback_clean(arena, matched_root, 100);
    let mat_sexp = readback_clean(arena, mat_root, 100);
    if mat_sexp == match_sexp {
        arena.free(mat_root);
        return false;
    }

    let region = arena.region_of(matched_root);
    let sup = superposition::superpose(arena, matched_root, 0, mat_root, 0, region);
    arena.uf_union(matched_root.0, mat_root.0);
    arena.uf_union(matched_root.0, sup.0);
    update_indices_after_superpose(arena, sup, matched_root, mat_root);
    arena.shockwave_queue.push(sup);
    true
}

/// Compute a dedup key from pattern match bindings: sorted list of
/// (variable_name, e-class_root) pairs, flattened to a Vec<u32>.
/// Two matches with the same binding key will produce the same materialized term.
fn binding_eclass_key(arena: &ArchonArena, bindings: &HashMap<String, Ptr>) -> Vec<u32> {
    let mut pairs: Vec<(&str, u32)> = bindings
        .iter()
        .map(|(name, ptr)| (name.as_str(), arena.uf_find_immut(ptr.0)))
        .collect();
    pairs.sort_by_key(|(name, _)| *name);
    pairs.into_iter().map(|(_, root)| root).collect()
}

/// Find pattern matches, but only scan nodes in the given set.
fn find_pattern_matches_filtered(
    arena: &ArchonArena,
    pattern: &Sexp,
    allowed_nodes: &BTreeSet<u32>,
) -> Vec<(Ptr, HashMap<String, Ptr>)> {
    let mut results = Vec::new();

    for &idx in allowed_nodes {
        let ptr = Ptr(idx);
        if arena.get(ptr).is_none() {
            continue;
        }

        let mut bindings = HashMap::new();
        if match_pattern(arena, ptr, pattern, &mut bindings) {
            results.push((ptr, bindings));
        }
    }

    results
}

/// Read back a node to Sexp, transparently stepping through superposition nodes
/// (picking child 1 = original). This gives a stable Sexp for dedup purposes.
fn readback_clean(arena: &ArchonArena, ptr: Ptr, fuel: usize) -> Sexp {
    if fuel == 0 {
        return Sexp::Atom("…".into());
    }
    // Transparently follow through superposition nodes.
    let resolved = resolve_through_super(arena, ptr);
    let node = match arena.get(resolved) {
        Some(n) => n,
        None => return Sexp::Atom("⊥".into()),
    };
    match &node.kind {
        OpCode::Sym { name, arity } => {
            if *arity == 0 {
                Sexp::Atom(name.clone())
            } else {
                let mut items = vec![Sexp::Atom(name.clone())];
                for i in 1..=*arity {
                    let port = arena.port(resolved, i);
                    if port.is_connected() {
                        items.push(readback_clean(arena, port.target, fuel - 1));
                    }
                }
                Sexp::List(items)
            }
        }
        _ => Sexp::Atom(format!("{:?}", node.kind)),
    }
}

/// Resolve a pointer through superposition nodes, picking child 1 (the original).
fn resolve_through_super(arena: &ArchonArena, mut ptr: Ptr) -> Ptr {
    for _ in 0..20 {
        if !superposition::is_superposition(arena, ptr) {
            return ptr;
        }
        let port = arena.port(ptr, 1);
        if !port.is_connected() {
            return ptr;
        }
        ptr = port.target;
    }
    ptr
}

/// Try to match a pattern S-expression against a subgraph rooted at `ptr`.
fn match_pattern(
    arena: &ArchonArena,
    ptr: Ptr,
    pattern: &Sexp,
    bindings: &mut HashMap<String, Ptr>,
) -> bool {
    let node = match arena.get(ptr) {
        Some(n) => n,
        None => return false,
    };

    match pattern {
        Sexp::Atom(name) => {
            if name.starts_with('?') {
                if let Some(&bound) = bindings.get(name.as_str()) {
                    bound == ptr
                } else {
                    bindings.insert(name.clone(), ptr);
                    true
                }
            } else {
                matches!(&node.kind, OpCode::Sym { name: n, arity: 0 } if n == name)
            }
        }
        Sexp::List(items) => {
            if items.is_empty() {
                return false;
            }
            let head = match &items[0] {
                Sexp::Atom(name) => name.as_str(),
                _ => return false,
            };

            let (node_name, node_arity) = match &node.kind {
                OpCode::Sym { name, arity } => (name.as_str(), *arity as usize),
                OpCode::App => ("app", 2),
                OpCode::Lam => ("lam", 2),
                _ => return false,
            };

            if head != node_name {
                return false;
            }

            let args = &items[1..];
            if args.len() != node_arity {
                return false;
            }

            for (i, arg_pat) in args.iter().enumerate() {
                let port = arena.port(ptr, (i + 1) as u8);
                if !port.is_connected() {
                    return false;
                }
                if !match_pattern(arena, port.target, arg_pat, bindings) {
                    return false;
                }
            }

            true
        }
    }
}

/// Materialize an S-expression template with meta-variable bindings.
///
/// When a meta-variable is bound to an existing node, we create a COPY of the
/// subgraph rooted at that node (interaction nets require single-occupancy ports).
/// The copied atoms will be merged with originals via the spatial index later.
fn materialize_with_bindings(
    arena: &mut ArchonArena,
    template: &Sexp,
    bindings: &HashMap<String, Ptr>,
    ops: &HashMap<String, u8>,
    region: u32,
) -> Option<Ptr> {
    match template {
        Sexp::Atom(name) => {
            if name.starts_with('?') {
                if let Some(&bound_ptr) = bindings.get(name.as_str()) {
                    // Clone the bound subgraph (can't share ports in an interaction net).
                    Some(clone_subgraph(arena, bound_ptr, ops, region))
                } else {
                    None
                }
            } else {
                let arity = ops.get(name.as_str()).copied().unwrap_or(0);
                Some(arena.spawn_in(
                    OpCode::Sym { name: name.clone(), arity },
                    region,
                ))
            }
        }
        Sexp::List(items) => {
            if items.is_empty() {
                return None;
            }
            let head = match &items[0] {
                Sexp::Atom(name) => name.clone(),
                _ => return None,
            };
            let args = &items[1..];
            let arity = args.len() as u8;
            let node_arity = ops.get(head.as_str()).copied().unwrap_or(arity);
            let node = arena.spawn_in(
                OpCode::Sym { name: head, arity: node_arity },
                region,
            );

            for (i, arg) in args.iter().enumerate() {
                let child = materialize_with_bindings(arena, arg, bindings, ops, region)?;
                arena.connect(node, (i + 1) as u8, child, 0);
            }

            Some(node)
        }
    }
}

/// Clone a subgraph rooted at `ptr`, creating fresh nodes with the same structure.
fn clone_subgraph(arena: &mut ArchonArena, ptr: Ptr, ops: &HashMap<String, u8>, region: u32) -> Ptr {
    let node = match arena.get(ptr) {
        Some(n) => n,
        None => return arena.spawn_in(OpCode::Sym { name: "_dead".into(), arity: 0 }, region),
    };
    let kind = node.kind.clone();
    let port_count = kind.port_count();
    let new_node = arena.spawn_in(kind, region);

    // Recursively clone children (aux ports, skip principal port 0).
    for slot in 1..port_count {
        let port = arena.port(ptr, slot as u8);
        if port.is_connected() {
            // Skip if child is a Superposition (don't clone e-class hubs).
            if superposition::is_superposition(arena, port.target) {
                // Connect to the superposition directly (shared).
                // This is safe because we connect to a non-principal port.
                continue;
            }
            let child_clone = clone_subgraph(arena, port.target, ops, region);
            arena.connect(new_node, slot as u8, child_clone, 0);
        }
    }

    new_node
}

// ── Helpers ────────────────────────────────────────────────────────────

/// Convert an Apeiron Sexp to Archon's implant Sexp.
pub fn from_apeiron_sexp(sexp: &apeiron::parser::Sexp) -> Sexp {
    match sexp {
        apeiron::parser::Sexp::Atom(s, _) => Sexp::atom(s),
        apeiron::parser::Sexp::List(items, _) => {
            Sexp::list(items.iter().map(from_apeiron_sexp).collect())
        }
    }
}

/// Convert Archon's implant Sexp to an Apeiron Sexp.
pub fn to_apeiron_sexp(sexp: &Sexp) -> apeiron::parser::Sexp {
    let sp = apeiron::parser::Span::default();
    match sexp {
        Sexp::Atom(s) => apeiron::parser::Sexp::Atom(s.clone(), sp),
        Sexp::List(items) => {
            apeiron::parser::Sexp::List(items.iter().map(to_apeiron_sexp).collect(), sp)
        }
    }
}

/// Extract operator arities from rules.
fn extract_all_ops(rules: &[SatRule]) -> HashMap<String, u8> {
    let mut ops = HashMap::new();
    for rule in rules {
        extract_ops_from_sexp(&rule.lhs, &mut ops);
        extract_ops_from_sexp(&rule.rhs, &mut ops);
    }
    ops
}

fn extract_ops_from_sexp(sexp: &Sexp, ops: &mut HashMap<String, u8>) {
    match sexp {
        Sexp::Atom(_) => {}
        Sexp::List(items) => {
            if let Some(Sexp::Atom(head)) = items.first() {
                if !head.starts_with('?') {
                    let arity = (items.len() - 1) as u8;
                    ops.entry(head.clone()).or_insert(arity);
                }
            }
            for item in items {
                extract_ops_from_sexp(item, ops);
            }
        }
    }
}

/// Filter rules where barrier operators appear asymmetrically.
pub fn filter_barrier_rules(rules: &[SatRule], barrier_ops: &[String]) -> Vec<SatRule> {
    if barrier_ops.is_empty() {
        return rules.to_vec();
    }
    rules
        .iter()
        .filter(|rule| {
            barrier_ops.iter().all(|op| {
                sexp_contains_atom(&rule.lhs, op) == sexp_contains_atom(&rule.rhs, op)
            })
        })
        .cloned()
        .collect()
}

fn sexp_contains_atom(sexp: &Sexp, name: &str) -> bool {
    match sexp {
        Sexp::Atom(s) => s == name,
        Sexp::List(items) => items.iter().any(|item| sexp_contains_atom(item, name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(s: &str) -> Sexp {
        Sexp::atom(s)
    }

    fn list(items: Vec<Sexp>) -> Sexp {
        Sexp::list(items)
    }

    fn directed(name: &str, lhs: Sexp, rhs: Sexp) -> SatRule {
        SatRule { name: name.into(), lhs, rhs, bidirectional: false }
    }

    fn law(name: &str, lhs: Sexp, rhs: Sexp) -> SatRule {
        SatRule { name: name.into(), lhs, rhs, bidirectional: true }
    }

    #[test]
    fn structural_equality() {
        let result = check_equal(&atom("a"), &atom("a"), &[], SatFuel::default());
        assert_eq!(result, SatResult::Equal);
    }

    #[test]
    fn structural_inequality() {
        let result = check_equal(&atom("a"), &atom("b"), &[], SatFuel::default());
        assert_eq!(result, SatResult::NotEqual);
    }

    #[test]
    fn directed_reduction() {
        // add(z, x) → x
        let rules = vec![directed(
            "add-z",
            list(vec![atom("add"), atom("z"), atom("?x")]),
            atom("?x"),
        )];
        let lhs = list(vec![atom("add"), atom("z"), list(vec![atom("s"), atom("z")])]);
        let rhs = list(vec![atom("s"), atom("z")]);

        let result = check_equal(&lhs, &rhs, &rules, SatFuel::default());
        assert_eq!(result, SatResult::Equal);
    }

    #[test]
    fn bidirectional_law() {
        // f(a, b) ≡ f(b, a)  (commutativity)
        let rules = vec![law(
            "comm",
            list(vec![atom("f"), atom("?x"), atom("?y")]),
            list(vec![atom("f"), atom("?y"), atom("?x")]),
        )];
        let lhs = list(vec![atom("f"), atom("a"), atom("b")]);
        let rhs = list(vec![atom("f"), atom("b"), atom("a")]);

        let result = check_equal(&lhs, &rhs, &rules, SatFuel::default());
        assert_eq!(result, SatResult::Equal);
    }

    #[test]
    fn transitivity_via_reduction() {
        // g(a) → b, b → c
        let rules = vec![
            directed("r1", list(vec![atom("g"), atom("a")]), atom("b")),
            directed("r2", atom("b"), atom("c")),
        ];
        let lhs = list(vec![atom("g"), atom("a")]);
        let rhs = atom("c");

        let result = check_equal(&lhs, &rhs, &rules, SatFuel::default());
        assert_eq!(result, SatResult::Equal);
    }

    #[test]
    fn associativity_law() {
        // (op (op a b) c) ≡ (op a (op b c)) via associativity
        let rules = vec![law(
            "assoc",
            list(vec![atom("op"), list(vec![atom("op"), atom("?x"), atom("?y")]), atom("?z")]),
            list(vec![atom("op"), atom("?x"), list(vec![atom("op"), atom("?y"), atom("?z")])]),
        )];
        let lhs = list(vec![atom("op"), list(vec![atom("op"), atom("a"), atom("b")]), atom("c")]);
        let rhs = list(vec![atom("op"), atom("a"), list(vec![atom("op"), atom("b"), atom("c")])]);

        let result = check_equal(&lhs, &rhs, &rules, SatFuel::default());
        assert_eq!(result, SatResult::Equal);
    }

    #[test]
    fn ac_comm_and_assoc() {
        let rules = vec![
            law("comm", list(vec![atom("op"), atom("?x"), atom("?y")]),
                         list(vec![atom("op"), atom("?y"), atom("?x")])),
            law("assoc", list(vec![atom("op"), list(vec![atom("op"), atom("?x"), atom("?y")]), atom("?z")]),
                          list(vec![atom("op"), atom("?x"), list(vec![atom("op"), atom("?y"), atom("?z")])])),
        ];
        // Commutativity
        let lhs = list(vec![atom("op"), atom("a"), atom("b")]);
        let rhs = list(vec![atom("op"), atom("b"), atom("a")]);
        assert_eq!(check_equal(&lhs, &rhs, &rules, SatFuel::default()), SatResult::Equal);

        // Associativity
        let lhs2 = list(vec![atom("op"), list(vec![atom("op"), atom("a"), atom("b")]), atom("c")]);
        let rhs2 = list(vec![atom("op"), atom("a"), list(vec![atom("op"), atom("b"), atom("c")])]);
        assert_eq!(check_equal(&lhs2, &rhs2, &rules, SatFuel::default()), SatResult::Equal);
    }

    #[test]
    fn beta_reduction_via_physics() {
        // [app [lam x x] a] should reduce to a via interaction net physics.
        let lhs = list(vec![
            atom("app"),
            list(vec![atom("lam"), atom("x"), atom("x")]),
            atom("a"),
        ]);
        let rhs = atom("a");

        let result = check_equal(&lhs, &rhs, &[], SatFuel::default());
        assert_eq!(result, SatResult::Equal);
    }

    #[test]
    fn eta_reduction_app() {
        // [lam x [app f x]] → f
        let lhs = list(vec![
            atom("lam"), atom("x"),
            list(vec![atom("app"), atom("f"), atom("x")]),
        ]);
        let rhs = atom("f");
        let result = check_equal(&lhs, &rhs, &[], SatFuel::default());
        assert_eq!(result, SatResult::Equal);
    }

    #[test]
    fn eta_reduction_sym() {
        // [lam x [f x]] where f has arity 1 (from rules context) → f
        let rules = vec![
            directed("r1", list(vec![atom("f"), atom("a")]), atom("b")),
        ];
        let lhs = list(vec![
            atom("lam"), atom("x"),
            list(vec![atom("f"), atom("x")]),
        ]);
        let rhs = atom("f");
        let result = check_equal(&lhs, &rhs, &rules, SatFuel::default());
        assert_eq!(result, SatResult::Equal);
    }

    #[test]
    fn filter_barrier_blocks_asymmetric() {
        let rules = vec![law(
            "collapse",
            list(vec![atom("box"), atom("?x")]),
            atom("?x"),
        )];
        let filtered = filter_barrier_rules(&rules, &["box".to_string()]);
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn filter_barrier_allows_symmetric() {
        let rules = vec![law(
            "idem",
            list(vec![atom("box"), list(vec![atom("box"), atom("?x")])]),
            list(vec![atom("box"), atom("?x")]),
        )];
        let filtered = filter_barrier_rules(&rules, &["box".to_string()]);
        assert_eq!(filtered.len(), 1);
    }
}
