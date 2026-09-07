//! Valid, target-neutral contracts produced by static type resolution.

/// A resolved, target-neutral `SurrealDB` value contract.
///
/// Recovery is deliberately not represented here. Unknown and invalid source
/// types are carried by [`crate::TypeResolution`], so a `SemanticType` can
/// never be mistaken for a proven contract.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    Record(RecordTargets),
    /// Canonical alternatives; ordering is structural, not an assignability relation.
    Union(Vec<SemanticType>),
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

/// A record identity is unrestricted or constrained to declared table identities.
/// Constrained targets produced by the resolver are nonempty, deduplicated,
/// and ordered by their compilation-local arena positions.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RecordTargets {
    Any,
    Tables(Vec<aureline_ast::ids::TableId>),
}

impl Ord for RecordTargets {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use aureline_ast::arena::ArenaId;
        use std::cmp::Ordering;
        match (self, other) {
            (Self::Any, Self::Any) => Ordering::Equal,
            (Self::Any, Self::Tables(_)) => Ordering::Less,
            (Self::Tables(_), Self::Any) => Ordering::Greater,
            (Self::Tables(left), Self::Tables(right)) => left
                .iter()
                .map(|id| id.into_index())
                .cmp(right.iter().map(|id| id.into_index())),
        }
    }
}

impl PartialOrd for RecordTargets {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Whether the resolved outer field contract permits absence (`NONE`).
/// This says nothing about a stored `NULL` value or absence nested in a collection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "contract-serde", derive(serde::Serialize))]
pub enum FieldPresence {
    Required,
    Optional,
}

impl SemanticType {
    pub(crate) fn admits_none(&self) -> bool {
        match self {
            Self::Any | Self::None => true,
            Self::Union(members) => members.iter().any(Self::admits_none),
            _ => false,
        }
    }
}
