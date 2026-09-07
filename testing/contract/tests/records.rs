use aureline_ast::{
    ast::{SourceType, TypeArgument},
    source::{SourceId, SourceSpan},
};
use aureline_checker::{Finding, RecordTargets, SemanticType, TypeResolution, check};
use aureline_test::aurl_test;

#[test]
fn links_resolve_forward_self_and_across_schema_modes() {
    aurl_test!(
        "table A schemafull { self_link record<A>\n peer record<B> }\n\
         table B schemaless { peer record<A> }"
    )
    .reports_with(
        |ast| {
            let checked = check(ast).into_checked().expect("all links resolve");
            let a = checked.tables()[0];
            let b = checked.tables()[1];
            let a_fields = checked.fields_of(a);
            let b_fields = checked.fields_of(b);
            matches!(
                (
                    checked.type_of_field(a_fields[0]),
                    checked.type_of_field(a_fields[1]),
                    checked.type_of_field(b_fields[0]),
                ),
                (
                    Some(SemanticType::Record(RecordTargets::Tables(self_target))),
                    Some(SemanticType::Record(RecordTargets::Tables(peer_target))),
                    Some(SemanticType::Record(RecordTargets::Tables(back_target))),
                ) if self_target == &[a]
                    && peer_target == &[b]
                    && back_target == &[a]
            )
        },
        "true",
    );
}

#[test]
fn nested_record_links_render_as_identity_constraints() {
    aurl_test!(
        "table any schemafull {}\n\
         table A schemafull {}\n\
         table B schemaless {}\n\
         table Links schemafull {\n\
           nested array<ReCoRd<A | B>>\n\
           broad ReCoRd\n\
           literal record<any>\n\
         }"
    )
    .checks_as(
        "(CheckedProgram
            (Table any)
            (Table A)
            (Table B)
            (Table Links
                (Field nested (Array (Record A B)))
                (Field broad Record)
                (Field literal (Record any))))",
    );
}

#[test]
fn constrained_targets_are_sorted_deduplicated_and_stay_constrained() {
    aurl_test!(
        "table A schemafull {}\n\
         table B schemafull {}\n\
         table Links schemafull { all record<B | A | B | Links> }"
    )
    .reports_with(
        |ast| {
            let checked = check(ast).into_checked().expect("all targets resolve");
            let links = checked.tables()[2];
            let field = checked.fields_of(links)[0];
            matches!(
                checked.type_of_field(field),
                Some(SemanticType::Record(RecordTargets::Tables(targets)))
                    if targets == &[
                        checked.tables()[0],
                        checked.tables()[1],
                        checked.tables()[2],
                    ]
            )
        },
        "true",
    );

    aurl_test!(
        "table A schemafull {}\n\
         table B schemafull {}\n\
         table Links schemafull { any_record record\n all_records record<A | B> }"
    )
    .reports_with(
        |ast| {
            let checked = check(ast).into_checked().expect("all targets resolve");
            let links = checked.tables()[2];
            let fields = checked.fields_of(links);
            matches!(
                (
                    checked.type_of_field(fields[0]),
                    checked.type_of_field(fields[1]),
                ),
                (
                    Some(SemanticType::Record(RecordTargets::Any)),
                    Some(SemanticType::Record(RecordTargets::Tables(targets))),
                ) if targets == &[checked.tables()[0], checked.tables()[1]]
            )
        },
        "true",
    );
}

#[test]
fn every_missing_reference_is_reported_and_invalidates_the_field() {
    aurl_test!(
        "table A schemafull { first record<Missing>\n second record<Missing | AlsoMissing> }"
    )
    .findings(
        "(Findings (MissingRecordTarget Missing) (MissingRecordTarget Missing) (MissingRecordTarget AlsoMissing))",
    );
}

#[test]
fn malformed_record_arguments_report_arity_roles_and_each_reference() {
    aurl_test!(
        "table A schemafull {\n\
           wrong record<A, Missing>\n\
           numeric ReCoRd<1>\n\
           malformed ReCoRd<array<A>>\n\
           repeated record<Missing | Missing>\n\
         }"
    )
    .findings(
        "(Findings
            (WrongArity record 1 1 2)
            (WrongArgumentRole ReCoRd 0 RecordTarget)
            (WrongArgumentRole ReCoRd 0 RecordTarget)
            (MissingRecordTarget Missing)
            (MissingRecordTarget Missing))",
    );
}

#[test]
fn ambiguous_references_retain_all_candidates_at_each_site() {
    aurl_test!(
        "table Duplicate schemafull {}\n\
         table Duplicate schemaless {}\n\
         table Links schemafull { first record<Duplicate>\n second record<Duplicate | Missing> }"
    )
    .findings(
        "(Findings (DuplicateTable Duplicate) (AmbiguousRecordTarget Duplicate) (AmbiguousRecordTarget Duplicate) (MissingRecordTarget Missing))",
    );

    aurl_test!(
        "table Duplicate schemafull {}\n\
         table Duplicate schemaless {}\n\
         table Links schemafull { first record<Duplicate>\n second record<Duplicate | Missing> }"
    )
    .with_source_id(SourceId::new(17))
    .reports_with(
        |ast| {
            let analysis = check(ast);
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
                    Finding::AmbiguousRecordTarget { candidates, .. } => Some(candidates),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let ambiguous_spans = analysis
                .findings()
                .iter()
                .filter_map(|finding| match finding {
                    Finding::AmbiguousRecordTarget { span, .. } => Some(*span),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let candidates_ok = ambiguous
                .iter()
                .all(|candidates| candidates.as_slice() == [tables[0], tables[1]]);
            let findings_ok = matches!(
                (
                    analysis.type_of_field(fields[0]),
                    analysis.type_of_field(fields[1]),
                ),
                (
                    Some(TypeResolution::Invalid(_)),
                    Some(TypeResolution::Invalid(_)),
                )
            );
            let checked_refused = analysis.into_checked().is_err();
            findings_ok && candidates_ok && ambiguous_spans == target_spans && checked_refused
        },
        "true",
    );
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
fn missing_target_finding_uses_the_target_name_span() {
    aurl_test!("table A schemafull { peer record<Missing> }")
        .with_source_id(SourceId::new(11))
        .reports_with(
            |ast| {
                let table = ast.root().tables()[0];
                let field = ast.table(table).expect("table exists").fields()[0];
                let source_type = ast.field(field).expect("field exists").source_type();
                let SourceType::Application(application) = source_type else {
                    return false;
                };
                let TypeArgument::Type(SourceType::Name(target)) = &application.arguments()[0]
                else {
                    return false;
                };
                let analysis = check(ast);
                let findings = analysis.findings();
                matches!(
                    &findings[0],
                    Finding::MissingRecordTarget { name, span }
                        if name == "Missing" && *span == target.span()
                )
            },
            "true",
        );
}
