use crate::{
    arena::Arena,
    ids::{FieldId, TableId},
    source::SourceSpan,
};

#[derive(Debug)]
#[cfg_attr(feature = "unstable-test-normalization", derive(serde::Serialize))]
#[cfg_attr(feature = "unstable-test-normalization", serde(rename = "$Ast"))]
pub struct Ast {
    #[cfg_attr(feature = "unstable-test-normalization", serde(rename = "$root"))]
    root: SourceFile,
    tables: Arena<TableId, TableDecl>,
    fields: Arena<FieldId, FieldDecl>,
    // Comments remain available through `Ast::comments` but are excluded from
    // the logical contract because they never change program meaning.
    #[cfg_attr(feature = "unstable-test-normalization", serde(skip))]
    comments: Vec<Comment>,
}

impl Ast {
    pub(crate) fn new(
        root: SourceFile,
        tables: Arena<TableId, TableDecl>,
        fields: Arena<FieldId, FieldDecl>,
        comments: Vec<Comment>,
    ) -> Self {
        Self {
            root,
            tables,
            fields,
            comments,
        }
    }

    #[must_use]
    pub fn root(&self) -> &SourceFile {
        &self.root
    }

    #[must_use]
    pub fn table(&self, id: TableId) -> Option<&TableDecl> {
        self.tables.get(id)
    }

    #[must_use]
    pub fn field(&self, id: FieldId) -> Option<&FieldDecl> {
        self.fields.get(id)
    }

    /// Returns comments in source order.
    #[must_use]
    pub fn comments(&self) -> &[Comment] {
        &self.comments
    }
}

/// Semantically inert comment syntax retained for source-aware tooling.
///
/// Its span covers the complete comment lexeme, including delimiters. A line
/// comment's span stops before the physical newline that terminates it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Comment {
    kind: CommentKind,
    span: SourceSpan,
}

impl Comment {
    /// Creates a comment whose span follows the complete-lexeme contract.
    #[must_use]
    pub const fn new(kind: CommentKind, span: SourceSpan) -> Self {
        Self { kind, span }
    }

    #[must_use]
    pub const fn kind(self) -> CommentKind {
        self.kind
    }

    #[must_use]
    pub const fn span(self) -> SourceSpan {
        self.span
    }
}

/// The delimiter form of a comment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommentKind {
    /// A `//` comment ending before the physical newline.
    Line,
    /// A non-nesting `/* ... */` comment.
    Block,
}

#[derive(Debug)]
#[cfg_attr(feature = "unstable-test-normalization", derive(serde::Serialize))]
#[cfg_attr(feature = "unstable-test-normalization", serde(rename = "SourceFile"))]
pub struct SourceFile {
    tables: Vec<TableId>,
}

impl SourceFile {
    #[must_use]
    pub(crate) fn new(tables: Vec<TableId>) -> Self {
        Self { tables }
    }

    #[must_use]
    pub fn tables(&self) -> &[TableId] {
        &self.tables
    }
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
    fields: Vec<FieldId>,
}

impl TableDecl {
    /// Creates a table declaration with its precise source locations.
    ///
    /// `name_span` and `schema_type_span` must use the same source as `span` and
    /// must be contained by it. This constructor does not validate that relationship;
    /// callers that violate it create a declaration whose reported provenance cannot
    /// be trusted. `fields` must list this table's fields in source order, and each
    /// referenced field must carry this table's identity as its owner.
    pub(crate) fn new(
        span: SourceSpan,
        name: impl Into<String>,
        name_span: SourceSpan,
        schema_type: SchemaType,
        schema_type_span: SourceSpan,
        fields: Vec<FieldId>,
    ) -> Self {
        Self {
            span,
            name: name.into(),
            name_span,
            schema_type,
            schema_type_span,
            fields,
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

    #[must_use]
    pub fn fields(&self) -> &[FieldId] {
        &self.fields
    }
}

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "unstable-test-normalization", derive(serde::Serialize))]
#[cfg_attr(feature = "unstable-test-normalization", serde(rename = "Field"))]
pub struct FieldDecl {
    #[cfg_attr(feature = "unstable-test-normalization", serde(skip))]
    span: SourceSpan,
    name: String,
    #[cfg_attr(feature = "unstable-test-normalization", serde(skip))]
    name_span: SourceSpan,
    source_type: SourceType,
    // Logical normalization follows the table-to-fields edge. Serializing this
    // storage back-reference would make that traversal cyclic.
    #[cfg_attr(feature = "unstable-test-normalization", serde(skip))]
    owner: TableId,
}

impl FieldDecl {
    /// Creates a field with the table that owns it in the same parsed AST.
    ///
    /// The field name and source type spans must use the same source as `span`
    /// and must be contained by it.
    pub(crate) fn new(
        span: SourceSpan,
        name: impl Into<String>,
        name_span: SourceSpan,
        source_type: SourceType,
        owner: TableId,
    ) -> Self {
        Self {
            span,
            name: name.into(),
            name_span,
            source_type,
            owner,
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
    pub fn source_type(&self) -> &SourceType {
        &self.source_type
    }

    #[must_use]
    pub fn owner(&self) -> TableId {
        self.owner
    }
}

/// Meaning-free source type syntax preserved for later static resolution.
///
/// The parser records exact spelling and locations here without deciding which
/// type names Aureline supports.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "unstable-test-normalization", derive(serde::Serialize))]
pub enum SourceType {
    Name(TypeName),
    Application(TypeApplication),
}

impl SourceType {
    #[must_use]
    pub fn name(name: impl Into<String>, span: SourceSpan) -> Self {
        Self::Name(TypeName::new(name, span))
    }

    /// Creates an application with a non-empty, source-ordered argument list.
    ///
    /// `name_span` covers only the applied name, while `span` covers the complete
    /// application. `name_span`, `span`, and every argument span must share one
    /// source, and the child spans must be contained by `span`; this constructor
    /// does not validate those provenance relationships.
    #[must_use]
    pub fn application(
        name: impl Into<String>,
        name_span: SourceSpan,
        first_argument: TypeArgument,
        remaining_arguments: Vec<TypeArgument>,
        span: SourceSpan,
    ) -> Self {
        Self::Application(TypeApplication::new(
            name,
            name_span,
            first_argument,
            remaining_arguments,
            span,
        ))
    }

    #[must_use]
    pub fn span(&self) -> SourceSpan {
        match self {
            Self::Name(name) => name.span(),
            Self::Application(application) => application.span(),
        }
    }
}

/// A meaning-free application of one type name to a non-empty, ordered argument list.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "unstable-test-normalization", derive(serde::Serialize))]
pub struct TypeApplication {
    #[cfg_attr(feature = "unstable-test-normalization", serde(skip))]
    span: SourceSpan,
    name: TypeName,
    arguments: Vec<TypeArgument>,
}

impl TypeApplication {
    fn new(
        name: impl Into<String>,
        name_span: SourceSpan,
        first_argument: TypeArgument,
        remaining_arguments: Vec<TypeArgument>,
        span: SourceSpan,
    ) -> Self {
        let mut arguments = Vec::with_capacity(remaining_arguments.len() + 1);
        arguments.push(first_argument);
        arguments.extend(remaining_arguments);
        Self {
            span,
            name: TypeName::new(name, name_span),
            arguments,
        }
    }

    #[must_use]
    pub fn span(&self) -> SourceSpan {
        self.span
    }

    #[must_use]
    pub fn name(&self) -> &TypeName {
        &self.name
    }

    #[must_use]
    pub fn arguments(&self) -> &[TypeArgument] {
        &self.arguments
    }
}

/// One ordered, uninterpreted argument in a type application.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "unstable-test-normalization", derive(serde::Serialize))]
pub enum TypeArgument {
    Type(SourceType),
    Integer(IntegerArgument),
}

impl TypeArgument {
    /// Records an integer argument's exact source spelling without interpreting it.
    ///
    /// `raw` must be the original, non-empty ASCII digit sequence covered by
    /// `span`. It is preserved byte-for-byte, including leading zeroes; this
    /// constructor does not validate either the spelling or its provenance.
    #[must_use]
    pub fn integer(raw: impl Into<String>, span: SourceSpan) -> Self {
        Self::Integer(IntegerArgument::new(raw, span))
    }

    #[must_use]
    pub fn span(&self) -> SourceSpan {
        match self {
            Self::Type(source_type) => source_type.span(),
            Self::Integer(integer) => integer.span(),
        }
    }
}

/// An uninterpreted integer argument and its exact source location.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "unstable-test-normalization", derive(serde::Serialize))]
pub struct IntegerArgument {
    raw: String,
    #[cfg_attr(feature = "unstable-test-normalization", serde(skip))]
    span: SourceSpan,
}

impl IntegerArgument {
    fn new(raw: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            raw: raw.into(),
            span,
        }
    }

    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    #[must_use]
    pub fn span(&self) -> SourceSpan {
        self.span
    }
}

/// An exact, case-sensitive name reference and its source location.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "unstable-test-normalization", derive(serde::Serialize))]
#[cfg_attr(feature = "unstable-test-normalization", serde(rename = "Name"))]
pub struct TypeName {
    name: String,
    #[cfg_attr(feature = "unstable-test-normalization", serde(skip))]
    span: SourceSpan,
}

impl TypeName {
    #[must_use]
    pub fn new(name: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            name: name.into(),
            span,
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn span(&self) -> SourceSpan {
        self.span
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
