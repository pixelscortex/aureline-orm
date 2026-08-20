//! Builds the public AST from the lexer's grammatical token stream.
//!
//! The grammar has two ways to reject input:
//!
//! - an ordinary Chumsky mismatch means the token stream does not match any
//!   known complete shape and becomes [`SyntaxProblem::UnexpectedToken`];
//! - a recovery parser consumes a known malformed shape and returns a
//!   [`problem::GrammarProblem`], which becomes a more specific public problem
//!   such as [`SyntaxProblem::MissingUnionMember`].
//!
//! Recovery values are carried through recursive type parsing rather than
//! immediately aborting. This is what lets `record<A |>` consume the closing
//! `>` and report the missing union member at `|` instead of reporting `>` as an
//! unrelated unexpected token. No malformed public AST node is constructed.

mod atom;
mod declared_name;
mod document;
mod field;
mod problem;
mod state;
mod table;
mod type_expression;

use aureline_ast::ast::Ast;
use chumsky::{extra, input::Input as _, prelude::*};

use crate::{SyntaxProblem, lexer::LexedSource};

use self::{problem::ParseTokensError, state::ParserState};

/// Parses a lexed source document into the arena-backed AST.
///
/// [`LexedSource::tokens`] become Chumsky's mapped input, preserving each
/// token's byte span and installing an empty end-of-input span at `source_len`.
/// Comments initialize the AST builder, while inline-whitespace spans initialize
/// the contextual declared-name recovery state.
///
/// [`document::parser`] returns the earliest typed recovery problem found during
/// its single construction walk. A normal parser failure takes the separate
/// [`ParseTokensError::Parser`] path. In either error case the in-progress
/// [`ParserState`] is dropped, so callers can never observe a partial AST.
pub(crate) fn parse(lexed: LexedSource<'_>) -> Result<Ast, Vec<SyntaxProblem>> {
    let LexedSource {
        tokens,
        comments,
        inline_whitespace,
        source,
        source_len,
    } = lexed;
    let mut state = extra::SimpleState(ParserState::new(source, comments, inline_whitespace));
    let input = tokens.split_spanned(SimpleSpan::from(source_len..source_len));

    let parsed = document::parser()
        .parse_with_state(input, &mut state)
        .into_result()
        .map_err(ParseTokensError::Parser);

    match parsed {
        Ok(Some(problem)) => Err(ParseTokensError::Problem(problem).into_syntax_problems(source)),
        Ok(None) => Ok(state.0.finish()),
        Err(error) => Err(error.into_syntax_problems(source)),
    }
}
