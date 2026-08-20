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

/// Consumes `//` through the byte immediately before the next physical newline.
///
/// The newline is deliberately excluded, so the outer lexer emits it as a
/// normal [`Token::Newline`]. For example:
///
/// ```text
/// first string // explanation
/// second int
/// ```
///
/// produces a line-comment AST occurrence followed by the newline token that
/// lets the grammar start `second int` as a new field.
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

/// Consumes a closed `/* ... */` comment and records each newline inside it.
///
/// The first `*/` closes the comment; block comments are not nested. A
/// single-line block comment returns only its comment occurrence. A multiline
/// block comment also returns one [`Token::Newline`] occurrence per physical
/// newline, allowing a comment to preserve rather than erase field separation.
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

/// Consumes `/*` and the remaining input when no earlier `*/` can close it.
///
/// ```text
/// table User schemafull {} /* still open
///                          ^^ UnterminatedBlockComment
/// ```
///
/// This alternative follows [`block`] in the outer lexer. Therefore it is a
/// recovery form for a genuinely unfinished comment, not a competing parse of
/// every valid block comment. The occurrence spans through EOF; [`super::lex`]
/// narrows the public diagnostic to the opening `/*`.
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

/// Expands one block comment into the separate output channels needed later.
///
/// The first occurrence covers the entire comment and becomes AST metadata.
/// Every collected newline span becomes a grammatical [`Token::Newline`]. The
/// ranges intentionally overlap the comment range: they describe two different
/// facts about the same source bytes rather than two disjoint tokens.
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
