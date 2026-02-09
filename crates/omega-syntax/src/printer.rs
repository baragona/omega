/// Pretty-printing core types back to S-expression text.
use omega_core::derivation::Derivation;
use omega_core::expr::Expr;
use omega_core::judgment::Rule;
use omega_core::theory::Theory;

/// Print an expression as an S-expression string.
pub fn print_expr(expr: &Expr) -> String {
    match expr {
        Expr::Free(n) => n.clone(),
        Expr::Bound(i) => format!("#{}", i),
        Expr::Meta(n) => format!("?{}", n),
        Expr::Sym(n) => n.clone(),
        Expr::App(args) => {
            let inner: Vec<String> = args.iter().map(|a| print_expr(a)).collect();
            format!("({})", inner.join(" "))
        }
        Expr::Binder {
            kind,
            ty,
            body,
            ..
        } if kind == omega_core::expr::ARROW => {
            // Flatten nested arrows
            let mut args = vec![print_expr(ty)];
            let mut current = body.as_ref();
            while let Expr::Binder {
                kind: ref inner_kind,
                ty: inner_ty,
                body: inner_body,
                ..
            } = current
            {
                if inner_kind != omega_core::expr::ARROW {
                    break;
                }
                args.push(print_expr(inner_ty));
                current = inner_body;
            }
            args.push(print_expr(current));
            format!("(-> {})", args.join(" "))
        }
        Expr::Binder {
            kind,
            hint,
            ty,
            body,
        } => {
            format!("({} ({} : {}) {})", kind, hint, print_expr(ty), print_expr(body))
        }
    }
}

/// Print a derivation as an S-expression string.
pub fn print_derivation(deriv: &Derivation) -> String {
    match deriv {
        Derivation::Assumption => "(assumption)".to_string(),
        Derivation::AssumptionIdx(i) => format!("(assumption {})", i),
        Derivation::RuleApp {
            rule_name,
            premises,
        } => {
            if premises.is_empty() {
                format!("({})", rule_name)
            } else {
                let args: Vec<String> = premises.iter().map(|p| print_derivation(p)).collect();
                format!("({} {})", rule_name, args.join(" "))
            }
        }
    }
}

/// Print a rule.
pub fn print_rule(rule: &Rule) -> String {
    let premises: Vec<String> = rule.premises.iter().map(|p| print_expr(p)).collect();
    let conclusion = print_expr(&rule.conclusion);

    let mut s = format!("(rule {}", rule.name);
    if !rule.premises.is_empty() {
        s.push_str(&format!("\n    :premises ({})", premises.join(" ")));
    }
    s.push_str(&format!("\n    :conclusion {})", conclusion));
    s
}

/// Print a theory summary.
pub fn print_theory_summary(theory: &Theory) -> String {
    let mut s = format!("Theory: {}\n", theory.name);
    s.push_str(&format!("  Sorts: {}\n", theory.sorts.len()));
    s.push_str(&format!("  Constructors: {}\n", theory.constructors.len()));
    s.push_str(&format!("  Judgments: {}\n", theory.judgments.len()));
    s.push_str(&format!("  Rules: {}\n", theory.rules.len()));

    for sort in &theory.sorts {
        s.push_str(&format!("  sort {}\n", sort.name));
    }
    for ctor in &theory.constructors {
        s.push_str(&format!("  constructor {} : {}\n", ctor.name, print_expr(&ctor.ty)));
    }
    for rule in &theory.rules {
        let reflected = if rule.reflected { " [reflected]" } else { "" };
        s.push_str(&format!("  rule {}{}\n", rule.name, reflected));
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_core::expr::Expr;

    #[test]
    fn print_simple_expr() {
        let e = Expr::app(vec![
            Expr::sym("proves"),
            Expr::app(vec![Expr::sym("and"), Expr::meta("A"), Expr::meta("B")]),
        ]);
        assert_eq!(print_expr(&e), "(proves (and ?A ?B))");
    }

    #[test]
    fn print_derivation_tree() {
        let d = Derivation::RuleApp {
            rule_name: "and-intro".to_string(),
            premises: vec![Derivation::Assumption, Derivation::Assumption],
        };
        assert_eq!(print_derivation(&d), "(and-intro (assumption) (assumption))");
    }

    #[test]
    fn roundtrip_expr() {
        let e = Expr::app(vec![Expr::sym("proves"), Expr::meta("X")]);
        let printed = print_expr(&e);
        assert_eq!(printed, "(proves ?X)");
    }
}
