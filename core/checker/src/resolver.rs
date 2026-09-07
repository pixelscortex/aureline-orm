//! One recursive operation for turning source type syntax into valid contracts.
//!
//! Each field enters this dispatch once. Constructor helpers validate argument
//! roles and return nested value types to this same operation; record targets
//! instead query the complete declaration index. For example,
//! `array<record<User>>` resolves one element type and one table identity. A
//! missing `User` reports at the target name, and its invalid proof propagates
//! through the array while independently resolvable fields continue.
//!
//! Builtins match ASCII case-insensitively as in `SurrealDB`; declaration
//! resolution retains exact spelling. Normalization changes semantic values
//! only, leaving the source AST available with its original order and spans.

use aureline_ast::ast::SourceType;

use crate::{
    Finding, Findings, TypeResolution,
    index::{ResolutionIndex, TableResolution},
    semantic_type::SemanticType,
};

const UNSUPPORTED_SCALARS: &[&str] = &[
    "geometry",
    "point",
    "line",
    "polygon",
    "multipoint",
    "multiline",
    "multipolygon",
    "collection",
    "file",
    "regex",
    "function",
];

pub(crate) fn resolve(
    source_type: &SourceType,
    index: &ResolutionIndex<'_>,
    findings: &mut Findings<Finding>,
) -> TypeResolution<SemanticType> {
    match source_type {
        SourceType::Name(name) => resolve_name(name.name(), name.span(), index, findings),
        SourceType::Application(application) => resolve_application(application, index, findings),
        SourceType::Union(union) => crate::unions::resolve(union, index, findings),
        SourceType::Tuple(tuple) => match resolve_members(tuple.members(), index, findings) {
            TypeResolution::Resolved(members) => {
                TypeResolution::Resolved(SemanticType::Tuple(members))
            }
            TypeResolution::Unknown => TypeResolution::Unknown,
            TypeResolution::Invalid(proof) => TypeResolution::Invalid(proof),
        },
    }
}

fn resolve_application(
    application: &aureline_ast::ast::TypeApplication,
    index: &ResolutionIndex<'_>,
    findings: &mut Findings<Finding>,
) -> TypeResolution<SemanticType> {
    let name = application.name().name();
    let span = application.name().span();
    if name.eq_ignore_ascii_case("option") {
        return crate::unions::resolve_option(application, index, findings);
    }

    if name.eq_ignore_ascii_case("record") {
        return crate::records::resolve(application, index, findings);
    }

    if name.eq_ignore_ascii_case("array") || name.eq_ignore_ascii_case("set") {
        return crate::collections::resolve(application, index, findings);
    }

    if SemanticType::scalar(name).is_some() {
        return invalid(
            findings,
            Finding::WrongArity {
                name: name.to_owned(),
                minimum: 0,
                maximum: 0,
                actual: application.arguments().len(),
                span,
            },
        );
    }
    if is_unsupported(name) {
        return invalid(
            findings,
            Finding::UnsupportedType {
                name: name.to_owned(),
                span,
            },
        );
    }
    if !matches!(index.resolve_table(name), TableResolution::Missing) {
        return invalid(
            findings,
            Finding::BareTableType {
                name: name.to_owned(),
                span,
            },
        );
    }
    invalid(
        findings,
        Finding::UnknownType {
            name: name.to_owned(),
            span,
        },
    )
}

fn resolve_name(
    name: &str,
    span: aureline_ast::source::SourceSpan,
    index: &ResolutionIndex<'_>,
    findings: &mut Findings<Finding>,
) -> TypeResolution<SemanticType> {
    if name.eq_ignore_ascii_case("option") {
        return invalid(
            findings,
            Finding::WrongArity {
                name: name.to_owned(),
                minimum: 1,
                maximum: 1,
                actual: 0,
                span,
            },
        );
    }
    if name.eq_ignore_ascii_case("record") {
        return TypeResolution::Resolved(SemanticType::Record(crate::RecordTargets::Any));
    }
    if name.eq_ignore_ascii_case("array") {
        return TypeResolution::Resolved(SemanticType::Array {
            element: Box::new(SemanticType::Any),
            exact_length: None,
        });
    }
    if name.eq_ignore_ascii_case("set") {
        return TypeResolution::Resolved(SemanticType::Set {
            element: Box::new(SemanticType::Any),
            max_distinct: None,
        });
    }
    if let Some(scalar) = SemanticType::scalar(name) {
        return TypeResolution::Resolved(scalar);
    }
    if is_unsupported(name) {
        return invalid(
            findings,
            Finding::UnsupportedType {
                name: name.to_owned(),
                span,
            },
        );
    }
    if !matches!(index.resolve_table(name), TableResolution::Missing) {
        return invalid(
            findings,
            Finding::BareTableType {
                name: name.to_owned(),
                span,
            },
        );
    }
    invalid(
        findings,
        Finding::UnknownType {
            name: name.to_owned(),
            span,
        },
    )
}

fn invalid(findings: &mut Findings<Finding>, finding: Finding) -> TypeResolution<SemanticType> {
    TypeResolution::Invalid(findings.report(finding))
}

fn is_unsupported(name: &str) -> bool {
    UNSUPPORTED_SCALARS
        .iter()
        .any(|candidate| name.eq_ignore_ascii_case(candidate))
}

/// Resolves an ordered collection of source members through the shared resolver.
///
/// Every member is visited even after an invalid outcome so independent
/// Findings remain visible. The first invalid proof wins the enclosing
/// outcome; an unknown outcome is returned only when no member is invalid.
pub(crate) fn resolve_members(
    members: &[SourceType],
    index: &ResolutionIndex<'_>,
    findings: &mut Findings<Finding>,
) -> TypeResolution<Vec<SemanticType>> {
    let mut resolved = Vec::with_capacity(members.len());
    let mut first_invalid = None;
    let mut has_unknown = false;

    for member in members {
        match resolve(member, index, findings) {
            TypeResolution::Resolved(member) => resolved.push(member),
            TypeResolution::Unknown => has_unknown = true,
            TypeResolution::Invalid(proof) => {
                if first_invalid.is_none() {
                    first_invalid = Some(proof);
                }
            }
        }
    }

    match (first_invalid, has_unknown) {
        (Some(proof), _) => TypeResolution::Invalid(proof),
        (None, true) => TypeResolution::Unknown,
        (None, false) => TypeResolution::Resolved(resolved),
    }
}
