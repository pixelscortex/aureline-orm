//! Internal result carriers for recursive type parsing.
//!
//! A public [`SourceType`] can represent only valid syntax. Recursive parsing,
//! however, must also carry a precise problem through enclosing tuples,
//! applications, and unions so those parsers can consume their closing tokens.
//! [`ParsedTypeExpression`] provides that private valid-or-recovered layer. The
//! field parser is the seam that finally converts it to `Result<SourceType,
//! GrammarProblem>` before any AST allocation occurs.

use aureline_ast::ast::{SourceType, TypeArgument};
use chumsky::prelude::Spanned;

use super::super::{problem::GrammarProblem, state::ParserState};

pub(in crate::grammar) struct ParsedTypeExpression {
    outcome: Result<SourceType, GrammarProblem>,
}

impl ParsedTypeExpression {
    /// Wraps a fully valid source type for use by enclosing recursive forms.
    pub(super) fn valid(source_type: SourceType) -> Self {
        Self {
            outcome: Ok(source_type),
        }
    }

    /// Carries a directed problem through enclosing delimiters without creating
    /// an invalid public [`SourceType`].
    pub(super) fn recovered(problem: GrammarProblem) -> Self {
        Self {
            outcome: Err(problem),
        }
    }

    /// Copies the staged problem when a parent must compare recovery outcomes.
    pub(in crate::grammar) fn problem(&self) -> Option<GrammarProblem> {
        self.outcome.as_ref().err().copied()
    }

    /// Resolves the private carrier at the field or argument construction seam.
    pub(in crate::grammar) fn into_result(self) -> Result<SourceType, GrammarProblem> {
        self.outcome
    }
}

/// One syntactically valid application argument before source-span conversion.
///
/// Recursive type arguments may still carry a directed recovery problem;
/// integers are always syntactically valid here and retain their token span.
pub(super) enum ParsedTypeArgument {
    Type(ParsedTypeExpression),
    Integer(Spanned<String>),
}

impl ParsedTypeArgument {
    /// Converts a staged argument to the public AST representation.
    ///
    /// Integer spans gain the source identity owned by [`ParserState`]. A
    /// recovered recursive type returns its problem instead, preventing the
    /// surrounding application from constructing a partial [`TypeArgument`].
    pub(super) fn into_result(self, state: &ParserState) -> Result<TypeArgument, GrammarProblem> {
        match self {
            Self::Type(type_expression) => type_expression.into_result().map(TypeArgument::Type),
            Self::Integer(integer) => Ok(TypeArgument::integer(
                integer.inner,
                state.source_span(integer.span),
            )),
        }
    }
}
