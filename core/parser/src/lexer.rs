use aureline_ast::tokens::Token;
use chumsky::prelude::*;

pub fn lexer<'src>() -> impl Parser<'src, &'src str, Vec<aureline_ast::tokens::Token>> {
    let table = just("table").map(|_| Token::Table);
    let schemafull = just("schemafull").map(|_| Token::Schemafull);
    let schemaless = just("schemaless").map(|_| Token::Schemaless);
    let l_brace = just("{").map(|_| Token::LBrace);
    let r_brace = just("}").map(|_| Token::RBrance);

    let token = choice((
        text::newline().to(Token::Newline),
        table,
        schemafull,
        schemaless,
        l_brace,
        r_brace,
    ));

    token
        // Ignore spaces and tabs around Token, but preserve newlines as real
        // Token because the grammar uses them as statement/field boundaries.
        .padded_by(text::inline_whitespace())
        .repeated()
        .collect()
}
