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
//!
//! Representative end-to-end flow:
//!
//! ```text
//! user source:  table User schemafull { owner record<User | Bot> }
//! lexer:       grammatical tokens + comment/layout spans
//! grammar:     staged table and field outcomes, or a source-spanned problem
//! builder:     TableId/FieldId ownership edges and source-order lists
//! result:      Ast, or SyntaxProblem values with no partial Ast
//! ```
//!
//! Grammar modules document the finer flow for precedence and directed recovery;
//! AST modules document the committed arena shape.

mod grammar;
mod lexer;
mod problem;

pub use aureline_ast::tokens::Token;
use aureline_ast::{ast::Ast, source::SourceId};
pub use problem::{IdentifierProblem, SyntaxProblem};

/// Classifies the grammatical tokens in one source document.
///
/// Comments are retained internally for parsing but do not appear in the
/// returned stream. Inline whitespace is layout and is discarded. Use [`parse`] when source locations or
/// syntax diagnostics beyond lexing are needed.
///
/// # Errors
///
/// Returns lexical [`SyntaxProblem`] values when the source contains an invalid
/// token, identifier, unterminated block comment, or unrepresentable byte length.
pub fn tokenize(source: &str) -> Result<Vec<Token<'_>>, Vec<SyntaxProblem>> {
    lexer::lex(SourceId::new(0), source).map(|lexed| {
        lexed
            .tokens
            .into_iter()
            .map(|occurrence| occurrence.inner)
            .collect()
    })
}

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
/// Offsets remain UTF-8 byte offsets into `source`. This function does not access
/// or validate a registry: the caller must ensure `source_id` identifies this
/// exact source text for as long as the returned locations are used.
///
/// # Errors
///
/// Returns [`SyntaxProblem`] values when the lexer or grammar cannot construct a
/// complete valid tree. No partial AST is returned.
pub fn parse_with_source(source_id: SourceId, source: &str) -> Result<Ast, Vec<SyntaxProblem>> {
    grammar::parse(lexer::lex(source_id, source)?)
}
