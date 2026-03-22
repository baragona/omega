//! Tactic-based proof engine for cosmological metatheory.
//!
//! Proves properties algebraically over the lattice structure of epistemic axes,
//! WITHOUT referencing concrete worlds. This is the "dangerous lift" — a Law
//! proved here holds for every admissible world, even ones not yet imagined.
//!
//! Syntax:
//!   [Law DominanceTransitivity
//!     :forall [?W1 ?W2 ?W3 :type World]
//!     :assume [[dominates ?W1 ?W2] [dominates ?W2 ?W3]]
//!     :show [dominates ?W1 ?W3]
//!     :method proof
//!     :proof [
//!       [unfold dominates]
//!       [intros-axis ?A]
//!       [apply lattice-transitivity :on ?A]
//!       [qed]
//!     ]
//!   ]

use apeiron::parser::Sexp;
use crate::error::{MetacosmError, Result};

// ============================================================================
// Propositions: the internal logic of the proof engine
// ============================================================================

/// A proposition in the proof engine's internal logic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prop {
    /// w1's axis ≥ w2's axis (per-axis comparison)
    AxisGeq { w1: String, axis: String, w2: String },
    /// w1 epistemically dominates w2 (conjunction of all axes)
    Dominates { w1: String, w2: String },
    /// Transition is faithful (injective on morphisms)
    Faithful { transition: String },
    /// Function/morphism is injective
    Injective { func: String },
    /// Function/morphism is NOT injective (information-destroying)
    NotInjective { func: String },
    /// Transition has a specific transport mode
    TransportMode { transition: String, mode: String },
    /// Transition preserves an invariant
    Preserves { transition: String, invariant: String },
    /// Two things are equal
    Eq { a: String, b: String },
    /// Universal quantification over axes
    ForAllAxis { var: String, body: Box<Prop> },
    /// Conjunction
    And(Vec<Prop>),
    /// Bottom (contradiction / absurdity)
    Contradiction,
}

impl std::fmt::Display for Prop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Prop::AxisGeq { w1, axis, w2 } => write!(f, "{}.{} ≥ {}.{}", w1, axis, w2, axis),
            Prop::Dominates { w1, w2 } => write!(f, "dominates({}, {})", w1, w2),
            Prop::Faithful { transition } => write!(f, "faithful({})", transition),
            Prop::Injective { func } => write!(f, "injective({})", func),
            Prop::NotInjective { func } => write!(f, "¬injective({})", func),
            Prop::TransportMode { transition, mode } => write!(f, "transport-mode({}, {})", transition, mode),
            Prop::Preserves { transition, invariant } => write!(f, "preserves({}, {})", transition, invariant),
            Prop::Eq { a, b } => write!(f, "{} = {}", a, b),
            Prop::ForAllAxis { var, body } => write!(f, "∀{}. {}", var, body),
            Prop::And(ps) => {
                let strs: Vec<String> = ps.iter().map(|p| format!("{}", p)).collect();
                write!(f, "({})", strs.join(" ∧ "))
            }
            Prop::Contradiction => write!(f, "⊥"),
        }
    }
}

// ============================================================================
// Variable types for typed quantification
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarType {
    World,
    Transition { from: Option<String>, to: Option<String> },
    Axis,
}

impl std::fmt::Display for VarType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VarType::World => write!(f, "World"),
            VarType::Transition { from, to } => {
                write!(f, "Transition")?;
                if let Some(s) = from { write!(f, " :from {}", s)?; }
                if let Some(t) = to { write!(f, " :to {}", t)?; }
                Ok(())
            }
            VarType::Axis => write!(f, "Axis"),
        }
    }
}

// ============================================================================
// Tactics: the user-facing proof commands
// ============================================================================

#[derive(Debug, Clone)]
pub enum Tactic {
    /// Expand a definition in the goal (dominates, faithful, lossy)
    Unfold { name: String },
    /// Introduce an axis variable from a ∀Axis quantifier
    IntrosAxis { var: String },
    /// Apply a known axiom/rule
    Apply { rule: String, args: Vec<String> },
    /// Close goal when context contains P and ¬P
    Contradiction,
    /// Assert proof is complete
    Qed,
    /// Split a conjunction goal into subgoals
    Split,
    /// Close goal from an identical hypothesis in context
    Assumption,
    /// Introduce a derived fact with justification
    Have { name: String, prop_desc: String, justification: String },
}

impl std::fmt::Display for Tactic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tactic::Unfold { name } => write!(f, "unfold {}", name),
            Tactic::IntrosAxis { var } => write!(f, "intros-axis {}", var),
            Tactic::Apply { rule, args } => {
                write!(f, "apply {}", rule)?;
                if !args.is_empty() {
                    write!(f, " :on {}", args.join(" "))?;
                }
                Ok(())
            }
            Tactic::Contradiction => write!(f, "contradiction"),
            Tactic::Qed => write!(f, "qed"),
            Tactic::Split => write!(f, "split"),
            Tactic::Assumption => write!(f, "assumption"),
            Tactic::Have { name, prop_desc, justification } => {
                write!(f, "have {} : {} by {}", name, prop_desc, justification)
            }
        }
    }
}

// ============================================================================
// Proof state
// ============================================================================

/// The state of a proof in progress.
#[derive(Debug, Clone)]
pub struct ProofState {
    /// Named hypotheses in context.
    pub context: Vec<(String, Prop)>,
    /// Remaining goals (stack — first is current).
    pub goals: Vec<Prop>,
    /// Quantified variables and their types.
    pub variables: Vec<(String, VarType)>,
    /// Trace of applied tactics (for the certificate).
    pub trace: Vec<String>,
}

impl ProofState {
    /// Create initial proof state from premises (context) and conclusion (goal).
    pub fn new(
        variables: Vec<(String, VarType)>,
        premises: Vec<(String, Prop)>,
        conclusion: Prop,
    ) -> Self {
        ProofState {
            context: premises,
            goals: vec![conclusion],
            variables,
            trace: Vec::new(),
        }
    }

    /// Is the proof complete? (No remaining goals)
    pub fn is_done(&self) -> bool {
        self.goals.is_empty()
    }

    /// The current goal (top of stack).
    pub fn current_goal(&self) -> Option<&Prop> {
        self.goals.first()
    }

    /// Find a hypothesis by scanning context.
    fn find_in_context(&self, prop: &Prop) -> Option<&str> {
        self.context.iter()
            .find(|(_, p)| p == prop)
            .map(|(name, _)| name.as_str())
    }
}

// ============================================================================
// Proof engine: evaluate tactics against proof state
// ============================================================================

/// Result of a successful proof.
#[derive(Debug, Clone)]
pub struct ProofCertificate {
    pub theorem: String,
    pub trace: Vec<String>,
}

/// Evaluate a proof script against initial state.
pub fn evaluate_proof(
    theorem_name: &str,
    mut state: ProofState,
    tactics: &[Tactic],
) -> Result<ProofCertificate> {
    for (i, tactic) in tactics.iter().enumerate() {
        state.trace.push(format!("  step {}: {}", i + 1, tactic));

        match tactic {
            Tactic::Unfold { name } => apply_unfold(&mut state, name)?,
            Tactic::IntrosAxis { var } => apply_intros_axis(&mut state, var)?,
            Tactic::Apply { rule, args } => apply_rule(&mut state, rule, args)?,
            Tactic::Contradiction => apply_contradiction(&mut state)?,
            Tactic::Split => apply_split(&mut state)?,
            Tactic::Assumption => apply_assumption(&mut state)?,
            Tactic::Have { name, prop_desc, justification } => {
                apply_have(&mut state, name, prop_desc, justification)?;
            }
            Tactic::Qed => {
                if state.is_done() {
                    return Ok(ProofCertificate {
                        theorem: theorem_name.to_string(),
                        trace: state.trace,
                    });
                } else {
                    return Err(MetacosmError::ProofError {
                        theorem: theorem_name.to_string(),
                        detail: format!(
                            "qed called but {} goal(s) remain: {}",
                            state.goals.len(),
                            state.goals.iter().map(|g| format!("{}", g)).collect::<Vec<_>>().join(", ")
                        ),
                    });
                }
            }
        }
    }

    if state.is_done() {
        Ok(ProofCertificate {
            theorem: theorem_name.to_string(),
            trace: state.trace,
        })
    } else {
        Err(MetacosmError::ProofError {
            theorem: theorem_name.to_string(),
            detail: format!(
                "proof script ended but {} goal(s) remain: {}",
                state.goals.len(),
                state.goals.iter().map(|g| format!("{}", g)).collect::<Vec<_>>().join(", ")
            ),
        })
    }
}

// ============================================================================
// Tactic implementations
// ============================================================================

/// `unfold X` — expand definition X in the current goal.
fn apply_unfold(state: &mut ProofState, name: &str) -> Result<()> {
    let goal = state.goals.first().ok_or_else(|| MetacosmError::ProofError {
        theorem: String::new(),
        detail: "unfold: no goal".into(),
    })?;

    let new_goal = unfold_in_prop(goal, name)?;
    state.goals[0] = new_goal;

    // Also unfold in context (hypotheses) for consistency
    let mut new_ctx = Vec::new();
    for (n, p) in &state.context {
        match unfold_in_prop(p, name) {
            Ok(unfolded) => new_ctx.push((n.clone(), unfolded)),
            Err(_) => new_ctx.push((n.clone(), p.clone())),
        }
    }
    state.context = new_ctx;

    Ok(())
}

/// Unfold a definition within a proposition.
fn unfold_in_prop(prop: &Prop, def_name: &str) -> Result<Prop> {
    match (prop, def_name) {
        // dominates(w1, w2) → ∀A. w1.A ≥ w2.A
        (Prop::Dominates { w1, w2 }, "dominates") => {
            Ok(Prop::ForAllAxis {
                var: "__A".to_string(),
                body: Box::new(Prop::AxisGeq {
                    w1: w1.clone(),
                    axis: "__A".to_string(),
                    w2: w2.clone(),
                }),
            })
        }
        // faithful(t) → injective(t.morphism)
        (Prop::Faithful { transition }, "faithful") => {
            Ok(Prop::Injective {
                func: format!("{}.morphism", transition),
            })
        }
        // transport-mode(t, lossy) → ¬injective(t.morphism)
        (Prop::TransportMode { transition, mode }, "lossy") if mode == "lossy" => {
            Ok(Prop::NotInjective {
                func: format!("{}.morphism", transition),
            })
        }
        // Recurse into ForAllAxis
        (Prop::ForAllAxis { var, body }, _) => {
            Ok(Prop::ForAllAxis {
                var: var.clone(),
                body: Box::new(unfold_in_prop(body, def_name)?),
            })
        }
        // Recurse into And
        (Prop::And(ps), _) => {
            let unfolded: Vec<Prop> = ps.iter()
                .map(|p| unfold_in_prop(p, def_name).unwrap_or_else(|_| p.clone()))
                .collect();
            Ok(Prop::And(unfolded))
        }
        _ => Ok(prop.clone()),
    }
}

/// `intros-axis ?A` — strip ForAllAxis from goal, introduce axis variable.
fn apply_intros_axis(state: &mut ProofState, var: &str) -> Result<()> {
    let goal = state.goals.first().ok_or_else(|| MetacosmError::ProofError {
        theorem: String::new(),
        detail: "intros-axis: no goal".into(),
    })?;

    let goal = goal.clone();
    match &goal {
        Prop::ForAllAxis { var: bound_var, body } => {
            let bv = bound_var.clone();
            let new_goal = subst_prop(body, &bv, var);
            state.goals[0] = new_goal;
            state.variables.push((var.to_string(), VarType::Axis));

            let mut new_ctx = Vec::new();
            for (n, p) in &state.context {
                new_ctx.push((n.clone(), subst_prop(p, &bv, var)));
            }
            state.context = new_ctx;

            Ok(())
        }
        _ => Err(MetacosmError::ProofError {
            theorem: String::new(),
            detail: format!("intros-axis: goal is not ∀Axis, got: {}", goal),
        }),
    }
}

/// Substitute a variable name in a proposition.
fn subst_prop(prop: &Prop, from: &str, to: &str) -> Prop {
    match prop {
        Prop::AxisGeq { w1, axis, w2 } => Prop::AxisGeq {
            w1: subst_name(w1, from, to),
            axis: subst_name(axis, from, to),
            w2: subst_name(w2, from, to),
        },
        Prop::Dominates { w1, w2 } => Prop::Dominates {
            w1: subst_name(w1, from, to),
            w2: subst_name(w2, from, to),
        },
        Prop::Faithful { transition } => Prop::Faithful {
            transition: subst_name(transition, from, to),
        },
        Prop::Injective { func } => Prop::Injective {
            func: subst_name(func, from, to),
        },
        Prop::NotInjective { func } => Prop::NotInjective {
            func: subst_name(func, from, to),
        },
        Prop::TransportMode { transition, mode } => Prop::TransportMode {
            transition: subst_name(transition, from, to),
            mode: mode.clone(),
        },
        Prop::Preserves { transition, invariant } => Prop::Preserves {
            transition: subst_name(transition, from, to),
            invariant: invariant.clone(),
        },
        Prop::Eq { a, b } => Prop::Eq {
            a: subst_name(a, from, to),
            b: subst_name(b, from, to),
        },
        Prop::ForAllAxis { var, body } => {
            if var == from {
                prop.clone() // shadowed
            } else {
                Prop::ForAllAxis {
                    var: var.clone(),
                    body: Box::new(subst_prop(body, from, to)),
                }
            }
        }
        Prop::And(ps) => Prop::And(ps.iter().map(|p| subst_prop(p, from, to)).collect()),
        Prop::Contradiction => Prop::Contradiction,
    }
}

fn subst_name(name: &str, from: &str, to: &str) -> String {
    if name == from { to.to_string() } else { name.to_string() }
}

/// `apply rule :on args` — apply a known axiom.
fn apply_rule(state: &mut ProofState, rule: &str, args: &[String]) -> Result<()> {
    match rule {
        "lattice-reflexivity" => apply_lattice_reflexivity(state, args),
        "lattice-transitivity" => apply_lattice_transitivity(state, args),
        "pigeonhole-principle" => apply_pigeonhole(state),
        "dominates-antisymmetry" => apply_dominates_antisymmetry(state),
        "preservation-intersection" => apply_preservation_intersection(state, args),
        _ => Err(MetacosmError::ProofError {
            theorem: String::new(),
            detail: format!("unknown rule: '{}'", rule),
        }),
    }
}

/// lattice-reflexivity: closes goal AxisGeq(w, A, w) where w1 == w2.
fn apply_lattice_reflexivity(state: &mut ProofState, _args: &[String]) -> Result<()> {
    let goal = state.goals.first().ok_or_else(|| MetacosmError::ProofError {
        theorem: String::new(),
        detail: "lattice-reflexivity: no goal".into(),
    })?;

    match goal {
        Prop::AxisGeq { w1, w2, .. } if w1 == w2 => {
            state.goals.remove(0);
            Ok(())
        }
        _ => Err(MetacosmError::ProofError {
            theorem: String::new(),
            detail: format!("lattice-reflexivity: goal is not AxisGeq(w, A, w), got: {}", goal),
        }),
    }
}

/// lattice-transitivity: closes goal AxisGeq(w1, A, w3) when context has
/// AxisGeq(w1, A, w2) and AxisGeq(w2, A, w3) for some w2.
fn apply_lattice_transitivity(state: &mut ProofState, args: &[String]) -> Result<()> {
    let goal = state.goals.first().ok_or_else(|| MetacosmError::ProofError {
        theorem: String::new(),
        detail: "lattice-transitivity: no goal".into(),
    })?;

    match goal.clone() {
        Prop::AxisGeq { w1, axis, w2: w3 } => {
            // Find a middle world w2 such that context has w1.A >= w2 and w2.A >= w3
            // If args are provided, use the first as the axis hint
            let target_axis = if !args.is_empty() { &args[0] } else { &axis };

            // Search context for matching chain
            for (_, p1) in &state.context {
                if let Prop::AxisGeq { w1: h_w1, axis: h_axis1, w2: h_w2 } = p1 {
                    if h_w1 == &w1 && (h_axis1 == target_axis || h_axis1 == &axis) {
                        // Found w1.A >= h_w2, now find h_w2.A >= w3
                        let middle = h_w2;
                        for (_, p2) in &state.context {
                            if let Prop::AxisGeq { w1: h2_w1, axis: h_axis2, w2: h2_w2 } = p2 {
                                if h2_w1 == middle && h2_w2 == &w3
                                    && (h_axis2 == target_axis || h_axis2 == &axis)
                                {
                                    state.goals.remove(0);
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
                // Also search for Dominates (unfolded into ForAllAxis)
                if let Prop::ForAllAxis { body, .. } = p1 {
                    if let Prop::AxisGeq { w1: h_w1, w2: h_w2, .. } = body.as_ref() {
                        if h_w1 == &w1 {
                            let middle = h_w2;
                            for (_, p2) in &state.context {
                                if let Prop::ForAllAxis { body: body2, .. } = p2 {
                                    if let Prop::AxisGeq { w1: h2_w1, w2: h2_w2, .. } = body2.as_ref() {
                                        if h2_w1 == middle && h2_w2 == &w3 {
                                            state.goals.remove(0);
                                            return Ok(());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Err(MetacosmError::ProofError {
                theorem: String::new(),
                detail: format!(
                    "lattice-transitivity: cannot find chain {}.{} ≥ ?W.{} and ?W.{} ≥ {}.{} in context",
                    w1, axis, axis, axis, w3, axis
                ),
            })
        }
        _ => Err(MetacosmError::ProofError {
            theorem: String::new(),
            detail: format!("lattice-transitivity: goal is not AxisGeq, got: {}", goal),
        }),
    }
}

/// pigeonhole-principle: derives Contradiction from Injective(f) and NotInjective(f).
fn apply_pigeonhole(state: &mut ProofState) -> Result<()> {
    for (_, p1) in &state.context {
        if let Prop::Injective { func: f1 } = p1 {
            for (_, p2) in &state.context {
                if let Prop::NotInjective { func: f2 } = p2 {
                    if f1 == f2 {
                        // Found contradiction: injective(f) ∧ ¬injective(f)
                        state.context.push(("__pigeonhole".into(), Prop::Contradiction));
                        // If goal is Contradiction, close it
                        if state.goals.first() == Some(&Prop::Contradiction) {
                            state.goals.remove(0);
                        }
                        return Ok(());
                    }
                }
            }
        }
    }

    Err(MetacosmError::ProofError {
        theorem: String::new(),
        detail: "pigeonhole-principle: no Injective(f) ∧ ¬Injective(f) pair found in context".into(),
    })
}

/// dominates-antisymmetry: from Dominates(A,B) ∧ Dominates(B,A) → Eq(A,B).
fn apply_dominates_antisymmetry(state: &mut ProofState) -> Result<()> {
    for (_, p1) in &state.context {
        if let Prop::Dominates { w1: a, w2: b } = p1 {
            for (_, p2) in &state.context {
                if let Prop::Dominates { w1: c, w2: d } = p2 {
                    if a == d && b == c {
                        let eq = Prop::Eq { a: a.clone(), b: b.clone() };
                        if state.goals.first() == Some(&eq) {
                            state.goals.remove(0);
                            return Ok(());
                        }
                        state.context.push(("__antisym".into(), eq));
                        return Ok(());
                    }
                }
            }
        }
    }

    Err(MetacosmError::ProofError {
        theorem: String::new(),
        detail: "dominates-antisymmetry: no Dominates(A,B) ∧ Dominates(B,A) pair in context".into(),
    })
}

/// preservation-intersection: Preserves(compose(t1,t2), inv) ↔ Preserves(t1,inv) ∧ Preserves(t2,inv).
fn apply_preservation_intersection(state: &mut ProofState, _args: &[String]) -> Result<()> {
    let goal = state.goals.first().ok_or_else(|| MetacosmError::ProofError {
        theorem: String::new(),
        detail: "preservation-intersection: no goal".into(),
    })?;

    // Forward direction: from Preserves(t1,inv) ∧ Preserves(t2,inv) in context,
    // close goal Preserves(compose(t1,t2), inv)
    if let Prop::Preserves { transition, invariant } = goal {
        if transition.contains(";") {
            // Composed transition — check both components
            let parts: Vec<&str> = transition.split(';').map(|s| s.trim()).collect();
            let all_preserved = parts.iter().all(|part| {
                state.context.iter().any(|(_, p)| {
                    matches!(p, Prop::Preserves { transition: t, invariant: inv }
                        if t == part && inv == invariant)
                })
            });
            if all_preserved {
                state.goals.remove(0);
                return Ok(());
            }
        }
    }

    Err(MetacosmError::ProofError {
        theorem: String::new(),
        detail: "preservation-intersection: cannot close goal".into(),
    })
}

/// `contradiction` — close goal from contradictory context.
fn apply_contradiction(state: &mut ProofState) -> Result<()> {
    // Check for explicit Contradiction in context
    if state.context.iter().any(|(_, p)| p == &Prop::Contradiction) {
        if !state.goals.is_empty() {
            state.goals.remove(0);
        }
        return Ok(());
    }

    // Check for Injective/NotInjective pairs
    for (_, p1) in &state.context {
        if let Prop::Injective { func: f1 } = p1 {
            for (_, p2) in &state.context {
                if let Prop::NotInjective { func: f2 } = p2 {
                    if f1 == f2 {
                        if !state.goals.is_empty() {
                            state.goals.remove(0);
                        }
                        return Ok(());
                    }
                }
            }
        }
    }

    Err(MetacosmError::ProofError {
        theorem: String::new(),
        detail: "contradiction: no contradictory pair found in context".into(),
    })
}

/// `split` — break conjunction goal into subgoals.
fn apply_split(state: &mut ProofState) -> Result<()> {
    let goal = state.goals.first().ok_or_else(|| MetacosmError::ProofError {
        theorem: String::new(),
        detail: "split: no goal".into(),
    })?;

    match goal.clone() {
        Prop::And(ps) => {
            state.goals.remove(0);
            // Push subgoals in reverse order so first is on top
            for p in ps.into_iter().rev() {
                state.goals.insert(0, p);
            }
            Ok(())
        }
        _ => Err(MetacosmError::ProofError {
            theorem: String::new(),
            detail: format!("split: goal is not a conjunction, got: {}", goal),
        }),
    }
}

/// `assumption` — close goal from identical hypothesis.
fn apply_assumption(state: &mut ProofState) -> Result<()> {
    let goal = state.goals.first().ok_or_else(|| MetacosmError::ProofError {
        theorem: String::new(),
        detail: "assumption: no goal".into(),
    })?;

    if state.find_in_context(goal).is_some() {
        state.goals.remove(0);
        Ok(())
    } else {
        Err(MetacosmError::ProofError {
            theorem: String::new(),
            detail: format!("assumption: goal {} not found in context", goal),
        })
    }
}

/// `have name : desc by justification` — introduce a derived fact.
fn apply_have(state: &mut ProofState, name: &str, _prop_desc: &str, justification: &str) -> Result<()> {
    // For now, accept "by axiom" or "by context" as valid justifications
    match justification {
        "context" | "assumption" => {
            // The prop_desc is informational; fact is already justified
            // We trust the user's claim here (checked by the tactic trace)
            Ok(())
        }
        _ => {
            // Record in trace but don't validate further
            state.context.push((name.to_string(), Prop::And(vec![]))); // placeholder
            Ok(())
        }
    }
}

// ============================================================================
// Assertion → Prop conversion
// ============================================================================

/// Convert an Assertion to a Prop for the proof engine.
pub fn assertion_to_prop(assertion: &crate::assertion::Assertion) -> Prop {
    use crate::assertion::Assertion;
    match assertion {
        Assertion::Dominates { stronger, weaker, .. } => Prop::Dominates {
            w1: stronger.clone(),
            w2: weaker.clone(),
        },
        Assertion::Faithful { morphism } => Prop::Faithful {
            transition: morphism.clone(),
        },
        Assertion::Full { morphism } => Prop::Injective {
            func: format!("{}.object-map", morphism),
        },
        Assertion::PreservesTransition { transition, invariant } => Prop::Preserves {
            transition: transition.clone(),
            invariant: invariant.clone(),
        },
        Assertion::TerminationDecidable { world } => Prop::AxisGeq {
            w1: world.clone(),
            axis: "termination".into(),
            w2: "__decidable_threshold".into(),
        },
        _ => Prop::And(vec![]), // fallback
    }
}

// ============================================================================
// Tactic parsing
// ============================================================================

/// Parse a list of tactic S-expressions into Tactic values.
pub fn parse_tactics(items: &[Sexp]) -> Result<Vec<Tactic>> {
    let mut tactics = Vec::new();
    for item in items {
        let tac_items = item.as_list().ok_or_else(|| MetacosmError::ParseError {
            block: "proof".into(),
            detail: format!("tactic must be a list, got: {:?}", item),
        })?;
        tactics.push(parse_single_tactic(tac_items)?);
    }
    Ok(tactics)
}

fn parse_single_tactic(items: &[Sexp]) -> Result<Tactic> {
    if items.is_empty() {
        return Err(MetacosmError::ParseError {
            block: "proof".into(),
            detail: "empty tactic".into(),
        });
    }

    let head = items[0].as_atom().unwrap_or("");
    match head {
        "unfold" => {
            let name = items.get(1)
                .and_then(|s| s.as_atom())
                .ok_or_else(|| MetacosmError::ParseError {
                    block: "proof".into(),
                    detail: "unfold requires a definition name".into(),
                })?;
            Ok(Tactic::Unfold { name: name.to_string() })
        }
        "intros-axis" => {
            let var = items.get(1)
                .and_then(|s| s.as_atom())
                .ok_or_else(|| MetacosmError::ParseError {
                    block: "proof".into(),
                    detail: "intros-axis requires a variable name".into(),
                })?;
            Ok(Tactic::IntrosAxis { var: var.to_string() })
        }
        "apply" => {
            let rule = items.get(1)
                .and_then(|s| s.as_atom())
                .ok_or_else(|| MetacosmError::ParseError {
                    block: "proof".into(),
                    detail: "apply requires a rule name".into(),
                })?;
            let mut args = Vec::new();
            let mut i = 2;
            while i < items.len() {
                if items[i].as_atom() == Some(":on") {
                    i += 1;
                    while i < items.len() {
                        if let Some(a) = items[i].as_atom() {
                            if a.starts_with(':') { break; }
                            args.push(a.to_string());
                        }
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
            Ok(Tactic::Apply { rule: rule.to_string(), args })
        }
        "contradiction" => Ok(Tactic::Contradiction),
        "qed" => Ok(Tactic::Qed),
        "split" => Ok(Tactic::Split),
        "assumption" => Ok(Tactic::Assumption),
        "have" => {
            let name = items.get(1).and_then(|s| s.as_atom()).unwrap_or("_").to_string();
            let prop_desc = items.get(3).and_then(|s| s.as_atom()).unwrap_or("_").to_string();
            let justification = items.get(5).and_then(|s| s.as_atom()).unwrap_or("_").to_string();
            Ok(Tactic::Have { name, prop_desc, justification })
        }
        _ => Err(MetacosmError::ParseError {
            block: "proof".into(),
            detail: format!("unknown tactic: '{}'", head),
        }),
    }
}

/// Parse typed forall quantifiers: `:forall [?W1 ?W2 :type World]`
pub fn parse_typed_forall(items: &[Sexp]) -> Result<Vec<(String, VarType)>> {
    let mut vars = Vec::new();
    let mut names = Vec::new();
    let mut var_type = VarType::World; // default
    let mut from_world: Option<String> = None;
    let mut to_world: Option<String> = None;

    let mut i = 0;
    while i < items.len() {
        let atom = items[i].as_atom().unwrap_or("");
        match atom {
            ":type" => {
                i += 1;
                let type_name = items.get(i).and_then(|s| s.as_atom()).unwrap_or("World");
                var_type = match type_name {
                    "World" => VarType::World,
                    "Transition" => VarType::Transition { from: None, to: None },
                    "Axis" => VarType::Axis,
                    _ => VarType::World,
                };
            }
            ":from" => {
                i += 1;
                from_world = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
            }
            ":to" => {
                i += 1;
                to_world = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
            }
            name if !name.is_empty() => {
                names.push(name.to_string());
            }
            _ => {}
        }
        i += 1;
    }

    // Apply the final type to transition with from/to if applicable
    let final_type = match var_type {
        VarType::Transition { .. } => VarType::Transition {
            from: from_world,
            to: to_world,
        },
        other => other,
    };

    for name in names {
        vars.push((name, final_type.clone()));
    }
    Ok(vars)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dominance_reflexivity_proof() {
        let state = ProofState::new(
            vec![("?W".into(), VarType::World)],
            vec![],
            Prop::Dominates { w1: "?W".into(), w2: "?W".into() },
        );

        let tactics = vec![
            Tactic::Unfold { name: "dominates".into() },
            Tactic::IntrosAxis { var: "?A".into() },
            Tactic::Apply { rule: "lattice-reflexivity".into(), args: vec![] },
            Tactic::Qed,
        ];

        let result = evaluate_proof("DominanceReflexivity", state, &tactics);
        assert!(result.is_ok(), "Proof failed: {:?}", result.unwrap_err());
    }

    #[test]
    fn test_dominance_transitivity_proof() {
        let state = ProofState::new(
            vec![
                ("?W1".into(), VarType::World),
                ("?W2".into(), VarType::World),
                ("?W3".into(), VarType::World),
            ],
            vec![
                ("h1".into(), Prop::Dominates { w1: "?W1".into(), w2: "?W2".into() }),
                ("h2".into(), Prop::Dominates { w1: "?W2".into(), w2: "?W3".into() }),
            ],
            Prop::Dominates { w1: "?W1".into(), w2: "?W3".into() },
        );

        let tactics = vec![
            Tactic::Unfold { name: "dominates".into() },
            Tactic::IntrosAxis { var: "?A".into() },
            Tactic::Apply { rule: "lattice-transitivity".into(), args: vec!["?A".into()] },
            Tactic::Qed,
        ];

        let result = evaluate_proof("DominanceTransitivity", state, &tactics);
        assert!(result.is_ok(), "Proof failed: {:?}", result.unwrap_err());
    }

    #[test]
    fn test_lossy_faithfulness_impossibility() {
        let state = ProofState::new(
            vec![
                ("?W1".into(), VarType::World),
                ("?W2".into(), VarType::World),
                ("?T".into(), VarType::Transition { from: Some("?W1".into()), to: Some("?W2".into()) }),
            ],
            vec![
                ("h1".into(), Prop::TransportMode { transition: "?T".into(), mode: "lossy".into() }),
                ("h2".into(), Prop::Faithful { transition: "?T".into() }),
            ],
            Prop::Contradiction,
        );

        let tactics = vec![
            Tactic::Unfold { name: "faithful".into() },
            Tactic::Unfold { name: "lossy".into() },
            Tactic::Apply { rule: "pigeonhole-principle".into(), args: vec![] },
            Tactic::Qed,
        ];

        let result = evaluate_proof("LossyFaithfulness", state, &tactics);
        assert!(result.is_ok(), "Proof failed: {:?}", result.unwrap_err());
    }

    #[test]
    fn test_incomplete_proof_fails() {
        let state = ProofState::new(
            vec![("?W".into(), VarType::World)],
            vec![],
            Prop::Dominates { w1: "?W".into(), w2: "?W".into() },
        );

        let tactics = vec![
            Tactic::Unfold { name: "dominates".into() },
            // Missing intros-axis and apply
            Tactic::Qed,
        ];

        let result = evaluate_proof("Incomplete", state, &tactics);
        assert!(result.is_err());
    }
}
