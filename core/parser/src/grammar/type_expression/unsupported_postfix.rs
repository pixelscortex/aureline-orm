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
