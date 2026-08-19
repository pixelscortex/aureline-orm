use aureline_ast::{
    TableFieldBuilder, ast::SourceType, ids::TableId, source::SourceSpan, tokens::Token,
};
use chumsky::prelude::*;

use crate::grammar::{ParserExtra, TokenInput, ident, schema_type_parser, source_type_parser};

struct ParsedField {
    span: SourceSpan,
    name: String,
    name_span: SourceSpan,
    source_type: SourceType,
}

impl ParsedField {
    fn alloc_in(self, fields: &mut TableFieldBuilder<'_>) {
        fields.alloc_field(self.span, self.name, self.name_span, self.source_type);
    }
}

fn field_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedField, ParserExtra> {
    ident()
        .then(source_type_parser())
        .map_with(|(name, source_type), context| {
            let field_span = context.span();
            let state = &context.state().0;
            ParsedField {
                span: state.source_span(field_span),
                name: name.inner,
                name_span: state.source_span(name.span),
                source_type,
            }
        })
}

pub(crate) fn table_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, TableId, ParserExtra> {
    let schema_type = schema_type_parser();
    let name = ident();
    let newlines = just(Token::Newline).repeated().at_least(1);
    let body = field_parser()
        .separated_by(newlines)
        .allow_leading()
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace));

    just(Token::Table)
        .ignore_then(name)
        .then(schema_type)
        .then(body)
        .map_with(|((name, schema_type), fields), context| {
            let table_span = context.span();
            let state = &mut context.state().0;
            let table_span = state.source_span(table_span);
            let name_span = state.source_span(name.span);
            let schema_type_span = state.source_span(schema_type.span);
            state.ast.alloc_table(
                table_span,
                name.inner,
                name_span,
                schema_type.inner,
                schema_type_span,
                |table_fields| {
                    for field in fields {
                        field.alloc_in(table_fields);
                    }
                },
            )
        })
}
