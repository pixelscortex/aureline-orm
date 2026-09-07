use aureline_ast::{ids::FieldId, source::SourceId};
use aureline_checker::{Finding, SemanticType, TypeResolution, check};
use aureline_test::aurl_test;
use serde::Serialize;

#[derive(Serialize)]
struct RejectionView {
    findings: Vec<Finding>,
    rejected: bool,
    invalid_outcomes: usize,
}

#[test]
fn independent_scalar_errors_keep_source_order_and_precise_utf8_spans() {
    aurl_test!(
        "// é\n\
         table Target schemafull {\n\
           missing UnknownType\n\
           location geometry\n\
           owner Other\n\
         }\n\
         table Other schemafull {}"
    )
    .with_source_id(SourceId::new(37))
    .reports_with(
        |ast| {
            let analysis = check(ast);
            assert_eq!(analysis.findings().len(), 3);

            let target = ast
                .root()
                .tables()
                .first()
                .copied()
                .expect("the fixture declares Target");
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

            true
        },
        "true",
    );
}

#[test]
fn duplicate_declarations_do_not_hide_independent_bad_types() {
    aurl_test!(
        "table User schemafull {\n\
           id UnknownOne\n\
           id geometry\n\
         }\n\
         table User schemafull {\n\
           id UnknownTwo\n\
         }"
    )
    .findings(
        "(Findings
            (DuplicateTable User)
            (DuplicateField id)
            (UnknownType UnknownOne)
            (UnsupportedType geometry)
            (UnknownType UnknownTwo)
        )",
    );
}

#[test]
fn rejected_analysis_retains_all_findings_and_invalid_field_outcomes() {
    aurl_test!(
        "table User schemafull {\n\
           unknown Unknown\n\
           unsupported file\n\
         }"
    )
    .reports_with(
        |ast| {
            let target = ast.root().tables()[0];
            let field_ids = ast.table(target).expect("User exists").fields();
            match check(ast).into_checked() {
                Err(analysis) => {
                    let invalid_outcomes = field_ids
                        .iter()
                        .filter(|field_id| {
                            matches!(
                                analysis.type_of_field(**field_id),
                                Some(TypeResolution::Invalid(_))
                            )
                        })
                        .count();
                    RejectionView {
                        findings: analysis.findings().to_vec(),
                        rejected: true,
                        invalid_outcomes,
                    }
                }
                Ok(_) => RejectionView {
                    findings: Vec::new(),
                    rejected: false,
                    invalid_outcomes: 0,
                },
            }
        },
        "(RejectionView (UnknownType Unknown) (UnsupportedType file) true 2)",
    );
}

#[test]
fn supported_scalar_names_take_precedence_over_declared_table_names() {
    aurl_test!(
        "table string schemafull {}\n\
         table User schemafull { value string }"
    )
    .findings("(Findings)");

    aurl_test!("table string schemafull { value string }").reports_with(
        |ast| {
            let user = ast.root().tables()[0];
            let field = ast.table(user).expect("string exists").fields()[0];
            let analysis = check(ast);
            assert!(matches!(
                analysis.type_of_field(field),
                Some(TypeResolution::Resolved(SemanticType::String))
            ));
            true
        },
        "true",
    );
}

#[test]
fn supported_scalars_produce_a_valid_checked_program_in_source_order() {
    aurl_test!(
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
         }"
    )
    .checks_as(
        "(CheckedProgram
            (Table Values
                (Field dynamic Any)
                (Field enabled Bool)
                (Field payload Bytes)
                (Field happened Datetime)
                (Field amount Decimal)
                (Field elapsed Duration)
                (Field ratio Float)
                (Field count Int)
                (Field measure Number)
                (Field metadata Object)
                (Field window Range)
                (Field label String)
                (Field identity Uuid)
                (Field absent None)
                (Field empty Null)))",
    );
}

#[test]
fn unsupported_and_malformed_type_forms_are_reported_at_their_heads() {
    aurl_test!(
        "table Values schemafull {\n\
           collection array<string>\n\
           malformed string<int>\n\
           typo FutureType<string>\n\
         }"
    )
    .findings(
        "(Findings
            (UnsupportedType array)
            (WrongArity string)
            (UnknownType FutureType))",
    );
}

#[test]
fn scalar_spellings_follow_surrealdb_case_rules_while_table_names_remain_exact() {
    aurl_test!(
        "table User schemafull {\n\
           upper STRING\n\
           mixed BoOl\n\
           regex ReGeX\n\
           typo user\n\
         }"
    )
    .findings("(Findings (UnsupportedType ReGeX) (UnknownType user))");

    aurl_test!("table User schemafull { value STRING }").findings("(Findings)");
}
