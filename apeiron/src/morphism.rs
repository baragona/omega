use std::collections::{HashMap, HashSet};

use crate::builder::{self, BuildEnv};
use crate::error::{ApeironError, Result};
use crate::parser::Sexp;
use crate::physics::{self, PhysicsConfig};
use crate::readback::{self, Term};
use crate::rewrite;
use crate::system::{BindingMode, CheckMode, SystemConfig};

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// Configuration options parsed from the AutoMorphism declaration.
#[derive(Debug, Clone, Default)]
pub struct MorphismConfig {
    /// When true (default), unmapped source operators are an error.
    /// When false, unmapped operators pass through unchanged.
    pub strict: bool,
    /// Explicit normalization strategy override.
    /// None = auto-derive from checking modes.
    /// Some(true) = always normalize before send.
    /// Some(false) = never normalize (send as-is).
    pub normalize_before_send: Option<bool>,
}

/// A declared morphism between two systems.
#[derive(Debug, Clone)]
pub struct AutoMorphism {
    pub name: String,
    pub source_system: String,
    pub target_system: String,
    /// Operator name mappings: source_name -> target_name.
    pub op_map: HashMap<String, String>,
    /// Computed binding pass.
    pub binding_pass: BindingPass,
    /// Computed checking pass.
    pub checking_pass: CheckingPass,
    /// User configuration.
    pub config: MorphismConfig,
}

/// How to translate binding structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingPass {
    /// Same binding mode — no transformation.
    Identity,
    /// Implicit -> Exposed: assign de Bruijn indices.
    DeBruijnize,
    /// Exposed -> Implicit: generate fresh names from indices.
    Namify,
    /// * -> Contextual: wrap in barrier.
    InjectScope,
    /// Contextual -> non-Contextual: strip barrier nodes.
    EraseScopes,
    /// * -> LinearExplicit: validate linearity after transform.
    ValidateLinear,
}

/// How to translate checking semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckingPass {
    /// Same checking class — no transformation.
    Identity,
    /// Compute -> Oracle: normalize in source before transport.
    NormalizeFirst,
    /// Oracle -> Compute: inject as trusted definition.
    InjectAxiom,
    /// Pattern/Unification -> *: must be ground (no Futures).
    ExtractGround,
}

// ---------------------------------------------------------------------------
// Resolution: derive passes from system configs
// ---------------------------------------------------------------------------

/// Analyze two system configs and derive the binding/checking passes + op map.
pub fn resolve_morphism(
    name: &str,
    source: &SystemConfig,
    target: &SystemConfig,
    explicit_ops: HashMap<String, String>,
    config: MorphismConfig,
) -> Result<AutoMorphism> {
    let binding_pass = derive_binding_pass(name, &source.binding, &target.binding)?;

    // Checking pass: use explicit strategy override if provided, else auto-derive
    let checking_pass = match config.normalize_before_send {
        Some(true) => CheckingPass::NormalizeFirst,
        Some(false) => CheckingPass::Identity,
        None => derive_checking_pass(&source.check_modes, &target.check_modes),
    };

    // Build operator map: explicit overrides + auto-matched by name.
    // Unmapped operators pass through at declaration time.
    // If @strict is true, transport-time validation catches leaked ops.
    let mut op_map = explicit_ops;
    let target_op_names: HashSet<String> = target.operators.iter().map(|o| o.name.clone()).collect();

    for src_op in &source.operators {
        if op_map.contains_key(&src_op.name) {
            continue;
        }
        if target_op_names.contains(&src_op.name) {
            op_map.insert(src_op.name.clone(), src_op.name.clone());
        }
        // Unmapped operators are allowed at declaration time.
        // Strict validation happens at transport time.
    }

    Ok(AutoMorphism {
        name: name.to_string(),
        source_system: source.name.clone(),
        target_system: target.name.clone(),
        op_map,
        binding_pass,
        checking_pass,
        config,
    })
}

fn derive_binding_pass(
    morph_name: &str,
    source: &BindingMode,
    target: &BindingMode,
) -> Result<BindingPass> {
    if source == target {
        return Ok(BindingPass::Identity);
    }

    match (source, target) {
        // Implicit/Linear → Exposed
        (BindingMode::Implicit | BindingMode::LinearExplicit, BindingMode::Exposed) => {
            Ok(BindingPass::DeBruijnize)
        }
        // Exposed → Implicit/Nominal
        (BindingMode::Exposed, BindingMode::Implicit | BindingMode::Nominal) => {
            Ok(BindingPass::Namify)
        }
        // Implicit/Exposed/Linear → Contextual
        (
            BindingMode::Implicit | BindingMode::Exposed | BindingMode::LinearExplicit,
            BindingMode::Contextual,
        ) => Ok(BindingPass::InjectScope),
        // Contextual → Implicit/Nominal
        (BindingMode::Contextual, BindingMode::Implicit | BindingMode::Nominal) => {
            Ok(BindingPass::EraseScopes)
        }
        // Contextual → Exposed
        (BindingMode::Contextual, BindingMode::Exposed) => {
            // Erase scopes then debruijnize — we use EraseScopes as the primary pass
            // and the readback already produces named terms, so DeBruijnize follows naturally.
            // For simplicity, we chain: EraseScopes handles the barrier stripping,
            // then the builder in exposed mode handles the rest.
            Ok(BindingPass::EraseScopes)
        }
        // * → LinearExplicit
        (_, BindingMode::LinearExplicit) => Ok(BindingPass::ValidateLinear),
        // Linear → Implicit
        (BindingMode::LinearExplicit, BindingMode::Implicit) => Ok(BindingPass::Identity),
        // Implicit → Nominal (names are generated fresh — fine)
        (BindingMode::Implicit, BindingMode::Nominal) => Ok(BindingPass::Identity),
        // Nominal → anything else: lossy
        (BindingMode::Nominal, _) => Err(ApeironError::MorphismError {
            name: morph_name.to_string(),
            detail: format!(
                "cannot translate from Nominal binding to {:?}: scope names carry semantic identity that would be lost",
                target
            ),
        }),
        // Distributed/Entangled: not yet supported
        (BindingMode::Distributed, _) | (_, BindingMode::Distributed) => {
            Err(ApeironError::MorphismError {
                name: morph_name.to_string(),
                detail: "AutoMorphism does not yet support Distributed binding mode".into(),
            })
        }
        (BindingMode::Entangled, _) | (_, BindingMode::Entangled) => {
            Err(ApeironError::MorphismError {
                name: morph_name.to_string(),
                detail: "AutoMorphism does not yet support Entangled binding mode".into(),
            })
        }
        // Fallback
        _ => Ok(BindingPass::Identity),
    }
}

fn derive_checking_pass(
    source: &HashSet<CheckMode>,
    target: &HashSet<CheckMode>,
) -> CheckingPass {
    let source_computes = source.contains(&CheckMode::Rewriting)
        || source.contains(&CheckMode::BetaReduction);
    let target_computes = target.contains(&CheckMode::Rewriting)
        || target.contains(&CheckMode::BetaReduction);
    let source_oracle = source.contains(&CheckMode::Oracle);
    let target_oracle = target.contains(&CheckMode::Oracle);
    let source_pattern = source.contains(&CheckMode::PatternUnification)
        || source.contains(&CheckMode::Unification);

    if source_pattern {
        return CheckingPass::ExtractGround;
    }
    if source_computes && target_oracle && !target_computes {
        return CheckingPass::NormalizeFirst;
    }
    if source_oracle && !source_computes && target_computes {
        return CheckingPass::InjectAxiom;
    }
    CheckingPass::Identity
}

// ---------------------------------------------------------------------------
// Term transformations
// ---------------------------------------------------------------------------

/// Apply a binding pass to a readback Term tree.
pub fn apply_binding_pass(
    term: &Term,
    pass: &BindingPass,
    scope_name: Option<&str>,
) -> Result<Term> {
    match pass {
        BindingPass::Identity => Ok(term.clone()),
        BindingPass::DeBruijnize => {
            let mut env = Vec::new();
            Ok(debruijnize(term, &mut env))
        }
        BindingPass::Namify => {
            let mut env = Vec::new();
            let mut counter = 0;
            Ok(namify(term, &mut env, &mut counter))
        }
        BindingPass::EraseScopes => Ok(erase_scopes(term)),
        BindingPass::InjectScope => {
            let sn = scope_name.unwrap_or("_scope");
            Ok(Term::App(
                Box::new(Term::Const("box".to_string())),
                vec![Term::Const(sn.to_string()), term.clone()],
            ))
        }
        BindingPass::ValidateLinear => {
            let mut counts: HashMap<String, usize> = HashMap::new();
            count_var_uses(term, &mut counts);
            for (name, count) in &counts {
                if *count != 1 {
                    return Err(ApeironError::MorphismError {
                        name: "ValidateLinear".to_string(),
                        detail: format!(
                            "variable '{}' used {} times (must be exactly 1 for linear mode)",
                            name, count
                        ),
                    });
                }
            }
            Ok(term.clone())
        }
    }
}

/// Implicit → Exposed: replace named variables with de Bruijn indices.
fn debruijnize(term: &Term, env: &mut Vec<String>) -> Term {
    match term {
        Term::Var(name) => {
            if let Some(idx) = env.iter().rev().position(|n| n == name) {
                Term::Const(format!("${}", idx))
            } else {
                term.clone() // free variable, keep as-is
            }
        }
        Term::Binder { kind, var, body } => {
            env.push(var.clone());
            let new_body = debruijnize(body, env);
            env.pop();
            // In exposed mode, binders don't carry variable names
            Term::Binder {
                kind: kind.clone(),
                var: "_".to_string(),
                body: Box::new(new_body),
            }
        }
        Term::App(func, args) => {
            let new_func = debruijnize(func, env);
            let new_args = args.iter().map(|a| debruijnize(a, env)).collect();
            Term::App(Box::new(new_func), new_args)
        }
        Term::Const(_) | Term::Future(_) | Term::Wire(_) | Term::Erased => term.clone(),
    }
}

/// Exposed → Implicit: replace de Bruijn indices ($N) with fresh names.
fn namify(term: &Term, env: &mut Vec<String>, counter: &mut usize) -> Term {
    match term {
        Term::Const(name) if name.starts_with('$') => {
            if let Ok(idx) = name[1..].parse::<usize>() {
                if idx < env.len() {
                    let var_name = env[env.len() - 1 - idx].clone();
                    return Term::Var(var_name);
                }
            }
            term.clone()
        }
        Term::Binder { kind, body, .. } => {
            let fresh = fresh_var_name(*counter);
            *counter += 1;
            env.push(fresh.clone());
            let new_body = namify(body, env, counter);
            env.pop();
            Term::Binder {
                kind: kind.clone(),
                var: fresh,
                body: Box::new(new_body),
            }
        }
        Term::App(func, args) => {
            let new_func = namify(func, env, counter);
            let new_args = args.iter().map(|a| namify(a, env, counter)).collect();
            Term::App(Box::new(new_func), new_args)
        }
        Term::Var(_) | Term::Const(_) | Term::Future(_) | Term::Wire(_) | Term::Erased => {
            term.clone()
        }
    }
}

fn fresh_var_name(n: usize) -> String {
    match n {
        0 => "x".to_string(),
        1 => "y".to_string(),
        2 => "z".to_string(),
        3 => "w".to_string(),
        n => format!("x{}", n),
    }
}

/// Contextual → *: strip barrier/box wrappers.
fn erase_scopes(term: &Term) -> Term {
    match term {
        Term::App(func, args) => {
            if let Term::Const(name) = func.as_ref() {
                if name.starts_with("barrier#") && args.len() == 1 {
                    return erase_scopes(&args[0]);
                }
            }
            let new_func = erase_scopes(func);
            let new_args = args.iter().map(|a| erase_scopes(a)).collect();
            Term::App(Box::new(new_func), new_args)
        }
        Term::Binder { kind, var, body } => Term::Binder {
            kind: kind.clone(),
            var: var.clone(),
            body: Box::new(erase_scopes(body)),
        },
        _ => term.clone(),
    }
}

/// Count variable occurrences for linearity validation.
fn count_var_uses(term: &Term, counts: &mut HashMap<String, usize>) {
    match term {
        Term::Var(name) => {
            *counts.entry(name.clone()).or_default() += 1;
        }
        Term::App(func, args) => {
            count_var_uses(func, counts);
            for arg in args {
                count_var_uses(arg, counts);
            }
        }
        Term::Binder { body, .. } => {
            count_var_uses(body, counts);
        }
        _ => {}
    }
}

/// Apply operator renaming to a Term tree.
pub fn apply_op_rename(term: &Term, op_map: &HashMap<String, String>) -> Term {
    match term {
        Term::Const(name) => {
            if let Some(new_name) = op_map.get(name.as_str()) {
                Term::Const(new_name.clone())
            } else {
                term.clone()
            }
        }
        Term::App(func, args) => {
            let new_func = apply_op_rename(func, op_map);
            let new_args = args.iter().map(|a| apply_op_rename(a, op_map)).collect();
            Term::App(Box::new(new_func), new_args)
        }
        Term::Binder { kind, var, body } => Term::Binder {
            kind: kind.clone(),
            var: var.clone(),
            body: Box::new(apply_op_rename(body, op_map)),
        },
        _ => term.clone(),
    }
}

/// Apply checking pass validation to a Term.
pub fn apply_checking_pass(term: &Term, pass: &CheckingPass) -> Result<Term> {
    match pass {
        CheckingPass::ExtractGround => {
            check_no_futures(term)?;
            Ok(term.clone())
        }
        // NormalizeFirst acts at graph level (before readback), not here.
        // InjectAxiom and Identity are pass-through.
        _ => Ok(term.clone()),
    }
}

fn check_no_futures(term: &Term) -> Result<()> {
    match term {
        Term::Future(id) => Err(ApeironError::MorphismError {
            name: "ExtractGround".to_string(),
            detail: format!(
                "cannot transport: source term contains unresolved meta-variable ?{}",
                id
            ),
        }),
        Term::App(func, args) => {
            check_no_futures(func)?;
            for arg in args {
                check_no_futures(arg)?;
            }
            Ok(())
        }
        Term::Binder { body, .. } => check_no_futures(body),
        _ => Ok(()),
    }
}

/// Strict validation: check that no unmapped source operators appear in the term.
/// This catches cases where the source-side compiler failed to eliminate a high-level op.
fn validate_no_leaked_ops(term: &Term, unmapped_ops: &HashSet<String>) -> Result<()> {
    match term {
        Term::Const(name) => {
            if unmapped_ops.contains(name.as_str()) {
                return Err(ApeironError::MorphismError {
                    name: "strict".to_string(),
                    detail: format!(
                        "source operator '{}' leaked through compilation (not rewritten away). \
                         Add a [Map {} ...] or ensure compiler rules eliminate it.",
                        name, name
                    ),
                });
            }
            Ok(())
        }
        Term::App(func, args) => {
            validate_no_leaked_ops(func, unmapped_ops)?;
            for arg in args {
                validate_no_leaked_ops(arg, unmapped_ops)?;
            }
            Ok(())
        }
        Term::Binder { body, .. } => validate_no_leaked_ops(body, unmapped_ops),
        _ => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// Transport pipeline
// ---------------------------------------------------------------------------

/// Transport an expression through a morphism.
///
/// Pipeline:
/// 1. Look up source system config → get known_ops
/// 2. Expand defs recursively (so graph is self-contained)
/// 3. Build graph using source's known_ops
/// 4. If NormalizeFirst: run physics + rewrite loop
/// 5. Readback to Term
/// 6. Apply checking pass validation
/// 7. Apply binding pass transformation
/// 8. Apply operator renaming
/// 9. Convert Term → Sexp
pub fn transport(
    arena: &mut crate::arena::Arena,
    morphism: &AutoMorphism,
    source_expr: &Sexp,
    source_config: &SystemConfig,
    defs: &HashMap<String, Sexp>,
    scopes: &HashMap<String, u32>,
    compiled_rules: &HashMap<String, Vec<rewrite::GraphRule>>,
    extra_known_ops: &HashSet<String>,
    target_scope_name: Option<&str>,
) -> Result<Sexp> {
    // 1. Source known_ops (system + theory-level)
    let mut known_ops: HashSet<String> = source_config
        .operators
        .iter()
        .map(|op| op.name.clone())
        .collect();
    known_ops.extend(extra_known_ops.iter().cloned());

    // 2. Recursive def expansion
    let expanded = rewrite::expand_defs(source_expr, defs);

    // 3. Build graph
    let mut env = BuildEnv::new();
    env.known_ops = known_ops;
    env.scope_ids = scopes.clone();
    let root = builder::build_rooted(arena, &mut env, &expanded);

    // 4. Run physics + rewrite loop.
    // Always normalize fully: the source system's rewrite rules should fire
    // regardless of checking pass. NormalizeFirst is about the target *requiring*
    // it, but we normalize anyway for a clean transport.
    let all_rules: Vec<rewrite::GraphRule> = compiled_rules
        .values()
        .flat_map(|rules| rules.iter().cloned())
        .collect();

    loop {
        let result = physics::run(arena, &PhysicsConfig::default());
        if result.halted_reason == physics::HaltReason::FuelExhausted {
            break;
        }
        if all_rules.is_empty() || !rewrite::try_rewrite_scan(arena, &all_rules) {
            break;
        }
    }

    // 5. Readback to Term
    let result_port = arena.port(root, 1);
    let term = if result_port.is_connected() {
        readback::readback(arena, result_port.target)
    } else {
        Term::Erased
    };

    // 6. Checking pass validation
    let term = apply_checking_pass(&term, &morphism.checking_pass)?;

    // 7. Binding pass transformation
    let term = apply_binding_pass(&term, &morphism.binding_pass, target_scope_name)?;

    // 8. Operator renaming
    let term = apply_op_rename(&term, &morphism.op_map);

    // 8.5. Strict validation: check no unmapped source ops leaked through
    if morphism.config.strict {
        let source_op_names: HashSet<String> = source_config
            .operators
            .iter()
            .map(|o| o.name.clone())
            .collect();
        let unmapped: HashSet<String> = source_op_names
            .into_iter()
            .filter(|name| !morphism.op_map.contains_key(name))
            .collect();
        if !unmapped.is_empty() {
            validate_no_leaked_ops(&term, &unmapped)?;
        }
    }

    // 9. Convert to Sexp
    Ok(rewrite::term_to_sexp(&term))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Binding pass resolution --

    #[test]
    fn binding_same_mode_is_identity() {
        let pass = derive_binding_pass("test", &BindingMode::Implicit, &BindingMode::Implicit);
        assert_eq!(pass.unwrap(), BindingPass::Identity);
    }

    #[test]
    fn binding_implicit_to_exposed() {
        let pass = derive_binding_pass("test", &BindingMode::Implicit, &BindingMode::Exposed);
        assert_eq!(pass.unwrap(), BindingPass::DeBruijnize);
    }

    #[test]
    fn binding_exposed_to_implicit() {
        let pass = derive_binding_pass("test", &BindingMode::Exposed, &BindingMode::Implicit);
        assert_eq!(pass.unwrap(), BindingPass::Namify);
    }

    #[test]
    fn binding_to_contextual() {
        let pass = derive_binding_pass("test", &BindingMode::Implicit, &BindingMode::Contextual);
        assert_eq!(pass.unwrap(), BindingPass::InjectScope);
    }

    #[test]
    fn binding_from_contextual() {
        let pass = derive_binding_pass("test", &BindingMode::Contextual, &BindingMode::Implicit);
        assert_eq!(pass.unwrap(), BindingPass::EraseScopes);
    }

    #[test]
    fn binding_to_linear() {
        let pass =
            derive_binding_pass("test", &BindingMode::Implicit, &BindingMode::LinearExplicit);
        assert_eq!(pass.unwrap(), BindingPass::ValidateLinear);
    }

    #[test]
    fn binding_nominal_escape_error() {
        let pass = derive_binding_pass("test", &BindingMode::Nominal, &BindingMode::Implicit);
        assert!(pass.is_err());
    }

    // -- Checking pass resolution --

    #[test]
    fn checking_compute_to_oracle() {
        let mut source = HashSet::new();
        source.insert(CheckMode::Rewriting);
        source.insert(CheckMode::BetaReduction);
        let mut target = HashSet::new();
        target.insert(CheckMode::Oracle);
        assert_eq!(derive_checking_pass(&source, &target), CheckingPass::NormalizeFirst);
    }

    #[test]
    fn checking_oracle_to_compute() {
        let mut source = HashSet::new();
        source.insert(CheckMode::Oracle);
        let mut target = HashSet::new();
        target.insert(CheckMode::Rewriting);
        assert_eq!(derive_checking_pass(&source, &target), CheckingPass::InjectAxiom);
    }

    #[test]
    fn checking_same_class_identity() {
        let mut source = HashSet::new();
        source.insert(CheckMode::Rewriting);
        let mut target = HashSet::new();
        target.insert(CheckMode::Rewriting);
        assert_eq!(derive_checking_pass(&source, &target), CheckingPass::Identity);
    }

    // -- DeBruijnize --

    #[test]
    fn debruijnize_identity() {
        // [lam x x] → [lam _ $0]
        let term = Term::Binder {
            kind: "lam".into(),
            var: "x".into(),
            body: Box::new(Term::Var("x".into())),
        };
        let result = apply_binding_pass(&term, &BindingPass::DeBruijnize, None).unwrap();
        assert_eq!(format!("{}", result), "[lam _ $0]");
    }

    #[test]
    fn debruijnize_nested() {
        // [lam x [lam y [app x y]]] → [lam _ [lam _ [app $1 $0]]]
        let term = Term::Binder {
            kind: "lam".into(),
            var: "x".into(),
            body: Box::new(Term::Binder {
                kind: "lam".into(),
                var: "y".into(),
                body: Box::new(Term::App(
                    Box::new(Term::Var("x".into())),
                    vec![Term::Var("y".into())],
                )),
            }),
        };
        let result = apply_binding_pass(&term, &BindingPass::DeBruijnize, None).unwrap();
        assert_eq!(format!("{}", result), "[lam _ [lam _ [$1 $0]]]");
    }

    // -- Namify --

    #[test]
    fn namify_identity() {
        // [lam _ $0] → [lam x x]
        let term = Term::Binder {
            kind: "lam".into(),
            var: "_".into(),
            body: Box::new(Term::Const("$0".into())),
        };
        let result = apply_binding_pass(&term, &BindingPass::Namify, None).unwrap();
        assert_eq!(format!("{}", result), "[lam x x]");
    }

    // -- EraseScopes --

    #[test]
    fn erase_scopes_strips_barrier() {
        // barrier#0(x) → x
        let term = Term::App(
            Box::new(Term::Const("barrier#0".into())),
            vec![Term::Const("x".into())],
        );
        let result = apply_binding_pass(&term, &BindingPass::EraseScopes, None).unwrap();
        assert_eq!(format!("{}", result), "x");
    }

    // -- Op rename --

    #[test]
    fn rename_ops() {
        let mut op_map = HashMap::new();
        op_map.insert("z".to_string(), "zero".to_string());
        op_map.insert("s".to_string(), "succ".to_string());
        let term = Term::App(
            Box::new(Term::Const("s".into())),
            vec![Term::Const("z".into())],
        );
        let result = apply_op_rename(&term, &op_map);
        assert_eq!(format!("{}", result), "[succ zero]");
    }

    // -- ExtractGround --

    #[test]
    fn extract_ground_with_future_fails() {
        let term = Term::Future(42);
        let result = apply_checking_pass(&term, &CheckingPass::ExtractGround);
        assert!(result.is_err());
    }

    #[test]
    fn extract_ground_no_future_ok() {
        let term = Term::Const("x".into());
        let result = apply_checking_pass(&term, &CheckingPass::ExtractGround);
        assert!(result.is_ok());
    }

    // -- ValidateLinear --

    #[test]
    fn validate_linear_ok() {
        // [lam x x] — x used exactly once
        let term = Term::Binder {
            kind: "lam".into(),
            var: "x".into(),
            body: Box::new(Term::Var("x".into())),
        };
        let result = apply_binding_pass(&term, &BindingPass::ValidateLinear, None);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_linear_dup_fails() {
        // [lam x [app x x]] — x used twice
        let term = Term::Binder {
            kind: "lam".into(),
            var: "x".into(),
            body: Box::new(Term::App(
                Box::new(Term::Var("x".into())),
                vec![Term::Var("x".into())],
            )),
        };
        let result = apply_binding_pass(&term, &BindingPass::ValidateLinear, None);
        assert!(result.is_err());
    }
}
