//! Mutable state shared by token parsers during the AST construction walk.
//!
//! Chumsky token spans remain source-relative byte ranges. [`ParserState`]
//! supplies the source identity when an AST node or public problem needs a full
//! [`SourceSpan`]. It also owns the AST builder used only at declaration commit
//! points.

use aureline_ast::{
    AstBuilder,
    ast::{Ast, Comment},
    source::{SourceId, SourceSpan},
    tokens::Token,
};
use chumsky::{error::Cheap, extra, input::MappedInput, prelude::SimpleSpan};

use crate::{lexer::TokenOccurrence, problem::source_span};

pub(super) type TokenInput<'tokens, 'src> =
    MappedInput<'tokens, Token<'src>, SimpleSpan, &'tokens [TokenOccurrence<'src>]>;

pub(super) struct ParserState {
    ast: AstBuilder,
    source: SourceId,
}

impl ParserState {
    pub(super) fn new(source: SourceId, comments: Vec<Comment>) -> Self {
        Self {
            ast: AstBuilder::new(comments),
            source,
        }
    }

    pub(super) fn source_span(&self, span: SimpleSpan) -> SourceSpan {
        source_span(self.source, span)
    }

    pub(super) fn ast_mut(&mut self) -> &mut AstBuilder {
        &mut self.ast
    }

    pub(super) fn finish(self) -> Ast {
        self.ast.finish()
    }
}

pub(super) type ParserExtra = extra::Full<Cheap<SimpleSpan>, extra::SimpleState<ParserState>, ()>;
