//! Parses complete table declarations and commits their staged Fields to the AST.
//!
//! The valid outer shape is:
//!
//! ```text
//! table <name> (schemafull | schemaless) {
//!     <field-name> <type-expression>
//! }
//! ```
//!
//! Physical newlines separate Fields. The table parser owns the `table` keyword,
//! its one-token name, mandatory schema mode, and brace-delimited body. The body
//! stages every Field before construction. Only a problem-free declaration calls
//! [`AstBuilder::alloc_table`](aureline_ast::AstBuilder::alloc_table), which
//! allocates the table, its Fields, and both ownership edges together.
//!
//! `table User schemafull { owner record<User | Bot> }` therefore becomes a
//! staged header and Field before producing either arena ID. In
//! `table User Profile schemafull {}`, `User` is the complete table name and the
//! schema parser reports `Profile` as the first token that cannot occupy the
//! mandatory mode slot.

use aureline_ast::{ast::SchemaType, tokens::Token};
use chumsky::prelude::*;

use super::{
    atom::schema_type,
    declared_name,
    field::{self, FieldOutcome},
    problem::GrammarProblem,
    state::{ParserExtra, TokenInput},
};

/// A valid header held until the body has parsed and the table can commit.
///
/// Token-relative spans stay attached to both values so allocation can translate
/// them to source spans only after the complete declaration is accepted.
struct ParsedTableHeader {
    name: Spanned<String>,
    schema_type: Spanned<SchemaType>,
}

/// Retains the integer-name problem in the same staged header position.
type StagedTableHeader = Result<ParsedTableHeader, GrammarProblem>;

/// Parses the one-token table name and mandatory schema-mode slot.
///
/// The valid schema slot emits its spanned mode. Any other token fails directly
/// at that slot through the ordinary grammar error path. The header parser
/// consumes neither `table` nor `{` and never mutates the AST.
fn header_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, StagedTableHeader, ParserExtra> {
    declared_name::parser()
        .then(schema_type())
        .map(|(name, schema_type)| match name {
            Ok(name) => Ok(ParsedTableHeader { name, schema_type }),
            Err(problem) => Err(problem),
        })
}

/// Parses the brace-delimited Field body without allocating AST nodes.
///
/// The consumed region includes `{` and `}`. It may contain no Fields or several
/// [`FieldOutcome`] values; physical newlines may surround Fields and must
/// separate adjacent Fields. The closing brace can terminate the final Field.
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

/// Parses and conditionally commits one complete `table` declaration.
///
/// The composition reads in source order: keyword, header, then body. Header and
/// Field problems stay as private staged values until the closing `}` has been
/// consumed. The earliest problem rejects the declaration without allocation;
/// otherwise the table and every Field are allocated atomically.
///
/// In these parser signatures, `'src` owns borrowed source spelling and
/// `'tokens` owns the token input; `'src: 'tokens` keeps spelling alive while
/// parsing. `impl Parser` describes a parser rather than an already parsed table.
pub(super) fn parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Option<GrammarProblem>, ParserExtra> {
    just(Token::Table)
        .ignore_then(header_parser())
        .then(body_parser())
        .map_with(|(header, fields), context| {
            let mut problem = header.as_ref().err().copied();
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

            let Ok(header) = header else {
                unreachable!("a problem-free table has a valid parsed header")
            };

            let parsed_table_span = context.span();
            let state = &mut context.state().0;
            let table_span = state.source_span(parsed_table_span);
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
        })
}
