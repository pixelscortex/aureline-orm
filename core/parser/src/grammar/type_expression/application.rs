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
//!
//! A valid `array<float, 3>` becomes a staged application with one recursive
//! type argument and one integer argument. `array<>` consumes both angle
//! brackets and returns `EmptyTypeArguments`; `array<string,>` consumes the
//! closing bracket and returns `TrailingTypeArgumentComma`. No form allocates
//! into the AST here.
//!
//! In the parser signature, `P` is the complete recursive type parser supplied
//! by the composition root. `'src` owns borrowed token spellings and `'tokens`
//! owns the input slice; `impl Parser` describes the parser rather than a parsed
//! application.

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

/// Parses `<name><<arguments>>` as one valid-or-recovered type application.
///
/// The consumed region includes the unresolved name and both angle brackets.
/// Each item is first staged as either an argument or comma so [`classify`] can
/// apply recovery precedence after the whole list is known. The emitted
/// [`ParsedTypeExpression`] does not mutate parser state or allocate AST nodes.
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
        argument.spanned().map(SequenceItem::Member),
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

/// Converts one fully consumed application item list into a source type or its
/// earliest applicable recovery result.
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
