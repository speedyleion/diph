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

/// A comment being written.
pub(crate) struct Draft {
    pub(crate) range: CommentRange,
}
