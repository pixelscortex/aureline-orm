//! Parses the single token that occupies a declaration-name slot.
//!
//! Table and Field names are exactly one bare identifier token. This module
//! does not inspect the declaration syntax that follows the name. The only
//! position-specific recovery retained here is for a pure integer, because the
//! lexer accepts integer tokens for type arguments while a name slot can
//! deterministically classify one as an identifier that starts with a digit.

use chumsky::prelude::*;

use super::{
    atom::{ident, integer},
    problem::GrammarProblem,
    state::{ParserExtra, TokenInput},
};

pub(super) type ParsedDeclaredName = Result<Spanned<String>, GrammarProblem>;

/// Consumes one declared-name token without consuming any following grammar.
///
/// `User` emits its exact spelling and token span. A pure integer such as `1`
/// emits a leading-digit problem at that token. Other token kinds fail through
/// the ordinary grammar error path. This parser never mutates the AST.
///
/// `'src` owns borrowed source spelling and `'tokens` owns the token input;
/// `'src: 'tokens` keeps spelling alive while parsing. `impl Parser` describes
/// a parser rather than an already parsed value.
pub(super) fn parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedDeclaredName, ParserExtra> {
    choice((
        ident().map(Ok),
        integer().map(|integer| Err(GrammarProblem::identifier_starts_with_digit(integer.span))),
    ))
}
