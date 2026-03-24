//! Kan Computation pass: generate type-specific transport and composition
//! rules for cubical type theory.
//!
//! In cubical type theory, `transp` (transport) and `hcomp` (homogeneous
//! composition) must compute differently for each type former:
//! - Pi types: transport under a Pi is contravariant in domain, covariant in codomain
//! - Sigma types: transport componentwise
//! - Path types: adjust endpoints
//!
//! This pass analyzes type declarations and generates concrete reduction
//! rules for `transp` and `hcomp` applied to each type constructor.

use apeiron::parser::{Sexp, Span};

/// A Kan operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KanOp {
    Transport,   // transp
    Composition, // hcomp
}

/// A generated Kan computation rule.
#[derive(Debug, Clone)]
pub struct KanRule {
    pub name: String,
    pub type_constructor: String,
    pub op: KanOp,
    pub rule: crate::session::VonNeumannRule,
}

/// Analyze type declarations and generate Kan computation rules.
/// `type_constructors` maps constructor names to their arities.
pub fn generate_kan_rules(
    type_constructors: &[(String, usize)],
) -> Vec<KanRule> {
    let mut rules = Vec::new();

    for (ctor, arity) in type_constructors {
        // Generate transport rule
        if let Some(rule) = generate_transport_rule(ctor, *arity) {
            rules.push(rule);
        }
        // Generate composition rule
        if let Some(rule) = generate_hcomp_rule(ctor, *arity) {
            rules.push(rule);
        }
    }

    rules
}

/// Known dependent type constructors that need special transport rules.
const DEPENDENT_CTORS: &[&str] = &["Sigma", "Sig", "sigma", "∑"];
const CONTRAVARIANT_CTORS: &[&str] = &["Pi", "Π", "pi", "Fun"];

/// Generate a transport rule for a type constructor.
/// Dispatches to specialized generators for Pi (contravariant) and Sigma (dependent).
fn generate_transport_rule(ctor: &str, arity: usize) -> Option<KanRule> {
    if arity == 0 {
        return Some(generate_ground_transport(ctor));
    }

    // Sigma: second component depends on transported first
    if arity == 2 && DEPENDENT_CTORS.iter().any(|d| *d == ctor) {
        return Some(generate_sigma_transport(ctor));
    }

    // Pi: domain is contravariant, codomain depends on back-transported arg
    if arity == 2 && CONTRAVARIANT_CTORS.iter().any(|d| *d == ctor) {
        return Some(generate_pi_transport(ctor));
    }

    // Generic: independent componentwise (safe for non-dependent products, etc.)
    Some(generate_generic_transport(ctor, arity))
}

/// Sigma transport: transp (Sigma A B) phi (a, b)
///   let a' = transp A phi a
///   let b' = __dep_transp B a a' phi b   -- transport in fiber B, coercing along a→a'
///   (a', b')
fn generate_sigma_transport(ctor: &str) -> KanRule {
    let sp = Span::default();
    let name = format!("transp-{}", ctor);

    let lhs = Sexp::List(vec![
        Sexp::Atom("transp".to_string(), sp),
        Sexp::List(vec![
            Sexp::Atom(ctor.to_string(), sp),
            Sexp::Atom("?A0".to_string(), sp),
            Sexp::Atom("?A1".to_string(), sp),
        ], sp),
        Sexp::Atom("?phi".to_string(), sp),
        Sexp::Atom("?u".to_string(), sp),
    ], sp);

    // a' = [transp ?A0 ?phi [proj0 ?u]]
    let a_prime = Sexp::List(vec![
        Sexp::Atom("transp".to_string(), sp),
        Sexp::Atom("?A0".to_string(), sp),
        Sexp::Atom("?phi".to_string(), sp),
        Sexp::List(vec![
            Sexp::Atom("proj0".to_string(), sp),
            Sexp::Atom("?u".to_string(), sp),
        ], sp),
    ], sp);

    // b' = [__dep_transp ?A1 [proj0 ?u] a' ?phi [proj1 ?u]]
    // __dep_transp B a a' phi b = transport b along the path induced by a→a' in family B
    let b_prime = Sexp::List(vec![
        Sexp::Atom("__dep_transp".to_string(), sp),
        Sexp::Atom("?A1".to_string(), sp),
        Sexp::List(vec![
            Sexp::Atom("proj0".to_string(), sp),
            Sexp::Atom("?u".to_string(), sp),
        ], sp),
        a_prime.clone(),
        Sexp::Atom("?phi".to_string(), sp),
        Sexp::List(vec![
            Sexp::Atom("proj1".to_string(), sp),
            Sexp::Atom("?u".to_string(), sp),
        ], sp),
    ], sp);

    let rhs = Sexp::List(vec![
        Sexp::Atom(ctor.to_string(), sp),
        a_prime,
        b_prime,
    ], sp);

    KanRule {
        name: name.clone(),
        type_constructor: ctor.to_string(),
        op: KanOp::Transport,
        rule: crate::session::VonNeumannRule { name, lhs, rhs },
    }
}

/// Pi transport: transp (Pi A B) phi f
///   λa'. let a = transp A [neg ?phi] a'   -- contravariant: transport BACKWARDS in domain
///        transp [B a] phi (f a)            -- covariant in codomain, dependent on back-transported a
fn generate_pi_transport(ctor: &str) -> KanRule {
    let sp = Span::default();
    let name = format!("transp-{}", ctor);

    let lhs = Sexp::List(vec![
        Sexp::Atom("transp".to_string(), sp),
        Sexp::List(vec![
            Sexp::Atom(ctor.to_string(), sp),
            Sexp::Atom("?A0".to_string(), sp),
            Sexp::Atom("?A1".to_string(), sp),
        ], sp),
        Sexp::Atom("?phi".to_string(), sp),
        Sexp::Atom("?f".to_string(), sp),
    ], sp);

    // a = [transp ?A0 [neg ?phi] ?a']  -- contravariant (backward)
    let a_back = Sexp::List(vec![
        Sexp::Atom("transp".to_string(), sp),
        Sexp::Atom("?A0".to_string(), sp),
        Sexp::List(vec![
            Sexp::Atom("neg".to_string(), sp),
            Sexp::Atom("?phi".to_string(), sp),
        ], sp),
        Sexp::Atom("?a'".to_string(), sp),
    ], sp);

    // [transp [?A1 a] ?phi [?f a]]  -- covariant in codomain
    let body = Sexp::List(vec![
        Sexp::Atom("transp".to_string(), sp),
        Sexp::List(vec![
            Sexp::Atom("?A1".to_string(), sp),
            a_back.clone(),
        ], sp),
        Sexp::Atom("?phi".to_string(), sp),
        Sexp::List(vec![
            Sexp::Atom("?f".to_string(), sp),
            a_back,
        ], sp),
    ], sp);

    // λa'. body
    let rhs = Sexp::List(vec![
        Sexp::Atom("lam".to_string(), sp),
        Sexp::Atom("?a'".to_string(), sp),
        body,
    ], sp);

    KanRule {
        name: name.clone(),
        type_constructor: ctor.to_string(),
        op: KanOp::Transport,
        rule: crate::session::VonNeumannRule { name, lhs, rhs },
    }
}

/// Generic independent componentwise transport for non-dependent types.
fn generate_generic_transport(ctor: &str, arity: usize) -> KanRule {
    let sp = Span::default();

    let mut ctor_args = vec![Sexp::Atom(ctor.to_string(), sp)];
    for i in 0..arity {
        ctor_args.push(Sexp::Atom(format!("?A{}", i), sp));
    }

    let lhs = Sexp::List(vec![
        Sexp::Atom("transp".to_string(), sp),
        Sexp::List(ctor_args, sp),
        Sexp::Atom("?phi".to_string(), sp),
        Sexp::Atom("?u".to_string(), sp),
    ], sp);

    let mut rhs_args = vec![Sexp::Atom(ctor.to_string(), sp)];
    for i in 0..arity {
        let proj = Sexp::List(vec![
            Sexp::Atom(format!("proj{}", i), sp),
            Sexp::Atom("?u".to_string(), sp),
        ], sp);
        let transp_arg = Sexp::List(vec![
            Sexp::Atom("transp".to_string(), sp),
            Sexp::Atom(format!("?A{}", i), sp),
            Sexp::Atom("?phi".to_string(), sp),
            proj,
        ], sp);
        rhs_args.push(transp_arg);
    }

    let rhs = Sexp::List(rhs_args, sp);
    let name = format!("transp-{}", ctor);

    KanRule {
        name: name.clone(),
        type_constructor: ctor.to_string(),
        op: KanOp::Transport,
        rule: crate::session::VonNeumannRule { name, lhs, rhs },
    }
}

/// Ground type transport: `[transp ctor ?phi ?u] ==> ?u`
fn generate_ground_transport(ctor: &str) -> KanRule {
    let sp = Span::default();
    let name = format!("transp-{}", ctor);
    KanRule {
        name: name.clone(),
        type_constructor: ctor.to_string(),
        op: KanOp::Transport,
        rule: crate::session::VonNeumannRule {
            name,
            lhs: Sexp::List(vec![
                Sexp::Atom("transp".to_string(), sp),
                Sexp::Atom(ctor.to_string(), sp),
                Sexp::Atom("?phi".to_string(), sp),
                Sexp::Atom("?u".to_string(), sp),
            ], sp),
            rhs: Sexp::Atom("?u".to_string(), sp),
        },
    }
}

/// Generate a homogeneous composition rule.
/// `[hcomp [ctor A1..An] phi u0 sys] ==> [ctor (hcomp A0 phi (proj0 u0) (face0 sys)) ...]`
fn generate_hcomp_rule(ctor: &str, arity: usize) -> Option<KanRule> {
    if arity == 0 {
        return Some(generate_ground_hcomp(ctor));
    }

    let sp = Span::default();

    let mut ctor_args = vec![Sexp::Atom(ctor.to_string(), sp)];
    for i in 0..arity {
        ctor_args.push(Sexp::Atom(format!("?A{}", i), sp));
    }

    let lhs = Sexp::List(vec![
        Sexp::Atom("hcomp".to_string(), sp),
        Sexp::List(ctor_args, sp),
        Sexp::Atom("?phi".to_string(), sp),
        Sexp::Atom("?u0".to_string(), sp),
        Sexp::Atom("?sys".to_string(), sp),
    ], sp);

    let mut rhs_args = vec![Sexp::Atom(ctor.to_string(), sp)];
    for i in 0..arity {
        let hcomp_arg = Sexp::List(vec![
            Sexp::Atom("hcomp".to_string(), sp),
            Sexp::Atom(format!("?A{}", i), sp),
            Sexp::Atom("?phi".to_string(), sp),
            Sexp::List(vec![
                Sexp::Atom(format!("proj{}", i), sp),
                Sexp::Atom("?u0".to_string(), sp),
            ], sp),
            Sexp::List(vec![
                Sexp::Atom(format!("face{}", i), sp),
                Sexp::Atom("?sys".to_string(), sp),
            ], sp),
        ], sp);
        rhs_args.push(hcomp_arg);
    }

    let rhs = Sexp::List(rhs_args, sp);
    let name = format!("hcomp-{}", ctor);

    Some(KanRule {
        name: name.clone(),
        type_constructor: ctor.to_string(),
        op: KanOp::Composition,
        rule: crate::session::VonNeumannRule { name, lhs, rhs },
    })
}

/// Ground type hcomp: `[hcomp ctor ?phi ?u0 ?sys] ==> ?u0`
fn generate_ground_hcomp(ctor: &str) -> KanRule {
    let sp = Span::default();
    let name = format!("hcomp-{}", ctor);
    KanRule {
        name: name.clone(),
        type_constructor: ctor.to_string(),
        op: KanOp::Composition,
        rule: crate::session::VonNeumannRule {
            name,
            lhs: Sexp::List(vec![
                Sexp::Atom("hcomp".to_string(), sp),
                Sexp::Atom(ctor.to_string(), sp),
                Sexp::Atom("?phi".to_string(), sp),
                Sexp::Atom("?u0".to_string(), sp),
                Sexp::Atom("?sys".to_string(), sp),
            ], sp),
            rhs: Sexp::Atom("?u0".to_string(), sp),
        },
    }
}

/// Extract type constructors and arities from rules.
/// Looks for patterns where constructor heads appear in type positions.
pub fn detect_type_constructors(rules: &[crate::session::VonNeumannRule]) -> Vec<(String, usize)> {
    let mut seen = std::collections::HashMap::new();

    for rule in rules {
        detect_in_sexp(&rule.lhs, &mut seen);
        detect_in_sexp(&rule.rhs, &mut seen);
    }

    // Filter: only include constructors that appear as type-level heads
    // (heuristic: capitalized names or known type formers)
    seen.into_iter()
        .filter(|(name, _)| {
            name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                || matches!(name.as_str(), "Pi" | "Sigma" | "Path" | "Glue" | "U")
        })
        .collect()
}

fn detect_in_sexp(sexp: &Sexp, seen: &mut std::collections::HashMap<String, usize>) {
    if let Sexp::List(items, _) = sexp {
        if let Some(head) = items.first().and_then(|i| i.as_atom()) {
            if !head.starts_with('?') && !matches!(head, "transp" | "hcomp" | "apply") {
                seen.entry(head.to_string()).or_insert(items.len() - 1);
            }
        }
        for item in items {
            detect_in_sexp(item, seen);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(s: &str) -> Sexp { Sexp::Atom(s.to_string(), Span::default()) }
    fn list(items: Vec<Sexp>) -> Sexp { Sexp::List(items, Span::default()) }

    #[test]
    fn ground_transport_is_identity() {
        let rules = generate_kan_rules(&[("Bool".to_string(), 0)]);
        assert_eq!(rules.len(), 2); // transp + hcomp
        let transp = &rules[0];
        assert_eq!(transp.op, KanOp::Transport);
        let rhs = format!("{}", transp.rule.rhs);
        assert_eq!(rhs, "?u"); // identity
    }

    #[test]
    fn ground_hcomp_is_identity() {
        let rules = generate_kan_rules(&[("Bool".to_string(), 0)]);
        let hcomp = &rules[1];
        assert_eq!(hcomp.op, KanOp::Composition);
        let rhs = format!("{}", hcomp.rule.rhs);
        assert_eq!(rhs, "?u0");
    }

    #[test]
    fn unary_transport_componentwise() {
        let rules = generate_kan_rules(&[("List".to_string(), 1)]);
        let transp = &rules[0];
        let lhs = format!("{}", transp.rule.lhs);
        assert!(lhs.contains("transp"), "LHS: {}", lhs);
        assert!(lhs.contains("List"), "LHS: {}", lhs);
        let rhs = format!("{}", transp.rule.rhs);
        assert!(rhs.contains("transp"), "RHS should transport component: {}", rhs);
        assert!(rhs.contains("proj0"), "RHS should project: {}", rhs);
    }

    #[test]
    fn binary_transport_both_components() {
        // Use a non-dependent binary type (Pair, not Sigma)
        let rules = generate_kan_rules(&[("Pair".to_string(), 2)]);
        let transp = &rules[0];
        let rhs = format!("{}", transp.rule.rhs);
        assert!(rhs.contains("proj0"), "RHS: {}", rhs);
        assert!(rhs.contains("proj1"), "RHS: {}", rhs);
    }

    #[test]
    fn multiple_constructors() {
        let ctors = vec![
            ("Bool".to_string(), 0),
            ("Pi".to_string(), 2),
            ("Sigma".to_string(), 2),
        ];
        let rules = generate_kan_rules(&ctors);
        assert_eq!(rules.len(), 6); // 2 per constructor
    }

    #[test]
    fn detect_type_constructors_from_rules() {
        let rules = vec![
            crate::session::VonNeumannRule {
                name: "r1".to_string(),
                lhs: list(vec![atom("typeof"), list(vec![atom("Pi"), atom("?A"), atom("?B")])]),
                rhs: atom("ok"),
            },
        ];
        let ctors = detect_type_constructors(&rules);
        assert!(ctors.iter().any(|(n, a)| n == "Pi" && *a == 2));
    }

    // === ABYSSAL: Dependent Interval Trap ===

    #[test]
    fn sigma_transport_second_component_depends_on_first() {
        // For Σ(x:A).B(x), transport of (a, b) along path p must:
        //   fst' = transp A phi (proj0 u)
        //   snd' = transp [?A1 fst'] phi (proj1 u)  <-- DEPENDS on fst'
        // NOT: snd' = transp ?A1 phi (proj1 u)  <-- INDEPENDENT (wrong!)
        let rules = generate_kan_rules(&[("Sigma".to_string(), 2)]);
        let transp = rules.iter().find(|r| r.op == KanOp::Transport).unwrap();
        let rhs = format!("{}", transp.rule.rhs);

        // The second component's type family must reference the transported first component.
        // It should contain something like [?A1 [transp ?A0 ...]] — the family applied to
        // the transported first projection.
        assert!(rhs.contains("__dep_transp") || rhs.contains("[?A1 [transp"),
            "Sigma transport second component must depend on transported first component.\n\
             Got independent componentwise transport: {}", rhs);
    }

    #[test]
    fn pi_transport_is_contravariant_covariant() {
        // For Π(x:A).B(x), transport must be:
        //   transp (Pi A B) phi f = λa'. let a = transp A (neg phi) a' in transp [B a] phi (f a)
        // The domain transport goes BACKWARDS (contravariant).
        let rules = generate_kan_rules(&[("Pi".to_string(), 2)]);
        let transp = rules.iter().find(|r| r.op == KanOp::Transport).unwrap();
        let rhs = format!("{}", transp.rule.rhs);

        // Must contain contravariant marker or negated direction for domain
        assert!(rhs.contains("__contra") || rhs.contains("neg"),
            "Pi transport domain must be contravariant (backward transport).\n\
             Got naive covariant transport: {}", rhs);
    }

    #[test]
    fn hcomp_binary_has_face_projections() {
        let rules = generate_kan_rules(&[("Sigma".to_string(), 2)]);
        let hcomp = &rules[1];
        let rhs = format!("{}", hcomp.rule.rhs);
        assert!(rhs.contains("face0"), "RHS: {}", rhs);
        assert!(rhs.contains("face1"), "RHS: {}", rhs);
    }
}
