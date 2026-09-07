//! Removes comment contents from the grammar while preserving comments and
//! physical line boundaries for the AST.
//!
//! Every complete comment produces [`Lexeme::Comment`]. Line-comment newlines
//! are left outside the comment parser and are lexed normally. Block comments
//! consume their internal newlines, so [`block_occurrences`] re-emits those
//! ranges as overlapping [`Token::Newline`] occurrences.
//!
//! That distinction affects field boundaries after comments disappear:
//!
//! ```text
//! first string /* one line */ second int
//! //                            ^ UnexpectedToken: no newline separates fields
//!
//! first string /* boundary
//! inside */ second int
//! //           ^ the physical newline separates two fields
//! ```

use aureline_ast::{ast::CommentKind, tokens::Token};
use chumsky::prelude::*;

use super::{Lexeme, LexerExtra, LexerOccurrence};

pub(super) fn line<'src>() -> impl Parser<'src, &'src str, Vec<LexerOccurrence<'src>>, LexerExtra> {
    just("//")
        .then(any().and_is(text::newline().not()).repeated())
        .map_with(|_, context| {
            vec![Spanned {
                inner: Lexeme::Comment(CommentKind::Line),
                span: context.span(),
            }]
        })
}

pub(super) fn block<'src>() -> impl Parser<'src, &'src str, Vec<LexerOccurrence<'src>>, LexerExtra>
{
    just("/*")
        .ignore_then(
            choice((
                text::newline().map_with(|(), context| Some(context.span())),
                any().and_is(just("*/").not()).to(None),
            ))
            .repeated()
            .collect::<Vec<_>>(),
        )
        .then_ignore(just("*/"))
        .map_with(|newlines, context| block_occurrences(newlines, context.span()))
}

pub(super) fn unterminated_block<'src>()
-> impl Parser<'src, &'src str, Vec<LexerOccurrence<'src>>, LexerExtra> {
    just("/*")
        .ignore_then(any().repeated())
        .map_with(|(), context| {
            vec![Spanned {
                inner: Lexeme::UnterminatedBlockComment,
                span: context.span(),
            }]
        })
}

fn block_occurrences<'src>(
    newlines: Vec<Option<SimpleSpan>>,
    span: SimpleSpan,
) -> Vec<LexerOccurrence<'src>> {
    let mut occurrences = vec![Spanned {
        inner: Lexeme::Comment(CommentKind::Block),
        span,
    }];
    occurrences.extend(newlines.into_iter().flatten().map(|span| Spanned {
        inner: Lexeme::Token(Token::Newline),
        span,
    }));
    occurrences
}
