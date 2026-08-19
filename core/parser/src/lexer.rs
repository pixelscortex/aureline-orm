use aureline_ast::tokens::Token;
use chumsky::{error::Cheap, extra, prelude::*};

pub(crate) type TokenOccurrence<'src> = Spanned<Token<'src>>;

#[must_use]
pub(crate) fn lexer<'src>()
-> impl Parser<'src, &'src str, Vec<TokenOccurrence<'src>>, extra::Err<Cheap<SimpleSpan>>> {
    let word = text::ident().map(|word: &'src str| match word {
        "table" => Token::Table,
        "schemafull" => Token::Schemafull,
        "schemaless" => Token::Schemaless,
        identifier => Token::Ident(identifier),
    });

    let punctuation = choice((just('{').to(Token::LBrace), just('}').to(Token::RBrace)));

    let token = choice((text::newline().to(Token::Newline), word, punctuation)).spanned();

    token
        // Ignore spaces and tabs around Token, but preserve newlines as real
        // Token because the grammar uses them as statement/field boundaries.
        .padded_by(text::inline_whitespace())
        .repeated()
        .collect()
}
