//! Mutable state shared by token parsers during the AST construction walk.
//!
//! Chumsky token spans remain source-relative byte ranges. [`ParserState`]
//! supplies the source identity when an AST node or public problem needs a full
//! [`SourceSpan`]. It also owns the AST builder and the lexer's otherwise
//! discarded inline-whitespace channel.

use aureline_ast::{
    AstBuilder,
    ast::{Ast, Comment},
    source::{SourceId, SourceSpan},
    tokens::Token,
};
use chumsky::{error::Cheap, extra, input::MappedInput, prelude::SimpleSpan};

use crate::{lexer::TokenOccurrence, problem::source_span};

/// Chumsky input over borrowed tokens with each token mapped to its original
/// source byte range.
pub(super) type TokenInput<'tokens, 'src> =
    MappedInput<'tokens, Token<'src>, SimpleSpan, &'tokens [TokenOccurrence<'src>]>;

/// Construction data that cannot be represented by the token stream alone.
pub(super) struct ParserState {
    /// Arena builder receiving only successfully constructed declarations.
    ast: AstBuilder,
    /// Identity attached when parser-relative spans enter the public AST.
    source: SourceId,
    /// Exact runs of spaces/tabs retained for contextual name diagnostics.
    inline_whitespace: Vec<SimpleSpan>,
}

impl ParserState {
    /// Starts a construction walk and transfers lexed comments into the AST
    /// builder before grammatical declarations are allocated.
    pub(super) fn new(
        source: SourceId,
        comments: Vec<Comment>,
        inline_whitespace: Vec<SimpleSpan>,
    ) -> Self {
        Self {
            ast: AstBuilder::new(comments),
            source,
            inline_whitespace,
        }
    }

    /// Attaches this file's identity to a parser-relative byte span.
    pub(super) fn source_span(&self, span: SimpleSpan) -> SourceSpan {
        source_span(self.source, span)
    }

    /// Returns the whitespace run that exactly fills the gap between two token
    /// spans.
    ///
    /// The exact match is what distinguishes a split declared name from more
    /// general invalid syntax. In `table User Profile schemafull {}`, the gap
    /// between `User` and `Profile` is a retained inline-whitespace span and
    /// becomes `InvalidIdentifier(ContainsWhitespace)`. A comment or newline in
    /// that gap does not match and is handled according to the surrounding
    /// grammar instead.
    pub(super) fn inline_whitespace_between(
        &self,
        left: SimpleSpan,
        right: SimpleSpan,
    ) -> Option<SimpleSpan> {
        let gap = SimpleSpan::from(left.end..right.start);
        self.inline_whitespace.contains(&gap).then_some(gap)
    }

    /// Gives a successful grammar action access to the arena builder.
    pub(super) fn ast_mut(&mut self) -> &mut AstBuilder {
        &mut self.ast
    }

    /// Finalizes the arena-backed AST after the entire document succeeds.
    pub(super) fn finish(self) -> Ast {
        self.ast.finish()
    }
}

/// Parser errors, construction state, and no custom context payload.
pub(super) type ParserExtra = extra::Full<Cheap<SimpleSpan>, extra::SimpleState<ParserState>, ()>;
