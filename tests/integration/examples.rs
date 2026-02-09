/// Integration tests: parse and check example .omega files end-to-end.
use omega_driver::batch;
use omega_driver::session::Session;

fn check_example(path: &str) -> Vec<String> {
    let mut session = Session::new();
    batch::process_file_path(&mut session, path).unwrap()
}

#[test]
fn prop_logic_example() {
    let results = check_example("examples/prop-logic.omega");
    assert!(results.iter().any(|r| r.contains("Theory PropLogic: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof identity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof weakening: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof and-comm: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof curry-and: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof or-comm: VALID")));
}

#[test]
fn stlc_example() {
    let results = check_example("examples/stlc.omega");
    assert!(results.iter().any(|r| r.contains("Theory STLC: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof identity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof const: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof church-false: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof apply-id-to-id: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof compose: VALID")));
}

#[test]
fn first_order_example() {
    let results = check_example("examples/first-order.omega");
    assert!(results.iter().any(|r| r.contains("Theory FOL: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof socrates-is-mortal: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof both-mortal: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof philosophers-are-mortal: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof teacher-is-mortal: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof greek-and-mortal: VALID")));
}

#[test]
fn reflection_demo_example() {
    let results = check_example("examples/reflection-demo.omega");
    assert!(results.iter().any(|r| r.contains("Theory SimpleLogic: registered OK")));
    assert!(results.iter().any(|r| r.contains("Metatheorem and-comm-meta: VERIFIED")));
    assert!(results.iter().any(|r| r.contains("Reflected and-comm-meta")));
    assert!(results.iter().any(|r| r.contains("Proof comm-test: VALID")));
}

#[test]
fn peano_example() {
    let results = check_example("examples/peano.omega");
    assert!(results.iter().any(|r| r.contains("Theory Peano: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof zero-plus-zero: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof zero-plus-one: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eq-combined: VALID")));
}

#[test]
fn zfc_example() {
    let results = check_example("examples/zfc.omega");
    assert!(results.iter().any(|r| r.contains("Theory ZFC: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof trivial: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof elem-pair-left: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof both-in-pair: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof subset-refl: VALID")));
    // Von Neumann ordinals
    assert!(results.iter().any(|r| r.contains("Proof zero-in-one: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof zero-in-five: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof one-in-two: VALID")));
    // Equality chains (tests freshened metas in nested eq-trans)
    assert!(results.iter().any(|r| r.contains("Proof eq-chain-2: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eq-chain-3: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eq-trans-trivial: VALID")));
    // Deep conjunction trees
    assert!(results.iter().any(|r| r.contains("Proof zero-in-ordinals: VALID")));
    // Axiom of Choice
    assert!(results.iter().any(|r| r.contains("Proof choice-basic: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof choice-singleton: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof choice-pair: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof choice-succ: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof choice-in-union: VALID")));
}

#[test]
fn modal_logic_example() {
    let results = check_example("examples/modal-logic.omega");
    assert!(results.iter().any(|r| r.contains("Theory ModalS5: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof K-theorem: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof box-and: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof box-dist: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof box-box: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof necessary-possible: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof possible-necessarily-possible: VALID")));
}

#[test]
fn linear_logic_example() {
    let results = check_example("examples/linear-logic.omega");
    assert!(results.iter().any(|r| r.contains("Theory LinearLogic: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof lolli-id: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof tensor-pair: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof tensor-comm: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof bang-dup: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof compose: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof uncurry: VALID")));
}

#[test]
fn implicit_demo_example() {
    let results = check_example("examples/implicit-demo.omega");
    assert!(results.iter().any(|r| r.contains("Theory ImplicitDemo: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof zero-eq-zero: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof trans-trivial: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof double-cong: VALID")));
}

#[test]
fn peano_compute_example() {
    let results = check_example("examples/peano-compute.omega");
    assert!(results.iter().any(|r| r.contains("Theory PeanoCompute: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof zero-plus-zero: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof one-plus-one: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof two-plus-one: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof two-plus-three: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof two-times-three: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof three-times-three: VALID")));
}

#[test]
fn affine_logic_example() {
    let results = check_example("examples/affine-logic.omega");
    assert!(results.iter().any(|r| r.contains("Theory AffineLogic: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof tensor-pair: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof unit-free: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof tensor-with-unit: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof shadow-use: VALID")));
}

#[test]
fn number_theory_example() {
    let results = check_example("examples/number-theory.omega");
    assert!(results.iter().any(|r| r.contains("Theory PeanoInduction: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof add-right-zero: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof add-right-succ: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof add-comm: VALID")));
}

#[test]
fn monoid_example() {
    let results = check_example("examples/monoid.omega");
    assert!(results.iter().any(|r| r.contains("Theory EqTheory: registered OK")));
    assert!(results.iter().any(|r| r.contains("Theory PeanoEq: registered OK")));
    assert!(results.iter().any(|r| r.contains("Theory TwoSorted: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof nat-refl: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof nat-symm: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof nat-trans: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof bool-refl: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof bool-symm: VALID")));
}

#[test]
fn sequent_calc_example() {
    let results = check_example("examples/sequent-calc.omega");
    assert!(results.iter().any(|r| r.contains("Theory SequentCalc: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof identity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof impl-refl: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof conj-comm: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof disj-comm: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof weakening-thm: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof modus-ponens: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof extract-direct: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof extract-via-cut: VALID")));
}

#[test]
fn hoare_logic_example() {
    let results = check_example("examples/hoare-logic.omega");
    assert!(results.iter().any(|r| r.contains("Theory HoareLogic: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof skip-rule: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof assign-const: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof increment: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof seq-assign: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof conditional: VALID")));
}

#[test]
fn option_lib() {
    let results = check_example("libs/option.omega");
    assert!(results.iter().any(|r| r.contains("Theory Option: registered OK")));
}

#[test]
fn result_lib() {
    let results = check_example("libs/result.omega");
    assert!(results.iter().any(|r| r.contains("Theory Result: registered OK")));
}

#[test]
fn pair_lib() {
    let results = check_example("libs/pair.omega");
    assert!(results.iter().any(|r| r.contains("Theory Pair: registered OK")));
}

#[test]
fn compiler_demo_example() {
    let results = check_example("examples/compiler-demo.omega");
    assert!(results.iter().any(|r| r.contains("Theory CompilerDemo: registered OK")));
    // Single-parameter instantiation
    assert!(results.iter().any(|r| r.contains("Proof opt-refl-zero: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof opt-symm-test: VALID")));
    // Multi-parameter instantiation
    assert!(results.iter().any(|r| r.contains("Proof res-ok-refl: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof res-err-refl: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof res-symm-test: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof res-trans-test: VALID")));
    // Pair with affine context
    assert!(results.iter().any(|r| r.contains("Proof pair-refl-test: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof pair-symm-test: VALID")));
    // Cross-module
    assert!(results.iter().any(|r| r.contains("Proof opt-none-refl: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof res-ok-one-refl: VALID")));
}

#[test]
fn rust_types() {
    let results = check_example("libs/omega-rust/rust-types.omega");
    assert!(results.iter().any(|r| r.contains("Theory RustTypes: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof box-u32-eq: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof static-outlives-any: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof u32-is-copy: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof pair-copy: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof option-u32-copy: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof static-ref-subtype: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof subtype-chain: VALID")));
}

#[test]
fn borrow_checker() {
    let results = check_example("libs/omega-rust/borrow.omega");
    assert!(results.iter().any(|r| r.contains("Theory BorrowChecker: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof type-literal-u32: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof box-construction: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof box-deref-literal: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof box-move-from-context: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof resource-split: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof nested-box-pair: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ref-creation: VALID")));
}

#[test]
fn rust_eval() {
    let results = check_example("libs/omega-rust/eval.omega");
    assert!(results.iter().any(|r| r.contains("Theory RustEval: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof eval-fst-pair: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eval-deref-box: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eval-unwrap-some: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eval-is-some-true: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eval-if-true: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eval-one-plus-two: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eval-compound: VALID")));
}

#[test]
fn torture_example() {
    let results = check_example("examples/torture.omega");
    assert!(results.iter().any(|r| r.contains("Theory TortureArith: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof zero-eq-zero: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof add-refl: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof double-2: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof double-3: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cong-succ-zero: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cong-add-zero: VALID")));
}

#[test]
fn string_lib() {
    let results = check_example("libs/string.omega");
    assert!(results.iter().any(|r| r.contains("Theory StringLib: registered OK")));
}

#[test]
fn codegen_demo_example() {
    let results = check_example("examples/codegen-demo.omega");
    assert!(results.iter().any(|r| r.contains("Theory StringLib: registered OK")));
    assert!(results.iter().any(|r| r.contains("Theory CodeGen: registered OK")));
    // Check that emit produces actual C code
    assert!(results.iter().any(|r| r.contains("hello, world")));
    assert!(results.iter().any(|r| r.contains("return 0;")));
    assert!(results.iter().any(|r| r.contains("int main()")));
    assert!(results.iter().any(|r| r.contains("#include <stdio.h>")));
    assert!(results.iter().any(|r| r.contains("no empty")));
}

#[test]
fn compile_factorial_example() {
    let results = check_example("examples/compile-factorial.omega");
    // Act I: Verified semantics
    assert!(results.iter().any(|r| r.contains("Theory PeanoFact: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof fact-0: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof fact-1: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof fact-2: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof fact-3: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof fact-4: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof fact-5: VALID")));
    // Act II: Compiler backend
    assert!(results.iter().any(|r| r.contains("Theory Compiler: registered OK")));
    // Act III: Emitted C code
    assert!(results.iter().any(|r| r.contains("int factorial(int n)")));
    assert!(results.iter().any(|r| r.contains("return 1;")));
    assert!(results.iter().any(|r| r.contains("n * factorial((n - 1))")));
}

#[test]
fn classical_logic_example() {
    let results = check_example("examples/classical-logic.omega");
    assert!(results.iter().any(|r| r.contains("Theory ClassicalLogic: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof dne-thm: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof lem: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof peirce: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof contraposition: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof not-imp-left: VALID")));
}

#[test]
fn dep_types_example() {
    let results = check_example("examples/dep-types.omega");
    assert!(results.iter().any(|r| r.contains("Theory DepTypes: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof eq-z-z: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof dep-refl: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof pi-id: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof pi-app-id: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof dep-app: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof nested-pi: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof partial-app: VALID")));
}

#[test]
fn compile_verified_example() {
    let results = check_example("examples/compile-verified.omega");
    // Theories
    assert!(results.iter().any(|r| r.contains("Theory RustAST: registered OK")));
    assert!(results.iter().any(|r| r.contains("Theory Eval: registered OK")));
    assert!(results.iter().any(|r| r.contains("Theory Compiler: registered OK")));
    // HOAS verification proofs (Act I)
    assert!(results.iter().any(|r| r.contains("Proof double-0: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof double-1: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof double-3: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof square-3: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof square-4: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof abs-0: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof abs-3: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof fact-3: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof fact-4: VALID")));
    // HOAS compilation output (Act III)
    assert!(results.iter().any(|r| r.contains("return x + x;")));
    assert!(results.iter().any(|r| r.contains("return x * x;")));
    assert!(results.iter().any(|r| r.contains("int factorial(int n)")));
    assert!(results.iter().any(|r| r.contains("n * factorial(n - 1)")));
}

#[test]
fn w_types_example() {
    let results = check_example("examples/w-types.omega");
    assert!(results.iter().any(|r| r.contains("Theory WTypes: registered OK")));
    // Universe tests
    assert!(results.iter().any(|r| r.contains("Proof type-0-in-1: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof lsuc-test: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof lmax-test: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof imax-prop: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof imax-pred: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof lmax-comm: VALID")));
    // Sigma tests
    assert!(results.iter().any(|r| r.contains("Proof fst-test: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof snd-test: VALID")));
    // Computation
    assert!(results.iter().any(|r| r.contains("Proof add-test: VALID")));
    // W-type wrec
    assert!(results.iter().any(|r| r.contains("Proof wrec-test: VALID")));
}

#[test]
fn hott_example() {
    let results = check_example("examples/hott.omega");
    assert!(results.iter().any(|r| r.contains("Theory HoTTBase: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof refl-z: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof inv-refl-eq: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof concat-refl-refl: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof transport-refl-id: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ap-refl-eq: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof left-unit: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof right-unit: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof left-inverse: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof right-inverse: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof inv-involution: VALID")));
}

#[test]
fn category_theory_example() {
    let results = check_example("examples/category-theory.omega");
    assert!(results.iter().any(|r| r.contains("Theory Category: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof left-identity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof right-identity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof associativity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof functor-id: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof functor-comp: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof psi-unfold: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof yoneda-roundtrip: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof phi-unfold: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof yoneda-psi-phi: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof composite-functor-id: VALID")));
}

#[test]
fn induction_recursion_example() {
    let results = check_example("examples/induction-recursion.omega");
    assert!(results.iter().any(|r| r.contains("Theory IR: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof decode-nat: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof decode-bool: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof decode-pi: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof decode-sigma: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof type-at-decoded: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof refl-decoded-id: VALID")));
}

#[test]
fn hits_example() {
    let results = check_example("examples/hits.omega");
    assert!(results.iter().any(|r| r.contains("Theory HITs: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof recS1-at-base: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof loop-is-path: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof recSusp-at-north: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof recSusp-at-south: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof trunc-intro: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof merid-is-path: VALID")));
}

#[test]
fn level_poly_example() {
    let results = check_example("examples/level-poly.omega");
    assert!(results.iter().any(|r| r.contains("Theory LevelPoly: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof type-0-in-1: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof nil-nat: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof nil-type-0: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof id-nat-refl: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cons-ze-nil: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof const-reduces: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof type-1-in-2: VALID")));
}

#[test]
fn eta_demo_example() {
    let results = check_example("examples/eta-demo.omega");
    assert!(results.iter().any(|r| r.contains("Theory EtaDemo: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof eta-basic: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eta-compound: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eta-nested: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof comp-id-left: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof comp-id-right: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eta-inside-comp: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof no-eta-when-used: VALID")));
}

#[test]
fn linear_demo_example() {
    let results = check_example("examples/linear-demo.omega");
    assert!(results.iter().any(|r| r.contains("Theory LinearDemo: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof linear-id: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof affine-use-once: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof affine-unused: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof standard-double: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof standard-unused: VALID")));
}

#[test]
fn ac_demo_example() {
    let results = check_example("examples/ac-demo.omega");
    assert!(results.iter().any(|r| r.contains("Theory ACDemo: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof ac-comm: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ac-assoc: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ac-nested: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ac-four: VALID")));
    assert!(results.iter().any(|r| r.contains("Theory ACIDemo: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof aci-idem: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof aci-comm-idem: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof aci-absorb: VALID")));
}
