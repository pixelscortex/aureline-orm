//! Parses complete table declarations and commits their staged fields to the AST.
//!
//! The valid outer shape is:
//!
//! ```text
//! table <name> (schemafull | schemaless) {
//!     <field-name> <type-expression>
//! }
//! ```
//!
//! Physical newlines—not commas or semicolons—separate fields. Recovery parsers
//! consume known malformed table names and headers so callers receive a precise
//! problem rather than an error at a later brace or keyword.

use aureline_ast::{ast::SchemaType, tokens::Token};
use chumsky::prelude::*;

use super::{
    atom::{ident, integer, schema_type},
    declared_name,
    field::{self, FieldOutcome},
    problem::GrammarProblem,
    state::{ParserExtra, TokenInput},
};

struct ParsedTableHeader {
    name: Spanned<String>,
    schema_type: Spanned<SchemaType>,
    problem: Option<GrammarProblem>,
}

fn header_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTableHeader, ParserExtra> {
    let split_name = ident().then(ident()).then(schema_type()).map_with(
        |((name, extra), schema_type), context| {
            let state = &context.state().0;
            let problem = state
                .inline_whitespace_between(name.span, extra.span)
                .map_or(GrammarProblem::unexpected(extra.span), |gap| {
                    GrammarProblem::identifier_whitespace(gap)
                });
            ParsedTableHeader {
                name,
                schema_type,
                problem: Some(problem),
            }
        },
    );

    let header = ident()
        .then(schema_type())
        .map(|(name, schema_type)| ParsedTableHeader {
            name,
            schema_type,
            problem: None,
        });

    // Try `name extra schema-type` first so the normal two-token header cannot
    // succeed early and leave `extra` to fail later at the body opener.
    choice((split_name, header))
}

fn body_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Vec<FieldOutcome>, ParserExtra> {
    let newlines = just(Token::Newline).repeated().at_least(1);
    field::parser()
        .separated_by(newlines)
        .allow_leading()
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
}

pub(super) fn parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Option<GrammarProblem>, ParserExtra> {
    // Pure digits are Integer tokens because they are legal type arguments.
    // Only this table-name position can reclassify `table 1 ...` as a declared
    // identifier that starts with a digit.
    let integer_name = just(Token::Table)
        .ignore_then(integer())
        .then(schema_type())
        .then(body_parser())
        .map(|((name, _), _)| Some(GrammarProblem::identifier_starts_with_digit(name.span)));

    // Consume the complete marked type-expression shape before the general
    // compound branch can absorb the following schema mode. For
    // `table array<string,3> schemafull {}`, this retains the opening `<` as the
    // first name violation.
    let marked_name = just(Token::Table)
        .ignore_then(declared_name::marked_type_expression())
        .then(schema_type())
        .then(body_parser())
        .map(|((problem, _), _)| Some(problem));

    let punctuated_name = just(Token::Table)
        .ignore_then(declared_name::punctuated())
        .then(schema_type())
        .then(body_parser())
        .map(|((name, _), _)| Some(name.into_problem()));

    // Recover `table User mystery {}` before normal header parsing so `mystery`
    // itself remains the unexpected token instead of a later `{` or EOF.
    let missing_schema_type = just(Token::Table)
        .ignore_then(ident())
        .then(ident())
        .then(body_parser())
        .map(|((_, unexpected), _)| Some(GrammarProblem::unexpected(unexpected.span)));

    let table = just(Token::Table)
        .ignore_then(header_parser())
        .then(body_parser())
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
            state.ast_mut().alloc_table(
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
        marked_name,
        punctuated_name,
        missing_schema_type,
        table,
    ))
}
