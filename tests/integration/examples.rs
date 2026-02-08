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
    assert!(results.iter().any(|r| r.contains("Proof and-comm: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof self-and: VALID")));
}

#[test]
fn stlc_example() {
    let results = check_example("examples/stlc.omega");
    assert!(results.iter().any(|r| r.contains("Theory STLC: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof identity-typing: VALID")));
}

#[test]
fn first_order_example() {
    let results = check_example("examples/first-order.omega");
    assert!(results.iter().any(|r| r.contains("Theory FOL: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof true-and-refl: VALID")));
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
    assert!(results.iter().any(|r| r.contains("Proof box-and: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof necessary-possible: VALID")));
}

#[test]
fn linear_logic_example() {
    let results = check_example("examples/linear-logic.omega");
    assert!(results.iter().any(|r| r.contains("Theory LinearLogic: registered OK")));
    assert!(results.iter().any(|r| r.contains("Proof tensor-pair: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof bang-dup: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof bang-contract: VALID")));
    assert!(results.iter().any(|r| r.contains("Proof unit-pf: VALID")));
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
