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
//!
//! [`parser`] is the declaration-kind-agnostic seam. Its caller supplies the
//! parser for the syntax immediately following a name: tables supply a schema
//! mode, fields supply a type expression and field boundary, and later function
//! or event declarations can supply their own tail. Parsing the name together
//! with that tail is what distinguishes a valid `owner record<User>` from the
//! whitespace-split name in `first name string` without copying recovery branches
//! into every declaration grammar.
//!
//! In parser signatures, `'src` owns spellings borrowed from source text and
//! `'tokens` owns the token slice Chumsky reads; `'src: 'tokens` keeps those
//! spellings alive throughout parsing. On [`parser`], `P` is the caller-supplied
//! following parser and `T` is the value that parser emits.

use aureline_ast::tokens::Token;
use chumsky::{error::Cheap, prelude::*};

use super::{
    atom::{ident, integer},
    problem::GrammarProblem,
    state::{ParserExtra, TokenInput},
    type_expression,
};

/// A valid declared name and the caller-defined syntax that follows it.
///
/// For a table header, `following` is its spanned schema mode. For a field,
/// `following` is its staged type expression. The name retains its token span so
/// the owning declaration can construct precise AST locations.
pub(super) struct ParsedDeclaredName<T> {
    pub(super) name: Spanned<String>,
    pub(super) following: T,
}

/// Parses one declared-name slot together with its declaration-specific tail.
///
/// `following` must consume enough of the surrounding construct to distinguish
/// a valid name from a malformed multi-token name. The result is `Ok` only for
/// one valid bare identifier; known invalid shapes are consumed completely and
/// returned as a directed [`GrammarProblem`]. This parser never mutates the AST.
///
/// ```text
/// table caller:  User schemafull   -> Ok(name = User, following = schemafull)
/// field caller:  first name string -> Err(IdentifierWhitespace)
/// ```
pub(super) fn parser<'tokens, 'src: 'tokens, T, P>(
    following: P,
) -> impl Parser<
    'tokens,
    TokenInput<'tokens, 'src>,
    Result<ParsedDeclaredName<T>, GrammarProblem>,
    ParserExtra,
>
where
    T: 'tokens,
    P: Parser<'tokens, TokenInput<'tokens, 'src>, T, ParserExtra> + 'tokens,
{
    let following = following.boxed();

    let valid_name = ident()
        .then(following.clone())
        .map(|(name, following)| Ok(ParsedDeclaredName { name, following }));

    // A second identifier is part of a malformed name only if the complete
    // declaration-specific tail follows it. This prevents `owner record<User>`
    // from being classified as a split name while still recognizing
    // `first name string`.
    let split_name =
        ident()
            .then(ident())
            .then(following.clone())
            .map_with(|((name, extra), _), context| {
                let state = &context.state().0;
                Err(state
                    .inline_whitespace_between(name.span, extra.span)
                    .map_or(GrammarProblem::unexpected(extra.span), |gap| {
                        GrammarProblem::identifier_whitespace(gap)
                    }))
            });

    // Integers are valid type arguments at the lexer seam. A declaration-name
    // caller supplies the context that reclassifies the same token as a leading
    // digit violation.
    let integer_name = integer()
        .then(following.clone())
        .map(|(name, _)| Err(GrammarProblem::identifier_starts_with_digit(name.span)));

    // Keep marked type shapes before the general punctuation recovery so an
    // input such as `array<string> bool` retains `<` as its first violation.
    let marked_name = marked_type_expression()
        .then(following.clone())
        .map(|(problem, _)| Err(problem));

    let punctuated_name = punctuated()
        .then(following)
        .map(|(name, _)| Err(name.into_problem()));

    choice((
        valid_name,
        split_name,
        integer_name,
        marked_name,
        punctuated_name,
    ))
}

/// Selects one structural punctuation token that may be valid outside a name.
///
/// The emitted unit value carries the token span. Classification is deliberately
/// deferred: the same `<` is valid in `array<string>` and invalid in
/// `table User<Name> schemafull {}` only because the caller is parsing a name slot.
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

/// Selects one name-like fragment that may follow structural punctuation.
///
/// Identifiers, integers, and reserved structural words can all participate in a
/// malformed compound spelling such as `User?table`. The payload is discarded;
/// only the fragment boundary is needed to consume the complete shape.
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

struct PunctuatedIdentifier {
    span: SimpleSpan,
}

impl PunctuatedIdentifier {
    const fn into_problem(self) -> GrammarProblem {
        GrammarProblem::identifier_punctuation(self.span)
    }
}

/// Consumes a declared name containing one or more structural punctuation runs.
///
/// For `User?Name,More`, this parser consumes the complete compound spelling and
/// retains the span of `?`, the first violation. It does not consume the
/// declaration-specific syntax following the name; [`parser`] composes that tail.
fn punctuated<'tokens, 'src: 'tokens>()
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

/// Reuses the complete type grammar to recognize a type-shaped declared name.
///
/// Inputs such as `array<string>` or `User[]` must be consumed as one malformed
/// name before the caller parses the declaration-specific tail. Outer-shape marks
/// from the type parser identify `<` or `[` as the name violation without
/// reimplementing recursive type syntax here.
fn marked_type_expression<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, GrammarProblem, ParserExtra> {
    type_expression::parser().try_map(|type_expression, span| {
        type_expression
            .declared_name_problem()
            .ok_or_else(|| Cheap::new(span))
    })
}
