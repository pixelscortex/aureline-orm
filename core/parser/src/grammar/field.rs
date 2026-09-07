//! Parses one physical table Field into a staged Field or structural problem.
//!
//! The complete Field grammar is `identifier type-expression`, terminated by a
//! physical newline or the table's closing `}`. The terminator is only observed
//! here; the table body consumes it. Type names remain meaning-free, so
//! `owner record<User | Bot>` stages the exact name and source type without
//! deciding whether any referenced type exists.
//!
//! Staging keeps allocation atomic at the surrounding table. A valid Field does
//! not receive an arena ID until the complete table has parsed without a header
//! or sibling Field problem. For malformed `first name string`, this parser
//! consumes `first` as the name and `name` as the type, then reports the extra
//! `string` through the ordinary unexpected-token path; it does not reconstruct
//! `first name` as a malformed identifier.

use aureline_ast::{TableFieldBuilder, ast::SourceType, source::SourceSpan, tokens::Token};
use chumsky::prelude::*;

use super::{
    declared_name,
    problem::GrammarProblem,
    state::{ParserExtra, TokenInput},
    type_expression,
};

/// A valid Field staged until its surrounding table is known to be valid.
///
/// It owns the exact name, source type, and spans needed for allocation, but no
/// [`FieldId`](aureline_ast::ids::FieldId) yet. Delaying the ID prevents a bad
/// sibling Field from leaving a partial table in the AST.
pub(super) struct ParsedField {
    span: SourceSpan,
    name: String,
    name_span: SourceSpan,
    source_type: SourceType,
}

impl ParsedField {
    /// Allocates this staged Field under the table being atomically constructed.
    pub(super) fn alloc_in(self, fields: &mut TableFieldBuilder<'_>) {
        fields.alloc_field(self.span, self.name, self.name_span, self.source_type);
    }
}

/// The result of consuming one complete recognizable Field shape.
///
/// `Field` remains staged; `Problem` carries a directed name or structural type
/// problem. Neither variant mutates the AST while this parser runs.
pub(super) enum FieldOutcome {
    Field(ParsedField),
    Problem(GrammarProblem),
}

/// Parses `<declared-name> <type-expression>` up to a physical Field boundary.
///
/// The parser visibly composes the name and type in source order. Its final
/// `rewind` observes a newline or `}` without consuming it because the table body
/// owns separators and the closing delimiter. A structurally valid result is
/// staged for later allocation; a recovered type or integer-name problem is
/// returned as [`FieldOutcome::Problem`].
///
/// In this signature, `'src` owns borrowed source spelling and `'tokens` owns
/// the token input; `'src: 'tokens` keeps spelling alive while parsing.
/// `impl Parser` describes a parser rather than an already parsed Field.
pub(super) fn parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, FieldOutcome, ParserExtra> {
    let field_end = choice((just(Token::Newline), just(Token::RBrace)))
        .ignored()
        .rewind();

    declared_name::parser()
        .then(type_expression::parser())
        .then_ignore(field_end)
        .map_with(
            |(name, source_type), context| match (name, source_type.into_result()) {
                (Err(problem), _) | (_, Err(problem)) => FieldOutcome::Problem(problem),
                (Ok(name), Ok(source_type)) => {
                    let field_span = context.span();
                    let state = &context.state().0;
                    FieldOutcome::Field(ParsedField {
                        span: state.source_span(field_span),
                        name: name.inner,
                        name_span: state.source_span(name.span),
                        source_type,
                    })
                }
            },
        )
}
