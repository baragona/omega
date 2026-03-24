//! Hyperion Runtime — bridges compiled programs to the host compiler.
//!
//! Provides opaque handle types for compiler objects (TheoryDef, Sort, etc.)
//! and the CompilerEffects trait for interacting with the host e-graph engine,
//! filesystem, and proof checker.
//!
//! Generated code from `hyperion kompile` on compiler-engine theories
//! depends on this crate.

pub mod types;
pub mod effects;
pub mod egraph;

pub use types::*;
pub use effects::*;
pub use egraph::*;
