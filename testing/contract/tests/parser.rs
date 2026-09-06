//! Public parser contracts, grouped by the language concern each test protects.
//!
//! The modules keep test names and assertions close to the grammar behavior
//! they describe; `support` contains only span construction shared by those
//! contracts.

#[path = "parser/comments_newlines.rs"]
mod comments_newlines;
#[path = "parser/declarations.rs"]
mod declarations;
#[path = "parser/diagnostics.rs"]
mod diagnostics;
#[path = "parser/identifiers.rs"]
mod identifiers;
#[path = "parser/support.rs"]
mod support;
#[path = "parser/tokenizer.rs"]
mod tokenizer;
#[path = "parser/tuples.rs"]
mod tuples;
#[path = "parser/type_expressions.rs"]
mod type_expressions;
