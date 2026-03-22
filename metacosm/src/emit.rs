//! Cosmological Emit: execute an Omega term through a Metacosm pipeline,
//! producing the normalized result wrapped in an epistemic receipt.
//!
//! Syntax:
//!   [Emit GoldenArtifact
//!     :term [plus [s [s z]] [s [s z]]]
//!     :theory PeanoArithmetic
//!     :pipeline GoldenPath
//!     :format epistemic-receipt
//!   ]
//!
//! The engine pushes the term through the pipeline:
//!   1. Normalize in the source world's substrate (Omega → Apeiron)
//!   2. Track the epistemic journey (Materialize)
//!   3. Produce the result + receipt

use apeiron::parser::Sexp;
use crate::error::{MetacosmError, Result};

/// A parsed Emit declaration.
#[derive(Debug, Clone)]
pub struct EmitDecl {
    pub name: String,
    /// The raw term S-expression to normalize.
    pub term: Sexp,
    /// The theory in which to normalize.
    pub theory: String,
    /// The pipeline to execute through.
    pub pipeline: String,
    /// Output format.
    pub format: EmitFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmitFormat {
    /// Full epistemic receipt: payload + journey + invariants + cost.
    EpistemicReceipt,
    /// Just the normalized term.
    Term,
}

/// The result of a cosmological emit.
#[derive(Debug, Clone)]
pub struct EmitResult {
    pub name: String,
    /// The original term (as string).
    pub input: String,
    /// The normalized term (as string).
    pub output: String,
    /// Number of interactions (rewrite steps) consumed.
    pub interactions: u64,
    /// The theory used for normalization.
    pub theory: String,
    /// The materialization result (epistemic journey).
    pub journey: crate::materialize::MaterializeResult,
    /// Empirical measurements collected during execution.
    pub cost: EmitCost,
}

/// Cost metrics from the emit execution.
#[derive(Debug, Clone)]
pub struct EmitCost {
    pub interactions: u64,
    pub term_size_input: usize,
    pub term_size_output: usize,
}

impl std::fmt::Display for EmitResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== EPISTEMIC RECEIPT ===")?;
        writeln!(f)?;
        writeln!(f, "--- Payload ---")?;
        writeln!(f, "  input:  {}", self.input)?;
        writeln!(f, "  output: {}", self.output)?;
        writeln!(f, "  theory: {}", self.theory)?;
        writeln!(f)?;
        writeln!(f, "--- Journey ---")?;
        for (i, step) in self.journey.steps.iter().enumerate() {
            writeln!(f, "  step {}: {}", i + 1, step)?;
        }
        writeln!(f)?;
        writeln!(f, "--- Invariants ---")?;
        if !self.journey.preserved.is_empty() {
            writeln!(f, "  preserved: [{}]",
                self.journey.preserved.iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
        }
        if !self.journey.total_degradation.is_empty() {
            writeln!(f, "  lost:      [{}]",
                self.journey.total_degradation.iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
        }
        writeln!(f, "  distance:  {}", self.journey.total_distance)?;
        writeln!(f)?;
        writeln!(f, "--- Cost ---")?;
        writeln!(f, "  interactions:    {}", self.cost.interactions)?;
        writeln!(f, "  term size (in):  {}", self.cost.term_size_input)?;
        writeln!(f, "  term size (out): {}", self.cost.term_size_output)?;
        writeln!(f, "=========================")
    }
}

/// Parse an `[Emit Name :term EXPR :theory T :pipeline P :format F]` block.
pub fn parse_emit(items: &[Sexp]) -> Result<EmitDecl> {
    if items.len() < 2 {
        return Err(MetacosmError::ParseError {
            block: "Emit".into(),
            detail: "missing emit name".into(),
        });
    }

    let name = items[1].as_atom().ok_or_else(|| MetacosmError::ParseError {
        block: "Emit".into(),
        detail: "emit name must be an atom".into(),
    })?.to_string();

    let mut term: Option<Sexp> = None;
    let mut theory: Option<String> = None;
    let mut pipeline: Option<String> = None;
    let mut format = EmitFormat::EpistemicReceipt;

    let mut i = 2;
    while i < items.len() {
        let key = items[i].as_atom().unwrap_or("");
        match key {
            ":term" => {
                i += 1;
                if i < items.len() {
                    term = Some(items[i].clone());
                }
            }
            ":theory" => {
                i += 1;
                theory = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
            }
            ":pipeline" => {
                i += 1;
                pipeline = items.get(i).and_then(|s| s.as_atom()).map(|s| s.to_string());
            }
            ":format" => {
                i += 1;
                if let Some(f) = items.get(i).and_then(|s| s.as_atom()) {
                    format = match f {
                        "epistemic-receipt" => EmitFormat::EpistemicReceipt,
                        "term" => EmitFormat::Term,
                        _ => return Err(MetacosmError::ParseError {
                            block: "Emit".into(),
                            detail: format!("unknown format: '{}' (expected epistemic-receipt or term)", f),
                        }),
                    };
                }
            }
            _ => {
                return Err(MetacosmError::ParseError {
                    block: "Emit".into(),
                    detail: format!("unknown keyword: {}", key),
                });
            }
        }
        i += 1;
    }

    let term = term.ok_or_else(|| MetacosmError::ParseError {
        block: "Emit".into(),
        detail: "missing :term".into(),
    })?;
    let theory = theory.ok_or_else(|| MetacosmError::ParseError {
        block: "Emit".into(),
        detail: "missing :theory".into(),
    })?;
    let pipeline = pipeline.ok_or_else(|| MetacosmError::ParseError {
        block: "Emit".into(),
        detail: "missing :pipeline".into(),
    })?;

    Ok(EmitDecl { name, term, theory, pipeline, format })
}

/// Count nodes in an S-expression (rough term size metric).
pub fn sexp_size(sexp: &Sexp) -> usize {
    match sexp {
        _ if sexp.as_atom().is_some() => 1,
        _ if sexp.as_list().is_some() => {
            1 + sexp.as_list().unwrap().iter().map(|s| sexp_size(s)).sum::<usize>()
        }
        _ => 1,
    }
}
