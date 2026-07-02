#![doc = "Browser WebAssembly wrapper for the Aureline language facade."]

use aureline_lang::inspect_schema_name;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmInspectResult {
    ok: bool,
    schema_name: String,
    diagnostics: Vec<WasmDiagnostic>,
    migration_preview: String,
}

#[derive(Debug, Serialize)]
struct WasmDiagnostic {
    message: String,
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen(js_name = inspectAurl)]
pub fn inspect_aurl(source: &str) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&inspect_aurl_dto(source))
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

fn inspect_aurl_dto(source: &str) -> WasmInspectResult {
    let result = inspect_schema_name(source);
    let diagnostics = result
        .diagnostics
        .into_iter()
        .map(|diagnostic| WasmDiagnostic {
            message: diagnostic.message,
        })
        .collect::<Vec<_>>();

    WasmInspectResult {
        ok: diagnostics.is_empty(),
        schema_name: result.schema.name,
        diagnostics,
        migration_preview: result.migration_preview,
    }
}

#[cfg(test)]
mod tests {
    use super::inspect_aurl_dto;

    #[test]
    fn inspect_aurl_dto_reports_schema() {
        let result = inspect_aurl_dto("blog");

        assert!(result.ok);
        assert_eq!(result.schema_name, "blog");
    }
}
