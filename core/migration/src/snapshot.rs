//! Snapshot JSON is an untrusted schema input. Its flat entities are checked
//! through the existing parser and semantic gate, then compared with their
//! canonical reserialization. This reuses the language's type and record-key
//! rules without giving snapshot text a path into generated SQL. Checked
//! Programs themselves are always lowered directly, never reparsed.

use std::{collections::BTreeMap, fmt::Write};

use serde::{Deserialize, Serialize};

use crate::model::{MigrationModel, SchemaMode};

const VERSION: u64 = 1;
const ID_ALPHABET: [char; 31] = [
    '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'j', 'k', 'm',
    'n', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];

/// Last generated schema state, with a versioned linear-history identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    id: String,
    prev_ids: Vec<String>,
    model: MigrationModel,
}

/// Error returned when snapshot JSON is malformed or fails schema validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotError(String);

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SnapshotError {}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Document {
    version: u64,
    id: String,
    prev_ids: Vec<String>,
    entities: Vec<Entity>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "entityType", rename_all = "camelCase", deny_unknown_fields)]
enum Entity {
    Table {
        name: String,
        #[serde(rename = "schemaMode")]
        schema_mode: SchemaMode,
    },
    Field {
        table: String,
        name: String,
        #[serde(rename = "type")]
        ty: String,
        #[serde(
            rename = "recordKey",
            default,
            skip_serializing_if = "std::ops::Not::not"
        )]
        record_key: bool,
    },
}

impl Snapshot {
    /// Creates a new history identity for a successfully generated model.
    ///
    /// A snapshot points only to its immediate predecessor. The CLI owns the
    /// migration directory history; the library keeps this identity so the
    /// next generation can prove which model it compared against.
    #[must_use]
    pub fn new(model: MigrationModel, previous: Option<&Self>) -> Self {
        let mut id = nanoid::nanoid!(16, &ID_ALPHABET);
        while previous.is_some_and(|previous| previous.id == id) {
            id = nanoid::nanoid!(16, &ID_ALPHABET);
        }
        Self {
            id,
            prev_ids: previous
                .map(|previous| previous.id.clone())
                .into_iter()
                .collect(),
            model,
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn prev_ids(&self) -> &[String] {
        &self.prev_ids
    }

    #[must_use]
    pub fn model(&self) -> &MigrationModel {
        &self.model
    }

    /// Writes flat entities in table-name then field-name order. No timestamp
    /// or source position enters this representation.
    ///
    /// # Panics
    /// Panics only if serialization of the private string-only document fails.
    #[must_use]
    pub fn to_json(&self) -> String {
        let document = Document {
            version: VERSION,
            id: self.id.clone(),
            prev_ids: self.prev_ids.clone(),
            entities: entities(&self.model),
        };
        let mut json = serde_json::to_string_pretty(&document)
            .expect("snapshot documents contain only JSON-representable values");
        json.push('\n');
        json
    }

    /// Reads only version 1, with zero or one predecessor and complete,
    /// canonical schema facts. Unknown properties and entity variants fail.
    ///
    /// # Errors
    /// Rejects malformed JSON, incompatible versions, invalid identities,
    /// duplicate or dangling entities, invalid type/key contracts, and
    /// noncanonical or injected schema text.
    pub fn from_json(json: &str) -> Result<Self, SnapshotError> {
        let document: Document = serde_json::from_str(json)
            .map_err(|error| SnapshotError(format!("invalid snapshot JSON: {error}")))?;
        if document.version != VERSION {
            return Err(SnapshotError(format!(
                "unsupported snapshot version {}; expected {VERSION}",
                document.version
            )));
        }
        if !valid_id(&document.id)
            || document.prev_ids.len() > 1
            || document
                .prev_ids
                .iter()
                .any(|id| !valid_id(id) || *id == document.id)
        {
            return Err(SnapshotError("invalid snapshot identity or predecessor; expected distinct 16-character lowercase NanoIDs and at most one predecessor".into()));
        }
        let model = validate_entities(document.entities)?;
        Ok(Self {
            id: document.id,
            prev_ids: document.prev_ids,
            model,
        })
    }
}

fn valid_id(id: &str) -> bool {
    id.len() == 16 && id.chars().all(|ch| ID_ALPHABET.contains(&ch))
}

fn entities(model: &MigrationModel) -> Vec<Entity> {
    let mut entities = Vec::new();
    for (name, table) in &model.tables {
        entities.push(Entity::Table {
            name: name.clone(),
            schema_mode: table.schema_mode,
        });
    }
    for (table_name, table) in &model.tables {
        for (name, field) in &table.fields {
            entities.push(Entity::Field {
                table: table_name.clone(),
                name: name.clone(),
                ty: field.ty.canonical(),
                record_key: field.record_key,
            });
        }
    }
    entities
}

fn validate_entities(input: Vec<Entity>) -> Result<MigrationModel, SnapshotError> {
    let mut tables = BTreeMap::new();
    let mut fields = BTreeMap::new();
    for entity in input {
        match &entity {
            Entity::Table { name, .. } => {
                if tables.insert(name.clone(), entity).is_some() {
                    return Err(SnapshotError("duplicate snapshot table identity".into()));
                }
            }
            Entity::Field { table, name, .. } => {
                if fields
                    .insert((table.clone(), name.clone()), entity)
                    .is_some()
                {
                    return Err(SnapshotError("duplicate snapshot field identity".into()));
                }
            }
        }
    }
    if fields.keys().any(|(table, _)| !tables.contains_key(table)) {
        return Err(SnapshotError(
            "snapshot field references an undeclared table".into(),
        ));
    }
    let source = schema_source(&tables, &fields);
    let ast = aureline_parser::parse(&source)
        .map_err(|_| SnapshotError("snapshot contains invalid schema syntax".into()))?;
    let checked = aureline_checker::check(&ast).into_checked().map_err(|_| {
        SnapshotError(
            "snapshot contains an invalid type, record target, or record-key contract".into(),
        )
    })?;
    let model = MigrationModel::lower(&checked);
    let expected: Vec<_> = tables.into_values().chain(fields.into_values()).collect();
    if entities(&model) != expected {
        return Err(SnapshotError("snapshot entities must contain exact identities, canonical types, and the correct record-key role".into()));
    }
    Ok(model)
}

fn schema_source(
    tables: &BTreeMap<String, Entity>,
    fields: &BTreeMap<(String, String), Entity>,
) -> String {
    let mut source = String::new();
    for (name, entity) in tables {
        let Entity::Table { schema_mode, .. } = entity else {
            unreachable!("table map contains only table entities")
        };
        let mode = match schema_mode {
            SchemaMode::Schemafull => "schemafull",
            SchemaMode::Schemaless => "schemaless",
        };
        writeln!(&mut source, "table {name} {mode} {{").expect("writing to String cannot fail");
        for ((table, field), entity) in fields {
            if table != name {
                continue;
            }
            let Entity::Field { ty, .. } = entity else {
                unreachable!("field map contains only field entities")
            };
            writeln!(&mut source, "  {field} {ty}").expect("writing to String cannot fail");
        }
        source.push_str("}\n");
    }
    source
}
