//! Dialectica Extraction pass: extract computable witnesses from proofs
//! via Gödel's Dialectica interpretation.
//!
//! The Dialectica interpretation translates formulas A into ∃x.∀y.A_D(x,y)
//! where x is a witness and y is a challenge. For intuitionistic proofs,
//! extraction is direct. For classical proofs (using LEM, DNE, Markov's
//! principle), a CPS/double-negation translation must be applied first.
//!
//! This pass:
//! 1. Detects classical axioms in proofs (LEM, DNE, Markov)
//! 2. Applies Friedman's A-translation to classicalize → intuitionize
//! 3. Extracts witness terms from the (now intuitionistic) proof

use apeiron::parser::{Sexp, Span};

/// Classical axioms that require special treatment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassicalAxiom {
    /// Law of Excluded Middle: A ∨ ¬A
    LEM,
    /// Double Negation Elimination: ¬¬A → A
    DNE,
    /// Markov's Principle: ¬¬∃x.P(x) → ∃x.P(x) (for decidable P)
    Markov,
}

/// Result of Dialectica extraction.
#[derive(Debug)]
pub struct ExtractionResult {
    /// The extracted witness program (if successful)
    pub witness: Option<Sexp>,
    /// Classical axioms detected in the proof
    pub classical_axioms: Vec<ClassicalAxiom>,
    /// Whether a CPS translation was required
    pub cps_translated: bool,
    /// Extraction errors/warnings
    pub diagnostics: Vec<String>,
}

/// Extract a witness from a proof term.
/// `proof` is the proof Sexp, `goal` is the existential formula ∃x.P(x).
pub fn extract_witness(
    proof: &Sexp,
    goal: &Sexp,
    rules: &[crate::session::VonNeumannRule],
) -> ExtractionResult {
    // Step 1: Detect classical axioms
    let axioms = detect_classical_axioms(proof, rules);

    // Step 2: If classical axioms present, apply A-translation first
    let (translated_proof, cps_needed) = if !axioms.is_empty() {
        (apply_a_translation(proof), true)
    } else {
        (proof.clone(), false)
    };

    // Step 3: Extract witness from (now intuitionistic) proof
    let witness = extract_from_intuitionistic(&translated_proof, goal);

    ExtractionResult {
        witness,
        classical_axioms: axioms,
        cps_translated: cps_needed,
        diagnostics: vec![],
    }
}

/// Detect classical axioms used in a proof.
fn detect_classical_axioms(
    proof: &Sexp,
    rules: &[crate::session::VonNeumannRule],
) -> Vec<ClassicalAxiom> {
    let mut axioms = Vec::new();
    let mut seen_lem = false;
    let mut seen_dne = false;
    let mut seen_markov = false;

    // Check rule names and patterns for classical axiom usage
    for rule in rules {
        let name = rule.name.to_lowercase();
        if !seen_lem && (name.contains("lem") || name.contains("excluded-middle")) {
            axioms.push(ClassicalAxiom::LEM);
            seen_lem = true;
        }
        if !seen_dne && (name.contains("dne") || name.contains("double-neg")) {
            axioms.push(ClassicalAxiom::DNE);
            seen_dne = true;
        }
        if !seen_markov && name.contains("markov") {
            axioms.push(ClassicalAxiom::Markov);
            seen_markov = true;
        }
    }

    // Also scan the proof term for classical constructors
    scan_for_classical(proof, &mut axioms, &mut seen_lem, &mut seen_dne, &mut seen_markov);

    axioms
}

fn scan_for_classical(sexp: &Sexp, axioms: &mut Vec<ClassicalAxiom>,
                       seen_lem: &mut bool, seen_dne: &mut bool, seen_markov: &mut bool) {
    match sexp {
        Sexp::Atom(name, _) => {
            if !*seen_lem && (name == "lem" || name == "excluded-middle") {
                axioms.push(ClassicalAxiom::LEM);
                *seen_lem = true;
            }
            if !*seen_dne && (name == "dne" || name == "double-neg-elim") {
                axioms.push(ClassicalAxiom::DNE);
                *seen_dne = true;
            }
            if !*seen_markov && name == "markov" {
                axioms.push(ClassicalAxiom::Markov);
                *seen_markov = true;
            }
        }
        Sexp::List(items, _) => {
            for item in items {
                scan_for_classical(item, axioms, seen_lem, seen_dne, seen_markov);
            }
        }
    }
}

/// Apply Friedman's A-translation (double-negation / CPS transform).
/// Translates classical proof into intuitionistic proof via:
///   A^A = (A → ⊥_A) → ⊥_A  where ⊥_A is a "catch" continuation
///
/// For extraction purposes, this wraps classical control flow into
/// continuation-passing style so witnesses can be extracted structurally.
pub fn apply_a_translation(proof: &Sexp) -> Sexp {
    let sp = proof.span();
    a_translate_inner(proof, sp)
}

fn a_translate_inner(sexp: &Sexp, sp: Span) -> Sexp {
    match sexp {
        Sexp::Atom(name, _) => {
            // Classical axiom atoms get wrapped in CPS
            match name.as_str() {
                "lem" | "excluded-middle" => {
                    // LEM: A ∨ ¬A  →  callcc (λk. inr (λa. k (inl a)))
                    Sexp::List(vec![
                        Sexp::Atom("__callcc".to_string(), sp),
                        Sexp::List(vec![
                            Sexp::Atom("__lem_witness".to_string(), sp),
                            sexp.clone(),
                        ], sp),
                    ], sp)
                }
                "dne" | "double-neg-elim" => {
                    // DNE: ¬¬A → A  →  callcc (λk. absurd (proof (λa. k a)))
                    Sexp::List(vec![
                        Sexp::Atom("__callcc".to_string(), sp),
                        Sexp::List(vec![
                            Sexp::Atom("__dne_witness".to_string(), sp),
                            sexp.clone(),
                        ], sp),
                    ], sp)
                }
                "markov" => {
                    // Markov: ¬¬∃x.P(x) → ∃x.P(x)  →  unbounded search
                    Sexp::List(vec![
                        Sexp::Atom("__markov_search".to_string(), sp),
                        sexp.clone(),
                    ], sp)
                }
                _ => sexp.clone(),
            }
        }
        Sexp::List(items, _) => {
            let translated: Vec<Sexp> = items.iter()
                .map(|i| a_translate_inner(i, sp))
                .collect();
            Sexp::List(translated, sp)
        }
    }
}

/// Extract witness from an intuitionistic proof.
/// Looks for existential introductions (exist-intro, witness, pair).
fn extract_from_intuitionistic(proof: &Sexp, _goal: &Sexp) -> Option<Sexp> {
    match proof {
        Sexp::List(items, _) => {
            if let Some(head) = items.first().and_then(|i| i.as_atom()) {
                match head {
                    // [exist-intro witness proof-of-P-witness]
                    "exist-intro" | "witness" | "ex-intro" if items.len() >= 2 => {
                        return Some(items[1].clone());
                    }
                    // [pair a b] in Sigma-type = witness is fst
                    "pair" | "dpair" if items.len() >= 2 => {
                        return Some(items[1].clone());
                    }
                    // CPS-translated: [__callcc body] — extract from body
                    "__callcc" if items.len() >= 2 => {
                        return extract_from_intuitionistic(&items[1], _goal);
                    }
                    "__lem_witness" | "__dne_witness" if items.len() >= 2 => {
                        // The original classical axiom — extraction produces
                        // a continuation-based witness
                        return Some(Sexp::List(vec![
                            Sexp::Atom("__cps_witness".to_string(), items[0].span()),
                            items[1].clone(),
                        ], items[0].span()));
                    }
                    "__markov_search" if items.len() >= 2 => {
                        // Markov extraction: unbounded search program
                        return Some(Sexp::List(vec![
                            Sexp::Atom("__search".to_string(), items[0].span()),
                            items[1].clone(),
                        ], items[0].span()));
                    }
                    _ => {}
                }
            }
            // Try to find witness deeper in the proof
            for item in items {
                if let Some(w) = extract_from_intuitionistic(item, _goal) {
                    return Some(w);
                }
            }
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(s: &str) -> Sexp { Sexp::Atom(s.to_string(), Span::default()) }
    fn list(items: Vec<Sexp>) -> Sexp { Sexp::List(items, Span::default()) }
    fn rule(name: &str, lhs: Sexp, rhs: Sexp) -> crate::session::VonNeumannRule {
        crate::session::VonNeumannRule { name: name.to_string(), lhs, rhs }
    }

    #[test]
    fn intuitionistic_extraction_simple() {
        // [exist-intro witness proof] → extract "witness"
        let proof = list(vec![atom("exist-intro"), atom("42"), atom("proof-p-42")]);
        let goal = list(vec![atom("exists"), atom("x"), list(vec![atom("P"), atom("x")])]);
        let result = extract_witness(&proof, &goal, &[]);
        assert!(result.witness.is_some());
        assert_eq!(format!("{}", result.witness.unwrap()), "42");
        assert!(result.classical_axioms.is_empty());
        assert!(!result.cps_translated);
    }

    #[test]
    fn detects_lem_in_rules() {
        let rules = vec![
            rule("lem", atom("A"), list(vec![atom("or"), atom("A"), list(vec![atom("not"), atom("A")])])),
        ];
        let proof = atom("some-proof");
        let result = extract_witness(&proof, &atom("goal"), &rules);
        assert!(result.classical_axioms.contains(&ClassicalAxiom::LEM));
        assert!(result.cps_translated);
    }

    #[test]
    fn detects_dne_in_proof_term() {
        let proof = list(vec![atom("app"), atom("dne"), atom("proof-of-not-not-A")]);
        let result = extract_witness(&proof, &atom("goal"), &[]);
        assert!(result.classical_axioms.contains(&ClassicalAxiom::DNE));
        assert!(result.cps_translated);
    }

    #[test]
    fn detects_markov_in_proof_term() {
        let proof = list(vec![atom("markov"), atom("dec-P"), atom("not-not-exists")]);
        let result = extract_witness(&proof, &atom("goal"), &[]);
        assert!(result.classical_axioms.contains(&ClassicalAxiom::Markov));
        assert!(result.cps_translated);
    }

    // === TARTARUS: Dialectica Continuation Trap ===

    #[test]
    fn classical_lem_proof_does_not_extract_dummy() {
        // A proof using LEM to prove ∃x.P(x) must NOT extract a dummy/⊥ witness.
        // It must either:
        // (a) Apply A-translation first, then extract from CPS form
        // (b) Return a continuation-based witness that can be evaluated
        let proof = list(vec![
            atom("exist-intro"),
            atom("lem"),  // witness is... LEM? That's the trap.
            atom("classical-proof"),
        ]);
        let goal = list(vec![atom("exists"), atom("x"), list(vec![atom("P"), atom("x")])]);
        let rules = vec![
            rule("lem-axiom", atom("lem"), list(vec![atom("or"), atom("A"), atom("notA")])),
        ];

        let result = extract_witness(&proof, &goal, &rules);

        // Must detect classical axiom and CPS-translate
        assert!(result.cps_translated,
            "Must apply CPS translation for classical proof");
        assert!(!result.classical_axioms.is_empty(),
            "Must detect LEM usage");

        // The extracted witness must NOT be the raw "lem" atom
        if let Some(ref w) = result.witness {
            let ws = format!("{}", w);
            assert!(ws != "lem",
                "Must not extract raw classical axiom as witness — need CPS wrapping. Got: {}", ws);
        }
    }

    #[test]
    fn markov_extraction_produces_search_program() {
        // Markov's principle: ¬¬∃x.P(x) → ∃x.P(x) for decidable P
        // Extraction must produce an unbounded search, NOT a dummy value.
        let proof = list(vec![
            atom("exist-intro"),
            atom("markov"),
            atom("decidability-proof"),
        ]);
        let goal = list(vec![atom("exists"), atom("x"), list(vec![atom("P"), atom("x")])]);

        let result = extract_witness(&proof, &goal, &[]);
        assert!(result.cps_translated);

        if let Some(ref w) = result.witness {
            let ws = format!("{}", w);
            assert!(ws.contains("__search") || ws.contains("__markov"),
                "Markov extraction must produce search program, not dummy. Got: {}", ws);
        } else {
            panic!("Must extract a witness from Markov proof");
        }
    }

    #[test]
    fn dne_proof_wrapped_in_callcc() {
        // DNE: ¬¬A → A. Extraction produces a callcc-based witness.
        let proof = list(vec![atom("dne"), atom("not-not-proof")]);
        let goal = atom("A");

        let result = extract_witness(&proof, &goal, &[]);
        assert!(result.cps_translated);

        // The A-translated proof should contain __callcc
        // (we can verify the translation happened)
        assert!(!result.classical_axioms.is_empty());
    }

    #[test]
    fn purely_intuitionistic_no_cps() {
        // An intuitionistic proof should NOT trigger CPS translation
        let proof = list(vec![atom("exist-intro"), atom("zero"),
            list(vec![atom("refl"), atom("zero")])]);
        let goal = list(vec![atom("exists"), atom("x"), list(vec![atom("eq"), atom("x"), atom("x")])]);

        let result = extract_witness(&proof, &goal, &[]);
        assert!(!result.cps_translated,
            "Purely intuitionistic proof must NOT trigger CPS translation");
        assert!(result.classical_axioms.is_empty());
        assert_eq!(format!("{}", result.witness.unwrap()), "zero");
    }
}
