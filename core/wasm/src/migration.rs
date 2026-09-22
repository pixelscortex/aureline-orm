use aureline_migration::{Consequence, Snapshot, Warning, WarningKind};
use aureline_parser::SyntaxProblem;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Debug, Serialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum MigrationDetails {
    Generated {
        script: String,
        snapshot: String,
        warnings: Vec<MigrationWarning>,
        operation_count: usize,
    },
    Unchanged,
    Invalid {
        phase: &'static str,
        messages: Vec<String>,
    },
}

#[derive(Debug, Serialize)]
struct MigrationWarning {
    kind: &'static str,
    consequence: &'static str,
    table: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<String>,
}

/// Generates an offline migration from source and the last generated snapshot JSON.
///
/// The caller retains the returned snapshot only for `generated` results. An
/// `unchanged` or `invalid` result supplies no replacement history state. No
/// database is contacted and this function owns no browser persistence.
///
/// # Errors
/// Returns a JavaScript string error only if the result cannot be serialized.
// wasm-bindgen cannot receive an optional borrowed string from JavaScript.
#[allow(clippy::needless_pass_by_value)]
#[wasm_bindgen]
pub fn generate_migration(
    source: &str,
    previous_snapshot: Option<String>,
) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&migration_details(source, previous_snapshot.as_deref()))
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

fn migration_details(source: &str, previous_snapshot: Option<&str>) -> MigrationDetails {
    let ast = match aureline_parser::parse(source) {
        Ok(ast) => ast,
        Err(problems) => {
            return MigrationDetails::Invalid {
                phase: "syntax",
                messages: problems.iter().map(syntax_message).collect(),
            };
        }
    };
    let checked = match aureline_checker::check(&ast).into_checked() {
        Ok(checked) => checked,
        Err(analysis) => {
            return MigrationDetails::Invalid {
                phase: "semantic",
                messages: analysis
                    .findings()
                    .iter()
                    .map(|finding| semantic_message(super::semantic_problem(&analysis, finding)))
                    .collect(),
            };
        }
    };
    let previous = match previous_snapshot.map(Snapshot::from_json).transpose() {
        Ok(previous) => previous,
        Err(error) => {
            return MigrationDetails::Invalid {
                phase: "snapshot",
                messages: vec![error.to_string()],
            };
        }
    };
    match aureline_migration::generate(&checked, previous.as_ref()) {
        Ok(generation) if generation.plan.is_empty() => MigrationDetails::Unchanged,
        Ok(generation) => MigrationDetails::Generated {
            script: generation.script,
            snapshot: generation.snapshot.to_json(),
            warnings: generation
                .plan
                .warnings()
                .iter()
                .map(migration_warning)
                .collect(),
            operation_count: generation.plan.operations().len(),
        },
        Err(errors) => MigrationDetails::Invalid {
            phase: "generation",
            messages: errors
                .into_iter()
                .map(|error| format!("{}.{}: {}", error.table, error.field, error.reason))
                .collect(),
        },
    }
}

fn migration_warning(warning: &Warning) -> MigrationWarning {
    MigrationWarning {
        kind: match warning.kind {
            WarningKind::TableRemoved => "tableRemoved",
            WarningKind::FieldRemoved => "fieldRemoved",
            WarningKind::FieldTypeChanged => "fieldTypeChanged",
            WarningKind::RecordKeyChanged => "recordKeyChanged",
            WarningKind::SchemaMadeFull => "schemaMadeFull",
            WarningKind::RequiredFieldAdded => "requiredFieldAdded",
        },
        consequence: match warning.consequence {
            Consequence::DataLoss => "dataLoss",
            Consequence::DataInvalidation => "dataInvalidation",
        },
        table: warning.table.clone(),
        field: warning.field.clone(),
    }
}

fn semantic_message(problem: super::SemanticProblem) -> String {
    use super::SemanticProblem;
    match problem {
        SemanticProblem::DuplicateTable { message, .. }
        | SemanticProblem::DuplicateField { message, .. }
        | SemanticProblem::UnknownType { message, .. }
        | SemanticProblem::UnsupportedType { message, .. }
        | SemanticProblem::BareTableType { message, .. }
        | SemanticProblem::WrongArity { message, .. }
        | SemanticProblem::MissingRecordTarget { message, .. }
        | SemanticProblem::AmbiguousRecordTarget { message, .. }
        | SemanticProblem::WrongArgumentRole { message, .. }
        | SemanticProblem::InvalidCollectionSize { message, .. }
        | SemanticProblem::InvalidRecordKey { message, .. } => message,
    }
}

fn syntax_message(problem: &SyntaxProblem) -> String {
    let (message, span) = match problem {
        SyntaxProblem::SourceTooLarge { byte_len } => {
            return format!("Source is too large ({byte_len} bytes)");
        }
        SyntaxProblem::InvalidToken { span } => ("Invalid token", span),
        SyntaxProblem::InvalidIdentifier { span, .. } => ("Invalid identifier", span),
        SyntaxProblem::EmptyTypeArguments { span } => ("Missing type arguments", span),
        SyntaxProblem::TrailingTypeArgumentComma { span } => {
            ("Trailing comma in type arguments", span)
        }
        SyntaxProblem::MissingUnionMember { span } => ("Missing union member", span),
        SyntaxProblem::MissingTupleMember { span } => ("Missing tuple member", span),
        SyntaxProblem::MissingTupleSeparator { span } => ("Missing tuple comma", span),
        SyntaxProblem::UnterminatedBlockComment { span } => ("Unterminated block comment", span),
        SyntaxProblem::UnexpectedToken { span } => ("Unexpected token", span),
    };
    format!("{message} at byte {}", span.range().start().get())
}

#[cfg(test)]
mod tests {
    use super::{MigrationDetails, migration_details};

    #[test]
    fn empty_source_has_no_migration_to_record() {
        assert!(matches!(
            migration_details("", None),
            MigrationDetails::Unchanged
        ));
    }

    #[test]
    fn invalid_generation_never_returns_replacement_history() {
        for (source, snapshot, expected_phase, expected_message) in [
            ("table", None, "syntax", "Unexpected token"),
            (
                "table T schemafull {\n value Unknown\n}",
                None,
                "semantic",
                "unknown type 'Unknown'",
            ),
            ("table T schemafull {}", Some("{}"), "snapshot", ""),
            (
                "table T schemafull {\n value set<int, 2> | string\n}",
                None,
                "generation",
                "sized-set enforcement",
            ),
        ] {
            let MigrationDetails::Invalid { phase, messages } = migration_details(source, snapshot)
            else {
                panic!("{expected_phase} failure must not return generation artifacts");
            };
            assert_eq!(phase, expected_phase);
            assert!(!messages.is_empty());
            assert!(messages[0].contains(expected_message), "{messages:?}");
        }
    }

    #[test]
    fn generate_records_only_changed_schema_and_compares_the_next_edit() {
        let source = "table User schemafull {\n name string\n}";
        let MigrationDetails::Generated {
            script,
            snapshot,
            operation_count,
            ..
        } = migration_details(source, None)
        else {
            panic!("first generation should record a snapshot");
        };
        assert!(script.contains("DEFINE TABLE"));
        assert_eq!(operation_count, 2);
        assert!(matches!(
            migration_details(source, Some(&snapshot)),
            MigrationDetails::Unchanged
        ));
        let MigrationDetails::Generated {
            script, warnings, ..
        } = migration_details("table User schemafull {\n name int\n}", Some(&snapshot))
        else {
            panic!("edited field should generate a migration");
        };
        assert!(script.contains("ALTER FIELD"));
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].kind, "fieldTypeChanged");
        assert_eq!(warnings[0].consequence, "dataInvalidation");
        assert_eq!(warnings[0].field.as_deref(), Some("name"));
    }
}
