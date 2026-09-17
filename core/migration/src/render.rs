use std::fmt::Write;

use crate::{Consequence, MigrationPlan, Operation, SchemaMode, WarningKind, target};

pub(crate) fn script(plan: &MigrationPlan) -> String {
    let mut output = String::new();
    warning_header(plan, &mut output);
    let mut previous_phase = None;
    for operation in plan.operations() {
        let phase = match operation {
            Operation::DefineTable { .. } | Operation::AlterTable { .. } => 0,
            Operation::DefineField { .. } | Operation::AlterField { .. } => 1,
            Operation::RemoveField { .. } => 2,
            Operation::RemoveTable { .. } => 3,
        };
        if previous_phase.is_some_and(|previous| previous != phase) {
            output.push('\n');
        }
        previous_phase = Some(phase);
        match operation {
            Operation::DefineTable { name, schema_mode } => {
                writeln!(
                    output,
                    "DEFINE TABLE {} TYPE NORMAL {};",
                    target::identifier(name),
                    mode(*schema_mode)
                )
                .unwrap();
            }
            Operation::AlterTable { name, schema_mode } => {
                writeln!(
                    output,
                    "ALTER TABLE {} {};",
                    target::identifier(name),
                    mode(*schema_mode)
                )
                .unwrap();
            }
            Operation::DefineField {
                table, name, ty, ..
            }
            | Operation::AlterField {
                table, name, ty, ..
            } => {
                let altering = matches!(operation, Operation::AlterField { .. });
                let verb = if altering { "ALTER" } else { "DEFINE" };
                let contract = target::field_contract(ty)
                    .expect("generation validates every target contract before planning");
                write!(
                    output,
                    "{verb} FIELD {} ON TABLE {} TYPE {}",
                    target::identifier(name),
                    target::identifier(table),
                    contract.ty
                )
                .unwrap();
                if let Some(assertion) = contract.assertion {
                    write!(output, " ASSERT {assertion}").unwrap();
                } else if altering {
                    // A previous bounded-set contract may have installed an assertion.
                    // This slice owns the full field contract and has no user attributes.
                    output.push_str(" DROP ASSERT");
                }
                output.push_str(";\n");
            }
            Operation::RemoveField { table, name } => {
                writeln!(
                    output,
                    "REMOVE FIELD {} ON TABLE {};",
                    target::identifier(name),
                    target::identifier(table)
                )
                .unwrap();
            }
            Operation::RemoveTable { name } => {
                writeln!(output, "REMOVE TABLE {};", target::identifier(name)).unwrap();
            }
        }
    }
    output
}

fn mode(mode: SchemaMode) -> &'static str {
    match mode {
        SchemaMode::Schemafull => "SCHEMAFULL",
        SchemaMode::Schemaless => "SCHEMALESS",
    }
}

fn warning_header(plan: &MigrationPlan, output: &mut String) {
    for warning in plan.warnings() {
        let consequence = match warning.consequence {
            Consequence::DataLoss => "DATA LOSS",
            Consequence::DataInvalidation => "DATA INVALIDATION",
        };
        let subject = warning.field.as_ref().map_or_else(
            || target::identifier(&warning.table),
            |field| {
                format!(
                    "{}.{}",
                    target::identifier(&warning.table),
                    target::identifier(field)
                )
            },
        );
        let message = match warning.kind {
            WarningKind::TableRemoved => "removing the table deletes its records",
            WarningKind::FieldRemoved => {
                "removing the field definition retains values but may invalidate later writes"
            }
            WarningKind::FieldTypeChanged => {
                "changing the field type may invalidate existing values"
            }
            WarningKind::RecordKeyChanged => {
                "changing the record-key contract may invalidate existing identities"
            }
            WarningKind::SchemaMadeFull => {
                "schemafull mode may invalidate writes containing undeclared fields"
            }
            WarningKind::RequiredFieldAdded => {
                "adding a required field may invalidate existing records"
            }
        };
        writeln!(output, "-- WARNING [{consequence}] {subject}: {message}.").unwrap();
    }
    if !plan.warnings().is_empty() {
        output.push('\n');
    }
}
