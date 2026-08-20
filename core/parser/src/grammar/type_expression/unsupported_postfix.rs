//! Recognizes unsupported postfix type spellings so diagnostics can direct the
//! author to Aureline's application syntax.
//!
//! The supported spellings are `option<T>` and `array<T>`. The familiar postfix
//! spellings `T?` and `T[]` are consumed here only to produce
//! [`crate::SyntaxProblem::PostfixOptionalType`] and
//! [`crate::SyntaxProblem::PostfixArrayType`] rather than a generic unexpected
//! token.

use aureline_ast::tokens::Token;
use chumsky::prelude::*;

use super::{
    super::state::{ParserExtra, TokenInput},
    parsed::{ParsedTypeExpression, PostfixArraySyntax},
};

/// Applies at most one `?` and then at most one `[]` to a parsed primary.
///
/// The order mirrors the source form this recovery understands:
///
/// ```text
/// string?     -> PostfixOptionalType at `?`
/// string[]    -> PostfixArrayType spanning `[]`
/// string?[]   -> PostfixOptionalType; the later `[]` is consumed but cannot
///                replace the earlier problem
/// array<>?    -> EmptyTypeArguments; `?` cannot replace a problem already
///                recovered by the primary
/// ```
///
/// Postfix recovery binds more tightly than union parsing because this parser
/// wraps a primary before [`super::union`] sees `|`. Thus `string? | int`
/// retains the problem on `?` inside the first union member.
pub(super) fn parser<'tokens, 'src: 'tokens, P>(
    primary: P,
) -> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTypeExpression, ParserExtra>
where
    P: Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTypeExpression, ParserExtra> + 'tokens,
{
    primary
        .then(just(Token::Question).spanned().or_not())
        .map(|(type_expression, question)| type_expression.with_postfix(question))
        .then(array_parser())
        .map(|(type_expression, brackets)| type_expression.with_postfix_array(brackets))
}

/// Recognizes one complete `[]` suffix and retains two diagnostic spans.
///
/// `brackets` covers both tokens for a `PostfixArrayType` problem. `opening`
/// covers only `[` because declared-name recovery reports the first punctuation
/// in `User[]`. The optional result lets the caller share one pipeline for
/// primaries with and without this suffix.
fn array_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Option<PostfixArraySyntax>, ParserExtra> {
    just(Token::LBracket)
        .spanned()
        .then(just(Token::RBracket))
        .map_with(
            |(opening, _): (Spanned<Token<'src>>, Token<'src>), context| PostfixArraySyntax {
                opening: opening.span,
                brackets: context.span(),
            },
        )
        .or_not()
}
