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

#[test]
fn reject_affine_double_use() {
    // In affine mode, using the same assumption twice should fail.
    // Only one (holds a) in context, but two `assumption` derivations.
    let source = "
    (theory AffineTest
      (context-mode affine)
      (sort Prop)
      (constructor tensor : (-> Prop Prop Prop))
      (judgment (holds ?A) :where A : Prop)
      (rule tensor-intro
        :premises ((holds ?A) (holds ?B))
        :conclusion (holds (tensor ?A ?B))))
    (proof bad-double-use
      :theory AffineTest
      :goal (holds (tensor a a))
      :assumptions ((holds a))
      :derivation (tensor-intro assumption assumption))
    ";
    let result = check_source(source);
    assert!(result.is_err(), "affine double-use should be rejected");
    let err = result.unwrap_err();
    assert!(
        err.contains("affine violation") || err.contains("already consumed") || err.contains("no matching assumption"),
        "expected affine error, got: {}",
        err
    );
}

#[test]
fn structural_allows_double_use() {
    // The same proof should work fine in structural mode (control test).
    let source = "
    (theory StructuralTest
      (sort Prop)
      (constructor tensor : (-> Prop Prop Prop))
      (judgment (holds ?A) :where A : Prop)
      (rule tensor-intro
        :premises ((holds ?A) (holds ?B))
        :conclusion (holds (tensor ?A ?B))))
    (proof ok-double-use
      :theory StructuralTest
      :goal (holds (tensor a a))
      :assumptions ((holds a))
      :derivation (tensor-intro assumption assumption))
    ";
    let result = check_source(source);
    assert!(result.is_ok(), "structural double-use should be allowed: {:?}", result.err());
}

#[test]
fn reject_affine_triple_use() {
    // Three uses of one assumption in affine mode.
    let source = "
    (theory AffineTriple
      (context-mode affine)
      (sort Prop)
      (constructor tensor : (-> Prop Prop Prop))
      (judgment (holds ?A) :where A : Prop)
      (rule tensor-intro
        :premises ((holds ?A) (holds ?B))
        :conclusion (holds (tensor ?A ?B))))
    (proof triple-fail
      :theory AffineTriple
      :goal (holds (tensor a (tensor a a)))
      :assumptions ((holds a))
      :derivation (tensor-intro assumption (tensor-intro assumption assumption)))
    ";
    let result = check_source(source);
    assert!(result.is_err(), "triple use in affine should be rejected");
}
