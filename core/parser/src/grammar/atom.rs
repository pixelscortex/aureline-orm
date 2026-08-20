//! Small token-to-domain parsers shared by the larger grammar modules.
//!
//! These parsers do no recovery and make no contextual decisions. They only
//! select one already classified token, convert borrowed source spelling into
//! owned AST data where necessary, and retain the token's byte span.

use aureline_ast::{ast::SchemaType, tokens::Token};
use chumsky::prelude::*;

use super::state::{ParserExtra, TokenInput};

pub(super) fn ident<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Spanned<String>, ParserExtra> {
    select_ref! {
        Token::Ident(name) => *name,
    }
    .map(str::to_owned)
    .spanned()
}

pub(super) fn schema_type<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Spanned<SchemaType>, ParserExtra> {
    choice((
        just(Token::Schemafull).to(SchemaType::Full),
        just(Token::Schemaless).to(SchemaType::Less),
    ))
    .spanned()
}

pub(super) fn integer<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Spanned<String>, ParserExtra> {
    select_ref! {
        Token::Integer(raw) => *raw,
    }
    .map(str::to_owned)
    .spanned()
}
