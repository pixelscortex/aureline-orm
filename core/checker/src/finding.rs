use aureline_ast::{ids::TableId, source::SourceSpan};

/// A typed semantic problem before diagnostic wording and presentation.
///
/// Findings preserve original names and precise source locations. Declaration
/// collisions retain the first name span as context; type failures locate the
/// offending type name or structural form.
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
    /// A source type name is not part of the supported `SurrealDB` scalar
    /// catalog and is not a declared table name.
    UnknownType { name: String, span: SourceSpan },
    /// A real `SurrealDB` type family is known, but this table slice does not
    /// provide its semantic contract yet.
    UnsupportedType { name: String, span: SourceSpan },
    /// A declared table name was used as a type instead of `record<Name>`.
    BareTableType { name: String, span: SourceSpan },
    /// A recursive type form belongs to a later semantic slice.
    UnsupportedTypeSyntax {
        kind: UnsupportedTypeSyntaxKind,
        span: SourceSpan,
    },
    /// A scalar name was incorrectly used as a generic constructor.
    WrongArity {
        name: String,
        expected: usize,
        actual: usize,
        span: SourceSpan,
    },
}

/// A source type shape whose semantic contract is owned by a later slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsupportedTypeSyntaxKind {
    Union,
    Tuple,
}
