//! Parses one physical table field into a staged field or recoverable problem.
//!
//! A successful field has the token shape `identifier type-expression`, for
//! example `name string` or `coordinates array<float, 3>`. The parser does not
//! decide what a type name means; it only preserves the source expression.
//!
//! Parsing proceeds in three stages:
//!
//! 1. The shared declared-name parser recognizes a name together with the field's
//!    type and physical boundary. This keeps a short recovery from succeeding
//!    while leaving tokens that hide a more precise problem.
//! 2. A valid name produces [`ParsedField`], while type-expression and
//!    declared-name recovery produces [`GrammarProblem`]. A consumed recursive
//!    type can therefore carry a problem without becoming an invalid public
//!    [`SourceType`].
//! 3. The table parser compares staged field and header problems by source
//!    position. Only a problem-free table allocates its fields, so rejected input
//!    never leaves a partial table in the AST.
//!
//! Representative flows:
//!
//! ```text
//! owner record<User | Bot>
//!   -> FieldOutcome::Field(name = "owner", type = application(record, union(User, Bot)))
//!   -> FieldDecl after the surrounding table commits
//!
//! first name string
//!   -> `first` is the name and `name` is unexpected after its type slot
//!   -> no FieldDecl allocation
//! ```
//!
//! In the parser signature, `'src` owns spellings borrowed from source text and
//! `'tokens` owns the token slice Chumsky reads; `'src: 'tokens` keeps those
//! spellings alive throughout parsing. `impl Parser` is the parser definition,
//! not a parsed field value.

use aureline_ast::{TableFieldBuilder, ast::SourceType, source::SourceSpan, tokens::Token};
use chumsky::prelude::*;

use super::{
    atom::{ident, integer},
    problem::GrammarProblem,
    state::{ParserExtra, TokenInput},
    type_expression,
};

/// A valid field staged until its surrounding table is known to be valid.
///
/// It owns the exact name, source type, and spans needed for allocation, but no
/// [`FieldId`](aureline_ast::ids::FieldId) yet. Delaying the ID prevents a bad
/// sibling field from leaving a partial table in the AST.
pub(super) struct ParsedField {
    span: SourceSpan,
    name: String,
    name_span: SourceSpan,
    source_type: SourceType,
}

impl ParsedField {
    /// Allocates this staged field under the table being atomically constructed.
    pub(super) fn alloc_in(self, fields: &mut TableFieldBuilder<'_>) {
        fields.alloc_field(self.span, self.name, self.name_span, self.source_type);
    }
}

/// The result of consuming one complete recognizable field shape.
///
/// `Field` remains staged; `Problem` carries a directed name or type problem.
/// Neither variant mutates the AST while the field parser runs.
pub(super) enum FieldOutcome {
    Field(ParsedField),
    Problem(GrammarProblem),
}

/// Parses `<declared-name> <type-expression>` up to a physical field boundary.
///
/// The parser consumes the name and type, then looks ahead for newline or `}`
/// without consuming that boundary; the table body owns separators and its
/// closing delimiter. It returns [`FieldOutcome::Field`] for valid syntax or
/// [`FieldOutcome::Problem`] for a recognized malformed name/type. Allocation is
/// deferred to [`ParsedField::alloc_in`] after the table selects no problem.
pub(super) fn parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, FieldOutcome, ParserExtra> {
    // Every declared-name alternative must reach a physical field boundary
    // before it can win. `rewind` makes this a lookahead: the table body still
    // consumes the newline or `}` after field classification. Without the guard,
    // a short recovery could leave tokens that hide a more precise field problem.
    let field_end = || {
        choice((just(Token::Newline), just(Token::RBrace)))
            .ignored()
            .rewind()
    };

    let named = ident()
        .then(type_expression::parser())
        .then_ignore(field_end())
        .map_with(|(name, source_type), context| {
            let field_span = context.span();
            let state = &context.state().0;
            match source_type.into_result() {
                Ok(source_type) => FieldOutcome::Field(ParsedField {
                    span: state.source_span(field_span),
                    name: name.inner,
                    name_span: state.source_span(name.span),
                    source_type,
                }),
                Err(problem) => FieldOutcome::Problem(problem),
            }
        });

    let integer_name = integer()
        .then(type_expression::parser())
        .then_ignore(field_end())
        .map_with(|(name, _), _| {
            FieldOutcome::Problem(GrammarProblem::identifier_starts_with_digit(name.span))
        });

    choice((integer_name, named))
}
