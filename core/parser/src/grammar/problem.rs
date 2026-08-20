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

/// A malformed form consumed during the construction walk so parsing can
/// select the earliest precise problem and discard any partially built AST.
#[derive(Clone, Copy)]
pub(super) enum GrammarProblem {
    /// Spaces/tabs split what otherwise occupies one declared-name slot.
    ///
    /// Trigger: `table User Profile schemafull {}`. The span covers the gap
    /// between `User` and `Profile`. The field form is `first name string`.
    IdentifierWhitespace(SimpleSpan),
    /// An all-digit token occupies a table or field name slot.
    ///
    /// Triggers: `table 1 schemafull {}` and
    /// `table T schemafull { 1 string }`. Mixed spellings such as `1User` are
    /// diagnosed earlier by the lexer.
    IdentifierStartsWithDigit(SimpleSpan),
    /// Structural type punctuation is attached to a declared name.
    ///
    /// Triggers include `table User?Name schemafull {}` and
    /// `table T schemafull { User[] string }`. The stored character and span
    /// identify the first contextual name violation.
    IdentifierPunctuation(char, SimpleSpan),
    /// A type application has no argument between its angle brackets.
    ///
    /// Trigger: `array<>`. The span covers the complete `<>` pair.
    EmptyTypeArguments(SimpleSpan),
    /// A type application closes immediately after an argument comma.
    ///
    /// Trigger: `array<string,>`. The span covers the final comma.
    TrailingTypeArgumentComma(SimpleSpan),
    /// A valid type expression is followed by unsupported postfix `?`.
    ///
    /// Trigger: `string?`. The span covers `?`; the supported spelling is
    /// `option<string>`.
    PostfixOptionalType(SimpleSpan),
    /// A `|` lacks a union member on its left, right, or both sides.
    ///
    /// Triggers: `| string`, `string |`, `string | | int`, and `record<|>`.
    /// The span covers the pipe at the missing-member boundary.
    MissingUnionMember(SimpleSpan),
    /// A tuple comma lacks a member on its left, right, or both sides.
    ///
    /// Triggers: `[, int]`, `[int,, string]`, and `[,]`. The span covers the
    /// comma at the missing-member boundary. `[int,]` is valid trailing-comma
    /// syntax and does not produce this problem.
    MissingTupleMember(SimpleSpan),
    /// A tuple member begins without a preceding comma.
    ///
    /// Triggers: `[int string]`, `[int record<A>]`, and `[[int string]]`. The
    /// span covers the complete first adjacent member (`string` or `record<A>`),
    /// not the zero-width gap where the comma should be inserted.
    MissingTupleSeparator(SimpleSpan),
    /// A valid type expression is followed by unsupported postfix `[]`.
    ///
    /// Trigger: `string[]`. The span covers `[]`; the supported spelling is
    /// `array<string>`.
    PostfixArrayType(SimpleSpan),
    /// A consumed recovery shape has no more specific typed classification.
    ///
    /// Examples include the unknown schema mode in `table User mystery {}` and
    /// separated punctuation in `table User ? Name schemafull {}`.
    Unexpected(SimpleSpan),
}

impl GrammarProblem {
    /// Returns the source-relative span used to order recovered problems.
    ///
    /// When multiple tables or fields are malformed, grammar construction keeps
    /// the problem whose span starts earliest in the source.
    pub(super) const fn span(self) -> SimpleSpan {
        match self {
            Self::IdentifierWhitespace(span)
            | Self::IdentifierStartsWithDigit(span)
            | Self::IdentifierPunctuation(_, span)
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

/// Separates ordinary parser mismatches from successfully consumed typed
/// recovery forms.
pub(super) enum ParseTokensError {
    /// Chumsky could not match a complete known token shape. Each error becomes
    /// a public `UnexpectedToken`.
    Parser(Vec<Cheap<SimpleSpan>>),
    /// The grammar matched a known malformed form and retained its precise
    /// domain-specific classification.
    Problem(GrammarProblem),
}

impl ParseTokensError {
    /// Attaches the source identity and exposes parser failures through the
    /// public [`SyntaxProblem`] vocabulary.
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
    /// Maps one internal recovery category to its public problem while
    /// preserving the exact source bytes selected by the recovery parser.
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
            Self::IdentifierPunctuation(punctuation, _) => SyntaxProblem::InvalidIdentifier {
                problem: IdentifierProblem::ContainsPunctuation(punctuation),
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
