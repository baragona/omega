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

#[test]
fn reject_duplicate_constructor() {
    let result = check_source(
        "(theory Bad
           (sort Prop)
           (constructor true : Prop)
           (constructor true : Prop)
           (judgment (proves ?P) :where P : Prop)
           (rule ax :premises () :conclusion (proves ?A)))",
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("duplicate constructor"),
        "expected 'duplicate constructor' in error, got: {}",
        err
    );
}

#[test]
fn reject_duplicate_judgment() {
    let result = check_source(
        "(theory Bad
           (sort Prop)
           (judgment (proves ?P) :where P : Prop)
           (judgment (proves ?P) :where P : Prop)
           (rule ax :premises () :conclusion (proves ?A)))",
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("duplicate judgment"),
        "expected 'duplicate judgment' in error, got: {}",
        err
    );
}

#[test]
fn reject_duplicate_rewrite() {
    // Duplicate rewrite names are detected during merge_from (import).
    // Base has the rewrite; Ext imports Base but also defines a rewrite with
    // the same name, causing a collision.
    let source = "
    (theory Base
      (sort Nat)
      (constructor z : Nat)
      (constructor s : (-> Nat Nat))
      (constructor add : (-> Nat Nat Nat))
      (judgment (eq ?a ?b) :where a : Nat b : Nat)
      (rewrite add-z (add ?n z) ?n)
      (rule refl :premises () :conclusion (eq ?a ?a)))
    (theory Bad
      (rewrite add-z (add ?n z) ?n)
      (import Base))
    ";
    let result = check_source(source);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("duplicate rewrite"),
        "expected 'duplicate rewrite' in error, got: {}",
        err
    );
}

#[test]
fn reject_duplicate_binding_spec() {
    // Duplicate binding-spec detection uses the (binding-spec ...) form.
    let result = check_source(
        "(theory Bad
           (binding-spec lam :binds 1 :scope (0))
           (binding-spec lam :binds 1 :scope (0) :linear)
           (sort Ty) (sort Tm)
           (constructor arr : (-> Ty Ty Ty))
           (judgment (has-type ?e ?T) :where e : Tm T : Ty)
           (rule t-lam
             :premises ((has-type ?body ?B))
             :conclusion (has-type (lam (x : ?A) ?body) (arr ?A ?B))))",
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("duplicate binding-spec"),
        "expected 'duplicate binding-spec' in error, got: {}",
        err
    );
}

#[test]
fn reject_goal_mismatch() {
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
      :assumptions ((proves p) (proves q))
      :goal (proves (and q p))
      :derivation (and-intro (assumption 0) (assumption 1)))
    ";
    let result = check_source(source);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("goal mismatch") || err.contains("pattern match failed"),
        "expected goal/pattern mismatch error, got: {}",
        err
    );
}

#[test]
fn reject_rewrite_meta_escape() {
    let result = check_source(
        "(theory Bad
           (sort Nat)
           (constructor z : Nat)
           (constructor s : (-> Nat Nat))
           (constructor add : (-> Nat Nat Nat))
           (judgment (eq ?a ?b) :where a : Nat b : Nat)
           (rewrite bad-rw (add ?n z) ?m)
           (rule refl :premises () :conclusion (eq ?a ?a)))",
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("RHS meta-variable") && err.contains("not in LHS"),
        "expected rewrite meta escape error, got: {}",
        err
    );
}

#[test]
fn reject_param_count_mismatch() {
    let source = "
    (theory Param
      :params ((T Type))
      (sort Prop)
      (judgment (proves ?P) :where P : Prop)
      (rule ax :premises () :conclusion (proves ?A)))
    (check-theory Param)
    (theory Bad
      (sort Nat)
      (constructor z : Nat)
      (import Param Nat z :as P))
    ";
    let result = check_source(source);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("expects") && err.contains("parameter"),
        "expected param count mismatch error, got: {}",
        err
    );
}

#[test]
fn reject_assumption_no_match() {
    let source = "
    (theory T
      (sort Prop)
      (judgment (proves ?P) :where P : Prop)
      (rule ax :premises () :conclusion (proves ?A)))
    (proof bad
      :theory T
      :assumptions ((proves p))
      :goal (proves q)
      :derivation (assumption))
    ";
    let result = check_source(source);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("no matching assumption"),
        "expected 'no matching assumption' error, got: {}",
        err
    );
}

#[test]
fn reject_assumption_index_out_of_bounds() {
    let source = "
    (theory T
      (sort Prop)
      (judgment (proves ?P) :where P : Prop)
      (rule ax :premises () :conclusion (proves ?A)))
    (proof bad
      :theory T
      :assumptions ((proves p))
      :goal (proves p)
      :derivation (assumption 5))
    ";
    let result = check_source(source);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("assumption index") && err.contains("out of bounds"),
        "expected 'assumption index out of bounds' error, got: {}",
        err
    );
}

#[test]
fn reject_affine_use_after_move() {
    // In affine context mode, consuming the same assumption twice triggers UseAfterMove.
    // We need 2 premises that both consume assumption 0 explicitly.
    let source = "
    (theory AffineMove
      (context-mode affine)
      (sort Prop)
      (constructor and : (-> Prop Prop Prop))
      (judgment (holds ?A) :where A : Prop)
      (rule and-intro
        :premises ((holds ?A) (holds ?B))
        :conclusion (holds (and ?A ?B))))
    (proof bad
      :theory AffineMove
      :goal (holds (and a a))
      :assumptions ((holds a))
      :derivation (and-intro (assumption 0) (assumption 0)))
    ";
    let result = check_source(source);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("already consumed") || err.contains("affine violation"),
        "expected affine use-after-move error, got: {}",
        err
    );
}

#[test]
fn reject_premise_check_failed() {
    // A valid rule application where the sub-derivation for a premise fails.
    let source = "
    (theory T
      (sort Prop)
      (constructor and : (-> Prop Prop Prop))
      (judgment (proves ?P) :where P : Prop)
      (rule and-intro
        :premises ((proves ?A) (proves ?B))
        :conclusion (proves (and ?A ?B)))
      (rule ax-p :premises () :conclusion (proves p)))
    (proof bad
      :theory T
      :goal (proves (and p q))
      :derivation (and-intro (ax-p) (assumption)))
    ";
    let result = check_source(source);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("no matching assumption") || err.contains("premise"),
        "expected premise check failure, got: {}",
        err
    );
}

#[test]
fn reject_non_exhaustive_metatheorem() {
    // Metatheorem must cover all rules that produce matching judgments.
    // Here we have two rules (ax-p, ax-q) but the metatheorem only handles ax-p.
    let source = "
    (theory T
      (sort Prop)
      (judgment (proves ?P) :where P : Prop)
      (rule ax-p :premises () :conclusion (proves p))
      (rule ax-q :premises () :conclusion (proves q)))
    (meta-theorem bad-meta
      :theory T
      :forall ((d (proves ?A)))
      :exists ((e (proves ?A)))
      :proof (case-analysis d
        (case ax-p ()
          (by-rule ax-p))))
    ";
    let result = check_source(source);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("non-exhaustive") || err.contains("missing"),
        "expected non-exhaustive error, got: {}",
        err
    );
}

#[test]
fn reject_metatheorem_rule_not_in_theory() {
    // Metatheorem proof uses a rule not in the theory.
    let source = "
    (theory T
      (sort Prop)
      (judgment (proves ?P) :where P : Prop)
      (rule ax :premises () :conclusion (proves ?A)))
    (theory T2
      (sort Prop)
      (judgment (proves ?P) :where P : Prop)
      (rule other-rule :premises () :conclusion (proves ?A)))
    (meta-theorem bad-meta
      :theory T
      :forall ((d (proves ?A)))
      :exists ((e (proves ?A)))
      :proof (case-analysis d
        (case ax ()
          (by-rule other-rule))))
    ";
    let result = check_source(source);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("not in theory") || err.contains("unknown rule"),
        "expected 'rule not in theory' error, got: {}",
        err
    );
}

#[test]
fn reject_reflection_no_existential() {
    // Reflecting a metatheorem that has no existential clause.
    let source = "
    (theory T
      (sort Prop)
      (judgment (proves ?P) :where P : Prop)
      (rule ax :premises () :conclusion (proves ?A)))
    (meta-theorem no-exist
      :theory T
      :forall ((d (proves ?A)))
      :proof (case-analysis d
        (case ax ()
          (by-rule ax))))
    (reflect no-exist :as derived :theory T)
    ";
    let result = check_source(source);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("no existential") || err.contains("nothing to reflect"),
        "expected 'no existential' error, got: {}",
        err
    );
}
