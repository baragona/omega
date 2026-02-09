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
