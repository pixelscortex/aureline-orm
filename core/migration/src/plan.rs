use crate::{MigrationModel, MigrationType, SchemaMode};

/// A consequence of the expected schema transition, without inspecting stored data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consequence {
    DataLoss,
    DataInvalidation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarningKind {
    TableRemoved,
    FieldRemoved,
    FieldTypeChanged,
    RecordKeyChanged,
    SchemaMadeFull,
    RequiredFieldAdded,
}

/// Structured warning repeated in the script's review header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    pub consequence: Consequence,
    pub kind: WarningKind,
    pub table: String,
    pub field: Option<String>,
}

/// Semantic operation selected before target text is rendered.
///
/// Every mutation in this table slice has a faithful `ALTER` on the pinned
/// target. There is no speculative overwrite/recreation fallback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    DefineTable {
        name: String,
        schema_mode: SchemaMode,
    },
    AlterTable {
        name: String,
        schema_mode: SchemaMode,
    },
    DefineField {
        table: String,
        name: String,
        ty: MigrationType,
        record_key: bool,
    },
    AlterField {
        table: String,
        name: String,
        ty: MigrationType,
        record_key: bool,
    },
    RemoveField {
        table: String,
        name: String,
    },
    RemoveTable {
        name: String,
    },
}

/// Dependency-safe phases, with case-sensitive identity order inside each phase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrationPlan {
    operations: Vec<Operation>,
    warnings: Vec<Warning>,
}

impl MigrationPlan {
    #[must_use]
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    #[must_use]
    pub fn warnings(&self) -> &[Warning] {
        &self.warnings
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    pub(crate) fn compare(previous: &MigrationModel, current: &MigrationModel) -> Self {
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
                            ty: field.ty.clone(),
                            record_key: field.record_key,
                        });
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
        // Remove fields only on surviving tables: REMOVE TABLE owns its fields.
        // Changing surviving record-link contracts precedes removal of their old targets.
        for (table_name, table) in &previous.tables {
            if let Some(current_table) = current.tables.get(table_name) {
                for (name, field) in &table.fields {
                    if !current_table.fields.contains_key(name) {
                        plan.operations.push(Operation::RemoveField {
                            table: table_name.clone(),
                            name: name.clone(),
                        });
                        plan.warn(
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
                plan.operations
                    .push(Operation::RemoveTable { name: name.clone() });
                plan.warn(name, None, WarningKind::TableRemoved, Consequence::DataLoss);
            }
        }
        plan
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
