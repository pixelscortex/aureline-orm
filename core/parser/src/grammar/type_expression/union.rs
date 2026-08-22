//! Union construction and recovery for missing members around `|`.
//!
//! A valid union contains at least two source-ordered members:
//!
//! ```text
//! string | int | FutureType
//! ```
//!
//! The parser consumes one member and each following pipe segment once. Plain
//! Rust then classifies missing members and nested problems, so the recovery
//! precedence is visible without competing whole-union parsers.

use aureline_ast::{ast::SourceType, tokens::Token};
use chumsky::prelude::*;

use super::{
    super::{
        problem::GrammarProblem,
        state::{ParserExtra, ParserState, TokenInput},
    },
    parsed::ParsedTypeExpression,
};

enum UnionItem {
    Member(Spanned<ParsedTypeExpression>),
    Pipe(SimpleSpan),
}

pub(super) fn parser<'tokens, 'src: 'tokens, P>(
    member: P,
) -> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTypeExpression, ParserExtra>
where
    P: Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTypeExpression, ParserExtra>
        + Clone
        + 'tokens,
{
    let member_item = member.clone().spanned().map(UnionItem::Member);
    let pipe = just(Token::Pipe)
        .spanned()
        .map(|pipe: Spanned<Token<'src>>| UnionItem::Pipe(pipe.span));

    // A leading pipe claims the following member, when present, so `| A`
    // remains one recoverable union rather than leaving `A` for an outer rule.
    let first = choice((
        pipe.clone()
            .then(member_item.clone().or_not())
            .map(|(pipe, member)| std::iter::once(pipe).chain(member).collect::<Vec<_>>()),
        member_item.map(|member| vec![member]),
    ));
    let segment = pipe
        .then(member.clone().spanned().or_not())
        .map(|(pipe, member)| {
            std::iter::once(pipe)
                .chain(member.map(UnionItem::Member))
                .collect::<Vec<_>>()
        });

    first
        .then(segment.repeated().collect::<Vec<_>>())
        .map(|(first, rest)| {
            first
                .into_iter()
                .chain(rest.into_iter().flatten())
                .collect()
        })
        .map_with(|items, context| classify(items, context.span(), &context.state().0))
}

fn classify(
    items: Vec<UnionItem>,
    union_span: SimpleSpan,
    state: &ParserState,
) -> ParsedTypeExpression {
    let mut problems = shape_problems(&items);
    problems.extend(items.iter().filter_map(|item| match item {
        UnionItem::Member(member) => member.inner.problem(),
        UnionItem::Pipe(_) => None,
    }));

    if items
        .iter()
        .all(|item| matches!(item, UnionItem::Member(_)))
    {
        return items
            .into_iter()
            .find_map(|item| match item {
                UnionItem::Member(member) => Some(member.inner),
                UnionItem::Pipe(_) => None,
            })
            .expect("a union parser consumes one member");
    }

    if let Some(problem) = problems
        .into_iter()
        .min_by_key(|problem| problem.span().start)
    {
        return ParsedTypeExpression::recovered(problem);
    }

    let mut members = items.into_iter().filter_map(|item| match item {
        UnionItem::Member(member) => Some(match member.inner.into_result() {
            Ok(member) => member,
            Err(_) => unreachable!("member problems were classified above"),
        }),
        UnionItem::Pipe(_) => None,
    });
    let first = members.next().expect("a union has a first member");
    let second = members.next().expect("a union has a second member");
    ParsedTypeExpression::valid(SourceType::union(
        first,
        second,
        members.collect(),
        state.source_span(union_span),
    ))
}

fn shape_problems(items: &[UnionItem]) -> Vec<GrammarProblem> {
    items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match item {
            // `| A` and `|` have no member on the left.
            UnionItem::Pipe(span) if index == 0 => Some(GrammarProblem::MissingUnionMember(*span)),
            // `A | | B` has no member between the two pipes.
            UnionItem::Pipe(span) if matches!(items[index - 1], UnionItem::Pipe(_)) => {
                Some(GrammarProblem::MissingUnionMember(*span))
            }
            // `A |` has no member on the right.
            UnionItem::Pipe(span) if index + 1 == items.len() => {
                Some(GrammarProblem::MissingUnionMember(*span))
            }
            UnionItem::Pipe(_) | UnionItem::Member(_) => None,
        })
        .collect()
}
