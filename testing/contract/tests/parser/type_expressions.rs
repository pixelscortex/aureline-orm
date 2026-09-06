//! Type expressions stay meaning-free at this parser boundary: nesting,
//! argument order, integer spelling, and source spans are preserved for later
//! stages rather than resolved here.

use aureline_ast::ast::{SourceType, TypeArgument};
use aureline_ast::source::SourceId;
use aureline_test::aurl_test;

use super::support::span;

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
fn unions_preserve_member_order_without_resolving_names() {
    aurl_test!("table Event schemafull { payload string | int | FutureType }").parses_as(
        "(SourceFile
            (Table Event Schemafull
                (Field payload
                    (Union
                        (Name string)
                        (Name int)
                        (Name FutureType)))))",
    );
}

#[test]
fn unions_compose_recursively_without_flattening() {
    aurl_test!(
        "table Link schemafull {\n\
           owner record<A | B>\n\
           choice box<A | B> | C\n\
         }"
    )
    .parses_as(
        "(SourceFile
            (Table Link Schemafull
                (Field owner
                    (Application
                        (Name record)
                        (Type
                            (Union (Name A) (Name B)))))
                (Field choice
                    (Union
                        (Application
                            (Name box)
                            (Type
                                (Union (Name A) (Name B))))
                        (Name C)))))",
    );
}

#[test]
fn nested_unions_and_their_members_retain_precise_spans() {
    let source_id = SourceId::new(23);
    let ast = aureline_parser::parse_with_source(
        source_id,
        "table T schemafull { value box<A | B> | C }",
    )
    .expect("nested unions should parse");
    let table = ast
        .table(ast.root().tables()[0])
        .expect("the table ID belongs to this AST");
    let field = ast
        .field(table.fields()[0])
        .expect("the field ID belongs to this AST");

    let SourceType::Union(outer) = field.source_type() else {
        panic!("the field source type is an outer union");
    };
    assert_eq!(outer.span(), span(source_id, 27, 41));
    assert_eq!(outer.members().len(), 2);
    assert_eq!(outer.members()[0].span(), span(source_id, 27, 37));
    assert_eq!(outer.members()[1].span(), span(source_id, 40, 41));

    let SourceType::Application(box_type) = &outer.members()[0] else {
        panic!("the first outer member is an application");
    };
    let TypeArgument::Type(SourceType::Union(inner)) = &box_type.arguments()[0] else {
        panic!("the application argument is an inner union");
    };
    assert_eq!(inner.span(), span(source_id, 31, 36));
    assert_eq!(inner.members()[0].span(), span(source_id, 31, 32));
    assert_eq!(inner.members()[1].span(), span(source_id, 35, 36));
}

#[test]
fn union_whitespace_does_not_change_the_logical_contract() {
    aurl_test!(
        "table Choice schemafull {\n\
           compact A|B\n\
           asymmetric A |B\n\
           tabs A\t|\tB\n\
         }"
    )
    .parses_as(
        "(SourceFile
            (Table Choice Schemafull
                (Field compact (Union (Name A) (Name B)))
                (Field asymmetric (Union (Name A) (Name B)))
                (Field tabs (Union (Name A) (Name B)))))",
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
fn deeply_nested_type_applications_complete_through_the_public_parser() {
    let mut source_type = String::from("string");
    for _ in 0..128 {
        source_type = format!("array<{source_type}>");
    }
    let source = format!("table T schemafull {{ value {source_type} }}");

    aureline_parser::parse(&source).expect("deeply nested applications should complete");
}
