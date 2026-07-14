mod table;

use aureline_ast::{
    AstBuilder,
    ast::{Ast, SchemaType, SourceFile},
    tokens::Token,
};
use chumsky::{error::EmptyErr, extra, prelude::*};

use crate::grammar::table::table_parser;

pub(crate) type ParserExtra = extra::State<extra::SimpleState<AstBuilder>>;

pub fn source_file_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, &'tokens [Token<'src>], SourceFile, ParserExtra> {
    let newlines = just(Token::Newline).repeated();

    let items = table_parser()
        .then_ignore(newlines.clone())
        .repeated()
        .collect::<Vec<_>>();

    newlines
        .ignore_then(items)
        .then_ignore(end())
        .map(SourceFile::new)
}

/// Parses a token stream into an arena-backed AST.
///
/// # Errors
///
/// Returns parser errors when the token stream does not match the grammar.
pub fn parse_tokens<'tokens, 'src: 'tokens>(
    tokens: &'tokens [Token<'src>],
) -> Result<Ast, Vec<EmptyErr>> {
    let mut state = extra::SimpleState(AstBuilder::new());

    let root = source_file_parser()
        .parse_with_state(tokens, &mut state)
        .into_result()?;

    Ok(state.0.finish(root))
}

pub(crate) fn ident<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, &'tokens [Token<'src>], String, ParserExtra> {
    select_ref! {
        Token::Ident(name) => *name,
    }
    .map(str::to_owned)
}

pub(crate) fn schema_type_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, &'tokens [Token<'src>], SchemaType, ParserExtra> {
    choice((
        just(Token::Schemafull).to(SchemaType::Full),
        just(Token::Schemaless).to(SchemaType::Less),
    ))
}
