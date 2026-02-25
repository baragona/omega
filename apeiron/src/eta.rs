use crate::arena::Arena;
use crate::node::{OpCode, Ptr};

/// Eta-contraction on interaction net graphs.
///
/// Pattern: `(lam x (app f x))` → `f`
///
/// On the interaction net (as built by the builder):
///   Lam: port 0=principal, port 1=var, port 2=body
///   App: port 0=principal (→ function), port 1=arg, port 2=result (→ Lam.body)
///
/// Eta-redex when:
/// - Lam.body (port 2) ↔ App.result (port 2)  [body IS the application result]
/// - App.arg  (port 1) ↔ Lam.var   (port 1)   [argument IS the bound variable]
///
/// Contract: wire Lam.principal's neighbor ↔ App.principal's neighbor (f),
/// then free both Lam and App nodes.
pub fn eta_contract(arena: &mut Arena, _root: Ptr) {
    let mut worklist = collect_lam_nodes(arena);
    let mut iterations = 0;
    let max_iterations = 1000;

    while let Some(lam_ptr) = worklist.pop() {
        if iterations >= max_iterations {
            break;
        }
        iterations += 1;

        // Validate lam_ptr still exists and is a Lam
        let Some(lam_node) = arena.get(lam_ptr) else {
            continue;
        };
        if !matches!(lam_node.kind, OpCode::Lam) {
            continue;
        }

        let lam_body = arena.port(lam_ptr, 2); // body port

        // Body must connect to an App's result port (port 2)
        if !lam_body.is_connected() {
            continue;
        }
        let app_ptr = lam_body.target;
        if lam_body.slot != 2 {
            continue; // must be App's result port
        }
        let Some(app_node) = arena.get(app_ptr) else {
            continue;
        };
        if !matches!(app_node.kind, OpCode::App) {
            continue;
        }

        // App's arg (port 1) must wire to Lam's var (port 1)
        let app_arg = arena.port(app_ptr, 1);
        if app_arg.target != lam_ptr || app_arg.slot != 1 {
            continue;
        }

        // Eta-redex confirmed! Contract.
        // Wire Lam.principal's neighbor ↔ App.principal's neighbor (the function f)
        let lam_principal = arena.port(lam_ptr, 0);
        let app_principal = arena.port(app_ptr, 0);

        if lam_principal.is_connected() && app_principal.is_connected() {
            arena.connect(
                lam_principal.target,
                lam_principal.slot,
                app_principal.target,
                app_principal.slot,
            );
        } else if lam_principal.is_connected() {
            arena.disconnect(lam_principal.target, lam_principal.slot);
        } else if app_principal.is_connected() {
            arena.disconnect(app_principal.target, app_principal.slot);
        }

        // Check if any neighbor is a Lam node — re-enqueue for nested eta
        if lam_principal.is_connected() {
            if let Some(n) = arena.get(lam_principal.target) {
                if matches!(n.kind, OpCode::Lam) {
                    worklist.push(lam_principal.target);
                }
            }
        }
        if app_principal.is_connected() {
            if let Some(n) = arena.get(app_principal.target) {
                if matches!(n.kind, OpCode::Lam) {
                    worklist.push(app_principal.target);
                }
            }
        }

        arena.free(lam_ptr);
        arena.free(app_ptr);
    }
}

/// Collect all live Lam nodes in the arena.
fn collect_lam_nodes(arena: &Arena) -> Vec<Ptr> {
    let capacity = arena.node_capacity();
    (0..capacity)
        .filter_map(|i| {
            let ptr = Ptr(i as u32);
            let node = arena.get(ptr)?;
            if matches!(node.kind, OpCode::Lam) {
                Some(ptr)
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eta_contract_simple() {
        // Build: (lam x (app f x)) where f is a Sym node
        // Wiring (as builder does it):
        //   Lam.body (2) ↔ App.result (2)
        //   App.arg (1) ↔ Lam.var (1)
        //   App.principal (0) ↔ f.principal (0)
        //   root.1 ↔ Lam.principal (0)
        let mut arena = Arena::new();

        let root = arena.spawn(OpCode::Sym {
            name: "ROOT".into(),
            arity: 1,
        });
        let lam = arena.spawn(OpCode::Lam);
        let app = arena.spawn(OpCode::App);
        let f = arena.spawn(OpCode::Sym {
            name: "f".into(),
            arity: 0,
        });

        // root.1 → lam.0 (principal)
        arena.connect(root, 1, lam, 0);
        // lam.2 (body) → app.2 (result)
        arena.connect(lam, 2, app, 2);
        // app.1 (arg) → lam.1 (var)
        arena.connect(app, 1, lam, 1);
        // app.0 (principal) → f.0 (principal)
        arena.connect(app, 0, f, 0);

        eta_contract(&mut arena, root);

        // After eta-contraction: root.1 should connect to f.0
        let root_port = arena.port(root, 1);
        assert!(root_port.is_connected(), "root should still be connected");
        assert_eq!(root_port.target, f, "root should connect to f");

        // Lam and App should be freed
        assert!(arena.get(lam).is_none(), "lam should be freed");
        assert!(arena.get(app).is_none(), "app should be freed");
    }

    #[test]
    fn eta_no_redex_unchanged() {
        // Build: (lam x (app x f)) — NOT an eta-redex (arg is f, not x)
        let mut arena = Arena::new();

        let root = arena.spawn(OpCode::Sym {
            name: "ROOT".into(),
            arity: 1,
        });
        let lam = arena.spawn(OpCode::Lam);
        let app = arena.spawn(OpCode::App);
        let f = arena.spawn(OpCode::Sym {
            name: "f".into(),
            arity: 0,
        });

        arena.connect(root, 1, lam, 0);
        // lam.2 (body) → app.2 (result)
        arena.connect(lam, 2, app, 2);
        // app.1 (arg) → f.0 (NOT lam.1)
        arena.connect(app, 1, f, 0);
        // app.0 (principal) → lam.1 (var) — NOT the eta pattern
        arena.connect(app, 0, lam, 1);

        let live_before = arena.live_count();
        eta_contract(&mut arena, root);
        let live_after = arena.live_count();

        // Nothing should change
        assert_eq!(live_before, live_after, "non-eta-redex should be unchanged");
        assert!(arena.get(lam).is_some(), "lam should still exist");
        assert!(arena.get(app).is_some(), "app should still exist");
    }
}
