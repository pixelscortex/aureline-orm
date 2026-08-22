//! Fixed-tuple construction and recovery for malformed comma placement.
//!
//! Tuple members are complete recursive type expressions. Empty, singleton, and
//! trailing-comma tuples are valid:
//!
//! ```text
//! []
//! [int]
//! [int,]
//! [A | B, record<C>]
//! ```
//!
//! The parser consumes the bracketed item sequence once. Ordinary Rust then
//! classifies comma placement and nested member problems, making recovery
//! precedence explicit instead of distributing it across competing parsers.

use aureline_ast::{ast::SourceType, tokens::Token};
use chumsky::prelude::*;

use super::{
    super::{
        problem::GrammarProblem,
        state::{ParserExtra, ParserState, TokenInput},
    },
    parsed::ParsedTypeExpression,
};

enum TupleItem {
    Member(Spanned<ParsedTypeExpression>),
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
    // Commas and members disagree on their first token, so this choice only
    // consumes one item at a time and never retries a complete member list.
    let item = choice((
        just(Token::Comma)
            .spanned()
            .map(|comma: Spanned<Token<'src>>| TupleItem::Comma(comma.span)),
        type_expression.spanned().map(TupleItem::Member),
    ))
    .boxed();

    item.repeated()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBracket), just(Token::RBracket))
        .map_with(|items, context| classify(items, context.span(), &context.state().0))
}

fn classify(
    items: Vec<TupleItem>,
    tuple_span: SimpleSpan,
    state: &ParserState,
) -> ParsedTypeExpression {
    let mut problems = shape_problems(&items);
    let mut members = Vec::new();

    for item in items {
        if let TupleItem::Member(member) = item {
            match member.inner.into_result() {
                Ok(member) => members.push(member),
                Err(problem) => problems.push(problem),
            }
        }
    }

    match problems
        .into_iter()
        .min_by_key(|problem| problem.span().start)
    {
        Some(problem) => ParsedTypeExpression::recovered(problem),
        None => {
            ParsedTypeExpression::valid(SourceType::tuple(members, state.source_span(tuple_span)))
        }
    }
}

fn shape_problems(items: &[TupleItem]) -> Vec<GrammarProblem> {
    items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match item {
            // `[, A]`, `[,]`, and `[,,,]` report the first comma without a
            // member on its left, matching the existing public contract.
            TupleItem::Comma(span) if index == 0 => Some(GrammarProblem::MissingTupleMember(*span)),
            // `[A,, B]` reports the second comma because no member occurs
            // between the two commas.
            TupleItem::Comma(span) if matches!(items[index - 1], TupleItem::Comma(_)) => {
                Some(GrammarProblem::MissingTupleMember(*span))
            }
            // `[A B]` reports the complete second member as the missing
            // separator's span.
            TupleItem::Member(member)
                if index > 0 && matches!(items[index - 1], TupleItem::Member(_)) =>
            {
                Some(GrammarProblem::MissingTupleSeparator(member.span))
            }
            // One trailing comma is valid tuple syntax.
            TupleItem::Comma(_) | TupleItem::Member(_) => None,
        })
        .collect()
}
