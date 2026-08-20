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
//! Recovery distinguishes two authoring mistakes. A comma without a member
//! produces [`GrammarProblem::MissingTupleMember`]; an adjacent member without a
//! comma produces [`GrammarProblem::MissingTupleSeparator`]. Recovery helpers
//! keep an earlier malformed member when present rather than replacing it with a
//! later tuple error.

use aureline_ast::{ast::SourceType, tokens::Token};
use chumsky::prelude::*;

use super::{
    super::{
        problem::GrammarProblem,
        state::{ParserExtra, ParserState, TokenInput},
    },
    parsed::ParsedTypeExpression,
};

/// Parses a complete bracket-delimited tuple, including known comma recovery
/// shapes.
///
/// Missing-member recovery covers:
///
/// ```text
/// [, int]         -> leading comma
/// [int,, string]  -> second comma
/// [,]             -> lone comma
/// ```
///
/// `leading_missing_member` consumes the offending comma and the following type
/// expression as one recovered member. In `[int,, string]`, this allows the
/// second comma to own the problem while `string` is still consumed before `]`.
/// `lone_missing_member` handles a comma followed immediately by `]`.
///
/// The normal member list accepts an optional final comma separately. Therefore
/// `[int,]` is valid and does not pass through missing-member recovery.
///
/// Missing-separator recovery captures the first adjacent recursive member:
///
/// ```text
/// [int string]       -> `string`
/// [int record<A>]    -> `record<A>`
/// [A | B C]          -> `C`
/// [int [string]]     -> `[string]`
/// [int string bool]  -> first adjacent member, `string`
/// ```
///
/// It consumes the remaining tail only so the enclosing parser can reach `]`;
/// later syntax cannot replace the first missing-separator location.
pub(super) fn parser<'tokens, 'src: 'tokens, P>(
    type_expression: P,
) -> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTypeExpression, ParserExtra>
where
    P: Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTypeExpression, ParserExtra>
        + Clone
        + 'tokens,
{
    // Consume `, member` before accepting a lone comma so a recoverable member
    // is not stranded after the punctuation that caused its problem.
    let leading_missing_member = just(Token::Comma)
        .spanned()
        .then_ignore(type_expression.clone())
        .map(|comma| recover_missing_member(comma.span));
    let lone_missing_member = just(Token::Comma)
        .spanned()
        .map(|comma: Spanned<Token<'src>>| recover_missing_member(comma.span));
    let member = choice((
        leading_missing_member,
        lone_missing_member,
        type_expression.clone(),
    ))
    .boxed();
    let separated_member = just(Token::Comma).ignore_then(member.clone());

    // Try missing-separator recovery before the normal tuple shape. Both begin
    // with `[ member`, but only this branch can consume an immediately adjacent
    // second member and attach the problem to that member's complete span.
    let missing_separator = member
        .clone()
        .then(separated_member.clone().repeated().collect::<Vec<_>>())
        .then(
            type_expression
                .clone()
                .map_with(|_, context| context.span()),
        )
        .then(
            choice((separated_member.clone(), member.clone()))
                .repeated()
                .collect::<Vec<_>>(),
        )
        .then_ignore(just(Token::Comma).or_not())
        .delimited_by(just(Token::LBracket), just(Token::RBracket))
        .map(|(((first, separated), adjacent_span), _)| {
            recover_missing_separator(
                std::iter::once(first).chain(separated).collect(),
                adjacent_span,
            )
        });

    let members = member
        .clone()
        .then(separated_member.repeated().collect::<Vec<_>>())
        .then_ignore(just(Token::Comma).or_not())
        .map(|(first, rest)| std::iter::once(first).chain(rest).collect::<Vec<_>>())
        .or_not()
        .map(Option::unwrap_or_default);
    let tuple = members
        .delimited_by(just(Token::LBracket), just(Token::RBracket))
        .map_with(|members, context| {
            build_tuple_or_preserve_problem(members, context.span(), &context.state().0)
        });
    choice((missing_separator, tuple))
}

/// Converts members in source order so the earliest recovered problem wins;
/// successful members remain structurally unchanged.
///
/// `[, int]` reaches this function with one recovered member and returns its
/// comma problem without constructing a public tuple. `[A, [B], C | D]` reaches
/// it with three valid nodes and constructs [`SourceType::Tuple`] spanning the
/// complete bracketed expression. An empty member vector constructs valid `[]`.
fn build_tuple_or_preserve_problem(
    members: Vec<ParsedTypeExpression>,
    tuple_span: SimpleSpan,
    state: &ParserState,
) -> ParsedTypeExpression {
    let members = match members
        .into_iter()
        .map(ParsedTypeExpression::into_result)
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(members) => members,
        Err(problem) => return ParsedTypeExpression::recovered(problem),
    };
    ParsedTypeExpression::valid(SourceType::tuple(members, state.source_span(tuple_span)))
}

/// Creates the recovered member associated with a comma that has no neighbor on
/// one side.
///
/// The span covers the comma in `[, int]`, the second comma in
/// `[int,, string]`, or the only comma in `[,]`.
fn recover_missing_member(span: SimpleSpan) -> ParsedTypeExpression {
    ParsedTypeExpression::recovered(GrammarProblem::MissingTupleMember(span))
}

/// Preserves a problem recovered in the comma-separated prefix because it is
/// earlier in source; otherwise the first adjacent member's full span identifies
/// the missing separator.
///
/// For a straightforward `[int string]`, `preceding` is valid and `string`'s
/// span becomes `MissingTupleSeparator`. If the prefix already contains a
/// missing-member or nested-type problem, that earlier problem remains the
/// result even though this parser also consumed an adjacent member.
fn recover_missing_separator(
    preceding: Vec<ParsedTypeExpression>,
    adjacent_span: SimpleSpan,
) -> ParsedTypeExpression {
    match preceding
        .into_iter()
        .find_map(|member| member.into_result().err())
    {
        Some(problem) => ParsedTypeExpression::recovered(problem),
        None => {
            ParsedTypeExpression::recovered(GrammarProblem::MissingTupleSeparator(adjacent_span))
        }
    }
}
