use aureline_parser::Token;
use chumsky::Parser;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LexerDetails {
    pub tokens: Vec<Token>,
    pub errors: Vec<String>,
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn lexer(source: &str) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&lexer_details(source))
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

fn lexer_details(source: &str) -> LexerDetails {
    match aureline_parser::lexer().parse(source).into_result() {
        Ok(tokens) => LexerDetails {
            tokens,
            errors: Vec::new(),
        },
        Err(errors) => LexerDetails {
            tokens: Vec::new(),
            errors: errors
                .into_iter()
                .map(|error| format!("{error:?}"))
                .collect(),
        },
    }
}
