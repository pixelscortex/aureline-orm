//! Recursive grammar for meaning-free type expressions written in source.
//!
//! “Meaning-free” means the parser preserves names and structure without
//! consulting a catalog of known types. `string`, `FutureType`, and
//! `custom_type` are all parsed identically as names; later compilation phases
//! decide what they mean.
//!
//! Precedence is wired here from tightest to loosest:
//!
//! 1. primary expressions: applications (`array<string>`), tuples
//!    (`[string, int]`), and bare names (`string`);
//! 2. recognized unsupported postfix forms (`string?` and `string[]`), retained
//!    so they can produce directed diagnostics;
//! 3. unions (`string | int`).
//!
//! Application arguments and tuple members recurse through the complete parser,
//! so lower-precedence unions remain valid inside them:
//!
//! ```text
//! record<A | B>
//! [A | B, record<C>]
//! box<A | B> | C
//! ```
//!
//! Each syntax form has its own file. This module should remain the small map of
//! recursion and precedence rather than accumulating the implementation of
//! every type form.

mod application;
mod name;
mod parsed;
mod tuple;
mod union;
mod unsupported_postfix;

use chumsky::prelude::*;

use super::state::{ParserExtra, TokenInput};
pub(in crate::grammar) use parsed::ParsedTypeExpression;

/// Parses one complete source type expression using the precedence documented
/// at module level.
///
/// Application alternatives must precede the bare-name alternative because
/// both begin with [`aureline_ast::tokens::Token::Ident`]. With the current
/// order, `array<string>` is consumed as one application. If `name` ran first,
/// it could accept only `array` and strand `<string>` for an unrelated outer
/// parser error.
pub(super) fn parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTypeExpression, ParserExtra> {
    recursive(|type_expression| {
        let application = application::parser(type_expression.clone());
        let tuple = tuple::parser(type_expression.clone());
        let name = name::parser();

        let primary = choice((application, tuple, name));
        let member = unsupported_postfix::parser(primary).boxed();

        union::parser(member).boxed()
    })
}
