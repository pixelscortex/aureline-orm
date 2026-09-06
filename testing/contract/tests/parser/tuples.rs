//! Tuple tests pin down recursive composition and separator recovery. Their
//! span checks ensure nesting remains visible to diagnostics and later AST use.

use aureline_ast::ast::SourceType;
use aureline_ast::source::SourceId;
use aureline_parser::SyntaxProblem;
use aureline_test::aurl_test;

use super::support::span;

#[test]
fn fixed_tuples_preserve_arity_order_and_ordinary_field_names() {
    aurl_test!(
        "table item schemafull {\n\
           id [datetime, uuid, int, string]\n\
           _id []\n\
           singleton [int]\n\
           trailing [int,]\n\
         }"
    )
    .parses_as(
        "(SourceFile
            (Table item Schemafull
                (Field id
                    (Tuple
                        (Name datetime)
                        (Name uuid)
                        (Name int)
                        (Name string)))
                (Field _id (Tuple))
                (Field singleton (Tuple (Name int)))
                (Field trailing (Tuple (Name int)))))",
    );
}

#[test]
fn tuples_compose_with_the_complete_type_grammar_without_flattening() {
    aurl_test!(
        "table Shape schemafull {\n\
           nested [[int], record<A | B>]\n\
           argument box<[A | B, record<C>]>\n\
           alternative [A, B] | FutureType\n\
         }"
    )
    .parses_as(
        "(SourceFile
            (Table Shape Schemafull
                (Field nested
                    (Tuple
                        (Tuple (Name int))
                        (Application
                            (Name record)
                            (Type (Union (Name A) (Name B))))))
                (Field argument
                    (Application
                        (Name box)
                        (Type
                            (Tuple
                                (Union (Name A) (Name B))
                                (Application (Name record) (Type (Name C)))))))
                (Field alternative
                    (Union
                        (Tuple (Name A) (Name B))
                        (Name FutureType)))))",
    );
}

#[test]
fn nested_tuples_and_their_members_retain_precise_spans() {
    let source_id = SourceId::new(29);
    let ast =
        aureline_parser::parse_with_source(source_id, "table T schemafull { value [A | B, [C]] }")
            .expect("nested tuple members should parse");
    let table = ast
        .table(ast.root().tables()[0])
        .expect("the table ID belongs to this AST");
    let field = ast
        .field(table.fields()[0])
        .expect("the field ID belongs to this AST");

    let SourceType::Tuple(outer) = field.source_type() else {
        panic!("the field source type is an outer tuple");
    };
    assert_eq!(outer.span(), span(source_id, 27, 39));
    assert_eq!(outer.members().len(), 2);
    assert_eq!(outer.members()[0].span(), span(source_id, 28, 33));
    assert_eq!(outer.members()[1].span(), span(source_id, 35, 38));

    let SourceType::Tuple(inner) = &outer.members()[1] else {
        panic!("the second member is a nested tuple");
    };
    assert_eq!(inner.members()[0].span(), span(source_id, 36, 37));
}

#[test]
fn malformed_tuple_members_and_separators_are_typed_problems() {
    for (source, problem) in [
        (
            "table T schemafull { value [, int] }",
            SyntaxProblem::MissingTupleMember {
                span: span(SourceId::new(0), 28, 29),
            },
        ),
        (
            "table T schemafull { value [int,, string] }",
            SyntaxProblem::MissingTupleMember {
                span: span(SourceId::new(0), 32, 33),
            },
        ),
        (
            "table T schemafull { value [,] }",
            SyntaxProblem::MissingTupleMember {
                span: span(SourceId::new(0), 28, 29),
            },
        ),
        (
            "table T schemafull { value [int string] }",
            SyntaxProblem::MissingTupleSeparator {
                span: span(SourceId::new(0), 32, 38),
            },
        ),
    ] {
        let errors =
            aureline_parser::parse(source).expect_err("tuple members require one comma separator");
        assert_eq!(errors, vec![problem]);
    }
}

#[test]
fn missing_tuple_separators_are_typed_in_recursive_contexts() {
    for (source, adjacent) in [
        ("table T schemafull { value box<[int string]> }", "string"),
        ("table T schemafull { value [[int string]] }", "string"),
        ("table T schemafull { value [record<A> string] }", "string"),
        ("table T schemafull { value [int record<A>] }", "record<A>"),
        ("table T schemafull { value [int string bool] }", "string"),
        ("table T schemafull { value [A | B C] }", "C"),
        ("table T schemafull { value [int [string]] }", "[string]"),
    ] {
        let start = source
            .rfind(adjacent)
            .expect("the malformed tuple contains its second member");
        let start = u32::try_from(start).expect("the short contract source fits text offsets");
        let end =
            start + u32::try_from(adjacent.len()).expect("the short member fits text offsets");
        let errors = aureline_parser::parse(source)
            .expect_err("recursive tuple members require comma separators");
        assert_eq!(
            errors,
            vec![SyntaxProblem::MissingTupleSeparator {
                span: span(SourceId::new(0), start, end),
            }]
        );
    }
}
