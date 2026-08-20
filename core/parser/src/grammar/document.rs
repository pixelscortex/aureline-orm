//! Parses the source-file level sequence of table declarations.
//!
//! Newlines may precede the first table, separate tables, or follow the last
//! table. Field separation is handled inside [`super::table`], not here.

use aureline_ast::tokens::Token;
use chumsky::prelude::*;

use super::{
    problem::GrammarProblem,
    state::{ParserExtra, TokenInput},
    table,
};

pub(super) fn parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Option<GrammarProblem>, ParserExtra> {
    let newlines = just(Token::Newline).repeated();

    let items = table::parser()
        .then_ignore(newlines.clone())
        .repeated()
        .collect::<Vec<_>>();

    // `ignored()` is not interchangeable with mapping the emitted values away:
    // it puts child parsers in Chumsky's check-only mode. In that mode the
    // `map_with` callbacks in table and field parsers do not allocate AST nodes.
    // Keep the outcomes until the construction walk finishes, then inspect only
    // their problems.
    newlines
        .ignore_then(items)
        .then_ignore(end())
        .map(|problems| {
            problems
                .into_iter()
                .flatten()
                .min_by_key(|problem| problem.span().start)
        })
}
