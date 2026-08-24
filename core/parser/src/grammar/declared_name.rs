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
//! reinterprets a structural token as
//! [`GrammarProblem::identifier_punctuation`]. Moving this decision into the
//! lexer would either reject valid type expressions or require the lexer to
//! understand table/field grammar.
//!
//! The first structural token is reported as the identifier violation whether
//! it is attached to the name or separated from it. The source span identifies
//! the exact punctuation, so callers that need to render it can recover the
//! spelling from the source.

use aureline_ast::tokens::Token;
use chumsky::{error::Cheap, prelude::*};

use super::{
    atom::{ident, integer},
    problem::GrammarProblem,
    state::{ParserExtra, TokenInput},
    type_expression,
};

fn structural_punctuation<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Spanned<()>, ParserExtra> {
    choice((
        just(Token::LAngle),
        just(Token::RAngle),
        just(Token::LBracket),
        just(Token::RBracket),
        just(Token::Comma),
        just(Token::Question),
        just(Token::Pipe),
    ))
    .ignored()
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
    span: SimpleSpan,
}

impl PunctuatedIdentifier {
    pub(super) const fn into_problem(self) -> GrammarProblem {
        GrammarProblem::identifier_punctuation(self.span)
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
        .map(|((_, suffixes), _)| {
            let first_run = suffixes
                .first()
                .map(|(run, _)| run)
                .expect("a punctuated identifier has a punctuation suffix");
            PunctuatedIdentifier {
                span: first_run
                    .first()
                    .expect("a punctuation run is non-empty")
                    .span,
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
