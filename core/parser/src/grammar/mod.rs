mod table;

use aureline_ast::{
    AstBuilder,
    ast::{Ast, Comment, SchemaType, SourceType},
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

pub(crate) struct ParserState {
    ast: AstBuilder,
    source: SourceId,
}

impl ParserState {
    fn new(source: SourceId, comments: Vec<Comment>) -> Self {
        Self {
            ast: AstBuilder::new(comments),
            source,
        }
    }

    pub(crate) fn source_span(&self, span: SimpleSpan) -> SourceSpan {
        super::source_span(self.source, span)
    }
}

pub(crate) type ParserExtra = extra::Full<Cheap<SimpleSpan>, extra::SimpleState<ParserState>, ()>;

pub(crate) fn source_file_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, (), ParserExtra> {
    let newlines = just(Token::Newline).repeated();

    let items = table_parser()
        .then_ignore(newlines.clone())
        .repeated()
        .collect::<Vec<_>>();

    // `ignored()` would put child parsers in Chumsky's check-only mode and skip
    // the `map_with` calls that allocate the AST. Discard the emitted IDs only
    // after the single construction walk has run.
    newlines.ignore_then(items).then_ignore(end()).map(drop)
}

/// Parses a token stream into an arena-backed AST.
///
/// # Errors
///
/// Returns parser errors when the token stream does not match the grammar.
pub(crate) fn parse_tokens<'tokens, 'src: 'tokens>(
    tokens: &'tokens [TokenOccurrence<'src>],
    comments: Vec<Comment>,
    source: SourceId,
    source_len: usize,
) -> Result<Ast, Vec<Cheap<SimpleSpan>>> {
    let mut state = extra::SimpleState(ParserState::new(source, comments));
    let input = tokens.split_spanned(SimpleSpan::from(source_len..source_len));

    source_file_parser()
        .parse_with_state(input, &mut state)
        .into_result()?;

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

pub(crate) fn source_type_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, SourceType, ParserExtra> {
    ident().map_with(|name, context| {
        SourceType::name(name.inner, context.state().0.source_span(name.span))
    })
}
