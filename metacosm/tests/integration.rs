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
            :epistemic [:discover complete :verify sound :canonicalize weak-nf :compress lossless]
            :admits [Split Tunnel]
        ]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.worlds.contains_key("Explorer"));
    let w = &session.worlds["Explorer"];
    assert_eq!(w.admissible_transitions.len(), 2);
}

#[test]
fn world_with_full_epistemic_syntax() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World W
            :category CartesianClosed
            :substrate ApeironStandard
            :epistemic [
                :discover [:strength semi-decidable]
                :verify [:capability yes :strength sound-complete]
                :canonicalize [:strength confluent]
                :compress [:strength codegen]
            ]
        ]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    let w = &session.worlds["W"];
    assert_eq!(w.epistemic.discover, metacosm::epistemic::DiscoveryStrength::SemiDecidable);
    assert_eq!(w.epistemic.verify, metacosm::epistemic::VerificationStrength::SoundComplete);
    assert_eq!(w.epistemic.canonicalize, metacosm::epistemic::CanonicalityStrength::Confluent);
    assert_eq!(w.epistemic.compress, metacosm::epistemic::CompressionMode::Codegen);
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
            :epistemic [:discover complete :verify sound]
            :admits [Tunnel]]
        [World Dst :category CartesianClosed :substrate ApeironStandard
            :epistemic [:verify decidable]]
        [Transition T :kind Tunnel :from Src :to Dst :preserves [Soundness]]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.transitions.contains_key("T"));
}

#[test]
fn transition_with_transport_epistemics() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :category CartesianClosed :substrate ApeironStandard
            :epistemic [:discover complete :verify sound]
            :admits [Tunnel]]
        [World B :category CartesianClosed :substrate ApeironStandard
            :epistemic [:verify decidable]]
        [Transition T :kind Tunnel :from A :to B
            :transport [:mode witness :loss [PathStructure]]
            :preserves [Soundness]]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    let t = &session.transitions["T"];
    assert_eq!(t.transport.mode, metacosm::transition::TransportMode::Witness);
    assert_eq!(t.transport.loss, vec![metacosm::transition::Invariant::PathStructure]);
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
            :epistemic [:discover complete]]
        [World B :category CartesianClosed :substrate ApeironStandard
            :epistemic [:verify none]]
        [Transition T :kind Tunnel :from A :to B]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("cannot verify"));
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
            :epistemic [:discover complete]]
        [Observable DiscPower :kind discovery-strength]
        [Measure :observable DiscPower :world A]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert_eq!(session.measurements.len(), 1);
}

#[test]
fn epistemic_distance_measurement() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :category CartesianClosed :substrate ApeironStandard
            :epistemic [:discover complete :verify heuristic]]
        [World B :category CartesianClosed :substrate ApeironStandard
            :epistemic [:discover none :verify decidable]]
        [Observable Dist :kind epistemic-distance]
        [Measure :observable Dist :world A :target B]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert_eq!(session.measurements.len(), 1);
}

#[test]
fn epistemic_distance_requires_target() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :category CartesianClosed :substrate ApeironStandard]
        [Observable Dist :kind epistemic-distance]
        [Measure :observable Dist :world A]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("requires :target"));
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
            :epistemic [:discover complete :verify sound]
            :admits [Tunnel]]
        [World Certifier :category CartesianClosed :substrate ApeironStandard
            :epistemic [:verify decidable]]
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
            :epistemic [:discover none]]
        [Pipeline Bad
            [Step search :action Discover :world NoDiscover]
        ]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("discover=none"));
}

#[test]
fn pipeline_infeasible_tunnel_target() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World Source :category CartesianClosed :substrate ApeironStandard
            :epistemic [:discover complete]]
        [World Target :category CartesianClosed :substrate ApeironStandard
            :epistemic [:verify none]]
        [Pipeline Bad
            [Step tunnel :action Tunnel :world Source :target Target]
        ]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("verify=none"));
}

// ========== Full cosmology demo ==========

#[test]
fn cosmology_demo_file() {
    let source = std::fs::read_to_string("examples/cosmology-demo.mcm").unwrap();
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, &source);
    assert!(errors.is_empty(), "errors: {:?}", errors);

    assert_eq!(session.worlds.len(), 3);
    assert_eq!(session.transitions.len(), 2);
    assert_eq!(session.observables.len(), 5);
    assert_eq!(session.families.len(), 1);
    assert_eq!(session.pipelines.len(), 1);
    assert_eq!(session.measurements.len(), 13);

    // Hyperion layer also populated
    assert!(session.hyperion.categories.contains_key("CartesianClosed"));
    assert!(session.hyperion.substrates.len() >= 3);
}

// ========== Conservative embedding properties ==========

#[test]
fn omega_is_trivial_world() {
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
    let w = metacosm::world::WorldDef::hyperion("HypWorld", "CartesianClosed", "ApeironStandard");
    assert!(!w.is_omega_mode());
    assert!(w.is_hyperion_mode());
    assert!(w.admissible_transitions.is_empty());
}

#[test]
fn epistemic_dominance_is_partial_order() {
    use metacosm::epistemic::*;
    let a = EpistemicProfile {
        discover: DiscoveryStrength::Complete,
        verify: VerificationStrength::Heuristic,
        canonicalize: CanonicalityStrength::None,
        compress: CompressionMode::None,
    };
    let b = EpistemicProfile {
        discover: DiscoveryStrength::None,
        verify: VerificationStrength::Decidable,
        canonicalize: CanonicalityStrength::UniqueNf,
        compress: CompressionMode::None,
    };
    // Neither dominates the other (a has better discovery, b has better verify+canonicalize)
    assert!(!a.dominates(&b));
    assert!(!b.dominates(&a));
    // Self-dominance (reflexivity)
    assert!(a.dominates(&a));
}

#[test]
fn strength_lattice_ordering() {
    use metacosm::epistemic::*;
    // Discovery lattice
    assert!(DiscoveryStrength::None < DiscoveryStrength::Heuristic);
    assert!(DiscoveryStrength::Heuristic < DiscoveryStrength::SemiDecidable);
    assert!(DiscoveryStrength::SemiDecidable < DiscoveryStrength::CompleteFragment);
    assert!(DiscoveryStrength::CompleteFragment < DiscoveryStrength::Complete);
    // Verification lattice
    assert!(VerificationStrength::None < VerificationStrength::Heuristic);
    assert!(VerificationStrength::Sound < VerificationStrength::SoundComplete);
    assert!(VerificationStrength::SoundComplete < VerificationStrength::Decidable);
    // Canonicality lattice
    assert!(CanonicalityStrength::None < CanonicalityStrength::WeakNf);
    assert!(CanonicalityStrength::Normalizing < CanonicalityStrength::Confluent);
    assert!(CanonicalityStrength::Confluent < CanonicalityStrength::UniqueNf);
}
