//! Converts source text into grammar tokens without discarding source layout.
//!
//! Lexing has two stages:
//!
//! 1. [`lexer`] scans the source and emits [`LexerOccurrence`] values. An
//!    occurrence can be a grammatical token, a comment, an inline-whitespace
//!    run, or a typed lexical problem.
//! 2. [`lex`] partitions those occurrences into [`LexedSource`]. The grammar
//!    receives only `tokens`, while comments become AST data and whitespace
//!    spans remain available to contextual name recovery.
//!
//! Keeping those channels separate is deliberate. Comments do not affect the
//! grammar, but callers still need their exact source locations. Spaces and
//! tabs usually do not affect the grammar either, but they distinguish an
//! attached malformed name such as `User?Name` from separated syntax such as
//! `User ? Name`.
//!
//! This module recognizes characters, not their grammatical role. In
//! particular, `<`, `>`, `[`, `]`, `,`, `?`, and `|` are always ordinary
//! structural tokens here because they are valid in type expressions:
//!
//! ```text
//! array<string>
//! [string, int]
//! User | Bot
//! ```
//!
//! The grammar's `declared_name` module decides whether those same tokens
//! violate a table or field name when they occur in a declared-name slot.

mod comment;
mod identifier;

use aureline_ast::{
    ast::{Comment, CommentKind},
    source::{SourceId, TextSize},
    tokens::Token,
};
use chumsky::{error::Cheap, extra, prelude::*};

use crate::{IdentifierProblem, SyntaxProblem, problem::source_span};

pub(crate) type TokenOccurrence<'src> = Spanned<Token<'src>>;

pub(super) type LexerOccurrence<'src> = Spanned<Lexeme<'src>>;

pub(super) type LexerExtra = extra::Err<Cheap<SimpleSpan>>;

pub(super) enum Lexeme<'src> {
    Token(Token<'src>),
    InvalidIdentifier(IdentifierProblem),
    InlineWhitespace,
    Comment(CommentKind),
    UnterminatedBlockComment,
}

pub(crate) struct LexedSource<'src> {
    pub(super) tokens: Vec<TokenOccurrence<'src>>,
    pub(super) comments: Vec<Comment>,
    pub(super) inline_whitespace: Vec<SimpleSpan>,
    pub(super) source: SourceId,
    pub(super) source_len: usize,
}

pub(crate) fn lex(
    source_id: SourceId,
    source: &str,
) -> Result<LexedSource<'_>, Vec<SyntaxProblem>> {
    if TextSize::try_from(source.len()).is_err() {
        return Err(vec![SyntaxProblem::SourceTooLarge {
            byte_len: source.len(),
        }]);
    }

    let occurrences = lexer().parse(source).into_result().map_err(|errors| {
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

    Ok(LexedSource {
        tokens,
        comments,
        inline_whitespace,
        source: source_id,
        source_len: source.len(),
    })
}

#[must_use]
fn lexer<'src>() -> impl Parser<'src, &'src str, Vec<LexerOccurrence<'src>>, LexerExtra> {
    let punctuation = choice((
        just('{').to(Token::LBrace),
        just('}').to(Token::RBrace),
        just('<').to(Token::LAngle),
        just('>').to(Token::RAngle),
        just('[').to(Token::LBracket),
        just(']').to(Token::RBracket),
        just(',').to(Token::Comma),
        just('?').to(Token::Question),
        just('|').to(Token::Pipe),
    ));

    let syntax_token = choice((text::newline().to(Token::Newline), punctuation))
        .spanned()
        .map(|occurrence: TokenOccurrence<'src>| {
            vec![Spanned {
                inner: Lexeme::Token(occurrence.inner),
                span: occurrence.span,
            }]
        });

    let inline_whitespace = one_of(" \t")
        .repeated()
        .at_least(1)
        .map_with(|(), context| {
            vec![Spanned {
                inner: Lexeme::InlineWhitespace,
                span: context.span(),
            }]
        });

    choice((
        comment::line(),
        comment::block(),
        comment::unterminated_block(),
        identifier::backtick(),
        identifier::candidate(),
        syntax_token,
        inline_whitespace,
    ))
    .repeated()
    .collect::<Vec<_>>()
    .map(|groups| groups.into_iter().flatten().collect())
}
