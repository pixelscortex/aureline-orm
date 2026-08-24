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
pub(super) struct GrammarProblem {
    kind: GrammarProblemKind,
    span: SimpleSpan,
}

#[derive(Clone, Copy)]
enum GrammarProblemKind {
    IdentifierWhitespace,
    IdentifierStartsWithDigit,
    IdentifierPunctuation,
    EmptyTypeArguments,
    TrailingTypeArgumentComma,
    PostfixOptionalType,
    MissingUnionMember,
    MissingTupleMember,
    MissingTupleSeparator,
    PostfixArrayType,
    Unexpected,
}

impl GrammarProblem {
    pub(super) const fn identifier_whitespace(span: SimpleSpan) -> Self {
        Self::new(GrammarProblemKind::IdentifierWhitespace, span)
    }

    pub(super) const fn identifier_starts_with_digit(span: SimpleSpan) -> Self {
        Self::new(GrammarProblemKind::IdentifierStartsWithDigit, span)
    }

    pub(super) const fn identifier_punctuation(span: SimpleSpan) -> Self {
        Self::new(GrammarProblemKind::IdentifierPunctuation, span)
    }

    pub(super) const fn empty_type_arguments(span: SimpleSpan) -> Self {
        Self::new(GrammarProblemKind::EmptyTypeArguments, span)
    }

    pub(super) const fn trailing_type_argument_comma(span: SimpleSpan) -> Self {
        Self::new(GrammarProblemKind::TrailingTypeArgumentComma, span)
    }

    pub(super) const fn postfix_optional_type(span: SimpleSpan) -> Self {
        Self::new(GrammarProblemKind::PostfixOptionalType, span)
    }

    pub(super) const fn missing_union_member(span: SimpleSpan) -> Self {
        Self::new(GrammarProblemKind::MissingUnionMember, span)
    }

    pub(super) const fn missing_tuple_member(span: SimpleSpan) -> Self {
        Self::new(GrammarProblemKind::MissingTupleMember, span)
    }

    pub(super) const fn missing_tuple_separator(span: SimpleSpan) -> Self {
        Self::new(GrammarProblemKind::MissingTupleSeparator, span)
    }

    pub(super) const fn postfix_array_type(span: SimpleSpan) -> Self {
        Self::new(GrammarProblemKind::PostfixArrayType, span)
    }

    pub(super) const fn unexpected(span: SimpleSpan) -> Self {
        Self::new(GrammarProblemKind::Unexpected, span)
    }

    const fn new(kind: GrammarProblemKind, span: SimpleSpan) -> Self {
        Self { kind, span }
    }

    pub(super) const fn span(self) -> SimpleSpan {
        self.span
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
        let span = source_span(source, self.span);
        match self.kind {
            GrammarProblemKind::IdentifierWhitespace => SyntaxProblem::InvalidIdentifier {
                problem: IdentifierProblem::ContainsWhitespace,
                span,
            },
            GrammarProblemKind::IdentifierStartsWithDigit => SyntaxProblem::InvalidIdentifier {
                problem: IdentifierProblem::StartsWithDigit,
                span,
            },
            GrammarProblemKind::IdentifierPunctuation => SyntaxProblem::InvalidIdentifier {
                problem: IdentifierProblem::ContainsPunctuation,
                span,
            },
            GrammarProblemKind::EmptyTypeArguments => SyntaxProblem::EmptyTypeArguments { span },
            GrammarProblemKind::TrailingTypeArgumentComma => {
                SyntaxProblem::TrailingTypeArgumentComma { span }
            }
            GrammarProblemKind::PostfixOptionalType => SyntaxProblem::PostfixOptionalType { span },
            GrammarProblemKind::MissingUnionMember => SyntaxProblem::MissingUnionMember { span },
            GrammarProblemKind::MissingTupleMember => SyntaxProblem::MissingTupleMember { span },
            GrammarProblemKind::MissingTupleSeparator => {
                SyntaxProblem::MissingTupleSeparator { span }
            }
            GrammarProblemKind::PostfixArrayType => SyntaxProblem::PostfixArrayType { span },
            GrammarProblemKind::Unexpected => SyntaxProblem::UnexpectedToken { span },
        }
    }
}
