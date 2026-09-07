//! Type application parsing, argument conversion, and incomplete-list recovery.
//!
//! A type application is an unresolved name followed by comma-separated
//! arguments in angle brackets:
//!
//! ```text
//! array<string>
//! array<float, 3>
//! custom_type<record<A | B>, 003>
//! ```
//!
//! The parser consumes the angle-bracket item list once. Ordinary Rust then
//! distinguishes valid arguments, the two directed incomplete forms, and
//! malformed comma/adjacency shapes that retain generic `UnexpectedToken`
//! behavior.

use aureline_ast::{ast::SourceType, tokens::Token};
use chumsky::prelude::*;

use super::{
    super::{
        atom::{ident, integer},
        problem::GrammarProblem,
        state::{ParserExtra, ParserState, TokenInput},
    },
    parsed::{ParsedTypeArgument, ParsedTypeExpression},
    sequence::{self, SequenceItem, SequenceShapeProblem},
};

/// Consumes the name and both angle delimiters, emitting a staged source type
/// or structural problem. `array<float, 3>` becomes a name plus ordered type and
/// integer arguments; `array<string,>` consumes `>` and returns the trailing-comma
/// problem. Allocation of table and field identities belongs to the caller.
/// Nested types own their delimiters; commas and `>` at this level belong here.
/// See the enclosing module for the shared parser lifetime signature.
pub(super) fn parser<'tokens, 'src: 'tokens, P>(
    type_expression: P,
) -> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTypeExpression, ParserExtra>
where
    P: Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTypeExpression, ParserExtra>
        + Clone
        + 'tokens,
{
    let integer_argument = integer().map(ParsedTypeArgument::Integer);
    let argument = choice((
        type_expression.map(ParsedTypeArgument::Type),
        integer_argument,
    ))
    .boxed();
    // Sequence spans identify an unexpected adjacent token, while the argument
    // itself retains its complete source span. For `box<A []>`, report `[`;
    // consuming the tuple still lets this application own its closing `>`.
    let member = any().spanned().rewind().then(argument).map(
        |(first, argument): (Spanned<Token<'src>>, _)| {
            SequenceItem::Member(Spanned {
                inner: argument,
                span: first.span,
            })
        },
    );
    let item = choice((
        member,
        just(Token::Comma)
            .spanned()
            .map(|comma: Spanned<Token<'src>>| SequenceItem::Separator(comma.span)),
    ))
    .boxed();

    ident()
        .then(just(Token::LAngle).spanned())
        .then(
            item.repeated()
                .collect::<Vec<_>>()
                .then_ignore(just(Token::RAngle)),
        )
        .map_with(|((name, opening), items), context| {
            classify(
                name,
                opening.span,
                items,
                context.span(),
                &context.state().0,
            )
        })
}

fn classify(
    name: Spanned<String>,
    opening: SimpleSpan,
    items: Vec<SequenceItem<ParsedTypeArgument>>,
    application_span: SimpleSpan,
    state: &ParserState,
) -> ParsedTypeExpression {
    let mut trailing_comma = None;
    for problem in sequence::shape_problems(&items) {
        match problem {
            SequenceShapeProblem::MissingMember(span)
            | SequenceShapeProblem::MissingSeparator(span) => {
                return ParsedTypeExpression::recovered(GrammarProblem::unexpected(span));
            }
            SequenceShapeProblem::TrailingSeparator(span) => trailing_comma = Some(span),
        }
    }

    if items.is_empty() {
        return ParsedTypeExpression::recovered(GrammarProblem::empty_type_arguments(
            SimpleSpan::from(opening.start..application_span.end),
        ));
    }

    let arguments = items.into_iter().filter_map(|item| match item {
        SequenceItem::Member(argument) => Some(argument.inner),
        SequenceItem::Separator(_) => None,
    });

    let mut converted = Vec::new();
    for argument in arguments {
        match argument.into_result(state) {
            Ok(argument) => converted.push(argument),
            Err(problem) => {
                return ParsedTypeExpression::recovered(problem);
            }
        }
    }

    if let Some(comma) = trailing_comma {
        return ParsedTypeExpression::recovered(GrammarProblem::trailing_type_argument_comma(
            comma,
        ));
    }

    let mut arguments = converted.into_iter();
    let first_argument = arguments
        .next()
        .expect("a non-empty application has a first argument");
    ParsedTypeExpression::valid(SourceType::application(
        name.inner,
        state.source_span(name.span),
        first_argument,
        arguments.collect(),
        state.source_span(application_span),
    ))
}
