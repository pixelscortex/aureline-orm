#![doc = "Aureline syntax tree and shared language data structures."]

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    pub name: String,
    pub models: Vec<Model>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub ty: TypeRef,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeRef {
    String,
    Integer,
    Boolean,
    Model(String),
}

impl Schema {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            models: Vec::new(),
        }
    }

    pub fn add_model(&mut self, model: Model) {
        self.models.push(model);
    }
}

#[cfg(test)]
mod tests {
    use super::{Model, Schema};

    #[test]
    fn schema_starts_empty_and_accepts_models() {
        let mut schema = Schema::new("blog");

        schema.add_model(Model {
            name: "Post".to_owned(),
            fields: Vec::new(),
        });

        assert_eq!(schema.name, "blog");
        assert_eq!(schema.models.len(), 1);
        assert_eq!(schema.models[0].name, "Post");
    }
}
