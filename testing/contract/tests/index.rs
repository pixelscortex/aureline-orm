use aureline_ast::{ast::Ast, source::SourceId};
use aureline_checker::{FieldResolution, Finding, ResolutionIndex, TableResolution, check};

#[test]
fn resolution_distinguishes_missing_unique_and_all_ambiguous_candidates() {
    let ast = parse(
        "table Alpha schemafull { id string\n id int\n id uuid }\n\
         table User schemafull { id string }\n\
         table User schemaless { label string }",
    );
    let index = ResolutionIndex::collect(&ast);

    assert_eq!(index.resolve_table("Missing"), TableResolution::Missing);
    let alpha = index.tables()[0];
    assert_eq!(index.resolve_table("Alpha"), TableResolution::Unique(alpha));
    let user_candidates = match index.resolve_table("User") {
        TableResolution::Ambiguous(candidates) => candidates,
        other => panic!("expected all duplicate User candidates, got {other:?}"),
    };
    assert_eq!(user_candidates, index.tables()[1..]);

    let alpha_fields = index.fields_of(alpha);
    let field_candidates = match index.resolve_field(alpha, "id") {
        FieldResolution::Ambiguous(candidates) => candidates,
        other => panic!("expected all duplicate Alpha.id candidates, got {other:?}"),
    };
    assert_eq!(field_candidates, alpha_fields);
    assert!(field_candidates[0] != field_candidates[1]);
    assert!(matches!(
        index.resolve_field(alpha, "missing"),
        FieldResolution::Missing
    ));
}

#[test]
fn duplicate_findings_are_grouped_by_check_then_source_order() {
    let ast = parse(
        "table User schemafull { id string\n id int\n id uuid }\n\
         table User schemafull { id uuid }\n\
         table User schemafull { id uuid }\n\
         table user schemafull { id string\n id uuid }",
    );
    let analysis = check(&ast);
    let findings = analysis.findings();

    assert_eq!(findings.len(), 5);
    assert!(matches!(
        findings[0],
        Finding::DuplicateTable { ref name, .. } if name == "User"
    ));
    assert!(matches!(
        findings[1],
        Finding::DuplicateTable { ref name, .. } if name == "User"
    ));
    for finding in &findings[2..] {
        assert!(matches!(
            finding,
            Finding::DuplicateField { name, .. } if name == "id"
        ));
    }
}

#[test]
fn duplicate_fields_in_separate_duplicate_tables_are_not_compared() {
    let ast = parse(
        "table User schemafull { id string }\n\
         table User schemafull { id int }",
    );
    let analysis = check(&ast);
    let findings = analysis.findings();
    assert_eq!(findings.len(), 1);
    assert!(matches!(
        findings[0],
        Finding::DuplicateTable { ref name, .. } if name == "User"
    ));
}

#[test]
fn names_are_exact_case_sensitive_and_empty_tables_are_valid() {
    let ast = parse(
        "table User schemafull {}\n\
         table user schemaless { label string }",
    );
    assert!(check(&ast).findings().is_empty());
    assert!(check(&parse("\n\n")).findings().is_empty());
}

#[test]
fn duplicate_findings_keep_later_primary_and_first_context_spans() {
    let ast = aureline_parser::parse_with_source(
        SourceId::new(11),
        "table User schemafull { id string\n id int\n id uuid }\n\
         table User schemafull { id uuid }",
    )
    .expect("the duplicate declarations are valid syntax");
    let first_table = ast.root().tables()[0];
    let second_table = ast.root().tables()[1];
    let first_table_decl = ast.table(first_table).expect("first table exists");
    let second_table_decl = ast.table(second_table).expect("second table exists");
    let first_field = first_table_decl.fields()[0];
    let second_field = first_table_decl.fields()[1];
    let third_field = first_table_decl.fields()[2];

    let analysis = check(&ast);
    let findings = analysis.findings();
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
}

#[test]
fn duplicate_spans_count_utf8_bytes_before_the_name() {
    let ast = aureline_parser::parse_with_source(
        SourceId::new(12),
        "// é\ntable User schemafull {}\ntable User schemafull {}",
    )
    .expect("the duplicate declarations are valid syntax");
    let first = ast
        .table(ast.root().tables()[0])
        .expect("first table exists");
    let second = ast
        .table(ast.root().tables()[1])
        .expect("second table exists");
    assert_eq!(first.name_span().range().start().get(), 12);
    assert_eq!(second.name_span().range().start().get(), 37);
    assert_eq!(
        check(&ast).findings()[0],
        Finding::DuplicateTable {
            name: "User".to_owned(),
            primary: second.name_span(),
            first: first.name_span(),
        }
    );
}

fn parse(source: &str) -> Ast {
    aureline_parser::parse(source).expect("the test source should parse")
}
