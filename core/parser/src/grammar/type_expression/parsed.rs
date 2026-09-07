//! Private results carried through recursive type syntax.
//!
//! A public [`SourceType`] represents valid syntax. The private carrier also
//! holds a precise structural problem so an enclosing parser can finish consuming
//! its delimiters. For `box<array<>>`, the inner application returns a recovered
//! empty-argument problem; the outer application consumes its `>` and propagates
//! that problem. No invalid source type or partial public AST is constructed.

use aureline_ast::ast::{SourceType, TypeArgument};
use chumsky::prelude::Spanned;

use super::super::{problem::GrammarProblem, state::ParserState};

pub(in crate::grammar) struct ParsedTypeExpression {
    outcome: Result<SourceType, GrammarProblem>,
}

impl ParsedTypeExpression {
    pub(super) fn valid(source_type: SourceType) -> Self {
        Self {
            outcome: Ok(source_type),
        }
    }

    pub(super) fn recovered(problem: GrammarProblem) -> Self {
        Self {
            outcome: Err(problem),
        }
    }

    pub(in crate::grammar) fn problem(&self) -> Option<GrammarProblem> {
        self.outcome.as_ref().err().copied()
    }

    pub(in crate::grammar) fn into_result(self) -> Result<SourceType, GrammarProblem> {
        self.outcome
    }
}

pub(super) enum ParsedTypeArgument {
    Type(ParsedTypeExpression),
    Integer(Spanned<String>),
}

impl ParsedTypeArgument {
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
