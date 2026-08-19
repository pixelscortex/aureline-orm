use aureline_ast::{ast::TableDecl, ids::ItemId, tokens::Token};
use chumsky::prelude::*;

use crate::grammar::{ParserExtra, TokenInput, ident, schema_type_parser};

pub(crate) fn table_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, ItemId, ParserExtra> {
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
            let table_span = context.span();
            let state = &mut context.state().0;
            let table = TableDecl::new(
                state.source_span(table_span),
                name.inner,
                state.source_span(name.span),
                schema_type.inner,
                state.source_span(schema_type.span),
            );
            state.ast.alloc(table)
        })
}
