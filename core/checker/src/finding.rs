use aureline_ast::{ids::TableId, source::SourceSpan};

/// A typed semantic problem before diagnostic wording and presentation.
///
/// Findings preserve original names and precise source locations. Declaration
/// collisions retain the first name span as context; type failures locate the
/// offending type name or structural form.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "contract-serde", derive(serde::Serialize))]
pub enum Finding {
    /// A table name was declared more than once, with exact case-sensitive
    /// matching.
    DuplicateTable {
        name: String,
        #[cfg_attr(feature = "contract-serde", serde(skip))]
        primary: SourceSpan,
        #[cfg_attr(feature = "contract-serde", serde(skip))]
        first: SourceSpan,
    },
    /// A field name was declared more than once within one table declaration.
    ///
    /// The owner ID keeps duplicate fields in separate duplicate table
    /// declarations independent; IDs are compilation-local and are omitted by
    /// the contract serializer.
    DuplicateField {
        #[cfg_attr(feature = "contract-serde", serde(skip))]
        owner: TableId,
        name: String,
        #[cfg_attr(feature = "contract-serde", serde(skip))]
        primary: SourceSpan,
        #[cfg_attr(feature = "contract-serde", serde(skip))]
        first: SourceSpan,
    },
    /// A source type name is not part of the supported `SurrealDB` scalar
    /// catalog and is not a declared table name.
    UnknownType {
        name: String,
        #[cfg_attr(feature = "contract-serde", serde(skip))]
        span: SourceSpan,
    },
    /// A real `SurrealDB` type family is known, but this table slice does not
    /// provide its semantic contract yet.
    UnsupportedType {
        name: String,
        #[cfg_attr(feature = "contract-serde", serde(skip))]
        span: SourceSpan,
    },
    /// A declared table name was used as a type instead of `record<Name>`.
    BareTableType {
        name: String,
        #[cfg_attr(feature = "contract-serde", serde(skip))]
        span: SourceSpan,
    },
    /// A recursive type form belongs to a later semantic slice.
    UnsupportedTypeSyntax {
        kind: UnsupportedTypeSyntaxKind,
        #[cfg_attr(feature = "contract-serde", serde(skip))]
        span: SourceSpan,
    },
    /// The application argument count is outside the constructor's accepted range.
    WrongArity {
        name: String,
        minimum: usize,
        maximum: usize,
        actual: usize,
        #[cfg_attr(feature = "contract-serde", serde(skip))]
        span: SourceSpan,
    },
    /// An application argument has a value in a role that requires a type, or conversely.
    WrongArgumentRole {
        name: String,
        position: usize,
        expected: ArgumentRole,
        #[cfg_attr(feature = "contract-serde", serde(skip))]
        span: SourceSpan,
    },
    /// A digit sequence cannot represent an unsigned 64-bit collection size.
    InvalidCollectionSize {
        name: String,
        raw: String,
        #[cfg_attr(feature = "contract-serde", serde(skip))]
        span: SourceSpan,
    },
}

/// A source type shape whose semantic contract is owned by a later slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "contract-serde", derive(serde::Serialize))]
pub enum UnsupportedTypeSyntaxKind {
    Union,
    Tuple,
}

/// The semantic role required at an application argument position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "contract-serde", derive(serde::Serialize))]
pub enum ArgumentRole {
    ElementType,
    CollectionSize,
}
