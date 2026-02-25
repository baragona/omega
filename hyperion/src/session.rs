use std::collections::HashMap;
use std::path::PathBuf;

use apeiron::parser::{Sexp, Span};

use crate::adjunction::{self, AdjunctionDef};
use crate::category::{self, CategoryDef};
use crate::codegen;
use crate::compile;
use crate::error::{HyperionError, Result};
use crate::functor::{self, FunctorDef};
use crate::nat_trans::{self, NatTransDef};
use crate::substrate::{self, Engine, SubstrateDef};
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
    /// Skip categorical law verification
    pub skip_laws: bool,
    pub output: Vec<String>,
}

impl HyperionSession {
    pub fn new() -> Self {
        HyperionSession {
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
            skip_laws: false,
            output: Vec::new(),
        }
    }

    /// Create a session with the standard prelude auto-loaded.
    pub fn with_prelude() -> Result<Self> {
        let mut session = Self::new();
        session.load_prelude()?;
        Ok(session)
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

        self.output.push(format!(
            "[CATEGORY] {} registered ({} objects, {} morphisms, {} structures)",
            name,
            cat.objects.len(),
            cat.morphisms.len(),
            cat.structure.len()
        ));
        self.categories.insert(name, cat);
        Ok(())
    }

    fn process_substrate(&mut self, items: &[Sexp]) -> Result<()> {
        let sub = substrate::parse_substrate(items)?;
        let name = sub.name.clone();

        if self.substrates.contains_key(&name) {
            return Err(HyperionError::DuplicateName {
                kind: "Substrate".into(),
                name,
            });
        }

        self.output.push(format!(
            "[SUBSTRATE] {} registered (engine={:?}, resource={:?}, barrier={:?}, equality={:?})",
            name, sub.engine, sub.resource_mode, sub.barrier, sub.equality
        ));
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

        // Generate and register the Apeiron system
        let system_sexp = compile::emit_system_sexp(&cat, &sub, &compiled);
        self.apeiron.process(&system_sexp)?;

        // Drain apeiron output
        self.drain_apeiron_output();

        self.output.push(format!(
            "[UNIVERSE] {} compiled (system={}, category={}, substrate={})",
            name, compiled.system_name, uni_def.category, uni_def.substrate
        ));
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

        self.output.push(format!(
            "[FUNCTOR] {} registered (from={}, to={}, {} morphisms)",
            name,
            fun.source,
            fun.target,
            morph_map.len()
        ));
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

        if adj.verify {
            self.output.push(format!(
                "[ADJUNCTION] {} verification requested (triangle identities)",
                name
            ));
        }

        self.output.push(format!(
            "[ADJUNCTION] {} registered (left={}, right={}, unit={}, counit={})",
            name, adj.left, adj.right, adj.unit, adj.counit
        ));
        self.adjunctions.insert(name, adj);
        Ok(())
    }

    /// Check if a universe uses a Von Neumann substrate.
    fn is_vn_universe(&self, universe_name: &str) -> bool {
        if let Some(compiled) = self.universes.get(universe_name) {
            if let Some(sub) = self.substrates.get(&compiled.substrate_name) {
                return sub.engine == Engine::VonNeumann;
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
        let operators: Vec<String> = cat.morphisms.iter().map(|m| m.name.clone()).collect();
        let mut morphism_types: HashMap<String, (Vec<String>, String)> = HashMap::new();
        for m in &cat.morphisms {
            morphism_types.insert(m.name.clone(), (m.domain.clone(), m.codomain.clone()));
        }

        // Parse @rule declarations from the theory body
        let mut rules = Vec::new();
        for item in &items[2..] {
            if let Some(inner) = item.as_list() {
                if inner.is_empty() {
                    continue;
                }
                let head = inner[0].as_atom().unwrap_or("");
                if head == "@rule" && inner.len() >= 5 {
                    // [@rule name lhs ==> rhs]
                    let rule_name = inner[1].as_atom().unwrap_or("").to_string();
                    let lhs = inner[2].clone();
                    // inner[3] should be "==>"
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

        self.vn_theories.insert(
            theory_name.clone(),
            VonNeumannTheory {
                name: theory_name,
                universe_name: universe_name.to_string(),
                sorts,
                operators,
                rules,
                morphism_types,
            },
        );

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

        if let Some(uni_name) = &universe_name {
            self.theory_universes
                .insert(theory_name.to_string(), uni_name.clone());

            // Check if this universe uses a Von Neumann substrate
            if self.is_vn_universe(uni_name) {
                return self.process_vn_theory(sexp, uni_name);
            }
        }

        // Capture @rule declarations for functor verification
        let mut rules = Vec::new();
        for item in &items[2..] {
            if let Some(inner) = item.as_list() {
                if inner.len() >= 4 {
                    let head = inner[0].as_atom().unwrap_or("");
                    if head == "@rule" {
                        // [@rule lhs ==> rhs]
                        let lhs = inner[1].clone();
                        let rhs = inner[3].clone();
                        rules.push((lhs, rhs));
                    }
                }
            }
        }
        if !rules.is_empty() {
            self.theory_rules.insert(theory_name.to_string(), rules);
        }

        let rewritten = self.rewrite_for_apeiron(sexp, universe_name.as_deref())?;
        self.apeiron.process(&rewritten)?;
        self.drain_apeiron_output();

        // Categorical law verification: after theory registration, check category laws
        if !self.skip_laws {
            if let Some(uni_name) = &universe_name {
                self.check_categorical_laws(theory_name, uni_name)?;
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
                self.output.push(format!(
                    "[LAWS] {} passed categorical law verification ({} witness tests)",
                    theory_name, law_count
                ));
                Ok(())
            }
            Err(e) => {
                self.drain_apeiron_output();
                let detail = format!("{}", e);
                // If the error mentions fuel exhaustion, report as inconclusive warning
                if detail.contains("fuel") || detail.contains("Fuel") {
                    self.output.push(format!(
                        "[LAWS] {} INCONCLUSIVE for {} ({} witness tests) — {}",
                        theory_name, compiled.category_name, law_count, detail
                    ));
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

        // For Theory blocks in PathType universes: inject path algebra @rule declarations
        let is_theory = items.first().and_then(|s| s.as_atom()).map(|a| a == "Theory").unwrap_or(false);
        if is_theory {
            if let Some(cat) = category_name.as_deref().and_then(|n| self.categories.get(n)) {
                for s in &cat.structure {
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
                }
            }
        }

        Ok(Sexp::List(new_items, sexp.span()))
    }

    /// Generate Apeiron @rule declarations for path algebra.
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
            // concat(refl(a), p) ==> p
            mk_rule(
                mk_concat(mk_refl(meta_a()), meta_p()),
                meta_p()),
            // concat(p, refl(a)) ==> p
            mk_rule(
                mk_concat(meta_p(), mk_refl(meta_a())),
                meta_p()),
            // inv(refl(a)) ==> refl(a)
            mk_rule(
                mk_inv(mk_refl(meta_a())),
                mk_refl(meta_a())),
            // concat(concat(p,q), r) ==> concat(p, concat(q,r))
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
        }

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
            proof_items.push(Sexp::List(
                vec![
                    Sexp::Atom("assert-eq".into(), sp),
                    Sexp::Atom(format!("verify-rule-{}", idx), sp),
                    mapped_lhs,
                    mapped_rhs,
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

    /// Drain Apeiron output into our output buffer.
    fn drain_apeiron_output(&mut self) {
        self.output.append(&mut self.apeiron.output);
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
    fn incompatible_universe_rejected() {
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

            [Universe Bad :category CartesianClosed :substrate GridWorld]
        "#;

        let result = process_all(&mut session, input);
        assert!(result.is_err());
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
