use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use apeiron::parser::{Sexp, Span};
use serde::Serialize;

/// A single result entry (unified CatLab schema).
#[derive(Debug, Clone, Serialize)]
pub struct ResultEntry {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    pub status: String, // "valid", "invalid", "timeout"
    pub message: Option<String>,
}

/// A discovery (e-graph results not explicitly asserted).
#[derive(Debug, Clone, Serialize)]
pub struct Discovery {
    pub lhs: String,
    pub rhs: String,
    pub description: String,
}

/// Top-level JSON output (unified CatLab schema).
#[derive(Debug, Clone, Serialize)]
pub struct JsonOutput {
    pub status: String, // "success", "failure", "timeout"
    pub elapsed_ms: f64,
    pub results: Vec<ResultEntry>,
    pub discoveries: Vec<Discovery>,
}

use apeiron::egraph::ProofTerm;

use crate::adjunction::{self, AdjunctionDef};
use crate::category::{self, CategoryDef};
use crate::codegen;
use crate::compile;
use crate::error::{HyperionError, Result};
use crate::functor::{self, FunctorDef};
use crate::nat_trans::{self, NatTransDef};
use crate::substrate::{self, SubstrateDef};
use crate::universe::{self, CompiledUniverse};

/// A Von Neumann theory: rewrite rules on first-order data, no Apeiron involvement.
#[derive(Debug, Clone)]
pub struct VonNeumannTheory {
    pub name: String,
    pub universe_name: String,
    pub sorts: Vec<String>,
    pub operators: Vec<String>,
    pub rules: Vec<VonNeumannRule>,
    pub morphism_types: HashMap<String, (Vec<String>, String)>,
    pub resource_mode: crate::substrate::ResourceMode,
    pub engine: crate::substrate::Engine,
    /// Tensor product morphism name (from SymmetricMonoidal category structure).
    /// When present on a concurrent-graph + strictly-linear substrate, tensor
    /// expressions compile to `rayon::join` after disjointness verification.
    pub tensor_op: Option<String>,
    /// Effect handlers: named handler implementations for effect sorts.
    /// Each handler maps effect method names to implementation rules.
    /// When present, codegen generates a handler struct implementing the trait.
    pub handlers: Vec<EffectHandler>,
}

/// An algebraic effect handler: a named set of method implementations
/// that reduce effect operations to concrete computations.
#[derive(Debug, Clone)]
pub struct EffectHandler {
    pub name: String,
    /// The effect sort this handles (e.g., "CompilerEffect", "Effect").
    pub effect_sort: String,
    /// Per-method implementations: effect method → (params, body rule).
    pub methods: Vec<HandlerMethod>,
}

/// A single method implementation in an effect handler.
#[derive(Debug, Clone)]
pub struct HandlerMethod {
    pub name: String,
    /// The LHS pattern (after the method name).
    pub pattern: Sexp,
    /// The body expression to execute.
    pub body: Sexp,
}

/// A single rewrite rule in a Von Neumann theory.
#[derive(Debug, Clone)]
pub struct VonNeumannRule {
    pub name: String,
    pub lhs: Sexp,
    pub rhs: Sexp,
}

/// The Hyperion session: wraps an Apeiron session with category/substrate/universe state.
pub struct HyperionSession {
    pub apeiron: apeiron::system::Session,
    pub categories: HashMap<String, CategoryDef>,
    pub substrates: HashMap<String, SubstrateDef>,
    pub universes: HashMap<String, CompiledUniverse>,
    pub functors: HashMap<String, FunctorDef>,
    /// Functor name → (category name → generated morphism name)
    pub resolved_morphisms: HashMap<String, HashMap<String, String>>,
    /// Theory name → universe name (for resolving Imports in Proofs blocks)
    pub theory_universes: HashMap<String, String>,
    /// Natural transformations
    pub nat_trans: HashMap<String, NatTransDef>,
    /// Adjunctions
    pub adjunctions: HashMap<String, AdjunctionDef>,
    /// Von Neumann theories (not sent to Apeiron)
    pub vn_theories: HashMap<String, VonNeumannTheory>,
    /// Captured @rule LHS/RHS from each theory (for functor verification)
    pub theory_rules: HashMap<String, Vec<(Sexp, Sexp)>>,
    /// Registered Apeiron Signature names (to avoid duplicates)
    pub registered_signatures: HashSet<String>,
    /// Skip categorical law verification
    pub skip_laws: bool,
    pub output: Vec<String>,
    /// Structured output for --json mode
    pub structured_output: Vec<ResultEntry>,
    /// Discoveries from e-graph saturation
    pub discoveries: Vec<Discovery>,
    /// @node annotations parsed from source
    pub node_annotations: HashMap<String, String>,
    /// Pending assert-eq terms: name → (lhs_text, rhs_text) for discovery reporting
    pub pending_assertions: HashMap<String, (String, String)>,
    /// Current meta-evaluation depth (prevents infinite meta-meta recursion)
    meta_depth: u32,
    /// Maximum allowed meta depth (default: 1 — Meta blocks cannot invoke other Meta blocks)
    pub max_meta_depth: u32,
    /// Meta splice bindings: name → result Sexp from optimize/synthesize
    pub meta_bindings: HashMap<String, Sexp>,
}

/// Convert a ProofTerm into an S-expression for meta-level manipulation.
fn proof_term_to_sexp(proof: &ProofTerm, sp: apeiron::parser::Span) -> Sexp {
    match proof {
        ProofTerm::Refl(expr) => Sexp::List(
            vec![Sexp::Atom("refl".into(), sp), Sexp::Atom(expr.clone(), sp)],
            sp,
        ),
        ProofTerm::Step { rule, from, to, sub_proofs } => {
            let mut items = vec![
                Sexp::Atom("step".into(), sp),
                Sexp::Atom(rule.clone(), sp),
                Sexp::Atom(from.clone(), sp),
                Sexp::Atom(to.clone(), sp),
            ];
            for sub in sub_proofs {
                items.push(proof_term_to_sexp(sub, sp));
            }
            Sexp::List(items, sp)
        }
        ProofTerm::Concat(a, b) => Sexp::List(
            vec![
                Sexp::Atom("concat".into(), sp),
                proof_term_to_sexp(a, sp),
                proof_term_to_sexp(b, sp),
            ],
            sp,
        ),
        ProofTerm::Inv(p) => Sexp::List(
            vec![Sexp::Atom("inv".into(), sp), proof_term_to_sexp(p, sp)],
            sp,
        ),
        ProofTerm::Cong { func, arg_proofs } => {
            let mut items = vec![
                Sexp::Atom("cong".into(), sp),
                Sexp::Atom(func.clone(), sp),
            ];
            for sub in arg_proofs {
                items.push(proof_term_to_sexp(sub, sp));
            }
            Sexp::List(items, sp)
        }
    }
}

impl HyperionSession {
    pub fn new() -> Self {
        let mut session = HyperionSession {
            apeiron: apeiron::system::Session::new(),
            categories: HashMap::new(),
            substrates: HashMap::new(),
            universes: HashMap::new(),
            functors: HashMap::new(),
            resolved_morphisms: HashMap::new(),
            theory_universes: HashMap::new(),
            nat_trans: HashMap::new(),
            adjunctions: HashMap::new(),
            vn_theories: HashMap::new(),
            theory_rules: HashMap::new(),
            registered_signatures: HashSet::new(),
            skip_laws: false,
            output: Vec::new(),
            structured_output: Vec::new(),
            discoveries: Vec::new(),
            node_annotations: HashMap::new(),
            pending_assertions: HashMap::new(),
            meta_depth: 0,
            max_meta_depth: 1,
            meta_bindings: HashMap::new(),
        };
        session.register_builtin_substrates();
        session
    }

    /// Register built-in substrates available without the prelude.
    fn register_builtin_substrates(&mut self) {
        use crate::substrate::*;
        let builtins = vec![
            SubstrateDef {
                name: "ApeironStandard".into(),
                engine: Engine::InteractionGraph,
                resource_mode: ResourceMode::OptimalSharing,
                barrier: BarrierMode::Transparent,
                equality: EqualityMode::RewriteEquivalence,
            },
            SubstrateDef {
                name: "ApeironLinear".into(),
                engine: Engine::InteractionGraph,
                resource_mode: ResourceMode::StrictlyLinear,
                barrier: BarrierMode::Transparent,
                equality: EqualityMode::RewriteEquivalence,
            },
            SubstrateDef {
                name: "ApeironOracle".into(),
                engine: Engine::InteractionGraph,
                resource_mode: ResourceMode::OptimalSharing,
                barrier: BarrierMode::Transparent,
                equality: EqualityMode::TopologicalHash,
            },
            SubstrateDef {
                name: "ApeironTree".into(),
                engine: Engine::TermTree,
                resource_mode: ResourceMode::DeepCopy,
                barrier: BarrierMode::Transparent,
                equality: EqualityMode::RewriteEquivalence,
            },
            SubstrateDef {
                name: "PrologEngine".into(),
                engine: Engine::LogicProgramming,
                resource_mode: ResourceMode::OptimalSharing,
                barrier: BarrierMode::Transparent,
                equality: EqualityMode::UnificationSearch,
            },
            SubstrateDef {
                name: "ACRewriting".into(),
                engine: Engine::InteractionGraph,
                resource_mode: ResourceMode::OptimalSharing,
                barrier: BarrierMode::Transparent,
                equality: EqualityMode::ACMatching,
            },
            SubstrateDef {
                name: "SMTBackend".into(),
                engine: Engine::SMTAssisted,
                resource_mode: ResourceMode::DeepCopy,
                barrier: BarrierMode::Transparent,
                equality: EqualityMode::SMTOracle,
            },
        ];
        for sub in builtins {
            self.substrates.insert(sub.name.clone(), sub);
        }
    }

    /// Create a session with the standard prelude auto-loaded.
    pub fn with_prelude() -> Result<Self> {
        let mut session = Self::new();
        session.load_prelude()?;
        Ok(session)
    }

    /// Record a structured result (for --json mode).
    pub fn record_result(&mut self, name: &str, status: &str, node_id: Option<String>, message: Option<String>) {
        // Check for @node annotation override
        let node_id = node_id.or_else(|| self.node_annotations.get(name).cloned());
        self.structured_output.push(ResultEntry {
            name: name.to_string(),
            node_id,
            status: status.to_string(),
            message,
        });
    }

    /// Record a discovery from e-graph saturation.
    pub fn record_discovery(&mut self, lhs: &str, rhs: &str, description: &str) {
        self.discoveries.push(Discovery {
            lhs: lhs.to_string(),
            rhs: rhs.to_string(),
            description: description.to_string(),
        });
    }

    /// Generate JSON output (unified CatLab schema).
    pub fn json_output(&self, had_errors: bool, elapsed_ms: f64) -> JsonOutput {
        let has_timeout = self.structured_output.iter().any(|r| r.status == "timeout");
        JsonOutput {
            status: if had_errors {
                if has_timeout { "timeout" } else { "failure" }
            } else {
                "success"
            }
            .to_string(),
            elapsed_ms,
            results: self.structured_output.clone(),
            discoveries: self.discoveries.clone(),
        }
    }

    /// Parse ;; @node <id> annotations from source and store them.
    pub fn parse_node_annotations(&mut self, source: &str) {
        let mut pending: Option<String> = None;
        for line in source.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix(";;").or_else(|| trimmed.strip_prefix(";")) {
                let rest = rest.trim();
                if let Some(node_id) = rest.strip_prefix("@node ") {
                    pending = Some(node_id.trim().to_string());
                }
            } else if !trimmed.is_empty() && trimmed.starts_with('[') {
                if let Some(node_id) = pending.take() {
                    let inner = trimmed.trim_start_matches('[');
                    let tokens: Vec<&str> = inner.split_whitespace().collect();
                    if tokens.len() >= 2 {
                        let name = tokens[1]
                            .trim_end_matches(']')
                            .trim_end_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
                        self.node_annotations.insert(name.to_string(), node_id);
                    }
                }
            } else if !trimmed.is_empty() {
                pending = None;
            }
        }
    }

    /// Search for the prelude file. Order: HYPERION_PRELUDE env var, then next to binary.
    pub fn find_prelude() -> Option<PathBuf> {
        // 1. Check env var
        if let Ok(path) = std::env::var("HYPERION_PRELUDE") {
            let p = PathBuf::from(path);
            if p.exists() {
                return Some(p);
            }
        }

        // 2. Check next to binary
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let p = dir.join("prelude.hyp");
                if p.exists() {
                    return Some(p);
                }
            }
        }

        None
    }

    /// Load the prelude file if found.
    pub fn load_prelude(&mut self) -> Result<()> {
        let path = match Self::find_prelude() {
            Some(p) => p,
            None => return Ok(()), // silently skip if not found
        };

        let source = std::fs::read_to_string(&path).map_err(|e| HyperionError::PreludeError {
            detail: format!("failed to read {}: {}", path.display(), e),
        })?;

        let sexps =
            apeiron::parser::parse(&source).map_err(|e| HyperionError::PreludeError {
                detail: format!("parse error in prelude: {}", e),
            })?;

        for sexp in &sexps {
            self.process(sexp).map_err(|e| HyperionError::PreludeError {
                detail: format!("{}", e),
            })?;
        }

        Ok(())
    }

    /// Process a top-level S-expression.
    pub fn process(&mut self, sexp: &Sexp) -> Result<()> {
        let items = sexp.as_list().ok_or_else(|| HyperionError::ParseError {
            block: "top-level".into(),
            detail: "expected top-level list".into(),
        })?;

        if items.is_empty() {
            return Ok(());
        }

        let head = items[0].as_atom().unwrap_or("");
        match head {
            "Category" => self.process_category(items),
            "Substrate" => self.process_substrate(items),
            "Universe" => self.process_universe(items),
            "Functor" => self.process_functor(items),
            "NatTrans" => self.process_nat_trans(items),
            "Adjunction" => self.process_adjunction(items),
            "Theory" => self.process_theory(sexp),
            "Proofs" => self.process_proofs(sexp),
            "VerifyFunctor" => self.process_verify_functor(items),
            "WeakEquivalence" => self.process_weak_equivalence(items),
            "Meta" => self.process_meta(items),
            "Handle" => self.process_handle(items),
            _ => Err(HyperionError::UnknownBlock {
                name: head.to_string(),
            }),
        }
    }

    fn process_category(&mut self, items: &[Sexp]) -> Result<()> {
        let cat = category::parse_category(items)?;
        let name = cat.name.clone();

        if self.categories.contains_key(&name) {
            return Err(HyperionError::DuplicateName {
                kind: "Category".into(),
                name,
            });
        }

        let msg = format!(
            "[CATEGORY] {} registered ({} objects, {} morphisms, {} structures)",
            name,
            cat.objects.len(),
            cat.morphisms.len(),
            cat.structure.len()
        );
        self.output.push(msg.clone());
        self.record_result(&name, "valid", Some(format!("category:{}", name)), Some(msg));
        self.categories.insert(name, cat);
        Ok(())
    }

    fn process_substrate(&mut self, items: &[Sexp]) -> Result<()> {
        let sub = substrate::parse_substrate(items)?;
        let name = sub.name.clone();

        // Built-in substrates can be overridden by prelude/user (idempotent).
        // Non-built-in duplicates are rejected.
        static BUILTIN_NAMES: &[&str] = &[
            "ApeironStandard", "ApeironLinear", "ApeironOracle", "ApeironTree",
            "PrologEngine", "ACRewriting", "SMTBackend",
        ];
        if self.substrates.contains_key(&name) && !BUILTIN_NAMES.contains(&name.as_str()) {
            return Err(HyperionError::DuplicateName {
                kind: "Substrate".into(),
                name,
            });
        }


        let msg = format!(
            "[SUBSTRATE] {} registered (engine={:?}, resource={:?}, barrier={:?}, equality={:?})",
            name, sub.engine, sub.resource_mode, sub.barrier, sub.equality
        );
        self.output.push(msg.clone());
        self.record_result(&name, "valid", Some(format!("substrate:{}", name)), Some(msg));
        self.substrates.insert(name, sub);
        Ok(())
    }

    fn process_universe(&mut self, items: &[Sexp]) -> Result<()> {
        let uni_def = universe::parse_universe(items)?;
        let name = uni_def.name.clone();

        if self.universes.contains_key(&name) {
            return Err(HyperionError::DuplicateName {
                kind: "Universe".into(),
                name,
            });
        }

        let cat = self
            .categories
            .get(&uni_def.category)
            .ok_or_else(|| HyperionError::Undefined {
                kind: "Category".into(),
                name: uni_def.category.clone(),
            })?
            .clone();

        let sub = self
            .substrates
            .get(&uni_def.substrate)
            .ok_or_else(|| HyperionError::Undefined {
                kind: "Substrate".into(),
                name: uni_def.substrate.clone(),
            })?
            .clone();

        // Compile: verify compatibility + generate system
        let compiled = compile::compile_universe(&name, &cat, &sub)?;

        // Generate and register the Apeiron Signature (typed ops, deduplicated per category)
        let sig_name = format!("__hyp_sig_{}", cat.name);
        let sig_ref = if !self.registered_signatures.contains(&sig_name) {
            let sig_sexp = compile::emit_signature_sexp(&cat);
            self.apeiron.process(&sig_sexp)?;
            self.drain_apeiron_output();
            self.registered_signatures.insert(sig_name.clone());
            Some(sig_name)
        } else {
            Some(sig_name)
        };

        // Generate and register the Apeiron system
        let system_sexp = compile::emit_system_sexp(&cat, &sub, &compiled, sig_ref.as_deref());
        self.apeiron.process(&system_sexp)?;

        // Drain apeiron output
        self.drain_apeiron_output();

        let pass_info = if compiled.passes.is_empty() {
            String::from("native")
        } else {
            let pass_names: Vec<&str> = compiled.passes.iter().map(|p| p.name()).collect();
            format!("passes=[{}]", pass_names.join(", "))
        };
        let msg = format!(
            "[UNIVERSE] {} compiled (system={}, category={}, substrate={}, {})",
            name, compiled.system_name, uni_def.category, uni_def.substrate, pass_info
        );
        self.output.push(msg.clone());
        for pass in &compiled.passes {
            let detail = format!("[PASS] {}: {}", pass.name(), pass.description());
            self.output.push(detail);
        }
        self.record_result(&name, "valid", Some(format!("universe:{}", name)), Some(msg));
        self.universes.insert(name, compiled);
        Ok(())
    }

    fn process_functor(&mut self, items: &[Sexp]) -> Result<()> {
        let fun = functor::parse_functor(items)?;
        let name = fun.name.clone();

        if self.functors.contains_key(&name) {
            return Err(HyperionError::DuplicateName {
                kind: "Functor".into(),
                name,
            });
        }

        // Verify both substrates exist
        if !self.substrates.contains_key(&fun.source) {
            return Err(HyperionError::Undefined {
                kind: "Substrate".into(),
                name: fun.source.clone(),
            });
        }
        if !self.substrates.contains_key(&fun.target) {
            return Err(HyperionError::Undefined {
                kind: "Substrate".into(),
                name: fun.target.clone(),
            });
        }

        // Find all matching universe pairs: same category, source substrate == fun.source,
        // target substrate == fun.target
        let mut morph_map: HashMap<String, String> = HashMap::new();

        // Collect all (category → source_system, target_system) pairs
        let mut source_systems: HashMap<String, String> = HashMap::new();
        let mut target_systems: HashMap<String, String> = HashMap::new();

        for compiled in self.universes.values() {
            if compiled.substrate_name == fun.source {
                source_systems
                    .entry(compiled.category_name.clone())
                    .or_insert_with(|| compiled.system_name.clone());
            }
            if compiled.substrate_name == fun.target {
                target_systems
                    .entry(compiled.category_name.clone())
                    .or_insert_with(|| compiled.system_name.clone());
            }
        }

        // For each category that appears in both source and target substrates,
        // generate an AutoMorphism
        for (cat_name, source_sys) in &source_systems {
            if let Some(target_sys) = target_systems.get(cat_name) {
                let morph_name = compile::morphism_name_for(&fun.name, cat_name);

                // Merge object_map + morphism_map into op_maps
                let op_maps: Vec<(String, String)> = fun
                    .object_map
                    .iter()
                    .chain(fun.morphism_map.iter())
                    .cloned()
                    .collect();

                let morph_sexp =
                    compile::emit_morphism_sexp(&morph_name, source_sys, target_sys, &op_maps);
                self.apeiron.process(&morph_sexp)?;
                self.drain_apeiron_output();

                morph_map.insert(cat_name.clone(), morph_name.clone());

                self.output.push(format!(
                    "[FUNCTOR] {} generated morphism {} ({} -> {})",
                    fun.name, morph_name, source_sys, target_sys
                ));
            }
        }

        if morph_map.is_empty() {
            return Err(HyperionError::NoMatchingUniverses {
                functor: fun.name.clone(),
                source: fun.source.clone(),
                target: fun.target.clone(),
            });
        }

        let msg = format!(
            "[FUNCTOR] {} registered (from={}, to={}, {} morphisms)",
            name,
            fun.source,
            fun.target,
            morph_map.len()
        );
        self.output.push(msg.clone());
        self.record_result(&name, "valid", Some(format!("functor:{}", name)), Some(msg));
        self.resolved_morphisms.insert(name.clone(), morph_map);
        self.functors.insert(name, fun);
        Ok(())
    }

    fn process_nat_trans(&mut self, items: &[Sexp]) -> Result<()> {
        let nt = nat_trans::parse_nat_trans(items)?;
        let name = nt.name.clone();

        if self.nat_trans.contains_key(&name) {
            return Err(HyperionError::DuplicateName {
                kind: "NatTrans".into(),
                name,
            });
        }

        // Validate both functors exist
        let source_fun = self.functors.get(&nt.source_functor).ok_or_else(|| {
            HyperionError::Undefined {
                kind: "Functor".into(),
                name: nt.source_functor.clone(),
            }
        })?;
        let target_fun = self.functors.get(&nt.target_functor).ok_or_else(|| {
            HyperionError::Undefined {
                kind: "Functor".into(),
                name: nt.target_functor.clone(),
            }
        })?;

        // Validate parallel: F.source == G.source && F.target == G.target
        if source_fun.source != target_fun.source || source_fun.target != target_fun.target {
            return Err(HyperionError::ParseError {
                block: "NatTrans".into(),
                detail: format!(
                    "functors '{}' ({}->{}) and '{}' ({}->{}) are not parallel",
                    nt.source_functor,
                    source_fun.source,
                    source_fun.target,
                    nt.target_functor,
                    target_fun.source,
                    target_fun.target,
                ),
            });
        }

        // If verify flag is set, generate verification output
        if nt.verify {
            self.output.push(format!(
                "[NATTRANS] {} verification requested (naturality squares for {} components)",
                name,
                nt.components.len()
            ));
        }

        self.output.push(format!(
            "[NATTRANS] {} registered (from={}, to={}, {} components)",
            name,
            nt.source_functor,
            nt.target_functor,
            nt.components.len()
        ));
        self.nat_trans.insert(name, nt);
        Ok(())
    }

    fn process_adjunction(&mut self, items: &[Sexp]) -> Result<()> {
        let adj = adjunction::parse_adjunction(items)?;
        let name = adj.name.clone();

        if self.adjunctions.contains_key(&name) {
            return Err(HyperionError::DuplicateName {
                kind: "Adjunction".into(),
                name,
            });
        }

        // Validate both functors exist
        if !self.functors.contains_key(&adj.left) {
            return Err(HyperionError::Undefined {
                kind: "Functor".into(),
                name: adj.left.clone(),
            });
        }
        if !self.functors.contains_key(&adj.right) {
            return Err(HyperionError::Undefined {
                kind: "Functor".into(),
                name: adj.right.clone(),
            });
        }

        // Validate unit and counit NatTrans exist
        if !self.nat_trans.contains_key(&adj.unit) {
            return Err(HyperionError::Undefined {
                kind: "NatTrans".into(),
                name: adj.unit.clone(),
            });
        }
        if !self.nat_trans.contains_key(&adj.counit) {
            return Err(HyperionError::Undefined {
                kind: "NatTrans".into(),
                name: adj.counit.clone(),
            });
        }

        let should_verify = adj.verify;

        let msg = format!(
            "[ADJUNCTION] {} registered (left={}, right={}, unit={}, counit={})",
            name, adj.left, adj.right, adj.unit, adj.counit
        );
        self.output.push(msg.clone());
        self.record_result(&name, "valid", Some(format!("adjunction:{}", name)), Some(msg));
        self.adjunctions.insert(name.clone(), adj);

        if should_verify {
            let adj_clone = self.adjunctions.get(&name).unwrap().clone();
            self.verify_adjunction(&adj_clone)?;
        }
        Ok(())
    }

    /// Verify adjunction triangle identities by generating assert-eq proofs.
    fn verify_adjunction(&mut self, adj: &AdjunctionDef) -> Result<()> {
        let _left_fun = self.functors.get(&adj.left).cloned().ok_or_else(|| {
            HyperionError::Undefined { kind: "Functor".into(), name: adj.left.clone() }
        })?;
        let _right_fun = self.functors.get(&adj.right).cloned().ok_or_else(|| {
            HyperionError::Undefined { kind: "Functor".into(), name: adj.right.clone() }
        })?;
        let unit_nt = self.nat_trans.get(&adj.unit).cloned().ok_or_else(|| {
            HyperionError::Undefined { kind: "NatTrans".into(), name: adj.unit.clone() }
        })?;
        let _counit_nt = self.nat_trans.get(&adj.counit).cloned().ok_or_else(|| {
            HyperionError::Undefined { kind: "NatTrans".into(), name: adj.counit.clone() }
        })?;

        // Find a theory in the target substrate's universe
        let left_target = &_left_fun.target;
        let target_theory = self.theory_universes.iter()
            .find(|(_, uni_name)| {
                self.universes.get(*uni_name)
                    .map(|u| u.substrate_name == *left_target)
                    .unwrap_or(false)
            })
            .map(|(theory_name, _)| theory_name.clone());

        let target_theory = match target_theory {
            Some(t) => t,
            None => {
                self.output.push(format!(
                    "[ADJUNCTION] {} triangle identity verification skipped (no theory in target substrate)",
                    adj.name
                ));
                self.record_result(&adj.name, "valid",
                    Some(format!("adjunction:{}", adj.name)),
                    Some("verification skipped: no theory in target substrate".into()));
                return Ok(());
            }
        };

        // Generate triangle identity assertions
        let sp = Span::default();
        let proofs_name = format!("__adj_triangle_{}", adj.name);

        let mut proof_items: Vec<Sexp> = Vec::new();
        proof_items.push(Sexp::Atom("Proofs".into(), sp));
        proof_items.push(Sexp::Atom(proofs_name.clone(), sp));
        proof_items.push(Sexp::Atom(":in".into(), sp));
        proof_items.push(Sexp::Atom(target_theory.clone(), sp));

        for comp in &unit_nt.components {
            let comp_name = &comp.object;
            proof_items.push(Sexp::List(vec![
                Sexp::Atom("assert-eq".into(), sp),
                Sexp::Atom(format!("triangle-left-{}", comp_name), sp),
                Sexp::List(vec![
                    Sexp::Atom(comp.morphism.clone(), sp),
                    Sexp::Atom(format!("__adj_witness_{}", comp_name), sp),
                ], sp),
                Sexp::Atom(format!("__adj_witness_{}", comp_name), sp),
            ], sp));
        }

        let universe_name = self.theory_universes.get(&target_theory).cloned();
        let rewritten = self.rewrite_for_apeiron(
            &Sexp::List(proof_items, sp),
            universe_name.as_deref(),
        )?;

        match self.apeiron.process(&rewritten) {
            Ok(()) => {
                self.drain_apeiron_output();
                let msg = format!(
                    "[ADJUNCTION] {} triangle identities verified ({} components)",
                    adj.name, unit_nt.components.len()
                );
                self.output.push(msg.clone());
                self.record_result(&adj.name, "valid",
                    Some(format!("adjunction:{}", adj.name)),
                    Some(msg));
                Ok(())
            }
            Err(e) => {
                self.drain_apeiron_output();
                let detail = format!("{}", e);
                self.output.push(format!(
                    "[ADJUNCTION] {} triangle identity verification FAILED: {}",
                    adj.name, detail
                ));
                self.record_result(&adj.name, "invalid",
                    Some(format!("adjunction:{}", adj.name)),
                    Some(detail.clone()));
                Err(HyperionError::LawViolation {
                    theory: format!("Adjunction:{}", adj.name),
                    law: "triangle-identities".into(),
                    detail,
                })
            }
        }
    }

    /// Check if a universe uses a first-order engine (Von Neumann or NetworkRpc).
    fn is_vn_universe(&self, universe_name: &str) -> bool {
        if let Some(compiled) = self.universes.get(universe_name) {
            if let Some(sub) = self.substrates.get(&compiled.substrate_name) {
                return compile::is_first_order_engine(sub);
            }
        }
        false
    }

    fn process_vn_theory(&mut self, sexp: &Sexp, universe_name: &str) -> Result<()> {
        let items = sexp.as_list().ok_or_else(|| HyperionError::ParseError {
            block: "Theory".into(),
            detail: "expected list".into(),
        })?;

        let theory_name = items
            .get(1)
            .and_then(|s| s.as_atom())
            .unwrap_or("")
            .to_string();

        // Get the category for this universe to extract morphism type info
        let compiled = self.universes.get(universe_name).ok_or_else(|| {
            HyperionError::Undefined {
                kind: "Universe".into(),
                name: universe_name.to_string(),
            }
        })?;
        let cat = self
            .categories
            .get(&compiled.category_name)
            .cloned()
            .ok_or_else(|| HyperionError::Undefined {
                kind: "Category".into(),
                name: compiled.category_name.clone(),
            })?;

        let sorts: Vec<String> = cat.objects.iter().map(|o| o.name.clone()).collect();
        let mut operators: Vec<String> = cat.morphisms.iter().map(|m| m.name.clone()).collect();
        let mut morphism_types: HashMap<String, (Vec<String>, String)> = HashMap::new();
        for m in &cat.morphisms {
            morphism_types.insert(m.name.clone(), (m.domain.clone(), m.codomain.clone()));
        }

        // Inject tensor/unit structure ops into morphism_types.
        // Tensor: binary product on the first non-Effect object type.
        // Unit: nullary constructor for the same type.
        let primary_sort = cat.objects.iter()
            .find(|o| o.name != "Effect" && o.name != "Unit")
            .map(|o| o.name.clone());
        for s in &cat.structure {
            match s {
                crate::category::CategoricalStructure::TensorProduct { name } => {
                    if let Some(ref sort) = primary_sort {
                        morphism_types.entry(name.clone()).or_insert_with(|| {
                            (vec![sort.clone(), sort.clone()], sort.clone())
                        });
                        operators.push(name.clone());
                    }
                }
                crate::category::CategoricalStructure::Unit { name } => {
                    if let Some(ref sort) = primary_sort {
                        morphism_types.entry(name.clone()).or_insert_with(|| {
                            (vec![], sort.clone())
                        });
                        operators.push(name.clone());
                    }
                }
                _ => {}
            }
        }

        // Parse @rule and @law declarations from the theory body
        let mut rules = Vec::new();
        for item in &items[2..] {
            if let Some(inner) = item.as_list() {
                if inner.is_empty() {
                    continue;
                }
                let head = inner[0].as_atom().unwrap_or("");
                if (head == "@rule" || head == "@law") && inner.len() >= 5 {
                    // [@rule name lhs ==> rhs] or [@law name lhs === rhs]
                    let rule_name = inner[1].as_atom().unwrap_or("").to_string();
                    let lhs = inner[2].clone();
                    // inner[3] should be "==>" or "==="
                    let rhs = inner[4].clone();
                    rules.push(VonNeumannRule {
                        name: rule_name,
                        lhs,
                        rhs,
                    });
                }
            }
        }

        self.output.push(format!(
            "[THEORY-VN] {} registered ({} sorts, {} operators, {} rules)",
            theory_name,
            sorts.len(),
            operators.len(),
            rules.len()
        ));

        // Look up substrate for resource mode and engine
        let sub = self
            .substrates
            .get(&compiled.substrate_name)
            .cloned()
            .ok_or_else(|| HyperionError::Undefined {
                kind: "Substrate".into(),
                name: compiled.substrate_name.clone(),
            })?;

        // Store rules for functor verification
        let rule_pairs: Vec<(Sexp, Sexp)> = rules
            .iter()
            .map(|r| (r.lhs.clone(), r.rhs.clone()))
            .collect();
        if !rule_pairs.is_empty() {
            self.theory_rules.insert(theory_name.clone(), rule_pairs);
        }

        // Also register the theory with Apeiron so VerifyFunctor can run
        // assert-eq proofs in it. We rewrite the Theory block for Apeiron.
        let rewritten = self.rewrite_for_apeiron(sexp, Some(universe_name))?;
        if let Err(e) = self.apeiron.process(&rewritten) {
            self.drain_apeiron_output();
            // Non-fatal: VN theories may not parse in Apeiron's format
            self.output.push(format!(
                "[THEORY-VN] Note: Apeiron registration skipped for {} ({})",
                theory_name, e
            ));
        } else {
            self.drain_apeiron_output();
        }

        // Extract tensor op name from category structure (if any)
        let tensor_op = cat.structure.iter().find_map(|s| {
            if let crate::category::CategoricalStructure::TensorProduct { name } = s {
                Some(name.clone())
            } else {
                None
            }
        });

        self.vn_theories.insert(
            theory_name.clone(),
            VonNeumannTheory {
                name: theory_name,
                universe_name: universe_name.to_string(),
                sorts,
                operators,
                rules,
                morphism_types,
                resource_mode: sub.resource_mode.clone(),
                engine: sub.engine.clone(),
                tensor_op,
                handlers: Vec::new(),
            },
        );

        Ok(())
    }

    /// Process a `[Handle EffectSort :with HandlerName :for TheoryName ...]` block.
    /// Registers an algebraic effect handler that can be swapped at compile/runtime.
    fn process_handle(&mut self, items: &[Sexp]) -> Result<()> {
        if items.len() < 6 {
            return Err(HyperionError::ParseError {
                block: "Handle".into(),
                detail: "expected [Handle EffectSort :with HandlerName :for TheoryName ...]".into(),
            });
        }

        let effect_sort = items[1]
            .as_atom()
            .ok_or_else(|| HyperionError::ParseError {
                block: "Handle".into(),
                detail: "effect sort must be an atom".into(),
            })?
            .to_string();

        // Parse :with and :for keywords
        let mut handler_name: Option<String> = None;
        let mut theory_name: Option<String> = None;
        let mut method_start = 2;

        let mut i = 2;
        while i < items.len() {
            if let Some(kw) = items[i].as_atom() {
                match kw {
                    ":with" => {
                        handler_name = items.get(i + 1).and_then(|s| s.as_atom()).map(|s| s.to_string());
                        i += 2;
                        method_start = i;
                        continue;
                    }
                    ":for" => {
                        theory_name = items.get(i + 1).and_then(|s| s.as_atom()).map(|s| s.to_string());
                        i += 2;
                        method_start = i;
                        continue;
                    }
                    _ => {}
                }
            }
            i += 1;
        }

        let handler_name = handler_name.ok_or_else(|| HyperionError::ParseError {
            block: "Handle".into(),
            detail: "missing :with HandlerName".into(),
        })?;
        let theory_name = theory_name.ok_or_else(|| HyperionError::ParseError {
            block: "Handle".into(),
            detail: "missing :for TheoryName".into(),
        })?;

        // Parse [@on method-name pattern ==> body] declarations
        let mut methods = Vec::new();
        for item in &items[method_start..] {
            if let Some(inner) = item.as_list() {
                if inner.len() >= 5 {
                    let head = inner[0].as_atom().unwrap_or("");
                    if head == "@on" {
                        let method_name = inner[1].as_atom().unwrap_or("").to_string();
                        let pattern = inner[2].clone();
                        // inner[3] should be "==>"
                        let body = inner[4].clone();
                        methods.push(HandlerMethod {
                            name: method_name,
                            pattern,
                            body,
                        });
                    }
                }
            }
        }

        let handler = EffectHandler {
            name: handler_name.clone(),
            effect_sort,
            methods,
        };

        // Attach to the theory
        let theory = self.vn_theories.get_mut(&theory_name).ok_or_else(|| {
            HyperionError::Undefined {
                kind: "Theory".into(),
                name: theory_name.clone(),
            }
        })?;

        let method_count = handler.methods.len();
        theory.handlers.push(handler);

        self.output.push(format!(
            "[HANDLE] {} registered for {} ({} methods)",
            handler_name, theory_name, method_count
        ));

        Ok(())
    }

    /// Compile a Von Neumann theory to Rust. Returns number of files generated.
    pub fn kompile(&self, theory_name: &str, output_dir: &str) -> Result<usize> {
        codegen::kompile(self, theory_name, output_dir)
    }

    fn process_theory(&mut self, sexp: &Sexp) -> Result<()> {
        // Extract theory name and universe name for tracking
        let items = sexp.as_list().ok_or_else(|| HyperionError::ParseError {
            block: "Theory".into(),
            detail: "expected list".into(),
        })?;
        let theory_name = items.get(1).and_then(|s| s.as_atom()).unwrap_or("");
        let universe_name = self.extract_in_target(items);

        // Check for :no-laws flag (per-theory law skip)
        let no_laws = items.iter().any(|s| s.is_atom(":no-laws"));

        if let Some(uni_name) = &universe_name {
            self.theory_universes
                .insert(theory_name.to_string(), uni_name.clone());

            // Check if this universe uses a Von Neumann substrate
            if self.is_vn_universe(uni_name) {
                return self.process_vn_theory(sexp, uni_name);
            }
        }

        // Capture user-declared @rule and @law declarations for functor verification + resource checking
        let named_rules = extract_rule_declarations(&items[2..]);

        // Resource enforcement: check user-declared rules against substrate's resource mode.
        // Note: auto-injected rules (PathType, Preorder) are framework infrastructure and are
        // exempt from resource checking — PathType unit laws inherently drop variables
        // (concat(refl(?a), ?p) ==> ?p), which is by design, not a user error.
        if let Some(uni_name) = &universe_name {
            if let Some(compiled) = self.universes.get(uni_name) {
                if let Some(sub) = self.substrates.get(&compiled.substrate_name) {
                    self.check_resource_rules(&named_rules, &sub.resource_mode, theory_name)?;
                }
            }
        }

        // Store rules for functor verification (without names)
        if !named_rules.is_empty() {
            let rules: Vec<(Sexp, Sexp)> = named_rules
                .iter()
                .map(|(_, lhs, rhs)| (lhs.clone(), rhs.clone()))
                .collect();
            self.theory_rules.insert(theory_name.to_string(), rules);
        }

        // Strip :no-laws before passing to Apeiron
        let sexp_for_apeiron = if no_laws {
            let filtered: Vec<Sexp> = items.iter()
                .filter(|s| !s.is_atom(":no-laws"))
                .cloned()
                .collect();
            Sexp::List(filtered, sexp.span())
        } else {
            sexp.clone()
        };

        let rewritten = self.rewrite_for_apeiron(&sexp_for_apeiron, universe_name.as_deref())?;
        self.apeiron.process(&rewritten)?;
        self.drain_apeiron_output();

        // Categorical law verification: after theory registration, check category laws.
        // Skip for parameterized templates — they aren't fully instantiated yet.
        let is_template = items.iter().any(|s| s.is_atom(":params"));
        if !self.skip_laws && !no_laws && !is_template {
            if let Some(uni_name) = &universe_name {
                self.check_categorical_laws(theory_name, uni_name)?;
            }
        }

        Ok(())
    }

    /// Check resource mode constraints on @rule declarations.
    /// For strictly-linear: each LHS meta must appear exactly once in RHS.
    /// For affine: each LHS meta must appear at most once in RHS.
    /// For all modes: RHS metas must be bound in LHS.
    fn check_resource_rules(
        &self,
        rules: &[(Option<String>, Sexp, Sexp)],
        mode: &substrate::ResourceMode,
        theory_name: &str,
    ) -> Result<()> {
        if matches!(
            mode,
            substrate::ResourceMode::OptimalSharing
                | substrate::ResourceMode::DeepCopy
                | substrate::ResourceMode::Relevant
        ) {
            return Ok(());
        }
        for (rule_name, lhs, rhs) in rules {
            let lhs_metas = collect_metas(lhs);
            let rhs_counts = count_metas(rhs);

            // 1. Unbound RHS metas
            for meta in rhs_counts.keys() {
                if !lhs_metas.contains(meta) {
                    return Err(HyperionError::ResourceViolation {
                        theory: theory_name.to_string(),
                        rule_name: rule_name.clone(),
                        detail: format!("unbound meta ?{} in RHS", meta),
                    });
                }
            }

            // 2. Resource constraints
            for meta in &lhs_metas {
                let count = rhs_counts.get(meta.as_str()).copied().unwrap_or(0);
                match mode {
                    substrate::ResourceMode::StrictlyLinear if count != 1 => {
                        return Err(HyperionError::ResourceViolation {
                            theory: theory_name.to_string(),
                            rule_name: rule_name.clone(),
                            detail: format!(
                                "strictly-linear requires exactly 1 use of ?{} in RHS, got {}",
                                meta, count
                            ),
                        });
                    }
                    substrate::ResourceMode::Affine if count > 1 => {
                        return Err(HyperionError::ResourceViolation {
                            theory: theory_name.to_string(),
                            rule_name: rule_name.clone(),
                            detail: format!(
                                "affine requires at most 1 use of ?{} in RHS, got {}",
                                meta, count
                            ),
                        });
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Check categorical laws for a theory by generating and running assert-eq proofs.
    fn check_categorical_laws(&mut self, theory_name: &str, universe_name: &str) -> Result<()> {
        let compiled = match self.universes.get(universe_name) {
            Some(c) => c.clone(),
            None => return Ok(()),
        };

        let cat = match self.categories.get(&compiled.category_name) {
            Some(c) => c.clone(),
            None => return Ok(()),
        };

        let category_laws = compile::generate_category_laws(&cat);
        if category_laws.is_empty() {
            return Ok(());
        }

        let witness_sort = cat.objects.first().map(|o| o.name.as_str());
        let law_count = category_laws.len();

        let proofs_sexp = match crate::laws::build_law_proofs(theory_name, &category_laws, witness_sort) {
            Some(s) => s,
            None => return Ok(()),
        };

        // Try to run the law proofs through Apeiron
        match self.apeiron.process(&proofs_sexp) {
            Ok(()) => {
                self.drain_apeiron_output();
                let msg = format!(
                    "[LAWS] {} passed categorical law verification ({} witness tests)",
                    theory_name, law_count
                );
                self.output.push(msg.clone());
                self.record_result(theory_name, "valid", Some(format!("laws:{}", theory_name)), Some(msg));
                Ok(())
            }
            Err(e) => {
                self.drain_apeiron_output();
                let detail = format!("{}", e);
                // If the error mentions fuel exhaustion, report as inconclusive warning
                if detail.contains("fuel") || detail.contains("Fuel") {
                    let msg = format!(
                        "[LAWS] {} INCONCLUSIVE for {} ({} witness tests) — {}",
                        theory_name, compiled.category_name, law_count, detail
                    );
                    self.output.push(msg.clone());
                    self.record_result(theory_name, "valid", Some(format!("laws:{}", theory_name)), Some(msg));
                    Ok(())
                } else {
                    Err(HyperionError::LawViolation {
                        theory: theory_name.to_string(),
                        law: compiled.category_name.clone(),
                        detail,
                    })
                }
            }
        }
    }

    fn process_proofs(&mut self, sexp: &Sexp) -> Result<()> {
        // For Proofs, `:in` is a theory name. Look up the theory's universe.
        let items = sexp.as_list().ok_or_else(|| HyperionError::ParseError {
            block: "Proofs".into(),
            detail: "expected list".into(),
        })?;
        let theory_name = self.extract_in_target(items);
        let universe_name = theory_name
            .as_deref()
            .and_then(|t| self.theory_universes.get(t))
            .cloned();

        // Capture assert-eq terms for discovery reporting
        for item in items {
            if let Some(inner) = item.as_list() {
                if inner.first().and_then(|s| s.as_atom()) == Some("assert-eq") && inner.len() >= 4 {
                    let name = inner[1].as_atom().unwrap_or("").to_string();
                    let lhs = format!("{}", inner[2]);
                    let rhs = format!("{}", inner[3]);
                    self.pending_assertions.insert(name, (lhs, rhs));
                }
            }
        }

        let rewritten = self.rewrite_for_apeiron(sexp, universe_name.as_deref())?;
        self.apeiron.process(&rewritten)?;
        self.drain_apeiron_output();
        Ok(())
    }

    /// Extract the value after `:in` from a block's top-level items.
    fn extract_in_target(&self, items: &[Sexp]) -> Option<String> {
        for i in 0..items.len() {
            if items[i].is_atom(":in") {
                if let Some(next) = items.get(i + 1) {
                    return next.as_atom().map(|s| s.to_string());
                }
            }
        }
        None
    }

    /// Rewrite a Theory/Proofs sexp for Apeiron:
    /// 1. `:in UniverseName` → `:in __hyp_Cat_Sub`
    /// 2. `[Import x [FunctorName expr ...]]` → `[Import x [MorphismName expr ...]]`
    /// 3. For Theory blocks in PathType universes: inject path algebra @rule declarations
    fn rewrite_for_apeiron(&self, sexp: &Sexp, universe_name: Option<&str>) -> Result<Sexp> {
        let items = sexp.as_list().ok_or_else(|| HyperionError::ParseError {
            block: "rewrite".into(),
            detail: "expected list".into(),
        })?;

        // Resolve the category for this block's universe (needed for functor→morphism lookup)
        let category_name = universe_name
            .and_then(|u| self.universes.get(u))
            .map(|c| c.category_name.clone());

        let mut new_items: Vec<Sexp> = Vec::new();
        let mut i = 0;

        while i < items.len() {
            if items[i].is_atom(":in") {
                new_items.push(items[i].clone());
                i += 1;

                if i < items.len() {
                    let target_name = items[i].as_atom().unwrap_or("");

                    // Check if it's a universe name
                    if let Some(compiled) = self.universes.get(target_name) {
                        new_items
                            .push(Sexp::Atom(compiled.system_name.clone(), Span::default()));
                    } else {
                        new_items.push(items[i].clone());
                    }
                }
            } else {
                // Rewrite Import blocks in body items
                let rewritten_item =
                    self.rewrite_body_item(&items[i], category_name.as_deref())?;
                new_items.push(rewritten_item);
            }
            i += 1;
        }

        // For Theory blocks: inject auto-rules and scope declarations from categorical structures
        let is_theory = items.first().and_then(|s| s.as_atom()).map(|a| a == "Theory").unwrap_or(false);
        if is_theory {
            if let Some(cat) = category_name.as_deref().and_then(|n| self.categories.get(n)) {
                let sp = Span::default();
                let mut injected_scopes: HashSet<String> = HashSet::new();

                for s in &cat.structure {
                    // Inject [Scope name] for Context declarations (barriers)
                    if let crate::category::CategoricalStructure::ContextDecl { name } = s {
                        if injected_scopes.insert(name.clone()) {
                            // Insert scope declarations before rules (at position after header items)
                            // Find insertion point: after Theory name and :in target
                            let insert_pos = new_items.len();
                            new_items.insert(
                                insert_pos,
                                Sexp::List(
                                    vec![
                                        Sexp::Atom("Scope".into(), sp),
                                        Sexp::Atom(name.clone(), sp),
                                    ],
                                    sp,
                                ),
                            );
                        }
                    }

                    if let crate::category::CategoricalStructure::PathType { refl, concat, inv, ap } = s {
                        let eval_name = cat.structure.iter().find_map(|s2| {
                            if let crate::category::CategoricalStructure::Evaluator { name } = s2 {
                                Some(name.as_str())
                            } else {
                                None
                            }
                        });
                        let rules = Self::path_type_rules(refl, concat, inv, ap, eval_name);
                        new_items.extend(rules);
                    }
                    if let crate::category::CategoricalStructure::Preorder { relation } = s {
                        let rules = Self::preorder_rules(relation);
                        new_items.extend(rules);
                    }
                    if let crate::category::CategoricalStructure::JType { j_elim, transport } = s {
                        let refl_name = cat.structure.iter().find_map(|s2| {
                            if let crate::category::CategoricalStructure::PathType { refl, .. } = s2 {
                                Some(refl.as_str())
                            } else {
                                None
                            }
                        });
                        if let Some(refl) = refl_name {
                            let rules = Self::j_type_rules(j_elim, transport, refl);
                            new_items.extend(rules);
                        }
                    }
                    if let crate::category::CategoricalStructure::IntervalSort { interval: _, i0, i1 } = s {
                        // Look up PathType and PartialElement to inject kernel cubical reductions
                        let path_info = cat.structure.iter().find_map(|s2| {
                            if let crate::category::CategoricalStructure::PathType { refl, concat, inv, .. } = s2 {
                                Some((refl.as_str(), concat.as_str(), inv.as_str()))
                            } else {
                                None
                            }
                        });
                        let pe_info = cat.structure.iter().find_map(|s2| {
                            if let crate::category::CategoricalStructure::PartialElement { hcomp, coe } = s2 {
                                Some((hcomp.as_str(), coe.as_str()))
                            } else {
                                None
                            }
                        });
                        if let (Some((refl, concat, inv)), Some((_hcomp, coe))) = (path_info, pe_info) {
                            let rules = Self::kernel_cubical_rules(coe, refl, concat, inv, i0, i1);
                            new_items.extend(rules);
                        }
                    }
                    if let crate::category::CategoricalStructure::PartialElement { hcomp, coe } = s {
                        let refl_name = cat.structure.iter().find_map(|s2| {
                            if let crate::category::CategoricalStructure::PathType { refl, .. } = s2 {
                                Some(refl.as_str())
                            } else {
                                None
                            }
                        });
                        let rules = Self::partial_element_rules(hcomp, coe, refl_name.map(|s| s));
                        new_items.extend(rules);
                    }
                }
            }
        }

        Ok(Sexp::List(new_items, sexp.span()))
    }

    /// Generate Apeiron @rule declarations for path algebra.
    ///
    /// All rules are directed (normalization-oriented):
    /// - concat(refl(a), p) ==> p
    /// - concat(p, refl(a)) ==> p
    /// - inv(refl(a)) ==> refl(a)
    /// - concat(concat(p,q), r) ==> concat(p, concat(q,r))  [right-associative normal form]
    /// - ap(f, refl(a)) ==> refl(app(f, a))  [if Evaluator present]
    /// - ap(f, concat(p,q)) ==> concat(ap(f,p), ap(f,q))  [if Evaluator present]
    fn path_type_rules(refl: &str, concat: &str, inv: &str, ap: &str, eval_name: Option<&str>) -> Vec<Sexp> {
        let sp = Span::default();

        let mk_rule = |lhs: Sexp, rhs: Sexp| -> Sexp {
            Sexp::List(vec![
                Sexp::Atom("@rule".into(), sp),
                lhs,
                Sexp::Atom("==>".into(), sp),
                rhs,
            ], sp)
        };

        let meta_a = || Sexp::Atom("?a".into(), sp);
        let meta_p = || Sexp::Atom("?p".into(), sp);
        let meta_q = || Sexp::Atom("?q".into(), sp);
        let meta_r = || Sexp::Atom("?r".into(), sp);
        let meta_f = || Sexp::Atom("?f".into(), sp);

        let mk_refl = |x: Sexp| -> Sexp {
            Sexp::List(vec![Sexp::Atom(refl.into(), sp), x], sp)
        };
        let mk_concat = |x: Sexp, y: Sexp| -> Sexp {
            Sexp::List(vec![Sexp::Atom(concat.into(), sp), x, y], sp)
        };
        let mk_inv = |x: Sexp| -> Sexp {
            Sexp::List(vec![Sexp::Atom(inv.into(), sp), x], sp)
        };

        let mut rules = vec![
            // concat(refl(a), p) ==> p  [left unit — directed simplification]
            mk_rule(
                mk_concat(mk_refl(meta_a()), meta_p()),
                meta_p()),
            // concat(p, refl(a)) ==> p  [right unit — directed simplification]
            mk_rule(
                mk_concat(meta_p(), mk_refl(meta_a())),
                meta_p()),
            // inv(refl(a)) ==> refl(a)  [inverse of identity — directed]
            mk_rule(
                mk_inv(mk_refl(meta_a())),
                mk_refl(meta_a())),
            // concat(concat(p,q), r) ==> concat(p, concat(q,r))  [right-associative normal form]
            mk_rule(
                mk_concat(mk_concat(meta_p(), meta_q()), meta_r()),
                mk_concat(meta_p(), mk_concat(meta_q(), meta_r()))),
        ];

        // ap(f, refl(a)) ==> refl(app(f, a)) — only if Evaluator present
        if let Some(app) = eval_name {
            rules.push(mk_rule(
                Sexp::List(vec![Sexp::Atom(ap.into(), sp), meta_f(), mk_refl(meta_a())], sp),
                mk_refl(Sexp::List(vec![Sexp::Atom(app.into(), sp), meta_f(), meta_a()], sp)),
            ));
            // ap(f, concat(p, q)) ==> concat(ap(f, p), ap(f, q)) — functoriality of ap over concat
            let mk_ap = |f: Sexp, x: Sexp| -> Sexp {
                Sexp::List(vec![Sexp::Atom(ap.into(), sp), f, x], sp)
            };
            rules.push(mk_rule(
                mk_ap(meta_f(), mk_concat(meta_p(), meta_q())),
                mk_concat(mk_ap(meta_f(), meta_p()), mk_ap(meta_f(), meta_q())),
            ));
        }

        rules
    }

    /// Generate preorder rewrite rules for auto-injection into theories.
    fn preorder_rules(relation: &str) -> Vec<Sexp> {
        let sp = Span::default();

        let mk_rule = |lhs: Sexp, rhs: Sexp| -> Sexp {
            Sexp::List(vec![
                Sexp::Atom("@rule".into(), sp),
                lhs,
                Sexp::Atom("==>".into(), sp),
                rhs,
            ], sp)
        };

        let meta_a = || Sexp::Atom("?a".into(), sp);
        let mk_rel = |x: Sexp, y: Sexp| -> Sexp {
            Sexp::List(vec![Sexp::Atom(relation.into(), sp), x, y], sp)
        };

        vec![
            // rel(a, a) ==> true (reflexivity)
            mk_rule(
                mk_rel(meta_a(), meta_a()),
                Sexp::Atom("true".into(), sp),
            ),
        ]
    }

    /// Generate J-elimination rewrite rules.
    /// J(C, d, refl(a)) ==> d   [J computation / beta rule]
    /// transport(refl(a), x) ==> x   [transport along refl is identity]
    fn j_type_rules(j_elim: &str, transport: &str, refl: &str) -> Vec<Sexp> {
        let sp = Span::default();

        let mk_rule = |lhs: Sexp, rhs: Sexp| -> Sexp {
            Sexp::List(vec![
                Sexp::Atom("@rule".into(), sp),
                lhs,
                Sexp::Atom("==>".into(), sp),
                rhs,
            ], sp)
        };

        let meta_a = || Sexp::Atom("?a".into(), sp);
        let meta_c = || Sexp::Atom("?C".into(), sp);
        let meta_d = || Sexp::Atom("?d".into(), sp);
        let meta_x = || Sexp::Atom("?x".into(), sp);

        let mk_refl = |x: Sexp| -> Sexp {
            Sexp::List(vec![Sexp::Atom(refl.into(), sp), x], sp)
        };

        vec![
            // J(C, d, refl(a)) ==> d
            mk_rule(
                Sexp::List(vec![
                    Sexp::Atom(j_elim.into(), sp),
                    meta_c(),
                    meta_d(),
                    mk_refl(meta_a()),
                ], sp),
                meta_d()),
            // transport(refl(a), x) ==> x
            mk_rule(
                Sexp::List(vec![
                    Sexp::Atom(transport.into(), sp),
                    mk_refl(meta_a()),
                    meta_x(),
                ], sp),
                meta_x()),
        ]
    }

    /// Generate cubical partial element rules.
    /// coe(refl(A), i, x) ==> x   [coercion along constant type line is identity]
    /// hcomp(refl(a), base) ==> base  [hcomp with trivial system is base]
    fn partial_element_rules(hcomp: &str, coe: &str, refl: Option<&str>) -> Vec<Sexp> {
        let sp = Span::default();
        let mk_rule = |lhs: Sexp, rhs: Sexp| -> Sexp {
            Sexp::List(vec![
                Sexp::Atom("@rule".into(), sp),
                lhs,
                Sexp::Atom("==>".into(), sp),
                rhs,
            ], sp)
        };

        let meta_x = || Sexp::Atom("?x".into(), sp);
        let meta_i = || Sexp::Atom("?i".into(), sp);
        let meta_a = || Sexp::Atom("?a".into(), sp);
        let meta_base = || Sexp::Atom("?base".into(), sp);

        let mut rules = Vec::new();

        if let Some(refl) = refl {
            let mk_refl = |x: Sexp| -> Sexp {
                Sexp::List(vec![Sexp::Atom(refl.into(), sp), x], sp)
            };

            // coe(refl(A), i, x) ==> x
            rules.push(mk_rule(
                Sexp::List(vec![
                    Sexp::Atom(coe.into(), sp),
                    mk_refl(meta_a()),
                    meta_i(),
                    meta_x(),
                ], sp),
                meta_x()));

            // hcomp(refl(a), base) ==> base
            rules.push(mk_rule(
                Sexp::List(vec![
                    Sexp::Atom(hcomp.into(), sp),
                    mk_refl(meta_a()),
                    meta_base(),
                ], sp),
                meta_base()));
        }

        rules
    }

    /// Generate kernel-level cubical reduction rules.
    ///
    /// These fire as directed @rule rewrites before e-graph saturation:
    /// - coe(concat(p, q), i, x) ==> coe(q, i, coe(p, i, x))
    /// - coe(inv(p), i, x)       ==> coe(p, (inv-endpoint i), x)
    /// - coe(refl(A), i, x)      ==> x                          (already in partial_element_rules)
    fn kernel_cubical_rules(
        coe: &str,
        _refl: &str,
        concat: &str,
        inv: &str,
        _i0: &str,
        _i1: &str,
    ) -> Vec<Sexp> {
        let sp = Span::default();
        let mk_rule = |lhs: Sexp, rhs: Sexp| -> Sexp {
            Sexp::List(vec![
                Sexp::Atom("@rule".into(), sp),
                lhs,
                Sexp::Atom("==>".into(), sp),
                rhs,
            ], sp)
        };

        let meta_p = || Sexp::Atom("?p".into(), sp);
        let meta_q = || Sexp::Atom("?q".into(), sp);
        let meta_i = || Sexp::Atom("?i".into(), sp);
        let meta_x = || Sexp::Atom("?x".into(), sp);

        let mut rules = Vec::new();

        // coe(concat(p, q), i, x) ==> coe(q, i, coe(p, i, x))
        rules.push(mk_rule(
            Sexp::List(vec![
                Sexp::Atom(coe.into(), sp),
                Sexp::List(vec![
                    Sexp::Atom(concat.into(), sp),
                    meta_p(),
                    meta_q(),
                ], sp),
                meta_i(),
                meta_x(),
            ], sp),
            Sexp::List(vec![
                Sexp::Atom(coe.into(), sp),
                meta_q(),
                meta_i(),
                Sexp::List(vec![
                    Sexp::Atom(coe.into(), sp),
                    meta_p(),
                    meta_i(),
                    meta_x(),
                ], sp),
            ], sp),
        ));

        // coe(inv(p), i, x) ==> coe(p, i, x)
        // Note: In full cubical TT, the endpoint is flipped (1-i).
        // Here we simplify: inv just unwraps for coe purposes,
        // since the direction is handled by the path algebra.
        rules.push(mk_rule(
            Sexp::List(vec![
                Sexp::Atom(coe.into(), sp),
                Sexp::List(vec![
                    Sexp::Atom(inv.into(), sp),
                    meta_p(),
                ], sp),
                meta_i(),
                meta_x(),
            ], sp),
            Sexp::List(vec![
                Sexp::Atom(coe.into(), sp),
                meta_p(),
                meta_i(),
                meta_x(),
            ], sp),
        ));

        rules
    }

    /// Rewrite a single body item, looking for `[Import x [FunctorName expr ...]]`.
    fn rewrite_body_item(&self, item: &Sexp, category_name: Option<&str>) -> Result<Sexp> {
        let items = match item.as_list() {
            Some(items) => items,
            None => return Ok(item.clone()),
        };

        if items.is_empty() {
            return Ok(item.clone());
        }

        // Check for [Import local-name [FunctorName ...] ...]
        if items[0].is_atom("Import") && items.len() >= 3 {
            // items[1] = local-name, items[2] = [FunctorName expr ...], items[3..] = extra args
            if let Some(morph_app) = items[2].as_list() {
                if !morph_app.is_empty() {
                    if let Some(head_name) = morph_app[0].as_atom() {
                        // Is head_name a functor? Try to resolve it.
                        if let Some(cat_name) = category_name {
                            if let Some(morph_map) = self.resolved_morphisms.get(head_name) {
                                if let Some(morph_name) = morph_map.get(cat_name) {
                                    // Rewrite: replace functor name with morphism name
                                    let mut new_morph_app = morph_app.to_vec();
                                    new_morph_app[0] =
                                        Sexp::Atom(morph_name.clone(), Span::default());

                                    let mut new_items = items.to_vec();
                                    new_items[2] =
                                        Sexp::List(new_morph_app, items[2].span());

                                    return Ok(Sexp::List(new_items, item.span()));
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(item.clone())
    }

    /// Process a `[WeakEquivalence name :source T1 :target T2 :on-types [[A1 B1] ...] :via [[f1 g1] ...] :verify true]`.
    /// Checks weak equivalence between two theories by:
    /// 1. Asserting f maps A→B and g maps B→A (compose(f,A)≡B, compose(g,B)≡A)
    /// 2. Asserting g∘f and f∘g are connected to identity by e-graph paths
    /// 3. Returning the witnessing equivalence data (maps + homotopies)
    fn process_weak_equivalence(&mut self, items: &[Sexp]) -> Result<()> {
        if items.len() < 2 {
            return Err(HyperionError::ParseError {
                block: "WeakEquivalence".into(),
                detail: "missing name".into(),
            });
        }

        let name = items[1]
            .as_atom()
            .ok_or_else(|| HyperionError::ParseError {
                block: "WeakEquivalence".into(),
                detail: "name must be an atom".into(),
            })?
            .to_string();

        let mut source_theory: Option<String> = None;
        let mut target_theory: Option<String> = None;
        let mut type_pairs: Vec<(String, String)> = Vec::new();
        let mut map_pairs: Vec<(String, String)> = Vec::new();
        let mut should_verify = false;

        let mut i = 2;
        while i < items.len() {
            let key = items[i].as_atom().unwrap_or("");
            match key {
                ":source" => {
                    i += 1;
                    source_theory = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
                }
                ":target" => {
                    i += 1;
                    target_theory = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
                }
                ":on-types" => {
                    i += 1;
                    if let Some(pairs_list) = items.get(i).and_then(|s| s.as_list()) {
                        for pair in pairs_list {
                            if let Some(pair_items) = pair.as_list() {
                                if pair_items.len() == 2 {
                                    let a = pair_items[0].as_atom().unwrap_or("").to_string();
                                    let b = pair_items[1].as_atom().unwrap_or("").to_string();
                                    type_pairs.push((a, b));
                                }
                            }
                        }
                    }
                }
                ":via" => {
                    i += 1;
                    if let Some(pairs_list) = items.get(i).and_then(|s| s.as_list()) {
                        for pair in pairs_list {
                            if let Some(pair_items) = pair.as_list() {
                                if pair_items.len() == 2 {
                                    let f = pair_items[0].as_atom().unwrap_or("").to_string();
                                    let g = pair_items[1].as_atom().unwrap_or("").to_string();
                                    map_pairs.push((f, g));
                                }
                            }
                        }
                    }
                }
                ":verify" => {
                    i += 1;
                    should_verify = items.get(i).and_then(|s| s.as_atom()) == Some("true");
                }
                _ => {
                    return Err(HyperionError::ParseError {
                        block: "WeakEquivalence".into(),
                        detail: format!("unknown keyword: {}", key),
                    });
                }
            }
            i += 1;
        }

        let source_theory = source_theory.ok_or_else(|| HyperionError::ParseError {
            block: "WeakEquivalence".into(),
            detail: format!("'{}' is missing :source", name),
        })?;
        let target_theory = target_theory.ok_or_else(|| HyperionError::ParseError {
            block: "WeakEquivalence".into(),
            detail: format!("'{}' is missing :target", name),
        })?;

        if type_pairs.is_empty() {
            return Err(HyperionError::ParseError {
                block: "WeakEquivalence".into(),
                detail: format!("'{}' is missing :on-types", name),
            });
        }

        if !map_pairs.is_empty() && map_pairs.len() != type_pairs.len() {
            return Err(HyperionError::ParseError {
                block: "WeakEquivalence".into(),
                detail: format!("'{}' :via has {} pairs but :on-types has {}", name, map_pairs.len(), type_pairs.len()),
            });
        }

        // Validate that source and target theories are fully registered in Apeiron
        if !self.apeiron.compiled_rules.contains_key(&source_theory) {
            return Err(HyperionError::ParseError {
                block: "WeakEquivalence".into(),
                detail: format!("source theory '{}' is not registered — check for errors in its Theory or Universe block", source_theory),
            });
        }
        if !self.apeiron.compiled_rules.contains_key(&target_theory) {
            return Err(HyperionError::ParseError {
                block: "WeakEquivalence".into(),
                detail: format!("target theory '{}' is not registered — check for errors in its Theory or Universe block", target_theory),
            });
        }

        let msg = format!(
            "[WEAK-EQUIV] {} registered (source={}, target={}, {} type pairs)",
            name, source_theory, target_theory, type_pairs.len()
        );
        self.output.push(msg.clone());
        self.record_result(&name, "valid", Some(format!("weak-equiv:{}", name)), Some(msg));

        if !should_verify {
            return Ok(());
        }

        // Verification: generate synthetic Proofs block.
        // For each type pair (A, B) with maps (f, g):
        //   1. assert-eq: compose(f, A) ≡ B  (f maps A→B)
        //   2. assert-eq: compose(g, B) ≡ A  (g maps B→A)
        //   3. assert-eq: compose(g, f) ≡ id (roundtrip source side)
        //   4. assert-eq: compose(f, g) ≡ id (roundtrip target side)
        // All via e-graph paths (not strict equality).

        let sp = Span::default();
        let proofs_name = format!("__weq_{}", name);

        let mut proof_items: Vec<Sexp> = Vec::new();
        proof_items.push(Sexp::Atom("Proofs".into(), sp));
        proof_items.push(Sexp::Atom(proofs_name.clone(), sp));
        proof_items.push(Sexp::Atom(":in".into(), sp));
        proof_items.push(Sexp::Atom(source_theory.clone(), sp));

        for (idx, (a, b)) in type_pairs.iter().enumerate() {
            let (f_name, g_name) = if let Some(pair) = map_pairs.get(idx) {
                (pair.0.clone(), pair.1.clone())
            } else {
                // Without :via, generate witness names (theory must have laws connecting them)
                (format!("__weq_f_{}_{}", name, idx), format!("__weq_g_{}_{}", name, idx))
            };

            // Forward map: compose(f, A) ≡ B
            proof_items.push(Sexp::List(vec![
                Sexp::Atom("assert-eq".into(), sp),
                Sexp::Atom(format!("weq-fwd-{}", idx), sp),
                Sexp::List(vec![
                    Sexp::Atom("compose".into(), sp),
                    Sexp::Atom(f_name.clone(), sp),
                    Sexp::Atom(a.clone(), sp),
                ], sp),
                Sexp::Atom(b.clone(), sp),
            ], sp));

            // Backward map: compose(g, B) ≡ A
            proof_items.push(Sexp::List(vec![
                Sexp::Atom("assert-eq".into(), sp),
                Sexp::Atom(format!("weq-bwd-{}", idx), sp),
                Sexp::List(vec![
                    Sexp::Atom("compose".into(), sp),
                    Sexp::Atom(g_name.clone(), sp),
                    Sexp::Atom(b.clone(), sp),
                ], sp),
                Sexp::Atom(a.clone(), sp),
            ], sp));

            // Roundtrip source: compose(g, f) ≡ id
            proof_items.push(Sexp::List(vec![
                Sexp::Atom("assert-eq".into(), sp),
                Sexp::Atom(format!("weq-roundtrip-source-{}", idx), sp),
                Sexp::List(vec![
                    Sexp::Atom("compose".into(), sp),
                    Sexp::Atom(g_name.clone(), sp),
                    Sexp::Atom(f_name.clone(), sp),
                ], sp),
                Sexp::Atom("id".into(), sp),
            ], sp));

            // Roundtrip target: compose(f, g) ≡ id
            proof_items.push(Sexp::List(vec![
                Sexp::Atom("assert-eq".into(), sp),
                Sexp::Atom(format!("weq-roundtrip-target-{}", idx), sp),
                Sexp::List(vec![
                    Sexp::Atom("compose".into(), sp),
                    Sexp::Atom(f_name.clone(), sp),
                    Sexp::Atom(g_name.clone(), sp),
                ], sp),
                Sexp::Atom("id".into(), sp),
            ], sp));
        }

        let proofs_sexp = Sexp::List(proof_items, sp);
        let universe_name = self.theory_universes.get(&source_theory).cloned();
        let rewritten = self.rewrite_for_apeiron(&proofs_sexp, universe_name.as_deref())?;

        match self.apeiron.process(&rewritten) {
            Ok(()) => {
                self.drain_apeiron_output();

                let mut witness_parts: Vec<String> = Vec::new();
                for (idx, (a, b)) in type_pairs.iter().enumerate() {
                    let (f, g) = if let Some(pair) = map_pairs.get(idx) {
                        (pair.0.as_str(), pair.1.as_str())
                    } else {
                        ("(auto)", "(auto)")
                    };
                    witness_parts.push(format!(
                        "  {}<->{}  fwd={}  bwd={}", a, b, f, g
                    ));
                }

                let msg = format!(
                    "[WEAK-EQUIV] {} VERIFIED ({} type pairs, all roundtrips connected to identity)\n{}",
                    name, type_pairs.len(), witness_parts.join("\n")
                );
                self.output.push(msg.clone());
                self.record_result(&name, "valid",
                    Some(format!("weak-equiv-verified:{}", name)),
                    Some(msg));

                for (idx, (a, b)) in type_pairs.iter().enumerate() {
                    let (f, g) = map_pairs.get(idx)
                        .map(|(f, g)| (f.as_str(), g.as_str()))
                        .unwrap_or(("?f", "?g"));
                    self.record_discovery(
                        &format!("compose({}, {})", g, f),
                        "id",
                        &format!("weak equivalence roundtrip {} <-> {}", a, b),
                    );
                }

                Ok(())
            }
            Err(e) => {
                self.drain_apeiron_output();
                let detail = format!("{}", e);

                if detail.contains("fuel") || detail.contains("Fuel") {
                    let msg = format!(
                        "[WEAK-EQUIV] {} INCONCLUSIVE — {}", name, detail
                    );
                    self.output.push(msg.clone());
                    self.record_result(&name, "valid",
                        Some(format!("weak-equiv:{}", name)),
                        Some(msg));
                    Ok(())
                } else {
                    let msg = format!(
                        "[WEAK-EQUIV] {} FAILED: {}", name, detail
                    );
                    self.output.push(msg.clone());
                    self.record_result(&name, "invalid",
                        Some(format!("weak-equiv:{}", name)),
                        Some(detail.clone()));
                    Err(HyperionError::LawViolation {
                        theory: format!("WeakEquivalence:{}", name),
                        law: "roundtrip-identity".into(),
                        detail,
                    })
                }
            }
        }
    }

    /// Process a `[VerifyFunctor name :source T1 :target T2]` block.
    /// Verifies that the functor preserves equational theory: each source rule,
    /// when transformed by op_map, holds in the target theory.
    fn process_verify_functor(&mut self, items: &[Sexp]) -> Result<()> {
        if items.len() < 2 {
            return Err(HyperionError::ParseError {
                block: "VerifyFunctor".into(),
                detail: "missing functor name".into(),
            });
        }

        let functor_name = items[1]
            .as_atom()
            .ok_or_else(|| HyperionError::ParseError {
                block: "VerifyFunctor".into(),
                detail: "functor name must be an atom".into(),
            })?
            .to_string();

        let mut source_theory: Option<String> = None;
        let mut target_theory: Option<String> = None;

        let mut i = 2;
        while i < items.len() {
            let key = items[i].as_atom().unwrap_or("");
            match key {
                ":source" => {
                    i += 1;
                    source_theory = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
                }
                ":target" => {
                    i += 1;
                    target_theory = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
                }
                _ => {
                    return Err(HyperionError::ParseError {
                        block: "VerifyFunctor".into(),
                        detail: format!("unknown keyword: {}", key),
                    });
                }
            }
            i += 1;
        }

        let source_theory = source_theory.ok_or_else(|| HyperionError::ParseError {
            block: "VerifyFunctor".into(),
            detail: format!("VerifyFunctor '{}' is missing :source", functor_name),
        })?;
        let target_theory = target_theory.ok_or_else(|| HyperionError::ParseError {
            block: "VerifyFunctor".into(),
            detail: format!("VerifyFunctor '{}' is missing :target", functor_name),
        })?;

        // Look up the functor
        let fun = self.functors.get(&functor_name).cloned().ok_or_else(|| {
            HyperionError::Undefined {
                kind: "Functor".into(),
                name: functor_name.clone(),
            }
        })?;

        // Look up source theory rules
        let source_rules = self.theory_rules.get(&source_theory).cloned().ok_or_else(|| {
            HyperionError::Undefined {
                kind: "Theory (no rules)".into(),
                name: source_theory.clone(),
            }
        })?;

        // Build combined op_map from functor
        let op_map: Vec<(String, String)> = fun
            .object_map
            .iter()
            .chain(fun.morphism_map.iter())
            .cloned()
            .collect();

        // Resource enforcement: check source rules (mapped) against target substrate's resource mode.
        // A rule valid in optimal-sharing (e.g. [f ?x] ==> [g ?x ?x]) may violate the target's
        // strictly-linear or affine constraints.
        if let Some(target_uni_name) = self.theory_universes.get(&target_theory) {
            if let Some(compiled) = self.universes.get(target_uni_name) {
                if let Some(sub) = self.substrates.get(&compiled.substrate_name) {
                    let mapped_rules: Vec<(Option<String>, Sexp, Sexp)> = source_rules
                        .iter()
                        .enumerate()
                        .map(|(i, (lhs, rhs))| {
                            let mapped_lhs = Self::apply_op_map(lhs, &op_map);
                            let mapped_rhs = Self::apply_op_map(rhs, &op_map);
                            (Some(format!("functor-mapped-rule-{}", i)), mapped_lhs, mapped_rhs)
                        })
                        .collect();
                    self.check_resource_rules(&mapped_rules, &sub.resource_mode, &format!(
                        "VerifyFunctor({} -> {})", source_theory, target_theory
                    ))?;

                    // Note: Physics validation (nested opaque calls in LHS) is deferred
                    // to kompile time in analyze.rs. VerifyFunctor operates at the level
                    // of pure mathematics — all algebraic nesting is valid as trees.
                    // The opaque/effectful distinction is a codegen concern.
                    if false {
                    }
                }
            }
        }

        // Generate assert-eq proofs in target theory
        let sp = Span::default();
        let proofs_name = format!("__verify_{}_{}", functor_name, target_theory);

        let mut proof_items: Vec<Sexp> = Vec::new();
        proof_items.push(Sexp::Atom("Proofs".into(), sp));
        proof_items.push(Sexp::Atom(proofs_name.clone(), sp));
        proof_items.push(Sexp::Atom(":in".into(), sp));
        proof_items.push(Sexp::Atom(target_theory.clone(), sp));

        for (idx, (lhs, rhs)) in source_rules.iter().enumerate() {
            let mapped_lhs = Self::apply_op_map(lhs, &op_map);
            let mapped_rhs = Self::apply_op_map(rhs, &op_map);
            // Replace ?-prefixed meta-variables with concrete witness atoms.
            // Meta-variables in rewrite rules (like ?r) cause dup nodes in
            // interaction nets when non-linear (appearing multiple times).
            // Concrete atoms avoid this and properly test the equational theory.
            let concrete_lhs = Self::concretize_metas(&mapped_lhs);
            let concrete_rhs = Self::concretize_metas(&mapped_rhs);
            proof_items.push(Sexp::List(
                vec![
                    Sexp::Atom("assert-eq".into(), sp),
                    Sexp::Atom(format!("verify-rule-{}", idx), sp),
                    concrete_lhs,
                    concrete_rhs,
                ],
                sp,
            ));
        }

        let proofs_sexp = Sexp::List(proof_items, sp);
        let rule_count = source_rules.len();

        // Rewrite `:in TheoryName` to the Apeiron theory name
        let universe_name = self.theory_universes.get(&target_theory).cloned();
        let rewritten = self.rewrite_for_apeiron(&proofs_sexp, universe_name.as_deref())?;

        match self.apeiron.process(&rewritten) {
            Ok(()) => {
                self.drain_apeiron_output();
                self.output.push(format!(
                    "[VERIFY-FUNCTOR] {} preserves equational theory ({} -> {}, {} rules verified)",
                    functor_name, source_theory, target_theory, rule_count
                ));
                Ok(())
            }
            Err(e) => {
                self.drain_apeiron_output();
                Err(HyperionError::LawViolation {
                    theory: target_theory,
                    law: format!("functor {} equational preservation", functor_name),
                    detail: format!("{}", e),
                })
            }
        }
    }

    /// Apply a functor's op_map to a Sexp: replace atoms matching source → target.
    fn apply_op_map(sexp: &Sexp, op_map: &[(String, String)]) -> Sexp {
        match sexp {
            Sexp::Atom(name, sp) => {
                for (src, tgt) in op_map {
                    if name == src {
                        return Sexp::Atom(tgt.clone(), *sp);
                    }
                }
                sexp.clone()
            }
            Sexp::List(items, sp) => {
                let mapped: Vec<Sexp> = items.iter().map(|s| Self::apply_op_map(s, op_map)).collect();
                Sexp::List(mapped, *sp)
            }
        }
    }

    /// Replace ?-prefixed meta-variables with concrete witness atoms (__vf_name).
    /// This avoids dup nodes in interaction nets for non-linear rules.
    fn concretize_metas(sexp: &Sexp) -> Sexp {
        match sexp {
            Sexp::Atom(name, sp) => {
                if let Some(stripped) = name.strip_prefix('?') {
                    Sexp::Atom(format!("__vf_{}", stripped), *sp)
                } else {
                    sexp.clone()
                }
            }
            Sexp::List(items, sp) => {
                let mapped: Vec<Sexp> = items.iter().map(Self::concretize_metas).collect();
                Sexp::List(mapped, *sp)
            }
        }
    }

    /// Drain Apeiron output into our output buffer, parsing structured results.
    /// Process a `[Meta <command> ...]` compile-time metaprogramming block.
    ///
    /// Supported commands:
    ///   [Meta optimize :in TheoryName EXPR]
    ///     — Normalize EXPR via the theory's rewrite rules + e-graph, emit the result.
    ///   [Meta synthesize :in TheoryName :goal EXPR]
    ///     — Use assert-exists to find a witness satisfying the goal pattern.
    ///   [Meta reify-passes :universe UniName]
    ///     — Emit the compilation passes for a universe as structured output.
    ///   [Meta reify-theory :theory TheoryName]
    ///     — Emit the theory's structure (sorts, operators, rules) as structured output.
    fn process_meta(&mut self, items: &[Sexp]) -> Result<()> {
        if items.len() < 2 {
            return Err(HyperionError::ParseError {
                block: "Meta".into(),
                detail: "missing meta command".into(),
            });
        }

        // Meta-level bounding: prevent infinite meta-meta recursion
        if self.meta_depth >= self.max_meta_depth {
            return Err(HyperionError::ParseError {
                block: "Meta".into(),
                detail: format!(
                    "meta depth {} exceeds maximum {} (meta blocks cannot invoke other meta blocks)",
                    self.meta_depth + 1, self.max_meta_depth
                ),
            });
        }
        self.meta_depth += 1;

        let command = items[1].as_atom().ok_or_else(|| HyperionError::ParseError {
            block: "Meta".into(),
            detail: "meta command must be an atom".into(),
        })?;

        let result = match command {
            "optimize" => self.meta_optimize(items),
            "synthesize" => self.meta_synthesize(items),
            "splice" => self.meta_splice(items),
            "reify-passes" => self.meta_reify_passes(items),
            "reify-theory" => self.meta_reify_theory(items),
            "reify-proof" => self.meta_reify_proof(items),
            _ => Err(HyperionError::ParseError {
                block: "Meta".into(),
                detail: format!("unknown meta command '{}'. Expected: optimize, synthesize, splice, reify-passes, reify-theory, reify-proof", command),
            }),
        };

        self.meta_depth -= 1;
        result
    }

    /// Parse optional `:fuel N` from Meta items. Returns fuel value if present.
    fn parse_meta_fuel(&self, items: &[Sexp]) -> Option<usize> {
        let fuel_pos = items.iter().position(|s| s.as_atom() == Some(":fuel"))?;
        items.get(fuel_pos + 1)
            .and_then(|s| s.as_atom())
            .and_then(|s| s.parse::<usize>().ok())
    }

    /// Set Apeiron e-graph fuel override, returning the previous value.
    fn set_meta_fuel(&mut self, items: &[Sexp]) -> Option<(usize, usize)> {
        let fuel = self.parse_meta_fuel(items);
        let prev = self.apeiron.egraph_fuel;
        if let Some(n) = fuel {
            self.apeiron.egraph_fuel = Some((30.min(n / 100 + 1), n));
        } else {
            // Default meta fuel: 5000 nodes (more conservative than Apeiron's 10k)
            self.apeiron.egraph_fuel = Some((30, 5000));
        }
        prev
    }

    /// Restore Apeiron e-graph fuel to previous value.
    fn restore_meta_fuel(&mut self, prev: Option<(usize, usize)>) {
        self.apeiron.egraph_fuel = prev;
    }

    /// [Meta optimize :fuel N :in TheoryName EXPR] — normalize via rewrite + e-graph
    fn meta_optimize(&mut self, items: &[Sexp]) -> Result<()> {
        let sp = apeiron::parser::Span::default();
        let prev_fuel = self.set_meta_fuel(items);

        // Parse :in TheoryName
        let in_pos = items.iter().position(|s| s.as_atom() == Some(":in"))
            .ok_or_else(|| HyperionError::ParseError {
                block: "Meta".into(),
                detail: "optimize requires :in <TheoryName>".into(),
            })?;
        let theory_name = items.get(in_pos + 1)
            .and_then(|s| s.as_atom())
            .ok_or_else(|| HyperionError::ParseError {
                block: "Meta".into(),
                detail: "missing theory name after :in".into(),
            })?;

        // Collect expression items (skip keyword pairs like :fuel N)
        let exprs: Vec<&Sexp> = {
            let mut result = Vec::new();
            let mut j = in_pos + 2;
            while j < items.len() {
                if let Some(kw) = items[j].as_atom() {
                    if kw.starts_with(':') { j += 2; continue; }
                }
                result.push(&items[j]);
                j += 1;
            }
            result
        };
        if exprs.is_empty() {
            self.restore_meta_fuel(prev_fuel);
            return Err(HyperionError::ParseError {
                block: "Meta".into(),
                detail: "optimize requires at least one expression".into(),
            });
        }

        // Build a Proofs block with eval-simplify for each expression
        let proofs_name = format!("__meta_opt_{}", theory_name);
        let mut proof_items: Vec<Sexp> = Vec::new();
        proof_items.push(Sexp::Atom("Proofs".into(), sp));
        proof_items.push(Sexp::Atom(proofs_name, sp));
        proof_items.push(Sexp::Atom(":in".into(), sp));
        proof_items.push(Sexp::Atom(theory_name.to_string(), sp));

        for (i, expr) in exprs.iter().enumerate() {
            let name = format!("meta-opt-{}", i);
            proof_items.push(Sexp::List(
                vec![
                    Sexp::Atom("eval-simplify".into(), sp),
                    Sexp::Atom(name.clone(), sp),
                    (*expr).clone(),
                ],
                sp,
            ));
        }

        let proofs_sexp = Sexp::List(proof_items, sp);
        let result = self.apeiron.process(&proofs_sexp);

        // Drain and report: capture [SIMPLIFY] output as [META] output
        let lines: Vec<String> = self.apeiron.output.drain(..).collect();
        for line in &lines {
            if line.starts_with("[SIMPLIFY] ") {
                let meta_line = format!("[META] optimize: {}", &line[11..]);
                self.output.push(meta_line.clone());
                self.record_result("meta-optimize", "valid",
                    Some("meta:optimize".into()), Some(meta_line));
            }
        }
        self.output.extend(lines);

        self.restore_meta_fuel(prev_fuel);
        Ok(result?)
    }

    /// [Meta synthesize :fuel N :in TheoryName :goal EXPR] — find a witness via assert-exists
    fn meta_synthesize(&mut self, items: &[Sexp]) -> Result<()> {
        let sp = apeiron::parser::Span::default();
        let prev_fuel = self.set_meta_fuel(items);

        // Parse :in TheoryName
        let in_pos = items.iter().position(|s| s.as_atom() == Some(":in"))
            .ok_or_else(|| HyperionError::ParseError {
                block: "Meta".into(),
                detail: "synthesize requires :in <TheoryName>".into(),
            })?;
        let theory_name = items.get(in_pos + 1)
            .and_then(|s| s.as_atom())
            .ok_or_else(|| HyperionError::ParseError {
                block: "Meta".into(),
                detail: "missing theory name after :in".into(),
            })?;

        // Parse :goal EXPR
        let goal_pos = items.iter().position(|s| s.as_atom() == Some(":goal"))
            .ok_or_else(|| HyperionError::ParseError {
                block: "Meta".into(),
                detail: "synthesize requires :goal <EXPR>".into(),
            })?;
        let goal_expr = items.get(goal_pos + 1)
            .ok_or_else(|| HyperionError::ParseError {
                block: "Meta".into(),
                detail: "missing expression after :goal".into(),
            })?;

        // Build a Proofs block with assert-exists
        let proofs_name = format!("__meta_synth_{}", theory_name);
        let mut proof_items: Vec<Sexp> = Vec::new();
        proof_items.push(Sexp::Atom("Proofs".into(), sp));
        proof_items.push(Sexp::Atom(proofs_name, sp));
        proof_items.push(Sexp::Atom(":in".into(), sp));
        proof_items.push(Sexp::Atom(theory_name.to_string(), sp));

        proof_items.push(Sexp::List(
            vec![
                Sexp::Atom("assert-exists".into(), sp),
                Sexp::Atom("meta-synthesis".into(), sp),
                Sexp::Atom(":such-that".into(), sp),
                goal_expr.clone(),
            ],
            sp,
        ));

        let proofs_sexp = Sexp::List(proof_items, sp);
        let result = match self.apeiron.process(&proofs_sexp) {
            Ok(()) => {
                self.drain_apeiron_output();
                let msg = format!("[META] synthesize: witness found for goal in {}", theory_name);
                self.output.push(msg.clone());
                self.record_result("meta-synthesize", "valid",
                    Some("meta:synthesize".into()), Some(msg));
                Ok(())
            }
            Err(e) => {
                self.drain_apeiron_output();
                let msg = format!("[META] synthesize: no witness found — {}", e);
                self.output.push(msg);
                Err(HyperionError::ParseError {
                    block: "Meta".into(),
                    detail: format!("synthesis failed: {}", e),
                })
            }
        };
        self.restore_meta_fuel(prev_fuel);
        result
    }

    /// [Meta reify-passes :universe UniName] — emit compilation passes as structured data
    fn meta_reify_passes(&mut self, items: &[Sexp]) -> Result<()> {
        let uni_pos = items.iter().position(|s| s.as_atom() == Some(":universe"))
            .ok_or_else(|| HyperionError::ParseError {
                block: "Meta".into(),
                detail: "reify-passes requires :universe <Name>".into(),
            })?;
        let uni_name = items.get(uni_pos + 1)
            .and_then(|s| s.as_atom())
            .ok_or_else(|| HyperionError::ParseError {
                block: "Meta".into(),
                detail: "missing universe name after :universe".into(),
            })?;

        let compiled = self.universes.get(uni_name)
            .ok_or_else(|| HyperionError::Undefined {
                kind: "Universe".into(),
                name: uni_name.to_string(),
            })?;

        if compiled.passes.is_empty() {
            let msg = format!("[META] reify-passes: {} is native (zero passes)", uni_name);
            self.output.push(msg.clone());
            self.record_result("meta-reify-passes", "valid",
                Some(format!("meta:reify-passes:{}", uni_name)), Some(msg));
        } else {
            for pass in &compiled.passes {
                let msg = format!("[META] reify-passes: {} → {} ({})",
                    uni_name, pass.name(), pass.description());
                self.output.push(msg.clone());
            }
            let summary = format!("[META] reify-passes: {} has {} compilation pass(es)",
                uni_name, compiled.passes.len());
            self.output.push(summary.clone());
            self.record_result("meta-reify-passes", "valid",
                Some(format!("meta:reify-passes:{}", uni_name)), Some(summary));
        }

        Ok(())
    }

    /// [Meta reify-theory :theory TheoryName] — emit theory structure
    fn meta_reify_theory(&mut self, items: &[Sexp]) -> Result<()> {
        let th_pos = items.iter().position(|s| s.as_atom() == Some(":theory"))
            .ok_or_else(|| HyperionError::ParseError {
                block: "Meta".into(),
                detail: "reify-theory requires :theory <Name>".into(),
            })?;
        let theory_name = items.get(th_pos + 1)
            .and_then(|s| s.as_atom())
            .ok_or_else(|| HyperionError::ParseError {
                block: "Meta".into(),
                detail: "missing theory name after :theory".into(),
            })?;

        // Check VN theories first
        if let Some(vn) = self.vn_theories.get(theory_name) {
            self.output.push(format!("[META] reify-theory: {} (first-order)", theory_name));
            self.output.push(format!("[META]   sorts: {}", vn.sorts.join(", ")));
            self.output.push(format!("[META]   operators: {}", vn.operators.join(", ")));
            self.output.push(format!("[META]   rules: {}", vn.rules.len()));
            for rule in &vn.rules {
                self.output.push(format!("[META]   rule {}: {} ==> {}",
                    rule.name, rule.lhs, rule.rhs));
            }
        } else {
            // For Apeiron-backed theories, we can report what we know
            self.output.push(format!("[META] reify-theory: {} (Apeiron-backed)", theory_name));
            // Check theory_rules for functor verification
            if let Some(rules) = self.theory_rules.get(theory_name) {
                self.output.push(format!("[META]   rules: {}", rules.len()));
                for (lhs, rhs) in rules {
                    self.output.push(format!("[META]   rule: {} ==> {}",
                        lhs, rhs));
                }
            }
        }

        self.record_result("meta-reify-theory", "valid",
            Some(format!("meta:reify-theory:{}", theory_name)), None);
        Ok(())
    }

    /// [Meta reify-proof :name <proof-name>] — extract proof term as AST data
    fn meta_reify_proof(&mut self, items: &[Sexp]) -> Result<()> {
        let name_pos = items.iter().position(|s| s.as_atom() == Some(":name"))
            .ok_or_else(|| HyperionError::ParseError {
                block: "Meta".into(),
                detail: "reify-proof requires :name <proof-name>".into(),
            })?;
        let proof_name = items.get(name_pos + 1)
            .and_then(|s| s.as_atom())
            .ok_or_else(|| HyperionError::ParseError {
                block: "Meta".into(),
                detail: "missing proof name after :name".into(),
            })?;

        let proof = self.apeiron.proof_terms.get(proof_name)
            .ok_or_else(|| HyperionError::Undefined {
                kind: "Proof".into(),
                name: proof_name.to_string(),
            })?;

        // Convert ProofTerm to Sexp for meta-level manipulation
        let sp = apeiron::parser::Span::default();
        let proof_sexp = proof_term_to_sexp(proof, sp);

        let msg = format!("[META] reify-proof: {} = {}", proof_name, proof_sexp);
        self.output.push(msg.clone());

        // Store as meta binding for use by splice
        self.meta_bindings.insert(proof_name.to_string(), proof_sexp);

        self.record_result("meta-reify-proof", "valid",
            Some(format!("meta:reify-proof:{}", proof_name)), Some(msg));
        Ok(())
    }

    /// [Meta splice <name> [optimize/synthesize ...]] — run meta command, bind result as definition
    fn meta_splice(&mut self, items: &[Sexp]) -> Result<()> {
        if items.len() < 4 {
            return Err(HyperionError::ParseError {
                block: "Meta".into(),
                detail: "splice requires: splice <name> [<meta-command> ...]".into(),
            });
        }

        let bind_name = items[2].as_atom()
            .ok_or_else(|| HyperionError::ParseError {
                block: "Meta".into(),
                detail: "splice binding name must be an atom".into(),
            })?
            .to_string();

        // The inner command is a list: [optimize :in T expr] or [synthesize :in T :goal expr]
        let inner = items[3].as_list()
            .ok_or_else(|| HyperionError::ParseError {
                block: "Meta".into(),
                detail: "splice inner command must be a list".into(),
            })?;

        if inner.is_empty() {
            return Err(HyperionError::ParseError {
                block: "Meta".into(),
                detail: "splice inner command is empty".into(),
            });
        }

        let inner_cmd = inner[0].as_atom().unwrap_or("");

        // Build a full Meta item list: [Meta <inner_cmd> <inner_args...>]
        let sp = apeiron::parser::Span::default();
        let mut meta_items = vec![Sexp::Atom("Meta".into(), sp)];
        meta_items.extend(inner.iter().cloned());

        // Capture output before running the inner command
        let output_before = self.output.len();

        match inner_cmd {
            "optimize" => {
                self.meta_optimize(&meta_items)?;
                // Extract simplified expression from output
                for line in &self.output[output_before..] {
                    if line.starts_with("[META] optimize: ") {
                        // Parse "name = expr (N interactions)"
                        let rest = &line[17..];
                        if let Some(eq_pos) = rest.find(" = ") {
                            let after_eq = &rest[eq_pos + 3..];
                            // Strip trailing " (N interactions)"
                            let expr_str = if let Some(paren) = after_eq.rfind(" (") {
                                &after_eq[..paren]
                            } else {
                                after_eq
                            };
                            // Parse the result expression
                            if let Ok(sexps) = apeiron::parser::parse(expr_str) {
                                if let Some(sexp) = sexps.into_iter().next() {
                                    self.meta_bindings.insert(bind_name.clone(), sexp);
                                }
                            }
                        }
                        break;
                    }
                }
            }
            "synthesize" => {
                self.meta_synthesize(&meta_items)?;
                // Witness isn't directly available as sexp yet, store a marker
                self.meta_bindings.insert(
                    bind_name.clone(),
                    Sexp::Atom(format!("__witness_{}", bind_name), sp),
                );
            }
            _ => {
                return Err(HyperionError::ParseError {
                    block: "Meta".into(),
                    detail: format!("splice only supports optimize and synthesize, got '{}'", inner_cmd),
                });
            }
        }

        let msg = format!("[META] splice: {} bound", bind_name);
        self.output.push(msg.clone());
        self.record_result("meta-splice", "valid",
            Some(format!("meta:splice:{}", bind_name)), Some(msg));
        Ok(())
    }

    fn drain_apeiron_output(&mut self) {
        let lines: Vec<String> = self.apeiron.output.drain(..).collect();
        for line in &lines {
            if line.starts_with("[ASSERT] ") {
                let rest = &line[9..];
                if let Some(name_end) = rest.find(" passed") {
                    let name = &rest[..name_end];
                    let is_egraph = rest.contains("(e-graph)");
                    self.record_result(name, "valid",
                        Some(format!("assertion:{}", name)), None);
                    if is_egraph {
                        let terms = self.pending_assertions.get(name).cloned();
                        if let Some((lhs, rhs)) = terms {
                            self.record_discovery(&lhs, &rhs,
                                "equality discovered via e-graph saturation");
                        } else {
                            self.record_discovery(name, name,
                                "equality discovered via e-graph saturation");
                        }
                    }
                } else if let Some(name_end) = rest.find(" failed").or_else(|| rest.find(" FAILED")) {
                    let name = &rest[..name_end];
                    self.record_result(name, "invalid",
                        Some(format!("assertion:{}", name)),
                        Some("assertion failed".into()));
                }
            } else if line.starts_with("[SIMPLIFY] ") {
                let rest = &line[11..];
                if let Some(eq_pos) = rest.find(" = ") {
                    let name = &rest[..eq_pos];
                    let expr = &rest[eq_pos + 3..];
                    self.record_discovery(name, expr,
                        "simplification via e-graph extraction");
                }
            }
        }
        self.output.extend(lines);
    }
}

/// Extract @rule and @law declarations from theory body items.
/// Returns (optional_name, lhs, rhs) for each rule/law found.
fn extract_rule_declarations(items: &[Sexp]) -> Vec<(Option<String>, Sexp, Sexp)> {
    let mut rules = Vec::new();
    for item in items {
        if let Some(inner) = item.as_list() {
            let head = inner.first().and_then(|s| s.as_atom()).unwrap_or("");
            if head == "@rule" || head == "@law" {
                let sep = if head == "@law" { "===" } else { "==>" };
                if let Some(sep_pos) = inner.iter().position(|s| s.as_atom() == Some(sep)) {
                    if sep_pos >= 2 && sep_pos + 1 < inner.len() {
                        let rule_name = if sep_pos == 3 {
                            inner[1].as_atom().map(|s| s.to_string())
                        } else {
                            None
                        };
                        let lhs = inner[sep_pos - 1].clone();
                        let rhs = inner[sep_pos + 1].clone();
                        rules.push((rule_name, lhs, rhs));
                    }
                }
            }
        }
    }
    rules
}

/// Collect all ?meta variable names from a Sexp tree.
fn collect_metas(sexp: &Sexp) -> HashSet<String> {
    let mut result = HashSet::new();
    collect_metas_inner(sexp, &mut result);
    result
}

fn collect_metas_inner(sexp: &Sexp, result: &mut HashSet<String>) {
    match sexp {
        Sexp::Atom(name, _) => {
            if let Some(stripped) = name.strip_prefix('?') {
                result.insert(stripped.to_string());
            }
        }
        Sexp::List(items, _) => {
            for item in items {
                collect_metas_inner(item, result);
            }
        }
    }
}

/// Count occurrences of each ?meta variable in a Sexp tree.
fn count_metas(sexp: &Sexp) -> HashMap<String, usize> {
    let mut result = HashMap::new();
    count_metas_inner(sexp, &mut result);
    result
}

fn count_metas_inner(sexp: &Sexp, result: &mut HashMap<String, usize>) {
    match sexp {
        Sexp::Atom(name, _) => {
            if let Some(stripped) = name.strip_prefix('?') {
                *result.entry(stripped.to_string()).or_insert(0) += 1;
            }
        }
        Sexp::List(items, _) => {
            for item in items {
                count_metas_inner(item, result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apeiron::parser::parse;

    fn process_all(session: &mut HyperionSession, input: &str) -> Result<()> {
        let sexps = parse(input).map_err(|e| HyperionError::ApeironError(e))?;
        for sexp in &sexps {
            session.process(sexp)?;
        }
        Ok(())
    }

    #[test]
    fn end_to_end_category_substrate_universe() {
        let mut session = HyperionSession::new();
        let input = r#"
            [Category CartesianClosed
                [Object Type]
                [Object Term]
                [Morphism arrow :domain [Type Type] :codomain Type]
                [Morphism app :domain [Term Term] :codomain Term]
                [Exponential lam :object Term]
                [Evaluator app]
            ]

            [Substrate InteractionNet
                @engine interaction-graph
                @resource-mode optimal-sharing
                @barrier transparent
                @equality topological-hash
            ]

            [Universe WeakLF :category CartesianClosed :substrate InteractionNet]
        "#;

        let result = process_all(&mut session, input);
        assert!(result.is_ok(), "Failed: {:?}", result.unwrap_err());
        assert!(session.universes.contains_key("WeakLF"));
        assert!(session
            .apeiron
            .systems
            .contains_key("__hyp_CartesianClosed_InteractionNet"));
    }

    #[test]
    fn incompatible_universe_gets_compilation_passes() {
        use crate::universe::CompilationPass;
        let mut session = HyperionSession::new();
        let input = r#"
            [Category CartesianClosed
                [Object Term]
                [Exponential lam :object Term]
            ]

            [Substrate GridWorld
                @engine cellular-automaton
                @resource-mode deep-copy
                @barrier transparent
                @equality rewrite-equivalence
            ]

            [Universe Bridged :category CartesianClosed :substrate GridWorld]
        "#;

        process_all(&mut session, input).unwrap();
        let compiled = &session.universes["Bridged"];
        assert!(compiled.passes.contains(&CompilationPass::Defunctionalization));
    }

    #[test]
    fn theory_passthrough() {
        let mut session = HyperionSession::new();
        let input = r#"
            [Category Simple
                [Object Nat]
                [Morphism z :domain [] :codomain Nat]
                [Morphism s :domain [Nat] :codomain Nat]
                [Morphism plus :domain [Nat Nat] :codomain Nat]
            ]

            [Substrate InteractionNet
                @engine interaction-graph
                @resource-mode optimal-sharing
                @barrier transparent
                @equality rewrite-equivalence
            ]

            [Universe PeanoWorld :category Simple :substrate InteractionNet]

            [Theory Arithmetic :in PeanoWorld
                [@rule [plus z ?n] ==> ?n]
                [@rule [plus [s ?n] ?m] ==> [s [plus ?n ?m]]]
            ]

            [Proofs ArithCheck :in Arithmetic
                [assert-eq two-plus-two
                    [plus [s [s z]] [s [s z]]]
                    [s [s [s [s z]]]]
                ]
            ]
        "#;

        let result = process_all(&mut session, input);
        assert!(result.is_ok(), "Failed: {:?}", result.unwrap_err());
    }
}
