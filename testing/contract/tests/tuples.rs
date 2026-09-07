use aureline_ast::ast::Ast;
use aureline_checker::{Finding, SemanticType, TypeResolution, check};

fn parse(source: &str) -> Ast {
    aureline_parser::parse(source).expect("source parses")
}

#[test]
fn tuple_arities_and_member_order_are_preserved() {
    let ast = parse(
        r"table Tuples schemafull {
empty []
one [int]
ordered [string, int, datetime,]
reversed [datetime, int, string]
homogeneous array<int, 1>
}",
    );
    let table = ast.root().tables()[0];
    let fields = ast.table(table).expect("table exists").fields();
    let analysis = check(&ast);
    assert!(matches!(
        analysis.type_of_field(fields[0]),
        Some(TypeResolution::Resolved(SemanticType::Tuple(members)))
            if members.is_empty()
    ));
    assert!(matches!(
        analysis.type_of_field(fields[1]),
        Some(TypeResolution::Resolved(SemanticType::Tuple(members)))
            if members == &vec![SemanticType::Int]
    ));
    assert!(matches!(
        analysis.type_of_field(fields[2]),
        Some(TypeResolution::Resolved(SemanticType::Tuple(members)))
            if members == &vec![
                SemanticType::String,
                SemanticType::Int,
                SemanticType::Datetime,
            ]
    ));
    assert!(matches!(
        analysis.type_of_field(fields[3]),
        Some(TypeResolution::Resolved(SemanticType::Tuple(members)))
            if members == &vec![
                SemanticType::Datetime,
                SemanticType::Int,
                SemanticType::String,
            ]
    ));
    assert!(matches!(
        analysis.type_of_field(fields[4]),
        Some(TypeResolution::Resolved(SemanticType::Array { .. }))
    ));
    assert_ne!(
        analysis.type_of_field(fields[2]),
        analysis.type_of_field(fields[1]),
        "tuples remain distinct from other tuple arities"
    );
    assert_ne!(
        analysis.type_of_field(fields[1]),
        analysis.type_of_field(fields[4]),
        "a one-member tuple remains distinct from an exact-length array"
    );
    assert_ne!(
        analysis.type_of_field(fields[2]),
        analysis.type_of_field(fields[3]),
        "tuple member order is semantic even at equal arity"
    );

    let checked = analysis
        .into_checked()
        .expect("all tuple and array members are valid");
    assert_eq!(checked.format_type(fields[0]).as_deref(), Some("[]"));
    assert_eq!(checked.format_type(fields[1]).as_deref(), Some("[int]"));
}

#[test]
fn nested_tuple_union_record_and_collection_members_keep_structure() {
    let ast = parse(
        r"table User schemafull {}
table Event schemafull {
key [record<User | Event>, array<int, 2>, string | int, [string, uuid]]
}",
    );
    let event = ast.root().tables()[1];
    let field = ast.table(event).expect("Event exists").fields()[0];
    let checked = check(&ast)
        .into_checked()
        .expect("all nested tuple members are valid");
    assert_eq!(
        checked.format_type(field).as_deref(),
        Some("[record<User | Event>, array<int, 2>, int | string, [string, uuid]]")
    );
}

#[test]
fn invalid_tuple_members_report_independently_and_invalidate_the_tuple() {
    let ast = parse(
        r"table User schemafull {
key [MissingOne, geometry, MissingTwo]
}",
    );
    let table = ast.root().tables()[0];
    let field = ast.table(table).expect("table exists").fields()[0];
    let analysis = check(&ast);
    assert!(matches!(
        analysis.type_of_field(field),
        Some(TypeResolution::Invalid(_))
    ));
    assert!(matches!(
        analysis.findings(),
        [
            Finding::UnknownType { name: first, .. },
            Finding::UnsupportedType { name: second, .. },
            Finding::UnknownType { name: third, .. },
        ] if first == "MissingOne" && second == "geometry" && third == "MissingTwo"
    ));
    let Err(rejected) = analysis.into_checked() else {
        panic!("an invalid tuple must reject the checked program");
    };
    assert_eq!(rejected.findings().len(), 3);
    assert!(matches!(
        rejected.type_of_field(field),
        Some(TypeResolution::Invalid(_))
    ));
}

#[test]
fn tuple_display_is_canonical_and_keeps_trailing_comma_out() {
    let ast = parse(
        r"table User schemafull {}
table Event schemafull {
key [datetime, uuid, int, string,]
}",
    );
    let event = ast.root().tables()[1];
    let field = ast.table(event).expect("Event exists").fields()[0];
    let checked = check(&ast)
        .into_checked()
        .expect("the tuple has only supported scalar members");
    assert_eq!(
        checked.format_type(field).as_deref(),
        Some("[datetime, uuid, int, string]")
    );
}
