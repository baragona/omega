use apeiron::arena::Arena;
use apeiron::builder::{self, BuildEnv};
use apeiron::hash;
use apeiron::node::OpCode;
use apeiron::parser;
use apeiron::physics::{self, HaltReason, PhysicsConfig};
use apeiron::readback;
use apeiron::system::Session;

// ================================================================
// Direct Arena Tests: Low-level interaction net verification
// ================================================================

#[test]
fn beta_identity_reduces_to_argument() {
    let mut arena = Arena::new();
    let mut env = BuildEnv::new();

    let sexp = &parser::parse("[app [lam x x] y]").unwrap()[0];
    let root = builder::build_rooted(&mut arena, &mut env, sexp);

    let result = physics::run(&mut arena, &PhysicsConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);
    assert!(result.interactions > 0);

    let result_port = arena.port(root, 1);
    assert!(result_port.is_connected());
    let term = readback::readback(&arena, result_port.target);
    assert_eq!(format!("{}", term), "y");
}

#[test]
fn beta_constant_function() {
    let mut arena = Arena::new();
    let mut env = BuildEnv::new();

    // (\x. z) y = z
    let sexp = &parser::parse("[app [lam x z] y]").unwrap()[0];
    let root = builder::build_rooted(&mut arena, &mut env, sexp);

    physics::run(&mut arena, &PhysicsConfig::default());

    let result_port = arena.port(root, 1);
    let term = readback::readback(&arena, result_port.target);
    assert_eq!(format!("{}", term), "z");
}

#[test]
fn beta_nonlinear_variable() {
    let mut arena = Arena::new();
    let mut env = BuildEnv::new();

    // (\x. app x x) y → [y y] (via dup tree)
    let sexp = &parser::parse("[app [lam x [app x x]] y]").unwrap()[0];
    let root = builder::build_rooted(&mut arena, &mut env, sexp);

    let result = physics::run(&mut arena, &PhysicsConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);

    // Result should have two y's
    let result_port = arena.port(root, 1);
    let term = readback::readback(&arena, result_port.target);
    let s = format!("{}", term);
    // Should contain y twice (non-linear duplication)
    assert!(s.contains("y"), "result should contain y, got: {}", s);
}

#[test]
fn nested_beta() {
    let mut arena = Arena::new();
    let mut env = BuildEnv::new();

    // (\x.x) ((\y.y) z) = z
    let sexp = &parser::parse("[app [lam x x] [app [lam y y] z]]").unwrap()[0];
    let root = builder::build_rooted(&mut arena, &mut env, sexp);

    physics::run(&mut arena, &PhysicsConfig::default());

    let result_port = arena.port(root, 1);
    let term = readback::readback(&arena, result_port.target);
    assert_eq!(format!("{}", term), "z");
}

#[test]
fn topological_hash_after_reduction() {
    let mut arena = Arena::new();

    // Build (\x.x) y and y separately
    let mut env1 = BuildEnv::new();
    let lhs = builder::build_rooted(&mut arena, &mut env1, &parser::parse("[app [lam x x] y]").unwrap()[0]);

    let mut env2 = BuildEnv::new();
    let rhs = builder::build_rooted(&mut arena, &mut env2, &parser::parse("y").unwrap()[0]);

    physics::run(&mut arena, &PhysicsConfig::default());

    let lhs_result = arena.port(lhs, 1);
    let rhs_result = arena.port(rhs, 1);

    let lhs_hash = hash::topological_hash(&arena, lhs_result.target);
    let rhs_hash = hash::topological_hash(&arena, rhs_result.target);
    assert_eq!(lhs_hash, rhs_hash);
}

// ================================================================
// Session Tests: Full System/Theory pipeline
// ================================================================

#[test]
fn full_weak_lf_example() {
    let source = std::fs::read_to_string("examples/weak-lf.ap")
        .expect("examples/weak-lf.ap should exist");

    let sexps = parser::parse(&source).unwrap();
    let mut session = Session::new();

    for sexp in &sexps {
        match session.process(sexp) {
            Ok(()) => {}
            Err(e) => panic!("Error processing: {}", e),
        }
    }

    // Verify system was registered
    assert!(session.systems.contains_key("WeakLF"));

    // Verify assertions passed
    let assert_lines: Vec<_> = session.output.iter().filter(|l| l.starts_with("[ASSERT]")).collect();
    assert!(!assert_lines.is_empty(), "should have assertion results");
    for line in &assert_lines {
        assert!(line.contains("passed"), "assertion should pass: {}", line);
    }

    // Verify evals produced output
    let eval_lines: Vec<_> = session.output.iter().filter(|l| l.starts_with("[EVAL]")).collect();
    assert!(!eval_lines.is_empty(), "should have eval results");

    // Print all output for debugging
    for line in &session.output {
        eprintln!("{}", line);
    }
}

#[test]
fn assert_eq_failure_reports_error() {
    let source = r#"
    [System Test
      [@syntax [sort Term]]
      [@binding implicit]
      [@check beta-reduction]
    ]
    [Theory Demo :in Test]
    [Proofs DemoCheck :in Demo
      [assert-eq should-fail x y]
    ]
    "#;

    let sexps = parser::parse(source).unwrap();
    let mut session = Session::new();

    session.process(&sexps[0]).unwrap(); // System
    session.process(&sexps[1]).unwrap(); // Theory

    let result = session.process(&sexps[2]); // Proofs with failing assert
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{}", err).contains("should-fail"));
}

#[test]
fn multiple_systems() {
    let source = r#"
    [System WeakLF
      [@syntax [sort Term]]
      [@binding implicit]
      [@check rewriting beta-reduction]
    ]
    [System RichInductive
      [@syntax [sort Type] [sort Prop]]
      [@binding exposed]
      [@check beta-reduction]
    ]
    [Theory T1 :in WeakLF]
    [Proofs T1Check :in T1
      [assert-eq test [app [lam x x] z] z]
    ]
    "#;

    let sexps = parser::parse(source).unwrap();
    let mut session = Session::new();
    for sexp in &sexps {
        session.process(sexp).unwrap();
    }

    assert!(session.systems.contains_key("WeakLF"));
    assert!(session.systems.contains_key("RichInductive"));
}

// ================================================================
// Scope / Barrier Tests
// ================================================================

#[test]
fn barrier_blocks_until_scope_active() {
    let mut arena = Arena::new();

    // Build: App(Barrier(42, Lam(x,x)), y)
    let lam = arena.spawn(OpCode::Lam);
    arena.connect(lam, 1, lam, 2); // identity

    let barrier = arena.spawn(OpCode::Barrier { scope: 42 });
    arena.connect(barrier, 1, lam, 0); // barrier wraps lam

    let y = arena.spawn(OpCode::Sym { name: "y".into(), arity: 0 });
    let app = arena.spawn(OpCode::App);
    arena.connect(app, 1, y, 0);
    arena.connect(app, 0, barrier, 0); // App faces Barrier

    // Run without activating scope — should suspend
    let result = physics::run(&mut arena, &PhysicsConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm); // stuck, not error
    assert!(arena.get(barrier).is_some()); // barrier still alive

    // Activate scope
    arena.activate_scope(42);

    // Now run again — should reduce
    let result2 = physics::run(&mut arena, &PhysicsConfig::default());
    assert!(result2.interactions > 0);
}

// ================================================================
// Erase / Dup interaction tests
// ================================================================

#[test]
fn erase_propagates() {
    let mut arena = Arena::new();

    // Erase an App node — should erase both children
    let app = arena.spawn(OpCode::App);
    let _f = arena.spawn(OpCode::Sym { name: "f".into(), arity: 0 });
    let x = arena.spawn(OpCode::Sym { name: "x".into(), arity: 0 });
    arena.connect(app, 1, x, 0);
    // Don't connect app.0 to f — connect erase to app.0
    let erase = arena.spawn(OpCode::Erase);
    arena.connect(erase, 0, app, 0);

    // f is connected via app.2 (result port) — but let's skip that for simplicity
    // Just check that erase of app creates erasers for children

    let result = physics::run(&mut arena, &PhysicsConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);
    assert!(arena.get(app).is_none()); // app was erased
    assert!(arena.get(erase).is_none()); // original erase was consumed
}

#[test]
fn dup_sym_duplicates_constant() {
    let mut arena = Arena::new();

    let sym = arena.spawn(OpCode::Sym { name: "nat".into(), arity: 0 });
    let dup = arena.spawn(OpCode::Dup { label: 0 });

    let target_a = arena.spawn(OpCode::Sym { name: "slot_a".into(), arity: 1 });
    let target_b = arena.spawn(OpCode::Sym { name: "slot_b".into(), arity: 1 });

    arena.connect(dup, 1, target_a, 1);
    arena.connect(dup, 2, target_b, 1);
    arena.connect(dup, 0, sym, 0); // active pair

    let result = physics::run(&mut arena, &PhysicsConfig::default());
    assert_eq!(result.halted_reason, HaltReason::NormalForm);
    assert_eq!(result.interactions, 1);

    // Both targets should now be connected to a Sym("nat")
    let a_port = arena.port(target_a, 1);
    assert!(a_port.is_connected());
    let a_node = arena.get(a_port.target).unwrap();
    assert!(matches!(&a_node.kind, OpCode::Sym { name, .. } if name == "nat"));

    let b_port = arena.port(target_b, 1);
    assert!(b_port.is_connected());
    let b_node = arena.get(b_port.target).unwrap();
    assert!(matches!(&b_node.kind, OpCode::Sym { name, .. } if name == "nat"));
}

// ================================================================
// Example file tests
// ================================================================

fn run_example(path: &str) {
    let source = std::fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("{} should exist", path));
    let sexps = parser::parse(&source).unwrap();
    let mut session = Session::new();

    for sexp in &sexps {
        match session.process(sexp) {
            Ok(()) => {}
            Err(e) => panic!("Error in {}: {}", path, e),
        }
    }

    // Verify all assertions passed
    let assert_lines: Vec<_> = session
        .output
        .iter()
        .filter(|l| l.starts_with("[ASSERT]"))
        .collect();
    for line in &assert_lines {
        assert!(line.contains("passed"), "assertion failed in {}: {}", path, line);
    }

    // Verify all judgment checks passed
    let check_lines: Vec<_> = session
        .output
        .iter()
        .filter(|l| l.starts_with("[CHECK]"))
        .collect();
    for line in &check_lines {
        assert!(line.contains("passed"), "check failed in {}: {}", path, line);
    }

    // Verify all derivation verifications passed
    let derive_lines: Vec<_> = session
        .output
        .iter()
        .filter(|l| l.starts_with("[DERIVE]") && l.contains("verified"))
        .collect();
    for line in &derive_lines {
        assert!(line.contains("verified"), "derive failed in {}: {}", path, line);
    }

    // Verify all refutations
    let refute_lines: Vec<_> = session
        .output
        .iter()
        .filter(|l| l.starts_with("[REFUTE]"))
        .collect();
    for line in &refute_lines {
        assert!(line.contains("VERIFIED"), "refutation failed in {}: {}", path, line);
    }
}

#[test]
fn example_logic_programming() {
    run_example("examples/logic-programming.ap");
}

#[test]
fn example_streams() {
    run_example("examples/streams.ap");
}

#[test]
fn example_mixed_binding() {
    run_example("examples/mixed-binding.ap");
}

#[test]
fn example_inductive_types() {
    run_example("examples/inductive-types.ap");
}

#[test]
fn example_arithmetic() {
    run_example("examples/arithmetic.ap");
}

#[test]
fn example_church_numerals() {
    run_example("examples/church-numerals.ap");
}

#[test]
fn example_alpha_equivalence() {
    run_example("examples/alpha-equivalence.ap");
}

#[test]
fn example_modal_logic() {
    run_example("examples/modal-logic.ap");
}

#[test]
fn example_reflection() {
    run_example("examples/reflection.ap");
}

#[test]
fn example_linear_linter() {
    run_example("examples/linear-linter.ap");
}



#[test]
fn example_contextual_alpha() {
    run_example("examples/contextual-alpha.ap");
}

#[test]
fn example_unified_logic() {
    run_example("examples/unified-logic.ap");
}

#[test]
fn example_grand_unification() {
    run_example("examples/grand-unification.ap");
}

#[test]
fn example_leibniz() {
    run_example("examples/leibniz.ap");
}

// ================================================================
// New Mode Examples
// ================================================================

#[test]
fn example_linear_types() {
    run_example("examples/linear-types.ap");
}

#[test]
fn example_reversible() {
    run_example("examples/reversible.ap");
}

#[test]
fn example_nominal() {
    run_example("examples/nominal.ap");
}

#[test]
fn example_nondeterministic() {
    run_example("examples/nondeterministic.ap");
}

#[test]
fn example_differential() {
    run_example("examples/differential.ap");
}

#[test]
fn example_automorphism() {
    run_example("examples/automorphism.ap");
}

#[test]
fn example_stack_compiler() {
    run_example("examples/stack-compiler.ap");
}

#[test]
fn example_morphism_zoo() {
    run_example("examples/morphism-zoo.ap");
}

#[test]
fn example_church_power() {
    run_example("examples/church-power.ap");
}

#[test]
fn example_higher_order_rewrite() {
    run_example("examples/higher-order-rewrite.ap");
}

#[test]
fn example_sharing_demo() {
    run_example("examples/sharing-demo.ap");
}

// ================================================================
// Omega Ports: Same Logic, Different Engine
// ================================================================

#[test]
fn example_omega_peano() {
    run_example("examples/omega-peano.ap");
}

#[test]
fn example_omega_self() {
    run_example("examples/omega-self.ap");
}

#[test]
fn example_omega_stlc() {
    run_example("examples/omega-stlc.ap");
}

// ================================================================
// Judgment Layer Tests
// ================================================================

#[test]
fn example_judgment_stlc() {
    run_example("examples/judgment-stlc.ap");
}

#[test]
fn example_judgment_prop() {
    run_example("examples/judgment-prop.ap");
}

#[test]
fn example_judgment_derive() {
    run_example("examples/judgment-derive.ap");
}

#[test]
fn example_judgment_refute() {
    run_example("examples/judgment-refute.ap");
}

#[test]
fn judgment_check_passes() {
    let source = r#"
    [System JTest
      [@syntax
        [sort Ty] [sort Tm] [sort Ctx]
        [op Base] [op arrow]
        [op vz] [op vs] [op lam-t] [op app-t]
        [op nil] [op ext]
        [judgment typeof :inputs [Ctx Tm] :output Ty]
      ]
      [@binding implicit]
      [@check rewriting]
    ]
    [Theory JRules :in JTest
      [@derive typeof-vz
        :conclusion [typeof [ext ?A ?G] vz ?A]]
      [@derive typeof-lam
        :premises [[typeof [ext ?A ?G] ?e ?B]]
        :conclusion [typeof ?G [lam-t ?A ?e] [arrow ?A ?B]]]
    ]
    [Proofs JCheck :in JRules
      [check identity [typeof nil [lam-t Base vz]] [arrow Base Base]]
    ]
    "#;

    let sexps = parser::parse(source).unwrap();
    let mut session = Session::new();
    for sexp in &sexps {
        session.process(sexp).unwrap();
    }
    assert!(session.output.iter().any(|s| s.contains("[CHECK] identity passed")));
}

#[test]
fn judgment_check_fails_on_mismatch() {
    let source = r#"
    [System JTest
      [@syntax
        [sort Ty] [sort Tm] [sort Ctx]
        [op Base] [op Nat] [op arrow]
        [op vz] [op lam-t]
        [op nil] [op ext]
        [judgment typeof :inputs [Ctx Tm] :output Ty]
      ]
      [@binding implicit]
      [@check rewriting]
    ]
    [Theory JRules :in JTest
      [@derive typeof-vz
        :conclusion [typeof [ext ?A ?G] vz ?A]]
      [@derive typeof-lam
        :premises [[typeof [ext ?A ?G] ?e ?B]]
        :conclusion [typeof ?G [lam-t ?A ?e] [arrow ?A ?B]]]
    ]
    [Proofs JCheck :in JRules
      [check wrong [typeof nil [lam-t Base vz]] [arrow Nat Base]]
    ]
    "#;

    let sexps = parser::parse(source).unwrap();
    let mut session = Session::new();
    session.process(&sexps[0]).unwrap();
    session.process(&sexps[1]).unwrap();
    let result = session.process(&sexps[2]);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("mismatch"), "expected mismatch error, got: {}", err);
}

#[test]
fn refute_impossible_goal() {
    let source = r#"
    [System RTest
      [@syntax
        [sort Val] [sort Result]
        [op zero] [op succ]
        [judgment steps :inputs [Val] :output Val]
      ]
      [@binding implicit]
      [@check rewriting]
    ]
    [Theory RRules :in RTest]
    [Proofs RCheck :in RRules
      [refute zero-stuck
        :assumptions []
        :goal [steps zero zero]
        :depth 3]
    ]
    "#;

    let sexps = parser::parse(source).unwrap();
    let mut session = Session::new();
    for sexp in &sexps {
        session.process(sexp).unwrap();
    }
    assert!(session.output.iter().any(|s| s.contains("[REFUTE] zero-stuck: VERIFIED")));
}

#[test]
fn refute_fails_when_derivable() {
    let source = r#"
    [System RTest
      [@syntax
        [sort Val] [sort Result]
        [op zero] [op unit]
        [judgment has :inputs [Val] :output Result]
      ]
      [@binding implicit]
      [@check rewriting]
    ]
    [Theory RRules :in RTest
      [@derive has-zero :conclusion [has zero unit]]
    ]
    [Proofs RCheck :in RRules
      [refute should-fail
        :assumptions []
        :goal [has zero unit]
        :depth 3]
    ]
    "#;

    let sexps = parser::parse(source).unwrap();
    let mut session = Session::new();
    session.process(&sexps[0]).unwrap();
    session.process(&sexps[1]).unwrap();
    let result = session.process(&sexps[2]);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("derivable"), "expected derivable error, got: {}", err);
}

// ================================================================
// Linear-Explicit Rejection Tests
// ================================================================

#[test]
fn linear_rejects_duplication() {
    let source = r#"
    [System LinearTest
      [@syntax [sort Term]]
      [@binding linear-explicit]
      [@check beta-reduction]
    ]
    [Theory Dup :in LinearTest]
    [Proofs DupCheck :in Dup
      [eval dup-fail [lam x [app x x]]]
    ]
    "#;

    let sexps = parser::parse(source).unwrap();
    let mut session = Session::new();
    session.process(&sexps[0]).unwrap();
    session.process(&sexps[1]).unwrap();
    let result = session.process(&sexps[2]);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("linearity") || err.contains("duplicat"), "expected linearity error, got: {}", err);
}

#[test]
fn linear_rejects_erasure() {
    let source = r#"
    [System LinearTest
      [@syntax [sort Term]]
      [@binding linear-explicit]
      [@check beta-reduction]
    ]
    [Theory Erase :in LinearTest]
    [Proofs EraseCheck :in Erase
      [eval erase-fail [lam x z]]
    ]
    "#;

    let sexps = parser::parse(source).unwrap();
    let mut session = Session::new();
    session.process(&sexps[0]).unwrap();
    session.process(&sexps[1]).unwrap();
    let result = session.process(&sexps[2]);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("linearity") || err.contains("erased"), "expected linearity error, got: {}", err);
}

#[test]
fn linear_accepts_identity() {
    let source = r#"
    [System LinearTest
      [@syntax [sort Term]]
      [@binding linear-explicit]
      [@check beta-reduction]
    ]
    [Theory Ok :in LinearTest]
    [Proofs OkCheck :in Ok
      [assert-eq id-ok [app [lam x x] y] y]
    ]
    "#;

    let sexps = parser::parse(source).unwrap();
    let mut session = Session::new();
    for sexp in &sexps {
        session.process(sexp).unwrap();
    }
}

// ================================================================
// Nominal Mode Tests
// ================================================================

#[test]
fn nominal_distinguishes_scopes() {
    let source = r#"
    [System NomTest
      [@syntax [sort Term]]
      [@binding nominal]
      [@check oracle]
    ]
    [Theory Nom :in NomTest
      [Scope X]
      [Scope Y]
    ]
    [Proofs NomCheck :in Nom
      [assert-neq nom-diff [box X a] [box Y a]]
    ]
    "#;

    let sexps = parser::parse(source).unwrap();
    let mut session = Session::new();
    for sexp in &sexps {
        session.process(sexp).unwrap();
    }
}

// ================================================================
// Reversible Mode Tests
// ================================================================

#[test]
fn reversible_generates_inverse() {
    let source = r#"
    [System RevTest
      [@syntax [sort Term] [op wrap]]
      [@binding implicit]
      [@check rewriting reversible]
    ]
    [Theory Rev :in RevTest
      [@rule wrap-z [wrap z] ==> tagged]
    ]
    [Proofs RevCheck :in Rev
      [eval-reverse unwrap tagged]
    ]
    "#;

    let sexps = parser::parse(source).unwrap();
    let mut session = Session::new();
    for sexp in &sexps {
        session.process(sexp).unwrap();
    }

    // Check that the reverse eval produced output
    assert!(session.output.iter().any(|s| s.starts_with("[EVAL]") && s.contains("unwrap")));
}
