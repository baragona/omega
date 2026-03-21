//! World audit: validate that a world's declared epistemic profile
//! is achievable given its substrate and category.
//!
//! Syntax:
//!   [CheckWorld Explorer]

use crate::epistemic::{
    CompressionMode, DiscoveryStrength, NormalizationStrength,
};
use crate::world::WorldDef;

/// Result of auditing a world.
#[derive(Debug, Clone)]
pub struct AuditResult {
    pub world: String,
    pub passed: bool,
    pub issues: Vec<AuditIssue>,
}

/// A single audit finding.
#[derive(Debug, Clone)]
pub struct AuditIssue {
    pub severity: Severity,
    pub axis: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for AuditIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sev = match self.severity {
            Severity::Error => "ERROR",
            Severity::Warning => "WARN",
            Severity::Info => "INFO",
        };
        write!(f, "[{}] {}: {}", sev, self.axis, self.detail)
    }
}

impl std::fmt::Display for AuditResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.passed {
            write!(f, "AUDIT {}: PASS ({} findings)", self.world, self.issues.len())
        } else {
            write!(f, "AUDIT {}: FAIL ({} findings)", self.world, self.issues.len())
        }
    }
}

/// Audit a world against its Hyperion substrate and category.
///
/// Uses Hyperion's actual compatibility rules from compile.rs:
/// 1. VonNeumann + Exponential → reject
/// 2. VonNeumann + ModalOperator/Context → reject
/// 3. VonNeumann + TensorProduct → reject
/// 4. NominalScoping + Exponential → reject
/// 5. Exponential/Evaluator requires lambda-capable engine
/// 6. ModalOperator/Context requires scope isolation barrier
/// 7. TensorProduct requires parallel-composition engine
/// 8. StrictlyLinear + Exponential → reject
/// 9. PathType + Evaluator requires lambda-capable engine
/// 10. TopologicalHomotopy requires lambda-capable engine
pub fn audit_world(
    world: &WorldDef,
    hyperion: &hyperion::session::HyperionSession,
) -> AuditResult {
    let mut issues = Vec::new();

    // If no Hyperion substrate, can't audit substrate compatibility
    let substrate = match hyperion.substrates.get(&world.substrate) {
        Some(s) => s,
        None => {
            if world.substrate != "Default" {
                issues.push(AuditIssue {
                    severity: Severity::Warning,
                    axis: "substrate".into(),
                    detail: format!("substrate '{}' not found in Hyperion session", world.substrate),
                });
            }
            // Still check epistemic plausibility even without substrate
            check_epistemic_plausibility(world, &mut issues);
            return AuditResult {
                world: world.name.clone(),
                passed: !issues.iter().any(|i| i.severity == Severity::Error),
                issues,
            };
        }
    };

    // Check category exists if declared
    let category = if world.category != "Implicit" {
        match hyperion.categories.get(&world.category) {
            Some(cat) => Some(cat),
            None => {
                issues.push(AuditIssue {
                    severity: Severity::Warning,
                    axis: "category".into(),
                    detail: format!("category '{}' not found in Hyperion session", world.category),
                });
                None
            }
        }
    } else {
        None
    };

    // Apply Hyperion's 10 compatibility rules when we have both category and substrate
    if let Some(cat) = category {
        check_hyperion_compatibility(cat, substrate, &mut issues);
    }

    // Epistemic plausibility checks (substrate-aware)
    check_substrate_epistemic(world, substrate, &mut issues);

    // Check class-epistemic overrides don't exceed substrate capabilities
    check_class_epistemic_substrate(world, substrate, &mut issues);

    let has_errors = issues.iter().any(|i| i.severity == Severity::Error);

    AuditResult {
        world: world.name.clone(),
        passed: !has_errors,
        issues,
    }
}

/// Check Hyperion's actual 10 compatibility rules between category and substrate.
fn check_hyperion_compatibility(
    cat: &hyperion::category::CategoryDef,
    sub: &hyperion::substrate::SubstrateDef,
    issues: &mut Vec<AuditIssue>,
) {
    use hyperion::substrate::{BarrierMode, Engine, ResourceMode, EqualityMode};

    let is_vn = sub.engine == Engine::VonNeumann;
    let lambda_capable = matches!(
        sub.engine,
        Engine::InteractionGraph | Engine::TermTree | Engine::AbstractMachine | Engine::ConcurrentGraph
    );
    let supports_scopes = matches!(
        sub.barrier,
        BarrierMode::ContextualMembranes | BarrierMode::Cryptographic | BarrierMode::NominalScoping
    );
    let supports_tensor = matches!(
        sub.engine,
        Engine::InteractionGraph | Engine::SymmetricMonoidal | Engine::ReversibleGraph | Engine::ConcurrentGraph
    );

    // Rule 1-3: VonNeumann rejects higher-order features
    if is_vn {
        if cat.has_exponential() {
            issues.push(AuditIssue {
                severity: Severity::Error,
                axis: "category×substrate".into(),
                detail: "VonNeumann engine does not support Exponential (no lambda at hardware level)".into(),
            });
        }
        if cat.has_modal_operator() || cat.has_context() {
            issues.push(AuditIssue {
                severity: Severity::Error,
                axis: "category×substrate".into(),
                detail: "VonNeumann engine does not support ModalOperator/Context (no scope isolation)".into(),
            });
        }
        if cat.has_tensor() {
            issues.push(AuditIssue {
                severity: Severity::Error,
                axis: "category×substrate".into(),
                detail: "VonNeumann engine does not support TensorProduct (no parallel composition)".into(),
            });
        }
    }

    // Rule 4: NominalScoping + Exponential
    if matches!(sub.barrier, BarrierMode::NominalScoping) && cat.has_exponential() {
        issues.push(AuditIssue {
            severity: Severity::Error,
            axis: "category×substrate".into(),
            detail: "Nominal scoping does not support Exponential (nominal logic cannot do higher-order abstraction)".into(),
        });
    }

    // Rule 5: Exponential/Evaluator requires lambda-capable engine
    if (cat.has_exponential() || cat.has_evaluator()) && !lambda_capable {
        issues.push(AuditIssue {
            severity: Severity::Error,
            axis: "category×substrate".into(),
            detail: format!(
                "Exponential/Evaluator requires lambda-capable engine, but {:?} has no lambda abstraction",
                sub.engine
            ),
        });
    }

    // Rule 6: ModalOperator/Context requires scope isolation
    if (cat.has_modal_operator() || cat.has_context()) && !supports_scopes {
        issues.push(AuditIssue {
            severity: Severity::Error,
            axis: "category×substrate".into(),
            detail: format!(
                "ModalOperator/Context requires scope isolation, but {:?} barrier provides none",
                sub.barrier
            ),
        });
    }

    // Rule 7: TensorProduct requires parallel composition
    if cat.has_tensor() && !supports_tensor {
        issues.push(AuditIssue {
            severity: Severity::Error,
            axis: "category×substrate".into(),
            detail: format!(
                "TensorProduct requires parallel composition, but {:?} engine has none",
                sub.engine
            ),
        });
    }

    // Rule 8: StrictlyLinear + Exponential
    if sub.resource_mode == ResourceMode::StrictlyLinear && cat.has_exponential() {
        issues.push(AuditIssue {
            severity: Severity::Error,
            axis: "category×substrate".into(),
            detail: "StrictlyLinear resource mode cannot support Exponential (closures require duplication)".into(),
        });
    }

    // Rule 9: PathType + Evaluator requires lambda-capable engine
    if cat.has_path_type() && cat.has_evaluator() && !lambda_capable {
        issues.push(AuditIssue {
            severity: Severity::Error,
            axis: "category×substrate".into(),
            detail: format!(
                "PathType+Evaluator requires lambda-capable engine, but {:?} has none",
                sub.engine
            ),
        });
    }

    // Rule 10: TopologicalHomotopy requires lambda-capable engine
    if sub.equality == EqualityMode::TopologicalHomotopy && !lambda_capable {
        issues.push(AuditIssue {
            severity: Severity::Error,
            axis: "category×substrate".into(),
            detail: format!(
                "TopologicalHomotopy equality requires lambda-capable engine, but {:?} cannot represent path spaces",
                sub.engine
            ),
        });
    }
}

/// Check epistemic profile plausibility against substrate properties.
fn check_substrate_epistemic(
    world: &WorldDef,
    substrate: &hyperion::substrate::SubstrateDef,
    issues: &mut Vec<AuditIssue>,
) {
    let engine = &substrate.engine;
    let equality = &substrate.equality;

    // Discovery vs engine/equality
    match world.epistemic.discover {
        DiscoveryStrength::Complete => {
            if !matches!(equality, hyperion::substrate::EqualityMode::EqualitySaturation) {
                issues.push(AuditIssue {
                    severity: Severity::Error,
                    axis: "discover".into(),
                    detail: format!(
                        "complete discovery requires equality-saturation, but substrate uses {:?} — \
                         the substrate cannot enumerate the equivalence classes needed for exhaustive search",
                        equality
                    ),
                });
            }
        }
        DiscoveryStrength::SemiDecidable => {
            if matches!(engine, hyperion::substrate::Engine::VonNeumann) {
                issues.push(AuditIssue {
                    severity: Severity::Error,
                    axis: "discover".into(),
                    detail: "semi-decidable discovery on von-neumann engine is impossible — \
                             VonNeumann has no term-level search capability".into(),
                });
            }
        }
        _ => {}
    }

    // Canonicalization vs equality mode
    if world.epistemic.canonicalize.normalization >= NormalizationStrength::Strong {
        if matches!(equality, hyperion::substrate::EqualityMode::EqualitySaturation) {
            issues.push(AuditIssue {
                severity: Severity::Warning,
                axis: "canonicalize".into(),
                detail: "strong normalization with equality-saturation is unusual — e-graphs don't produce unique normal forms via normalization".into(),
            });
        }
        // VonNeumann or hash-only engines can't normalize terms
        if matches!(engine, hyperion::substrate::Engine::VonNeumann)
            && !matches!(equality, hyperion::substrate::EqualityMode::RewriteEquivalence
                | hyperion::substrate::EqualityMode::EqualitySaturation)
        {
            issues.push(AuditIssue {
                severity: Severity::Error,
                axis: "canonicalize".into(),
                detail: format!(
                    "strong normalization requires a term rewriting engine, but substrate uses {:?} with {:?} — \
                     no reduction strategy available",
                    engine, equality
                ),
            });
        }
    }

    if world.epistemic.canonicalize.unique_normal_forms {
        if matches!(equality, hyperion::substrate::EqualityMode::EqualitySaturation) {
            issues.push(AuditIssue {
                severity: Severity::Warning,
                axis: "canonicalize".into(),
                detail: "unique-normal-forms with equality-saturation — e-graphs use equivalence classes, not unique forms".into(),
            });
        }
    }

    // Compression vs engine
    if world.epistemic.compress.mode == CompressionMode::Codegen {
        if !matches!(engine,
            hyperion::substrate::Engine::AbstractMachine | hyperion::substrate::Engine::VonNeumann
        ) {
            issues.push(AuditIssue {
                severity: Severity::Warning,
                axis: "compress".into(),
                detail: format!(
                    "codegen compression typically requires abstract-machine or von-neumann engine, but substrate uses {:?}",
                    engine
                ),
            });
        }
    }

    // Verification vs engine capability
    if world.epistemic.verify.termination >= crate::epistemic::Termination::Decidable {
        if matches!(engine, hyperion::substrate::Engine::InteractionGraph) {
            issues.push(AuditIssue {
                severity: Severity::Info,
                axis: "verify".into(),
                detail: "decidable termination on interaction-graph — ensure rewrite rules are terminating".into(),
            });
        }
    }
}

/// Check epistemic plausibility without substrate context.
fn check_epistemic_plausibility(
    world: &WorldDef,
    issues: &mut Vec<AuditIssue>,
) {
    // Codegen compression with complete discovery is unusual
    if world.epistemic.compress.mode == CompressionMode::Codegen
        && world.epistemic.discover == DiscoveryStrength::Complete
    {
        issues.push(AuditIssue {
            severity: Severity::Warning,
            axis: "compress×discover".into(),
            detail: "codegen compression with complete discovery is unusual — codegen worlds typically have no discovery".into(),
        });
    }

    // Unique normal forms without confluence
    if world.epistemic.canonicalize.unique_normal_forms
        && !world.epistemic.canonicalize.confluence
    {
        issues.push(AuditIssue {
            severity: Severity::Error,
            axis: "canonicalize".into(),
            detail: "unique-normal-forms requires confluence — normal forms cannot be unique without confluence".into(),
        });
    }
}

/// Check that class-epistemic overrides don't exceed what the substrate can physically support.
///
/// A world can be *weaker* in certain domains (e.g., an e-graph failing at resource-sensitive logic),
/// but it cannot be *stronger* than its physical substrate allows. A directed rewriter cannot
/// suddenly do complete discovery just because the theorem is labeled "MagicClass".
fn check_class_epistemic_substrate(
    world: &WorldDef,
    substrate: &hyperion::substrate::SubstrateDef,
    issues: &mut Vec<AuditIssue>,
) {
    let equality = &substrate.equality;
    let engine = &substrate.engine;

    for (class, ovr) in &world.epistemic.class_overrides {
        // Discovery override can't exceed substrate capability
        if let Some(discover) = ovr.discover {
            if discover == DiscoveryStrength::Complete
                && !matches!(equality, hyperion::substrate::EqualityMode::EqualitySaturation)
            {
                issues.push(AuditIssue {
                    severity: Severity::Error,
                    axis: format!("class-epistemic({})", class),
                    detail: format!(
                        "class override declares complete discovery, but substrate uses {:?} — \
                         a theorem class cannot exceed the substrate's physical search capability",
                        equality
                    ),
                });
            }
            if discover >= DiscoveryStrength::SemiDecidable
                && matches!(engine, hyperion::substrate::Engine::VonNeumann)
            {
                issues.push(AuditIssue {
                    severity: Severity::Error,
                    axis: format!("class-epistemic({})", class),
                    detail: "class override declares search capability on VonNeumann engine — \
                             a theorem class cannot exceed the substrate's physical limits".into(),
                });
            }
        }

        // Normalization override can't exceed substrate capability
        if let Some(ref canon) = ovr.canonicalize {
            if canon.normalization >= NormalizationStrength::Strong {
                if matches!(engine, hyperion::substrate::Engine::VonNeumann)
                    && !matches!(equality, hyperion::substrate::EqualityMode::RewriteEquivalence
                        | hyperion::substrate::EqualityMode::EqualitySaturation)
                {
                    issues.push(AuditIssue {
                        severity: Severity::Error,
                        axis: format!("class-epistemic({})", class),
                        detail: format!(
                            "class override declares strong normalization, but substrate {:?}/{:?} \
                             cannot normalize terms — physical limits apply to all theorem classes",
                            engine, equality
                        ),
                    });
                }
            }
        }
    }
}
