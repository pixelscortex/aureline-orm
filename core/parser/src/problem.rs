//! Public, source-spanned syntax problems returned by the parser entrypoints.
//!
//! Expected malformed language forms receive stable typed variants so callers
//! can offer directed messages without inspecting parser-library errors. Each
//! problem identifies the exact source bytes that caused it.

use aureline_ast::source::{SourceId, SourceSpan, TextRange, TextSize};
use chumsky::prelude::SimpleSpan;

/// The violated part of Aureline's ASCII bare-identifier boundary,
/// `[A-Za-z_][A-Za-z0-9_]*`.
///
/// The lexer reports character-level violations such as `User.Name`. The grammar
/// additionally recognizes a pure integer in a table or field name slot as a
/// leading-digit violation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentifierProblem {
    /// An identifier began with an ASCII digit.
    ///
    /// Triggers include `table 1User schemafull {}` and the pure-integer name
    /// `table 1 schemafull {}`. The problem span covers the leading digit (or
    /// complete integer token for a pure-integer name).
    StartsWithDigit,
    /// An identifier contained a character outside ASCII.
    ///
    /// `table Café schemafull {}` stores `'é'` and spans its complete UTF-8 byte
    /// sequence.
    ContainsNonAscii(char),
    /// An identifier contained `.`.
    ///
    /// Trigger: `table User.Name schemafull {}`. The span covers `.`.
    ContainsDot,
    /// An identifier contained `-`.
    ///
    /// Trigger: `table User-Name schemafull {}`. The span covers `-`.
    ContainsHyphen,
    /// An identifier contained another ASCII punctuation character.
    ///
    /// Examples include `User@Name` and `User/Name`. The problem span identifies
    /// the first violation.
    ContainsPunctuation,
    /// A name used backticks, which Aureline reserves rather than accepting as
    /// identifier escaping.
    ///
    /// Trigger: ``table `User` schemafull {}``. The span covers the complete
    /// backtick-delimited spelling.
    BackticksReserved,
}

/// A typed problem produced before the parser can construct a complete syntax tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyntaxProblem {
    /// The source byte length exceeds the largest Aureline text offset.
    ///
    /// This is checked before lexing so every later source boundary fits in
    /// [`TextSize`]. `byte_len` is the actual UTF-8 byte length received.
    SourceTooLarge { byte_len: usize },
    /// The lexer could not form a token; `span` covers the offending source
    /// bytes.
    ///
    /// Trigger: the unsupported `;` in `table T schemafull { value string; }`.
    InvalidToken { span: SourceSpan },
    /// A name crossed Aureline's bare-identifier boundary; `span` covers the
    /// bytes that violate the boundary.
    ///
    /// See [`IdentifierProblem`] for concrete trigger shapes and span behavior.
    InvalidIdentifier {
        problem: IdentifierProblem,
        span: SourceSpan,
    },
    /// A type application contained no arguments; `span` covers `<>`.
    ///
    /// Trigger: `array<>`.
    EmptyTypeArguments { span: SourceSpan },
    /// A type application ended immediately after a comma; `span` covers the
    /// comma.
    ///
    /// Trigger: `array<string,>`.
    TrailingTypeArgumentComma { span: SourceSpan },
    /// A union pipe was missing a member on at least one side; `span` covers the
    /// offending `|`.
    ///
    /// Triggers: `| string`, `string |`, `string | | int`, and `record<|>`.
    MissingUnionMember { span: SourceSpan },
    /// A tuple comma was missing a member; `span` covers the offending comma.
    ///
    /// Triggers: `[, int]`, `[int,, string]`, and `[,]`. `[int,]` is valid.
    MissingTupleMember { span: SourceSpan },
    /// A tuple member began without a comma; `span` covers that member.
    ///
    /// Triggers include `[int string]`, `[int record<A>]`, and
    /// `[int [string]]`. The span covers the complete first adjacent member.
    MissingTupleSeparator { span: SourceSpan },
    /// A block comment reached the end of input; `span` points at its opening
    /// delimiter.
    ///
    /// Trigger: `table T schemafull {} /* still open`.
    UnterminatedBlockComment { span: SourceSpan },
    /// The token stream did not match the grammar; `span` covers the unexpected
    /// token or is empty at the unexpected end of input.
    ///
    /// Examples include an unknown schema mode in `table T mystery {}` and the
    /// second field in `first string second int`, where no physical newline
    /// separates the fields.
    UnexpectedToken { span: SourceSpan },
}

/// Attaches a source identity to a Chumsky byte range after source length has
/// been validated.
///
/// Both lexer and grammar spans use half-open UTF-8 byte offsets. The parser
/// entrypoint rejects an oversized source before either stage runs, so these
/// conversions are invariants rather than recoverable failures.
pub(crate) fn source_span(source: SourceId, span: SimpleSpan) -> SourceSpan {
    // The mapped token input preserves Chumsky's ordered source boundaries. The
    // entrypoint's length check makes each boundary representable as TextSize.
    let start = TextSize::try_from(span.start)
        .expect("parser span starts within the prevalidated source length");
    let end = TextSize::try_from(span.end)
        .expect("parser span ends within the prevalidated source length");
    let range = TextRange::new(start, end).expect("mapped parser spans preserve input order");
    SourceSpan::new(source, range)
}
