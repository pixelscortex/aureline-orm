use aureline_ast::{ids::TableId, source::SourceSpan};

/// A typed semantic problem owned by the table declaration checks.
///
/// Locations use the declaration name spans: the later declaration is the
/// primary location and the first declaration is retained as context. The
/// exact source spelling is copied into the Finding so a report remains
/// self-contained after the AST is no longer available to a renderer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Finding {
    /// A table name was declared more than once, with exact case-sensitive
    /// matching.
    DuplicateTable {
        name: String,
        primary: SourceSpan,
        first: SourceSpan,
    },
    /// A field name was declared more than once within one table declaration.
    ///
    /// The owner ID keeps duplicate fields in separate duplicate table
    /// declarations independent; IDs are compilation-local and are omitted by
    /// the contract serializer.
    DuplicateField {
        owner: TableId,
        name: String,
        primary: SourceSpan,
        first: SourceSpan,
    },
}
