#![doc = "Aureline migration preview primitives."]

use aureline_ast::Schema;

#[must_use]
pub fn preview_migration(schema: &Schema) -> String {
    format!("-- migration preview for {}", schema.name)
}

#[cfg(test)]
mod tests {
    use aureline_ast::Schema;

    use super::preview_migration;

    #[test]
    fn migration_preview_names_schema() {
        assert_eq!(
            preview_migration(&Schema::new("blog")),
            "-- migration preview for blog"
        );
    }
}
