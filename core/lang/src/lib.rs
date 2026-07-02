#![doc = "Facade over Aureline AST, parser, checker, and migration crates."]

pub use aureline_ast::{Field, Model, Schema, TypeRef};
pub use aureline_checker::{Diagnostic, check_schema};
pub use aureline_migration::preview_migration;
pub use aureline_parser::{ParseResult, parse_schema_name};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectResult {
    pub schema: Schema,
    pub diagnostics: Vec<Diagnostic>,
    pub migration_preview: String,
}

#[must_use]
pub fn inspect_schema_name(source: &str) -> InspectResult {
    let parsed = parse_schema_name(source);
    let diagnostics = check_schema(&parsed.schema);
    let migration_preview = preview_migration(&parsed.schema);

    InspectResult {
        schema: parsed.schema,
        diagnostics,
        migration_preview,
    }
}

#[cfg(test)]
mod tests {
    use super::inspect_schema_name;

    #[test]
    fn facade_runs_language_pipeline() {
        let result = inspect_schema_name("blog");

        assert_eq!(result.schema.name, "blog");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.migration_preview, "-- migration preview for blog");
    }
}
