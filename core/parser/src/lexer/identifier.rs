//! Recognizes bare identifier candidates and reports their first bad character.
//!
//! Aureline bare identifiers follow `[A-Za-z_][A-Za-z0-9_]*`. This module scans
//! a wider *identifier-shaped candidate* first, then classifies it. Scanning the
//! wider candidate prevents a malformed spelling such as `User-é` from being
//! split into an apparently valid `User` followed by an unrelated token error;
//! the result is the more useful first violation, `-`.
//!
//! Structural type punctuation is intentionally not part of a candidate. The
//! lexer must tokenize `array<string>`, `[A, B]`, `A | B`, and `string?` before
//! it knows whether those forms are valid in their grammar position. When such
//! punctuation is attached to a table or field name,
//! The grammar's `declared_name` module reconstructs the token sequence and
//! reports the contextual identifier problem.

use aureline_ast::tokens::Token;
use chumsky::prelude::*;

use crate::IdentifierProblem;

use super::{Lexeme, LexerExtra, LexerOccurrence};

/// Scans one maximal identifier-shaped candidate and classifies it as a keyword,
/// integer, bare identifier, or [`Lexeme::InvalidIdentifier`].
///
/// The candidate includes internal non-structural punctuation only when another
/// identifier atom follows it. That lets the classifier retain the complete
/// malformed spelling while reporting the first offending character:
///
/// ```text
/// User.Name   -> InvalidIdentifier(ContainsDot) at `.`
/// User-Name   -> InvalidIdentifier(ContainsHyphen) at `-`
/// User@Name   -> InvalidIdentifier(ContainsPunctuation('@')) at `@`
/// User/Name   -> InvalidIdentifier(ContainsPunctuation('/')) at `/`
/// User-é      -> InvalidIdentifier(ContainsHyphen) at `-`, not the later `é`
/// Useré-Name  -> InvalidIdentifier(ContainsNonAscii('é')), not the later `-`
/// ```
///
/// `/` is part of a malformed candidate only when it does not begin `//` or
/// `/*`. Thus `User/Name` reports name punctuation, while
/// `User/* note */schemafull` produces `Ident("User")`, a comment, and the
/// `schemafull` keyword.
///
/// Pure digit candidates remain [`Token::Integer`] because integer type
/// arguments are legal (`array<string, 3>`). A mixed candidate such as `1User`
/// is not a pure integer and reports [`IdentifierProblem::StartsWithDigit`].
pub(super) fn candidate<'src>()
-> impl Parser<'src, &'src str, Vec<LexerOccurrence<'src>>, LexerExtra> {
    let identifier_atom = || any().filter(|character: &char| is_identifier_atom(*character));
    let internal_punctuation = || {
        choice((
            any().filter(|character: &char| is_internal_identifier_punctuation(*character)),
            just('/').and_is(choice((just("//"), just("/*"))).not()),
        ))
    };

    identifier_atom()
        .then(
            internal_punctuation()
                .repeated()
                .then(identifier_atom())
                .repeated(),
        )
        .to_slice()
        .map_with(|candidate: &'src str, context| {
            vec![identifier_occurrence(candidate, context.span())]
        })
}

/// Recognizes a complete backtick-delimited spelling as reserved identifier
/// syntax.
///
/// ```text
/// table `User` schemafull {}
///       ^^^^^^ InvalidIdentifier(BackticksReserved)
/// ```
///
/// The diagnostic covers the complete spelling because backticks reserve a
/// future embedded-SurrealQL escape hatch; the contents are not interpreted as
/// an Aureline bare identifier. An unmatched backtick does not satisfy this
/// parser and falls through to the lexer's generic [`crate::SyntaxProblem::InvalidToken`].
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

/// Returns whether a character can anchor or end an identifier-shaped
/// candidate.
///
/// Non-ASCII, non-whitespace characters are included so `Café` and `User😀`
/// remain single candidates and can receive a precise `ContainsNonAscii`
/// problem. Acceptance here therefore means "safe to scan", not "valid in an
/// Aureline identifier".
fn is_identifier_atom(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || character == '_'
        || (!character.is_ascii() && !character.is_whitespace())
}

/// Returns whether ASCII punctuation may occur *inside* the wider candidate.
///
/// `_` is valid identifier syntax. Backticks have their own parser. Braces and
/// type punctuation remain structural tokens for the grammar. `/` is handled
/// separately because it must look ahead for comment openers.
fn is_internal_identifier_punctuation(character: char) -> bool {
    character.is_ascii_punctuation()
        && !matches!(
            character,
            '_' | '`' | '{' | '}' | '<' | '>' | '[' | ']' | ',' | '?' | '|' | '/'
        )
}

/// Converts a scanned candidate into exactly one occurrence.
///
/// Classification order is observable:
///
/// 1. all-digit text becomes [`Token::Integer`];
/// 2. the first identifier violation becomes a precisely spanned
///    [`Lexeme::InvalidIdentifier`];
/// 3. exact reserved words become keyword tokens;
/// 4. every other valid candidate becomes [`Token::Ident`].
///
/// Checking integers first is what makes `3` legal as a type argument while
/// still making `3D` an identifier-shaped error at its leading `3`.
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

/// Finds the first character that violates the bare-identifier boundary.
///
/// The returned tuple is `(problem, byte_offset, byte_length)`. Byte positions
/// are required because all public source spans are byte ranges; using
/// `character.len_utf8()` makes the span cover the complete offending scalar.
/// For example, `Café` reports the two UTF-8 bytes of `é`, while `User-Name`
/// reports the one byte occupied by `-`.
///
/// This function runs only after the all-digit special case. Consequently the
/// first digit in `123` never becomes [`IdentifierProblem::StartsWithDigit`],
/// but the first digit in `123User` does.
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
