//! Offline schema generation for `SurrealDB` 3.2.0.
//!
//! [`generate`] lowers a generation-safe checked program, compares it with the
//! last generated [`Snapshot`], and renders a classified plan. It never connects
//! to a database. The caller owns folder naming and the indivisible write of
//! `migration.surql` plus `snapshot.json` (CLI work in #53).

mod model;
mod plan;
mod render;
mod snapshot;
mod target;

pub use model::{Field, MigrationModel, MigrationType, SchemaMode, Table};
pub use plan::{Consequence, MigrationPlan, Operation, Warning, WarningKind};
pub use snapshot::{Snapshot, SnapshotError};
pub use target::TargetError;

/// A field contract that cannot be faithfully enforced on the pinned target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationError {
    pub table: String,
    pub field: String,
    pub reason: TargetError,
}

#[derive(Clone, Debug)]
pub struct Generation {
    pub plan: MigrationPlan,
    pub script: String,
    pub snapshot: Snapshot,
}

/// Generates reviewable artifacts, including unsafe but representable changes.
///
/// An empty plan/script means no migration should be written. Snapshot identity
/// is nondeterministic; model order and rendered scripts are deterministic.
///
/// # Errors
/// Returns all unsupported target contracts in identity order, without any
/// partial migration. Prior snapshots are validated by [`Snapshot::from_json`].
pub fn generate(
    checked: &aureline_checker::CheckedProgram<'_>,
    previous: Option<&Snapshot>,
) -> Result<Generation, Vec<GenerationError>> {
    let current = MigrationModel::lower(checked);
    let mut errors = Vec::new();
    for (table_name, table) in &current.tables {
        for (field_name, field) in &table.fields {
            if let Err(reason) = target::field_contract(&field.ty) {
                errors.push(GenerationError {
                    table: table_name.clone(),
                    field: field_name.clone(),
                    reason,
                });
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let empty = MigrationModel::default();
    let plan = MigrationPlan::compare(previous.map_or(&empty, Snapshot::model), &current);
    let script = render::script(&plan);
    let snapshot = if plan.is_empty() {
        previous
            .cloned()
            .unwrap_or_else(|| Snapshot::new(current.clone(), None))
    } else {
        Snapshot::new(current, previous)
    };
    Ok(Generation {
        plan,
        script,
        snapshot,
    })
}
