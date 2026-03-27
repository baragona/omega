//! Dumb Implantation — a pure, 1:1 topological encoder from Apeiron
//! S-expressions into Archon's extended arena with region assignment.
//!
//! This is the "zero optimization" bridge: it takes an Apeiron Sexp
//! (bracket-based S-expression) and builds the raw interaction net graph
//! in a specified region. No compilation passes, no semantic transforms.
//!
//! The resulting graph is unoptimized — Archon's boundary physics will
//! transform it as it flows through the membrane topology.
//!
//! ## Pipeline
//!
//! ```text
//! Path A (classical): Omega → Hyperion passes → Apeiron
//! Path B (Archon):    Omega → implant::build_raw() → Archon manifold → settled graph
//! ```

use std::collections::HashMap;

use apeiron::node::{OpCode, Ptr};

use crate::extended_arena::ArchonArena;

/// A minimal S-expression type for the implantation layer.
/// This mirrors Apeiron's Sexp but is owned by Archon so we don't
/// depend on Apeiron's parser module being public.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Sexp {
    /// An atomic symbol.
    Atom(String),
    /// A nested list of S-expressions.
    List(Vec<Sexp>),
}

impl Sexp {
    pub fn atom(s: impl Into<String>) -> Self {
        Sexp::Atom(s.into())
    }

    pub fn list(items: Vec<Sexp>) -> Self {
        Sexp::List(items)
    }

    pub fn as_atom(&self) -> Option<&str> {
        match self {
            Sexp::Atom(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Sexp]> {
        match self {
            Sexp::List(items) => Some(items),
            _ => None,
        }
    }
}

/// Build environment tracking variable scopes during graph construction.
struct BuildEnv {
    /// Stack of scope frames: variable name → dangling port waiting for the var's usage.
    scopes: Vec<HashMap<String, Vec<DanglingPort>>>,
    /// Known operator arities (from theory declarations).
    known_ops: HashMap<String, u8>,
}

/// A port waiting to be connected to a variable's usage site.
#[derive(Clone, Debug)]
struct DanglingPort {
    node: Ptr,
    slot: u8,
}

impl BuildEnv {
    fn new() -> Self {
        BuildEnv {
            scopes: vec![HashMap::new()],
            known_ops: HashMap::new(),
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) -> HashMap<String, Vec<DanglingPort>> {
        self.scopes.pop().unwrap_or_default()
    }

    fn record_usage(&mut self, name: &str, port: DanglingPort) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(usages) = scope.get_mut(name) {
                usages.push(port);
                return;
            }
        }
        // Not found in any scope — record in outermost as a free variable.
        self.scopes[0]
            .entry(name.to_string())
            .or_default()
            .push(port);
    }

    fn declare_var(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.entry(name.to_string()).or_default();
        }
    }
}

/// Result of building a raw graph.
pub struct ImplantResult {
    /// The root node of the built graph.
    pub root: Ptr,
    /// Free variables that weren't bound (name → dangling ports).
    pub free_vars: HashMap<String, Vec<Ptr>>,
}

/// Build a raw interaction net graph from an S-expression, placing all
/// nodes in the specified region. No optimization passes are applied.
///
/// ## Recognized forms
///
/// - `[lam VAR BODY]` → Lam node (VAR is bound in BODY)
/// - `[app FN ARG]` → App node
/// - `[F A B C ...]` → curried application: `(((F A) B) C)`
/// - `atom` → Sym node with arity 0
/// - `?name` → Future node (meta-variable)
pub fn build_raw(
    arena: &mut ArchonArena,
    sexp: &Sexp,
    region: u32,
) -> ImplantResult {
    let mut env = BuildEnv::new();
    let root = build_term(arena, sexp, region, &mut env);

    // Collect free variables (usages in the outermost scope).
    let outer = env.pop_scope();
    let mut free_vars = HashMap::new();
    for (name, ports) in outer {
        free_vars.insert(name, ports.into_iter().map(|p| p.node).collect());
    }

    ImplantResult { root, free_vars }
}

/// Build a raw graph with known operator arities.
pub fn build_raw_with_ops(
    arena: &mut ArchonArena,
    sexp: &Sexp,
    region: u32,
    ops: HashMap<String, u8>,
) -> ImplantResult {
    let mut env = BuildEnv::new();
    env.known_ops = ops;
    let root = build_term(arena, sexp, region, &mut env);

    let outer = env.pop_scope();
    let mut free_vars = HashMap::new();
    for (name, ports) in outer {
        free_vars.insert(name, ports.into_iter().map(|p| p.node).collect());
    }

    ImplantResult { root, free_vars }
}

/// Recursively build a term.
fn build_term(
    arena: &mut ArchonArena,
    sexp: &Sexp,
    region: u32,
    env: &mut BuildEnv,
) -> Ptr {
    match sexp {
        Sexp::Atom(name) => build_atom(arena, name, region, env),
        Sexp::List(items) if items.is_empty() => {
            // Empty list → unit/nil symbol.
            arena.spawn_in(OpCode::Sym { name: "nil".into(), arity: 0 }, region)
        }
        Sexp::List(items) => build_list(arena, items, region, env),
    }
}

/// Build an atomic term.
///
/// If the atom is a bound variable (declared in an enclosing lambda scope),
/// we create a wire-proxy node that will be eliminated during scope wiring.
/// In interaction nets, variables are wires, not nodes.
fn build_atom(
    arena: &mut ArchonArena,
    name: &str,
    region: u32,
    env: &mut BuildEnv,
) -> Ptr {
    // Meta-variable: ?name → Future node.
    if name.starts_with('?') {
        return arena.spawn_in(OpCode::Future, region);
    }

    // Check if this name is a bound variable in any enclosing scope.
    let is_bound = env.scopes.iter().rev().any(|scope| scope.contains_key(name));

    if is_bound {
        // Bound variable: create a wire-proxy node.
        // This will be bypassed when the lambda scope wires up.
        let proxy = arena.spawn_in(
            OpCode::Sym {
                name: format!("__var_{}", name),
                arity: 0,
            },
            region,
        );
        env.record_usage(name, DanglingPort { node: proxy, slot: 0 });
        return proxy;
    }

    // Not a bound variable — create a Sym node.
    let arity = env.known_ops.get(name).copied().unwrap_or(0);
    let node = arena.spawn_in(
        OpCode::Sym {
            name: name.to_string(),
            arity,
        },
        region,
    );
    node
}

/// Build a list-form term.
fn build_list(
    arena: &mut ArchonArena,
    items: &[Sexp],
    region: u32,
    env: &mut BuildEnv,
) -> Ptr {
    // Special forms.
    if let Some(Sexp::Atom(head)) = items.first() {
        match head.as_str() {
            "lam" | "lambda" if items.len() == 3 => {
                return build_lambda(arena, &items[1], &items[2], region, env);
            }
            "app" if items.len() == 3 => {
                return build_app(arena, &items[1], &items[2], region, env);
            }
            "box" if items.len() == 2 => {
                return build_modal(arena, "__archon_box", &items[1], region, env);
            }
            "diamond" if items.len() == 2 => {
                return build_modal(arena, "__archon_diamond", &items[1], region, env);
            }
            "forall" if items.len() == 3 => {
                return build_quantifier(arena, "forall", &items[1], &items[2], region, env);
            }
            "exists" if items.len() == 3 => {
                return build_quantifier(arena, "exists", &items[1], &items[2], region, env);
            }
            // ── Tensor products ──
            "tensor" | "⊗" if items.len() == 3 => {
                return build_tensor(arena, &items[1], &items[2], region, env);
            }
            "tensor-unit" | "I" if items.len() == 1 => {
                return arena.spawn_in(
                    OpCode::Sym { name: "__archon_tensor_unit".into(), arity: 0 },
                    region,
                );
            }
            // ── HoTT primitives ──
            "refl" if items.len() == 2 => {
                return build_refl(arena, &items[1], region, env);
            }
            "J" if items.len() >= 4 => {
                return build_j_elim(arena, &items[1..], region, env);
            }
            "path" if items.len() == 4 => {
                return build_path(arena, &items[1], &items[2], &items[3], region, env);
            }
            "transport" if items.len() == 4 => {
                return build_transport(arena, &items[1], &items[2], &items[3], region, env);
            }
            // ── Contextual types (Beluga-style) ──
            "ctx-var" if items.len() == 2 => {
                return build_ctx_var(arena, &items[1], region, env);
            }
            "ctx-empty" if items.len() == 1 => {
                return arena.spawn_in(
                    OpCode::Sym { name: "__archon_ctx_empty".into(), arity: 0 },
                    region,
                );
            }
            "ctx-extend" if items.len() == 3 => {
                return build_binary_sym(arena, "__archon_ctx_extend", &items[1], &items[2], region, env);
            }
            // ── Effect operations ──
            "effect" if items.len() == 3 => {
                return build_binary_sym(arena, "__archon_effect", &items[1], &items[2], region, env);
            }
            // ── Explicit substitution closures ──
            "closure" if items.len() == 3 => {
                return build_binary_sym(arena, "__archon_closure", &items[1], &items[2], region, env);
            }
            "subst" if items.len() == 3 => {
                return build_binary_sym(arena, "__archon_subst", &items[1], &items[2], region, env);
            }
            _ => {}
        }
    }

    // General case: [F A B C ...] → curried application ((F A) B) C
    if items.len() == 1 {
        return build_term(arena, &items[0], region, env);
    }

    let head = build_term(arena, &items[0], region, env);
    let mut result = head;

    for arg_sexp in &items[1..] {
        let arg = build_term(arena, arg_sexp, region, env);
        let app = arena.spawn_in(OpCode::App, region);
        arena.connect(app, 0, result, 0); // fn
        arena.connect(app, 1, arg, 0);    // arg
        result = app;
    }

    result
}

/// Build a lambda: [lam VAR BODY]
fn build_lambda(
    arena: &mut ArchonArena,
    var_sexp: &Sexp,
    body_sexp: &Sexp,
    region: u32,
    env: &mut BuildEnv,
) -> Ptr {
    let var_name = match var_sexp {
        Sexp::Atom(n) => n.clone(),
        _ => "__anon".to_string(),
    };

    let lam = arena.spawn_in(OpCode::Lam, region);

    // Push a new scope and declare the variable.
    env.push_scope();
    env.declare_var(&var_name);

    // Build the body.
    let body = build_term(arena, body_sexp, region, env);

    // Pop scope and wire variable usages.
    let scope = env.pop_scope();

    // Wire body to lam.
    arena.connect(lam, 2, body, 0); // body port

    // Wire variable usages.
    // Variable proxies (__var_NAME nodes) must be bypassed:
    // In interaction nets, variables are wires, not nodes.
    if let Some(usages) = scope.get(&var_name) {
        match usages.len() {
            0 => {
                // Unused variable → connect var port to Erase.
                let erase = arena.spawn_in(OpCode::Erase, region);
                arena.connect(lam, 1, erase, 0);
            }
            1 => {
                // Single use → bypass the proxy node.
                // The proxy's principal (port 0) is connected to the
                // parent node's slot that references this variable.
                // We want lam.var to connect there instead.
                let proxy = usages[0].node;
                let proxy_port = arena.port(proxy, 0);
                if proxy_port.is_connected() {
                    // Rewire: lam.var ↔ proxy's parent.
                    arena.connect(lam, 1, proxy_port.target, proxy_port.slot);
                } else {
                    // Proxy is the body itself (identity case: body == proxy).
                    // Wire lam.var ↔ lam.body (self-loop).
                    arena.connect(lam, 1, lam, 2);
                }
                arena.free(proxy);
            }
            n => {
                // Multiple uses → build a Dup fan-tree.
                // First, collect all proxy nodes and find their parent connections.
                let mut parent_connections: Vec<(Ptr, u8)> = Vec::new();
                for usage in usages {
                    let proxy = usage.node;
                    let proxy_port = arena.port(proxy, 0);
                    if proxy_port.is_connected() {
                        parent_connections.push((proxy_port.target, proxy_port.slot));
                    }
                    arena.free(proxy);
                }

                // Build a Dup fan-tree from lam.var to all usage sites.
                let mut current_port_node = lam;
                let mut current_port_slot: u8 = 1;

                for (i, &(target, slot)) in parent_connections.iter().enumerate() {
                    if i == n - 1 {
                        arena.connect(current_port_node, current_port_slot, target, slot);
                    } else {
                        let label = arena.inner.fresh_dup_label();
                        let dup = arena.spawn_in(OpCode::Dup { label }, region);
                        arena.connect(current_port_node, current_port_slot, dup, 0);
                        arena.connect(dup, 1, target, slot);
                        current_port_node = dup;
                        current_port_slot = 2;
                    }
                }
            }
        }
    } else {
        // Variable not even mentioned in scope → Erase.
        let erase = arena.spawn_in(OpCode::Erase, region);
        arena.connect(lam, 1, erase, 0);
    }

    lam
}

/// Build an explicit application: [app FN ARG]
fn build_app(
    arena: &mut ArchonArena,
    fn_sexp: &Sexp,
    arg_sexp: &Sexp,
    region: u32,
    env: &mut BuildEnv,
) -> Ptr {
    let app = arena.spawn_in(OpCode::App, region);
    let fun = build_term(arena, fn_sexp, region, env);
    let arg = build_term(arena, arg_sexp, region, env);

    arena.connect(app, 0, fun, 0);  // function → principal
    arena.connect(app, 1, arg, 0);  // argument

    app
}

/// Build a modal operator: [box CONTENT] or [diamond CONTENT]
fn build_modal(
    arena: &mut ArchonArena,
    op_name: &str,
    content_sexp: &Sexp,
    region: u32,
    env: &mut BuildEnv,
) -> Ptr {
    let modal = arena.spawn_in(
        OpCode::Sym {
            name: op_name.to_string(),
            arity: 1,
        },
        region,
    );
    let content = build_term(arena, content_sexp, region, env);
    arena.connect(modal, 1, content, 0);
    modal
}

/// Build a tensor product: [tensor A B] or [⊗ A B]
///
/// Implanted as a deterministic binary node. Strictly-linear region
/// physics will prevent duplication naturally.
fn build_tensor(
    arena: &mut ArchonArena,
    left_sexp: &Sexp,
    right_sexp: &Sexp,
    region: u32,
    env: &mut BuildEnv,
) -> Ptr {
    let tensor = arena.spawn_in(
        OpCode::Sym {
            name: "__archon_tensor".into(),
            arity: 2,
        },
        region,
    );
    let left = build_term(arena, left_sexp, region, env);
    let right = build_term(arena, right_sexp, region, env);
    arena.connect(tensor, 1, left, 0);
    arena.connect(tensor, 2, right, 0);
    tensor
}

/// Build refl: [refl A] → identity loop.
///
/// In HoTT, refl is a path from A to A. We implant it as a node
/// whose content port loops back, encoding the identity path.
fn build_refl(
    arena: &mut ArchonArena,
    term_sexp: &Sexp,
    region: u32,
    env: &mut BuildEnv,
) -> Ptr {
    let refl = arena.spawn_in(
        OpCode::Sym {
            name: "__archon_refl".into(),
            arity: 1,
        },
        region,
    );
    let term = build_term(arena, term_sexp, region, env);
    arena.connect(refl, 1, term, 0);
    refl
}

/// Build J eliminator: [J C target proof ...args]
///
/// The J eliminator is the structural router for path types.
/// KanComputation boundary physics will handle actual transport reduction.
fn build_j_elim(
    arena: &mut ArchonArena,
    args: &[Sexp],
    region: u32,
    env: &mut BuildEnv,
) -> Ptr {
    // J is a multi-port router: port 1=motive, port 2=target, rest=args
    let j = arena.spawn_in(
        OpCode::Sym {
            name: "__archon_J".into(),
            arity: args.len() as u8,
        },
        region,
    );
    for (i, arg) in args.iter().enumerate() {
        let child = build_term(arena, arg, region, env);
        arena.connect(j, (i + 1) as u8, child, 0);
    }
    j
}

/// Build path type: [path A a b]
fn build_path(
    arena: &mut ArchonArena,
    ty_sexp: &Sexp,
    left_sexp: &Sexp,
    right_sexp: &Sexp,
    region: u32,
    env: &mut BuildEnv,
) -> Ptr {
    let path = arena.spawn_in(
        OpCode::Sym {
            name: "__archon_path".into(),
            arity: 3,
        },
        region,
    );
    let ty = build_term(arena, ty_sexp, region, env);
    let left = build_term(arena, left_sexp, region, env);
    let right = build_term(arena, right_sexp, region, env);
    arena.connect(path, 1, ty, 0);
    arena.connect(path, 2, left, 0);
    arena.connect(path, 3, right, 0);
    path
}

/// Build transport: [transport P path x]
fn build_transport(
    arena: &mut ArchonArena,
    fib_sexp: &Sexp,
    path_sexp: &Sexp,
    x_sexp: &Sexp,
    region: u32,
    env: &mut BuildEnv,
) -> Ptr {
    let transport = arena.spawn_in(
        OpCode::Sym {
            name: "__archon_transport".into(),
            arity: 3,
        },
        region,
    );
    let fib = build_term(arena, fib_sexp, region, env);
    let path = build_term(arena, path_sexp, region, env);
    let x = build_term(arena, x_sexp, region, env);
    arena.connect(transport, 1, fib, 0);
    arena.connect(transport, 2, path, 0);
    arena.connect(transport, 3, x, 0);
    transport
}

/// Build a contextual variable: [ctx-var name]
///
/// Contextual variables are "radioactive" — they emit radiation with
/// a distinct flavor so boundaries know it's a context variable.
fn build_ctx_var(
    arena: &mut ArchonArena,
    name_sexp: &Sexp,
    region: u32,
    env: &mut BuildEnv,
) -> Ptr {
    let name = match name_sexp {
        Sexp::Atom(n) => n.clone(),
        _ => "__anon_ctx".to_string(),
    };
    let node = arena.spawn_in(
        OpCode::Sym {
            name: format!("__archon_ctx_{}", name),
            arity: 0,
        },
        region,
    );
    // Mark with radiation — distinct flavor for contextual variables.
    let marker = arena.add_radiation_source(node);
    // Tag the marker in a way that distinguishes it (marker ID encodes flavor).
    let _ = marker; // The radiation infrastructure tracks it.
    // Also record as a variable usage for scope tracking.
    env.record_usage(&name, DanglingPort { node, slot: 0 });
    node
}

/// Build a generic binary Sym node: [name A B]
fn build_binary_sym(
    arena: &mut ArchonArena,
    sym_name: &str,
    left_sexp: &Sexp,
    right_sexp: &Sexp,
    region: u32,
    env: &mut BuildEnv,
) -> Ptr {
    let node = arena.spawn_in(
        OpCode::Sym {
            name: sym_name.to_string(),
            arity: 2,
        },
        region,
    );
    let left = build_term(arena, left_sexp, region, env);
    let right = build_term(arena, right_sexp, region, env);
    arena.connect(node, 1, left, 0);
    arena.connect(node, 2, right, 0);
    node
}

/// Build a quantifier: [forall VAR BODY] or [exists VAR BODY]
fn build_quantifier(
    arena: &mut ArchonArena,
    quant_name: &str,
    var_sexp: &Sexp,
    body_sexp: &Sexp,
    region: u32,
    env: &mut BuildEnv,
) -> Ptr {
    let var_name = match var_sexp {
        Sexp::Atom(n) => n.clone(),
        _ => "__anon".to_string(),
    };

    let quant = arena.spawn_in(
        OpCode::Sym {
            name: quant_name.to_string(),
            arity: 1,
        },
        region,
    );

    // Build body with var in scope (for potential usage tracking).
    env.push_scope();
    env.declare_var(&var_name);
    let body = build_term(arena, body_sexp, region, env);
    let _scope = env.pop_scope(); // usages tracked but quantifier is not a binder in the inet sense

    arena.connect(quant, 1, body, 0);
    quant
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_atom() {
        let mut arena = ArchonArena::new();
        let result = build_raw(&mut arena, &Sexp::atom("x"), 0);
        let node = arena.get(result.root).unwrap();
        assert!(matches!(&node.kind, OpCode::Sym { name, arity: 0 } if name == "x"));
    }

    #[test]
    fn build_meta_var() {
        let mut arena = ArchonArena::new();
        let result = build_raw(&mut arena, &Sexp::atom("?X"), 0);
        let node = arena.get(result.root).unwrap();
        assert!(matches!(&node.kind, OpCode::Future));
    }

    #[test]
    fn build_identity_lambda() {
        let mut arena = ArchonArena::new();
        let sexp = Sexp::list(vec![
            Sexp::atom("lam"),
            Sexp::atom("x"),
            Sexp::atom("x"),
        ]);
        let result = build_raw(&mut arena, &sexp, 0);
        let node = arena.get(result.root).unwrap();
        assert_eq!(node.kind, OpCode::Lam);

        // var port (1) and body port (2) should be connected
        // (via the single usage of x in the body).
        let var_port = arena.port(result.root, 1);
        let body_port = arena.port(result.root, 2);
        assert!(var_port.is_connected());
        assert!(body_port.is_connected());
    }

    #[test]
    fn build_const_lambda() {
        let mut arena = ArchonArena::new();
        // λx.c — x is unused, should get Erase node on var port.
        let sexp = Sexp::list(vec![
            Sexp::atom("lam"),
            Sexp::atom("x"),
            Sexp::atom("c"),
        ]);
        let result = build_raw(&mut arena, &sexp, 0);
        let var_port = arena.port(result.root, 1);
        assert!(var_port.is_connected());
        let var_target = arena.get(var_port.target).unwrap();
        assert_eq!(var_target.kind, OpCode::Erase);
    }

    #[test]
    fn build_application() {
        let mut arena = ArchonArena::new();
        // [f x y] → ((f x) y)
        let sexp = Sexp::list(vec![
            Sexp::atom("f"),
            Sexp::atom("x"),
            Sexp::atom("y"),
        ]);
        let result = build_raw(&mut arena, &sexp, 0);
        let node = arena.get(result.root).unwrap();
        // Result should be an App node (outermost application).
        assert_eq!(node.kind, OpCode::App);
    }

    #[test]
    fn build_in_specific_region() {
        let mut topo = crate::region::Topology::new();
        let r = topo.add_region(
            crate::region::Region::new(0, "target").with_parent(0),
        );
        let mut arena = ArchonArena::new().with_topology(topo);

        let result = build_raw(&mut arena, &Sexp::atom("x"), r);
        assert_eq!(arena.region_of(result.root), r);
    }

    #[test]
    fn build_box_modal() {
        let mut arena = ArchonArena::new();
        let sexp = Sexp::list(vec![
            Sexp::atom("box"),
            Sexp::atom("A"),
        ]);
        let result = build_raw(&mut arena, &sexp, 0);
        let node = arena.get(result.root).unwrap();
        assert!(matches!(&node.kind, OpCode::Sym { name, arity: 1 } if name == "__archon_box"));
    }

    #[test]
    fn build_quantifier() {
        let mut arena = ArchonArena::new();
        let sexp = Sexp::list(vec![
            Sexp::atom("forall"),
            Sexp::atom("x"),
            Sexp::atom("P"),
        ]);
        let result = build_raw(&mut arena, &sexp, 0);
        let node = arena.get(result.root).unwrap();
        assert!(matches!(&node.kind, OpCode::Sym { name, arity: 1 } if name == "forall"));
    }
}
