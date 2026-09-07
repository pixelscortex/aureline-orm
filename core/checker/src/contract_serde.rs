//! Logical contract representation for table-check Findings.
//!
//! Findings retain source spans and compilation-local owner IDs for semantic
//! consumers. Contract tests compare the stable logical problem shape, so this
//! adapter intentionally projects those location and identity details out of
//! the S-expression while the Rust tests inspect them directly when needed.

use serde::{Serialize, Serializer};

use crate::{
    analysis::CheckedProgram,
    finding::{Finding, UnsupportedTypeSyntaxKind},
    semantic_type::SemanticType,
};

impl Serialize for Finding {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::DuplicateTable { name, .. } => {
                ContractFinding::DuplicateTable { name }.serialize(serializer)
            }
            Self::DuplicateField { name, .. } => {
                ContractFinding::DuplicateField { name }.serialize(serializer)
            }
            Self::UnknownType { name, .. } => {
                ContractFinding::UnknownType { name }.serialize(serializer)
            }
            Self::UnsupportedType { name, .. } => {
                ContractFinding::UnsupportedType { name }.serialize(serializer)
            }
            Self::BareTableType { name, .. } => {
                ContractFinding::BareTableType { name }.serialize(serializer)
            }
            Self::UnsupportedTypeSyntax { kind, .. } => {
                ContractFinding::UnsupportedTypeSyntax { kind: kind.name() }.serialize(serializer)
            }
            Self::WrongArity { name, .. } => {
                ContractFinding::WrongArity { name }.serialize(serializer)
            }
        }
    }
}

#[derive(Serialize)]
enum ContractFinding<'finding> {
    DuplicateTable { name: &'finding str },
    DuplicateField { name: &'finding str },
    UnknownType { name: &'finding str },
    UnsupportedType { name: &'finding str },
    BareTableType { name: &'finding str },
    UnsupportedTypeSyntax { kind: &'finding str },
    WrongArity { name: &'finding str },
}

impl Serialize for SemanticType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_unit_variant("SemanticType", 0, self.name())
    }
}

impl SemanticType {
    fn name(&self) -> &'static str {
        match self {
            Self::Any => "Any",
            Self::Bool => "Bool",
            Self::Bytes => "Bytes",
            Self::Datetime => "Datetime",
            Self::Decimal => "Decimal",
            Self::Duration => "Duration",
            Self::Float => "Float",
            Self::Int => "Int",
            Self::Number => "Number",
            Self::Object => "Object",
            Self::Range => "Range",
            Self::String => "String",
            Self::Uuid => "Uuid",
            Self::None => "None",
            Self::Null => "Null",
        }
    }
}

impl UnsupportedTypeSyntaxKind {
    fn name(self) -> &'static str {
        match self {
            Self::Union => "Union",
            Self::Tuple => "Tuple",
        }
    }
}

impl Serialize for CheckedProgram<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let tables = self
            .tables()
            .iter()
            .map(|&table_id| {
                let table = self
                    .table(table_id)
                    .expect("a checked program table ID belongs to its AST");
                let fields = self
                    .fields_of(table_id)
                    .iter()
                    .map(|&field_id| {
                        let field = self
                            .field(field_id)
                            .expect("a checked program field ID belongs to its AST");
                        let semantic_type = self
                            .type_of_field(field_id)
                            .expect("a checked program field has a resolved type");
                        ContractField {
                            name: field.name(),
                            semantic_type,
                        }
                    })
                    .collect();
                ContractTable {
                    name: table.name(),
                    fields,
                }
            })
            .collect();
        ContractProgram { tables }.serialize(serializer)
    }
}

#[derive(Serialize)]
#[serde(rename = "CheckedProgram")]
struct ContractProgram<'program> {
    tables: Vec<ContractTable<'program>>,
}

#[derive(Serialize)]
#[serde(rename = "Table")]
struct ContractTable<'program> {
    name: &'program str,
    fields: Vec<ContractField<'program>>,
}

#[derive(Serialize)]
#[serde(rename = "Field")]
struct ContractField<'program> {
    name: &'program str,
    semantic_type: &'program SemanticType,
}
