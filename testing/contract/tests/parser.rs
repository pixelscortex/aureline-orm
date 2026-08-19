use aureline_ast::{
    ast::{CommentKind, SourceType},
    source::{SourceId, SourceSpan, TextRange, TextSize},
};
use aureline_parser::{IdentifierProblem, SyntaxProblem};
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

#[test]
fn identifier_cannot_start_with_a_digit() {
    let errors = aureline_parser::parse("table 1User schemafull {}")
        .expect_err("an identifier cannot start with a digit");

    assert_eq!(
        errors,
        vec![SyntaxProblem::InvalidIdentifier {
            problem: IdentifierProblem::StartsWithDigit,
            span: span(SourceId::new(0), 6, 7),
        }]
    );
}

#[test]
fn identifier_cannot_contain_a_non_ascii_character() {
    let errors = aureline_parser::parse("table Café schemafull {}")
        .expect_err("an identifier cannot contain a non-ASCII character");

    assert_eq!(
        errors,
        vec![SyntaxProblem::InvalidIdentifier {
            problem: IdentifierProblem::ContainsNonAscii('é'),
            span: span(SourceId::new(0), 9, 11),
        }]
    );
}

#[test]
fn identifier_punctuation_reports_its_specific_boundary() {
    for (source, problem) in [
        (
            "table User.Name schemafull {}",
            IdentifierProblem::ContainsDot,
        ),
        (
            "table User-Name schemafull {}",
            IdentifierProblem::ContainsHyphen,
        ),
        (
            "table User@Name schemafull {}",
            IdentifierProblem::ContainsPunctuation('@'),
        ),
    ] {
        let errors = aureline_parser::parse(source)
            .expect_err("an identifier cannot contain ASCII punctuation");

        assert_eq!(
            errors,
            vec![SyntaxProblem::InvalidIdentifier {
                problem,
                span: span(SourceId::new(0), 10, 11),
            }]
        );
    }
}

#[test]
fn backtick_escaped_identifier_is_reserved() {
    let errors = aureline_parser::parse("table `User` schemafull {}")
        .expect_err("backtick-escaped identifiers are reserved");

    assert_eq!(
        errors,
        vec![SyntaxProblem::InvalidIdentifier {
            problem: IdentifierProblem::BackticksReserved,
            span: span(SourceId::new(0), 6, 12),
        }]
    );
}

#[test]
fn table_identifier_cannot_contain_whitespace() {
    let errors = aureline_parser::parse("table User Profile schemafull {}")
        .expect_err("a table identifier cannot contain whitespace");

    assert_eq!(
        errors,
        vec![SyntaxProblem::InvalidIdentifier {
            problem: IdentifierProblem::ContainsWhitespace,
            span: span(SourceId::new(0), 10, 11),
        }]
    );
}

#[test]
fn field_identifier_cannot_contain_whitespace() {
    let errors = aureline_parser::parse("table User schemafull { first name string }")
        .expect_err("a field identifier cannot contain whitespace");

    assert_eq!(
        errors,
        vec![SyntaxProblem::InvalidIdentifier {
            problem: IdentifierProblem::ContainsWhitespace,
            span: span(SourceId::new(0), 29, 30),
        }]
    );
}

#[test]
fn ascii_identifier_boundary_and_declared_spelling_are_preserved() {
    aurl_test!(
        "table _A2 schemafull {\n\
           a_1 string\n\
           Z9 record\n\
         }\n\
         table string schemafull {\n\
           record string\n\
           Table Schemafull\n\
         }"
    )
    .parses_as(
        "(SourceFile
            (Table _A2 Schemafull
                (Field a_1 (Name string))
                (Field Z9 (Name record)))
            (Table string Schemafull
                (Field record (Name string))
                (Field Table (Name Schemafull))))",
    );
}

#[test]
fn only_structural_words_are_reserved_as_names() {
    for keyword in ["table", "schemafull", "schemaless"] {
        let table_source = format!("table {keyword} schemafull {{}}");
        let table_errors = aureline_parser::parse(&table_source)
            .expect_err("a structural word cannot be a table name");
        assert!(matches!(
            table_errors.as_slice(),
            [SyntaxProblem::UnexpectedToken { .. }]
        ));

        let field_source = format!("table User schemafull {{ {keyword} string }}");
        let field_errors = aureline_parser::parse(&field_source)
            .expect_err("a structural word cannot be a field name");
        assert!(matches!(
            field_errors.as_slice(),
            [SyntaxProblem::UnexpectedToken { .. }]
        ));
    }
}

#[test]
fn field_names_share_the_identifier_boundary() {
    for (source, problem, start, end) in [
        (
            "table User schemafull { 1name string }",
            IdentifierProblem::StartsWithDigit,
            24,
            25,
        ),
        (
            "table User schemafull { na.me string }",
            IdentifierProblem::ContainsDot,
            26,
            27,
        ),
        (
            "table User schemafull { na-me string }",
            IdentifierProblem::ContainsHyphen,
            26,
            27,
        ),
        (
            "table User schemafull { na@me string }",
            IdentifierProblem::ContainsPunctuation('@'),
            26,
            27,
        ),
        (
            "table User schemafull { naéme string }",
            IdentifierProblem::ContainsNonAscii('é'),
            26,
            28,
        ),
        (
            "table User schemafull { `name` string }",
            IdentifierProblem::BackticksReserved,
            24,
            30,
        ),
    ] {
        let errors = aureline_parser::parse(source)
            .expect_err("a field name must obey the identifier boundary");
        assert_eq!(
            errors,
            vec![SyntaxProblem::InvalidIdentifier {
                problem,
                span: span(SourceId::new(0), start, end),
            }]
        );
    }
}

#[test]
fn compound_identifier_violations_report_the_first_offending_bytes() {
    for (source, problem, start, end) in [
        (
            "table User::Name schemafull {}",
            IdentifierProblem::ContainsPunctuation(':'),
            10,
            11,
        ),
        (
            "table User-é schemafull {}",
            IdentifierProblem::ContainsHyphen,
            10,
            11,
        ),
        (
            "table Useré-Name schemafull {}",
            IdentifierProblem::ContainsNonAscii('é'),
            10,
            12,
        ),
    ] {
        let errors = aureline_parser::parse(source)
            .expect_err("the first identifier-boundary violation must be reported");
        assert_eq!(
            errors,
            vec![SyntaxProblem::InvalidIdentifier {
                problem,
                span: span(SourceId::new(0), start, end),
            }]
        );
    }
}

#[test]
fn slash_inside_an_identifier_is_punctuation_not_a_comment() {
    let errors = aureline_parser::parse("table User/Name schemafull {}")
        .expect_err("a slash inside an identifier is punctuation");
    assert_eq!(
        errors,
        vec![SyntaxProblem::InvalidIdentifier {
            problem: IdentifierProblem::ContainsPunctuation('/'),
            span: span(SourceId::new(0), 10, 11),
        }]
    );

    aurl_test!("table User/* still a comment */schemafull {}")
        .parses_as("(SourceFile (Table User Schemafull))");
}

fn span(source: SourceId, start: u32, end: u32) -> SourceSpan {
    SourceSpan::new(
        source,
        TextRange::new(TextSize::new(start), TextSize::new(end)).expect("test range is ordered"),
    )
}
