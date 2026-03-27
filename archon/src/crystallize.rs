//! Crystallization fronts — CPS and monadic transforms via catalyst
//! particles that propagate through the graph.
//!
//! A catalyst (continuation) node is injected at the effect boundary.
//! As it drifts through the graph, it locally transforms each node:
//! - App node + catalyst → CPS'd application (thread continuation)
//! - Lam node + catalyst → continuation-parameterized lambda
//!
//! The global CPS transform emerges from the wavefront completing.

use apeiron::node::{OpCode, Ptr};

use crate::extended_arena::ArchonArena;

/// Result of a catalyst interaction.
#[derive(Debug)]
pub enum CatalystResult {
    /// Catalyst transformed an application node (CPS).
    TransformedApp,
    /// Catalyst transformed a lambda node.
    TransformedLam,
    /// Catalyst reached a value (base case).
    ReachedValue,
    /// Not a catalyst interaction.
    NotCatalyst,
}

/// Check if a node is a catalyst.
pub fn is_catalyst(arena: &ArchonArena, ptr: Ptr) -> bool {
    arena
        .get(ptr)
        .map_or(false, |n| matches!(&n.kind, OpCode::Sym { name, .. } if name == "__catalyst"))
}

/// Handle a catalyst meeting an App node.
///
/// CPS transform: [[M N]](k) = [[M]](λm. [[N]](λn. m n k))
///
/// The catalyst splits into sub-catalysts that propagate into M and N.
pub fn catalyst_meets_app(arena: &mut ArchonArena, catalyst: Ptr, app: Ptr) -> CatalystResult {
    let region = arena.region_of(catalyst);

    let app_arg = arena.port(app, 1);     // N
    let app_result = arena.port(app, 2);  // where result goes

    // The original catalyst carries continuation k (its aux port 1).
    let k_port = arena.port(catalyst, 1);

    // Create the inner continuation: λn. m n k
    // This is a new application that applies m to n with continuation k.
    let inner_app = arena.spawn_in(OpCode::App, region);
    let outer_app = arena.spawn_in(OpCode::App, region);

    // Create sub-catalyst for N: will transform N with continuation (λn. m n k)
    let catalyst_n = arena.spawn_in(
        OpCode::Sym {
            name: "__catalyst".into(),
            arity: 1,
        },
        region,
    );

    // Create sub-catalyst for M: will transform M with continuation (λm. [[N]](λn. m n k))
    let catalyst_m = arena.spawn_in(
        OpCode::Sym {
            name: "__catalyst".into(),
            arity: 1,
        },
        region,
    );

    // Wire: catalyst_m goes into the function position
    if app_result.is_connected() {
        arena.connect(catalyst_m, 0, app_result.target, app_result.slot);
    }
    // catalyst_m's continuation is the outer application
    arena.connect(catalyst_m, 1, outer_app, 0);

    // Wire: catalyst_n goes into the argument position
    if app_arg.is_connected() {
        arena.connect(catalyst_n, 0, app_arg.target, app_arg.slot);
    }
    // catalyst_n's continuation chains into inner_app
    arena.connect(catalyst_n, 1, inner_app, 0);

    // The original continuation k connects to the final application
    if k_port.is_connected() {
        arena.connect(outer_app, 2, k_port.target, k_port.slot);
    }

    // Free the original catalyst and app.
    arena.free(catalyst);
    arena.free(app);

    CatalystResult::TransformedApp
}

/// Handle a catalyst meeting a value node (Sym with arity 0).
///
/// CPS transform: [[v]](k) = k(v)
///
/// The catalyst applies the continuation to the value.
pub fn catalyst_meets_value(arena: &mut ArchonArena, catalyst: Ptr, value: Ptr) -> CatalystResult {
    let region = arena.region_of(catalyst);
    let k_port = arena.port(catalyst, 1);

    if !k_port.is_connected() {
        // No continuation — the value is the final result.
        arena.free(catalyst);
        return CatalystResult::ReachedValue;
    }

    // Create: k(v) — apply continuation to value.
    let app = arena.spawn_in(OpCode::App, region);
    arena.connect(app, 0, k_port.target, k_port.slot); // k
    arena.connect(app, 1, value, 0);                     // v

    // The result of k(v) goes wherever the catalyst's principal was connected.
    let catalyst_principal = arena.port(catalyst, 0);
    if catalyst_principal.is_connected() {
        arena.connect(app, 2, catalyst_principal.target, catalyst_principal.slot);
    }

    arena.free(catalyst);
    CatalystResult::ReachedValue
}

/// Handle a catalyst meeting a Lam node.
///
/// CPS transform: [[λx.M]](k) = k(λx.λk'. [[M]](k'))
///
/// The lambda gets an extra continuation parameter. The body M gets
/// a sub-catalyst with continuation k'. The whole thing is passed to k.
pub fn catalyst_meets_lam(arena: &mut ArchonArena, catalyst: Ptr, lam: Ptr) -> CatalystResult {
    let region = arena.region_of(catalyst);

    let lam_var = arena.port(lam, 1);   // x
    let lam_body = arena.port(lam, 2);  // M
    let k_port = arena.port(catalyst, 1); // k

    // Create the CPS'd lambda: λx.λk'.[[M]](k')
    // We keep the outer lambda (for x), add an inner lambda (for k'),
    // and inject a sub-catalyst into the body with continuation k'.

    // Inner lambda for the continuation parameter k'.
    let inner_lam = arena.spawn_in(OpCode::Lam, region);

    // Sub-catalyst for the body M, carrying k' as continuation.
    let sub_catalyst = arena.spawn_in(
        OpCode::Sym {
            name: "__catalyst".into(),
            arity: 1,
        },
        region,
    );

    // Wire inner_lam's var port (k') to the sub-catalyst's continuation.
    arena.connect(sub_catalyst, 1, inner_lam, 1); // k' ↔ sub_catalyst.aux1

    // Wire sub-catalyst into the body.
    if lam_body.is_connected() {
        arena.connect(sub_catalyst, 0, lam_body.target, lam_body.slot);
    }

    // The inner_lam's body is the sub-catalyst's work area.
    // (The sub-catalyst will transform M and produce the result.)
    // Wire inner_lam.body ↔ sub_catalyst.principal ... but sub_catalyst.principal
    // is already wired to M. Instead, inner_lam.body points to sub_catalyst.
    arena.connect(inner_lam, 2, sub_catalyst, 0);

    // The outer lambda keeps its variable (x) and its body becomes inner_lam.
    // Rewire: lam.body → inner_lam
    arena.connect(lam, 2, inner_lam, 0);

    // Apply continuation k to the CPS'd lambda: k(λx.λk'.[[M]](k'))
    if k_port.is_connected() {
        let app = arena.spawn_in(OpCode::App, region);
        arena.connect(app, 0, k_port.target, k_port.slot); // k
        arena.connect(app, 1, lam, 0);                       // λx.λk'...

        // The result goes wherever the catalyst's principal was.
        let cat_principal = arena.port(catalyst, 0);
        if cat_principal.is_connected() {
            arena.connect(app, 2, cat_principal.target, cat_principal.slot);
        }
    }

    arena.free(catalyst);
    CatalystResult::TransformedLam
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalyst_detection() {
        let mut arena = ArchonArena::new();
        let cat = arena.spawn(OpCode::Sym {
            name: "__catalyst".into(),
            arity: 1,
        });
        let not_cat = arena.spawn(OpCode::Lam);

        assert!(is_catalyst(&arena, cat));
        assert!(!is_catalyst(&arena, not_cat));
    }

    #[test]
    fn catalyst_meets_value_applies_k() {
        let mut arena = ArchonArena::new();

        let catalyst = arena.spawn(OpCode::Sym {
            name: "__catalyst".into(),
            arity: 1,
        });
        let value = arena.spawn(OpCode::Sym {
            name: "v".into(),
            arity: 0,
        });
        let continuation = arena.spawn(OpCode::Sym {
            name: "k".into(),
            arity: 1,
        });
        let root = arena.spawn(OpCode::Sym {
            name: "root".into(),
            arity: 1,
        });

        // catalyst.principal ↔ root.aux1
        arena.connect(catalyst, 0, root, 1);
        // catalyst.aux1 ↔ continuation (the k)
        arena.connect(catalyst, 1, continuation, 0);

        let result = catalyst_meets_value(&mut arena, catalyst, value);
        assert!(matches!(result, CatalystResult::ReachedValue));

        // Catalyst should be freed.
        assert!(arena.get(catalyst).is_none());
    }
}
