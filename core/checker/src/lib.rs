#![doc = "Aureline semantic checker and diagnostics."]

use aureline_ast::Schema;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
}

#[must_use]
pub fn check_schema(schema: &Schema) -> Vec<Diagnostic> {
    if schema.name.is_empty() {
        vec![Diagnostic {
            message: "schema name is required".to_owned(),
        }]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use aureline_ast::Schema;

    use super::check_schema;

    #[test]
    fn checker_rejects_empty_schema_name() {
        let diagnostics = check_schema(&Schema::new(""));

        assert_eq!(diagnostics.len(), 1);
    }
}
