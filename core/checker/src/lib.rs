//! Static checking of table-schema declarations without database execution.
//!
//! [`check`] borrows the immutable source AST, collects a lossless name index,
//! and reports duplicate tables, duplicate fields, type failures, then record-key
//! failures in that fixed order. Each check follows source-registration order;
//! Findings are never sorted or deduplicated after emission.
//!
//! [`Analysis::into_checked`] is the generation gate. It exposes a
//! [`CheckedProgram`] only when every obligation succeeds, retaining the rejected
//! analysis otherwise. Consumers query resolved facts; diagnostic presentation
//! and target-specific generation remain separate phases.

mod analysis;
mod collections;
mod finding;
mod id_contract;
mod index;
mod records;
mod reporting;
mod resolver;
mod semantic_type;
mod type_display;
mod unions;

pub use analysis::{Analysis, CheckedProgram};
pub use finding::{ArgumentRole, Finding};
pub use id_contract::{IdContract, RecordKeyType};
pub use index::{FieldResolution, ResolutionIndex, TableResolution};
pub use reporting::{Findings, Reported, TypeResolution};
pub use semantic_type::{FieldPresence, RecordTargets, SemanticType};

/// Checks all table-schema obligations and retains ordered typed Findings.
#[must_use]
pub fn check(ast: &aureline_ast::ast::Ast) -> Analysis<'_> {
    analysis::run(ast)
}
