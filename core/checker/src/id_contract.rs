//! Contextual validation for a table's top-level `id` field.
//!
//! `SurrealDB` validates the type of `DEFINE FIELD id` more narrowly than the
//! type of an ordinary stored field. The v3.2 validator accepts `any`,
//! `number`, `int`, `string`, `uuid`, `object`, arrays, and either-types whose
//! members all pass; see the tagged
//! [`RecordIdKeyLit::kind_supported`](https://raw.githubusercontent.com/surrealdb/surrealdb/refs/tags/v3.2.0/surrealdb/core/src/expr/record_id/key.rs#L39-L59)
//! implementation and its [language tests](https://raw.githubusercontent.com/surrealdb/surrealdb/refs/tags/v3.2.0/language-tests/tests/language/statements/define/field/id_kind.surql#L47-L119).
//!
//! `RecordKeyType` is a validated borrowed view over [`SemanticType`]. It is
//! deliberately not a second key-specific type algebra: the semantic type
//! remains the only representation of the contract, while this view carries
//! the contextual proof needed by downstream consumers.

use aureline_ast::ids::FieldId;

use crate::SemanticType;

/// A resolved semantic type known to satisfy the top-level record-key contract.
///
/// The reference is private so callers can only obtain this view through the
/// checker-owned validation path. Nested members are intentionally not
/// revalidated here: `SurrealDB` permits arbitrary values inside array and
/// object keys, and all nested semantic values have already been resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordKeyType<'a> {
    semantic: &'a SemanticType,
}

impl<'a> RecordKeyType<'a> {
    /// Returns the complete resolved contract represented by this view.
    #[must_use]
    pub fn semantic_type(&self) -> &'a SemanticType {
        self.semantic
    }
}

/// The validated identity contract for one explicit `id` field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdContract<'a> {
    field_id: FieldId,
    key_type: RecordKeyType<'a>,
}

impl<'a> IdContract<'a> {
    /// Returns the explicit field that supplies this table's record identity.
    #[must_use]
    pub fn field_id(&self) -> FieldId {
        self.field_id
    }

    /// Returns the validated, borrowed key contract.
    #[must_use]
    pub fn key_type(&self) -> RecordKeyType<'a> {
        self.key_type
    }
}

/// Checks the top-level portion of `SurrealDB`'s record-key type contract.
///
/// This function intentionally does not recurse into array, tuple, or object
/// members. The server's schema validator applies the narrower rule only to
/// the outer kind; nested structural values are permitted. A union remains
/// valid only when every member is an accepted outer kind.
pub(crate) fn validate_semantic_type(semantic: &SemanticType) -> bool {
    match semantic {
        SemanticType::Any
        | SemanticType::Number
        | SemanticType::Int
        | SemanticType::String
        | SemanticType::Uuid
        | SemanticType::Object
        | SemanticType::Array { .. }
        | SemanticType::Tuple(_) => true,
        SemanticType::Union(members) => members.iter().all(validate_semantic_type),
        SemanticType::Bool
        | SemanticType::Bytes
        | SemanticType::Datetime
        | SemanticType::Decimal
        | SemanticType::Duration
        | SemanticType::Float
        | SemanticType::Range
        | SemanticType::None
        | SemanticType::Null
        | SemanticType::Record(_)
        | SemanticType::Set { .. } => false,
    }
}

/// Builds the public view from a field ID that has already passed validation.
///
/// The field ID is stored in private per-table side data by the analysis pass,
/// so [`crate::CheckedProgram`] does not need to revalidate or clone the
/// semantic type when answering `id_contract`.
pub(crate) fn from_validated(field_id: FieldId, semantic: &SemanticType) -> IdContract<'_> {
    IdContract {
        field_id,
        key_type: RecordKeyType { semantic },
    }
}
