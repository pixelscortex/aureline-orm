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
//! 2. unions (`string | int`).
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
mod sequence;
mod tuple;
mod union;

use chumsky::prelude::*;

use super::state::{ParserExtra, TokenInput};
pub(in crate::grammar) use parsed::ParsedTypeExpression;

/// Consumes one complete source type, leaving its enclosing comma, closing
/// delimiter, or field boundary to the caller. Nested applications and tuples
/// consume their own delimiters. This emits a valid type or a recovered
/// structural problem and does not allocate declarations in the AST.
///
/// `box<A | B>` builds a union argument inside an application; `box<A |>`
/// carries the missing-member problem through the application's closing `>`.
///
/// In these parser signatures, `'src` owns borrowed source spelling and
/// `'tokens` owns the token input; `'src: 'tokens` keeps spelling alive while
/// parsing. `impl Parser` describes a parser, rather than an already parsed value.
pub(super) fn parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTypeExpression, ParserExtra> {
    recursive(|type_expression| {
        let application = application::parser(type_expression.clone());
        let tuple = tuple::parser(type_expression.clone());
        let name = name::parser();

        // Applications precede names because both begin with an identifier.
        let member = choice((application, tuple, name)).boxed();

        union::parser(member).boxed()
    })
}
