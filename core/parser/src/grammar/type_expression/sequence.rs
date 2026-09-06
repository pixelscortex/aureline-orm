//! Shared structural classification for delimited type-expression sequences.
//!
//! Applications, tuples, and unions attach different language meaning to
//! malformed separators, but they share one structural rule: members and
//! separators alternate. This module reports neutral shape problems so each
//! grammar can map them to its own typed diagnostic policy.
//!
//! For example, `A,,B` becomes `Member(A), Separator, Separator, Member(B)` and
//! reports `MissingMember` at the second separator. `A B` reports
//! `MissingSeparator` at `B`; `A,` reports `TrailingSeparator` at the comma.

use chumsky::prelude::{SimpleSpan, Spanned};

/// One consumed member or separator, retaining the span used for diagnostics.
pub(super) enum SequenceItem<T> {
    Member(Spanned<T>),
    Separator(SimpleSpan),
}

/// A grammar-neutral violation of alternating member/separator structure.
pub(super) enum SequenceShapeProblem {
    MissingMember(SimpleSpan),
    MissingSeparator(SimpleSpan),
    TrailingSeparator(SimpleSpan),
}

/// Reports every local alternation problem in source order.
///
/// This function does not inspect member validity or decide whether a trailing
/// separator is legal. Application, tuple, and union classifiers own those
/// language-specific decisions.
pub(super) fn shape_problems<T>(items: &[SequenceItem<T>]) -> Vec<SequenceShapeProblem> {
    items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match item {
            SequenceItem::Separator(span)
                if index == 0 || matches!(items[index - 1], SequenceItem::Separator(_)) =>
            {
                Some(SequenceShapeProblem::MissingMember(*span))
            }
            SequenceItem::Separator(span) if index + 1 == items.len() => {
                Some(SequenceShapeProblem::TrailingSeparator(*span))
            }
            SequenceItem::Member(member)
                if index > 0 && matches!(items[index - 1], SequenceItem::Member(_)) =>
            {
                Some(SequenceShapeProblem::MissingSeparator(member.span))
            }
            SequenceItem::Member(_) | SequenceItem::Separator(_) => None,
        })
        .collect()
}
