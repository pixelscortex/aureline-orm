//! Parses Aureline source into an arena-backed, source-spanned AST.
//!
//! The implementation is split into two explicit stages:
//!
//! - `lexer` classifies source characters, retains comments and layout spans,
//!   and reports character-level identifier problems;
//! - `grammar` consumes only grammatical tokens, constructs the AST, and
//!   recovers known malformed shapes into directed [`SyntaxProblem`] values.
//!
//! Type names remain meaning-free at this layer. Parsing `FutureType` succeeds
//! exactly like parsing `string`; name resolution and type validation belong to
//! later compilation contexts.

mod grammar;
mod lexer;
mod problem;

pub use aureline_ast::tokens::Token;
use aureline_ast::{ast::Ast, source::SourceId};
pub use problem::{IdentifierProblem, SyntaxProblem};

/// Parses one source document using source ID `0` from [`SourceId::new`].
///
/// Use [`parse_with_source`] when a caller manages multiple files and needs AST
/// and diagnostic spans to retain the originating file identity.
///
/// # Errors
///
/// Returns [`SyntaxProblem`] values when the lexer or grammar cannot construct a
/// complete valid tree. No partial AST is returned.
pub fn parse(source: &str) -> Result<Ast, Vec<SyntaxProblem>> {
    parse_with_source(SourceId::new(0), source)
}

/// Parses a source document with the identity assigned by its source registry.
///
/// `source_id` is copied into every AST and problem [`aureline_ast::source::SourceSpan`].
/// Offsets remain UTF-8 byte offsets into `source`.
///
/// # Errors
///
/// Returns [`SyntaxProblem`] values when the lexer or grammar cannot construct a
/// complete valid tree. No partial AST is returned.
pub fn parse_with_source(source_id: SourceId, source: &str) -> Result<Ast, Vec<SyntaxProblem>> {
    grammar::parse(lexer::lex(source_id, source)?)
}
