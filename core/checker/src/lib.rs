//! Static semantic results and recovery mechanics.
//!
//! The checker keeps typed phase-local finding values in source and check
//! order. This module deliberately stops at that seam: rendering those values
//! into consumer-facing diagnostics belongs to a later phase.

mod reporting;

pub use reporting::{Findings, Reported, TypeResolution};
