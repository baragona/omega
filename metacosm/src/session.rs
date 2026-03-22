use std::collections::HashMap;

use apeiron::parser::Sexp;
use serde::Serialize;

use crate::embedding::{self, EmbeddingDef};
use crate::epistemic::{self, Measurement, MeasureValue, Observable};
use crate::error::{MetacosmError, Result};
use crate::knowledge::{self, SemanticProperty};
use crate::morphism::WorldMorphism;
use crate::pipeline::{self, PipelineAction, PipelineDef};
use crate::transition::{self, TransitionDef};
use crate::world::{self, FamilyDef, WorldDef};

/// Structured result entry for JSON output.
#[derive(Debug, Clone, Serialize)]
pub struct ResultEntry {
    pub name: String,
    pub status: String,
    pub message: Option<String>,
}

/// The Metacosm session: wraps a Hyperion session (which wraps Apeiron).
///
/// Three modes of operation:
/// - Omega mode: blocks go straight through Hyperion → Apeiron (single world, no cosmology)
/// - Hyperion mode: Category/Substrate/Universe/Functor blocks handled by Hyperion
/// - Cosmology mode: World/Transition/Observable/Family/Pipeline blocks handled here
pub struct MetacosmSession {
    pub hyperion: hyperion::session::HyperionSession,
    pub worlds: HashMap<String, WorldDef>,
    pub transitions: HashMap<String, TransitionDef>,
    pub observables: HashMap<String, Observable>,
    pub families: HashMap<String, FamilyDef>,
    pub pipelines: HashMap<String, PipelineDef>,
    pub measurements: Vec<Measurement>,
    pub morphisms: HashMap<String, WorldMorphism>,
    pub embeddings: HashMap<String, EmbeddingDef>,
    pub lemmas: HashMap<String, crate::lemma::CrossWorldLemma>,
    pub laws: HashMap<String, crate::law::CosmologicalLaw>,
    pub impossibilities: HashMap<String, crate::refute::ImpossibilityProof>,
    pub semantic_properties: Vec<SemanticProperty>,
    pub output: Vec<String>,
    pub structured_output: Vec<ResultEntry>,
}

impl MetacosmSession {
    pub fn new() -> Self {
        let mut session = MetacosmSession {
            hyperion: hyperion::session::HyperionSession::new(),
            worlds: HashMap::new(),
            transitions: HashMap::new(),
            observables: HashMap::new(),
            families: HashMap::new(),
            pipelines: HashMap::new(),
            measurements: Vec::new(),
            morphisms: HashMap::new(),
            embeddings: HashMap::new(),
            lemmas: HashMap::new(),
            laws: HashMap::new(),
            impossibilities: HashMap::new(),
            semantic_properties: Vec::new(),
            output: Vec::new(),
            structured_output: Vec::new(),
        };
        session.register_builtin_embeddings();
        session
    }

    pub fn with_prelude() -> Result<Self> {
        let mut session = Self::new();
        match hyperion::session::HyperionSession::with_prelude() {
            Ok(hyp) => session.hyperion = hyp,
            Err(e) => {
                session.output.push(format!("Warning: prelude loading failed: {}", e));
            }
        }
        Ok(session)
    }

    fn record_result(&mut self, name: &str, status: &str, message: Option<String>) {
        self.structured_output.push(ResultEntry {
            name: name.to_string(),
            status: status.to_string(),
            message,
        });
    }

    /// Process a single top-level S-expression.
    ///
    /// Routing logic:
    /// - World, Transition, Observable, Family, Pipeline → Metacosm (cosmology mode)
    /// - Category, Substrate, Universe, Functor, NatTrans, Adjunction → Hyperion (hyperion mode)
    /// - Theory, Proofs → Hyperion → Apeiron (omega mode pass-through)
    pub fn process(&mut self, sexp: &Sexp) -> Result<()> {
        let items = sexp.as_list().ok_or_else(|| MetacosmError::ParseError {
            block: "top-level".into(),
            detail: "expected top-level list".into(),
        })?;

        if items.is_empty() {
            return Ok(());
        }

        let head = items[0].as_atom().unwrap_or("");
        match head {
            // --- Metacosm native blocks (Layer 3: cosmology) ---
            "World" => self.process_world(items),
            "Transition" => self.process_transition(items),
            "Observable" => self.process_observable(items),
            "Family" => self.process_family(items),
            "Pipeline" => self.process_pipeline(items),
            "Measure" => self.process_measure(items),
            "Compose" => self.process_compose(items),
            "Embedding" => self.process_embedding(items),
            "Assert" => self.process_assertion(items),
            "CheckWorld" => self.process_check_world(items),
            "Lemma" => self.process_lemma(items),
            "Materialize" => self.process_materialize(items),
            "Promote" => self.process_promote(items),
            "Law" => self.process_law(items),
            "Refute" | "Impossibility" => self.process_refute(items),
            "Emit" => self.process_emit(items),

            // --- Hyperion pass-through (Layer 2: category + substrate) ---
            "Category" | "Substrate" | "Universe" | "Functor"
            | "NatTrans" | "Adjunction" | "VerifyFunctor" => {
                self.hyperion.process(sexp).map_err(MetacosmError::from)?;
                self.drain_hyperion_output();
                Ok(())
            }

            // --- Omega pass-through (Layer 1: theories + proofs) ---
            "Theory" | "Proofs" => {
                self.hyperion.process(sexp).map_err(MetacosmError::from)?;
                self.drain_hyperion_output();
                Ok(())
            }

            _ => Err(MetacosmError::UnknownBlock {
                name: head.to_string(),
            }),
        }
    }

    fn drain_hyperion_output(&mut self) {
        let new_output: Vec<String> = self.hyperion.output.drain(..).collect();
        self.output.extend(new_output);
    }

    fn process_world(&mut self, items: &[Sexp]) -> Result<()> {
        let w = world::parse_world(items)?;
        let name = w.name.clone();

        if self.worlds.contains_key(&name) {
            return Err(MetacosmError::DuplicateName {
                kind: "World".into(),
                name,
            });
        }

        // If this world has explicit category + substrate, register as Hyperion Universe
        if w.category != "Implicit" && w.substrate != "Default" {
            let hyp_sexp = format!(
                "[Universe {} :category {} :substrate {}]",
                name, w.category, w.substrate
            );
            if let Ok(sexps) = apeiron::parser::parse(&hyp_sexp) {
                if let Err(e) = self.hyperion.process(&sexps[0]) {
                    self.output.push(format!(
                        "[WORLD] Warning: Hyperion universe registration for '{}' failed: {}",
                        name, e
                    ));
                }
                self.drain_hyperion_output();
            }
        }

        let mode = if w.is_omega_mode() {
            "omega-mode"
        } else if w.is_hyperion_mode() {
            "hyperion-mode"
        } else {
            "cosmology-mode"
        };

        let msg = format!(
            "[WORLD] {} registered (category={}, substrate={}, mode={}, discover={}, verify={}, canonicalize={}, compress={})",
            name, w.category, w.substrate, mode,
            w.epistemic.discover, w.epistemic.verify,
            w.epistemic.canonicalize, w.epistemic.compress,
        );
        self.output.push(msg.clone());
        self.record_result(&name, "valid", Some(msg));
        self.worlds.insert(name.clone(), w);

        // Derive epistemic properties from substrate/category structure
        let derive_msgs = self.derive_epistemic_properties(&name);
        for m in derive_msgs {
            self.output.push(m);
        }

        // Register identity morphism
        let structures = self.category_structure_names(&self.worlds[&name].category);
        let id_morph = WorldMorphism::identity(&name, structures);
        self.morphisms.insert(format!("id_{}", name), id_morph);

        Ok(())
    }

    fn process_transition(&mut self, items: &[Sexp]) -> Result<()> {
        let t = transition::parse_transition(items)?;
        let name = t.name.clone();

        if self.transitions.contains_key(&name) {
            return Err(MetacosmError::DuplicateName {
                kind: "Transition".into(),
                name,
            });
        }

        let source_ep = self.worlds.get(&t.source)
            .ok_or_else(|| MetacosmError::Undefined {
                kind: "World".into(),
                name: t.source.clone(),
            })?
            .epistemic.clone();

        let target_ep = self.worlds.get(&t.target)
            .ok_or_else(|| MetacosmError::Undefined {
                kind: "World".into(),
                name: t.target.clone(),
            })?
            .epistemic.clone();

        // Check admissibility
        let source_world = self.worlds.get(&t.source).unwrap();
        if !source_world.admissible_transitions.is_empty()
            && !source_world.admissible_transitions.contains(&t.kind)
        {
            return Err(MetacosmError::InvalidTransition {
                from: t.source.clone(),
                to: t.target.clone(),
                detail: format!(
                    "world '{}' does not admit {} transitions",
                    t.source, t.kind
                ),
            });
        }

        // Epistemic validation
        let warnings = transition::check_transition_epistemic(&t, &source_ep, &target_ep)?;
        for w in &warnings {
            self.output.push(format!("[TRANSITION] Warning: {}", w));
        }

        // Create world morphism
        let mut morph = if let Some(ref functor_name) = t.functor {
            WorldMorphism::with_functor(t.clone(), functor_name.clone())
        } else if t.source == t.target {
            // Same-world transition: identity functor
            let structures = self.category_structure_names(&self.worlds.get(&t.source).unwrap().category);
            let mut m = WorldMorphism::opaque(t.clone());
            m.functor = Some(crate::morphism::FunctorRef::Identity);
            m.properties = crate::morphism::MorphismProperties {
                preserves_structure: structures,
                ..crate::morphism::MorphismProperties::identity()
            };
            m
        } else {
            WorldMorphism::opaque(t.clone())
        };

        // Validate functor consistency if functor is named
        if matches!(&morph.functor, Some(crate::morphism::FunctorRef::Named(_))) {
            let warnings = crate::morphism::validate_morphism(
                &mut morph,
                &self.worlds,
                &self.hyperion,
            )?;
            for w in &warnings {
                self.output.push(format!("[MORPHISM] Warning: {}", w));
            }
        }

        let morph_info = if morph.functor.is_some() {
            format!(", morphism={}", morph.properties)
        } else {
            String::new()
        };

        let msg = format!(
            "[TRANSITION] {} registered ({}: {} → {}, transport={}, preserves=[{}], breaks=[{}]{})",
            name, t.kind, t.source, t.target, t.transport.mode,
            t.preserves.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(", "),
            t.breaks.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(", "),
            morph_info,
        );
        self.output.push(msg.clone());
        self.record_result(&name, "valid", Some(msg));
        self.morphisms.insert(name.clone(), morph);
        self.transitions.insert(name, t);
        Ok(())
    }

    fn process_observable(&mut self, items: &[Sexp]) -> Result<()> {
        let obs = epistemic::parse_observable(items)?;
        let name = obs.name.clone();

        if self.observables.contains_key(&name) {
            return Err(MetacosmError::DuplicateName {
                kind: "Observable".into(),
                name,
            });
        }

        let msg = format!("[OBSERVABLE] {} registered (kind={:?})", name, obs.kind);
        self.output.push(msg.clone());
        self.record_result(&name, "valid", Some(msg));
        self.observables.insert(name, obs);
        Ok(())
    }

    fn process_family(&mut self, items: &[Sexp]) -> Result<()> {
        let fam = world::parse_family(items)?;
        let name = fam.name.clone();

        if self.families.contains_key(&name) {
            return Err(MetacosmError::DuplicateName {
                kind: "Family".into(),
                name,
            });
        }

        for w in &fam.worlds {
            if !self.worlds.contains_key(w) {
                return Err(MetacosmError::Undefined {
                    kind: "World".into(),
                    name: w.clone(),
                });
            }
        }

        let msg = format!(
            "[FAMILY] {} registered ({} worlds, {} invariants)",
            name, fam.worlds.len(), fam.invariants.len()
        );
        self.output.push(msg.clone());
        self.record_result(&name, "valid", Some(msg));
        self.families.insert(name, fam);
        Ok(())
    }

    fn process_pipeline(&mut self, items: &[Sexp]) -> Result<()> {
        let pipe = pipeline::parse_pipeline(items)?;
        let name = pipe.name.clone();

        if self.pipelines.contains_key(&name) {
            return Err(MetacosmError::DuplicateName {
                kind: "Pipeline".into(),
                name,
            });
        }

        // Validate world references
        for step in &pipe.steps {
            if !self.worlds.contains_key(&step.world) {
                return Err(MetacosmError::Undefined {
                    kind: "World".into(),
                    name: step.world.clone(),
                });
            }
            if let Some(ref target) = step.target {
                if !self.worlds.contains_key(target) {
                    return Err(MetacosmError::Undefined {
                        kind: "World".into(),
                        name: target.clone(),
                    });
                }
            }
        }

        // Validate epistemic feasibility
        for step in &pipe.steps {
            let world = self.worlds.get(&step.world).unwrap();
            // Use class-specific epistemic profile if :class is specified
            let ep = if let Some(ref class_name) = step.class {
                let tc = crate::theorem_class::parse_theorem_class(class_name)
                    .map_err(|_| MetacosmError::PipelineError {
                        pipeline: name.clone(),
                        step: step.name.clone(),
                        detail: format!("unknown theorem class: '{}'", class_name),
                    })?;
                world.epistemic.for_class(&tc)
            } else {
                world.epistemic.clone()
            };
            match step.action {
                PipelineAction::Discover => {
                    if !ep.can_discover() {
                        let class_msg = step.class.as_ref()
                            .map(|c| format!(" for class {}", c))
                            .unwrap_or_default();
                        return Err(MetacosmError::PipelineError {
                            pipeline: name.clone(),
                            step: step.name.clone(),
                            detail: format!(
                                "world '{}' has discover=none{}, cannot discover",
                                step.world, class_msg
                            ),
                        });
                    }
                }
                PipelineAction::Verify => {
                    if !ep.can_verify() {
                        return Err(MetacosmError::PipelineError {
                            pipeline: name.clone(),
                            step: step.name.clone(),
                            detail: format!(
                                "world '{}' has verify=none, cannot verify",
                                step.world
                            ),
                        });
                    }
                }
                PipelineAction::Tunnel => {
                    // Tunnel target must be able to verify
                    if let Some(ref target) = step.target {
                        let target_world = self.worlds.get(target).unwrap();
                        if !target_world.epistemic.can_verify() {
                            return Err(MetacosmError::PipelineError {
                                pipeline: name.clone(),
                                step: step.name.clone(),
                                detail: format!(
                                    "tunnel target '{}' has verify=none",
                                    target
                                ),
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        let msg = format!(
            "[PIPELINE] {} registered ({} steps: {})",
            name,
            pipe.steps.len(),
            pipe.steps
                .iter()
                .map(|s| format!("{}({})", s.action, s.world))
                .collect::<Vec<_>>()
                .join(" → ")
        );
        self.output.push(msg.clone());
        self.record_result(&name, "valid", Some(msg));
        self.pipelines.insert(name, pipe);
        Ok(())
    }

    fn process_measure(&mut self, items: &[Sexp]) -> Result<()> {
        let mut obs_name: Option<String> = None;
        let mut world_name: Option<String> = None;
        let mut target_name: Option<String> = None;
        let mut class_name: Option<String> = None;
        let mut explicit_value: Option<String> = None;

        let mut i = 1;
        while i < items.len() {
            let key = items[i].as_atom().unwrap_or("");
            match key {
                ":observable" => {
                    i += 1;
                    obs_name = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
                }
                ":world" => {
                    i += 1;
                    world_name = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
                }
                ":target" => {
                    i += 1;
                    target_name = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
                }
                ":class" => {
                    i += 1;
                    class_name = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
                }
                ":value" => {
                    i += 1;
                    explicit_value = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
                }
                _ => {
                    return Err(MetacosmError::ParseError {
                        block: "Measure".into(),
                        detail: format!("unknown keyword: {}", key),
                    });
                }
            }
            i += 1;
        }

        let obs_name = obs_name.ok_or_else(|| MetacosmError::ParseError {
            block: "Measure".into(),
            detail: "missing :observable".into(),
        })?;
        let world_name = world_name.ok_or_else(|| MetacosmError::ParseError {
            block: "Measure".into(),
            detail: "missing :world".into(),
        })?;

        let obs = self.observables.get(&obs_name)
            .ok_or_else(|| MetacosmError::Undefined {
                kind: "Observable".into(),
                name: obs_name.clone(),
            })?;
        let world = self.worlds.get(&world_name)
            .ok_or_else(|| MetacosmError::Undefined {
                kind: "World".into(),
                name: world_name.clone(),
            })?;

        // Semantic observables cannot have explicit :value overrides
        if obs.species == knowledge::KnowledgeSpecies::Semantic && explicit_value.is_some() {
            return Err(MetacosmError::ParseError {
                block: "Measure".into(),
                detail: format!(
                    "semantic observable '{}' derives its value from the epistemic profile — \
                     cannot override with explicit :value",
                    obs_name
                ),
            });
        }

        // For empirical observables, require explicit :value
        if obs.species == knowledge::KnowledgeSpecies::Empirical {
            let val = explicit_value.ok_or_else(|| MetacosmError::ParseError {
                block: "Measure".into(),
                detail: format!("empirical observable '{}' requires :value", obs_name),
            })?;
            let measurement = Measurement {
                observable: obs_name.clone(),
                world: world_name.clone(),
                target_world: target_name.clone(),
                value: MeasureValue::Grade(val.clone()),
            };
            let msg = format!("[MEASURE] {}({}) = {} (empirical)", obs_name, world_name, val);
            self.output.push(msg.clone());
            self.record_result(&format!("{}:{}", obs_name, world_name), "valid", Some(msg));
            self.measurements.push(measurement);
            return Ok(());
        }

        // Get effective epistemic profile (possibly class-specific)
        let effective_ep = if let Some(ref cn) = class_name {
            let class = crate::theorem_class::parse_theorem_class(cn)?;
            world.epistemic.for_class(&class)
        } else {
            world.epistemic.clone()
        };

        let value = if let Some(v) = epistemic::measure_profile(&effective_ep, &obs.kind) {
            v
        } else if obs.kind == epistemic::ObservableKind::EpistemicDistance {
            if let Some(ref target) = target_name {
                let target_world = self.worlds.get(target)
                    .ok_or_else(|| MetacosmError::Undefined {
                        kind: "World".into(),
                        name: target.clone(),
                    })?;
                let target_ep = if let Some(ref cn) = class_name {
                    let class = crate::theorem_class::parse_theorem_class(cn)?;
                    target_world.epistemic.for_class(&class)
                } else {
                    target_world.epistemic.clone()
                };
                let dist = effective_ep.distance(&target_ep);
                MeasureValue::Distance(dist)
            } else {
                return Err(MetacosmError::ParseError {
                    block: "Measure".into(),
                    detail: "epistemic-distance requires :target".into(),
                });
            }
        } else {
            MeasureValue::Boolean(true)
        };

        let measurement = Measurement {
            observable: obs_name.clone(),
            world: world_name.clone(),
            target_world: target_name.clone(),
            value: value.clone(),
        };

        let class_suffix = class_name.as_ref().map(|c| format!(" [class={}]", c)).unwrap_or_default();
        let msg = if let Some(ref t) = target_name {
            format!("[MEASURE] {}({} → {}) = {}{}", obs_name, world_name, t, value, class_suffix)
        } else {
            format!("[MEASURE] {}({}) = {}{}", obs_name, world_name, value, class_suffix)
        };
        self.output.push(msg.clone());
        self.record_result(&format!("{}:{}", obs_name, world_name), "valid", Some(msg));
        self.measurements.push(measurement);
        Ok(())
    }

    fn process_compose(&mut self, items: &[Sexp]) -> Result<()> {
        let (name, transition_names) = transition::parse_compose(items)?;

        if self.transitions.contains_key(&name) {
            return Err(MetacosmError::DuplicateName {
                kind: "Transition".into(),
                name,
            });
        }

        // Look up all transitions
        for tn in &transition_names {
            if !self.transitions.contains_key(tn) {
                return Err(MetacosmError::Undefined {
                    kind: "Transition".into(),
                    name: tn.clone(),
                });
            }
        }

        // Compose pairwise left-to-right
        let first = self.transitions[&transition_names[0]].clone();
        let mut composed = first;
        for tn in &transition_names[1..] {
            let next = self.transitions[tn].clone();
            composed = transition::compose_transitions(&composed, &next, &name)?;
        }

        // Compute the algebraic signature
        let src_ep = &self.worlds.get(&composed.source)
            .map(|w| w.epistemic.clone())
            .unwrap_or_default();
        let tgt_ep = &self.worlds.get(&composed.target)
            .map(|w| w.epistemic.clone())
            .unwrap_or_default();
        let sig = transition::TransitionSignature::from_transition(&composed, src_ep, tgt_ep);

        // Compose morphisms too
        let first_morph = self.morphisms.get(&transition_names[0]).cloned()
            .unwrap_or_else(|| WorldMorphism::opaque(self.transitions[&transition_names[0]].clone()));
        let mut composed_morph = first_morph;
        for tn in &transition_names[1..] {
            let next_morph = self.morphisms.get(tn).cloned()
                .unwrap_or_else(|| WorldMorphism::opaque(self.transitions[tn].clone()));
            composed_morph = crate::morphism::compose_morphisms(&composed_morph, &next_morph, &name)?;
        }

        let morph_info = if composed_morph.functor.is_some() {
            format!(", morphism={}", composed_morph.properties)
        } else {
            String::new()
        };

        let msg = format!(
            "[COMPOSE] {} = {} → {} (composed from [{}], signature={}{})",
            name, composed.source, composed.target,
            transition_names.join(" ; "),
            sig, morph_info,
        );
        self.output.push(msg.clone());
        self.record_result(&name, "valid", Some(msg));
        self.morphisms.insert(name.clone(), composed_morph);
        self.transitions.insert(name, composed);
        Ok(())
    }

    fn process_embedding(&mut self, items: &[Sexp]) -> Result<()> {
        let emb = embedding::parse_embedding(items)?;
        let name = emb.name.clone();

        if self.embeddings.contains_key(&name) {
            return Err(MetacosmError::DuplicateName {
                kind: "Embedding".into(),
                name,
            });
        }

        // Check properties
        let mut warnings = Vec::new();
        match (&emb.source, &emb.target) {
            (embedding::EmbeddingEndpoint::Layer(src), embedding::EmbeddingEndpoint::Layer(tgt)) => {
                let results = embedding::check_layer_embedding(src, tgt, &emb.properties);
                for r in results {
                    match r {
                        Ok(msg) => {
                            self.output.push(format!("[EMBEDDING] {} ✓ {}", name, msg));
                        }
                        Err(e) => {
                            warnings.push(format!("{}", e));
                        }
                    }
                }
            }
            (embedding::EmbeddingEndpoint::World(src_w), embedding::EmbeddingEndpoint::World(tgt_w)) => {
                // World-to-world embedding: check dominance for conservative
                if emb.properties.contains(&embedding::EmbeddingProperty::Conservative) {
                    let src_ep = self.worlds.get(src_w)
                        .ok_or_else(|| MetacosmError::Undefined {
                            kind: "World".into(),
                            name: src_w.clone(),
                        })?
                        .epistemic.clone();
                    let tgt_ep = self.worlds.get(tgt_w)
                        .ok_or_else(|| MetacosmError::Undefined {
                            kind: "World".into(),
                            name: tgt_w.clone(),
                        })?
                        .epistemic.clone();
                    if tgt_ep.dominates(&src_ep) {
                        self.output.push(format!(
                            "[EMBEDDING] {} ✓ conservative: {} dominates {}",
                            name, tgt_w, src_w
                        ));
                    } else {
                        return Err(MetacosmError::EmbeddingViolation {
                            embedding: name,
                            property: "conservative".into(),
                            detail: format!("{} does not epistemically dominate {}", tgt_w, src_w),
                        });
                    }
                }
            }
            _ => {
                self.output.push(format!("[EMBEDDING] {} registered (mixed endpoints)", name));
            }
        }

        for w in &warnings {
            self.output.push(format!("[EMBEDDING] Warning: {}", w));
        }

        let msg = format!(
            "[EMBEDDING] {} registered ({} → {}, properties=[{}])",
            name, emb.source, emb.target,
            emb.properties.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", "),
        );
        self.output.push(msg.clone());
        self.record_result(&name, "valid", Some(msg));
        self.embeddings.insert(name, emb);
        Ok(())
    }

    fn process_assertion(&mut self, items: &[Sexp]) -> Result<()> {
        let assertion = crate::assertion::parse_assertion(items)?;
        let result = crate::assertion::check_assertion(
            &assertion,
            &self.worlds,
            &self.families,
            &self.transitions,
            &self.morphisms,
        );

        let status = if result.passed { "pass" } else { "fail" };
        let msg = format!("[ASSERT] {}", result);
        self.output.push(msg.clone());
        self.record_result(&result.assertion, status, Some(msg));

        if !result.passed {
            return Err(MetacosmError::AssertionFailed {
                assertion: result.assertion,
                detail: result.detail,
            });
        }

        Ok(())
    }

    fn process_check_world(&mut self, items: &[Sexp]) -> Result<()> {
        if items.len() < 2 {
            return Err(MetacosmError::ParseError {
                block: "CheckWorld".into(),
                detail: "missing world name".into(),
            });
        }
        let name = items[1].as_atom().ok_or_else(|| MetacosmError::ParseError {
            block: "CheckWorld".into(),
            detail: "world name must be an atom".into(),
        })?;

        let world = self.worlds.get(name).ok_or_else(|| MetacosmError::Undefined {
            kind: "World".into(),
            name: name.into(),
        })?.clone();

        let result = crate::audit::audit_world(&world, &self.hyperion);

        for issue in &result.issues {
            self.output.push(format!("[AUDIT] {}: {}", name, issue));
        }

        let status = if result.passed { "pass" } else { "fail" };
        let msg = format!("[AUDIT] {}", result);
        self.output.push(msg.clone());
        self.record_result(name, status, Some(msg));
        Ok(())
    }

    fn process_lemma(&mut self, items: &[Sexp]) -> Result<()> {
        let (name, source, transition_name, target, statement) =
            crate::lemma::parse_cross_world_lemma(items)?;

        let transition = self.transitions.get(&transition_name).ok_or_else(|| MetacosmError::Undefined {
            kind: "Transition".into(),
            name: transition_name.clone(),
        })?.clone();

        let result = crate::lemma::check_transport(&source, &target, &transition, &self.worlds)?;

        // Apply functor maps to the statement if the transition has a functor
        let (transported_statement, obj_maps, morph_maps) = crate::lemma::apply_functor_maps(
            &statement,
            &self.hyperion,
            transition.functor.as_deref(),
        );

        let provenance = crate::lemma::LemmaProvenance {
            origin_world: source.clone(),
            via_transition: transition_name.clone(),
            functor: transition.functor.clone(),
            object_maps: obj_maps,
            morphism_maps: morph_maps,
        };

        for w in &result.warnings {
            self.output.push(format!("[LEMMA] Warning: {}: {}", name, w));
        }

        let transport_info = if transported_statement != statement {
            format!(" (renamed: {} → {})", statement, transported_statement)
        } else {
            String::new()
        };

        let msg = format!("[LEMMA] {} ({} in {} → {} via {}): {}{}",
            name, statement, source, target, transition_name, result, transport_info
        );
        self.output.push(msg.clone());
        self.record_result(&name, if result.valid { "valid" } else { "invalid" }, Some(msg));

        self.lemmas.insert(name.clone(), crate::lemma::CrossWorldLemma {
            name,
            source,
            transition: transition_name,
            target,
            statement,
            transported_statement,
            provenance,
            result,
        });
        Ok(())
    }

    fn process_materialize(&mut self, items: &[Sexp]) -> Result<()> {
        if items.len() < 2 {
            return Err(MetacosmError::ParseError {
                block: "Materialize".into(),
                detail: "missing materialization name".into(),
            });
        }
        let name = items[1].as_atom().ok_or_else(|| MetacosmError::ParseError {
            block: "Materialize".into(),
            detail: "name must be an atom".into(),
        })?.to_string();

        let mut pipeline_name = None;
        let mut i = 2;
        while i < items.len() {
            let key = items[i].as_atom().unwrap_or("");
            match key {
                ":pipeline" => {
                    i += 1;
                    pipeline_name = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
                }
                _ => {
                    return Err(MetacosmError::ParseError {
                        block: "Materialize".into(),
                        detail: format!("unknown keyword: {}", key),
                    });
                }
            }
            i += 1;
        }

        let pipeline_name = pipeline_name.ok_or_else(|| MetacosmError::ParseError {
            block: "Materialize".into(),
            detail: "missing :pipeline".into(),
        })?;

        let pipeline = self.pipelines.get(&pipeline_name).ok_or_else(|| MetacosmError::Undefined {
            kind: "Pipeline".into(),
            name: pipeline_name.clone(),
        })?.clone();

        let result = crate::materialize::materialize_pipeline(&pipeline, &self.worlds, &self.transitions)?;

        for (i, step) in result.steps.iter().enumerate() {
            self.output.push(format!("[MATERIALIZE] Step {}: {}", i + 1, step));
        }

        let msg = format!("[MATERIALIZE] {}: {}", name, result);
        self.output.push(msg.clone());
        self.record_result(&name, "valid", Some(msg));
        Ok(())
    }

    fn process_promote(&mut self, items: &[Sexp]) -> Result<()> {
        let promotion = crate::promote::parse_promotion(items)?;

        // Check the assertion first — must pass
        let assertion_result = crate::assertion::check_assertion(
            &promotion.assertion,
            &self.worlds,
            &self.families,
            &self.transitions,
            &self.morphisms,
        );

        if !assertion_result.passed {
            return Err(MetacosmError::AssertionFailed {
                assertion: assertion_result.assertion,
                detail: format!("cannot promote: {}", assertion_result.detail),
            });
        }

        match &promotion.target {
            crate::promote::PromotionTarget::Transition { kind } => {
                // Derive a transition from the assertion
                if let crate::assertion::Assertion::Dominates { stronger, weaker, .. } = &promotion.assertion {
                    let t = crate::transition::TransitionDef {
                        name: promotion.name.clone(),
                        kind: kind.clone(),
                        source: weaker.clone(),
                        target: stronger.clone(),
                        preserves: vec![
                            crate::transition::Invariant::Soundness,
                        ],
                        breaks: vec![],
                        transport: crate::transition::TransportEpistemics {
                            mode: crate::transition::TransportMode::Conservative,
                            loss: vec![],
                        },
                        functor: None,
                    };
                    let morph = crate::morphism::WorldMorphism::opaque(t.clone());

                    let msg = format!(
                        "[PROMOTE] {} → transition {} ({}: {} → {}, licensed by {})",
                        promotion.name, promotion.name, kind, weaker, stronger, assertion_result.assertion
                    );
                    self.output.push(msg.clone());
                    self.record_result(&promotion.name, "promoted", Some(msg));
                    self.morphisms.insert(promotion.name.clone(), morph);
                    self.transitions.insert(promotion.name.clone(), t);
                } else {
                    return Err(MetacosmError::ParseError {
                        block: "Promote".into(),
                        detail: "only dominates assertions can be promoted to transitions".into(),
                    });
                }
            }
            crate::promote::PromotionTarget::Constraint => {
                // Inject the assertion as an inference constraint
                let constraints = crate::inference::constraints_from_assertion(
                    &promotion.assertion,
                    &promotion.name,
                );

                let mut applied_count = 0;
                for c in &constraints {
                    match crate::inference::try_apply_constraint_pub(c, &mut self.worlds) {
                        crate::inference::ApplyResultPub::Applied => {
                            self.output.push(format!("[PROMOTE] constraint applied: {}", c));
                            applied_count += 1;
                        }
                        crate::inference::ApplyResultPub::AlreadySatisfied => {}
                        crate::inference::ApplyResultPub::Conflict => {
                            self.output.push(format!("[PROMOTE] constraint conflict: {}", c));
                        }
                    }
                }

                let msg = format!(
                    "[PROMOTE] {} → constraint ({} applied, licensed by {})",
                    promotion.name, applied_count, assertion_result.assertion
                );
                self.output.push(msg.clone());
                self.record_result(&promotion.name, "promoted", Some(msg));
            }
        }

        Ok(())
    }

    fn process_law(&mut self, items: &[Sexp]) -> Result<()> {
        let law = crate::law::parse_law(items)?;
        let name = law.name.clone();

        let result = crate::law::check_law(
            &law,
            &self.worlds,
            &self.families,
            &self.transitions,
            &self.morphisms,
        );

        let (status, msg) = match &result {
            crate::metatheory::ProofResult::Proved(cert) => {
                ("proved", format!("[LAW] {} — PROVED ({}): {:?}", name, law.method, cert.witness))
            }
            crate::metatheory::ProofResult::Refuted(cx) => {
                ("refuted", format!("[LAW] {} — REFUTED ({}): {}", name, law.method, cx.detail))
            }
        };

        self.output.push(msg.clone());
        self.record_result(&name, status, Some(msg));
        self.laws.insert(name, law);
        Ok(())
    }

    fn process_refute(&mut self, items: &[Sexp]) -> Result<()> {
        let proof = crate::refute::parse_refutation(items)?;
        let name = proof.name.clone();

        let result = crate::refute::check_impossibility(
            &proof,
            &self.worlds,
            &self.families,
            &self.transitions,
            &self.morphisms,
        );

        let (status, msg) = if result.confirmed {
            ("confirmed", format!("[REFUTE] {} — CONFIRMED ({}): {}", name, proof.method, result.proof))
        } else {
            ("refuted", format!("[REFUTE] {} — WITNESS FOUND ({}): {}", name, proof.method, result.proof))
        };

        self.output.push(msg.clone());
        self.record_result(&name, status, Some(msg));
        self.impossibilities.insert(name, proof);
        Ok(())
    }

    fn process_emit(&mut self, items: &[Sexp]) -> Result<()> {
        let decl = crate::emit::parse_emit(items)?;

        // Validate references
        let pipeline = self.pipelines.get(&decl.pipeline).ok_or_else(|| MetacosmError::Undefined {
            kind: "Pipeline".into(),
            name: decl.pipeline.clone(),
        })?.clone();

        // Check that the theory exists in Hyperion
        if !self.hyperion.theory_universes.contains_key(&decl.theory) {
            return Err(MetacosmError::Undefined {
                kind: "Theory".into(),
                name: decl.theory.clone(),
            });
        }

        let input_str = format!("{}", decl.term);
        let input_size = crate::emit::sexp_size(&decl.term);

        // Construct a synthetic [Proofs __EmitProof :in TheoryName [eval __emit EXPR]]
        use apeiron::parser::Span;
        let sp = Span::default();
        let proofs_sexp = Sexp::List(vec![
            Sexp::Atom("Proofs".into(), sp),
            Sexp::Atom("__EmitProof".into(), sp),
            Sexp::Atom(":in".into(), sp),
            Sexp::Atom(decl.theory.clone(), sp),
            Sexp::List(vec![
                Sexp::Atom("eval".into(), sp),
                Sexp::Atom(format!("__emit_{}", decl.name), sp),
                decl.term.clone(),
            ], sp),
        ], sp);

        // Process through Hyperion → Apeiron to normalize the term
        self.hyperion.process(&proofs_sexp).map_err(MetacosmError::from)?;

        // Capture the eval output from Hyperion
        let eval_prefix = format!("[EVAL] __emit_{}", decl.name);
        let mut normalized_str = String::new();
        let mut interactions: u64 = 0;

        // Drain Hyperion output, capturing the eval result
        let hyp_output: Vec<String> = self.hyperion.output.drain(..).collect();
        for line in &hyp_output {
            if line.starts_with(&eval_prefix) {
                // Parse: "[EVAL] __emit_Name = RESULT (N interactions)"
                if let Some(eq_pos) = line.find(" = ") {
                    let rest = &line[eq_pos + 3..];
                    if let Some(paren_pos) = rest.rfind(" (") {
                        normalized_str = rest[..paren_pos].to_string();
                        // Extract interaction count
                        let cost_str = &rest[paren_pos + 2..];
                        if let Some(space) = cost_str.find(' ') {
                            interactions = cost_str[..space].parse().unwrap_or(0);
                        }
                    } else {
                        normalized_str = rest.to_string();
                    }
                }
            } else {
                self.output.push(line.clone());
            }
        }

        // Also drain Apeiron output through Hyperion
        let ap_output: Vec<String> = self.hyperion.apeiron.output.drain(..).collect();
        for line in &ap_output {
            if line.starts_with(&eval_prefix) {
                if let Some(eq_pos) = line.find(" = ") {
                    let rest = &line[eq_pos + 3..];
                    if let Some(paren_pos) = rest.rfind(" (") {
                        normalized_str = rest[..paren_pos].to_string();
                        let cost_str = &rest[paren_pos + 2..];
                        if let Some(space) = cost_str.find(' ') {
                            interactions = cost_str[..space].parse().unwrap_or(0);
                        }
                    } else {
                        normalized_str = rest.to_string();
                    }
                }
            }
        }

        if normalized_str.is_empty() {
            normalized_str = format!("{}", decl.term);
        }

        let output_size = normalized_str.len();

        // Execute pipeline materialization (epistemic journey)
        let journey = crate::materialize::materialize_pipeline(
            &pipeline, &self.worlds, &self.transitions,
        )?;

        let result = crate::emit::EmitResult {
            name: decl.name.clone(),
            input: input_str,
            output: normalized_str.clone(),
            interactions,
            theory: decl.theory.clone(),
            journey,
            cost: crate::emit::EmitCost {
                interactions,
                term_size_input: input_size,
                term_size_output: output_size,
            },
        };

        // Format output based on requested format
        match decl.format {
            crate::emit::EmitFormat::EpistemicReceipt => {
                for line in format!("{}", result).lines() {
                    self.output.push(format!("[EMIT] {}", line));
                }
            }
            crate::emit::EmitFormat::Term => {
                self.output.push(format!("[EMIT] {} = {}", decl.name, normalized_str));
            }
        }

        self.record_result(&decl.name, "emitted", Some(format!("[EMIT] {} → {}", decl.name, normalized_str)));
        Ok(())
    }

    fn register_builtin_embeddings(&mut self) {
        use embedding::*;
        let builtins = vec![
            EmbeddingDef {
                name: "OmegaInHyperion".into(),
                source: EmbeddingEndpoint::Layer(LayerName::Omega),
                target: EmbeddingEndpoint::Layer(LayerName::Hyperion),
                properties: vec![
                    EmbeddingProperty::Conservative,
                    EmbeddingProperty::DefinableFragment,
                    EmbeddingProperty::StrictExtension,
                    EmbeddingProperty::NonPerturbing,
                ],
                checked: true,
            },
            EmbeddingDef {
                name: "HyperionInMetacosm".into(),
                source: EmbeddingEndpoint::Layer(LayerName::Hyperion),
                target: EmbeddingEndpoint::Layer(LayerName::Metacosm),
                properties: vec![
                    EmbeddingProperty::Conservative,
                    EmbeddingProperty::DefinableFragment,
                    EmbeddingProperty::StrictExtension,
                    EmbeddingProperty::NonPerturbing,
                ],
                checked: true,
            },
            EmbeddingDef {
                name: "OmegaInMetacosm".into(),
                source: EmbeddingEndpoint::Layer(LayerName::Omega),
                target: EmbeddingEndpoint::Layer(LayerName::Metacosm),
                properties: vec![
                    EmbeddingProperty::Conservative,
                    EmbeddingProperty::DefinableFragment,
                    EmbeddingProperty::StrictExtension,
                    EmbeddingProperty::NonPerturbing,
                ],
                checked: true,
            },
        ];
        for emb in builtins {
            self.embeddings.insert(emb.name.clone(), emb);
        }
    }

    /// Derive epistemic properties from substrate/category structure.
    /// Only fills in fields that are at their default values.
    fn derive_epistemic_properties(&mut self, world_name: &str) -> Vec<String> {
        use crate::knowledge::PropertyStatus;

        let world = match self.worlds.get(world_name) {
            Some(w) => w,
            None => return vec![],
        };

        if !world.derive_epistemics {
            return vec![];
        }

        let substrate_name = world.substrate.clone();
        let defaults = crate::epistemic::EpistemicProfile::trivial();
        let mut msgs = Vec::new();
        let mut derived_props = Vec::new();

        // Look up substrate properties from Hyperion
        if let Some(sub) = self.hyperion.substrates.get(&substrate_name) {
            let equality = format!("{:?}", sub.equality);
            let engine = format!("{:?}", sub.engine);

            let world = self.worlds.get_mut(world_name).unwrap();

            // equality-saturation → confluence
            if equality.contains("EqualitySaturation") && !world.epistemic.canonicalize.confluence {
                if world.epistemic.canonicalize == defaults.canonicalize {
                    world.epistemic.canonicalize.confluence = true;
                    let rule = "equality-saturation → confluence";
                    msgs.push(format!("[DERIVED] {}: confluence=true (from {})", world_name, rule));
                    derived_props.push((rule.to_string(), PropertyStatus::Derived { rule: rule.to_string() }));
                }
            }

            // interaction-graph engine → semi-decidable discovery
            if engine.contains("InteractionGraph") && world.epistemic.discover == defaults.discover {
                world.epistemic.discover = crate::epistemic::DiscoveryStrength::SemiDecidable;
                let rule = "interaction-graph → semi-decidable discovery";
                msgs.push(format!("[DERIVED] {}: discover=semi-decidable (from {})", world_name, rule));
                derived_props.push((rule.to_string(), PropertyStatus::Derived { rule: rule.to_string() }));
            }

            // abstract-machine engine → codegen compression
            if engine.contains("AbstractMachine") && world.epistemic.compress == defaults.compress {
                world.epistemic.compress = crate::epistemic::CompressionProfile {
                    mode: crate::epistemic::CompressionMode::Codegen,
                    lossy: true,
                    invertible: false,
                };
                let rule = "abstract-machine → codegen compression";
                msgs.push(format!("[DERIVED] {}: compress=codegen (from {})", world_name, rule));
                derived_props.push((rule.to_string(), PropertyStatus::Derived { rule: rule.to_string() }));
            }

            // term-tree + rewrite-equivalence → weak normalization
            if engine.contains("TermTree")
                && equality.contains("RewriteEquivalence")
                && world.epistemic.canonicalize.normalization == defaults.canonicalize.normalization
            {
                world.epistemic.canonicalize.normalization = crate::epistemic::NormalizationStrength::Weak;
                let rule = "term-tree+rewrite-equivalence → weak normalization";
                msgs.push(format!("[DERIVED] {}: normalization=weak (from {})", world_name, rule));
                derived_props.push((rule.to_string(), PropertyStatus::Derived { rule: rule.to_string() }));
            }

            world.derived_properties = derived_props;
        }

        // Record as semantic properties
        for (rule, status) in &self.worlds.get(world_name).unwrap().derived_properties {
            self.semantic_properties.push(SemanticProperty {
                name: rule.clone(),
                holder: world_name.to_string(),
                status: status.clone(),
            });
        }

        msgs
    }

    fn category_structure_names(&self, category_name: &str) -> Vec<String> {
        self.hyperion.categories.get(category_name)
            .map(|cat| cat.structure.iter().map(|s| {
                use hyperion::category::CategoricalStructure::*;
                match s {
                    Exponential { name, .. } => format!("Exponential({})", name),
                    Evaluator { name } => format!("Evaluator({})", name),
                    ModalOperator { name } => format!("Modal({})", name),
                    ContextDecl { name } => format!("Context({})", name),
                    TensorProduct { name } => format!("Tensor({})", name),
                    Unit { name } => format!("Unit({})", name),
                    Preorder { relation } => format!("Preorder({})", relation),
                    PathType { .. } => "PathType".to_string(),
                    JType { .. } => "JType".to_string(),
                    PartialElement { .. } => "PartialElement".to_string(),
                    IntervalSort { .. } => "IntervalSort".to_string(),
                }
            }).collect())
            .unwrap_or_default()
    }

    /// Run epistemic inference to fixpoint.
    ///
    /// Propagates constraints from transitions through the world graph.
    /// Updates world profiles when inferred values are stronger than defaults.
    /// Returns the inference result with propagated constraints and violations.
    pub fn run_inference(&mut self) -> crate::inference::InferenceResult {
        let result = crate::inference::infer_epistemics(
            &mut self.worlds,
            &self.transitions,
            &self.morphisms,
        );

        for c in &result.propagated {
            self.output.push(format!("[INFERRED] {}", c));
        }
        for c in &result.violations {
            self.output.push(format!("[INFERENCE CONFLICT] {}", c));
        }

        if !result.propagated.is_empty() || !result.violations.is_empty() {
            self.output.push(format!(
                "[INFERENCE] {} constraints propagated, {} conflicts, {} iterations",
                result.propagated.len(), result.violations.len(), result.iterations,
            ));
        }

        result
    }

    /// Run all metatheorems and return results.
    pub fn verify_metatheorems(&self) -> Vec<crate::metatheory::ProofResult> {
        crate::metatheory::verify_all(self)
    }

    pub fn json_output(&self, had_errors: bool, elapsed_ms: f64) -> serde_json::Value {
        serde_json::json!({
            "status": if had_errors { "failure" } else { "success" },
            "elapsed_ms": elapsed_ms,
            "results": self.structured_output,
            "measurements": self.measurements.iter().map(|m| {
                serde_json::json!({
                    "observable": m.observable,
                    "world": m.world,
                    "target_world": m.target_world,
                    "value": m.value.to_string(),
                })
            }).collect::<Vec<_>>(),
            "stats": {
                "worlds": self.worlds.len(),
                "transitions": self.transitions.len(),
                "observables": self.observables.len(),
                "families": self.families.len(),
                "pipelines": self.pipelines.len(),
                "measurements": self.measurements.len(),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apeiron::parser::parse;

    #[test]
    fn omega_passthrough() {
        let mut session = MetacosmSession::new();
        let input = "[Theory Dummy]";
        let sexps = parse(input).unwrap();
        let result = session.process(&sexps[0]);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(!err_msg.contains("unknown top-level block"));
    }

    #[test]
    fn hyperion_passthrough() {
        let mut session = MetacosmSession::new();
        let input = r#"[Category SimpleCategory
            [Object Type]
            [Morphism app :domain [Type Type] :codomain Type]
        ]"#;
        let sexps = parse(input).unwrap();
        let result = session.process(&sexps[0]);
        assert!(result.is_ok(), "error: {}", result.unwrap_err());
        assert!(session.output.iter().any(|s| s.contains("[CATEGORY]")));
    }

    #[test]
    fn world_registration() {
        let mut session = MetacosmSession::new();
        let input = r#"[World Explorer
            :category CartesianClosed
            :substrate ApeironStandard
            :epistemic [:discover complete :verify sound :canonicalize weak-nf :compress lossless]
            :admits [Split Tunnel]
        ]"#;
        let sexps = parse(input).unwrap();
        let result = session.process(&sexps[0]);
        assert!(result.is_ok());
        assert!(session.worlds.contains_key("Explorer"));
    }

    #[test]
    fn transition_validation() {
        let mut session = MetacosmSession::new();

        let w1 = "[World Explorer :category CartesianClosed :substrate ApeironStandard :epistemic [:discover complete :verify sound] :admits [Tunnel]]";
        let w2 = "[World Certifier :category CartesianClosed :substrate ApeironStandard :epistemic [:discover heuristic :verify decidable :canonicalize unique-nf]]";
        for input in [w1, w2] {
            let sexps = parse(input).unwrap();
            session.process(&sexps[0]).unwrap();
        }

        let t = "[Transition DiscoverAndCertify :kind Tunnel :from Explorer :to Certifier :preserves [Soundness] :transport [:mode witness :loss [PathStructure]]]";
        let sexps = parse(t).unwrap();
        let result = session.process(&sexps[0]);
        assert!(result.is_ok());
        assert!(session.transitions.contains_key("DiscoverAndCertify"));
    }

    #[test]
    fn tunnel_target_must_verify() {
        let mut session = MetacosmSession::new();

        let w1 = "[World A :category CartesianClosed :substrate ApeironStandard :epistemic [:discover complete]]";
        let w2 = "[World B :category CartesianClosed :substrate ApeironStandard :epistemic [:verify none]]";
        for input in [w1, w2] {
            let sexps = parse(input).unwrap();
            session.process(&sexps[0]).unwrap();
        }

        let t = "[Transition Bad :kind Tunnel :from A :to B]";
        let sexps = parse(t).unwrap();
        let result = session.process(&sexps[0]);
        assert!(result.is_err());
    }

    #[test]
    fn pipeline_validation() {
        let mut session = MetacosmSession::new();

        let w1 = "[World Explorer :category CartesianClosed :substrate ApeironStandard :epistemic [:discover complete :verify sound] :admits [Tunnel]]";
        let w2 = "[World Certifier :category CartesianClosed :substrate ApeironStandard :epistemic [:verify decidable :canonicalize unique-nf]]";
        for input in [w1, w2] {
            let sexps = parse(input).unwrap();
            session.process(&sexps[0]).unwrap();
        }

        let pipe = r#"[Pipeline Demo
            [Step search :action Discover :world Explorer]
            [Step transport :action Tunnel :world Explorer :target Certifier]
            [Step check :action Verify :world Certifier]
        ]"#;
        let sexps = parse(pipe).unwrap();
        let result = session.process(&sexps[0]);
        assert!(result.is_ok());
    }

    #[test]
    fn measurement() {
        let mut session = MetacosmSession::new();

        let w1 = "[World A :category CartesianClosed :substrate ApeironStandard :epistemic [:discover complete :verify heuristic]]";
        let w2 = "[World B :category CartesianClosed :substrate ApeironStandard :epistemic [:discover none :verify decidable]]";
        for input in [w1, w2] {
            let sexps = parse(input).unwrap();
            session.process(&sexps[0]).unwrap();
        }

        let obs = "[Observable Dist :kind epistemic-distance]";
        let sexps = parse(obs).unwrap();
        session.process(&sexps[0]).unwrap();

        let measure = "[Measure :observable Dist :world A :target B]";
        let sexps = parse(measure).unwrap();
        session.process(&sexps[0]).unwrap();

        assert_eq!(session.measurements.len(), 1);
    }
}
