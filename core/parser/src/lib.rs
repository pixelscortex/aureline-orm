mod grammar;
mod lexer;

use aureline_ast::ast::{Ast, Comment};
use aureline_ast::source::{SourceId, SourceSpan, TextRange, TextSize};
pub use aureline_ast::tokens::Token;
use chumsky::{
    Parser,
    prelude::{SimpleSpan, Spanned},
};
use lexer::Lexeme;

/// The violated part of Aureline's ASCII bare-identifier boundary,
/// `[A-Za-z_][A-Za-z0-9_]*`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentifierProblem {
    /// An identifier began with an ASCII digit.
    StartsWithDigit,
    /// An identifier contained a character outside ASCII.
    ContainsNonAscii(char),
    /// An identifier contained `.`.
    ContainsDot,
    /// An identifier contained `-`.
    ContainsHyphen,
    /// An identifier contained another ASCII punctuation character.
    ContainsPunctuation(char),
    /// An identifier contained inline whitespace.
    ContainsWhitespace,
    /// A name used backticks reserved for a future embedded-SurrealQL escape hatch.
    BackticksReserved,
}

/// A typed problem produced before the parser can construct a complete syntax tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyntaxProblem {
    /// The source byte length exceeds the largest Aureline text offset.
    SourceTooLarge { byte_len: usize },
    /// The lexer could not form a token; `span` covers the offending source bytes.
    InvalidToken { span: SourceSpan },
    /// A name crossed Aureline's bare-identifier boundary; `span` covers the
    /// bytes that violate the boundary.
    InvalidIdentifier {
        problem: IdentifierProblem,
        span: SourceSpan,
    },
    /// A block comment reached the end of input; `span` points at its opening delimiter.
    UnterminatedBlockComment { span: SourceSpan },
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

    let occurrences = lexer::lexer()
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

    let mut tokens = Vec::new();
    let mut comments = Vec::new();
    let mut inline_whitespace = Vec::new();
    for Spanned { inner, span } in occurrences {
        match inner {
            Lexeme::InvalidIdentifier(problem) => {
                return Err(vec![SyntaxProblem::InvalidIdentifier {
                    problem,
                    span: source_span(source_id, span),
                }]);
            }
            Lexeme::Comment(kind) => {
                comments.push(Comment::new(kind, source_span(source_id, span)));
            }
            Lexeme::InlineWhitespace => inline_whitespace.push(span),
            Lexeme::UnterminatedBlockComment => {
                let opening = SimpleSpan::from(span.start..span.start + 2);
                return Err(vec![SyntaxProblem::UnterminatedBlockComment {
                    span: source_span(source_id, opening),
                }]);
            }
            Lexeme::Token(token) => tokens.push(Spanned { inner: token, span }),
        }
    }

    grammar::parse_tokens(
        &tokens,
        comments,
        inline_whitespace,
        source_id,
        source.len(),
    )
    .map_err(|error| match error {
        grammar::ParseTokensError::Parser(errors) => errors
            .into_iter()
            .map(|error| SyntaxProblem::UnexpectedToken {
                span: source_span(source_id, *error.span()),
            })
            .collect(),
        grammar::ParseTokensError::Problem(problem) => {
            let span = source_span(source_id, problem.span());
            let problem = match problem {
                grammar::GrammarProblem::IdentifierWhitespace(_) => {
                    SyntaxProblem::InvalidIdentifier {
                        problem: IdentifierProblem::ContainsWhitespace,
                        span,
                    }
                }
                grammar::GrammarProblem::Unexpected(_) => SyntaxProblem::UnexpectedToken { span },
            };
            vec![problem]
        }
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
