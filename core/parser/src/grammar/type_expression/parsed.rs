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

/// Records the `<` that proves the outer expression was parsed as an
/// application and whether it touched the application name.
#[derive(Clone, Copy)]
struct ApplicationMark {
    opening: SimpleSpan,
    joined_to_name: bool,
}

impl ApplicationMark {
    /// Reclassifies the application opener for a declared-name slot.
    ///
    /// `array<string>` reports `<` as identifier punctuation. `array <string>`
    /// reports the same token as unexpected syntax because the source gap means
    /// it was not part of one attached spelling.
    const fn into_declared_name_problem(self) -> GrammarProblem {
        if self.joined_to_name {
            GrammarProblem::IdentifierPunctuation('<', self.opening)
        } else {
            GrammarProblem::Unexpected(self.opening)
        }
    }
}

/// Records the `[` that begins an outer postfix `[]` and whether it touched the
/// preceding type expression.
#[derive(Clone, Copy)]
struct PostfixArrayMark {
    opening: SimpleSpan,
    joined_to_type: bool,
}

impl PostfixArrayMark {
    /// Reclassifies the postfix-array opener for a declared-name slot.
    ///
    /// `User[]` reports `[` as identifier punctuation; `User []` reports it as
    /// unexpected separated syntax.
    const fn into_declared_name_problem(self) -> GrammarProblem {
        if self.joined_to_type {
            GrammarProblem::IdentifierPunctuation('[', self.opening)
        } else {
            GrammarProblem::Unexpected(self.opening)
        }
    }
}

/// Token spans captured while recognizing one postfix `[]` pair.
#[derive(Clone, Copy)]
pub(super) struct PostfixArraySyntax {
    /// Span of `[` alone, used for declared-name punctuation diagnostics.
    pub(super) opening: SimpleSpan,
    /// Span of the complete `[]`, used for the directed postfix-array problem.
    pub(super) brackets: SimpleSpan,
}

/// Lets recursive parsing consume a malformed type expression without
/// constructing an invalid public [`SourceType`]. Structural marks describe
/// only the outer parsed shape so nested punctuation cannot later be
/// reclassified as part of a declaration name.
///
/// Examples of `outcome` values:
///
/// - `string` carries `Ok(SourceType::Name(...))`;
/// - `array<>` carries `Err(EmptyTypeArguments)` plus an application mark;
/// - `[int string]` carries `Err(MissingTupleSeparator)` with no outer
///   declared-name mark;
/// - `string[]` carries `Err(PostfixArrayType)` plus a postfix-array mark.
pub(in crate::grammar) struct ParsedTypeExpression {
    /// Either the complete valid public node or the earliest recovered problem.
    outcome: Result<SourceType, GrammarProblem>,
    /// Present only when the outer parsed shape is an application.
    application: Option<ApplicationMark>,
    /// Present only when a valid outer type was followed by `[]`.
    postfix_array: Option<PostfixArrayMark>,
}

impl ParsedTypeExpression {
    /// Wraps a complete valid type node with no declared-name structural mark.
    ///
    /// Bare names, tuples, and unions enter through this constructor. Application
    /// nodes use [`Self::application`] because their opening `<` matters if the
    /// whole expression later appears in a name slot.
    pub(super) fn valid(source_type: SourceType) -> Self {
        Self {
            outcome: Ok(source_type),
            application: None,
            postfix_array: None,
        }
    }

    /// Carries a typed recovery problem through recursive enclosing parsers.
    ///
    /// No placeholder [`SourceType`] is created. For example, the tuple parser
    /// can return `MissingTupleMember` for `[, int]`, after consuming through
    /// `]`, and an enclosing `box<...>` can then consume its own `>`.
    pub(super) fn recovered(problem: GrammarProblem) -> Self {
        Self {
            outcome: Err(problem),
            application: None,
            postfix_array: None,
        }
    }

    /// Wraps either a valid or recovered application and records its outer `<`.
    ///
    /// The mark is retained even for `array<>` or `array<string,>` because a
    /// complete application-shaped sequence in a declared-name slot still first
    /// violates the name boundary at `<`. `joined_to_name` compares original
    /// source byte boundaries, so comments or spaces between the name and `<`
    /// change the contextual result to unexpected syntax.
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

    /// Consumes an optional postfix `?` and records its directed problem only if
    /// the primary expression was valid.
    ///
    /// `string?` becomes `PostfixOptionalType` at `?`. If the primary already
    /// recovered—`array<>?`, for example—the earlier `EmptyTypeArguments`
    /// remains the outcome. Any application mark also remains available to
    /// declared-name recovery.
    pub(super) fn with_postfix(mut self, question: Option<Spanned<Token<'_>>>) -> Self {
        if let (Ok(_), Some(question)) = (&self.outcome, question) {
            self.outcome = Err(GrammarProblem::PostfixOptionalType(question.span));
        }
        self
    }

    /// Consumes an optional postfix `[]`, records its outer-shape mark, and
    /// reports the directed array problem only if no earlier problem exists.
    ///
    /// `string[]` becomes `PostfixArrayType` spanning both brackets. In
    /// `string?[]`, the earlier `PostfixOptionalType` remains the outcome even
    /// though `[]` is consumed. In a declared-name slot, joined `User[]` can be
    /// reclassified at `[` by [`Self::declared_name_problem`].
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

    /// Returns the first outer structural mark reinterpreted for a declared-name
    /// slot, or `None` when this expression has no relevant outer shape.
    ///
    /// Application punctuation always wins because its `<` occurs before a
    /// possible following `[]`: `array<string>[]` reports `<`, not `[`, when the
    /// entire sequence is used as a field or table name. A bare valid `User`
    /// returns `None` and remains eligible as an ordinary declared name.
    pub(in crate::grammar) fn declared_name_problem(&self) -> Option<GrammarProblem> {
        self.application
            .map(ApplicationMark::into_declared_name_problem)
            .or_else(|| {
                self.postfix_array
                    .map(PostfixArrayMark::into_declared_name_problem)
            })
    }

    /// Exposes the valid node or recovered problem to the enclosing grammar
    /// without exposing the contextual outer-shape marks publicly.
    pub(in crate::grammar) fn into_result(self) -> Result<SourceType, GrammarProblem> {
        self.outcome
    }
}

/// Recursive types already own Aureline spans or a recovered problem; integer
/// spelling keeps its parser-relative span until application construction can
/// attach the source identity without leaking parser-library locations into the
/// AST.
pub(super) enum ParsedTypeArgument {
    /// A recursive type argument such as `string`, `A | B`, or `[A, B]`.
    Type(ParsedTypeExpression),
    /// An integer argument such as `3` or `003`, with exact spelling and span.
    Integer(Spanned<String>),
}

impl ParsedTypeArgument {
    /// Converts an internal argument to public AST data or propagates its
    /// recovered type problem.
    ///
    /// Type arguments already contain source-aware AST spans. Integer spans are
    /// still parser-relative, so this is the point where [`ParserState`] attaches
    /// the current [`aureline_ast::source::SourceId`].
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
