/// Transform a VonNeumannTheory into a RustCrate.
use std::collections::{HashMap, HashSet};

use apeiron::parser::Sexp;

use super::rust_ast::*;
use crate::error::{HyperionError, Result};
use crate::session::VonNeumannTheory;

/// Sorts that are handled specially and should not generate enums.
const SKIP_SORTS: &[&str] = &["Effect", "Prop", "String", "Unit"];

/// Analyze a VN theory and produce a RustCrate.
pub fn analyze(theory: &VonNeumannTheory) -> Result<RustCrate> {
    let crate_name = to_snake_case(&theory.name);

    // Detect Effect sort and its constructors
    let has_effect_sort = theory.sorts.contains(&"Effect".to_string());

    // Collect effect constructors: operators whose codomain is Effect
    let mut effect_ops: HashSet<String> = HashSet::new();
    if has_effect_sort {
        for (op_name, (_, codomain)) in &theory.morphism_types {
            if codomain == "Effect" {
                effect_ops.insert(op_name.clone());
            }
        }
    }

    // Check for Tuple sort (used for linear resource threading)
    let has_tuple_sort = theory.sorts.contains(&"Tuple".to_string());

    // Find the tuple constructor's domain to determine the tuple field types
    let tuple_fields: Vec<String> = if has_tuple_sort {
        ["pair", "tuple", "mk-tuple"]
            .iter()
            .find_map(|name| theory.morphism_types.get(*name).map(|(d, _)| d.clone()))
            .unwrap_or_default()
    } else {
        vec![]
    };

    // Step 1: Detect rewrite heads — these become functions, not enum variants
    let mut rewrite_heads: HashSet<String> = HashSet::new();
    for rule in &theory.rules {
        if let Some(head) = sexp_head(&rule.lhs) {
            rewrite_heads.insert(head.to_string());
        }
    }

    // Step 2: Build sort dependency graph for recursive boxing
    let recursive_sorts = find_recursive_sorts(&theory.morphism_types);

    // Step 3: Group operators by codomain → each sort becomes an enum
    let mut sort_variants: HashMap<String, Vec<(String, Vec<String>)>> = HashMap::new();
    for sort in &theory.sorts {
        if !SKIP_SORTS.contains(&sort.as_str()) && !(has_tuple_sort && sort == "Tuple") {
            sort_variants.entry(sort.clone()).or_default();
        }
    }

    for (op_name, (domain, codomain)) in &theory.morphism_types {
        if rewrite_heads.contains(op_name) || effect_ops.contains(op_name) {
            continue; // functions or effect methods, not variants
        }
        if SKIP_SORTS.contains(&codomain.as_str()) {
            continue;
        }
        // For Tuple constructors, skip — they'll be handled as native tuples
        if has_tuple_sort && codomain == "Tuple" {
            continue;
        }
        sort_variants
            .entry(codomain.clone())
            .or_default()
            .push((op_name.clone(), domain.clone()));
    }

    // Step 4: Build enums
    let mut type_items: Vec<RustItem> = Vec::new();
    let mut has_box_patterns = false;

    for (sort_name, variants) in &sort_variants {
        if variants.is_empty() {
            continue; // Skip sorts with no constructors
        }
        let enum_name = to_pascal_case(sort_name);
        let mut rust_variants = Vec::new();

        for (op_name, domain) in variants {
            let variant_name = to_pascal_case(op_name);
            let fields: Vec<RustField> = domain
                .iter()
                .map(|d| {
                    let base_ty = sort_to_type(d, has_tuple_sort, &tuple_fields);
                    if recursive_sorts.contains(d) {
                        RustField {
                            ty: RustType::Boxed(Box::new(base_ty)),
                        }
                    } else {
                        RustField { ty: base_ty }
                    }
                })
                .collect();

            rust_variants.push(RustVariant {
                name: variant_name,
                fields,
            });
        }

        type_items.push(RustItem::Enum(RustEnum {
            name: enum_name,
            variants: rust_variants,
        }));
    }

    // Step 5: Build trait from Effect sort (if present)
    let trait_name = if has_effect_sort {
        Some(format!("{}Effects", to_pascal_case(&theory.name)))
    } else {
        None
    };

    if has_effect_sort {
        let mut methods = Vec::new();
        for op_name in &effect_ops {
            if let Some((domain, _)) = theory.morphism_types.get(op_name) {
                let method_name = to_snake_case(op_name);
                let params: Vec<RustParam> = domain
                    .iter()
                    .enumerate()
                    .map(|(i, d)| RustParam {
                        name: derive_param_name(d, i),
                        ty: sort_to_type(d, has_tuple_sort, &tuple_fields),
                    })
                    .collect();
                methods.push(RustTraitMethod {
                    name: method_name,
                    params,
                    ret: RustType::Unit,
                });
            }
        }
        type_items.push(RustItem::Trait(RustTrait {
            name: trait_name.clone().unwrap(),
            methods,
        }));
    }

    // Step 5.5: Physics validation — reject nested function calls in LHS patterns.
    // Von Neumann substrates use strict (call-by-value) evaluation: inner calls
    // collapse to opaque values before the outer call sees them. You cannot
    // pattern-match on the history of an evaluated function call.
    for rule in &theory.rules {
        validate_lhs_physics(&rule.lhs, &rule.name, &rewrite_heads, true)?;
    }

    // Step 6: Group rewrite rules by head → match functions
    let mut func_items: Vec<RustItem> = Vec::new();
    let mut rules_by_head: HashMap<String, Vec<&crate::session::VonNeumannRule>> = HashMap::new();
    for rule in &theory.rules {
        if let Some(head) = sexp_head(&rule.lhs) {
            rules_by_head
                .entry(head.to_string())
                .or_default()
                .push(rule);
        }
    }

    for (head, rules) in &rules_by_head {
        let func_name = to_snake_case(head);

        // Determine parameter types from morphism_types
        let (domain, codomain) = theory.morphism_types.get(head).ok_or_else(|| {
            HyperionError::ParseError {
                block: "analyze".into(),
                detail: format!("no morphism type info for '{}'", head),
            }
        })?;

        let ret_type = sort_to_type(codomain, has_tuple_sort, &tuple_fields);

        // Check if this function returns Effect → effectful function
        let is_effectful = has_effect_sort && codomain == "Effect";

        // Build params
        let params: Vec<RustParam> = domain
            .iter()
            .enumerate()
            .map(|(i, d)| RustParam {
                name: derive_param_name(d, i),
                ty: sort_to_type(d, has_tuple_sort, &tuple_fields),
            })
            .collect();

        // Build match arms from rules
        let mut arms: Vec<RustMatchArm> = Vec::new();

        for rule in rules {
            let lhs_args = sexp_args(&rule.lhs);

            // Collect meta-variables for non-linear detection
            let mut meta_counts: HashMap<String, usize> = HashMap::new();
            for arg in &lhs_args {
                count_metas(arg, &mut meta_counts);
            }

            // Track rename map for duplicate metas
            let mut rename_map: HashMap<String, Vec<String>> = HashMap::new();
            for (meta, count) in &meta_counts {
                if *count > 1 {
                    let names: Vec<String> = (0..*count)
                        .map(|i| {
                            if i == 0 {
                                meta.clone()
                            } else {
                                format!("{}_{}", meta, i)
                            }
                        })
                        .collect();
                    rename_map.insert(meta.clone(), names);
                }
            }

            // Build match pattern (one per param)
            let patterns: Vec<RustPattern> = if lhs_args.len() == params.len() {
                lhs_args
                    .iter()
                    .zip(domain.iter())
                    .map(|(arg, sort)| {
                        sexp_to_pattern(arg, sort, &theory.morphism_types, &recursive_sorts, &mut rename_map.clone(), has_tuple_sort)
                    })
                    .collect()
            } else {
                // Fallback: wildcard all
                params.iter().map(|_| RustPattern::Wildcard).collect()
            };

            // Build guard for non-linear patterns
            let guard = build_guard(&rename_map);

            // Count RHS meta usages for clone insertion
            let mut rhs_meta_counts: HashMap<String, usize> = HashMap::new();
            count_metas(&rule.rhs, &mut rhs_meta_counts);
            let mut rhs_meta_used: HashMap<String, usize> = HashMap::new();

            let body = sexp_to_expr(
                &rule.rhs,
                &theory.morphism_types,
                &recursive_sorts,
                &rewrite_heads,
                &effect_ops,
                &rhs_meta_counts,
                &mut rhs_meta_used,
                has_tuple_sort,
            );

            // Build the scrutinee pattern
            let pattern = if patterns.len() == 1 {
                patterns.into_iter().next().unwrap()
            } else {
                // Tuple pattern for multi-arg functions
                RustPattern::TuplePattern(patterns)
            };

            arms.push(RustMatchArm {
                pattern,
                guard,
                body,
            });
        }

        // Add unreachable wildcard arm if needed
        arms.push(RustMatchArm {
            pattern: RustPattern::Wildcard,
            guard: None,
            body: RustExpr::Unreachable,
        });

        // Build scrutinee
        let scrutinee = if params.len() == 1 {
            Box::new(RustExpr::Var(params[0].name.clone()))
        } else {
            Box::new(RustExpr::TupleExpr(
                params.iter().map(|p| RustExpr::Var(p.name.clone())).collect(),
            ))
        };

        // Check if any pattern uses box
        for arm in &arms {
            if pattern_has_box(&arm.pattern) {
                has_box_patterns = true;
            }
        }

        let body = RustExpr::Match { scrutinee, arms };

        func_items.push(RustItem::Function(RustFunction {
            name: func_name,
            params,
            ret: if is_effectful { RustType::Unit } else { ret_type },
            body,
            effects_trait: if is_effectful { trait_name.clone() } else { None },
        }));
    }

    // Build modules
    let mut modules = Vec::new();

    if !type_items.is_empty() {
        modules.push(RustModule {
            name: "types".into(),
            items: type_items,
            uses: vec![],
        });
    }

    if !func_items.is_empty() {
        let uses = if modules.iter().any(|m| m.name == "types") {
            vec!["super::types::*".into()]
        } else {
            vec![]
        };
        modules.push(RustModule {
            name: "functions".into(),
            items: func_items,
            uses,
        });
    }

    Ok(RustCrate {
        name: crate_name,
        modules,
        has_box_patterns,
    })
}

/// Convert a sort name to a RustType, handling Tuple and Unit specially.
fn sort_to_type(sort: &str, has_tuple_sort: bool, tuple_fields: &[String]) -> RustType {
    if sort == "Unit" {
        RustType::Unit
    } else if has_tuple_sort && sort == "Tuple" && !tuple_fields.is_empty() {
        RustType::Tuple(
            tuple_fields
                .iter()
                .map(|s| sort_to_type(s, false, &[]))
                .collect(),
        )
    } else {
        RustType::Named(to_pascal_case(sort))
    }
}

/// Get the head symbol of an S-expression (if it's a list).
fn sexp_head(sexp: &Sexp) -> Option<&str> {
    match sexp {
        Sexp::List(items, _) if !items.is_empty() => items[0].as_atom(),
        _ => None,
    }
}

/// Get the arguments of an S-expression (tail of list).
fn sexp_args(sexp: &Sexp) -> Vec<&Sexp> {
    match sexp {
        Sexp::List(items, _) if items.len() > 1 => items[1..].iter().collect(),
        _ => vec![],
    }
}

/// Count meta-variable occurrences in an S-expression.
fn count_metas(sexp: &Sexp, counts: &mut HashMap<String, usize>) {
    match sexp {
        Sexp::Atom(name, _) => {
            if let Some(meta) = name.strip_prefix('?') {
                *counts.entry(meta.to_string()).or_insert(0) += 1;
            }
        }
        Sexp::List(items, _) => {
            for item in items {
                count_metas(item, counts);
            }
        }
    }
}

/// Convert an S-expression to a Rust pattern.
fn sexp_to_pattern(
    sexp: &Sexp,
    sort: &str,
    morphism_types: &HashMap<String, (Vec<String>, String)>,
    recursive_sorts: &HashSet<String>,
    rename_map: &mut HashMap<String, Vec<String>>,
    has_tuple_sort: bool,
) -> RustPattern {
    match sexp {
        Sexp::Atom(name, _) => {
            if let Some(meta) = name.strip_prefix('?') {
                // Check if this meta has been renamed (non-linear)
                if let Some(names) = rename_map.get_mut(meta) {
                    if let Some(n) = names.first().cloned() {
                        names.remove(0);
                        RustPattern::Var(n)
                    } else {
                        RustPattern::Var(meta.to_string())
                    }
                } else {
                    RustPattern::Var(meta.to_string())
                }
            } else {
                // Nullary constructor
                let enum_name = to_pascal_case(sort);
                let variant = to_pascal_case(name);
                RustPattern::Constructor {
                    enum_name,
                    variant,
                    fields: vec![],
                }
            }
        }
        Sexp::List(items, _) if !items.is_empty() => {
            let head = items[0].as_atom().unwrap_or("");

            // Check if this is a tuple constructor
            if has_tuple_sort && (head == "pair" || head == "tuple" || head == "mk-tuple") {
                let child_sorts: Vec<String> = morphism_types
                    .get(head)
                    .map(|(d, _)| d.clone())
                    .unwrap_or_default();
                let fields: Vec<RustPattern> = items[1..]
                    .iter()
                    .enumerate()
                    .map(|(i, arg)| {
                        let child_sort = child_sorts.get(i).map(|s| s.as_str()).unwrap_or(sort);
                        sexp_to_pattern(arg, child_sort, morphism_types, recursive_sorts, rename_map, has_tuple_sort)
                    })
                    .collect();
                return RustPattern::TuplePattern(fields);
            }

            let enum_name = to_pascal_case(sort);
            let variant = to_pascal_case(head);

            // Get domain types for this constructor
            let child_sorts: Vec<String> = morphism_types
                .get(head)
                .map(|(d, _)| d.clone())
                .unwrap_or_default();

            let fields: Vec<RustPattern> = items[1..]
                .iter()
                .enumerate()
                .map(|(i, arg)| {
                    let child_sort = child_sorts.get(i).map(|s| s.as_str()).unwrap_or(sort);
                    let inner =
                        sexp_to_pattern(arg, child_sort, morphism_types, recursive_sorts, rename_map, has_tuple_sort);
                    if recursive_sorts.contains(child_sort) {
                        RustPattern::Box(Box::new(inner))
                    } else {
                        inner
                    }
                })
                .collect();

            RustPattern::Constructor {
                enum_name,
                variant,
                fields,
            }
        }
        _ => RustPattern::Wildcard,
    }
}

/// Convert an S-expression to a Rust expression.
fn sexp_to_expr(
    sexp: &Sexp,
    morphism_types: &HashMap<String, (Vec<String>, String)>,
    recursive_sorts: &HashSet<String>,
    rewrite_heads: &HashSet<String>,
    effect_ops: &HashSet<String>,
    rhs_meta_counts: &HashMap<String, usize>,
    rhs_meta_used: &mut HashMap<String, usize>,
    has_tuple_sort: bool,
) -> RustExpr {
    match sexp {
        Sexp::Atom(name, _) => {
            if let Some(meta) = name.strip_prefix('?') {
                let total = rhs_meta_counts.get(meta).copied().unwrap_or(1);
                let used = rhs_meta_used.entry(meta.to_string()).or_insert(0);
                *used += 1;
                if *used < total {
                    RustExpr::Clone(Box::new(RustExpr::Var(meta.to_string())))
                } else {
                    RustExpr::Var(meta.to_string())
                }
            } else if name == "unit" || name == "done" {
                // Unit value
                RustExpr::TupleExpr(vec![])
            } else {
                // Nullary constructor — need to figure out which enum
                let enum_name = morphism_types
                    .get(name.as_str())
                    .map(|(_, c)| to_pascal_case(c))
                    .unwrap_or_else(|| "Unknown".into());
                let variant = to_pascal_case(name);
                RustExpr::Constructor {
                    enum_name,
                    variant,
                    args: vec![],
                }
            }
        }
        Sexp::List(items, _) if !items.is_empty() => {
            let head = items[0].as_atom().unwrap_or("");

            // Effect constructor → method call on effects trait
            if effect_ops.contains(head) {
                let args: Vec<RustExpr> = items[1..]
                    .iter()
                    .map(|arg| {
                        sexp_to_expr(arg, morphism_types, recursive_sorts, rewrite_heads, effect_ops, rhs_meta_counts, rhs_meta_used, has_tuple_sort)
                    })
                    .collect();
                return RustExpr::EffectCall {
                    method: to_snake_case(head),
                    args,
                };
            }

            // Tuple constructor → tuple expression
            if has_tuple_sort && (head == "pair" || head == "tuple" || head == "mk-tuple") {
                let elems: Vec<RustExpr> = items[1..]
                    .iter()
                    .map(|arg| {
                        sexp_to_expr(arg, morphism_types, recursive_sorts, rewrite_heads, effect_ops, rhs_meta_counts, rhs_meta_used, has_tuple_sort)
                    })
                    .collect();
                return RustExpr::TupleExpr(elems);
            }

            let args: Vec<RustExpr> = items[1..]
                .iter()
                .enumerate()
                .map(|(i, arg)| {
                    let child_sort = morphism_types
                        .get(head)
                        .and_then(|(d, _)| d.get(i))
                        .map(|s| s.as_str())
                        .unwrap_or("");
                    let inner = sexp_to_expr(
                        arg,
                        morphism_types,
                        recursive_sorts,
                        rewrite_heads,
                        effect_ops,
                        rhs_meta_counts,
                        rhs_meta_used,
                        has_tuple_sort,
                    );
                    if recursive_sorts.contains(child_sort) {
                        RustExpr::BoxNew(Box::new(inner))
                    } else {
                        inner
                    }
                })
                .collect();

            if rewrite_heads.contains(head) {
                // Function call — args are NOT boxed (function params are the sort type directly)
                let func_args: Vec<RustExpr> = items[1..]
                    .iter()
                    .map(|arg| {
                        sexp_to_expr(
                            arg,
                            morphism_types,
                            recursive_sorts,
                            rewrite_heads,
                            effect_ops,
                            rhs_meta_counts,
                            rhs_meta_used,
                            has_tuple_sort,
                        )
                    })
                    .collect();
                RustExpr::Call {
                    func: to_snake_case(head),
                    args: func_args,
                }
            } else {
                // Constructor
                let enum_name = morphism_types
                    .get(head)
                    .map(|(_, c)| to_pascal_case(c))
                    .unwrap_or_else(|| "Unknown".into());
                let variant = to_pascal_case(head);
                RustExpr::Constructor {
                    enum_name,
                    variant,
                    args,
                }
            }
        }
        _ => RustExpr::Unreachable,
    }
}

/// Build a match guard for non-linear patterns (e.g., `if n == n_1`).
fn build_guard(rename_map: &HashMap<String, Vec<String>>) -> Option<String> {
    let mut guards = Vec::new();
    for (base, names) in rename_map {
        for name in names.iter().skip(1) {
            if name != base {
                guards.push(format!("{} == {}", base, name));
            }
        }
    }
    if guards.is_empty() {
        None
    } else {
        Some(guards.join(" && "))
    }
}

/// Check if a pattern contains box patterns.
fn pattern_has_box(pat: &RustPattern) -> bool {
    match pat {
        RustPattern::Box(_) => true,
        RustPattern::Constructor { fields, .. } => fields.iter().any(pattern_has_box),
        RustPattern::TuplePattern(elems) => elems.iter().any(pattern_has_box),
        _ => false,
    }
}

/// Convert a name to PascalCase.
pub fn to_pascal_case(s: &str) -> String {
    s.split(|c: char| c == '-' || c == '_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    upper + &chars.as_str()
                }
                None => String::new(),
            }
        })
        .collect()
}

/// Convert a name to snake_case.
pub fn to_snake_case(s: &str) -> String {
    s.replace('-', "_")
}

/// Derive a parameter name from a sort name and index.
fn derive_param_name(sort: &str, idx: usize) -> String {
    let base = sort.to_lowercase();
    if idx == 0 {
        base
    } else {
        format!("{}_{}", base, idx + 1)
    }
}

/// Find recursive sorts by building a dependency graph.
fn find_recursive_sorts(
    morphism_types: &HashMap<String, (Vec<String>, String)>,
) -> HashSet<String> {
    // Build adjacency: codomain → set of domain sorts
    let mut edges: HashSet<(String, String)> = HashSet::new();
    for (_, (domain, codomain)) in morphism_types {
        for d in domain {
            edges.insert((codomain.clone(), d.clone()));
        }
    }

    // Find self-loops and cycles via simple reachability
    let mut recursive = HashSet::new();
    let nodes: HashSet<String> = edges.iter().flat_map(|(a, b)| vec![a.clone(), b.clone()]).collect();

    for node in &nodes {
        if can_reach(&edges, node, node, &mut HashSet::new()) {
            recursive.insert(node.clone());
        }
    }

    recursive
}

/// Check if `from` can reach `target` in the edge graph.
fn can_reach(
    edges: &HashSet<(String, String)>,
    from: &str,
    target: &str,
    visited: &mut HashSet<String>,
) -> bool {
    if visited.contains(from) {
        return false;
    }
    visited.insert(from.to_string());

    for (a, b) in edges {
        if a == from {
            if b == target {
                return true;
            }
            if can_reach(edges, b, target, visited) {
                return true;
            }
        }
    }
    false
}

/// Validate that LHS patterns don't contain nested function calls (rewrite heads).
/// In Von Neumann physics, strict evaluation collapses inner calls to opaque values
/// before the outer call executes — values don't remember their computational history.
/// The `is_root` flag allows the outermost head (which IS the function being defined).
fn validate_lhs_physics(
    sexp: &Sexp,
    rule_name: &str,
    rewrite_heads: &HashSet<String>,
    is_root: bool,
) -> Result<()> {
    match sexp {
        Sexp::List(items, _) if !items.is_empty() => {
            if let Some(head) = items[0].as_atom() {
                if !is_root && rewrite_heads.contains(head) {
                    return Err(HyperionError::ParseError {
                        block: "kompile".into(),
                        detail: format!(
                            "Physics mismatch in rule '{}': substrate uses strict evaluation \
                             and cannot pattern-match on the history of evaluated function calls \
                             (found '{}' nested in LHS pattern). Use data constructors for matching, \
                             or switch to a graph-reduction substrate.",
                            rule_name, head
                        ),
                    });
                }
            }
            for arg in &items[1..] {
                validate_lhs_physics(arg, rule_name, rewrite_heads, false)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("nat"), "Nat");
        assert_eq!(to_pascal_case("plus"), "Plus");
        assert_eq!(to_pascal_case("my-type"), "MyType");
        assert_eq!(to_pascal_case("some_thing"), "SomeThing");
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("plus"), "plus");
        assert_eq!(to_snake_case("plus-s"), "plus_s");
    }

    #[test]
    fn test_find_recursive_sorts() {
        let mut mt = HashMap::new();
        // s: Nat -> Nat (self-recursive)
        mt.insert("s".to_string(), (vec!["Nat".to_string()], "Nat".to_string()));
        // z: [] -> Nat
        mt.insert("z".to_string(), (vec![], "Nat".to_string()));

        let recursive = find_recursive_sorts(&mt);
        assert!(recursive.contains("Nat"));
    }
}
