//! Logical checked-program views omit source locations and arena identities.

use crate::{CheckedProgram, RecordTargets, SemanticType};
use serde::{Serialize, Serializer};

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
                            semantic_type: ContractType {
                                program: self,
                                value: semantic_type,
                            },
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
struct ContractProgram<'program, 'ast> {
    tables: Vec<ContractTable<'program, 'ast>>,
}

#[derive(Serialize)]
#[serde(rename = "Table")]
struct ContractTable<'program, 'ast> {
    name: &'program str,
    fields: Vec<ContractField<'program, 'ast>>,
}

#[derive(Serialize)]
#[serde(rename = "Field")]
struct ContractField<'program, 'ast> {
    name: &'program str,
    semantic_type: ContractType<'program, 'ast>,
}

struct ContractType<'program, 'ast> {
    program: &'program CheckedProgram<'ast>,
    value: &'program SemanticType,
}

impl Serialize for ContractType<'_, '_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let nested = |value| ContractType {
            program: self.program,
            value,
        };
        let scalar = match self.value {
            SemanticType::Any => "Any",
            SemanticType::Bool => "Bool",
            SemanticType::Bytes => "Bytes",
            SemanticType::Datetime => "Datetime",
            SemanticType::Decimal => "Decimal",
            SemanticType::Duration => "Duration",
            SemanticType::Float => "Float",
            SemanticType::Int => "Int",
            SemanticType::Number => "Number",
            SemanticType::Object => "Object",
            SemanticType::Range => "Range",
            SemanticType::String => "String",
            SemanticType::Uuid => "Uuid",
            SemanticType::None => "None",
            SemanticType::Null => "Null",
            SemanticType::Array {
                element,
                exact_length,
            } => {
                return serializer.serialize_newtype_variant(
                    "SemanticType",
                    0,
                    "Array",
                    &(nested(element), exact_length),
                );
            }
            SemanticType::Set {
                element,
                max_distinct,
            } => {
                return serializer.serialize_newtype_variant(
                    "SemanticType",
                    0,
                    "Set",
                    &(nested(element), max_distinct),
                );
            }
            SemanticType::Union(members) => {
                let members = members.iter().map(nested).collect::<Vec<_>>();
                return serializer.serialize_newtype_variant("SemanticType", 0, "Union", &members);
            }
            SemanticType::Record(RecordTargets::Any) => "Record",
            SemanticType::Record(RecordTargets::Tables(targets)) => {
                let names = targets
                    .iter()
                    .map(|&id| {
                        self.program
                            .table(id)
                            .expect("resolved target belongs to checked AST")
                            .name()
                    })
                    .collect::<Vec<_>>();
                return serializer.serialize_newtype_variant("SemanticType", 0, "Record", &names);
            }
        };
        serializer.serialize_unit_variant("SemanticType", 0, scalar)
    }
}
