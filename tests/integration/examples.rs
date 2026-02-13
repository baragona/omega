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
    // New: deep derivation trees
    assert!(results.iter().any(|r| r.contains("Proof s-combinator: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof church-pair: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof flip: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eta-expansion: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof church-two: VALID")));
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
    // Two metatheorems: and-comm and or-comm
    assert!(results.iter().any(|r| r.contains("Metatheorem and-comm-meta: VERIFIED")));
    assert!(results.iter().any(|r| r.contains("Reflected and-comm-meta")));
    assert!(results.iter().any(|r| r.contains("Metatheorem or-comm-meta: VERIFIED")));
    assert!(results.iter().any(|r| r.contains("Reflected or-comm-meta")));
    // Proofs using reflected rules
    assert!(results.iter().any(|r| r.contains("Proof comm-test: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof comm-roundtrip: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof elim-from-assumption: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof comm-both-intro: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof comm-nested: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof comm-contraction: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof or-comm-test: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof or-intro-from-assumption: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof and-to-or: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof or-comm-contraction: VALID")));
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
    assert!(results.iter().any(|r| r.contains("Proof succ-zero-eq: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof symm-succ: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof trans-trivial: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof double-cong: VALID")));
    // New: multi-step proofs with implicit inference
    assert!(results.iter().any(|r| r.contains("Proof deep-cong-assumption: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cong-trans-chain: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof symm-assumption: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof triple-trans: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof deep-cong-symm: VALID")));
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
    // New: linear implication + multi-step
    assert!(results.iter().any(|r| r.contains("Proof lolli-id: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof modus-ponens: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof tensor-comm: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof linear-compose: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof tensor-unit-right: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof apply-and-unit: VALID")));
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
    assert!(results.iter().any(|r| r.contains("Theory MonoidTheory: registered OK")));
    assert!(results.iter().any(|r| r.contains("Theory NatMonoid: registered OK")));
    assert!(results.iter().any(|r| r.contains("Theory BoolMonoid: registered OK")));
    // Nat monoid proofs
    assert!(results.iter().any(|r| r.contains("Proof nat-refl: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof nat-symm: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof nat-triple-assoc: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof nat-unit-simplify: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof nat-cong-both: VALID")));
    // Bool monoid proofs
    assert!(results.iter().any(|r| r.contains("Proof bool-refl: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof bool-assoc: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof bool-left-id: VALID")));
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
    // New HOAS functions
    assert!(results.iter().any(|r| r.contains("Proof triple-0: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof triple-1: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof triple-2: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof is-zero-0: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof is-zero-1: VALID")));
    // Multi-step equational reasoning
    assert!(results.iter().any(|r| r.contains("Proof fact-3-symm: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof double-cong: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof succ-succ-cong: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof square-cong-l: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof triple-cong: VALID")));
    // HOAS compilation output (Act III)
    assert!(results.iter().any(|r| r.contains("return x + x;")));
    assert!(results.iter().any(|r| r.contains("return x * x;")));
    assert!(results.iter().any(|r| r.contains("return x + x + x;")));
    assert!(results.iter().any(|r| r.contains("return x == 0;")));
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
    assert!(results.iter().any(|r| r.contains("Theory HoTT: registered OK")));
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
    assert!(results.iter().any(|r| r.contains("Proof tfam-refl-eq: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof transport-concat: VALID")));
}

#[test]
fn category_theory_example() {
    let results = check_example("examples/category-theory.omega");
    assert!(results.iter().any(|r| r.contains("Theory Category: registered OK")));
    // Part 1: Definitional equalities
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
    // Part 2: Equational reasoning (multi-step)
    assert!(results.iter().any(|r| r.contains("Proof yoneda-full-roundtrip: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof comp-cong-both: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof functor-respects-eq: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cancel-right: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eq-from-assumption: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof functor-trans-chain: VALID")));
}

#[test]
fn induction_recursion_example() {
    let results = check_example("examples/induction-recursion.omega");
    assert!(results.iter().any(|r| r.contains("Theory IR: registered OK")));
    // Part 1: Decoding + basic typing
    assert!(results.iter().any(|r| r.contains("Proof decode-nat: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof decode-bool: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof decode-pi: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof decode-sigma: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof type-at-decoded: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof refl-decoded-id: VALID")));
    // Part 2: Equational reasoning over decoded types
    assert!(results.iter().any(|r| r.contains("Proof decode-nat-symm: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof decoded-trans: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof pi-cong-from-assumption: VALID")));
    // Part 3: Multi-step typing
    assert!(results.iter().any(|r| r.contains("Proof type-two: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof succ-at-decoded: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof refl-decoded-bool-id: VALID")));
}

#[test]
fn hits_example() {
    let results = check_example("examples/hits.omega");
    assert!(results.iter().any(|r| r.contains("Theory HITs: registered OK")));
    // Part 1: Computation rules
    assert!(results.iter().any(|r| r.contains("Proof recS1-at-base: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof loop-is-path: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof recSusp-at-north: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof recSusp-at-south: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof trunc-intro: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof merid-is-path: VALID")));
    // Part 2: Multi-step typing derivations
    assert!(results.iter().any(|r| r.contains("Proof double-loop: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof inv-loop: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof susp-north-loop: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof squash-proof: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof recS1-succ: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof loop-right-unit: VALID")));
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
    // New: multi-step typing
    assert!(results.iter().any(|r| r.contains("Proof type-three: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof nested-cons: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof pi-generic: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof refl-succ: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof nil-nested-list: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof const-symm: VALID")));
}

#[test]
fn eta_demo_example() {
    let results = check_example("examples/eta-demo.omega");
    assert!(results.iter().any(|r| r.contains("Theory EtaDemo: registered OK")));
    // Part 1: Eta-contraction (all meq-refl)
    assert!(results.iter().any(|r| r.contains("Proof eta-basic: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eta-compound: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eta-nested: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof comp-id-left: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof comp-id-right: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eta-inside-comp: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof no-eta-when-used: VALID")));
    // Part 2: Eta + equational reasoning from assumptions
    assert!(results.iter().any(|r| r.contains("Proof eta-from-assumption: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cong-from-assumption: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof double-cong-comp: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof symm-from-assumption: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eta-comp-rewrite: VALID")));
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
    // Part 1: AC normalization (eq-refl)
    assert!(results.iter().any(|r| r.contains("Proof ac-comm: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ac-assoc: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ac-nested: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ac-four: VALID")));
    // Part 2: AC + equational reasoning
    assert!(results.iter().any(|r| r.contains("Proof ac-symm: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ac-trans: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ac-cong: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ac-trans-normalized: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ac-symm-cong: VALID")));
    // ACI
    assert!(results.iter().any(|r| r.contains("Theory ACIDemo: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof aci-idem: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof aci-comm-idem: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof aci-absorb: VALID")));
}

#[test]
fn calc_example() {
    let results = check_example("examples/calc.omega");
    assert!(results.iter().any(|r| r.contains("Theory Calc: registered OK")));
    // Arithmetic proofs
    assert!(results.iter().any(|r| r.contains("Proof add-2-3: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof mul-2-3: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof sub-5-3: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof sub-3-5: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof pow-2-3: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof fact-4: VALID")));
    // Comparison proofs
    assert!(results.iter().any(|r| r.contains("Proof lt-true: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof lt-false: VALID")));
    // Expression evaluation proofs
    assert!(results.iter().any(|r| r.contains("Proof eval-add: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eval-mul: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eval-pow: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eval-fact: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eval-if-truthy: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eval-if-falsy: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eval-min: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eval-max: VALID")));
}

#[test]
fn separation_logic_example() {
    let results = check_example("examples/separation-logic.omega");
    assert!(results.iter().any(|r| r.contains("Theory SeparationLogic: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof skip-emp: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof frame-mutate: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof wand-elim: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof swap-xy: VALID")));
}

#[test]
fn temporal_logic_example() {
    let results = check_example("examples/temporal-logic.omega");
    assert!(results.iter().any(|r| r.contains("Theory LTL: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof always-to-now: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof always-distributes: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof until-guarantees: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof next-distributes: VALID")));
}

#[test]
fn lambek_example() {
    let results = check_example("examples/lambek.omega");
    assert!(results.iter().any(|r| r.contains("Theory Lambek: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof transitive-verb: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof type-raising: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof left-compose: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof full-sentence: VALID")));
}

#[test]
fn provability_logic_example() {
    let results = check_example("examples/provability-logic.omega");
    assert!(results.iter().any(|r| r.contains("Theory GL: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof lob-theorem: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof godel-two: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof box-and-intro: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof lob-lifts: VALID")));
}

#[test]
fn relevant_logic_example() {
    let results = check_example("examples/relevant-logic.omega");
    assert!(results.iter().any(|r| r.contains("Theory RelevantLogic: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof identity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof syllogism: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof currying: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof fusion-symmetric: VALID")));
}

#[test]
fn girard_bridge_example() {
    let results = check_example("examples/girard.omega");
    assert!(results.iter().any(|r| r.contains("Theory GirardBridge: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof c-identity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof compile-identity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof l-identity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof compile-contraction: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof l-contraction: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof l-chain: VALID")));
}

#[test]
fn glivenko_bridge_example() {
    let results = check_example("examples/glivenko.omega");
    assert!(results.iter().any(|r| r.contains("Theory GlivenkoBridge: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof c-lem: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof compile-lem: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof i-lem: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof i-peirce: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof i-dummett: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof i-contraposition: VALID")));
}

#[test]
fn collapse_filter_example() {
    let results = check_example("examples/collapse.omega");
    assert!(results.iter().any(|r| r.contains("Theory CollapseFilter: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof p-paradox: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof filter-paradox: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof c-dne-thm: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof c-lem: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof filter-tautology: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof c-composition: VALID")));
}

#[test]
fn cut_elim_example() {
    let results = check_example("examples/cut-elim.omega");
    assert!(results.iter().any(|r| r.contains("Theory LinearMachine: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof beta-identity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cut-let: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof contraction: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof double-two: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof compose-succ-succ: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof girard-roundtrip: VALID")));
}

#[test]
fn category_example() {
    let results = check_example("examples/category.omega");
    assert!(results.iter().any(|r| r.contains("Theory CCC: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof identity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof composition: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof diagonal: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof swap: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof product-beta-1: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof product-beta-2: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof modus-ponens: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof weakening: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof syllogism: VALID")));
}

#[test]
fn temporal_bridge_example() {
    let results = check_example("examples/temporal.omega");
    assert!(results.iter().any(|r| r.contains("Theory TemporalBridge: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof light-go: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof light-cycle: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof light-reach-red: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof mutex-acquire: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof mutex-protocol: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof mutex-safe-free: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof mutex-reach-free: VALID")));
}

#[test]
fn monad_bridge_example() {
    let results = check_example("examples/monad.omega");
    assert!(results.iter().any(|r| r.contains("Theory KleisliMonad: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof kleisli-return: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof kleisli-compose: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof monad-left-id: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof monad-right-id: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof counter-inc-inc: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof hoare-inc-inc: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof hoare-inc-inc-inc: VALID")));
}

#[test]
fn separation_bi_example() {
    let results = check_example("examples/separation.omega");
    assert!(results.iter().any(|r| r.contains("Theory BunchedImplications: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof classical-contraction: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof star-commutative: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof distribution-bridge: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof wand-modus-ponens: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof heap-sharing: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof swap-safe: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof framed-write: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof write-then-swap: VALID")));
}

#[test]
fn topos_example() {
    let results = check_example("examples/topos.omega");
    assert!(results.iter().any(|r| r.contains("Theory Topos: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof bool-dne-true: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof bool-dne-false: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof bool-lem: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof heyt-dne-unknown: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof heyt-lem-fails: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof heyt-peirce-fails: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof bool-noncontradiction: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof heyt-noncontradiction: VALID")));
}

#[test]
fn godel_example() {
    let results = check_example("examples/godel.omega");
    assert!(results.iter().any(|r| r.contains("Theory GodelGL: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof box-top: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof box-identity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof introspection: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof lob-instance: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof godel-implies-unprovable: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof unprovable-implies-godel: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof box-godel-fwd: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof second-incompleteness: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof distribution: VALID")));
}

#[test]
fn game_semantics_example() {
    let results = check_example("examples/game.omega");
    assert!(results.iter().any(|r| r.contains("Theory GameSemantics: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof trivial-game: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof copycat: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof constant-strategy: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof fork-strategy: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof case-analysis: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof peirce-game: VALID")));
}

#[test]
fn pi_calculus_example() {
    let results = check_example("examples/pi.omega");
    assert!(results.iter().any(|r| r.contains("Theory SessionPi: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof request-response-duality: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof server-typed: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof client-typed: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof request-response-safe: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof request-response-terminates: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof auth-duality: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof auth-server-typed: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof auth-accept-safe: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof auth-accept-terminates: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof auth-reject-terminates: VALID")));
}

#[test]
fn cubical_example() {
    let results = check_example("examples/cubical.omega");
    assert!(results.iter().any(|r| r.contains("Theory CubicalTT: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof refl-left: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof refl-right: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof sym-computes: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof transport-refl-computes: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof funext-left: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof neg-involution: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ua-computes: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ua-roundtrip: VALID")));
}

#[test]
fn quotients_example() {
    let results = check_example("examples/quotients.omega");
    assert!(results.iter().any(|r| r.contains("Theory QuotientInt: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof zero-equiv: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof zero-equiv-2: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof two-equiv: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof neg-two-equiv: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof add-one-one: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof neg-two: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof add-commutes: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof add-well-defined: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof neg-well-defined: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof subtract-works: VALID")));
}

#[test]
fn effects_example() {
    let results = check_example("examples/effects.omega");
    assert!(results.iter().any(|r| r.contains("Theory AlgEffects: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof safe-pure: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof safe-crash: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof safe-state: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof log-pure: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof log-error: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof log-state-error: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof all-pure: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof all-choose: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof all-choose-error: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof all-state-choose: VALID")));
}

#[test]
fn inference_example() {
    let results = check_example("examples/inference.omega");
    assert!(results.iter().any(|r| r.contains("Theory TypeInference: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof type-literal: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof type-var: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof type-identity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof type-apply: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof type-compose: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof subst-partial: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof subst-full: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof subst-compound: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof unify-same: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof unify-clash: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof unify-var: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof unify-arrow: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof sound-same: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof sound-var: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof sound-arrow: VALID")));
}

#[test]
fn large_cardinals_example() {
    let results = check_example("examples/large-cardinals.omega");
    assert!(results.iter().any(|r| r.contains("Theory LargeCardinals: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof ordinal-zero-lt-one: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ordinal-zero-lt-two: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof aleph-zero-is-infinite: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cantor-theorem: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof inacc-from-components: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof inacc-implies-weakly: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof inacc-properties: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof measurable-is-regular: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof measurable-is-infinite: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof reflection-principle: VALID")));
}

#[test]
fn ordinal_arithmetic_example() {
    let results = check_example("examples/ordinal-arithmetic.omega");
    assert!(results.iter().any(|r| r.contains("Theory OrdinalArith: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof add-zero-left: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof add-zero-right: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof mul-one-right: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof mul-zero-annihilates: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof expw-zero-is-one: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof expw-one-is-omega: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof veb-zero-is-expw: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eps-is-veb-one: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof eps-zero-fixed-point: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof twin-fixed-points: VALID")));
}

#[test]
fn surreal_numbers_example() {
    let results = check_example("examples/surreal-numbers.omega");
    assert!(results.iter().any(|r| r.contains("Theory SurrealNumbers: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof zero-plus-zero: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof neg-zero-is-zero: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof double-negation: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof mul-one-identity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof mul-zero-annihilates: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof zero-leq-one: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof epsilon-positive: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof epsilon-less-than-half: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof omega-plus-one-gt-omega: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof infinite-and-infinitesimal: VALID")));
}

#[test]
fn continuum_example() {
    let results = check_example("examples/continuum.omega");
    assert!(results.iter().any(|r| r.contains("Theory ContinuumHypothesis: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof b-and-identity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof b-or-identity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof b-and-annihilate: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof b-double-negation: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof b-imp-unfold: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof b-imp-top-left: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cantor-proof: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ch-is-consistent: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ch-is-independent: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof full-picture: VALID")));
}

#[test]
fn club_filter_example() {
    let results = check_example("examples/club-filter.omega");
    assert!(results.iter().any(|r| r.contains("Theory ClubFilter: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof intersect-idempotent: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof intersect-empty: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof intersect-full: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof full-set-is-club: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof club-intersection-is-club: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof club-implies-stationary: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof stationary-meets-every-club: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof fodor-lemma: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof club-decomposition: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof club-full-properties: VALID")));
}

#[test]
fn refinement_example() {
    let results = check_example("examples/refinement.omega");
    assert!(results.iter().any(|r| r.contains("Theory RefinementTypes: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof pos-sub-nonzero: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof pos-sub-nat: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof arrow-subtype: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof one-is-pos: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof one-is-nat: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof two-is-nonzero: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof safe-div: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof add-one-one: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof decision-computes: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof decide-pos-typing: VALID")));
}

#[test]
fn bidirectional_example() {
    let results = check_example("examples/bidirectional.omega");
    assert!(results.iter().any(|r| r.contains("Theory BiDi: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof synth-zero: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof synth-true: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof synth-succ: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof synth-var: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof check-id-nat: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof check-id-bool: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof check-const: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof synth-annotated: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof synth-app-nat: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof synth-app-bool: VALID")));
}

#[test]
fn nominal_example() {
    let results = check_example("examples/nominal.omega");
    assert!(results.iter().any(|r| r.contains("Theory NominalLogic: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof swap-hit: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof swap-miss: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof swap-through-binder: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof fresh-different: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof fresh-bound: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof fresh-other-binder: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof fresh-in-app: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof alpha-identity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof alpha-self: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof alpha-under-app: VALID")));
}

#[test]
fn streams_example() {
    let results = check_example("examples/streams.omega");
    assert!(results.iter().any(|r| r.contains("Theory Streams: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof hd-ones-is-one: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof hd-nats-zero: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof hd-map-succ-zeros: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof hd-zip-ones: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof second-nat: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof bisim-ones-self: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof bisim-ones-const: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof bisim-map-succ: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof bisim-zeros-const: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof bisim-add-zero: VALID")));
}

#[test]
fn zk_circuit_example() {
    let results = check_example("examples/zk-circuit.omega");
    assert!(results.iter().any(|r| r.contains("Theory ZKCircuit: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof two-times-three: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof three-times-five: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof one-plus-two: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof gate-2x3: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof gate-3x5: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof circuit-two-gates: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof gate-5x3: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof gate-1x15: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof three-squared: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof circuit-complex: VALID")));
}

#[test]
fn forcing_example() {
    let results = check_example("examples/forcing.omega");
    assert!(results.iter().any(|r| r.contains("Theory Forcing: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof beth-zero-is-aleph-zero: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cantor-theorem: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof L-models-set-theory: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof L-satisfies-CH: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof godel-1938: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cohen-poset-ccc: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof forcing-preserves: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cohen-negates-CH: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cohen-1963: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ch-independent: VALID")));
}

#[test]
fn ch_complete_example() {
    let results = check_example("examples/ch-complete.omega");
    assert!(results.iter().any(|r| r.contains("Theory CH-Independence: registered OK")));
    // Part A: Ordinal & Cardinal Arithmetic
    assert!(results.iter().any(|r| r.contains("Proof zero-lt-one: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof zero-lt-two: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof omega-is-limit-proof: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cantor: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cantor-aleph-one: VALID")));
    // Part B: Gödel's Constructible Universe
    assert!(results.iter().any(|r| r.contains("Proof gch-implies-ch-proof: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof godel-consistency: VALID")));
    // Part C: Cohen Forcing
    assert!(results.iter().any(|r| r.contains("Proof cohen-consistency: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cohen-ccc: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof aleph-two-gt-aleph-one: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof forcing-preserves: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof zero-lt-three: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ext-is-zfc-axiom: VALID")));
    // Part D: The Independence Theorem
    assert!(results.iter().any(|r| r.contains("Proof ch-independent: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof model-witnesses: VALID")));
}

#[test]
fn zfc_foundations_example() {
    let results = check_example("examples/zfc-foundations.omega");
    assert!(results.iter().any(|r| r.contains("Theory ZFC-Foundations: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof zero-in-omega: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof one-in-omega: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof two-in-omega: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof zero-is-ordinal: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof one-is-ordinal: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof two-is-ordinal: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof omega-is-ordinal-proof: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof zero-lt-one: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof zero-lt-two: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof one-lt-two: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof empty-has-no-elements: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof empty-in-power: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof self-in-power: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof kpair-computes: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof kpair-injective: VALID")));
}

#[test]
fn zfc_cardinals_example() {
    let results = check_example("examples/zfc-cardinals.omega");
    assert!(results.iter().any(|r| r.contains("Theory ZFC-Foundations: registered OK")));
    assert!(results.iter().any(|r| r.contains("Theory ZFC-Cardinals: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof id-maps-proof: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof id-bij-proof: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof card-le-refl: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof card-eq-refl: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cantor-theorem-proof: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof omega-lt-continuum: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof aleph-0-lt-1: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof aleph-0-lt-2: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof godel-proof: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cohen-proof: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ch-is-independent: VALID")));
}

#[test]
fn zfc_independence_example() {
    let results = check_example("examples/zfc-independence.omega");
    // Three theories, layered
    assert!(results.iter().any(|r| r.contains("Theory ZFC-Base: registered OK")));
    assert!(results.iter().any(|r| r.contains("Theory ZFC-Cardinals: registered OK")));
    assert!(results.iter().any(|r| r.contains("Theory ZFC-Independence: registered OK")));
    // Foundation proofs (ZFC-Base)
    assert!(results.iter().any(|r| r.contains("Proof zero-in-omega: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof two-in-omega: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof zero-lt-two: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof empty-in-power: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof kpair-injective: VALID")));
    // Cardinals proofs (ZFC-Cardinals)
    assert!(results.iter().any(|r| r.contains("Proof id-bij: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cantor-theorem-proof: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof aleph-0-lt-2: VALID")));
    // Gödel's L (ZFC-Independence)
    assert!(results.iter().any(|r| r.contains("Proof L-is-transitive: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof L-ext: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof L-choice: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof gch-implies-ch-proof: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof godel-consistency: VALID")));
    // Cohen forcing (ZFC-Independence)
    assert!(results.iter().any(|r| r.contains("Proof cohen-ccc: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof forcing-preserves: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cohen-consistency: VALID")));
    // The crown jewel
    assert!(results.iter().any(|r| r.contains("Proof ch-independent: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof omega-is-limit: VALID")));
}

#[test]
fn zfc_honest_example() {
    let results = check_example("examples/zfc-honest.omega");
    assert!(results.iter().any(|r| r.contains("Theory CH-Honest: registered OK")));
    // Phase 9: derived from pure logic (bic-absurd) and factored axioms
    assert!(results.iter().any(|r| r.contains("Lemma bic-absurd: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma cohen-is-poset: VALID [DERIVED]")));
    // Phase 9: L-construction infrastructure (from Def operator)
    assert!(results.iter().any(|r| r.contains("Lemma L-contains-empty: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma L-contains-omega: VALID [DERIVED]")));
    // Phase 13: L-hierarchy decomposition (L-transitive, L-def-closed from sub-results)
    assert!(results.iter().any(|r| r.contains("Lemma L-transitive: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma L-def-closed: VALID [DERIVED]")));
    // Phase 9: absoluteness from Δ₀ general principle
    assert!(results.iter().any(|r| r.contains("Lemma abs-ext: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma abs-reg: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma abs-empty: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma abs-inf: VALID [DERIVED]")));
    // Phase 9: satisfaction bridge (syntax/semantics separation)
    assert!(results.iter().any(|r| r.contains("Lemma models-zfc-contains-empty: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma models-zfc-contains-omega: VALID [DERIVED]")));
    // Phase 10: ordinal definitions (empty-ordinal, succ-ordinal from transitive-def + ordinal-def)
    assert!(results.iter().any(|r| r.contains("Lemma empty-ordinal: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma succ-ordinal: VALID [DERIVED]")));
    // Phase 10: DEF-CLOSURE factoring (general principle + classification facts)
    assert!(results.iter().any(|r| r.contains("Lemma def-closed-sat-pair: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma def-closed-sat-union: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma def-closed-sat-sep: VALID [DERIVED]")));
    // Phase 11: omega ordinals (via induction) + function infrastructure (composition)
    assert!(results.iter().any(|r| r.contains("Lemma omega-ordinal: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma omega-is-limit: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma inj-to-power: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma id-is-bij: VALID [DERIVED]")));
    // Phase 12: subset definition + eq-trans
    assert!(results.iter().any(|r| r.contains("Lemma eq-trans: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma sub-refl: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma empty-sub: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma sub-mem: VALID [DERIVED]")));
    // Phase 12: function infrastructure from biconditional definitions
    assert!(results.iter().any(|r| r.contains("Lemma id-maps: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma id-is-inj: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma id-is-surj: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma singleton-maps: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma surj-preimage-in-dom: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma surj-preimage-hits: VALID [DERIVED]")));
    // Phase 12: Kuratowski pair injectivity from first principles
    assert!(results.iter().any(|r| r.contains("Lemma singleton-eq: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma singleton-is-inj: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma kpair-fst: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma kpair-snd: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma kpair-inj: VALID [DERIVED]")));
    // Cantor lemma chain (proved from axioms via Cut)
    assert!(results.iter().any(|r| r.contains("Lemma diag-in-power: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma diag-contradiction: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma cantor-no-surj: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma cantor: VALID [DERIVED]")));
    // L-sat lemmas (from model-theoretic infrastructure: absoluteness + definable closure)
    assert!(results.iter().any(|r| r.contains("Lemma L-sat-ext: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma L-sat-empty: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma L-sat-pair: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma L-sat-union: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma L-sat-inf: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma L-sat-sep: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma L-sat-reg: VALID [DERIVED]")));
    // Phase 14: primitive recovery lemmas (each from 2-step decomposition)
    assert!(results.iter().any(|r| r.contains("Lemma L-has-elem-submodels: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma L-has-collapse: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma L-has-stage-collapse: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma L-has-cofinal-stages: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma L-has-godel-numbering: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma L-has-stage-ordering: VALID [DERIVED]")));
    // Decomposition lemmas: L properties from fine-grained sub-results
    assert!(results.iter().any(|r| r.contains("Lemma L-has-condensation: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma L-has-reflection: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma L-has-definable-wellorder: VALID [DERIVED]")));
    // Phase 14: application recovery lemmas (each from 2-step decomposition)
    assert!(results.iter().any(|r| r.contains("Lemma condensation-gives-power: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma reflection-gives-rep: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma wellorder-gives-choice: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma condensation-gives-gch: VALID [DERIVED]")));
    // L-sat hard lemmas (from condensation/reflection/wellorder infrastructure)
    assert!(results.iter().any(|r| r.contains("Lemma L-sat-power: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma L-sat-rep: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma L-sat-choice: VALID [DERIVED]")));
    // Infrastructure lemmas (L ⊨ ZFC + GCH + Gödel + well-formedness)
    assert!(results.iter().any(|r| r.contains("Lemma L-models-zfc: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma L-satisfies-gch: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma godel-theorem: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma all-well-formed: VALID [DERIVED]")));
    // Truth Lemma + Forcing (decomposed into per-case/per-axiom + induction/chain)
    assert!(results.iter().any(|r| r.contains("Lemma truth-lemma-induction: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma truth-lemma: VALID [DERIVED]")));
    // Phase 15: forcing recovery lemmas (each from 2-step decomposition)
    assert!(results.iter().any(|r| r.contains("Lemma truth-holds-for-means: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma forcing-def-closed: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma ccc-gives-nice-names: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma nice-names-give-power: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma rep-transfers: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma choice-transfers: VALID [DERIVED]")));
    // Forcing-sat lemmas (from model-theoretic infrastructure)
    assert!(results.iter().any(|r| r.contains("Lemma forcing-sat-ext: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma forcing-sat-empty: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma forcing-sat-pair: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma forcing-sat-union: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma forcing-sat-inf: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma forcing-sat-sep: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma forcing-sat-reg: VALID [DERIVED]")));
    // Forcing-sat hard lemmas (from CCC/nice-names/transfer infrastructure)
    assert!(results.iter().any(|r| r.contains("Lemma forcing-sat-power: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma forcing-sat-rep: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma forcing-sat-choice: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma forcing-preserves-zfc: VALID [DERIVED]")));
    // Decomposition lemmas: Cohen/forcing from fine-grained sub-results
    assert!(results.iter().any(|r| r.contains("Lemma delta-system-ccc: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma rasiowa-sikorski: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma ccc-no-cardinal-collapse: VALID [DERIVED]")));
    // Cohen infrastructure lemmas
    assert!(results.iter().any(|r| r.contains("Lemma cohen-has-ccc: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma generic-existence: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma cohen-adds-reals: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma ccc-preserves-cardinals: VALID [DERIVED]")));
    // Cohen theorem (derived from forcing-preserves + generic + cohen-adds-reals)
    assert!(results.iter().any(|r| r.contains("Lemma cohen-theorem: VALID [DERIVED]")));
    // Tier 1: pure axiom derivations
    assert!(results.iter().any(|r| r.contains("Proof zero-in-omega: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof empty-in-power: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof self-in-power: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof kpair-injective: VALID")));
    // Tier 2: Cantor derived (uses lemmas as rules)
    assert!(results.iter().any(|r| r.contains("Proof cantor-theorem: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof continuum-uncountable: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof id-bij: VALID")));
    // Tier 3: architecture (all derived)
    assert!(results.iter().any(|r| r.contains("Proof godel-consistency: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof cohen-consistency: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof ch-independent: VALID")));
}

#[test]
fn lemma_demo_example() {
    let results = check_example("examples/lemma-demo.omega");
    assert!(results.iter().any(|r| r.contains("Theory PropCut: registered OK")));
    // Tier 1: direct proofs
    assert!(results.iter().any(|r| r.contains("Proof identity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof and-comm-direct: VALID")));
    // Tier 2: lemmas (Cut rule)
    assert!(results.iter().any(|r| r.contains("Lemma and-comm: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Lemma and-assoc: VALID [DERIVED]")));
    // Tier 3: using derived rules
    assert!(results.iter().any(|r| r.contains("Proof and-comm-via-lemma: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof and-assoc-via-lemma: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof chain-assoc-comm: VALID")));
    // Tier 4: lemma-on-lemma chaining
    assert!(results.iter().any(|r| r.contains("Lemma and-comm-assoc: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Proof deep-chain: VALID")));
    assert!(results.iter().any(|r| r.contains("Lemma top-theorem: VALID [DERIVED]")));
    assert!(results.iter().any(|r| r.contains("Proof use-top-theorem: VALID")));
    // Tier 5: modus ponens as lemma
    assert!(results.iter().any(|r| r.contains("Lemma mp: VALID [DERIVED]")));
}

#[test]
fn self_representation_example() {
    let results = check_example("examples/self.omega");
    assert!(results.iter().any(|r| r.contains("Theory OmegaSelf: registered OK")));
    // ACT I: Checker
    assert!(results.iter().any(|r| r.contains("Proof check-top-intro: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof check-assume: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof check-and-intro: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof check-and-elim-l: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof check-modus-ponens: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof check-and-comm: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof check-and-assoc: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof reject-and-elim-on-top: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof reject-propagation: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof reject-mp-non-imp: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof reject-mp-mismatch: VALID")));
    // ACT II: Solver
    assert!(results.iter().any(|r| r.contains("Proof auto-identity: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof auto-and-comm: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof auto-and-assoc: VALID")));
    // ACT III: Soundness link
    assert!(results.iter().any(|r| r.contains("Proof true-and-comm: VALID")));
    // ACT IV: Arithmetic
    assert!(results.iter().any(|r| r.contains("Theory OmegaArith: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof arith-refl-zero: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof arith-succ-cong: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof arith-one-plus-one: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof arith-two-plus-two: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof arith-two-times-three: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof arith-reject-one-neq-two: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof true-two-plus-two: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof true-three-times-three: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof true-nested-computation: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof true-two-times-three: VALID")));
}
