//! Shared source-location construction for contracts that assert byte spans.
//!
//! Callers pass byte offsets, matching the parser's source representation even
//! when a fixture contains multibyte text.

use aureline_ast::source::{SourceId, SourceSpan, TextRange, TextSize};

pub(super) fn span(source: SourceId, start: u32, end: u32) -> SourceSpan {
    SourceSpan::new(
        source,
        TextRange::new(TextSize::new(start), TextSize::new(end)).expect("test range is ordered"),
    )
}
