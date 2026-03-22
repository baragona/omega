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
    // PathType + Evaluator requires lambda-capable engine
    let mut session = HyperionSession::new();
    let input = r#"
        [Category PathCat
            [Object X]
            [Morphism app :domain [X X] :codomain X]
            [Evaluator app]
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
    assert!(msg.contains("PathType") || msg.contains("Exponential"), "Got: {}", msg);
}

#[test]
fn path_type_without_evaluator_on_non_lambda_engine() {
    // PathType WITHOUT Evaluator is purely first-order — no lambda needed
    let mut session = HyperionSession::new();
    let input = r#"
        [Category PathCat
            [Object X]
            [PathType :refl refl :concat concat :inv inv :ap ap]
        ]
        [Substrate Grid
            @engine symmetric-monoidal
            @resource-mode deep-copy
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe OK :category PathCat :substrate Grid]
    "#;
    process_all(&mut session, input).expect("PathType without Evaluator should work on non-lambda engine");
}

#[test]
fn symmetric_monoidal_compound_syntax() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category MonCat
            [Object T]
            [Morphism tensor :domain [T T] :codomain T]
            [Morphism unit :domain [] :codomain T]
            [SymmetricMonoidal tensor unit]
        ]
        [Substrate Net
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe MonWorld :category MonCat :substrate Net]
        [Theory MonTheory :in MonWorld
            [@rule [tensor [tensor ?a ?b] ?c] ==> [tensor ?a [tensor ?b ?c]]]
            [@rule [tensor unit ?a] ==> ?a]
            [@rule [tensor ?a unit] ==> ?a]
        ]
    "#;
    process_all(&mut session, input).expect("SymmetricMonoidal compound syntax should work");
    let output = session.output.join("\n");
    assert!(output.contains("6 witness tests"), "Should verify monoidal laws: {}", output);
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
            [assert-eq ap-concat [ap f [concat p q]] [concat [ap f p] [ap f q]]]
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
fn verify_functor_resource_leak_rejected() {
    // Source in optimal-sharing has a duplicating rule [dup ?x] ==> [tensor ?x ?x].
    // Target in strictly-linear has the same rule text — but VerifyFunctor must reject
    // because the mapped source rule violates the target's resource mode.
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C
            [Object T]
            [Morphism dup :domain [T] :codomain T]
            [Morphism tensor :domain [T T] :codomain T]
            [Morphism id :domain [T] :codomain T]
        ]
        [Substrate Sharing
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Substrate Linear
            @engine interaction-graph
            @resource-mode strictly-linear
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe USrc :category C :substrate Sharing]
        [Universe UTgt :category C :substrate Linear]
        [Functor F :from Sharing :to Linear]
        [Theory Source :in USrc :no-laws
            [@rule [dup ?x] ==> [tensor ?x ?x]]
        ]
        [Theory Target :in UTgt :no-laws
            [@rule [id ?x] ==> ?x]
        ]
        [VerifyFunctor F :source Source :target Target]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("resource violation"), "Expected resource violation, got: {}", msg);
    assert!(msg.contains("strictly-linear"), "Should mention strictly-linear, got: {}", msg);
}

#[test]
fn verify_functor_resource_affine_rejects_dup() {
    // Source in optimal-sharing duplicates; target is affine — should reject
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C
            [Object T]
            [Morphism dup :domain [T] :codomain T]
            [Morphism tensor :domain [T T] :codomain T]
            [Morphism id :domain [T] :codomain T]
        ]
        [Substrate Sharing
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Substrate Aff
            @engine interaction-graph
            @resource-mode affine
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe USrc :category C :substrate Sharing]
        [Universe UTgt :category C :substrate Aff]
        [Functor F :from Sharing :to Aff]
        [Theory Source :in USrc :no-laws
            [@rule [dup ?x] ==> [tensor ?x ?x]]
        ]
        [Theory Target :in UTgt :no-laws
            [@rule [id ?x] ==> ?x]
        ]
        [VerifyFunctor F :source Source :target Target]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("resource violation"), "Expected resource violation, got: {}", msg);
    assert!(msg.contains("affine"), "Should mention affine, got: {}", msg);
}

// ============================================================
// Eckmann-Hilton: 2-Categorical Interchange from 1D Path Algebra
// ============================================================

#[test]
fn eckmann_hilton_example() {
    // Parts 1-5: refl interchange, ap-concat distributivity, naturality gap (assert-neq),
    // true Eckmann-Hilton via e-graph (assert-eq + eval-simplify), and physics dependence
    // (assert-neq: same laws, directed substrate, discovery blocked)
    run_file("examples/eckmann-hilton.hyp");
}

// ============================================================
// Ouroboros: Strictly-Linear Meta-Universes
// ============================================================

#[test]
fn ouroboros_drop_framework_rejected() {
    // In a strictly-linear meta-universe, dropping a framework (?C) is forbidden.
    // [compose-cat ?C ?D] ==> ?D drops ?C — must fail.
    let mut session = HyperionSession::new();
    let input = r#"
        [Category MetaCat
            [Object Cat]
            [Morphism functor :domain [Cat Cat] :codomain Cat]
            [Morphism compose-cat :domain [Cat Cat] :codomain Cat]
        ]
        [Substrate LinearMetaPhysics
            @engine interaction-graph
            @resource-mode strictly-linear
            @barrier transparent
            @equality topological-homotopy
        ]
        [Universe LinearMetaWorld :category MetaCat :substrate LinearMetaPhysics]
        [Theory MetaDrop :in LinearMetaWorld :no-laws
            [@rule drop-framework [compose-cat ?C ?D] ==> ?D]
        ]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("resource violation"), "Expected resource violation, got: {}", msg);
    assert!(msg.contains("strictly-linear requires exactly 1 use"), "Got: {}", msg);
}

#[test]
fn ouroboros_clone_framework_rejected() {
    // In a strictly-linear meta-universe, duplicating a framework (?C) is forbidden.
    // [functor ?C ?D] ==> [compose-cat ?C [functor ?C ?D]] duplicates ?C — must fail.
    let mut session = HyperionSession::new();
    let input = r#"
        [Category MetaCat
            [Object Cat]
            [Morphism functor :domain [Cat Cat] :codomain Cat]
            [Morphism compose-cat :domain [Cat Cat] :codomain Cat]
        ]
        [Substrate LinearMetaPhysics
            @engine interaction-graph
            @resource-mode strictly-linear
            @barrier transparent
            @equality topological-homotopy
        ]
        [Universe LinearMetaWorld :category MetaCat :substrate LinearMetaPhysics]
        [Theory MetaClone :in LinearMetaWorld :no-laws
            [@rule clone-framework [functor ?C ?D] ==> [compose-cat ?C [functor ?C ?D]]]
        ]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("resource violation"), "Expected resource violation, got: {}", msg);
    assert!(msg.contains("strictly-linear requires exactly 1 use"), "Got: {}", msg);
}

#[test]
fn ouroboros_linear_meta_rule_accepted() {
    run_file("examples/ouroboros-linear.hyp");
}

#[test]
fn verify_functor_resource_linear_to_linear_ok() {
    run_file("examples/verify-functor-resource.hyp");
}

#[test]
fn cross_substrate_verified() {
    // The updated cross-substrate.hyp includes VerifyFunctor
    run_file("examples/cross-substrate.hyp");
}

#[test]
fn simple_ccc_example() {
    run_file("examples/simple-ccc.hyp");
}

#[test]
fn modal_hott_example() {
    run_file("examples/modal-hott.hyp");
}

#[test]
fn named_rules_captured_for_verify_functor() {
    // Regression test: named @rule format [@rule name lhs ==> rhs]
    // must be captured correctly for VerifyFunctor (not just unnamed [@rule lhs ==> rhs])
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C [Object T] [Morphism f :domain [T] :codomain T]]
        [Substrate A @engine interaction-graph @resource-mode optimal-sharing @barrier transparent @equality rewrite-equivalence]
        [Substrate B @engine term-tree @resource-mode deep-copy @barrier transparent @equality topological-hash]
        [Universe U1 :category C :substrate A]
        [Universe U2 :category C :substrate B]
        [Theory T1 :in U1
            [@rule my-rule [f [f ?x]] ==> [f ?x]]
        ]
        [Theory T2 :in U2
            [@rule my-rule [f [f ?x]] ==> [f ?x]]
        ]
        [Functor F :from A :to B :verify]
        [VerifyFunctor F :source T1 :target T2]
    "#;
    process_all(&mut session, input).expect("named rules should be captured and verified");
    let output = session.output.join("\n");
    assert!(output.contains("[VERIFY-FUNCTOR]"), "should produce verify output");
    assert!(output.contains("1 rules verified"), "should verify 1 rule");
}

#[test]
fn monoidal_hott_example() {
    run_file("examples/monoidal-hott.hyp");
}

#[test]
fn wild_linear_meta_example() {
    run_file("examples/wild-linear-meta.hyp");
}

#[test]
fn preorder_auto_injects_reflexivity() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category P [Object X] [Morphism rel :domain [X X] :codomain X] [Preorder rel]]
        [Substrate S @engine term-tree @resource-mode deep-copy @barrier transparent @equality rewrite-equivalence]
        [Universe U :category P :substrate S]
        [Theory T :in U [const a X]]
        [Proofs Check :in T [assert-eq refl-test [rel a a] true]]
    "#;
    process_all(&mut session, input).expect("Preorder reflexivity should be auto-injected");
    let output = session.output.join("\n");
    assert!(output.contains("2 witness tests"), "Should verify preorder laws: {}", output);
}

#[test]
fn preorder_parses_in_category() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category P [Object X] [Morphism leq :domain [X X] :codomain X] [Preorder leq]]
    "#;
    process_all(&mut session, input).expect("Preorder should parse");
    let output = session.output.join("\n");
    assert!(output.contains("1 structures"), "Got: {}", output);
}

// ============================================================
// Meta-coherence: the "framework³" stress test
// ============================================================

#[test]
fn meta_coherence_example() {
    run_file("examples/meta-coherence.hyp");
}

#[test]
fn meta_coherence_both_universes_compile() {
    // The same MetaCat + PathType compiles to two different equality physics
    let mut session = HyperionSession::new();
    let input = r#"
        [Category MetaCat
          [Object Cat]
          [Morphism functor :domain [Cat Cat] :codomain Cat]
          [PathType :refl refl_cat :concat concat_cat :inv inv_cat :ap ap_cat]
        ]
        [Substrate MetaHash
          @engine interaction-graph @resource-mode optimal-sharing
          @barrier transparent @equality topological-hash]
        [Substrate MetaHomotopy
          @engine interaction-graph @resource-mode optimal-sharing
          @barrier transparent @equality topological-homotopy]
        [Universe MetaU_Hash :category MetaCat :substrate MetaHash]
        [Universe MetaU_HoTT :category MetaCat :substrate MetaHomotopy]
    "#;
    process_all(&mut session, input).expect("Both universes should compile");
    assert!(session.universes.contains_key("MetaU_Hash"));
    assert!(session.universes.contains_key("MetaU_HoTT"));
    // Verify they use different systems
    let sys1 = &session.universes["MetaU_Hash"].system_name;
    let sys2 = &session.universes["MetaU_HoTT"].system_name;
    assert_ne!(sys1, sys2, "Different substrates should produce different systems");
}

#[test]
fn meta_coherence_pathtype_law_verification() {
    // PathType at meta-level: 4 witness tests (no Evaluator → no ap-refl)
    let mut session = HyperionSession::new();
    let input = r#"
        [Category MetaCat
          [Object Cat]
          [Morphism functor :domain [Cat Cat] :codomain Cat]
          [PathType :refl refl_cat :concat concat_cat :inv inv_cat :ap ap_cat]
        ]
        [Substrate S @engine interaction-graph @resource-mode optimal-sharing
          @barrier transparent @equality topological-homotopy]
        [Universe U :category MetaCat :substrate S]
        [Theory T :in U
          [const A Cat]
          [@rule func-comp [functor [functor ?F ?G] ?H] ==> [functor ?F [functor ?G ?H]]]
          [@rule func-id-l [functor id_func ?G] ==> ?G]
          [@rule func-id-r [functor ?F id_func] ==> ?F]
          [@rule func-refl [functor ?F [refl_cat ?G]] ==> [refl_cat [functor ?F ?G]]]
        ]
    "#;
    process_all(&mut session, input).expect("Meta PathType laws should pass");
    let output = session.output.join("\n");
    assert!(output.contains("4 witness tests"),
        "PathType without Evaluator should have 4 tests: {}", output);
    assert!(output.contains("passed categorical law verification"),
        "Laws should pass: {}", output);
}

#[test]
fn meta_coherence_normalization_and_transport() {
    // A nontrivial meta-term normalizes and transports correctly
    let mut session = HyperionSession::new();
    let input = r#"
        [Category MetaCat
          [Object Cat]
          [Morphism functor :domain [Cat Cat] :codomain Cat]
          [PathType :refl refl_cat :concat concat_cat :inv inv_cat :ap ap_cat]
        ]
        [Substrate SrcSub @engine interaction-graph @resource-mode optimal-sharing
          @barrier transparent @equality topological-hash]
        [Substrate TgtSub @engine interaction-graph @resource-mode optimal-sharing
          @barrier transparent @equality topological-homotopy]
        [Universe U1 :category MetaCat :substrate SrcSub]
        [Universe U2 :category MetaCat :substrate TgtSub]

        [Theory Src :in U1
          [const PreCat Cat]
          [const id_func Cat]
          [@rule func-comp [functor [functor ?F ?G] ?H] ==> [functor ?F [functor ?G ?H]]]
          [@rule func-id-l [functor id_func ?G] ==> ?G]
          [@rule func-id-r [functor ?F id_func] ==> ?F]
          [@rule func-refl [functor ?F [refl_cat ?G]] ==> [refl_cat [functor ?F ?G]]]
          [def t [functor [functor id_func id_func] PreCat]]
        ]

        [Proofs SrcCheck :in Src
          [assert-eq norm t PreCat]
        ]

        [Theory Tgt :in U2
          [const PreCat Cat]
          [const id_func Cat]
          [@rule func-comp [functor [functor ?F ?G] ?H] ==> [functor ?F [functor ?G ?H]]]
          [@rule func-id-l [functor id_func ?G] ==> ?G]
          [@rule func-id-r [functor ?F id_func] ==> ?F]
          [@rule func-refl [functor ?F [refl_cat ?G]] ==> [refl_cat [functor ?F ?G]]]
        ]

        [Functor F :from SrcSub :to TgtSub :verify]
        [VerifyFunctor F :source Src :target Tgt]

        [Theory Transported :in U2
          [const PreCat Cat]
          [Import result [F t]]
        ]

        [Proofs TransCheck :in Transported
          [assert-eq ok result PreCat]
        ]
    "#;
    process_all(&mut session, input).expect("Transport should preserve normalization");
    let output = session.output.join("\n");
    assert!(output.contains("[VERIFY-FUNCTOR]"), "Should verify functor: {}", output);
    assert!(output.contains("4 rules verified"), "Should verify 4 rules: {}", output);
}

#[test]
fn meta_coherence_pathtype_stable_across_equality_modes() {
    // PathType auto-injection works under both topological-hash and topological-homotopy
    let mut session = HyperionSession::new();
    let input = r#"
        [Category MetaCat
          [Object Cat]
          [Morphism functor :domain [Cat Cat] :codomain Cat]
          [PathType :refl refl_cat :concat concat_cat :inv inv_cat :ap ap_cat]
        ]
        [Substrate HashSub @engine interaction-graph @resource-mode optimal-sharing
          @barrier transparent @equality topological-hash]
        [Substrate HoTTSub @engine interaction-graph @resource-mode optimal-sharing
          @barrier transparent @equality topological-homotopy]
        [Universe U_Hash :category MetaCat :substrate HashSub]
        [Universe U_HoTT :category MetaCat :substrate HoTTSub]

        [Theory T_Hash :in U_Hash [const A Cat]]
        [Proofs P_Hash :in T_Hash
          [assert-eq lunit [concat_cat [refl_cat A] [refl_cat A]] [refl_cat A]]
          [assert-eq inv   [inv_cat [refl_cat A]]                 [refl_cat A]]
        ]

        [Theory T_HoTT :in U_HoTT [const A Cat]]
        [Proofs P_HoTT :in T_HoTT
          [assert-eq lunit [concat_cat [refl_cat A] [refl_cat A]] [refl_cat A]]
          [assert-eq inv   [inv_cat [refl_cat A]]                 [refl_cat A]]
        ]
    "#;
    process_all(&mut session, input).expect("PathType should work under both equality modes");
}

#[test]
fn meta_coherence_verify_functor_fails_on_broken_rule() {
    // Deliberately break a target rule and confirm VerifyFunctor catches it
    let mut session = HyperionSession::new();
    let input = r#"
        [Category MetaCat
          [Object Cat]
          [Morphism functor :domain [Cat Cat] :codomain Cat]
          [PathType :refl refl_cat :concat concat_cat :inv inv_cat :ap ap_cat]
        ]
        [Substrate A @engine interaction-graph @resource-mode optimal-sharing
          @barrier transparent @equality rewrite-equivalence]
        [Substrate B @engine term-tree @resource-mode deep-copy
          @barrier transparent @equality rewrite-equivalence]
        [Universe U1 :category MetaCat :substrate A]
        [Universe U2 :category MetaCat :substrate B]

        [Theory Src :in U1
          [@rule func-comp [functor [functor ?F ?G] ?H] ==> [functor ?F [functor ?G ?H]]]
          [@rule func-id-l [functor id_func ?G] ==> ?G]
          [@rule func-id-r [functor ?F id_func] ==> ?F]
        ]

        [Theory BrokenTgt :in U2
          [@rule func-comp [functor [functor ?F ?G] ?H] ==> [functor ?F [functor ?G ?H]]]
          [@rule func-id-l [functor id_func ?G] ==> ?G]
          ;; DELIBERATELY WRONG: func-id-r target is broken (returns id_func, not ?F)
          [@rule func-id-r [functor ?F id_func] ==> id_func]
        ]

        [Functor F :from A :to B :verify]
        [VerifyFunctor F :source Src :target BrokenTgt]
    "#;
    let result = process_all(&mut session, input);
    assert!(result.is_err(), "VerifyFunctor should fail when target rule is broken");
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("equational preservation") || err.contains("verify"),
        "Error should mention equational preservation: {}", err);
}

// ============================================================
// LF³ Grand Tour
// ============================================================

#[test]
fn lf3_grand_tour_example() {
    run_file("examples/lf3-grand-tour.hyp");
}

#[test]
fn lf3_grand_tour_part_a_law_counts() {
    // Part A: CCC(3) + Monoidal(6) + PathType+Eval(6) = 15 witness tests
    let mut session = HyperionSession::new();
    let input = r#"
        [Category StageHoTT
          [Object Type] [Object Term]
          [Morphism arrow :domain [Type Type] :codomain Type]
          [Morphism app :domain [Term Term] :codomain Term]
          [Exponential lam :object Term]
          [Evaluator app]
          [Morphism tensor :domain [Term Term] :codomain Term]
          [SymmetricMonoidal tensor unit]
          [PathType :refl refl :concat concat :inv inv :ap ap]
        ]
        [Substrate S @engine interaction-graph @resource-mode optimal-sharing
          @barrier transparent @equality rewrite-equivalence]
        [Universe U :category StageHoTT :substrate S]
        [Theory T :in U
          [const a Term]
          [@rule mon-assoc [tensor [tensor ?x ?y] ?z] ==> [tensor ?x [tensor ?y ?z]]]
          [@rule mon-lunit [tensor unit ?x] ==> ?x]
          [@rule mon-runit [tensor ?x unit] ==> ?x]
        ]
    "#;
    process_all(&mut session, input).expect("Part A laws should pass");
    let output = session.output.join("\n");
    assert!(output.contains("15 witness tests"),
        "CCC+Monoidal+PathType(+Eval) should be 15 tests: {}", output);
}

#[test]
fn lf3_grand_tour_part_b_law_counts() {
    // Part B: Monoidal(6) + PathType-no-Eval(4) = 10 witness tests
    let mut session = HyperionSession::new();
    let input = r#"
        [Category LinHoTT
          [Object Obj]
          [Morphism tensor :domain [Obj Obj] :codomain Obj]
          [SymmetricMonoidal tensor unit]
          [PathType :refl reflL :concat concatL :inv invL :ap apL]
        ]
        [Substrate S @engine interaction-graph @resource-mode affine
          @barrier transparent @equality rewrite-equivalence]
        [Universe U :category LinHoTT :substrate S]
        [Theory T :in U
          [const p Obj]
          [@rule assoc [tensor [tensor ?x ?y] ?z] ==> [tensor ?x [tensor ?y ?z]]]
          [@rule lunit [tensor unit ?x] ==> ?x]
          [@rule runit [tensor ?x unit] ==> ?x]
        ]
    "#;
    process_all(&mut session, input).expect("Part B laws should pass");
    let output = session.output.join("\n");
    assert!(output.contains("10 witness tests"),
        "Monoidal+PathType(no Eval) should be 10 tests: {}", output);
}

#[test]
fn no_laws_flag_skips_law_verification() {
    // :no-laws on a Theory skips categorical law verification
    let mut session = HyperionSession::new();
    let input = r#"
        [Category MonCat
          [Object Obj]
          [Morphism tensor :domain [Obj Obj] :codomain Obj]
          [SymmetricMonoidal tensor unit]
        ]
        [Substrate S @engine interaction-graph @resource-mode optimal-sharing
          @barrier transparent @equality rewrite-equivalence]
        [Universe U :category MonCat :substrate S]
        [Theory T :in U :no-laws
          [const a Obj]
        ]
    "#;
    // Without :no-laws, this would fail because T has no monoidal rules.
    // With :no-laws, it should succeed.
    process_all(&mut session, input).expect(":no-laws should skip law verification");
    let output = session.output.join("\n");
    assert!(!output.contains("witness tests"),
        ":no-laws should skip law verification entirely: {}", output);
}

#[test]
fn no_laws_flag_without_flag_fails() {
    // Without :no-laws, a theory lacking rules in a monoidal universe should fail laws
    let mut session = HyperionSession::new();
    let input = r#"
        [Category MonCat
          [Object Obj]
          [Morphism tensor :domain [Obj Obj] :codomain Obj]
          [SymmetricMonoidal tensor unit]
        ]
        [Substrate S @engine interaction-graph @resource-mode optimal-sharing
          @barrier transparent @equality rewrite-equivalence]
        [Universe U :category MonCat :substrate S]
        [Theory T :in U
          [const a Obj]
        ]
    "#;
    let result = process_all(&mut session, input);
    assert!(result.is_err(), "Theory without monoidal rules should fail law verification");
}

// ============================================================
// Resource enforcement tests
// ============================================================

#[test]
fn resource_linear_rejects_dup() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C [Object T] [Morphism dup :domain [T] :codomain T] [Morphism tensor :domain [T T] :codomain T]]
        [Substrate Lin
            @engine interaction-graph
            @resource-mode strictly-linear
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe U :category C :substrate Lin]
        [Theory T :in U :no-laws
            [@rule [dup ?x] ==> [tensor ?x ?x]]
        ]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("resource violation"), "Got: {}", msg);
    assert!(msg.contains("strictly-linear requires exactly 1 use"), "Got: {}", msg);
}

#[test]
fn resource_linear_rejects_drop() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C [Object T] [Morphism drop :domain [T] :codomain T]]
        [Substrate Lin
            @engine interaction-graph
            @resource-mode strictly-linear
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe U :category C :substrate Lin]
        [Theory T :in U :no-laws
            [@rule [drop ?x] ==> unit]
        ]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("resource violation"), "Got: {}", msg);
    assert!(msg.contains("strictly-linear requires exactly 1 use"), "Got: {}", msg);
}

#[test]
fn resource_affine_allows_drop() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C [Object T] [Morphism drop :domain [T] :codomain T]]
        [Substrate Aff
            @engine interaction-graph
            @resource-mode affine
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe U :category C :substrate Aff]
        [Theory T :in U :no-laws
            [@rule [drop ?x] ==> unit]
        ]
    "#;
    process_all(&mut session, input).expect("affine should allow drop (0 uses)");
}

#[test]
fn resource_affine_rejects_dup() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C [Object T] [Morphism dup :domain [T] :codomain T] [Morphism tensor :domain [T T] :codomain T]]
        [Substrate Aff
            @engine interaction-graph
            @resource-mode affine
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe U :category C :substrate Aff]
        [Theory T :in U :no-laws
            [@rule [dup ?x] ==> [tensor ?x ?x]]
        ]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("resource violation"), "Got: {}", msg);
    assert!(msg.contains("affine requires at most 1 use"), "Got: {}", msg);
}

#[test]
fn resource_sharing_allows_all() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C [Object T] [Morphism dup :domain [T] :codomain T] [Morphism tensor :domain [T T] :codomain T] [Morphism drop :domain [T] :codomain T]]
        [Substrate Sharing
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe U :category C :substrate Sharing]
        [Theory T :in U :no-laws
            [@rule [dup ?x] ==> [tensor ?x ?x]]
            [@rule [drop ?x] ==> unit]
        ]
    "#;
    process_all(&mut session, input).expect("optimal-sharing should allow dup and drop");
}

#[test]
fn resource_rejects_unbound_rhs_meta() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C [Object T] [Morphism tensor :domain [T T] :codomain T]]
        [Substrate Lin
            @engine interaction-graph
            @resource-mode strictly-linear
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe U :category C :substrate Lin]
        [Theory T :in U :no-laws
            [@rule [tensor ?x ?y] ==> ?z]
        ]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("resource violation"), "Got: {}", msg);
    assert!(msg.contains("unbound meta ?z"), "Got: {}", msg);
}

// ============================================================
// Barrier scope injection tests
// ============================================================

#[test]
fn barrier_scope_injection() {
    // Context declarations should become Scope declarations in Apeiron output
    let mut session = HyperionSession::new();
    let input = r#"
        [Category ModalCat
            [Object Prop]
            [Morphism box :domain [Prop] :codomain Prop]
            [ModalOperator box]
            [Context WorldA]
        ]
        [Substrate Net
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier contextual-membranes
            @equality rewrite-equivalence
        ]
        [Universe ModalWorld :category ModalCat :substrate Net]
        [Theory ModalTheory :in ModalWorld :no-laws
            [const p Prop]
        ]
    "#;
    process_all(&mut session, input).expect("barrier scope injection should work");
}

#[test]
fn barrier_stuckness_and_activation() {
    // barrier blocks inner reduction; with-scope enables it
    let mut session = HyperionSession::new();
    let input = r#"
        [Category CCC
            [Object Type] [Object Term]
            [Morphism app :domain [Term Term] :codomain Term]
            [Exponential lam :object Term]
            [Evaluator app]
            [Context WorldA]
        ]
        [Substrate Net
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier contextual-membranes
            @equality topological-hash
        ]
        [Universe LamWorld :category CCC :substrate Net]
        [Theory LamTheory :in LamWorld
            [const a Term]
        ]
        [Proofs BarrierCheck :in LamTheory
            ;; Without scope activation, barrier blocks inner beta
            [assert-neq barrier-stuck [barrier WorldA [app [lam x x] a]] a]

            ;; With scope active, inner reduction proceeds
            [with-scope WorldA
                [assert-eq barrier-inner [barrier WorldA [app [lam x x] a]] [barrier WorldA a]]
            ]
        ]
    "#;
    process_all(&mut session, input).expect("barrier stuckness and activation should work");
}

// ============================================================
// Eta-contraction tests
// ============================================================

#[test]
fn eta_homotopy_vs_hash() {
    // Under topological-homotopy, [lam x [app f x]] = f (eta-contraction)
    // Under topological-hash, [lam x [app f x]] != f
    let mut session = HyperionSession::new();
    let input = r#"
        [Category CCC
            [Object Type] [Object Term]
            [Morphism app :domain [Term Term] :codomain Term]
            [Exponential lam :object Term]
            [Evaluator app]
        ]
        [Substrate HomotopySub
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-homotopy
        ]
        [Substrate HashSub
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-hash
        ]
        [Universe HomotopyWorld :category CCC :substrate HomotopySub]
        [Universe HashWorld :category CCC :substrate HashSub]

        [Theory EtaHomotopy :in HomotopyWorld
            [const f Term]
        ]
        [Proofs EtaHomotopyCheck :in EtaHomotopy
            [assert-eq eta-equal [lam x [app f x]] f]
        ]

        [Theory EtaHash :in HashWorld
            [const f Term]
        ]
        [Proofs EtaHashCheck :in EtaHash
            [assert-neq eta-diff [lam x [app f x]] f]
        ]
    "#;
    process_all(&mut session, input).expect("eta homotopy vs hash should work");
}

#[test]
fn eta_does_not_loop() {
    // A term already in eta-normal form should terminate quickly
    let mut session = HyperionSession::new();
    let input = r#"
        [Category CCC
            [Object Type] [Object Term]
            [Morphism app :domain [Term Term] :codomain Term]
            [Exponential lam :object Term]
            [Evaluator app]
        ]
        [Substrate HomotopySub
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-homotopy
        ]
        [Universe EtaWorld :category CCC :substrate HomotopySub]
        [Theory EtaNormal :in EtaWorld
            [const a Term]
            [const b Term]
        ]
        [Proofs EtaCheck :in EtaNormal
            ;; These are eta-normal already — should just hash-compare fine
            [assert-eq already-normal a a]
            [assert-neq diff-terms a b]
            ;; [lam x [app f [app g x]]] is NOT an eta-redex (arg to f is [app g x], not x)
            ;; So it should remain distinct from [lam y [app f [app g y]]]... actually those ARE alpha-equal
            ;; Better test: [lam x [app f [app g x]]] != g
            [assert-neq not-eta [lam x [app f [app g x]]] g]
        ]
    "#;
    process_all(&mut session, input).expect("eta-normal terms should not loop");
}

#[test]
fn substrate_with_egraph() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category AlgCat
            [Object S]
            [Morphism f :domain [S S] :codomain S]
            [Morphism g :domain [S S] :codomain S]
        ]
        [Substrate EGraphNet
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality equality-saturation
        ]
        [Universe AlgUniverse :category AlgCat :substrate EGraphNet]
    "#;
    process_all(&mut session, input).expect("e-graph substrate should compile");

    // Verify the compiled system has equality-saturation check mode
    let all_output: Vec<String> = session
        .output
        .iter()
        .chain(session.apeiron.output.iter())
        .cloned()
        .collect();
    let output_str = all_output.join("\n");
    assert!(
        output_str.contains("EqualitySaturation"),
        "compiled system should include EqualitySaturation check mode, got: {}",
        output_str
    );
}

// ============================================================
// @law + eval-simplify example tests
// ============================================================

#[test]
fn schrodinger_egraph_barrier_blocks_jailbreak() {
    run_file("examples/schrodinger-egraph.hyp");
}

#[test]
fn equational_algebra_example() {
    run_file("examples/equational-algebra.hyp");
}

#[test]
fn transport_discovery_example() {
    // The Final Boss: e-graph discovery → VerifyFunctor (5 rules) → Import transport →
    // directed verification + physics gap (assert-neq)
    run_file("examples/transport-discovery.hyp");
}

#[test]
fn egraph_transport_example() {
    run_file("examples/egraph-transport.hyp");
}

// ============================================================
// Surjection gap closure tests
// ============================================================

#[test]
fn nominal_scoping_end_to_end() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category NomCat
            [Object Name]
            [Morphism bind :domain [Name Name] :codomain Name]
            [ModalOperator box]
            [Context Scope1]
        ]
        [Substrate NomSub
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier nominal-scoping
            @equality topological-hash
        ]
        [Universe NomWorld :category NomCat :substrate NomSub]
    "#;
    process_all(&mut session, input).expect("NominalScoping should compile");
    assert!(session.universes.contains_key("NomWorld"));
}

#[test]
fn nominal_rejects_exponential() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category CCC
            [Object Term]
            [Exponential lam :object Term]
        ]
        [Substrate NomSub
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier nominal-scoping
            @equality topological-hash
        ]
        [Universe Bad :category CCC :substrate NomSub]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("Nominal scoping"), "Got: {}", msg);
}

#[test]
fn reversible_engine_end_to_end() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category MonCat
            [Object Obj]
            [Morphism tensor :domain [Obj Obj] :codomain Obj]
            [TensorProduct tensor]
            [Unit unit]
        ]
        [Substrate RevSub
            @engine reversible-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe RevWorld :category MonCat :substrate RevSub]
        [Theory RevTheory :in RevWorld
            [@rule [tensor [tensor ?a ?b] ?c] ==> [tensor ?a [tensor ?b ?c]]]
            [@rule [tensor unit ?a] ==> ?a]
            [@rule [tensor ?a unit] ==> ?a]
        ]
    "#;
    process_all(&mut session, input).expect("ReversibleGraph should compile");
    assert!(session.universes.contains_key("RevWorld"));
}

#[test]
fn reversible_rejects_exponential() {
    // ReversibleGraph does not support lambda (beta-reduction is information-destroying)
    let mut session = HyperionSession::new();
    let input = r#"
        [Category CCC
            [Object Term]
            [Exponential lam :object Term]
        ]
        [Substrate RevSub
            @engine reversible-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality topological-hash
        ]
        [Universe Bad :category CCC :substrate RevSub]
    "#;
    let err = process_all(&mut session, input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("Exponential support"), "Got: {}", msg);
}

#[test]
fn concurrent_engine_end_to_end() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category CCC
            [Object Type] [Object Term]
            [Morphism app :domain [Term Term] :codomain Term]
            [Exponential lam :object Term]
            [Evaluator app]
        ]
        [Substrate ConcSub
            @engine concurrent-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe ConcWorld :category CCC :substrate ConcSub]
    "#;
    process_all(&mut session, input).expect("ConcurrentGraph should compile");
    assert!(session.universes.contains_key("ConcWorld"));
}

#[test]
fn extensional_equality_end_to_end() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category Simple [Object T] [Morphism f :domain [T] :codomain T]]
        [Substrate ExtSub
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality extensional-equivalence
        ]
        [Universe ExtWorld :category Simple :substrate ExtSub]
    "#;
    process_all(&mut session, input).expect("ExtensionalEquivalence should compile");
    assert!(session.universes.contains_key("ExtWorld"));
}

#[test]
fn full_unification_end_to_end() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category Simple [Object T] [Morphism f :domain [T] :codomain T]]
        [Substrate FullUnifSub
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality full-unification
        ]
        [Universe FullUnifWorld :category Simple :substrate FullUnifSub]
    "#;
    process_all(&mut session, input).expect("FullUnification should compile");
    assert!(session.universes.contains_key("FullUnifWorld"));
}

#[test]
fn typed_signature_generated() {
    // Verify that Apeiron session has a Signature with typed ops
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
    "#;
    process_all(&mut session, input).expect("Should compile with typed signature");
    assert!(session.registered_signatures.contains("__hyp_sig_Arith"));
    // The Apeiron session should have the signature registered
    assert!(session.apeiron.signatures.contains_key("__hyp_sig_Arith"));
    let sig = &session.apeiron.signatures["__hyp_sig_Arith"];
    assert_eq!(sig.sorts.len(), 1); // Nat
    assert_eq!(sig.operators.len(), 3); // z, s, plus
    // plus should have typed args [Nat, Nat, Nat]
    let plus_op = sig.operators.iter().find(|o| o.name == "plus").unwrap();
    assert_eq!(plus_op.args, vec!["Nat", "Nat", "Nat"]);
}

#[test]
fn signature_deduplication() {
    // Two universes with the same category should share a signature
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C [Object T] [Morphism f :domain [T] :codomain T]]
        [Substrate A @engine interaction-graph @resource-mode optimal-sharing @barrier transparent @equality rewrite-equivalence]
        [Substrate B @engine term-tree @resource-mode deep-copy @barrier transparent @equality rewrite-equivalence]
        [Universe U1 :category C :substrate A]
        [Universe U2 :category C :substrate B]
    "#;
    process_all(&mut session, input).expect("Should compile both universes");
    // Only one signature should be registered
    assert_eq!(session.registered_signatures.len(), 1);
    assert!(session.registered_signatures.contains("__hyp_sig_C"));
}

#[test]
fn surjection_demo_example() {
    run_file("examples/surjection-demo.hyp");
}

#[test]
fn adjoint_meta_ascent_example() {
    run_file("examples/adjoint-meta-ascent.hyp");
}

#[test]
fn weak_equivalence_example() {
    run_file("examples/weak-equivalence.hyp");
}

#[test]
fn weak_equivalence_verification() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C
          [Object T]
          [Morphism compose :domain [T T] :codomain T]
          [Morphism id :domain [] :codomain T]
          [Morphism A :domain [] :codomain T]
          [Morphism B :domain [] :codomain T]
          [Morphism f :domain [] :codomain T]
          [Morphism g :domain [] :codomain T]
        ]
        [Substrate S @engine interaction-graph @resource-mode optimal-sharing @barrier transparent @equality equality-saturation]
        [Universe U :category C :substrate S]
        [Theory Th :in U
          [@rule ul [compose id ?x] ==> ?x]
          [@rule ur [compose ?x id] ==> ?x]
          [@law fg [compose g f] === id]
          [@law gf [compose f g] === id]
          [@law fm [compose f A] === B]
          [@law gm [compose g B] === A]
        ]
        [WeakEquivalence E :source Th :target Th :on-types [[A B]] :via [[f g]] :verify true]
    "#;
    process_all(&mut session, input).expect("weak equivalence should verify");
    let output = session.output.join("\n");
    assert!(output.contains("VERIFIED"), "should contain VERIFIED: {}", output);
}

#[test]
fn weak_equivalence_no_verify() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category C [Object T] [Morphism compose :domain [T T] :codomain T] [Morphism id :domain [] :codomain T]]
        [Substrate S @engine interaction-graph @resource-mode optimal-sharing @barrier transparent @equality equality-saturation]
        [Universe U :category C :substrate S]
        [Theory Th :in U [@rule ul [compose id ?x] ==> ?x]]
        [WeakEquivalence E :source Th :target Th :on-types [[A B]] :via [[f g]] :verify false]
    "#;
    process_all(&mut session, input).expect("should register without verifying");
    let output = session.output.join("\n");
    assert!(output.contains("registered"), "should be registered: {}", output);
    assert!(!output.contains("VERIFIED"), "should not verify: {}", output);
}

#[test]
fn example_tactics_demo() {
    run_file("examples/tactics-demo.hyp");
}

#[test]
fn judgment_in_category() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category JudgCat
          [Object Term]
          [Morphism implies :domain [Term Term] :codomain Term]
          [Judgment typeof :inputs [Term Term] :output Term]
        ]
        [Substrate JNet
          @engine interaction-graph
          @resource-mode optimal-sharing
          @barrier transparent
          @equality rewrite-equivalence
        ]
        [Universe JWorld :category JudgCat :substrate JNet]
        [Theory JLogic :in JWorld
          [op p] [op q]
          [@derive ax-p :premises [] :conclusion [typeof p p]]
          [@derive ax-pq :premises [] :conclusion [typeof p [implies p q]]]
          [@derive mp :premises [[typeof ?A [implies ?A ?B]] [typeof ?A ?A]]
                      :conclusion [typeof ?A ?B]]
        ]
        [Proofs JCheck :in JLogic :no-laws
          [tactic mp-proof
            :goal [typeof p q]
            :steps [[apply mp] [auto 3] [auto 3]]]
        ]
    "#;
    process_all(&mut session, input).unwrap();
    let output = session.output.join("\n");
    assert!(output.contains("[TACTIC] mp-proof passed"), "output: {}", output);
}

#[test]
fn template_skips_law_verification() {
    let mut session = HyperionSession::new();
    let input = r#"
        [Category MonCat
          [Object Elem]
          [Morphism op :domain [Elem Elem] :codomain Elem]
        ]
        [Substrate MNet
          @engine interaction-graph
          @resource-mode optimal-sharing
          @barrier transparent
          @equality rewrite-equivalence
        ]
        [Universe MWorld :category MonCat :substrate MNet]
        [Theory Template :params [[T Sort] [binop Op]] :in MWorld
          [@rule unit-l [binop ?x ?x] ==> ?x]
        ]
    "#;
    // This should not error — templates skip law verification
    process_all(&mut session, input).unwrap();
}
