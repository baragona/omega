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
    /// Skip physical equality saturation (bidirectional law application).
    /// Used for rewrite-equivalence mode where only directed reduction applies.
    pub skip_saturation: bool,
}

impl Default for SatFuel {
    fn default() -> Self {
        SatFuel {
            max_iterations: 30,
            max_nodes: 10_000,
            max_interactions: 100_000,
            enable_eta: true,
            skip_saturation: false,
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

    // Phase 1.5: AC normalization fixpoint — alternate AC-canonicalization
    // and directed reduction until stable. Identity elements are detected
    // from directed rules and stripped during flattening.
    let ac_ops = detect_ac_operators(rules);
    if !ac_ops.is_empty() {
        let ids = detect_identity_elements(rules);
        let mut lhs_cur = lhs_nf.clone();
        let mut rhs_cur = rhs_nf.clone();
        for _ in 0..5 {
            lhs_cur = ac_normalize_with_ids(&lhs_cur, &ac_ops, &ids);
            rhs_cur = ac_normalize_with_ids(&rhs_cur, &ac_ops, &ids);
            if lhs_cur == rhs_cur {
                return SatResult::Equal;
            }
            let lhs_red = reduce_via_graph(&lhs_cur, &graph_rules, &ops, fuel.max_interactions, fuel.enable_eta);
            let rhs_red = reduce_via_graph(&rhs_cur, &graph_rules, &ops, fuel.max_interactions, fuel.enable_eta);
            if lhs_red == lhs_cur && rhs_red == rhs_cur {
                break; // fixpoint
            }
            lhs_cur = lhs_red;
            rhs_cur = rhs_red;
        }
        // Final AC check after fixpoint.
        let lhs_final = ac_normalize_with_ids(&lhs_cur, &ac_ops, &ids);
        let rhs_final = ac_normalize_with_ids(&rhs_cur, &ac_ops, &ids);
        if lhs_final == rhs_final {
            return SatResult::Equal;
        }
    }

    // Phase 2: Physical equality saturation via graph-level superposition.
    // Implant both normal forms into one arena, run law application +
    // physics + congruence closure until the roots merge or fuel runs out.
    // Skip saturation for rewrite-equivalence mode (directed reduction only).
    if fuel.skip_saturation {
        return SatResult::NotEqual;
    }
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

    // Debug: dump initial graph
    {
        let cap = arena.inner.node_capacity();
        for idx in 0..cap {
            let ptr = Ptr(idx as u32);
            if let Some(node) = arena.get(ptr) {
                let kind = format!("{:?}", node.kind);
                let ports: Vec<String> = (0..node.kind.port_count())
                    .map(|s| {
                        let p = arena.inner.port(ptr, s as u8);
                        if p.is_connected() { format!("{}:{}", p.target.0, p.slot) } else { "X".into() }
                    })
                    .collect();
                eprintln!("[INIT] node={} kind={} ports=[{}]", idx, kind, ports.join(", "));
            }
        }
    }

    // Snapshot of original e-class roots — used to restrict bare-variable
    // reverse matching (expansion laws like ?p => wrapper(?p)) to only bind
    // against original terms, preventing combinatorial blowup.
    let original_eclasses: HashSet<u32> = {
        let cap = arena.inner.node_capacity();
        (0..cap)
            .filter(|&idx| {
                let ptr = Ptr(idx as u32);
                arena.get(ptr).is_some()
                    && !superposition::is_superposition(&arena, ptr)
            })
            .map(|idx| arena.uf_find_immut(idx as u32))
            .collect()
    };

    // Initial rebuild: register nodes in spatial index, merge duplicate atoms.
    rebuild_to_fixpoint(&mut arena);

    // Saturation loop: apply laws, rebuild, congruence closure, physics.
    let mut prev_node_count = arena.node_count();

    for _round in 0..fuel.max_iterations {
        // Check if LHS and RHS are in the same e-class (via union-find).
        if arena.uf_same(lhs_term.0, rhs_term.0) {
            return SatResult::Equal;
        }

        // Apply all laws one round (graph-level pattern matching + superposition).
        // Bare-variable expansion laws (like ?p => hcomp(refl, ?p)) only run in
        // the first 2 rounds to seed the search space, then are disabled to prevent
        // exponential nesting of identity wrappers.
        let new_supers = apply_laws_one_round_saturating(
            &mut arena, laws, ops, &original_eclasses, _round,
        );

        // Rebuild to fixpoint: populate spatial index, detect congruences,
        // propagate merges, repeat until stable.
        let congruence_merges = rebuild_to_fixpoint(&mut arena);

        // Count e-classes
        let cap = arena.inner.node_capacity();
        let mut eclass_roots: HashSet<u32> = HashSet::new();
        for idx in 0..cap {
            if arena.get(Ptr(idx as u32)).is_some() {
                eclass_roots.insert(arena.uf_find_immut(idx as u32));
            }
        }
        let lhs_root = arena.uf_find_immut(lhs_term.0);
        let rhs_root = arena.uf_find_immut(rhs_term.0);
        // Count how many nodes are in each e-class
        let lhs_members: usize = (0..cap).filter(|&i| arena.get(Ptr(i as u32)).is_some() && arena.uf_find_immut(i as u32) == lhs_root).count();
        let rhs_members: usize = (0..cap).filter(|&i| arena.get(Ptr(i as u32)).is_some() && arena.uf_find_immut(i as u32) == rhs_root).count();
        eprintln!("[SAT] round={} supers={} cong={} nodes={} eclasses={} spatial={} lhs_ec={}({}) rhs_ec={}({})",
            _round, new_supers, congruence_merges, arena.node_count(),
            eclass_roots.len(), arena.spatial_index.len(),
            lhs_root, lhs_members, rhs_root, rhs_members);

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

        // Node count guard: absolute cap.
        let current_count = arena.node_count();
        if current_count > fuel.max_nodes {
            break;
        }

        // Growth rate guard: bail out if graph is growing too fast.
        if prev_node_count > 100 && current_count > prev_node_count * 4 {
            break;
        }
        prev_node_count = current_count;
    }

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
/// Check if a pattern is a bare meta-variable (e.g., `?p`).
/// These require restricted reverse matching to avoid combinatorial blowup.
fn is_bare_meta(sexp: &Sexp) -> bool {
    matches!(sexp, Sexp::Atom(name) if name.starts_with('?'))
}

fn apply_laws_one_round_saturating(
    arena: &mut ArchonArena,
    laws: &[&SatRule],
    ops: &HashMap<String, u8>,
    original_eclasses: &HashSet<u32>,
    round: usize,
) -> usize {
    // After the expansion phase (first 2 rounds), disable bare-variable
    // reverse matching to prevent exponential identity wrapper nesting.
    let expansion_phase = round < 2;
    let mut new_supers = 0;
    let capacity = arena.inner.node_capacity();
    // Track which (binding_e-class_roots, law_name, direction) combos we've already done.
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

    // For bare-variable reverse matching, only match against nodes whose
    // e-class root is in the original set. This prevents expansion laws
    // like ?p => wrapper(?p) from firing on intermediary noise.
    let original_nodes: BTreeSet<u32> = existing_nodes.iter()
        .filter(|&&idx| original_eclasses.contains(&arena.uf_find_immut(idx)))
        .copied()
        .collect();

    for (law_idx, law) in laws.iter().enumerate() {
        // Forward: match LHS, materialize RHS, superpose.
        // Skip bare-meta matches after expansion phase (prevents identity nesting).
        let fwd_bare = is_bare_meta(&law.lhs);
        let matches_fwd = if fwd_bare && !expansion_phase {
            vec![]
        } else {
            let fwd_nodes = if fwd_bare { &original_nodes } else { &existing_nodes };
            find_pattern_matches_filtered(arena, &law.lhs, fwd_nodes)
        };
        if !matches_fwd.is_empty() {
            eprintln!("[MATCH-FWD] law={} matches={}", law.name, matches_fwd.len());
        }
        for (matched_root, bindings) in &matches_fwd {
            let binding_key = binding_eclass_key(arena, bindings);
            if !applied.insert((binding_key, law_idx, true)) {
                continue;
            }

            if let Some(mat_root) = materialize_with_bindings(arena, &law.rhs, bindings, ops, 0) {
                if try_superpose_new(arena, *matched_root, mat_root) {
                    new_supers += 1;
                    if law.name.contains("interchange") {
                        eprintln!("[LAW-FWD] {} matched node {} bindings={:?}",
                            law.name, matched_root.0,
                            bindings.iter().map(|(k,v)| (k.clone(), v.0)).collect::<Vec<_>>());
                    }
                }
            }
        }

        // Reverse: match RHS, materialize LHS, superpose.
        // When RHS is a bare meta-variable (?p), restrict to original e-classes
        // to prevent unbounded expansion.
        let rev_bare = is_bare_meta(&law.rhs);
        let matches_rev = if rev_bare && !expansion_phase {
            vec![]
        } else {
            let rev_nodes = if rev_bare { &original_nodes } else { &existing_nodes };
            find_pattern_matches_filtered(arena, &law.rhs, rev_nodes)
        };
        if !matches_rev.is_empty() {
            eprintln!("[MATCH-REV] law={} matches={}", law.name, matches_rev.len());
        }
        for (matched_root, bindings) in &matches_rev {
            let binding_key = binding_eclass_key(arena, bindings);
            if !applied.insert((binding_key, law_idx, false)) {
                continue;
            }

            if let Some(mat_root) = materialize_with_bindings(arena, &law.lhs, bindings, ops, 0) {
                if try_superpose_new(arena, *matched_root, mat_root) {
                    new_supers += 1;
                    if law.name.contains("interchange") {
                        eprintln!("[LAW-REV] {} matched node {} bindings={:?}",
                            law.name, matched_root.0,
                            bindings.iter().map(|(k,v)| (k.clone(), v.0)).collect::<Vec<_>>());
                    }
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

/// Rebuild the e-graph to a fixpoint: populate spatial index, detect congruences,
/// propagate merges, and repeat until no new merges occur.
///
/// This is the standard "rebuild" phase from egg-style equality saturation.
/// It must loop because:
/// - Populating the spatial index can discover congruences (hashcons collisions)
/// - Congruence propagation can merge nodes, invalidating spatial index signatures
/// - New merges may reveal further congruences in a cascade
fn rebuild_to_fixpoint(arena: &mut ArchonArena) -> usize {
    let mut total_merges = 0;
    let max_rebuild_rounds = 100;

    for _rebuild_round in 0..max_rebuild_rounds {
        eprintln!("[REBUILD] round={} queue={} nodes={}", _rebuild_round, arena.shockwave_queue.len(), arena.node_count());
        // Step A: Clear and populate the spatial index from scratch.
        arena.spatial_index.clear();
        let mut round_merges = 0;

        let capacity = arena.inner.node_capacity();
        for idx in 0..capacity {
            let ptr = Ptr(idx as u32);
            if arena.get(ptr).is_none() {
                continue;
            }
            if superposition::is_superposition(arena, ptr) {
                continue;
            }

            // Step B: Insert into spatial index; catch collisions.
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
                round_merges += 1;
            }
        }

        // Step C: Drain the shockwave queue (congruence closure).
        // propagate_congruence uses the now-fully-populated spatial index.
        round_merges += superposition::propagate_congruence(arena);

        // Step D: If no merges, the graph is canonical — done.
        total_merges += round_merges;
        if round_merges == 0 {
            break;
        }
    }

    total_merges
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
    let mut visited = HashSet::new();
    record_subtree_parents_inner(arena, root, &mut visited);
}

fn record_subtree_parents_inner(arena: &mut ArchonArena, root: Ptr, visited: &mut HashSet<u32>) {
    if !visited.insert(root.0) {
        return;
    }
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
        record_subtree_parents_inner(arena, child, visited);
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

// ── AC normalization ──────────────────────────────────────────────────

/// Detect identity elements for operators from directed rules.
/// Pattern: `op(e, ?x) ==> ?x` means `e` is a left-identity for `op`.
fn detect_identity_elements(rules: &[SatRule]) -> HashMap<String, HashSet<String>> {
    let mut identities: HashMap<String, HashSet<String>> = HashMap::new();
    for rule in rules {
        if rule.bidirectional { continue; }
        // op(e, ?x) ==> ?x
        if let Sexp::List(lhs) = &rule.lhs {
            if lhs.len() == 3 {
                if let Sexp::Atom(ref op) = lhs[0] {
                    if let Sexp::Atom(ref maybe_id) = lhs[1] {
                        if !maybe_id.starts_with('?') {
                            if let Sexp::Atom(ref rhs_var) = rule.rhs {
                                if let Sexp::Atom(ref lhs_var) = lhs[2] {
                                    if rhs_var == lhs_var && lhs_var.starts_with('?') {
                                        identities.entry(op.clone()).or_default()
                                            .insert(maybe_id.clone());
                                    }
                                }
                            }
                        }
                    }
                    // Also check: op(?x, e) ==> ?x
                    if let Sexp::Atom(ref maybe_id) = lhs[2] {
                        if !maybe_id.starts_with('?') {
                            if let Sexp::Atom(ref rhs_var) = rule.rhs {
                                if let Sexp::Atom(ref lhs_var) = lhs[1] {
                                    if rhs_var == lhs_var && lhs_var.starts_with('?') {
                                        identities.entry(op.clone()).or_default()
                                            .insert(maybe_id.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    identities
}

/// Detect operators that have both commutativity and associativity laws.
fn detect_ac_operators(rules: &[SatRule]) -> HashSet<String> {
    let mut commutative: HashSet<String> = HashSet::new();
    let mut associative: HashSet<String> = HashSet::new();

    for rule in rules {
        if !rule.bidirectional { continue; }
        if let (Sexp::List(lhs), Sexp::List(rhs)) = (&rule.lhs, &rule.rhs) {
            if lhs.len() == 3 && rhs.len() == 3 {
                if let (Sexp::Atom(lop), Sexp::Atom(rop)) = (&lhs[0], &rhs[0]) {
                    if lop == rop {
                        // Commutativity: op(?x, ?y) === op(?y, ?x)
                        if is_meta(&lhs[1]) && is_meta(&lhs[2])
                            && lhs[1] == rhs[2] && lhs[2] == rhs[1]
                        {
                            commutative.insert(lop.clone());
                        }
                        // Associativity: op(op(?x,?y),?z) === op(?x,op(?y,?z))
                        if let Sexp::List(il) = &lhs[1] {
                            if il.len() == 3 {
                                if let Sexp::Atom(ref iop) = il[0] {
                                    if iop == lop {
                                        if let Sexp::List(ir) = &rhs[2] {
                                            if ir.len() == 3 {
                                                if matches!(&ir[0], Sexp::Atom(ref o) if o == rop) {
                                                    associative.insert(lop.clone());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        // Also: op(?x,op(?y,?z)) === op(op(?x,?y),?z)
                        if let Sexp::List(il) = &lhs[2] {
                            if il.len() == 3 {
                                if let Sexp::Atom(ref iop) = il[0] {
                                    if iop == lop {
                                        if let Sexp::List(ir) = &rhs[1] {
                                            if ir.len() == 3 {
                                                if matches!(&ir[0], Sexp::Atom(ref o) if o == rop) {
                                                    associative.insert(lop.clone());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    commutative.intersection(&associative).cloned().collect()
}

fn is_meta(s: &Sexp) -> bool {
    matches!(s, Sexp::Atom(name) if name.starts_with('?'))
}

/// AC-normalize: flatten nested AC operator applications into sorted multisets,
/// strip identity elements, then rebuild as right-associated canonical form.
fn ac_normalize(expr: &Sexp, ac_ops: &HashSet<String>) -> Sexp {
    ac_normalize_with_ids(expr, ac_ops, &HashMap::new())
}

fn ac_normalize_with_ids(expr: &Sexp, ac_ops: &HashSet<String>, ids: &HashMap<String, HashSet<String>>) -> Sexp {
    match expr {
        Sexp::Atom(_) => expr.clone(),
        Sexp::List(items) => {
            if items.len() >= 3 {
                if let Sexp::Atom(ref op) = items[0] {
                    if ac_ops.contains(op) {
                        let mut children = Vec::new();
                        ac_flatten_with_ids(expr, op, &mut children, ac_ops, ids);
                        // Strip identity elements for this operator.
                        if let Some(id_set) = ids.get(op) {
                            children.retain(|c| {
                                !matches!(c, Sexp::Atom(ref a) if id_set.contains(a))
                            });
                        }
                        if children.is_empty() {
                            // All children were identities — return the identity.
                            if let Some(id_set) = ids.get(op) {
                                if let Some(id) = id_set.iter().next() {
                                    return Sexp::Atom(id.clone());
                                }
                            }
                            return expr.clone();
                        }
                        children.sort_by(|a, b| format!("{:?}", a).cmp(&format!("{:?}", b)));
                        return ac_rebuild(op, &children);
                    }
                }
            }
            Sexp::List(items.iter().map(|c| ac_normalize_with_ids(c, ac_ops, ids)).collect())
        }
    }
}

fn ac_flatten_with_ids(expr: &Sexp, op: &str, children: &mut Vec<Sexp>, ac_ops: &HashSet<String>, ids: &HashMap<String, HashSet<String>>) {
    match expr {
        Sexp::List(items) if items.len() >= 3 => {
            if let Sexp::Atom(ref head) = items[0] {
                if head == op {
                    for child in &items[1..] {
                        ac_flatten_with_ids(child, op, children, ac_ops, ids);
                    }
                    return;
                }
            }
            children.push(ac_normalize_with_ids(expr, ac_ops, ids));
        }
        _ => children.push(ac_normalize_with_ids(expr, ac_ops, ids)),
    }
}

fn ac_flatten(expr: &Sexp, op: &str, children: &mut Vec<Sexp>, ac_ops: &HashSet<String>) {
    match expr {
        Sexp::List(items) if items.len() >= 3 => {
            if let Sexp::Atom(ref head) = items[0] {
                if head == op {
                    for child in &items[1..] {
                        ac_flatten(child, op, children, ac_ops);
                    }
                    return;
                }
            }
            children.push(ac_normalize(expr, ac_ops));
        }
        _ => children.push(ac_normalize(expr, ac_ops)),
    }
}

fn ac_rebuild(op: &str, children: &[Sexp]) -> Sexp {
    match children.len() {
        0 => unreachable!(),
        1 => children[0].clone(),
        _ => {
            // Build right-associated tree iteratively to avoid stack overflow
            // on large flattened lists.
            let mut result = children[children.len() - 1].clone();
            for i in (0..children.len() - 1).rev() {
                result = Sexp::List(vec![
                    Sexp::Atom(op.into()),
                    children[i].clone(),
                    result,
                ]);
            }
            result
        }
    }
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
    let max_rewrites = max_interactions; // Secondary fuel for non-physics steps.
    let mut total_rewrites = 0u64;

    loop {
        if total_interactions >= max_interactions || total_rewrites >= max_rewrites {
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

        // Count non-physics work toward fuel to prevent infinite rewrite loops.
        if rewrite_fired || eta_fired {
            total_rewrites += 1;
        }

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
            } else if lam_principal.is_connected() {
                // App function not connected — erase the dangling lam peer.
                let era = arena.spawn(OpCode::Erase);
                arena.connect(lam_principal.target, lam_principal.slot, era, 0);
            } else if app_function.is_connected() {
                // Lam principal not connected — erase the dangling app function peer.
                let era = arena.spawn(OpCode::Erase);
                arena.connect(app_function.target, app_function.slot, era, 0);
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

    // Quick check: if already in same e-class, skip.
    if arena.uf_same(matched_root.0, mat_root.0) {
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
/// Returns ALL valid binding sets per starting node (multiple e-class peers
/// at any depth can produce different bindings that lead to different materializations).
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

        // Collect ALL valid binding sets for this node.
        let all_bindings = match_pattern_all_bindings(arena, ptr, pattern, 64);
        for bindings in all_bindings {
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
/// E-class aware: when ptr doesn't directly match, tries all e-class members
/// that share the same union-find root, enabling pattern matching through
/// superposition nodes.
/// Multi-valued e-class-aware pattern matching: returns ALL valid binding sets
/// for matching `pattern` against `ptr`'s e-class. Different e-class peers at
/// any depth can produce different bindings that lead to distinct materializations.
fn match_pattern_all_bindings(
    arena: &ArchonArena,
    ptr: Ptr,
    pattern: &Sexp,
    depth: usize,
) -> Vec<HashMap<String, Ptr>> {
    if depth == 0 { return vec![]; }

    let mut results = Vec::new();

    // Collect all candidate nodes: ptr itself + e-class peers with matching head.
    let mut candidates = vec![ptr];
    if let Sexp::List(items) = pattern {
        if let Some(Sexp::Atom(head)) = items.first() {
            if !head.starts_with('?') {
                let root = arena.uf_find_immut(ptr.0);
                let cap = arena.inner.node_capacity();
                for idx in 0..cap {
                    let peer = Ptr(idx as u32);
                    if peer == ptr { continue; }
                    if arena.get(peer).is_none() { continue; }
                    if superposition::is_superposition(arena, peer) { continue; }
                    if arena.uf_find_immut(peer.0) != root { continue; }
                    candidates.push(peer);
                }
            }
        }
    }

    for candidate in candidates {
        let new_bindings = match_direct_all(arena, candidate, pattern, &HashMap::new(), depth);
        results.extend(new_bindings);
    }

    results
}

/// Direct pattern match against a specific node, returning ALL valid binding sets.
/// For compound patterns, enumerates all combinations of child binding sets.
fn match_direct_all(
    arena: &ArchonArena,
    ptr: Ptr,
    pattern: &Sexp,
    base_bindings: &HashMap<String, Ptr>,
    depth: usize,
) -> Vec<HashMap<String, Ptr>> {
    if depth == 0 { return vec![]; }
    let node = match arena.get(ptr) {
        Some(n) => n,
        None => return vec![],
    };

    match pattern {
        Sexp::Atom(name) => {
            if name.starts_with('?') {
                if let Some(&bound) = base_bindings.get(name.as_str()) {
                    if arena.uf_find_immut(bound.0) == arena.uf_find_immut(ptr.0) {
                        vec![base_bindings.clone()]
                    } else {
                        vec![]
                    }
                } else {
                    let mut b = base_bindings.clone();
                    b.insert(name.clone(), ptr);
                    vec![b]
                }
            } else {
                if matches!(&node.kind, OpCode::Sym { name: n, arity: 0 } if n == name) {
                    vec![base_bindings.clone()]
                } else {
                    vec![]
                }
            }
        }
        Sexp::List(items) => {
            if items.is_empty() {
                return vec![];
            }
            let head = match &items[0] {
                Sexp::Atom(name) => name.as_str(),
                _ => return vec![],
            };

            let (node_name, node_arity) = match &node.kind {
                OpCode::Sym { name, arity } => (name.as_str(), *arity as usize),
                OpCode::App => ("app", 2),
                OpCode::Lam => ("lam", 2),
                _ => return vec![],
            };

            if head != node_name {
                return vec![];
            }

            let args = &items[1..];
            if args.len() != node_arity {
                return vec![];
            }

            // For each child, recursively collect all binding sets,
            // then take the Cartesian product across children.
            let mut current_binding_sets = vec![base_bindings.clone()];

            for (i, arg_pat) in args.iter().enumerate() {
                let port = arena.port(ptr, (i + 1) as u8);
                if !port.is_connected() {
                    return vec![];
                }

                let mut next_binding_sets = Vec::new();
                for bs in &current_binding_sets {
                    // Get all binding sets for this child, starting from current bindings.
                    let child_results = match_child_all(arena, port.target, arg_pat, bs, depth - 1);
                    next_binding_sets.extend(child_results);
                }
                current_binding_sets = next_binding_sets;
                if current_binding_sets.is_empty() {
                    return vec![];
                }
                // Cap combinatorial explosion.
                if current_binding_sets.len() > 32 {
                    current_binding_sets.truncate(8);
                }
            }

            current_binding_sets
        }
    }
}

/// Match a child node against a pattern, considering all e-class peers.
/// Returns all valid binding sets (extending `base_bindings`).
fn match_child_all(
    arena: &ArchonArena,
    ptr: Ptr,
    pattern: &Sexp,
    base_bindings: &HashMap<String, Ptr>,
    depth: usize,
) -> Vec<HashMap<String, Ptr>> {
    if depth == 0 { return vec![]; }

    let mut results = Vec::new();

    // Direct match.
    results.extend(match_direct_all(arena, ptr, pattern, base_bindings, depth));

    // E-class peer matches (for compound patterns).
    if let Sexp::List(items) = pattern {
        if let Some(Sexp::Atom(head)) = items.first() {
            if !head.starts_with('?') {
                let root = arena.uf_find_immut(ptr.0);
                let cap = arena.inner.node_capacity();
                for idx in 0..cap {
                    let peer = Ptr(idx as u32);
                    if peer == ptr { continue; }
                    if arena.get(peer).is_none() { continue; }
                    if superposition::is_superposition(arena, peer) { continue; }
                    if arena.uf_find_immut(peer.0) != root { continue; }

                    results.extend(match_direct_all(arena, peer, pattern, base_bindings, depth));
                }
            }
        }
    }

    // Cap results to prevent combinatorial explosion.
    if results.len() > 32 {
        results.truncate(8);
    }

    results
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
/// Uses a visited map to handle cycles in the interaction net graph.
fn clone_subgraph(arena: &mut ArchonArena, ptr: Ptr, ops: &HashMap<String, u8>, region: u32) -> Ptr {
    let mut visited: HashMap<u32, Ptr> = HashMap::new();
    clone_subgraph_inner(arena, ptr, ops, region, &mut visited)
}

fn clone_subgraph_inner(
    arena: &mut ArchonArena,
    ptr: Ptr,
    ops: &HashMap<String, u8>,
    region: u32,
    visited: &mut HashMap<u32, Ptr>,
) -> Ptr {
    if let Some(&already) = visited.get(&ptr.0) {
        return already;
    }

    let node = match arena.get(ptr) {
        Some(n) => n,
        None => return arena.spawn_in(OpCode::Sym { name: "_dead".into(), arity: 0 }, region),
    };
    let kind = node.kind.clone();
    let port_count = kind.port_count();
    let new_node = arena.spawn_in(kind, region);
    visited.insert(ptr.0, new_node);

    // Recursively clone children (aux ports, skip principal port 0).
    for slot in 1..port_count {
        let port = arena.port(ptr, slot as u8);
        if port.is_connected() {
            // Skip if child is a Superposition (don't clone e-class hubs).
            if superposition::is_superposition(arena, port.target) {
                continue;
            }
            let child_clone = clone_subgraph_inner(arena, port.target, ops, region, visited);
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

    #[test]
    fn ac_normalize_comm_assoc() {
        let a = atom("a"); let b = atom("b"); let c = atom("c");
        // op(a, op(b, c)) and op(op(c, a), b) should normalize to the same form.
        let lhs = list(vec![atom("op"), a.clone(), list(vec![atom("op"), b.clone(), c.clone()])]);
        let rhs = list(vec![atom("op"), list(vec![atom("op"), c.clone(), a.clone()]), b.clone()]);
        let ac_ops: HashSet<String> = vec!["op".to_string()].into_iter().collect();
        assert_eq!(super::ac_normalize(&lhs, &ac_ops), super::ac_normalize(&rhs, &ac_ops));
    }
}
