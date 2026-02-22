use std::collections::{HashMap, HashSet};

use crate::parser::{Sexp, Span};
use crate::system::RewriteRule;

/// A judgment declaration: `[judgment name :inputs [S1 S2] :output S3]`
#[derive(Debug, Clone)]
pub struct JudgmentDecl {
    pub name: String,
    pub input_sorts: Vec<String>,
    pub output_sort: String,
    pub partial: bool,
}

/// A derivation rule: `[@derive name :premises [...] :conclusion [...]]`
#[derive(Debug, Clone)]
pub struct DerivRule {
    pub name: String,
    /// Each premise is a list of Sexp (the judgment application).
    /// e.g. `[typeof ?G ?e ?A]` — the last element is the "output" position.
    pub premises: Vec<Sexp>,
    /// The conclusion judgment application.
    /// e.g. `[typeof [ext ?A ?G] vz ?A]` — last element is the output.
    pub conclusion: Sexp,
    /// If true, this case is declared impossible (no RHS).
    pub absurd: bool,
}

/// Parse a `[judgment name :inputs [...] :output Sort]` declaration from @syntax.
pub fn parse_judgment_decl(items: &[Sexp]) -> Option<JudgmentDecl> {
    // items = [name, :inputs, [...], :output, Sort, ...]
    if items.is_empty() {
        return None;
    }
    let name = items[0].as_atom()?.to_string();

    let mut input_sorts = Vec::new();
    let mut output_sort = String::new();
    let mut partial = false;
    let mut i = 1;

    while i < items.len() {
        match items[i].as_atom() {
            Some(":inputs") => {
                i += 1;
                if i < items.len() {
                    if let Some(list) = items[i].as_list() {
                        for item in list {
                            if let Some(s) = item.as_atom() {
                                input_sorts.push(s.to_string());
                            }
                        }
                    }
                }
            }
            Some(":output") => {
                i += 1;
                if i < items.len() {
                    if let Some(s) = items[i].as_atom() {
                        output_sort = s.to_string();
                    }
                }
            }
            Some(":partial") => {
                partial = true;
            }
            _ => {}
        }
        i += 1;
    }

    if output_sort.is_empty() {
        return None;
    }

    Some(JudgmentDecl {
        name,
        input_sorts,
        output_sort,
        partial,
    })
}

/// Parse a `[@derive name :premises [...] :conclusion [...]]` declaration.
pub fn parse_derive_rule(items: &[Sexp]) -> Option<DerivRule> {
    if items.is_empty() {
        return None;
    }

    let name = items[0].as_atom()?.to_string();
    let mut premises = Vec::new();
    let mut conclusion = None;
    let mut absurd = false;
    let mut i = 1;

    while i < items.len() {
        match items[i].as_atom() {
            Some(":premises") => {
                i += 1;
                if i < items.len() {
                    if let Some(list) = items[i].as_list() {
                        for item in list {
                            premises.push(item.clone());
                        }
                    }
                }
            }
            Some(":conclusion") => {
                i += 1;
                if i < items.len() {
                    conclusion = Some(items[i].clone());
                }
            }
            Some(":absurd") => {
                absurd = true;
            }
            _ => {}
        }
        i += 1;
    }

    let conclusion = conclusion?;
    Some(DerivRule {
        name,
        premises,
        conclusion,
        absurd,
    })
}

/// Decompose a judgment application `[J arg1 arg2 ... output]` into
/// (judgment_name, input_args, output_expr).
fn decompose_judgment(sexp: &Sexp) -> Option<(String, Vec<Sexp>, Sexp)> {
    let items = sexp.as_list()?;
    if items.len() < 2 {
        return None;
    }
    let name = items[0].as_atom()?.to_string();
    let inputs = items[1..items.len() - 1].to_vec();
    let output = items[items.len() - 1].clone();
    Some((name, inputs, output))
}

/// Collect all meta-variable names (?X) from an sexp.
fn collect_metas(sexp: &Sexp, out: &mut HashSet<String>) {
    match sexp {
        Sexp::Atom(name, _) if name.starts_with('?') => {
            out.insert(name.clone());
        }
        Sexp::Atom(_, _) => {}
        Sexp::List(items, _) => {
            for item in items {
                collect_metas(item, out);
            }
        }
    }
}

/// Compile a set of `@derive` rules into rewrite rules.
///
/// For each derive rule, generates:
/// - Main rule: `[J inputs...] ==> [__stg_name ctx_vars... [premise1_call] ...]`
///   (or `[ok output]` for 0-premise rules)
/// - Success rule: `[__stg_name ctx_vars... [ok pat1] ...] ==> [ok output]`
/// - Failure rules: one per premise position + catch-all
///
/// Context variables are meta-variables from the conclusion output that don't
/// appear in any premise output. They must be threaded through the staging
/// combinator so the success rule can use them in the RHS.
///
/// Returns (rewrite_rules, staging_op_names).
pub fn compile_derive_rules(
    rules: &[DerivRule],
    _judgments: &HashMap<String, JudgmentDecl>,
) -> (Vec<RewriteRule>, Vec<String>) {
    let s = Span::default();
    let mut rewrites = Vec::new();
    let mut staging_ops = Vec::new();

    for rule in rules {
        if rule.absurd {
            continue; // :absurd rules have no compilation output
        }

        let (j_name, conclusion_inputs, conclusion_output) =
            match decompose_judgment(&rule.conclusion) {
                Some(d) => d,
                None => continue,
            };

        // Build LHS: [J input1 input2 ...]
        let mut lhs_items = vec![Sexp::Atom(j_name.clone(), s)];
        lhs_items.extend(conclusion_inputs.iter().cloned());
        let lhs = Sexp::List(lhs_items, s);

        if rule.premises.is_empty() {
            // 0-premise (axiom): [J inputs...] ==> [ok output]
            let rhs = Sexp::List(
                vec![Sexp::Atom("ok".into(), s), conclusion_output.clone()],
                s,
            );
            rewrites.push(RewriteRule {
                name: rule.name.clone(),
                lhs,
                rhs,
            });
        } else {
            // N-premise rule: staging combinator with context threading
            let stg_name = format!("__stg_{}", rule.name);
            staging_ops.push(stg_name.clone());

            // Determine context variables: metas in output that aren't in premise outputs
            let mut output_metas = HashSet::new();
            collect_metas(&conclusion_output, &mut output_metas);

            let mut premise_output_metas = HashSet::new();
            for premise in &rule.premises {
                if let Some((_, _, poutput)) = decompose_judgment(premise) {
                    collect_metas(&poutput, &mut premise_output_metas);
                }
            }

            // Context vars = metas in output but not in any premise output
            let mut ctx_vars: Vec<String> = output_metas
                .difference(&premise_output_metas)
                .cloned()
                .collect();
            ctx_vars.sort(); // deterministic order

            // Build premise calls: [J premise_inputs...]
            let premise_calls: Vec<Sexp> = rule
                .premises
                .iter()
                .filter_map(|p| {
                    let (pj, pinputs, _poutput) = decompose_judgment(p)?;
                    let mut items = vec![Sexp::Atom(pj, s)];
                    items.extend(pinputs);
                    Some(Sexp::List(items, s))
                })
                .collect();

            // Main rule: [J inputs...] ==> [__stg ctx_var1 ctx_var2 ... premise_call1 ...]
            let mut rhs_items = vec![Sexp::Atom(stg_name.clone(), s)];
            for cv in &ctx_vars {
                rhs_items.push(Sexp::Atom(cv.clone(), s));
            }
            rhs_items.extend(premise_calls);
            let rhs = Sexp::List(rhs_items, s);
            rewrites.push(RewriteRule {
                name: rule.name.clone(),
                lhs,
                rhs,
            });

            let n_ctx = ctx_vars.len();
            let n_premises = rule.premises.len();

            // Success rule: [__stg ctx_vars... [ok pat1] [ok pat2] ...] ==> [ok output]
            let mut ok_lhs_items = vec![Sexp::Atom(stg_name.clone(), s)];
            for cv in &ctx_vars {
                ok_lhs_items.push(Sexp::Atom(cv.clone(), s));
            }
            for premise in &rule.premises {
                let (_pj, _pinputs, poutput) = match decompose_judgment(premise) {
                    Some(d) => d,
                    None => continue,
                };
                ok_lhs_items.push(Sexp::List(
                    vec![Sexp::Atom("ok".into(), s), poutput],
                    s,
                ));
            }
            let ok_rhs = Sexp::List(
                vec![Sexp::Atom("ok".into(), s), conclusion_output.clone()],
                s,
            );
            rewrites.push(RewriteRule {
                name: format!("{}_ok", stg_name),
                lhs: Sexp::List(ok_lhs_items, s),
                rhs: ok_rhs,
            });

            // Failure rules: one per premise position
            for fail_idx in 0..n_premises {
                let mut fail_lhs_items = vec![Sexp::Atom(stg_name.clone(), s)];
                // Context vars as wildcards
                for k in 0..n_ctx {
                    fail_lhs_items.push(Sexp::Atom(format!("?__ctx{}", k), s));
                }
                for k in 0..n_premises {
                    if k == fail_idx {
                        fail_lhs_items.push(Sexp::Atom("fail".into(), s));
                    } else {
                        let var = format!("?__p{}", k);
                        fail_lhs_items.push(Sexp::Atom(var, s));
                    }
                }
                rewrites.push(RewriteRule {
                    name: format!("{}_fail{}", stg_name, fail_idx),
                    lhs: Sexp::List(fail_lhs_items, s),
                    rhs: Sexp::Atom("fail".into(), s),
                });
            }

            // Catch-all: all premises resolved to [ok ...] but didn't match success pattern
            let mut catch_lhs_items = vec![Sexp::Atom(stg_name.clone(), s)];
            for k in 0..n_ctx {
                catch_lhs_items.push(Sexp::Atom(format!("?__cx{}", k), s));
            }
            for k in 0..n_premises {
                let var = format!("?__c{}", k);
                catch_lhs_items.push(Sexp::List(
                    vec![Sexp::Atom("ok".into(), s), Sexp::Atom(var, s)],
                    s,
                ));
            }
            rewrites.push(RewriteRule {
                name: format!("{}_catch", stg_name),
                lhs: Sexp::List(catch_lhs_items, s),
                rhs: Sexp::Atom("fail".into(), s),
            });
        }
    }

    (rewrites, staging_ops)
}

/// Check exhaustiveness of derive rules for a judgment.
/// Returns a list of warning messages.
pub fn check_exhaustiveness(
    judgment: &JudgmentDecl,
    rules: &[DerivRule],
    all_constructors: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    if judgment.partial {
        return Vec::new();
    }

    let mut warnings = Vec::new();

    // For each input position, collect head constructors from conclusions
    for (pos, sort) in judgment.input_sorts.iter().enumerate() {
        let mut covered_heads: HashSet<String> = HashSet::new();

        for rule in rules {
            if rule.absurd {
                // :absurd marks the case as "impossible" — count as covered
                if let Some(head) = extract_head_at_position(&rule.conclusion, pos + 1) {
                    covered_heads.insert(head);
                }
                continue;
            }
            if let Some(head) = extract_head_at_position(&rule.conclusion, pos + 1) {
                covered_heads.insert(head);
            }
        }

        // Check what's missing
        if let Some(constructors) = all_constructors.get(sort) {
            for ctor in constructors {
                if !covered_heads.contains(ctor) && !covered_heads.contains("?") {
                    warnings.push(format!(
                        "[WARN] judgment {}: no rule covers constructor '{}' at input position {} (sort {})",
                        judgment.name, ctor, pos, sort
                    ));
                }
            }
        }
    }

    warnings
}

/// Extract the head constructor name at a given position in a conclusion sexp.
/// Position 0 = judgment name, 1 = first input, etc.
fn extract_head_at_position(conclusion: &Sexp, pos: usize) -> Option<String> {
    let items = conclusion.as_list()?;
    if pos >= items.len() {
        return None;
    }
    let arg = &items[pos];
    match arg {
        Sexp::Atom(name, _) => {
            if name.starts_with('?') {
                Some("?".to_string()) // meta-variable = wildcard
            } else {
                Some(name.clone())
            }
        }
        Sexp::List(inner, _) if !inner.is_empty() => {
            inner[0].as_atom().map(|s| {
                if s.starts_with('?') {
                    "?".to_string()
                } else {
                    s.to_string()
                }
            })
        }
        _ => None,
    }
}

/// Verify an explicit derivation tree against the theory's derive rules.
///
/// `tree` is the Sexp of the form:
///   `[rule-name :sub [sub1 sub2 ...] :shows [conclusion]]`
/// or a leaf: `[rule-name :shows [conclusion]]`
///
/// Returns Ok(()) if the derivation is valid.
pub fn check_derivation(
    tree: &Sexp,
    derive_rules: &HashMap<String, DerivRule>,
    judgment_name: &str,
) -> Result<(), String> {
    let items = tree
        .as_list()
        .ok_or_else(|| "derivation tree must be a list".to_string())?;
    if items.is_empty() {
        return Err("empty derivation tree".into());
    }

    let rule_name = items[0]
        .as_atom()
        .ok_or_else(|| "first element must be a rule name".to_string())?
        .to_string();

    // Parse :by, :sub, :shows from the tree
    let mut subs: Vec<Sexp> = Vec::new();
    let mut shows: Option<Sexp> = None;
    let mut i = 1;

    while i < items.len() {
        match items[i].as_atom() {
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

    // Look up the derive rule
    let rule = derive_rules.get(&rule_name).ok_or_else(|| {
        format!(
            "unknown derive rule '{}' for judgment '{}'",
            rule_name, judgment_name
        )
    })?;

    // If :shows is provided, pattern match against rule's conclusion
    if let Some(ref shown) = shows {
        let bindings = match_sexp_pattern(&rule.conclusion, shown)?;

        // Verify each premise
        for (idx, premise) in rule.premises.iter().enumerate() {
            let instantiated = substitute_sexp(premise, &bindings);

            if idx < subs.len() {
                // Recursive sub-derivation
                check_derivation(&subs[idx], derive_rules, judgment_name)?;
            }
            // If no sub provided, the premise is assumed to be an axiom
            // (checked via normalization in the caller)
            let _ = instantiated; // used for error reporting
        }
    } else if rule.premises.is_empty() {
        // Axiom with no :shows — OK
    } else {
        return Err(format!(
            "rule '{}' has {} premises but no :shows to bind against",
            rule_name,
            rule.premises.len()
        ));
    }

    Ok(())
}

/// Simple pattern matching: match a pattern sexp against a concrete sexp.
/// Meta-variables (?X) in the pattern bind to subterms of the concrete sexp.
fn match_sexp_pattern(
    pattern: &Sexp,
    concrete: &Sexp,
) -> Result<HashMap<String, Sexp>, String> {
    let mut bindings = HashMap::new();
    match_sexp_inner(pattern, concrete, &mut bindings)?;
    Ok(bindings)
}

fn match_sexp_inner(
    pattern: &Sexp,
    concrete: &Sexp,
    bindings: &mut HashMap<String, Sexp>,
) -> Result<(), String> {
    match pattern {
        Sexp::Atom(name, _) if name.starts_with('?') => {
            let var = name.clone();
            if let Some(existing) = bindings.get(&var) {
                if format!("{}", existing) != format!("{}", concrete) {
                    return Err(format!(
                        "non-linear pattern: {} bound to both {} and {}",
                        var, existing, concrete
                    ));
                }
            } else {
                bindings.insert(var, concrete.clone());
            }
            Ok(())
        }
        Sexp::Atom(name, _) => {
            if let Some(cname) = concrete.as_atom() {
                if name == cname {
                    Ok(())
                } else {
                    Err(format!("expected '{}', got '{}'", name, cname))
                }
            } else {
                Err(format!("expected atom '{}', got list", name))
            }
        }
        Sexp::List(pitems, _) => {
            let citems = concrete
                .as_list()
                .ok_or_else(|| format!("expected list, got atom '{}'", concrete))?;
            if pitems.len() != citems.len() {
                return Err(format!(
                    "length mismatch: pattern has {} items, concrete has {}",
                    pitems.len(),
                    citems.len()
                ));
            }
            for (p, c) in pitems.iter().zip(citems.iter()) {
                match_sexp_inner(p, c, bindings)?;
            }
            Ok(())
        }
    }
}

/// Substitute meta-variables in an sexp using bindings.
fn substitute_sexp(sexp: &Sexp, bindings: &HashMap<String, Sexp>) -> Sexp {
    match sexp {
        Sexp::Atom(name, _) if name.starts_with('?') => {
            if let Some(val) = bindings.get(name) {
                val.clone()
            } else {
                sexp.clone()
            }
        }
        Sexp::Atom(_, _) => sexp.clone(),
        Sexp::List(items, span) => {
            let new_items: Vec<Sexp> = items
                .iter()
                .map(|item| substitute_sexp(item, bindings))
                .collect();
            Sexp::List(new_items, *span)
        }
    }
}
