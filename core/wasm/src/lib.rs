//! Browser adapters for inspecting lexer output and parsed table declarations.
//!
//! The table view follows the parser's arena ownership edges and borrows type
//! spelling from the exact input using canonical UTF-8 byte spans. It is an
//! inspection view, not a Checked Program or a serialized compiler AST.
//! Parser failures carry the original phase-local problems as data; the later
//! Diagnostic renderer owns stable codes and human-facing wording.

use aureline_ast::{ast::SchemaType, source::SourceSpan};
use aureline_parser::{SyntaxProblem, Token};
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

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
enum ParseDetails<'source> {
    Parsed { tables: Vec<TableDetails<'source>> },
    Invalid { problems: Vec<SyntaxProblem> },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TableDetails<'source> {
    name: String,
    schema_mode: &'static str,
    span: SourceSpan,
    fields: Vec<FieldDetails<'source>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FieldDetails<'source> {
    name: String,
    source_type: &'source str,
    span: SourceSpan,
}

/// Parses exact source text and returns a browser inspection view.
///
/// After awaiting the generated module's `init()`, call `parse(source)` to get
/// `{ status: "parsed", tables }` or `{ status: "invalid", problems }`. Invalid
/// input exposes no tables. Spans are half-open UTF-8 byte offsets into `source`,
/// not JavaScript character indices. Parsing checks syntax only; type names and
/// references are preserved without semantic validation.
///
/// Problems are serialized directly from the parser's phase-local model. This
/// experimental inspection interface is not the versioned Diagnostic envelope.
///
/// # Errors
///
/// Returns a JavaScript string error if the inspection view cannot be serialized.
#[wasm_bindgen]
pub fn parse(source: &str) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&parse_details(source))
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

fn parse_details(source: &str) -> ParseDetails<'_> {
    let ast = match aureline_parser::parse(source) {
        Ok(ast) => ast,
        Err(problems) => return ParseDetails::Invalid { problems },
    };
    let tables = ast
        .root()
        .tables()
        .iter()
        .map(|&id| {
            let table = ast.table(id).expect("parsed root owns its table IDs");
            let fields = table
                .fields()
                .iter()
                .map(|&id| {
                    let field = ast.field(id).expect("parsed table owns its field IDs");
                    let range = field.source_type().span().range();
                    FieldDetails {
                        name: field.name().to_owned(),
                        source_type: &source
                            [range.start().get() as usize..range.end().get() as usize],
                        span: field.span(),
                    }
                })
                .collect();
            TableDetails {
                name: table.name().to_owned(),
                schema_mode: match table.schema_type() {
                    SchemaType::Full => "schemafull",
                    SchemaType::Less => "schemaless",
                },
                span: table.span(),
                fields,
            }
        })
        .collect();
    ParseDetails::Parsed { tables }
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
