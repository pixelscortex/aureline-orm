//! Static semantic results and recovery mechanics.
//!
//! The checker keeps typed phase-local finding values in source and check
//! order. This module deliberately stops at that seam: rendering those values
//! into consumer-facing diagnostics belongs to a later phase.

mod analysis;
mod collections;
mod finding;
mod index;
mod records;
mod reporting;
mod resolver;
mod semantic_type;
mod unions;

#[cfg(feature = "contract-serde")]
mod contract_serde;

pub use analysis::{Analysis, CheckedProgram};
pub use finding::{ArgumentRole, Finding, UnsupportedTypeSyntaxKind};
pub use index::{FieldResolution, ResolutionIndex, TableResolution};
pub use reporting::{Findings, Reported, TypeResolution};
pub use semantic_type::{FieldPresence, RecordTargets, SemanticType};

/// Collects declarations, judges duplicates, and resolves every field type.
#[must_use]
pub fn check(ast: &aureline_ast::ast::Ast) -> Analysis<'_> {
    analysis::run(ast)
}
