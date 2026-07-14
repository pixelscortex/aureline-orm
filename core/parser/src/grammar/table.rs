use aureline_ast::{ast::TableDecl, ids::ItemId, tokens::Token};
use chumsky::prelude::*;

use crate::grammar::{ParserExtra, ident, schema_type_parser};

pub(crate) fn table_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, &'tokens [Token<'src>], ItemId, ParserExtra> {
    let schema_type = schema_type_parser();
    let name = ident();
    let body = just(Token::Newline)
        .repeated()
        .delimited_by(just(Token::LBrace), just(Token::RBrace));

    just(Token::Table)
        .ignore_then(name)
        .then(schema_type)
        .then_ignore(body)
        .map_with(|(name, schema_type), context| {
            context.state().alloc(TableDecl::new(name, schema_type))
        })
}
