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

use aureline_ast::{TableFieldBuilder, ast::SourceType, source::SourceSpan};
use chumsky::prelude::*;

use super::{
    atom::{ident, integer},
    declared_name,
    problem::GrammarProblem,
    state::{ParserExtra, TokenInput},
    type_expression,
};

/// A valid field staged until its surrounding table is known to be valid.
pub(super) struct ParsedField {
    /// Span of the complete `name type-expression` pair.
    span: SourceSpan,
    /// Owned declared field name.
    name: String,
    /// Span of the field name alone.
    name_span: SourceSpan,
    /// Meaning-free type expression already converted to public AST data.
    source_type: SourceType,
}

impl ParsedField {
    /// Allocates the staged field under the table currently owned by the
    /// supplied builder.
    pub(super) fn alloc_in(self, fields: &mut TableFieldBuilder<'_>) {
        fields.alloc_field(self.span, self.name, self.name_span, self.source_type);
    }
}

/// Result of consuming one complete field-shaped token sequence.
pub(super) enum FieldOutcome {
    /// A valid field, ready for allocation if the complete table succeeds.
    Field(ParsedField),
    /// A known malformed field. No public field node was constructed.
    Problem(GrammarProblem),
}

/// Parses a valid field or one of the recoverable malformed field shapes.
///
/// Alternative order is significant because several shapes share the same
/// leading identifier:
///
/// 1. A normal-looking field whose type parser already recovered a precise
///    problem wins first. `value string?` must retain `PostfixOptionalType`
///    instead of being reconsidered as a split field name.
/// 2. Three adjacent identifier tokens recover a whitespace-split name.
///    `first name string` reports the gap between `first` and `name` as
///    `InvalidIdentifier(ContainsWhitespace)`.
/// 3. A pure integer name is recovered: `1 string` reports
///    `InvalidIdentifier(StartsWithDigit)`.
/// 4. Complete type-shaped names are recovered: `array<string> bool` reports
///    `<`, and `User[] string` reports `[`, as identifier punctuation.
/// 5. General reconstructed names are recovered: `na?me string` reports `?`.
/// 6. The ordinary valid field is accepted last.
///
/// Structural-word tokens such as `table` do not match [`ident`] in a field
/// name slot and remain ordinary parser failures unless they follow already
/// recognized punctuation, as in `name?table string`.
pub(super) fn parser<'tokens, 'src: 'tokens>()
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
        .then(type_expression::parser())
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
    let recovered_field = field
        .clone()
        .filter(|field| matches!(field, FieldOutcome::Problem(_)));

    // Pure digits are Integer tokens because `array<string, 3>` is legal. Only
    // this field-name position provides enough context to reinterpret `1` in
    // `1 string` as a leading-digit identifier problem.
    let integer_name = integer().then(type_expression::parser()).map(|(name, _)| {
        FieldOutcome::Problem(GrammarProblem::IdentifierStartsWithDigit(name.span))
    });

    // Consume a complete marked type-expression shape before general compound
    // recovery can absorb the following field type as another name suffix. In
    // `array<string> bool`, this branch preserves `<` as the first violation.
    let marked_name = declared_name::marked_type_expression()
        .then(type_expression::parser())
        .map(|(problem, _)| FieldOutcome::Problem(problem));

    let punctuated_name = declared_name::punctuated()
        .then(type_expression::parser())
        .map(|(name, _)| FieldOutcome::Problem(name.into_problem()));

    // `recovered_field` accepts only the Problem output of `field`. Successful
    // two-token fields fail its filter and rewind into the later alternatives.
    // This preserves a structured type problem before compound-name recovery,
    // while `first name string` can still reach `split_name` and report its
    // identifier-specific whitespace problem.
    choice((
        recovered_field,
        split_name,
        integer_name,
        marked_name,
        punctuated_name,
        field,
    ))
}
