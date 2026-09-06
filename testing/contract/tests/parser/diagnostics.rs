//! Diagnostic contracts distinguish typed recovery cases from generic syntax
//! failures, including the spans that let callers explain malformed input.

use aureline_ast::source::SourceId;
use aureline_parser::SyntaxProblem;

use super::support::span;

#[test]
fn a_union_pipe_without_a_member_is_a_typed_problem() {
    for (source, start, end) in [
        ("table T schemafull { value | string }", 27, 28),
        ("table T schemafull { value string | }", 34, 35),
        ("table T schemafull { value string | | int }", 36, 37),
    ] {
        let errors = aureline_parser::parse(source)
            .expect_err("every union pipe must have a member on both sides");
        assert_eq!(
            errors,
            vec![SyntaxProblem::MissingUnionMember {
                span: span(SourceId::new(0), start, end),
            }]
        );
    }
}

#[test]
fn postfix_empty_brackets_direct_callers_to_array_application() {
    let errors = aureline_parser::parse("table T schemafull { value string[] }")
        .expect_err("array types use array<T>, not postfix brackets");
    assert_eq!(
        errors,
        vec![SyntaxProblem::PostfixArrayType {
            span: span(SourceId::new(0), 33, 35),
        }]
    );
}

#[test]
fn missing_union_members_are_typed_inside_generic_arguments() {
    for (source, last_pipe) in [
        ("table T schemafull { value record<| A> }", false),
        ("table T schemafull { value record<A |> }", false),
        ("table T schemafull { value record<A | | B> }", true),
        ("table T schemafull { value record<|> }", false),
    ] {
        let pipe = if last_pipe {
            source.rfind('|')
        } else {
            source.find('|')
        }
        .expect("the malformed union contains a pipe");
        let start = u32::try_from(pipe).expect("the short contract source fits text offsets");
        let errors = aureline_parser::parse(source)
            .expect_err("a generic argument cannot omit a union member");
        assert_eq!(
            errors,
            vec![SyntaxProblem::MissingUnionMember {
                span: span(SourceId::new(0), start, start + 1),
            }]
        );
    }
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
fn collection_shapes_preserve_current_public_behavior() {
    for (source, start, end) in [
        ("table T schemafull { value array<,A> }", 33, 34),
        ("table T schemafull { value array<A,,B> }", 35, 36),
    ] {
        let errors = aureline_parser::parse(source).expect_err("the application is malformed");
        assert_eq!(
            errors,
            vec![SyntaxProblem::UnexpectedToken {
                span: span(SourceId::new(0), start, end),
            }]
        );
    }

    for (source, start) in [
        ("table T schemafull { value [,,,] }", 28),
        ("table T schemafull { value [,] }", 28),
    ] {
        let errors = aureline_parser::parse(source).expect_err("the tuple is malformed");
        assert_eq!(
            errors,
            vec![SyntaxProblem::MissingTupleMember {
                span: span(SourceId::new(0), start, start + 1),
            }]
        );
    }

    assert!(aureline_parser::parse("table T schemafull { value [A,] }").is_ok());
    let errors = aureline_parser::parse("table T schemafull { value [A B C] }")
        .expect_err("the tuple is missing separators");
    assert_eq!(
        errors,
        vec![SyntaxProblem::MissingTupleSeparator {
            span: span(SourceId::new(0), 30, 31),
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
