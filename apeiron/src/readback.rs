use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::arena::Arena;
use crate::node::{OpCode, Ptr};

/// A readback term — the user-facing representation.
#[derive(Debug, Clone)]
pub enum Term {
    /// A named variable.
    Var(String),
    /// A constant/symbol.
    Const(String),
    /// Application: function applied to arguments.
    App(Box<Term>, Vec<Term>),
    /// A binder: [lam x body].
    Binder {
        kind: String,
        var: String,
        body: Box<Term>,
    },
    /// An unresolved future (meta-variable).
    Future,
    /// A raw wire reference (for debugging).
    Wire(u32),
    /// Erased term.
    Erased,
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Term::Var(name) => write!(f, "{}", name),
            Term::Const(name) => write!(f, "{}", name),
            Term::App(func, args) => {
                write!(f, "[{}", func)?;
                for arg in args {
                    write!(f, " {}", arg)?;
                }
                write!(f, "]")
            }
            Term::Binder { kind, var, body } => {
                write!(f, "[{} {} {}]", kind, var, body)
            }
            Term::Future => write!(f, "?"),
            Term::Wire(ptr) => write!(f, "<wire:{}>", ptr),
            Term::Erased => write!(f, "*"),
        }
    }
}

/// State for readback traversal.
struct ReadbackState {
    visited: HashSet<Ptr>,
    /// Variable names keyed by (target_node, target_slot) — the far end of the var wire.
    var_names: HashMap<(Ptr, u8), String>,
    var_counter: usize,
}

impl ReadbackState {
    fn new() -> Self {
        ReadbackState {
            visited: HashSet::new(),
            var_names: HashMap::new(),
            var_counter: 0,
        }
    }

    fn fresh_var(&mut self) -> String {
        let name = match self.var_counter {
            0 => "x".to_string(),
            1 => "y".to_string(),
            2 => "z".to_string(),
            3 => "w".to_string(),
            n => format!("x{}", n),
        };
        self.var_counter += 1;
        name
    }
}

/// Readback: walk the graph from root and construct a Term.
pub fn readback(arena: &Arena, root: Ptr) -> Term {
    let mut state = ReadbackState::new();
    readback_inner(arena, root, &mut state)
}

/// Readback starting from a specific port (target, slot).
pub fn readback_from_port(arena: &Arena, target: Ptr, slot: u8) -> Term {
    // Follow the port to get the actual term root
    if let Some(node) = arena.get(target) {
        if let Some(port) = node.ports.get(slot as usize) {
            if port.is_connected() {
                return readback(arena, port.target);
            }
        }
    }
    Term::Wire(target.0)
}

/// Follow a port and return the term. If the destination port is a known variable, return the name.
fn readback_port(arena: &Arena, port: crate::node::Port, state: &mut ReadbackState) -> Term {
    if !port.is_connected() {
        return Term::Wire(u32::MAX);
    }
    // Check if the destination (target, slot) is a known variable binding
    if let Some(name) = state.var_names.get(&(port.target, port.slot)) {
        return Term::Var(name.clone());
    }
    readback_inner(arena, port.target, state)
}

/// Trace a Lam's var port through Dup fan trees, marking all copy ports
/// with the variable name. This allows readback to display non-linear variables
/// (used 2+ times) as their name instead of showing raw Dup nodes.
fn trace_var_dups(arena: &Arena, state: &mut ReadbackState, lam_ptr: Ptr, var_name: &str) {
    let var_port = arena.port(lam_ptr, 1);
    if !var_port.is_connected() {
        return;
    }
    // If the var port connects to a Dup's principal (slot 0), trace the fan tree
    if var_port.slot == 0 {
        if let Some(node) = arena.get(var_port.target) {
            if matches!(node.kind, OpCode::Dup { .. }) {
                trace_dup_tree(arena, state, var_port.target, var_name);
            }
        }
    }
}

/// Recursively mark Dup copy ports (slots 1 and 2) with a variable name.
/// If a copy port chains to another Dup's principal, recurse into that Dup.
fn trace_dup_tree(arena: &Arena, state: &mut ReadbackState, dup_ptr: Ptr, var_name: &str) {
    // Mark both copy ports
    state
        .var_names
        .insert((dup_ptr, 1), var_name.to_string());
    state
        .var_names
        .insert((dup_ptr, 2), var_name.to_string());

    // For each copy port, check if it chains to another Dup
    for copy_slot in [1u8, 2u8] {
        let copy_port = arena.port(dup_ptr, copy_slot);
        if copy_port.is_connected() && copy_port.slot == 0 {
            if let Some(node) = arena.get(copy_port.target) {
                if matches!(node.kind, OpCode::Dup { .. }) {
                    trace_dup_tree(arena, state, copy_port.target, var_name);
                }
            }
        }
    }
}

fn readback_inner(arena: &Arena, ptr: Ptr, state: &mut ReadbackState) -> Term {
    if ptr.is_none() {
        return Term::Wire(u32::MAX);
    }

    if state.visited.contains(&ptr) {
        return Term::Wire(ptr.0);
    }
    state.visited.insert(ptr);

    let node = match arena.get(ptr) {
        Some(n) => n,
        None => return Term::Wire(ptr.0),
    };

    match &node.kind {
        OpCode::Lam => {
            // Lam: 0=principal, 1=var, 2=body
            let var_name = state.fresh_var();

            // Mark the Lam's own var port (ptr, 1) as this variable.
            // Other wires reference the variable by targeting (ptr, 1).
            state
                .var_names
                .insert((ptr, 1), var_name.clone());

            // If the var port connects to a Dup fan tree (non-linear variable),
            // trace through and mark all copy ports so readback resolves them
            // as the variable name instead of showing raw Dup nodes.
            trace_var_dups(arena, state, ptr, &var_name);

            // Read the body (port 2) — use readback_port for var detection
            let body = readback_port(arena, node.ports[2], state);

            Term::Binder {
                kind: "lam".to_string(),
                var: var_name,
                body: Box::new(body),
            }
        }
        OpCode::App => {
            // App: 0=principal, 1=arg, 2=result
            let func = readback_port(arena, node.ports[0], state);
            let arg = readback_port(arena, node.ports[1], state);

            Term::App(Box::new(func), vec![arg])
        }
        OpCode::Sym { name, arity } => {
            if *arity == 0 {
                Term::Const(name.clone())
            } else {
                // Read aux ports as arguments — use readback_port for var detection
                let mut args = Vec::new();
                for slot in 1..node.ports.len() {
                    let port = node.ports[slot];
                    if port.is_connected() {
                        args.push(readback_port(arena, port, state));
                    }
                }
                Term::App(Box::new(Term::Const(name.clone())), args)
            }
        }
        OpCode::Erase => Term::Erased,
        OpCode::Dup { label } => {
            // Dup shouldn't appear in normal forms, but handle gracefully
            let a = readback_port(arena, node.ports[1], state);
            let b = readback_port(arena, node.ports[2], state);
            Term::App(
                Box::new(Term::Const(format!("dup#{}", label))),
                vec![a, b],
            )
        }
        OpCode::Barrier { scope } => {
            let inner = readback_port(arena, node.ports[1], state);
            Term::App(
                Box::new(Term::Const(format!("barrier#{}", scope))),
                vec![inner],
            )
        }
        OpCode::Future => Term::Future,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::Arena;
    use crate::node::OpCode;

    #[test]
    fn readback_constant() {
        let mut arena = Arena::new();
        let sym = arena.spawn(OpCode::Sym {
            name: "true".into(),
            arity: 0,
        });
        let term = readback(&arena, sym);
        assert_eq!(format!("{}", term), "true");
    }

    #[test]
    fn readback_identity() {
        let mut arena = Arena::new();
        let lam = arena.spawn(OpCode::Lam);
        // Identity: var port connects to body port (self-loop)
        arena.connect(lam, 1, lam, 2);

        let term = readback(&arena, lam);
        assert_eq!(format!("{}", term), "[lam x x]");
    }

    #[test]
    fn readback_erased() {
        let mut arena = Arena::new();
        let e = arena.spawn(OpCode::Erase);
        let term = readback(&arena, e);
        assert_eq!(format!("{}", term), "*");
    }
}
