use aureline_ast::{ast::CommentKind, tokens::Token};
use chumsky::{error::Cheap, extra, prelude::*};

use crate::IdentifierProblem;

pub(crate) type TokenOccurrence<'src> = Spanned<Token<'src>>;
pub(crate) type LexerOccurrence<'src> = Spanned<Lexeme<'src>>;

pub(crate) enum Lexeme<'src> {
    Token(Token<'src>),
    InvalidIdentifier(IdentifierProblem),
    /// One complete space/tab run retained for grammar recovery, never emitted
    /// into the grammatical token stream.
    InlineWhitespace,
    Comment(CommentKind),
    UnterminatedBlockComment,
}

#[must_use]
pub(crate) fn lexer<'src>()
-> impl Parser<'src, &'src str, Vec<LexerOccurrence<'src>>, extra::Err<Cheap<SimpleSpan>>> {
    let newline = text::newline();
    // Keep compound malformed names in one candidate, then classify the first
    // offending character and its UTF-8 width in `identifier_problem`. Structural
    // delimiters and backticks stay outside; `/` joins only when it does not open
    // `//` or `/*`, preserving both punctuation problems and comment delimiters.
    let identifier_atom = || any().filter(|character: &char| is_identifier_atom(*character));
    let internal_punctuation = || {
        choice((
            any().filter(|character: &char| is_internal_identifier_punctuation(*character)),
            just('/').and_is(choice((just("//"), just("/*"))).not()),
        ))
    };
    let identifier_candidate = identifier_atom()
        .then(
            internal_punctuation()
                .repeated()
                .then(identifier_atom())
                .repeated(),
        )
        .to_slice()
        .map_with(|candidate: &'src str, context| {
            vec![identifier_occurrence(candidate, context.span())]
        });

    let punctuation = choice((
        just('{').to(Token::LBrace),
        just('}').to(Token::RBrace),
        just('<').to(Token::LAngle),
        just('>').to(Token::RAngle),
        just('[').to(Token::LBracket),
        just(']').to(Token::RBracket),
        just(',').to(Token::Comma),
        just('?').to(Token::Question),
        just('|').to(Token::Pipe),
    ));

    let backtick_identifier = just('`')
        .then(any().and_is(just('`').not()).repeated())
        .then(just('`'))
        .map_with(|_, context| {
            vec![Spanned {
                inner: Lexeme::InvalidIdentifier(IdentifierProblem::BackticksReserved),
                span: context.span(),
            }]
        });

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

    let syntax_token = choice((newline.to(Token::Newline), punctuation))
        .spanned()
        .map(|occurrence: TokenOccurrence<'src>| {
            vec![Spanned {
                inner: Lexeme::Token(occurrence.inner),
                span: occurrence.span,
            }]
        });

    let inline_whitespace = one_of(" \t")
        .repeated()
        .at_least(1)
        .map_with(|(), context| {
            vec![Spanned {
                inner: Lexeme::InlineWhitespace,
                span: context.span(),
            }]
        });

    let occurrence = choice((
        line_comment,
        block_comment,
        unterminated_block_comment,
        backtick_identifier,
        identifier_candidate,
        syntax_token,
        inline_whitespace,
    ));

    occurrence
        .repeated()
        .collect::<Vec<_>>()
        .map(|groups| groups.into_iter().flatten().collect())
}

fn is_identifier_atom(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || character == '_'
        || (!character.is_ascii() && !character.is_whitespace())
}

fn is_internal_identifier_punctuation(character: char) -> bool {
    character.is_ascii_punctuation()
        && !matches!(
            character,
            '_' | '`' | '{' | '}' | '<' | '>' | '[' | ']' | ',' | '?' | '|' | '/'
        )
}

fn identifier_occurrence(candidate: &str, span: SimpleSpan) -> LexerOccurrence<'_> {
    if candidate.bytes().all(|byte| byte.is_ascii_digit()) {
        return Spanned {
            inner: Lexeme::Token(Token::Integer(candidate)),
            span,
        };
    }

    if let Some((problem, offset, byte_len)) = identifier_problem(candidate) {
        return Spanned {
            inner: Lexeme::InvalidIdentifier(problem),
            span: SimpleSpan::from(span.start + offset..span.start + offset + byte_len),
        };
    }

    let token = match candidate {
        "table" => Token::Table,
        "schemafull" => Token::Schemafull,
        "schemaless" => Token::Schemaless,
        identifier => Token::Ident(identifier),
    };
    Spanned {
        inner: Lexeme::Token(token),
        span,
    }
}

fn identifier_problem(candidate: &str) -> Option<(IdentifierProblem, usize, usize)> {
    candidate.char_indices().find_map(|(offset, character)| {
        let problem = if offset == 0 && character.is_ascii_digit() {
            IdentifierProblem::StartsWithDigit
        } else if !character.is_ascii() {
            IdentifierProblem::ContainsNonAscii(character)
        } else {
            match character {
                '.' => IdentifierProblem::ContainsDot,
                '-' => IdentifierProblem::ContainsHyphen,
                punctuation if punctuation.is_ascii_punctuation() && punctuation != '_' => {
                    IdentifierProblem::ContainsPunctuation(punctuation)
                }
                _ => return None,
            }
        };
        Some((problem, offset, character.len_utf8()))
    })
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
