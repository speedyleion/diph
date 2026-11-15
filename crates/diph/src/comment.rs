//! Review comments on the text of a side.

use diph_core::Edit;

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

    /// Moves the commented lines with the text `edit` changed on `side`.
    /// Lines whose text moved go where it went, and lines it deleted go to
    /// the line it starts on. Columns stay as they were.
    pub(crate) fn apply(&mut self, side: usize, edit: &Edit) {
        if self.side != side {
            return;
        }
        let Edit {
            start,
            deleted_end,
            inserted_end,
        } = *edit;
        let moved = |line: usize| match line {
            line if line > deleted_end.line
                || (line == deleted_end.line && deleted_end.byte == 0) =>
            {
                line - deleted_end.line + inserted_end.line
            }
            line if line > start.line => start.line,
            line => line,
        };
        self.range.start.line = moved(self.range.start.line);
        self.range.end.line = moved(self.range.end.line);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Comment {
    pub(crate) range: CommentRange,
    pub(crate) text: String,
}

#[cfg(test)]
mod tests {
    use diph_core::{BytePosition, Edit, Position};

    use super::CommentRange;
    use crate::selection::{SelectionKind, SelectionRange};

    /// A line comment on lines 3 to 5 of the right side.
    fn on_lines_3_to_5() -> CommentRange {
        let range = SelectionRange {
            start: Position { line: 3, column: 0 },
            end: Position { line: 5, column: 0 },
        };
        CommentRange {
            side: 1,
            kind: SelectionKind::Line,
            range,
        }
    }

    #[yare::parameterized(
        a_new_line_above_moves_them_down = { 1, BytePosition { line: 1, byte: 2 }, BytePosition { line: 1, byte: 2 }, BytePosition { line: 2, byte: 0 }, [4, 6] },
        a_line_opened_above_their_first_moves_them_down = { 1, BytePosition { line: 3, byte: 0 }, BytePosition { line: 3, byte: 0 }, BytePosition { line: 4, byte: 0 }, [4, 6] },
        a_new_line_within_their_first_moves_only_the_rest = { 1, BytePosition { line: 3, byte: 2 }, BytePosition { line: 3, byte: 2 }, BytePosition { line: 4, byte: 0 }, [3, 6] },
        joining_their_first_to_the_line_above_moves_them_up = { 1, BytePosition { line: 2, byte: 4 }, BytePosition { line: 3, byte: 0 }, BytePosition { line: 2, byte: 4 }, [2, 4] },
        deleting_lines_through_their_first_moves_it_to_the_start = { 1, BytePosition { line: 1, byte: 0 }, BytePosition { line: 4, byte: 1 }, BytePosition { line: 1, byte: 0 }, [1, 2] },
        typing_within_a_line_keeps_them = { 1, BytePosition { line: 4, byte: 1 }, BytePosition { line: 4, byte: 1 }, BytePosition { line: 4, byte: 2 }, [3, 5] },
        an_edit_after_them_keeps_them = { 1, BytePosition { line: 7, byte: 0 }, BytePosition { line: 7, byte: 0 }, BytePosition { line: 9, byte: 0 }, [3, 5] },
        an_edit_on_the_other_side_keeps_them = { 0, BytePosition { line: 1, byte: 0 }, BytePosition { line: 1, byte: 0 }, BytePosition { line: 2, byte: 0 }, [3, 5] },
    )]
    fn apply(
        side: usize,
        start: BytePosition,
        deleted_end: BytePosition,
        inserted_end: BytePosition,
        lines: [usize; 2],
    ) {
        let mut range = on_lines_3_to_5();
        let edit = Edit {
            start,
            deleted_end,
            inserted_end,
        };

        range.apply(side, &edit);

        assert_eq!([range.range.start.line, range.range.end.line], lines);
    }
}
