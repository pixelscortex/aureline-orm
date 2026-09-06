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
//!
//! `A | B | C` becomes three staged members and two separators before producing
//! one source-ordered union. `A | | C` consumes the complete shape but carries
//! `MissingUnionMember` at the first offending pipe. No form allocates into the
//! AST here.
//!
//! In the parser signature, `P` parses one tighter-precedence primary. `'src`
//! owns borrowed token spellings and `'tokens` owns the input slice;
//! `impl Parser` describes the parser rather than a parsed union.

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

/// Parses an optional pipe sequence around one primary type expression.
///
/// A single member is returned unchanged. Once a pipe is present, the parser
/// consumes all following pipe/member segments and [`classify`] either creates
/// a union or emits a directed recovery result. It does not mutate parser state
/// or allocate AST nodes.
pub(super) fn parser<'tokens, 'src: 'tokens, P>(
    member: P,
) -> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTypeExpression, ParserExtra>
where
    P: Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTypeExpression, ParserExtra>
        + Clone
        + 'tokens,
{
    let member_item = member.clone().spanned().map(SequenceItem::Member);
    let pipe = just(Token::Pipe)
        .spanned()
        .map(|pipe: Spanned<Token<'src>>| SequenceItem::Separator(pipe.span));

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
                .chain(member.map(SequenceItem::Member))
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

/// Selects the earliest structural or nested problem, or constructs a union from
/// at least two valid staged members.
fn classify(
    items: Vec<SequenceItem<ParsedTypeExpression>>,
    union_span: SimpleSpan,
    state: &ParserState,
) -> ParsedTypeExpression {
    let mut problems = sequence::shape_problems(&items)
        .into_iter()
        .map(|problem| match problem {
            SequenceShapeProblem::MissingMember(span)
            | SequenceShapeProblem::TrailingSeparator(span) => {
                GrammarProblem::missing_union_member(span)
            }
            SequenceShapeProblem::MissingSeparator(span) => GrammarProblem::unexpected(span),
        })
        .collect::<Vec<_>>();
    problems.extend(items.iter().filter_map(|item| match item {
        SequenceItem::Member(member) => member.inner.problem(),
        SequenceItem::Separator(_) => None,
    }));

    if items
        .iter()
        .all(|item| matches!(item, SequenceItem::Member(_)))
    {
        return items
            .into_iter()
            .find_map(|item| match item {
                SequenceItem::Member(member) => Some(member.inner),
                SequenceItem::Separator(_) => None,
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
        SequenceItem::Member(member) => Some(match member.inner.into_result() {
            Ok(member) => member,
            Err(_) => unreachable!("member problems were classified above"),
        }),
        SequenceItem::Separator(_) => None,
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
