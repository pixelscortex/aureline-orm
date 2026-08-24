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
    sequence::{self, SequenceItem, SequenceShapeProblem},
};

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
            .map(|comma: Spanned<Token<'src>>| SequenceItem::Separator(comma.span)),
        type_expression.spanned().map(SequenceItem::Member),
    ))
    .boxed();

    item.repeated()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBracket), just(Token::RBracket))
        .map_with(|items, context| classify(items, context.span(), &context.state().0))
}

fn classify(
    items: Vec<SequenceItem<ParsedTypeExpression>>,
    tuple_span: SimpleSpan,
    state: &ParserState,
) -> ParsedTypeExpression {
    let mut problems = sequence::shape_problems(&items)
        .into_iter()
        .filter_map(|problem| match problem {
            SequenceShapeProblem::MissingMember(span) => {
                Some(GrammarProblem::missing_tuple_member(span))
            }
            SequenceShapeProblem::MissingSeparator(span) => {
                Some(GrammarProblem::missing_tuple_separator(span))
            }
            // One trailing comma is valid tuple syntax.
            SequenceShapeProblem::TrailingSeparator(_) => None,
        })
        .collect::<Vec<_>>();
    let mut members = Vec::new();

    for item in items {
        if let SequenceItem::Member(member) = item {
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
