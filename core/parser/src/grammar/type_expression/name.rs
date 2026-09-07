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

pub(super) fn parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTypeExpression, ParserExtra> {
    ident().map_with(|name, context| {
        ParsedTypeExpression::valid(SourceType::name(
            name.inner,
            context.state().0.source_span(name.span),
        ))
    })
}
