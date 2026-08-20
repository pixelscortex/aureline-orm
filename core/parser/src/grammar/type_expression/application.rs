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

/// Parses a valid application, an empty argument list, or a trailing argument
/// comma.
///
/// The supplied `type_expression` is the complete recursive parser, so arguments
/// may themselves be applications, tuples, or unions. Integer parsing follows
/// type parsing because integers have their own token variant and cannot be
/// mistaken for a type name.
///
/// Recovery alternatives precede the valid form because all three begin with
/// `Ident LAngle`:
///
/// - `array<>` becomes [`GrammarProblem::EmptyTypeArguments`] spanning `<>`;
/// - `array<string,>` becomes
///   [`GrammarProblem::TrailingTypeArgumentComma`] spanning the final comma;
/// - `array<string>` constructs a public [`SourceType::Application`].
///
/// Every result records the outer `<` through
/// [`ParsedTypeExpression::application`]. That mark is unrelated to type
/// validity; it lets declared-name recovery later explain why
/// `table array<string> schemafull {}` is an invalid name.
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

/// Recognizes a name followed immediately by an empty `<>` pair.
///
/// This parser consumes the complete application-shaped form so an enclosing
/// parser can continue. In `box<array<>>`, for example, the inner `array<>`
/// carries `EmptyTypeArguments` through the outer `box<...>` rather than leaving
/// either closing `>` as an unexpected token.
fn empty_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTypeExpression, ParserExtra> {
    ident()
        .then(just(Token::LAngle).spanned())
        .then(just(Token::RAngle).spanned())
        .map(|((name, opening), closing)| finish_empty(&name, opening.span, closing.span))
}

/// Converts a syntactically complete non-empty application into a public node,
/// unless one of its recursive arguments already carries a problem.
///
/// Arguments are converted in source order. The first failed conversion is
/// returned through a recovered application, while later arguments are not
/// allowed to mask it. For example, `box<[int string], string?>` retains the
/// missing tuple separator in the first argument rather than the later postfix
/// optional problem.
///
/// On success, the application span covers the name through `>`, the name span
/// covers only the identifier, and each integer argument receives the current
/// source identity during conversion.
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

/// Produces the trailing-comma problem for a fully consumed application while
/// preserving any earlier nested argument problem.
///
/// `array<string,>` reports the comma. If an earlier argument was already
/// malformed, as in `box<string?,>`, the postfix-optional problem at `?` wins
/// because it appears earlier in source. The returned value still carries the
/// application's outer `<` mark for declared-name recovery.
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

/// Produces the empty-arguments problem spanning the complete `<>` pair.
///
/// The application name is retained only for its contextual mark. No public
/// empty application node is constructed because the public syntax contract
/// requires at least one argument.
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
