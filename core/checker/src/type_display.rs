//! Canonical display of checked semantic types.
//!
//! This formatter renders the canonical semantic algebra for downstream
//! consumers. It formats semantic types rather than source syntax, so
//! whitespace and trailing commas disappear. Record targets are rendered
//! through their checked table identities to preserve declared spelling
//! without making names part of semantic type storage.

use crate::{CheckedProgram, RecordTargets, SemanticType};

/// Formats one checked semantic type using canonical `SurrealDB` type spelling.
pub(crate) fn format_type(checked: &CheckedProgram<'_>, ty: &SemanticType) -> String {
    match ty {
        SemanticType::Any => "any".to_owned(),
        SemanticType::Bool => "bool".to_owned(),
        SemanticType::Bytes => "bytes".to_owned(),
        SemanticType::Datetime => "datetime".to_owned(),
        SemanticType::Decimal => "decimal".to_owned(),
        SemanticType::Duration => "duration".to_owned(),
        SemanticType::Float => "float".to_owned(),
        SemanticType::Int => "int".to_owned(),
        SemanticType::Number => "number".to_owned(),
        SemanticType::Object => "object".to_owned(),
        SemanticType::Range => "range".to_owned(),
        SemanticType::String => "string".to_owned(),
        SemanticType::Uuid => "uuid".to_owned(),
        SemanticType::None => "none".to_owned(),
        SemanticType::Null => "null".to_owned(),
        SemanticType::Record(targets) => format_record(checked, targets),
        SemanticType::Array {
            element,
            exact_length,
        } => format_collection(checked, "array", element, *exact_length),
        SemanticType::Set {
            element,
            max_distinct,
        } => format_collection(checked, "set", element, *max_distinct),
        SemanticType::Union(members) => members
            .iter()
            .map(|member| format_type(checked, member))
            .collect::<Vec<_>>()
            .join(" | "),
        SemanticType::Tuple(members) => format_tuple(checked, members),
    }
}

fn format_record(checked: &CheckedProgram<'_>, targets: &RecordTargets) -> String {
    match targets {
        RecordTargets::Any => "record".to_owned(),
        RecordTargets::Tables(targets) => {
            let names = targets
                .iter()
                .map(|&table| {
                    checked
                        .table(table)
                        .expect("a checked record target belongs to its program")
                        .name()
                })
                .collect::<Vec<_>>()
                .join(" | ");
            format!("record<{names}>")
        }
    }
}

fn format_collection(
    checked: &CheckedProgram<'_>,
    constructor: &str,
    element: &SemanticType,
    size: Option<u64>,
) -> String {
    let element = format_type(checked, element);
    match size {
        Some(size) => format!("{constructor}<{element}, {size}>"),
        None => format!("{constructor}<{element}>"),
    }
}

fn format_tuple(checked: &CheckedProgram<'_>, members: &[SemanticType]) -> String {
    let members = members
        .iter()
        .map(|member| format_type(checked, member))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{members}]")
}
