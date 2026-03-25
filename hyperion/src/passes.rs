//! Compilation pass implementations.
//!
//! Each pass transforms Sexp ASTs (rules, terms) before they are handed
//! to the Apeiron e-graph or VonNeumann runtime. Passes are pure
//! AST→AST functions — no external processes, no new engines.

pub mod ac_normalize;
pub mod context_reify;
pub mod dialectica;
pub mod explicit_subst;
pub mod goal_directed;
pub mod hoas_defunc;
pub mod kan_compute;
pub mod logic_engine;
pub mod modal_restrict;
pub mod smt_bridge;
