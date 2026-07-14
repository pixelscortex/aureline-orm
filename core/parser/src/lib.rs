pub mod grammar;
pub mod lexer;

use aureline_ast::ast::Ast;
pub use aureline_ast::tokens::Token;
use chumsky::{Parser, error::EmptyErr};
pub use grammar::parse_tokens;
pub use lexer::lexer;

#[must_use]
pub fn parse(source: &str) -> Result<Ast, Vec<EmptyErr>> {
    let tokens = lexer().parse(source).into_result()?;
    grammar::parse_tokens(&tokens)
}
