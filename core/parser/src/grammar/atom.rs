//! Small token-to-domain parsers shared by the larger grammar modules.
//!
//! These parsers do no recovery and make no contextual decisions. They only
//! select one already classified token, convert borrowed source spelling into
//! owned AST data where necessary, and retain the token's byte span.

use aureline_ast::{ast::SchemaType, tokens::Token};
use chumsky::prelude::*;

use super::state::{ParserExtra, TokenInput};

/// Selects a bare [`Token::Ident`], owns its source spelling, and retains its
/// lexer span.
///
/// Reserved structural words (`table`, `schemafull`, and `schemaless`) have
/// distinct token variants and therefore do not match this parser. Names such
/// as `string`, `record`, and `FutureType` are ordinary identifiers; the parser
/// deliberately does not resolve or restrict type names.
pub(super) fn ident<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Spanned<String>, ParserExtra> {
    select_ref! {
        Token::Ident(name) => *name,
    }
    .map(str::to_owned)
    .spanned()
}

/// Converts a schema-mode keyword into its AST enum while retaining the keyword
/// span.
///
/// Only `schemafull` and `schemaless` match. For example, `table User mystery
/// {}` reaches table recovery with `mystery` as an unexpected token.
pub(super) fn schema_type<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Spanned<SchemaType>, ParserExtra> {
    choice((
        just(Token::Schemafull).to(SchemaType::Full),
        just(Token::Schemaless).to(SchemaType::Less),
    ))
    .spanned()
}

/// Selects an all-digit [`Token::Integer`] and preserves its exact spelling.
///
/// Integer type arguments are meaning-free source values, so leading zeroes are
/// retained: `array<string, 003>` stores `"003"`. When the same token occupies
/// a table or field name slot (`table 1 schemafull {}`), the owning grammar
/// reclassifies it as [`crate::IdentifierProblem::StartsWithDigit`].
pub(super) fn integer<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Spanned<String>, ParserExtra> {
    select_ref! {
        Token::Integer(raw) => *raw,
    }
    .map(str::to_owned)
    .spanned()
}
