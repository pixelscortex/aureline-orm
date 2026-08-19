use aureline_ast::{ast::CommentKind, tokens::Token};
use chumsky::{error::Cheap, extra, prelude::*};

pub(crate) type TokenOccurrence<'src> = Spanned<Token<'src>>;
pub(crate) type LexerOccurrence<'src> = Spanned<Lexeme<'src>>;

pub(crate) enum Lexeme<'src> {
    Token(Token<'src>),
    Comment(CommentKind),
    UnterminatedBlockComment,
}

#[must_use]
pub(crate) fn lexer<'src>()
-> impl Parser<'src, &'src str, Vec<LexerOccurrence<'src>>, extra::Err<Cheap<SimpleSpan>>> {
    let newline = text::newline();
    let word = text::ident().map(|word: &'src str| match word {
        "table" => Token::Table,
        "schemafull" => Token::Schemafull,
        "schemaless" => Token::Schemaless,
        identifier => Token::Ident(identifier),
    });

    let punctuation = choice((just('{').to(Token::LBrace), just('}').to(Token::RBrace)));

    let line_comment = just("//")
        .then(any().and_is(newline.not()).repeated())
        .map_with(|_, context| {
            vec![Spanned {
                inner: Lexeme::Comment(CommentKind::Line),
                span: context.span(),
            }]
        });

    let block_comment = just("/*")
        .ignore_then(
            choice((
                newline.map_with(|(), context| Some(context.span())),
                any().and_is(just("*/").not()).to(None),
            ))
            .repeated()
            .collect::<Vec<_>>(),
        )
        .then_ignore(just("*/"))
        .map_with(|newlines, context| block_comment_occurrences(newlines, context.span()));

    let unterminated_block_comment =
        just("/*")
            .ignore_then(any().repeated())
            .map_with(|(), context| {
                vec![Spanned {
                    inner: Lexeme::UnterminatedBlockComment,
                    span: context.span(),
                }]
            });

    let syntax_token = choice((newline.to(Token::Newline), word, punctuation))
        .spanned()
        .map(|occurrence: TokenOccurrence<'src>| {
            vec![Spanned {
                inner: Lexeme::Token(occurrence.inner),
                span: occurrence.span,
            }]
        });

    let occurrence = choice((
        line_comment,
        block_comment,
        unterminated_block_comment,
        syntax_token,
    ));

    occurrence
        // Ignore spaces and tabs around Token, but preserve newlines as real
        // Token because the grammar uses them as statement/field boundaries.
        .padded_by(text::inline_whitespace())
        .repeated()
        .collect::<Vec<_>>()
        .map(|groups| groups.into_iter().flatten().collect())
}

fn block_comment_occurrences<'src>(
    newlines: Vec<Option<SimpleSpan>>,
    span: SimpleSpan,
) -> Vec<LexerOccurrence<'src>> {
    // The comment occurrence retains its whole location. Overlapping newline
    // occurrences preserve physical field boundaries after comments are removed
    // from the grammatical token stream.
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
