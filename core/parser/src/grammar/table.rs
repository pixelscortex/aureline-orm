use aureline_ast::{TableFieldBuilder, ast::SourceType, source::SourceSpan, tokens::Token};
use chumsky::prelude::*;

use crate::grammar::{
    GrammarProblem, ParserExtra, TokenInput, applied_source_type_parser, ident, integer,
    punctuated_identifier, schema_type_parser, source_type_parser,
};

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

enum FieldOutcome {
    Field(ParsedField),
    Problem(GrammarProblem),
}

fn field_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, FieldOutcome, ParserExtra> {
    let split_name =
        ident()
            .then(ident())
            .then(ident())
            .map_with(|((name, source_type), extra), context| {
                let state = &context.state().0;
                let problem = match (
                    state.inline_whitespace_between(name.span, source_type.span),
                    state.inline_whitespace_between(source_type.span, extra.span),
                ) {
                    (Some(name_gap), Some(_)) => GrammarProblem::IdentifierWhitespace(name_gap),
                    _ => GrammarProblem::Unexpected(extra.span),
                };
                FieldOutcome::Problem(problem)
            });

    let field = ident()
        .then(source_type_parser())
        .map_with(|(name, source_type), context| {
            let field_span = context.span();
            let state = &context.state().0;
            match source_type.into_result() {
                Ok(source_type) => FieldOutcome::Field(ParsedField {
                    span: state.source_span(field_span),
                    name: name.inner,
                    name_span: state.source_span(name.span),
                    source_type,
                }),
                Err(problem) => FieldOutcome::Problem(problem),
            }
        });

    // Pure digits are Integer tokens for type arguments; recover them before
    // normal field parsing when they occupy the declared-name slot.
    let integer_name = integer().then(source_type_parser()).map(|(name, _)| {
        FieldOutcome::Problem(GrammarProblem::IdentifierStartsWithDigit(name.span))
    });

    // Consume the complete recursive application before the general compound
    // recovery can absorb the following field type as another name suffix.
    let applied_name = applied_source_type_parser()
        .then(source_type_parser())
        .map(|(problem, _)| FieldOutcome::Problem(problem));

    let punctuated_name = punctuated_identifier()
        .then(source_type_parser())
        .map(|(name, _)| FieldOutcome::Problem(name.into_problem()));

    // Try the three-word form first so one physical `split name type` field can
    // retain its identifier-specific whitespace problem.
    choice((
        split_name,
        integer_name,
        applied_name,
        punctuated_name,
        field,
    ))
}

struct ParsedTableHeader {
    name: Spanned<String>,
    schema_type: Spanned<aureline_ast::ast::SchemaType>,
    problem: Option<GrammarProblem>,
}

fn table_header_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTableHeader, ParserExtra> {
    let split_name = ident().then(ident()).then(schema_type_parser()).map_with(
        |((name, extra), schema_type), context| {
            let state = &context.state().0;
            let problem = state
                .inline_whitespace_between(name.span, extra.span)
                .map_or(GrammarProblem::Unexpected(extra.span), |gap| {
                    GrammarProblem::IdentifierWhitespace(gap)
                });
            ParsedTableHeader {
                name,
                schema_type,
                problem: Some(problem),
            }
        },
    );

    let header = ident()
        .then(schema_type_parser())
        .map(|(name, schema_type)| ParsedTableHeader {
            name,
            schema_type,
            problem: None,
        });

    // Try `name extra schema-type` first. An exact preserved whitespace gap
    // makes `extra` the second half of a split name; any other gap leaves it an
    // unexpected token while the same table shape is consumed for recovery.
    choice((split_name, header))
}

fn table_body_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Vec<FieldOutcome>, ParserExtra> {
    let newlines = just(Token::Newline).repeated().at_least(1);
    field_parser()
        .separated_by(newlines)
        .allow_leading()
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
}

pub(crate) fn table_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Option<GrammarProblem>, ParserExtra> {
    // Pure digits are Integer tokens for type arguments; recover them before
    // normal table parsing when they occupy the declared-name slot.
    let integer_name = just(Token::Table)
        .ignore_then(integer())
        .then(schema_type_parser())
        .then(table_body_parser())
        .map(|((name, _), _)| Some(GrammarProblem::IdentifierStartsWithDigit(name.span)));

    // As in field recovery, consume the complete recursive application before
    // the general compound branch can absorb the following schema mode.
    let applied_name = just(Token::Table)
        .ignore_then(applied_source_type_parser())
        .then(schema_type_parser())
        .then(table_body_parser())
        .map(|((problem, _), _)| Some(problem));

    let punctuated_name = just(Token::Table)
        .ignore_then(punctuated_identifier())
        .then(schema_type_parser())
        .then(table_body_parser())
        .map(|((name, _), _)| Some(name.into_problem()));

    // Recover `table name unknown { ... }` before the normal header alternatives
    // so the unknown schema word itself remains the unexpected token.
    let missing_schema_type = just(Token::Table)
        .ignore_then(ident())
        .then(ident())
        .then(table_body_parser())
        .map(|((_, unexpected), _)| Some(GrammarProblem::Unexpected(unexpected.span)));

    let table = just(Token::Table)
        .ignore_then(table_header_parser())
        .then(table_body_parser())
        .map_with(|(header, fields), context| {
            let mut problem = header.problem;
            let mut parsed_fields = Vec::new();
            for field in fields {
                match field {
                    FieldOutcome::Field(field) => parsed_fields.push(field),
                    FieldOutcome::Problem(next) => {
                        if problem.is_none_or(|current| next.span().start < current.span().start) {
                            problem = Some(next);
                        }
                    }
                }
            }
            if problem.is_some() {
                return problem;
            }

            let table_span = context.span();
            let state = &mut context.state().0;
            let table_span = state.source_span(table_span);
            let name_span = state.source_span(header.name.span);
            let schema_type_span = state.source_span(header.schema_type.span);
            state.ast.alloc_table(
                table_span,
                header.name.inner,
                name_span,
                header.schema_type.inner,
                schema_type_span,
                |table_fields| {
                    for field in parsed_fields {
                        field.alloc_in(table_fields);
                    }
                },
            );
            None
        });

    choice((
        integer_name,
        applied_name,
        punctuated_name,
        missing_schema_type,
        table,
    ))
}
