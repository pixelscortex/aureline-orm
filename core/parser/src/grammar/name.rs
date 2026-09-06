//! Deterministic recovery for a name slot shared by declaration parsers.
//!
//! The lexer emits a pure numeric token as [`Token::Integer`] so that type
//! arguments can preserve integer spelling. In a declaration-name slot, the
//! same token is invalid because Aureline bare names must begin with a letter
//! or underscore. This module translates that one lexical distinction into a
//! typed grammar problem without inspecting or guessing about the declaration
//! tail.

use chumsky::prelude::*;

use super::{
    atom::integer,
    problem::GrammarProblem,
    state::{ParserExtra, TokenInput},
};

/// Parses a pure integer occupying a declaration-name slot.
///
/// The caller remains responsible for parsing the syntax after the name. The
/// emitted problem covers only the integer token, so `table 1 ...` and
/// `table User ... { 1 string }` share the classification while retaining
/// their declaration-specific composition roots.
pub(super) fn leading_digit_name_problem<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, GrammarProblem, ParserExtra> {
    integer().map(|name| GrammarProblem::identifier_starts_with_digit(name.span))
}
