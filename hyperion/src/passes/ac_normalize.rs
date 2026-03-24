//! ACNormalization pass: flatten and sort AC operator trees into canonical form.
//!
//! Given an operator `op` that is associative-commutative:
//! - `op(a, op(b, c))` → `op(a, op(b, c))` (right-nested canonical form, sorted)
//! - `op(op(c, a), b)` → `op(a, op(b, c))` (flattened, sorted, re-nested)
//!
//! This transforms O(n!) naive matching into O(n log n) sorted comparison.

use apeiron::parser::{Sexp, Span};
use std::collections::HashSet;

/// Flatten an AC operator's nested tree into a sorted canonical form.
/// `ac_ops` is the set of operator names that are AC.
pub fn ac_normalize_sexp(sexp: &Sexp, ac_ops: &HashSet<String>) -> Sexp {
    let sp = sexp.span();
    match sexp {
        Sexp::Atom(_, _) => sexp.clone(),
        Sexp::List(items, _) => {
            // First, recursively normalize children
            let normalized: Vec<Sexp> = items.iter()
                .map(|item| ac_normalize_sexp(item, ac_ops))
                .collect();

            // Check if head is an AC operator
            if let Some(head_name) = normalized.first().and_then(|s| s.as_atom()) {
                if ac_ops.contains(head_name) && normalized.len() == 3 {
                    // Flatten: collect all leaves under this AC op
                    let mut leaves = Vec::new();
                    collect_ac_leaves(&Sexp::List(normalized.clone(), sp), head_name, &mut leaves);

                    // Sort leaves by canonical string representation
                    leaves.sort_by(|a, b| format!("{}", a).cmp(&format!("{}", b)));

                    // Re-nest right-associatively: op(a, op(b, op(c, d)))
                    return right_nest(head_name, &leaves, sp);
                }
            }

            Sexp::List(normalized, sp)
        }
    }
}

/// Collect all leaf operands under a chain of the same AC operator.
fn collect_ac_leaves(sexp: &Sexp, op_name: &str, leaves: &mut Vec<Sexp>) {
    if let Some(items) = sexp.as_list() {
        if items.len() == 3 {
            if let Some(head) = items[0].as_atom() {
                if head == op_name {
                    collect_ac_leaves(&items[1], op_name, leaves);
                    collect_ac_leaves(&items[2], op_name, leaves);
                    return;
                }
            }
        }
    }
    // Not an application of op_name — it's a leaf
    leaves.push(sexp.clone());
}

/// Build a right-nested tree from sorted leaves: op(a, op(b, c))
fn right_nest(op_name: &str, leaves: &[Sexp], sp: Span) -> Sexp {
    assert!(!leaves.is_empty());
    if leaves.len() == 1 {
        return leaves[0].clone();
    }
    if leaves.len() == 2 {
        return Sexp::List(vec![
            Sexp::Atom(op_name.to_string(), sp),
            leaves[0].clone(),
            leaves[1].clone(),
        ], sp);
    }
    // Right-associate: op(first, right_nest(rest))
    let rest = right_nest(op_name, &leaves[1..], sp);
    Sexp::List(vec![
        Sexp::Atom(op_name.to_string(), sp),
        leaves[0].clone(),
        rest,
    ], sp)
}

/// Detect AC operators by examining rules for commutativity patterns.
/// A rule `[op ?x ?y] ==> [op ?y ?x]` or `[op ?x ?y] === [op ?y ?x]`
/// indicates `op` is commutative (and we assume AC when paired with ACMatching mode).
pub fn detect_ac_ops(rules: &[crate::session::VonNeumannRule]) -> HashSet<String> {
    let mut ac_ops = HashSet::new();
    for rule in rules {
        // Check: LHS = [op ?x ?y], RHS = [op ?y ?x] (same op, swapped args)
        if let (Some(lhs_items), Some(rhs_items)) = (rule.lhs.as_list(), rule.rhs.as_list()) {
            if lhs_items.len() == 3 && rhs_items.len() == 3 {
                if let (Some(lop), Some(rop)) = (lhs_items[0].as_atom(), rhs_items[0].as_atom()) {
                    if lop == rop {
                        // Check if args are swapped metavars
                        if let (Some(lx), Some(ly), Some(rx), Some(ry)) = (
                            lhs_items[1].as_atom(), lhs_items[2].as_atom(),
                            rhs_items[1].as_atom(), rhs_items[2].as_atom(),
                        ) {
                            if lx.starts_with('?') && ly.starts_with('?')
                                && lx == ry && ly == rx
                            {
                                ac_ops.insert(lop.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    ac_ops
}

/// Normalize all rules in a theory: apply AC normalization to both LHS and RHS.
pub fn ac_normalize_rules(
    rules: &mut Vec<crate::session::VonNeumannRule>,
    ac_ops: &HashSet<String>,
) {
    for rule in rules.iter_mut() {
        rule.lhs = ac_normalize_sexp(&rule.lhs, ac_ops);
        rule.rhs = ac_normalize_sexp(&rule.rhs, ac_ops);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(s: &str) -> Sexp {
        Sexp::Atom(s.to_string(), Span::default())
    }

    fn list(items: Vec<Sexp>) -> Sexp {
        Sexp::List(items, Span::default())
    }

    fn op(a: Sexp, b: Sexp) -> Sexp {
        list(vec![atom("op"), a, b])
    }

    fn ac_set() -> HashSet<String> {
        let mut s = HashSet::new();
        s.insert("op".to_string());
        s
    }

    #[test]
    fn flatten_right_nested() {
        // op(a, op(b, c)) → op(a, op(b, c)) (already sorted)
        let input = op(atom("a"), op(atom("b"), atom("c")));
        let result = ac_normalize_sexp(&input, &ac_set());
        assert_eq!(format!("{}", result), "[op a [op b c]]");
    }

    #[test]
    fn flatten_left_nested() {
        // op(op(c, a), b) → op(a, op(b, c)) (flattened + sorted)
        let input = op(op(atom("c"), atom("a")), atom("b"));
        let result = ac_normalize_sexp(&input, &ac_set());
        assert_eq!(format!("{}", result), "[op a [op b c]]");
    }

    #[test]
    fn commute_two_elements() {
        // op(b, a) → op(a, b)
        let input = op(atom("b"), atom("a"));
        let result = ac_normalize_sexp(&input, &ac_set());
        assert_eq!(format!("{}", result), "[op a b]");
    }

    #[test]
    fn deep_scramble_4_elements() {
        // op(op(d, c), op(b, a)) → op(a, op(b, op(c, d)))
        let input = op(op(atom("d"), atom("c")), op(atom("b"), atom("a")));
        let result = ac_normalize_sexp(&input, &ac_set());
        assert_eq!(format!("{}", result), "[op a [op b [op c d]]]");
    }

    #[test]
    fn six_elements_scrambled() {
        // Right: op(a, op(b, op(c, op(d, op(e, f)))))
        // Left scrambled: op(op(op(e, c), f), op(a, op(d, b)))
        let input = op(
            op(op(atom("e"), atom("c")), atom("f")),
            op(atom("a"), op(atom("d"), atom("b"))),
        );
        let result = ac_normalize_sexp(&input, &ac_set());
        assert_eq!(format!("{}", result), "[op a [op b [op c [op d [op e f]]]]]");
    }

    #[test]
    fn non_ac_operator_untouched() {
        // add(b, a) stays as add(b, a) when add is not AC
        let input = list(vec![atom("add"), atom("b"), atom("a")]);
        let result = ac_normalize_sexp(&input, &ac_set());
        assert_eq!(format!("{}", result), "[add b a]");
    }

    #[test]
    fn nested_mixed_operators() {
        // op(op(b, a), f(c)) → op(a, op(b, f(c)))  (f is not AC)
        let f_c = list(vec![atom("f"), atom("c")]);
        let input = op(op(atom("b"), atom("a")), f_c);
        let result = ac_normalize_sexp(&input, &ac_set());
        // "[f c]" sorts before "a" (ASCII: '[' < 'a'), so compound terms come first
        assert_eq!(format!("{}", result), "[op [f c] [op a b]]");
    }

    #[test]
    fn single_leaf_passthrough() {
        let input = atom("x");
        let result = ac_normalize_sexp(&input, &ac_set());
        assert_eq!(format!("{}", result), "x");
    }

    #[test]
    fn meta_variables_preserved() {
        // op(?y, ?x) → op(?x, ?y) (metavars sort alphabetically)
        let input = op(atom("?y"), atom("?x"));
        let result = ac_normalize_sexp(&input, &ac_set());
        assert_eq!(format!("{}", result), "[op ?x ?y]");
    }
}
