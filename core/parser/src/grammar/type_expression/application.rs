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
};

enum ApplicationItem {
    Argument(Spanned<ParsedTypeArgument>),
    Comma(SimpleSpan),
}

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
    let item = choice((
        argument.spanned().map(ApplicationItem::Argument),
        just(Token::Comma)
            .spanned()
            .map(|comma: Spanned<Token<'src>>| ApplicationItem::Comma(comma.span)),
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
    items: Vec<ApplicationItem>,
    application_span: SimpleSpan,
    state: &ParserState,
) -> ParsedTypeExpression {
    if let Some(problem) = malformed_shape(&items) {
        return ParsedTypeExpression::application(Err(problem), name.span, opening);
    }

    if items.is_empty() {
        return ParsedTypeExpression::application(
            Err(GrammarProblem::EmptyTypeArguments(SimpleSpan::from(
                opening.start..application_span.end,
            ))),
            name.span,
            opening,
        );
    }

    let trailing_comma = match items.last() {
        Some(ApplicationItem::Comma(span)) => Some(*span),
        _ => None,
    };
    let arguments = items.into_iter().filter_map(|item| match item {
        ApplicationItem::Argument(argument) => Some(argument.inner),
        ApplicationItem::Comma(_) => None,
    });

    let mut converted = Vec::new();
    for argument in arguments {
        match argument.into_result(state) {
            Ok(argument) => converted.push(argument),
            Err(problem) => {
                return ParsedTypeExpression::application(Err(problem), name.span, opening);
            }
        }
    }

    if let Some(comma) = trailing_comma {
        return ParsedTypeExpression::application(
            Err(GrammarProblem::TrailingTypeArgumentComma(comma)),
            name.span,
            opening,
        );
    }

    let mut arguments = converted.into_iter();
    let first_argument = arguments
        .next()
        .expect("a non-empty application has a first argument");
    ParsedTypeExpression::application(
        Ok(SourceType::application(
            name.inner,
            state.source_span(name.span),
            first_argument,
            arguments.collect(),
            state.source_span(application_span),
        )),
        name.span,
        opening,
    )
}

fn malformed_shape(items: &[ApplicationItem]) -> Option<GrammarProblem> {
    for (index, item) in items.iter().enumerate() {
        match item {
            ApplicationItem::Comma(span)
                if index == 0 || matches!(items[index - 1], ApplicationItem::Comma(_)) =>
            {
                return Some(GrammarProblem::Unexpected(*span));
            }
            ApplicationItem::Argument(argument)
                if index > 0 && matches!(items[index - 1], ApplicationItem::Argument(_)) =>
            {
                return Some(GrammarProblem::Unexpected(argument.span));
            }
            ApplicationItem::Argument(_) | ApplicationItem::Comma(_) => {}
        }
    }
    None
}
