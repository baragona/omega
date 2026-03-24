//! Logic programming engine: backward-chaining Horn clause resolution
//! with occurs check and backtracking.
//!
//! This is a lightweight Prolog-style engine operating on Sexp terms.
//! - **Trail**: records variable bindings for rollback
//! - **Choice points**: when multiple clauses match, try each with backtracking
//! - **Occurs check**: prevents infinite terms (X = f(X))
//! - **Depth limit**: prevents infinite recursion

use apeiron::parser::{Sexp, Span};
use std::collections::HashMap;

/// A Horn clause: head ==> body (or head ==> true for facts)
#[derive(Debug, Clone)]
pub struct Clause {
    pub name: String,
    pub head: Sexp,
    pub body: Sexp,
}

/// Result of a logic query
#[derive(Debug, Clone)]
pub enum QueryResult {
    /// Query succeeded with variable bindings
    Success(HashMap<String, Sexp>),
    /// Query failed (no matching clauses, or occurs check violation)
    Failure,
}

impl QueryResult {
    pub fn is_success(&self) -> bool {
        matches!(self, QueryResult::Success(_))
    }
    pub fn is_failure(&self) -> bool {
        matches!(self, QueryResult::Failure)
    }
}

/// A substitution mapping metavariables to terms
type Subst = HashMap<String, Sexp>;

/// Resolve a query against a set of clauses using backward-chaining.
/// Returns the first successful substitution, or Failure.
pub fn resolve(query: &Sexp, clauses: &[Clause], max_depth: usize) -> QueryResult {
    let mut subst = Subst::new();
    if solve(query, clauses, &mut subst, max_depth, 0) {
        QueryResult::Success(subst)
    } else {
        QueryResult::Failure
    }
}

/// Backward-chaining solver with depth limit.
fn solve(
    goal: &Sexp,
    clauses: &[Clause],
    subst: &mut Subst,
    max_depth: usize,
    depth: usize,
) -> bool {
    if depth > max_depth {
        return false; // depth limit exceeded
    }

    // Apply current substitution to the goal
    let goal = apply_subst(goal, subst);

    // Check if goal is `true` or `[true]` (base case)
    if goal.is_atom("true") {
        return true;
    }
    // [true] (single-element list) is also a success
    if let Some(items) = goal.as_list() {
        if items.len() == 1 && items[0].is_atom("true") {
            return true;
        }
    }

    // Check if goal is `[and G1 G2]` — solve both conjuncts
    if let Some(items) = goal.as_list() {
        if items.len() == 3 && items[0].is_atom("and") {
            let saved = subst.clone();
            if solve(&items[1], clauses, subst, max_depth, depth + 1) {
                if solve(&items[2], clauses, subst, max_depth, depth + 1) {
                    return true;
                }
            }
            *subst = saved;
            return false;
        }
    }

    // Try each clause (choice points with backtracking)
    for (i, clause) in clauses.iter().enumerate() {
        // Freshen clause variables to avoid capture
        let fresh_clause = freshen_clause(clause, depth, i);
        let saved = subst.clone();

        // Try to unify goal with clause head
        if unify(&goal, &fresh_clause.head, subst) {
            // Unified — now solve the body
            if solve(&fresh_clause.body, clauses, subst, max_depth, depth + 1) {
                return true;
            }
        }
        // Backtrack: restore substitution
        *subst = saved;
    }

    false
}

/// Unify two Sexp terms, extending the substitution.
/// Returns true if unification succeeds, false otherwise.
/// Implements the occurs check.
fn unify(a: &Sexp, b: &Sexp, subst: &mut Subst) -> bool {
    let a = walk(a, subst);
    let b = walk(b, subst);

    match (&a, &b) {
        // Both atoms
        (Sexp::Atom(x, _), Sexp::Atom(y, _)) => {
            if x == y {
                return true;
            }
            // Metavar binding
            if x.starts_with('?') {
                return bind(x, &b, subst);
            }
            if y.starts_with('?') {
                return bind(y, &a, subst);
            }
            false
        }
        // Metavar vs compound
        (Sexp::Atom(x, _), _) if x.starts_with('?') => {
            bind(x, &b, subst)
        }
        (_, Sexp::Atom(y, _)) if y.starts_with('?') => {
            bind(y, &a, subst)
        }
        // Both lists: unify element-wise
        (Sexp::List(xs, _), Sexp::List(ys, _)) => {
            if xs.len() != ys.len() {
                return false;
            }
            for (x, y) in xs.iter().zip(ys.iter()) {
                if !unify(x, y, subst) {
                    return false;
                }
            }
            true
        }
        _ => false,
    }
}

/// Walk a term through the substitution, resolving metavariables.
fn walk(term: &Sexp, subst: &Subst) -> Sexp {
    match term {
        Sexp::Atom(name, _) if name.starts_with('?') => {
            if let Some(val) = subst.get(name) {
                walk(val, subst)
            } else {
                term.clone()
            }
        }
        Sexp::List(items, sp) => {
            let walked: Vec<Sexp> = items.iter().map(|i| walk(i, subst)).collect();
            Sexp::List(walked, *sp)
        }
        _ => term.clone(),
    }
}

/// Bind a metavariable to a term, with occurs check.
/// Returns false if the variable occurs in the term (prevents infinite terms).
fn bind(var: &str, term: &Sexp, subst: &mut Subst) -> bool {
    // Occurs check: does var appear in term?
    if occurs(var, term, subst) {
        return false;
    }
    subst.insert(var.to_string(), term.clone());
    true
}

/// Check if a metavariable occurs in a term (after walking through subst).
fn occurs(var: &str, term: &Sexp, subst: &Subst) -> bool {
    let term = walk(term, subst);
    match &term {
        Sexp::Atom(name, _) => name == var,
        Sexp::List(items, _) => items.iter().any(|i| occurs(var, i, subst)),
    }
}

/// Apply a substitution to a term, replacing all bound metavariables.
pub fn apply_subst(term: &Sexp, subst: &Subst) -> Sexp {
    walk(term, subst)
}

/// Freshen a clause's metavariables to avoid capture.
/// Each metavar `?X` becomes `?X$depth_idx`.
fn freshen_clause(clause: &Clause, depth: usize, idx: usize) -> Clause {
    let suffix = format!("${}_{}", depth, idx);
    Clause {
        name: clause.name.clone(),
        head: freshen_sexp(&clause.head, &suffix),
        body: freshen_sexp(&clause.body, &suffix),
    }
}

fn freshen_sexp(sexp: &Sexp, suffix: &str) -> Sexp {
    match sexp {
        Sexp::Atom(name, sp) if name.starts_with('?') => {
            Sexp::Atom(format!("{}{}", name, suffix), *sp)
        }
        Sexp::List(items, sp) => {
            Sexp::List(items.iter().map(|i| freshen_sexp(i, suffix)).collect(), *sp)
        }
        _ => sexp.clone(),
    }
}

/// One-way pattern matching: match pattern against a ground term.
/// Like unify but only metavars in `pattern` can bind (term is ground).
pub fn try_match(pattern: &Sexp, term: &Sexp, subst: &mut Subst) -> bool {
    let pattern = walk(pattern, subst);
    match (&pattern, term) {
        (Sexp::Atom(x, _), _) if x.starts_with('?') => {
            bind(x, term, subst)
        }
        (Sexp::Atom(x, _), Sexp::Atom(y, _)) => x == y,
        (Sexp::List(xs, _), Sexp::List(ys, _)) => {
            if xs.len() != ys.len() {
                return false;
            }
            for (x, y) in xs.iter().zip(ys.iter()) {
                if !try_match(x, y, subst) {
                    return false;
                }
            }
            true
        }
        _ => false,
    }
}

/// Convert VonNeumann rules into Horn clauses for the logic engine.
pub fn rules_to_clauses(rules: &[crate::session::VonNeumannRule]) -> Vec<Clause> {
    rules
        .iter()
        .map(|r| Clause {
            name: r.name.clone(),
            head: r.lhs.clone(),
            body: r.rhs.clone(),
        })
        .collect()
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

    fn clause(name: &str, head: Sexp, body: Sexp) -> Clause {
        Clause { name: name.to_string(), head, body }
    }

    // ── Basic unification ──

    #[test]
    fn unify_identical_atoms() {
        let mut s = Subst::new();
        assert!(unify(&atom("a"), &atom("a"), &mut s));
    }

    #[test]
    fn unify_different_atoms_fails() {
        let mut s = Subst::new();
        assert!(!unify(&atom("a"), &atom("b"), &mut s));
    }

    #[test]
    fn unify_metavar_to_atom() {
        let mut s = Subst::new();
        assert!(unify(&atom("?X"), &atom("hello"), &mut s));
        assert_eq!(s.get("?X").unwrap().as_atom(), Some("hello"));
    }

    #[test]
    fn unify_compound_terms() {
        let mut s = Subst::new();
        let a = list(vec![atom("f"), atom("?X"), atom("b")]);
        let b = list(vec![atom("f"), atom("a"), atom("?Y")]);
        assert!(unify(&a, &b, &mut s));
        assert_eq!(s.get("?X").unwrap().as_atom(), Some("a"));
        assert_eq!(s.get("?Y").unwrap().as_atom(), Some("b"));
    }

    #[test]
    fn unify_nested_structures() {
        let mut s = Subst::new();
        let a = list(vec![atom("f"), list(vec![atom("g"), atom("?X")])]);
        let b = list(vec![atom("f"), list(vec![atom("g"), atom("a")])]);
        assert!(unify(&a, &b, &mut s));
        assert_eq!(s.get("?X").unwrap().as_atom(), Some("a"));
    }

    // ── Occurs check ──

    #[test]
    fn occurs_check_simple() {
        // ?X = f(?X) should fail
        let mut s = Subst::new();
        let a = atom("?X");
        let b = list(vec![atom("f"), atom("?X")]);
        assert!(!unify(&a, &b, &mut s));
    }

    #[test]
    fn occurs_check_transitive() {
        // ?X = ?Y, then ?Y = f(?X) should fail
        let mut s = Subst::new();
        assert!(unify(&atom("?X"), &atom("?Y"), &mut s));
        let b = list(vec![atom("f"), atom("?X")]);
        assert!(!unify(&atom("?Y"), &b, &mut s));
    }

    // ── Backward chaining ──

    #[test]
    fn resolve_simple_fact() {
        let clauses = vec![
            clause("p1", list(vec![atom("parent"), atom("alice"), atom("bob")]), atom("true")),
        ];
        let query = list(vec![atom("parent"), atom("alice"), atom("bob")]);
        match resolve(&query, &clauses, 100) {
            QueryResult::Success(_) => {}
            QueryResult::Failure => panic!("Should succeed"),
        }
    }

    #[test]
    fn resolve_with_variable() {
        let clauses = vec![
            clause("p1", list(vec![atom("parent"), atom("alice"), atom("bob")]), atom("true")),
            clause("p2", list(vec![atom("parent"), atom("bob"), atom("carol")]), atom("true")),
        ];
        let query = list(vec![atom("parent"), atom("alice"), atom("?X")]);
        match resolve(&query, &clauses, 100) {
            QueryResult::Success(subst) => {
                let val = apply_subst(&atom("?X"), &subst);
                assert_eq!(val.as_atom(), Some("bob"));
            }
            QueryResult::Failure => panic!("Should succeed"),
        }
    }

    #[test]
    fn resolve_conjunction() {
        // grandparent(X,Z) :- parent(X,Y), parent(Y,Z)
        let clauses = vec![
            clause("p1", list(vec![atom("parent"), atom("alice"), atom("bob")]), atom("true")),
            clause("p2", list(vec![atom("parent"), atom("bob"), atom("carol")]), atom("true")),
            clause("gp", list(vec![atom("grandparent"), atom("?X"), atom("?Z")]),
                list(vec![atom("and"),
                    list(vec![atom("parent"), atom("?X"), atom("?Y")]),
                    list(vec![atom("parent"), atom("?Y"), atom("?Z")]),
                ])),
        ];
        let query = list(vec![atom("grandparent"), atom("alice"), atom("?Who")]);
        match resolve(&query, &clauses, 100) {
            QueryResult::Success(subst) => {
                let val = apply_subst(&atom("?Who"), &subst);
                assert_eq!(val.as_atom(), Some("carol"));
            }
            QueryResult::Failure => panic!("Should find grandparent"),
        }
    }

    #[test]
    fn resolve_no_match_fails() {
        let clauses = vec![
            clause("p1", list(vec![atom("parent"), atom("alice"), atom("bob")]), atom("true")),
        ];
        let query = list(vec![atom("parent"), atom("bob"), atom("alice")]);
        assert!(resolve(&query, &clauses, 100).is_failure());
    }

    // ── Peano arithmetic ──

    #[test]
    fn peano_add_ground() {
        // add(z, Y, Y). add(s(X), Y, s(Z)) :- add(X, Y, Z).
        let clauses = vec![
            clause("add-z",
                list(vec![atom("add"), atom("z"), atom("?Y"), atom("?Y")]),
                atom("true")),
            clause("add-s",
                list(vec![atom("add"), list(vec![atom("s"), atom("?X")]), atom("?Y"), list(vec![atom("s"), atom("?Z")])]),
                list(vec![atom("add"), atom("?X"), atom("?Y"), atom("?Z")])),
        ];

        // 0 + 1 = 1
        let q1 = list(vec![atom("add"), atom("z"), list(vec![atom("s"), atom("z")]), atom("?R")]);
        match resolve(&q1, &clauses, 100) {
            QueryResult::Success(subst) => {
                let r = apply_subst(&atom("?R"), &subst);
                assert_eq!(format!("{}", r), "[s z]");
            }
            QueryResult::Failure => panic!("0+1=1 should succeed"),
        }

        // 2 + 1 = 3
        let two = list(vec![atom("s"), list(vec![atom("s"), atom("z")])]);
        let one = list(vec![atom("s"), atom("z")]);
        let q2 = list(vec![atom("add"), two, one, atom("?R")]);
        match resolve(&q2, &clauses, 100) {
            QueryResult::Success(subst) => {
                let r = apply_subst(&atom("?R"), &subst);
                assert_eq!(format!("{}", r), "[s [s [s z]]]");
            }
            QueryResult::Failure => panic!("2+1=3 should succeed"),
        }
    }

    // ── THE BACKTRACKING ABYSS ──

    #[test]
    fn occurs_check_prevents_infinite_peano() {
        // add(X, S(0), X) has no solution — X + 1 = X is impossible.
        // Without occurs check, engine binds X = S(X) infinitely.
        let clauses = vec![
            clause("add-z",
                list(vec![atom("add"), atom("z"), atom("?Y"), atom("?Y")]),
                atom("true")),
            clause("add-s",
                list(vec![atom("add"), list(vec![atom("s"), atom("?X")]), atom("?Y"), list(vec![atom("s"), atom("?Z")])]),
                list(vec![atom("add"), atom("?X"), atom("?Y"), atom("?Z")])),
        ];

        // Query: add(X, S(0), X) — find X where X + 1 = X
        let q = list(vec![
            atom("add"),
            atom("?X"),
            list(vec![atom("s"), atom("z")]),
            atom("?X"),
        ]);

        // This MUST return Failure, not diverge
        let result = resolve(&q, &clauses, 50);
        assert!(result.is_failure(),
            "add(X, S(0), X) should fail — no X satisfies X + 1 = X");
    }

    #[test]
    fn backtracking_finds_second_solution() {
        // Two facts: parent(alice, bob) and parent(alice, carol)
        // Query: parent(alice, ?X) — should find bob first
        let clauses = vec![
            clause("p1", list(vec![atom("parent"), atom("alice"), atom("bob")]), atom("true")),
            clause("p2", list(vec![atom("parent"), atom("alice"), atom("carol")]), atom("true")),
        ];
        let query = list(vec![atom("parent"), atom("alice"), atom("?X")]);
        match resolve(&query, &clauses, 100) {
            QueryResult::Success(subst) => {
                let val = apply_subst(&atom("?X"), &subst);
                assert_eq!(val.as_atom(), Some("bob"), "Should find first match");
            }
            QueryResult::Failure => panic!("Should find at least one match"),
        }
    }

    #[test]
    fn depth_limit_prevents_infinite_recursion() {
        // ancestor(X, Y) :- ancestor(X, Z), parent(Z, Y)
        // With no base case — this would loop forever without depth limit
        let clauses = vec![
            clause("loop",
                list(vec![atom("ancestor"), atom("?X"), atom("?Y")]),
                list(vec![atom("and"),
                    list(vec![atom("ancestor"), atom("?X"), atom("?Z")]),
                    list(vec![atom("parent"), atom("?Z"), atom("?Y")]),
                ])),
        ];
        let query = list(vec![atom("ancestor"), atom("a"), atom("b")]);
        // Should fail due to depth limit, not hang
        let result = resolve(&query, &clauses, 10);
        assert!(result.is_failure());
    }
}
