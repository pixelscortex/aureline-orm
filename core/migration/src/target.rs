//! `SurrealDB` 3.2.0's native sized set checks equality, while Aureline's bound is
//! a maximum. Keep native unsized sets for deduplication and element validation,
//! then enforce every bound in an ASSERT over the type-checked value. See the
//! checker sized-set ADR and `SurrealDB` v3.2.0 `core/src/val/value/convert/coerce.rs`
//! (`coerce_to_set_kind_len`), `doc/field.rs` (`process_assert_clause`), and
//! `fnc/set.rs` (`len` and `all`).

use crate::model::MigrationType;

pub(crate) struct FieldContract {
    /// Native target type. Sized sets are intentionally rendered unsized.
    pub ty: String,
    /// Optional recursive assertion that preserves Aureline-only constraints.
    pub assertion: Option<String>,
}

/// A schema contract whose database enforcement is unproven on `SurrealDB` 3.2.0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetError {
    /// A bound occurs in a union with multiple non-NONE alternatives. Removing
    /// it from TYPE can overlap the alternatives; the assertion must retain
    /// each alternative's complete type guard before this can be supported.
    UnsupportedBoundedSetUnion,
    /// The assertion's positive integer literal cannot fit the pinned target's
    /// signed 64-bit expression integer. Native array lengths use a separate
    /// unsigned parser and do not have this restriction.
    UnsupportedSetBound { max_distinct: u64 },
}

impl std::fmt::Display for TargetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedBoundedSetUnion => f.write_str(
                "SurrealDB 3.2.0 sized-set enforcement in union alternatives is not proven",
            ),
            Self::UnsupportedSetBound { max_distinct } => write!(
                f,
                "SurrealDB 3.2.0 sized-set bound {max_distinct} exceeds the supported assertion integer range",
            ),
        }
    }
}

impl std::error::Error for TargetError {}

pub(crate) fn field_contract(ty: &MigrationType) -> Result<FieldContract, TargetError> {
    // Native set length syntax means exact equality on the pinned target. Keep
    // the native type unsized and express Aureline's maximum with ASSERT so
    // values at or below the bound remain valid.
    Ok(FieldContract {
        ty: render_type(ty),
        assertion: assertion(ty, "$value", 0)?,
    })
}

/// Returns the wildcard descendants created implicitly by a parent DEFINE,
/// ordered from `field[*]` outward. ALTER and REMOVE do not maintain them.
///
/// This mirrors the pinned target's `Kind::inner_kind` and
/// `DefineFieldStatement::process_recursive_definitions`. In particular, a
/// singleton union containing `any` still creates a definition, whereas bare
/// `any` stops recursion. Tuple members never create wildcard definitions.
/// Sized-set bounds belong to the parent's assertion and are absent from all
/// native child types, including bounds below another collection or tuple.
pub(crate) fn implicit_fields(ty: &MigrationType) -> Vec<MigrationType> {
    let mut fields = Vec::new();
    let mut current = inner_kind(&native_type(ty));
    while let Some(kind) = current {
        if matches!(&kind, MigrationType::Scalar(name) if name == "any") {
            break;
        }
        current = inner_kind(&kind);
        fields.push(kind);
    }
    fields
}

fn inner_kind(ty: &MigrationType) -> Option<MigrationType> {
    match ty {
        MigrationType::Array { element, .. } | MigrationType::Set { element, .. } => {
            Some(element.as_ref().clone())
        }
        MigrationType::Union(members) => {
            let kinds: Vec<_> = members.iter().filter_map(inner_kind).collect();
            (!kinds.is_empty()).then_some(MigrationType::Union(kinds))
        }
        MigrationType::Scalar(_) | MigrationType::Record(_) | MigrationType::Tuple(_) => None,
    }
}

fn native_type(ty: &MigrationType) -> MigrationType {
    match ty {
        MigrationType::Array {
            element,
            exact_length,
        } => MigrationType::Array {
            element: Box::new(native_type(element)),
            exact_length: *exact_length,
        },
        MigrationType::Set { element, .. } => MigrationType::Set {
            element: Box::new(native_type(element)),
            max_distinct: None,
        },
        MigrationType::Union(members) => {
            MigrationType::Union(members.iter().map(native_type).collect())
        }
        MigrationType::Tuple(members) => {
            MigrationType::Tuple(members.iter().map(native_type).collect())
        }
        MigrationType::Scalar(_) | MigrationType::Record(_) => ty.clone(),
    }
}

fn is_none(ty: &MigrationType) -> bool {
    matches!(ty, MigrationType::Scalar(name) if name == "none")
}

fn render_type(ty: &MigrationType) -> String {
    match ty {
        MigrationType::Scalar(name) => name.clone(),
        MigrationType::Record(tables) if tables.is_empty() => "record".into(),
        MigrationType::Record(tables) => format!(
            "record<{}>",
            tables
                .iter()
                .map(|table| identifier(table))
                .collect::<Vec<_>>()
                .join(" | ")
        ),
        MigrationType::Array {
            element,
            exact_length,
        } => match exact_length {
            Some(length) => format!("array<{}, {length}>", render_type(element)),
            None => format!("array<{}>", render_type(element)),
        },
        MigrationType::Set { element, .. } => format!("set<{}>", render_type(element)),
        MigrationType::Tuple(members) => format!(
            "[{}]",
            members
                .iter()
                .map(render_type)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        MigrationType::Union(members) => render_union(members),
    }
}

fn render_union(members: &[MigrationType]) -> String {
    // Recursive wildcard derivation preserves the server's nested Either
    // wrappers. SurrealQL cannot spell `any | int` or `option<int> |
    // option<string>`: flatten only for rendering, retaining the structural
    // types for descendant bookkeeping. An `any` descendant needs no further
    // type restriction because its parent still validates the full contract.
    let mut flattened = Vec::new();
    flatten_union(members, &mut flattened);
    if flattened
        .iter()
        .any(|member| matches!(member, MigrationType::Scalar(name) if name == "any"))
    {
        return "any".into();
    }
    let optional = flattened.iter().any(|member| is_none(member));
    let mut values = Vec::new();
    for member in flattened {
        if !is_none(member) {
            let rendered = render_type(member);
            if !values.contains(&rendered) {
                values.push(rendered);
            }
        }
    }
    let joined = values.join(" | ");
    if values.is_empty() {
        "none".into()
    } else if optional {
        format!("option<{joined}>")
    } else {
        joined
    }
}

fn flatten_union<'a>(members: &'a [MigrationType], flattened: &mut Vec<&'a MigrationType>) {
    for member in members {
        if let MigrationType::Union(inner) = member {
            flatten_union(inner, flattened);
        } else {
            flattened.push(member);
        }
    }
}

fn assertion(ty: &MigrationType, value: &str, depth: usize) -> Result<Option<String>, TargetError> {
    // Assertions follow the same shape as the model: collection constraints
    // recurse through `all`, tuple constraints use indexed values, and option
    // constraints explicitly allow NONE. A constrained multi-branch union is
    // rejected because a target-independent type guard is not established.
    match ty {
        MigrationType::Scalar(_) | MigrationType::Record(_) => Ok(None),
        MigrationType::Array { element, .. } => {
            collection_assertion("array", element, value, depth)
        }
        MigrationType::Set {
            element,
            max_distinct,
        } => {
            let mut predicates = Vec::new();
            if let Some(maximum) = max_distinct {
                // Unlike TYPE lengths, expression integers pass through the
                // target's ParsedInt::into_int and must fit signed 64 bits.
                if i64::try_from(*maximum).is_err() {
                    return Err(TargetError::UnsupportedSetBound {
                        max_distinct: *maximum,
                    });
                }
                predicates.push(format!("set::len({value}) <= {maximum}"));
            }
            if let Some(inner) = collection_assertion("set", element, value, depth)? {
                predicates.push(inner);
            }
            Ok(conjunction(predicates))
        }
        MigrationType::Tuple(members) => {
            let mut predicates = Vec::new();
            for (index, member) in members.iter().enumerate() {
                if let Some(predicate) = assertion(member, &format!("{value}[{index}]"), depth)? {
                    predicates.push(predicate);
                }
            }
            Ok(conjunction(predicates))
        }
        MigrationType::Union(members) => {
            let mut constrained = Vec::new();
            for member in members {
                if let Some(predicate) = assertion(member, value, depth)? {
                    constrained.push(predicate);
                }
            }
            if constrained.is_empty() {
                return Ok(None);
            }
            let present = members.iter().filter(|member| !is_none(member)).count();
            if present == 1 && members.iter().any(is_none) {
                return Ok(Some(format!("({value} = NONE OR ({}))", constrained[0])));
            }
            // Erasing a bound can make previously different union alternatives
            // overlap. A global bound would reject values admitted by another
            // branch; accepting either bound without its type guard is unsound.
            Err(TargetError::UnsupportedBoundedSetUnion)
        }
    }
}

fn collection_assertion(
    collection: &str,
    element: &MigrationType,
    value: &str,
    depth: usize,
) -> Result<Option<String>, TargetError> {
    let parameter = format!("$aurl_item_{depth}");
    Ok(assertion(element, &parameter, depth + 1)?
        .map(|predicate| format!("{collection}::all({value}, |{parameter}| {predicate})")))
}

fn conjunction(predicates: Vec<String>) -> Option<String> {
    match predicates.len() {
        0 => None,
        1 => predicates.into_iter().next(),
        _ => Some(
            predicates
                .into_iter()
                .map(|predicate| format!("({predicate})"))
                .collect::<Vec<_>>()
                .join(" AND "),
        ),
    }
}

pub(crate) fn identifier(name: &str) -> String {
    // This is the pinned target's reserved-identifier set, not every SQL token:
    // surrealdb/core/src/syn/lexer/keywords.rs, tag v3.2.0.
    const RESERVED: &[&str] = &[
        "ALTER", "BEGIN", "BREAK", "CANCEL", "COMMIT", "CONTINUE", "CREATE", "DEFINE", "DELETE",
        "FOR", "IF", "INFO", "INSERT", "KILL", "LIVE", "OPTION", "REBUILD", "RETURN", "RELATE",
        "REMOVE", "SELECT", "LET", "SHOW", "SLEEP", "THROW", "UPDATE", "UPSERT", "USE", "DIFF",
        "RAND", "NONE", "NULL", "AFTER", "BEFORE", "VALUE", "BY", "ALL", "TRUE", "FALSE", "WHERE",
        "TABLE", "TB", "SEQUENCE", "FUNCTION",
    ];
    if !name.is_empty()
        && !name.starts_with(|ch: char| ch.is_ascii_digit())
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && name != "NaN"
        && name != "Infinity"
        && !RESERVED
            .iter()
            .any(|keyword| name.eq_ignore_ascii_case(keyword))
    {
        return name.into();
    }
    let mut escaped = String::from("`");
    for ch in name.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '`' => escaped.push_str("\\`"),
            '\0' => escaped.push_str("\\0"),
            '\r' => escaped.push_str("\\r"),
            '\n' => escaped.push_str("\\n"),
            '\t' => escaped.push_str("\\t"),
            '\u{8}' => escaped.push_str("\\u{8}"),
            '\u{c}' => escaped.push_str("\\f"),
            _ => escaped.push(ch),
        }
    }
    escaped.push('`');
    escaped
}
