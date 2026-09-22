//! Rendering of a validated [`MigrationPlan`] into reviewable `SurrealQL`.
//!
//! Rendering is deliberately last in the pipeline. [`crate::generate`] has
//! already checked every field contract, so this module can treat a failed
//! target contract as an invariant violation while concentrating on target
//! quoting, assertions, and operation grouping.

use std::fmt::Write;

use crate::{Consequence, MigrationPlan, Operation, SchemaMode, WarningKind, target};

pub(crate) fn script(plan: &MigrationPlan) -> String {
    // Keep the script's phase boundaries visible to reviewers. The plan is
    // already ordered; this pass only inserts spacing and target syntax.
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
                table,
                name,
                ty,
                element_depth,
                ..
            }
            | Operation::AlterField {
                table,
                name,
                ty,
                element_depth,
                ..
            } => {
                let altering = matches!(operation, Operation::AlterField { .. });
                let verb = if altering { "ALTER" } else { "DEFINE" };
                let contract = target::field_contract(ty)
                    .expect("generation validates every target contract before planning");
                write!(
                    output,
                    "{verb} FIELD {} ON TABLE {} TYPE {}",
                    field_path(name, *element_depth),
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
            Operation::RemoveField {
                table,
                name,
                element_depth,
            } => {
                writeln!(
                    output,
                    "REMOVE FIELD {} ON TABLE {};",
                    field_path(name, *element_depth),
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
    // Warnings are comments so the script remains executable while carrying
    // the plan's data-loss and data-invalidation review record with it.
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

fn field_path(name: &str, element_depth: usize) -> String {
    format!(
        "{}{}",
        target::identifier(name),
        "[*]".repeat(element_depth)
    )
}
