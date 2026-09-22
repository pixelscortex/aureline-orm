//! Schema diffing for the migration model.
//!
//! The planner turns two normalized models into reviewable semantic operations
//! and warnings. It decides what changed and in which dependency-safe order;
//! [`crate::render`] owns the target syntax. This separation keeps target text
//! out of comparison logic and makes unsafe consequences visible before a
//! caller writes a migration file.

use crate::{MigrationModel, MigrationType, SchemaMode, target};

/// A consequence of the expected schema transition, without inspecting stored data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consequence {
    /// The operation removes persisted records.
    DataLoss,
    /// Existing records may remain, but future validation or identity checks
    /// can reject data that was valid under the previous schema.
    DataInvalidation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarningKind {
    /// A table and all of its records will be removed.
    TableRemoved,
    /// A field definition is removed while its stored values remain.
    FieldRemoved,
    /// A field's accepted value contract changes.
    FieldTypeChanged,
    /// A field's role as the table record key changes.
    RecordKeyChanged,
    /// A table changes from schemaless to schemafull mode.
    SchemaMadeFull,
    /// A new field does not admit `none` and may reject existing records.
    RequiredFieldAdded,
}

/// Structured warning repeated in the script's review header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    /// What a caller should expect if the generated operation is applied.
    pub consequence: Consequence,
    /// The schema transition that caused this warning.
    pub kind: WarningKind,
    /// The affected table's exact database name.
    pub table: String,
    /// The affected field, or `None` for a table-wide transition.
    pub field: Option<String>,
}

/// Semantic operation selected before target text is rendered.
///
/// Every mutation in this table slice has a faithful `ALTER` on the pinned
/// target. There is no speculative overwrite/recreation fallback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    /// Creates a table before any fields are defined on it.
    DefineTable {
        /// The table's exact database name.
        name: String,
        /// The table mode after the operation.
        schema_mode: SchemaMode,
    },
    /// Changes an existing table's schema strictness.
    AlterTable {
        /// The table's exact database name.
        name: String,
        /// The table mode after the operation.
        schema_mode: SchemaMode,
    },
    /// Defines a field or one of the wildcard descendants synthesized by a
    /// collection-valued parent.
    DefineField {
        /// The owning table's exact database name.
        table: String,
        /// The declared field name, before wildcard suffixes.
        name: String,
        /// Zero is the declared field; each additional level appends `[*]`.
        element_depth: usize,
        /// The value contract to render for this path.
        ty: MigrationType,
        /// Whether the declared path is the table's record key.
        record_key: bool,
    },
    /// Changes a field or an already-defined wildcard descendant.
    AlterField {
        /// The owning table's exact database name.
        table: String,
        /// The declared field name, before wildcard suffixes.
        name: String,
        /// Zero is the declared field; each additional level appends `[*]`.
        element_depth: usize,
        /// The value contract to render for this path.
        ty: MigrationType,
        /// Whether the declared path is the table's record key.
        record_key: bool,
    },
    /// Removes a field or wildcard descendant from a surviving table.
    RemoveField {
        /// The owning table's exact database name.
        table: String,
        /// The declared field name, before wildcard suffixes.
        name: String,
        /// Zero is the declared field; each additional level appends `[*]`.
        element_depth: usize,
    },
    /// Removes a table and therefore all of its fields and records.
    RemoveTable { name: String },
}

/// Dependency-safe phases, with case-sensitive identity order inside each phase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrationPlan {
    operations: Vec<Operation>,
    warnings: Vec<Warning>,
}

impl MigrationPlan {
    /// Returns operations in execution order: tables, fields, descendants,
    /// then removals.
    #[must_use]
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    /// Returns warnings in the same identity order used by the diff.
    #[must_use]
    pub fn warnings(&self) -> &[Warning] {
        &self.warnings
    }

    /// Reports whether applying this plan would change the schema.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    pub(crate) fn compare(previous: &MigrationModel, current: &MigrationModel) -> Self {
        // Tables and fields are BTreeMaps, so each phase is deterministic. The
        // explicit phases also preserve target dependencies: a parent exists
        // before its fields, and descendants are removed before their parent.
        let mut plan = Self {
            operations: Vec::new(),
            warnings: Vec::new(),
        };
        plan.tables(previous, current);
        for (table_name, table) in &current.tables {
            let old_table = previous.tables.get(table_name);
            for (name, field) in &table.fields {
                match old_table.and_then(|table| table.fields.get(name)) {
                    None => {
                        plan.operations.push(Operation::DefineField {
                            table: table_name.clone(),
                            name: name.clone(),
                            element_depth: 0,
                            ty: field.ty.clone(),
                            record_key: field.record_key,
                        });
                        if old_table.is_some() && (field.record_key || !admits_none(&field.ty)) {
                            plan.warn(
                                table_name,
                                Some(name),
                                if field.record_key {
                                    WarningKind::RecordKeyChanged
                                } else {
                                    WarningKind::RequiredFieldAdded
                                },
                                Consequence::DataInvalidation,
                            );
                        }
                    }
                    Some(old) if old != field => {
                        plan.operations.push(Operation::AlterField {
                            table: table_name.clone(),
                            name: name.clone(),
                            element_depth: 0,
                            ty: field.ty.clone(),
                            record_key: field.record_key,
                        });
                        plan.element_changes(table_name, name, &old.ty, &field.ty);
                        plan.warn(
                            table_name,
                            Some(name),
                            if field.record_key {
                                WarningKind::RecordKeyChanged
                            } else {
                                WarningKind::FieldTypeChanged
                            },
                            Consequence::DataInvalidation,
                        );
                    }
                    Some(_) => {}
                }
            }
        }
        plan.removals(previous, current);
        plan
    }

    fn removals(&mut self, previous: &MigrationModel, current: &MigrationModel) {
        // Remove fields only on surviving tables: REMOVE TABLE owns its fields.
        // Changing surviving record-link contracts precedes removal of their old targets.
        for (table_name, table) in &previous.tables {
            if let Some(current_table) = current.tables.get(table_name) {
                for (name, field) in &table.fields {
                    if !current_table.fields.contains_key(name) {
                        self.remove_elements(
                            table_name,
                            name,
                            0,
                            target::implicit_fields(&field.ty).len(),
                        );
                        self.warn(
                            table_name,
                            Some(name),
                            if field.record_key {
                                WarningKind::RecordKeyChanged
                            } else {
                                WarningKind::FieldRemoved
                            },
                            Consequence::DataInvalidation,
                        );
                    }
                }
            }
        }
        for name in previous.tables.keys() {
            if !current.tables.contains_key(name) {
                self.operations
                    .push(Operation::RemoveTable { name: name.clone() });
                self.warn(name, None, WarningKind::TableRemoved, Consequence::DataLoss);
            }
        }
    }

    fn tables(&mut self, previous: &MigrationModel, current: &MigrationModel) {
        for (name, table) in &current.tables {
            match previous.tables.get(name) {
                None => self.operations.push(Operation::DefineTable {
                    name: name.clone(),
                    schema_mode: table.schema_mode,
                }),
                Some(old) if old.schema_mode != table.schema_mode => {
                    self.operations.push(Operation::AlterTable {
                        name: name.clone(),
                        schema_mode: table.schema_mode,
                    });
                    if table.schema_mode == SchemaMode::Schemafull {
                        self.warn(
                            name,
                            None,
                            WarningKind::SchemaMadeFull,
                            Consequence::DataInvalidation,
                        );
                    }
                }
                Some(_) => {}
            }
        }
    }

    fn element_changes(
        &mut self,
        table: &str,
        name: &str,
        old: &MigrationType,
        new: &MigrationType,
    ) {
        let old_elements = target::implicit_fields(old);
        let new_elements = target::implicit_fields(new);
        for (index, ty) in new_elements.iter().enumerate() {
            if old_elements.get(index) == Some(ty) {
                continue;
            }
            let element_depth = index + 1;
            if index >= old_elements.len() {
                // DEFINE with the final collection type would itself synthesize
                // descendants. Seed just this path with any, then ALTER it, so
                // every derived definition has one explicit planned operation.
                self.operations.push(Operation::DefineField {
                    table: table.to_owned(),
                    name: name.to_owned(),
                    element_depth,
                    ty: MigrationType::Scalar("any".into()),
                    record_key: false,
                });
            }
            self.operations.push(Operation::AlterField {
                table: table.to_owned(),
                name: name.to_owned(),
                element_depth,
                ty: ty.clone(),
                record_key: false,
            });
        }
        if old_elements.len() > new_elements.len() {
            self.remove_elements(table, name, new_elements.len() + 1, old_elements.len());
        }
    }

    fn remove_elements(&mut self, table: &str, name: &str, minimum: usize, maximum: usize) {
        for element_depth in (minimum..=maximum).rev() {
            self.operations.push(Operation::RemoveField {
                table: table.to_owned(),
                name: name.to_owned(),
                element_depth,
            });
        }
    }

    fn warn(
        &mut self,
        table: &str,
        field: Option<&str>,
        kind: WarningKind,
        consequence: Consequence,
    ) {
        self.warnings.push(Warning {
            consequence,
            kind,
            table: table.to_owned(),
            field: field.map(str::to_owned),
        });
    }
}

fn admits_none(ty: &MigrationType) -> bool {
    match ty {
        MigrationType::Scalar(name) => matches!(name.as_str(), "any" | "none"),
        MigrationType::Union(members) => members.iter().any(admits_none),
        _ => false,
    }
}
