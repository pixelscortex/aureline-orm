//! Checker contracts for record targets and identity-preserving links.
//!
//! Record types resolve to concrete table sets, all tables, or invalid/unknown
//! outcomes. The cases below pin forward and self links, nested constraints,
//! schema modes, and the source spans attached to target failures.

use aureline_ast::{
    ast::{Ast, SourceType, TypeArgument},
    source::{SourceId, SourceSpan},
};
use aureline_checker::{Finding, RecordTargets, SemanticType, TypeResolution, check};

fn parse(source: &str) -> Ast {
    aureline_parser::parse(source).expect("the record fixture should parse")
}

fn parse_with_source(source_id: SourceId, source: &str) -> Ast {
    aureline_parser::parse_with_source(source_id, source).expect("the record fixture should parse")
}

fn first_target_span(source_type: &SourceType) -> SourceSpan {
    match source_type {
        SourceType::Name(name) => name.span(),
        SourceType::Union(union) => first_target_span(
            union
                .members()
                .first()
                .expect("a parser-produced union has a first member"),
        ),
        SourceType::Application(_) | SourceType::Tuple(_) => {
            panic!("record target is a name or union")
        }
    }
}

#[test]
fn links_resolve_forward_self_and_across_schema_modes() {
    let ast = parse(
        "table A schemafull { self_link record<A>\n peer record<B> }\n\
                    table B schemaless { peer record<A> }",
    );
    let checked = check(&ast)
        .into_checked()
        .expect("forward, self, and cross-mode links resolve");
    let a = checked.tables()[0];
    let b = checked.tables()[1];
    let a_fields = checked.fields_of(a);
    let b_fields = checked.fields_of(b);

    assert!(matches!(
        checked.type_of_field(a_fields[0]),
        Some(SemanticType::Record(RecordTargets::Tables(target))) if target == &[a]
    ));
    assert!(matches!(
        checked.type_of_field(a_fields[1]),
        Some(SemanticType::Record(RecordTargets::Tables(target))) if target == &[b]
    ));
    assert!(matches!(
        checked.type_of_field(b_fields[0]),
        Some(SemanticType::Record(RecordTargets::Tables(target))) if target == &[a]
    ));
}

#[test]
fn nested_record_links_render_as_identity_constraints() {
    let ast = parse(
        "table any schemafull {}\n\
         table A schemafull {}\n\
         table B schemaless {}\n\
         table Links schemafull {\n\
           nested array<ReCoRd<A | B>>\n\
           broad ReCoRd\n\
           literal record<any>\n\
         }",
    );
    let checked = check(&ast)
        .into_checked()
        .expect("nested and bare record links resolve");
    let links = checked.tables()[3];
    let fields = checked.fields_of(links);

    assert!(matches!(
        checked.type_of_field(fields[0]),
        Some(SemanticType::Array { element, exact_length: None })
            if matches!(element.as_ref(), SemanticType::Record(RecordTargets::Tables(targets))
                if targets == &[checked.tables()[1], checked.tables()[2]])
    ));
    assert_eq!(
        checked.type_of_field(fields[1]),
        Some(&SemanticType::Record(RecordTargets::Any))
    );
    assert!(matches!(
        checked.type_of_field(fields[2]),
        Some(SemanticType::Record(RecordTargets::Tables(targets)))
            if targets == &[checked.tables()[0]]
    ));
}

#[test]
fn constrained_targets_are_sorted_deduplicated_and_stay_constrained() {
    let ast = parse(
        "table A schemafull {}\n\
         table B schemafull {}\n\
         table Links schemafull { all record<B | A | B | Links> }",
    );
    let checked = check(&ast)
        .into_checked()
        .expect("all constrained targets resolve");
    let links = checked.tables()[2];
    let field = checked.fields_of(links)[0];

    assert!(matches!(
        checked.type_of_field(field),
        Some(SemanticType::Record(RecordTargets::Tables(targets)))
            if targets == checked.tables()
    ));
}

#[test]
fn every_missing_reference_is_reported_and_invalidates_the_field() {
    let ast = parse(
        "table A schemafull { first record<Missing>\n second record<Missing | AlsoMissing> }",
    );
    let analysis = check(&ast);
    let findings = analysis.findings();

    assert_eq!(findings.len(), 3);
    assert!(matches!(
        &findings[0],
        Finding::MissingRecordTarget { name, .. } if name == "Missing"
    ));
    assert!(matches!(
        &findings[1],
        Finding::MissingRecordTarget { name, .. } if name == "Missing"
    ));
    assert!(matches!(
        &findings[2],
        Finding::MissingRecordTarget { name, .. } if name == "AlsoMissing"
    ));
    let fields = analysis.fields_of(analysis.tables()[0]);
    assert!(matches!(
        analysis.type_of_field(fields[0]),
        Some(TypeResolution::Invalid(_))
    ));
    assert!(matches!(
        analysis.type_of_field(fields[1]),
        Some(TypeResolution::Invalid(_))
    ));
    assert!(analysis.into_checked().is_err());
}

#[test]
fn malformed_record_arguments_report_arity_roles_and_each_reference() {
    let ast = parse(
        "table A schemafull {\n\
           wrong record<A, Missing>\n\
           numeric ReCoRd<1>\n\
           malformed ReCoRd<array<A>>\n\
           repeated record<Missing | Missing>\n\
         }",
    );
    let analysis = check(&ast);
    let findings = analysis.findings();

    assert!(matches!(
        &findings[0],
        Finding::WrongArity {
            name,
            minimum: 1,
            maximum: 1,
            actual: 2,
            ..
        } if name == "record"
    ));
    assert!(matches!(
        &findings[1],
        Finding::WrongArgumentRole { name, position: 0, .. } if name == "ReCoRd"
    ));
    assert!(matches!(
        &findings[2],
        Finding::WrongArgumentRole { name, position: 0, .. } if name == "ReCoRd"
    ));
    assert!(matches!(
        &findings[3],
        Finding::MissingRecordTarget { name, .. } if name == "Missing"
    ));
    assert!(matches!(
        &findings[4],
        Finding::MissingRecordTarget { name, .. } if name == "Missing"
    ));
    assert_eq!(findings.len(), 5);

    let fields = analysis.fields_of(analysis.tables()[0]);
    for field in fields {
        assert!(matches!(
            analysis.type_of_field(*field),
            Some(TypeResolution::Invalid(_))
        ));
    }
    assert!(analysis.into_checked().is_err());
}

#[test]
fn ambiguous_references_retain_all_candidates_at_each_site() {
    let ast = parse_with_source(
        SourceId::new(17),
        "table Duplicate schemafull {}\n\
         table Duplicate schemaless {}\n\
         table Links schemafull { first record<Duplicate>\n second record<Duplicate | Missing> }",
    );
    let analysis = check(&ast);
    let tables = analysis.tables();
    let links = tables[2];
    let fields = analysis.fields_of(links);
    let target_spans = fields
        .iter()
        .map(|field| {
            let source_type = ast.field(*field).expect("field exists").source_type();
            let SourceType::Application(application) = source_type else {
                panic!("record field has an application source type");
            };
            let TypeArgument::Type(target) = &application.arguments()[0] else {
                panic!("record target is a type argument");
            };
            first_target_span(target)
        })
        .collect::<Vec<_>>();
    let ambiguous = analysis
        .findings()
        .iter()
        .filter_map(|finding| match finding {
            Finding::AmbiguousRecordTarget {
                candidates, span, ..
            } => Some((candidates, *span)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(analysis.findings().len(), 4);
    assert_eq!(ambiguous.len(), 2);
    assert!(
        ambiguous
            .iter()
            .all(|(candidates, _)| candidates.as_slice() == [tables[0], tables[1]])
    );
    assert_eq!(
        ambiguous.iter().map(|(_, span)| *span).collect::<Vec<_>>(),
        target_spans
    );
    assert!(matches!(
        analysis.type_of_field(fields[0]),
        Some(TypeResolution::Invalid(_))
    ));
    assert!(matches!(
        analysis.type_of_field(fields[1]),
        Some(TypeResolution::Invalid(_))
    ));
    assert!(analysis.into_checked().is_err());
}

#[test]
fn missing_target_finding_uses_the_target_name_span() {
    let ast = parse_with_source(
        SourceId::new(11),
        "table A schemafull { peer record<Missing> }",
    );
    let table = ast.root().tables()[0];
    let field = ast.table(table).expect("table exists").fields()[0];
    let source_type = ast.field(field).expect("field exists").source_type();
    let SourceType::Application(application) = source_type else {
        panic!("the field source type is an application");
    };
    let TypeArgument::Type(SourceType::Name(target)) = &application.arguments()[0] else {
        panic!("the record target is a name");
    };
    let analysis = check(&ast);
    assert!(matches!(
        &analysis.findings()[0],
        Finding::MissingRecordTarget { name, span }
            if name == "Missing" && *span == target.span()
    ));
}
