use aureline_parser::Token;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LexerDetails<'source> {
    pub tokens: Vec<Token<'source>>,
    pub errors: Vec<String>,
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
/// Returns the source's grammatical tokens and any lexical problems as a
/// JavaScript value.
///
/// # Errors
///
/// Returns a JavaScript string error when the result cannot be serialized for
/// the WASM boundary.
pub fn lexer(source: &str) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&lexer_details(source))
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

fn lexer_details(source: &str) -> LexerDetails<'_> {
    match aureline_parser::tokenize(source) {
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
