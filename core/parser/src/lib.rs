#![doc = "Aureline parser entrypoints."]

use aureline_ast::Schema;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseResult {
    pub schema: Schema,
}

#[must_use]
pub fn parse_schema_name(source: &str) -> ParseResult {
    ParseResult {
        schema: Schema::new(source.trim()),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_schema_name;

    #[test]
    fn parser_trims_schema_name() {
        let parsed = parse_schema_name(" blog ");

        assert_eq!(parsed.schema.name, "blog");
    }
}
