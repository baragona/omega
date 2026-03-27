//! # Archon
//!
//! A topological field theory engine that extends Apeiron's interaction nets
//! with regions, membranes, gauge fields, and thermodynamic annealing.
//!
//! Hyperion's 21 compilation passes become emergent boundary physics:
//! graphs physically deform as they cross membrane boundaries between
//! regions with different local physics.

pub mod region;
pub mod extended_arena;
pub mod boundary;
pub mod radiation;
pub mod kripke;
pub mod crystallize;
pub mod thermo;
pub mod antimatter;
pub mod physics;
pub mod observer;
pub mod superposition;
pub mod saturation;
pub mod implant;
pub mod readback;
