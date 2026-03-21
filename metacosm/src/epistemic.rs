use apeiron::parser::Sexp;

use crate::error::{MetacosmError, Result};

/// Epistemic capacity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Capacity {
    None = 0,
    Low = 1,
    Medium = 2,
    High = 3,
}

impl Default for Capacity {
    fn default() -> Self {
        Capacity::Medium
    }
}

impl std::fmt::Display for Capacity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Capacity::None => write!(f, "none"),
            Capacity::Low => write!(f, "low"),
            Capacity::Medium => write!(f, "medium"),
            Capacity::High => write!(f, "high"),
        }
    }
}

fn parse_capacity(s: &str) -> Result<Capacity> {
    match s {
        "none" => Ok(Capacity::None),
        "low" => Ok(Capacity::Low),
        "medium" => Ok(Capacity::Medium),
        "high" => Ok(Capacity::High),
        _ => Err(MetacosmError::ParseError {
            block: "EpistemicProfile".into(),
            detail: format!("unknown capacity: '{}' (expected none/low/medium/high)", s),
        }),
    }
}

/// The epistemic signature of a world: what it can discover, verify, normalize, transport, compress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpistemicProfile {
    pub discovery: Capacity,
    pub verification: Capacity,
    pub canonicality: Capacity,
    pub transportability: Capacity,
    pub compression: Capacity,
}

impl Default for EpistemicProfile {
    fn default() -> Self {
        EpistemicProfile {
            discovery: Capacity::Medium,
            verification: Capacity::Medium,
            canonicality: Capacity::Medium,
            transportability: Capacity::Medium,
            compression: Capacity::Medium,
        }
    }
}

impl EpistemicProfile {
    /// The trivial profile: everything medium. Used for Omega/Hyperion embedding
    /// where epistemic structure is not specified.
    pub fn trivial() -> Self {
        Self::default()
    }

    pub fn can_discover(&self) -> bool {
        self.discovery > Capacity::None
    }

    pub fn can_verify(&self) -> bool {
        self.verification > Capacity::None
    }

    pub fn can_transport(&self) -> bool {
        self.transportability > Capacity::None
    }

    /// Epistemic distance: sum of per-axis absolute differences.
    pub fn distance(&self, other: &EpistemicProfile) -> u32 {
        fn diff(a: Capacity, b: Capacity) -> u32 {
            (a as i32 - b as i32).unsigned_abs()
        }
        diff(self.discovery, other.discovery)
            + diff(self.verification, other.verification)
            + diff(self.canonicality, other.canonicality)
            + diff(self.transportability, other.transportability)
            + diff(self.compression, other.compression)
    }

    /// Does this profile dominate another? (>= on all axes)
    pub fn dominates(&self, other: &EpistemicProfile) -> bool {
        self.discovery >= other.discovery
            && self.verification >= other.verification
            && self.canonicality >= other.canonicality
            && self.transportability >= other.transportability
            && self.compression >= other.compression
    }
}

/// Parse `(:discovery high :verification medium ...)`.
pub fn parse_epistemic_profile(items: &[Sexp]) -> Result<EpistemicProfile> {
    let mut profile = EpistemicProfile::default();

    let mut i = 0;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":discovery" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    profile.discovery = parse_capacity(v)?;
                }
            }
            ":verification" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    profile.verification = parse_capacity(v)?;
                }
            }
            ":canonicality" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    profile.canonicality = parse_capacity(v)?;
                }
            }
            ":transportability" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    profile.transportability = parse_capacity(v)?;
                }
            }
            ":compression" => {
                i += 1;
                if let Some(v) = items.get(i).and_then(|s| s.as_atom()) {
                    profile.compression = parse_capacity(v)?;
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

/// A named observable: a measurable epistemic quantity.
#[derive(Debug, Clone)]
pub struct Observable {
    pub name: String,
    pub kind: ObservableKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservableKind {
    DiscoveryCost,
    VerificationCost,
    TransportCost,
    Canonicality,
    Compression,
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
                        "discovery-cost" => ObservableKind::DiscoveryCost,
                        "verification-cost" => ObservableKind::VerificationCost,
                        "transport-cost" => ObservableKind::TransportCost,
                        "canonicality" => ObservableKind::Canonicality,
                        "compression" => ObservableKind::Compression,
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
    Capacity(Capacity),
    Cost(u64),
    Boolean(bool),
}

impl std::fmt::Display for MeasureValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MeasureValue::Capacity(c) => write!(f, "{}", c),
            MeasureValue::Cost(n) => write!(f, "{}", n),
            MeasureValue::Boolean(b) => write!(f, "{}", b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apeiron::parser::parse;

    #[test]
    fn parse_profile() {
        let input = "[:discovery high :verification low :canonicality none :transportability medium :compression high]";
        let sexps = parse(input).unwrap();
        let items = sexps[0].as_list().unwrap();
        let p = parse_epistemic_profile(items).unwrap();
        assert_eq!(p.discovery, Capacity::High);
        assert_eq!(p.verification, Capacity::Low);
        assert_eq!(p.canonicality, Capacity::None);
        assert_eq!(p.transportability, Capacity::Medium);
        assert_eq!(p.compression, Capacity::High);
    }

    #[test]
    fn epistemic_distance() {
        let a = EpistemicProfile {
            discovery: Capacity::High,
            verification: Capacity::High,
            canonicality: Capacity::Low,
            transportability: Capacity::Medium,
            compression: Capacity::None,
        };
        let b = EpistemicProfile::default(); // all medium
        assert_eq!(a.distance(&b), 1 + 1 + 1 + 0 + 2); // 5
    }

    #[test]
    fn dominance() {
        let high = EpistemicProfile {
            discovery: Capacity::High,
            verification: Capacity::High,
            canonicality: Capacity::High,
            transportability: Capacity::High,
            compression: Capacity::High,
        };
        let med = EpistemicProfile::default();
        assert!(high.dominates(&med));
        assert!(!med.dominates(&high));
    }

    #[test]
    fn trivial_is_default() {
        assert_eq!(EpistemicProfile::trivial(), EpistemicProfile::default());
    }
}
