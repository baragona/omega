use std::collections::{HashMap, HashSet};

use crate::arena::Arena;
use crate::builder::{self, BuildEnv};
use crate::error::{ApeironError, Result};
use crate::hash;
use crate::judgment::{self, DerivRule, JudgmentDecl};
use crate::morphism::{self, AutoMorphism};
use crate::parser::Sexp;
use crate::physics::{self, PhysicsConfig};
use crate::readback;
use crate::refute;
use crate::rewrite;

/// Binding mode for a system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingMode {
    Implicit,
    Exposed,
    Contextual,
    /// Enforce linear usage: every variable must be used exactly once.
    /// Dup (multi-use) and Erase (unused) are rejected.
    LinearExplicit,
    /// Nominal binding: names are meaningful, not alpha-equivalent.
    /// Hashing does NOT canonicalize scope/label IDs.
    Nominal,
}

/// A checking capability.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CheckMode {
    Rewriting,
    Unification,
    BetaReduction,
    Oracle,
    Extensional,
    PatternUnification,
    /// Every rule must be invertible; auto-generates inverse rules.
    Reversible,
    /// Multiple rules may match; non-deterministic selection.
    ConfluentRace,
}

/// An operator declared in @syntax.
#[derive(Debug, Clone)]
pub struct OpDecl {
    pub name: String,
    pub args: Vec<String>,
    pub result: String,
}

/// A sort declared in @syntax.
#[derive(Debug, Clone)]
pub struct SortDecl {
    pub name: String,
}

/// System-level configuration.
#[derive(Debug, Clone)]
pub struct SystemConfig {
    pub name: String,
    pub sorts: Vec<SortDecl>,
    pub operators: Vec<OpDecl>,
    pub binding: BindingMode,
    pub check_modes: HashSet<CheckMode>,
}

/// A rewrite rule: lhs ==> rhs.
#[derive(Debug, Clone)]
pub struct RewriteRule {
    pub name: String,
    pub lhs: Sexp,
    pub rhs: Sexp,
}

/// A running session with loaded systems and theories.
/// A parameterized theory template: raw declarations awaiting instantiation.
#[derive(Debug, Clone)]
pub struct TheoryTemplate {
    pub params: Vec<(String, String)>,       // (name, sort) pairs
    pub system_name: String,
    pub ops: Vec<String>,                    // theory-level op declarations
    pub raw_rules: Vec<RewriteRule>,         // explicit @rule declarations
    pub raw_derives: Vec<DerivRule>,         // @derive declarations
}

pub struct Session {
    pub systems: HashMap<String, SystemConfig>,
    pub arena: Arena,
    /// Rewrite rules indexed by theory.
    pub rules: HashMap<String, Vec<RewriteRule>>,
    /// Definitions: name → Sexp body.
    pub defs: HashMap<String, Sexp>,
    /// Named scopes: name → numeric ID.
    pub scopes: HashMap<String, u32>,
    /// Next scope ID.
    pub next_scope_id: u32,
    /// Output log.
    pub output: Vec<String>,
    /// Registered auto-morphisms: name → AutoMorphism.
    pub morphisms: HashMap<String, AutoMorphism>,
    /// Compiled graph rules indexed by theory name.
    pub compiled_rules: HashMap<String, Vec<rewrite::GraphRule>>,
    /// Theory-level operator declarations (extend known_ops for builder).
    pub extra_known_ops: HashSet<String>,
    /// Theory → System mapping (resolved system name for each theory).
    pub theory_systems: HashMap<String, String>,
    /// Judgment declarations indexed by theory name → judgment name → decl.
    pub judgments: HashMap<String, HashMap<String, JudgmentDecl>>,
    /// Derive rules indexed by theory name → rule name → DerivRule.
    pub derive_rules: HashMap<String, HashMap<String, DerivRule>>,
    /// Derive rules indexed by theory name (ordered list for search).
    pub derive_rules_ordered: HashMap<String, Vec<DerivRule>>,
    /// Parameterized theory templates awaiting instantiation.
    pub templates: HashMap<String, TheoryTemplate>,
    /// Per-theory operator names (for alias renaming at import).
    pub theory_ops: HashMap<String, Vec<String>>,
    /// Per-theory raw @rule declarations (for alias renaming at import).
    pub raw_theory_rules: HashMap<String, Vec<RewriteRule>>,
}

impl Session {
    pub fn new() -> Self {
        Session {
            systems: HashMap::new(),
            arena: Arena::new(),
            rules: HashMap::new(),
            defs: HashMap::new(),
            scopes: HashMap::new(),
            next_scope_id: 0,
            output: Vec::new(),
            morphisms: HashMap::new(),
            compiled_rules: HashMap::new(),
            extra_known_ops: HashSet::new(),
            theory_systems: HashMap::new(),
            judgments: HashMap::new(),
            derive_rules: HashMap::new(),
            derive_rules_ordered: HashMap::new(),
            templates: HashMap::new(),
            theory_ops: HashMap::new(),
            raw_theory_rules: HashMap::new(),
        }
    }

    /// Process a top-level S-expression declaration.
    pub fn process(&mut self, sexp: &Sexp) -> Result<()> {
        let items = sexp
            .as_list()
            .ok_or_else(|| ApeironError::ParseError {
                message: "expected top-level list".into(),
                line: 0,
                col: 0,
            })?;

        if items.is_empty() {
            return Ok(());
        }

        let head = items[0].as_atom().unwrap_or("");
        match head {
            "System" => self.process_system(items),
            "Theory" => self.process_theory(items),
            "Proofs" => self.process_proofs(items),
            "AutoMorphism" => self.process_automorphism(items),
            _ => Err(ApeironError::ParseError {
                message: format!("unknown top-level form: {}", head),
                line: 0,
                col: 0,
            }),
        }
    }

    fn process_system(&mut self, items: &[Sexp]) -> Result<()> {
        if items.len() < 2 {
            return Err(ApeironError::InvalidConfig {
                block: "System".into(),
                detail: "missing system name".into(),
            });
        }

        let name = items[1]
            .as_atom()
            .ok_or_else(|| ApeironError::InvalidConfig {
                block: "System".into(),
                detail: "system name must be an atom".into(),
            })?
            .to_string();

        let mut config = SystemConfig {
            name: name.clone(),
            sorts: Vec::new(),
            operators: Vec::new(),
            binding: BindingMode::Implicit,
            check_modes: HashSet::new(),
        };

        // Parse blocks within the System
        for item in &items[2..] {
            if let Some(block) = item.as_list() {
                if block.is_empty() {
                    continue;
                }
                let block_head = block[0].as_atom().unwrap_or("");
                match block_head {
                    "@syntax" => parse_syntax_block(&block[1..], &mut config)?,
                    "@binding" => parse_binding_block(&block[1..], &mut config)?,
                    "@check" => parse_check_block(&block[1..], &mut config)?,
                    _ => {
                        return Err(ApeironError::InvalidConfig {
                            block: "System".into(),
                            detail: format!("unknown block: {}", block_head),
                        })
                    }
                }
            }
        }

        // Parse judgment declarations from @syntax blocks
        let mut system_judgments = HashMap::new();
        for item in &items[2..] {
            if let Some(block) = item.as_list() {
                if block.is_empty() {
                    continue;
                }
                if block[0].as_atom() == Some("@syntax") {
                    for decl_sexp in &block[1..] {
                        if let Some(decl) = decl_sexp.as_list() {
                            if !decl.is_empty() && decl[0].as_atom() == Some("judgment") {
                                if let Some(jd) = judgment::parse_judgment_decl(&decl[1..]) {
                                    system_judgments.insert(jd.name.clone(), jd);
                                }
                            }
                        }
                    }
                }
            }
        }

        let judgment_count = system_judgments.len();
        let msg = if judgment_count > 0 {
            format!("[SYSTEM] {} registered ({} sorts, {} ops, {} judgments, binding={:?}, check={:?})",
                name, config.sorts.len(), config.operators.len(), judgment_count, config.binding, config.check_modes)
        } else {
            format!("[SYSTEM] {} registered ({} sorts, {} ops, binding={:?}, check={:?})",
                name, config.sorts.len(), config.operators.len(), config.binding, config.check_modes)
        };
        self.output.push(msg);
        // Store judgments keyed by system name (will be copied to theories later)
        if !system_judgments.is_empty() {
            self.judgments.insert(name.clone(), system_judgments);
        }
        self.systems.insert(name, config);
        Ok(())
    }

    fn process_theory(&mut self, items: &[Sexp]) -> Result<()> {
        if items.len() < 2 {
            return Err(ApeironError::InvalidConfig {
                block: "Theory".into(),
                detail: "missing theory name".into(),
            });
        }

        let theory_name = items[1].as_atom().unwrap_or("?").to_string();

        // Parse optional :params and :in from the header
        let mut system_name = None;
        let body_start;
        let mut theory_params: Vec<(String, String)> = Vec::new();

        // Scan for :params and :in keywords in the header
        let mut i = 2;
        while i < items.len() {
            if items[i].is_atom(":params") {
                i += 1;
                if i < items.len() {
                    if let Some(param_list) = items[i].as_list() {
                        for p in param_list {
                            if let Some(pair) = p.as_list() {
                                if pair.len() == 2 {
                                    let pname = pair[0].as_atom().unwrap_or("?").to_string();
                                    let psort = pair[1].as_atom().unwrap_or("?").to_string();
                                    // Validate parameter name
                                    if pname.starts_with('?') || pname.starts_with('@') || pname.starts_with("__") {
                                        return Err(ApeironError::InvalidConfig {
                                            block: "Theory".into(),
                                            detail: format!("invalid parameter name '{}': must not start with ?, @, or __", pname),
                                        });
                                    }
                                    let reserved = ["ok", "fail", "import", "lam", "app"];
                                    if reserved.contains(&pname.as_str()) {
                                        return Err(ApeironError::InvalidConfig {
                                            block: "Theory".into(),
                                            detail: format!("invalid parameter name '{}': reserved word", pname),
                                        });
                                    }
                                    theory_params.push((pname, psort));
                                }
                            }
                        }
                    }
                }
                i += 1;
                continue;
            }
            if items[i].is_atom(":in") {
                i += 1;
                if i < items.len() {
                    system_name = items[i].as_atom().map(|s| s.to_string());
                }
                i += 1;
                continue;
            }
            // First non-keyword item = start of body
            break;
        }
        body_start = i;

        // Resolve system config
        let system = if let Some(ref sn) = system_name {
            self.systems
                .get(sn)
                .cloned()
                .ok_or_else(|| ApeironError::UnknownSystem { name: sn.clone() })?
        } else {
            // Default: use the first/only registered system
            self.systems
                .values()
                .next()
                .cloned()
                .ok_or_else(|| ApeironError::InvalidConfig {
                    block: "Theory".into(),
                    detail: "no system registered".into(),
                })?
        };

        // If parameterized, collect template and return early (no compilation)
        if !theory_params.is_empty() {
            let template = self.collect_template(
                &theory_name,
                &theory_params,
                &system.name,
                &items[body_start..],
            )?;
            self.templates.insert(theory_name.clone(), template);
            self.theory_systems
                .insert(theory_name.clone(), system.name.clone());
            self.output.push(format!(
                "[TEMPLATE] {} stored ({} params)",
                theory_name,
                theory_params.len()
            ));
            return Ok(());
        }

        // Collect known operator names from the system's @syntax
        let mut known_ops: HashSet<String> = system
            .operators
            .iter()
            .map(|op| op.name.clone())
            .collect();

        let mut theory_rules = Vec::new();
        let mut graph_rules: Vec<rewrite::GraphRule> = Vec::new();
        let is_reversible = system.check_modes.contains(&CheckMode::Reversible);
        let mut local_ops: Vec<String> = Vec::new();

        // Process theory body
        for item in &items[body_start..] {
            if let Some(decl) = item.as_list() {
                if decl.is_empty() {
                    continue;
                }
                let decl_head = decl[0].as_atom().unwrap_or("");
                match decl_head {
                    "const" => {
                        if decl.len() >= 2 {
                            let name = decl[1].as_atom().unwrap_or("?");
                            self.output.push(format!("[CONST] {}", name));
                        }
                    }
                    "def" | "Define" => {
                        // [def name body]
                        if decl.len() >= 3 {
                            let name = decl[1].as_atom().unwrap_or("?").to_string();
                            self.defs.insert(name.clone(), decl[2].clone());
                            self.output.push(format!("[DEF] {}", name));
                        }
                    }
                    "Scope" => {
                        // [Scope name]
                        if decl.len() >= 2 {
                            let name = decl[1].as_atom().unwrap_or("?").to_string();
                            let id = self.next_scope_id;
                            self.next_scope_id += 1;
                            self.scopes.insert(name.clone(), id);
                            self.output.push(format!("[SCOPE] {} (id={})", name, id));
                        }
                    }
                    "op" | "Op" => {
                        // [op name] — theory-local operator (extends known_ops)
                        if decl.len() >= 2 {
                            let name = decl[1].as_atom().unwrap_or("?").to_string();
                            known_ops.insert(name.clone());
                            self.extra_known_ops.insert(name.clone());
                            local_ops.push(name.clone());
                            self.output.push(format!("[OP] {}", name));
                        }
                    }
                    "@rule" => {
                        self.process_rule(&decl[1..], &mut theory_rules)?;
                        // Compile the just-added rule into a GraphRule
                        if let Some(rule) = theory_rules.last() {
                            // Store raw rule for aliased import
                            self.raw_theory_rules
                                .entry(theory_name.clone())
                                .or_default()
                                .push(rule.clone());
                            if let Some(gr) =
                                rewrite::compile_rule(&rule.name, &rule.lhs, &rule.rhs)
                            {
                                graph_rules.push(gr);
                            }
                            // Reversible mode: log auto-generated inverse
                            if is_reversible {
                                let inv_name = format!("{}-inv", rule.name);
                                self.output.push(format!(
                                    "[RULE-INV] {} (auto-generated inverse)",
                                    inv_name
                                ));
                            }
                        }
                    }
                    "@derive" => {
                        if let Some(dr) = judgment::parse_derive_rule(&decl[1..]) {
                            self.output.push(format!(
                                "[DERIVE] {} ({} premises)",
                                dr.name,
                                dr.premises.len()
                            ));
                            // Store in ordered list and by-name map
                            self.derive_rules_ordered
                                .entry(theory_name.clone())
                                .or_default()
                                .push(dr.clone());
                            self.derive_rules
                                .entry(theory_name.clone())
                                .or_default()
                                .insert(dr.name.clone(), dr);
                        }
                    }
                    "eval" | "eval-reverse" | "assert-eq" | "assert-neq" | "with-scope" => {
                        return Err(ApeironError::InvalidConfig {
                            block: "Theory".into(),
                            detail: format!(
                                "'{}' belongs in a [Proofs] block, not [Theory]. \
                                 Use: [Proofs Name :in {} ...]",
                                decl_head, theory_name
                            ),
                        });
                    }
                    "reflect" => {
                        // [reflect name expr] — show graph structure
                        if decl.len() >= 2 {
                            self.process_reflect(&decl[1..], &known_ops)?;
                        }
                    }
                    "Import" => {
                        self.process_import(
                            &decl[1..],
                            &graph_rules,
                            &known_ops,
                            &system,
                        )?;
                    }
                    "import" => {
                        // Theory-level import: [import Name args... :as Alias]
                        if decl.len() >= 2 {
                            let import_name = decl[1].as_atom().unwrap_or("?").to_string();

                            // Parse args and :as Alias from remaining items
                            let mut import_args: Vec<Sexp> = Vec::new();
                            let mut alias: Option<String> = None;
                            let mut j = 2;
                            while j < decl.len() {
                                if decl[j].is_atom(":as") {
                                    j += 1;
                                    if j < decl.len() {
                                        alias = decl[j].as_atom().map(|s| s.to_string());
                                    }
                                    j += 1;
                                } else {
                                    import_args.push(decl[j].clone());
                                    j += 1;
                                }
                            }

                            // Dispatch based on whether it's a template or existing theory
                            if self.templates.contains_key(&import_name) {
                                // Parameterized import
                                let a = alias.as_deref().ok_or_else(|| {
                                    ApeironError::InvalidConfig {
                                        block: "Theory".into(),
                                        detail: format!(
                                            "parameterized import of '{}' requires ':as Alias'",
                                            import_name
                                        ),
                                    }
                                })?;
                                self.process_parameterized_import(
                                    &import_name,
                                    &import_args,
                                    a,
                                    &theory_name,
                                    &mut known_ops,
                                    &mut theory_rules,
                                    &mut graph_rules,
                                )?;
                            } else if !import_args.is_empty() {
                                // Args provided but theory is not parameterized
                                return Err(ApeironError::InvalidConfig {
                                    block: "Theory".into(),
                                    detail: format!(
                                        "theory '{}' is not parameterized, but args were provided",
                                        import_name
                                    ),
                                });
                            } else {
                                // Simple import (with optional alias)
                                self.process_simple_import(
                                    &import_name,
                                    alias.as_deref(),
                                    &theory_name,
                                    &mut known_ops,
                                    &mut theory_rules,
                                    &mut graph_rules,
                                )?;
                            }
                        }
                    }
                    "Inductive" => {
                        if decl.len() >= 2 {
                            let name = decl[1].as_atom().unwrap_or("?");
                            self.output.push(format!("[INDUCTIVE] {}", name));
                        }
                    }
                    _ => {
                        return Err(ApeironError::InvalidConfig {
                            block: "Theory".into(),
                            detail: format!("unknown declaration: {}", decl_head),
                        })
                    }
                }
            }
        }

        // Compile @derive rules into rewrite rules
        if let Some(derive_rules) = self.derive_rules_ordered.get(&theory_name) {
            if !derive_rules.is_empty() {
                let theory_judgments = self
                    .judgments
                    .get(&system.name)
                    .cloned()
                    .unwrap_or_default();

                let (derived_rewrites, staging_ops) =
                    judgment::compile_derive_rules(derive_rules, &theory_judgments);

                // Register staging ops as known operators
                for op in &staging_ops {
                    known_ops.insert(op.clone());
                    self.extra_known_ops.insert(op.clone());
                }
                // Also register ok and fail as known ops
                known_ops.insert("ok".to_string());
                known_ops.insert("fail".to_string());
                self.extra_known_ops.insert("ok".to_string());
                self.extra_known_ops.insert("fail".to_string());

                for rule in &derived_rewrites {
                    self.output.push(format!(
                        "[RULE] {} : {} ==> {}",
                        rule.name, rule.lhs, rule.rhs
                    ));
                    if let Some(gr) =
                        rewrite::compile_rule(&rule.name, &rule.lhs, &rule.rhs)
                    {
                        graph_rules.push(gr);
                    }
                    theory_rules.push(rule.clone());
                }

                // Exhaustiveness warnings (only if sort→constructor mapping exists)
                // Currently operators don't carry sort annotations, so we skip
                // exhaustiveness to avoid false positives. When sort annotations
                // are added, this will use them for precise checking.

                self.output.push(format!(
                    "[DERIVE-COMPILE] {} rules compiled from {} @derive declarations",
                    derived_rewrites.len(),
                    derive_rules.len()
                ));
            }
        }

        // Store theory-level ops for aliased import
        self.theory_ops.insert(theory_name.clone(), local_ops);

        self.rules.insert(theory_name.clone(), theory_rules);
        self.compiled_rules
            .insert(theory_name.clone(), graph_rules);
        self.theory_systems
            .insert(theory_name.clone(), system.name.clone());
        self.output
            .push(format!("[THEORY] {} loaded", theory_name));
        Ok(())
    }

    /// Collect raw declarations from a parameterized theory body into a template.
    fn collect_template(
        &self,
        theory_name: &str,
        params: &[(String, String)],
        system_name: &str,
        body: &[Sexp],
    ) -> Result<TheoryTemplate> {
        let mut ops = Vec::new();
        let mut raw_rules = Vec::new();
        let mut raw_derives = Vec::new();

        for item in body {
            if let Some(decl) = item.as_list() {
                if decl.is_empty() {
                    continue;
                }
                let head = decl[0].as_atom().unwrap_or("");
                match head {
                    "op" | "Op" => {
                        if decl.len() >= 2 {
                            let name = decl[1].as_atom().unwrap_or("?").to_string();
                            ops.push(name);
                        }
                    }
                    "@rule" => {
                        // Parse raw rule: [@rule name [lhs] ==> rhs]
                        if let Some(rule) = Self::parse_rule_raw(&decl[1..]) {
                            raw_rules.push(rule);
                        }
                    }
                    "@derive" => {
                        if let Some(dr) = judgment::parse_derive_rule(&decl[1..]) {
                            raw_derives.push(dr);
                        }
                    }
                    "import" => {
                        return Err(ApeironError::InvalidConfig {
                            block: "Theory".into(),
                            detail: format!(
                                "parameterized theory '{}' cannot contain [import]",
                                theory_name
                            ),
                        });
                    }
                    _ => {} // skip other declarations in template
                }
            }
        }

        Ok(TheoryTemplate {
            params: params.to_vec(),
            system_name: system_name.to_string(),
            ops,
            raw_rules,
            raw_derives,
        })
    }

    /// Parse a raw @rule without side effects: returns (name, lhs, rhs).
    fn parse_rule_raw(items: &[Sexp]) -> Option<RewriteRule> {
        // items = [name [lhs] ==> rhs] or [name [lhs] ==> [rhs]]
        if items.len() < 4 {
            return None;
        }
        let name = items[0].as_atom()?.to_string();
        let lhs = items[1].clone();
        // items[2] should be "==>"
        let rhs = items[3].clone();
        Some(RewriteRule { name, lhs, rhs })
    }

    /// Process a simple (non-parameterized) import, optionally with alias.
    fn process_simple_import(
        &mut self,
        import_theory: &str,
        alias: Option<&str>,
        theory_name: &str,
        known_ops: &mut HashSet<String>,
        theory_rules: &mut Vec<RewriteRule>,
        graph_rules: &mut Vec<rewrite::GraphRule>,
    ) -> Result<()> {
        if alias.is_none() {
            // Existing behavior: copy everything verbatim
            if let Some(imported) = self.compiled_rules.get(import_theory).cloned() {
                let count = imported.len();
                for gr in imported {
                    graph_rules.push(gr);
                }
                self.output.push(format!(
                    "[IMPORT] {} graph rules from {}",
                    count, import_theory
                ));
            }

            if let Some(imported) = self.rules.get(import_theory).cloned() {
                for rule in imported {
                    theory_rules.push(rule);
                }
            }

            if let Some(imported) = self.derive_rules_ordered.get(import_theory).cloned() {
                for dr in &imported {
                    self.derive_rules_ordered
                        .entry(theory_name.to_string())
                        .or_default()
                        .push(dr.clone());
                    self.derive_rules
                        .entry(theory_name.to_string())
                        .or_default()
                        .insert(dr.name.clone(), dr.clone());
                }
            }

            for op in &self.extra_known_ops.clone() {
                known_ops.insert(op.clone());
            }

            return Ok(());
        }

        // Aliased import: build rename map from theory_ops
        let alias = alias.unwrap();
        let source_ops = self.theory_ops.get(import_theory).cloned().unwrap_or_default();

        let mut rename_map: HashMap<String, Sexp> = HashMap::new();
        let s = crate::parser::Span::default();
        for op_name in &source_ops {
            if op_name == "ok" || op_name == "fail" {
                continue;
            }
            let aliased = format!("{}.{}", alias, op_name);
            rename_map.insert(op_name.clone(), Sexp::Atom(aliased, s));
        }

        // Rename and re-compile @derive rules
        if let Some(imported_derives) = self.derive_rules_ordered.get(import_theory).cloned() {
            let renamed_derives: Vec<DerivRule> = imported_derives
                .iter()
                .map(|dr| {
                    let new_name = format!("{}.{}", alias, dr.name);
                    DerivRule {
                        name: new_name,
                        premises: dr.premises.iter().map(|p| judgment::subst_sexp(p, &rename_map)).collect(),
                        conclusion: judgment::subst_sexp(&dr.conclusion, &rename_map),
                        absurd: dr.absurd,
                    }
                })
                .collect();

            // Compile renamed derives
            let theory_judgments = {
                let sys_name = self.theory_systems.get(import_theory).cloned().unwrap_or_default();
                self.judgments.get(&sys_name).cloned().unwrap_or_default()
            };
            let (derived_rewrites, staging_ops) =
                judgment::compile_derive_rules(&renamed_derives, &theory_judgments);

            for op in &staging_ops {
                known_ops.insert(op.clone());
                self.extra_known_ops.insert(op.clone());
            }
            known_ops.insert("ok".to_string());
            known_ops.insert("fail".to_string());
            self.extra_known_ops.insert("ok".to_string());
            self.extra_known_ops.insert("fail".to_string());

            for rule in &derived_rewrites {
                self.output.push(format!(
                    "[RULE] {} : {} ==> {}",
                    rule.name, rule.lhs, rule.rhs
                ));
                if let Some(gr) = rewrite::compile_rule(&rule.name, &rule.lhs, &rule.rhs) {
                    graph_rules.push(gr);
                }
                theory_rules.push(rule.clone());
            }

            // Register derive rules
            for dr in &renamed_derives {
                self.derive_rules_ordered
                    .entry(theory_name.to_string())
                    .or_default()
                    .push(dr.clone());
                self.derive_rules
                    .entry(theory_name.to_string())
                    .or_default()
                    .insert(dr.name.clone(), dr.clone());
            }

            // Register aliased ops
            for op_name in &source_ops {
                if op_name == "ok" || op_name == "fail" {
                    continue;
                }
                let aliased = format!("{}.{}", alias, op_name);
                known_ops.insert(aliased.clone());
                self.extra_known_ops.insert(aliased);
            }

            self.output.push(format!(
                "[IMPORT] {} with alias {} ({} derive rules)",
                import_theory, alias, renamed_derives.len()
            ));
        }

        // Rename and import @rule declarations
        if let Some(imported_rules) = self.raw_theory_rules.get(import_theory).cloned() {
            for rule in &imported_rules {
                let new_name = format!("{}.{}", alias, rule.name);
                let new_lhs = judgment::subst_sexp(&rule.lhs, &rename_map);
                let new_rhs = judgment::subst_sexp(&rule.rhs, &rename_map);
                let renamed_rule = RewriteRule {
                    name: new_name,
                    lhs: new_lhs,
                    rhs: new_rhs,
                };
                if let Some(gr) = rewrite::compile_rule(&renamed_rule.name, &renamed_rule.lhs, &renamed_rule.rhs) {
                    graph_rules.push(gr);
                }
                theory_rules.push(renamed_rule);
            }
        }

        Ok(())
    }

    /// Process a parameterized import: instantiate template with args.
    fn process_parameterized_import(
        &mut self,
        template_name: &str,
        args: &[Sexp],
        alias: &str,
        theory_name: &str,
        known_ops: &mut HashSet<String>,
        theory_rules: &mut Vec<RewriteRule>,
        graph_rules: &mut Vec<rewrite::GraphRule>,
    ) -> Result<()> {
        let template = self.templates.get(template_name).cloned().ok_or_else(|| {
            ApeironError::InvalidConfig {
                block: "Theory".into(),
                detail: format!("unknown template '{}'", template_name),
            }
        })?;

        if args.len() != template.params.len() {
            return Err(ApeironError::InvalidConfig {
                block: "Theory".into(),
                detail: format!(
                    "parameterized import of '{}' expects {} args, got {}",
                    template_name,
                    template.params.len(),
                    args.len()
                ),
            });
        }

        let s = crate::parser::Span::default();

        // Build combined substitution map:
        // 1. Parameters → argument values (Sexp, supports compound args)
        let mut subst_map: HashMap<String, Sexp> = HashMap::new();
        for (i, (param_name, _)) in template.params.iter().enumerate() {
            subst_map.insert(param_name.clone(), args[i].clone());
        }

        // 2. Internal ops → Alias.op (skip ok/fail, skip params)
        for op_name in &template.ops {
            if op_name == "ok" || op_name == "fail" {
                continue;
            }
            if subst_map.contains_key(op_name) {
                continue; // parameter substitution wins
            }
            let aliased = format!("{}.{}", alias, op_name);
            subst_map.insert(op_name.clone(), Sexp::Atom(aliased, s));
        }

        // Apply substitution to raw @rule declarations
        for rule in &template.raw_rules {
            let new_name = format!("{}.{}", alias, rule.name);
            let new_lhs = judgment::subst_sexp(&rule.lhs, &subst_map);
            let new_rhs = judgment::subst_sexp(&rule.rhs, &subst_map);
            let renamed_rule = RewriteRule {
                name: new_name,
                lhs: new_lhs,
                rhs: new_rhs,
            };
            self.output.push(format!(
                "[RULE] {} : {} ==> {}",
                renamed_rule.name, renamed_rule.lhs, renamed_rule.rhs
            ));
            if let Some(gr) = rewrite::compile_rule(&renamed_rule.name, &renamed_rule.lhs, &renamed_rule.rhs) {
                graph_rules.push(gr);
            }
            theory_rules.push(renamed_rule);
        }

        // Apply substitution to @derive rules
        let renamed_derives: Vec<DerivRule> = template
            .raw_derives
            .iter()
            .map(|dr| {
                let new_name = format!("{}.{}", alias, dr.name);
                DerivRule {
                    name: new_name,
                    premises: dr.premises.iter().map(|p| judgment::subst_sexp(p, &subst_map)).collect(),
                    conclusion: judgment::subst_sexp(&dr.conclusion, &subst_map),
                    absurd: dr.absurd,
                }
            })
            .collect();

        // Compile renamed derives
        let theory_judgments = {
            let sys_name = &template.system_name;
            self.judgments.get(sys_name).cloned().unwrap_or_default()
        };
        let (derived_rewrites, staging_ops) =
            judgment::compile_derive_rules(&renamed_derives, &theory_judgments);

        for op in &staging_ops {
            known_ops.insert(op.clone());
            self.extra_known_ops.insert(op.clone());
        }
        known_ops.insert("ok".to_string());
        known_ops.insert("fail".to_string());
        self.extra_known_ops.insert("ok".to_string());
        self.extra_known_ops.insert("fail".to_string());

        for rule in &derived_rewrites {
            self.output.push(format!(
                "[RULE] {} : {} ==> {}",
                rule.name, rule.lhs, rule.rhs
            ));
            if let Some(gr) = rewrite::compile_rule(&rule.name, &rule.lhs, &rule.rhs) {
                graph_rules.push(gr);
            }
            theory_rules.push(rule.clone());
        }

        // Register derive rules
        for dr in &renamed_derives {
            self.derive_rules_ordered
                .entry(theory_name.to_string())
                .or_default()
                .push(dr.clone());
            self.derive_rules
                .entry(theory_name.to_string())
                .or_default()
                .insert(dr.name.clone(), dr.clone());
        }

        // Register aliased ops
        for op_name in &template.ops {
            if op_name == "ok" || op_name == "fail" {
                continue;
            }
            if template.params.iter().any(|(pn, _)| pn == op_name) {
                continue;
            }
            let aliased = format!("{}.{}", alias, op_name);
            known_ops.insert(aliased.clone());
            self.extra_known_ops.insert(aliased);
        }

        self.output.push(format!(
            "[IMPORT] {} instantiated as {} ({} args, {} derives)",
            template_name, alias, args.len(), renamed_derives.len()
        ));

        Ok(())
    }

    /// Process a [Proofs Name :in TheoryName ...] block.
    /// Sealed: inherits parent Theory's rules (read-only), rejects @rule.
    /// Allows: assert-eq, assert-neq, eval, eval-reverse, def, with-scope, reflect.
    fn process_proofs(&mut self, items: &[Sexp]) -> Result<()> {
        if items.len() < 2 {
            return Err(ApeironError::InvalidConfig {
                block: "Proofs".into(),
                detail: "missing proofs block name".into(),
            });
        }

        let proofs_name = items[1].as_atom().unwrap_or("?").to_string();

        // Require :in TheoryName
        if items.len() < 4 || !items[2].is_atom(":in") {
            return Err(ApeironError::InvalidConfig {
                block: "Proofs".into(),
                detail: "Proofs block requires ':in TheoryName'".into(),
            });
        }
        let theory_name = items[3]
            .as_atom()
            .ok_or_else(|| ApeironError::InvalidConfig {
                block: "Proofs".into(),
                detail: "theory name must be an atom".into(),
            })?
            .to_string();

        // Look up the parent Theory's compiled rules
        let mut graph_rules = self
            .compiled_rules
            .get(&theory_name)
            .cloned()
            .ok_or_else(|| ApeironError::InvalidConfig {
                block: "Proofs".into(),
                detail: format!("unknown theory: {}", theory_name),
            })?;

        // Look up the System config via theory→system mapping
        let system_name = self
            .theory_systems
            .get(&theory_name)
            .cloned()
            .ok_or_else(|| ApeironError::InvalidConfig {
                block: "Proofs".into(),
                detail: format!("theory '{}' has no registered system", theory_name),
            })?;
        let system = self
            .systems
            .get(&system_name)
            .cloned()
            .ok_or_else(|| ApeironError::UnknownSystem {
                name: system_name.clone(),
            })?;

        // Build known_ops (same as Theory would see)
        let mut known_ops: HashSet<String> = system
            .operators
            .iter()
            .map(|op| op.name.clone())
            .collect();
        known_ops.extend(self.extra_known_ops.iter().cloned());

        let is_reversible = system.check_modes.contains(&CheckMode::Reversible);
        let inverse_graph_rules: Vec<rewrite::GraphRule> = if is_reversible {
            graph_rules
                .iter()
                .filter_map(|gr| {
                    // Reconstruct inverse from the theory's stored rules
                    if let Some(theory_rules) = self.rules.get(&theory_name) {
                        for rule in theory_rules {
                            if rule.name == gr.name {
                                return rewrite::compile_rule(
                                    &format!("{}-inv", rule.name),
                                    &rule.rhs,
                                    &rule.lhs,
                                );
                            }
                        }
                    }
                    None
                })
                .collect()
        } else {
            Vec::new()
        };

        // Snapshot defs for local scope isolation (fork pattern)
        let defs_snapshot = self.defs.clone();

        let mut assertion_count = 0u32;

        // Process body (starts after :in TheoryName)
        for item in &items[4..] {
            if let Some(decl) = item.as_list() {
                if decl.is_empty() {
                    continue;
                }
                let decl_head = decl[0].as_atom().unwrap_or("");
                match decl_head {
                    "def" | "Define" => {
                        // Local definition (doesn't persist after Proofs block)
                        if decl.len() >= 3 {
                            let name = decl[1].as_atom().unwrap_or("?").to_string();
                            self.defs.insert(name.clone(), decl[2].clone());
                            self.output.push(format!("[DEF] {} (local)", name));
                        }
                    }
                    "assert-eq" => {
                        self.process_assert_eq(
                            &decl[1..],
                            &graph_rules,
                            &known_ops,
                            &system,
                        )?;
                        assertion_count += 1;
                    }
                    "assert-neq" => {
                        self.process_assert_neq(
                            &decl[1..],
                            &graph_rules,
                            &known_ops,
                            &system,
                        )?;
                        assertion_count += 1;
                    }
                    "eval" => {
                        self.process_eval(&decl[1..], &graph_rules, &known_ops, &system)?;
                    }
                    "eval-reverse" => {
                        self.process_eval(
                            &decl[1..],
                            &inverse_graph_rules,
                            &known_ops,
                            &system,
                        )?;
                    }
                    "with-scope" => {
                        if decl.len() >= 3 {
                            let scope_name = decl[1].as_atom().unwrap_or("?");
                            if let Some(&scope_id) = self.scopes.get(scope_name) {
                                self.arena.activate_scope(scope_id);
                                for inner in &decl[2..] {
                                    if let Some(inner_decl) = inner.as_list() {
                                        if inner_decl.is_empty() {
                                            continue;
                                        }
                                        let inner_head =
                                            inner_decl[0].as_atom().unwrap_or("");
                                        match inner_head {
                                            "eval" => {
                                                self.process_eval(
                                                    &inner_decl[1..],
                                                    &graph_rules,
                                                    &known_ops,
                                                    &system,
                                                )?;
                                            }
                                            "assert-eq" => {
                                                self.process_assert_eq(
                                                    &inner_decl[1..],
                                                    &graph_rules,
                                                    &known_ops,
                                                    &system,
                                                )?;
                                                assertion_count += 1;
                                            }
                                            "assert-neq" => {
                                                self.process_assert_neq(
                                                    &inner_decl[1..],
                                                    &graph_rules,
                                                    &known_ops,
                                                    &system,
                                                )?;
                                                assertion_count += 1;
                                            }
                                            _ => {
                                                return Err(ApeironError::InvalidConfig {
                                                    block: "Proofs/with-scope".into(),
                                                    detail: format!(
                                                        "unknown declaration: {}",
                                                        inner_head
                                                    ),
                                                })
                                            }
                                        }
                                    }
                                }
                                self.arena.deactivate_scope(scope_id);
                            }
                        }
                    }
                    "check" => {
                        self.process_check(
                            &decl[1..],
                            &graph_rules,
                            &known_ops,
                            &system,
                        )?;
                        assertion_count += 1;
                    }
                    "auto" => {
                        self.process_auto(
                            &decl[1..],
                            &graph_rules,
                            &known_ops,
                            &system,
                        )?;
                        assertion_count += 1;
                    }
                    "lemma" => {
                        self.process_lemma(
                            &decl[1..],
                            &mut graph_rules,
                            &mut known_ops,
                            &system,
                            &theory_name,
                        )?;
                        assertion_count += 1;
                    }
                    "derive" => {
                        self.process_derive_check(
                            &decl[1..],
                            &graph_rules,
                            &known_ops,
                            &system,
                            &theory_name,
                        )?;
                        assertion_count += 1;
                    }
                    "refute" => {
                        self.process_refute(
                            &decl[1..],
                            &graph_rules,
                            &known_ops,
                            &system,
                            &theory_name,
                        )?;
                        assertion_count += 1;
                    }
                    "reflect" => {
                        if decl.len() >= 2 {
                            self.process_reflect(&decl[1..], &known_ops)?;
                        }
                    }
                    "@rule" => {
                        return Err(ApeironError::InvalidConfig {
                            block: "Proofs".into(),
                            detail: format!(
                                "cannot add rules in Proofs block '{}' — \
                                 rules belong in Theory '{}'",
                                proofs_name, theory_name
                            ),
                        });
                    }
                    _ => {
                        return Err(ApeironError::InvalidConfig {
                            block: "Proofs".into(),
                            detail: format!(
                                "unknown declaration '{}' — Proofs blocks allow: \
                                 assert-eq, assert-neq, eval, check, auto, lemma, derive, refute, def",
                                decl_head
                            ),
                        });
                    }
                }
            }
        }

        // Restore defs snapshot (local defs don't leak)
        self.defs = defs_snapshot;

        // Persist any lemma-added rules back to compiled_rules
        self.compiled_rules.insert(theory_name.clone(), graph_rules);

        self.output.push(format!(
            "[PROOFS] {} verified ({} assertions)",
            proofs_name, assertion_count
        ));
        Ok(())
    }

    fn process_rule(&mut self, items: &[Sexp], rules: &mut Vec<RewriteRule>) -> Result<()> {
        // Find the ==> separator
        let mut name = String::new();
        let mut lhs_parts = Vec::new();
        let mut rhs_parts = Vec::new();
        let mut found_arrow = false;

        for item in items {
            if item.is_atom("==>") {
                found_arrow = true;
                continue;
            }
            if !found_arrow {
                // Before ==>: could be name or LHS
                if lhs_parts.is_empty() && item.as_atom().is_some() && item.as_list().is_none() {
                    // First atom before any list: could be the rule name
                    if name.is_empty() {
                        // Check if the next thing is ==> (then this is the LHS)
                        name = item.as_atom().unwrap_or("").to_string();
                    } else {
                        lhs_parts.push(item.clone());
                    }
                } else {
                    lhs_parts.push(item.clone());
                }
            } else {
                rhs_parts.push(item.clone());
            }
        }

        if !found_arrow || lhs_parts.is_empty() || rhs_parts.is_empty() {
            // Try alternate format: name [lhs] ==> rhs
            // If name was taken as the LHS, fix it
            if !name.is_empty() && !lhs_parts.is_empty() {
                // name is actually the rule name, lhs_parts[0] is the LHS
            } else if !name.is_empty() && lhs_parts.is_empty() && !rhs_parts.is_empty() {
                // name is actually the LHS atom
                lhs_parts.push(Sexp::Atom(
                    name.clone(),
                    crate::parser::Span::default(),
                ));
                name = String::new();
            }
        }

        if lhs_parts.is_empty() || rhs_parts.is_empty() {
            return Err(ApeironError::InvalidConfig {
                block: "@rule".into(),
                detail: "rule must have LHS ==> RHS".into(),
            });
        }

        let lhs = if lhs_parts.len() == 1 {
            lhs_parts.into_iter().next().unwrap()
        } else {
            Sexp::List(lhs_parts, crate::parser::Span::default())
        };

        let rhs = if rhs_parts.len() == 1 {
            rhs_parts.into_iter().next().unwrap()
        } else {
            Sexp::List(rhs_parts, crate::parser::Span::default())
        };

        if name.is_empty() {
            name = format!("rule_{}", rules.len());
        }

        self.output
            .push(format!("[RULE] {} : {} ==> {}", name, lhs, rhs));
        rules.push(RewriteRule { name, lhs, rhs });
        Ok(())
    }

    fn process_automorphism(&mut self, items: &[Sexp]) -> Result<()> {
        // [AutoMorphism Name SourceSystem TargetSystem
        //   [Map src tgt] ...
        //   [@strict true]
        //   [@strategy normalize-before-send]
        // ]
        if items.len() < 4 {
            return Err(ApeironError::InvalidConfig {
                block: "AutoMorphism".into(),
                detail: "need: [AutoMorphism Name SourceSystem TargetSystem]".into(),
            });
        }

        let name = items[1]
            .as_atom()
            .ok_or_else(|| ApeironError::InvalidConfig {
                block: "AutoMorphism".into(),
                detail: "morphism name must be an atom".into(),
            })?
            .to_string();

        let source_name = items[2]
            .as_atom()
            .ok_or_else(|| ApeironError::InvalidConfig {
                block: "AutoMorphism".into(),
                detail: "source system must be an atom".into(),
            })?
            .to_string();

        let target_name = items[3]
            .as_atom()
            .ok_or_else(|| ApeironError::InvalidConfig {
                block: "AutoMorphism".into(),
                detail: "target system must be an atom".into(),
            })?
            .to_string();

        // Parse optional blocks: [Map src tgt], [@strict bool], [@strategy ...]
        let mut explicit_ops = HashMap::new();
        let mut config = morphism::MorphismConfig::default();

        for item in &items[4..] {
            if let Some(block) = item.as_list() {
                if block.is_empty() {
                    continue;
                }
                let head = block[0].as_atom().unwrap_or("");
                match head {
                    "Map" => {
                        if block.len() >= 3 {
                            let src = block[1].as_atom().unwrap_or("?").to_string();
                            let tgt = block[2].as_atom().unwrap_or("?").to_string();
                            explicit_ops.insert(src, tgt);
                        }
                    }
                    "@strict" => {
                        if block.len() >= 2 {
                            let val = block[1].as_atom().unwrap_or("false");
                            config.strict = val == "true";
                        }
                    }
                    "@strategy" => {
                        if block.len() >= 2 {
                            let val = block[1].as_atom().unwrap_or("");
                            match val {
                                "normalize-before-send" => {
                                    config.normalize_before_send = Some(true);
                                }
                                "as-is" => {
                                    config.normalize_before_send = Some(false);
                                }
                                _ => {
                                    return Err(ApeironError::InvalidConfig {
                                        block: "AutoMorphism".into(),
                                        detail: format!(
                                            "unknown @strategy: '{}' (expected normalize-before-send or as-is)",
                                            val
                                        ),
                                    });
                                }
                            }
                        }
                    }
                    _ => {
                        return Err(ApeironError::InvalidConfig {
                            block: "AutoMorphism".into(),
                            detail: format!("unknown block: {}", head),
                        })
                    }
                }
            }
        }

        let source = self
            .systems
            .get(&source_name)
            .cloned()
            .ok_or_else(|| ApeironError::UnknownSystem {
                name: source_name.clone(),
            })?;
        let target = self
            .systems
            .get(&target_name)
            .cloned()
            .ok_or_else(|| ApeironError::UnknownSystem {
                name: target_name.clone(),
            })?;

        let morph = morphism::resolve_morphism(&name, &source, &target, explicit_ops, config)?;

        self.output.push(format!(
            "[MORPHISM] {}: {} -> {} (binding={:?}, checking={:?})",
            name, source_name, target_name, morph.binding_pass, morph.checking_pass
        ));

        self.morphisms.insert(name, morph);
        Ok(())
    }

    fn process_import(
        &mut self,
        items: &[Sexp],
        _graph_rules: &[rewrite::GraphRule],
        _known_ops: &HashSet<String>,
        _system: &SystemConfig,
    ) -> Result<()> {
        // [Import local-name [MorphismName source-expr :scope ScopeName]]
        if items.len() < 2 {
            return Err(ApeironError::InvalidConfig {
                block: "Import".into(),
                detail: "need: [Import name [MorphName expr]]".into(),
            });
        }

        let local_name = items[0]
            .as_atom()
            .ok_or_else(|| ApeironError::InvalidConfig {
                block: "Import".into(),
                detail: "import name must be an atom".into(),
            })?
            .to_string();

        let morph_app = items[1]
            .as_list()
            .ok_or_else(|| ApeironError::InvalidConfig {
                block: "Import".into(),
                detail: "second argument must be [MorphName expr ...]".into(),
            })?;

        if morph_app.is_empty() {
            return Err(ApeironError::InvalidConfig {
                block: "Import".into(),
                detail: "[MorphName expr] is empty".into(),
            });
        }

        let morph_name = morph_app[0]
            .as_atom()
            .ok_or_else(|| ApeironError::InvalidConfig {
                block: "Import".into(),
                detail: "morphism name must be an atom".into(),
            })?
            .to_string();

        if morph_app.len() < 2 {
            return Err(ApeironError::InvalidConfig {
                block: "Import".into(),
                detail: "need source expression after morphism name".into(),
            });
        }

        let source_expr = &morph_app[1];

        // Parse optional keyword arguments after source-expr.
        // :scope ScopeName — target scope for InjectScope binding pass.
        let mut target_scope_name: Option<String> = None;
        let mut i = 2;
        while i < morph_app.len() {
            if let Some(kw) = morph_app[i].as_atom() {
                if kw == ":scope" && i + 1 < morph_app.len() {
                    target_scope_name =
                        morph_app[i + 1].as_atom().map(|s| s.to_string());
                    i += 2;
                    continue;
                }
            }
            // Legacy: bare positional scope name (backwards compat)
            if target_scope_name.is_none() {
                target_scope_name = morph_app[i].as_atom().map(|s| s.to_string());
            }
            i += 1;
        }

        // Look up morphism
        let morph = self
            .morphisms
            .get(&morph_name)
            .cloned()
            .ok_or_else(|| ApeironError::UnknownMorphism {
                name: morph_name.clone(),
            })?;

        // Look up source system config
        let source_config = self
            .systems
            .get(&morph.source_system)
            .cloned()
            .ok_or_else(|| ApeironError::UnknownSystem {
                name: morph.source_system.clone(),
            })?;

        // Transport
        let result_sexp = morphism::transport(
            &mut self.arena,
            &morph,
            source_expr,
            &source_config,
            &self.defs,
            &self.scopes,
            &self.compiled_rules,
            &self.extra_known_ops,
            target_scope_name.as_deref(),
        )?;

        self.defs.insert(local_name.clone(), result_sexp);
        self.output
            .push(format!("[IMPORT] {} via {}", local_name, morph_name));
        Ok(())
    }

    /// Build a term, run physics + graph rewrite loop, return (root_ptr, total_interactions).
    fn build_and_normalize(
        &mut self,
        expr: &Sexp,
        graph_rules: &[rewrite::GraphRule],
        known_ops: &HashSet<String>,
        config: &SystemConfig,
    ) -> Result<(crate::node::Ptr, u64)> {
        // Expand defs
        let expanded = rewrite::expand_defs(expr, &self.defs);

        // Build with known ops (system + theory-level)
        let mut env = BuildEnv::new();
        env.known_ops = known_ops.clone();
        env.known_ops.extend(self.extra_known_ops.iter().cloned());
        env.scope_ids = self.scopes.clone();
        let root = builder::build_rooted(&mut self.arena, &mut env, &expanded);

        // Linear-explicit validation: reject Dup (multi-use) and Erase (unused)
        if config.binding == BindingMode::LinearExplicit {
            validate_linearity(&self.arena, root)?;
        }

        // Physics + rewrite loop
        let mut total = 0u64;
        loop {
            let result = physics::run(&mut self.arena, &PhysicsConfig::default());
            total += result.interactions;

            match result.halted_reason {
                physics::HaltReason::NormalForm => {}
                physics::HaltReason::FuelExhausted => {
                    return Err(ApeironError::FuelExhausted { interactions: total });
                }
                physics::HaltReason::Error(msg) => {
                    return Err(ApeironError::InvalidConfig {
                        block: "physics".into(),
                        detail: msg,
                    });
                }
            }

            if graph_rules.is_empty() {
                break;
            }
            if !rewrite::try_rewrite_scan(&mut self.arena, graph_rules) {
                break;
            }
        }

        Ok((root, total))
    }

    fn process_eval(
        &mut self,
        items: &[Sexp],
        graph_rules: &[rewrite::GraphRule],
        known_ops: &HashSet<String>,
        config: &SystemConfig,
    ) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        // [eval name expr] or [eval expr]
        let (name, expr) =
            if items.len() >= 2 && items[0].as_atom().is_some() && items[0].as_list().is_none() {
                (items[0].as_atom().unwrap_or("?").to_string(), &items[1])
            } else {
                (format!("eval_{}", self.output.len()), &items[0])
            };

        let (root, interactions) = self.build_and_normalize(expr, graph_rules, known_ops, config)?;

        let result_port = self.arena.port(root, 1);
        let term = if result_port.is_connected() {
            readback::readback(&self.arena, result_port.target)
        } else {
            readback::Term::Wire(root.0)
        };

        self.output.push(format!(
            "[EVAL] {} = {} ({} interactions)",
            name, term, interactions
        ));
        Ok(())
    }

    fn process_assert_eq(
        &mut self,
        items: &[Sexp],
        graph_rules: &[rewrite::GraphRule],
        known_ops: &HashSet<String>,
        config: &SystemConfig,
    ) -> Result<()> {
        if items.len() < 2 {
            return Err(ApeironError::InvalidConfig {
                block: "assert-eq".into(),
                detail: "need at least LHS and RHS".into(),
            });
        }

        // [assert-eq name lhs rhs] or [assert-eq lhs rhs]
        let (name, lhs_sexp, rhs_sexp) = if items.len() >= 3
            && items[0].as_atom().is_some()
            && items[0].as_list().is_none()
        {
            (
                items[0].as_atom().unwrap_or("?").to_string(),
                &items[1],
                &items[2],
            )
        } else {
            (
                format!("assert_{}", self.output.len()),
                &items[0],
                &items[1],
            )
        };

        let (lhs_root, _) = self.build_and_normalize(lhs_sexp, graph_rules, known_ops, config)?;
        let (rhs_root, _) = self.build_and_normalize(rhs_sexp, graph_rules, known_ops, config)?;

        let lhs_port = self.arena.port(lhs_root, 1);
        let rhs_port = self.arena.port(rhs_root, 1);

        let lhs_ptr = if lhs_port.is_connected() {
            lhs_port.target
        } else {
            lhs_root
        };
        let rhs_ptr = if rhs_port.is_connected() {
            rhs_port.target
        } else {
            rhs_root
        };

        let canonical = config.binding != BindingMode::Nominal;
        let lhs_hash = hash::topological_hash_mode(&self.arena, lhs_ptr, canonical);
        let rhs_hash = hash::topological_hash_mode(&self.arena, rhs_ptr, canonical);

        if lhs_hash == rhs_hash {
            self.output.push(format!("[ASSERT] {} passed", name));
            Ok(())
        } else {
            let lhs_term = readback::readback(&self.arena, lhs_ptr);
            let rhs_term = readback::readback(&self.arena, rhs_ptr);
            Err(ApeironError::AssertionFailed {
                name,
                detail: format!("{} != {}", lhs_term, rhs_term),
            })
        }
    }

    fn process_assert_neq(
        &mut self,
        items: &[Sexp],
        graph_rules: &[rewrite::GraphRule],
        known_ops: &HashSet<String>,
        config: &SystemConfig,
    ) -> Result<()> {
        if items.len() < 2 {
            return Err(ApeironError::InvalidConfig {
                block: "assert-neq".into(),
                detail: "need at least LHS and RHS".into(),
            });
        }

        // [assert-neq name lhs rhs] or [assert-neq lhs rhs]
        let (name, lhs_sexp, rhs_sexp) = if items.len() >= 3
            && items[0].as_atom().is_some()
            && items[0].as_list().is_none()
        {
            (
                items[0].as_atom().unwrap_or("?").to_string(),
                &items[1],
                &items[2],
            )
        } else {
            (
                format!("assert_{}", self.output.len()),
                &items[0],
                &items[1],
            )
        };

        let (lhs_root, _) = self.build_and_normalize(lhs_sexp, graph_rules, known_ops, config)?;
        let (rhs_root, _) = self.build_and_normalize(rhs_sexp, graph_rules, known_ops, config)?;

        let lhs_port = self.arena.port(lhs_root, 1);
        let rhs_port = self.arena.port(rhs_root, 1);

        let lhs_ptr = if lhs_port.is_connected() {
            lhs_port.target
        } else {
            lhs_root
        };
        let rhs_ptr = if rhs_port.is_connected() {
            rhs_port.target
        } else {
            rhs_root
        };

        let canonical = config.binding != BindingMode::Nominal;
        let lhs_hash = hash::topological_hash_mode(&self.arena, lhs_ptr, canonical);
        let rhs_hash = hash::topological_hash_mode(&self.arena, rhs_ptr, canonical);

        if lhs_hash != rhs_hash {
            self.output.push(format!("[ASSERT] {} passed (neq)", name));
            Ok(())
        } else {
            let lhs_term = readback::readback(&self.arena, lhs_ptr);
            let rhs_term = readback::readback(&self.arena, rhs_ptr);
            Err(ApeironError::AssertionFailed {
                name,
                detail: format!("expected != but {} == {}", lhs_term, rhs_term),
            })
        }
    }

    /// Process a `[check name [judgment args...] expected-output]` command.
    fn process_check(
        &mut self,
        items: &[Sexp],
        graph_rules: &[rewrite::GraphRule],
        known_ops: &HashSet<String>,
        config: &SystemConfig,
    ) -> Result<()> {
        if items.len() < 3 {
            return Err(ApeironError::InvalidConfig {
                block: "check".into(),
                detail: "need: [check name judgment-expr expected-output]".into(),
            });
        }

        let name = items[0]
            .as_atom()
            .unwrap_or("?")
            .to_string();
        let judgment_expr = &items[1];
        let expected_output = &items[2];

        // Wildcard `_` means "accept any [ok X]" (like auto)
        if expected_output.as_atom() == Some("_") {
            return self.process_auto(
                &[items[0].clone(), judgment_expr.clone()],
                graph_rules,
                known_ops,
                config,
            );
        }

        // Build and normalize the judgment expression
        let (lhs_root, _) =
            self.build_and_normalize(judgment_expr, graph_rules, known_ops, config)?;

        // Build [ok expected-output] and normalize
        let s = crate::parser::Span::default();
        let ok_expected = Sexp::List(
            vec![
                Sexp::Atom("ok".into(), s),
                expected_output.clone(),
            ],
            s,
        );
        let (rhs_root, _) =
            self.build_and_normalize(&ok_expected, graph_rules, known_ops, config)?;

        // Compare via hash
        let lhs_port = self.arena.port(lhs_root, 1);
        let rhs_port = self.arena.port(rhs_root, 1);

        let lhs_ptr = if lhs_port.is_connected() {
            lhs_port.target
        } else {
            lhs_root
        };
        let rhs_ptr = if rhs_port.is_connected() {
            rhs_port.target
        } else {
            rhs_root
        };

        let canonical = config.binding != BindingMode::Nominal;
        let lhs_hash = hash::topological_hash_mode(&self.arena, lhs_ptr, canonical);
        let rhs_hash = hash::topological_hash_mode(&self.arena, rhs_ptr, canonical);

        if lhs_hash == rhs_hash {
            self.output.push(format!("[CHECK] {} passed", name));
            Ok(())
        } else {
            let lhs_term = readback::readback(&self.arena, lhs_ptr);
            let rhs_term = readback::readback(&self.arena, rhs_ptr);
            Err(ApeironError::JudgmentMismatch {
                name,
                detail: format!(
                    "judgment yielded {} but expected {}",
                    lhs_term, rhs_term
                ),
            })
        }
    }

    /// Process an `[auto name judgment-expr]` command.
    /// Like `check` but without specifying the expected output — computes and reports it.
    fn process_auto(
        &mut self,
        items: &[Sexp],
        graph_rules: &[rewrite::GraphRule],
        known_ops: &HashSet<String>,
        config: &SystemConfig,
    ) -> Result<()> {
        if items.len() < 2 {
            return Err(ApeironError::InvalidConfig {
                block: "auto".into(),
                detail: "need: [auto name judgment-expr]".into(),
            });
        }

        let proof_name = items[0]
            .as_atom()
            .unwrap_or("?")
            .to_string();
        let judgment_expr = &items[1];

        // Build and normalize the judgment expression
        let (lhs_root, _) =
            self.build_and_normalize(judgment_expr, graph_rules, known_ops, config)?;

        // Read back the result
        let lhs_port = self.arena.port(lhs_root, 1);
        let lhs_ptr = if lhs_port.is_connected() {
            lhs_port.target
        } else {
            lhs_root
        };

        let result_term = readback::readback(&self.arena, lhs_ptr);

        // Check if result is [ok X]
        match &result_term {
            readback::Term::App(func, args) if args.len() == 1 => {
                if let readback::Term::Const(ref head) = **func {
                    if head == "ok" {
                        self.output.push(format!(
                            "[AUTO] {} computed {}",
                            proof_name, args[0]
                        ));
                        return Ok(());
                    }
                }
                Err(ApeironError::JudgmentMismatch {
                    name: proof_name,
                    detail: format!("judgment got stuck: {}", result_term),
                })
            }
            readback::Term::Const(ref head) if head == "fail" => {
                Err(ApeironError::JudgmentMismatch {
                    name: proof_name,
                    detail: "judgment reduced to fail (no matching rule)".into(),
                })
            }
            _ => Err(ApeironError::JudgmentMismatch {
                name: proof_name,
                detail: format!("judgment got stuck: {}", result_term),
            }),
        }
    }

    /// Process a `[lemma name judgment-expr expected-output]` command.
    /// Like `check` but also injects the proved conclusion as a new 0-premise @derive rule.
    fn process_lemma(
        &mut self,
        items: &[Sexp],
        graph_rules: &mut Vec<rewrite::GraphRule>,
        known_ops: &mut HashSet<String>,
        config: &SystemConfig,
        theory_name: &str,
    ) -> Result<()> {
        if items.len() < 3 {
            return Err(ApeironError::InvalidConfig {
                block: "lemma".into(),
                detail: "need: [lemma name judgment-expr expected-output]".into(),
            });
        }

        let proof_name = items[0]
            .as_atom()
            .unwrap_or("?")
            .to_string();
        let judgment_expr = &items[1];
        let expected_output = &items[2];

        // 1. Verify (like check)
        let (lhs_root, _) =
            self.build_and_normalize(judgment_expr, graph_rules, known_ops, config)?;

        let s = crate::parser::Span::default();
        let ok_expected = Sexp::List(
            vec![
                Sexp::Atom("ok".into(), s),
                expected_output.clone(),
            ],
            s,
        );
        let (rhs_root, _) =
            self.build_and_normalize(&ok_expected, graph_rules, known_ops, config)?;

        let lhs_port = self.arena.port(lhs_root, 1);
        let rhs_port = self.arena.port(rhs_root, 1);
        let lhs_ptr = if lhs_port.is_connected() {
            lhs_port.target
        } else {
            lhs_root
        };
        let rhs_ptr = if rhs_port.is_connected() {
            rhs_port.target
        } else {
            rhs_root
        };

        let canonical = config.binding != BindingMode::Nominal;
        let lhs_hash = hash::topological_hash_mode(&self.arena, lhs_ptr, canonical);
        let rhs_hash = hash::topological_hash_mode(&self.arena, rhs_ptr, canonical);

        if lhs_hash != rhs_hash {
            let lhs_term = readback::readback(&self.arena, lhs_ptr);
            let rhs_term = readback::readback(&self.arena, rhs_ptr);
            return Err(ApeironError::JudgmentMismatch {
                name: proof_name,
                detail: format!(
                    "lemma: judgment yielded {} but expected {}",
                    lhs_term, rhs_term
                ),
            });
        }

        // 2. Create a new 0-premise @derive rule
        // Reconstruct the full conclusion: [J inputs... output]
        let conclusion = if let Some(j_items) = judgment_expr.as_list() {
            let mut concl_items = j_items.to_vec();
            concl_items.push(expected_output.clone());
            Sexp::List(concl_items, s)
        } else {
            Sexp::List(
                vec![judgment_expr.clone(), expected_output.clone()],
                s,
            )
        };

        let derive_rule = DerivRule {
            name: format!("__lemma_{}", proof_name),
            premises: vec![],
            conclusion,
            absurd: false,
        };

        // 3. Compile to rewrite rules
        let system_name = self
            .theory_systems
            .get(theory_name)
            .cloned()
            .unwrap_or_default();
        let theory_judgments = self
            .judgments
            .get(&system_name)
            .cloned()
            .unwrap_or_default();

        let (derived_rewrites, staging_ops) =
            judgment::compile_derive_rules(&[derive_rule.clone()], &theory_judgments);

        for op in &staging_ops {
            known_ops.insert(op.clone());
            self.extra_known_ops.insert(op.clone());
        }

        for rule in &derived_rewrites {
            if let Some(gr) = rewrite::compile_rule(&rule.name, &rule.lhs, &rule.rhs) {
                graph_rules.push(gr);
            }
        }

        // 4. Register the derive rule
        self.derive_rules_ordered
            .entry(theory_name.to_string())
            .or_default()
            .push(derive_rule.clone());
        self.derive_rules
            .entry(theory_name.to_string())
            .or_default()
            .insert(derive_rule.name.clone(), derive_rule);

        self.output.push(format!(
            "[LEMMA] {} verified and added as derived rule",
            proof_name
        ));
        Ok(())
    }

    /// Process a `[derive name :by rule :sub [...] :shows [conclusion]]` command.
    fn process_derive_check(
        &mut self,
        items: &[Sexp],
        graph_rules: &[rewrite::GraphRule],
        known_ops: &HashSet<String>,
        config: &SystemConfig,
        theory_name: &str,
    ) -> Result<()> {
        if items.is_empty() {
            return Err(ApeironError::InvalidConfig {
                block: "derive".into(),
                detail: "need: [derive name :by rule-name ...]".into(),
            });
        }

        let name = items[0]
            .as_atom()
            .unwrap_or("?")
            .to_string();

        // Parse :by, :sub, :shows
        let mut by_rule = String::new();
        let mut subs: Vec<Sexp> = Vec::new();
        let mut shows: Option<Sexp> = None;
        let mut i = 1;

        while i < items.len() {
            match items[i].as_atom() {
                Some(":by") => {
                    i += 1;
                    if i < items.len() {
                        by_rule = items[i].as_atom().unwrap_or("?").to_string();
                    }
                }
                Some(":sub") => {
                    i += 1;
                    if i < items.len() {
                        if let Some(list) = items[i].as_list() {
                            subs = list.to_vec();
                        }
                    }
                }
                Some(":shows") => {
                    i += 1;
                    if i < items.len() {
                        shows = Some(items[i].clone());
                    }
                }
                _ => {}
            }
            i += 1;
        }

        // Get the derive rules for this theory
        let derive_rules = self
            .derive_rules
            .get(theory_name)
            .cloned()
            .unwrap_or_default();

        if derive_rules.is_empty() {
            return Err(ApeironError::InvalidConfig {
                block: "derive".into(),
                detail: format!("theory '{}' has no @derive rules", theory_name),
            });
        }

        // Look up the named rule
        let rule = derive_rules.get(&by_rule).ok_or_else(|| {
            ApeironError::DerivationFailed {
                name: name.clone(),
                detail: format!("unknown derive rule '{}'", by_rule),
            }
        })?;

        // If :shows provided, match against conclusion and check premises
        if let Some(ref shown) = shows {
            // First: verify the conclusion normalizes correctly via rewriting
            // Build the judgment call from the conclusion
            let (concl_root, _) =
                self.build_and_normalize(shown, graph_rules, known_ops, config)?;
            let concl_port = self.arena.port(concl_root, 1);
            let _concl_ptr = if concl_port.is_connected() {
                concl_port.target
            } else {
                concl_root
            };

            // Build the derivation tree sexp for checking
            let s = crate::parser::Span::default();
            let mut tree_items = vec![Sexp::Atom(by_rule.clone(), s)];
            if !subs.is_empty() {
                tree_items.push(Sexp::Atom(":sub".into(), s));
                tree_items.push(Sexp::List(subs, s));
            }
            tree_items.push(Sexp::Atom(":shows".into(), s));
            tree_items.push(shown.clone());
            let tree = Sexp::List(tree_items, s);

            // Find the judgment name from the conclusion
            let judgment_name = if let Some(items_list) = rule.conclusion.as_list() {
                items_list[0].as_atom().unwrap_or("?").to_string()
            } else {
                "?".to_string()
            };

            match judgment::check_derivation(&tree, &derive_rules, &judgment_name) {
                Ok(()) => {
                    self.output
                        .push(format!("[DERIVE] {} verified", name));
                    Ok(())
                }
                Err(e) => Err(ApeironError::DerivationFailed {
                    name,
                    detail: e,
                }),
            }
        } else {
            // No :shows — just verify the rule exists and premises count matches
            self.output.push(format!(
                "[DERIVE] {} acknowledged (rule {} with {} premises)",
                name,
                by_rule,
                rule.premises.len()
            ));
            Ok(())
        }
    }

    /// Process a `[refute name :assumptions [...] :goal [...] :depth N]` command.
    fn process_refute(
        &mut self,
        items: &[Sexp],
        _graph_rules: &[rewrite::GraphRule],
        _known_ops: &HashSet<String>,
        _config: &SystemConfig,
        theory_name: &str,
    ) -> Result<()> {
        if items.is_empty() {
            return Err(ApeironError::InvalidConfig {
                block: "refute".into(),
                detail: "need: [refute name :assumptions [...] :goal [...] :depth N]".into(),
            });
        }

        let name = items[0]
            .as_atom()
            .unwrap_or("?")
            .to_string();

        let mut assumptions: Vec<Sexp> = Vec::new();
        let mut goal: Option<Sexp> = None;
        let mut max_depth: usize = 5;
        let mut i = 1;

        while i < items.len() {
            match items[i].as_atom() {
                Some(":assumptions") => {
                    i += 1;
                    if i < items.len() {
                        if let Some(list) = items[i].as_list() {
                            assumptions = list.to_vec();
                        }
                    }
                }
                Some(":goal") => {
                    i += 1;
                    if i < items.len() {
                        goal = Some(items[i].clone());
                    }
                }
                Some(":depth") => {
                    i += 1;
                    if i < items.len() {
                        if let Some(d) = items[i].as_atom() {
                            max_depth = d.parse().unwrap_or(5);
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }

        let goal = goal.ok_or_else(|| ApeironError::InvalidConfig {
            block: "refute".into(),
            detail: "missing :goal".into(),
        })?;

        // Get derive rules for this theory
        let derive_rules = self
            .derive_rules_ordered
            .get(theory_name)
            .cloned()
            .unwrap_or_default();

        // Check if theory uses affine/linear binding
        let system_name = self.theory_systems.get(theory_name).cloned().unwrap_or_default();
        let affine = self
            .systems
            .get(&system_name)
            .map(|s| s.binding == BindingMode::LinearExplicit)
            .unwrap_or(false);

        let max_budget = 1_000_000;
        let result = refute::exhaustive_refute(
            &derive_rules,
            &assumptions,
            &goal,
            max_depth,
            max_budget,
            affine,
        );

        match result {
            refute::RefuteResult::Refuted { depth } => {
                self.output.push(format!(
                    "[REFUTE] {}: VERIFIED (impossible at depth {})",
                    name, depth
                ));
                Ok(())
            }
            refute::RefuteResult::Derivable => {
                Err(ApeironError::RefutationFailed {
                    name,
                    detail: "goal is derivable (proof found)".into(),
                })
            }
            refute::RefuteResult::Inconclusive { steps_used } => {
                Err(ApeironError::RefutationInconclusive {
                    name,
                    detail: format!("budget exhausted after {} steps", steps_used),
                })
            }
        }
    }

    fn process_reflect(
        &mut self,
        items: &[Sexp],
        known_ops: &HashSet<String>,
    ) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let (name, expr) =
            if items.len() >= 2 && items[0].as_atom().is_some() && items[0].as_list().is_none() {
                (items[0].as_atom().unwrap_or("?").to_string(), &items[1])
            } else {
                (format!("reflect_{}", self.output.len()), &items[0])
            };

        // Build without reducing — we want to inspect the graph structure
        let expanded = rewrite::expand_defs(expr, &self.defs);
        let mut env = BuildEnv::new();
        env.known_ops = known_ops.clone();
        env.scope_ids = self.scopes.clone();
        let root = builder::build_rooted(&mut self.arena, &mut env, &expanded);

        // Walk the graph and describe its topology
        let result_port = self.arena.port(root, 1);
        if !result_port.is_connected() {
            self.output
                .push(format!("[REFLECT] {} = <empty>", name));
            return Ok(());
        }

        let start = result_port.target;
        let mut nodes_desc = Vec::new();
        let mut wires_desc = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = vec![start];

        while let Some(ptr) = queue.pop() {
            if ptr.is_none() || visited.contains(&ptr) {
                continue;
            }
            visited.insert(ptr);

            if let Some(node) = self.arena.get(ptr) {
                let kind_str = match &node.kind {
                    crate::node::OpCode::Lam => "Lam".to_string(),
                    crate::node::OpCode::App => "App".to_string(),
                    crate::node::OpCode::Erase => "Erase".to_string(),
                    crate::node::OpCode::Dup { label } => format!("Dup#{}", label),
                    crate::node::OpCode::Barrier { scope } => format!("Barrier#{}", scope),
                    crate::node::OpCode::Future => "Future".to_string(),
                    crate::node::OpCode::Sym { name, arity } => {
                        if *arity > 0 {
                            format!("{}(arity={})", name, arity)
                        } else {
                            name.clone()
                        }
                    }
                };
                nodes_desc.push(format!("{}:{}", ptr.0, kind_str));

                for (slot, port) in node.ports.iter().enumerate() {
                    if port.is_connected() && !visited.contains(&port.target) {
                        queue.push(port.target);
                    }
                    if port.is_connected() {
                        wires_desc.push(format!(
                            "{}:{} <-> {}:{}",
                            ptr.0, slot, port.target.0, port.slot
                        ));
                    }
                }
            }
        }

        self.output.push(format!(
            "[REFLECT] {} = [Graph  Nodes: [{}]  Wires: [{}]]",
            name,
            nodes_desc.join(", "),
            wires_desc.join(", "),
        ));
        Ok(())
    }
}

/// Validate linear variable usage: reject Dup (multi-use) and Erase (unused) nodes.
/// Walks the graph from `root` and reports violations.
fn validate_linearity(arena: &Arena, root: crate::node::Ptr) -> Result<()> {
    use crate::node::OpCode;
    let mut visited = HashSet::new();
    let mut queue = vec![root];

    while let Some(ptr) = queue.pop() {
        if ptr.is_none() || visited.contains(&ptr) {
            continue;
        }
        visited.insert(ptr);

        if let Some(node) = arena.get(ptr) {
            match &node.kind {
                OpCode::Dup { .. } => {
                    return Err(ApeironError::LinearityViolation {
                        detail: format!(
                            "variable duplicated (non-linear use) at node {}",
                            ptr.0
                        ),
                    });
                }
                OpCode::Erase => {
                    return Err(ApeironError::LinearityViolation {
                        detail: format!(
                            "variable erased (unused) at node {}",
                            ptr.0
                        ),
                    });
                }
                _ => {}
            }

            for port in &node.ports {
                if port.is_connected() && !visited.contains(&port.target) {
                    queue.push(port.target);
                }
            }
        }
    }

    Ok(())
}

fn parse_syntax_block(items: &[Sexp], config: &mut SystemConfig) -> Result<()> {
    for item in items {
        if let Some(decl) = item.as_list() {
            if decl.is_empty() {
                continue;
            }
            let kind = decl[0].as_atom().unwrap_or("");
            match kind {
                "sort" | "Sort" => {
                    if decl.len() >= 2 {
                        let name = decl[1].as_atom().unwrap_or("?").to_string();
                        config.sorts.push(SortDecl { name });
                    }
                }
                "op" | "Op" => {
                    if decl.len() >= 2 {
                        let name = decl[1].as_atom().unwrap_or("?").to_string();
                        config.operators.push(OpDecl {
                            name,
                            args: Vec::new(),
                            result: String::new(),
                        });
                    }
                }
                "judgment" => {
                    // Parsed later in process_system; just register the op name
                    if decl.len() >= 2 {
                        let name = decl[1].as_atom().unwrap_or("?").to_string();
                        config.operators.push(OpDecl {
                            name,
                            args: Vec::new(),
                            result: String::new(),
                        });
                    }
                }
                _ => {
                    return Err(ApeironError::InvalidConfig {
                        block: "@syntax".into(),
                        detail: format!("unknown syntax declaration: {}", kind),
                    })
                }
            }
        }
    }
    Ok(())
}

fn parse_binding_block(items: &[Sexp], config: &mut SystemConfig) -> Result<()> {
    for item in items {
        if let Some(mode) = item.as_atom() {
            config.binding = match mode {
                "implicit" => BindingMode::Implicit,
                "exposed" => BindingMode::Exposed,
                "contextual" => BindingMode::Contextual,
                "linear-explicit" | "linear" => BindingMode::LinearExplicit,
                "nominal" => BindingMode::Nominal,
                _ => {
                    return Err(ApeironError::InvalidConfig {
                        block: "@binding".into(),
                        detail: format!("unknown binding mode: {}", mode),
                    })
                }
            };
        }
    }
    Ok(())
}

fn parse_check_block(items: &[Sexp], config: &mut SystemConfig) -> Result<()> {
    for item in items {
        let mode_name = if let Some(name) = item.as_atom() {
            name.to_string()
        } else if let Some(list) = item.as_list() {
            // [Mode name] format
            if list.len() >= 2 && list[0].is_atom("Mode") {
                list[1].as_atom().unwrap_or("").to_string()
            } else {
                continue;
            }
        } else {
            continue;
        };

        let mode = match mode_name.as_str() {
            "rewriting" => CheckMode::Rewriting,
            "unification" => CheckMode::Unification,
            "beta-reduction" | "compute" => CheckMode::BetaReduction,
            "oracle" => CheckMode::Oracle,
            "extensional" => CheckMode::Extensional,
            "pattern-unification" => CheckMode::PatternUnification,
            "reversible" => CheckMode::Reversible,
            "confluent-race" | "race" => CheckMode::ConfluentRace,
            _ => {
                return Err(ApeironError::InvalidConfig {
                    block: "@check".into(),
                    detail: format!("unknown check mode: {}", mode_name),
                })
            }
        };
        config.check_modes.insert(mode);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    #[test]
    fn parse_system_config() {
        let source = r#"
        [System WeakLF
          [@syntax
            [sort Term]
            [sort Type]
            [op lam]
            [op app]
          ]
          [@binding implicit]
          [@check rewriting beta-reduction]
        ]
        "#;

        let sexps = parser::parse(source).unwrap();
        let mut session = Session::new();
        session.process(&sexps[0]).unwrap();

        let sys = session.systems.get("WeakLF").unwrap();
        assert_eq!(sys.sorts.len(), 2);
        assert_eq!(sys.operators.len(), 2);
        assert_eq!(sys.binding, BindingMode::Implicit);
        assert!(sys.check_modes.contains(&CheckMode::Rewriting));
        assert!(sys.check_modes.contains(&CheckMode::BetaReduction));
    }

    #[test]
    fn process_theory_with_eval_in_proofs() {
        let source = r#"
        [System Test
          [@syntax [sort Term]]
          [@binding implicit]
          [@check beta-reduction]
        ]
        [Theory Demo :in Test]
        [Proofs DemoCheck :in Demo
          [eval test-id [app [lam x x] y]]
        ]
        "#;

        let sexps = parser::parse(source).unwrap();
        let mut session = Session::new();
        for sexp in &sexps {
            session.process(sexp).unwrap();
        }

        // Check that eval produced output
        assert!(session.output.iter().any(|s| s.starts_with("[EVAL]")));
    }

    #[test]
    fn process_assert_eq_in_proofs() {
        let source = r#"
        [System Test
          [@syntax [sort Term]]
          [@binding implicit]
          [@check beta-reduction]
        ]
        [Theory Demo :in Test]
        [Proofs DemoCheck :in Demo
          [assert-eq id-test [app [lam x x] y] y]
        ]
        "#;

        let sexps = parser::parse(source).unwrap();
        let mut session = Session::new();
        for sexp in &sexps {
            session.process(sexp).unwrap();
        }

        assert!(session
            .output
            .iter()
            .any(|s| s.contains("id-test") && s.contains("passed")));
    }

    #[test]
    fn theory_rejects_assertions() {
        let source = r#"
        [System Test
          [@syntax [sort Term]]
          [@binding implicit]
          [@check beta-reduction]
        ]
        [Theory Demo :in Test
          [assert-eq bad [app [lam x x] y] y]
        ]
        "#;

        let sexps = parser::parse(source).unwrap();
        let mut session = Session::new();
        session.process(&sexps[0]).unwrap();
        let result = session.process(&sexps[1]);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("Proofs"), "expected Proofs redirect, got: {}", err);
    }

    #[test]
    fn theory_rejects_eval() {
        let source = r#"
        [System Test
          [@syntax [sort Term]]
          [@binding implicit]
          [@check beta-reduction]
        ]
        [Theory Demo :in Test
          [eval test [app [lam x x] y]]
        ]
        "#;

        let sexps = parser::parse(source).unwrap();
        let mut session = Session::new();
        session.process(&sexps[0]).unwrap();
        let result = session.process(&sexps[1]);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("Proofs"), "expected Proofs redirect, got: {}", err);
    }

    #[test]
    fn proofs_block_basic() {
        let source = r#"
        [System S
          [@syntax [sort Nat] [op z] [op s] [op add]]
          [@binding implicit]
          [@check rewriting]
        ]
        [Theory T :in S
          [@rule add-z [add z ?n] ==> ?n]
          [@rule add-s [add [s ?n] ?m] ==> [s [add ?n ?m]]]
        ]
        [Proofs P :in T
          [assert-eq one-plus-one [add [s z] [s z]] [s [s z]]]
        ]
        "#;

        let sexps = parser::parse(source).unwrap();
        let mut session = Session::new();
        for sexp in &sexps {
            session.process(sexp).unwrap();
        }

        assert!(session.output.iter().any(|s| s.contains("one-plus-one") && s.contains("passed")));
        assert!(session.output.iter().any(|s| s.contains("[PROOFS] P verified (1 assertions)")));
    }

    #[test]
    fn proofs_block_rejects_rule() {
        let source = r#"
        [System S
          [@syntax [sort Nat] [op z] [op s]]
          [@binding implicit]
          [@check rewriting]
        ]
        [Theory T :in S]
        [Proofs P :in T
          [@rule sneaky z ==> [s z]]
        ]
        "#;

        let sexps = parser::parse(source).unwrap();
        let mut session = Session::new();
        session.process(&sexps[0]).unwrap();
        session.process(&sexps[1]).unwrap();
        let result = session.process(&sexps[2]);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("cannot add rules"), "expected rule rejection, got: {}", err);
    }

    #[test]
    fn proofs_block_local_defs_dont_leak() {
        let source = r#"
        [System S
          [@syntax [sort Nat] [op z] [op s] [op add]]
          [@binding implicit]
          [@check rewriting]
        ]
        [Theory T :in S
          [@rule add-z [add z ?n] ==> ?n]
          [def two [s [s z]]]
        ]
        [Proofs P1 :in T
          [def local-three [s [s [s z]]]]
          [assert-eq test [add z local-three] local-three]
        ]
        [Proofs P2 :in T
          [assert-eq test2 [add z two] two]
        ]
        "#;

        let sexps = parser::parse(source).unwrap();
        let mut session = Session::new();
        for sexp in &sexps {
            session.process(sexp).unwrap();
        }

        // Both proof blocks pass
        assert!(session.output.iter().any(|s| s.contains("P1 verified")));
        assert!(session.output.iter().any(|s| s.contains("P2 verified")));
        // Theory def 'two' persists, but proof-local 'local-three' should not
        assert!(session.defs.contains_key("two"));
        assert!(!session.defs.contains_key("local-three"));
    }
}
