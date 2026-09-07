//! Logical checked-program views omit source locations and arena identities.

use crate::{CheckedProgram, SemanticType};
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
