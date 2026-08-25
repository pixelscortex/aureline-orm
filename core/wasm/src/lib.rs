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
/// Returns `{ tokens, errors }` as a JavaScript value.
///
/// Successful lexing fills `tokens` and leaves `errors` empty. Any lexical
/// problem leaves `tokens` empty and fills `errors`; partial token streams are
/// not exposed. Error entries currently use Rust's debug representation and are
/// not a stable structured diagnostic interface.
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
