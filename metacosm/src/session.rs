use std::collections::HashMap;

use apeiron::parser::Sexp;
use serde::Serialize;

use crate::epistemic::{self, Measurement, MeasureValue, Observable};
use crate::error::{MetacosmError, Result};
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
    /// The underlying Hyperion session (which contains Apeiron)
    pub hyperion: hyperion::session::HyperionSession,
    /// Metacosm worlds (superset of Hyperion universes)
    pub worlds: HashMap<String, WorldDef>,
    /// Transitions between worlds
    pub transitions: HashMap<String, TransitionDef>,
    /// Epistemic observables
    pub observables: HashMap<String, Observable>,
    /// Universe families
    pub families: HashMap<String, FamilyDef>,
    /// Pipelines
    pub pipelines: HashMap<String, PipelineDef>,
    /// Measurements taken
    pub measurements: Vec<Measurement>,
    /// Output messages
    pub output: Vec<String>,
    /// Structured output for JSON mode
    pub structured_output: Vec<ResultEntry>,
}

impl MetacosmSession {
    pub fn new() -> Self {
        MetacosmSession {
            hyperion: hyperion::session::HyperionSession::new(),
            worlds: HashMap::new(),
            transitions: HashMap::new(),
            observables: HashMap::new(),
            families: HashMap::new(),
            pipelines: HashMap::new(),
            measurements: Vec::new(),
            output: Vec::new(),
            structured_output: Vec::new(),
        }
    }

    /// Create a session with Hyperion prelude loaded.
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

            // --- Hyperion pass-through (Layer 2: category + substrate) ---
            // These go directly to Hyperion, which handles them natively.
            "Category" | "Substrate" | "Universe" | "Functor"
            | "NatTrans" | "Adjunction" | "VerifyFunctor" => {
                self.hyperion.process(sexp).map_err(MetacosmError::from)?;
                // Mirror Hyperion output
                self.drain_hyperion_output();
                Ok(())
            }

            // --- Omega pass-through (Layer 1: theories + proofs) ---
            // These go through Hyperion → Apeiron, preserving exact Omega semantics.
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

    /// Drain output from Hyperion into our output buffer.
    fn drain_hyperion_output(&mut self) {
        let new_output: Vec<String> = self.hyperion.output.drain(..).collect();
        self.output.extend(new_output);
    }

    // --- Metacosm native processors ---

    fn process_world(&mut self, items: &[Sexp]) -> Result<()> {
        let w = world::parse_world(items)?;
        let name = w.name.clone();

        if self.worlds.contains_key(&name) {
            return Err(MetacosmError::DuplicateName {
                kind: "World".into(),
                name,
            });
        }

        // If this world has explicit category + substrate, also register it as a
        // Hyperion Universe so theories can be checked in it.
        if w.category != "Implicit" && w.substrate != "Default" {
            let hyp_sexp = format!(
                "[Universe {} :category {} :substrate {}]",
                name, w.category, w.substrate
            );
            if let Ok(sexps) = apeiron::parser::parse(&hyp_sexp) {
                if let Err(e) = self.hyperion.process(&sexps[0]) {
                    // Only warn — world registration itself still succeeds
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
            "[WORLD] {} registered (category={}, substrate={}, mode={}, transitions={})",
            name,
            w.category,
            w.substrate,
            mode,
            w.admissible_transitions.len()
        );
        self.output.push(msg.clone());
        self.record_result(&name, "valid", Some(msg));
        self.worlds.insert(name, w);
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

        // Validate source and target worlds exist
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

        // Check that source world admits this transition kind
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

        let msg = format!(
            "[TRANSITION] {} registered ({}: {} → {}, preserves=[{}], breaks=[{}])",
            name,
            t.kind,
            t.source,
            t.target,
            t.preserves.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(", "),
            t.breaks.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(", "),
        );
        self.output.push(msg.clone());
        self.record_result(&name, "valid", Some(msg));
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

        // Validate all worlds exist
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
            name,
            fam.worlds.len(),
            fam.invariants.len()
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

        // Validate all referenced worlds exist
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

        // Validate epistemic feasibility of each step
        for step in &pipe.steps {
            let world = self.worlds.get(&step.world).unwrap();
            match step.action {
                PipelineAction::Discover => {
                    if !world.epistemic.can_discover() {
                        return Err(MetacosmError::PipelineError {
                            pipeline: name.clone(),
                            step: step.name.clone(),
                            detail: format!(
                                "world '{}' has discovery=none, cannot discover",
                                step.world
                            ),
                        });
                    }
                }
                PipelineAction::Verify => {
                    if !world.epistemic.can_verify() {
                        return Err(MetacosmError::PipelineError {
                            pipeline: name.clone(),
                            step: step.name.clone(),
                            detail: format!(
                                "world '{}' has verification=none, cannot verify",
                                step.world
                            ),
                        });
                    }
                }
                PipelineAction::Tunnel => {
                    if !world.epistemic.can_transport() {
                        return Err(MetacosmError::PipelineError {
                            pipeline: name.clone(),
                            step: step.name.clone(),
                            detail: format!(
                                "world '{}' has transportability=none, cannot tunnel",
                                step.world
                            ),
                        });
                    }
                    if let Some(ref target) = step.target {
                        let target_world = self.worlds.get(target).unwrap();
                        if !target_world.epistemic.can_verify() {
                            return Err(MetacosmError::PipelineError {
                                pipeline: name.clone(),
                                step: step.name.clone(),
                                detail: format!(
                                    "tunnel target '{}' has verification=none",
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
        // [Measure :observable O :world W [:target T]]
        let mut obs_name: Option<String> = None;
        let mut world_name: Option<String> = None;
        let mut target_name: Option<String> = None;

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

        // Validate references
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

        // Compute measurement from epistemic profile
        let value = match obs.kind {
            epistemic::ObservableKind::DiscoveryCost => {
                MeasureValue::Capacity(world.epistemic.discovery)
            }
            epistemic::ObservableKind::VerificationCost => {
                MeasureValue::Capacity(world.epistemic.verification)
            }
            epistemic::ObservableKind::TransportCost => {
                if let Some(ref target) = target_name {
                    let target_world = self.worlds.get(target)
                        .ok_or_else(|| MetacosmError::Undefined {
                            kind: "World".into(),
                            name: target.clone(),
                        })?;
                    let dist = world.epistemic.distance(&target_world.epistemic);
                    MeasureValue::Cost(dist as u64)
                } else {
                    MeasureValue::Capacity(world.epistemic.transportability)
                }
            }
            epistemic::ObservableKind::Canonicality => {
                MeasureValue::Capacity(world.epistemic.canonicality)
            }
            epistemic::ObservableKind::Compression => {
                MeasureValue::Capacity(world.epistemic.compression)
            }
            epistemic::ObservableKind::Custom => {
                MeasureValue::Boolean(true) // placeholder for custom
            }
        };

        let measurement = Measurement {
            observable: obs_name.clone(),
            world: world_name.clone(),
            target_world: target_name.clone(),
            value: value.clone(),
        };

        let msg = if let Some(ref t) = target_name {
            format!("[MEASURE] {}({} → {}) = {}", obs_name, world_name, t, value)
        } else {
            format!("[MEASURE] {}({}) = {}", obs_name, world_name, value)
        };
        self.output.push(msg.clone());
        self.record_result(&format!("{}:{}", obs_name, world_name), "valid", Some(msg));
        self.measurements.push(measurement);
        Ok(())
    }

    /// Generate JSON output.
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
        // A bare Theory + Proofs block should pass through to Hyperion → Apeiron
        let mut session = MetacosmSession::new();
        // This is an Omega-mode usage: just theories and proofs, no worlds at all
        // We don't test full Omega here (needs real theory), just that routing works
        let input = "[Theory Dummy]";
        let sexps = parse(input).unwrap();
        // This will fail in Hyperion (no :in clause) but that's expected —
        // the point is it routes to Hyperion, not to Metacosm
        let result = session.process(&sexps[0]);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        // Should be a Hyperion error, not a Metacosm "unknown block" error
        assert!(!err_msg.contains("unknown top-level block"));
    }

    #[test]
    fn hyperion_passthrough() {
        let mut session = MetacosmSession::new();
        // Category block should pass through to Hyperion
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
            :epistemic [:discovery high :verification high :canonicality low :transportability medium :compression low]
            :admits [Split Tunnel]
        ]"#;
        let sexps = parse(input).unwrap();
        let result = session.process(&sexps[0]);
        // May warn about Hyperion universe registration (prelude not loaded) but should succeed
        assert!(result.is_ok());
        assert!(session.worlds.contains_key("Explorer"));
    }

    #[test]
    fn transition_validation() {
        let mut session = MetacosmSession::new();

        // Register two worlds
        let w1 = "[World Explorer :category CartesianClosed :substrate ApeironStandard :epistemic [:discovery high :verification high :transportability high] :admits [Tunnel]]";
        let w2 = "[World Certifier :category CartesianClosed :substrate ApeironStandard :epistemic [:discovery low :verification high :canonicality high]]";
        for input in [w1, w2] {
            let sexps = parse(input).unwrap();
            session.process(&sexps[0]).unwrap();
        }

        // Register a tunnel transition
        let t = "[Transition DiscoverAndCertify :kind Tunnel :from Explorer :to Certifier :preserves [Soundness]]";
        let sexps = parse(t).unwrap();
        let result = session.process(&sexps[0]);
        assert!(result.is_ok());
        assert!(session.transitions.contains_key("DiscoverAndCertify"));
    }

    #[test]
    fn tunnel_target_must_verify() {
        let mut session = MetacosmSession::new();

        let w1 = "[World A :category CartesianClosed :substrate ApeironStandard :epistemic [:transportability high]]";
        let w2 = "[World B :category CartesianClosed :substrate ApeironStandard :epistemic [:verification none]]";
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

        let w1 = "[World Explorer :category CartesianClosed :substrate ApeironStandard :epistemic [:discovery high :transportability high] :admits [Tunnel]]";
        let w2 = "[World Certifier :category CartesianClosed :substrate ApeironStandard :epistemic [:verification high :canonicality high]]";
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

        let w1 = "[World A :category CartesianClosed :substrate ApeironStandard :epistemic [:discovery high :verification low]]";
        let w2 = "[World B :category CartesianClosed :substrate ApeironStandard :epistemic [:discovery low :verification high]]";
        for input in [w1, w2] {
            let sexps = parse(input).unwrap();
            session.process(&sexps[0]).unwrap();
        }

        let obs = "[Observable TransportDistance :kind transport-cost]";
        let sexps = parse(obs).unwrap();
        session.process(&sexps[0]).unwrap();

        let measure = "[Measure :observable TransportDistance :world A :target B]";
        let sexps = parse(measure).unwrap();
        session.process(&sexps[0]).unwrap();

        assert_eq!(session.measurements.len(), 1);
    }
}
