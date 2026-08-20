//! Union construction and recovery for missing members around `|`.
//!
//! A valid union contains at least two source-ordered members:
//!
//! ```text
//! string | int | FutureType
//! ```
//!
//! Whitespace around `|` is insignificant. Nested unions are not flattened:
//! the union inside `record<A | B>` remains an application argument, while the
//! later `| C` in `record<A | B> | C` constructs a distinct outer union.
//!
//! Missing members are recognizable malformed union shapes, not generic parse
//! failures. Every helper returns a [`ParsedTypeExpression`], allowing an
//! enclosing tuple or application to consume its delimiter while retaining the
//! earliest precise problem.

use aureline_ast::{ast::SourceType, tokens::Token};
use chumsky::prelude::*;

use super::{
    super::{
        problem::GrammarProblem,
        state::{ParserExtra, ParserState, TokenInput},
    },
    parsed::ParsedTypeExpression,
};

/// Parses one member unchanged, two or more pipe-separated members as a union,
/// or a known missing-member shape as recovery.
///
/// The supplied `member` parser already includes primary and unsupported
/// postfix forms but not another same-level union. This gives `|` the loosest
/// precedence at the current recursion level.
///
/// Recovery covers every tested missing-member position:
///
/// ```text
/// | string          -> MissingUnionMember at the leading `|`
/// string |          -> MissingUnionMember at the trailing `|`
/// string | | int    -> MissingUnionMember at the second `|`
/// record<|>         -> MissingUnionMember at the lone `|`
/// ```
///
/// `leading_missing_member` consumes both `|` and the following member. That is
/// important in `string | | int`: the second pipe and `int` become one recovered
/// member, allowing the entire outer union shape to finish. The lone-pipe branch
/// handles cases where a delimiter such as `>` follows immediately.
pub(super) fn parser<'tokens, 'src: 'tokens, P>(
    member: P,
) -> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTypeExpression, ParserExtra>
where
    P: Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTypeExpression, ParserExtra>
        + Clone
        + 'tokens,
{
    let leading_missing_member = just(Token::Pipe)
        .spanned()
        .then_ignore(member.clone())
        .map(|pipe| recover_missing_member(pipe.span));
    let lone_missing_member = just(Token::Pipe)
        .spanned()
        .map(|pipe: Spanned<Token<'src>>| recover_missing_member(pipe.span));

    // Claim `| member` before the lone-pipe fallback. Reversing them would let
    // the fallback consume only `|` and strand the following `int` in
    // `string | | int` outside the recovered union.
    let union_member =
        choice((leading_missing_member, lone_missing_member, member.clone())).boxed();
    let separated_members = union_member
        .clone()
        .then(
            just(Token::Pipe)
                .ignore_then(union_member.clone())
                .repeated()
                .collect::<Vec<_>>(),
        )
        .boxed();
    let trailing_union = separated_members
        .clone()
        .then(just(Token::Pipe).spanned())
        .map(|((first, rest), pipe)| recover_trailing_pipe(first, rest, pipe.span));
    let union = union_member
        .clone()
        .then(
            just(Token::Pipe)
                .ignore_then(union_member.clone())
                .repeated()
                .at_least(1)
                .collect::<Vec<_>>(),
        )
        .map_with(|(first, rest), context| {
            build_union_or_preserve_problem(first, rest, context.span(), &context.state().0)
        });

    // Trailing recovery precedes a complete union because `A |` shares the
    // complete union's prefix. Both union alternatives precede a single member
    // so pipe syntax cannot be left behind as a generic outer failure.
    choice((trailing_union, union, union_member))
}

/// Converts members in source order so the earliest recovered problem wins.
/// Successful members remain structurally unchanged—including nested unions—
/// before construction of the outer union.
///
/// For `array<> | string?`, conversion encounters the empty-arguments problem
/// in the first member before the postfix-optional problem in the second and
/// returns it unchanged. Only when every member is valid does this function
/// construct [`SourceType::Union`] with a span covering the complete expression.
fn build_union_or_preserve_problem(
    first_member: ParsedTypeExpression,
    remaining_members: Vec<ParsedTypeExpression>,
    union_span: SimpleSpan,
    state: &ParserState,
) -> ParsedTypeExpression {
    let mut members = std::iter::once(first_member)
        .chain(remaining_members)
        .map(ParsedTypeExpression::into_result);
    let first_member = match members.next().expect("a union has at least two members") {
        Ok(member) => member,
        Err(problem) => return ParsedTypeExpression::recovered(problem),
    };
    let second_member = match members.next().expect("a union has at least two members") {
        Ok(member) => member,
        Err(problem) => return ParsedTypeExpression::recovered(problem),
    };
    let remaining_members = match members.collect::<Result<Vec<_>, _>>() {
        Ok(members) => members,
        Err(problem) => return ParsedTypeExpression::recovered(problem),
    };
    ParsedTypeExpression::valid(SourceType::union(
        first_member,
        second_member,
        remaining_members,
        state.source_span(union_span),
    ))
}

/// Preserves an earlier recovered member problem over the later missing-member
/// pipe; the pipe is reported only when every consumed member is valid.
///
/// `string |` reports the trailing pipe. `string? |` instead retains the
/// earlier postfix-optional problem at `?`, even though the parser consumes the
/// later pipe to complete recovery.
fn recover_trailing_pipe(
    first_member: ParsedTypeExpression,
    remaining_members: Vec<ParsedTypeExpression>,
    missing: SimpleSpan,
) -> ParsedTypeExpression {
    let earlier_problem = std::iter::once(first_member)
        .chain(remaining_members)
        .find_map(|member| member.into_result().err());
    ParsedTypeExpression::recovered(
        earlier_problem.unwrap_or(GrammarProblem::MissingUnionMember(missing)),
    )
}

/// Creates the recovered member used wherever a pipe has no member on one side.
///
/// The span is always the offending pipe itself. In `| string` it is the first
/// token; in `string | | int` it is the second pipe; in `record<|>` it is the
/// lone pipe between the application delimiters.
fn recover_missing_member(span: SimpleSpan) -> ParsedTypeExpression {
    ParsedTypeExpression::recovered(GrammarProblem::MissingUnionMember(span))
}
