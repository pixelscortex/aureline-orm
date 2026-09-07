//! Shared structural classification for delimited type-expression sequences.
//!
//! Applications, tuples, and unions attach different language meaning to
//! malformed separators, but they share one structural rule: members and
//! separators alternate. This module reports neutral shape problems so each
//! grammar can map them to its own typed diagnostic policy.

use chumsky::prelude::{SimpleSpan, Spanned};

pub(super) enum SequenceItem<T> {
    Member(Spanned<T>),
    Separator(SimpleSpan),
}

pub(super) enum SequenceShapeProblem {
    MissingMember(SimpleSpan),
    MissingSeparator(SimpleSpan),
    TrailingSeparator(SimpleSpan),
}

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
