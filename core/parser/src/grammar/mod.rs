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
mod document;
mod field;
mod name;
mod problem;
mod state;
mod table;
mod type_expression;

use aureline_ast::ast::Ast;
use chumsky::{extra, input::Input as _, prelude::*};

use crate::{SyntaxProblem, lexer::LexedSource};

use self::{problem::ParseTokensError, state::ParserState};

pub(crate) fn parse(lexed: LexedSource<'_>) -> Result<Ast, Vec<SyntaxProblem>> {
    let LexedSource {
        tokens,
        comments,
        source,
        source_len,
    } = lexed;
    let mut state = extra::SimpleState(ParserState::new(source, comments));
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
