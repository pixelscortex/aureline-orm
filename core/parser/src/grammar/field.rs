//! Parses one table field and recovers malformed field-name and type shapes.
//!
//! A successful field has the token shape `identifier type-expression`, for
//! example `name string` or `coordinates array<float, 3>`. The parser does not
//! decide what a type name means; it only preserves the source expression.
//!
//! Recovery alternatives consume recognizable malformed fields so the table
//! parser can compare their precise problems with problems from other fields.
//! Fields themselves do not mutate the AST. They become [`ParsedField`] values
//! and are allocated only after the surrounding table has no problem.

use aureline_ast::{TableFieldBuilder, ast::SourceType, source::SourceSpan, tokens::Token};
use chumsky::prelude::*;

use super::{
    atom::{ident, integer},
    declared_name,
    problem::GrammarProblem,
    state::{ParserExtra, TokenInput},
    type_expression,
};

pub(super) struct ParsedField {
    span: SourceSpan,
    name: String,
    name_span: SourceSpan,
    source_type: SourceType,
}

impl ParsedField {
    pub(super) fn alloc_in(self, fields: &mut TableFieldBuilder<'_>) {
        fields.alloc_field(self.span, self.name, self.name_span, self.source_type);
    }
}

pub(super) enum FieldOutcome {
    Field(ParsedField),
    Problem(GrammarProblem),
}

pub(super) fn parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, FieldOutcome, ParserExtra> {
    // Each alternative must reach a physical field boundary before it can win.
    // Without this lookahead, a shorter alternative can succeed and leave
    // tokens that prevent the enclosing table parser from trying a better one.
    let field_end = || {
        choice((just(Token::Newline), just(Token::RBrace)))
            .ignored()
            .rewind()
    };

    let field = ident()
        .then(type_expression::parser())
        .then_ignore(field_end())
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
        })
        .boxed();

    let split_name = ident()
        .then(ident())
        .then(type_expression::parser())
        .then_ignore(field_end())
        .map_with(|((name, extra), _), context| {
            let state = &context.state().0;
            let problem = state
                .inline_whitespace_between(name.span, extra.span)
                .map_or(GrammarProblem::unexpected(extra.span), |gap| {
                    GrammarProblem::identifier_whitespace(gap)
                });
            FieldOutcome::Problem(problem)
        });

    // Pure digits are Integer tokens because `array<string, 3>` is legal. Only
    // this field-name position provides enough context to reinterpret `1` in
    // `1 string` as a leading-digit identifier problem.
    let integer_name = integer()
        .then(type_expression::parser())
        .then_ignore(field_end())
        .map(|(name, _)| {
            FieldOutcome::Problem(GrammarProblem::identifier_starts_with_digit(name.span))
        });

    // Consume a complete marked type-expression shape before general compound
    // recovery can absorb the following field type as another name suffix. In
    // `array<string> bool`, this branch preserves `<` as the first violation.
    let marked_name = declared_name::marked_type_expression()
        .then(type_expression::parser())
        .then_ignore(field_end())
        .map(|(problem, _)| FieldOutcome::Problem(problem));

    let punctuated_name = declared_name::punctuated()
        .then(type_expression::parser())
        .then_ignore(field_end())
        .map(|(name, _)| FieldOutcome::Problem(name.into_problem()));

    choice((
        field,
        split_name,
        integer_name,
        marked_name,
        punctuated_name,
    ))
}
