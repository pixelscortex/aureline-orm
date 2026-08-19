use aureline_ast::{
    ast::Item,
    source::{SourceId, SourceSpan, TextRange, TextSize},
};
use aureline_parser::SyntaxProblem;
use aureline_test::aurl_test;

#[test]
fn schemafull_table_matches_the_logical_contract() {
    aurl_test!("table User schemafull {}").parses_as("(SourceFile (Table User Schemafull))");
}

#[test]
fn multibyte_table_text_has_precise_utf8_byte_spans() {
    let source_id = SourceId::new(9);
    let ast = aureline_parser::parse_with_source(source_id, "table Café schemafull {}")
        .expect("table parses");
    let item = ast
        .item(ast.root().items()[0])
        .expect("table item is allocated");
    let Item::Table(table) = item;

    assert_eq!(table.span(), span(source_id, 0, 25));
    assert_eq!(table.name_span(), span(source_id, 6, 11));
    assert_eq!(table.schema_type_span(), span(source_id, 12, 22));
}

#[test]
fn invalid_table_syntax_is_typed_and_located() {
    let errors = aureline_parser::parse("table User mystery {}")
        .expect_err("unknown schema mode does not parse");

    assert_eq!(
        errors,
        vec![SyntaxProblem::UnexpectedToken {
            span: span(SourceId::new(0), 11, 18),
        }]
    );
}

fn span(source: SourceId, start: u32, end: u32) -> SourceSpan {
    SourceSpan::new(
        source,
        TextRange::new(TextSize::new(start), TextSize::new(end)).expect("test range is ordered"),
    )
}
