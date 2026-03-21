use metacosm::session::MetacosmSession;

fn process_all(session: &mut MetacosmSession, source: &str) -> Vec<String> {
    let sexps = apeiron::parser::parse(source).unwrap();
    let mut errors = Vec::new();
    for sexp in &sexps {
        if let Err(e) = session.process(sexp) {
            errors.push(format!("{}", e));
        }
    }
    errors
}

// ========== Omega mode (Layer 1): pass-through to Hyperion → Apeiron ==========

#[test]
fn omega_mode_category_passthrough() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [Category SimpleLogic
            [Object Prop]
            [Morphism mp :domain [Prop Prop] :codomain Prop]
        ]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.hyperion.categories.contains_key("SimpleLogic"));
}

#[test]
fn omega_mode_substrate_passthrough() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [Substrate MySubstrate
            @engine term-tree
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.hyperion.substrates.contains_key("MySubstrate"));
}

// ========== Hyperion mode (Layer 2): categories + substrates + universes ==========

#[test]
fn hyperion_mode_full() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [Category CCC
            [Object Type]
            [Object Term]
            [Morphism app :domain [Term Term] :codomain Term]
            [Exponential lam :object Type]
            [Evaluator app]
        ]
        [Substrate Std
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality rewrite-equivalence
        ]
        [Universe TestWorld :category CCC :substrate Std]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.hyperion.universes.contains_key("TestWorld"));
}

// ========== Cosmology mode (Layer 3): worlds + transitions + observables ==========

#[test]
fn world_registration() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World Explorer
            :category CartesianClosed
            :substrate ApeironStandard
            :epistemic [:discovery high :verification high :canonicality low :transportability medium :compression low]
            :admits [Split Tunnel]
        ]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.worlds.contains_key("Explorer"));
    let w = &session.worlds["Explorer"];
    assert_eq!(w.admissible_transitions.len(), 2);
}

#[test]
fn world_duplicate_rejected() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :category CartesianClosed :substrate ApeironStandard]
        [World A :category CartesianClosed :substrate ApeironStandard]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("duplicate"));
}

#[test]
fn transition_basic() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World Src :category CartesianClosed :substrate ApeironStandard
            :epistemic [:transportability high]
            :admits [Tunnel]]
        [World Dst :category CartesianClosed :substrate ApeironStandard
            :epistemic [:verification high]]
        [Transition T :kind Tunnel :from Src :to Dst :preserves [Soundness]]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.transitions.contains_key("T"));
}

#[test]
fn transition_undefined_source() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World Dst :category CartesianClosed :substrate ApeironStandard]
        [Transition T :kind Tunnel :from NoSuchWorld :to Dst]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("undefined"));
}

#[test]
fn tunnel_target_must_verify() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :category CartesianClosed :substrate ApeironStandard
            :epistemic [:transportability high]]
        [World B :category CartesianClosed :substrate ApeironStandard
            :epistemic [:verification none]]
        [Transition T :kind Tunnel :from A :to B]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("cannot verify"));
}

#[test]
fn tunnel_source_must_transport() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :category CartesianClosed :substrate ApeironStandard
            :epistemic [:transportability none]]
        [World B :category CartesianClosed :substrate ApeironStandard
            :epistemic [:verification high]]
        [Transition T :kind Tunnel :from A :to B]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("cannot transport"));
}

#[test]
fn invariant_conflict_rejected() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :category CartesianClosed :substrate ApeironStandard]
        [World B :category CartesianClosed :substrate ApeironStandard]
        [Transition T :kind Split :from A :to B
            :preserves [Soundness]
            :breaks [Soundness]]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("invariant"));
}

#[test]
fn transition_not_admitted() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :category CartesianClosed :substrate ApeironStandard
            :admits [Split]]
        [World B :category CartesianClosed :substrate ApeironStandard]
        [Transition T :kind Tunnel :from A :to B]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("does not admit"));
}

#[test]
fn observable_and_measure() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :category CartesianClosed :substrate ApeironStandard
            :epistemic [:discovery high]]
        [Observable DiscPower :kind discovery-cost]
        [Measure :observable DiscPower :world A]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert_eq!(session.measurements.len(), 1);
}

#[test]
fn transport_cost_measurement() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :category CartesianClosed :substrate ApeironStandard
            :epistemic [:discovery high :verification low]]
        [World B :category CartesianClosed :substrate ApeironStandard
            :epistemic [:discovery low :verification high]]
        [Observable Dist :kind transport-cost]
        [Measure :observable Dist :world A :target B]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert_eq!(session.measurements.len(), 1);
}

#[test]
fn family_validation() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :category CartesianClosed :substrate ApeironStandard]
        [World B :category CartesianClosed :substrate ApeironStandard]
        [Family MyFamily :worlds [A B] :invariants [Soundness]]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.families.contains_key("MyFamily"));
}

#[test]
fn family_undefined_world() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :category CartesianClosed :substrate ApeironStandard]
        [Family Bad :worlds [A NonExistent] :invariants [Soundness]]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("undefined"));
}

#[test]
fn pipeline_basic() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World Explorer :category CartesianClosed :substrate ApeironStandard
            :epistemic [:discovery high :transportability high]
            :admits [Tunnel]]
        [World Certifier :category CartesianClosed :substrate ApeironStandard
            :epistemic [:verification high]]
        [Pipeline Demo
            [Step search :action Discover :world Explorer]
            [Step tunnel :action Tunnel :world Explorer :target Certifier]
            [Step check :action Verify :world Certifier]
        ]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.pipelines.contains_key("Demo"));
}

#[test]
fn pipeline_infeasible_discovery() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World NoDiscover :category CartesianClosed :substrate ApeironStandard
            :epistemic [:discovery none]]
        [Pipeline Bad
            [Step search :action Discover :world NoDiscover]
        ]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("discovery=none"));
}

#[test]
fn pipeline_infeasible_tunnel() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World NoTransport :category CartesianClosed :substrate ApeironStandard
            :epistemic [:transportability none]]
        [World Target :category CartesianClosed :substrate ApeironStandard
            :epistemic [:verification high]]
        [Pipeline Bad
            [Step tunnel :action Tunnel :world NoTransport :target Target]
        ]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("transportability=none"));
}

// ========== Full cosmology demo ==========

#[test]
fn cosmology_demo_file() {
    let source = std::fs::read_to_string("examples/cosmology-demo.mcm").unwrap();
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, &source);
    assert!(errors.is_empty(), "errors: {:?}", errors);

    // Check all structures registered
    assert_eq!(session.worlds.len(), 3);
    assert_eq!(session.transitions.len(), 2);
    assert_eq!(session.observables.len(), 4);
    assert_eq!(session.families.len(), 1);
    assert_eq!(session.pipelines.len(), 1);
    assert_eq!(session.measurements.len(), 10);

    // Hyperion layer also populated
    assert!(session.hyperion.categories.contains_key("CartesianClosed"));
    assert!(session.hyperion.substrates.len() >= 3);
}

// ========== Conservative embedding properties ==========

#[test]
fn omega_is_trivial_world() {
    // Omega mode = one world with Implicit category, Default substrate, trivial epistemic
    let w = metacosm::world::WorldDef::omega_default("OmegaWorld");
    assert!(w.is_omega_mode());
    assert!(!w.is_hyperion_mode());
    assert_eq!(w.category, "Implicit");
    assert_eq!(w.substrate, "Default");
    assert!(w.admissible_transitions.is_empty());
    assert_eq!(w.epistemic, metacosm::epistemic::EpistemicProfile::trivial());
}

#[test]
fn hyperion_is_static_world() {
    // Hyperion mode = explicit category + substrate, no transitions
    let w = metacosm::world::WorldDef::hyperion("HypWorld", "CartesianClosed", "ApeironStandard");
    assert!(!w.is_omega_mode());
    assert!(w.is_hyperion_mode());
    assert!(w.admissible_transitions.is_empty());
}

#[test]
fn epistemic_dominance_is_partial_order() {
    use metacosm::epistemic::{EpistemicProfile, Capacity};
    let a = EpistemicProfile {
        discovery: Capacity::High,
        verification: Capacity::Medium,
        canonicality: Capacity::Low,
        transportability: Capacity::High,
        compression: Capacity::Medium,
    };
    let b = EpistemicProfile {
        discovery: Capacity::Medium,
        verification: Capacity::High,
        canonicality: Capacity::Medium,
        transportability: Capacity::Medium,
        compression: Capacity::High,
    };
    // Neither dominates the other
    assert!(!a.dominates(&b));
    assert!(!b.dominates(&a));
    // Self-dominance (reflexivity)
    assert!(a.dominates(&a));
}
