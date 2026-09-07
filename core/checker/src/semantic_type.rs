//! Valid, target-neutral contracts produced by static type resolution.

/// A resolved `SurrealDB` scalar contract.
///
/// Recovery is deliberately not represented here. Unknown and invalid source
/// types are carried by [`crate::TypeResolution`], so a `SemanticType` can
/// never be mistaken for a proven contract.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
