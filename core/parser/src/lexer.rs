use aureline_ast::tokens::Token;
use chumsky::prelude::*;

#[must_use]
pub fn lexer<'src>() -> impl Parser<'src, &'src str, Vec<aureline_ast::tokens::Token<'src>>> {
    let word = text::ident().map(|word: &'src str| match word {
        "table" => Token::Table,
        "schemafull" => Token::Schemafull,
        "schemaless" => Token::Schemaless,
        identifier => Token::Ident(identifier),
    });

    let punctuation = choice((just('{').to(Token::LBrace), just('}').to(Token::RBrace)));

    let token = choice((text::newline().to(Token::Newline), word, punctuation));

    token
        // Ignore spaces and tabs around Token, but preserve newlines as real
        // Token because the grammar uses them as statement/field boundaries.
        .padded_by(text::inline_whitespace())
        .repeated()
        .collect()
}
