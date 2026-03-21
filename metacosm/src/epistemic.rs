use apeiron::parser::Sexp;

use crate::error::{MetacosmError, Result};

// ============================================================================
// Layer A: Primitive grades — the atoms of epistemic structure
// ============================================================================

/// Discovery strength: what class of search/derivation a world supports.
/// This axis is clean as a single chain: none < heuristic < semi-decidable < complete-fragment < complete
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiscoveryStrength {
    None = 0,
    Heuristic = 1,
    SemiDecidable = 2,
    CompleteFragment = 3,
    Complete = 4,
}

impl Default for DiscoveryStrength {
    fn default() -> Self {
        DiscoveryStrength::Heuristic
    }
}

impl std::fmt::Display for DiscoveryStrength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscoveryStrength::None => write!(f, "none"),
            DiscoveryStrength::Heuristic => write!(f, "heuristic"),
            DiscoveryStrength::SemiDecidable => write!(f, "semi-decidable"),
            DiscoveryStrength::CompleteFragment => write!(f, "complete-fragment"),
            DiscoveryStrength::Complete => write!(f, "complete"),
        }
    }
}

fn parse_discovery_strength(s: &str) -> Result<DiscoveryStrength> {
    match s {
        "none" => Ok(DiscoveryStrength::None),
        "heuristic" => Ok(DiscoveryStrength::Heuristic),
        "semi-decidable" => Ok(DiscoveryStrength::SemiDecidable),
        "complete-fragment" => Ok(DiscoveryStrength::CompleteFragment),
        "complete" => Ok(DiscoveryStrength::Complete),
        _ => Err(MetacosmError::ParseError {
            block: "EpistemicProfile".into(),
            detail: format!(
                "unknown discovery strength: '{}' (expected none/heuristic/semi-decidable/complete-fragment/complete)",
                s
            ),
        }),
    }
}

// ============================================================================
// Verification — decomposed into soundness x completeness x termination
// ============================================================================

/// Soundness: does the checker reject invalid things?
/// none < heuristic < sound
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Soundness {
    None = 0,
    Heuristic = 1,
    Sound = 2,
}

impl Default for Soundness {
    fn default() -> Self {
        Soundness::Sound
    }
}

impl std::fmt::Display for Soundness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Soundness::None => write!(f, "none"),
            Soundness::Heuristic => write!(f, "heuristic"),
            Soundness::Sound => write!(f, "sound"),
        }
    }
}

fn parse_soundness(s: &str) -> Result<Soundness> {
    match s {
        "none" => Ok(Soundness::None),
        "heuristic" => Ok(Soundness::Heuristic),
        "sound" => Ok(Soundness::Sound),
        _ => Err(MetacosmError::ParseError {
            block: "EpistemicProfile/verification".into(),
            detail: format!("unknown soundness: '{}' (expected none/heuristic/sound)", s),
        }),
    }
}

/// Completeness: does the checker accept all valid things?
/// none < partial < complete
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Completeness {
    None = 0,
    Partial = 1,
    Complete = 2,
}

impl Default for Completeness {
    fn default() -> Self {
        Completeness::None
    }
}

impl std::fmt::Display for Completeness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Completeness::None => write!(f, "none"),
            Completeness::Partial => write!(f, "partial"),
            Completeness::Complete => write!(f, "complete"),
        }
    }
}

fn parse_completeness(s: &str) -> Result<Completeness> {
    match s {
        "none" => Ok(Completeness::None),
        "partial" => Ok(Completeness::Partial),
        "complete" => Ok(Completeness::Complete),
        _ => Err(MetacosmError::ParseError {
            block: "EpistemicProfile/verification".into(),
            detail: format!("unknown completeness: '{}' (expected none/partial/complete)", s),
        }),
    }
}

/// Termination: does the checker always halt?
/// unknown < semi-decidable < decidable
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Termination {
    Unknown = 0,
    SemiDecidable = 1,
    Decidable = 2,
}

impl Default for Termination {
    fn default() -> Self {
        Termination::Unknown
    }
}

impl std::fmt::Display for Termination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Termination::Unknown => write!(f, "unknown"),
            Termination::SemiDecidable => write!(f, "semi-decidable"),
            Termination::Decidable => write!(f, "decidable"),
        }
    }
}

fn parse_termination(s: &str) -> Result<Termination> {
    match s {
        "unknown" => Ok(Termination::Unknown),
        "semi-decidable" => Ok(Termination::SemiDecidable),
        "decidable" => Ok(Termination::Decidable),
        _ => Err(MetacosmError::ParseError {
            block: "EpistemicProfile/verification".into(),
            detail: format!("unknown termination: '{}' (expected unknown/semi-decidable/decidable)", s),
        }),
    }
}

/// Verification profile: the product of soundness x completeness x termination.
///
/// NOT a single linear order. A sound-and-complete verifier that doesn't terminate
/// is genuinely different from a decidable checker with no completeness guarantee.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VerificationProfile {
    pub soundness: Soundness,
    pub completeness: Completeness,
    pub termination: Termination,
}

impl Default for VerificationProfile {
    fn default() -> Self {
        VerificationProfile {
            soundness: Soundness::default(),
            completeness: Completeness::default(),
            termination: Termination::default(),
        }
    }
}

impl VerificationProfile {
    pub fn none() -> Self {
        VerificationProfile {
            soundness: Soundness::None,
            completeness: Completeness::None,
            termination: Termination::Unknown,
        }
    }

    pub fn can_verify(&self) -> bool {
        self.soundness > Soundness::None
    }

    pub fn dominates(&self, other: &Self) -> bool {
        self.soundness >= other.soundness
            && self.completeness >= other.completeness
            && self.termination >= other.termination
    }

    pub fn distance(&self, other: &Self) -> u32 {
        (self.soundness as i32 - other.soundness as i32).unsigned_abs()
            + (self.completeness as i32 - other.completeness as i32).unsigned_abs()
            + (self.termination as i32 - other.termination as i32).unsigned_abs()
    }
}

impl std::fmt::Display for VerificationProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.soundness == Soundness::None {
            return write!(f, "none");
        }
        let mut parts = vec![self.soundness.to_string()];
        if self.completeness > Completeness::None {
            parts.push(self.completeness.to_string());
        }
        if self.termination > Termination::Unknown {
            parts.push(self.termination.to_string());
        }
        write!(f, "{}", parts.join("+"))
    }
}

/// Parse short verification sugar into a profile.
///
/// Short syntax sugar (backward-compatible):
///   none         → {none, none, unknown}
///   heuristic    → {heuristic, none, unknown}
///   sound        → {sound, none, unknown}
///   sound-complete → {sound, complete, unknown}
///   decidable    → {sound, complete, decidable}
fn parse_verification_sugar(s: &str) -> Result<VerificationProfile> {
    match s {
        "none" => Ok(VerificationProfile::none()),
        "heuristic" => Ok(VerificationProfile {
            soundness: Soundness::Heuristic,
            completeness: Completeness::None,
            termination: Termination::Unknown,
        }),
        "sound" => Ok(VerificationProfile {
            soundness: Soundness::Sound,
            completeness: Completeness::None,
            termination: Termination::Unknown,
        }),
        "sound-complete" => Ok(VerificationProfile {
            soundness: Soundness::Sound,
            completeness: Completeness::Complete,
            termination: Termination::Unknown,
        }),
        "decidable" => Ok(VerificationProfile {
            soundness: Soundness::Sound,
            completeness: Completeness::Complete,
            termination: Termination::Decidable,
        }),
        _ => Err(MetacosmError::ParseError {
            block: "EpistemicProfile".into(),
            detail: format!(
                "unknown verification sugar: '{}' (expected none/heuristic/sound/sound-complete/decidable, or use full syntax [:soundness S :completeness C :termination T])",
                s
            ),
        }),
    }
}

/// Parse full verification sub-block: `[:soundness S :completeness C :termination T]`
fn parse_verification_block(items: &[Sexp]) -> Result<VerificationProfile> {
    let mut p = VerificationProfile::default();
    let mut i = 0;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":soundness" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    p.soundness = parse_soundness(v)?;
                }
            }
            ":completeness" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    p.completeness = parse_completeness(v)?;
                }
            }
            ":termination" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    p.termination = parse_termination(v)?;
                }
            }
            _ => {
                return Err(MetacosmError::ParseError {
                    block: "EpistemicProfile/verification".into(),
                    detail: format!("unknown key: {} (expected :soundness/:completeness/:termination)", key),
                });
            }
        }
        i += 1;
    }
    Ok(p)
}

// ============================================================================
// Canonicality — decomposed into normalization x confluence x unique-nf
// ============================================================================

/// Normalization strength: does the system reach normal forms?
/// none < weak < strong
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NormalizationStrength {
    None = 0,
    Weak = 1,
    Strong = 2,
}

impl Default for NormalizationStrength {
    fn default() -> Self {
        NormalizationStrength::Weak
    }
}

impl std::fmt::Display for NormalizationStrength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NormalizationStrength::None => write!(f, "none"),
            NormalizationStrength::Weak => write!(f, "weak"),
            NormalizationStrength::Strong => write!(f, "strong"),
        }
    }
}

fn parse_normalization_strength(s: &str) -> Result<NormalizationStrength> {
    match s {
        "none" => Ok(NormalizationStrength::None),
        "weak" => Ok(NormalizationStrength::Weak),
        "strong" => Ok(NormalizationStrength::Strong),
        _ => Err(MetacosmError::ParseError {
            block: "EpistemicProfile/canonicality".into(),
            detail: format!("unknown normalization: '{}' (expected none/weak/strong)", s),
        }),
    }
}

/// Canonicality profile: the product of normalization x confluence x unique-normal-forms.
///
/// NOT a total order. A system can be confluent without being normalizing,
/// or normalizing without being confluent. unique-normal-forms typically
/// requires both confluence and normalization.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalityProfile {
    pub normalization: NormalizationStrength,
    pub confluence: bool,
    pub unique_normal_forms: bool,
}

impl Default for CanonicalityProfile {
    fn default() -> Self {
        CanonicalityProfile {
            normalization: NormalizationStrength::default(),
            confluence: false,
            unique_normal_forms: false,
        }
    }
}

impl CanonicalityProfile {
    pub fn none() -> Self {
        CanonicalityProfile {
            normalization: NormalizationStrength::None,
            confluence: false,
            unique_normal_forms: false,
        }
    }

    pub fn dominates(&self, other: &Self) -> bool {
        self.normalization >= other.normalization
            && (self.confluence || !other.confluence)
            && (self.unique_normal_forms || !other.unique_normal_forms)
    }

    pub fn distance(&self, other: &Self) -> u32 {
        (self.normalization as i32 - other.normalization as i32).unsigned_abs()
            + if self.confluence != other.confluence { 1 } else { 0 }
            + if self.unique_normal_forms != other.unique_normal_forms { 1 } else { 0 }
    }
}

impl std::fmt::Display for CanonicalityProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.normalization == NormalizationStrength::None && !self.confluence && !self.unique_normal_forms {
            return write!(f, "none");
        }
        let mut parts = Vec::new();
        if self.normalization > NormalizationStrength::None {
            parts.push(format!("norm={}", self.normalization));
        }
        if self.confluence {
            parts.push("confluent".to_string());
        }
        if self.unique_normal_forms {
            parts.push("unique-nf".to_string());
        }
        write!(f, "{}", parts.join("+"))
    }
}

/// Parse short canonicality sugar.
///
///   none       → {none, false, false}
///   weak-nf    → {weak, false, false}
///   normalizing → {strong, false, false}
///   confluent  → {none, true, false}
///   unique-nf  → {strong, true, true}
fn parse_canonicality_sugar(s: &str) -> Result<CanonicalityProfile> {
    match s {
        "none" => Ok(CanonicalityProfile::none()),
        "weak-nf" => Ok(CanonicalityProfile {
            normalization: NormalizationStrength::Weak,
            confluence: false,
            unique_normal_forms: false,
        }),
        "normalizing" => Ok(CanonicalityProfile {
            normalization: NormalizationStrength::Strong,
            confluence: false,
            unique_normal_forms: false,
        }),
        "confluent" => Ok(CanonicalityProfile {
            normalization: NormalizationStrength::None,
            confluence: true,
            unique_normal_forms: false,
        }),
        "unique-nf" => Ok(CanonicalityProfile {
            normalization: NormalizationStrength::Strong,
            confluence: true,
            unique_normal_forms: true,
        }),
        _ => Err(MetacosmError::ParseError {
            block: "EpistemicProfile".into(),
            detail: format!(
                "unknown canonicality sugar: '{}' (expected none/weak-nf/normalizing/confluent/unique-nf, or use full syntax [:normalization N :confluence B :unique-normal-forms B])",
                s
            ),
        }),
    }
}

fn parse_bool(s: &str) -> Result<bool> {
    match s {
        "yes" | "true" => Ok(true),
        "no" | "false" => Ok(false),
        _ => Err(MetacosmError::ParseError {
            block: "EpistemicProfile".into(),
            detail: format!("expected yes/no or true/false, got: '{}'", s),
        }),
    }
}

/// Parse full canonicality sub-block: `[:normalization N :confluence B :unique-normal-forms B]`
fn parse_canonicality_block(items: &[Sexp]) -> Result<CanonicalityProfile> {
    let mut p = CanonicalityProfile::default();
    let mut i = 0;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":normalization" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    p.normalization = parse_normalization_strength(v)?;
                }
            }
            ":confluence" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    p.confluence = parse_bool(v)?;
                }
            }
            ":unique-normal-forms" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    p.unique_normal_forms = parse_bool(v)?;
                }
            }
            _ => {
                return Err(MetacosmError::ParseError {
                    block: "EpistemicProfile/canonicality".into(),
                    detail: format!("unknown key: {} (expected :normalization/:confluence/:unique-normal-forms)", key),
                });
            }
        }
        i += 1;
    }
    Ok(p)
}

// ============================================================================
// Compression — mode + semantic properties (lossy, invertible)
// ============================================================================

/// Compression mode: the kind of compression/compilation.
/// Not ordinal — these are qualitatively different morphism semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompressionMode {
    None,
    Lossless,
    Lossy,
    Quotient,
    Abstraction,
    Codegen,
}

impl Default for CompressionMode {
    fn default() -> Self {
        CompressionMode::None
    }
}

impl std::fmt::Display for CompressionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompressionMode::None => write!(f, "none"),
            CompressionMode::Lossless => write!(f, "lossless"),
            CompressionMode::Lossy => write!(f, "lossy"),
            CompressionMode::Quotient => write!(f, "quotient"),
            CompressionMode::Abstraction => write!(f, "abstraction"),
            CompressionMode::Codegen => write!(f, "codegen"),
        }
    }
}

fn parse_compression_mode(s: &str) -> Result<CompressionMode> {
    match s {
        "none" => Ok(CompressionMode::None),
        "lossless" => Ok(CompressionMode::Lossless),
        "lossy" => Ok(CompressionMode::Lossy),
        "quotient" => Ok(CompressionMode::Quotient),
        "abstraction" => Ok(CompressionMode::Abstraction),
        "codegen" => Ok(CompressionMode::Codegen),
        _ => Err(MetacosmError::ParseError {
            block: "EpistemicProfile".into(),
            detail: format!(
                "unknown compression mode: '{}' (expected none/lossless/lossy/quotient/abstraction/codegen)",
                s
            ),
        }),
    }
}

/// Compression profile: mode + semantic properties.
///
/// The mode is a tag (non-ordinal). The properties give compositional
/// structure: whether the compression is lossy, whether it is invertible.
/// More properties (preserves_proofs, preserves_executability, target_operational)
/// can be added later without breaking syntax.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompressionProfile {
    pub mode: CompressionMode,
    pub lossy: bool,
    pub invertible: bool,
}

impl Default for CompressionProfile {
    fn default() -> Self {
        CompressionProfile {
            mode: CompressionMode::None,
            lossy: false,
            invertible: false,
        }
    }
}

impl CompressionProfile {
    pub fn distance(&self, other: &Self) -> u32 {
        (if self.mode != other.mode { 1 } else { 0 })
            + (if self.lossy != other.lossy { 1 } else { 0 })
            + (if self.invertible != other.invertible { 1 } else { 0 })
    }
}

impl std::fmt::Display for CompressionProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.mode == CompressionMode::None {
            return write!(f, "none");
        }
        let mut props = Vec::new();
        if self.lossy {
            props.push("lossy");
        }
        if self.invertible {
            props.push("invertible");
        }
        if props.is_empty() {
            write!(f, "{}", self.mode)
        } else {
            write!(f, "{}({})", self.mode, props.join(","))
        }
    }
}

/// Parse short compression sugar.
///
///   none        → {None, false, false}
///   lossless    → {Lossless, false, true}
///   lossy       → {Lossy, true, false}
///   quotient    → {Quotient, true, false}
///   abstraction → {Abstraction, true, false}
///   codegen     → {Codegen, true, false}
fn parse_compression_sugar(s: &str) -> Result<CompressionProfile> {
    let mode = parse_compression_mode(s)?;
    let (lossy, invertible) = match mode {
        CompressionMode::None => (false, false),
        CompressionMode::Lossless => (false, true),
        CompressionMode::Lossy => (true, false),
        CompressionMode::Quotient => (true, false),
        CompressionMode::Abstraction => (true, false),
        CompressionMode::Codegen => (true, false),
    };
    Ok(CompressionProfile { mode, lossy, invertible })
}

/// Parse full compression sub-block: `[:mode M :lossy B :invertible B]`
fn parse_compression_block(items: &[Sexp]) -> Result<CompressionProfile> {
    let mut p = CompressionProfile::default();
    let mut i = 0;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":mode" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    p.mode = parse_compression_mode(v)?;
                }
            }
            ":lossy" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    p.lossy = parse_bool(v)?;
                }
            }
            ":invertible" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    p.invertible = parse_bool(v)?;
                }
            }
            _ => {
                return Err(MetacosmError::ParseError {
                    block: "EpistemicProfile/compression".into(),
                    detail: format!("unknown key: {} (expected :mode/:lossy/:invertible)", key),
                });
            }
        }
        i += 1;
    }
    Ok(p)
}

// ============================================================================
// Layer B: Epistemic profile — intrinsic, unary properties of a world
// ============================================================================

/// A partial override of an epistemic profile for a specific theorem class.
/// Only fields that are `Some` override the default.
#[derive(Debug, Clone, Default)]
pub struct EpistemicOverride {
    pub discover: Option<DiscoveryStrength>,
    pub verify: Option<VerificationProfile>,
    pub canonicalize: Option<CanonicalityProfile>,
    pub compress: Option<CompressionProfile>,
}

/// The epistemic signature of a world.
///
/// Four axes, each now a typed product rather than a single scalar:
///   discover:      what class of search the world supports (single chain — clean)
///   verify:        soundness x completeness x termination (product — these are independent)
///   canonicalize:  normalization x confluence x unique-nf (product — not a total order)
///   compress:      mode + lossy + invertible (tag + properties — compositional)
///
/// Transportability is NOT here — it is relational and belongs on transitions.
///
/// The `class_overrides` map allows theorem-class-sensitive epistemic claims.
/// A world might be excellent at equational discovery but weak at resource-sensitive reasoning.
#[derive(Debug, Clone)]
pub struct EpistemicProfile {
    pub discover: DiscoveryStrength,
    pub verify: VerificationProfile,
    pub canonicalize: CanonicalityProfile,
    pub compress: CompressionProfile,
    /// Per-theorem-class overrides. If a class is absent, the default profile applies.
    pub class_overrides: std::collections::HashMap<crate::theorem_class::TheoremClass, EpistemicOverride>,
}

impl PartialEq for EpistemicProfile {
    fn eq(&self, other: &Self) -> bool {
        self.discover == other.discover
            && self.verify == other.verify
            && self.canonicalize == other.canonicalize
            && self.compress == other.compress
    }
}

impl Eq for EpistemicProfile {}

impl Default for EpistemicProfile {
    fn default() -> Self {
        EpistemicProfile {
            discover: DiscoveryStrength::default(),
            verify: VerificationProfile::default(),
            canonicalize: CanonicalityProfile::default(),
            compress: CompressionProfile::default(),
            class_overrides: std::collections::HashMap::new(),
        }
    }
}

impl EpistemicProfile {
    /// The trivial profile: defaults for Omega/Hyperion embedding.
    pub fn trivial() -> Self {
        Self::default()
    }

    pub fn can_discover(&self) -> bool {
        self.discover > DiscoveryStrength::None
    }

    pub fn can_verify(&self) -> bool {
        self.verify.can_verify()
    }

    /// Get the effective profile for a specific theorem class.
    /// Merges class-specific overrides onto the default profile.
    pub fn for_class(&self, class: &crate::theorem_class::TheoremClass) -> EpistemicProfile {
        if let Some(ovr) = self.class_overrides.get(class) {
            EpistemicProfile {
                discover: ovr.discover.unwrap_or(self.discover),
                verify: ovr.verify.clone().unwrap_or_else(|| self.verify.clone()),
                canonicalize: ovr.canonicalize.clone().unwrap_or_else(|| self.canonicalize.clone()),
                compress: ovr.compress.clone().unwrap_or_else(|| self.compress.clone()),
                class_overrides: std::collections::HashMap::new(),
            }
        } else {
            self.clone()
        }
    }

    /// Provisional epistemic distance. Sum of per-sub-axis differences.
    ///
    /// This is a debugging/demo metric, not yet a deep semantic notion.
    /// A proper treatment would define separate capability, guarantee,
    /// transport-loss, and operational distances.
    pub fn distance(&self, other: &EpistemicProfile) -> u32 {
        let d_disc = (self.discover as i32 - other.discover as i32).unsigned_abs();
        d_disc + self.verify.distance(&other.verify)
            + self.canonicalize.distance(&other.canonicalize)
            + self.compress.distance(&other.compress)
    }

    /// Does this profile dominate another on all ordinal sub-axes?
    /// Compression mode is non-ordinal — only lossy/invertible compared.
    pub fn dominates(&self, other: &EpistemicProfile) -> bool {
        self.discover >= other.discover
            && self.verify.dominates(&other.verify)
            && self.canonicalize.dominates(&other.canonicalize)
    }
}

/// Parse an epistemic profile from S-expression items.
///
/// Syntax per axis:
///   Short: `:verify sound`                  (sugar → product defaults)
///   Full:  `:verify [:soundness sound :completeness complete :termination decidable]`
///
///   Short: `:canonicalize confluent`         (sugar → product defaults)
///   Full:  `:canonicalize [:normalization strong :confluence yes :unique-normal-forms yes]`
///
///   Short: `:compress codegen`              (sugar → mode + default properties)
///   Full:  `:compress [:mode codegen :lossy yes :invertible no]`
///
///   Short: `:discover complete`             (single chain, no decomposition needed)
///   Full:  `:discover [:strength complete]`
pub fn parse_epistemic_profile(items: &[Sexp]) -> Result<EpistemicProfile> {
    let mut profile = EpistemicProfile::default();

    let mut i = 0;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":discover" => {
                i += 1;
                if let Some(item) = items.get(i) {
                    if let Some(v) = item.as_atom() {
                        profile.discover = parse_discovery_strength(v)?;
                    } else if let Some(inner) = item.as_list() {
                        // Full: [:strength S]
                        profile.discover = parse_discovery_block(inner)?;
                    }
                }
            }
            ":verify" => {
                i += 1;
                if let Some(item) = items.get(i) {
                    if let Some(v) = item.as_atom() {
                        profile.verify = parse_verification_sugar(v)?;
                    } else if let Some(inner) = item.as_list() {
                        profile.verify = parse_verification_block(inner)?;
                    }
                }
            }
            ":canonicalize" => {
                i += 1;
                if let Some(item) = items.get(i) {
                    if let Some(v) = item.as_atom() {
                        profile.canonicalize = parse_canonicality_sugar(v)?;
                    } else if let Some(inner) = item.as_list() {
                        profile.canonicalize = parse_canonicality_block(inner)?;
                    }
                }
            }
            ":compress" => {
                i += 1;
                if let Some(item) = items.get(i) {
                    if let Some(v) = item.as_atom() {
                        profile.compress = parse_compression_sugar(v)?;
                    } else if let Some(inner) = item.as_list() {
                        profile.compress = parse_compression_block(inner)?;
                    }
                }
            }
            _ => {
                return Err(MetacosmError::ParseError {
                    block: "EpistemicProfile".into(),
                    detail: format!("unknown epistemic key: {}", key),
                });
            }
        }
        i += 1;
    }

    Ok(profile)
}

/// Parse `[:strength S]` for the discovery axis (still a single chain).
fn parse_discovery_block(items: &[Sexp]) -> Result<DiscoveryStrength> {
    let mut i = 0;
    let mut result = None;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":strength" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    result = Some(parse_discovery_strength(v)?);
                }
            }
            _ => {
                return Err(MetacosmError::ParseError {
                    block: "EpistemicProfile/discovery".into(),
                    detail: format!("unknown key: {} (expected :strength)", key),
                });
            }
        }
        i += 1;
    }
    result.ok_or_else(|| MetacosmError::ParseError {
        block: "EpistemicProfile/discovery".into(),
        detail: "block missing :strength".into(),
    })
}

// ============================================================================
// Observables and measurements
// ============================================================================

/// A named observable: a measurable epistemic quantity.
#[derive(Debug, Clone)]
pub struct Observable {
    pub name: String,
    pub kind: ObservableKind,
    /// Whether this is a semantic (meta-theoretic) or empirical (operational) observable.
    pub species: crate::knowledge::KnowledgeSpecies,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservableKind {
    // --- Single-axis observables ---
    DiscoveryStrength,

    // --- Verification sub-axes ---
    VerificationSoundness,
    VerificationCompleteness,
    VerificationTermination,
    /// Summary: the full verification profile
    VerificationProfile,

    // --- Canonicality sub-axes ---
    CanonicalityNormalization,
    CanonicalityConfluence,
    CanonicalityUniqueNf,
    /// Summary: the full canonicality profile
    CanonicalityProfile,

    // --- Compression sub-axes ---
    CompressionMode,
    CompressionLossy,
    CompressionInvertible,
    /// Summary: the full compression profile
    CompressionProfile,

    // --- Relational ---
    EpistemicDistance,

    // --- Empirical (operational, not derivable from profile) ---
    ProofSize,
    SearchCost,
    Runtime,
    WitnessExtractionCost,

    Custom,
}

pub fn parse_observable(items: &[Sexp]) -> Result<Observable> {
    if items.len() < 2 {
        return Err(MetacosmError::ParseError {
            block: "Observable".into(),
            detail: "missing observable name".into(),
        });
    }

    let name = items[1]
        .as_atom()
        .ok_or_else(|| MetacosmError::ParseError {
            block: "Observable".into(),
            detail: "observable name must be an atom".into(),
        })?
        .to_string();

    let mut kind = ObservableKind::Custom;
    let mut species = crate::knowledge::KnowledgeSpecies::Semantic;

    let mut i = 2;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":kind" => {
                i += 1;
                if let Some(k) = items.get(i).and_then(|s| s.as_atom()) {
                    kind = match k {
                        "discovery-strength" => ObservableKind::DiscoveryStrength,
                        // Verification
                        "verification-soundness" => ObservableKind::VerificationSoundness,
                        "verification-completeness" => ObservableKind::VerificationCompleteness,
                        "verification-termination" => ObservableKind::VerificationTermination,
                        "verification-strength" | "verification-profile" => ObservableKind::VerificationProfile,
                        // Canonicality
                        "canonicality-normalization" => ObservableKind::CanonicalityNormalization,
                        "canonicality-confluence" => ObservableKind::CanonicalityConfluence,
                        "canonicality-unique-nf" => ObservableKind::CanonicalityUniqueNf,
                        "canonicality-strength" | "canonicality-profile" => ObservableKind::CanonicalityProfile,
                        // Compression
                        "compression-mode" => ObservableKind::CompressionMode,
                        "compression-lossy" => ObservableKind::CompressionLossy,
                        "compression-invertible" => ObservableKind::CompressionInvertible,
                        "compression-profile" => ObservableKind::CompressionProfile,
                        // Relational
                        "epistemic-distance" => ObservableKind::EpistemicDistance,
                        // Empirical
                        "proof-size" => { species = crate::knowledge::KnowledgeSpecies::Empirical; ObservableKind::ProofSize }
                        "search-cost" => { species = crate::knowledge::KnowledgeSpecies::Empirical; ObservableKind::SearchCost }
                        "runtime" => { species = crate::knowledge::KnowledgeSpecies::Empirical; ObservableKind::Runtime }
                        "witness-extraction-cost" => { species = crate::knowledge::KnowledgeSpecies::Empirical; ObservableKind::WitnessExtractionCost }
                        _ => ObservableKind::Custom,
                    };
                }
            }
            ":species" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    species = match v {
                        "semantic" => crate::knowledge::KnowledgeSpecies::Semantic,
                        "empirical" => crate::knowledge::KnowledgeSpecies::Empirical,
                        _ => {
                            return Err(MetacosmError::ParseError {
                                block: "Observable".into(),
                                detail: format!("unknown species: {} (expected semantic/empirical)", v),
                            });
                        }
                    };
                }
            }
            _ => {
                return Err(MetacosmError::ParseError {
                    block: "Observable".into(),
                    detail: format!("unknown keyword: {}", key),
                });
            }
        }
        i += 1;
    }

    Ok(Observable { name, kind, species })
}

/// Measure result: the outcome of evaluating an observable.
#[derive(Debug, Clone)]
pub struct Measurement {
    pub observable: String,
    pub world: String,
    pub target_world: Option<String>,
    pub value: MeasureValue,
}

#[derive(Debug, Clone)]
pub enum MeasureValue {
    Grade(String),
    Distance(u32),
    Boolean(bool),
}

impl std::fmt::Display for MeasureValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MeasureValue::Grade(s) => write!(f, "{}", s),
            MeasureValue::Distance(n) => write!(f, "{}", n),
            MeasureValue::Boolean(b) => write!(f, "{}", b),
        }
    }
}

/// Extract a measurement value from an epistemic profile for a given observable kind.
pub fn measure_profile(ep: &EpistemicProfile, kind: &ObservableKind) -> Option<MeasureValue> {
    match kind {
        ObservableKind::DiscoveryStrength => Some(MeasureValue::Grade(ep.discover.to_string())),
        // Verification sub-axes
        ObservableKind::VerificationSoundness => Some(MeasureValue::Grade(ep.verify.soundness.to_string())),
        ObservableKind::VerificationCompleteness => Some(MeasureValue::Grade(ep.verify.completeness.to_string())),
        ObservableKind::VerificationTermination => Some(MeasureValue::Grade(ep.verify.termination.to_string())),
        ObservableKind::VerificationProfile => Some(MeasureValue::Grade(ep.verify.to_string())),
        // Canonicality sub-axes
        ObservableKind::CanonicalityNormalization => Some(MeasureValue::Grade(ep.canonicalize.normalization.to_string())),
        ObservableKind::CanonicalityConfluence => Some(MeasureValue::Boolean(ep.canonicalize.confluence)),
        ObservableKind::CanonicalityUniqueNf => Some(MeasureValue::Boolean(ep.canonicalize.unique_normal_forms)),
        ObservableKind::CanonicalityProfile => Some(MeasureValue::Grade(ep.canonicalize.to_string())),
        // Compression sub-axes
        ObservableKind::CompressionMode => Some(MeasureValue::Grade(ep.compress.mode.to_string())),
        ObservableKind::CompressionLossy => Some(MeasureValue::Boolean(ep.compress.lossy)),
        ObservableKind::CompressionInvertible => Some(MeasureValue::Boolean(ep.compress.invertible)),
        ObservableKind::CompressionProfile => Some(MeasureValue::Grade(ep.compress.to_string())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apeiron::parser::parse;

    #[test]
    fn parse_profile_short_syntax() {
        let input = "[:discover complete :verify sound :canonicalize confluent :compress lossless]";
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let p = parse_epistemic_profile(items).unwrap();
        assert_eq!(p.discover, DiscoveryStrength::Complete);
        assert_eq!(p.verify.soundness, Soundness::Sound);
        assert_eq!(p.verify.completeness, Completeness::None);
        assert_eq!(p.verify.termination, Termination::Unknown);
        assert!(p.canonicalize.confluence);
        assert!(!p.canonicalize.unique_normal_forms);
        assert_eq!(p.compress.mode, CompressionMode::Lossless);
        assert!(!p.compress.lossy);
        assert!(p.compress.invertible);
    }

    #[test]
    fn parse_profile_full_verification() {
        let input = r#"[
            :discover [:strength semi-decidable]
            :verify [:soundness sound :completeness complete :termination decidable]
            :canonicalize [:normalization strong :confluence yes :unique-normal-forms yes]
            :compress [:mode codegen :lossy yes :invertible no]
        ]"#;
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let p = parse_epistemic_profile(items).unwrap();
        assert_eq!(p.discover, DiscoveryStrength::SemiDecidable);
        assert_eq!(p.verify.soundness, Soundness::Sound);
        assert_eq!(p.verify.completeness, Completeness::Complete);
        assert_eq!(p.verify.termination, Termination::Decidable);
        assert_eq!(p.canonicalize.normalization, NormalizationStrength::Strong);
        assert!(p.canonicalize.confluence);
        assert!(p.canonicalize.unique_normal_forms);
        assert_eq!(p.compress.mode, CompressionMode::Codegen);
        assert!(p.compress.lossy);
        assert!(!p.compress.invertible);
    }

    #[test]
    fn verification_sugar_decidable() {
        let p = parse_verification_sugar("decidable").unwrap();
        assert_eq!(p.soundness, Soundness::Sound);
        assert_eq!(p.completeness, Completeness::Complete);
        assert_eq!(p.termination, Termination::Decidable);
    }

    #[test]
    fn verification_sugar_sound() {
        let p = parse_verification_sugar("sound").unwrap();
        assert_eq!(p.soundness, Soundness::Sound);
        assert_eq!(p.completeness, Completeness::None);
        assert_eq!(p.termination, Termination::Unknown);
    }

    #[test]
    fn canonicality_sugar_confluent() {
        let p = parse_canonicality_sugar("confluent").unwrap();
        assert_eq!(p.normalization, NormalizationStrength::None);
        assert!(p.confluence);
        assert!(!p.unique_normal_forms);
    }

    #[test]
    fn canonicality_sugar_unique_nf() {
        let p = parse_canonicality_sugar("unique-nf").unwrap();
        assert_eq!(p.normalization, NormalizationStrength::Strong);
        assert!(p.confluence);
        assert!(p.unique_normal_forms);
    }

    #[test]
    fn compression_sugar_lossless() {
        let p = parse_compression_sugar("lossless").unwrap();
        assert_eq!(p.mode, CompressionMode::Lossless);
        assert!(!p.lossy);
        assert!(p.invertible);
    }

    #[test]
    fn compression_sugar_codegen() {
        let p = parse_compression_sugar("codegen").unwrap();
        assert_eq!(p.mode, CompressionMode::Codegen);
        assert!(p.lossy);
        assert!(!p.invertible);
    }

    #[test]
    fn canonicality_not_total_order() {
        // Confluent but not normalizing
        let a = CanonicalityProfile {
            normalization: NormalizationStrength::None,
            confluence: true,
            unique_normal_forms: false,
        };
        // Normalizing but not confluent
        let b = CanonicalityProfile {
            normalization: NormalizationStrength::Strong,
            confluence: false,
            unique_normal_forms: false,
        };
        // Neither dominates the other
        assert!(!a.dominates(&b));
        assert!(!b.dominates(&a));
    }

    #[test]
    fn verification_product_incomparable() {
        // Sound but not complete, decidable
        let a = VerificationProfile {
            soundness: Soundness::Sound,
            completeness: Completeness::None,
            termination: Termination::Decidable,
        };
        // Heuristic but complete, unknown termination
        let b = VerificationProfile {
            soundness: Soundness::Heuristic,
            completeness: Completeness::Complete,
            termination: Termination::Unknown,
        };
        assert!(!a.dominates(&b));
        assert!(!b.dominates(&a));
    }

    #[test]
    fn epistemic_distance() {
        let a = EpistemicProfile {
            discover: DiscoveryStrength::Complete,
            verify: VerificationProfile {
                soundness: Soundness::Sound,
                completeness: Completeness::None,
                termination: Termination::Unknown,
            },
            canonicalize: CanonicalityProfile::none(),
            compress: CompressionProfile {
                mode: CompressionMode::Lossless,
                lossy: false,
                invertible: true,
            },
            ..Default::default()
        };
        let b = EpistemicProfile {
            discover: DiscoveryStrength::Heuristic,
            verify: VerificationProfile {
                soundness: Soundness::Sound,
                completeness: Completeness::Complete,
                termination: Termination::Decidable,
            },
            canonicalize: CanonicalityProfile {
                normalization: NormalizationStrength::None,
                confluence: true,
                unique_normal_forms: false,
            },
            compress: CompressionProfile {
                mode: CompressionMode::Codegen,
                lossy: true,
                invertible: false,
            },
            ..Default::default()
        };
        // disc: |4-1| = 3
        // ver: |2-2| + |0-2| + |0-2| = 0+2+2 = 4
        // can: |0-0| + (f!=t) + (f!=f) = 0+1+0 = 1
        // comp: (lossless!=codegen) + (f!=t) + (t!=f) = 1+1+1 = 3
        // total = 3+4+1+3 = 11
        assert_eq!(a.distance(&b), 11);
    }

    #[test]
    fn dominance() {
        let strong = EpistemicProfile {
            discover: DiscoveryStrength::Complete,
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
            compress: CompressionProfile {
                mode: CompressionMode::Codegen,
                lossy: true,
                invertible: false,
            },
            ..Default::default()
        };
        let weak = EpistemicProfile::default();
        assert!(strong.dominates(&weak));
        assert!(!weak.dominates(&strong));
    }

    #[test]
    fn trivial_is_default() {
        assert_eq!(EpistemicProfile::trivial(), EpistemicProfile::default());
    }

    #[test]
    fn none_strength_means_no_capability() {
        let p = EpistemicProfile {
            discover: DiscoveryStrength::None,
            verify: VerificationProfile::none(),
            ..Default::default()
        };
        assert!(!p.can_discover());
        assert!(!p.can_verify());
    }

    #[test]
    fn parse_observable_typed() {
        let input = "[Observable DiscPower :kind discovery-strength]";
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let obs = parse_observable(items).unwrap();
        assert_eq!(obs.name, "DiscPower");
        assert_eq!(obs.kind, ObservableKind::DiscoveryStrength);
    }

    #[test]
    fn parse_observable_sub_axes() {
        let input = "[Observable VSoundness :kind verification-soundness]";
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let obs = parse_observable(items).unwrap();
        assert_eq!(obs.kind, ObservableKind::VerificationSoundness);
    }

    #[test]
    fn parse_observable_backward_compat() {
        // Old kind name still works
        let input = "[Observable VStr :kind verification-strength]";
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let obs = parse_observable(items).unwrap();
        assert_eq!(obs.kind, ObservableKind::VerificationProfile);
    }

    #[test]
    fn compression_modes_are_non_ordinal() {
        assert_ne!(CompressionMode::Lossless, CompressionMode::Quotient);
        assert_ne!(CompressionMode::Lossless, CompressionMode::None);
    }

    #[test]
    fn sub_axis_lattice_ordering() {
        // Soundness
        assert!(Soundness::None < Soundness::Heuristic);
        assert!(Soundness::Heuristic < Soundness::Sound);
        // Completeness
        assert!(Completeness::None < Completeness::Partial);
        assert!(Completeness::Partial < Completeness::Complete);
        // Termination
        assert!(Termination::Unknown < Termination::SemiDecidable);
        assert!(Termination::SemiDecidable < Termination::Decidable);
        // Normalization
        assert!(NormalizationStrength::None < NormalizationStrength::Weak);
        assert!(NormalizationStrength::Weak < NormalizationStrength::Strong);
        // Discovery (unchanged)
        assert!(DiscoveryStrength::None < DiscoveryStrength::Heuristic);
        assert!(DiscoveryStrength::Heuristic < DiscoveryStrength::SemiDecidable);
        assert!(DiscoveryStrength::SemiDecidable < DiscoveryStrength::CompleteFragment);
        assert!(DiscoveryStrength::CompleteFragment < DiscoveryStrength::Complete);
    }
}
