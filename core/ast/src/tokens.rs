use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Token<'src> {
    Newline,
    Table,
    Ident(&'src str),
    Schemafull,
    Schemaless,
    LBrace,
    RBrace,
    LAngle,
    RAngle,
    Comma,
    Question,
    Pipe,
    Integer(&'src str),
}
