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
//!
//! Construction is atomic at the table seam. The parser first consumes the full
//! header and body into staged outcomes, then selects the earliest header or field
//! problem. Only when no problem exists does it call
//! [`AstBuilder::alloc_table`](aureline_ast::AstBuilder::alloc_table)
//! to allocate the table and both field-ownership edges in one construction walk.
//! Malformed input therefore cannot leave a partial public AST behind.
//!
//! For example, this user input:
//!
//! ```text
//! table User schemafull {
//!     owner record<User | Bot>
//! }
//! ```
//!
//! becomes a staged `User`/`schemafull` header plus one staged `owner` field.
//! With no problem, allocation produces one table ID and one field ID: the table
//! lists the field ID, the field points back to the table ID, and the source file
//! lists the table ID. If the field were `first name string`, the full table would
//! still be consumed, but the first structural problem wins and neither ID would
//! be allocated.
//!
//! The word *header* in this file means only the declared name and schema mode:
//!
//! ```text
//! table | User schemafull | {
//!         ^ table header ^
//! ```
//!
//! Parser-producing functions in this module share two lifetimes. `'src` is the
//! lifetime of source spellings borrowed by tokens; `'tokens` is the shorter
//! lifetime of the token slice Chumsky reads. The bound `'src: 'tokens` guarantees
//! that borrowed spellings remain valid throughout parsing. Returning
//! `impl Parser<...>` builds a parser definition; the function does not consume
//! input until the grammar entrypoint runs that parser.

use aureline_ast::{ast::SchemaType, tokens::Token};
use chumsky::prelude::*;

use super::{
    atom::{ident, integer, schema_type},
    field::{self, FieldOutcome},
    problem::GrammarProblem,
    state::{ParserExtra, TokenInput},
};

/// Staged result parsed between the `table` keyword and the opening `{`.
///
/// `Ok` contains the valid declared name and its following spanned schema mode.
/// Header parsing emits one name and one schema mode. Malformed header tokens
/// fail at the grammar boundary; the body is only staged for a structurally
/// valid header before the table can commit.
type ParsedTableHeader = (Spanned<String>, Spanned<SchemaType>);

/// Parses the declared name and schema mode after `table` and before `{`.
///
/// For example, the token shape `User schemafull` becomes `Ok(name = User,
/// following = schemafull)`. `User Profile schemafull` is also consumed
/// completely, but becomes `Err(IdentifierWhitespace)`. This parser consumes
/// neither the `table` keyword nor the body opener; the complete-table [`parser`]
/// owns both.
fn header_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedTableHeader, ParserExtra> {
    ident().then(schema_type())
}

/// Parses the brace-delimited field body without allocating AST nodes.
///
/// The consumed region includes `{` and `}`. It may contain no fields or several
/// [`FieldOutcome`] values; physical newlines may surround fields and must
/// separate adjacent fields. The closing brace can terminate the final field.
/// Outcomes remain staged so the complete-table [`parser`] can select the
/// earliest problem before committing any table or field to the AST.
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

/// Parses a table with a recognized schema mode, then commits or rejects it.
///
/// Every body item is a staged [`FieldOutcome`]. This parser folds all problems by source position
/// before touching the AST. A problem-free declaration allocates the table and
/// fields together; a rejected declaration allocates nothing.
fn recognized_table_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Option<GrammarProblem>, ParserExtra> {
    just(Token::Table)
        .ignore_then(header_parser())
        .then(body_parser())
        .map_with(|(header, fields), context| {
            // Fold every staged result before mutating AstBuilder. Source order,
            // rather than recovery-alternative order, decides which problem the
            // public parser reports for a table containing several bad shapes.
            let mut problem: Option<GrammarProblem> = None;
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
            if let Some(problem) = problem {
                return Some(problem);
            }

            let table_span = context.span();
            let state = &mut context.state().0;
            let table_span = state.source_span(table_span);
            let name_span = state.source_span(header.0.span);
            let schema_type_span = state.source_span(header.1.span);
            state.ast_mut().alloc_table(
                table_span,
                header.0.inner,
                name_span,
                header.1.inner,
                schema_type_span,
                |table_fields| {
                    for field in parsed_fields {
                        field.alloc_in(table_fields);
                    }
                },
            );
            None
        })
}

/// Combines the mini parsers that recognize one complete `table` declaration.
///
/// [`missing_schema_type_parser`] owns the table-specific malformed schema slot;
/// [`recognized_table_parser`] owns headers with a real schema mode and delegates
/// the complete declaration grammar directly. The output describes
/// the commit result:
///
/// - `None` means the declaration was valid and has been allocated in
///   [`ParserState`](super::state::ParserState)'s
///   [`AstBuilder`](aureline_ast::AstBuilder);
/// - `Some(problem)` means a recognizable declaration was consumed and no part
///   of that table entered the AST.
///
/// An unrecognized token shape fails through Chumsky and becomes
/// [`SyntaxProblem::UnexpectedToken`](crate::SyntaxProblem::UnexpectedToken) at
/// the grammar seam.
pub(super) fn parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Option<GrammarProblem>, ParserExtra> {
    let integer_name = just(Token::Table)
        .ignore_then(integer())
        .then(schema_type())
        .then(body_parser())
        .map(|((integer, _), _)| Some(GrammarProblem::identifier_starts_with_digit(integer.span)));
    choice((integer_name, recognized_table_parser()))
}
