use aureline_ast::{
    ast::{CommentKind, SourceType},
    source::{SourceId, SourceSpan, TextRange, TextSize},
};
use aureline_parser::SyntaxProblem;
use aureline_test::aurl_test;

#[test]
fn schemafull_table_matches_the_logical_contract() {
    aurl_test!("table User schemafull {}").parses_as("(SourceFile (Table User Schemafull))");
}

#[test]
fn tables_declare_fields_with_meaning_free_source_type_names() {
    aurl_test!(
        "table UserProfile schemafull {\n  name string\n  nickname string\n}\n\n\
         table audit_log schemaless { message FutureType }"
    )
    .parses_as(
        "(SourceFile
            (Table UserProfile Schemafull
                (Field name (Name string))
                (Field nickname (Name string)))
            (Table audit_log Schemaless
                (Field message (Name FutureType))))",
    );
}

#[test]
fn table_bodies_allow_blank_lines_around_fields() {
    aurl_test!("table User schemafull {\n\n  name string\n\n}")
        .parses_as("(SourceFile (Table User Schemafull (Field name (Name string))))");
}

#[test]
fn duplicate_table_declarations_remain_distinct_and_in_source_order() {
    aurl_test!(
        "table User schemafull {}\n\
         table User schemaless { first string }\n\
         table User schemafull { second int }"
    )
    .parses_as(
        "(SourceFile
            (Table User Schemafull)
            (Table User Schemaless (Field first (Name string)))
            (Table User Schemafull (Field second (Name int))))",
    );
}

#[test]
fn comments_are_inert_and_block_comment_newlines_separate_fields() {
    aurl_test!(
        "/* A profile. */\n\
         table /* exact identity */ User schemafull {\n\
           name string // display name\n\
           nickname /* still one field */ string\n\
           first string /* a physical\n\
                           boundary */ second int\n\
         }"
    )
    .parses_as(
        "(SourceFile
            (Table User Schemafull
                (Field name (Name string))
                (Field nickname (Name string))
                (Field first (Name string))
                (Field second (Name int))))",
    );
}

#[test]
fn block_comments_close_at_the_first_terminator() {
    aurl_test!("/* outer /* inner */ table User schemafull {}")
        .parses_as("(SourceFile (Table User Schemafull))");
}

#[test]
fn comments_share_the_language_physical_newline_rule() {
    aurl_test!("// comment\rtable User schemafull {}")
        .parses_as("(SourceFile (Table User Schemafull))");
    aurl_test!("table User schemafull { first string /* boundary\rinside */ second int }")
        .parses_as(
            "(SourceFile
            (Table User Schemafull
                (Field first (Name string))
                (Field second (Name int))))",
        );
}

#[test]
fn a_single_line_block_comment_does_not_separate_fields() {
    let errors = aureline_parser::parse(
        "table User schemafull { first string /* no boundary */ second int }",
    )
    .expect_err("a physical newline must separate adjacent fields");

    assert!(matches!(
        errors.as_slice(),
        [SyntaxProblem::UnexpectedToken { .. }]
    ));
}

#[test]
fn unterminated_block_comment_points_at_its_opening() {
    let errors = aureline_parser::parse("table User schemafull {} /* open")
        .expect_err("unterminated block comment is invalid");

    assert_eq!(
        errors,
        vec![SyntaxProblem::UnterminatedBlockComment {
            span: span(SourceId::new(0), 25, 27),
        }]
    );
}

#[test]
fn comment_kinds_and_multibyte_locations_are_retained() {
    let source_id = SourceId::new(4);
    let ast =
        aureline_parser::parse_with_source(source_id, "// é\n/* b */\ntable User schemafull {}")
            .expect("comments are valid");

    assert_eq!(ast.comments().len(), 2);
    assert_eq!(ast.comments()[0].kind(), CommentKind::Line);
    assert_eq!(ast.comments()[0].span(), span(source_id, 0, 5));
    assert_eq!(ast.comments()[1].kind(), CommentKind::Block);
    assert_eq!(ast.comments()[1].span(), span(source_id, 6, 13));
}

#[test]
fn multibyte_table_text_has_precise_utf8_byte_spans() {
    let source_id = SourceId::new(9);
    let ast = aureline_parser::parse_with_source(source_id, "table Café schemafull {}")
        .expect("table parses");
    let table = ast
        .table(ast.root().tables()[0])
        .expect("table item is allocated");

    assert_eq!(table.span(), span(source_id, 0, 25));
    assert_eq!(table.name_span(), span(source_id, 6, 11));
    assert_eq!(table.schema_type_span(), span(source_id, 12, 22));
}

#[test]
fn field_arena_preserves_ownership_source_order_and_precise_spans() {
    let source_id = SourceId::new(7);
    let ast = aureline_parser::parse_with_source(
        source_id,
        "table User schemafull { name string\n  age int }",
    )
    .expect("table parses");
    let table_id = ast.root().tables()[0];
    let table = ast.table(table_id).expect("table is allocated");

    assert_eq!(table.span(), span(source_id, 0, 47));
    assert_eq!(table.fields().len(), 2);

    let name = ast
        .field(table.fields()[0])
        .expect("name field is allocated");
    assert_eq!(name.owner(), table_id);
    assert_eq!(name.name(), "name");
    assert_eq!(name.span(), span(source_id, 24, 35));
    assert_eq!(name.name_span(), span(source_id, 24, 28));
    assert_eq!(name.source_type().span(), span(source_id, 29, 35));
    let SourceType::Name(type_name) = name.source_type();
    assert_eq!(type_name.name(), "string");
    assert_eq!(type_name.span(), span(source_id, 29, 35));

    let age = ast
        .field(table.fields()[1])
        .expect("age field is allocated");
    assert_eq!(age.owner(), table_id);
    assert_eq!(age.name(), "age");
    assert_eq!(age.span(), span(source_id, 38, 45));
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

#[test]
fn unsupported_field_separators_are_typed_syntax_problems() {
    let adjacent = aureline_parser::parse("table User schemafull { name string age int }")
        .expect_err("adjacent fields require a newline");
    assert!(matches!(
        adjacent.as_slice(),
        [SyntaxProblem::UnexpectedToken { .. }]
    ));

    for source in [
        "table User schemafull { name string, }",
        "table User schemafull { name string; }",
    ] {
        let errors = aureline_parser::parse(source).expect_err("punctuation is not a separator");
        assert!(matches!(
            errors.as_slice(),
            [SyntaxProblem::InvalidToken { .. }]
        ));
    }
}

fn span(source: SourceId, start: u32, end: u32) -> SourceSpan {
    SourceSpan::new(
        source,
        TextRange::new(TextSize::new(start), TextSize::new(end)).expect("test range is ordered"),
    )
}
