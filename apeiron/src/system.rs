use std::collections::{HashMap, HashSet};

use crate::arena::Arena;
use crate::builder::{self, BuildEnv};
use crate::error::{ApeironError, Result};
use crate::hash;
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
}

/// A checking capability.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CheckMode {
    Rewriting,
    Unification,
    BetaReduction,
    IotaReduction,
    Oracle,
    Extensional,
    PatternUnification,
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
                    _ => {} // ignore unknown blocks for now
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
        let known_ops: HashSet<String> = system
            .operators
            .iter()
            .map(|op| op.name.clone())
            .collect();

        let mut theory_rules = Vec::new();
        let mut graph_rules: Vec<rewrite::GraphRule> = Vec::new();

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
                    "@rule" => {
                        self.process_rule(&decl[1..], &mut theory_rules)?;
                        // Compile the just-added rule into a GraphRule
                        if let Some(rule) = theory_rules.last() {
                            if let Some(gr) =
                                rewrite::compile_rule(&rule.name, &rule.lhs, &rule.rhs)
                            {
                                graph_rules.push(gr);
                            }
                        }
                    }
                    "eval" => {
                        self.process_eval(&decl[1..], &graph_rules, &known_ops)?;
                    }
                    "assert-eq" => {
                        self.process_assert_eq(&decl[1..], &graph_rules, &known_ops)?;
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
                                                )?;
                                            }
                                            "assert-eq" => {
                                                self.process_assert_eq(
                                                    &inner_decl[1..],
                                                    &graph_rules,
                                                    &known_ops,
                                                )?;
                                            }
                                            _ => {}
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
                    "Inductive" => {
                        if decl.len() >= 2 {
                            let name = decl[1].as_atom().unwrap_or("?");
                            self.output.push(format!("[INDUCTIVE] {}", name));
                        }
                    }
                    _ => {
                        // Unknown declaration — skip
                    }
                }
            }
        }

        self.rules.insert(theory_name.clone(), theory_rules);
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

    /// Build a term, run physics + graph rewrite loop, return (root_ptr, total_interactions).
    fn build_and_normalize(
        &mut self,
        expr: &Sexp,
        graph_rules: &[rewrite::GraphRule],
        known_ops: &HashSet<String>,
    ) -> (crate::node::Ptr, u64) {
        // Expand defs
        let expanded = rewrite::expand_defs(expr, &self.defs);

        // Build with known ops
        let mut env = BuildEnv::new();
        env.known_ops = known_ops.clone();
        env.scope_ids = self.scopes.clone();
        let root = builder::build_rooted(&mut self.arena, &mut env, &expanded);

        // Physics + rewrite loop
        let mut total = 0u64;
        loop {
            let result = physics::run(&mut self.arena, &PhysicsConfig::default());
            total += result.interactions;

            if graph_rules.is_empty() {
                break;
            }
            if !rewrite::try_rewrite_scan(&mut self.arena, graph_rules) {
                break;
            }
        }

        (root, total)
    }

    fn process_eval(
        &mut self,
        items: &[Sexp],
        graph_rules: &[rewrite::GraphRule],
        known_ops: &HashSet<String>,
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

        let (root, interactions) = self.build_and_normalize(expr, graph_rules, known_ops);

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

        let (lhs_root, _) = self.build_and_normalize(lhs_sexp, graph_rules, known_ops);
        let (rhs_root, _) = self.build_and_normalize(rhs_sexp, graph_rules, known_ops);

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

        let lhs_hash = hash::topological_hash(&self.arena, lhs_ptr);
        let rhs_hash = hash::topological_hash(&self.arena, rhs_ptr);

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
                    crate::node::OpCode::Lens { shift } => format!("Lens({})", shift),
                    crate::node::OpCode::Future { constraint_id } => {
                        format!("Future#{}", constraint_id)
                    }
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
                _ => {}
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
            "iota-reduction" => CheckMode::IotaReduction,
            "oracle" => CheckMode::Oracle,
            "extensional" => CheckMode::Extensional,
            "pattern-unification" => CheckMode::PatternUnification,
            _ => continue,
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
