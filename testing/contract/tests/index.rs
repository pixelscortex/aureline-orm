use aureline_ast::ids::TableId;
use aureline_checker::{FieldResolution, Finding, ResolutionIndex, TableResolution, check};
use aureline_test::aurl_test;
use serde::Serialize;

#[derive(Serialize)]
struct ResolutionView {
    tables: Vec<TableView>,
    fields: Vec<FieldView>,
}

#[derive(Serialize)]
enum TableView {
    Missing,
    Unique(String),
    Ambiguous(Vec<String>),
}

#[derive(Serialize)]
enum FieldView {
    Missing,
    Unique(String),
    Ambiguous(Vec<String>),
}

#[test]
fn resolution_distinguishes_missing_unique_and_all_ambiguous_candidates() {
    aurl_test!(
        "table Alpha schemafull { id string\n id int\n id bool }\n\
         table User schemafull { id string }\n\
         table User schemaless { label string }"
    )
    .reports_with(
        |ast| {
            let index = ResolutionIndex::collect(ast);
            let tables = ["Missing", "Alpha", "User"]
                .into_iter()
                .map(|name| table_view(&index, name))
                .collect();
            let alpha = index
                .tables()
                .first()
                .copied()
                .expect("the fixture declares Alpha");
            let fields = ["id", "missing"]
                .into_iter()
                .map(|name| field_view(&index, alpha, name))
                .collect();
            let FieldResolution::Ambiguous(ids) = index.resolve_field(alpha, "id") else {
                panic!("Alpha.id should retain all duplicate field candidates");
            };
            assert_eq!(ids.len(), 3);
            assert!(ids[0] != ids[1] && ids[1] != ids[2]);
            ResolutionView { tables, fields }
        },
        "(ResolutionView Missing (Unique Alpha) (Ambiguous User User) (Ambiguous id id id) Missing)",
    );
}

#[test]
fn duplicate_findings_are_grouped_by_check_then_source_order() {
    aurl_test!(
        "table User schemafull { id string\n id int\n id bool }\n\
         table User schemafull { id bool }\n\
         table User schemafull { id bytes }\n\
         table user schemafull { id string\n id bool }"
    )
    .findings(
        "(Findings (DuplicateTable User) (DuplicateTable User) (DuplicateField id) (DuplicateField id) (DuplicateField id))",
    );
}

#[test]
fn duplicate_fields_in_separate_duplicate_tables_are_not_compared() {
    aurl_test!(
        "table User schemafull { id string }\n\
         table User schemafull { id int }"
    )
    .findings("(Findings (DuplicateTable User))");
}

#[test]
fn names_are_exact_case_sensitive_and_empty_tables_are_valid() {
    aurl_test!(
        "table User schemafull {}\n\
         table user schemaless { label string }"
    )
    .findings("(Findings)");

    aurl_test!("\n\n").findings("(Findings)");
}

#[test]
fn duplicate_findings_keep_later_primary_and_first_context_spans() {
    aurl_test!(
        "table User schemafull { id string\n id int\n id bool }\n\
         table User schemafull { id bool }"
    )
    .with_source_id(aureline_ast::source::SourceId::new(11))
    .reports_with(
        |ast| {
            let first_table = ast.root().tables()[0];
            let second_table = ast.root().tables()[1];
            let first_table_decl = ast.table(first_table).expect("first table exists");
            let second_table_decl = ast.table(second_table).expect("second table exists");
            let first_field = first_table_decl.fields()[0];
            let second_field = first_table_decl.fields()[1];
            let third_field = first_table_decl.fields()[2];

            let findings = check(ast).findings().to_vec();
            assert_eq!(findings.len(), 3);
            assert_eq!(
                findings[0],
                Finding::DuplicateTable {
                    name: "User".to_owned(),
                    primary: second_table_decl.name_span(),
                    first: first_table_decl.name_span(),
                }
            );
            assert_eq!(
                findings[1],
                Finding::DuplicateField {
                    owner: first_table,
                    name: "id".to_owned(),
                    primary: ast
                        .field(second_field)
                        .expect("second field exists")
                        .name_span(),
                    first: ast
                        .field(first_field)
                        .expect("first field exists")
                        .name_span(),
                }
            );
            assert_eq!(
                findings[2],
                Finding::DuplicateField {
                    owner: first_table,
                    name: "id".to_owned(),
                    primary: ast
                        .field(third_field)
                        .expect("third field exists")
                        .name_span(),
                    first: ast
                        .field(first_field)
                        .expect("first field exists")
                        .name_span(),
                }
            );

            assert_eq!(
                first_table_decl.name_span().source(),
                second_table_decl.name_span().source()
            );
            true
        },
        "true",
    );
}

#[test]
fn duplicate_spans_count_utf8_bytes_before_the_name() {
    aurl_test!("// é\ntable User schemafull {}\ntable User schemafull {}")
        .with_source_id(aureline_ast::source::SourceId::new(12))
        .reports_with(
            |ast| {
                let first = ast
                    .table(ast.root().tables()[0])
                    .expect("first table exists");
                let second = ast
                    .table(ast.root().tables()[1])
                    .expect("second table exists");
                assert_eq!(first.name_span().range().start().get(), 12);
                assert_eq!(second.name_span().range().start().get(), 37);
                let findings = check(ast).findings().to_vec();
                assert_eq!(
                    findings[0],
                    Finding::DuplicateTable {
                        name: "User".to_owned(),
                        primary: second.name_span(),
                        first: first.name_span(),
                    }
                );
                true
            },
            "true",
        );
}

fn table_view(index: &aureline_checker::ResolutionIndex<'_>, name: &str) -> TableView {
    match index.resolve_table(name) {
        TableResolution::Missing => TableView::Missing,
        TableResolution::Unique(table) => TableView::Unique(
            index
                .table(table)
                .expect("a unique candidate belongs to the AST")
                .name()
                .to_owned(),
        ),
        TableResolution::Ambiguous(tables) => TableView::Ambiguous(
            tables
                .into_iter()
                .map(|table| {
                    index
                        .table(table)
                        .expect("an ambiguous candidate belongs to the AST")
                        .name()
                        .to_owned()
                })
                .collect(),
        ),
    }
}

fn field_view(
    index: &aureline_checker::ResolutionIndex<'_>,
    table: TableId,
    name: &str,
) -> FieldView {
    match index.resolve_field(table, name) {
        FieldResolution::Missing => FieldView::Missing,
        FieldResolution::Unique(field) => FieldView::Unique(
            index
                .field(field)
                .expect("a unique candidate belongs to the AST")
                .name()
                .to_owned(),
        ),
        FieldResolution::Ambiguous(fields) => FieldView::Ambiguous(
            fields
                .into_iter()
                .map(|field| {
                    index
                        .field(field)
                        .expect("an ambiguous candidate belongs to the AST")
                        .name()
                        .to_owned()
                })
                .collect(),
        ),
    }
}
