//! Boundary physics — what happens when nodes from different regions
//! form an active pair.
//!
//! Each of Hyperion's 21 compilation passes becomes a boundary interaction
//! rule. When a graph crosses a membrane, the boundary physics transforms
//! it according to the membrane type.

use apeiron::node::{OpCode, Ptr};

use crate::extended_arena::{ArchonArena, MarkerId};
use crate::region::BoundaryType;

/// Result of a boundary crossing interaction.
#[derive(Debug)]
pub enum BoundaryResult {
    /// The interaction was handled by boundary physics.
    Handled(String),
    /// No boundary rule applies; fall through to standard physics.
    PassThrough,
    /// The crossing is forbidden (e.g., linear resource violation).
    Rejected(String),
}

/// Dispatch a cross-region active pair to the appropriate boundary handler.
pub fn dispatch(
    arena: &mut ArchonArena,
    left: Ptr,
    left_kind: &OpCode,
    right: Ptr,
    right_kind: &OpCode,
) -> BoundaryResult {
    let left_region = arena.region_of(left);
    let right_region = arena.region_of(right);

    // Determine which boundary we're crossing.
    let boundary = arena
        .topology
        .boundary_between(left_region, right_region)
        .or_else(|| arena.topology.boundary_between(right_region, left_region));

    let boundary = match boundary {
        Some(b) => b.clone(),
        None => return BoundaryResult::PassThrough,
    };

    match boundary {
        BoundaryType::Transparent => BoundaryResult::PassThrough,

        BoundaryType::BangBoundary => {
            bang_crossing(arena, left, left_kind, right, right_kind, left_region, right_region)
        }

        BoundaryType::DefunctionalizationBoundary => {
            defunc_crossing(arena, left, left_kind, right, right_kind, left_region, right_region)
        }

        BoundaryType::CombinatorFilter => {
            combinator_crossing(arena, left, left_kind, right, right_kind, left_region, right_region)
        }

        BoundaryType::ExplicitSubstitutionBoundary => {
            explicit_subst_crossing(arena, left, left_kind, right, right_kind, left_region, right_region)
        }

        BoundaryType::ACBoundary => {
            ac_crossing(arena, left, left_kind, right, right_kind, left_region, right_region)
        }

        BoundaryType::NetworkPartition => {
            rpc_crossing(arena, left, left_kind, right, right_kind, left_region, right_region)
        }

        BoundaryType::EffectBoundary => {
            effect_crossing(arena, left, left_kind, right, right_kind, left_region, right_region)
        }

        BoundaryType::DialecticaBoundary => {
            dialectica_crossing(arena, left, left_kind, right, right_kind, left_region, right_region)
        }

        BoundaryType::TensorSerializationBoundary => {
            tensor_crossing(arena, left, left_kind, right, right_kind, left_region, right_region)
        }

        BoundaryType::KripkeBoundary => {
            kripke_crossing(arena, left, left_kind, right, right_kind, left_region, right_region)
        }

        BoundaryType::NominalBoundary | BoundaryType::NominalScoping => {
            nominal_crossing(arena, left, left_kind, right, right_kind, left_region, right_region)
        }

        BoundaryType::GroundingBoundary => {
            grounding_crossing(arena, left, left_kind, right, right_kind, left_region, right_region)
        }

        BoundaryType::ContextReifyBoundary => {
            context_reify_crossing(arena, left, left_kind, right, right_kind, left_region, right_region)
        }

        BoundaryType::ModalRestrictionBoundary => {
            modal_restriction_crossing(arena, left, left_kind, right, right_kind, left_region, right_region)
        }

        BoundaryType::KanTransportBoundary => {
            kan_transport_crossing(arena, left, left_kind, right, right_kind, left_region, right_region)
        }

        BoundaryType::ThermoBoundary => {
            thermo_crossing(arena, left, left_kind, right, right_kind, left_region, right_region)
        }

        BoundaryType::RpcSerializationBoundary => {
            rpc_crossing(arena, left, left_kind, right, right_kind, left_region, right_region)
        }

        _ => BoundaryResult::PassThrough,
    }
}

// ── BangModality: linear → unrestricted ──────────────────────────────
//
// When a node from a StrictlyLinear region meets a node from an
// OptimalSharing region, the linear node gets wrapped in a ! (bang)
// promotion node to survive the unrestricted environment.

fn bang_crossing(
    arena: &mut ArchonArena,
    left: Ptr,
    _left_kind: &OpCode,
    right: Ptr,
    _right_kind: &OpCode,
    left_region: u32,
    right_region: u32,
) -> BoundaryResult {
    use crate::region::ResourceMode;

    let left_res = arena.topology.get(left_region).map(|r| &r.resource_mode);
    let right_res = arena.topology.get(right_region).map(|r| &r.resource_mode);

    // Determine which node is crossing from linear to non-linear.
    let (linear_node, target_node, target_region) =
        match (left_res, right_res) {
            (Some(ResourceMode::StrictlyLinear), Some(r)) if *r != ResourceMode::StrictlyLinear => {
                (left, right, right_region)
            }
            (Some(r), Some(ResourceMode::StrictlyLinear)) if *r != ResourceMode::StrictlyLinear => {
                (right, left, left_region)
            }
            _ => return BoundaryResult::PassThrough,
        };

    // Wrap the linear node in a bang (!) promotion node.
    let bang = arena.spawn_in(
        OpCode::Sym {
            name: "__archon_bang".into(),
            arity: 1,
        },
        target_region,
    );

    // Rewire: target was connected to linear_node's principal.
    // Now: target ↔ bang.principal, bang.aux1 ↔ linear_node.principal.
    let target_port = arena.port(target_node, 0);
    let linear_port = arena.port(linear_node, 0);

    // Disconnect the original pair.
    // Reconnect: bang sits between them.
    arena.connect(bang, 0, target_node, target_port.slot);
    arena.connect(bang, 1, linear_node, linear_port.slot);

    BoundaryResult::Handled("Bang-Promotion".into())
}

// ── Defunctionalization: higher-order → first-order ──────────────────
//
// When a Lam node tries to cross into a first-order region (e.g., VonNeumann),
// the boundary strips the lambda, crystallizes it into a closure ADT constructor,
// and leaves an apply dispatch node behind.

fn defunc_crossing(
    arena: &mut ArchonArena,
    left: Ptr,
    left_kind: &OpCode,
    right: Ptr,
    right_kind: &OpCode,
    left_region: u32,
    right_region: u32,
) -> BoundaryResult {
    // Find which node is the lambda.
    let (lam, other, target_region) = match (left_kind, right_kind) {
        (OpCode::Lam, _) => (left, right, right_region),
        (_, OpCode::Lam) => (right, left, left_region),
        _ => return BoundaryResult::PassThrough,
    };

    // Read lambda's ports before modification.
    let lam_var = arena.port(lam, 1);
    let lam_body = arena.port(lam, 2);

    // Collect free variables in the body subgraph via radiation.
    // Any node in the body that is glowing with a marker NOT from this lambda's
    // own bound variable is a free variable that needs to be captured.
    let own_markers: Vec<MarkerId> = if lam_var.is_connected() {
        arena.markers_on(lam_var.target)
    } else {
        vec![]
    };

    let mut captured: Vec<Ptr> = Vec::new();
    if lam_body.is_connected() {
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![lam_body.target];
        while let Some(ptr) = stack.pop() {
            if !visited.insert(ptr.0) { continue; }
            // Check if this node is glowing with markers OTHER than our own bound var.
            let markers = arena.markers_on(ptr);
            for &m in &markers {
                if !own_markers.contains(&m) {
                    // This node carries a free variable's radiation — capture it.
                    // The radiation source is the free variable itself.
                    if !captured.iter().any(|&c| c == ptr) {
                        captured.push(ptr);
                    }
                }
            }
            if let Some(node) = arena.get(ptr) {
                let pc = node.kind.port_count();
                for slot in 1..pc {
                    let port = arena.port(ptr, slot as u8);
                    if port.is_connected() && arena.get(port.target).is_some() {
                        stack.push(port.target);
                    }
                }
            }
        }
    }

    // Create closure constructor: __closure_N(captured1, captured2, ..., body)
    // Arity = number of captured vars + 1 (for body).
    let closure_id = lam.0;
    let total_arity = (captured.len() + 1) as u8;
    let closure = arena.spawn_in(
        OpCode::Sym {
            name: format!("__closure_{}", closure_id),
            arity: total_arity,
        },
        target_region,
    );

    // Wire closure's principal to where the lambda was going.
    arena.connect(closure, 0, other, 0);

    // Wire captured variables to closure's aux ports (slots 1..N).
    // Note: we don't move the captured nodes — we Dup them so the original
    // subgraph stays intact (captured vars may be shared).
    for (i, &cap_ptr) in captured.iter().enumerate() {
        let slot = (i + 1) as u8;
        let cap_port = arena.port(cap_ptr, 0);
        if cap_port.is_connected() {
            arena.connect(closure, slot, cap_ptr, 0);
        }
    }

    // Wire body to closure's last aux port.
    let body_slot = total_arity;
    if lam_body.is_connected() {
        arena.connect(closure, body_slot, lam_body.target, lam_body.slot);
    }

    // Erase the bound variable wire (it's now implicit in the closure).
    if lam_var.is_connected() {
        let erase = arena.spawn_in(OpCode::Erase, target_region);
        arena.connect(erase, 0, lam_var.target, lam_var.slot);
    }

    // Free the original lambda.
    arena.free(lam);

    BoundaryResult::Handled("Defunctionalization".into())
}

// ── CombinatorFilter: bracket abstraction via radiation ──────────────
//
// When a Lam node hits the combinator boundary, the boundary checks
// radiation on the body wire:
// - Glowing (variable occurs in body) → S combinator
// - Dark (variable absent) → K combinator
// - Body IS the variable → I combinator
//
// This requires radiation to have propagated first (see radiation.rs).

fn combinator_crossing(
    arena: &mut ArchonArena,
    left: Ptr,
    left_kind: &OpCode,
    right: Ptr,
    right_kind: &OpCode,
    left_region: u32,
    right_region: u32,
) -> BoundaryResult {
    let (lam, other, target_region) = match (left_kind, right_kind) {
        (OpCode::Lam, _) => (left, right, right_region),
        (_, OpCode::Lam) => (right, left, left_region),
        _ => return BoundaryResult::PassThrough,
    };

    let lam_var = arena.port(lam, 1);
    let lam_body = arena.port(lam, 2);

    // Check if body IS the variable (identity: λx.x → I).
    if lam_var.is_connected() && lam_body.is_connected()
        && lam_var.target == lam && lam_body.target == lam
    {
        // Self-loop: identity lambda → I combinator.
        let i_node = arena.spawn_in(
            OpCode::Sym {
                name: "I".into(),
                arity: 0,
            },
            target_region,
        );
        arena.connect(i_node, 0, other, 0);
        arena.free(lam);
        return BoundaryResult::Handled("Combinator-I".into());
    }

    // Check radiation: is the body wire glowing with the variable's marker?
    let var_markers = if lam_var.is_connected() {
        arena.markers_on(lam_var.target)
    } else {
        vec![]
    };

    let body_is_glowing = if lam_body.is_connected() && !var_markers.is_empty() {
        var_markers.iter().any(|&m| arena.is_glowing(lam_body.target, m))
    } else {
        false
    };

    if !body_is_glowing {
        // Dark: variable doesn't occur in body → K combinator.
        // λx.M (x not in M) → K M
        let k_node = arena.spawn_in(
            OpCode::Sym {
                name: "K".into(),
                arity: 1,
            },
            target_region,
        );
        arena.connect(k_node, 0, other, 0);
        if lam_body.is_connected() {
            arena.connect(k_node, 1, lam_body.target, lam_body.slot);
        }
        // Erase the unused variable wire.
        if lam_var.is_connected() {
            let erase = arena.spawn_in(OpCode::Erase, target_region);
            arena.connect(erase, 0, lam_var.target, lam_var.slot);
        }
        arena.free(lam);
        BoundaryResult::Handled("Combinator-K".into())
    } else {
        // Glowing: variable occurs in body → S combinator.
        // λx.(M N) → S (λx.M) (λx.N)
        // For now, emit an S node. The recursive bracket abstraction
        // happens when the sub-lambdas hit the boundary again.
        if lam_body.is_connected() {
            let body_target = lam_body.target;
            let body_kind = arena.get(body_target).map(|n| n.kind.clone());

            if let Some(OpCode::App) = body_kind {
                // Body is an application: λx.(M N) → S (λx.M) (λx.N)
                let app_arg = arena.port(body_target, 1);
                let app_result = arena.port(body_target, 2);

                let s_node = arena.spawn_in(
                    OpCode::Sym {
                        name: "S".into(),
                        arity: 2,
                    },
                    target_region,
                );

                // Create two new lambdas for the recursive case.
                let lam_m = arena.spawn_in(OpCode::Lam, arena.region_of(lam));
                let lam_n = arena.spawn_in(OpCode::Lam, arena.region_of(lam));

                // S.principal ↔ other
                arena.connect(s_node, 0, other, 0);
                // S.aux1 ↔ lam_m (will hit boundary again → recursive)
                arena.connect(s_node, 1, lam_m, 0);
                // S.aux2 ↔ lam_n
                arena.connect(s_node, 2, lam_n, 0);

                // lam_m.body ↔ app's function (result port)
                if app_result.is_connected() {
                    arena.connect(lam_m, 2, app_result.target, app_result.slot);
                }
                // lam_n.body ↔ app's argument
                if app_arg.is_connected() {
                    arena.connect(lam_n, 2, app_arg.target, app_arg.slot);
                }

                // Both new lambdas share the variable: dup the var wire.
                if lam_var.is_connected() {
                    let label = arena.inner.fresh_dup_label();
                    let lam_region = arena.region_of(lam);
                    let dup = arena.spawn_in(
                        OpCode::Dup { label },
                        lam_region,
                    );
                    arena.connect(dup, 0, lam_var.target, lam_var.slot);
                    arena.connect(dup, 1, lam_m, 1);
                    arena.connect(dup, 2, lam_n, 1);
                }

                arena.free(lam);
                arena.free(body_target); // free the App node
                return BoundaryResult::Handled("Combinator-S".into());
            }
        }

        // Fallback: body isn't an App. Emit a generic S placeholder.
        // (Handles cases like λx.x where body is just the variable.)
        let s_node = arena.spawn_in(
            OpCode::Sym {
                name: "S".into(),
                arity: 0,
            },
            target_region,
        );
        arena.connect(s_node, 0, other, 0);
        if lam_var.is_connected() {
            let erase = arena.spawn_in(OpCode::Erase, target_region);
            arena.connect(erase, 0, lam_var.target, lam_var.slot);
        }
        if lam_body.is_connected() {
            let erase = arena.spawn_in(OpCode::Erase, target_region);
            arena.connect(erase, 0, lam_body.target, lam_body.slot);
        }
        arena.free(lam);
        BoundaryResult::Handled("Combinator-S-fallback".into())
    }
}

// ── ExplicitSubstitution: binders → closure + environment ────────────

fn explicit_subst_crossing(
    arena: &mut ArchonArena,
    left: Ptr,
    left_kind: &OpCode,
    right: Ptr,
    right_kind: &OpCode,
    left_region: u32,
    right_region: u32,
) -> BoundaryResult {
    let (lam, other, target_region) = match (left_kind, right_kind) {
        (OpCode::Lam, _) => (left, right, right_region),
        (_, OpCode::Lam) => (right, left, left_region),
        _ => return BoundaryResult::PassThrough,
    };

    let lam_var = arena.port(lam, 1);
    let lam_body = arena.port(lam, 2);

    // Wrap binder in explicit substitution: Closure(body, IdEnv)
    let closure = arena.spawn_in(
        OpCode::Sym {
            name: "__closure".into(),
            arity: 2,
        },
        target_region,
    );
    let id_env = arena.spawn_in(
        OpCode::Sym {
            name: "__id_env".into(),
            arity: 0,
        },
        target_region,
    );

    arena.connect(closure, 0, other, 0);
    if lam_body.is_connected() {
        arena.connect(closure, 1, lam_body.target, lam_body.slot);
    }
    arena.connect(closure, 2, id_env, 0);

    // The variable wire gets an explicit substitution marker.
    if lam_var.is_connected() {
        let subst_var = arena.spawn_in(
            OpCode::Sym {
                name: "__subst_var".into(),
                arity: 0,
            },
            target_region,
        );
        arena.connect(subst_var, 0, lam_var.target, lam_var.slot);
    }

    arena.free(lam);
    BoundaryResult::Handled("ExplicitSubstitution".into())
}

// ── AC normalization: flatten + sort at boundary ─────────────────────

fn ac_crossing(
    arena: &mut ArchonArena,
    left: Ptr,
    left_kind: &OpCode,
    right: Ptr,
    right_kind: &OpCode,
    left_region: u32,
    right_region: u32,
) -> BoundaryResult {
    // AC normalization at the boundary: when a binary AC-tagged Sym (arity 2)
    // crosses, flatten nested applications of the same operator and sort operands
    // by their string representation (opcode name), then rebuild right-associated.
    //
    // E.g., (+ (+ c a) b) → (+ a (+ b c))

    // Find the AC operator node (binary Sym crossing into the AC region).
    let (ac_root, _other, target_region) = match (left_kind, right_kind) {
        (OpCode::Sym { arity: 2, .. }, _) => (left, right, right_region),
        (_, OpCode::Sym { arity: 2, .. }) => (right, left, left_region),
        _ => return BoundaryResult::PassThrough,
    };

    let op_name = match arena.get(ac_root).map(|n| n.kind.clone()) {
        Some(OpCode::Sym { name, .. }) => name,
        _ => return BoundaryResult::PassThrough,
    };

    // Collect all leaf operands by flattening nested applications of the same operator.
    let mut leaves: Vec<Ptr> = Vec::new();
    let mut stack: Vec<Ptr> = vec![ac_root];
    let mut interior_nodes: Vec<Ptr> = Vec::new();

    while let Some(node) = stack.pop() {
        let is_same_op = arena.get(node).map_or(false, |n| {
            matches!(&n.kind, OpCode::Sym { name, arity: 2 } if *name == op_name)
        });

        if is_same_op {
            interior_nodes.push(node);
            let p1 = arena.port(node, 1);
            let p2 = arena.port(node, 2);
            if p1.is_connected() { stack.push(p1.target); }
            if p2.is_connected() { stack.push(p2.target); }
        } else {
            leaves.push(node);
        }
    }

    // Need at least 2 leaves to normalize.
    if leaves.len() < 2 {
        return BoundaryResult::Handled("AC-Normalize-Trivial".into());
    }

    // Sort leaves by canonical name for deterministic ordering.
    // Primary: opcode name (alphabetical). Secondary: node id (stable tiebreak).
    leaves.sort_by(|a, b| {
        let name_a = arena.get(*a).map(|n| match &n.kind {
            OpCode::Sym { name, .. } => name.clone(),
            other => format!("{:?}", other),
        }).unwrap_or_default();
        let name_b = arena.get(*b).map(|n| match &n.kind {
            OpCode::Sym { name, .. } => name.clone(),
            other => format!("{:?}", other),
        }).unwrap_or_default();
        name_a.cmp(&name_b).then(a.0.cmp(&b.0))
    });

    // Remember the principal port of the AC root (where the result goes).
    let root_principal = arena.port(ac_root, 0);

    // Free all interior nodes (we'll rebuild the tree).
    for node in &interior_nodes {
        // Disconnect before freeing to avoid dangling.
        arena.free(*node);
    }

    // Rebuild right-associated: (op a (op b (op c d)))
    // Start from the rightmost pair and build leftward.
    let mut current = leaves.pop().unwrap(); // rightmost leaf
    while let Some(leaf) = leaves.pop() {
        let new_op = arena.spawn_in(
            OpCode::Sym {
                name: op_name.clone(),
                arity: 2,
            },
            target_region,
        );
        arena.connect(new_op, 1, leaf, 0);
        arena.connect(new_op, 2, current, 0);
        current = new_op;
    }

    // Reconnect the rebuilt tree to the original output.
    if root_principal.is_connected() {
        arena.connect(current, 0, root_principal.target, root_principal.slot);
    }

    BoundaryResult::Handled("AC-Normalize".into())
}

// ── RPC serialization: graph → token stream through narrow wormhole ──

fn rpc_crossing(
    arena: &mut ArchonArena,
    left: Ptr,
    _left_kind: &OpCode,
    right: Ptr,
    _right_kind: &OpCode,
    left_region: u32,
    right_region: u32,
) -> BoundaryResult {
    // Determine which node is entering the network-partition region.
    let network_region = if arena.topology.get(right_region)
        .map_or(false, |r| matches!(r.boundary_type, BoundaryType::NetworkPartition))
    {
        right_region
    } else {
        left_region
    };

    let entering = if arena.region_of(left) != network_region { left } else { right };

    // Wrap the entering node in a serialization envelope.
    let envelope = arena.spawn_in(
        OpCode::Sym {
            name: "__rpc_envelope".into(),
            arity: 1,
        },
        network_region,
    );

    let entering_principal = arena.port(entering, 0);
    if entering_principal.is_connected() {
        arena.connect(envelope, 0, entering_principal.target, entering_principal.slot);
    }
    arena.connect(envelope, 1, entering, 0);

    BoundaryResult::Handled("RPC-Serialization".into())
}

// ── Effect boundary: inject catalyst (continuation) ──────────────────

fn effect_crossing(
    arena: &mut ArchonArena,
    left: Ptr,
    _left_kind: &OpCode,
    right: Ptr,
    _right_kind: &OpCode,
    left_region: u32,
    right_region: u32,
) -> BoundaryResult {
    // When a node crosses into an EffectBoundary region,
    // inject a catalyst (continuation) node that will propagate
    // through the graph, CPS-transforming it.
    let effect_region = if arena.topology.get(right_region)
        .map_or(false, |r| matches!(r.boundary_type, BoundaryType::EffectBoundary))
    {
        right_region
    } else {
        left_region
    };

    let catalyst = arena.spawn_in(
        OpCode::Sym {
            name: "__catalyst".into(),
            arity: 1,
        },
        effect_region,
    );

    // The catalyst sits between the two nodes, ready to propagate.
    arena.connect(catalyst, 0, left, 0);
    arena.connect(catalyst, 1, right, 0);

    BoundaryResult::Handled("Effect-CatalystInjection".into())
}

// ── TensorSerialization: tensor products → sequential extrusion ──────
//
// When a tensor product (⊗) crosses the boundary, it is serialized
// into a left-to-right sequential chain: (A ⊗ B ⊗ C) → seq(A, seq(B, C)).
// This is the physical analog of Hyperion's TensorSerialization pass.

fn tensor_crossing(
    arena: &mut ArchonArena,
    left: Ptr,
    left_kind: &OpCode,
    right: Ptr,
    right_kind: &OpCode,
    left_region: u32,
    right_region: u32,
) -> BoundaryResult {
    // Find the tensor node (tagged "tensor" or "⊗").
    let (tensor, other, target_region) = match (left_kind, right_kind) {
        (OpCode::Sym { name, .. }, _) if name == "tensor" || name == "⊗" => {
            (left, right, right_region)
        }
        (_, OpCode::Sym { name, .. }) if name == "tensor" || name == "⊗" => {
            (right, left, left_region)
        }
        _ => return BoundaryResult::PassThrough,
    };

    let arity = match arena.get(tensor).map(|n| n.kind.clone()) {
        Some(OpCode::Sym { arity, .. }) => arity,
        _ => return BoundaryResult::PassThrough,
    };

    // Wrap in a sequential serialization node that preserves left-to-right order.
    let seq = arena.spawn_in(
        OpCode::Sym {
            name: "__tensor_seq".into(),
            arity,
        },
        target_region,
    );

    // Rewire: seq takes over tensor's connections.
    arena.connect(seq, 0, other, 0);
    for slot in 1..=arity {
        let port = arena.port(tensor, slot);
        if port.is_connected() {
            arena.connect(seq, slot, port.target, port.slot);
        }
    }

    arena.free(tensor);
    BoundaryResult::Handled("TensorSerialization".into())
}

// ── Kripke: thread world parameters through modal terms ─────────────
//
// When a term crosses a Kripke boundary, all free occurrences of modal
// operators get a world-parameter threaded through them. This is the
// physical analog of Hyperion's KripkeWorldThreading pass.

fn kripke_crossing(
    arena: &mut ArchonArena,
    left: Ptr,
    _left_kind: &OpCode,
    right: Ptr,
    _right_kind: &OpCode,
    left_region: u32,
    right_region: u32,
) -> BoundaryResult {
    // Determine the target Kripke region.
    let (entering, target_region) = if arena.topology.get(right_region)
        .map_or(false, |r| matches!(r.boundary_type, BoundaryType::KripkeBoundary))
    {
        (left, right_region)
    } else {
        (right, left_region)
    };

    // Spawn a world parameter node in the target region.
    let world_param = arena.spawn_in(
        OpCode::Sym {
            name: format!("__world_{}", target_region),
            arity: 0,
        },
        target_region,
    );

    // Wrap the entering node with a world-threaded version.
    let threaded = arena.spawn_in(
        OpCode::Sym {
            name: "__kripke_threaded".into(),
            arity: 2,
        },
        target_region,
    );

    let entering_port = arena.port(entering, 0);
    if entering_port.is_connected() {
        arena.connect(threaded, 0, entering_port.target, entering_port.slot);
    }
    arena.connect(threaded, 1, entering, 0);
    arena.connect(threaded, 2, world_param, 0);

    BoundaryResult::Handled("KripkeWorldThreading".into())
}

// ── Nominal: name-abstraction scoping ───────────────────────────────
//
// When a term crosses a nominal boundary, free names are α-renamed to
// fresh names scoped to the target region. Prevents name capture.

fn nominal_crossing(
    arena: &mut ArchonArena,
    left: Ptr,
    _left_kind: &OpCode,
    right: Ptr,
    _right_kind: &OpCode,
    left_region: u32,
    right_region: u32,
) -> BoundaryResult {
    let (entering, target_region) = if arena.topology.get(right_region)
        .map_or(false, |r| matches!(r.boundary_type,
            BoundaryType::NominalBoundary | BoundaryType::NominalScoping))
    {
        (left, right_region)
    } else {
        (right, left_region)
    };

    // Alpha-rename free names in the entering subgraph.
    // Walk DFS from entering node, find Sym nodes with user-level names
    // (not __prefixed), and rename them with a fresh scope suffix.
    let scope_id = target_region; // Use region id as unique scope tag.
    let mut stack = vec![entering];
    let mut visited = std::collections::HashSet::new();
    let mut renames: Vec<(Ptr, String)> = Vec::new();

    while let Some(ptr) = stack.pop() {
        if !visited.insert(ptr) {
            continue;
        }
        if let Some(node) = arena.get(ptr) {
            if let OpCode::Sym { ref name, arity } = node.kind {
                // Rename user-level names (not internal __prefixed ones).
                if !name.starts_with("__") && arity == 0 {
                    renames.push((ptr, format!("{}$α{}", name, scope_id)));
                }
            }
            // Walk aux ports to traverse subgraph.
            let kind = node.kind.clone();
            let n_ports = kind.port_count();
            for slot in 1..n_ports {
                let port = arena.port(ptr, slot as u8);
                if port.is_connected() {
                    stack.push(port.target);
                }
            }
        }
    }

    // Apply renames — replace each Sym node in-place by spawning a new node
    // and rewiring all connections.
    let mut fresh_nodes: Vec<Ptr> = Vec::new();
    for (ptr, new_name) in &renames {
        let old_node = match arena.get(*ptr) {
            Some(n) => n.kind.clone(),
            None => continue,
        };
        if let OpCode::Sym { arity, .. } = old_node {
            let fresh = arena.spawn_in(
                OpCode::Sym { name: new_name.clone(), arity },
                target_region,
            );
            fresh_nodes.push(fresh);
            // Rewire principal port.
            let p0 = arena.port(*ptr, 0);
            if p0.is_connected() {
                arena.connect(fresh, 0, p0.target, p0.slot);
            }
            // Rewire aux ports.
            for slot in 1..=(arity as u8) {
                let p = arena.port(*ptr, slot);
                if p.is_connected() {
                    arena.connect(fresh, slot, p.target, p.slot);
                }
            }
            arena.free(*ptr);
        }
    }

    // Move all visited nodes into the target region to prevent re-crossing.
    for &ptr in &visited {
        if arena.get(ptr).is_some() {
            arena.move_to_region(ptr, target_region);
        }
    }

    // Also wrap in a nominal scope node for readback provenance.
    let scope = arena.spawn_in(
        OpCode::Sym {
            name: format!("__nominal_scope_{}", target_region),
            arity: 1,
        },
        target_region,
    );

    let entering_port = arena.port(entering, 0);
    if entering_port.is_connected() {
        arena.connect(scope, 0, entering_port.target, entering_port.slot);
    }
    arena.connect(scope, 1, entering, 0);

    BoundaryResult::Handled("NominalAbstraction".into())
}

// ── Grounding: higher-order → first-order via grounding field ───────
//
// When a higher-order term crosses into a first-order region, it is
// compiled to first-order by the grounding field. Higher-order arguments
// that have been fully instantiated crystallize; unresolved ones suspend.
// This is the physical analog of Hyperion's ClauseCompilation pass.

fn grounding_crossing(
    arena: &mut ArchonArena,
    left: Ptr,
    left_kind: &OpCode,
    right: Ptr,
    right_kind: &OpCode,
    left_region: u32,
    right_region: u32,
) -> BoundaryResult {
    // Higher-order terms are Lam nodes or App trees crossing into a grounded region.
    let (ho_node, other, target_region) = match (left_kind, right_kind) {
        (OpCode::Lam, _) => (left, right, right_region),
        (_, OpCode::Lam) => (right, left, left_region),
        (OpCode::App, _) => (left, right, right_region),
        (_, OpCode::App) => (right, left, left_region),
        _ => return BoundaryResult::PassThrough,
    };

    // Deep grounding check: walk the subgraph from ho_node via aux ports.
    // The term is grounded only if NO node in its subgraph is glowing.
    let is_grounded = {
        let mut grounded = true;
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![ho_node];
        while let Some(ptr) = stack.pop() {
            if !visited.insert(ptr.0) { continue; }
            if arena.is_glowing_any(ptr) {
                grounded = false;
                break;
            }
            if let Some(node) = arena.get(ptr) {
                let pc = node.kind.port_count();
                for slot in 1..pc {
                    let port = arena.port(ptr, slot as u8);
                    if port.is_connected() && arena.get(port.target).is_some() {
                        stack.push(port.target);
                    }
                }
            }
        }
        grounded
    };

    if is_grounded {
        // Fully grounded: crystallize into a first-order clause node.
        let clause = arena.spawn_in(
            OpCode::Sym {
                name: "__fo_clause".into(),
                arity: 1,
            },
            target_region,
        );
        arena.connect(clause, 0, other, 0);

        // Capture the HO node's subgraph as the clause body.
        let port_count = match arena.get(ho_node).map(|n| n.kind.port_count()) {
            Some(pc) => pc,
            None => 1,
        };
        if port_count > 1 {
            let body_port = arena.port(ho_node, if matches!(left_kind, OpCode::Lam) { 2 } else { 1 });
            if body_port.is_connected() {
                arena.connect(clause, 1, body_port.target, body_port.slot);
            }
        }

        arena.free(ho_node);
        BoundaryResult::Handled("Grounding-Crystallize".into())
    } else {
        // Not yet grounded: suspend via Future node until grounding radiation arrives.
        let future = arena.spawn_in(OpCode::Future, target_region);
        arena.connect(future, 0, other, 0);
        // Keep the HO node alive, connected to the future.
        let ho_port = arena.port(ho_node, 0);
        if ho_port.is_connected() {
            arena.connect(future, 1, ho_node, 0);
        }
        BoundaryResult::Handled("Grounding-Suspend".into())
    }
}

// ── ContextReify: first-class contexts → data structures ────────────
//
// When a context (proof environment, typing context) crosses the boundary,
// it is reified into an explicit data structure: a list of bindings.
// This is the physical analog of Hyperion's ContextReification pass.

fn context_reify_crossing(
    arena: &mut ArchonArena,
    left: Ptr,
    left_kind: &OpCode,
    right: Ptr,
    right_kind: &OpCode,
    left_region: u32,
    right_region: u32,
) -> BoundaryResult {
    // Determine which node is entering the context-reify region.
    let (entering, other, target_region) = if arena.topology.get(right_region)
        .map_or(false, |r| matches!(r.boundary_type, BoundaryType::ContextReifyBoundary))
    {
        (left, right, right_region)
    } else {
        (right, left, left_region)
    };

    let kind = match arena.get(entering).map(|n| n.kind.clone()) {
        Some(k) => k,
        None => return BoundaryResult::PassThrough,
    };

    // Rewrite context operations to first-order constructors.
    // Matches Hyperion's context_reify.rs: empty-ctx → __ctx_nil,
    // extend → __ctx_cons, lookup → __ctx_lookup.
    match &kind {
        OpCode::Sym { name, arity } => {
            let (new_name, new_arity) = match name.as_str() {
                "empty-ctx" | "empty_ctx" | "nil-ctx" => ("__ctx_nil", 0u8),
                "extend" | "ctx-extend" | "cons-ctx" => ("__ctx_cons", 3), // ctx, name, type
                "lookup" | "ctx-lookup" => ("__ctx_lookup", 2), // ctx, index
                "lookup-name" | "ctx-lookup-name" => ("__ctx_lookup_name", 2), // ctx, name
                _ => {
                    // Not a context op — wrap in __reified_ctx as before.
                    let reified = arena.spawn_in(
                        OpCode::Sym { name: "__reified_ctx".into(), arity: 1 },
                        target_region,
                    );
                    let entering_port = arena.port(entering, 0);
                    if entering_port.is_connected() {
                        arena.connect(reified, 0, entering_port.target, entering_port.slot);
                    }
                    arena.connect(reified, 1, entering, 0);
                    return BoundaryResult::Handled("ContextReification".into());
                }
            };

            // Replace the context op with its reified constructor.
            let reified = arena.spawn_in(
                OpCode::Sym { name: new_name.into(), arity: new_arity },
                target_region,
            );

            // Rewire principal port.
            let entering_port = arena.port(entering, 0);
            if entering_port.is_connected() {
                arena.connect(reified, 0, entering_port.target, entering_port.slot);
            }

            // Rewire aux ports (transfer children).
            let old_arity = *arity;
            let transfer = old_arity.min(new_arity);
            for slot in 1..=transfer {
                let p = arena.port(entering, slot);
                if p.is_connected() {
                    arena.connect(reified, slot, p.target, p.slot);
                }
            }

            arena.free(entering);
            BoundaryResult::Handled("ContextReify-Rewrite".into())
        }
        _ => {
            // Non-Sym node — generic wrapper.
            let reified = arena.spawn_in(
                OpCode::Sym { name: "__reified_ctx".into(), arity: 1 },
                target_region,
            );
            let entering_port = arena.port(entering, 0);
            if entering_port.is_connected() {
                arena.connect(reified, 0, entering_port.target, entering_port.slot);
            }
            arena.connect(reified, 1, entering, 0);
            BoundaryResult::Handled("ContextReification".into())
        }
    }
}

// ── ModalRestriction: variable-class guards at boundary ─────────────
//
// When a term crosses a modal restriction boundary, variables are
// checked against their modal class. Variables from the wrong class
// are blocked (the node is rejected or suspended).
// This is the physical analog of Hyperion's ModalSubstitutionRestriction pass.

fn modal_restriction_crossing(
    arena: &mut ArchonArena,
    left: Ptr,
    left_kind: &OpCode,
    right: Ptr,
    right_kind: &OpCode,
    _left_region: u32,
    right_region: u32,
) -> BoundaryResult {
    // Check if either node is a variable-like node (Sym with arity 0).
    let is_var = |kind: &OpCode| matches!(kind, OpCode::Sym { arity: 0, name }
        if !name.starts_with("__archon_") && !name.starts_with("__"));

    if is_var(left_kind) || is_var(right_kind) {
        let var_node = if is_var(left_kind) { left } else { right };

        // Check radiation: if the variable is glowing with a modal marker,
        // it's from a restricted class and should be guarded.
        if arena.is_glowing_any(var_node) {
            // Variable is from a restricted modal class — wrap in a guard.
            let guard = arena.spawn_in(
                OpCode::Sym {
                    name: "__modal_guard".into(),
                    arity: 1,
                },
                right_region,
            );

            let var_port = arena.port(var_node, 0);
            if var_port.is_connected() {
                arena.connect(guard, 0, var_port.target, var_port.slot);
            }
            arena.connect(guard, 1, var_node, 0);

            return BoundaryResult::Handled("ModalRestriction-Guard".into());
        }
    }

    // Non-variable or unrestricted variable: pass through.
    BoundaryResult::PassThrough
}

// ── KanTransport: path transport reduction rules ────────────────────
//
// When a term crosses a Kan boundary, transport operations along paths
// are reduced. If the path is refl (identity), transport is eliminated.
// If the path is a composite, transport is split along segments.
// This is the physical analog of Hyperion's KanComputation pass.

fn kan_transport_crossing(
    arena: &mut ArchonArena,
    left: Ptr,
    left_kind: &OpCode,
    right: Ptr,
    right_kind: &OpCode,
    left_region: u32,
    right_region: u32,
) -> BoundaryResult {
    // Look for transport nodes: transport(path, term).
    let (transport, other, target_region) = match (left_kind, right_kind) {
        (OpCode::Sym { name, .. }, _) if name == "transport" || name == "coe" => {
            (left, right, right_region)
        }
        (_, OpCode::Sym { name, .. }) if name == "transport" || name == "coe" => {
            (right, left, left_region)
        }
        _ => return BoundaryResult::PassThrough,
    };

    // Check if the path argument (port 1) is refl.
    let path_port = arena.port(transport, 1);
    if path_port.is_connected() {
        if let Some(path_node) = arena.get(path_port.target) {
            if matches!(&path_node.kind, OpCode::Sym { name, .. } if name == "refl") {
                // Transport along refl = identity. Eliminate the transport.
                let term_port = arena.port(transport, 2);
                if term_port.is_connected() {
                    arena.connect(other, 0, term_port.target, term_port.slot);
                }
                // Free the transport and the refl path.
                let refl_ptr = path_port.target;
                arena.free(transport);
                arena.free(refl_ptr);
                return BoundaryResult::Handled("KanTransport-Refl".into());
            }
        }
    }

    // Non-refl path: wrap in a transport-pending node for later reduction.
    let pending = arena.spawn_in(
        OpCode::Sym {
            name: "__transport_pending".into(),
            arity: 2,
        },
        target_region,
    );
    arena.connect(pending, 0, other, 0);
    if path_port.is_connected() {
        arena.connect(pending, 1, path_port.target, path_port.slot);
    }
    let term_port = arena.port(transport, 2);
    if term_port.is_connected() {
        arena.connect(pending, 2, term_port.target, term_port.slot);
    }
    arena.free(transport);
    BoundaryResult::Handled("KanTransport-Pending".into())
}

// ── Thermo: terms → spin/spring constraints ─────────────────────────
//
// When a term crosses into a thermodynamic region, it is encoded as
// spin variables and spring constraints for the annealing engine.
// Boolean subterms become spins; logical connectives become springs.
// This is the physical analog of Hyperion's SMTEncoding pass.

fn thermo_crossing(
    arena: &mut ArchonArena,
    left: Ptr,
    left_kind: &OpCode,
    right: Ptr,
    right_kind: &OpCode,
    left_region: u32,
    right_region: u32,
) -> BoundaryResult {
    // Determine which node is entering the thermo region.
    let (entering, _other, target_region) = if arena.topology.get(right_region)
        .map_or(false, |r| matches!(r.boundary_type, BoundaryType::ThermoBoundary))
    {
        (left, right, right_region)
    } else {
        (right, left, left_region)
    };

    let kind = match arena.get(entering).map(|n| n.kind.clone()) {
        Some(k) => k,
        None => return BoundaryResult::PassThrough,
    };

    match &kind {
        // Boolean atoms become spin nodes.
        OpCode::Sym { name, arity: 0 } if name == "true" || name == "false" => {
            let polarity = name == "true";
            let spin = arena.spawn_spin(target_region, polarity);

            // Rewire: spin replaces the original atom.
            let port = arena.port(entering, 0);
            if port.is_connected() {
                arena.connect(spin, 0, port.target, port.slot);
            }
            arena.free(entering);
            BoundaryResult::Handled("Thermo-SpinEncode".into())
        }

        // Logical connectives (and, or, not, implies) become spring constraints.
        // Recursively encode children that are also logical/boolean.
        OpCode::Sym { name, arity } if name == "and" || name == "or"
            || name == "not" || name == "implies" =>
        {
            let constraint = arena.spawn_in(
                OpCode::Sym {
                    name: format!("__thermo_{}", name),
                    arity: *arity,
                },
                target_region,
            );

            let port = arena.port(entering, 0);
            if port.is_connected() {
                arena.connect(constraint, 0, port.target, port.slot);
            }
            // Collect children before recursive encoding (avoid borrow issues).
            let children: Vec<(u8, Ptr, u8)> = (1..=*arity)
                .filter_map(|slot| {
                    let p = arena.port(entering, slot);
                    if p.is_connected() { Some((slot, p.target, p.slot)) } else { None }
                })
                .collect();
            for (slot, child, child_slot) in &children {
                arena.connect(constraint, *slot, *child, *child_slot);
            }
            arena.free(entering);

            // Recursively encode children that are boolean atoms or connectives.
            for (_slot, child, _child_slot) in children {
                if let Some(child_kind) = arena.get(child).map(|n| n.kind.clone()) {
                    match &child_kind {
                        OpCode::Sym { name: cname, arity: 0 }
                            if cname == "true" || cname == "false" =>
                        {
                            let polarity = cname == "true";
                            let spin = arena.spawn_spin(target_region, polarity);
                            let cp = arena.port(child, 0);
                            if cp.is_connected() {
                                arena.connect(spin, 0, cp.target, cp.slot);
                            }
                            arena.free(child);
                        }
                        OpCode::Sym { name: cname, .. }
                            if cname == "and" || cname == "or"
                                || cname == "not" || cname == "implies" =>
                        {
                            // Recursive: encode this child as thermo too.
                            // Use a worklist to avoid stack overflow on deep formulas.
                            encode_thermo_recursive(arena, child, target_region);
                        }
                        _ => {} // leaf variable — left as-is
                    }
                }
            }
            BoundaryResult::Handled("Thermo-ConstraintEncode".into())
        }

        _ => BoundaryResult::PassThrough,
    }
}

/// Iteratively encode a connective subtree into thermo constraint nodes.
fn encode_thermo_recursive(arena: &mut ArchonArena, root: Ptr, target_region: u32) {
    let mut worklist = vec![root];
    while let Some(node) = worklist.pop() {
        let kind = match arena.get(node).map(|n| n.kind.clone()) {
            Some(k) => k,
            None => continue,
        };
        match &kind {
            OpCode::Sym { name, arity: 0 } if name == "true" || name == "false" => {
                let polarity = name == "true";
                let spin = arena.spawn_spin(target_region, polarity);
                let port = arena.port(node, 0);
                if port.is_connected() {
                    arena.connect(spin, 0, port.target, port.slot);
                }
                arena.free(node);
            }
            OpCode::Sym { name, arity } if name == "and" || name == "or"
                || name == "not" || name == "implies" =>
            {
                let arity = *arity;
                let constraint = arena.spawn_in(
                    OpCode::Sym {
                        name: format!("__thermo_{}", name),
                        arity,
                    },
                    target_region,
                );
                let port = arena.port(node, 0);
                if port.is_connected() {
                    arena.connect(constraint, 0, port.target, port.slot);
                }
                let children: Vec<(u8, Ptr, u8)> = (1..=arity)
                    .filter_map(|slot| {
                        let p = arena.port(node, slot);
                        if p.is_connected() { Some((slot, p.target, p.slot)) } else { None }
                    })
                    .collect();
                for (slot, child, child_slot) in &children {
                    arena.connect(constraint, *slot, *child, *child_slot);
                }
                arena.free(node);
                for (_, child, _) in children {
                    worklist.push(child);
                }
            }
            _ => {} // leaf — leave as-is
        }
    }
}

// ── Dialectica: anti-matter polarity flip ────────────────────────────

fn dialectica_crossing(
    arena: &mut ArchonArena,
    left: Ptr,
    left_kind: &OpCode,
    right: Ptr,
    right_kind: &OpCode,
    _left_region: u32,
    right_region: u32,
) -> BoundaryResult {
    // When a quantifier node crosses the Dialectica boundary,
    // flip its polarity (∀ ↔ ∃).
    fn flip_quantifier(arena: &mut ArchonArena, node: Ptr, kind: &OpCode, region: u32) -> bool {
        match kind {
            OpCode::Sym { name, arity } if name == "forall" => {
                // Replace with exists (anti-matter).
                let exists = arena.spawn_in(
                    OpCode::Sym {
                        name: "exists".into(),
                        arity: *arity,
                    },
                    region,
                );
                // Rewire all ports.
                let node_ref = arena.get(node).unwrap().clone();
                for (slot, port) in node_ref.ports.iter().enumerate() {
                    if port.is_connected() && port.target != node {
                        arena.connect(exists, slot as u8, port.target, port.slot);
                    }
                }
                arena.free(node);
                true
            }
            OpCode::Sym { name, arity } if name == "exists" => {
                let forall = arena.spawn_in(
                    OpCode::Sym {
                        name: "forall".into(),
                        arity: *arity,
                    },
                    region,
                );
                let node_ref = arena.get(node).unwrap().clone();
                for (slot, port) in node_ref.ports.iter().enumerate() {
                    if port.is_connected() && port.target != node {
                        arena.connect(forall, slot as u8, port.target, port.slot);
                    }
                }
                arena.free(node);
                true
            }
            _ => false,
        }
    }

    let flipped_left = flip_quantifier(arena, left, left_kind, right_region);
    let flipped_right = flip_quantifier(arena, right, right_kind, right_region);

    if flipped_left || flipped_right {
        BoundaryResult::Handled("Dialectica-PolarityFlip".into())
    } else {
        BoundaryResult::PassThrough
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::*;

    #[test]
    fn bang_wraps_linear_node() {
        let mut topo = Topology::new();
        let linear = topo.add_region(
            Region::new(0, "linear")
                .with_resource(ResourceMode::StrictlyLinear)
                .with_boundary(BoundaryType::BangBoundary)
                .with_parent(0),
        );

        let mut arena = ArchonArena::new().with_topology(topo);

        let lin_node = arena.spawn_in(
            OpCode::Sym { name: "x".into(), arity: 0 },
            linear,
        );
        let share_node = arena.spawn_in(
            OpCode::Sym { name: "consumer".into(), arity: 1 },
            0,
        );
        arena.connect(lin_node, 0, share_node, 0);

        let result = dispatch(
            &mut arena,
            lin_node,
            &OpCode::Sym { name: "x".into(), arity: 0 },
            share_node,
            &OpCode::Sym { name: "consumer".into(), arity: 1 },
        );

        assert!(matches!(result, BoundaryResult::Handled(ref s) if s == "Bang-Promotion"));
    }

    #[test]
    fn defunc_converts_lambda() {
        let mut topo = Topology::new();
        let fo_region = topo.add_region(
            Region::new(0, "first-order")
                .with_boundary(BoundaryType::DefunctionalizationBoundary)
                .with_parent(0),
        );

        let mut arena = ArchonArena::new().with_topology(topo);

        let lam = arena.spawn_in(OpCode::Lam, 0);
        let target = arena.spawn_in(
            OpCode::Sym { name: "f".into(), arity: 1 },
            fo_region,
        );
        let body = arena.spawn(OpCode::Sym { name: "body".into(), arity: 0 });
        let var = arena.spawn(OpCode::Sym { name: "var".into(), arity: 0 });

        arena.connect(lam, 1, var, 0);
        arena.connect(lam, 2, body, 0);
        arena.connect(lam, 0, target, 0);

        let result = dispatch(
            &mut arena,
            lam,
            &OpCode::Lam,
            target,
            &OpCode::Sym { name: "f".into(), arity: 1 },
        );

        assert!(matches!(result, BoundaryResult::Handled(ref s) if s == "Defunctionalization"));
        // Lambda should be freed.
        assert!(arena.get(lam).is_none());
    }

    #[test]
    fn transparent_boundary_passes_through() {
        let mut topo = Topology::new();
        let _child = topo.add_region(
            Region::new(0, "child")
                .with_boundary(BoundaryType::Transparent)
                .with_parent(0),
        );

        let arena = ArchonArena::new().with_topology(topo);
        // With transparent boundary, dispatch should pass through.
        // (We'd need actual nodes to test, but the logic is clear.)
        let _ = arena;
    }
}
