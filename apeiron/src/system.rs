use std::collections::{HashMap, HashSet};

use crate::arena::Arena;
use crate::builder::{self, BuildEnv};
use crate::error::{ApeironError, Result};
use crate::hash;
use crate::morphism::{self, AutoMorphism};
use crate::parser::Sexp;
use crate::physics::{self, PhysicsConfig};
use crate::readback;
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

        let msg = format!("[SYSTEM] {} registered ({} sorts, {} ops, binding={:?}, check={:?})",
            name, config.sorts.len(), config.operators.len(), config.binding, config.check_modes);
        self.output.push(msg);
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

        // Find system reference: either `:in SystemName` or look up by convention
        let mut system_name = None;
        let mut body_start = 2;

        if items.len() > 3 && items[2].is_atom(":in") {
            system_name = items[3].as_atom().map(|s| s.to_string());
            body_start = 4;
        }

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

        // Collect known operator names from the system's @syntax
        let mut known_ops: HashSet<String> = system
            .operators
            .iter()
            .map(|op| op.name.clone())
            .collect();

        let mut theory_rules = Vec::new();
        let mut graph_rules: Vec<rewrite::GraphRule> = Vec::new();
        let mut inverse_graph_rules: Vec<rewrite::GraphRule> = Vec::new();
        let is_reversible = system.check_modes.contains(&CheckMode::Reversible);

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
                            self.output.push(format!("[OP] {}", name));
                        }
                    }
                    "@rule" => {
                        self.process_rule(&decl[1..], &mut theory_rules)?;
                        // Compile the just-added rule into a GraphRule
                        if let Some(rule) = theory_rules.last() {
                            if let Some(gr) =
                                rewrite::compile_rule(&rule.name, &rule.lhs, &rule.rhs)
                            {
                                graph_rules.push(gr);
                            }
                            // Reversible mode: auto-generate inverse rule
                            if is_reversible {
                                let inv_name = format!("{}-inv", rule.name);
                                if let Some(inv_gr) =
                                    rewrite::compile_rule(&inv_name, &rule.rhs, &rule.lhs)
                                {
                                    inverse_graph_rules.push(inv_gr);
                                    self.output.push(format!(
                                        "[RULE-INV] {} (auto-generated inverse)",
                                        inv_name
                                    ));
                                }
                            }
                        }
                    }
                    "eval" => {
                        self.process_eval(&decl[1..], &graph_rules, &known_ops, &system)?;
                    }
                    "eval-reverse" => {
                        // Run with inverse rules (backward execution)
                        self.process_eval(
                            &decl[1..],
                            &inverse_graph_rules,
                            &known_ops,
                            &system,
                        )?;
                    }
                    "assert-eq" => {
                        self.process_assert_eq(
                            &decl[1..],
                            &graph_rules,
                            &known_ops,
                            &system,
                        )?;
                    }
                    "assert-neq" => {
                        self.process_assert_neq(
                            &decl[1..],
                            &graph_rules,
                            &known_ops,
                            &system,
                        )?;
                    }
                    "with-scope" => {
                        // [with-scope ScopeName body...]
                        if decl.len() >= 3 {
                            let scope_name = decl[1].as_atom().unwrap_or("?");
                            if let Some(&scope_id) = self.scopes.get(scope_name) {
                                self.arena.activate_scope(scope_id);
                                // Process inner declarations
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
                                            "eval-reverse" => {
                                                self.process_eval(
                                                    &inner_decl[1..],
                                                    &inverse_graph_rules,
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
                                            }
                                            "assert-neq" => {
                                                self.process_assert_neq(
                                                    &inner_decl[1..],
                                                    &graph_rules,
                                                    &known_ops,
                                                    &system,
                                                )?;
                                            }
                                            _ => {
                                                return Err(ApeironError::InvalidConfig {
                                                    block: "with-scope".into(),
                                                    detail: format!("unknown declaration: {}", inner_head),
                                                })
                                            }
                                        }
                                    }
                                }
                                self.arena.deactivate_scope(scope_id);
                            }
                        }
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

        self.rules.insert(theory_name.clone(), theory_rules);
        self.compiled_rules
            .insert(theory_name.clone(), graph_rules);
        self.output
            .push(format!("[THEORY] {} loaded", theory_name));
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
    fn process_theory_with_eval() {
        let source = r#"
        [System Test
          [@syntax [sort Term]]
          [@binding implicit]
          [@check beta-reduction]
        ]
        [Theory Demo :in Test
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
    fn process_assert_eq_identity() {
        let source = r#"
        [System Test
          [@syntax [sort Term]]
          [@binding implicit]
          [@check beta-reduction]
        ]
        [Theory Demo :in Test
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
}
