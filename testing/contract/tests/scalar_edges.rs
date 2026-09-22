//! Scalar-resolution contracts for diagnostics, ordering, and exact spans.
//!
//! These cases cover supported spellings, unsupported keywords, bare table
//! references, duplicate declarations, and invalid checked outcomes without
//! allowing one bad field to hide another.

use aureline_ast::{ast::Ast, ids::FieldId, source::SourceId};
use aureline_checker::{Finding, SemanticType, TypeResolution, check};

fn parse(source: &str) -> Ast {
    aureline_parser::parse(source).expect("source parses")
}

fn parse_with_source(source_id: SourceId, source: &str) -> Ast {
    aureline_parser::parse_with_source(source_id, source).expect("source parses")
}

#[test]
fn independent_scalar_errors_keep_source_order_and_precise_utf8_spans() {
    let source = "// é\ntable Target schemafull {\nmissing UnknownType\nlocation geometry\nowner Other\n}\ntable Other schemafull {}";
    let ast = parse_with_source(SourceId::new(37), source);
    let analysis = check(&ast);
    assert_eq!(analysis.findings().len(), 3);

    let target = ast.root().tables()[0];
    let fields = ast.table(target).expect("Target exists").fields();
    let source_span = |field_id: FieldId| {
        ast.field(field_id)
            .expect("the field belongs to Target")
            .source_type()
            .span()
    };

    match &analysis.findings()[0] {
        Finding::UnknownType { name, span } => {
            assert_eq!(name, "UnknownType");
            assert_eq!(*span, source_span(fields[0]));
            assert_eq!(span.source(), SourceId::new(37));
        }
        finding => panic!("expected the unknown type first, got {finding:?}"),
    }
    match &analysis.findings()[1] {
        Finding::UnsupportedType { name, span } => {
            assert_eq!(name, "geometry");
            assert_eq!(*span, source_span(fields[1]));
            assert_eq!(span.range().start().get(), 61);
        }
        finding => panic!("expected the unsupported type second, got {finding:?}"),
    }
    match &analysis.findings()[2] {
        Finding::BareTableType { name, span } => {
            assert_eq!(name, "Other");
            assert_eq!(*span, source_span(fields[2]));
        }
        finding => panic!("expected the bare table type third, got {finding:?}"),
    }
}

#[test]
fn duplicate_declarations_do_not_hide_independent_bad_types() {
    let ast = parse(
        "table User schemafull {\n\
           id UnknownOne\n\
           id geometry\n\
         }\n\
         table User schemafull {\n\
           id UnknownTwo\n\
         }",
    );
    let analysis = check(&ast);
    let findings = analysis.findings();
    assert_eq!(findings.len(), 5);
    assert!(matches!(&findings[0], Finding::DuplicateTable { name, .. } if name == "User"));
    assert!(matches!(&findings[1], Finding::DuplicateField { name, .. } if name == "id"));
    assert!(matches!(&findings[2], Finding::UnknownType { name, .. } if name == "UnknownOne"));
    assert!(matches!(&findings[3], Finding::UnsupportedType { name, .. } if name == "geometry"));
    assert!(matches!(&findings[4], Finding::UnknownType { name, .. } if name == "UnknownTwo"));
}

#[test]
fn rejected_analysis_retains_all_findings_and_invalid_field_outcomes() {
    let ast = parse(
        "table User schemafull {\n\
           unknown Unknown\n\
           unsupported file\n\
         }",
    );
    let target = ast.root().tables()[0];
    let field_ids = ast.table(target).expect("User exists").fields();
    let Err(analysis) = check(&ast).into_checked() else {
        panic!("invalid field types must reject the checked program");
    };
    assert_eq!(analysis.findings().len(), 2);
    assert!(
        matches!(&analysis.findings()[0], Finding::UnknownType { name, .. } if name == "Unknown")
    );
    assert!(
        matches!(&analysis.findings()[1], Finding::UnsupportedType { name, .. } if name == "file")
    );
    assert_eq!(
        field_ids
            .iter()
            .filter(|field_id| {
                matches!(
                    analysis.type_of_field(**field_id),
                    Some(TypeResolution::Invalid(_))
                )
            })
            .count(),
        2
    );
}

#[test]
fn supported_scalar_names_take_precedence_over_declared_table_names() {
    let ast = parse("table string schemafull {}\ntable User schemafull { value string }");
    assert!(check(&ast).findings().is_empty());

    let ast = parse("table string schemafull { value string }");
    let user = ast.root().tables()[0];
    let field = ast.table(user).expect("string exists").fields()[0];
    assert!(matches!(
        check(&ast).type_of_field(field),
        Some(TypeResolution::Resolved(SemanticType::String))
    ));
}

#[test]
fn supported_scalars_produce_a_valid_checked_program_in_source_order() {
    let ast = parse(
        "table Values schemafull {\n\
           dynamic any\n\
           enabled bool\n\
           payload bytes\n\
           happened datetime\n\
           amount decimal\n\
           elapsed duration\n\
           ratio float\n\
           count int\n\
           measure number\n\
           metadata object\n\
           window range\n\
           label string\n\
           identity uuid\n\
           absent none\n\
           empty null\n\
         }",
    );
    let table = ast.root().tables()[0];
    let fields = ast.table(table).expect("Values exists").fields();
    let expected = [
        SemanticType::Any,
        SemanticType::Bool,
        SemanticType::Bytes,
        SemanticType::Datetime,
        SemanticType::Decimal,
        SemanticType::Duration,
        SemanticType::Float,
        SemanticType::Int,
        SemanticType::Number,
        SemanticType::Object,
        SemanticType::Range,
        SemanticType::String,
        SemanticType::Uuid,
        SemanticType::None,
        SemanticType::Null,
    ];
    let checked = check(&ast)
        .into_checked()
        .expect("all supported scalar fields should be checked");
    assert_eq!(fields.len(), expected.len());
    for (&field, expected) in fields.iter().zip(expected.iter()) {
        assert_eq!(checked.type_of_field(field), Some(expected));
    }
}

#[test]
fn unsupported_and_malformed_type_forms_are_reported_at_their_heads() {
    let ast = parse(
        "table Values schemafull {\n\
           collection geometry<point>\n\
           malformed string<int>\n\
           typo FutureType<string>\n\
         }",
    );
    let analysis = check(&ast);
    let findings = analysis.findings();
    assert_eq!(findings.len(), 3);
    assert!(matches!(&findings[0], Finding::UnsupportedType { name, .. } if name == "geometry"));
    assert!(matches!(
        &findings[1],
        Finding::WrongArity {
            name,
            minimum: 0,
            maximum: 0,
            actual: 1,
            ..
        } if name == "string"
    ));
    assert!(matches!(&findings[2], Finding::UnknownType { name, .. } if name == "FutureType"));
}

#[test]
fn scalar_spellings_follow_surrealdb_case_rules_while_table_names_remain_exact() {
    let ast = parse(
        "table User schemafull {\n\
           upper STRING\n\
           mixed BoOl\n\
           regex ReGeX\n\
           typo user\n\
         }",
    );
    let analysis = check(&ast);
    let findings = analysis.findings();
    assert_eq!(findings.len(), 2);
    assert!(matches!(&findings[0], Finding::UnsupportedType { name, .. } if name == "ReGeX"));
    assert!(matches!(&findings[1], Finding::UnknownType { name, .. } if name == "user"));

    let ast = parse("table User schemafull { value STRING }");
    assert!(check(&ast).findings().is_empty());
}

#[test]
fn real_unsupported_type_keywords_retain_their_original_spelling() {
    let ast = parse("table T schemafull { value TABLE }");
    let analysis = check(&ast);
    assert!(
        matches!(analysis.findings(), [Finding::UnsupportedType { name, .. }] if name == "TABLE")
    );
    assert!(analysis.into_checked().is_err());
}
