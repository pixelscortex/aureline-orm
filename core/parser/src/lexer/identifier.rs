//! Recognizes bare identifier candidates and reports their first bad character.
//!
//! Aureline bare identifiers follow `[A-Za-z_][A-Za-z0-9_]*`. This module scans
//! a wider *identifier-shaped candidate* first, then classifies it. A candidate
//! must contain an identifier atom, but invalid punctuation may precede that
//! atom. Scanning the wider shape prevents `.User` or `User-é` from becoming
//! unrelated token errors; the result identifies the first bad byte.
//!
//! Structural type punctuation is intentionally not part of a candidate. The
//! lexer must tokenize `array<string>`, `[A, B]`, `A | B`, and `string?` before
//! it knows whether those forms are valid in their grammar position. When such
//! punctuation follows a complete declaration element, the owning grammar
//! reports it as an ordinary unexpected token.

use aureline_ast::tokens::Token;
use chumsky::prelude::*;

use crate::IdentifierProblem;

use super::{Lexeme, LexerExtra, LexerOccurrence};

pub(super) fn candidate<'src>()
-> impl Parser<'src, &'src str, Vec<LexerOccurrence<'src>>, LexerExtra> {
    let identifier_atom = || any().filter(|character: &char| is_identifier_atom(*character));
    let internal_punctuation = || {
        choice((
            any().filter(|character: &char| is_internal_identifier_punctuation(*character)),
            just('/').and_is(choice((just("//"), just("/*"))).not()),
        ))
    };

    let trailing_atoms = || {
        internal_punctuation()
            .repeated()
            .then(identifier_atom())
            .repeated()
    };
    let ordinary = identifier_atom().then(trailing_atoms()).ignored();
    let leading_punctuation = internal_punctuation()
        .repeated()
        .at_least(1)
        .then(identifier_atom())
        .then(trailing_atoms())
        .ignored();

    choice((leading_punctuation, ordinary))
        .to_slice()
        .map_with(|candidate: &'src str, context| {
            vec![identifier_occurrence(candidate, context.span())]
        })
}

pub(super) fn backtick<'src>()
-> impl Parser<'src, &'src str, Vec<LexerOccurrence<'src>>, LexerExtra> {
    just('`')
        .then(any().and_is(just('`').not()).repeated())
        .then(just('`'))
        .map_with(|_, context| {
            vec![Spanned {
                inner: Lexeme::InvalidIdentifier(IdentifierProblem::BackticksReserved),
                span: context.span(),
            }]
        })
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
                    IdentifierProblem::ContainsPunctuation
                }
                _ => return None,
            }
        };
        Some((problem, offset, character.len_utf8()))
    })
}
