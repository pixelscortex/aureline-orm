//! Valid, target-neutral contracts produced by static type resolution.

/// A resolved, target-neutral `SurrealDB` value contract.
///
/// Recovery is deliberately not represented here. Unknown and invalid source
/// types are carried by [`crate::TypeResolution`], so a `SemanticType` can
/// never be mistaken for a proven contract.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "contract-serde", derive(serde::Serialize))]
pub enum SemanticType {
    Any,
    Bool,
    Bytes,
    Datetime,
    Decimal,
    Duration,
    Float,
    Int,
    Number,
    Object,
    Range,
    String,
    Uuid,
    None,
    Null,
    /// An ordered homogeneous collection with an optional exact length.
    Array {
        element: Box<SemanticType>,
        exact_length: Option<u64>,
    },
    /// An ordered, deduplicated collection with an optional maximum distinct count.
    /// See the sized-set ADR for the documented server compatibility gap.
    Set {
        element: Box<SemanticType>,
        max_distinct: Option<u64>,
    },
}

impl SemanticType {
    pub(crate) fn scalar(name: &str) -> Option<Self> {
        let canonical = name.to_ascii_lowercase();
        Some(match canonical.as_str() {
            "any" => Self::Any,
            "bool" => Self::Bool,
            "bytes" => Self::Bytes,
            "datetime" => Self::Datetime,
            "decimal" => Self::Decimal,
            "duration" => Self::Duration,
            "float" => Self::Float,
            "int" => Self::Int,
            "number" => Self::Number,
            "object" => Self::Object,
            "range" => Self::Range,
            "string" => Self::String,
            "uuid" => Self::Uuid,
            "none" => Self::None,
            "null" => Self::Null,
            _ => return None,
        })
    }
}
