use std::collections::BTreeMap;

use aureline_ast::ast::SchemaType;
use aureline_checker::{CheckedProgram, RecordTargets, SemanticType};
use serde::{Deserialize, Serialize};

/// Complete migration-relevant facts, independent of source arenas and target DDL.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MigrationModel {
    pub(crate) tables: BTreeMap<String, Table>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub(crate) schema_mode: SchemaMode,
    pub(crate) fields: BTreeMap<String, Field>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchemaMode {
    Schemafull,
    Schemaless,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub(crate) ty: MigrationType,
    pub(crate) record_key: bool,
}

/// Semantic contracts whose record targets use stable database names.
///
/// Collection bounds retain their Aureline meaning: arrays have exact length,
/// while sets have a maximum distinct count. A target renderer must prove it
/// can enforce those contracts before emitting DDL.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MigrationType {
    Scalar(String),
    /// An empty target list denotes an unrestricted record identity.
    Record(Vec<String>),
    Array {
        element: Box<Self>,
        exact_length: Option<u64>,
    },
    Set {
        element: Box<Self>,
        max_distinct: Option<u64>,
    },
    Union(Vec<Self>),
    Tuple(Vec<Self>),
}

impl MigrationModel {
    /// Lowers proven schema facts without revisiting source type syntax.
    ///
    /// # Panics
    /// Panics only if the checker's private arena invariants are violated.
    #[must_use]
    pub fn lower(checked: &CheckedProgram<'_>) -> Self {
        let tables = checked
            .tables()
            .iter()
            .map(|&table_id| {
                let table = checked.table(table_id).expect("checked table exists");
                let key = checked.id_contract(table_id).map(|key| key.field_id());
                let fields = checked
                    .fields_of(table_id)
                    .iter()
                    .map(|&field_id| {
                        let field = checked.field(field_id).expect("checked field exists");
                        let ty = checked
                            .type_of_field(field_id)
                            .expect("checked field has a type");
                        (
                            field.name().to_owned(),
                            Field {
                                ty: MigrationType::lower(checked, ty),
                                record_key: key == Some(field_id),
                            },
                        )
                    })
                    .collect();
                (
                    table.name().to_owned(),
                    Table {
                        schema_mode: match table.schema_type() {
                            SchemaType::Full => SchemaMode::Schemafull,
                            SchemaType::Less => SchemaMode::Schemaless,
                        },
                        fields,
                    },
                )
            })
            .collect();
        Self { tables }
    }

    /// Iterates tables in exact-name order, independent of declaration order.
    pub fn tables(&self) -> impl Iterator<Item = (&str, &Table)> {
        self.tables
            .iter()
            .map(|(name, table)| (name.as_str(), table))
    }
}

impl Table {
    #[must_use]
    pub fn schema_mode(&self) -> SchemaMode {
        self.schema_mode
    }

    pub fn fields(&self) -> impl Iterator<Item = (&str, &Field)> {
        self.fields
            .iter()
            .map(|(name, field)| (name.as_str(), field))
    }
}

impl Field {
    #[must_use]
    pub fn ty(&self) -> &MigrationType {
        &self.ty
    }

    #[must_use]
    pub fn is_record_key(&self) -> bool {
        self.record_key
    }
}

impl MigrationType {
    fn lower(checked: &CheckedProgram<'_>, ty: &SemanticType) -> Self {
        match ty {
            SemanticType::Record(targets) => {
                let mut names = match targets {
                    RecordTargets::Any => Vec::new(),
                    RecordTargets::Tables(tables) => tables
                        .iter()
                        .map(|&id| {
                            checked
                                .table(id)
                                .expect("checked record target exists")
                                .name()
                                .to_owned()
                        })
                        .collect(),
                };
                names.sort();
                Self::Record(names)
            }
            SemanticType::Array {
                element,
                exact_length,
            } => Self::Array {
                element: Box::new(Self::lower(checked, element)),
                exact_length: *exact_length,
            },
            SemanticType::Set {
                element,
                max_distinct,
            } => Self::Set {
                element: Box::new(Self::lower(checked, element)),
                max_distinct: *max_distinct,
            },
            SemanticType::Tuple(members) => {
                Self::Tuple(members.iter().map(|ty| Self::lower(checked, ty)).collect())
            }
            SemanticType::Union(members) => {
                let mut members: Vec<_> =
                    members.iter().map(|ty| Self::lower(checked, ty)).collect();
                members.sort();
                Self::Union(members)
            }
            SemanticType::Any => Self::Scalar("any".into()),
            SemanticType::Bool => Self::Scalar("bool".into()),
            SemanticType::Bytes => Self::Scalar("bytes".into()),
            SemanticType::Datetime => Self::Scalar("datetime".into()),
            SemanticType::Decimal => Self::Scalar("decimal".into()),
            SemanticType::Duration => Self::Scalar("duration".into()),
            SemanticType::Float => Self::Scalar("float".into()),
            SemanticType::Int => Self::Scalar("int".into()),
            SemanticType::Number => Self::Scalar("number".into()),
            SemanticType::Object => Self::Scalar("object".into()),
            SemanticType::Range => Self::Scalar("range".into()),
            SemanticType::String => Self::Scalar("string".into()),
            SemanticType::Uuid => Self::Scalar("uuid".into()),
            SemanticType::None => Self::Scalar("none".into()),
            SemanticType::Null => Self::Scalar("null".into()),
        }
    }

    /// Canonical snapshot spelling; sized sets here describe the contract,
    /// not a claim that native target syntax enforces its upper bound.
    #[must_use]
    pub fn canonical(&self) -> String {
        match self {
            Self::Scalar(name) => name.clone(),
            Self::Record(names) if names.is_empty() => "record".into(),
            Self::Record(names) => format!("record<{}>", names.join(" | ")),
            Self::Array {
                element,
                exact_length,
            } => collection("array", element, *exact_length),
            Self::Set {
                element,
                max_distinct,
            } => collection("set", element, *max_distinct),
            Self::Tuple(members) => format!("[{}]", join(members, ", ")),
            Self::Union(members) => {
                let present: Vec<_> = members
                    .iter()
                    .filter(|ty| !matches!(ty, Self::Scalar(name) if name == "none"))
                    .cloned()
                    .collect();
                if present.len() < members.len() {
                    format!("option<{}>", join(&present, " | "))
                } else {
                    join(members, " | ")
                }
            }
        }
    }
}

fn collection(name: &str, element: &MigrationType, bound: Option<u64>) -> String {
    let element = element.canonical();
    bound.map_or_else(
        || format!("{name}<{element}>"),
        |bound| format!("{name}<{element}, {bound}>"),
    )
}

fn join(members: &[MigrationType], separator: &str) -> String {
    members
        .iter()
        .map(MigrationType::canonical)
        .collect::<Vec<_>>()
        .join(separator)
}
