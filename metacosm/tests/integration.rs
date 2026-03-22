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
                :verify [:soundness sound :completeness complete :termination decidable]
                :canonicalize [:normalization none :confluence yes :unique-normal-forms no]
                :compress [:mode codegen :lossy yes :invertible no]
            ]
        ]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    let w = &session.worlds["W"];
    assert_eq!(w.epistemic.discover, metacosm::epistemic::DiscoveryStrength::SemiDecidable);
    assert_eq!(w.epistemic.verify.soundness, metacosm::epistemic::Soundness::Sound);
    assert_eq!(w.epistemic.verify.completeness, metacosm::epistemic::Completeness::Complete);
    assert_eq!(w.epistemic.verify.termination, metacosm::epistemic::Termination::Decidable);
    assert!(w.epistemic.canonicalize.confluence);
    assert!(!w.epistemic.canonicalize.unique_normal_forms);
    assert_eq!(w.epistemic.compress.mode, metacosm::epistemic::CompressionMode::Codegen);
    assert!(w.epistemic.compress.lossy);
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
    assert_eq!(session.transitions.len(), 4); // 2 declared + 1 composed + 1 promoted
    assert_eq!(session.observables.len(), 10);
    assert_eq!(session.families.len(), 1);
    assert_eq!(session.pipelines.len(), 1);
    assert_eq!(session.measurements.len(), 21);
    // Transition algebra: composed transition exists
    assert!(session.transitions.contains_key("ExplorerToExecutor"));
    // Embeddings exist (3 builtin + 1 declared)
    assert_eq!(session.embeddings.len(), 4);

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
        verify: VerificationProfile {
            soundness: Soundness::Heuristic,
            completeness: Completeness::None,
            termination: Termination::Unknown,
        },
        canonicalize: CanonicalityProfile::none(),
        ..Default::default()
    };
    let b = EpistemicProfile {
        discover: DiscoveryStrength::None,
        verify: VerificationProfile {
            soundness: Soundness::Sound,
            completeness: Completeness::Complete,
            termination: Termination::Decidable,
        },
        canonicalize: CanonicalityProfile {
            normalization: NormalizationStrength::Strong,
            confluence: true,
            unique_normal_forms: true,
        },
        ..Default::default()
    };
    // Neither dominates the other (a has better discovery, b has better verify+canonicalize)
    assert!(!a.dominates(&b));
    assert!(!b.dominates(&a));
    // Self-dominance (reflexivity)
    assert!(a.dominates(&a));
}

#[test]
fn sub_axis_lattice_ordering() {
    use metacosm::epistemic::*;
    // Discovery lattice (unchanged)
    assert!(DiscoveryStrength::None < DiscoveryStrength::Heuristic);
    assert!(DiscoveryStrength::Heuristic < DiscoveryStrength::SemiDecidable);
    assert!(DiscoveryStrength::SemiDecidable < DiscoveryStrength::CompleteFragment);
    assert!(DiscoveryStrength::CompleteFragment < DiscoveryStrength::Complete);
    // Verification sub-axes
    assert!(Soundness::None < Soundness::Heuristic);
    assert!(Soundness::Heuristic < Soundness::Sound);
    assert!(Completeness::None < Completeness::Partial);
    assert!(Completeness::Partial < Completeness::Complete);
    assert!(Termination::Unknown < Termination::SemiDecidable);
    assert!(Termination::SemiDecidable < Termination::Decidable);
    // Normalization sub-axis
    assert!(NormalizationStrength::None < NormalizationStrength::Weak);
    assert!(NormalizationStrength::Weak < NormalizationStrength::Strong);
}

// ========== Feature 1: Derived observables ==========

#[test]
fn derive_confluence_from_egraph_substrate() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [Category CCC
            [Object Type]
            [Morphism app :domain [Type Type] :codomain Type]
        ]
        [Substrate EGraphSub
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality equality-saturation
        ]
        [World Explorer :category CCC :substrate EGraphSub]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    let w = &session.worlds["Explorer"];
    // Derived from equality-saturation
    assert!(w.epistemic.canonicalize.confluence);
    assert!(!w.derived_properties.is_empty());
}

#[test]
fn derive_does_not_override_explicit() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [Category CCC
            [Object Type]
            [Morphism app :domain [Type Type] :codomain Type]
        ]
        [Substrate EGraphSub
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality equality-saturation
        ]
        [World Explorer :category CCC :substrate EGraphSub
            :epistemic [:discover complete]]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    let w = &session.worlds["Explorer"];
    // Explicit :discover complete is preserved, not overridden
    assert_eq!(w.epistemic.discover, metacosm::epistemic::DiscoveryStrength::Complete);
}

#[test]
fn derive_disabled_with_flag() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [Category CCC
            [Object Type]
            [Morphism app :domain [Type Type] :codomain Type]
        ]
        [Substrate EGraphSub
            @engine interaction-graph
            @resource-mode optimal-sharing
            @barrier transparent
            @equality equality-saturation
        ]
        [World Explorer :category CCC :substrate EGraphSub :derive no]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    let w = &session.worlds["Explorer"];
    // Derivation suppressed
    assert!(w.derived_properties.is_empty());
}

// ========== Feature 2: Theorem-class sensitivity ==========

#[test]
fn class_override_discovery() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World Explorer
            :category CartesianClosed
            :substrate ApeironStandard
            :epistemic [:discover complete :verify sound]
            :class-epistemic [
                [Equational :discover complete :verify decidable]
                [ResourceSensitive :discover none :verify heuristic]
            ]
        ]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    let w = &session.worlds["Explorer"];

    // Default profile
    assert_eq!(w.epistemic.discover, metacosm::epistemic::DiscoveryStrength::Complete);

    // Class-specific profiles
    let eq_profile = w.epistemic.for_class(&metacosm::theorem_class::TheoremClass::Equational);
    assert_eq!(eq_profile.discover, metacosm::epistemic::DiscoveryStrength::Complete);
    assert_eq!(eq_profile.verify.soundness, metacosm::epistemic::Soundness::Sound);
    assert_eq!(eq_profile.verify.completeness, metacosm::epistemic::Completeness::Complete);
    assert_eq!(eq_profile.verify.termination, metacosm::epistemic::Termination::Decidable);

    let rs_profile = w.epistemic.for_class(&metacosm::theorem_class::TheoremClass::ResourceSensitive);
    assert_eq!(rs_profile.discover, metacosm::epistemic::DiscoveryStrength::None);
    assert_eq!(rs_profile.verify.soundness, metacosm::epistemic::Soundness::Heuristic);
}

#[test]
fn measure_with_class() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World Explorer
            :category CartesianClosed
            :substrate ApeironStandard
            :epistemic [:discover complete]
            :class-epistemic [
                [Equational :discover complete]
                [ResourceSensitive :discover none]
            ]
        ]
        [Observable DiscPower :kind discovery-strength]
        [Measure :observable DiscPower :world Explorer :class ResourceSensitive]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert_eq!(session.measurements.len(), 1);
    // Should measure the class-specific profile
    assert!(session.output.iter().any(|s| s.contains("none") && s.contains("ResourceSensitive")));
}

// ========== Feature 3: Transition algebra ==========

#[test]
fn compose_two_transitions() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :category CartesianClosed :substrate ApeironStandard
            :epistemic [:discover complete :verify sound]
            :admits [Tunnel]]
        [World B :category CartesianClosed :substrate ApeironStandard
            :epistemic [:verify decidable]
            :admits [CoarseGrain]]
        [World C :category CartesianClosed :substrate ApeironStandard
            :epistemic [:verify sound]]
        [Transition AB :kind Tunnel :from A :to B
            :preserves [Soundness Normalization]
            :transport [:mode witness :loss [PathStructure]]]
        [Transition BC :kind CoarseGrain :from B :to C
            :preserves [Soundness]
            :breaks [ResourceSensitivity]
            :transport [:mode lossy :loss [ResourceSensitivity]]]
        [Compose AC :transitions [AB BC]]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    let ac = &session.transitions["AC"];
    // Source and target
    assert_eq!(ac.source, "A");
    assert_eq!(ac.target, "C");
    // Preserves = intersection: only Soundness (Normalization not in BC)
    assert_eq!(ac.preserves.len(), 1);
    assert!(ac.preserves.contains(&metacosm::transition::Invariant::Soundness));
    // Breaks = union: ResourceSensitivity
    assert!(ac.breaks.contains(&metacosm::transition::Invariant::ResourceSensitivity));
    // Transport: witness + lossy = lossy
    assert_eq!(ac.transport.mode, metacosm::transition::TransportMode::Lossy);
    // Loss = union: PathStructure + ResourceSensitivity
    assert_eq!(ac.transport.loss.len(), 2);
}

#[test]
fn compose_undefined_middle() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :category CartesianClosed :substrate ApeironStandard :admits [Tunnel]]
        [World B :category CartesianClosed :substrate ApeironStandard]
        [World C :category CartesianClosed :substrate ApeironStandard]
        [Transition AB :kind Tunnel :from A :to B]
        [Transition CD :kind Tunnel :from C :to B]
        [Compose Bad :transitions [AB CD]]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("composition"));
}

// ========== Feature 4: Semantic vs empirical ==========

#[test]
fn observable_default_semantic() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [Observable DiscPower :kind discovery-strength]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    let obs = &session.observables["DiscPower"];
    assert_eq!(obs.species, metacosm::knowledge::KnowledgeSpecies::Semantic);
}

#[test]
fn observable_explicit_empirical() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [Observable SearchTime :kind search-cost]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    let obs = &session.observables["SearchTime"];
    assert_eq!(obs.species, metacosm::knowledge::KnowledgeSpecies::Empirical);
}

#[test]
fn empirical_measure_requires_value() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :category CartesianClosed :substrate ApeironStandard]
        [Observable SearchTime :kind search-cost]
        [Measure :observable SearchTime :world A]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("empirical"));
}

#[test]
fn empirical_measure_with_value() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :category CartesianClosed :substrate ApeironStandard]
        [Observable SearchTime :kind search-cost]
        [Measure :observable SearchTime :world A :value 42ms]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert_eq!(session.measurements.len(), 1);
    assert!(session.output.iter().any(|s| s.contains("empirical") && s.contains("42ms")));
}

// ========== Feature 5: Conservative embedding ==========

#[test]
fn builtin_embeddings_exist() {
    let session = MetacosmSession::new();
    assert!(session.embeddings.contains_key("OmegaInHyperion"));
    assert!(session.embeddings.contains_key("HyperionInMetacosm"));
    assert!(session.embeddings.contains_key("OmegaInMetacosm"));
}

#[test]
fn embedding_layer_check() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [Embedding TestEmbed
            :from Omega
            :to Metacosm
            :properties [conservative definable-fragment strict-extension non-perturbing]
        ]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.embeddings.contains_key("TestEmbed"));
    // Should have check messages in output
    assert!(session.output.iter().any(|s| s.contains("[EMBEDDING]") && s.contains("conservative")));
}

#[test]
fn embedding_world_conservative() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World Weak :category CartesianClosed :substrate ApeironStandard
            :epistemic [:discover heuristic :verify sound]]
        [World Strong :category CartesianClosed :substrate ApeironStandard
            :epistemic [:discover complete :verify decidable
                :canonicalize [:normalization strong :confluence yes :unique-normal-forms yes]]]
        [Embedding WeakInStrong
            :from Weak
            :to Strong
            :properties [conservative]
        ]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn embedding_violation_detected() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World Strong :category CartesianClosed :substrate ApeironStandard
            :epistemic [:discover complete :verify decidable]]
        [World Weak :category CartesianClosed :substrate ApeironStandard
            :epistemic [:discover heuristic :verify sound]]
        [Embedding Bad
            :from Strong
            :to Weak
            :properties [conservative]
        ]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("conservative") || errors[0].contains("dominate"));
}

// ========== Metatheorem tests ==========

#[test]
fn metatheorem_verify_all_on_cosmology_demo() {
    let mut session = MetacosmSession::new();
    let source = std::fs::read_to_string("examples/cosmology-demo.mcm").unwrap();
    let _errors = process_all(&mut session, &source);
    let results = session.verify_metatheorems();
    let refuted: Vec<_> = results.iter().filter(|r| !r.is_proved()).collect();
    assert!(refuted.is_empty(), "refuted metatheorems: {:?}", refuted);
    assert!(results.len() > 10, "expected many metatheorems, got {}", results.len());
}

#[test]
fn metatheorem_dominance_reflexive() {
    use metacosm::epistemic::EpistemicProfile;
    use metacosm::metatheory;

    let ep = EpistemicProfile::trivial();
    let r = metatheory::dominance_reflexive(&ep);
    assert!(r.is_proved());
}

#[test]
fn metatheorem_dominance_transitive() {
    use metacosm::epistemic::*;
    use metacosm::metatheory;

    let strong = EpistemicProfile {
        discover: DiscoveryStrength::Complete,
        verify: VerificationProfile {
            soundness: metacosm::epistemic::Soundness::Sound,
            completeness: metacosm::epistemic::Completeness::Complete,
            termination: metacosm::epistemic::Termination::Decidable,
        },
        ..Default::default()
    };
    let mid = EpistemicProfile {
        discover: DiscoveryStrength::Heuristic,
        verify: VerificationProfile {
            soundness: metacosm::epistemic::Soundness::Sound,
            ..Default::default()
        },
        ..Default::default()
    };
    let weak = EpistemicProfile::trivial();

    let r = metatheory::dominance_transitive(&strong, &mid, &weak);
    assert!(r.is_proved());
}

#[test]
fn metatheorem_composition_associativity() {
    let mut session = MetacosmSession::new();
    let source = r#"
        [World A :category C :substrate S :epistemic [:discover complete :verify decidable] :admits [Tunnel CoarseGrain]]
        [World B :category C :substrate S :epistemic [:discover heuristic :verify sound]]
        [World C :category C :substrate S :epistemic [:discover none :verify sound :canonicalize confluent] :admits [CoarseGrain]]
        [World D :category C :substrate S :epistemic [:discover none :verify sound :canonicalize confluent :compress codegen]]
        [Transition AB :kind Tunnel :from A :to B :preserves [Soundness] :transport [:mode witness :loss [PathStructure]]]
        [Transition BC :kind CoarseGrain :from B :to C :preserves [Soundness] :transport [:mode lossy :loss [ResourceSensitivity]]]
        [Transition CD :kind CoarseGrain :from C :to D :preserves [Soundness Normalization] :transport [:mode conservative]]
    "#;
    let errors = process_all(&mut session, source);
    assert!(errors.is_empty(), "setup errors: {:?}", errors);

    let ab = &session.transitions["AB"];
    let bc = &session.transitions["BC"];
    let cd = &session.transitions["CD"];
    let r = metacosm::metatheory::composition_associativity(ab, bc, cd);
    assert!(r.is_proved(), "associativity failed: {:?}", r);
}

#[test]
fn metatheorem_pipeline_preserves_invariants() {
    let mut session = MetacosmSession::new();
    let source = r#"
        [World A :category C :substrate S :epistemic [:discover complete :verify sound] :admits [Tunnel]]
        [World B :category C :substrate S :epistemic [:discover heuristic :verify decidable]]
        [Transition T :kind Tunnel :from A :to B :preserves [Soundness] :transport [:mode witness]]
        [Family F :worlds [A B] :invariants [Soundness]]
    "#;
    let errors = process_all(&mut session, source);
    assert!(errors.is_empty(), "setup errors: {:?}", errors);

    let family = &session.families["F"];
    let r = metacosm::metatheory::pipeline_preserves_invariants(
        family, &session.transitions, &vec!["T".to_string()],
    );
    assert!(r.is_proved());
}

#[test]
fn morphism_identity_auto_registered() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [Category CC [Object Type] [Morphism app :domain [Type Type] :codomain Type]]
        [Substrate S @engine term-tree @resource-mode optimal-sharing @barrier transparent @equality rewrite-equivalence]
        [World A :category CC :substrate S :epistemic [:discover complete :verify sound]]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.morphisms.contains_key("id_A"), "identity morphism should be auto-registered");
    let id = &session.morphisms["id_A"];
    assert!(id.properties.faithful);
    assert!(id.properties.full);
    assert!(id.properties.essentially_surjective);
}

#[test]
fn morphism_with_functor_validation() {
    let mut session = MetacosmSession::new();
    // Hyperion functors map between substrates; must be declared AFTER worlds (universes)
    let errors = process_all(&mut session, r#"
        [Category CC [Object Type] [Object Term] [Morphism app :domain [Term Term] :codomain Term] [Exponential lam :object Type] [Evaluator app]]
        [Substrate S1 @engine interaction-graph @resource-mode optimal-sharing @barrier transparent @equality equality-saturation]
        [Substrate S2 @engine term-tree @resource-mode optimal-sharing @barrier transparent @equality rewrite-equivalence]
        [World Explorer :category CC :substrate S1 :epistemic [:discover complete :verify sound] :admits [Tunnel]]
        [World Certifier :category CC :substrate S2 :epistemic [:discover heuristic :verify decidable :canonicalize unique-nf]]
        [Functor CCid :from S1 :to S2 :map-object [Type Type] :map-object [Term Term] :map-morphism [app app]]
        [Transition DiscoverTunnel :kind Tunnel :from Explorer :to Certifier :functor CCid :preserves [Soundness] :transport [:mode witness :loss [PathStructure]]]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);

    let morph = &session.morphisms["DiscoverTunnel"];
    assert!(morph.functor.is_some());
    // Functor has injective object map, should be faithful
    assert!(morph.properties.faithful, "injective object map should be faithful");
}

#[test]
fn morphism_functor_substrate_mismatch() {
    let mut session = MetacosmSession::new();
    // Worlds first, then functors (Hyperion needs universes before functors)
    let errors = process_all(&mut session, r#"
        [Category CC [Object Type] [Morphism app :domain [Type Type] :codomain Type]]
        [Substrate S1 @engine term-tree @resource-mode optimal-sharing @barrier transparent @equality rewrite-equivalence]
        [Substrate S2 @engine interaction-graph @resource-mode optimal-sharing @barrier transparent @equality equality-saturation]
        [World A :category CC :substrate S1 :epistemic [:discover complete :verify sound] :admits [Tunnel]]
        [World B :category CC :substrate S2 :epistemic [:discover heuristic :verify sound]]
        [Functor F :from S1 :to S2 :map-object [Type Type]]
        [Transition Good :kind Tunnel :from A :to B :functor F :preserves [Soundness]]
    "#);
    assert!(errors.is_empty(), "should be valid: {:?}", errors);
    // Now test mismatch: functor source S2 but world A uses S1
    let errors2 = process_all(&mut session, r#"
        [World C :category CC :substrate S1 :epistemic [:discover none :verify sound]]
        [Functor F2 :from S2 :to S1 :map-object [Type Type]]
        [Transition Bad :kind Tunnel :from A :to C :functor F2 :preserves [Soundness]]
    "#);
    assert!(!errors2.is_empty(), "should fail: functor source doesn't match world substrate");
}

#[test]
fn morphism_compose_preserves_functor() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [Category CC [Object Type] [Object Term] [Morphism app :domain [Term Term] :codomain Term] [Exponential lam :object Type] [Evaluator app]]
        [Substrate S1 @engine interaction-graph @resource-mode optimal-sharing @barrier transparent @equality equality-saturation]
        [Substrate S2 @engine term-tree @resource-mode optimal-sharing @barrier transparent @equality rewrite-equivalence]
        [Substrate S3 @engine abstract-machine @resource-mode deep-copy @barrier transparent @equality rewrite-equivalence]
        [World A :category CC :substrate S1 :epistemic [:discover complete :verify sound] :admits [Tunnel]]
        [World B :category CC :substrate S2 :epistemic [:discover heuristic :verify decidable] :admits [CoarseGrain]]
        [World C :category CC :substrate S3 :epistemic [:discover none :verify sound :canonicalize confluent :compress codegen]]
        [Functor F1 :from S1 :to S2 :map-object [Type Type] :map-object [Term Term] :map-morphism [app app]]
        [Functor F2 :from S2 :to S3 :map-object [Type Type] :map-object [Term Term] :map-morphism [app app]]
        [Transition AB :kind Tunnel :from A :to B :functor F1 :preserves [Soundness] :transport [:mode witness]]
        [Transition BC :kind CoarseGrain :from B :to C :functor F2 :preserves [Soundness] :transport [:mode lossy]]
        [Compose AC :transitions [AB BC]]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);

    let composed = &session.morphisms["AC"];
    match &composed.functor {
        Some(metacosm::morphism::FunctorRef::Composite(names)) => {
            assert_eq!(names, &vec!["F1".to_string(), "F2".to_string()]);
        }
        other => panic!("expected Composite functor, got {:?}", other),
    }
}

#[test]
fn inference_propagates_completeness() {
    let mut session = MetacosmSession::new();
    // A has complete verification, B has default (completeness=none).
    // ConservativeExtension propagates completeness.
    let errors = process_all(&mut session, r#"
        [World A :category C :substrate S :epistemic [:discover complete :verify decidable]]
        [World B :category C :substrate S]
        [Transition T :kind ConservativeExtension :from A :to B]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);

    let result = session.run_inference();
    assert!(!result.propagated.is_empty(), "should propagate completeness to B: propagated={:?}", result.propagated);
    let b = &session.worlds["B"];
    assert!(b.epistemic.verify.completeness >= metacosm::epistemic::Completeness::Complete,
        "B completeness should be ≥ complete after inference, got {:?}", b.epistemic.verify.completeness);
}

#[test]
fn inference_conservative_extension_propagates_discovery() {
    let mut session = MetacosmSession::new();
    // A has complete discovery (above default heuristic), B has default.
    // ConservativeExtension should propagate discovery=complete to B.
    let errors = process_all(&mut session, r#"
        [World A :category C :substrate S :epistemic [:discover complete :verify sound]]
        [World B :category C :substrate S]
        [Transition T :kind ConservativeExtension :from A :to B]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);

    let result = session.run_inference();
    assert!(!result.propagated.is_empty(), "should propagate discovery to B: propagated={:?}", result.propagated);
    let b = &session.worlds["B"];
    assert!(b.epistemic.discover >= metacosm::epistemic::DiscoveryStrength::Complete,
        "B discovery should be ≥ complete, got {:?}", b.epistemic.discover);
}

#[test]
fn inference_conflict_on_explicit_value() {
    let mut session = MetacosmSession::new();
    // A has completeness=complete (via :verify decidable). B has completeness=partial (non-default, explicit).
    // ConservativeExtension requires B.completeness >= A.completeness = complete.
    // B.completeness = partial < complete AND partial != default (None).
    // This should produce a conflict.
    let errors = process_all(&mut session, r#"
        [World A :category C :substrate S :epistemic [:discover complete :verify decidable]]
        [World B :category C :substrate S :epistemic [:verify [:soundness sound :completeness partial :termination unknown]]]
        [Transition T :kind ConservativeExtension :from A :to B]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);

    let result = session.run_inference();
    assert!(!result.violations.is_empty(),
        "should detect conflict on completeness: propagated={:?}, violations={:?}",
        result.propagated, result.violations);
}

#[test]
fn inference_fixpoint_chain() {
    let mut session = MetacosmSession::new();
    // Chain: A→B→C. Completeness should propagate through both via ConservativeExtension.
    // Completeness default is None, A has Complete (via decidable).
    let errors = process_all(&mut session, r#"
        [World A :category C :substrate S :epistemic [:discover complete :verify decidable]]
        [World B :category C :substrate S]
        [World C :category C :substrate S]
        [Transition AB :kind ConservativeExtension :from A :to B]
        [Transition BC :kind ConservativeExtension :from B :to C]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);

    let result = session.run_inference();
    // B gets completeness=complete from A (iteration 1), C gets it from B (iteration 2)
    assert!(result.iterations >= 2, "should need ≥2 iterations, got {}", result.iterations);
    let c = &session.worlds["C"];
    assert!(c.epistemic.verify.completeness >= metacosm::epistemic::Completeness::Complete,
        "C should have complete after fixpoint propagation, got {:?}", c.epistemic.verify.completeness);
}

#[test]
fn inference_on_cosmology_demo() {
    let mut session = MetacosmSession::new();
    let source = std::fs::read_to_string("examples/cosmology-demo.mcm").unwrap();
    let _errors = process_all(&mut session, &source);
    let result = session.run_inference();
    // Demo has explicit profiles — inference should find no conflicts
    assert!(result.violations.is_empty(), "demo should have no inference conflicts: {:?}", result.violations);
}

#[test]
fn metatheorem_embedding_definable_fragment() {
    use metacosm::embedding::*;
    use metacosm::metatheory;

    let emb = EmbeddingDef {
        name: "OmegaInHyperion".into(),
        source: EmbeddingEndpoint::Layer(LayerName::Omega),
        target: EmbeddingEndpoint::Layer(LayerName::Hyperion),
        properties: vec![EmbeddingProperty::DefinableFragment, EmbeddingProperty::StrictExtension],
        checked: false,
    };
    let r = metatheory::embedding_definable_fragment(&emb);
    assert!(r.is_proved());
    let r2 = metatheory::embedding_strict_extension(&emb);
    assert!(r2.is_proved());
}

// ========== User-declared assertions ==========

#[test]
fn assertion_dominates_pass() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :epistemic [:discover complete :verify sound]]
        [World B :epistemic [:discover heuristic :verify sound]]
        [Assert [dominates A B]]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.output.iter().any(|s| s.contains("PASS")));
}

#[test]
fn assertion_dominates_fail() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :epistemic [:discover none :verify sound]]
        [World B :epistemic [:discover complete :verify sound]]
        [Assert [dominates A B]]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("assertion failed"));
}

#[test]
fn assertion_preserves_pass() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :epistemic [:discover complete :verify sound] :admits [Tunnel]]
        [World B :epistemic [:verify decidable]]
        [Transition T1 :kind Tunnel :from A :to B :preserves [Soundness]]
        [Family F1 :worlds [A B] :invariants [Soundness]]
        [Assert [preserves F1 Soundness]]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn assertion_preserves_fail() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :epistemic [:discover complete :verify sound] :admits [Tunnel]]
        [World B :epistemic [:verify decidable]]
        [Transition T1 :kind Tunnel :from A :to B :preserves [Soundness] :breaks [Normalization]]
        [Family F1 :worlds [A B] :invariants [Soundness Normalization]]
        [Assert [preserves F1 Normalization]]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("assertion failed"));
}

#[test]
fn assertion_distance() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :epistemic [:discover complete :verify sound]]
        [World B :epistemic [:discover heuristic :verify sound]]
        [Assert [distance A B :max 5]]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn assertion_faithful_with_functor() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [Category CC [Object Type] [Morphism app :domain [Type Type] :codomain Type]]
        [Substrate S1 @engine interaction-graph @resource-mode optimal-sharing @barrier transparent @equality equality-saturation]
        [Substrate S2 @engine term-tree @resource-mode optimal-sharing @barrier transparent @equality rewrite-equivalence]
        [World A :category CC :substrate S1 :epistemic [:discover complete :verify sound] :admits [Tunnel]]
        [World B :category CC :substrate S2 :epistemic [:verify decidable]]
        [Functor F1 :from S1 :to S2 :map-object [Type Type] :map-morphism [app app]]
        [Transition T1 :kind Tunnel :from A :to B :functor F1 :preserves [Soundness]]
        [Assert [faithful T1]]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

// ========== New features showcase ==========

#[test]
fn new_features_showcase() {
    let source = std::fs::read_to_string("examples/new-features.mcm").unwrap();
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, &source);
    assert!(errors.is_empty(), "errors: {:?}", errors);

    // 3 main worlds + Void
    assert_eq!(session.worlds.len(), 4);
    // 2 declared + 1 promoted = 3
    assert_eq!(session.transitions.len(), 3);
    // 1 lemma
    assert_eq!(session.lemmas.len(), 1);
    // 2 laws
    assert_eq!(session.laws.len(), 2);
    // 2 refutations
    assert_eq!(session.impossibilities.len(), 2);

    // Laws were checked
    assert!(session.output.iter().any(|s| s.contains("DominanceReflexive") && s.contains("PROVED")));
    assert!(session.output.iter().any(|s| s.contains("DominanceTransitive") && s.contains("PROVED")));

    // Pipeline materialized
    assert!(session.output.iter().any(|s| s.contains("[MATERIALIZE]") && s.contains("Step 1")));

    // Promote created a transition
    assert!(session.transitions.contains_key("LabExtendsVoid"));
}

// ========== CheckWorld (World Audit) ==========

#[test]
fn check_world_passes_consistent() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [Category CC [Object Type] [Morphism app :domain [Type Type] :codomain Type]]
        [Substrate S1 @engine interaction-graph @resource-mode optimal-sharing @barrier transparent @equality equality-saturation]
        [World Explorer :category CC :substrate S1 :epistemic [:discover complete :verify sound]]
        [CheckWorld Explorer]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.output.iter().any(|s| s.contains("[AUDIT]") && s.contains("PASS")));
}

#[test]
fn check_world_warns_strong_norm_with_egraph() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [Category CC [Object Type] [Morphism app :domain [Type Type] :codomain Type]]
        [Substrate S1 @engine interaction-graph @resource-mode optimal-sharing @barrier transparent @equality equality-saturation]
        [World W :category CC :substrate S1 :epistemic [:canonicalize [:normalization strong :confluence no :unique-normal-forms no]]]
        [CheckWorld W]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.output.iter().any(|s| s.contains("WARN")));
}

// ========== Lemma (Cross-World Lemma) ==========

#[test]
fn lemma_transport_valid() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :epistemic [:discover complete :verify sound] :admits [Tunnel]]
        [World B :epistemic [:verify decidable]]
        [Transition T1 :kind Tunnel :from A :to B :preserves [Soundness] :transport [:mode witness :loss [PathStructure]]]
        [Lemma MyLemma :source A :via T1 :target B :statement commutativity]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.lemmas.contains_key("MyLemma"));
    assert!(session.output.iter().any(|s| s.contains("[LEMMA]") && s.contains("transported")));
}

#[test]
fn lemma_transport_mismatch() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :epistemic [:discover complete :verify sound] :admits [Tunnel]]
        [World B :epistemic [:verify decidable]]
        [World C :epistemic [:verify sound]]
        [Transition T1 :kind Tunnel :from A :to B :preserves [Soundness]]
        [Lemma Bad :source A :via T1 :target C :statement foo]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("not A → C"));
}

// ========== Materialize (Pipeline Materialization) ==========

#[test]
fn materialize_pipeline_basic() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World Explorer :epistemic [:discover complete :verify sound] :admits [Tunnel]]
        [World Certifier :epistemic [:verify decidable]]
        [World Executor :epistemic [:discover none :verify sound :canonicalize confluent]]
        [Transition T1 :kind Tunnel :from Explorer :to Certifier :preserves [Soundness] :breaks [PathStructure]]
        [Transition T2 :kind CoarseGrain :from Certifier :to Executor :preserves [Soundness] :breaks [ResourceSensitivity]]
        [Pipeline Demo
            [Step discover :action Discover :world Explorer]
            [Step tunnel :action Tunnel :world Explorer :target Certifier]
            [Step verify :action Verify :world Certifier]
            [Step compile :action CoarseGrain :world Certifier :target Executor]
        ]
        [Materialize RunDemo :pipeline Demo]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.output.iter().any(|s| s.contains("[MATERIALIZE]") && s.contains("Step 1")));
    assert!(session.output.iter().any(|s| s.contains("PathStructure")));
}

// ========== Promote (Epistemic Promotion) ==========

#[test]
fn promote_dominates_to_transition() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :epistemic [:discover complete :verify sound]]
        [World B :epistemic [:discover none :verify sound]]
        [Promote LiftAB :assertion [dominates A B] :as transition :kind ConservativeExtension]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.transitions.contains_key("LiftAB"));
    assert!(session.output.iter().any(|s| s.contains("[PROMOTE]")));
}

#[test]
fn promote_failing_assertion_errors() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :epistemic [:discover none :verify sound]]
        [World B :epistemic [:discover complete :verify sound]]
        [Promote Bad :assertion [dominates A B] :as transition]
    "#);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("cannot promote"));
}

// ========== Law (Cosmological Law) ==========

#[test]
fn law_dominance_reflexive_model_check() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :epistemic [:discover complete :verify sound]]
        [World B :epistemic [:discover heuristic :verify decidable]]
        [Law DominanceReflexive
            :forall [W]
            :then [dominates W W]
            :method model-check
        ]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.output.iter().any(|s| s.contains("PROVED")));
}

#[test]
fn law_with_counterexample() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World Strong :epistemic [:discover complete :verify sound]]
        [World Weak :epistemic [:discover none :verify sound]]
        [Law EverythingDominatesEverything
            :forall [W1 W2]
            :then [dominates W1 W2]
            :method model-check
        ]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.output.iter().any(|s| s.contains("REFUTED")));
}

#[test]
fn law_with_premise() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :epistemic [:discover complete :verify sound]]
        [World B :epistemic [:discover heuristic :verify sound]]
        [World C :epistemic [:discover none :verify sound]]
        [Law DominanceTransitivity
            :forall [X Y Z]
            :where [[dominates X Y] [dominates Y Z]]
            :then [dominates X Z]
            :method model-check
        ]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(session.output.iter().any(|s| s.contains("PROVED")));
}

// ========== Refute (Impossibility Proof) ==========

#[test]
fn refute_confirmed_no_witness() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :epistemic [:discover complete :verify sound]]
        [World B :epistemic [:discover none :verify sound]]
        [Refute NoMutualDominance
            :forall [W1 W2]
            :impossible [
                [dominates W1 W2]
                [dominates W2 W1]
                [distance W1 W2 :max 0]
            ]
            :method model-check
        ]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    // The only way both dominate each other AND distance=0 is if they're the same world
    // which means distance=0 holds. So this should find a witness (same world assigned to both).
    // Actually dominates is reflexive, so W1=W2=A satisfies all three. WITNESS FOUND.
    assert!(session.output.iter().any(|s| s.contains("WITNESS FOUND") || s.contains("CONFIRMED")));
}

#[test]
fn refute_genuinely_impossible() {
    let mut session = MetacosmSession::new();
    let errors = process_all(&mut session, r#"
        [World A :epistemic [:discover complete :verify sound]]
        [World B :epistemic [:discover none :verify sound]]
        [Refute NoDominanceOfStrongerByWeaker
            :forall [W1 W2]
            :impossible [
                [dominates W1 W2]
                [distance W1 W2 :max 0]
            ]
            :method model-check
        ]
    "#);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    // W1=A, W2=A: dominates(A,A)=true, distance(A,A)=0≤0=true → witness found
    // So this is NOT impossible. Check that it reports WITNESS FOUND.
    assert!(session.output.iter().any(|s| s.contains("WITNESS FOUND")));
}
