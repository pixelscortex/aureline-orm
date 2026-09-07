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
    /// declarations independent; IDs remain compilation-local.
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
    /// The application argument count is outside the constructor's accepted range.
    WrongArity {
        name: String,
        minimum: usize,
        maximum: usize,
        actual: usize,
        span: SourceSpan,
    },
    /// A record constraint refers to no declared table.
    MissingRecordTarget { name: String, span: SourceSpan },
    /// A record constraint has multiple declared candidates; none is a resolved identity.
    AmbiguousRecordTarget {
        name: String,
        span: SourceSpan,
        candidates: Vec<TableId>,
    },
    /// An application argument has a value in a role that requires a type, or conversely.
    WrongArgumentRole {
        name: String,
        position: usize,
        expected: ArgumentRole,
        span: SourceSpan,
    },
    /// A digit sequence cannot represent an unsigned 64-bit collection size.
    InvalidCollectionSize {
        name: String,
        raw: String,
        span: SourceSpan,
    },
}

/// The semantic role required at an application argument position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgumentRole {
    ElementType,
    CollectionSize,
    RecordTarget,
    OptionType,
}
