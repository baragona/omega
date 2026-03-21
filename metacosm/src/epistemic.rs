use apeiron::parser::Sexp;

use crate::error::{MetacosmError, Result};

// ============================================================================
// Layer A: Strength grades — finite explicit lattices, not adjectives
// ============================================================================

/// Discovery strength: what class of search/derivation a world supports.
/// none < heuristic < semi_decidable < complete_fragment < complete
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

/// Verification strength: what kind of checking judgment a world supports.
/// none < heuristic < sound < sound_complete < decidable
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VerificationStrength {
    None = 0,
    Heuristic = 1,
    Sound = 2,
    SoundComplete = 3,
    Decidable = 4,
}

impl Default for VerificationStrength {
    fn default() -> Self {
        VerificationStrength::Sound
    }
}

impl std::fmt::Display for VerificationStrength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationStrength::None => write!(f, "none"),
            VerificationStrength::Heuristic => write!(f, "heuristic"),
            VerificationStrength::Sound => write!(f, "sound"),
            VerificationStrength::SoundComplete => write!(f, "sound-complete"),
            VerificationStrength::Decidable => write!(f, "decidable"),
        }
    }
}

fn parse_verification_strength(s: &str) -> Result<VerificationStrength> {
    match s {
        "none" => Ok(VerificationStrength::None),
        "heuristic" => Ok(VerificationStrength::Heuristic),
        "sound" => Ok(VerificationStrength::Sound),
        "sound-complete" => Ok(VerificationStrength::SoundComplete),
        "decidable" => Ok(VerificationStrength::Decidable),
        _ => Err(MetacosmError::ParseError {
            block: "EpistemicProfile".into(),
            detail: format!(
                "unknown verification strength: '{}' (expected none/heuristic/sound/sound-complete/decidable)",
                s
            ),
        }),
    }
}

/// Canonicality strength: normalization-theoretic properties.
/// none < weak_nf < normalizing < confluent < unique_nf
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalityStrength {
    None = 0,
    WeakNf = 1,
    Normalizing = 2,
    Confluent = 3,
    UniqueNf = 4,
}

impl Default for CanonicalityStrength {
    fn default() -> Self {
        CanonicalityStrength::Normalizing
    }
}

impl std::fmt::Display for CanonicalityStrength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CanonicalityStrength::None => write!(f, "none"),
            CanonicalityStrength::WeakNf => write!(f, "weak-nf"),
            CanonicalityStrength::Normalizing => write!(f, "normalizing"),
            CanonicalityStrength::Confluent => write!(f, "confluent"),
            CanonicalityStrength::UniqueNf => write!(f, "unique-nf"),
        }
    }
}

fn parse_canonicality_strength(s: &str) -> Result<CanonicalityStrength> {
    match s {
        "none" => Ok(CanonicalityStrength::None),
        "weak-nf" => Ok(CanonicalityStrength::WeakNf),
        "normalizing" => Ok(CanonicalityStrength::Normalizing),
        "confluent" => Ok(CanonicalityStrength::Confluent),
        "unique-nf" => Ok(CanonicalityStrength::UniqueNf),
        _ => Err(MetacosmError::ParseError {
            block: "EpistemicProfile".into(),
            detail: format!(
                "unknown canonicality strength: '{}' (expected none/weak-nf/normalizing/confluent/unique-nf)",
                s
            ),
        }),
    }
}

/// Compression mode: what kind of compression/compilation a world supports.
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

// ============================================================================
// Layer B: Epistemic profile — intrinsic, unary properties of a world
// ============================================================================

/// The epistemic signature of a world.
///
/// Each axis separates capability (can it do this at all?) from strength
/// (with what formal guarantees?). Transportability is NOT here — it is
/// relational and belongs on transitions.
///
/// Four axes:
///   discover:     what class of search the world supports
///   verify:       what kind of checking judgment it supports
///   canonicalize:  normalization-theoretic properties of the substrate
///   compress:     what kind of compression/compilation is available
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpistemicProfile {
    pub discover: DiscoveryStrength,
    pub verify: VerificationStrength,
    pub canonicalize: CanonicalityStrength,
    pub compress: CompressionMode,
}

impl Default for EpistemicProfile {
    fn default() -> Self {
        EpistemicProfile {
            discover: DiscoveryStrength::default(),
            verify: VerificationStrength::default(),
            canonicalize: CanonicalityStrength::default(),
            compress: CompressionMode::default(),
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
        self.verify > VerificationStrength::None
    }

    /// Epistemic distance on the ordinal axes (discover, verify, canonicalize).
    /// Compression is non-ordinal so contributes 0 if equal, 1 if different.
    pub fn distance(&self, other: &EpistemicProfile) -> u32 {
        let d_disc = (self.discover as i32 - other.discover as i32).unsigned_abs();
        let d_ver = (self.verify as i32 - other.verify as i32).unsigned_abs();
        let d_can = (self.canonicalize as i32 - other.canonicalize as i32).unsigned_abs();
        let d_comp = if self.compress == other.compress { 0 } else { 1 };
        d_disc + d_ver + d_can + d_comp
    }

    /// Does this profile dominate another on ordinal axes?
    /// Compression is non-ordinal — not compared.
    pub fn dominates(&self, other: &EpistemicProfile) -> bool {
        self.discover >= other.discover
            && self.verify >= other.verify
            && self.canonicalize >= other.canonicalize
    }
}

/// Parse an epistemic profile from S-expression items.
///
/// Two syntax forms per axis:
///   Short: `:discover heuristic`           (strength directly, implies capability)
///   Full:  `:discover [:strength heuristic]` (explicit sub-block)
///
/// Axes: :discover, :verify, :canonicalize, :compress
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
                        profile.discover = parse_axis_block(inner, "discover",
                            |s| parse_discovery_strength(s).map(|v| Box::new(v) as Box<dyn std::any::Any>)
                        )?.downcast::<DiscoveryStrength>().map(|b| *b).unwrap_or_default();
                    }
                }
            }
            ":verify" => {
                i += 1;
                if let Some(item) = items.get(i) {
                    if let Some(v) = item.as_atom() {
                        profile.verify = parse_verification_strength(v)?;
                    } else if let Some(inner) = item.as_list() {
                        profile.verify = parse_axis_block(inner, "verify",
                            |s| parse_verification_strength(s).map(|v| Box::new(v) as Box<dyn std::any::Any>)
                        )?.downcast::<VerificationStrength>().map(|b| *b).unwrap_or_default();
                    }
                }
            }
            ":canonicalize" => {
                i += 1;
                if let Some(item) = items.get(i) {
                    if let Some(v) = item.as_atom() {
                        profile.canonicalize = parse_canonicality_strength(v)?;
                    } else if let Some(inner) = item.as_list() {
                        profile.canonicalize = parse_axis_block(inner, "canonicalize",
                            |s| parse_canonicality_strength(s).map(|v| Box::new(v) as Box<dyn std::any::Any>)
                        )?.downcast::<CanonicalityStrength>().map(|b| *b).unwrap_or_default();
                    }
                }
            }
            ":compress" => {
                i += 1;
                if let Some(item) = items.get(i) {
                    if let Some(v) = item.as_atom() {
                        profile.compress = parse_compression_mode(v)?;
                    } else if let Some(inner) = item.as_list() {
                        profile.compress = parse_axis_block(inner, "compress",
                            |s| parse_compression_mode(s).map(|v| Box::new(v) as Box<dyn std::any::Any>)
                        )?.downcast::<CompressionMode>().map(|b| *b).unwrap_or_default();
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

/// Parse `[:strength S]` sub-block for an axis.
fn parse_axis_block<F>(items: &[Sexp], axis_name: &str, parse_strength: F) -> Result<Box<dyn std::any::Any>>
where
    F: Fn(&str) -> Result<Box<dyn std::any::Any>>,
{
    let mut i = 0;
    let mut result: Option<Box<dyn std::any::Any>> = None;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":strength" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    result = Some(parse_strength(v)?);
                }
            }
            ":capability" => {
                // Accepted but redundant — strength != none implies capability
                i += 1;
            }
            _ => {
                return Err(MetacosmError::ParseError {
                    block: "EpistemicProfile".into(),
                    detail: format!("unknown key in {} block: {}", axis_name, key),
                });
            }
        }
        i += 1;
    }
    result.ok_or_else(|| MetacosmError::ParseError {
        block: "EpistemicProfile".into(),
        detail: format!("{} block missing :strength", axis_name),
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservableKind {
    /// Discovery strength of a world
    DiscoveryStrength,
    /// Verification strength of a world
    VerificationStrength,
    /// Canonicality strength of a world
    CanonicalityStrength,
    /// Compression mode of a world
    CompressionMode,
    /// Epistemic distance between two worlds (relational)
    EpistemicDistance,
    /// Custom observable
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

    let mut i = 2;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":kind" => {
                i += 1;
                if let Some(k) = items.get(i).and_then(|s| s.as_atom()) {
                    kind = match k {
                        "discovery-strength" => ObservableKind::DiscoveryStrength,
                        "verification-strength" => ObservableKind::VerificationStrength,
                        "canonicality-strength" => ObservableKind::CanonicalityStrength,
                        "compression-mode" => ObservableKind::CompressionMode,
                        "epistemic-distance" => ObservableKind::EpistemicDistance,
                        _ => ObservableKind::Custom,
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

    Ok(Observable { name, kind })
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
    /// A strength grade (formatted as its Display)
    Grade(String),
    /// A numeric distance
    Distance(u32),
    /// A boolean
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
        assert_eq!(p.verify, VerificationStrength::Sound);
        assert_eq!(p.canonicalize, CanonicalityStrength::Confluent);
        assert_eq!(p.compress, CompressionMode::Lossless);
    }

    #[test]
    fn parse_profile_full_syntax() {
        let input = r#"[
            :discover [:strength semi-decidable]
            :verify [:strength sound-complete]
            :canonicalize [:strength unique-nf]
            :compress [:strength codegen]
        ]"#;
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let p = parse_epistemic_profile(items).unwrap();
        assert_eq!(p.discover, DiscoveryStrength::SemiDecidable);
        assert_eq!(p.verify, VerificationStrength::SoundComplete);
        assert_eq!(p.canonicalize, CanonicalityStrength::UniqueNf);
        assert_eq!(p.compress, CompressionMode::Codegen);
    }

    #[test]
    fn parse_profile_with_capability() {
        let input = "[:discover [:capability yes :strength heuristic] :verify sound]";
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let p = parse_epistemic_profile(items).unwrap();
        assert_eq!(p.discover, DiscoveryStrength::Heuristic);
        assert_eq!(p.verify, VerificationStrength::Sound);
    }

    #[test]
    fn epistemic_distance() {
        let a = EpistemicProfile {
            discover: DiscoveryStrength::Complete,     // 4
            verify: VerificationStrength::Sound,        // 2
            canonicalize: CanonicalityStrength::None,   // 0
            compress: CompressionMode::Lossless,
        };
        let b = EpistemicProfile {
            discover: DiscoveryStrength::Heuristic,     // 1
            verify: VerificationStrength::Decidable,    // 4
            canonicalize: CanonicalityStrength::Confluent, // 3
            compress: CompressionMode::Codegen,         // different
        };
        // |4-1| + |2-4| + |0-3| + 1(different compress) = 3 + 2 + 3 + 1 = 9
        assert_eq!(a.distance(&b), 9);
    }

    #[test]
    fn epistemic_distance_same_compress() {
        let a = EpistemicProfile {
            discover: DiscoveryStrength::Complete,
            verify: VerificationStrength::Sound,
            canonicalize: CanonicalityStrength::None,
            compress: CompressionMode::Lossless,
        };
        let b = EpistemicProfile {
            discover: DiscoveryStrength::Heuristic,
            verify: VerificationStrength::Decidable,
            canonicalize: CanonicalityStrength::Confluent,
            compress: CompressionMode::Lossless,         // same
        };
        // 3 + 2 + 3 + 0 = 8
        assert_eq!(a.distance(&b), 8);
    }

    #[test]
    fn dominance() {
        let strong = EpistemicProfile {
            discover: DiscoveryStrength::Complete,
            verify: VerificationStrength::Decidable,
            canonicalize: CanonicalityStrength::UniqueNf,
            compress: CompressionMode::Codegen,
        };
        let weak = EpistemicProfile::default();
        assert!(strong.dominates(&weak));
        assert!(!weak.dominates(&strong));
    }

    #[test]
    fn incomparable_profiles() {
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
        assert!(!a.dominates(&b));
        assert!(!b.dominates(&a));
    }

    #[test]
    fn trivial_is_default() {
        assert_eq!(EpistemicProfile::trivial(), EpistemicProfile::default());
    }

    #[test]
    fn none_strength_means_no_capability() {
        let p = EpistemicProfile {
            discover: DiscoveryStrength::None,
            verify: VerificationStrength::None,
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
    fn compression_modes_are_non_ordinal() {
        // Lossless and Quotient are qualitatively different, not ranked
        assert_ne!(CompressionMode::Lossless, CompressionMode::Quotient);
        // But both are != None
        assert_ne!(CompressionMode::Lossless, CompressionMode::None);
    }
}
