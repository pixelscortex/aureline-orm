//! The migration model is the stable seam between semantic checking and DDL.
//!
//! A checked program still refers to source arenas and checker-owned IDs. This
//! module replaces those references with sorted database names and preserves
//! only the contracts needed to compare schemas, validate snapshots, and render
//! `SurrealQL`. Keeping this representation target-neutral lets the planner and
//! renderer share one deterministic view of the schema.

use std::collections::BTreeMap;

use aureline_ast::ast::SchemaType;
use aureline_checker::{CheckedProgram, RecordTargets, SemanticType};
use serde::{Deserialize, Serialize};

/// Complete migration-relevant facts, independent of source arenas and target DDL.
///
/// `MigrationModel::lower` is the one-way boundary from checker output into
/// migration generation: record targets become table names, and `BTreeMap`
/// ordering makes comparison and snapshot serialization independent of source
/// declaration order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MigrationModel {
    pub(crate) tables: BTreeMap<String, Table>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    /// Whether the target table accepts only declared fields.
    pub(crate) schema_mode: SchemaMode,
    /// Fields keyed by their exact database names.
    pub(crate) fields: BTreeMap<String, Field>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchemaMode {
    /// Every stored field must be declared by the table schema.
    Schemafull,
    /// Undeclared fields remain allowed by the table schema.
    Schemaless,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    /// The checked value contract, including nested collections and unions.
    pub(crate) ty: MigrationType,
    /// Whether this field is the table's declared record identity field.
    pub(crate) record_key: bool,
}

/// Semantic contracts whose record targets use stable database names.
///
/// Collection bounds retain their Aureline meaning: arrays have exact length,
/// while sets have a maximum distinct count. A target renderer must prove it
/// can enforce those contracts before emitting DDL.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MigrationType {
    /// A target scalar kind such as `string`, `int`, or `any`.
    Scalar(String),
    /// An empty target list denotes an unrestricted record identity.
    Record(Vec<String>),
    /// An ordered collection whose length must equal `exact_length` when set.
    Array {
        element: Box<Self>,
        exact_length: Option<u64>,
    },
    /// A deduplicating collection with an optional maximum distinct count.
    Set {
        element: Box<Self>,
        max_distinct: Option<u64>,
    },
    /// A value accepted by at least one of the member contracts.
    Union(Vec<Self>),
    /// A fixed-position collection; members do not create wildcard fields.
    Tuple(Vec<Self>),
}

impl MigrationModel {
    /// Lowers proven schema facts without revisiting source type syntax.
    ///
    /// The checker has already resolved names, record targets, collection
    /// bounds, and record-key roles. This pass only translates that result into
    /// owned, target-neutral data; it intentionally does not reparse source or
    /// decide whether `SurrealDB` can enforce a contract.
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
    /// Returns this table's schema strictness for target rendering.
    #[must_use]
    pub fn schema_mode(&self) -> SchemaMode {
        self.schema_mode
    }

    /// Iterates fields in exact-name order, independent of declaration order.
    pub fn fields(&self) -> impl Iterator<Item = (&str, &Field)> {
        self.fields
            .iter()
            .map(|(name, field)| (name.as_str(), field))
    }
}

impl Field {
    /// Returns the normalized contract used by planning and rendering.
    #[must_use]
    pub fn ty(&self) -> &MigrationType {
        &self.ty
    }

    /// Reports whether this field supplies the table's record identity.
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

    /// Returns the canonical spelling used in snapshots and equality checks.
    ///
    /// This is a model spelling, not necessarily valid target DDL: in
    /// particular, a sized set retains its maximum distinct-count contract
    /// here even though the target renderer emits an assertion for that bound.
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
