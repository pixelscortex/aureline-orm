//! Record-link resolution for the table type algebra.
//!
//! The helper resolves only the target syntax of a `record` application. It
//! queries the already-collected [`ResolutionIndex`] for each name, retaining
//! table identities instead of copying names into the resulting contract. A
//! missing or ambiguous name reports at its own source span; the enclosing
//! record then carries the existing [`crate::Reported`] proof as invalid recovery.
//!
//! Target resolution does not follow fields on the target tables. For example,
//! `table A schemafull { peer record<B> } table B schemaless { peer record<A> }`
//! resolves two IDs without recursively entering either declaration, so forward
//! references and cycles have the same implementation as ordinary links.

use aureline_ast::{
    arena::ArenaId,
    ast::{SourceType, TypeApplication, TypeArgument},
};

use crate::{
    Finding, Findings, TypeResolution,
    finding::ArgumentRole,
    index::{ResolutionIndex, TableResolution},
    semantic_type::{RecordTargets, SemanticType},
};

/// Resolves one case-insensitive `record` constructor application.
///
/// The constructor accepts exactly one type argument. When arity is wrong,
/// the first supplied argument is still checked so a meaningful missing or
/// ambiguous target remains visible; the arity Finding itself keeps the whole
/// application invalid. An integer argument, or a nested type form other than
/// a name or union, is a role error at that argument's span.
pub(crate) fn resolve(
    application: &TypeApplication,
    index: &ResolutionIndex<'_>,
    findings: &mut Findings<Finding>,
) -> TypeResolution<SemanticType> {
    let name = application.name().name();
    let mut invalid_proof = None;

    if application.arguments().len() != 1 {
        invalid_proof = Some(findings.report(Finding::WrongArity {
            name: name.to_owned(),
            minimum: 1,
            maximum: 1,
            actual: application.arguments().len(),
            span: application.name().span(),
        }));
    }

    // With too many arguments, the first argument is the only one that can be
    // the record target. The arity Finding accounts for the extra arguments;
    // walking them as targets would manufacture unrelated diagnostics.
    let Some(argument) = application.arguments().first() else {
        return TypeResolution::Invalid(
            invalid_proof.expect("an empty record application reports wrong arity"),
        );
    };

    let target = match argument {
        TypeArgument::Type(source_type) => resolve_target(source_type, name, index, findings),
        TypeArgument::Integer(integer) => invalid(
            findings,
            Finding::WrongArgumentRole {
                name: name.to_owned(),
                position: 0,
                expected: ArgumentRole::RecordTarget,
                span: integer.span(),
            },
        ),
    };

    if let Some(proof) = invalid_proof {
        return TypeResolution::Invalid(proof);
    }

    match target {
        TypeResolution::Resolved(targets) => {
            TypeResolution::Resolved(SemanticType::Record(RecordTargets::Tables(targets)))
        }
        TypeResolution::Invalid(proof) => TypeResolution::Invalid(proof),
        TypeResolution::Unknown => TypeResolution::Unknown,
    }
}

/// Resolves the source target expression of a `record` application.
///
/// A name must resolve to one declared table; a union resolves each member,
/// then sorts and deduplicates the resulting compilation-local IDs. Missing or
/// ambiguous names report their own Finding, while nested applications and
/// tuples are invalid argument roles. The returned IDs are identities only;
/// this helper never recursively checks the target tables' fields.
fn resolve_target(
    source_type: &SourceType,
    constructor_name: &str,
    index: &ResolutionIndex<'_>,
    findings: &mut Findings<Finding>,
) -> TypeResolution<Vec<aureline_ast::ids::TableId>> {
    match source_type {
        SourceType::Name(name) => match index.resolve_table(name.name()) {
            TableResolution::Missing => invalid(
                findings,
                Finding::MissingRecordTarget {
                    name: name.name().to_owned(),
                    span: name.span(),
                },
            ),
            TableResolution::Unique(table) => TypeResolution::Resolved(vec![table]),
            TableResolution::Ambiguous(candidates) => invalid(
                findings,
                Finding::AmbiguousRecordTarget {
                    name: name.name().to_owned(),
                    span: name.span(),
                    candidates,
                },
            ),
        },
        SourceType::Union(union) => {
            let mut targets = Vec::new();
            let mut invalid_proof = None;

            let mut has_unknown = false;
            for member in union.members() {
                match resolve_target(member, constructor_name, index, findings) {
                    TypeResolution::Resolved(mut member_targets) => {
                        targets.append(&mut member_targets);
                    }
                    TypeResolution::Invalid(proof) => {
                        invalid_proof.get_or_insert(proof);
                    }
                    TypeResolution::Unknown => has_unknown = true,
                }
            }

            if let Some(proof) = invalid_proof {
                TypeResolution::Invalid(proof)
            } else if has_unknown {
                TypeResolution::Unknown
            } else {
                // Table IDs are compilation-local arena identities and have no
                // semantic ordering of their own. Their allocation index gives
                // a stable canonical order for equality and downstream reads.
                targets.sort_unstable_by_key(|table| table.into_index());
                targets.dedup();
                TypeResolution::Resolved(targets)
            }
        }
        SourceType::Application(_) | SourceType::Tuple(_) => invalid(
            findings,
            Finding::WrongArgumentRole {
                name: constructor_name.to_owned(),
                position: 0,
                expected: ArgumentRole::RecordTarget,
                span: source_type.span(),
            },
        ),
    }
}

fn invalid<T>(findings: &mut Findings<Finding>, finding: Finding) -> TypeResolution<T> {
    TypeResolution::Invalid(findings.report(finding))
}
