//! Lexer-to-parser boundary: comments disappear from grammar tokens while
//! structural keywords and punctuation remain available to the parser.

use aureline_parser::Token;

#[test]
fn tokenizer_exposes_only_grammar_tokens() {
    let tokens = aureline_parser::tokenize("table User /* note */ schemafull {}")
        .expect("the source contains only valid lexical forms");

    assert_eq!(
        tokens,
        vec![
            Token::Table,
            Token::Ident("User"),
            Token::Schemafull,
            Token::LBrace,
            Token::RBrace,
        ]
    );
}
