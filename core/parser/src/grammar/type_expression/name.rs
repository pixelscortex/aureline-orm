//! Parses a bare identifier as an unresolved source type name.
//!
//! This is the smallest type-expression form: it consumes one identifier,
//! attaches the current source identity to its token span, and emits a valid
//! [`ParsedTypeExpression`]. It does not validate the name against known types
//! or mutate the AST.

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
