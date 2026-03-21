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
    assert_eq!(session.transitions.len(), 3); // 2 declared + 1 composed
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
