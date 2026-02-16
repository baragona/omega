use std::collections::{HashMap, HashSet};

use crate::arena::Arena;
use crate::node::{OpCode, Port, Ptr};
use crate::parser::Sexp;

/// Build environment: tracks variable scopes and their usage sites.
///
/// When we enter a `[lam x body]`, we push `x` with an empty usage list.
/// Every time `x` appears in `body`, we record a dangling port that needs
/// the variable's value. When we leave the lam scope, we handle the usages:
/// - 0 uses → Erase node on Lam's var port
/// - 1 use → direct wire
/// - N uses → balanced tree of Dup nodes
pub struct BuildEnv {
    /// Stack of scopes. Each scope maps variable names to their usage ports.
    scopes: Vec<HashMap<String, Vec<DanglingPort>>>,
    /// Known operators — built as Sym{arity:N} instead of curried App.
    pub known_ops: HashSet<String>,
    /// Named scope IDs for barrier nodes.
    pub scope_ids: HashMap<String, u32>,
}

/// A dangling port that needs to be wired to a variable's value.
#[derive(Clone, Debug)]
struct DanglingPort {
    /// The node that needs the variable
    node: Ptr,
    /// The slot on that node
    slot: u8,
}

impl BuildEnv {
    pub fn new() -> Self {
        BuildEnv {
            scopes: vec![HashMap::new()],
            known_ops: HashSet::new(),
            scope_ids: HashMap::new(),
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) -> HashMap<String, Vec<DanglingPort>> {
        self.scopes.pop().expect("scope stack underflow: push/pop imbalance")
    }

    /// Register a new variable in the current scope.
    fn bind(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), Vec::new());
        }
    }

    /// Record a usage of a variable. Returns true if found.
    fn use_var(&mut self, name: &str, node: Ptr, slot: u8) -> bool {
        // Search from innermost scope outward
        for scope in self.scopes.iter_mut().rev() {
            if let Some(usages) = scope.get_mut(name) {
                usages.push(DanglingPort { node, slot });
                return true;
            }
        }
        false
    }

    /// Take the usages for a variable from the current scope.
    fn take_usages(&mut self, name: &str) -> Vec<DanglingPort> {
        if let Some(scope) = self.scopes.last_mut() {
            scope.remove(name).unwrap_or_default()
        } else {
            Vec::new()
        }
    }
}

/// Build a rooted term: wraps the result in a ROOT node so the result
/// survives interaction net reductions. Returns the Ptr of the ROOT node.
/// After physics, read ROOT.ports[1] to get the result.
pub fn build_rooted(arena: &mut Arena, env: &mut BuildEnv, sexp: &Sexp) -> Ptr {
    arena.begin_building();
    let root = arena.spawn(OpCode::Sym {
        name: "ROOT".into(),
        arity: 1,
    });
    let term_port = build_term(arena, env, sexp);
    arena.connect(root, 1, term_port.target, term_port.slot);
    arena.end_building();
    root
}

/// Build a term from an S-expression, returning the principal port of the root node.
///
/// The returned `Port` has `target` = the root node and `slot` = 0 (its principal port).
/// The caller should wire this port into whatever context needs it.
pub fn build_term(arena: &mut Arena, env: &mut BuildEnv, sexp: &Sexp) -> Port {
    match sexp {
        Sexp::Atom(name, _) => build_atom(arena, env, name),
        Sexp::List(items, _) if items.is_empty() => {
            // Empty list → unit/nil
            let sym = arena.spawn(OpCode::Sym {
                name: "nil".into(),
                arity: 0,
            });
            Port::new(sym, 0)
        }
        Sexp::List(items, _) => {
            let head = items[0].as_atom().unwrap_or("");

            match head {
                "lam" | "Lam" | "lambda" if items.len() >= 3 => {
                    build_lam(arena, env, &items[1], &items[2])
                }
                "app" | "App" if items.len() >= 3 => build_app(arena, env, &items[1], &items[2]),

                // Barrier: [barrier ScopeName expr] or [box ScopeName expr]
                "barrier" | "box" if items.len() >= 3 => {
                    let scope_name = items[1].as_atom().unwrap_or("?");
                    let scope_id = env
                        .scope_ids
                        .get(scope_name)
                        .copied()
                        .unwrap_or(0);
                    let barrier = arena.spawn(OpCode::Barrier { scope: scope_id });

                    // Capture active pair count before building inner body
                    let pairs_before = arena.active_pairs.len();

                    let inner_port = build_term(arena, env, &items[2]);
                    arena.connect(
                        barrier,
                        1,
                        inner_port.target,
                        inner_port.slot,
                    );

                    // If scope is inactive, suspend any active pairs created
                    // during the inner build — the barrier is opaque.
                    if !arena.active_scopes.contains(&scope_id) {
                        let suspended: Vec<_> =
                            arena.active_pairs.drain(pairs_before..).collect();
                        if !suspended.is_empty() {
                            arena
                                .suspended_pairs
                                .entry(scope_id)
                                .or_default()
                                .extend(suspended);
                        }
                    }

                    Port::new(barrier, 0)
                }

                // De Bruijn variable: [var N]
                "var" if items.len() >= 2 => {
                    let idx: u32 = items[1]
                        .as_atom()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    let sym = arena.spawn(OpCode::Sym {
                        name: format!("${}", idx),
                        arity: 0,
                    });
                    Port::new(sym, 0)
                }

                // Constant: [const name] — just a named symbol
                "const" if items.len() >= 2 => {
                    let name = items[1].as_atom().unwrap_or("?").to_string();
                    let sym = arena.spawn(OpCode::Sym {
                        name,
                        arity: 0,
                    });
                    Port::new(sym, 0)
                }

                _ => {
                    // Check if head is a known operator → build as Sym with arity
                    if items.len() > 1
                        && items[0]
                            .as_atom()
                            .map_or(false, |n| env.known_ops.contains(n))
                    {
                        let name = items[0].as_atom().unwrap().to_string();
                        let arity = (items.len() - 1) as u8;
                        let sym = arena.spawn(OpCode::Sym { name, arity });
                        for (i, arg) in items[1..].iter().enumerate() {
                            let arg_port = build_term(arena, env, arg);
                            arena.connect(
                                sym,
                                (i + 1) as u8,
                                arg_port.target,
                                arg_port.slot,
                                    );
                        }
                        Port::new(sym, 0)
                    } else if items.len() == 1 {
                        // Generic application: head applied to args
                        build_term(arena, env, &items[0])
                    } else {
                        // Curried application: [f a b] = [app [app f a] b]
                        let mut result = build_term(arena, env, &items[0]);
                        for arg in &items[1..] {
                            let arg_port = build_term(arena, env, arg);
                            let app = arena.spawn(OpCode::App);

                            arena.connect(
                                app,
                                1,
                                arg_port.target,
                                arg_port.slot,
                                    );
                            arena.connect(
                                app,
                                0,
                                result.target,
                                result.slot,
                                    );

                            // The result of this application is App's port 2
                            result = Port::new(app, 2);
                        }
                        result
                    }
                }
            }
        }
    }
}

fn build_atom(arena: &mut Arena, env: &mut BuildEnv, name: &str) -> Port {
    if name.starts_with('?') {
        // Meta-variable / future
        let sym = arena.spawn(OpCode::Future);
        Port::new(sym, 0)
    } else {
        // Check if it's a bound variable
        // We need a placeholder node for the variable usage
        let placeholder = arena.spawn(OpCode::Sym {
            name: name.to_string(),
            arity: 0,
        });
        if env.use_var(name, placeholder, 0) {
            // It's a bound variable — the placeholder will be wired later
            Port::new(placeholder, 0)
        } else {
            // It's a free constant
            Port::new(placeholder, 0)
        }
    }
}

fn build_lam(arena: &mut Arena, env: &mut BuildEnv, var: &Sexp, body: &Sexp) -> Port {
    let var_name = var.as_atom().unwrap_or("_");

    // Spawn the Lam node
    let lam = arena.spawn(OpCode::Lam);

    // Enter scope and bind the variable
    env.push_scope();
    env.bind(var_name);

    // Build the body (may record usages of var_name)
    let body_port = build_term(arena, env, body);

    // Wire body to Lam's body port (slot 2)
    arena.connect(lam, 2, body_port.target, body_port.slot);

    // Handle variable usages
    let usages = env.take_usages(var_name);
    env.pop_scope();

    wire_var_usages(arena, lam, 1, usages);

    // Return Lam's principal port
    Port::new(lam, 0)
}

fn build_app(arena: &mut Arena, env: &mut BuildEnv, fun: &Sexp, arg: &Sexp) -> Port {
    let app = arena.spawn(OpCode::App);

    let fun_port = build_term(arena, env, fun);
    let arg_port = build_term(arena, env, arg);

    // Wire function to App's principal port (will auto-enqueue if fun is Lam)
    arena.connect(app, 0, fun_port.target, fun_port.slot);
    // Wire argument to App's arg port (slot 1)
    arena.connect(app, 1, arg_port.target, arg_port.slot);

    // Return App's result port (slot 2)
    Port::new(app, 2)
}

/// Wire a binder's variable port to the usage sites.
///
/// Each usage is a placeholder Sym node that was created during body construction.
/// The placeholder is already wired into the body graph (e.g., app.0 ↔ placeholder.0).
/// We bypass the placeholder: read what it's connected to, wire binder.var directly
/// to that body node, then free the placeholder.
///
/// - 0 usages → spawn Erase on the var port
/// - 1 usage → bypass placeholder, direct wire
/// - N usages → bypass all placeholders, build Dup tree
fn wire_var_usages(arena: &mut Arena, binder: Ptr, var_slot: u8, usages: Vec<DanglingPort>) {
    match usages.len() {
        0 => {
            let erase = arena.spawn(OpCode::Erase);
            arena.connect(binder, var_slot, erase, 0);
        }
        1 => {
            let u = &usages[0];
            // Read what the placeholder is connected to in the body graph
            let body_conn = arena.port(u.node, u.slot);
            if body_conn.is_connected() {
                // Bypass: wire binder.var directly to the body node
                arena.connect(
                    binder,
                    var_slot,
                    body_conn.target,
                    body_conn.slot,
                );
                arena.free(u.node); // free the placeholder
            } else {
                // Placeholder not connected to anything yet — wire directly
                arena.connect(binder, var_slot, u.node, u.slot);
            }
        }
        _ => {
            // Bypass all placeholders and collect the body-side ports
            let mut body_ports = Vec::new();
            for u in &usages {
                let body_conn = arena.port(u.node, u.slot);
                if body_conn.is_connected() {
                    body_ports.push(DanglingPort {
                        node: body_conn.target,
                        slot: body_conn.slot,
                    });
                    arena.free(u.node); // free placeholder
                } else {
                    body_ports.push(DanglingPort {
                        node: u.node,
                        slot: u.slot,
                    });
                }
            }
            let fan_label = arena.fresh_dup_label();
            let root_port = build_dup_tree(arena, &body_ports, fan_label);
            arena.connect(
                binder,
                var_slot,
                root_port.target,
                root_port.slot,
            );
        }
    }
}

/// Build a balanced binary tree of Dup nodes to fan out to N usage sites.
/// All Dup nodes in the same fan tree share the same label, so they
/// annihilate correctly when meeting their counterparts.
/// Returns the principal port of the root Dup node.
fn build_dup_tree(arena: &mut Arena, usages: &[DanglingPort], label: u32) -> Port {
    assert!(!usages.is_empty());

    if usages.len() == 1 {
        // Single usage: return directly
        Port::new(usages[0].node, usages[0].slot)
    } else if usages.len() == 2 {
        // Base case: single Dup node
        let dup = arena.spawn(OpCode::Dup { label });
        arena.connect(dup, 1, usages[0].node, usages[0].slot);
        arena.connect(dup, 2, usages[1].node, usages[1].slot);
        Port::new(dup, 0)
    } else {
        // Recursive case: split usages in half
        let mid = usages.len() / 2;
        let left_port = build_dup_tree(arena, &usages[..mid], label);
        let right_port = build_dup_tree(arena, &usages[mid..], label);

        let dup = arena.spawn(OpCode::Dup { label });
        arena.connect(dup, 1, left_port.target, left_port.slot);
        arena.connect(dup, 2, right_port.target, right_port.slot);
        Port::new(dup, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;
    use crate::physics::{self, PhysicsConfig};

    #[test]
    fn build_identity() {
        let mut arena = Arena::new();
        let mut env = BuildEnv::new();

        let sexp = &parser::parse("[lam x x]").unwrap()[0];
        let root = build_term(&mut arena, &mut env, sexp);

        // Should have: Lam node with var↔body (identity)
        let lam_node = arena.get(root.target).unwrap();
        assert_eq!(lam_node.kind, OpCode::Lam);
    }

    #[test]
    fn build_and_reduce_identity_applied() {
        let mut arena = Arena::new();
        let mut env = BuildEnv::new();

        // [app [lam x x] y]
        let sexp = &parser::parse("[app [lam x x] y]").unwrap()[0];
        let root = build_term(&mut arena, &mut env, sexp);

        // Run physics
        let result = physics::run(&mut arena, &PhysicsConfig::default());
        assert_eq!(result.halted_reason, physics::HaltReason::NormalForm);

        // The result port should point to the 'y' node
        let _result_port = arena.port(root.target, root.slot);
        // After beta reduction, root.target (App) is freed.
        // The result should be y connected to whatever was on App.2
        // After beta, app is freed. The result is y connected to ROOT via build_rooted.
    }

    #[test]
    fn build_nonlinear_two_uses() {
        let mut arena = Arena::new();
        let mut env = BuildEnv::new();

        // [lam x [app x x]] — variable x used twice
        let sexp = &parser::parse("[lam x [app x x]]").unwrap()[0];
        let root = build_term(&mut arena, &mut env, sexp);

        let lam_node = arena.get(root.target).unwrap();
        assert_eq!(lam_node.kind, OpCode::Lam);

        // Lam's var port (1) should be connected to a Dup node
        let var_port = lam_node.ports[1];
        assert!(var_port.is_connected());
        let dup_node = arena.get(var_port.target).unwrap();
        assert!(matches!(dup_node.kind, OpCode::Dup { .. }));
    }

    #[test]
    fn build_erased_var() {
        let mut arena = Arena::new();
        let mut env = BuildEnv::new();

        // [lam x y] — variable x not used
        let sexp = &parser::parse("[lam x y]").unwrap()[0];
        let root = build_term(&mut arena, &mut env, sexp);

        let lam_node = arena.get(root.target).unwrap();
        // Lam's var port (1) should be connected to an Erase node
        let var_port = lam_node.ports[1];
        assert!(var_port.is_connected());
        let erase_node = arena.get(var_port.target).unwrap();
        assert_eq!(erase_node.kind, OpCode::Erase);
    }

    #[test]
    fn build_three_uses() {
        let mut arena = Arena::new();
        let mut env = BuildEnv::new();

        // [lam x [app [app x x] x]] — variable x used 3 times
        let sexp = &parser::parse("[lam x [app [app x x] x]]").unwrap()[0];
        let _root = build_term(&mut arena, &mut env, sexp);

        // Should have 2 Dup nodes (balanced tree for 3 usages)
        let dup_count = (0..arena.live_count() + 10)
            .filter(|&i| {
                arena
                    .get(Ptr(i as u32))
                    .map_or(false, |n| matches!(n.kind, OpCode::Dup { .. }))
            })
            .count();
        assert_eq!(dup_count, 2); // tree: Dup(Dup(x,x), x) = 2 dups
    }
}
