//! Identifier contracts define the byte-level boundary shared by table and
//! field names, including the first offending byte in malformed spellings.

use aureline_ast::source::{SourceId, SourceSpan};
use aureline_parser::{IdentifierProblem, SyntaxProblem};
use aureline_test::aurl_test;

use super::support::span;

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
            IdentifierProblem::ContainsPunctuation,
        ),
        (
            "table User?Name schemafull {}",
            IdentifierProblem::ContainsPunctuation,
        ),
        (
            "table User<Name schemafull {}",
            IdentifierProblem::ContainsPunctuation,
        ),
        (
            "table User>Name schemafull {}",
            IdentifierProblem::ContainsPunctuation,
        ),
        (
            "table User,Name schemafull {}",
            IdentifierProblem::ContainsPunctuation,
        ),
        (
            "table User??Name schemafull {}",
            IdentifierProblem::ContainsPunctuation,
        ),
        (
            "table User?Name,More schemafull {}",
            IdentifierProblem::ContainsPunctuation,
        ),
        (
            "table User?Name , More schemafull {}",
            IdentifierProblem::ContainsPunctuation,
        ),
        (
            "table User?1 schemafull {}",
            IdentifierProblem::ContainsPunctuation,
        ),
        (
            "table User?table schemafull {}",
            IdentifierProblem::ContainsPunctuation,
        ),
    ] {
        assert_invalid_identifier(source, problem, span(SourceId::new(0), 10, 11));
    }

    let errors = aureline_parser::parse("table array<string> schemafull {}")
        .expect_err("an applied type shape cannot be a table name");
    assert_eq!(
        errors,
        vec![SyntaxProblem::InvalidIdentifier {
            problem: IdentifierProblem::ContainsPunctuation,
            span: span(SourceId::new(0), 11, 12),
        }]
    );

    let errors = aureline_parser::parse("table array<3> schemafull {}")
        .expect_err("an integer argument shape cannot be a table name");
    assert_eq!(
        errors,
        vec![SyntaxProblem::InvalidIdentifier {
            problem: IdentifierProblem::ContainsPunctuation,
            span: span(SourceId::new(0), 11, 12),
        }]
    );

    let errors = aureline_parser::parse("table array<string,3> schemafull {}")
        .expect_err("a multi-argument application shape cannot be a table name");
    assert_eq!(
        errors,
        vec![SyntaxProblem::InvalidIdentifier {
            problem: IdentifierProblem::ContainsPunctuation,
            span: span(SourceId::new(0), 11, 12),
        }]
    );

    for source in [
        "table User ? Name schemafull {}",
        "table User schemafull { na ? me string }",
    ] {
        let errors = aureline_parser::parse(source)
            .expect_err("separate punctuation still violates the declared name");
        assert!(matches!(
            errors.as_slice(),
            [SyntaxProblem::InvalidIdentifier {
                problem: IdentifierProblem::ContainsPunctuation,
                ..
            }]
        ));
    }
}

#[test]
fn identifier_cannot_start_with_punctuation() {
    for (source, problem) in [
        ("table .User schemafull {}", IdentifierProblem::ContainsDot),
        (
            "table -User schemafull {}",
            IdentifierProblem::ContainsHyphen,
        ),
        (
            "table @User schemafull {}",
            IdentifierProblem::ContainsPunctuation,
        ),
    ] {
        assert_invalid_identifier(source, problem, span(SourceId::new(0), 6, 7));
    }

    for (source, problem) in [
        (
            "table User schemafull { .name string }",
            IdentifierProblem::ContainsDot,
        ),
        (
            "table User schemafull { -name string }",
            IdentifierProblem::ContainsHyphen,
        ),
        (
            "table User schemafull { @name string }",
            IdentifierProblem::ContainsPunctuation,
        ),
    ] {
        assert_invalid_identifier(source, problem, span(SourceId::new(0), 24, 25));
    }
}

fn assert_invalid_identifier(source: &str, problem: IdentifierProblem, location: SourceSpan) {
    let errors = aureline_parser::parse(source)
        .expect_err("the public parser should reject the invalid identifier");
    assert_eq!(
        errors,
        vec![SyntaxProblem::InvalidIdentifier {
            problem,
            span: location,
        }]
    );
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
    for source in [
        "table User schemafull { first name string }",
        "table User schemafull { first name array<string> }",
        "table User schemafull { first name [string] }",
    ] {
        let errors = aureline_parser::parse(source)
            .expect_err("a field identifier cannot contain whitespace");

        assert_eq!(
            errors,
            vec![SyntaxProblem::InvalidIdentifier {
                problem: IdentifierProblem::ContainsWhitespace,
                span: span(SourceId::new(0), 29, 30),
            }]
        );
    }
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
            IdentifierProblem::ContainsPunctuation,
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
    for candidate in [
        "na?me",
        "na<me",
        "na>me",
        "na,me",
        "na??me",
        "na?me,more",
        "na?me , more",
        "User?1",
        "User?table",
        "array<string>",
        "array<3>",
        "array<string,3>",
    ] {
        let source = format!("table User schemafull {{ {candidate} string }}");
        let punctuation_offset = candidate
            .find(|character: char| character.is_ascii_punctuation() && character != '_')
            .expect("the case contains punctuation");
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
                problem: IdentifierProblem::ContainsPunctuation,
                span: span(SourceId::new(0), start, start + 1),
            }]
        );
    }
}

#[test]
fn postfix_array_brackets_are_identifier_punctuation_in_declared_names() {
    for (source, opening) in [
        ("table User[] schemafull {}", 10),
        ("table User schemafull { User[] string }", 28),
    ] {
        let errors = aureline_parser::parse(source)
            .expect_err("postfix brackets cannot occur in a declared name");
        assert_eq!(
            errors,
            vec![SyntaxProblem::InvalidIdentifier {
                problem: IdentifierProblem::ContainsPunctuation,
                span: span(SourceId::new(0), opening, opening + 1),
            }]
        );
    }
}

#[test]
fn compound_identifier_violations_report_the_first_offending_bytes() {
    for (source, problem, start, end) in [
        (
            "table User::Name schemafull {}",
            IdentifierProblem::ContainsPunctuation,
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
            problem: IdentifierProblem::ContainsPunctuation,
            span: span(SourceId::new(0), 10, 11),
        }]
    );

    aurl_test!("table User/* still a comment */schemafull {}")
        .parses_as("(SourceFile (Table User Schemafull))");
}
