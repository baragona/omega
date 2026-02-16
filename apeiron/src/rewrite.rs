use std::collections::HashMap;

use crate::arena::Arena;
use crate::node::{OpCode, Port, Ptr};
use crate::parser::Sexp;
use crate::readback::Term;

// ---------------------------------------------------------------------------
// Sexp utilities (def expansion, term↔sexp conversion)
// ---------------------------------------------------------------------------

/// Expand all def references in an Sexp.
pub fn expand_defs(sexp: &Sexp, defs: &HashMap<String, Sexp>) -> Sexp {
    match sexp {
        Sexp::Atom(name, _) => {
            if let Some(def_body) = defs.get(name.as_str()) {
                expand_defs(def_body, defs)
            } else {
                sexp.clone()
            }
        }
        Sexp::List(items, span) => {
            let new_items: Vec<Sexp> = items
                .iter()
                .map(|item| expand_defs(item, defs))
                .collect();
            Sexp::List(new_items, *span)
        }
    }
}

/// Convert a readback Term to Sexp (for display/comparison).
pub fn term_to_sexp(term: &Term) -> Sexp {
    let s = crate::parser::Span::default();
    match term {
        Term::Var(name) => Sexp::Atom(name.clone(), s),
        Term::Const(name) => Sexp::Atom(name.clone(), s),
        Term::App(func, args) => {
            let mut items = vec![term_to_sexp(func)];
            for arg in args {
                items.push(term_to_sexp(arg));
            }
            Sexp::List(items, s)
        }
        Term::Binder { kind, var, body } => Sexp::List(
            vec![
                Sexp::Atom(kind.clone(), s),
                Sexp::Atom(var.clone(), s),
                term_to_sexp(body),
            ],
            s,
        ),
        Term::Future => Sexp::Atom("?".into(), s),
        Term::Wire(id) => Sexp::Atom(format!("<wire:{}>", id), s),
        Term::Erased => Sexp::Atom("*".into(), s),
    }
}

// ---------------------------------------------------------------------------
// Graph-level pattern matching and rewriting
// ---------------------------------------------------------------------------

/// A compiled pattern for matching against graph nodes.
#[derive(Debug, Clone)]
pub enum Pattern {
    /// Match a specific symbol with sub-patterns for each aux port.
    Sym { name: String, args: Vec<Pattern> },
    /// Match anything, bind the node to a named meta-variable.
    MetaVar(String),
}

/// A compiled rewrite rule for graph-level application.
#[derive(Debug, Clone)]
pub struct GraphRule {
    pub name: String,
    /// The outermost symbol name (e.g. "plus").
    pub head: String,
    /// Patterns for each argument of the head symbol.
    pub arg_patterns: Vec<Pattern>,
    /// RHS template (Sexp) — built as a fresh subgraph when applied.
    pub rhs: Sexp,
}

/// Compile a single @rule (LHS ==> RHS) into a GraphRule.
pub fn compile_rule(name: &str, lhs: &Sexp, rhs: &Sexp) -> Option<GraphRule> {
    match lhs {
        Sexp::List(items, _) if !items.is_empty() => {
            let head = items[0].as_atom()?.to_string();
            let arg_patterns = items[1..].iter().map(compile_pattern).collect();
            Some(GraphRule {
                name: name.to_string(),
                head,
                arg_patterns,
                rhs: rhs.clone(),
            })
        }
        Sexp::Atom(name_str, _) if !name_str.starts_with('?') => {
            // Atom LHS: treat as a 0-arity symbol (e.g., inverse rules)
            Some(GraphRule {
                name: name.to_string(),
                head: name_str.clone(),
                arg_patterns: vec![],
                rhs: rhs.clone(),
            })
        }
        _ => None,
    }
}

fn compile_pattern(sexp: &Sexp) -> Pattern {
    match sexp {
        Sexp::Atom(name, _) if name.starts_with('?') => {
            Pattern::MetaVar(name[1..].to_string())
        }
        Sexp::Atom(name, _) => Pattern::Sym {
            name: name.clone(),
            args: vec![],
        },
        Sexp::List(items, _) if !items.is_empty() => {
            let head = items[0].as_atom().unwrap_or("?").to_string();
            let args = items[1..].iter().map(compile_pattern).collect();
            Pattern::Sym { name: head, args }
        }
        _ => Pattern::MetaVar("_".into()),
    }
}

/// Try to match a pattern against a live node in the arena.
fn match_pattern(
    arena: &Arena,
    pattern: &Pattern,
    ptr: Ptr,
    bindings: &mut HashMap<String, Ptr>,
) -> bool {
    match pattern {
        Pattern::MetaVar(name) => {
            bindings.insert(name.clone(), ptr);
            true
        }
        Pattern::Sym { name, args } => {
            let node = match arena.get(ptr) {
                Some(n) => n,
                None => return false,
            };
            match &node.kind {
                OpCode::Sym {
                    name: node_name,
                    arity,
                } => {
                    if node_name != name || *arity as usize != args.len() {
                        return false;
                    }
                    for (i, arg_pat) in args.iter().enumerate() {
                        let port = node.ports[i + 1];
                        if !port.is_connected() {
                            return false;
                        }
                        if !match_pattern(arena, arg_pat, port.target, bindings) {
                            return false;
                        }
                    }
                    true
                }
                _ => false,
            }
        }
    }
}

/// Collect all structurally matched nodes (not meta-var bindings) for freeing.
fn collect_matched_nodes(arena: &Arena, pattern: &Pattern, ptr: Ptr, out: &mut Vec<Ptr>) {
    match pattern {
        Pattern::MetaVar(_) => {} // keep — reused in RHS
        Pattern::Sym { args, .. } => {
            out.push(ptr);
            if let Some(node) = arena.get(ptr) {
                for (i, arg_pat) in args.iter().enumerate() {
                    if let Some(port) = node.ports.get(i + 1) {
                        if port.is_connected() {
                            collect_matched_nodes(arena, arg_pat, port.target, out);
                        }
                    }
                }
            }
        }
    }
}

/// Count how many times each meta-variable appears in an RHS template.
fn count_meta_uses(sexp: &Sexp, counts: &mut HashMap<String, usize>) {
    match sexp {
        Sexp::Atom(name, _) if name.starts_with('?') => {
            *counts.entry(name[1..].to_string()).or_default() += 1;
        }
        Sexp::Atom(_, _) => {}
        Sexp::List(items, _) => {
            for item in items {
                count_meta_uses(item, counts);
            }
        }
    }
}

/// Build a Dup fan chain: original node → N copies via (N-1) Dup nodes.
/// Returns a Vec of Ports, each pointing to an aux port of a Dup (or the original).
fn build_meta_fan(arena: &mut Arena, original: Ptr, count: usize) -> Vec<Port> {
    assert!(count >= 2);
    let mut leaves = Vec::with_capacity(count);
    let mut current = Port::new(original, 0);
    for _ in 0..(count - 1) {
        let label = arena.fresh_dup_label();
        let dup = arena.spawn(OpCode::Dup { label });
        arena.connect(dup, 0, current.target, current.slot);
        leaves.push(Port::new(dup, 1));
        current = Port::new(dup, 2);
    }
    leaves.push(current);
    leaves
}

/// Build a fresh subgraph from an RHS Sexp template, using meta-var bindings.
/// Handles multi-use meta-variables by creating Dup fan trees.
///
/// Returns the principal port of the root node.
fn build_rhs(arena: &mut Arena, rhs: &Sexp, bindings: &HashMap<String, Ptr>) -> Port {
    // Count meta-variable uses to detect multi-use
    let mut counts = HashMap::new();
    count_meta_uses(rhs, &mut counts);

    // For multi-use metas, create Dup fan trees; for single-use, direct port
    let mut meta_ports: HashMap<String, Vec<Port>> = HashMap::new();
    for (name, count) in &counts {
        if let Some(&ptr) = bindings.get(name.as_str()) {
            if *count == 1 {
                meta_ports.insert(name.clone(), vec![Port::new(ptr, 0)]);
            } else {
                let ports = build_meta_fan(arena, ptr, *count);
                meta_ports.insert(name.clone(), ports);
            }
        }
    }

    build_rhs_inner(arena, rhs, bindings, &mut meta_ports)
}

fn build_rhs_inner(
    arena: &mut Arena,
    rhs: &Sexp,
    bindings: &HashMap<String, Ptr>,
    meta_ports: &mut HashMap<String, Vec<Port>>,
) -> Port {
    match rhs {
        Sexp::Atom(name, _) if name.starts_with('?') => {
            let var = &name[1..];
            // Pop the next available port for this meta-variable
            if let Some(ports) = meta_ports.get_mut(var) {
                if let Some(port) = ports.pop() {
                    return port;
                }
            }
            // Fallback: unbound meta → placeholder symbol
            if let Some(&ptr) = bindings.get(var) {
                Port::new(ptr, 0)
            } else {
                let sym = arena.spawn(OpCode::Sym {
                    name: name.clone(),
                    arity: 0,
                });
                Port::new(sym, 0)
            }
        }
        Sexp::Atom(name, _) => {
            let sym = arena.spawn(OpCode::Sym {
                name: name.clone(),
                arity: 0,
            });
            Port::new(sym, 0)
        }
        Sexp::List(items, _) if !items.is_empty() => {
            let head_name = items[0].as_atom().unwrap_or("?");

            // Single-element list: unwrap
            if items.len() == 1 {
                return build_rhs_inner(arena, &items[0], bindings, meta_ports);
            }

            // Head is a meta-var — build as curried App chain
            if head_name.starts_with('?') {
                let mut result = build_rhs_inner(arena, &items[0], bindings, meta_ports);
                for arg_sexp in &items[1..] {
                    let arg_port = build_rhs_inner(arena, arg_sexp, bindings, meta_ports);
                    let app = arena.spawn(OpCode::App);
                    arena.connect(app, 1, arg_port.target, arg_port.slot);
                    arena.connect(app, 0, result.target, result.slot);
                    result = Port::new(app, 2);
                }
                return result;
            }

            // "app" keyword — build as OpCode::App (enables beta reduction)
            if head_name == "app" && items.len() >= 3 {
                let mut result = build_rhs_inner(arena, &items[1], bindings, meta_ports);
                for arg_sexp in &items[2..] {
                    let arg_port = build_rhs_inner(arena, arg_sexp, bindings, meta_ports);
                    let app = arena.spawn(OpCode::App);
                    arena.connect(app, 1, arg_port.target, arg_port.slot);
                    arena.connect(app, 0, result.target, result.slot);
                    result = Port::new(app, 2);
                }
                return result;
            }

            let args: Vec<&Sexp> = items[1..].iter().collect();
            let arity = args.len() as u8;
            let sym = arena.spawn(OpCode::Sym {
                name: head_name.to_string(),
                arity,
            });

            for (i, arg) in args.iter().enumerate() {
                let arg_port = build_rhs_inner(arena, arg, bindings, meta_ports);
                arena.connect(
                    sym,
                    (i + 1) as u8,
                    arg_port.target,
                    arg_port.slot,
                );
            }

            Port::new(sym, 0)
        }
        _ => {
            let sym = arena.spawn(OpCode::Sym {
                name: "nil".into(),
                arity: 0,
            });
            Port::new(sym, 0)
        }
    }
}

/// Scan all live Sym nodes and try to apply one rewrite rule.
///
/// Returns `true` if a rewrite fired (caller should re-run physics).
pub fn try_rewrite_scan(arena: &mut Arena, rules: &[GraphRule]) -> bool {
    if rules.is_empty() {
        return false;
    }

    // Snapshot live Sym nodes (name, arity, ptr)
    let capacity = arena.node_capacity();
    let candidates: Vec<(Ptr, String, u8)> = (0..capacity)
        .filter_map(|i| {
            let ptr = Ptr(i as u32);
            let node = arena.get(ptr)?;
            match &node.kind {
                OpCode::Sym { name, arity } if name != "ROOT" => {
                    Some((ptr, name.clone(), *arity))
                }
                _ => None,
            }
        })
        .collect();

    for (ptr, sym_name, arity) in &candidates {
        // Node may have been freed by a previous rewrite in this scan
        if arena.get(*ptr).is_none() {
            continue;
        }

        for rule in rules {
            if rule.head != *sym_name || rule.arg_patterns.len() != *arity as usize {
                continue;
            }

            // Try to match all arg patterns
            let mut bindings = HashMap::new();
            let node = match arena.get(*ptr) {
                Some(n) => n,
                None => break,
            };

            let mut ok = true;
            for (i, pat) in rule.arg_patterns.iter().enumerate() {
                let port = node.ports[i + 1];
                if !port.is_connected() {
                    ok = false;
                    break;
                }
                if !match_pattern(arena, pat, port.target, &mut bindings) {
                    ok = false;
                    break;
                }
            }
            if !ok {
                continue;
            }

            // ---- Match succeeded — apply the rewrite ----

            // 1. Save context (what was connected to head's principal port)
            let ctx_port = arena.port(*ptr, 0);

            // 2. Collect structurally matched nodes to free
            let mut matched = vec![*ptr];
            // Re-read node (borrow checker)
            let node = arena.get(*ptr).unwrap();
            for (i, pat) in rule.arg_patterns.iter().enumerate() {
                let port = node.ports[i + 1];
                if port.is_connected() {
                    collect_matched_nodes(arena, pat, port.target, &mut matched);
                }
            }

            // 3. Build RHS subgraph (rewires bindings away from old parents)
            let rhs_port = build_rhs(arena, &rule.rhs, &bindings);

            // 4. Rewire: connect RHS root to context
            if ctx_port.is_connected() {
                arena.connect(
                    rhs_port.target,
                    rhs_port.slot,
                    ctx_port.target,
                    ctx_port.slot,
                );
            }

            // 5. Free consumed nodes
            for node_ptr in matched {
                arena.free(node_ptr);
            }

            return true; // one rewrite per scan pass
        }
    }

    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;
    #[allow(unused_imports)]
    use crate::physics;
    use crate::readback;

    /// Helper: build a Sym-tree term from Sexp using known ops.
    fn build_sym_tree(arena: &mut Arena, sexp: &Sexp) -> Ptr {
        let root = arena.spawn(OpCode::Sym {
            name: "ROOT".into(),
            arity: 1,
        });
        let port = build_sym_term(arena, sexp);
        arena.connect(root, 1, port.target, port.slot);
        root
    }

    fn build_sym_term(arena: &mut Arena, sexp: &Sexp) -> Port {
        match sexp {
            Sexp::Atom(name, _) => {
                let sym = arena.spawn(OpCode::Sym {
                    name: name.clone(),
                    arity: 0,
                });
                Port::new(sym, 0)
            }
            Sexp::List(items, _) if !items.is_empty() => {
                let head = items[0].as_atom().unwrap_or("?");
                let args: Vec<&Sexp> = items[1..].iter().collect();
                let arity = args.len() as u8;
                let sym = arena.spawn(OpCode::Sym {
                    name: head.to_string(),
                    arity,
                });
                for (i, arg) in args.iter().enumerate() {
                    let p = build_sym_term(arena, arg);
                    arena.connect(sym, (i + 1) as u8, p.target, p.slot);
                }
                Port::new(sym, 0)
            }
            _ => {
                let sym = arena.spawn(OpCode::Sym {
                    name: "nil".into(),
                    arity: 0,
                });
                Port::new(sym, 0)
            }
        }
    }

    #[test]
    fn rewrite_plus_z_n() {
        let rules = vec![compile_rule(
            "plus-z",
            &parser::parse("[plus z ?n]").unwrap()[0],
            &parser::parse("?n").unwrap()[0],
        )
        .unwrap()];

        let mut arena = Arena::new();
        let input = parser::parse("[plus z [s z]]").unwrap().remove(0);
        let root = build_sym_tree(&mut arena, &input);

        assert!(try_rewrite_scan(&mut arena, &rules));

        let result_port = arena.port(root, 1);
        let term = readback::readback(&arena, result_port.target);
        assert_eq!(format!("{}", term), "[s z]");
    }

    #[test]
    fn rewrite_plus_s() {
        let rules = vec![
            compile_rule(
                "plus-z",
                &parser::parse("[plus z ?n]").unwrap()[0],
                &parser::parse("?n").unwrap()[0],
            )
            .unwrap(),
            compile_rule(
                "plus-s",
                &parser::parse("[plus [s ?n] ?m]").unwrap()[0],
                &parser::parse("[s [plus ?n ?m]]").unwrap()[0],
            )
            .unwrap(),
        ];

        let mut arena = Arena::new();
        let input = parser::parse("[plus [s [s z]] [s [s z]]]")
            .unwrap()
            .remove(0);
        let root = build_sym_tree(&mut arena, &input);

        // Run rewrite loop until fixpoint
        let mut steps = 0;
        while try_rewrite_scan(&mut arena, &rules) {
            steps += 1;
            assert!(steps < 100, "rewrite loop diverged");
        }

        let result_port = arena.port(root, 1);
        let term = readback::readback(&arena, result_port.target);
        assert_eq!(format!("{}", term), "[s [s [s [s z]]]]");
        assert_eq!(steps, 3); // 2 plus-s + 1 plus-z
    }
}
