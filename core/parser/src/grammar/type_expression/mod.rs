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
//! Every recognized form returns a [`ParsedTypeExpression`], not a public
//! [`SourceType`](aureline_ast::ast::SourceType) directly. Valid syntax carries
//! its source type; directed recovery carries a [`GrammarProblem`] through the
//! remaining delimiters. For example, `record<A |>` consumes the application
//! closing `>` while retaining `MissingUnionMember` for the field parser. Type
//! parsing never mutates the AST.
//!
//! Each syntax form has its own file. This module should remain the small map of
//! recursion and precedence rather than accumulating the implementation of
//! every type form.
//!
//! In parser signatures, `'src` owns spellings borrowed from source tokens and
//! `'tokens` owns the token slice Chumsky reads; `'src: 'tokens` keeps those
//! spellings alive for the parse. `impl Parser` is a parser definition, not an
//! already parsed type expression.

mod application;
mod name;
mod parsed;
mod sequence;
mod tuple;
mod union;

use chumsky::prelude::*;

use super::state::{ParserExtra, TokenInput};
pub(in crate::grammar) use parsed::ParsedTypeExpression;

/// Composes the complete type-expression grammar at its recursion seam.
///
/// Applications and tuples receive a clone of the complete recursive parser so
/// their arguments and members may themselves contain unions. [`union::parser`]
/// then wraps the primary choice, giving `|` the loosest precedence. The parser
/// consumes exactly one recognized type expression and emits a valid-or-recovered
/// [`ParsedTypeExpression`]; it performs no AST allocation.
pub(super) fn parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTypeExpression, ParserExtra> {
    recursive(|type_expression| {
        let application = application::parser(type_expression.clone());
        let tuple = tuple::parser(type_expression.clone());
        let name = name::parser();

        let primary = choice((application, tuple, name)).boxed();
        union::parser(primary).boxed()
    })
}
