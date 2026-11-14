//! Review comments on the text of a side.

use crate::selection::{SelectionKind, SelectionRange};

/// The text of one side a comment is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CommentRange {
    /// 0 for the left side, 1 for the right.
    pub(crate) side: usize,
    pub(crate) kind: SelectionKind,
    pub(crate) range: SelectionRange,
}

impl CommentRange {
    /// Whether `line` of `side` is one of the commented lines.
    pub(crate) fn is_on(&self, side: usize, line: usize) -> bool {
        self.side == side && (self.range.start.line..=self.range.end.line).contains(&line)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Comment {
    pub(crate) range: CommentRange,
    pub(crate) text: String,
}
