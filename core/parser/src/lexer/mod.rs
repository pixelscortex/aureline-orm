//! Converts source text into grammar tokens without discarding source layout.
//!
//! Lexing has two stages:
//!
//! 1. [`lexer`] scans the source and emits [`LexerOccurrence`] values. An
//!    occurrence can be a grammatical token, a comment, an inline-whitespace
//!    run, or a typed lexical problem.
//! 2. [`lex`] partitions those occurrences into [`LexedSource`]. The grammar
//!    receives only `tokens`, while comments become AST data and whitespace
//!    spans remain available to contextual name recovery.
//!
//! Keeping those channels separate is deliberate. Comments do not affect the
//! grammar, but callers still need their exact source locations. Spaces and
//! tabs usually do not affect the grammar either, but they distinguish an
//! attached malformed name such as `User?Name` from separated syntax such as
//! `User ? Name`.
//!
//! This module recognizes characters, not their grammatical role. In
//! particular, `<`, `>`, `[`, `]`, `,`, `?`, and `|` are always ordinary
//! structural tokens here because they are valid in type expressions:
//!
//! ```text
//! array<string>
//! [string, int]
//! User | Bot
//! ```
//!
//! The grammar's `declared_name` module decides whether those same tokens
//! violate a table or field name when they occur in a declared-name slot.

mod comment;
mod identifier;

use aureline_ast::{
    ast::{Comment, CommentKind},
    source::{SourceId, TextSize},
    tokens::Token,
};
use chumsky::{error::Cheap, extra, prelude::*};

use crate::{IdentifierProblem, SyntaxProblem, problem::source_span};

/// A grammatical token paired with its half-open byte range in the source.
pub(crate) type TokenOccurrence<'src> = Spanned<Token<'src>>;

/// An intermediate lexer result paired with its half-open byte range.
///
/// Most source fragments produce one occurrence. A multiline block comment is
/// the exception: it produces one comment occurrence plus an overlapping
/// newline occurrence for every physical line boundary inside the comment.
pub(super) type LexerOccurrence<'src> = Spanned<Lexeme<'src>>;

/// Chumsky's unrecovered character-level error channel.
///
/// Expected malformed forms use [`Lexeme`] variants instead. This channel is
/// reserved for characters that none of the lexical parsers can consume, such
/// as `;` in `table T schemafull { value string; }`.
pub(super) type LexerExtra = extra::Err<Cheap<SimpleSpan>>;

/// One classified source occurrence before the output channels are separated.
pub(super) enum Lexeme<'src> {
    /// A token that participates in grammar parsing.
    ///
    /// Examples include `table`, an identifier such as `User`, the integer
    /// argument `3`, a newline, and structural punctuation such as `<` or `|`.
    Token(Token<'src>),
    /// A recognized identifier-shaped candidate with a precise boundary
    /// violation.
    ///
    /// Examples:
    ///
    /// - `1User` carries [`IdentifierProblem::StartsWithDigit`] on `1`;
    /// - `Café` carries [`IdentifierProblem::ContainsNonAscii`] on `é`;
    /// - `User.Name` carries [`IdentifierProblem::ContainsDot`] on `.`;
    /// - `` `User` `` carries [`IdentifierProblem::BackticksReserved`] on the
    ///   complete backtick-delimited spelling.
    InvalidIdentifier(IdentifierProblem),
    /// One complete space/tab run retained for contextual grammar recovery.
    ///
    /// It is never emitted into the grammatical token stream. Its exact range
    /// lets declared-name recovery distinguish `User?Name` from `User ? Name`.
    InlineWhitespace,
    /// A complete line or block comment retained as AST metadata.
    ///
    /// The comment itself never enters the grammar token stream. Newlines
    /// inside a block comment are emitted separately as [`Token::Newline`].
    Comment(CommentKind),
    /// A `/*` opener for which no closing `*/` exists before end of input.
    ///
    /// The occurrence covers the entire unfinished comment so the scanner can
    /// consume the input. [`lex`] narrows the public diagnostic to the opening
    /// `/*`, which is the location the author needs to fix.
    UnterminatedBlockComment,
}

/// Lexer output split according to what later parsing stages need.
///
/// All stored spans are byte ranges into the original source. Tokens and
/// identifiers borrow their spelling from that source; the source text is not
/// copied into this value.
pub(crate) struct LexedSource<'src> {
    /// The only occurrences supplied to the token grammar.
    pub(super) tokens: Vec<TokenOccurrence<'src>>,
    /// Comments preserved for the final AST even though the grammar ignores
    /// their contents.
    pub(super) comments: Vec<Comment>,
    /// Complete runs of spaces/tabs, used only to classify name-recovery gaps.
    pub(super) inline_whitespace: Vec<SimpleSpan>,
    /// Identity attached to every public AST and problem span.
    pub(super) source: SourceId,
    /// End-of-input byte offset used by Chumsky when an error occurs at EOF.
    pub(super) source_len: usize,
}

/// Lexes one source file and converts recognized lexical failures into public
/// [`SyntaxProblem`] values.
///
/// The common typed failures are:
///
/// ```text
/// table 1User schemafull {}       # InvalidIdentifier(StartsWithDigit) at `1`
/// table Café schemafull {}        # InvalidIdentifier(ContainsNonAscii) at `é`
/// table User.Name schemafull {}   # InvalidIdentifier(ContainsDot) at `.`
/// table `User` schemafull {}      # InvalidIdentifier(BackticksReserved)
/// table T schemafull { x T; }     # InvalidToken at `;`
/// table T schemafull {} /* open   # UnterminatedBlockComment at `/*`
/// ```
///
/// Structural punctuation is intentionally absent from this list. For
/// example, `?` lexes successfully as [`Token::Question`]; the grammar later
/// reports either postfix optional syntax (`string?`) or punctuation in a
/// declared name (`User?Name`) according to context.
///
/// Sources larger than [`TextSize`] can represent fail before scanning so every
/// later byte boundary can be converted into an AST [`aureline_ast::source::SourceSpan`].
pub(crate) fn lex(
    source_id: SourceId,
    source: &str,
) -> Result<LexedSource<'_>, Vec<SyntaxProblem>> {
    if TextSize::try_from(source.len()).is_err() {
        return Err(vec![SyntaxProblem::SourceTooLarge {
            byte_len: source.len(),
        }]);
    }

    let occurrences = lexer().parse(source).into_result().map_err(|errors| {
        errors
            .into_iter()
            .map(|error| SyntaxProblem::InvalidToken {
                span: source_span(source_id, *error.span()),
            })
            .collect::<Vec<_>>()
    })?;

    let mut tokens = Vec::new();
    let mut comments = Vec::new();
    let mut inline_whitespace = Vec::new();
    for Spanned { inner, span } in occurrences {
        match inner {
            Lexeme::InvalidIdentifier(problem) => {
                return Err(vec![SyntaxProblem::InvalidIdentifier {
                    problem,
                    span: source_span(source_id, span),
                }]);
            }
            Lexeme::Comment(kind) => {
                comments.push(Comment::new(kind, source_span(source_id, span)));
            }
            Lexeme::InlineWhitespace => inline_whitespace.push(span),
            Lexeme::UnterminatedBlockComment => {
                let opening = SimpleSpan::from(span.start..span.start + 2);
                return Err(vec![SyntaxProblem::UnterminatedBlockComment {
                    span: source_span(source_id, opening),
                }]);
            }
            Lexeme::Token(token) => tokens.push(Spanned { inner: token, span }),
        }
    }

    Ok(LexedSource {
        tokens,
        comments,
        inline_whitespace,
        source: source_id,
        source_len: source.len(),
    })
}

/// Builds the character-level parser that covers the complete source.
///
/// Alternative order records several lexical decisions:
///
/// - `//` and `/*` are attempted before identifier candidates, so comment
///   delimiters never become identifier punctuation;
/// - a terminated block comment is attempted before the unterminated form, so
///   `/* closed */` is not diagnosed merely because both forms begin with `/*`;
/// - a complete backtick-delimited spelling becomes the dedicated reserved-name
///   problem rather than a generic invalid token;
/// - newlines become grammar tokens, while spaces and tabs enter the separate
///   [`Lexeme::InlineWhitespace`] channel.
///
/// Each branch returns a vector because a block comment may also emit newline
/// tokens. The final `flatten` restores one source-ordered occurrence stream.
#[must_use]
fn lexer<'src>() -> impl Parser<'src, &'src str, Vec<LexerOccurrence<'src>>, LexerExtra> {
    let punctuation = choice((
        just('{').to(Token::LBrace),
        just('}').to(Token::RBrace),
        just('<').to(Token::LAngle),
        just('>').to(Token::RAngle),
        just('[').to(Token::LBracket),
        just(']').to(Token::RBracket),
        just(',').to(Token::Comma),
        just('?').to(Token::Question),
        just('|').to(Token::Pipe),
    ));

    let syntax_token = choice((text::newline().to(Token::Newline), punctuation))
        .spanned()
        .map(|occurrence: TokenOccurrence<'src>| {
            vec![Spanned {
                inner: Lexeme::Token(occurrence.inner),
                span: occurrence.span,
            }]
        });

    let inline_whitespace = one_of(" \t")
        .repeated()
        .at_least(1)
        .map_with(|(), context| {
            vec![Spanned {
                inner: Lexeme::InlineWhitespace,
                span: context.span(),
            }]
        });

    choice((
        comment::line(),
        comment::block(),
        comment::unterminated_block(),
        identifier::backtick(),
        identifier::candidate(),
        syntax_token,
        inline_whitespace,
    ))
    .repeated()
    .collect::<Vec<_>>()
    .map(|groups| groups.into_iter().flatten().collect())
}
