//! One recursive operation for turning source type syntax into valid contracts.
//!
//! The current slice owns scalar names. The operation still dispatches over
//! every source shape so later collection, link, union, and tuple work extends
//! one owner rather than introducing another AST walk. Unsupported forms report
//! one root Finding and return invalid recovery proof; they are never widened.
//! Builtins match ASCII case-insensitively as in `SurrealDB`; names used for
//! declaration resolution retain their exact spelling.

use aureline_ast::ast::SourceType;

use crate::{
    Finding, Findings, TypeResolution,
    finding::UnsupportedTypeSyntaxKind,
    index::{ResolutionIndex, TableResolution},
    semantic_type::SemanticType,
};

const UNSUPPORTED_SCALARS: &[&str] = &[
    "array",
    "set",
    "record",
    "option",
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
        SourceType::Union(union) => invalid(
            findings,
            Finding::UnsupportedTypeSyntax {
                kind: UnsupportedTypeSyntaxKind::Union,
                span: union.span(),
            },
        ),
        SourceType::Tuple(tuple) => invalid(
            findings,
            Finding::UnsupportedTypeSyntax {
                kind: UnsupportedTypeSyntaxKind::Tuple,
                span: tuple.span(),
            },
        ),
    }
}

fn resolve_application(
    application: &aureline_ast::ast::TypeApplication,
    index: &ResolutionIndex<'_>,
    findings: &mut Findings<Finding>,
) -> TypeResolution<SemanticType> {
    let name = application.name().name();
    let span = application.name().span();
    if SemanticType::scalar(name).is_some() {
        return invalid(
            findings,
            Finding::WrongArity {
                name: name.to_owned(),
                expected: 0,
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
