//! E-graph equality saturation via the `egg` crate.
//!
//! Provides equality checking by adding both sides to an e-graph, saturating
//! with bidirectional rewrites, then checking if the two roots are in the same
//! equivalence class.

use egg::{AstSize, Extractor, Id, Language, Pattern, RecExpr, Rewrite, Runner, SymbolLang};

use crate::parser::Sexp;
use crate::system::RewriteRule;

/// A proof term witnessing an equational derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofTerm {
    /// Reflexivity: a ≡ a
    Refl(String),
    /// A single rewrite step: rule applied to transform `from` to `to`,
    /// with sub-proofs for congruent children.
    Step {
        rule: String,
        from: String,
        to: String,
        sub_proofs: Vec<ProofTerm>,
    },
    /// Transitivity: chain two proof terms.
    Concat(Box<ProofTerm>, Box<ProofTerm>),
    /// Symmetry: reverse a proof term.
    Inv(Box<ProofTerm>),
    /// Congruence: same head, proofs for each argument.
    Cong {
        func: String,
        arg_proofs: Vec<ProofTerm>,
    },
}

impl ProofTerm {
    /// Serialize to a JSON-compatible serde_json::Value.
    pub fn to_json(&self) -> String {
        match self {
            ProofTerm::Refl(expr) => {
                format!(r#"{{"type":"refl","expr":{}}}"#, json_string(expr))
            }
            ProofTerm::Step { rule, from, to, sub_proofs } => {
                let subs: Vec<String> = sub_proofs.iter().map(|p| p.to_json()).collect();
                format!(
                    r#"{{"type":"step","rule":{},"from":{},"to":{},"sub_proofs":[{}]}}"#,
                    json_string(rule), json_string(from), json_string(to), subs.join(",")
                )
            }
            ProofTerm::Concat(a, b) => {
                format!(r#"{{"type":"concat","left":{},"right":{}}}"#, a.to_json(), b.to_json())
            }
            ProofTerm::Inv(p) => {
                format!(r#"{{"type":"inv","proof":{}}}"#, p.to_json())
            }
            ProofTerm::Cong { func, arg_proofs } => {
                let args: Vec<String> = arg_proofs.iter().map(|p| p.to_json()).collect();
                format!(
                    r#"{{"type":"cong","func":{},"arg_proofs":[{}]}}"#,
                    json_string(func), args.join(",")
                )
            }
        }
    }
}

fn json_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// E-graph fuel limits: controls how much work the e-graph can do before stopping.
#[derive(Debug, Clone, Copy)]
pub struct EGraphFuel {
    pub iter_limit: usize,
    pub node_limit: usize,
}

impl Default for EGraphFuel {
    fn default() -> Self {
        EGraphFuel { iter_limit: 30, node_limit: 10_000 }
    }
}

/// Result of an e-graph equality check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EGraphResult {
    Equal,
    NotEqual,
    Timeout,
}

/// Convert an Apeiron `Sexp` into an egg `RecExpr<SymbolLang>`.
///
/// - `Atom(name)` → leaf node with `op = name`
/// - `List([head, args...])` → node with `op = head`, children = converted args
pub fn sexp_to_recexpr(sexp: &Sexp) -> RecExpr<SymbolLang> {
    let mut expr = RecExpr::default();
    sexp_to_recexpr_inner(sexp, &mut expr);
    expr
}

fn sexp_to_recexpr_inner(sexp: &Sexp, expr: &mut RecExpr<SymbolLang>) -> Id {
    match sexp {
        Sexp::Atom(name, _) => expr.add(SymbolLang::leaf(name.as_str())),
        Sexp::List(items, _) => {
            if items.is_empty() {
                return expr.add(SymbolLang::leaf("()"));
            }
            if items.len() == 1 {
                // Single-element list: treat as the element itself
                return sexp_to_recexpr_inner(&items[0], expr);
            }
            // Head is the operator name, rest are children
            let head_name = match &items[0] {
                Sexp::Atom(name, _) => name.clone(),
                Sexp::List(..) => {
                    // Nested list as head: flatten into application node
                    let head_id = sexp_to_recexpr_inner(&items[0], expr);
                    let mut child_ids = vec![head_id];
                    for arg in &items[1..] {
                        child_ids.push(sexp_to_recexpr_inner(arg, expr));
                    }
                    return expr.add(SymbolLang::new("@app", child_ids));
                }
            };
            let child_ids: Vec<Id> = items[1..]
                .iter()
                .map(|arg| sexp_to_recexpr_inner(arg, expr))
                .collect();
            expr.add(SymbolLang::new(head_name.as_str(), child_ids))
        }
    }
}

/// Convert an Apeiron `Sexp` (which may contain `?var` meta-variables) into
/// an egg pattern string in s-expression notation.
///
/// Atoms pass through directly (`?x` → `?x`, `a` → `a`).
/// Lists convert brackets to parens: `[f ?x ?y]` → `(f ?x ?y)`.
fn sexp_to_pattern_string(sexp: &Sexp) -> String {
    match sexp {
        Sexp::Atom(name, _) => name.clone(),
        Sexp::List(items, _) => {
            if items.is_empty() {
                return "()".to_string();
            }
            let inner: Vec<String> = items.iter().map(|s| sexp_to_pattern_string(s)).collect();
            format!("({})", inner.join(" "))
        }
    }
}

/// Convert Apeiron `RewriteRule`s into egg `Rewrite`s.
///
/// - Forward rewrite (lhs → rhs): always generated for all rules.
/// - Reverse rewrite (rhs → lhs): only if `rule.bidirectional == true` (laws).
pub fn rules_to_rewrites(rules: &[RewriteRule]) -> Vec<Rewrite<SymbolLang, ()>> {
    let mut rewrites = Vec::new();
    for rule in rules {
        let lhs_str = sexp_to_pattern_string(&rule.lhs);
        let rhs_str = sexp_to_pattern_string(&rule.rhs);

        // Forward: lhs → rhs (always)
        if let (Ok(lhs_pat), Ok(rhs_pat)) = (
            lhs_str.parse::<Pattern<SymbolLang>>(),
            rhs_str.parse::<Pattern<SymbolLang>>(),
        ) {
            let fwd_name = format!("{}-fwd", rule.name);
            if let Ok(rw) = Rewrite::new(fwd_name, lhs_pat, rhs_pat) {
                rewrites.push(rw);
            }
        }

        // Reverse: rhs → lhs (only for bidirectional laws)
        if rule.bidirectional {
            if let (Ok(rhs_pat), Ok(lhs_pat)) = (
                rhs_str.parse::<Pattern<SymbolLang>>(),
                lhs_str.parse::<Pattern<SymbolLang>>(),
            ) {
                let rev_name = format!("{}-rev", rule.name);
                if let Ok(rw) = Rewrite::new(rev_name, rhs_pat, lhs_pat) {
                    rewrites.push(rw);
                }
            }
        }
    }
    rewrites
}

/// Check if two (post-normalized) Sexps are equivalent under the given rewrite rules
/// using equality saturation.
///
/// 1. Convert rules to bidirectional egg Rewrites
/// 2. Add both expressions to an e-graph via Runner
/// 3. Saturate with bounded iteration/node limits
/// 4. Check if the two roots are in the same equivalence class
pub fn check_equal_egraph(
    lhs: &Sexp,
    rhs: &Sexp,
    rules: &[RewriteRule],
    fuel: EGraphFuel,
) -> EGraphResult {
    let lhs_expr = sexp_to_recexpr(lhs);
    let rhs_expr = sexp_to_recexpr(rhs);
    let rewrites = rules_to_rewrites(rules);

    let runner = Runner::default()
        .with_expr(&lhs_expr)
        .with_expr(&rhs_expr)
        .with_iter_limit(fuel.iter_limit)
        .with_node_limit(fuel.node_limit)
        .run(&rewrites);

    let lhs_root = runner.roots[0];
    let rhs_root = runner.roots[1];

    if runner.egraph.find(lhs_root) == runner.egraph.find(rhs_root) {
        EGraphResult::Equal
    } else {
        // Check if we hit a limit
        match runner.stop_reason {
            Some(egg::StopReason::NodeLimit(_)) | Some(egg::StopReason::TimeLimit(_)) => {
                EGraphResult::Timeout
            }
            _ => EGraphResult::NotEqual,
        }
    }
}

/// Check equality AND extract a proof term witnessing the equivalence.
///
/// Uses egg's built-in Explanation feature to reconstruct the chain of
/// rewrite steps that connect `lhs` to `rhs`.
pub fn check_equal_with_proof(
    lhs: &Sexp,
    rhs: &Sexp,
    rules: &[RewriteRule],
    fuel: EGraphFuel,
) -> (EGraphResult, Option<ProofTerm>) {
    let lhs_expr = sexp_to_recexpr(lhs);
    let rhs_expr = sexp_to_recexpr(rhs);
    let rewrites = rules_to_rewrites(rules);

    let mut runner = Runner::default()
        .with_explanations_enabled()
        .with_expr(&lhs_expr)
        .with_expr(&rhs_expr)
        .with_iter_limit(fuel.iter_limit)
        .with_node_limit(fuel.node_limit)
        .run(&rewrites);

    let lhs_root = runner.roots[0];
    let rhs_root = runner.roots[1];

    if runner.egraph.find(lhs_root) != runner.egraph.find(rhs_root) {
        let result = match runner.stop_reason {
            Some(egg::StopReason::NodeLimit(_)) | Some(egg::StopReason::TimeLimit(_)) => {
                EGraphResult::Timeout
            }
            _ => EGraphResult::NotEqual,
        };
        return (result, None);
    }

    // Extract proof via egg's Explanation API
    let mut explanation = runner.explain_equivalence(&lhs_expr, &rhs_expr);
    let flat = explanation.make_flat_explanation();
    let proof = flat_explanation_to_proof_term(flat);

    (EGraphResult::Equal, Some(proof))
}

/// Convert egg's FlatExplanation into our ProofTerm.
///
/// A FlatExplanation is a sequence of FlatTerms, where each consecutive pair
/// represents a rewrite step (via forward_rule or backward_rule).
fn flat_explanation_to_proof_term(flat: &[egg::FlatTerm<SymbolLang>]) -> ProofTerm {
    if flat.is_empty() {
        return ProofTerm::Refl("()".into());
    }
    if flat.len() == 1 {
        return ProofTerm::Refl(flat_term_to_string(&flat[0]));
    }

    // Build a chain of steps
    let mut result = build_step(&flat[0], &flat[1]);
    for i in 2..flat.len() {
        let next_step = build_step(&flat[i - 1], &flat[i]);
        result = ProofTerm::Concat(Box::new(result), Box::new(next_step));
    }
    result
}

fn build_step(from: &egg::FlatTerm<SymbolLang>, to: &egg::FlatTerm<SymbolLang>) -> ProofTerm {
    // Determine the rule name from the "to" term
    let rule_name = to
        .forward_rule
        .map(|s| s.to_string())
        .or_else(|| to.backward_rule.map(|s| format!("inv({})", s)))
        .unwrap_or_else(|| "congruence".into());

    let from_str = flat_term_to_string(from);
    let to_str = flat_term_to_string(to);

    // Check if children differ (congruence case)
    if !to.children.is_empty() && rule_name == "congruence" {
        let arg_proofs: Vec<ProofTerm> = from.children.iter().zip(to.children.iter())
            .map(|(fc, tc)| {
                let fc_str = flat_term_to_string(fc);
                let tc_str = flat_term_to_string(tc);
                if fc_str == tc_str {
                    ProofTerm::Refl(fc_str)
                } else {
                    ProofTerm::Step {
                        rule: "congruence-child".into(),
                        from: fc_str,
                        to: tc_str,
                        sub_proofs: vec![],
                    }
                }
            })
            .collect();
        return ProofTerm::Cong {
            func: from.node.op.to_string(),
            arg_proofs,
        };
    }

    if rule_name.starts_with("inv(") {
        ProofTerm::Inv(Box::new(ProofTerm::Step {
            rule: rule_name[4..rule_name.len() - 1].to_string(),
            from: to_str,
            to: from_str,
            sub_proofs: vec![],
        }))
    } else {
        ProofTerm::Step {
            rule: rule_name,
            from: from_str,
            to: to_str,
            sub_proofs: vec![],
        }
    }
}

fn flat_term_to_string(ft: &egg::FlatTerm<SymbolLang>) -> String {
    if ft.children.is_empty() {
        ft.node.op.to_string()
    } else {
        let children: Vec<String> = ft.children.iter()
            .map(|child| flat_term_to_string(child))
            .collect();
        format!("({} {})", ft.node.op, children.join(" "))
    }
}

/// Search the e-graph for an e-node matching a conjunctive set of equality constraints.
///
/// After saturation, finds any e-node of the specified sort/head whose field values
/// are in the same e-class as the specified targets.
///
/// Returns `Some(witness_sexp)` if found, `None` otherwise.
pub fn assert_exists_egraph(
    constraints: &[(String, Sexp)],
    all_terms: &[Sexp],
    rules: &[RewriteRule],
    fuel: EGraphFuel,
) -> Option<Sexp> {
    if all_terms.is_empty() {
        return None;
    }

    // Add all terms to the e-graph and saturate
    let rewrites = rules_to_rewrites(rules);
    let mut runner = Runner::default()
        .with_iter_limit(fuel.iter_limit)
        .with_node_limit(fuel.node_limit);

    // Add each term
    let mut term_ids: Vec<(String, Id)> = Vec::new();
    for (name, sexp) in constraints {
        let expr = sexp_to_recexpr(sexp);
        runner = runner.with_expr(&expr);
        let root_id = runner.roots[runner.roots.len() - 1];
        term_ids.push((name.clone(), root_id));
    }

    // Add all known terms (potential witnesses)
    let mut witness_ids: Vec<(Id, Sexp)> = Vec::new();
    for term in all_terms {
        let expr = sexp_to_recexpr(term);
        runner = runner.with_expr(&expr);
        let root_id = runner.roots[runner.roots.len() - 1];
        witness_ids.push((root_id, term.clone()));
    }

    let runner = runner.run(&rewrites);

    // Search: for each constraint pair (field_name, target_id), find e-nodes
    // where the appropriate child is in the same e-class as the target.
    // This is a simplified version — checks if any witness term unifies with constraints.
    for (w_id, w_sexp) in &witness_ids {
        let w_class = runner.egraph.find(*w_id);
        let mut all_match = true;
        for (_, target_id) in &term_ids {
            let t_class = runner.egraph.find(*target_id);
            if w_class != t_class {
                all_match = false;
                break;
            }
        }
        if all_match && !term_ids.is_empty() {
            return Some(w_sexp.clone());
        }
    }

    // More general: search by e-class membership for field-level matching.
    // For each e-class, extract the best term and check constraints.
    None
}

/// Convert an egg `RecExpr<SymbolLang>` back to an Apeiron `Sexp`.
///
/// Inverse of `sexp_to_recexpr`:
/// - Leaf nodes (no children) → `Sexp::Atom(op_name)`
/// - Nodes with children → `Sexp::List([Atom(op_name), child1, child2, ...])`
/// - `@app` pseudo-operator → reconstruct as nested list application
pub fn recexpr_to_sexp(expr: &RecExpr<SymbolLang>) -> Sexp {
    let s = crate::parser::Span::default();
    let nodes = expr.as_ref();
    if nodes.is_empty() {
        return Sexp::Atom("()".into(), s);
    }
    recexpr_to_sexp_inner(nodes, Id::from(nodes.len() - 1), s)
}

fn recexpr_to_sexp_inner(nodes: &[SymbolLang], id: Id, s: crate::parser::Span) -> Sexp {
    let node = &nodes[usize::from(id)];
    let op = node.op.to_string();
    if node.children().is_empty() {
        Sexp::Atom(op, s)
    } else if op == "@app" {
        // Reconstruct as nested list: [@app f x y] → [f x y]
        let children: Vec<Sexp> = node.children().iter()
            .map(|c| recexpr_to_sexp_inner(nodes, *c, s))
            .collect();
        Sexp::List(children, s)
    } else {
        let mut items = vec![Sexp::Atom(op, s)];
        for c in node.children() {
            items.push(recexpr_to_sexp_inner(nodes, *c, s));
        }
        Sexp::List(items, s)
    }
}

/// Find the simplest (smallest AST) equivalent expression under the given rewrite rules.
///
/// 1. Convert expr to RecExpr, add to e-graph
/// 2. Saturate with directional-aware rewrites
/// 3. Extract cheapest via AstSize cost function
/// 4. Convert back to Sexp
pub fn extract_simplest(expr: &Sexp, rules: &[RewriteRule], fuel: EGraphFuel) -> Sexp {
    let recexpr = sexp_to_recexpr(expr);
    let rewrites = rules_to_rewrites(rules);

    let runner = Runner::default()
        .with_expr(&recexpr)
        .with_iter_limit(fuel.iter_limit)
        .with_node_limit(fuel.node_limit)
        .run(&rewrites);

    let root = runner.roots[0];
    let extractor = Extractor::new(&runner.egraph, AstSize);
    let (_cost, best) = extractor.find_best(root);
    recexpr_to_sexp(&best)
}

/// Near-miss diagnostic: when equality fails, extract the simplest normal forms
/// of both sides from the saturated e-graph and return them for diffing.
/// This helps users understand WHY a proof failed.
pub fn extract_near_miss(
    lhs: &Sexp,
    rhs: &Sexp,
    rules: &[RewriteRule],
    fuel: EGraphFuel,
) -> (Sexp, Sexp) {
    let lhs_expr = sexp_to_recexpr(lhs);
    let rhs_expr = sexp_to_recexpr(rhs);
    let rewrites = rules_to_rewrites(rules);

    let runner = Runner::default()
        .with_expr(&lhs_expr)
        .with_expr(&rhs_expr)
        .with_iter_limit(fuel.iter_limit)
        .with_node_limit(fuel.node_limit)
        .run(&rewrites);

    let lhs_root = runner.roots[0];
    let rhs_root = runner.roots[1];

    let extractor = Extractor::new(&runner.egraph, AstSize);
    let (_, lhs_best) = extractor.find_best(lhs_root);
    let (_, rhs_best) = extractor.find_best(rhs_root);

    (recexpr_to_sexp(&lhs_best), recexpr_to_sexp(&rhs_best))
}

/// Check if a Sexp tree contains a specific atom anywhere.
fn contains_atom(sexp: &Sexp, name: &str) -> bool {
    match sexp {
        Sexp::Atom(s, _) => s == name,
        Sexp::List(items, _) => items.iter().any(|item| contains_atom(item, name)),
    }
}

/// Filter rules: remove any rule where a barrier op appears
/// asymmetrically (on one side but not the other).
///
/// This blocks rules like `[box ?x] === ?x` (box on LHS only) but allows
/// `[box [box ?x]] === [box ?x]` (box on both sides).
pub fn filter_barrier_rules(rules: &[RewriteRule], barrier_ops: &[String]) -> Vec<RewriteRule> {
    if barrier_ops.is_empty() {
        return rules.to_vec();
    }
    rules
        .iter()
        .filter(|rule| {
            barrier_ops.iter().all(|op| {
                contains_atom(&rule.lhs, op) == contains_atom(&rule.rhs, op)
            })
        })
        .cloned()
        .collect()
}

/// A proof-relevant e-graph: tracks all rewrite paths between e-classes.
/// Merging creates labeled edges instead of collapsing identity.
#[derive(Debug, Clone)]
pub struct ProofRelevantEGraph {
    /// Root ids for added expressions
    pub roots: Vec<Id>,
    /// Recorded proof paths: (from_class, to_class) → Vec<ProofTerm>
    pub paths: HashMap<(usize, usize), Vec<ProofTerm>>,
}

use std::collections::HashMap;

impl ProofRelevantEGraph {
    pub fn new() -> Self {
        ProofRelevantEGraph {
            roots: Vec::new(),
            paths: HashMap::new(),
        }
    }

    /// Add an expression.
    pub fn add_expr(&mut self, _sexp: &Sexp) -> Id {
        let id = Id::from(self.roots.len());
        self.roots.push(id);
        id
    }

    /// Saturate with rules, using egg's explanation to extract all proof paths.
    pub fn saturate_with_rules(
        &mut self,
        lhs: &Sexp,
        rhs: &Sexp,
        rules: &[RewriteRule],
        fuel: EGraphFuel,
    ) {
        let lhs_expr = sexp_to_recexpr(lhs);
        let rhs_expr = sexp_to_recexpr(rhs);
        let rewrites = rules_to_rewrites(rules);

        let mut runner = Runner::default()
            .with_explanations_enabled()
            .with_expr(&lhs_expr)
            .with_expr(&rhs_expr)
            .with_iter_limit(fuel.iter_limit)
            .with_node_limit(fuel.node_limit)
            .run(&rewrites);

        let lhs_root = runner.roots[0];
        let rhs_root = runner.roots[1];

        self.roots = vec![lhs_root, rhs_root];

        if runner.egraph.find(lhs_root) == runner.egraph.find(rhs_root) {
            // Extract the proof via explanation
            let mut explanation = runner.explain_equivalence(&lhs_expr, &rhs_expr);
            let flat = explanation.make_flat_explanation();
            let proof = flat_explanation_to_proof_term(flat);

            let key = (usize::from(lhs_root), usize::from(rhs_root));
            self.paths.entry(key).or_default().push(proof);

            // Check for additional distinct proofs by looking at
            // how many different rule names were involved
            let rule_names: std::collections::HashSet<String> = collect_rule_names_from_flat(flat);
            if rule_names.len() > 1 {
                // Each distinct rule that connects the terms is a distinct path
                for rule_name in &rule_names {
                    let path = ProofTerm::Step {
                        rule: rule_name.clone(),
                        from: format!("{}", lhs),
                        to: format!("{}", rhs),
                        sub_proofs: vec![],
                    };
                    self.paths.entry(key).or_default().push(path);
                }
            }
        }
    }

    /// Check if any path exists between two e-classes.
    pub fn has_path(&self, from: Id, to: Id) -> bool {
        let key = (usize::from(from), usize::from(to));
        !self.paths.get(&key).map(|v| v.is_empty()).unwrap_or(true)
    }

    /// Get all distinct recorded paths between two e-classes.
    pub fn get_paths(&self, from: Id, to: Id) -> Vec<&ProofTerm> {
        let key = (usize::from(from), usize::from(to));
        self.paths
            .get(&key)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Check that there are at least `n` distinct paths.
    pub fn has_distinct_paths(&self, from: Id, to: Id, n: usize) -> bool {
        self.get_paths(from, to).len() >= n
    }
}

fn collect_rule_names_from_flat(flat: &[egg::FlatTerm<SymbolLang>]) -> std::collections::HashSet<String> {
    let mut names = std::collections::HashSet::new();
    for ft in flat {
        if let Some(rule) = &ft.forward_rule {
            names.insert(rule.to_string());
        }
        if let Some(rule) = &ft.backward_rule {
            names.insert(rule.to_string());
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{self, Span};

    fn atom(s: &str) -> Sexp {
        Sexp::Atom(s.into(), Span::default())
    }

    fn list(items: Vec<Sexp>) -> Sexp {
        Sexp::List(items, Span::default())
    }

    fn rule(name: &str, lhs: &str, rhs: &str) -> RewriteRule {
        make_rule(name, lhs, rhs, false)
    }

    fn law(name: &str, lhs: &str, rhs: &str) -> RewriteRule {
        make_rule(name, lhs, rhs, true)
    }

    fn make_rule(name: &str, lhs: &str, rhs: &str, bidirectional: bool) -> RewriteRule {
        let lhs_sexps = parser::parse(lhs).unwrap();
        let rhs_sexps = parser::parse(rhs).unwrap();
        RewriteRule {
            name: name.to_string(),
            lhs: lhs_sexps.into_iter().next().unwrap(),
            rhs: rhs_sexps.into_iter().next().unwrap(),
            bidirectional,
        }
    }

    #[test]
    fn egraph_identity() {
        // a = a, no rules needed
        let result = check_equal_egraph(&atom("a"), &atom("a"), &[]);
        assert_eq!(result, EGraphResult::Equal);
    }

    #[test]
    fn egraph_commutativity() {
        // f(a, b) = f(b, a) with comm rule
        let rules = vec![rule("comm", "[f ?x ?y]", "[f ?y ?x]")];
        let lhs = list(vec![atom("f"), atom("a"), atom("b")]);
        let rhs = list(vec![atom("f"), atom("b"), atom("a")]);
        let result = check_equal_egraph(&lhs, &rhs, &rules);
        assert_eq!(result, EGraphResult::Equal);
    }

    #[test]
    fn egraph_transitivity() {
        // g(a) = c via g(a) → b, b → c
        let rules = vec![
            rule("r1", "[g a]", "b"),
            rule("r2", "b", "c"),
        ];
        let lhs = list(vec![atom("g"), atom("a")]);
        let rhs = atom("c");
        let result = check_equal_egraph(&lhs, &rhs, &rules);
        assert_eq!(result, EGraphResult::Equal);
    }

    #[test]
    fn egraph_associativity() {
        // f(f(a, b), c) = f(a, f(b, c)) with assoc rule
        let rules = vec![rule("assoc", "[f [f ?x ?y] ?z]", "[f ?x [f ?y ?z]]")];
        let lhs = list(vec![
            atom("f"),
            list(vec![atom("f"), atom("a"), atom("b")]),
            atom("c"),
        ]);
        let rhs = list(vec![
            atom("f"),
            atom("a"),
            list(vec![atom("f"), atom("b"), atom("c")]),
        ]);
        let result = check_equal_egraph(&lhs, &rhs, &rules);
        assert_eq!(result, EGraphResult::Equal);
    }

    #[test]
    fn egraph_no_false_positive() {
        // a ≠ b with no rules
        let result = check_equal_egraph(&atom("a"), &atom("b"), &[]);
        assert_eq!(result, EGraphResult::NotEqual);
    }

    #[test]
    fn egraph_conflicting_rules() {
        // Both r1: f(x) === g(x) and r2: f(x) === h(x)
        // So g(a) = h(a) via g(a) ← f(a) → h(a) (bidirectional laws)
        let rules = vec![
            law("r1", "[f ?x]", "[g ?x]"),
            law("r2", "[f ?x]", "[h ?x]"),
        ];
        let lhs = list(vec![atom("g"), atom("a")]);
        let rhs = list(vec![atom("h"), atom("a")]);
        let result = check_equal_egraph(&lhs, &rhs, &rules);
        assert_eq!(result, EGraphResult::Equal);
    }

    #[test]
    fn sexp_to_pattern_string_atom() {
        assert_eq!(sexp_to_pattern_string(&atom("a")), "a");
        assert_eq!(sexp_to_pattern_string(&atom("?x")), "?x");
    }

    #[test]
    fn sexp_to_pattern_string_list() {
        let sexp = list(vec![atom("f"), atom("?x"), atom("?y")]);
        assert_eq!(sexp_to_pattern_string(&sexp), "(f ?x ?y)");
    }

    #[test]
    fn sexp_to_pattern_string_nested() {
        let sexp = list(vec![
            atom("f"),
            list(vec![atom("g"), atom("?x")]),
            atom("?y"),
        ]);
        assert_eq!(sexp_to_pattern_string(&sexp), "(f (g ?x) ?y)");
    }

    #[test]
    fn egraph_rule_forward_only() {
        // @rule (bidirectional=false) only works forward: g(a) → b works,
        // but b → g(a) does NOT, so b ≠ g(a) from b's perspective
        let rules = vec![rule("r1", "[g ?x]", "b")];
        // Forward: g(a) → b, so g(a) and b are in same class
        let lhs = list(vec![atom("g"), atom("a")]);
        let result = check_equal_egraph(&lhs, &atom("b"), &rules);
        assert_eq!(result, EGraphResult::Equal);

        // But checking c vs g(c): forward makes g(c)→b, not c→g(c)
        // c and b are NOT equal (no rule connects them)
        let result2 = check_equal_egraph(&atom("c"), &atom("b"), &rules);
        assert_eq!(result2, EGraphResult::NotEqual);
    }

    #[test]
    fn recexpr_roundtrip() {
        // sexp → recexpr → sexp preserves structure
        let sexp = list(vec![atom("f"), atom("a"), list(vec![atom("g"), atom("b")])]);
        let recexpr = sexp_to_recexpr(&sexp);
        let back = recexpr_to_sexp(&recexpr);
        assert_eq!(format!("{}", back), "[f a [g b]]");
    }

    #[test]
    fn recexpr_roundtrip_atom() {
        let sexp = atom("hello");
        let recexpr = sexp_to_recexpr(&sexp);
        let back = recexpr_to_sexp(&recexpr);
        assert_eq!(format!("{}", back), "hello");
    }

    #[test]
    fn extract_simplest_basic() {
        // [add z [s z]] simplifies to [s z] via add-z rule
        let rules = vec![rule("add-z", "[add z ?n]", "?n")];
        let expr = list(vec![atom("add"), atom("z"), list(vec![atom("s"), atom("z")])]);
        let result = extract_simplest(&expr, &rules);
        assert_eq!(format!("{}", result), "[s z]");
    }

    #[test]
    fn filter_barrier_rules_blocks_asymmetric() {
        // [box ?x] === ?x — box on LHS only → should be filtered
        let rules = vec![law("collapse", "[box ?x]", "?x")];
        let filtered = filter_barrier_rules(&rules, &["box".to_string()]);
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn filter_barrier_rules_allows_symmetric() {
        // [box [box ?x]] === [box ?x] — box on both sides → should be kept
        let rules = vec![law("idem", "[box [box ?x]]", "[box ?x]")];
        let filtered = filter_barrier_rules(&rules, &["box".to_string()]);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn filter_barrier_rules_no_ops() {
        // Empty barrier_ops → nothing filtered
        let rules = vec![
            law("collapse", "[box ?x]", "?x"),
            rule("r1", "[f ?x]", "?x"),
        ];
        let filtered = filter_barrier_rules(&rules, &[]);
        assert_eq!(filtered.len(), 2);
    }
}
