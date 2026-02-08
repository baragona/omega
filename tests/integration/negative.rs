/// Negative tests: things that should be rejected.
use omega_driver::batch;
use omega_driver::session::Session;

fn check_source(source: &str) -> Result<Vec<String>, String> {
    let mut session = Session::new();
    batch::process_file(&mut session, source, "<test>")
}

#[test]
fn reject_duplicate_sort() {
    let result = check_source(
        "(theory Bad
           (sort Prop)
           (sort Prop))",
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("duplicate sort"));
}

#[test]
fn reject_duplicate_rule() {
    let result = check_source(
        "(theory Bad
           (sort Prop)
           (judgment (proves ?P) :where P : Prop)
           (rule r1
             :premises ()
             :conclusion (proves ?A))
           (rule r1
             :premises ()
             :conclusion (proves ?B)))",
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("duplicate rule"));
}

#[test]
fn reject_unknown_theory_in_proof() {
    let result = check_source(
        "(proof bad-proof
           :theory NonexistentTheory
           :goal (proves true)
           :derivation (assumption))",
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("Unknown theory") || err.contains("unknown theory"),
        "expected 'unknown theory' in error, got: {}",
        err
    );
}

#[test]
fn reject_unknown_rule_in_derivation() {
    let source = "
    (theory T
      (sort Prop)
      (judgment (proves ?P) :where P : Prop)
      (rule axiom
        :premises ()
        :conclusion (proves ?A)))
    (proof bad
      :theory T
      :goal (proves true)
      :derivation (nonexistent-rule))
    ";
    let result = check_source(source);
    assert!(result.is_err());
}

#[test]
fn reject_wrong_premise_count() {
    let source = "
    (theory T
      (sort Prop)
      (constructor and : (-> Prop Prop Prop))
      (judgment (proves ?P) :where P : Prop)
      (rule and-intro
        :premises ((proves ?A) (proves ?B))
        :conclusion (proves (and ?A ?B))))
    (proof bad
      :theory T
      :assumptions ((proves p))
      :goal (proves (and p q))
      :derivation (and-intro (assumption)))
    ";
    let result = check_source(source);
    assert!(result.is_err());
}

#[test]
fn reject_unproven_metatheorem_reflection() {
    let source = "
    (theory T
      (sort Prop)
      (judgment (proves ?P) :where P : Prop))
    (reflect nonexistent :as some-rule :theory T)
    ";
    let result = check_source(source);
    assert!(result.is_err());
}

#[test]
fn reject_invalid_parse() {
    let result = check_source("(theory");
    assert!(result.is_err());
}

#[test]
fn reject_empty_form() {
    let result = check_source("()");
    assert!(result.is_err());
}
