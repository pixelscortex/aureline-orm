//! Parses complete table declarations and commits their staged fields to the AST.
//!
//! The valid outer shape is:
//!
//! ```text
//! table <name> (schemafull | schemaless) {
//!     <field-name> <type-expression>
//! }
//! ```
//!
//! Physical newlines—not commas or semicolons—separate fields. Recovery parsers
//! consume known malformed table names and headers so callers receive a precise
//! problem rather than an error at a later brace or keyword.

use aureline_ast::{ast::SchemaType, tokens::Token};
use chumsky::prelude::*;

use super::{
    atom::{ident, integer, schema_type},
    declared_name,
    field::{self, FieldOutcome},
    problem::GrammarProblem,
    state::{ParserExtra, TokenInput},
};

/// A parsed header retained until the body has also been checked.
struct ParsedTableHeader {
    /// Declared table name and its token span.
    name: Spanned<String>,
    /// Schema mode and its keyword span.
    schema_type: Spanned<SchemaType>,
    /// A recovered header problem, if the complete header shape was malformed.
    problem: Option<GrammarProblem>,
}

/// Parses either a normal `<name> <schema-mode>` header or the recoverable
/// three-identifier shape `<name> <extra> <schema-mode>`.
///
/// The latter recognizes a whitespace-split table name:
///
/// ```text
/// table User Profile schemafull {}
///           ^ gap -> InvalidIdentifier(ContainsWhitespace)
/// ```
///
/// The lexer omits comments from the token stream, so the raw byte gap may
/// contain more than inline whitespace. Only a retained whitespace span that
/// exactly fills the gap receives the identifier-specific classification;
/// otherwise `<extra>` becomes [`GrammarProblem::Unexpected`].
fn header_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTableHeader, ParserExtra> {
    let split_name = ident().then(ident()).then(schema_type()).map_with(
        |((name, extra), schema_type), context| {
            let state = &context.state().0;
            let problem = state
                .inline_whitespace_between(name.span, extra.span)
                .map_or(GrammarProblem::Unexpected(extra.span), |gap| {
                    GrammarProblem::IdentifierWhitespace(gap)
                });
            ParsedTableHeader {
                name,
                schema_type,
                problem: Some(problem),
            }
        },
    );

    let header = ident()
        .then(schema_type())
        .map(|(name, schema_type)| ParsedTableHeader {
            name,
            schema_type,
            problem: None,
        });

    // Try `name extra schema-type` first so the normal two-token header cannot
    // succeed early and leave `extra` to fail later at the body opener.
    choice((split_name, header))
}

/// Parses a brace-delimited sequence of newline-separated field outcomes.
///
/// Leading, trailing, and repeated newlines are allowed, so blank lines around
/// fields are inert. A newline re-emitted from inside a multiline block comment
/// also separates fields:
///
/// ```text
/// first string /* boundary
/// inside */ second int
/// ```
///
/// A single-line block comment emits no newline, so `first string /* note */
/// second int` reaches an unexpected-token problem instead of becoming two
/// fields. Commas and semicolons are never field separators.
fn body_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Vec<FieldOutcome>, ParserExtra> {
    let newlines = just(Token::Newline).repeated().at_least(1);
    field::parser()
        .separated_by(newlines)
        .allow_leading()
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
}

/// Parses one complete table, returns its earliest typed problem, and allocates
/// the table only when its header and every field are valid.
///
/// Name/header recovery precedes normal parsing:
///
/// - `table 1 schemafull {}` reports a leading-digit identifier problem;
/// - `table array<string> schemafull {}` reports `<` as identifier punctuation;
/// - `table User[] schemafull {}` reports `[` as identifier punctuation;
/// - `table User?Name schemafull {}` reports `?` as identifier punctuation;
/// - `table User ? Name schemafull {}` reports `?` as unexpected separated
///   syntax;
/// - `table User mystery {}` reports `mystery` as an unexpected schema mode.
///
/// The normal branch compares any header problem with all field problems by
/// source position. Valid fields remain staged until that comparison succeeds;
/// this prevents a malformed table from leaving public partial data in the AST.
pub(super) fn parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Option<GrammarProblem>, ParserExtra> {
    // Pure digits are Integer tokens because they are legal type arguments.
    // Only this table-name position can reclassify `table 1 ...` as a declared
    // identifier that starts with a digit.
    let integer_name = just(Token::Table)
        .ignore_then(integer())
        .then(schema_type())
        .then(body_parser())
        .map(|((name, _), _)| Some(GrammarProblem::IdentifierStartsWithDigit(name.span)));

    // Consume the complete marked type-expression shape before the general
    // compound branch can absorb the following schema mode. For
    // `table array<string,3> schemafull {}`, this retains the opening `<` as the
    // first name violation.
    let marked_name = just(Token::Table)
        .ignore_then(declared_name::marked_type_expression())
        .then(schema_type())
        .then(body_parser())
        .map(|((problem, _), _)| Some(problem));

    let punctuated_name = just(Token::Table)
        .ignore_then(declared_name::punctuated())
        .then(schema_type())
        .then(body_parser())
        .map(|((name, _), _)| Some(name.into_problem()));

    // Recover `table User mystery {}` before normal header parsing so `mystery`
    // itself remains the unexpected token instead of a later `{` or EOF.
    let missing_schema_type = just(Token::Table)
        .ignore_then(ident())
        .then(ident())
        .then(body_parser())
        .map(|((_, unexpected), _)| Some(GrammarProblem::Unexpected(unexpected.span)));

    let table = just(Token::Table)
        .ignore_then(header_parser())
        .then(body_parser())
        .map_with(|(header, fields), context| {
            let mut problem = header.problem;
            let mut parsed_fields = Vec::new();
            for field in fields {
                match field {
                    FieldOutcome::Field(field) => parsed_fields.push(field),
                    FieldOutcome::Problem(next) => {
                        if problem.is_none_or(|current| next.span().start < current.span().start) {
                            problem = Some(next);
                        }
                    }
                }
            }
            if problem.is_some() {
                return problem;
            }

            let table_span = context.span();
            let state = &mut context.state().0;
            let table_span = state.source_span(table_span);
            let name_span = state.source_span(header.name.span);
            let schema_type_span = state.source_span(header.schema_type.span);
            state.ast_mut().alloc_table(
                table_span,
                header.name.inner,
                name_span,
                header.schema_type.inner,
                schema_type_span,
                |table_fields| {
                    for field in parsed_fields {
                        field.alloc_in(table_fields);
                    }
                },
            );
            None
        });

    choice((
        integer_name,
        marked_name,
        punctuated_name,
        missing_schema_type,
        table,
    ))
}
