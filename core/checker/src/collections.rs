//! Recursive resolution of `array` and `set` type applications.
//!
//! The resolver owns the recursive walk of source type syntax. This helper
//! validates the collection constructor's argument roles and size while
//! delegating its element type back to that same resolver. A collection with
//! an invalid child is therefore represented by the child's `Reported` proof,
//! without adding an enclosing cascade.

use aureline_ast::ast::{TypeApplication, TypeArgument};

use crate::{
    Finding, Findings, Reported, TypeResolution, index::ResolutionIndex, resolver,
    semantic_type::SemanticType,
};

/// Resolves one `array` or `set` application.
///
/// The caller has already established that the constructor name is one of the
/// two supported collection spellings. The first argument is the element type
/// and the optional second argument is the collection size. Arity is reported
/// before independently meaningful arguments are examined; arguments beyond
/// the first two have no established role and are ignored after the arity
/// Finding. Argument positions in Findings are zero-based, matching the
/// source argument sequence.
pub(crate) fn resolve(
    application: &TypeApplication,
    index: &ResolutionIndex<'_>,
    findings: &mut Findings<Finding>,
) -> TypeResolution<SemanticType> {
    let name = application.name().name();
    let arguments = application.arguments();
    let mut first_invalid = None;
    let arity_is_invalid = !(1..=2).contains(&arguments.len());
    if arity_is_invalid {
        remember(
            &mut first_invalid,
            findings.report(Finding::WrongArity {
                name: name.to_owned(),
                minimum: 1,
                maximum: 2,
                actual: arguments.len(),
                span: application.name().span(),
            }),
        );
    }

    let mut element = None;
    let mut element_unknown = false;
    if let Some(argument) = arguments.first() {
        match argument {
            TypeArgument::Type(source_type) => {
                match resolver::resolve(source_type, index, findings) {
                    TypeResolution::Resolved(resolved) => element = Some(resolved),
                    TypeResolution::Unknown => element_unknown = true,
                    TypeResolution::Invalid(proof) => remember(&mut first_invalid, proof),
                }
            }
            TypeArgument::Integer(_) => remember(
                &mut first_invalid,
                findings.report(Finding::WrongArgumentRole {
                    name: name.to_owned(),
                    position: 0,
                    expected: crate::ArgumentRole::ElementType,
                    span: argument.span(),
                }),
            ),
        }
    }

    let mut size = None;
    if let Some(argument) = arguments.get(1) {
        match argument {
            TypeArgument::Type(_) => remember(
                &mut first_invalid,
                findings.report(Finding::WrongArgumentRole {
                    name: name.to_owned(),
                    position: 1,
                    expected: crate::ArgumentRole::CollectionSize,
                    span: argument.span(),
                }),
            ),
            TypeArgument::Integer(integer) => match integer.raw().parse::<u64>() {
                Ok(value) => size = Some(value),
                Err(_) => remember(
                    &mut first_invalid,
                    findings.report(Finding::InvalidCollectionSize {
                        name: name.to_owned(),
                        raw: integer.raw().to_owned(),
                        span: integer.span(),
                    }),
                ),
            },
        }
    }

    if let Some(proof) = first_invalid {
        return TypeResolution::Invalid(proof);
    }
    if element_unknown {
        return TypeResolution::Unknown;
    }
    let Some(element) = element else {
        unreachable!("a missing collection element is covered by the arity Finding");
    };

    if name.eq_ignore_ascii_case("array") {
        TypeResolution::Resolved(SemanticType::Array {
            element: Box::new(element),
            exact_length: size,
        })
    } else {
        TypeResolution::Resolved(SemanticType::Set {
            element: Box::new(element),
            max_distinct: size,
        })
    }
}

fn remember(first: &mut Option<Reported>, proof: Reported) {
    if first.is_none() {
        *first = Some(proof);
    }
}
