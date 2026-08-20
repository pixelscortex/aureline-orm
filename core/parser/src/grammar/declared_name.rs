//! Contextual recovery for malformed declaration names.
//!
//! Table and field names are bare identifiers. The lexer can directly diagnose
//! characters that are never structural Aureline syntax—`.` in `User.Name`, for
//! example—but it cannot reject every punctuation character globally:
//!
//! ```text
//! table T schemafull { value array<string> }  // `<` and `>` are valid here
//! table User?Name schemafull {}               // `?` violates this name
//! ```
//!
//! The same `<`, `>`, `[`, `]`, `,`, `?`, and `|` tokens therefore reach the
//! grammar. This module is called only from declared-name positions and
//! reinterprets an attached structural token as
//! [`GrammarProblem::IdentifierPunctuation`]. Moving this decision into the
//! lexer would either reject valid type expressions or require the lexer to
//! understand table/field grammar.
//!
//! Adjacency is part of the classification. `User?Name` is one malformed name,
//! so the diagnostic points at `?`. `User ? Name` is three separated syntax
//! fragments, so the same token becomes [`GrammarProblem::Unexpected`]. The
//! lexer removes spaces/tabs from the grammatical stream but retains their spans
//! in [`super::state::ParserState`] so this module can recover that distinction.

use aureline_ast::tokens::Token;
use chumsky::{error::Cheap, prelude::*};

use super::{
    atom::{ident, integer},
    problem::GrammarProblem,
    state::{ParserExtra, TokenInput},
    type_expression,
};

/// Selects punctuation that is valid somewhere in the type grammar but invalid
/// when attached to a declared name.
///
/// Braces are omitted because they delimit table bodies rather than type
/// expressions. Non-structural identifier punctuation such as `.`, `-`, `@`,
/// and `/` is already classified by the lexer's `identifier` module.
fn structural_punctuation<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Spanned<char>, ParserExtra> {
    choice((
        just(Token::LAngle).to('<'),
        just(Token::RAngle).to('>'),
        just(Token::LBracket).to('['),
        just(Token::RBracket).to(']'),
        just(Token::Comma).to(','),
        just(Token::Question).to('?'),
        just(Token::Pipe).to('|'),
    ))
    .spanned()
}

/// Consumes a token spelling that can continue a reconstructed identifier
/// candidate after structural punctuation.
///
/// Integer and structural-word tokens are not valid declared names themselves,
/// but their original spellings can follow punctuation inside a malformed name:
///
/// ```text
/// User?1
/// User?table
/// User?schemafull
/// ```
///
/// Accepting these fragments lets [`punctuated`] consume the complete malformed
/// candidate and retain the earlier `?` diagnostic. Without them, parsing would
/// stop before the suffix and likely surface a less useful generic token error.
fn fragment<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Spanned<()>, ParserExtra> {
    choice((
        ident().ignored(),
        integer().ignored(),
        just(Token::Table).ignored(),
        just(Token::Schemafull).ignored(),
        just(Token::Schemaless).ignored(),
    ))
    .spanned()
}

/// The first structural punctuation found in a reconstructed name plus whether
/// its first suffix is physically joined to the initial identifier.
///
/// Only the first violation is retained. In `User?Name,More`, `punctuation` and
/// `span` identify `?`; the later comma is consumed so it cannot mask that
/// earlier problem.
pub(super) struct PunctuatedIdentifier {
    punctuation: char,
    span: SimpleSpan,
    adjacent: bool,
}

impl PunctuatedIdentifier {
    /// Converts joined punctuation into an identifier problem and separated
    /// punctuation into ordinary unexpected syntax.
    ///
    /// `User?Name` produces `IdentifierPunctuation('?')`; `User ? Name`
    /// produces `Unexpected` at `?`. Both shapes use the same lexer tokens, and
    /// the retained whitespace span is what changes the result.
    pub(super) const fn into_problem(self) -> GrammarProblem {
        if self.adjacent {
            GrammarProblem::IdentifierPunctuation(self.punctuation, self.span)
        } else {
            GrammarProblem::Unexpected(self.span)
        }
    }
}

/// Reconstructs a declared-name candidate split by structural type punctuation.
///
/// The accepted shape is:
///
/// ```text
/// identifier (punctuation+ fragment)+ punctuation*
/// ```
///
/// Concrete matches include `User?Name`, `User??Name`, `User?1`,
/// `User?table`, and `User?Name,More`. The complete shape is consumed so later
/// punctuation or whitespace cannot mask its first violation.
///
/// Only the first punctuation run and following fragment determine adjacency.
/// This is enough because that run contains the earliest possible violation. In
/// `User?Name , More`, the joined `?` is still reported even though a later gap
/// precedes the comma. In `User ? Name`, the first run is separated and becomes
/// unexpected syntax rather than an identifier problem.
///
/// Complete application-shaped names such as `array<string>` and postfix-array
/// names such as `User[]` are handled by [`marked_type_expression`] before this
/// general reconstruction parser.
pub(super) fn punctuated<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, PunctuatedIdentifier, ParserExtra> {
    let punctuation_run = structural_punctuation()
        .repeated()
        .at_least(1)
        .collect::<Vec<_>>()
        .boxed();
    let suffix = punctuation_run.clone().then(fragment());

    ident()
        .then(suffix.repeated().at_least(1).collect::<Vec<_>>())
        .then(punctuation_run.or_not())
        .map(|((first_name, suffixes), _)| {
            let (first_run, next_name) = suffixes
                .first()
                .expect("a punctuated identifier has a punctuation suffix");
            let first_punctuation = first_run.first().expect("a punctuation run is non-empty");
            let mut previous_end = first_name.span.end;
            let mut adjacent = true;
            for punctuation in first_run {
                adjacent &= previous_end == punctuation.span.start;
                previous_end = punctuation.span.end;
            }
            adjacent &= previous_end == next_name.span.start;
            PunctuatedIdentifier {
                punctuation: first_punctuation.inner,
                span: first_punctuation.span,
                adjacent,
            }
        })
}

/// Consumes a complete outer type-expression shape carrying a declared-name
/// mark and returns its contextual problem.
///
/// Two complete shapes carry marks:
///
/// - `array<string>` is an application mark. When used as a name, its `<`
///   becomes `IdentifierPunctuation('<')`.
/// - `User[]` is a postfix-array mark. When used as a name, its `[` becomes
///   `IdentifierPunctuation('[')`.
///
/// The whole type shape is consumed before the caller parses the following
/// schema mode or field type. This prevents a general compound-name branch from
/// swallowing pieces of the type expression and losing the first punctuation.
/// For `array<string,3>`, the diagnostic remains on the opening `<`, not the
/// later comma.
///
/// A space changes the interpretation: `array <string>` and `User []` return
/// [`GrammarProblem::Unexpected`] at the opener. An unmarked bare type name does
/// not match this parser at all, because an ordinary identifier is a valid name.
pub(super) fn marked_type_expression<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, GrammarProblem, ParserExtra> {
    type_expression::parser().try_map(|type_expression, span| {
        type_expression
            .declared_name_problem()
            .ok_or_else(|| Cheap::new(span))
    })
}
