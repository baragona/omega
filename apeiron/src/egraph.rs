//! E-graph equality saturation via the `egg` crate.
//!
//! Provides equality checking by adding both sides to an e-graph, saturating
//! with bidirectional rewrites, then checking if the two roots are in the same
//! equivalence class.

use egg::{AstSize, Extractor, Id, Language, Pattern, RecExpr, Rewrite, Runner, SymbolLang};

use crate::parser::Sexp;
use crate::system::RewriteRule;

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
) -> EGraphResult {
    let lhs_expr = sexp_to_recexpr(lhs);
    let rhs_expr = sexp_to_recexpr(rhs);
    let rewrites = rules_to_rewrites(rules);

    let runner = Runner::default()
        .with_expr(&lhs_expr)
        .with_expr(&rhs_expr)
        .with_iter_limit(30)
        .with_node_limit(10_000)
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
pub fn extract_simplest(expr: &Sexp, rules: &[RewriteRule]) -> Sexp {
    let recexpr = sexp_to_recexpr(expr);
    let rewrites = rules_to_rewrites(rules);

    let runner = Runner::default()
        .with_expr(&recexpr)
        .with_iter_limit(30)
        .with_node_limit(10_000)
        .run(&rewrites);

    let root = runner.roots[0];
    let extractor = Extractor::new(&runner.egraph, AstSize);
    let (_cost, best) = extractor.find_best(root);
    recexpr_to_sexp(&best)
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
