//! Comments are retained as source metadata, while their physical newlines
//! still participate in field separation and error locations.

use aureline_ast::ast::CommentKind;
use aureline_ast::source::SourceId;
use aureline_parser::SyntaxProblem;
use aureline_test::aurl_test;

use super::support::span;

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
fn multibyte_prefix_preserves_precise_table_byte_spans() {
    let source_id = SourceId::new(9);
    let ast = aureline_parser::parse_with_source(source_id, "// é\ntable Cafe schemafull {}")
        .expect("a multibyte comment before an ASCII table parses");
    let table = ast
        .table(ast.root().tables()[0])
        .expect("table item is allocated");

    assert_eq!(table.span(), span(source_id, 6, 30));
    assert_eq!(table.name_span(), span(source_id, 12, 16));
    assert_eq!(table.schema_type_span(), span(source_id, 17, 27));
}
