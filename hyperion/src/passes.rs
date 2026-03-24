//! Compilation pass implementations.
//!
//! Each pass transforms Sexp ASTs (rules, terms) before they are handed
//! to the Apeiron e-graph or VonNeumann runtime. Passes are pure
//! AST→AST functions — no external processes, no new engines.

pub mod ac_normalize;
pub mod logic_engine;
pub mod smt_bridge;
