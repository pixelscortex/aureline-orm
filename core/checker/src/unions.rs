//! Resolution and normalization of union and `option` type expressions.
//!
//! Source members are resolved in their written order so every independent
//! member problem is retained in FIFO order. Only after all members have been
//! visited does this module flatten nested unions, remove duplicate contracts,
//! and merge constrained record contracts. An invalid member therefore makes
//! the whole union invalid without suppressing a later independent Finding.

use aureline_ast::{
    arena::ArenaId,
    ast::{TypeApplication, TypeArgument, TypeUnion},
};

use crate::{
    Finding, Findings, Reported, TypeResolution,
    finding::ArgumentRole,
    index::ResolutionIndex,
    resolver,
    semantic_type::{RecordTargets, SemanticType},
};

/// Resolves a source union with one recursive resolver shared by all type forms.
pub(crate) fn resolve(
    union: &TypeUnion,
    index: &ResolutionIndex<'_>,
    findings: &mut Findings<Finding>,
) -> TypeResolution<SemanticType> {
    let mut members = Vec::with_capacity(union.members().len());
    let mut first_invalid = None;
    let mut has_unknown = false;

    for member in union.members() {
        match resolver::resolve(member, index, findings) {
            TypeResolution::Resolved(member) => members.push(member),
            TypeResolution::Unknown => has_unknown = true,
            TypeResolution::Invalid(proof) => remember(&mut first_invalid, proof),
        }
    }

    if let Some(proof) = first_invalid {
        TypeResolution::Invalid(proof)
    } else if has_unknown {
        TypeResolution::Unknown
    } else {
        TypeResolution::Resolved(normalize(members))
    }
}

/// Resolves `option<T>` as the normalized union `T | none`.
pub(crate) fn resolve_option(
    application: &TypeApplication,
    index: &ResolutionIndex<'_>,
    findings: &mut Findings<Finding>,
) -> TypeResolution<SemanticType> {
    let name = application.name().name();
    let arguments = application.arguments();
    let mut first_invalid = None;

    if arguments.len() != 1 {
        first_invalid = Some(findings.report(Finding::WrongArity {
            name: name.to_owned(),
            minimum: 1,
            maximum: 1,
            actual: arguments.len(),
            span: application.name().span(),
        }));
    }

    let Some(argument) = arguments.first() else {
        return TypeResolution::Invalid(
            first_invalid.expect("an empty option application reports wrong arity"),
        );
    };

    let member = match argument {
        TypeArgument::Type(source_type) => resolver::resolve(source_type, index, findings),
        TypeArgument::Integer(integer) => {
            TypeResolution::Invalid(findings.report(Finding::WrongArgumentRole {
                name: name.to_owned(),
                position: 0,
                expected: ArgumentRole::OptionType,
                span: integer.span(),
            }))
        }
    };

    match (first_invalid, member) {
        (Some(proof), _) | (None, TypeResolution::Invalid(proof)) => TypeResolution::Invalid(proof),
        (None, TypeResolution::Resolved(member)) => {
            TypeResolution::Resolved(normalize(vec![member, SemanticType::None]))
        }
        (None, TypeResolution::Unknown) => TypeResolution::Unknown,
    }
}

/// Produces canonical union semantics without changing source syntax.
pub(crate) fn normalize(members: Vec<SemanticType>) -> SemanticType {
    let mut flattened = Vec::new();
    for member in members {
        flatten(member, &mut flattened);
    }

    if flattened
        .iter()
        .any(|member| matches!(member, SemanticType::Any))
    {
        return SemanticType::Any;
    }

    let mut records = Vec::new();
    let mut non_records = Vec::new();
    for member in flattened {
        match member {
            SemanticType::Record(targets) => records.push(targets),
            member => non_records.push(member),
        }
    }

    if !records.is_empty() {
        let merged = merge_records(records);
        non_records.push(SemanticType::Record(merged));
    }

    non_records.sort_unstable();
    non_records.dedup();
    if non_records.len() == 1 {
        return non_records
            .pop()
            .expect("the union has one normalized member");
    }
    SemanticType::Union(non_records)
}

fn flatten(member: SemanticType, output: &mut Vec<SemanticType>) {
    match member {
        SemanticType::Union(members) => {
            for member in members {
                flatten(member, output);
            }
        }
        member => output.push(member),
    }
}

fn merge_records(records: Vec<RecordTargets>) -> RecordTargets {
    let mut tables = Vec::new();
    for record in records {
        match record {
            RecordTargets::Any => return RecordTargets::Any,
            RecordTargets::Tables(mut record_tables) => tables.append(&mut record_tables),
        }
    }
    tables.sort_unstable_by_key(|table| table.into_index());
    tables.dedup();
    RecordTargets::Tables(tables)
}

fn remember(first: &mut Option<Reported>, proof: Reported) {
    if first.is_none() {
        *first = Some(proof);
    }
}
