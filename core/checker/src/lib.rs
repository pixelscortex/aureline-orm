//! Static semantic results and recovery mechanics.
//!
//! The checker keeps typed phase-local finding values in source and check
//! order. This module deliberately stops at that seam: rendering those values
//! into consumer-facing diagnostics belongs to a later phase.

mod finding;
mod index;
mod reporting;

#[cfg(feature = "contract-serde")]
mod contract_serde;

pub use finding::Finding;
pub use index::{FieldResolution, ResolutionIndex, TableResolution, check};
pub use reporting::{Findings, Reported, TypeResolution};
