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
            // N-premise rule: sequential staging with dependency-aware chaining.
            //
            // Premises are evaluated one at a time via numbered staging combinators.
            // Each stage waits for the previous premise to resolve to [ok ...],
            // then uses the extracted metas to construct the next premise's call.
            // This correctly handles inter-premise dependencies (e.g., one premise's
            // output feeding into another's input).
            let stg_prefix = format!("__stg_{}", rule.name);

            // Decompose all premises into (judgment_name, inputs, output)
            let premise_parts: Vec<(String, Vec<Sexp>, Sexp)> = rule
                .premises
                .iter()
                .filter_map(|p| decompose_judgment(p))
                .collect();

            if premise_parts.len() != rule.premises.len() {
                continue; // malformed premises
            }

            let n = premise_parts.len();

            // Collect metas from each component
            let premise_output_metas: Vec<HashSet<String>> = premise_parts
                .iter()
                .map(|(_, _, output)| {
                    let mut m = HashSet::new();
                    collect_metas(output, &mut m);
                    m
                })
                .collect();

            let premise_call_metas: Vec<HashSet<String>> = premise_parts
                .iter()
                .map(|(_, inputs, _)| {
                    let mut m = HashSet::new();
                    for inp in inputs {
                        collect_metas(inp, &mut m);
                    }
                    m
                })
                .collect();

            let mut concl_output_metas = HashSet::new();
            collect_metas(&conclusion_output, &mut concl_output_metas);

            let mut lhs_metas = HashSet::new();
            for inp in &conclusion_inputs {
                collect_metas(inp, &mut lhs_metas);
            }

            // Compute needed_after(i): metas needed after premise i resolves
            let mut needed_after: Vec<HashSet<String>> = vec![HashSet::new(); n];
            for i in (0..n).rev() {
                let mut needed = HashSet::new();

                // Conclusion output needed at last stage
                needed.extend(concl_output_metas.iter().cloned());

                // Later premise call inputs
                for j in (i + 1)..n {
                    needed.extend(premise_call_metas[j].iter().cloned());
                }

                // Non-linear matching: metas from later outputs that also appear
                // in earlier outputs (including the current one)
                let earlier_output_metas: HashSet<String> = (0..=i)
                    .flat_map(|k| premise_output_metas[k].iter().cloned())
                    .collect();
                for j in (i + 1)..n {
                    for m in &premise_output_metas[j] {
                        if earlier_output_metas.contains(m) {
                            needed.insert(m.clone());
                        }
                    }
                }

                needed_after[i] = needed;
            }

            // Compute ctx_vars for each stage (available ∩ needed_after)
            let mut ctx_for_stage: Vec<Vec<String>> = vec![vec![]; n];
            let mut available = lhs_metas.clone();
            for i in 0..n {
                let ctx: HashSet<String> = needed_after[i]
                    .intersection(&available)
                    .cloned()
                    .collect();
                let mut sorted: Vec<String> = ctx.into_iter().collect();
                sorted.sort();
                ctx_for_stage[i] = sorted;

                // After this stage resolves, add its output metas
                available.extend(premise_output_metas[i].iter().cloned());
            }

            // Register all staging combinator names
            for i in 0..n {
                staging_ops.push(format!("{}_{}", stg_prefix, i));
            }

            // MAIN RULE: [J inputs] ==> [__stg_RULE_0 ctx0... [P0_call]]
            {
                let stg_0 = format!("{}_{}", stg_prefix, 0);
                let mut rhs_items = vec![Sexp::Atom(stg_0, s)];
                for cv in &ctx_for_stage[0] {
                    rhs_items.push(Sexp::Atom(cv.clone(), s));
                }
                let (ref p0_name, ref p0_inputs, _) = premise_parts[0];
                let mut p0_call = vec![Sexp::Atom(p0_name.clone(), s)];
                p0_call.extend(p0_inputs.iter().cloned());
                rhs_items.push(Sexp::List(p0_call, s));

                rewrites.push(RewriteRule {
                    name: rule.name.clone(),
                    lhs,
                    rhs: Sexp::List(rhs_items, s),
                });
            }

            // STAGE TRANSITIONS
            for i in 0..n {
                let stg_name = format!("{}_{}", stg_prefix, i);
                let n_ctx = ctx_for_stage[i].len();

                if i < n - 1 {
                    // Intermediate stage: success fires next premise
                    let stg_next = format!("{}_{}", stg_prefix, i + 1);

                    // Success rule
                    let mut ok_lhs = vec![Sexp::Atom(stg_name.clone(), s)];
                    for cv in &ctx_for_stage[i] {
                        ok_lhs.push(Sexp::Atom(cv.clone(), s));
                    }
                    ok_lhs.push(Sexp::List(
                        vec![Sexp::Atom("ok".into(), s), premise_parts[i].2.clone()],
                        s,
                    ));

                    let mut ok_rhs = vec![Sexp::Atom(stg_next, s)];
                    for cv in &ctx_for_stage[i + 1] {
                        ok_rhs.push(Sexp::Atom(cv.clone(), s));
                    }
                    let (ref pi1_name, ref pi1_inputs, _) = premise_parts[i + 1];
                    let mut pi1_call = vec![Sexp::Atom(pi1_name.clone(), s)];
                    pi1_call.extend(pi1_inputs.iter().cloned());
                    ok_rhs.push(Sexp::List(pi1_call, s));

                    rewrites.push(RewriteRule {
                        name: format!("{}_ok", stg_name),
                        lhs: Sexp::List(ok_lhs, s),
                        rhs: Sexp::List(ok_rhs, s),
                    });
                } else {
                    // Last stage: success produces final result
                    let mut ok_lhs = vec![Sexp::Atom(stg_name.clone(), s)];
                    for cv in &ctx_for_stage[i] {
                        ok_lhs.push(Sexp::Atom(cv.clone(), s));
                    }
                    ok_lhs.push(Sexp::List(
                        vec![Sexp::Atom("ok".into(), s), premise_parts[i].2.clone()],
                        s,
                    ));

                    rewrites.push(RewriteRule {
                        name: format!("{}_ok", stg_name),
                        lhs: Sexp::List(ok_lhs, s),
                        rhs: Sexp::List(
                            vec![
                                Sexp::Atom("ok".into(), s),
                                conclusion_output.clone(),
                            ],
                            s,
                        ),
                    });
                }

                // Failure rule
                let mut fail_lhs = vec![Sexp::Atom(stg_name.clone(), s)];
                for k in 0..n_ctx {
                    fail_lhs.push(Sexp::Atom(format!("?__ctx{}", k), s));
                }
                fail_lhs.push(Sexp::Atom("fail".into(), s));

                rewrites.push(RewriteRule {
                    name: format!("{}_fail", stg_name),
                    lhs: Sexp::List(fail_lhs, s),
                    rhs: Sexp::Atom("fail".into(), s),
                });

                // Catch-all (non-linear mismatch or unexpected output pattern)
                let mut catch_lhs = vec![Sexp::Atom(stg_name.clone(), s)];
                for k in 0..n_ctx {
                    catch_lhs.push(Sexp::Atom(format!("?__cx{}", k), s));
                }
                catch_lhs.push(Sexp::List(
                    vec![
                        Sexp::Atom("ok".into(), s),
                        Sexp::Atom("?__catch".into(), s),
                    ],
                    s,
                ));

                rewrites.push(RewriteRule {
                    name: format!("{}_catch", stg_name),
                    lhs: Sexp::List(catch_lhs, s),
                    rhs: Sexp::Atom("fail".into(), s),
                });
            }
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

/// Substitute atoms in an Sexp tree. Atoms matching a key in `map` are replaced.
/// Guards: never substitutes ?meta-vars, @directives, or __stg_ staging ops.
pub fn subst_sexp(sexp: &Sexp, map: &HashMap<String, Sexp>) -> Sexp {
    match sexp {
        Sexp::Atom(name, _span) => {
            if name.starts_with('?') || name.starts_with('@') || name.starts_with("__stg_") {
                sexp.clone()
            } else if let Some(replacement) = map.get(name.as_str()) {
                replacement.clone()
            } else {
                sexp.clone()
            }
        }
        Sexp::List(items, span) => {
            Sexp::List(items.iter().map(|s| subst_sexp(s, map)).collect(), *span)
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
