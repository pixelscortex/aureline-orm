//! Internal result carriers for recursive type parsing and declared-name reuse.
//!
//! A public [`SourceType`] can represent only valid syntax. Recursive parsing,
//! however, must also carry a precise problem through enclosing tuples,
//! applications, and unions so those parsers can consume their closing tokens.
//! [`ParsedTypeExpression`] provides that private valid-or-recovered layer.
//!
//! It also records selected *outer-shape marks*. The declared-name grammar reuses
//! the complete type parser to consume names such as `array<string>` or
//! `User[]`; marks let it reinterpret the outer `<` or `[` as name punctuation
//! without confusing punctuation nested inside an otherwise larger shape.

use aureline_ast::{
    ast::{SourceType, TypeArgument},
    tokens::Token,
};
use chumsky::prelude::{SimpleSpan, Spanned};

use super::super::{problem::GrammarProblem, state::ParserState};

#[derive(Clone, Copy)]
struct ApplicationMark {
    opening: SimpleSpan,
    joined_to_name: bool,
}

impl ApplicationMark {
    const fn into_declared_name_problem(self) -> GrammarProblem {
        if self.joined_to_name {
            GrammarProblem::IdentifierPunctuation(self.opening)
        } else {
            GrammarProblem::Unexpected(self.opening)
        }
    }
}

#[derive(Clone, Copy)]
struct PostfixArrayMark {
    opening: SimpleSpan,
    joined_to_type: bool,
}

impl PostfixArrayMark {
    const fn into_declared_name_problem(self) -> GrammarProblem {
        if self.joined_to_type {
            GrammarProblem::IdentifierPunctuation(self.opening)
        } else {
            GrammarProblem::Unexpected(self.opening)
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct PostfixArraySyntax {
    pub(super) opening: SimpleSpan,
    pub(super) brackets: SimpleSpan,
}

pub(in crate::grammar) struct ParsedTypeExpression {
    outcome: Result<SourceType, GrammarProblem>,
    application: Option<ApplicationMark>,
    postfix_array: Option<PostfixArrayMark>,
}

impl ParsedTypeExpression {
    pub(super) fn valid(source_type: SourceType) -> Self {
        Self {
            outcome: Ok(source_type),
            application: None,
            postfix_array: None,
        }
    }

    pub(super) fn recovered(problem: GrammarProblem) -> Self {
        Self {
            outcome: Err(problem),
            application: None,
            postfix_array: None,
        }
    }

    pub(super) fn application(
        outcome: Result<SourceType, GrammarProblem>,
        name: SimpleSpan,
        opening: SimpleSpan,
    ) -> Self {
        Self {
            outcome,
            application: Some(ApplicationMark {
                opening,
                joined_to_name: name.end == opening.start,
            }),
            postfix_array: None,
        }
    }

    pub(super) fn with_postfix(mut self, question: Option<Spanned<Token<'_>>>) -> Self {
        if let (Ok(_), Some(question)) = (&self.outcome, question) {
            self.outcome = Err(GrammarProblem::PostfixOptionalType(question.span));
        }
        self
    }

    pub(super) fn with_postfix_array(mut self, syntax: Option<PostfixArraySyntax>) -> Self {
        if let (Ok(source_type), Some(syntax)) = (&self.outcome, syntax) {
            // Public type spans and parser token spans use the same source byte
            // origin. Equal end/start boundaries therefore mean no whitespace or
            // removed comment separated the type from `[`. The checked usize-to-
            // u32 conversion also keeps oversized offsets from looking joined.
            let source_end = source_type.span().range().end().get();
            self.postfix_array = Some(PostfixArrayMark {
                opening: syntax.opening,
                joined_to_type: u32::try_from(syntax.opening.start)
                    .is_ok_and(|opening| source_end == opening),
            });
            self.outcome = Err(GrammarProblem::PostfixArrayType(syntax.brackets));
        }
        self
    }

    pub(in crate::grammar) fn declared_name_problem(&self) -> Option<GrammarProblem> {
        self.application
            .map(ApplicationMark::into_declared_name_problem)
            .or_else(|| {
                self.postfix_array
                    .map(PostfixArrayMark::into_declared_name_problem)
            })
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
