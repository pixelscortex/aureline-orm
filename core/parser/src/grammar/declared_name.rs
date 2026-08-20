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

pub(super) struct PunctuatedIdentifier {
    punctuation: char,
    span: SimpleSpan,
    adjacent: bool,
}

impl PunctuatedIdentifier {
    pub(super) const fn into_problem(self) -> GrammarProblem {
        if self.adjacent {
            GrammarProblem::IdentifierPunctuation(self.punctuation, self.span)
        } else {
            GrammarProblem::Unexpected(self.span)
        }
    }
}

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

pub(super) fn marked_type_expression<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, GrammarProblem, ParserExtra> {
    type_expression::parser().try_map(|type_expression, span| {
        type_expression
            .declared_name_problem()
            .ok_or_else(|| Cheap::new(span))
    })
}
