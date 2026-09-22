//! Checker contracts for union normalization and field presence.
//!
//! Semantic unions flatten nested members, remove duplicates, and canonicalize
//! order while preserving source syntax and spans. These fixtures also pin the
//! difference between outer optionality and nested `none`/`null` values.

use aureline_ast::{ast::SourceType, ids::FieldId, source::SourceId};
use aureline_checker::{
    ArgumentRole, FieldPresence, Finding, RecordTargets, SemanticType, TypeResolution, check,
};

fn parse(source: &str) -> aureline_ast::ast::Ast {
    aureline_parser::parse(source).expect("fixture should parse")
}

fn field_type<'ast>(
    analysis: &'ast aureline_checker::Analysis<'ast>,
    field: FieldId,
) -> &'ast SemanticType {
    match analysis.type_of_field(field) {
        Some(TypeResolution::Resolved(semantic_type)) => semantic_type,
        outcome => panic!("expected a resolved field type, got {outcome:?}"),
    }
}

#[test]
fn unions_are_flattened_deduplicated_and_order_independent() {
    let first = parse("table T schemafull { value string | int | string }");
    let second = parse("table T schemafull { value int | string }");
    let first_analysis = check(&first);
    let second_analysis = check(&second);
    let first_table = first.root().tables()[0];
    let second_table = second.root().tables()[0];
    let first_value = first.table(first_table).unwrap().fields()[0];
    let second_value = second.table(second_table).unwrap().fields()[0];

    assert_eq!(
        field_type(&first_analysis, first_value),
        field_type(&second_analysis, second_value)
    );
    assert_eq!(
        field_type(&first_analysis, first_value),
        &SemanticType::Union(vec![SemanticType::Int, SemanticType::String])
    );
}

#[test]
fn any_absorbs_other_union_members() {
    let ast = parse("table T schemafull { value string | any | int }");
    let analysis = check(&ast);
    let field = ast.table(ast.root().tables()[0]).unwrap().fields()[0];
    assert_eq!(field_type(&analysis, field), &SemanticType::Any);
}

#[test]
fn none_and_null_remain_distinct_union_members() {
    let ast = parse("table T schemafull { value none | null }");
    let analysis = check(&ast);
    let field = ast.table(ast.root().tables()[0]).unwrap().fields()[0];
    assert_eq!(
        field_type(&analysis, field),
        &SemanticType::Union(vec![SemanticType::None, SemanticType::Null])
    );
}

#[test]
fn option_and_explicit_none_have_equal_semantics() {
    let option = parse("table T schemafull { value option<string> }");
    let explicit = parse("table T schemafull { value string | none }");
    let option_analysis = check(&option);
    let explicit_analysis = check(&explicit);
    let option_field = option.table(option.root().tables()[0]).unwrap().fields()[0];
    let explicit_field = explicit
        .table(explicit.root().tables()[0])
        .unwrap()
        .fields()[0];
    assert_eq!(
        field_type(&option_analysis, option_field),
        field_type(&explicit_analysis, explicit_field)
    );
}

#[test]
fn constrained_records_merge_and_bare_record_absorbs_constraints() {
    let ast = parse(
        "table A schemafull {}\n\
         table B schemafull {}\n\
         table T schemafull { merged record<A> | record<B> | record<A>\n\
                              all record | record<A> }",
    );
    let analysis = check(&ast).into_checked().expect("fixture should check");
    let tables = ast.root().tables();
    let fields = analysis.fields_of(tables[2]);
    assert_eq!(
        analysis.type_of_field(fields[0]),
        Some(&SemanticType::Record(RecordTargets::Tables(vec![
            tables[0], tables[1]
        ])))
    );
    assert_eq!(
        analysis.type_of_field(fields[1]),
        Some(&SemanticType::Record(RecordTargets::Any))
    );
}

#[test]
fn field_presence_distinguishes_outer_none_from_nested_none() {
    let ast = parse(
        "table T schemafull {\n\
           required string\n\
           nullable string | null\n\
           optional option<string>\n\
           both option<string | null>\n\
           dynamic any\n\
           nested array<option<string>>\n\
         }",
    );
    let checked = check(&ast).into_checked().expect("fixture should check");
    let fields = checked.fields_of(checked.tables()[0]);
    assert_eq!(
        checked.field_presence(fields[0]),
        Some(FieldPresence::Required)
    );
    assert_eq!(
        checked.field_presence(fields[1]),
        Some(FieldPresence::Required)
    );
    assert_eq!(
        checked.field_presence(fields[2]),
        Some(FieldPresence::Optional)
    );
    assert_eq!(
        checked.field_presence(fields[3]),
        Some(FieldPresence::Optional)
    );
    assert_eq!(
        checked.field_presence(fields[4]),
        Some(FieldPresence::Optional)
    );
    assert_eq!(
        checked.field_presence(fields[5]),
        Some(FieldPresence::Required)
    );
}

#[test]
fn invalid_any_union_member_remains_invalid_and_keeps_independent_findings() {
    let ast = parse("table T schemafull { value any | Bad | geometry }");
    let analysis = check(&ast);
    let field = ast.table(ast.root().tables()[0]).unwrap().fields()[0];
    assert!(matches!(
        analysis.type_of_field(field),
        Some(TypeResolution::Invalid(_))
    ));
    assert!(matches!(
        analysis.findings(),
        [
            Finding::UnknownType { name, .. },
            Finding::UnsupportedType { name: geometry, .. }
        ] if name == "Bad" && geometry == "geometry"
    ));
}

#[test]
fn option_reports_arity_before_an_invalid_member_and_validates_argument_role() {
    let ast = parse("table T schemafull { value option<Bad, string> }");
    let analysis = check(&ast);
    assert!(matches!(
        analysis.findings(),
        [
            Finding::WrongArity { name, minimum: 1, maximum: 1, actual: 2, .. },
            Finding::UnknownType { name: bad, .. }
        ] if name == "option" && bad == "Bad"
    ));

    let ast = parse("table T schemafull { value option<1> }");
    let analysis = check(&ast);
    assert!(matches!(
        analysis.findings(),
        [Finding::WrongArgumentRole {
            name,
            position: 0,
            expected: ArgumentRole::OptionType,
            ..
        }] if name == "option"
    ));

    let ast = parse("table T schemafull { value option }");
    let analysis = check(&ast);
    assert!(matches!(
        analysis.findings(),
        [Finding::WrongArity {
            name,
            minimum: 1,
            maximum: 1,
            actual: 0,
            ..
        }] if name == "option"
    ));
}

#[test]
fn union_source_order_and_spans_survive_semantic_normalization() {
    let ast = aureline_parser::parse_with_source(
        SourceId::new(9),
        "table T schemafull { value int | Unknown | string }",
    )
    .expect("fixture should parse");
    let analysis = check(&ast);
    let field = ast.table(ast.root().tables()[0]).unwrap().fields()[0];
    let SourceType::Union(union) = ast.field(field).unwrap().source_type() else {
        panic!("fixture should retain source union syntax");
    };
    let SourceType::Name(first) = &union.members()[0] else {
        panic!("first source member should be int");
    };
    assert_eq!(first.name(), "int");
    assert_eq!(first.span().source(), SourceId::new(9));
    assert!(matches!(
        &analysis.findings()[0],
        Finding::UnknownType { name, span } if name == "Unknown" && span.source() == SourceId::new(9)
    ));
}

#[test]
fn option_constructor_is_case_insensitive() {
    let ast = parse("table T schemafull { value OpTiOn<STRING> }");
    let checked = check(&ast).into_checked().expect("option should resolve");
    let field = checked.fields_of(checked.tables()[0])[0];
    assert_eq!(checked.field_presence(field), Some(FieldPresence::Optional));
}

#[test]
fn nested_option_unions_flatten_without_changing_valid_source_syntax() {
    let ast = parse("table T schemafull { value option<STRING | option<int> | string> }");
    let table = ast.root().tables()[0];
    let field = ast.table(table).unwrap().fields()[0];
    let before = ast.field(field).unwrap().source_type();
    let checked = check(&ast).into_checked().unwrap();
    assert_eq!(
        checked.type_of_field(field),
        Some(&SemanticType::Union(vec![
            SemanticType::Int,
            SemanticType::String,
            SemanticType::None,
        ]))
    );
    let SourceType::Application(application) = ast.field(field).unwrap().source_type() else {
        panic!("option syntax remains an application")
    };
    let aureline_ast::ast::TypeArgument::Type(SourceType::Union(union)) =
        &application.arguments()[0]
    else {
        panic!("source union remains nested")
    };
    let SourceType::Name(name) = &union.members()[0] else {
        panic!("original first member remains a name")
    };
    assert_eq!(name.name(), "STRING");
    assert_eq!(before, ast.field(field).unwrap().source_type());
}
