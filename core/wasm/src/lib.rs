//! Browser adapters for inspecting lexer output and parsed table declarations.
//!
//! The table view follows the parser's arena ownership edges and borrows type
//! spelling from the exact input using canonical UTF-8 byte spans. It is an
//! inspection view, not a Checked Program or a serialized compiler AST.
//! Parser failures carry the original phase-local problems as data. Semantic
//! Findings are mapped here to a browser DTO with contextual spans and brief
//! messages; stable diagnostic codes remain a later rendering concern.

use aureline_ast::{ast::SchemaType, source::SourceSpan};
use aureline_checker::{ArgumentRole, Finding};
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
#[serde(tag = "status", rename_all = "camelCase")]
enum CheckDetails<'source> {
    Checked {
        tables: Vec<CheckedTableDetails<'source>>,
    },
    #[serde(rename = "invalid")]
    Invalid {
        phase: &'static str,
        problems: Vec<CheckProblem>,
    },
}

/// The check seam keeps syntax problems in the parser's typed model and gives
/// semantic Findings a browser-oriented tagged shape. This keeps the checker
/// free of serialization and presentation policy while retaining every finding
/// in its original order.
#[derive(Serialize)]
#[serde(untagged)]
enum CheckProblem {
    Syntax(SyntaxProblem),
    Semantic(SemanticProblem),
}

#[derive(Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum SemanticProblem {
    DuplicateTable {
        name: String,
        span: SourceSpan,
        first_span: SourceSpan,
        message: String,
    },
    DuplicateField {
        table: String,
        name: String,
        span: SourceSpan,
        first_span: SourceSpan,
        message: String,
    },
    UnknownType {
        name: String,
        span: SourceSpan,
        message: String,
    },
    UnsupportedType {
        name: String,
        span: SourceSpan,
        message: String,
    },
    BareTableType {
        name: String,
        span: SourceSpan,
        message: String,
    },
    WrongArity {
        name: String,
        minimum: usize,
        maximum: usize,
        actual: usize,
        span: SourceSpan,
        message: String,
    },
    MissingRecordTarget {
        name: String,
        span: SourceSpan,
        message: String,
    },
    AmbiguousRecordTarget {
        name: String,
        candidates: Vec<AmbiguousCandidate>,
        span: SourceSpan,
        message: String,
    },
    WrongArgumentRole {
        name: String,
        position: usize,
        expected: &'static str,
        span: SourceSpan,
        message: String,
    },
    InvalidCollectionSize {
        name: String,
        raw: String,
        span: SourceSpan,
        message: String,
    },
    InvalidRecordKey {
        table: String,
        field: String,
        span: SourceSpan,
        message: String,
    },
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckedTableDetails<'source> {
    name: String,
    schema_mode: &'static str,
    span: SourceSpan,
    fields: Vec<CheckedFieldDetails<'source>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckedFieldDetails<'source> {
    name: String,
    source_type: &'source str,
    semantic_type: String,
    presence: &'static str,
    span: SourceSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AmbiguousCandidate {
    name: String,
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

/// Parses and semantically checks exact source text for browser inspection.
///
/// A successful result is `{ status: "checked", tables }`, with canonical
/// semantic type strings and required/optional field presence. A failure is
/// `{ status: "invalid", phase, problems }`; syntax problems retain the
/// parser's typed representation, while semantic problems expose a tagged
/// browser DTO. Tables follow source order; findings follow the checker's fixed
/// order (declarations, field types, then record-key validation), with source
/// order within each phase. Invalid results never include partial tables. All
/// source spans are half-open UTF-8 byte ranges, matching [`parse`].
///
/// # Errors
///
/// Returns a JavaScript string error when the result cannot be serialized.
#[wasm_bindgen]
pub fn check(source: &str) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&check_details(source))
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

fn check_details(source: &str) -> CheckDetails<'_> {
    let ast = match aureline_parser::parse(source) {
        Ok(ast) => ast,
        Err(problems) => {
            return CheckDetails::Invalid {
                phase: "syntax",
                problems: problems.into_iter().map(CheckProblem::Syntax).collect(),
            };
        }
    };

    let analysis = aureline_checker::check(&ast);
    match analysis.into_checked() {
        Ok(checked) => CheckDetails::Checked {
            tables: checked_tables(source, &checked),
        },
        Err(rejected) => CheckDetails::Invalid {
            phase: "semantic",
            problems: rejected
                .findings()
                .iter()
                .map(|finding| CheckProblem::Semantic(semantic_problem(&rejected, finding)))
                .collect(),
        },
    }
}

fn checked_tables<'source>(
    source: &'source str,
    checked: &aureline_checker::CheckedProgram<'_>,
) -> Vec<CheckedTableDetails<'source>> {
    checked
        .tables()
        .iter()
        .map(|&table_id| {
            let table = checked
                .table(table_id)
                .expect("checked table IDs belong to the checked program");
            let fields = checked
                .fields_of(table_id)
                .iter()
                .map(|&field_id| {
                    let field = checked
                        .field(field_id)
                        .expect("checked field IDs belong to the checked program");
                    CheckedFieldDetails {
                        name: field.name().to_owned(),
                        source_type: source_slice(source, field.source_type().span()),
                        semantic_type: checked
                            .format_type(field_id)
                            .expect("checked fields have canonical semantic types"),
                        presence: match checked
                            .field_presence(field_id)
                            .expect("checked fields have field presence")
                        {
                            aureline_checker::FieldPresence::Required => "required",
                            aureline_checker::FieldPresence::Optional => "optional",
                        },
                        span: field.span(),
                    }
                })
                .collect();
            CheckedTableDetails {
                name: table.name().to_owned(),
                schema_mode: match table.schema_type() {
                    SchemaType::Full => "schemafull",
                    SchemaType::Less => "schemaless",
                },
                span: table.span(),
                fields,
            }
        })
        .collect()
}

fn source_slice(source: &str, span: SourceSpan) -> &str {
    let range = span.range();
    &source[range.start().get() as usize..range.end().get() as usize]
}

// The checker has one typed variant per semantic obligation. Keeping this
// exhaustive match in the browser adapter makes additions fail at compile
// time instead of silently dropping a finding from the inspection seam.
#[allow(clippy::too_many_lines)]
fn semantic_problem(
    analysis: &aureline_checker::Analysis<'_>,
    finding: &Finding,
) -> SemanticProblem {
    match finding {
        Finding::DuplicateTable {
            name,
            primary,
            first,
        } => SemanticProblem::DuplicateTable {
            name: name.clone(),
            span: *primary,
            first_span: *first,
            message: format!("table '{name}' is declared more than once"),
        },
        Finding::DuplicateField {
            owner,
            name,
            primary,
            first,
        } => SemanticProblem::DuplicateField {
            table: table_name(analysis, *owner),
            name: name.clone(),
            span: *primary,
            first_span: *first,
            message: format!("field '{name}' is declared more than once"),
        },
        Finding::UnknownType { name, span } => SemanticProblem::UnknownType {
            name: name.clone(),
            span: *span,
            message: format!("unknown type '{name}'"),
        },
        Finding::UnsupportedType { name, span } => SemanticProblem::UnsupportedType {
            name: name.clone(),
            span: *span,
            message: format!("type '{name}' is not supported here"),
        },
        Finding::BareTableType { name, span } => SemanticProblem::BareTableType {
            name: name.clone(),
            span: *span,
            message: format!("table type '{name}' must use record<{name}>"),
        },
        Finding::WrongArity {
            name,
            minimum,
            maximum,
            actual,
            span,
        } => SemanticProblem::WrongArity {
            name: name.clone(),
            minimum: *minimum,
            maximum: *maximum,
            actual: *actual,
            span: *span,
            message: format!("type '{name}' expects {minimum}..={maximum} arguments, got {actual}"),
        },
        Finding::MissingRecordTarget { name, span } => SemanticProblem::MissingRecordTarget {
            name: name.clone(),
            span: *span,
            message: format!("record target '{name}' is not declared"),
        },
        Finding::AmbiguousRecordTarget {
            name,
            span,
            candidates,
        } => SemanticProblem::AmbiguousRecordTarget {
            name: name.clone(),
            candidates: candidates
                .iter()
                .map(|&id| {
                    let table = analysis
                        .table(id)
                        .expect("ambiguous record candidates belong to the analysis");
                    AmbiguousCandidate {
                        name: table.name().to_owned(),
                        span: table.name_span(),
                    }
                })
                .collect(),
            span: *span,
            message: format!("record target '{name}' is ambiguous"),
        },
        Finding::WrongArgumentRole {
            name,
            position,
            expected,
            span,
        } => SemanticProblem::WrongArgumentRole {
            name: name.clone(),
            position: *position,
            expected: argument_role_name(*expected),
            span: *span,
            message: format!(
                "argument {position} of '{name}' must be a {}",
                argument_role_name(*expected)
            ),
        },
        Finding::InvalidCollectionSize { name, raw, span } => {
            SemanticProblem::InvalidCollectionSize {
                name: name.clone(),
                raw: raw.clone(),
                span: *span,
                message: format!("collection size '{raw}' is not a valid unsigned 64-bit integer"),
            }
        }
        Finding::InvalidRecordKey { field, span } => {
            let field_decl = analysis
                .field(*field)
                .expect("invalid record key refers to a field in the analysis");
            SemanticProblem::InvalidRecordKey {
                table: table_name(analysis, field_decl.owner()),
                field: field_decl.name().to_owned(),
                span: *span,
                message: format!(
                    "field '{}' cannot be used as a record key",
                    field_decl.name()
                ),
            }
        }
    }
}

fn table_name(analysis: &aureline_checker::Analysis<'_>, id: aureline_ast::ids::TableId) -> String {
    // Finding identities are issued by this analysis' index; a missing table
    // would violate the checker invariant rather than provide browser recovery.
    analysis
        .table(id)
        .expect("semantic table identities belong to the analysis")
        .name()
        .to_owned()
}

const fn argument_role_name(role: ArgumentRole) -> &'static str {
    match role {
        ArgumentRole::ElementType => "element type",
        ArgumentRole::CollectionSize => "collection size",
        ArgumentRole::RecordTarget => "record target",
        ArgumentRole::OptionType => "option type",
    }
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

#[cfg(test)]
mod tests {
    use super::{CheckDetails, CheckProblem, SemanticProblem, check_details};
    use aureline_parser::SyntaxProblem;

    #[test]
    fn check_exposes_canonical_types_presence_and_utf8_spans() {
        let source = "// café\ntable User schemafull {\n  id string\n  name option<string>\n  tags array<record<User | User>, 2>\n}";
        let CheckDetails::Checked { tables } = check_details(source) else {
            panic!("fixture should check");
        };
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "User");
        assert_eq!(tables[0].schema_mode, "schemafull");
        assert_eq!(
            tables[0].span.range().start().get(),
            u32::try_from("// café\n".len()).expect("fixture length fits")
        );
        assert_eq!(tables[0].fields[0].semantic_type, "string");
        assert_eq!(tables[0].fields[0].presence, "required");
        assert_eq!(tables[0].fields[1].source_type, "option<string>");
        assert_eq!(tables[0].fields[1].semantic_type, "string | none");
        assert_eq!(tables[0].fields[1].presence, "optional");
        assert_eq!(tables[0].fields[2].semantic_type, "array<record<User>, 2>");
    }

    #[test]
    fn semantic_findings_keep_checker_order_and_expose_context() {
        let source = "table User schemafull {\n  id string\n  first Unknown\n  first geometry\n}";
        let CheckDetails::Invalid { phase, problems } = check_details(source) else {
            panic!("fixture should be semantically invalid");
        };
        assert_eq!(phase, "semantic");
        assert_eq!(problems.len(), 3);
        assert!(matches!(
            &problems[0],
            CheckProblem::Semantic(SemanticProblem::DuplicateField { name, .. }) if name == "first"
        ));
        assert!(matches!(
            &problems[1],
            CheckProblem::Semantic(SemanticProblem::UnknownType { name, .. }) if name == "Unknown"
        ));
        assert!(matches!(
            &problems[2],
            CheckProblem::Semantic(SemanticProblem::UnsupportedType { name, .. }) if name == "geometry"
        ));
    }

    #[test]
    fn ambiguous_targets_keep_each_candidate_name_span() {
        let source = "table User schemafull {}\ntable User schemafull {}\ntable Holder schemafull {\n  owner record<User>\n}";
        let CheckDetails::Invalid { problems, .. } = check_details(source) else {
            panic!("fixture should be semantically invalid");
        };
        let CheckProblem::Semantic(SemanticProblem::AmbiguousRecordTarget { candidates, .. }) =
            &problems[1]
        else {
            panic!("second finding should be the ambiguous record target");
        };
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].name, "User");
        assert_eq!(candidates[1].name, "User");
        let user_starts: Vec<_> = source
            .match_indices("User")
            .map(|(start, _)| u32::try_from(start).expect("fixture offset fits"))
            .collect();
        assert_eq!(candidates[0].span.range().start().get(), user_starts[0]);
        assert_eq!(candidates[1].span.range().start().get(), user_starts[1]);
    }

    #[test]
    fn syntax_findings_preserve_phase_and_utf8_byte_ranges() {
        let source = "// café\ntable User schemafull {\n  é string\n}";
        let CheckDetails::Invalid { phase, problems } = check_details(source) else {
            panic!("fixture should be syntactically invalid");
        };
        assert_eq!(phase, "syntax");
        assert!(matches!(
            &problems[0],
            CheckProblem::Syntax(SyntaxProblem::InvalidIdentifier { span, .. })
                if span.range().start().get()
                    == u32::try_from("// café\ntable User schemafull {\n  ".len())
                        .expect("fixture length fits")
                    && span.range().end().get()
                        == u32::try_from("// café\ntable User schemafull {\n  é".len())
                            .expect("fixture length fits")
        ));
    }
}
