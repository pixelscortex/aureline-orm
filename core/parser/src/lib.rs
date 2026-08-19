mod grammar;
mod lexer;

use aureline_ast::ast::Ast;
use aureline_ast::source::{SourceId, SourceSpan, TextRange, TextSize};
pub use aureline_ast::tokens::Token;
use chumsky::{Parser, prelude::SimpleSpan};

/// A typed problem produced before the parser can construct a complete syntax tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyntaxProblem {
    /// The source byte length exceeds the largest Aureline text offset.
    SourceTooLarge { byte_len: usize },
    /// The lexer could not form a token; `span` covers the offending source bytes.
    InvalidToken { span: SourceSpan },
    /// The token stream did not match the grammar; `span` covers the unexpected
    /// token or is empty at the unexpected end of input.
    UnexpectedToken { span: SourceSpan },
}

/// Parses one source document using source ID `0` from [`SourceId::new`].
///
/// # Errors
///
/// Returns [`SyntaxProblem`] values when the parser cannot construct the tree.
pub fn parse(source: &str) -> Result<Ast, Vec<SyntaxProblem>> {
    parse_with_source(SourceId::new(0), source)
}

/// Parses a source document with the identity assigned by its source registry.
///
/// # Errors
///
/// Returns [`SyntaxProblem`] values when the parser cannot construct the tree.
pub fn parse_with_source(source_id: SourceId, source: &str) -> Result<Ast, Vec<SyntaxProblem>> {
    if TextSize::try_from(source.len()).is_err() {
        return Err(vec![SyntaxProblem::SourceTooLarge {
            byte_len: source.len(),
        }]);
    }

    let tokens = lexer::lexer()
        .parse(source)
        .into_result()
        .map_err(|errors| {
            errors
                .into_iter()
                .map(|error| SyntaxProblem::InvalidToken {
                    span: source_span(source_id, *error.span()),
                })
                .collect::<Vec<_>>()
        })?;

    grammar::parse_tokens(&tokens, source_id, source.len()).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| SyntaxProblem::UnexpectedToken {
                span: source_span(source_id, *error.span()),
            })
            .collect()
    })
}

fn source_span(source: SourceId, span: SimpleSpan) -> SourceSpan {
    // The mapped token input preserves Chumsky's ordered source boundaries. The
    // entrypoint's length check makes each boundary representable as TextSize.
    let start = TextSize::try_from(span.start)
        .expect("parser span starts within the prevalidated source length");
    let end = TextSize::try_from(span.end)
        .expect("parser span ends within the prevalidated source length");
    let range = TextRange::new(start, end).expect("mapped parser spans preserve input order");
    SourceSpan::new(source, range)
}
