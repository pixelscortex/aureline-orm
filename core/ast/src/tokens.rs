//! Lexical tokens retained by the parser contract.
//!
//! Tokens borrow identifier text from the source buffer and carry only lexical
//! meaning. Grammar modules consume them to build the arena AST; semantic
//! checking intentionally does not depend on this token representation.

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
    LBracket,
    RBracket,
    Comma,
    Question,
    Pipe,
    Integer(&'src str),
}
