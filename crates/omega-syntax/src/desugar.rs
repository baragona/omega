/// Desugaring: Sexp → Core types (Theory, Derivation, MetaTheorem, etc.)
use omega_core::binding_spec::BindingSpec;
use omega_core::derivation::Derivation;
use omega_core::expr::Expr;
use omega_core::judgment::{ConstructorDecl, JudgmentForm, RewriteRule, Rule, SortDecl};
use omega_core::metatheorem::{MetaCase, MetaProof, MetaTheorem};
use omega_core::theory::{ContextMode, Import, Theory};

use crate::sexp::Sexp;
use crate::span::Span;

/// Desugaring errors.
#[derive(Debug, Clone)]
pub struct DesugarError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for DesugarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "desugar error at {}: {}", self.span, self.message)
    }
}

type Result<T> = std::result::Result<T, DesugarError>;

/// A top-level command parsed from the source.
#[derive(Debug, Clone)]
pub enum Command {
    /// A theory definition.
    TheoryDef(Theory),
    /// Check a theory.
    CheckTheory(String),
    /// A proof with an explicit derivation.
    Proof {
        name: String,
        theory: String,
        goal: Expr,
        derivation: Derivation,
        assumptions: Vec<Expr>,
    },
    /// A proof with tactics.
    TacticProof {
        name: String,
        theory: String,
        goal: Expr,
        tactics: Vec<TacticCmd>,
        assumptions: Vec<Expr>,
    },
    /// A metatheorem declaration.
    MetaTheoremDef(MetaTheorem),
    /// A reflection command.
    Reflect {
        metatheorem: String,
        rule_name: String,
        theory: String,
    },
    /// Emit a rope expression as flattened text.
    Emit {
        theory: String,
        expr: Expr,
    },
}

/// A tactic command (parsed from S-expressions).
#[derive(Debug, Clone)]
pub enum TacticCmd {
    Apply(String),
    Assumption,
    Intro(Option<String>),
    Exact(Derivation),
    Auto(Option<usize>),
    Qed,
}

/// Desugar a list of top-level S-expressions into commands.
pub fn desugar_program(sexps: &[Sexp]) -> Result<Vec<Command>> {
    let mut commands = Vec::new();
    for sexp in sexps {
        commands.push(desugar_command(sexp)?);
    }
    Ok(commands)
}

/// Desugar a single top-level S-expression.
fn desugar_command(sexp: &Sexp) -> Result<Command> {
    let items = sexp
        .as_list()
        .ok_or_else(|| DesugarError {
            message: "top-level form must be a list".to_string(),
            span: sexp.span(),
        })?;

    if items.is_empty() {
        return Err(DesugarError {
            message: "empty top-level form".to_string(),
            span: sexp.span(),
        });
    }

    let head = items[0]
        .as_atom()
        .ok_or_else(|| DesugarError {
            message: "top-level form must start with a keyword".to_string(),
            span: items[0].span(),
        })?;

    match head {
        "theory" => desugar_theory(items, sexp.span()),
        "check-theory" => {
            if items.len() != 2 {
                return Err(DesugarError {
                    message: "check-theory expects exactly one argument".to_string(),
                    span: sexp.span(),
                });
            }
            let name = expect_atom(&items[1])?;
            Ok(Command::CheckTheory(name.to_string()))
        }
        "proof" => desugar_proof(items, sexp.span()),
        "meta-theorem" => desugar_metatheorem(items, sexp.span()),
        "reflect" => desugar_reflect(items, sexp.span()),
        "emit" => desugar_emit(items, sexp.span()),
        _ => Err(DesugarError {
            message: format!("unknown top-level form: {}", head),
            span: items[0].span(),
        }),
    }
}

fn desugar_theory(items: &[Sexp], span: Span) -> Result<Command> {
    if items.len() < 3 {
        return Err(DesugarError {
            message: "theory needs a name and at least one declaration".to_string(),
            span,
        });
    }

    let name = expect_atom(&items[1])?;
    let mut theory = Theory::new(name);

    // Check for :params keyword before declarations
    let decl_start = if items.len() > 3 && items[2].is_keyword(":params") {
        let param_list = items[3].as_list().ok_or_else(|| DesugarError {
            message: ":params expects a list of (name type) pairs".to_string(),
            span: items[3].span(),
        })?;
        for param_sexp in param_list {
            let pair = param_sexp.as_list().ok_or_else(|| DesugarError {
                message: "each parameter must be (name type)".to_string(),
                span: param_sexp.span(),
            })?;
            if pair.len() != 2 {
                return Err(DesugarError {
                    message: "parameter must be (name type)".to_string(),
                    span: param_sexp.span(),
                });
            }
            let param_name = expect_atom(&pair[0])?;
            let param_ty = desugar_expr(&pair[1])?;
            theory.params.push((param_name.to_string(), param_ty));
        }
        4 // declarations start after :params and the param list
    } else {
        2
    };

    for item in &items[decl_start..] {
        let decl = item
            .as_list()
            .ok_or_else(|| DesugarError {
                message: "theory declaration must be a list".to_string(),
                span: item.span(),
            })?;

        if decl.is_empty() {
            continue;
        }

        let kind = expect_atom(&decl[0])?;
        match kind {
            "sort" => {
                let sort_name = expect_atom(&decl[1])?;
                theory.sorts.push(SortDecl {
                    name: sort_name.to_string(),
                });
            }
            "constructor" => {
                let ctor_name = expect_atom(&decl[1])?;
                // (constructor name : type)
                if decl.len() >= 4 && expect_atom(&decl[2]).ok() == Some(":") {
                    let ty = desugar_expr(&decl[3])?;
                    theory.constructors.push(ConstructorDecl {
                        name: ctor_name.to_string(),
                        ty,
                    });
                } else {
                    // Just a name, no explicit type
                    theory.constructors.push(ConstructorDecl {
                        name: ctor_name.to_string(),
                        ty: Expr::sym("_"),
                    });
                }
            }
            "judgment" => {
                let pattern_sexp = &decl[1];
                let pattern = desugar_expr(pattern_sexp)?;

                // Extract judgment name from pattern
                let jname = match &pattern {
                    Expr::App(args) if !args.is_empty() => match &args[0] {
                        Expr::Sym(n) => n.clone(),
                        _ => "unnamed".to_string(),
                    },
                    Expr::Sym(n) => n.clone(),
                    _ => "unnamed".to_string(),
                };

                // Parse :where constraints
                let mut constraints = Vec::new();
                let mut i = 2;
                while i < decl.len() {
                    if decl[i].is_keyword(":where") {
                        i += 1;
                        // Parse pairs: name : sort
                        while i + 2 < decl.len() {
                            let var = expect_atom(&decl[i])?;
                            let colon = expect_atom(&decl[i + 1])?;
                            if colon != ":" {
                                break;
                            }
                            let sort = expect_atom(&decl[i + 2])?;
                            constraints.push((var.to_string(), sort.to_string()));
                            i += 3;
                        }
                    } else {
                        i += 1;
                    }
                }

                theory.judgments.push(JudgmentForm {
                    name: jname,
                    pattern,
                    constraints,
                });
            }
            "rule" => {
                let rule = desugar_rule(decl)?;
                theory.rules.push(rule);
            }
            "binding-spec" => {
                let bs = desugar_binding_spec(decl)?;
                theory.binding_specs.push(bs);
            }
            "context-mode" => {
                // (context-mode affine) or (context-mode structural)
                if decl.len() != 2 {
                    return Err(DesugarError {
                        message: "context-mode expects exactly one argument: affine or structural".to_string(),
                        span: decl[0].span(),
                    });
                }
                let mode = expect_atom(&decl[1])?;
                match mode {
                    "affine" => theory.context_mode = ContextMode::Affine,
                    "structural" => theory.context_mode = ContextMode::Structural,
                    _ => {
                        return Err(DesugarError {
                            message: format!("unknown context mode: {} (expected affine or structural)", mode),
                            span: decl[1].span(),
                        });
                    }
                }
            }
            "rewrite" => {
                // (rewrite name lhs rhs)
                if decl.len() != 4 {
                    return Err(DesugarError {
                        message: "rewrite expects name, lhs, and rhs".to_string(),
                        span: decl[0].span(),
                    });
                }
                let rw_name = expect_atom(&decl[1])?;
                let lhs = desugar_expr(&decl[2])?;
                let rhs = desugar_expr(&decl[3])?;
                theory.rewrites.push(RewriteRule {
                    name: rw_name.to_string(),
                    lhs,
                    rhs,
                });
            }
            "mutual" => {
                // (mutual (constructor ...) (rule ...) (rewrite ...) ...)
                // Syntactic grouping: flatten sub-declarations into the theory
                for sub in &decl[1..] {
                    let sub_decl = sub.as_list().ok_or_else(|| DesugarError {
                        message: "mutual sub-declaration must be a list".to_string(),
                        span: sub.span(),
                    })?;
                    if sub_decl.is_empty() {
                        continue;
                    }
                    let sub_kind = expect_atom(&sub_decl[0])?;
                    match sub_kind {
                        "sort" => {
                            let sort_name = expect_atom(&sub_decl[1])?;
                            theory.sorts.push(SortDecl {
                                name: sort_name.to_string(),
                            });
                        }
                        "constructor" => {
                            let ctor_name = expect_atom(&sub_decl[1])?;
                            if sub_decl.len() >= 4 && expect_atom(&sub_decl[2]).ok() == Some(":") {
                                let ty = desugar_expr(&sub_decl[3])?;
                                theory.constructors.push(ConstructorDecl {
                                    name: ctor_name.to_string(),
                                    ty,
                                });
                            } else {
                                theory.constructors.push(ConstructorDecl {
                                    name: ctor_name.to_string(),
                                    ty: Expr::sym("_"),
                                });
                            }
                        }
                        "judgment" => {
                            let pattern_sexp = &sub_decl[1];
                            let pattern = desugar_expr(pattern_sexp)?;
                            let jname = match &pattern {
                                Expr::App(args) if !args.is_empty() => match &args[0] {
                                    Expr::Sym(n) => n.clone(),
                                    _ => "unnamed".to_string(),
                                },
                                Expr::Sym(n) => n.clone(),
                                _ => "unnamed".to_string(),
                            };
                            let mut constraints = Vec::new();
                            let mut ci = 2;
                            while ci < sub_decl.len() {
                                if sub_decl[ci].is_keyword(":where") {
                                    ci += 1;
                                    while ci + 2 < sub_decl.len() {
                                        let var = expect_atom(&sub_decl[ci])?;
                                        let colon = expect_atom(&sub_decl[ci + 1])?;
                                        if colon != ":" { break; }
                                        let sort = expect_atom(&sub_decl[ci + 2])?;
                                        constraints.push((var.to_string(), sort.to_string()));
                                        ci += 3;
                                    }
                                } else {
                                    ci += 1;
                                }
                            }
                            theory.judgments.push(JudgmentForm {
                                name: jname,
                                pattern,
                                constraints,
                            });
                        }
                        "rule" => {
                            let rule = desugar_rule(sub_decl)?;
                            theory.rules.push(rule);
                        }
                        "rewrite" => {
                            if sub_decl.len() != 4 {
                                return Err(DesugarError {
                                    message: "rewrite expects name, lhs, and rhs".to_string(),
                                    span: sub_decl[0].span(),
                                });
                            }
                            let rw_name = expect_atom(&sub_decl[1])?;
                            let lhs = desugar_expr(&sub_decl[2])?;
                            let rhs = desugar_expr(&sub_decl[3])?;
                            theory.rewrites.push(RewriteRule {
                                name: rw_name.to_string(),
                                lhs,
                                rhs,
                            });
                        }
                        _ => {
                            return Err(DesugarError {
                                message: format!("unsupported declaration in mutual block: {}", sub_kind),
                                span: sub_decl[0].span(),
                            });
                        }
                    }
                }
            }
            "import" => {
                // (import TheoryName)
                // (import TheoryName :as Alias)
                // (import TheoryName arg1 arg2 ... :as Alias)
                if decl.len() < 2 {
                    return Err(DesugarError {
                        message: "import expects at least a theory name".to_string(),
                        span: decl[0].span(),
                    });
                }
                let import_name = expect_atom(&decl[1])?;
                let mut args = Vec::new();
                let mut alias = None;
                let mut ii = 2;
                while ii < decl.len() {
                    if decl[ii].is_keyword(":as") {
                        ii += 1;
                        if ii >= decl.len() {
                            return Err(DesugarError {
                                message: ":as requires an alias name".to_string(),
                                span: decl[ii - 1].span(),
                            });
                        }
                        alias = Some(expect_atom(&decl[ii])?.to_string());
                        ii += 1;
                    } else {
                        args.push(desugar_expr(&decl[ii])?);
                        ii += 1;
                    }
                }
                if !args.is_empty() && alias.is_none() {
                    return Err(DesugarError {
                        message: "parameterized import requires :as alias".to_string(),
                        span: decl[0].span(),
                    });
                }
                theory.imports.push(Import {
                    theory_name: import_name.to_string(),
                    args,
                    alias,
                });
            }
            "binder-behavior" => {
                // (binder-behavior name :substitutive :eta :linear :affine)
                if decl.len() < 3 {
                    return Err(DesugarError {
                        message: "binder-behavior expects a name and at least one flag".to_string(),
                        span: decl[0].span(),
                    });
                }
                let binder_name = expect_atom(&decl[1])?;
                for flag_sexp in &decl[2..] {
                    let flag = expect_atom(flag_sexp)?;
                    match flag {
                        ":substitutive" => { theory.substitutive_binders.insert(binder_name.to_string()); }
                        ":eta" => { theory.eta_binders.insert(binder_name.to_string()); }
                        ":linear" => { theory.linear_binders.insert(binder_name.to_string()); }
                        ":affine" => { theory.affine_binders.insert(binder_name.to_string()); }
                        _ => {
                            return Err(DesugarError {
                                message: format!("unknown binder-behavior flag: {} (expected :substitutive, :eta, :linear, :affine)", flag),
                                span: flag_sexp.span(),
                            });
                        }
                    }
                }
            }
            "attribute" => {
                // (attribute symbol-name :ac) or (attribute symbol-name :aci)
                if decl.len() != 3 {
                    return Err(DesugarError {
                        message: "attribute expects a symbol name and a flag (:ac or :aci)".to_string(),
                        span: decl[0].span(),
                    });
                }
                let sym_name = expect_atom(&decl[1])?;
                let flag = expect_atom(&decl[2])?;
                let attr = match flag {
                    ":ac" => omega_core::theory::Attribute::AC,
                    ":aci" => omega_core::theory::Attribute::ACI,
                    _ => {
                        return Err(DesugarError {
                            message: format!("unknown attribute flag: {} (expected :ac or :aci)", flag),
                            span: decl[2].span(),
                        });
                    }
                };
                theory.attributes.entry(sym_name.to_string()).or_insert_with(std::collections::HashSet::new).insert(attr);
            }
            _ => {
                return Err(DesugarError {
                    message: format!("unknown theory declaration: {}", kind),
                    span: decl[0].span(),
                });
            }
        }
    }

    // Auto-register "lambda" as substitutive (triggers beta-reduction)
    theory.substitutive_binders.insert(omega_core::expr::LAMBDA.to_string());
    theory.compute_hash();
    Ok(Command::TheoryDef(theory))
}

fn desugar_rule(items: &[Sexp]) -> Result<Rule> {
    // (rule name :premises (...) :conclusion ...)
    let name = expect_atom(&items[1])?;
    let mut premises = Vec::new();
    let mut conclusion = None;

    let mut i = 2;
    while i < items.len() {
        if items[i].is_keyword(":levels") {
            // Skip :levels for backwards compatibility (ignored)
            i += 1;
            if i < items.len() && items[i].as_list().is_some() {
                i += 1;
            }
        } else if items[i].is_keyword(":premises") {
            i += 1;
            if let Some(plist) = items[i].as_list() {
                for p in plist {
                    premises.push(desugar_expr(p)?);
                }
            }
            i += 1;
        } else if items[i].is_keyword(":conclusion") {
            i += 1;
            conclusion = Some(desugar_expr(&items[i])?);
            i += 1;
        } else {
            i += 1;
        }
    }

    let conclusion = conclusion.ok_or_else(|| DesugarError {
        message: format!("rule {} has no conclusion", name),
        span: items[0].span(),
    })?;

    // Parse optional :implicit and :context
    let mut implicit_args = Vec::new();
    let mut context_extensions = Vec::new();
    let mut j = 2;
    while j < items.len() {
        if items[j].is_keyword(":implicit") {
            j += 1;
            if let Some(ilist) = items[j].as_list() {
                for item in ilist {
                    if let Some(a) = item.as_atom() {
                        implicit_args.push(a.to_string());
                    }
                }
            }
            j += 1;
        } else if items[j].is_keyword(":context") {
            j += 1;
            if let Some(clist) = items[j].as_list() {
                for item in clist {
                    if let Some(pair) = item.as_list() {
                        if pair.len() >= 2 {
                            if let Some(idx_str) = pair[0].as_atom() {
                                if let Ok(idx) = idx_str.parse::<usize>() {
                                    if let Ok(expr) = desugar_expr(&pair[1]) {
                                        context_extensions.push((idx, expr));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            j += 1;
        } else {
            j += 1;
        }
    }

    Ok(Rule {
        name: name.to_string(),
        premises,
        conclusion,
        reflected: false,
        provenance: None,
        implicit_args,
        context_extensions,
    })
}

fn desugar_binding_spec(items: &[Sexp]) -> Result<BindingSpec> {
    // (binding-spec name :binds N [:scope (positions...)] [:linear] [:affine] [:display pattern])
    let name = expect_atom(&items[1])?;
    let mut arity = 1;
    let mut body_positions = Vec::new();
    let mut linear = false;
    let mut affine = false;
    let mut display = None;

    let mut i = 2;
    while i < items.len() {
        if items[i].is_keyword(":binds") {
            i += 1;
            let n = expect_atom(&items[i])?;
            arity = n.parse::<usize>().map_err(|_| DesugarError {
                message: format!("invalid arity: {}", n),
                span: items[i].span(),
            })?;
            i += 1;
        } else if items[i].is_keyword(":scope") {
            i += 1;
            if let Some(plist) = items[i].as_list() {
                for p in plist {
                    let idx_str = expect_atom(p)?;
                    let idx = idx_str.parse::<usize>().map_err(|_| DesugarError {
                        message: format!("invalid scope position: {}", idx_str),
                        span: p.span(),
                    })?;
                    body_positions.push(idx);
                }
            } else {
                // Single position
                let idx_str = expect_atom(&items[i])?;
                let idx = idx_str.parse::<usize>().map_err(|_| DesugarError {
                    message: format!("invalid scope position: {}", idx_str),
                    span: items[i].span(),
                })?;
                body_positions.push(idx);
            }
            i += 1;
        } else if items[i].is_keyword(":linear") {
            linear = true;
            i += 1;
        } else if items[i].is_keyword(":affine") {
            affine = true;
            i += 1;
        } else if items[i].is_keyword(":display") {
            i += 1;
            // Take the next atom or sexp as the display pattern string
            display = Some(format!("{}", items[i]));
            i += 1;
        } else {
            i += 1;
        }
    }

    Ok(BindingSpec {
        name: name.to_string(),
        arity,
        body_positions,
        linear,
        affine,
        display,
    })
}

fn desugar_proof(items: &[Sexp], span: Span) -> Result<Command> {
    // (proof name :theory T :goal G :derivation D)
    // (proof name :theory T :goal G :assumptions (...) :derivation D)
    // (proof name :theory T :goal G :tactics ...)
    let name = expect_atom(&items[1])?;
    let mut theory = None;
    let mut goal = None;
    let mut derivation = None;
    let mut tactics = Vec::new();
    let mut assumptions = Vec::new();
    let mut is_tactic = false;

    let mut i = 2;
    while i < items.len() {
        if items[i].is_keyword(":theory") {
            i += 1;
            theory = Some(expect_atom(&items[i])?.to_string());
            i += 1;
        } else if items[i].is_keyword(":goal") {
            i += 1;
            goal = Some(desugar_expr(&items[i])?);
            i += 1;
        } else if items[i].is_keyword(":assumptions") {
            i += 1;
            if let Some(alist) = items[i].as_list() {
                for a in alist {
                    assumptions.push(desugar_expr(a)?);
                }
            }
            i += 1;
        } else if items[i].is_keyword(":derivation") {
            i += 1;
            derivation = Some(desugar_derivation(&items[i])?);
            i += 1;
        } else if items[i].is_keyword(":tactics") {
            is_tactic = true;
            i += 1;
            while i < items.len() {
                tactics.push(desugar_tactic(&items[i])?);
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    let theory = theory.ok_or_else(|| DesugarError {
        message: "proof requires :theory".to_string(),
        span,
    })?;
    let goal = goal.ok_or_else(|| DesugarError {
        message: "proof requires :goal".to_string(),
        span,
    })?;

    if is_tactic {
        Ok(Command::TacticProof {
            name: name.to_string(),
            theory,
            goal,
            tactics,
            assumptions,
        })
    } else {
        let derivation = derivation.ok_or_else(|| DesugarError {
            message: "proof requires :derivation or :tactics".to_string(),
            span,
        })?;
        Ok(Command::Proof {
            name: name.to_string(),
            theory,
            goal,
            derivation,
            assumptions,
        })
    }
}

fn desugar_derivation(sexp: &Sexp) -> Result<Derivation> {
    match sexp {
        Sexp::Atom(s, _) if s == "assumption" => Ok(Derivation::Assumption),
        Sexp::List(items, span) => {
            if items.is_empty() {
                return Err(DesugarError {
                    message: "empty derivation".to_string(),
                    span: *span,
                });
            }
            let head = expect_atom(&items[0])?;
            if head == "assumption" {
                if items.len() == 1 {
                    return Ok(Derivation::Assumption);
                } else if items.len() == 2 {
                    let idx_str = expect_atom(&items[1])?;
                    let idx: usize = idx_str.parse().map_err(|_| DesugarError {
                        message: format!("expected integer for assumption index, got '{}'", idx_str),
                        span: items[1].span(),
                    })?;
                    return Ok(Derivation::AssumptionIdx(idx));
                }
            }
            // (rule-name sub1 sub2 ...)
            let mut premises = Vec::new();
            for item in &items[1..] {
                premises.push(desugar_derivation(item)?);
            }
            Ok(Derivation::RuleApp {
                rule_name: head.to_string(),
                premises,
            })
        }
        _ => Err(DesugarError {
            message: "invalid derivation".to_string(),
            span: sexp.span(),
        }),
    }
}

fn desugar_tactic(sexp: &Sexp) -> Result<TacticCmd> {
    match sexp {
        Sexp::Atom(s, _) if s == "assumption" => Ok(TacticCmd::Assumption),
        Sexp::Atom(s, _) if s == "qed" => Ok(TacticCmd::Qed),
        Sexp::List(items, span) => {
            if items.is_empty() {
                return Err(DesugarError {
                    message: "empty tactic".to_string(),
                    span: *span,
                });
            }
            let head = expect_atom(&items[0])?;
            match head {
                "apply" => {
                    let rule = expect_atom(&items[1])?;
                    Ok(TacticCmd::Apply(rule.to_string()))
                }
                "intro" => {
                    let name = if items.len() > 1 {
                        Some(expect_atom(&items[1])?.to_string())
                    } else {
                        None
                    };
                    Ok(TacticCmd::Intro(name))
                }
                "auto" => {
                    let depth = if items.len() > 1 {
                        let s = expect_atom(&items[1])?;
                        Some(s.parse::<usize>().map_err(|_| DesugarError {
                            message: format!("invalid auto depth: {}", s),
                            span: items[1].span(),
                        })?)
                    } else {
                        None
                    };
                    Ok(TacticCmd::Auto(depth))
                }
                _ => Err(DesugarError {
                    message: format!("unknown tactic: {}", head),
                    span: items[0].span(),
                }),
            }
        }
        _ => Err(DesugarError {
            message: "invalid tactic".to_string(),
            span: sexp.span(),
        }),
    }
}

fn desugar_metatheorem(items: &[Sexp], span: Span) -> Result<Command> {
    // (meta-theorem name :theory T :forall (...) :exists (...) :proof ...)
    let name = expect_atom(&items[1])?;
    let mut theory_name = None;
    let mut forall = Vec::new();
    let mut exists = Vec::new();
    let mut proof = None;

    let mut i = 2;
    while i < items.len() {
        if items[i].is_keyword(":theory") {
            i += 1;
            theory_name = Some(expect_atom(&items[i])?.to_string());
            i += 1;
        } else if items[i].is_keyword(":forall") {
            i += 1;
            if let Some(flist) = items[i].as_list() {
                for item in flist {
                    let pair = item.as_list().ok_or_else(|| DesugarError {
                        message: "forall binding must be a list".to_string(),
                        span: item.span(),
                    })?;
                    let var = expect_atom(&pair[0])?;
                    let judgment = desugar_expr(&pair[1])?;
                    forall.push((var.to_string(), judgment));
                }
            }
            i += 1;
        } else if items[i].is_keyword(":exists") {
            i += 1;
            if let Some(elist) = items[i].as_list() {
                for item in elist {
                    let pair = item.as_list().ok_or_else(|| DesugarError {
                        message: "exists binding must be a list".to_string(),
                        span: item.span(),
                    })?;
                    let var = expect_atom(&pair[0])?;
                    let judgment = desugar_expr(&pair[1])?;
                    exists.push((var.to_string(), judgment));
                }
            }
            i += 1;
        } else if items[i].is_keyword(":proof") {
            i += 1;
            proof = Some(desugar_metaproof(&items[i])?);
            i += 1;
        } else {
            i += 1;
        }
    }

    let theory_name = theory_name.ok_or_else(|| DesugarError {
        message: "meta-theorem requires :theory".to_string(),
        span,
    })?;
    let proof = proof.ok_or_else(|| DesugarError {
        message: "meta-theorem requires :proof".to_string(),
        span,
    })?;

    Ok(Command::MetaTheoremDef(MetaTheorem {
        name: name.to_string(),
        theory_name,
        forall,
        exists,
        proof,
    }))
}

fn desugar_metaproof(sexp: &Sexp) -> Result<MetaProof> {
    let items = sexp.as_list().ok_or_else(|| DesugarError {
        message: "meta-proof must be a list".to_string(),
        span: sexp.span(),
    })?;

    if items.is_empty() {
        return Err(DesugarError {
            message: "empty meta-proof".to_string(),
            span: sexp.span(),
        });
    }

    let head = expect_atom(&items[0])?;
    match head {
        "case-analysis" => {
            let scrutinee = expect_atom(&items[1])?.to_string();
            let mut cases = Vec::new();
            for item in &items[2..] {
                cases.push(desugar_metacase(item)?);
            }
            Ok(MetaProof::CaseAnalysis { scrutinee, cases })
        }
        "by-rule" => {
            let rule_name = expect_atom(&items[1])?.to_string();
            let mut args = Vec::new();
            for item in &items[2..] {
                args.push(desugar_metaproof_atom(item)?);
            }
            Ok(MetaProof::ByRule { rule_name, args })
        }
        "inductive" => {
            let mt_name = expect_atom(&items[1])?.to_string();
            let arg = expect_atom(&items[2])?.to_string();
            Ok(MetaProof::Inductive {
                metatheorem_name: mt_name,
                arg,
            })
        }
        "var" => {
            let name = expect_atom(&items[1])?.to_string();
            Ok(MetaProof::Var(name))
        }
        _ => {
            // Could be a var reference if it's a simple atom
            Err(DesugarError {
                message: format!("unknown meta-proof form: {}", head),
                span: items[0].span(),
            })
        }
    }
}

fn desugar_metaproof_atom(sexp: &Sexp) -> Result<MetaProof> {
    match sexp {
        Sexp::Atom(s, _) => Ok(MetaProof::Var(s.clone())),
        _ => desugar_metaproof(sexp),
    }
}

fn desugar_metacase(sexp: &Sexp) -> Result<MetaCase> {
    // (case rule-name (premise-names...) body)
    // or (case rule-name body)  -- for zero-premise rules
    let items = sexp.as_list().ok_or_else(|| DesugarError {
        message: "case must be a list".to_string(),
        span: sexp.span(),
    })?;

    if items.len() < 3 {
        return Err(DesugarError {
            message: "case needs at least a rule name and body".to_string(),
            span: sexp.span(),
        });
    }

    let head = expect_atom(&items[0])?;
    if head != "case" {
        return Err(DesugarError {
            message: format!("expected 'case', got '{}'", head),
            span: items[0].span(),
        });
    }

    let rule_name = expect_atom(&items[1])?.to_string();

    // Check if items[2] is a list of premise names or a proof body
    if items.len() == 4 {
        // (case rule-name (premise-names) body)
        let premise_names = if let Some(plist) = items[2].as_list() {
            plist.iter().map(|s| expect_atom(s).map(|a| a.to_string())).collect::<Result<Vec<_>>>()?
        } else {
            return Err(DesugarError {
                message: "expected list of premise names".to_string(),
                span: items[2].span(),
            });
        };
        let body = desugar_metaproof(&items[3])?;
        Ok(MetaCase {
            rule_name,
            premise_names,
            body,
        })
    } else {
        // (case rule-name body) -- no premises
        let body = desugar_metaproof(&items[2])?;
        Ok(MetaCase {
            rule_name,
            premise_names: Vec::new(),
            body,
        })
    }
}

fn desugar_reflect(items: &[Sexp], span: Span) -> Result<Command> {
    // (reflect metatheorem-name :as rule-name :theory theory-name)
    let mt_name = expect_atom(&items[1])?.to_string();
    let mut rule_name = None;
    let mut theory = None;

    let mut i = 2;
    while i < items.len() {
        if items[i].is_keyword(":as") {
            i += 1;
            rule_name = Some(expect_atom(&items[i])?.to_string());
            i += 1;
        } else if items[i].is_keyword(":theory") {
            i += 1;
            theory = Some(expect_atom(&items[i])?.to_string());
            i += 1;
        } else {
            i += 1;
        }
    }

    let rule_name = rule_name.ok_or_else(|| DesugarError {
        message: "reflect requires :as".to_string(),
        span,
    })?;
    let theory = theory.ok_or_else(|| DesugarError {
        message: "reflect requires :theory".to_string(),
        span,
    })?;

    Ok(Command::Reflect {
        metatheorem: mt_name,
        rule_name,
        theory,
    })
}

fn desugar_emit(items: &[Sexp], span: Span) -> Result<Command> {
    // (emit :theory T EXPR)
    let mut theory = None;
    let mut expr = None;
    let mut i = 1;
    while i < items.len() {
        if items[i].is_keyword(":theory") {
            i += 1;
            theory = Some(expect_atom(&items[i])?.to_string());
            i += 1;
        } else {
            expr = Some(desugar_expr(&items[i])?);
            i += 1;
        }
    }
    let theory = theory.ok_or_else(|| DesugarError {
        message: "emit requires :theory".to_string(),
        span,
    })?;
    let expr = expr.ok_or_else(|| DesugarError {
        message: "emit requires an expression".to_string(),
        span,
    })?;
    Ok(Command::Emit { theory, expr })
}

/// Desugar an S-expression into an Expr, with level parameter names in scope.
/// Helper: desugar a universe level argument as a regular expression.
/// Numeric literals get special handling: 0 → lzero, 1 → (lsuc lzero), etc.
/// Level operations (lsuc, lmax, imax) are recursively processed so that
/// (lsuc 2) → (lsuc (lsuc (lsuc lzero))).
fn desugar_level_expr(sexp: &Sexp) -> Result<Expr> {
    match sexp {
        Sexp::Atom(s, _) => {
            if let Ok(n) = s.parse::<usize>() {
                // Numeric literal: build nested lsuc applications
                let mut result = Expr::sym("lzero");
                for _ in 0..n {
                    result = Expr::app(vec![Expr::sym("lsuc"), result]);
                }
                return Ok(result);
            }
            // Everything else: desugar as a normal expr (lzero, lsuc, ?u, etc.)
            desugar_expr(sexp)
        }
        Sexp::List(items, _) => {
            if let Some(head) = items.first().and_then(|i| i.as_atom()) {
                match head {
                    "lsuc" | "lmax" | "imax" => {
                        // Recursively process level sub-expressions
                        let mut exprs = vec![Expr::sym(head)];
                        for item in &items[1..] {
                            exprs.push(desugar_level_expr(item)?);
                        }
                        return Ok(Expr::app(exprs));
                    }
                    _ => {}
                }
            }
            desugar_expr(sexp)
        }
    }
}

/// Desugar an S-expression into an Expr.
pub fn desugar_expr(sexp: &Sexp) -> Result<Expr> {
    match sexp {
        Sexp::Atom(s, _) => {
            if let Some(meta_name) = s.strip_prefix('?') {
                Ok(Expr::Meta(meta_name.to_string()))
            } else if s.starts_with('#') {
                // de Bruijn index — only if the rest is a valid integer
                if let Ok(idx) = s[1..].parse::<usize>() {
                    Ok(Expr::Bound(idx))
                } else {
                    Ok(Expr::Sym(s.clone()))
                }
            } else {
                // Treat as symbol by default (constructors, sort names, etc.)
                Ok(Expr::Sym(s.clone()))
            }
        }
        Sexp::List(items, span) => {
            if items.is_empty() {
                return Err(DesugarError {
                    message: "empty expression".to_string(),
                    span: *span,
                });
            }

            // Check for special forms
            if let Some(head) = items[0].as_atom() {
                match head {
                    "lambda" | "forall" => {
                        return desugar_binder(items, *span);
                    }
                    "->" if items.len() >= 3 => {
                        return desugar_arrow(items, *span);
                    }
                    "Type" if items.len() == 2 => {
                        let level_expr = desugar_level_expr(&items[1])?;
                        return Ok(Expr::app(vec![Expr::sym("Type"), level_expr]));
                    }
                    _ => {
                        // Check for custom binder syntax: (name (x : T) body)
                        // A 3-element list where the second element looks like a binding form
                        if items.len() == 3 {
                            if let Some(binding) = items[1].as_list() {
                                if binding.len() == 3 {
                                    if let Some(":") = binding[1].as_atom() {
                                        // This looks like a custom binder
                                        return desugar_custom_binder(items, *span);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // General application
            let exprs: Vec<Expr> = items
                .iter()
                .map(|item| desugar_expr(item))
                .collect::<Result<Vec<_>>>()?;
            Ok(Expr::app(exprs))
        }
    }
}

fn desugar_binder(items: &[Sexp], span: Span) -> Result<Expr> {
    // (lambda (x : T) body) or (forall (x : T) body)
    if items.len() != 3 {
        return Err(DesugarError {
            message: "binder needs exactly a binding and a body".to_string(),
            span,
        });
    }

    let kind_str = expect_atom(&items[0])?;
    let kind = match kind_str {
        "lambda" => omega_core::expr::LAMBDA.to_string(),
        "forall" => omega_core::expr::FORALL.to_string(),
        _ => unreachable!(),
    };

    let binding = items[1].as_list().ok_or_else(|| DesugarError {
        message: "binding must be a list (x : T)".to_string(),
        span: items[1].span(),
    })?;

    let (hint, ty) = if binding.len() == 3 && binding[1].is_keyword(":") {
        (expect_atom(&binding[0])?.to_string(), desugar_expr(&binding[2])?)
    } else if binding.len() == 1 {
        (expect_atom(&binding[0])?.to_string(), Expr::sym("_"))
    } else {
        return Err(DesugarError {
            message: "binding must be (x : T) or (x)".to_string(),
            span: items[1].span(),
        });
    };

    let body = desugar_expr(&items[2])?;

    Ok(Expr::Binder {
        kind,
        hint,
        ty: Box::new(ty),
        body: Box::new(body),
    })
}

fn desugar_custom_binder(items: &[Sexp], span: Span) -> Result<Expr> {
    // (kind (x : T) body) — custom binder kind
    if items.len() != 3 {
        return Err(DesugarError {
            message: "custom binder needs exactly a binding and a body".to_string(),
            span,
        });
    }

    let kind = expect_atom(&items[0])?.to_string();

    let binding = items[1].as_list().ok_or_else(|| DesugarError {
        message: "binding must be a list (x : T)".to_string(),
        span: items[1].span(),
    })?;

    let (hint, ty) = if binding.len() == 3 && binding[1].is_keyword(":") {
        (expect_atom(&binding[0])?.to_string(), desugar_expr(&binding[2])?)
    } else if binding.len() == 1 {
        (expect_atom(&binding[0])?.to_string(), Expr::sym("_"))
    } else {
        return Err(DesugarError {
            message: "binding must be (x : T) or (x)".to_string(),
            span: items[1].span(),
        });
    };

    let body = desugar_expr(&items[2])?;

    Ok(Expr::Binder {
        kind,
        hint,
        ty: Box::new(ty),
        body: Box::new(body),
    })
}

fn desugar_arrow(items: &[Sexp], span: Span) -> Result<Expr> {
    // (-> A B C ...) is right-associative: A -> (B -> C)
    // Represented as nested Binder with kind "->"
    let types: Vec<Expr> = items[1..]
        .iter()
        .map(|item| desugar_expr(item))
        .collect::<Result<Vec<_>>>()?;

    if types.len() < 2 {
        return Err(DesugarError {
            message: "arrow type needs at least two arguments".to_string(),
            span,
        });
    }

    // Build right-to-left
    let mut result = types.last().unwrap().clone();
    for ty in types[..types.len() - 1].iter().rev() {
        result = Expr::Binder {
            kind: omega_core::expr::ARROW.to_string(),
            hint: "_".to_string(),
            ty: Box::new(ty.clone()),
            body: Box::new(result),
        };
    }

    Ok(result)
}

// desugar_level removed — universe levels are now regular expressions

fn expect_atom<'a>(sexp: &'a Sexp) -> Result<&'a str> {
    sexp.as_atom().ok_or_else(|| DesugarError {
        message: "expected an atom".to_string(),
        span: sexp.span(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    #[test]
    fn desugar_simple_expr() {
        let sexps = parser::parse("(proves ?A)").unwrap();
        let expr = desugar_expr(&sexps[0]).unwrap();
        assert_eq!(
            expr,
            Expr::app(vec![Expr::sym("proves"), Expr::Meta("A".to_string())])
        );
    }

    #[test]
    fn desugar_meta_vars() {
        let sexps = parser::parse("?X").unwrap();
        let expr = desugar_expr(&sexps[0]).unwrap();
        assert_eq!(expr, Expr::Meta("X".to_string()));
    }

    #[test]
    fn desugar_theory_form() {
        let input = r#"
(theory PropLogic
  (sort Prop)
  (constructor true : Prop)
  (constructor and : (-> Prop Prop Prop))
  (judgment (proves ?P) :where P : Prop)
  (rule and-intro
    :premises ((proves ?A) (proves ?B))
    :conclusion (proves (and ?A ?B))))
"#;
        let sexps = parser::parse(input).unwrap();
        let cmds = desugar_program(&sexps).unwrap();
        assert_eq!(cmds.len(), 1);

        if let Command::TheoryDef(theory) = &cmds[0] {
            assert_eq!(theory.name, "PropLogic");
            assert_eq!(theory.sorts.len(), 1);
            assert_eq!(theory.constructors.len(), 2);
            assert_eq!(theory.judgments.len(), 1);
            assert_eq!(theory.rules.len(), 1);
            assert_eq!(theory.rules[0].name, "and-intro");
            assert_eq!(theory.rules[0].premises.len(), 2);
        } else {
            panic!("expected TheoryDef");
        }
    }

    #[test]
    fn desugar_proof_form() {
        let input = r#"
(proof my-thm
  :theory PropLogic
  :goal (proves (and ?A ?B))
  :derivation (and-intro (assumption) (assumption)))
"#;
        let sexps = parser::parse(input).unwrap();
        let cmds = desugar_program(&sexps).unwrap();
        assert_eq!(cmds.len(), 1);

        if let Command::Proof { name, theory, .. } = &cmds[0] {
            assert_eq!(name, "my-thm");
            assert_eq!(theory, "PropLogic");
        } else {
            panic!("expected Proof");
        }
    }

    #[test]
    fn desugar_binding_spec_form() {
        let input = r#"
(theory Linear
  (sort Prop)
  (binding-spec lin-lam :binds 1 :scope (0) :linear)
  (judgment (proves ?P) :where P : Prop))
"#;
        let sexps = parser::parse(input).unwrap();
        let cmds = desugar_program(&sexps).unwrap();
        if let Command::TheoryDef(theory) = &cmds[0] {
            assert_eq!(theory.binding_specs.len(), 1);
            assert_eq!(theory.binding_specs[0].name, "lin-lam");
            assert_eq!(theory.binding_specs[0].arity, 1);
            assert_eq!(theory.binding_specs[0].body_positions, vec![0]);
            assert!(theory.binding_specs[0].linear);
            assert!(!theory.binding_specs[0].affine);
        } else {
            panic!("expected TheoryDef");
        }
    }

    #[test]
    fn desugar_reflect_form() {
        let input = "(reflect and-comm-meta :as proves/and-comm :theory PropLogic)";
        let sexps = parser::parse(input).unwrap();
        let cmds = desugar_program(&sexps).unwrap();
        if let Command::Reflect {
            metatheorem,
            rule_name,
            theory,
        } = &cmds[0]
        {
            assert_eq!(metatheorem, "and-comm-meta");
            assert_eq!(rule_name, "proves/and-comm");
            assert_eq!(theory, "PropLogic");
        } else {
            panic!("expected Reflect");
        }
    }
}
