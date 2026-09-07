use aureline_checker::{Finding, check};
use aureline_test::aurl_test;
use serde::Serialize;

#[test]
fn bare_and_constrained_collections_keep_their_distinct_contracts() {
    aurl_test!(
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
         }"
    )
    .checks_as(
        "(CheckedProgram
            (Table Catalog
                (Field everything (Array Any))
                (Field labels (Set Any))
                (Field names (Array String))
                (Field tags (Set Int))
                (Field fixed (Array String 3))
                (Field bounded (Set String 3))
                (Field upper_array (Array String 1))
                (Field upper_set (Set Int))
                (Field nested (Array (Set String 2)))
                (Field empty_set (Set Int 0))))",
    );
}

#[test]
fn constructor_arity_is_reported_before_meaningful_argument_checks() {
    aurl_test!("table T schemafull { value array<Unknown, 1, 2> }")
        .findings("(Findings (WrongArity array 1 2 3) (UnknownType Unknown))");
}

#[test]
fn wrong_argument_roles_are_independent_and_positioned() {
    aurl_test!("table T schemafull { value array<1, string> }").findings(
        "(Findings
            (WrongArgumentRole array 0 ElementType)
            (WrongArgumentRole array 1 CollectionSize))",
    );
}

#[test]
fn sized_collections_accept_zero_and_u64_max_but_reject_overflow() {
    aurl_test!(
        "table T schemafull {\n\
           empty array<string, 0>\n\
           all set<string, 18446744073709551615>\n\
           overflow set<string, 18446744073709551616>\n\
         }"
    )
    .findings("(Findings (InvalidCollectionSize set 18446744073709551616))");
}

#[test]
fn invalid_element_and_size_findings_do_not_create_a_collection_cascade() {
    aurl_test!("table T schemafull { value array<Unknown, 18446744073709551616> }").findings(
        "(Findings (UnknownType Unknown) (InvalidCollectionSize array 18446744073709551616))",
    );
}

#[test]
fn collection_argument_findings_retain_the_offending_source_span() {
    aurl_test!("// é\ntable T schemafull { value set<string, 18446744073709551616> }")
        .with_source_id(aureline_ast::source::SourceId::new(17))
        .reports_with(
            |ast| {
                let analysis = check(ast);
                let Finding::InvalidCollectionSize { span, .. } = &analysis.findings()[0] else {
                    panic!("the fixture should report an invalid collection size");
                };
                SpanView {
                    source: span.source().get(),
                    start: span.range().start().get(),
                    end: span.range().end().get(),
                }
            },
            "(SpanView 17 45 65)",
        );
}

#[derive(Serialize)]
struct SpanView {
    source: u32,
    start: u32,
    end: u32,
}
