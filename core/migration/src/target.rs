//! `SurrealDB` 3.2.0's native sized set checks equality, while Aureline's bound is
//! a maximum. Keep native unsized sets for deduplication and element validation,
//! then enforce every bound in an ASSERT over the type-checked value. See the
//! checker sized-set ADR and `SurrealDB` v3.2.0 `core/src/val/value/convert/coerce.rs`
//! (`coerce_to_set_kind_len`), `doc/field.rs` (`process_assert_clause`), and
//! `fnc/set.rs` (`len` and `all`).

use crate::model::MigrationType;

pub(crate) struct FieldContract {
    pub ty: String,
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
    Ok(FieldContract {
        ty: render_type(ty),
        assertion: assertion(ty, "$value", 0)?,
    })
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
        MigrationType::Union(members) => {
            let values: Vec<_> = members.iter().filter(|member| !is_none(member)).collect();
            let joined = values
                .iter()
                .map(|member| render_type(member))
                .collect::<Vec<_>>()
                .join(" | ");
            if values.len() < members.len() {
                format!("option<{joined}>")
            } else {
                joined
            }
        }
    }
}

fn assertion(ty: &MigrationType, value: &str, depth: usize) -> Result<Option<String>, TargetError> {
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
