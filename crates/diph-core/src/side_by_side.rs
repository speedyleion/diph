//! Aligns two texts line by line for a side-by-side view.

use std::ops::Range;

use similar::utils::TextDiffRemapper;
use similar::{ChangeTag, DiffTag, TextDiff};

use crate::Buffer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    Equal,
    /// The line exists only on the left side.
    LeftOnly,
    /// The line exists only on the right side.
    RightOnly,
    /// A left and a right line that differ, paired up inside a changed block.
    Changed,
}

/// One row of the side-by-side view: the left and right lines shown opposite
/// each other. A side is `None` where the other side has a line with nothing
/// opposite it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub kind: RowKind,
    pub left: Option<Line>,
    pub right: Option<Line>,
}

/// One side's line in a row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// Index of the line in its side's buffer.
    pub index: usize,
    /// For [`RowKind::Changed`] rows, byte ranges of the words that are not in
    /// the line opposite. Empty for other kinds.
    pub changed_words: Vec<Range<usize>>,
}

/// The rows of a side-by-side view of two buffers.
pub struct SideBySide {
    /// The left and right buffer, whose lines the rows name by index.
    buffers: [Buffer; 2],
    rows: Vec<Row>,
    /// Indices into `rows` where each run of differing rows starts.
    changes: Vec<usize>,
    /// For the left and right side, the index into `rows` of each line.
    line_rows: [Vec<usize>; 2],
}

impl SideBySide {
    /// The diff of the left and right `buffers`.
    pub fn new(buffers: [Buffer; 2]) -> Self {
        let [left, right] = &buffers;
        let left_lines: Vec<_> = left.lines().iter().map(String::as_str).collect();
        let right_lines: Vec<_> = right.lines().iter().map(String::as_str).collect();
        let diff = TextDiff::from_slices(&left_lines, &right_lines);

        let mut rows = Vec::new();
        let mut changes = Vec::new();
        for op in diff.ops() {
            let (tag, left, right) = op.as_tag_tuple();
            // similar merges neighboring deletes and inserts into one op, so
            // each differing op is a change of its own.
            if tag != DiffTag::Equal {
                changes.push(rows.len());
            }
            // Pair lines from the top of each block. In a changed block the
            // longer side's leftovers have nothing opposite them.
            for i in 0..left.len().max(right.len()) {
                let left = (i < left.len()).then_some(left.start + i);
                let right = (i < right.len()).then_some(right.start + i);
                let (kind, (left_words, right_words)) = match (tag, left, right) {
                    (DiffTag::Equal, ..) => (RowKind::Equal, Default::default()),
                    (_, Some(l), Some(r)) => (
                        RowKind::Changed,
                        word_changes(left_lines[l], right_lines[r]),
                    ),
                    (_, Some(_), None) => (RowKind::LeftOnly, Default::default()),
                    _ => (RowKind::RightOnly, Default::default()),
                };
                rows.push(Row {
                    kind,
                    left: left.map(|index| Line {
                        index,
                        changed_words: left_words,
                    }),
                    right: right.map(|index| Line {
                        index,
                        changed_words: right_words,
                    }),
                });
            }
        }

        let line_rows = [0, 1].map(|side| side_line_rows(&rows, side));
        Self {
            buffers,
            rows,
            changes,
            line_rows,
        }
    }

    pub fn buffers(&self) -> &[Buffer; 2] {
        &self.buffers
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Indices into [`Self::rows`] where each run of differing rows starts.
    pub fn changes(&self) -> &[usize] {
        &self.changes
    }

    /// Edits the buffer of `side`, 0 for the left and 1 for the right, with
    /// `edit`, then aligns the rows to the edited text. Returns what `edit`
    /// returns.
    pub fn edit<R>(&mut self, side: usize, edit: impl FnOnce(&mut Buffer) -> R) -> R {
        let edited = edit(&mut self.buffers[side]);
        *self = Self::new(std::mem::take(&mut self.buffers));
        edited
    }

    /// The index into `rows` of the `line`th line of `side`, 0 for the left
    /// side and 1 for the right.
    pub fn row_of(&self, side: usize, line: usize) -> usize {
        self.line_rows[side][line]
    }

    /// The first line of `side` in `row` or below it. When all of the side's
    /// lines are above `row`, its last line instead. `None` when `side` has
    /// no lines.
    pub fn line_at(&self, side: usize, row: usize) -> Option<usize> {
        let rows = &self.line_rows[side];
        let last = rows.len().checked_sub(1)?;
        Some(rows.partition_point(|&r| r < row).min(last))
    }
}

/// The index into `rows` of each of `side`'s lines, in order.
fn side_line_rows(rows: &[Row], side: usize) -> Vec<usize> {
    rows.iter()
        .enumerate()
        .filter(|(_, r)| [&r.left, &r.right][side].is_some())
        .map(|(row, _)| row)
        .collect()
}

/// Byte ranges of the words in `left` that are not in `right`, and of the
/// words in `right` that are not in `left`.
fn word_changes(left: &str, right: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    let diff = TextDiff::configure().diff_unicode_words(left, right);
    let remapper = TextDiffRemapper::from_text_diff(&diff, left, right);
    let slices: Vec<_> = diff
        .ops()
        .iter()
        .flat_map(|op| remapper.iter_slices(op))
        .collect();
    (
        ranges_in(left, &slices, ChangeTag::Delete),
        ranges_in(right, &slices, ChangeTag::Insert),
    )
}

/// Byte ranges in `line` of the slices tagged `tag`, which must borrow from
/// `line`.
fn ranges_in(line: &str, slices: &[(ChangeTag, &str)], tag: ChangeTag) -> Vec<Range<usize>> {
    slices
        .iter()
        .filter(|(t, _)| *t == tag)
        .filter_map(|(_, slice)| line.substr_range(slice).map(Into::into))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Position;
    use insta::assert_snapshot;

    /// Draws the rows of the diff of `left` and `right` as text: a kind
    /// marker, then each side's 1-based line number and text, with changed
    /// words in brackets.
    fn render(left: &str, right: &str) -> String {
        let diff = SideBySide::new([Buffer::new(left), Buffer::new(right)]);
        let [left, right] = diff.buffers();
        let mut out = String::new();
        for row in diff.rows() {
            let marker = match row.kind {
                RowKind::Equal => ' ',
                RowKind::LeftOnly => '<',
                RowKind::RightOnly => '>',
                RowKind::Changed => '~',
            };
            let left_side = render_side(left, row.left.as_ref());
            let right_side = render_side(right, row.right.as_ref());
            out += format!("{marker} {left_side:<26}| {right_side}").trim_end();
            out += "\n";
        }
        out += &format!("changes at {:?}", diff.changes());
        out
    }

    /// One side of a rendered row: the 1-based line number and the text
    /// from `buffer`, with the changed words in brackets. Empty if the side
    /// has no line.
    fn render_side(buffer: &Buffer, line: Option<&Line>) -> String {
        let Some(line) = line else {
            return String::new();
        };
        let mut text = buffer.lines()[line.index].clone();
        for r in line.changed_words.iter().rev() {
            text.insert(r.end, ']');
            text.insert(r.start, '[');
        }
        format!("{} {text}", line.index + 1)
    }

    #[test]
    fn pairs_replaced_lines_and_leaves_the_rest_unpaired() {
        let left = "keep\nlet x = 1;\nlet y = 2;\ngone\ntail\n";
        let right = "fresh\nkeep\nlet x = 10;\ntail\nextra\n";
        assert_snapshot!(render(left, right), @r"
        >                           | 1 fresh
          1 keep                    | 2 keep
        ~ 2 let x = [1];            | 3 let x = [10];
        < 3 let y = 2;              |
        < 4 gone                    |
          5 tail                    | 4 tail
        >                           | 5 extra
        changes at [0, 2, 6]
        ");
    }

    #[test]
    fn marks_changed_words_in_replaced_lines() {
        let left = "let total = a + b;\nfoo(one, two)\nsame\n";
        let right = "let sum = a + b + c;\nfoo(two)\nsame\n";
        assert_snapshot!(render(left, right), @r"
        ~ 1 let [total] = a + b;    | 1 let [sum] = a + b[ + c];
        ~ 2 foo([one, ]two)         | 2 foo(two)
          3 same                    | 3 same
        changes at [0]
        ");
    }

    #[test]
    fn keeps_dotted_names_as_one_word() {
        let left = "counts.entry(word.to_string())\n";
        let right = "counts.entry(word.to_lowercase())\n";
        assert_snapshot!(render(left, right), @r"
        ~ 1 counts.entry([word.to_string]())| 1 counts.entry([word.to_lowercase]())
        changes at [0]
        ");
    }

    #[test]
    fn marks_multibyte_words_on_char_boundaries() {
        assert_snapshot!(render("café ok\n", "naïve ok\n"), @r"
        ~ 1 [café] ok               | 1 [naïve] ok
        changes at [0]
        ");
    }

    #[test]
    fn ignores_line_ending_differences() {
        let diff = SideBySide::new([Buffer::new("a\r\nb\r\nc"), Buffer::new("a\nb\nc\n")]);
        assert!(diff.rows().iter().all(|r| r.kind == RowKind::Equal));
    }

    #[test]
    fn empty_left_side_is_all_right_only() {
        assert_snapshot!(render("", "a\nb"), @r"
        >                           | 1 a
        >                           | 2 b
        changes at [0]
        ");
    }

    /// The diff of four lines on the left against the first and last of
    /// them on the right, which has no line in rows 1 and 2.
    fn two_lines_deleted() -> SideBySide {
        SideBySide::new([Buffer::new("a\nb\nc\nd\n"), Buffer::new("a\nd\n")])
    }

    #[yare::parameterized(
        a_left_line_sits_in_its_own_row = { 0, 2, 2 },
        a_right_line_after_a_deletion_sits_below_it = { 1, 1, 3 },
    )]
    fn row_of(side: usize, line: usize, row: usize) {
        assert_eq!(two_lines_deleted().row_of(side, line), row);
    }

    #[yare::parameterized(
        a_row_with_a_line_has_that_line = { 0, 2, Some(2) },
        a_row_without_one_has_the_line_below = { 1, 1, Some(1) },
        a_row_past_the_last_line_has_the_last_line = { 0, 9, Some(3) },
    )]
    fn line_at(side: usize, row: usize, line: Option<usize>) {
        assert_eq!(two_lines_deleted().line_at(side, row), line);
    }

    #[test]
    fn a_side_without_lines_has_no_line_at_any_row() {
        let diff = SideBySide::new([Buffer::new(""), Buffer::new("a\n")]);
        assert_eq!(diff.line_at(0, 0), None);
    }

    /// The diff of four lines on the left against the first and last of
    /// them on the right, after inserting the middle two on the right.
    fn deleted_lines_put_back() -> SideBySide {
        let mut diff = two_lines_deleted();
        let at = Position { line: 1, column: 0 };
        diff.edit(1, |buffer| buffer.insert(at, "b\nc\n"));
        diff
    }

    #[test]
    fn edit_aligns_the_rows_to_the_edited_text() {
        let diff = deleted_lines_put_back();
        assert!(diff.rows().iter().all(|r| r.kind == RowKind::Equal));
    }

    #[test]
    fn edit_finds_the_edited_lines_rows() {
        assert_eq!(deleted_lines_put_back().row_of(1, 3), 3);
    }

    #[test]
    fn edit_returns_what_the_edit_returns() {
        let mut diff = two_lines_deleted();
        let at = Position { line: 0, column: 1 };
        let past = diff.edit(0, |buffer| buffer.insert(at, "!"));
        assert_eq!(past, Position { line: 0, column: 2 });
    }
}
