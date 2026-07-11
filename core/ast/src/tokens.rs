use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Token {
    Newline,

    Table,
    Ident,
    Schemafull,
    Schemaless,
    LBrace,
    RBrance,
}
