//! Private Serde adapter for the logical contract-test representation.
//!
//! The production AST is arena-backed and source-spanned. Contract tests need a
//! tree-shaped view containing only program meaning, so this module follows
//! ownership edges, omits source locations and comments, and gives constructors
//! their stable S-expression names.

use serde::{Serialize, Serializer};

use crate::ast::{
    Ast, FieldDecl, SchemaType, SourceType, TableDecl, TypeApplication, TypeArgument, TypeName,
    TypeTuple, TypeUnion,
};

impl Serialize for Ast {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ContractSourceFile::from(self).serialize(serializer)
    }
}

#[derive(Serialize)]
#[serde(rename = "SourceFile")]
struct ContractSourceFile<'ast> {
    tables: Vec<ContractTable<'ast>>,
}

impl<'ast> From<&'ast Ast> for ContractSourceFile<'ast> {
    fn from(ast: &'ast Ast) -> Self {
        let tables = ast
            .root()
            .tables()
            .iter()
            .map(|&id| {
                ContractTable::new(
                    ast,
                    ast.table(id)
                        .expect("a source file references a table in its own AST"),
                )
            })
            .collect();
        Self { tables }
    }
}

#[derive(Serialize)]
#[serde(rename = "Table")]
struct ContractTable<'ast> {
    name: &'ast str,
    schema_type: ContractSchemaType,
    fields: Vec<ContractField<'ast>>,
}

impl<'ast> ContractTable<'ast> {
    fn new(ast: &'ast Ast, table: &'ast TableDecl) -> Self {
        let fields = table
            .fields()
            .iter()
            .map(|&id| {
                ContractField::from(
                    ast.field(id)
                        .expect("a table references a field in its own AST"),
                )
            })
            .collect();
        Self {
            name: table.name(),
            schema_type: table.schema_type().into(),
            fields,
        }
    }
}

#[derive(Serialize)]
#[serde(rename = "Field")]
struct ContractField<'ast> {
    name: &'ast str,
    source_type: ContractSourceType<'ast>,
}

impl<'ast> From<&'ast FieldDecl> for ContractField<'ast> {
    fn from(field: &'ast FieldDecl) -> Self {
        Self {
            name: field.name(),
            source_type: field.source_type().into(),
        }
    }
}

#[derive(Serialize)]
enum ContractSourceType<'ast> {
    Name(ContractName<'ast>),
    Application(ContractApplication<'ast>),
    Union(ContractUnion<'ast>),
    Tuple(ContractTuple<'ast>),
}

impl<'ast> From<&'ast SourceType> for ContractSourceType<'ast> {
    fn from(source_type: &'ast SourceType) -> Self {
        match source_type {
            SourceType::Name(name) => Self::Name(name.into()),
            SourceType::Application(application) => Self::Application(application.into()),
            SourceType::Union(union) => Self::Union(union.into()),
            SourceType::Tuple(tuple) => Self::Tuple(tuple.into()),
        }
    }
}

#[derive(Serialize)]
struct ContractApplication<'ast> {
    name: ContractName<'ast>,
    arguments: Vec<ContractTypeArgument<'ast>>,
}

impl<'ast> From<&'ast TypeApplication> for ContractApplication<'ast> {
    fn from(application: &'ast TypeApplication) -> Self {
        Self {
            name: application.name().into(),
            arguments: application.arguments().iter().map(Into::into).collect(),
        }
    }
}

#[derive(Serialize)]
struct ContractUnion<'ast> {
    members: Vec<ContractSourceType<'ast>>,
}

impl<'ast> From<&'ast TypeUnion> for ContractUnion<'ast> {
    fn from(union: &'ast TypeUnion) -> Self {
        Self {
            members: union.members().iter().map(Into::into).collect(),
        }
    }
}

#[derive(Serialize)]
struct ContractTuple<'ast> {
    members: Vec<ContractSourceType<'ast>>,
}

impl<'ast> From<&'ast TypeTuple> for ContractTuple<'ast> {
    fn from(tuple: &'ast TypeTuple) -> Self {
        Self {
            members: tuple.members().iter().map(Into::into).collect(),
        }
    }
}

#[derive(Serialize)]
enum ContractTypeArgument<'ast> {
    Type(ContractSourceType<'ast>),
    Integer(&'ast str),
}

impl<'ast> From<&'ast TypeArgument> for ContractTypeArgument<'ast> {
    fn from(argument: &'ast TypeArgument) -> Self {
        match argument {
            TypeArgument::Type(source_type) => Self::Type(source_type.into()),
            TypeArgument::Integer(integer) => Self::Integer(integer.raw()),
        }
    }
}

#[derive(Serialize)]
#[serde(rename = "Name")]
struct ContractName<'ast> {
    name: &'ast str,
}

impl<'ast> From<&'ast TypeName> for ContractName<'ast> {
    fn from(name: &'ast TypeName) -> Self {
        Self { name: name.name() }
    }
}

#[derive(Serialize)]
enum ContractSchemaType {
    Schemafull,
    Schemaless,
}

impl From<SchemaType> for ContractSchemaType {
    fn from(schema_type: SchemaType) -> Self {
        match schema_type {
            SchemaType::Full => Self::Schemafull,
            SchemaType::Less => Self::Schemaless,
        }
    }
}
