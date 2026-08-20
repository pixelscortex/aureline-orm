//! Bare, unresolved type-name expressions.

use aureline_ast::ast::SourceType;
use chumsky::prelude::*;

use super::{
    super::{
        atom::ident,
        state::{ParserExtra, TokenInput},
    },
    parsed::ParsedTypeExpression,
};

/// Converts any bare identifier in a type position into [`SourceType::Name`].
///
/// This parser intentionally does not special-case built-in-looking names.
/// `string`, `record`, and `FutureType` all produce the same AST shape with
/// their original spelling and source span. Application parsing runs before this
/// parser, so the name in `record<User>` becomes part of an application rather
/// than leaving `<User>` unconsumed.
pub(super) fn parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTypeExpression, ParserExtra> {
    ident().map_with(|name, context| {
        ParsedTypeExpression::valid(SourceType::name(
            name.inner,
            context.state().0.source_span(name.span),
        ))
    })
}
