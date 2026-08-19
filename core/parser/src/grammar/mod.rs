mod table;

use aureline_ast::{
    AstBuilder,
    ast::{Ast, Comment, SchemaType},
    source::{SourceId, SourceSpan},
    tokens::Token,
};
use chumsky::{
    error::Cheap,
    extra,
    input::{Input as _, MappedInput},
    prelude::*,
};

use crate::grammar::table::table_parser;
use crate::lexer::TokenOccurrence;

pub(crate) type TokenInput<'tokens, 'src> =
    MappedInput<'tokens, Token<'src>, SimpleSpan, &'tokens [TokenOccurrence<'src>]>;

/// A malformed form consumed during the construction walk so parsing can
/// select the earliest precise problem and then discard any partially built AST.
#[derive(Clone, Copy)]
pub(crate) enum GrammarProblem {
    IdentifierWhitespace(SimpleSpan),
    Unexpected(SimpleSpan),
}

impl GrammarProblem {
    pub(crate) const fn span(self) -> SimpleSpan {
        match self {
            Self::IdentifierWhitespace(span) | Self::Unexpected(span) => span,
        }
    }
}

pub(crate) enum ParseTokensError {
    Parser(Vec<Cheap<SimpleSpan>>),
    Problem(GrammarProblem),
}

pub(crate) struct ParserState {
    ast: AstBuilder,
    source: SourceId,
    inline_whitespace: Vec<SimpleSpan>,
}

impl ParserState {
    fn new(source: SourceId, comments: Vec<Comment>, inline_whitespace: Vec<SimpleSpan>) -> Self {
        Self {
            ast: AstBuilder::new(comments),
            source,
            inline_whitespace,
        }
    }

    pub(crate) fn source_span(&self, span: SimpleSpan) -> SourceSpan {
        super::source_span(self.source, span)
    }

    pub(crate) fn inline_whitespace_between(
        &self,
        left: SimpleSpan,
        right: SimpleSpan,
    ) -> Option<SimpleSpan> {
        let gap = SimpleSpan::from(left.end..right.start);
        self.inline_whitespace.contains(&gap).then_some(gap)
    }
}

pub(crate) type ParserExtra = extra::Full<Cheap<SimpleSpan>, extra::SimpleState<ParserState>, ()>;

pub(crate) fn source_file_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Option<GrammarProblem>, ParserExtra> {
    let newlines = just(Token::Newline).repeated();

    let items = table_parser()
        .then_ignore(newlines.clone())
        .repeated()
        .collect::<Vec<_>>();

    // `ignored()` would put child parsers in Chumsky's check-only mode and skip
    // the `map_with` calls that allocate the AST. Inspect emitted problems only
    // after the single construction walk has run.
    newlines
        .ignore_then(items)
        .then_ignore(end())
        .map(|problems| {
            problems
                .into_iter()
                .flatten()
                .min_by_key(|problem| problem.span().start)
        })
}

/// Parses a token stream into an arena-backed AST.
///
/// # Errors
///
/// Returns [`ParseTokensError::Parser`] when the token stream cannot be consumed,
/// or [`ParseTokensError::Problem`] when a consumed recovery form carries the
/// earliest precise grammar problem.
pub(crate) fn parse_tokens<'tokens, 'src: 'tokens>(
    tokens: &'tokens [TokenOccurrence<'src>],
    comments: Vec<Comment>,
    inline_whitespace: Vec<SimpleSpan>,
    source: SourceId,
    source_len: usize,
) -> Result<Ast, ParseTokensError> {
    let mut state = extra::SimpleState(ParserState::new(source, comments, inline_whitespace));
    let input = tokens.split_spanned(SimpleSpan::from(source_len..source_len));

    let problem = source_file_parser()
        .parse_with_state(input, &mut state)
        .into_result()
        .map_err(ParseTokensError::Parser)?;

    if let Some(problem) = problem {
        return Err(ParseTokensError::Problem(problem));
    }

    Ok(state.0.ast.finish())
}

pub(crate) fn ident<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Spanned<String>, ParserExtra> {
    select_ref! {
        Token::Ident(name) => *name,
    }
    .map(str::to_owned)
    .spanned()
}

pub(crate) fn schema_type_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Spanned<SchemaType>, ParserExtra> {
    choice((
        just(Token::Schemafull).to(SchemaType::Full),
        just(Token::Schemaless).to(SchemaType::Less),
    ))
    .spanned()
}
