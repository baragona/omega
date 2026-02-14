/// Core transformation: Theory → RustCrate.
///
/// Sorts become enums, constructors become variants, rewrite rules become functions.
use std::collections::{HashMap, HashSet};

use omega_core::expr::{Expr, ARROW};
use omega_core::theory::Theory;

use super::rust_ast::*;

/// Skip these sorts — they are verification/emit infrastructure, not data.
const SKIP_SORTS: &[&str] = &["Prop", "String"];

/// Rust keywords that need escaping (appended with `_`).
const RUST_KEYWORDS: &[&str] = &[
    "Self", "as", "async", "await", "break", "continue", "crate", "else",
    "enum", "extern", "fn", "for", "if", "impl", "in", "let", "loop",
    "match", "mod", "move", "mut", "pub", "ref", "return", "self", "struct",
    "super", "trait", "type", "use", "where", "while", "yield",
];

fn sanitize_keyword(s: &str) -> String {
    if s == "type" {
        "Type_".to_string()
    } else if RUST_KEYWORDS.contains(&s) {
        format!("{}_", s)
    } else {
        s.to_string()
    }
}

/// Convert a name like "syn-recvd" or "ev-listen" to PascalCase: "SynRecvd", "EvListen".
fn to_pascal_case(s: &str) -> String {
    let raw: String = s
        .split(|c: char| c == '-' || c == '_' || c == '.')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    upper + &chars.as_str().to_lowercase()
                }
                None => String::new(),
            }
        })
        .collect();
    sanitize_keyword(&raw)
}

/// Convert a name like "can-send" to snake_case: "can_send".
fn to_snake_case(s: &str) -> String {
    let raw = s.replace('-', "_").replace('.', "_").to_lowercase();
    sanitize_keyword(&raw)
}

/// Flatten an arrow type to (param_types, return_type).
/// `(-> A B C)` means `A -> B -> C`, i.e. params=[A,B], ret=C.
/// A bare `Sym("Sort")` means nullary (no params).
fn flatten_arrow(ty: &Expr) -> (Vec<&Expr>, &Expr) {
    match ty {
        Expr::Binder {
            kind, ty: a, body, ..
        } if kind == ARROW => {
            let (mut params, ret) = flatten_arrow(body);
            params.insert(0, a.as_ref());
            (params, ret)
        }
        Expr::App(args) if args.len() >= 3 && matches!(&args[0], Expr::Sym(s) if s == "->") => {
            // (-> A B ... ReturnType) — last element is return type
            let param_types: Vec<&Expr> = args[1..args.len() - 1].iter().collect();
            let ret = &args[args.len() - 1];
            (param_types, ret)
        }
        _ => (vec![], ty),
    }
}

/// Get the sort name from a type expression (the return type of a constructor).
fn sort_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Sym(s) => Some(s.as_str()),
        _ => None,
    }
}

/// Extract the head symbol of an expression.
fn head_sym(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Sym(s) => Some(s),
        Expr::App(args) if !args.is_empty() => head_sym(&args[0]),
        _ => None,
    }
}

/// Collect all meta-variable names from an expression, in order of first appearance.
fn collect_metas_ordered(expr: &Expr) -> Vec<String> {
    let mut metas = Vec::new();
    collect_metas_inner(expr, &mut metas);
    metas
}

fn collect_metas_inner(expr: &Expr, acc: &mut Vec<String>) {
    match expr {
        Expr::Meta(n) => {
            if !acc.contains(n) {
                acc.push(n.clone());
            }
        }
        Expr::App(args) => {
            for a in args {
                collect_metas_inner(a, acc);
            }
        }
        Expr::Binder { ty, body, .. } => {
            collect_metas_inner(ty, acc);
            collect_metas_inner(body, acc);
        }
        _ => {}
    }
}

/// Count how many times a meta-variable name appears in a pattern expression.
fn count_meta(expr: &Expr, name: &str) -> usize {
    match expr {
        Expr::Meta(n) if n == name => 1,
        Expr::App(args) => args.iter().map(|a| count_meta(a, name)).sum(),
        Expr::Binder { ty, body, .. } => count_meta(ty, name) + count_meta(body, name),
        _ => 0,
    }
}

/// Count how many times a variable name is referenced in a RHS expression.
fn count_var_in_expr(expr: &Expr, name: &str) -> usize {
    match expr {
        Expr::Meta(n) if n == name => 1,
        Expr::Sym(n) if n == name => 1,
        Expr::App(args) => args.iter().map(|a| count_var_in_expr(a, name)).sum(),
        Expr::Binder { ty, body, .. } => {
            count_var_in_expr(ty, name) + count_var_in_expr(body, name)
        }
        _ => 0,
    }
}

/// Derive a parameter name from a type expression.
/// E.g., `Sym("State")` → "state", `Sym("Event")` → "event".
/// Tracks duplicates: first occurrence gets bare name, subsequent get numbered.
fn derive_param_name(ty: &Expr, seen: &mut HashMap<String, usize>) -> String {
    let base = match ty {
        Expr::Sym(s) => to_snake_case(s),
        _ => "arg".to_string(),
    };
    let count = seen.entry(base.clone()).or_insert(0);
    let name = if *count == 0 {
        base.clone()
    } else {
        format!("{}{}", base, count)
    };
    *count += 1;
    sanitize_keyword(&name)
}

/// Analyze a theory and produce a RustCrate.
pub fn analyze(theory: &Theory) -> RustCrate {
    let sort_names: HashSet<&str> = theory
        .sorts()
        .iter()
        .map(|s| s.name.as_str())
        .filter(|s| !SKIP_SORTS.contains(s))
        .collect();

    // Detect Effect sort and its constructors
    let has_effect_sort = theory.get_sort("Effect").is_some();
    let effect_ctors: HashSet<&str> = if has_effect_sort {
        theory
            .constructors()
            .iter()
            .filter(|c| {
                let (_, ret) = flatten_arrow(&c.ty);
                sort_name(ret) == Some("Effect")
            })
            .map(|c| c.name.as_str())
            .collect()
    } else {
        HashSet::new()
    };

    // Build set of constructor names that are heads of rewrite LHS — these are functions, not variants.
    let rewrite_heads: HashSet<&str> = theory
        .rewrites()
        .iter()
        .filter_map(|rw| head_sym(&rw.lhs))
        .collect();

    // Build constructor → sort mapping (return type of the constructor).
    let mut ctor_sort: HashMap<&str, &str> = HashMap::new();
    // Build constructor → param sorts mapping (sort of each parameter).
    let mut ctor_param_sorts: HashMap<&str, Vec<Option<&str>>> = HashMap::new();
    for c in theory.constructors() {
        let (params, ret) = flatten_arrow(&c.ty);
        if let Some(s) = sort_name(ret) {
            ctor_sort.insert(c.name.as_str(), s);
        }
        let param_sorts: Vec<Option<&str>> = params.iter().map(|p| sort_name(p)).collect();
        ctor_param_sorts.insert(c.name.as_str(), param_sorts);
    }

    // Also skip constructors whose return sort is skipped, or used only for verification
    let skip_ctors: HashSet<&str> = theory
        .constructors()
        .iter()
        .filter(|c| {
            let (_, ret) = flatten_arrow(&c.ty);
            match sort_name(ret) {
                Some(s) => SKIP_SORTS.contains(&s),
                None => true,
            }
        })
        .map(|c| c.name.as_str())
        .collect();

    // ── Build enums ──
    let mut enums: Vec<RustEnum> = Vec::new();
    for sort in theory.sorts() {
        if SKIP_SORTS.contains(&sort.name.as_str()) {
            continue;
        }
        // Effect sort becomes a trait, not an enum
        if has_effect_sort && sort.name == "Effect" {
            continue;
        }

        let variants: Vec<RustVariant> = theory
            .constructors()
            .iter()
            .filter(|c| {
                if rewrite_heads.contains(c.name.as_str()) {
                    return false;
                }
                if skip_ctors.contains(c.name.as_str()) {
                    return false;
                }
                let (_, ret) = flatten_arrow(&c.ty);
                sort_name(ret) == Some(&sort.name)
            })
            .map(|c| {
                let (params, _) = flatten_arrow(&c.ty);
                let fields: Vec<RustField> = params
                    .iter()
                    .map(|p| {
                        let ty = expr_to_rust_type(p, &sort.name, &sort_names);
                        RustField { ty }
                    })
                    .collect();
                RustVariant {
                    name: to_pascal_case(&c.name),
                    fields,
                }
            })
            .collect();

        if !variants.is_empty() {
            enums.push(RustEnum {
                name: to_pascal_case(&sort.name),
                variants,
            });
        }
    }

    // ── Build functions from rewrite rules ──
    // Group rewrites by head symbol
    let mut rewrite_groups: HashMap<&str, Vec<&omega_core::judgment::RewriteRule>> = HashMap::new();
    for rw in theory.rewrites() {
        if let Some(head) = head_sym(&rw.lhs) {
            if skip_ctors.contains(head) {
                continue;
            }
            rewrite_groups.entry(head).or_default().push(rw);
        }
    }

    let mut functions: Vec<RustFunction> = Vec::new();
    for (head, rules) in &rewrite_groups {
        // Look up constructor to get type signature
        let ctor = match theory.get_constructor(head) {
            Some(c) => c,
            None => continue,
        };
        let (param_types, ret_type) = flatten_arrow(&ctor.ty);
        let ret_sort = match sort_name(ret_type) {
            Some(s) => s,
            None => continue,
        };
        if SKIP_SORTS.contains(&ret_sort) {
            continue;
        }

        // Detect if this function returns Effect → effectful function
        let is_effectful = has_effect_sort && ret_sort == "Effect";
        let effects_trait_name = if is_effectful {
            Some(format!("{}Effects", theory.name()))
        } else {
            None
        };

        let ret = if is_effectful {
            RustType::Named("()".to_string())
        } else {
            expr_to_rust_type(ret_type, "", &sort_names)
        };

        // Derive parameter names from types
        let mut name_seen: HashMap<String, usize> = HashMap::new();
        let params: Vec<RustParam> = param_types
            .iter()
            .map(|t| RustParam {
                name: derive_param_name(t, &mut name_seen),
                ty: expr_to_rust_type(t, "", &sort_names),
            })
            .collect();

        // Build match arms
        let arms: Vec<RustMatchArm> = rules
            .iter()
            .map(|rw| {
                build_match_arm(
                    rw,
                    &sort_names,
                    &rewrite_heads,
                    &ctor_sort,
                    &ctor_param_sorts,
                    &effect_ctors,
                )
            })
            .collect();

        // Build body
        let body = if params.is_empty() && arms.len() == 1 {
            // Nullary function (constant alias) — emit body directly, no match
            arms.into_iter().next().unwrap().body
        } else {
            // Add wildcard unreachable arm
            let mut all_arms = arms;
            all_arms.push(RustMatchArm {
                pattern: RustPattern::Wildcard,
                guard: None,
                body: RustExpr::Unreachable,
            });

            // Build scrutinee: single param → direct; multiple → tuple
            let scrutinee = if params.len() == 1 {
                RustExpr::Var(params[0].name.clone())
            } else {
                RustExpr::Tuple(
                    params.iter().map(|p| RustExpr::Var(p.name.clone())).collect(),
                )
            };

            RustExpr::Match {
                scrutinee: Box::new(scrutinee),
                arms: all_arms,
            }
        };

        functions.push(RustFunction {
            name: to_snake_case(head),
            params,
            ret,
            body,
            effects_trait: effects_trait_name,
        });
    }

    // Sort functions by name for deterministic output
    functions.sort_by(|a, b| a.name.cmp(&b.name));

    // ── Build trait (if Effect sort exists) ──
    let mut traits: Vec<RustTrait> = Vec::new();
    if has_effect_sort {
        let methods: Vec<RustMethod> = theory
            .constructors()
            .iter()
            .filter(|c| {
                // Only pure effect constructors (not rewrite heads — those become functions)
                if rewrite_heads.contains(c.name.as_str()) {
                    return false;
                }
                let (_, ret) = flatten_arrow(&c.ty);
                sort_name(ret) == Some("Effect")
            })
            .map(|c| {
                let (params, _) = flatten_arrow(&c.ty);
                let mut name_seen: HashMap<String, usize> = HashMap::new();
                let method_params: Vec<RustParam> = params
                    .iter()
                    .map(|t| RustParam {
                        name: derive_param_name(t, &mut name_seen),
                        ty: expr_to_rust_type(t, "", &sort_names),
                    })
                    .collect();
                RustMethod {
                    name: to_snake_case(&c.name),
                    params: method_params,
                    ret: None,
                }
            })
            .collect();

        if !methods.is_empty() {
            traits.push(RustTrait {
                name: format!("{}Effects", theory.name()),
                methods,
            });
        }
    }

    // ── Assemble single module ──
    let mut items: Vec<RustItem> = enums.into_iter().map(RustItem::Enum).collect();
    for t in traits {
        items.push(RustItem::Trait(t));
    }
    for f in functions {
        items.push(RustItem::Function(f));
    }

    let mut modules = Vec::new();
    if !items.is_empty() {
        modules.push(RustModule {
            name: "omega_generated".to_string(),
            items,
            uses: vec![],
        });
    }

    // Detect if any enum has Box fields (requires box_patterns feature)
    let has_box_patterns = modules.iter().any(|m| {
        m.items.iter().any(|item| match item {
            RustItem::Enum(e) => e
                .variants
                .iter()
                .any(|v| v.fields.iter().any(|f| matches!(f.ty, RustType::Boxed(_)))),
            _ => false,
        })
    });

    RustCrate {
        name: to_snake_case(theory.name()),
        modules,
        has_box_patterns,
    }
}

/// Convert an Omega type expression to a Rust type.
fn expr_to_rust_type(expr: &Expr, self_sort: &str, sort_names: &HashSet<&str>) -> RustType {
    match expr {
        Expr::Sym(s) if sort_names.contains(s.as_str()) => {
            let name = to_pascal_case(s);
            if s == self_sort {
                RustType::Boxed(Box::new(RustType::Named(name)))
            } else {
                RustType::Named(name)
            }
        }
        Expr::Sym(s) => RustType::Named(to_pascal_case(s)),
        _ => RustType::Named("()".to_string()),
    }
}

/// Build a match arm from a rewrite rule.
fn build_match_arm(
    rw: &omega_core::judgment::RewriteRule,
    sort_names: &HashSet<&str>,
    rewrite_heads: &HashSet<&str>,
    ctor_sort: &HashMap<&str, &str>,
    ctor_param_sorts: &HashMap<&str, Vec<Option<&str>>>,
    effect_ctors: &HashSet<&str>,
) -> RustMatchArm {
    // Extract arguments from LHS: (head arg1 arg2 ...)
    let lhs_args = match &rw.lhs {
        Expr::App(args) if args.len() >= 2 => &args[1..],
        _ => &[] as &[Expr],
    };

    // Collect metas in LHS and detect non-linear (repeated) ones
    let lhs_metas = collect_metas_ordered(&rw.lhs);
    let rhs_metas_set: HashSet<String> = rw.rhs.meta_vars().into_iter().collect();
    let unused_metas: HashSet<&str> = lhs_metas
        .iter()
        .filter(|m| !rhs_metas_set.contains(*m))
        .map(|m| m.as_str())
        .collect();
    let mut meta_counts: HashMap<&str, usize> = HashMap::new();
    for m in &lhs_metas {
        let count = count_meta(&rw.lhs, m);
        meta_counts.insert(m.as_str(), count);
    }

    // Track which metas have been bound already (for non-linear patterns)
    let mut bound_metas: HashMap<String, String> = HashMap::new(); // meta_name → rust_var_name
    let mut non_linear_guards: Vec<(String, String)> = Vec::new();

    // Build pattern
    let pattern = if lhs_args.len() == 1 {
        expr_to_pattern(
            &lhs_args[0],
            sort_names,
            rewrite_heads,
            ctor_sort,
            ctor_param_sorts,
            &meta_counts,
            &unused_metas,
            &mut bound_metas,
            &mut non_linear_guards,
        )
    } else {
        let pats: Vec<RustPattern> = lhs_args
            .iter()
            .map(|a| {
                expr_to_pattern(
                    a,
                    sort_names,
                    rewrite_heads,
                    ctor_sort,
                    ctor_param_sorts,
                    &meta_counts,
                    &unused_metas,
                    &mut bound_metas,
                    &mut non_linear_guards,
                )
            })
            .collect();
        RustPattern::Tuple(pats)
    };

    // Guard for non-linear patterns
    let guard = if non_linear_guards.is_empty() {
        None
    } else {
        let parts: Vec<String> = non_linear_guards
            .iter()
            .map(|(a, b)| format!("{} == {}", a, b))
            .collect();
        Some(parts.join(" && "))
    };

    // Compute how many times each meta is used in the RHS
    let mut rhs_usage: HashMap<String, usize> = HashMap::new();
    for m in bound_metas.keys() {
        rhs_usage.insert(m.clone(), count_var_in_expr(&rw.rhs, m));
    }

    // Track remaining uses for clone insertion
    let mut remaining_uses: HashMap<String, usize> = rhs_usage.clone();

    let body = expr_to_rust_expr(
        &rw.rhs,
        sort_names,
        rewrite_heads,
        ctor_sort,
        ctor_param_sorts,
        &bound_metas,
        &rhs_usage,
        &mut remaining_uses,
        effect_ctors,
    );

    RustMatchArm {
        pattern,
        guard,
        body,
    }
}

/// Convert an Omega LHS pattern to a Rust pattern.
fn expr_to_pattern(
    expr: &Expr,
    sort_names: &HashSet<&str>,
    rewrite_heads: &HashSet<&str>,
    ctor_sort: &HashMap<&str, &str>,
    ctor_param_sorts: &HashMap<&str, Vec<Option<&str>>>,
    meta_counts: &HashMap<&str, usize>,
    unused_metas: &HashSet<&str>,
    bound_metas: &mut HashMap<String, String>,
    non_linear_guards: &mut Vec<(String, String)>,
) -> RustPattern {
    match expr {
        Expr::Meta(n) => {
            // If this meta is not used in the RHS, bind as wildcard
            if unused_metas.contains(n.as_str()) {
                return RustPattern::Wildcard;
            }
            let count = meta_counts.get(n.as_str()).copied().unwrap_or(1);
            let var_name = to_snake_case(n);
            if count > 1 {
                if let Some(first_name) = bound_metas.get(n) {
                    // Second (or later) occurrence — bind a new name and add guard
                    let new_name = format!("{}_1", var_name);
                    non_linear_guards.push((first_name.clone(), new_name.clone()));
                    RustPattern::Var(new_name)
                } else {
                    bound_metas.insert(n.clone(), var_name.clone());
                    RustPattern::Var(var_name)
                }
            } else {
                bound_metas.insert(n.clone(), var_name.clone());
                RustPattern::Var(var_name)
            }
        }
        Expr::Sym(s) => {
            // A nullary constructor
            if let Some(sort) = ctor_sort.get(s.as_str()) {
                if !SKIP_SORTS.contains(sort) && !rewrite_heads.contains(s.as_str()) {
                    return RustPattern::Constructor {
                        enum_name: to_pascal_case(sort),
                        variant: to_pascal_case(s),
                        fields: vec![],
                    };
                }
            }
            // Fallback: treat as variable
            RustPattern::Var(to_snake_case(s))
        }
        Expr::App(args) if args.len() >= 2 => {
            if let Expr::Sym(head) = &args[0] {
                if let Some(sort) = ctor_sort.get(head.as_str()) {
                    if !SKIP_SORTS.contains(sort) && !rewrite_heads.contains(head.as_str()) {
                        // Constructor pattern — check which params are boxed (recursive)
                        let param_sorts = ctor_param_sorts.get(head.as_str());
                        let self_sort = *sort;
                        let fields: Vec<RustPattern> = args[1..]
                            .iter()
                            .enumerate()
                            .map(|(i, a)| {
                                let is_boxed = param_sorts
                                    .and_then(|ps| ps.get(i))
                                    .and_then(|s| *s)
                                    .map(|s| s == self_sort)
                                    .unwrap_or(false);
                                let inner = expr_to_pattern(
                                    a,
                                    sort_names,
                                    rewrite_heads,
                                    ctor_sort,
                                    ctor_param_sorts,
                                    meta_counts,
                                    unused_metas,
                                    bound_metas,
                                    non_linear_guards,
                                );
                                if is_boxed {
                                    RustPattern::Box(Box::new(inner))
                                } else {
                                    inner
                                }
                            })
                            .collect();
                        return RustPattern::Constructor {
                            enum_name: to_pascal_case(sort),
                            variant: to_pascal_case(head),
                            fields,
                        };
                    }
                }
            }
            // Unknown application — treat inner parts as nested
            RustPattern::Wildcard
        }
        _ => RustPattern::Wildcard,
    }
}


/// Convert an Omega RHS expression to a Rust expression.
fn expr_to_rust_expr(
    expr: &Expr,
    sort_names: &HashSet<&str>,
    rewrite_heads: &HashSet<&str>,
    ctor_sort: &HashMap<&str, &str>,
    ctor_param_sorts: &HashMap<&str, Vec<Option<&str>>>,
    bound_metas: &HashMap<String, String>,
    rhs_usage: &HashMap<String, usize>,
    remaining_uses: &mut HashMap<String, usize>,
    effect_ctors: &HashSet<&str>,
) -> RustExpr {
    match expr {
        Expr::Meta(n) => {
            let var_name = bound_metas
                .get(n)
                .cloned()
                .unwrap_or_else(|| to_snake_case(n));
            let total = rhs_usage.get(n).copied().unwrap_or(1);
            let remaining = remaining_uses.get(n).copied().unwrap_or(1);

            // Decrement remaining uses
            if remaining > 0 {
                remaining_uses.insert(n.clone(), remaining - 1);
            }

            if total > 1 && remaining > 1 {
                // Not the last use — clone it
                RustExpr::Clone(Box::new(RustExpr::Var(var_name)))
            } else {
                RustExpr::Var(var_name)
            }
        }
        Expr::Sym(s) => {
            // Effect constructor (nullary) → method call on effects
            if effect_ctors.contains(s.as_str()) {
                return RustExpr::MethodCall {
                    receiver: "effects".to_string(),
                    method: to_snake_case(s),
                    args: vec![],
                };
            }
            // Nullary constructor or constant
            if let Some(sort) = ctor_sort.get(s.as_str()) {
                if !SKIP_SORTS.contains(sort) && !rewrite_heads.contains(s.as_str()) {
                    return RustExpr::Constructor {
                        enum_name: to_pascal_case(sort),
                        variant: to_pascal_case(s),
                        args: vec![],
                    };
                }
            }
            RustExpr::Var(to_snake_case(s))
        }
        Expr::App(args) if args.len() >= 2 => {
            if let Expr::Sym(head) = &args[0] {
                let child_args: Vec<RustExpr> = args[1..]
                    .iter()
                    .map(|a| {
                        expr_to_rust_expr(
                            a,
                            sort_names,
                            rewrite_heads,
                            ctor_sort,
                            ctor_param_sorts,
                            bound_metas,
                            rhs_usage,
                            remaining_uses,
                            effect_ctors,
                        )
                    })
                    .collect();

                // Effect constructor (with args) → method call on effects
                if effect_ctors.contains(head.as_str()) {
                    return RustExpr::MethodCall {
                        receiver: "effects".to_string(),
                        method: to_snake_case(head),
                        args: child_args,
                    };
                }

                if rewrite_heads.contains(head.as_str()) {
                    // This is a function call
                    return RustExpr::Call {
                        func: to_snake_case(head),
                        args: child_args,
                    };
                }

                if let Some(sort) = ctor_sort.get(head.as_str()) {
                    if !SKIP_SORTS.contains(sort) {
                        // Check which fields are recursive via ctor_param_sorts
                        let self_sort = *sort;
                        let param_sorts = ctor_param_sorts.get(head.as_str());
                        let boxed_args: Vec<RustExpr> = child_args
                            .into_iter()
                            .enumerate()
                            .map(|(i, rust_expr)| {
                                let is_recursive = param_sorts
                                    .and_then(|ps| ps.get(i))
                                    .and_then(|s| *s)
                                    .map(|s| s == self_sort)
                                    .unwrap_or(false);
                                if is_recursive {
                                    RustExpr::BoxNew(Box::new(rust_expr))
                                } else {
                                    rust_expr
                                }
                            })
                            .collect();

                        return RustExpr::Constructor {
                            enum_name: to_pascal_case(sort),
                            variant: to_pascal_case(head),
                            args: boxed_args,
                        };
                    }
                }

                // Unknown function call
                return RustExpr::Call {
                    func: to_snake_case(head),
                    args: child_args,
                };
            }
            RustExpr::Unreachable
        }
        _ => RustExpr::Unreachable,
    }
}

