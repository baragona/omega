use hyperion::session::HyperionSession;

fn process_all(session: &mut HyperionSession, input: &str) -> Result<(), hyperion::error::HyperionError> {
    let sexps = apeiron::parser::parse(input)
        .map_err(|e| hyperion::error::HyperionError::ApeironError(e))?;
    for sexp in &sexps {
        session.process(sexp)?;
    }
    Ok(())
}

fn run_file(path: &str) {
    let source = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e));
    let sexps = apeiron::parser::parse(&source)
        .unwrap_or_else(|e| panic!("Parse error in {}: {}", path, e));
    let mut session = HyperionSession::new();
    for sexp in &sexps {
        session.process(sexp)
            .unwrap_or_else(|e| panic!("Error in {}: {}", path, e));
    }
}

// ============================================================
// Example file tests
// ============================================================

#[test]
fn example_weak_lf() {
    run_file("examples/weak-lf.hyp");
}

#[test]
fn example_modal_logic() {
    run_file("examples/modal-logic.hyp");
}

#[test]
fn example_cross_substrate() {
    run_file("examples/cross-substrate.hyp");
}

// ============================================================
// Category parsing tests
// ============================================================

#[test]
fn category_with_all_structures() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category Full
            [Object Type]
            [Object Term]
            [Morphism arrow :domain [Type Type] :codomain Type]
            [Exponential lam :object Term]
            [Evaluator app]
            [ModalOperator box]
            [Context W1]
            [TensorProduct tensor]
            [Unit unit]
        ]
    "#;
    process_all(&mut session, input).unwrap();
    let cat = &session.categories["Full"];
    assert_eq!(cat.objects.len(), 2);
    assert_eq!(cat.morphisms.len(), 1);
    assert_eq!(cat.structure.len(), 6);
    assert!(cat.has_exponential());
    assert!(cat.has_evaluator());
    assert!(cat.has_modal_operator());
    assert!(cat.has_context());
    assert!(cat.has_tensor());
}

// ============================================================
// Compatibility rejection tests
// ============================================================

#[test]
fn reject_ccc_on_cellular_automaton() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category CCC
            [Object Term]
            [Exponential lam :object Term]
        ]
        [Substrate Grid
            @engine cellular-automaton
            @resource-mode deep-copy
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe Bad :category CCC :substrate Grid]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("Exponential support"), "Got: {}", msg);
}

#[test]
fn reject_modal_on_transparent_barrier() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category Modal
            [Object Prop]
            [ModalOperator box]
        ]
        [Substrate Plain
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-hash
        ]
        [Universe Bad :category Modal :substrate Plain]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("scope isolation"), "Got: {}", msg);
}

#[test]
fn reject_tensor_on_term_tree() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category Monoidal
            [Object Obj]
            [TensorProduct tensor]
        ]
        [Substrate Tree
            @engine term-tree
            @resource-mode deep-copy
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe Bad :category Monoidal :substrate Tree]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("TensorProduct"), "Got: {}", msg);
}

#[test]
fn reject_strictly_linear_exponential() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category CCC
            [Object Term]
            [Exponential lam :object Term]
        ]
        [Substrate Linear
            @engine interaction-graph
            @resource-mode strictly-linear
            @barrier transparent
            @equality topological-hash
        ]
        [Universe Bad :category CCC :substrate Linear]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("strictly-linear"), "Got: {}", msg);
}

// ============================================================
// Substrate parsing error tests
// ============================================================

#[test]
fn substrate_missing_engine() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Substrate Bad
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-hash
        ]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("@engine"), "Got: {}", msg);
}

#[test]
fn substrate_unknown_engine() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Substrate Bad
            @engine quantum-flux
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-hash
        ]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("quantum-flux"), "Got: {}", msg);
}

// ============================================================
// Universe error tests
// ============================================================

#[test]
fn universe_undefined_category() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Substrate S
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-hash
        ]
        [Universe Bad :category Nonexistent :substrate S]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("Nonexistent"), "Got: {}", msg);
}

#[test]
fn universe_undefined_substrate() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C [Object X]]
        [Universe Bad :category C :substrate Nonexistent]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("Nonexistent"), "Got: {}", msg);
}

// ============================================================
// Functor transport tests
// ============================================================

/// Helper: shared preamble for two-substrate setup (compute + oracle)
fn two_substrate_preamble() -> &'static str {
    r#"
        [Category SimpleMath
            [Object Nat]
            [Morphism z :domain [] :codomain Nat]
            [Morphism s :domain [Nat] :codomain Nat]
            [Morphism plus :domain [Nat Nat] :codomain Nat]
        ]

        [Substrate ComputeNet
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]

        [Substrate OracleTree
            @engine term-tree
            @resource-mode deep-copy
            @barrier transparent
            @equality topological-hash
        ]

        [Universe PeanoCompute :category SimpleMath :substrate ComputeNet]
        [Universe PeanoOracle :category SimpleMath :substrate OracleTree]
    "#
}

#[test]
fn functor_end_to_end_transport() {
    let mut session = HyperionSession::new();
    let mut input = two_substrate_preamble().to_string();
    input.push_str(r#"
        [Functor NetToTree :from ComputeNet :to OracleTree]

        [Theory PeanoArith :in PeanoCompute
            [@rule [plus z ?n] ==> ?n]
            [@rule [plus [s ?n] ?m] ==> [s [plus ?n ?m]]]
            [def two-plus-one [plus [s [s z]] [s z]]]
        ]

        [Theory OracleArith :in PeanoOracle
            [Import transported [NetToTree two-plus-one]]
        ]

        [Proofs OracleCheck :in OracleArith
            [assert-eq transport-ok transported [s [s [s z]]]]
        ]
    "#);
    process_all(&mut session, &input).unwrap();

    // Verify the morphism was generated
    assert!(session.resolved_morphisms.contains_key("NetToTree"));
    let morph_map = &session.resolved_morphisms["NetToTree"];
    assert_eq!(morph_map["SimpleMath"], "__fun_NetToTree_SimpleMath");
}

#[test]
fn functor_with_op_maps() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category Arith
            [Object Nat]
            [Morphism zero :domain [] :codomain Nat]
            [Morphism succ :domain [Nat] :codomain Nat]
            [Morphism add :domain [Nat Nat] :codomain Nat]
        ]

        [Substrate NetA
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]

        [Substrate TreeB
            @engine term-tree
            @resource-mode deep-copy
            @barrier transparent
            @equality rewrite-equivalence
        ]

        [Universe AWorld :category Arith :substrate NetA]
        [Universe BWorld :category Arith :substrate TreeB]

        [Functor AB
            :from NetA
            :to TreeB
        ]

        [Theory SourceArith :in AWorld
            [@rule [add zero ?n] ==> ?n]
            [@rule [add [succ ?n] ?m] ==> [succ [add ?n ?m]]]
            [def my-term [add [succ zero] [succ zero]]]
        ]

        [Theory TargetArith :in BWorld
            [Import result [AB my-term]]
        ]

        [Proofs TargetCheck :in TargetArith
            [assert-eq add-works result [succ [succ zero]]]
        ]
    "#;
    process_all(&mut session, input).unwrap();
}

#[test]
fn functor_same_binding_mode() {
    // Both substrates use same binding mode (implicit) — Identity binding pass
    let mut session = HyperionSession::new();
    let input = r#"
        [Category Basic
            [Object Nat]
            [Morphism z :domain [] :codomain Nat]
            [Morphism s :domain [Nat] :codomain Nat]
        ]

        [Substrate NetA
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]

        [Substrate NetB
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-hash
        ]

        [Universe WorldA :category Basic :substrate NetA]
        [Universe WorldB :category Basic :substrate NetB]

        [Functor AtoB :from NetA :to NetB]

        [Theory Source :in WorldA
            [def val [s [s z]]]
        ]

        [Theory Target :in WorldB
            [Import imported [AtoB val]]
        ]

        [Proofs Check :in Target
            [assert-eq identity-ok imported [s [s z]]]
        ]
    "#;
    process_all(&mut session, input).unwrap();
}

#[test]
fn functor_multiple_categories() {
    // Two categories sharing the same substrates → functor generates 2 morphisms
    let mut session = HyperionSession::new();
    let input = r#"
        [Category CatA
            [Object Nat]
            [Morphism z :domain [] :codomain Nat]
        ]

        [Category CatB
            [Object Bool]
            [Morphism tt :domain [] :codomain Bool]
        ]

        [Substrate SubX
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]

        [Substrate SubY
            @engine term-tree
            @resource-mode deep-copy
            @barrier transparent
            @equality topological-hash
        ]

        [Universe NatX :category CatA :substrate SubX]
        [Universe NatY :category CatA :substrate SubY]
        [Universe BoolX :category CatB :substrate SubX]
        [Universe BoolY :category CatB :substrate SubY]

        [Functor XtoY :from SubX :to SubY]
    "#;
    process_all(&mut session, &input).unwrap();

    let morph_map = &session.resolved_morphisms["XtoY"];
    assert_eq!(morph_map.len(), 2);
    assert!(morph_map.contains_key("CatA"));
    assert!(morph_map.contains_key("CatB"));
    assert_eq!(morph_map["CatA"], "__fun_XtoY_CatA");
    assert_eq!(morph_map["CatB"], "__fun_XtoY_CatB");
}

#[test]
fn functor_no_matching_universes() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category CatA
            [Object Nat]
            [Morphism z :domain [] :codomain Nat]
        ]

        [Substrate SubX
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]

        [Substrate SubY
            @engine term-tree
            @resource-mode deep-copy
            @barrier transparent
            @equality rewrite-equivalence
        ]

        [Substrate SubZ
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-hash
        ]

        [Universe WorldX :category CatA :substrate SubX]

        [Functor BadFunctor :from SubY :to SubZ]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("no matching universe pairs"), "Got: {}", msg);
}

#[test]
fn functor_normalize_first_checking_pass() {
    // Compute substrate → Oracle substrate triggers NormalizeFirst
    // Rewrite rules normalize the term before transport
    let mut session = HyperionSession::new();
    let mut input = two_substrate_preamble().to_string();
    input.push_str(r#"
        [Functor F :from ComputeNet :to OracleTree]

        [Theory Source :in PeanoCompute
            [@rule [plus z ?n] ==> ?n]
            [@rule [plus [s ?n] ?m] ==> [s [plus ?n ?m]]]
            [def big-sum [plus [s [s [s z]]] [s [s z]]]]
        ]

        [Theory Target :in PeanoOracle
            [Import result [F big-sum]]
        ]

        [Proofs TargetCheck :in Target
            [assert-eq big-sum-ok result [s [s [s [s [s z]]]]]]
        ]
    "#);
    process_all(&mut session, &input).unwrap();
}

// ============================================================
// Functor error tests
// ============================================================

#[test]
fn functor_undefined_source() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Substrate B
            @engine term-tree
            @resource-mode deep-copy
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Functor F :from Nonexistent :to B]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("Nonexistent"), "Got: {}", msg);
}

// ============================================================
// Duplicate name tests
// ============================================================

#[test]
fn duplicate_category() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C [Object X]]
        [Category C [Object Y]]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("duplicate"), "Got: {}", msg);
}

#[test]
fn duplicate_substrate() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Substrate S
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-hash
        ]
        [Substrate S
            @engine term-tree
            @resource-mode deep-copy
            @barrier transparent
            @equality rewrite-equivalence
        ]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("duplicate"), "Got: {}", msg);
}

// ============================================================
// End-to-end theory pass-through with rewrite rules
// ============================================================

#[test]
fn rewrite_rules_pass_through() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category Arith
            [Object Nat]
            [Morphism z :domain [] :codomain Nat]
            [Morphism s :domain [Nat] :codomain Nat]
            [Morphism plus :domain [Nat Nat] :codomain Nat]
        ]

        [Substrate Net
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]

        [Universe PeanoWorld :category Arith :substrate Net]

        [Theory Peano :in PeanoWorld
            [@rule [plus z ?n] ==> ?n]
            [@rule [plus [s ?n] ?m] ==> [s [plus ?n ?m]]]
        ]

        [Proofs PeanoCheck :in Peano
            [assert-eq add-base [plus z [s z]] [s z]]
            [assert-eq add-step [plus [s z] [s z]] [s [s z]]]
            [eval compute-3-2 [plus [s [s [s z]]] [s [s z]]]]
        ]
    "#;
    process_all(&mut session, input).unwrap();
}

// ============================================================
// Theory with beta reduction (CCC + interaction graph)
// ============================================================

#[test]
fn beta_reduction_pass_through() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category CCC
            [Object Type]
            [Object Term]
            [Morphism app :domain [Term Term] :codomain Term]
            [Exponential lam :object Term]
            [Evaluator app]
        ]

        [Substrate Net
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-hash
        ]

        [Universe LambdaWorld :category CCC :substrate Net]

        [Theory Lambda :in LambdaWorld
            [const a Term]
            [const b Term]
        ]

        [Proofs LambdaCheck :in Lambda
            [assert-eq identity [app [lam x x] a] a]
            [assert-eq constant [app [lam x a] b] a]
        ]
    "#;
    process_all(&mut session, input).unwrap();
}

// ============================================================
// Prelude tests
// ============================================================

#[test]
fn no_prelude_starts_empty() {
    let session = HyperionSession::new();
    assert_eq!(session.categories.len(), 0);
    assert_eq!(session.substrates.len(), 0);
}

/// Combined prelude test (env vars are process-global, can't parallelize safely)
#[test]
fn prelude_full_test() {
    // Use a single unique temp dir to avoid races with other tests
    let dir = std::env::temp_dir().join("hyperion_test_prelude_full");
    let _ = std::fs::create_dir_all(&dir);
    let prelude_path = dir.join("full_prelude.hyp");
    std::fs::write(
        &prelude_path,
        r#"
        [Category CartesianClosed
          [Object Type] [Object Term]
          [Morphism arrow :domain [Type Type] :codomain Type]
          [Morphism app :domain [Term Term] :codomain Term]
          [Exponential lam :object Term] [Evaluator app]
        ]
        [Category SymmetricMonoidal
          [Object Obj]
          [TensorProduct tensor] [Unit unit]
        ]
        [Category Preorder
          [Object Elem]
          [Morphism leq :domain [Elem Elem] :codomain Elem]
        ]
        [Substrate ApeironStandard @engine interaction-graph @resource-mode optimal-sharing @barrier transparent @equality rewrite-equivalence]
        [Substrate ApeironLinear @engine interaction-graph @resource-mode strictly-linear @barrier transparent @equality rewrite-equivalence]
        [Substrate ApeironOracle @engine interaction-graph @resource-mode optimal-sharing @barrier transparent @equality topological-hash]
        [Substrate ApeironTree @engine term-tree @resource-mode deep-copy @barrier transparent @equality rewrite-equivalence]
    "#,
    )
    .unwrap();

    std::env::set_var("HYPERION_PRELUDE", prelude_path.to_str().unwrap());
    let mut session = HyperionSession::with_prelude().unwrap();
    std::env::remove_var("HYPERION_PRELUDE");

    // Test 1: categories available
    assert!(session.categories.contains_key("CartesianClosed"));
    assert!(session.categories.contains_key("SymmetricMonoidal"));
    assert!(session.categories.contains_key("Preorder"));

    // Test 2: substrates available
    assert!(session.substrates.contains_key("ApeironStandard"));
    assert!(session.substrates.contains_key("ApeironLinear"));
    assert!(session.substrates.contains_key("ApeironOracle"));
    assert!(session.substrates.contains_key("ApeironTree"));

    // Test 3: user can add on top of prelude
    let input = "[Category UserCat [Object Y]]";
    process_all(&mut session, input).unwrap();
    assert!(session.categories.contains_key("UserCat"));
    assert!(session.categories.contains_key("CartesianClosed")); // still there

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn prelude_demo_example() {
    run_file("examples/prelude-demo.hyp");
}

// ============================================================
// NatTrans tests
// ============================================================

fn nat_trans_preamble() -> &'static str {
    r#"
        [Category SimpleMath
            [Object Nat]
            [Morphism z :domain [] :codomain Nat]
            [Morphism s :domain [Nat] :codomain Nat]
        ]

        [Substrate SubA
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]

        [Substrate SubB
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-hash
        ]

        [Universe UA :category SimpleMath :substrate SubA]
        [Universe UB :category SimpleMath :substrate SubB]

        [Functor F :from SubA :to SubB]
        [Functor G :from SubA :to SubB]
    "#
}

#[test]
fn parse_nat_trans() {
    let mut session = HyperionSession::new();
    let mut input = nat_trans_preamble().to_string();
    input.push_str(r#"
        [NatTrans eta :from F :to G :component [Nat tau_nat]]
    "#);
    process_all(&mut session, &input).unwrap();
    assert!(session.nat_trans.contains_key("eta"));
    let nt = &session.nat_trans["eta"];
    assert_eq!(nt.source_functor, "F");
    assert_eq!(nt.target_functor, "G");
    assert_eq!(nt.components.len(), 1);
}

#[test]
fn parse_adjunction() {
    let mut session = HyperionSession::new();
    let mut input = nat_trans_preamble().to_string();
    // Add backward functor and nat trans for unit/counit
    input.push_str(r#"
        [Functor H :from SubB :to SubA]

        [NatTrans eta :from F :to G :component [Nat tau_nat]]
        [NatTrans eps :from G :to F :component [Nat eps_nat]]
        [Adjunction Adj :left F :right H :unit eta :counit eps]
    "#);
    process_all(&mut session, &input).unwrap();
    assert!(session.adjunctions.contains_key("Adj"));
    let adj = &session.adjunctions["Adj"];
    assert_eq!(adj.left, "F");
    assert_eq!(adj.right, "H");
}

#[test]
fn nat_trans_validates_functors() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C [Object X]]
        [Substrate S @engine interaction-graph @resource-mode optimal-sharing @barrier transparent @equality rewrite-equivalence]
        [Substrate T @engine term-tree @resource-mode deep-copy @barrier transparent @equality rewrite-equivalence]
        [Universe U1 :category C :substrate S]
        [Universe U2 :category C :substrate T]
        [Functor F :from S :to T]
        [NatTrans eta :from F :to Nonexistent :component [X tau]]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("Nonexistent"), "Got: {}", msg);
}

#[test]
fn nat_trans_validates_parallel() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C [Object X]]
        [Substrate A @engine interaction-graph @resource-mode optimal-sharing @barrier transparent @equality rewrite-equivalence]
        [Substrate B @engine term-tree @resource-mode deep-copy @barrier transparent @equality rewrite-equivalence]
        [Substrate D @engine interaction-graph @resource-mode optimal-sharing @barrier transparent @equality topological-hash]
        [Universe U1 :category C :substrate A]
        [Universe U2 :category C :substrate B]
        [Universe U3 :category C :substrate D]
        [Functor F :from A :to B]
        [Functor G :from A :to D]
        [NatTrans eta :from F :to G :component [X tau]]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("not parallel"), "Got: {}", msg);
}

#[test]
fn adjunction_validates_nat_trans() {
    let mut session = HyperionSession::new();
    let mut input = nat_trans_preamble().to_string();
    input.push_str(r#"
        [Functor H :from SubB :to SubA]
        [NatTrans eta :from F :to G :component [Nat tau]]
        [Adjunction Adj :left F :right H :unit eta :counit nonexistent]
    "#);
    let err = process_all(&mut session, &input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("nonexistent"), "Got: {}", msg);
}

#[test]
fn nat_trans_structural_ok() {
    let mut session = HyperionSession::new();
    let mut input = nat_trans_preamble().to_string();
    input.push_str(r#"
        [NatTrans eta :from F :to G :component [Nat tau_nat]]
    "#);
    process_all(&mut session, &input).unwrap();
    let nt = &session.nat_trans["eta"];
    assert_eq!(nt.components[0].object, "Nat");
    assert_eq!(nt.components[0].morphism, "tau_nat");
}

#[test]
fn nat_trans_verify_generates_output() {
    let mut session = HyperionSession::new();
    let mut input = nat_trans_preamble().to_string();
    input.push_str(r#"
        [NatTrans eta :from F :to G :component [Nat tau_nat] :verify]
    "#);
    process_all(&mut session, &input).unwrap();
    assert!(session.output.iter().any(|s| s.contains("verification requested")));
}

#[test]
fn adjunction_demo_example() {
    run_file("examples/adjunction-demo.hyp");
}

// ============================================================
// Von Neumann backend tests
// ============================================================

fn vn_preamble() -> &'static str {
    r#"
        [Substrate VonNeumannMachine
            @engine von-neumann
            @resource-mode deep-copy
            @barrier transparent
            @equality rewrite-equivalence
        ]
    "#
}

#[test]
fn vn_exponential_rejected() {
    let mut session = HyperionSession::new();
    let mut input = String::from(r#"
        [Category CCC
            [Object Term]
            [Exponential lam :object Term]
        ]
    "#);
    input.push_str(vn_preamble());
    input.push_str("[Universe Bad :category CCC :substrate VonNeumannMachine]");
    let err = process_all(&mut session, &input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("Exponential"), "Got: {}", msg);
}

#[test]
fn vn_modal_rejected() {
    let mut session = HyperionSession::new();
    let mut input = String::from(r#"
        [Category Modal
            [Object Prop]
            [ModalOperator box]
        ]
    "#);
    input.push_str(vn_preamble());
    input.push_str("[Universe Bad :category Modal :substrate VonNeumannMachine]");
    let err = process_all(&mut session, &input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("ModalOperator"), "Got: {}", msg);
}

#[test]
fn vn_simple_accepted() {
    let mut session = HyperionSession::new();
    let mut input = String::from(r#"
        [Category Simple
            [Object Nat]
            [Morphism z :domain [] :codomain Nat]
            [Morphism s :domain [Nat] :codomain Nat]
        ]
    "#);
    input.push_str(vn_preamble());
    input.push_str("[Universe OK :category Simple :substrate VonNeumannMachine]");
    process_all(&mut session, &input).unwrap();
    assert!(session.universes.contains_key("OK"));
}

#[test]
fn vn_theory_captured() {
    let mut session = HyperionSession::new();
    let mut input = String::from(r#"
        [Category Arith
            [Object Nat]
            [Morphism z :domain [] :codomain Nat]
            [Morphism s :domain [Nat] :codomain Nat]
            [Morphism plus :domain [Nat Nat] :codomain Nat]
        ]
    "#);
    input.push_str(vn_preamble());
    input.push_str(r#"
        [Universe PeanoVN :category Arith :substrate VonNeumannMachine]
        [Theory PeanoRules :in PeanoVN
            [@rule plus-z [plus z ?n] ==> ?n]
            [@rule plus-s [plus [s ?n] ?m] ==> [s [plus ?n ?m]]]
        ]
    "#);
    process_all(&mut session, &input).unwrap();
    assert!(session.vn_theories.contains_key("PeanoRules"));
    // Theory is captured locally, not sent to Apeiron
    // (The universe/system IS registered in Apeiron, but the theory body is VN-only)
}

#[test]
fn vn_theory_rules_parsed() {
    let mut session = HyperionSession::new();
    let mut input = String::from(r#"
        [Category Arith
            [Object Nat]
            [Morphism z :domain [] :codomain Nat]
            [Morphism s :domain [Nat] :codomain Nat]
            [Morphism plus :domain [Nat Nat] :codomain Nat]
        ]
    "#);
    input.push_str(vn_preamble());
    input.push_str(r#"
        [Universe PeanoVN :category Arith :substrate VonNeumannMachine]
        [Theory PeanoRules :in PeanoVN
            [@rule plus-z [plus z ?n] ==> ?n]
            [@rule plus-s [plus [s ?n] ?m] ==> [s [plus ?n ?m]]]
        ]
    "#);
    process_all(&mut session, &input).unwrap();
    let theory = &session.vn_theories["PeanoRules"];
    assert_eq!(theory.rules.len(), 2);
    assert_eq!(theory.rules[0].name, "plus-z");
    assert_eq!(theory.rules[1].name, "plus-s");
}

#[test]
fn analyze_peano() {
    let mut session = HyperionSession::new();
    let mut input = String::from(r#"
        [Category Arith
            [Object Nat]
            [Morphism z :domain [] :codomain Nat]
            [Morphism s :domain [Nat] :codomain Nat]
            [Morphism plus :domain [Nat Nat] :codomain Nat]
        ]
    "#);
    input.push_str(vn_preamble());
    input.push_str(r#"
        [Universe PeanoVN :category Arith :substrate VonNeumannMachine]
        [Theory PeanoRules :in PeanoVN
            [@rule plus-z [plus z ?n] ==> ?n]
            [@rule plus-s [plus [s ?n] ?m] ==> [s [plus ?n ?m]]]
        ]
    "#);
    process_all(&mut session, &input).unwrap();

    let theory = &session.vn_theories["PeanoRules"];
    let krate = hyperion::codegen::analyze::analyze(theory).unwrap();

    assert_eq!(krate.name, "PeanoRules");
    // Should have types module with Nat enum (z, s are variants; plus is a function)
    let types_mod = krate.modules.iter().find(|m| m.name == "types").unwrap();
    let nat_enum = types_mod.items.iter().find_map(|item| {
        if let hyperion::codegen::rust_ast::RustItem::Enum(e) = item {
            if e.name == "Nat" { Some(e) } else { None }
        } else { None }
    }).unwrap();
    // z (nullary) and s (unary) should be variants
    assert!(nat_enum.variants.iter().any(|v| v.name == "Z"));
    assert!(nat_enum.variants.iter().any(|v| v.name == "S"));
    // plus should NOT be a variant (it's a rewrite head → function)
    assert!(!nat_enum.variants.iter().any(|v| v.name == "Plus"));

    // Should have functions module with plus function
    let func_mod = krate.modules.iter().find(|m| m.name == "functions").unwrap();
    let has_plus = func_mod.items.iter().any(|item| {
        if let hyperion::codegen::rust_ast::RustItem::Function(f) = item {
            f.name == "plus"
        } else { false }
    });
    assert!(has_plus);
}

#[test]
fn emit_peano() {
    let mut session = HyperionSession::new();
    let mut input = String::from(r#"
        [Category Arith
            [Object Nat]
            [Morphism z :domain [] :codomain Nat]
            [Morphism s :domain [Nat] :codomain Nat]
            [Morphism plus :domain [Nat Nat] :codomain Nat]
        ]
    "#);
    input.push_str(vn_preamble());
    input.push_str(r#"
        [Universe PeanoVN :category Arith :substrate VonNeumannMachine]
        [Theory PeanoRules :in PeanoVN
            [@rule plus-z [plus z ?n] ==> ?n]
            [@rule plus-s [plus [s ?n] ?m] ==> [s [plus ?n ?m]]]
        ]
    "#);
    process_all(&mut session, &input).unwrap();

    let theory = &session.vn_theories["PeanoRules"];
    let krate = hyperion::codegen::analyze::analyze(theory).unwrap();
    let files = hyperion::codegen::emit::emit_crate(&krate);

    assert!(files.contains_key("Cargo.toml"));
    assert!(files.contains_key("src/lib.rs"));
    assert!(files.contains_key("src/types.rs"));
    assert!(files.contains_key("src/functions.rs"));

    let cargo = &files["Cargo.toml"];
    assert!(cargo.contains("PeanoRules"));

    let types = &files["src/types.rs"];
    assert!(types.contains("pub enum Nat"));
    assert!(types.contains("Z,"));
    assert!(types.contains("S("));

    let funcs = &files["src/functions.rs"];
    assert!(funcs.contains("pub fn plus("));
    assert!(funcs.contains("Nat::S("));
}

#[test]
fn peano_vn_example() {
    run_file("examples/peano-vn.hyp");
}

// ============================================================
// Categorical law verification tests
// ============================================================

#[test]
fn law_check_monoidal_pass() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category Mon
            [Object Obj]
            [Morphism f :domain [Obj Obj] :codomain Obj]
            [TensorProduct tensor]
            [Unit unit]
        ]
        [Substrate Net
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe MonWorld :category Mon :substrate Net]
        [Theory MonTheory :in MonWorld
            [@rule [tensor [tensor ?a ?b] ?c] ==> [tensor ?a [tensor ?b ?c]]]
            [@rule [tensor unit ?a] ==> ?a]
            [@rule [tensor ?a unit] ==> ?a]
        ]
    "#;
    process_all(&mut session, input).unwrap();
    assert!(session.output.iter().any(|s| s.contains("[LAWS]") && s.contains("passed")));
}

#[test]
fn law_check_monoidal_fail() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category Mon
            [Object Obj]
            [TensorProduct tensor]
            [Unit unit]
        ]
        [Substrate Net
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe MonWorld :category Mon :substrate Net]
        [Theory BadMonoid :in MonWorld
            ;; No rewrite rules — associativity/unit laws will fail
        ]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("law violation"), "Got: {}", msg);
}

#[test]
fn law_check_ccc_pass() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category CCC
            [Object Type]
            [Object Term]
            [Morphism app :domain [Term Term] :codomain Term]
            [Exponential lam :object Term]
            [Evaluator app]
        ]
        [Substrate Net
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-hash
        ]
        [Universe LamWorld :category CCC :substrate Net]
        [Theory LamTheory :in LamWorld]
    "#;
    process_all(&mut session, input).unwrap();
    assert!(session.output.iter().any(|s| s.contains("[LAWS]") && s.contains("passed")));
}

#[test]
fn law_check_skip_laws_flag() {
    let mut session = HyperionSession::new();
    session.skip_laws = true;
    let input = r#"
        [Category Mon
            [Object Obj]
            [TensorProduct tensor]
            [Unit unit]
        ]
        [Substrate Net
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe MonWorld :category Mon :substrate Net]
        [Theory BadMonoid :in MonWorld
            ;; No rewrite rules — would fail without skip_laws
        ]
    "#;
    // Should succeed with skip_laws = true
    process_all(&mut session, input).unwrap();
    assert!(!session.output.iter().any(|s| s.contains("[LAWS]")));
}

#[test]
fn law_check_no_laws_for_simple_category() {
    // A category with no TensorProduct/Unit/Exponential has no laws to check
    let mut session = HyperionSession::new();
    let input = r#"
        [Category Simple
            [Object Nat]
            [Morphism z :domain [] :codomain Nat]
            [Morphism s :domain [Nat] :codomain Nat]
        ]
        [Substrate Net
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe SimpleWorld :category Simple :substrate Net]
        [Theory SimpleTheory :in SimpleWorld]
    "#;
    process_all(&mut session, input).unwrap();
    assert!(!session.output.iter().any(|s| s.contains("[LAWS]")));
}

#[test]
fn law_check_demo_example() {
    run_file("examples/law-check-demo.hyp");
}

// ============================================================
// HoTT equality mode tests
// ============================================================

#[test]
fn hott_equality_mode_parses() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Substrate HoTT
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-homotopy
        ]
    "#;
    process_all(&mut session, input).unwrap();
    let sub = &session.substrates["HoTT"];
    assert_eq!(sub.equality, hyperion::substrate::EqualityMode::TopologicalHomotopy);
}

#[test]
fn hott_requires_lambda_engine() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C [Object X]]
        [Substrate Bad
            @engine cellular-automaton
            @resource-mode deep-copy
            @barrier transparent
            @equality topological-homotopy
        ]
        [Universe BadWorld :category C :substrate Bad]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("topological-homotopy") || msg.contains("path spaces"), "Got: {}", msg);
}

#[test]
fn hott_on_lambda_engine_accepted() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C [Object X]]
        [Substrate HoTT
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-homotopy
        ]
        [Universe HoTTWorld :category C :substrate HoTT]
    "#;
    process_all(&mut session, input).unwrap();
    assert!(session.universes.contains_key("HoTTWorld"));
}

#[test]
fn hott_vn_rejected() {
    // Von Neumann can't do HoTT (not lambda-capable)
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C [Object X]]
        [Substrate Bad
            @engine von-neumann
            @resource-mode deep-copy
            @barrier transparent
            @equality topological-homotopy
        ]
        [Universe BadWorld :category C :substrate Bad]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("path spaces") || msg.contains("topological-homotopy"), "Got: {}", msg);
}

#[test]
fn hott_demo_example() {
    run_file("examples/hott-demo.hyp");
}

// ============================================================
// Fix 1: PathType tests
// ============================================================

#[test]
fn path_type_parses() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category PathCat
            [Object Type]
            [Object Term]
            [Morphism app :domain [Term Term] :codomain Term]
            [Exponential lam :object Term]
            [Evaluator app]
            [PathType :refl refl :concat concat :inv inv :ap ap]
        ]
    "#;
    process_all(&mut session, input).unwrap();
    let cat = &session.categories["PathCat"];
    assert!(cat.has_path_type());
    assert!(cat.has_exponential());
    assert!(cat.has_evaluator());
}

#[test]
fn path_type_requires_lambda() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category PathCat
            [Object X]
            [PathType :refl refl :concat concat :inv inv :ap ap]
        ]
        [Substrate Grid
            @engine cellular-automaton
            @resource-mode deep-copy
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe Bad :category PathCat :substrate Grid]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("PathType"), "Got: {}", msg);
}

#[test]
fn path_type_auto_injects_rules() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category PathCat
            [Object Type]
            [Object Term]
            [Morphism app :domain [Term Term] :codomain Term]
            [Exponential lam :object Term]
            [Evaluator app]
            [PathType :refl refl :concat concat :inv inv :ap ap]
        ]
        [Substrate Net
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe PathWorld :category PathCat :substrate Net]

        ;; Empty theory body — path rules auto-injected
        [Theory PathTheory :in PathWorld]

        [Proofs PathCheck :in PathTheory
            [assert-eq left-unit [concat [refl a] p] p]
            [assert-eq right-unit [concat p [refl a]] p]
            [assert-eq assoc [concat [concat p q] r] [concat p [concat q r]]]
            [assert-eq inv-refl [inv [refl a]] [refl a]]
            [assert-eq ap-refl [ap f [refl a]] [refl [app f a]]]
        ]
    "#;
    process_all(&mut session, input).unwrap();
}

#[test]
fn hott_demo_with_path_type() {
    // The updated hott-demo.hyp uses PathType with empty theory body
    run_file("examples/hott-demo.hyp");
}

// ============================================================
// Fix 2: Honest law verification tests
// ============================================================

#[test]
fn law_check_structural_witnesses() {
    // Verify that structural witnesses generate additional tests
    let mut session = HyperionSession::new();
    let input = r#"
        [Category Mon
            [Object Obj]
            [Morphism f :domain [Obj Obj] :codomain Obj]
            [TensorProduct tensor]
            [Unit unit]
        ]
        [Substrate Net
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe MonWorld :category Mon :substrate Net]
        [Theory MonTheory :in MonWorld
            [@rule [tensor [tensor ?a ?b] ?c] ==> [tensor ?a [tensor ?b ?c]]]
            [@rule [tensor unit ?a] ==> ?a]
            [@rule [tensor ?a unit] ==> ?a]
        ]
    "#;
    process_all(&mut session, input).unwrap();
    // Should report 6 witness tests (3 base + 3 structural)
    let law_msg = session.output.iter().find(|s| s.contains("[LAWS]")).unwrap();
    assert!(law_msg.contains("6 witness tests"), "Got: {}", law_msg);
}

#[test]
fn law_check_reports_count() {
    // CCC laws should report 3 witness tests (1 base + 2 structural)
    let mut session = HyperionSession::new();
    let input = r#"
        [Category CCC
            [Object Type]
            [Object Term]
            [Morphism app :domain [Term Term] :codomain Term]
            [Exponential lam :object Term]
            [Evaluator app]
        ]
        [Substrate Net
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-hash
        ]
        [Universe LamWorld :category CCC :substrate Net]
        [Theory LamTheory :in LamWorld]
    "#;
    process_all(&mut session, input).unwrap();
    let law_msg = session.output.iter().find(|s| s.contains("[LAWS]")).unwrap();
    assert!(law_msg.contains("3 witness tests"), "Got: {}", law_msg);
}

#[test]
fn law_check_inconclusive_on_fuel() {
    // We can't easily trigger fuel exhaustion in a test, but we can verify
    // the code path exists by checking that LawInconclusive is a valid error variant
    let err = hyperion::error::HyperionError::LawInconclusive {
        theory: "T".into(),
        law: "L".into(),
        detail: "fuel exhausted".into(),
    };
    let msg = format!("{}", err);
    assert!(msg.contains("INCONCLUSIVE"), "Got: {}", msg);
}

// ============================================================
// Fix 3: VerifyFunctor tests
// ============================================================

#[test]
fn verify_functor_parses() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C
            [Object Nat]
            [Morphism z :domain [] :codomain Nat]
            [Morphism s :domain [Nat] :codomain Nat]
            [Morphism plus :domain [Nat Nat] :codomain Nat]
        ]
        [Substrate A
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Substrate B
            @engine term-tree
            @resource-mode deep-copy
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe UA :category C :substrate A]
        [Universe UB :category C :substrate B]
        [Functor F :from A :to B :verify]
        [Theory T1 :in UA
            [@rule [plus z ?n] ==> ?n]
        ]
        [Theory T2 :in UB
            [@rule [plus z ?n] ==> ?n]
        ]
        [VerifyFunctor F :source T1 :target T2]
    "#;
    process_all(&mut session, input).unwrap();
    assert!(session.output.iter().any(|s| s.contains("[VERIFY-FUNCTOR]")));
}

#[test]
fn verify_functor_passes() {
    // Identity functor (no op_map) — rules preserved exactly
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C
            [Object Nat]
            [Morphism z :domain [] :codomain Nat]
            [Morphism s :domain [Nat] :codomain Nat]
            [Morphism plus :domain [Nat Nat] :codomain Nat]
        ]
        [Substrate A
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Substrate B
            @engine term-tree
            @resource-mode deep-copy
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe UA :category C :substrate A]
        [Universe UB :category C :substrate B]
        [Functor F :from A :to B]
        [Theory Source :in UA
            [@rule [plus z ?n] ==> ?n]
            [@rule [plus [s ?n] ?m] ==> [s [plus ?n ?m]]]
        ]
        [Theory Target :in UB
            [@rule [plus z ?n] ==> ?n]
            [@rule [plus [s ?n] ?m] ==> [s [plus ?n ?m]]]
        ]
        [VerifyFunctor F :source Source :target Target]
    "#;
    process_all(&mut session, input).unwrap();
    let msg = session.output.iter().find(|s| s.contains("[VERIFY-FUNCTOR]")).unwrap();
    assert!(msg.contains("2 rules verified"), "Got: {}", msg);
}

#[test]
fn verify_functor_fails() {
    // Target theory missing a rule → verification fails
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C
            [Object Nat]
            [Morphism z :domain [] :codomain Nat]
            [Morphism s :domain [Nat] :codomain Nat]
            [Morphism plus :domain [Nat Nat] :codomain Nat]
        ]
        [Substrate A
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Substrate B
            @engine term-tree
            @resource-mode deep-copy
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe UA :category C :substrate A]
        [Universe UB :category C :substrate B]
        [Functor F :from A :to B]
        [Theory Source :in UA
            [@rule [plus z ?n] ==> ?n]
            [@rule [plus [s ?n] ?m] ==> [s [plus ?n ?m]]]
        ]
        [Theory Target :in UB
            ;; Only one rule — missing plus-step
            [@rule [plus z ?n] ==> ?n]
        ]
        [VerifyFunctor F :source Source :target Target]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("law violation") || msg.contains("failed"), "Got: {}", msg);
}

#[test]
fn verify_functor_applies_op_map() {
    // Functor with op_map: z→zero, s→succ, plus→add
    // Both universes use the same category (required for functor morphism generation)
    // but the target theory uses renamed ops via functor's op_map
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C
            [Object Nat]
            [Morphism z :domain [] :codomain Nat]
            [Morphism s :domain [Nat] :codomain Nat]
            [Morphism plus :domain [Nat Nat] :codomain Nat]
            [Morphism zero :domain [] :codomain Nat]
            [Morphism succ :domain [Nat] :codomain Nat]
            [Morphism add :domain [Nat Nat] :codomain Nat]
        ]
        [Substrate A
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Substrate B
            @engine term-tree
            @resource-mode deep-copy
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe UA :category C :substrate A]
        [Universe UB :category C :substrate B]
        [Functor F
            :from A :to B
            :map-morphism [z zero]
            :map-morphism [s succ]
            :map-morphism [plus add]
        ]
        [Theory Source :in UA
            [@rule [plus z ?n] ==> ?n]
            [@rule [plus [s ?n] ?m] ==> [s [plus ?n ?m]]]
        ]
        [Theory Target :in UB
            [@rule [add zero ?n] ==> ?n]
            [@rule [add [succ ?n] ?m] ==> [succ [add ?n ?m]]]
        ]
        [VerifyFunctor F :source Source :target Target]
    "#;
    process_all(&mut session, input).unwrap();
    let msg = session.output.iter().find(|s| s.contains("[VERIFY-FUNCTOR]")).unwrap();
    assert!(msg.contains("2 rules verified"), "Got: {}", msg);
}

#[test]
fn cross_substrate_verified() {
    // The updated cross-substrate.hyp includes VerifyFunctor
    run_file("examples/cross-substrate.hyp");
}
