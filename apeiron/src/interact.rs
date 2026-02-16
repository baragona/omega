use crate::arena::Arena;
use crate::node::{OpCode, Port, Ptr};

/// Helper: rewire two ports together, bypassing two nodes being freed.
/// Connects whatever was on port_a's far end to whatever was on port_b's far end.
fn rewire(arena: &mut Arena, port_a: Port, port_b: Port) {
    if port_a.is_connected() && port_b.is_connected() {
        arena.connect(
            port_a.target,
            port_a.slot,
            port_b.target,
            port_b.slot,
        );
    } else if port_a.is_connected() {
        // port_b is disconnected — erase port_a's target
        let erase = arena.spawn(OpCode::Erase);
        arena.connect(erase, 0, port_a.target, port_a.slot);
    } else if port_b.is_connected() {
        let erase = arena.spawn(OpCode::Erase);
        arena.connect(erase, 0, port_b.target, port_b.slot);
    }
}

/// Beta reduction: App × Lam annihilation.
///
/// ```text
///         result          body
///           |               |
///    App[0=principal, 1=arg, 2=result]
///     |
///    Lam[0=principal, 1=var, 2=body]
///           |
///          arg              var
/// ```
///
/// After:
///   - App.arg ↔ Lam.var  (substitution: argument goes where variable was)
///   - App.result ↔ Lam.body (continuation: result flows from body)
///   - Both App and Lam are freed.
pub fn beta(arena: &mut Arena, app: Ptr, lam: Ptr) {
    let app_arg = arena.port(app, 1);
    let app_result = arena.port(app, 2);
    let lam_var = arena.port(lam, 1);
    let lam_body = arena.port(lam, 2);

    // Check for internal wires (self-loops back into nodes being freed).
    // Identity `[lam x x]` has lam.1 ↔ lam.2 (var↔body self-loop).
    let var_is_internal = lam_var.target == lam || lam_var.target == app;
    let body_is_internal = lam_body.target == lam || lam_body.target == app;

    if var_is_internal && body_is_internal {
        // Identity case: var and body are self-connected.
        // Connect arg directly to result (the argument IS the result).
        rewire(arena, app_arg, app_result);
    } else if var_is_internal {
        // Variable is internally wired (unused externally).
        // Connect result to body's external target.
        rewire(arena, app_result, lam_body);
        // Erase the argument since variable has no external usage.
        if app_arg.is_connected() {
            let erase = arena.spawn(OpCode::Erase);
            arena.connect(erase, 0, app_arg.target, app_arg.slot);
        }
    } else if body_is_internal {
        // Body is internally wired.
        rewire(arena, app_arg, lam_var);
        if app_result.is_connected() {
            let erase = arena.spawn(OpCode::Erase);
            arena.connect(erase, 0, app_result.target, app_result.slot);
        }
    } else {
        // General case: both point to external nodes.
        // Substitution: wire arg into var hole
        rewire(arena, app_arg, lam_var);
        // Continuation: wire result out from body
        rewire(arena, app_result, lam_body);
    }

    arena.free(app);
    arena.free(lam);
}

/// Erase node meets any other node: erase all aux ports of the target.
pub fn erase_node(arena: &mut Arena, _eraser: Ptr, target: Ptr) {
    let target_node = match arena.get(target) {
        Some(n) => n.clone(),
        None => {
            arena.free(_eraser);
            return;
        }
    };

    // For each auxiliary port (skip principal port 0), spawn an eraser
    for slot in 1..target_node.ports.len() {
        let port = target_node.ports[slot];
        if port.is_connected() {
            let new_erase = arena.spawn(OpCode::Erase);
            arena.connect(new_erase, 0, port.target, port.slot);
        }
    }

    arena.free(_eraser);
    arena.free(target);
}

/// Dup × Lam: duplicate a lambda.
///
/// ```text
///   Dup[0] -- Lam[0]
///   Dup[1]=copyA, Dup[2]=copyB
///   Lam[1]=var,   Lam[2]=body
/// ```
///
/// Result: spawn 2 new Lams + 2 new Dups (for var and body).
/// ```text
///   copyA -- Lam1[0]    copyB -- Lam2[0]
///   DupVar[0] -- var    DupVar[1] -- Lam1[1]   DupVar[2] -- Lam2[1]
///   DupBody[0] -- body  DupBody[1] -- Lam1[2]  DupBody[2] -- Lam2[2]
/// ```
pub fn dup_lam(arena: &mut Arena, dup: Ptr, lam: Ptr) {
    let dup_label = match arena.get(dup).unwrap().kind.clone() {
        OpCode::Dup { label } => label,
        _ => unreachable!(),
    };

    let dup_copy_a = arena.port(dup, 1);
    let dup_copy_b = arena.port(dup, 2);
    let lam_var = arena.port(lam, 1);
    let lam_body = arena.port(lam, 2);

    // Spawn two new Lams
    let lam1 = arena.spawn(OpCode::Lam);
    let lam2 = arena.spawn(OpCode::Lam);

    // Spawn two new Dups (for var and body)
    let dup_var = arena.spawn(OpCode::Dup { label: dup_label });
    let dup_body = arena.spawn(OpCode::Dup { label: dup_label });

    // Wire copies out: Lam1 → copyA, Lam2 → copyB
    if dup_copy_a.is_connected() {
        arena.connect(lam1, 0, dup_copy_a.target, dup_copy_a.slot);
    }
    if dup_copy_b.is_connected() {
        arena.connect(lam2, 0, dup_copy_b.target, dup_copy_b.slot);
    }

    // Wire DupVar: original var → DupVar.principal, DupVar copies → Lam1.var, Lam2.var
    if lam_var.is_connected() {
        arena.connect(dup_var, 0, lam_var.target, lam_var.slot);
    }
    arena.connect(dup_var, 1, lam1, 1);
    arena.connect(dup_var, 2, lam2, 1);

    // Wire DupBody: original body → DupBody.principal, DupBody copies → Lam1.body, Lam2.body
    if lam_body.is_connected() {
        arena.connect(dup_body, 0, lam_body.target, lam_body.slot);
    }
    arena.connect(dup_body, 1, lam1, 2);
    arena.connect(dup_body, 2, lam2, 2);

    arena.free(dup);
    arena.free(lam);
}

/// Dup × App: duplicate an application.
///
/// Same pattern as dup_lam but for App (3 ports: principal, arg, result).
pub fn dup_app(arena: &mut Arena, dup: Ptr, app: Ptr) {
    let dup_label = match arena.get(dup).unwrap().kind.clone() {
        OpCode::Dup { label } => label,
        _ => unreachable!(),
    };

    let dup_copy_a = arena.port(dup, 1);
    let dup_copy_b = arena.port(dup, 2);
    let app_arg = arena.port(app, 1);
    let app_result = arena.port(app, 2);

    // Spawn two new Apps
    let app1 = arena.spawn(OpCode::App);
    let app2 = arena.spawn(OpCode::App);

    // Spawn two new Dups (for arg and result)
    let dup_arg = arena.spawn(OpCode::Dup { label: dup_label });
    let dup_result = arena.spawn(OpCode::Dup { label: dup_label });

    // Wire copies out
    if dup_copy_a.is_connected() {
        arena.connect(app1, 0, dup_copy_a.target, dup_copy_a.slot);
    }
    if dup_copy_b.is_connected() {
        arena.connect(app2, 0, dup_copy_b.target, dup_copy_b.slot);
    }

    // Wire DupArg
    if app_arg.is_connected() {
        arena.connect(dup_arg, 0, app_arg.target, app_arg.slot);
    }
    arena.connect(dup_arg, 1, app1, 1);
    arena.connect(dup_arg, 2, app2, 1);

    // Wire DupResult
    if app_result.is_connected() {
        arena.connect(dup_result, 0, app_result.target, app_result.slot);
    }
    arena.connect(dup_result, 1, app1, 2);
    arena.connect(dup_result, 2, app2, 2);

    arena.free(dup);
    arena.free(app);
}

/// Dup × Sym: duplicate a symbol/constant.
///
/// Sym nodes with arity 0 are just cloned. Sym nodes with arity > 0
/// get their arguments duplicated.
pub fn dup_sym(arena: &mut Arena, dup: Ptr, sym: Ptr) {
    let (name, arity) = match arena.get(sym).unwrap().kind.clone() {
        OpCode::Sym { name, arity } => (name, arity),
        _ => unreachable!(),
    };
    let dup_label = match arena.get(dup).unwrap().kind.clone() {
        OpCode::Dup { label } => label,
        _ => unreachable!(),
    };

    let dup_copy_a = arena.port(dup, 1);
    let dup_copy_b = arena.port(dup, 2);

    // Spawn two copies of the Sym
    let sym1 = arena.spawn(OpCode::Sym {
        name: name.clone(),
        arity,
    });
    let sym2 = arena.spawn(OpCode::Sym { name, arity });

    // Wire copies out
    if dup_copy_a.is_connected() {
        arena.connect(sym1, 0, dup_copy_a.target, dup_copy_a.slot);
    }
    if dup_copy_b.is_connected() {
        arena.connect(sym2, 0, dup_copy_b.target, dup_copy_b.slot);
    }

    // For each aux port, spawn a Dup to split the argument
    let sym_node = arena.get(sym).unwrap().clone();
    for slot in 1..sym_node.ports.len() {
        let arg_port = sym_node.ports[slot];
        if arg_port.is_connected() {
            let new_dup = arena.spawn(OpCode::Dup { label: dup_label });
            arena.connect(new_dup, 0, arg_port.target, arg_port.slot);
            arena.connect(new_dup, 1, sym1, slot as u8);
            arena.connect(new_dup, 2, sym2, slot as u8);
        }
    }

    arena.free(dup);
    arena.free(sym);
}

/// Dup × Dup (same label): annihilate — cross-connect auxiliary ports.
///
/// ```text
///   D1[1] ↔ D2[1]
///   D1[2] ↔ D2[2]
/// ```
pub fn dup_dup_annihilate(arena: &mut Arena, d1: Ptr, d2: Ptr) {
    let d1_a = arena.port(d1, 1);
    let d1_b = arena.port(d1, 2);
    let d2_a = arena.port(d2, 1);
    let d2_b = arena.port(d2, 2);

    rewire(arena, d1_a, d2_a);
    rewire(arena, d1_b, d2_b);

    arena.free(d1);
    arena.free(d2);
}

/// Dup × Dup (different label): commute — spawn 4 fresh Dups in diamond.
///
/// ```text
///   D1[1] → A[0]    D1[2] → B[0]
///   D2[1] → C[0]    D2[2] → D[0]
///   A[1] ↔ C[1]     A[2] ↔ D[1]
///   B[1] ↔ C[2]     B[2] ↔ D[2]
/// ```
pub fn dup_dup_commute(arena: &mut Arena, d1: Ptr, d2: Ptr) {
    let l1 = match arena.get(d1).unwrap().kind.clone() {
        OpCode::Dup { label } => label,
        _ => unreachable!(),
    };
    let l2 = match arena.get(d2).unwrap().kind.clone() {
        OpCode::Dup { label } => label,
        _ => unreachable!(),
    };

    let d1_a = arena.port(d1, 1);
    let d1_b = arena.port(d1, 2);
    let d2_a = arena.port(d2, 1);
    let d2_b = arena.port(d2, 2);

    // Spawn 4 new dups: A,B get label l2; C,D get label l1
    let a = arena.spawn(OpCode::Dup { label: l2 });
    let b = arena.spawn(OpCode::Dup { label: l2 });
    let c = arena.spawn(OpCode::Dup { label: l1 });
    let d = arena.spawn(OpCode::Dup { label: l1 });

    // Wire to original aux ports
    if d1_a.is_connected() {
        arena.connect(a, 0, d1_a.target, d1_a.slot);
    }
    if d1_b.is_connected() {
        arena.connect(b, 0, d1_b.target, d1_b.slot);
    }
    if d2_a.is_connected() {
        arena.connect(c, 0, d2_a.target, d2_a.slot);
    }
    if d2_b.is_connected() {
        arena.connect(d, 0, d2_b.target, d2_b.slot);
    }

    // Cross-connect the diamond
    arena.connect(a, 1, c, 1);
    arena.connect(a, 2, d, 1);
    arena.connect(b, 1, c, 2);
    arena.connect(b, 2, d, 2);

    arena.free(d1);
    arena.free(d2);
}

/// Barrier × Any: check if scope is active.
/// If active, dissolve barrier and pass through.
/// If not, park in listeners (suspend).
pub fn barrier_check(arena: &mut Arena, barrier: Ptr, other: Ptr) {
    let scope = match arena.get(barrier).unwrap().kind.clone() {
        OpCode::Barrier { scope } => scope,
        _ => unreachable!(),
    };

    if arena.active_scopes.contains(&scope) {
        // Scope active: dissolve barrier, connect inner to other
        let inner = arena.port(barrier, 1);
        if inner.is_connected() {
            // Reconnect: other's principal ↔ barrier's inner target
            // The other node's principal was connected to barrier's principal.
            // We need to connect other to whatever is inside the barrier.
            arena.connect(other, 0, inner.target, inner.slot);
        }
        arena.free(barrier);
    } else {
        // Scope not active: suspend
        arena.listeners.entry(scope).or_default().push(barrier);
        // Don't re-add to active_pairs — it's suspended
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::Arena;
    use crate::node::OpCode;
    use crate::physics::{self, PhysicsConfig};

    /// Build identity: [lam x x] → a Lam node whose var port is wired to its body port.
    /// Returns (lam_ptr, root_port) where root_port is the principal port.
    fn build_identity(arena: &mut Arena) -> Ptr {
        let lam = arena.spawn(OpCode::Lam);
        // Identity: body and var are the same wire.
        // In interaction nets, this means var port (1) connects to body port (2).
        arena.connect(lam, 1, lam, 2);
        lam
    }

    #[test]
    fn beta_identity_applied_to_const() {
        let mut arena = Arena::new();

        // Build: [app [lam x x] y]
        let lam = build_identity(&mut arena);
        let y = arena.spawn(OpCode::Sym {
            name: "y".into(),
            arity: 0,
        });
        let app = arena.spawn(OpCode::App);

        // A "root" node to hold the result
        let root = arena.spawn(OpCode::Sym {
            name: "ROOT".into(),
            arity: 1,
        });

        // Wire: root.1 → app.2 (result of app goes to root)
        arena.connect(root, 1, app, 2);
        // Wire: app.1 → y.0 (argument is y)
        arena.connect(app, 1, y, 0);
        // Wire: app.0 → lam.0 (principal ports → active pair!)
        arena.connect(app, 0, lam, 0);

        // Run physics
        let result = physics::run(&mut arena, &PhysicsConfig::default());
        assert_eq!(result.halted_reason, physics::HaltReason::NormalForm);
        assert_eq!(result.interactions, 1); // exactly one beta reduction

        // After beta: root.1 should be connected to y
        let root_child = arena.port(root, 1);
        assert!(root_child.is_connected());
        let child_node = arena.get(root_child.target).unwrap();
        assert_eq!(
            child_node.kind,
            OpCode::Sym {
                name: "y".into(),
                arity: 0
            }
        );
    }

    #[test]
    fn erase_lambda() {
        let mut arena = Arena::new();

        let eraser = arena.spawn(OpCode::Erase);
        let lam = arena.spawn(OpCode::Lam);
        let body = arena.spawn(OpCode::Sym {
            name: "body".into(),
            arity: 0,
        });
        let var_target = arena.spawn(OpCode::Sym {
            name: "var".into(),
            arity: 0,
        });

        arena.connect(lam, 1, var_target, 0);
        arena.connect(lam, 2, body, 0);
        arena.connect(eraser, 0, lam, 0);

        let result = physics::run(&mut arena, &PhysicsConfig::default());
        assert_eq!(result.halted_reason, physics::HaltReason::NormalForm);

        // After erase: lam, eraser, body, var_target should all be freed
        // (erase propagates to aux ports)
        assert!(arena.get(lam).is_none());
        assert!(arena.get(eraser).is_none());
    }

    #[test]
    fn dup_annihilate() {
        let mut arena = Arena::new();

        let d1 = arena.spawn(OpCode::Dup { label: 0 });
        let d2 = arena.spawn(OpCode::Dup { label: 0 });

        let a = arena.spawn(OpCode::Sym {
            name: "a".into(),
            arity: 0,
        });
        let b = arena.spawn(OpCode::Sym {
            name: "b".into(),
            arity: 0,
        });
        let c = arena.spawn(OpCode::Sym {
            name: "c".into(),
            arity: 0,
        });
        let dd = arena.spawn(OpCode::Sym {
            name: "d".into(),
            arity: 0,
        });

        arena.connect(d1, 1, a, 0);
        arena.connect(d1, 2, b, 0);
        arena.connect(d2, 1, c, 0);
        arena.connect(d2, 2, dd, 0);
        arena.connect(d1, 0, d2, 0); // active pair

        let result = physics::run(&mut arena, &PhysicsConfig::default());
        assert_eq!(result.halted_reason, physics::HaltReason::NormalForm);

        // After annihilation: a↔c, b↔d
        let a_port = arena.port(a, 0);
        assert_eq!(a_port.target, c);
        let b_port = arena.port(b, 0);
        assert_eq!(b_port.target, dd);
    }
}
