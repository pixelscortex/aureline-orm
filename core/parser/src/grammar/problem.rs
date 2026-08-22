//! Internal problems for malformed token shapes the grammar can recognize.
//!
//! A recovery parser consumes the whole malformed construct and returns one of
//! these values inside its normal output. That allows surrounding recursive
//! syntax to finish—for example, the application parser can consume the `>` in
//! `record<A |>`—without ever constructing an invalid public AST node.
//!
//! Spans remain Chumsky byte ranges here. [`ParseTokensError::into_syntax_problems`]
//! attaches the [`SourceId`] only at the grammar boundary.

use aureline_ast::source::SourceId;
use chumsky::{error::Cheap, prelude::SimpleSpan};

use crate::{IdentifierProblem, SyntaxProblem, problem::source_span};

#[derive(Clone, Copy)]
pub(super) enum GrammarProblem {
    IdentifierWhitespace(SimpleSpan),
    IdentifierStartsWithDigit(SimpleSpan),
    IdentifierPunctuation(SimpleSpan),
    EmptyTypeArguments(SimpleSpan),
    TrailingTypeArgumentComma(SimpleSpan),
    PostfixOptionalType(SimpleSpan),
    MissingUnionMember(SimpleSpan),
    MissingTupleMember(SimpleSpan),
    MissingTupleSeparator(SimpleSpan),
    PostfixArrayType(SimpleSpan),
    Unexpected(SimpleSpan),
}

impl GrammarProblem {
    pub(super) const fn span(self) -> SimpleSpan {
        match self {
            Self::IdentifierWhitespace(span)
            | Self::IdentifierStartsWithDigit(span)
            | Self::IdentifierPunctuation(span)
            | Self::EmptyTypeArguments(span)
            | Self::TrailingTypeArgumentComma(span)
            | Self::PostfixOptionalType(span)
            | Self::MissingUnionMember(span)
            | Self::MissingTupleMember(span)
            | Self::MissingTupleSeparator(span)
            | Self::PostfixArrayType(span)
            | Self::Unexpected(span) => span,
        }
    }
}

pub(super) enum ParseTokensError {
    Parser(Vec<Cheap<SimpleSpan>>),
    Problem(GrammarProblem),
}

impl ParseTokensError {
    pub(super) fn into_syntax_problems(self, source: SourceId) -> Vec<SyntaxProblem> {
        match self {
            Self::Parser(errors) => errors
                .into_iter()
                .map(|error| SyntaxProblem::UnexpectedToken {
                    span: source_span(source, *error.span()),
                })
                .collect(),
            Self::Problem(problem) => vec![problem.into_syntax_problem(source)],
        }
    }
}

impl GrammarProblem {
    fn into_syntax_problem(self, source: SourceId) -> SyntaxProblem {
        let span = source_span(source, self.span());
        match self {
            Self::IdentifierWhitespace(_) => SyntaxProblem::InvalidIdentifier {
                problem: IdentifierProblem::ContainsWhitespace,
                span,
            },
            Self::IdentifierStartsWithDigit(_) => SyntaxProblem::InvalidIdentifier {
                problem: IdentifierProblem::StartsWithDigit,
                span,
            },
            Self::IdentifierPunctuation(_) => SyntaxProblem::InvalidIdentifier {
                problem: IdentifierProblem::ContainsPunctuation,
                span,
            },
            Self::EmptyTypeArguments(_) => SyntaxProblem::EmptyTypeArguments { span },
            Self::TrailingTypeArgumentComma(_) => SyntaxProblem::TrailingTypeArgumentComma { span },
            Self::PostfixOptionalType(_) => SyntaxProblem::PostfixOptionalType { span },
            Self::MissingUnionMember(_) => SyntaxProblem::MissingUnionMember { span },
            Self::MissingTupleMember(_) => SyntaxProblem::MissingTupleMember { span },
            Self::MissingTupleSeparator(_) => SyntaxProblem::MissingTupleSeparator { span },
            Self::PostfixArrayType(_) => SyntaxProblem::PostfixArrayType { span },
            Self::Unexpected(_) => SyntaxProblem::UnexpectedToken { span },
        }
    }
}
