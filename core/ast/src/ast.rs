use crate::{arena::Arena, ids::ItemId, source::SourceSpan};

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
    #[cfg_attr(feature = "unstable-test-normalization", serde(skip))]
    span: SourceSpan,
    name: String,
    #[cfg_attr(feature = "unstable-test-normalization", serde(skip))]
    name_span: SourceSpan,
    schema_type: SchemaType,
    #[cfg_attr(feature = "unstable-test-normalization", serde(skip))]
    schema_type_span: SourceSpan,
}

impl TableDecl {
    /// Creates a table declaration with its precise source locations.
    ///
    /// `name_span` and `schema_type_span` must use the same source as `span` and
    /// must be contained by it. This constructor does not validate that relationship;
    /// callers that violate it create a declaration whose reported provenance cannot
    /// be trusted.
    pub fn new(
        span: SourceSpan,
        name: impl Into<String>,
        name_span: SourceSpan,
        schema_type: SchemaType,
        schema_type_span: SourceSpan,
    ) -> Self {
        Self {
            span,
            name: name.into(),
            name_span,
            schema_type,
            schema_type_span,
        }
    }

    #[must_use]
    pub fn span(&self) -> SourceSpan {
        self.span
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn name_span(&self) -> SourceSpan {
        self.name_span
    }

    #[must_use]
    pub fn schema_type(&self) -> SchemaType {
        self.schema_type
    }

    #[must_use]
    pub fn schema_type_span(&self) -> SourceSpan {
        self.schema_type_span
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
