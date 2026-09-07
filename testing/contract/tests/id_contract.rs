use aureline_ast::{ast::Ast, ids::FieldId, source::SourceId};
use aureline_checker::{
    FieldPresence, Finding, RecordTargets, SemanticType, TypeResolution, check,
};

fn parse(source: &str) -> Ast {
    aureline_parser::parse(source).expect("the fixture should parse")
}

fn first_id(ast: &Ast, table_index: usize) -> (aureline_ast::ids::TableId, FieldId) {
    let table = ast.root().tables()[table_index];
    let field = ast.table(table).expect("table exists").fields()[0];
    (table, field)
}

#[test]
fn every_allowed_outer_kind_exposes_a_borrowed_id_contract() {
    let ast = parse(
        "table User schemafull {}\n\
         table AnyKey schemafull { id any }\n\
         table NumberKey schemafull { id number }\n\
         table IntKey schemafull { id int }\n\
         table StringKey schemafull { id string }\n\
         table UuidKey schemafull { id uuid }\n\
         table ObjectKey schemafull { id object }\n\
         table ArrayKey schemafull { id array<record<User>, 2> }\n\
         table NestedArrayKey schemafull { id array<set<int>> }\n\
         table NestedTypesKey schemafull { id [[int], set<string>, option<string>] }\n\
         table EmptyTupleKey schemafull { id [] }\n\
         table SingleTupleKey schemafull { id [int] }\n\
         table TupleKey schemafull { id [datetime, uuid, int, string] }\n\
         table UnionKey schemafull { id int | string }",
    );
    let checked = check(&ast).into_checked().expect("all key kinds are valid");

    for table_index in 1..ast.root().tables().len() {
        let (table, field) = first_id(&ast, table_index);
        let contract = checked
            .id_contract(table)
            .expect("explicit valid id has a contract");
        assert_eq!(contract.field_id(), field);
        assert!(std::ptr::eq(
            contract.key_type().semantic_type(),
            checked.type_of_field(field).unwrap()
        ));
    }

    let array_table = checked.table_named("ArrayKey").unwrap();
    let array_field = checked.fields_of(array_table)[0];
    let user = checked.table_named("User").unwrap();
    assert_eq!(
        checked.type_of_field(array_field),
        Some(&SemanticType::Array {
            element: Box::new(SemanticType::Record(RecordTargets::Tables(vec![user]))),
            exact_length: Some(2),
        })
    );

    let nested_array_table = checked.table_named("NestedArrayKey").unwrap();
    assert_eq!(
        checked.type_of_field(checked.fields_of(nested_array_table)[0]),
        Some(&SemanticType::Array {
            element: Box::new(SemanticType::Set {
                element: Box::new(SemanticType::Int),
                max_distinct: None,
            }),
            exact_length: None,
        })
    );

    let nested_types_table = checked.table_named("NestedTypesKey").unwrap();
    assert!(matches!(
        checked.type_of_field(checked.fields_of(nested_types_table)[0]),
        Some(SemanticType::Tuple(members))
            if matches!(members.as_slice(), [
                SemanticType::Tuple(_),
                SemanticType::Set { .. },
                SemanticType::Union(_),
            ])
    ));

    for name in ["EmptyTupleKey", "SingleTupleKey"] {
        let table = checked.table_named(name).unwrap();
        assert!(checked.id_contract(table).is_some());
    }

    let tuple_table = checked.table_named("TupleKey").unwrap();
    let tuple_field = checked.fields_of(tuple_table)[0];
    assert_eq!(
        checked.type_of_field(tuple_field),
        Some(&SemanticType::Tuple(vec![
            SemanticType::Datetime,
            SemanticType::Uuid,
            SemanticType::Int,
            SemanticType::String,
        ]))
    );
}

#[test]
fn rejected_outer_kinds_report_one_finding_each_in_source_order() {
    let ast = parse(
        "table BoolKey schemafull { id bool }\n\
         table BytesKey schemafull { id bytes }\n\
         table DatetimeKey schemafull { id datetime }\n\
         table DecimalKey schemafull { id decimal }\n\
         table DurationKey schemafull { id duration }\n\
         table FloatKey schemafull { id float }\n\
         table RangeKey schemafull { id range }\n\
         table NoneKey schemafull { id none }\n\
         table NullKey schemafull { id null }\n\
         table RecordKey schemafull { id record<BoolKey> }\n\
         table SetKey schemafull { id set<int> }\n\
         table UnionKey schemafull { id string | record<BoolKey> }\n\
         table OptionKey schemafull { id option<string> }",
    );
    let analysis = check(&ast);
    assert_eq!(analysis.findings().len(), 13);
    assert!(
        analysis
            .findings()
            .iter()
            .all(|finding| matches!(finding, Finding::InvalidRecordKey { .. }))
    );
    assert!(analysis.into_checked().is_err());
}

#[test]
fn record_is_valid_as_an_ordinary_field_but_invalid_as_id() {
    let ast = parse(
        "table User schemafull {}\n\
         table Data schemafull { id record<User>\nlink record<User> }",
    );
    let analysis = check(&ast);
    assert!(matches!(
        analysis.type_of_field(analysis.fields_of(analysis.tables()[1])[1]),
        Some(TypeResolution::Resolved(SemanticType::Record(
            RecordTargets::Tables(_)
        )))
    ));
    assert!(matches!(
        analysis.findings().last(),
        Some(Finding::InvalidRecordKey { .. })
    ));
    assert!(analysis.into_checked().is_err());
}

#[test]
fn structural_keys_allow_resolved_nested_values_without_widening() {
    let ast = parse(
        "table User schemafull {}\n\
         table Keys schemafull {
           id [datetime, uuid, record<User>]
           nested array<set<int>>
           object_value object
         }",
    );
    let checked = check(&ast).into_checked().expect("structural key is valid");
    let table = checked.table_named("Keys").unwrap();
    let id = checked.fields_of(table)[0];
    let user = checked.table_named("User").unwrap();
    assert_eq!(
        checked.type_of_field(id),
        Some(&SemanticType::Tuple(vec![
            SemanticType::Datetime,
            SemanticType::Uuid,
            SemanticType::Record(RecordTargets::Tables(vec![user])),
        ]))
    );
    assert_eq!(
        checked.type_of_field(checked.fields_of(table)[1]),
        Some(&SemanticType::Array {
            element: Box::new(SemanticType::Set {
                element: Box::new(SemanticType::Int),
                max_distinct: None,
            }),
            exact_length: None,
        })
    );
}

#[test]
fn invalid_or_unknown_field_resolution_does_not_cascade_into_key_findings() {
    let ast = parse(
        "table UnknownKey schemafull { id Unknown }\n\
         table NestedUnknownKey schemafull { id array<Unknown> }",
    );
    let analysis = check(&ast);
    assert_eq!(analysis.findings().len(), 2);
    assert!(
        analysis
            .findings()
            .iter()
            .all(|finding| matches!(finding, Finding::UnknownType { .. }))
    );
    assert!(analysis.into_checked().is_err());
}

#[test]
fn unknown_guid_and_missing_record_target_do_not_cascade_into_key_findings() {
    let ast = parse(
        "table GuidKey schemafull { id guid }\n\
         table MissingTargetKey schemafull { id record<Missing> }",
    );
    let analysis = check(&ast);
    assert_eq!(analysis.findings().len(), 2);
    assert!(matches!(
        &analysis.findings()[0],
        Finding::UnknownType { name, .. } if name == "guid"
    ));
    assert!(matches!(
        &analysis.findings()[1],
        Finding::MissingRecordTarget { name, .. } if name == "Missing"
    ));
    assert!(
        analysis
            .findings()
            .iter()
            .all(|finding| !matches!(finding, Finding::InvalidRecordKey { .. }))
    );
    assert!(analysis.into_checked().is_err());
}

#[test]
fn invalid_keys_keep_precise_utf8_spans_and_fifo_order() {
    let source = "// é\ntable First schemafull { id datetime }\n\
                   table Second schemafull { id set<int> }";
    let ast = aureline_parser::parse_with_source(SourceId::new(41), source)
        .expect("the fixture should parse");
    let analysis = check(&ast);
    assert_eq!(analysis.findings().len(), 2);
    let expected = analysis
        .tables()
        .iter()
        .map(|&table| {
            let field = analysis.fields_of(table)[0];
            ast.field(field).unwrap().source_type().span()
        })
        .collect::<Vec<_>>();
    let actual = analysis
        .findings()
        .iter()
        .map(|finding| match finding {
            Finding::InvalidRecordKey { span, .. } => *span,
            finding => panic!("unexpected finding: {finding:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert!(actual.iter().all(|span| span.source() == SourceId::new(41)));
}

#[test]
fn mixed_declaration_and_type_findings_keep_the_checker_phase_order() {
    let source = "table User schemafull { id bool\n id string\n peer record<Missing>\n unknown Unknown }\n\
                   table User schemaless { id string }";
    let ast = aureline_parser::parse_with_source(SourceId::new(52), source)
        .expect("the fixture should parse");
    let first_table = ast.root().tables()[0];
    let first_fields = ast.table(first_table).unwrap().fields();
    let analysis = check(&ast);

    assert_eq!(analysis.findings().len(), 5);
    assert!(matches!(
        &analysis.findings()[0],
        Finding::DuplicateTable { name, .. } if name == "User"
    ));
    assert!(matches!(
        &analysis.findings()[1],
        Finding::DuplicateField { name, .. } if name == "id"
    ));
    assert!(matches!(
        &analysis.findings()[2],
        Finding::MissingRecordTarget { name, .. } if name == "Missing"
    ));
    assert!(matches!(
        &analysis.findings()[3],
        Finding::UnknownType { name, .. } if name == "Unknown"
    ));
    assert!(matches!(
        &analysis.findings()[4],
        Finding::InvalidRecordKey { field, span }
            if *field == first_fields[0]
                && *span == ast.field(first_fields[0]).unwrap().source_type().span()
                && span.source() == SourceId::new(52)
    ));
    assert_eq!(analysis.findings(), check(&ast).findings());
    assert!(analysis.into_checked().is_err());
}

#[test]
fn absent_or_nonexact_id_fields_have_no_id_contract() {
    let ast = parse(
        "table NoId schemafull { value string }\n\
         table Capitalized schemafull { Id string }",
    );
    let checked = check(&ast)
        .into_checked()
        .expect("ordinary fields are valid");
    assert!(
        checked
            .id_contract(checked.table_named("NoId").unwrap())
            .is_none()
    );
    assert!(
        checked
            .id_contract(checked.table_named("Capitalized").unwrap())
            .is_none()
    );
}

#[test]
fn a_valid_any_id_is_required_even_though_ordinary_any_admits_none() {
    let ast = parse("table T schemafull { id any\npayload any\noptioned option<string> }");
    let checked = check(&ast).into_checked().expect("all fields are valid");
    let table = checked.table_named("T").unwrap();
    let fields = checked.fields_of(table);
    assert_eq!(
        checked.field_presence(fields[0]),
        Some(FieldPresence::Required)
    );
    assert_eq!(
        checked.field_presence(fields[1]),
        Some(FieldPresence::Optional)
    );
    assert_eq!(
        checked.field_presence(fields[2]),
        Some(FieldPresence::Optional)
    );
}
