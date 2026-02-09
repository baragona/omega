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
fn reject_linear_double_use() {
    // linear-lam binder requires variable used exactly once; 2 uses should fail
    let source = "
    (theory LinearFail
      (binder-behavior linear-lam :substitutive :linear)
      (sort Ty) (sort Tm)
      (constructor A : Ty)
      (constructor pair : (-> Tm Tm Tm))
      (constructor lolli : (-> Ty Ty Ty))
      (constructor tensor : (-> Ty Ty Ty))
      (judgment (has-type ?e ?T) :where e : Tm T : Ty)
      (rule t-linear-lam
        :premises ((has-type ?body ?B))
        :conclusion (has-type (linear-lam (x : ?A) ?body) (lolli ?A ?B))))
    (proof bad
      :theory LinearFail
      :goal (has-type (linear-lam (x : A) (pair #0 #0)) (lolli A (tensor A A)))
      :derivation (t-linear-lam assumption)
      :assumptions ((has-type (pair #0 #0) (tensor A A))))
    ";
    let result = check_source(source);
    assert!(result.is_err(), "linear double-use should be rejected");
    let err = result.unwrap_err();
    assert!(
        err.contains("linear binder") && err.contains("2 times"),
        "expected linear violation error, got: {}",
        err
    );
}

#[test]
fn reject_linear_zero_use() {
    // linear-lam binder requires variable used exactly once; 0 uses should fail
    let source = "
    (theory LinearZero
      (binder-behavior linear-lam :substitutive :linear)
      (sort Ty) (sort Tm)
      (constructor A : Ty)
      (constructor star : Tm)
      (constructor lolli : (-> Ty Ty Ty))
      (constructor unit : Ty)
      (judgment (has-type ?e ?T) :where e : Tm T : Ty)
      (rule t-linear-lam
        :premises ((has-type ?body ?B))
        :conclusion (has-type (linear-lam (x : ?A) ?body) (lolli ?A ?B)))
      (rule t-star :premises () :conclusion (has-type star unit)))
    (proof bad
      :theory LinearZero
      :goal (has-type (linear-lam (x : A) star) (lolli A unit))
      :derivation (t-linear-lam (t-star)))
    ";
    let result = check_source(source);
    assert!(result.is_err(), "linear zero-use should be rejected");
    let err = result.unwrap_err();
    assert!(
        err.contains("linear binder") && err.contains("0 times"),
        "expected linear violation error, got: {}",
        err
    );
}

#[test]
fn reject_affine_double_use_binder() {
    // affine-lam binder requires variable used at most once; 2 uses should fail
    let source = "
    (theory AffineFail
      (binder-behavior affine-lam :substitutive :affine)
      (sort Ty) (sort Tm)
      (constructor A : Ty)
      (constructor pair : (-> Tm Tm Tm))
      (constructor lolli : (-> Ty Ty Ty))
      (constructor tensor : (-> Ty Ty Ty))
      (judgment (has-type ?e ?T) :where e : Tm T : Ty)
      (rule t-affine-lam
        :premises ((has-type ?body ?B))
        :conclusion (has-type (affine-lam (x : ?A) ?body) (lolli ?A ?B))))
    (proof bad
      :theory AffineFail
      :goal (has-type (affine-lam (x : A) (pair #0 #0)) (lolli A (tensor A A)))
      :derivation (t-affine-lam assumption)
      :assumptions ((has-type (pair #0 #0) (tensor A A))))
    ";
    let result = check_source(source);
    assert!(result.is_err(), "affine double-use should be rejected");
    let err = result.unwrap_err();
    assert!(
        err.contains("affine binder") && err.contains("2 times"),
        "expected affine violation error, got: {}",
        err
    );
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
