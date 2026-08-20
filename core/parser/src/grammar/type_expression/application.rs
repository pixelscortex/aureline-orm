//! Type application parsing, argument conversion, and incomplete-list recovery.
//!
//! A type application is an unresolved name followed by one or more comma-
//! separated arguments in angle brackets:
//!
//! ```text
//! array<string>
//! array<float, 3>
//! custom_type<record<A | B>, 003>
//! ```
//!
//! An argument is either a complete recursive type expression or an integer.
//! The parser preserves integer spelling and does not validate the application
//! name, arity, or semantics. Two known incomplete forms receive directed
//! problems: `array<>` and `array<string,>`.

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
    let arguments = argument.clone().then(
        just(Token::Comma)
            .ignore_then(argument)
            .repeated()
            .collect::<Vec<_>>(),
    );

    let application = ident()
        .then(just(Token::LAngle).spanned())
        .then(arguments.clone())
        .then_ignore(just(Token::RAngle))
        .map_with(|((name, opening), (first, rest)), context| {
            finish(
                name,
                opening.span,
                first,
                rest,
                context.span(),
                &context.state().0,
            )
        });

    let trailing_comma = ident()
        .then(just(Token::LAngle).spanned())
        .then(arguments.then(just(Token::Comma).spanned()))
        .then_ignore(just(Token::RAngle))
        .map_with(|((name, opening), ((first, rest), comma)), context| {
            finish_trailing_comma(
                &name,
                opening.span,
                first,
                rest,
                comma.span,
                &context.state().0,
            )
        });

    choice((empty_parser(), trailing_comma, application))
}

fn empty_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTypeExpression, ParserExtra> {
    ident()
        .then(just(Token::LAngle).spanned())
        .then(just(Token::RAngle).spanned())
        .map(|((name, opening), closing)| finish_empty(&name, opening.span, closing.span))
}

fn finish(
    name: Spanned<String>,
    opening: SimpleSpan,
    first_argument: ParsedTypeArgument,
    remaining_arguments: Vec<ParsedTypeArgument>,
    application_span: SimpleSpan,
    state: &ParserState,
) -> ParsedTypeExpression {
    let mut arguments = std::iter::once(first_argument)
        .chain(remaining_arguments)
        .map(|argument| argument.into_result(state));
    let first_argument = match arguments.next().expect("application is non-empty") {
        Ok(argument) => argument,
        Err(problem) => {
            return ParsedTypeExpression::application(Err(problem), name.span, opening);
        }
    };
    let remaining_arguments = match arguments.collect::<Result<Vec<_>, _>>() {
        Ok(arguments) => arguments,
        Err(problem) => {
            return ParsedTypeExpression::application(Err(problem), name.span, opening);
        }
    };
    ParsedTypeExpression::application(
        Ok(SourceType::application(
            name.inner,
            state.source_span(name.span),
            first_argument,
            remaining_arguments,
            state.source_span(application_span),
        )),
        name.span,
        opening,
    )
}

fn finish_trailing_comma(
    name: &Spanned<String>,
    opening: SimpleSpan,
    first_argument: ParsedTypeArgument,
    remaining_arguments: Vec<ParsedTypeArgument>,
    comma: SimpleSpan,
    state: &ParserState,
) -> ParsedTypeExpression {
    let nested_problem = std::iter::once(first_argument)
        .chain(remaining_arguments)
        .find_map(|argument| argument.into_result(state).err());
    ParsedTypeExpression::application(
        Err(nested_problem.unwrap_or(GrammarProblem::TrailingTypeArgumentComma(comma))),
        name.span,
        opening,
    )
}

fn finish_empty(
    name: &Spanned<String>,
    opening: SimpleSpan,
    closing: SimpleSpan,
) -> ParsedTypeExpression {
    ParsedTypeExpression::application(
        Err(GrammarProblem::EmptyTypeArguments(SimpleSpan::from(
            opening.start..closing.end,
        ))),
        name.span,
        opening,
    )
}
