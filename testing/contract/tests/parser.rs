use aureline_ast::{
    ast::{CommentKind, SourceType, TypeArgument},
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
fn a_type_name_applies_to_a_type_argument() {
    aurl_test!("table User schemafull { names array<string> }").parses_as(
        "(SourceFile
            (Table User Schemafull
                (Field names
                    (Application
                        (Name array)
                        (Type (Name string))))))",
    );
}

#[test]
fn type_applications_preserve_ordered_type_and_integer_arguments() {
    aurl_test!("table Place schemafull { coordinates array<float, 3> }").parses_as(
        "(SourceFile
            (Table Place Schemafull
                (Field coordinates
                    (Application
                        (Name array)
                        (Type (Name float))
                        (Integer 3)))))",
    );
}

#[test]
fn type_applications_are_recursive_open_and_meaning_free() {
    aurl_test!(
        "table Shape schemafull {\n\
           future FutureType\n\
           custom custom_type<string, 003>\n\
           nested array<array<float, 3>, 2>\n\
           optional option<string>\n\
           array_name array\n\
           record_name record\n\
           record_value record<User>\n\
           spaced array< string >\n\
         }"
    )
    .parses_as(
        "(SourceFile
            (Table Shape Schemafull
                (Field future (Name FutureType))
                (Field custom
                    (Application
                        (Name custom_type)
                        (Type (Name string))
                        (Integer 003)))
                (Field nested
                    (Application
                        (Name array)
                        (Type
                            (Application
                                (Name array)
                                (Type (Name float))
                                (Integer 3)))
                        (Integer 2)))
                (Field optional
                    (Application (Name option) (Type (Name string))))
                (Field array_name (Name array))
                (Field record_name (Name record))
                (Field record_value
                    (Application (Name record) (Type (Name User))))
                (Field spaced
                    (Application (Name array) (Type (Name string))))))",
    );
}

#[test]
fn recursive_type_applications_retain_precise_spans_and_integer_spelling() {
    let source_id = SourceId::new(17);
    let ast = aureline_parser::parse_with_source(
        source_id,
        "table T schemafull { value custom_type<array<float, 3>, 003> }",
    )
    .expect("recursive applications should parse");
    let table = ast
        .table(ast.root().tables()[0])
        .expect("the table ID belongs to this AST");
    let field = ast
        .field(table.fields()[0])
        .expect("the field ID belongs to this AST");

    let SourceType::Application(outer) = field.source_type() else {
        panic!("the field uses an applied source type");
    };
    assert_eq!(outer.name().name(), "custom_type");
    assert_eq!(outer.name().span(), span(source_id, 27, 38));
    assert_eq!(outer.span(), span(source_id, 27, 60));
    assert_eq!(outer.arguments().len(), 2);

    let TypeArgument::Type(SourceType::Application(inner)) = &outer.arguments()[0] else {
        panic!("the first argument is a nested application");
    };
    assert_eq!(inner.name().name(), "array");
    assert_eq!(inner.name().span(), span(source_id, 39, 44));
    assert_eq!(inner.span(), span(source_id, 39, 54));
    assert_eq!(inner.arguments()[0].span(), span(source_id, 45, 50));
    let TypeArgument::Integer(length) = &inner.arguments()[1] else {
        panic!("the nested second argument is an integer");
    };
    assert_eq!(length.raw(), "3");
    assert_eq!(length.span(), span(source_id, 52, 53));

    let TypeArgument::Integer(length) = &outer.arguments()[1] else {
        panic!("the outer second argument is an integer");
    };
    assert_eq!(length.raw(), "003");
    assert_eq!(length.span(), span(source_id, 56, 59));
}

#[test]
fn an_empty_type_argument_list_is_incomplete() {
    let errors = aureline_parser::parse("table User schemafull { values array<> }")
        .expect_err("a type application requires at least one argument");
    assert_eq!(
        errors,
        vec![SyntaxProblem::EmptyTypeArguments {
            span: span(SourceId::new(0), 36, 38),
        }]
    );
}

#[test]
fn a_trailing_type_argument_comma_is_incomplete() {
    let errors = aureline_parser::parse("table User schemafull { values array<string,> }")
        .expect_err("a comma must be followed by another type argument");
    assert_eq!(
        errors,
        vec![SyntaxProblem::TrailingTypeArgumentComma {
            span: span(SourceId::new(0), 43, 44),
        }]
    );
}

#[test]
fn postfix_optional_type_syntax_directs_callers_to_option_application() {
    let errors = aureline_parser::parse("table User schemafull { value string? }")
        .expect_err("optional types use option<T>, not postfix question marks");
    assert_eq!(
        errors,
        vec![SyntaxProblem::PostfixOptionalType {
            span: span(SourceId::new(0), 36, 37),
        }]
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
    let SourceType::Name(type_name) = name.source_type() else {
        panic!("the field uses a bare source type name");
    };
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

    let comma = aureline_parser::parse("table User schemafull { name string, }")
        .expect_err("a comma is not a field separator");
    assert!(matches!(
        comma.as_slice(),
        [SyntaxProblem::UnexpectedToken { .. }]
    ));

    let semicolon = aureline_parser::parse("table User schemafull { name string; }")
        .expect_err("a semicolon is not a field separator");
    assert!(matches!(
        semicolon.as_slice(),
        [SyntaxProblem::InvalidToken { .. }]
    ));
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
fn a_pure_integer_cannot_be_a_declared_name() {
    for (source, start, end) in [
        ("table 1 schemafull {}", 6, 7),
        ("table User schemafull { 1 string }", 24, 25),
    ] {
        let errors = aureline_parser::parse(source)
            .expect_err("an integer cannot be used as a declared name");
        assert_eq!(
            errors,
            vec![SyntaxProblem::InvalidIdentifier {
                problem: IdentifierProblem::StartsWithDigit,
                span: span(SourceId::new(0), start, end),
            }]
        );
    }
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
        (
            "table User?Name schemafull {}",
            IdentifierProblem::ContainsPunctuation('?'),
        ),
        (
            "table User<Name schemafull {}",
            IdentifierProblem::ContainsPunctuation('<'),
        ),
        (
            "table User>Name schemafull {}",
            IdentifierProblem::ContainsPunctuation('>'),
        ),
        (
            "table User,Name schemafull {}",
            IdentifierProblem::ContainsPunctuation(','),
        ),
        (
            "table User??Name schemafull {}",
            IdentifierProblem::ContainsPunctuation('?'),
        ),
        (
            "table User?Name,More schemafull {}",
            IdentifierProblem::ContainsPunctuation('?'),
        ),
        (
            "table User?Name , More schemafull {}",
            IdentifierProblem::ContainsPunctuation('?'),
        ),
        (
            "table User?1 schemafull {}",
            IdentifierProblem::ContainsPunctuation('?'),
        ),
        (
            "table User?table schemafull {}",
            IdentifierProblem::ContainsPunctuation('?'),
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

    let errors = aureline_parser::parse("table array<string> schemafull {}")
        .expect_err("an applied type shape cannot be a table name");
    assert_eq!(
        errors,
        vec![SyntaxProblem::InvalidIdentifier {
            problem: IdentifierProblem::ContainsPunctuation('<'),
            span: span(SourceId::new(0), 11, 12),
        }]
    );

    let errors = aureline_parser::parse("table array<3> schemafull {}")
        .expect_err("an integer argument shape cannot be a table name");
    assert_eq!(
        errors,
        vec![SyntaxProblem::InvalidIdentifier {
            problem: IdentifierProblem::ContainsPunctuation('<'),
            span: span(SourceId::new(0), 11, 12),
        }]
    );

    let errors = aureline_parser::parse("table array<string,3> schemafull {}")
        .expect_err("a multi-argument application shape cannot be a table name");
    assert_eq!(
        errors,
        vec![SyntaxProblem::InvalidIdentifier {
            problem: IdentifierProblem::ContainsPunctuation('<'),
            span: span(SourceId::new(0), 11, 12),
        }]
    );

    for source in [
        "table User ? Name schemafull {}",
        "table User schemafull { na ? me string }",
    ] {
        let errors = aureline_parser::parse(source)
            .expect_err("separate punctuation is syntax, not part of an identifier");
        assert!(matches!(
            errors.as_slice(),
            [SyntaxProblem::UnexpectedToken { .. }]
        ));
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
fn structural_type_punctuation_in_field_names_retains_the_first_violation() {
    for (candidate, punctuation) in [
        ("na?me", '?'),
        ("na<me", '<'),
        ("na>me", '>'),
        ("na,me", ','),
        ("na??me", '?'),
        ("na?me,more", '?'),
        ("na?me , more", '?'),
        ("User?1", '?'),
        ("User?table", '?'),
        ("array<string>", '<'),
        ("array<3>", '<'),
        ("array<string,3>", '<'),
    ] {
        let source = format!("table User schemafull {{ {candidate} string }}");
        let punctuation_offset = candidate
            .find(punctuation)
            .expect("the case contains its expected punctuation");
        let candidate_start = source
            .find(candidate)
            .expect("the generated source contains the field-name candidate");
        let start = u32::try_from(candidate_start + punctuation_offset)
            .expect("the short contract source fits Aureline text offsets");
        let errors = aureline_parser::parse(&source)
            .expect_err("structural punctuation cannot occur in a field name");
        assert_eq!(
            errors,
            vec![SyntaxProblem::InvalidIdentifier {
                problem: IdentifierProblem::ContainsPunctuation(punctuation),
                span: span(SourceId::new(0), start, start + 1),
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
