use aureline_ast::{ast::Ast, source::SourceId};
use aureline_checker::{ArgumentRole, Finding, SemanticType, TypeResolution, check};

#[test]
fn bare_and_constrained_collections_keep_their_distinct_contracts() {
    let ast = parse(
        "table Catalog schemafull {\n\
           everything array\n\
           labels set\n\
           names array<string>\n\
           tags set<int>\n\
           fixed array<string, 3>\n\
           bounded set<string, 3>\n\
           upper_array ARRAY<string, 1>\n\
           upper_set SET<int>\n\
           nested array<set<string, 2>>\n\
           empty_set set<int, 0>\n\
         }",
    );
    let analysis = check(&ast);
    assert!(analysis.findings().is_empty());
    let checked = analysis
        .into_checked()
        .expect("valid collection declarations should produce a Checked Program");
    let table = checked.tables()[0];

    let expected_types = [
        (
            "everything",
            SemanticType::Array {
                element: Box::new(SemanticType::Any),
                exact_length: None,
            },
        ),
        (
            "labels",
            SemanticType::Set {
                element: Box::new(SemanticType::Any),
                max_distinct: None,
            },
        ),
        (
            "names",
            SemanticType::Array {
                element: Box::new(SemanticType::String),
                exact_length: None,
            },
        ),
        (
            "tags",
            SemanticType::Set {
                element: Box::new(SemanticType::Int),
                max_distinct: None,
            },
        ),
        (
            "fixed",
            SemanticType::Array {
                element: Box::new(SemanticType::String),
                exact_length: Some(3),
            },
        ),
        (
            "bounded",
            SemanticType::Set {
                element: Box::new(SemanticType::String),
                max_distinct: Some(3),
            },
        ),
        (
            "upper_array",
            SemanticType::Array {
                element: Box::new(SemanticType::String),
                exact_length: Some(1),
            },
        ),
        (
            "upper_set",
            SemanticType::Set {
                element: Box::new(SemanticType::Int),
                max_distinct: None,
            },
        ),
        (
            "nested",
            SemanticType::Array {
                element: Box::new(SemanticType::Set {
                    element: Box::new(SemanticType::String),
                    max_distinct: Some(2),
                }),
                exact_length: None,
            },
        ),
        (
            "empty_set",
            SemanticType::Set {
                element: Box::new(SemanticType::Int),
                max_distinct: Some(0),
            },
        ),
    ];
    for (name, expected) in expected_types {
        assert_field_type(&checked, table, name, &expected);
    }
}

#[test]
fn constructor_arity_is_reported_before_meaningful_argument_checks() {
    let ast = parse("table T schemafull { value array<Unknown, 1, 2> }");
    let analysis = check(&ast);
    assert_eq!(analysis.findings().len(), 2);
    assert!(matches!(
        &analysis.findings()[0],
        Finding::WrongArity {
            name,
            minimum: 1,
            maximum: 2,
            actual: 3,
            ..
        } if name == "array"
    ));
    assert!(matches!(
        &analysis.findings()[1],
        Finding::UnknownType { name, .. } if name == "Unknown"
    ));
    assert_invalid_field(&analysis, 0);
    assert!(analysis.into_checked().is_err());
}

#[test]
fn wrong_argument_roles_are_independent_and_positioned() {
    let ast = parse("table T schemafull { value array<1, string> }");
    let analysis = check(&ast);
    assert_eq!(analysis.findings().len(), 2);
    assert!(matches!(
        &analysis.findings()[0],
        Finding::WrongArgumentRole {
            name,
            position: 0,
            expected: ArgumentRole::ElementType,
            ..
        } if name == "array"
    ));
    assert!(matches!(
        &analysis.findings()[1],
        Finding::WrongArgumentRole {
            name,
            position: 1,
            expected: ArgumentRole::CollectionSize,
            ..
        } if name == "array"
    ));
    assert_invalid_field(&analysis, 0);
}

#[test]
fn sized_collections_accept_zero_and_u64_max_but_reject_overflow() {
    let ast = parse(
        "table T schemafull {\n\
           empty array<string, 0>\n\
           all set<string, 18446744073709551615>\n\
           overflow set<string, 18446744073709551616>\n\
         }",
    );
    let analysis = check(&ast);
    assert_eq!(analysis.findings().len(), 1);
    assert!(matches!(
        &analysis.findings()[0],
        Finding::InvalidCollectionSize { name, raw, .. }
            if name == "set" && raw == "18446744073709551616"
    ));
    assert_invalid_field(&analysis, 2);
}

#[test]
fn invalid_element_and_size_findings_do_not_create_a_collection_cascade() {
    let ast = parse("table T schemafull { value array<Unknown, 18446744073709551616> }");
    let analysis = check(&ast);
    assert_eq!(analysis.findings().len(), 2);
    assert!(matches!(
        &analysis.findings()[0],
        Finding::UnknownType { name, .. } if name == "Unknown"
    ));
    assert!(matches!(
        &analysis.findings()[1],
        Finding::InvalidCollectionSize { name, raw, .. }
            if name == "array" && raw == "18446744073709551616"
    ));
    assert_invalid_field(&analysis, 0);
}

#[test]
fn collection_argument_findings_retain_the_offending_source_span() {
    let ast = aureline_parser::parse_with_source(
        SourceId::new(17),
        "// é\ntable T schemafull { value set<string, 18446744073709551616> }",
    )
    .expect("the collection declaration is valid syntax");
    let analysis = check(&ast);
    let Finding::InvalidCollectionSize { span, .. } = &analysis.findings()[0] else {
        panic!("the fixture should report an invalid collection size");
    };
    assert_eq!(span.source(), SourceId::new(17));
    assert_eq!(span.range().start().get(), 45);
    assert_eq!(span.range().end().get(), 65);
}

fn parse(source: &str) -> Ast {
    aureline_parser::parse(source).expect("the test source should parse")
}

fn assert_invalid_field(analysis: &aureline_checker::Analysis<'_>, field_index: usize) {
    let table = analysis.tables()[0];
    let field = analysis.fields_of(table)[field_index];
    assert!(matches!(
        analysis.type_of_field(field),
        Some(TypeResolution::Invalid(_))
    ));
}

fn assert_field_type(
    checked: &aureline_checker::CheckedProgram<'_>,
    table: aureline_ast::ids::TableId,
    name: &str,
    expected: &SemanticType,
) {
    let field = checked
        .fields_of(table)
        .iter()
        .find(|&&field| checked.field(field).is_some_and(|decl| decl.name() == name))
        .copied()
        .expect("the expected field exists");
    assert_eq!(
        checked.type_of_field(field),
        Some(expected),
        "semantic type for {name}"
    );
}
