use crate::{arena::Arena, ids::ItemId};

#[derive(Debug)]
#[cfg_attr(feature = "unstable-test-normalization", derive(serde::Serialize))]
#[cfg_attr(feature = "unstable-test-normalization", serde(rename = "$Ast"))]
pub struct Ast {
    #[cfg_attr(feature = "unstable-test-normalization", serde(rename = "$root"))]
    root: SourceFile,
    items: Arena<ItemId, Item>,
}

impl Ast {
    pub(crate) fn new(root: SourceFile, items: Arena<ItemId, Item>) -> Self {
        Self { root, items }
    }

    #[must_use]
    pub fn root(&self) -> &SourceFile {
        &self.root
    }

    #[must_use]
    pub fn item(&self, id: ItemId) -> Option<&Item> {
        self.items.get(id)
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "unstable-test-normalization", derive(serde::Serialize))]
#[cfg_attr(feature = "unstable-test-normalization", serde(rename = "SourceFile"))]
pub struct SourceFile {
    items: Vec<ItemId>,
}

impl SourceFile {
    #[must_use]
    pub fn new(items: Vec<ItemId>) -> Self {
        Self { items }
    }

    #[must_use]
    pub fn items(&self) -> &[ItemId] {
        &self.items
    }
}

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "unstable-test-normalization", derive(serde::Serialize))]
pub enum Item {
    #[cfg_attr(feature = "unstable-test-normalization", serde(rename = "Table"))]
    Table(TableDecl),
}

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "unstable-test-normalization", derive(serde::Serialize))]
#[cfg_attr(feature = "unstable-test-normalization", serde(rename = "Table"))]
pub struct TableDecl {
    name: String,
    schema_type: SchemaType,
}

impl TableDecl {
    pub fn new(name: impl Into<String>, schema_type: SchemaType) -> Self {
        Self {
            name: name.into(),
            schema_type,
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn schema_type(&self) -> SchemaType {
        self.schema_type
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "unstable-test-normalization", derive(serde::Serialize))]
pub enum SchemaType {
    #[cfg_attr(feature = "unstable-test-normalization", serde(rename = "Schemafull"))]
    Full,
    #[cfg_attr(feature = "unstable-test-normalization", serde(rename = "Schemaless"))]
    Less,
}
