//! Aligns two texts line by line for a side-by-side view.

use std::ops::Range;

use similar::utils::TextDiffRemapper;
use similar::{ChangeTag, DiffTag, TextDiff};

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
    /// Index of the line in the left text, counting from zero.
    pub left: Option<usize>,
    /// Index of the line in the right text, counting from zero.
    pub right: Option<usize>,
    /// For [`RowKind::Changed`] rows, byte ranges in the left line of the
    /// words that are not in the right line. Empty for other kinds.
    pub left_changed_words: Vec<Range<usize>>,
    /// For [`RowKind::Changed`] rows, byte ranges in the right line of the
    /// words that are not in the left line. Empty for other kinds.
    pub right_changed_words: Vec<Range<usize>>,
}

pub struct SideBySide {
    /// The left text's lines, without line endings.
    pub left_lines: Vec<String>,
    /// The right text's lines, without line endings.
    pub right_lines: Vec<String>,
    pub rows: Vec<Row>,
}

impl SideBySide {
    pub fn new(left_text: &str, right_text: &str) -> Self {
        let diff = TextDiff::from_lines(left_text, right_text);
        let left_lines = split_lines(left_text);
        let right_lines = split_lines(right_text);

        let mut rows = Vec::new();
        for op in diff.ops() {
            let (tag, left, right) = op.as_tag_tuple();
            // Pair lines from the top of each block. In a changed block the
            // longer side's leftovers have nothing opposite them.
            for i in 0..left.len().max(right.len()) {
                let left = (i < left.len()).then_some(left.start + i);
                let right = (i < right.len()).then_some(right.start + i);
                let (kind, (left_changed_words, right_changed_words)) = match (tag, left, right) {
                    (DiffTag::Equal, ..) => (RowKind::Equal, Default::default()),
                    (_, Some(l), Some(r)) => (
                        RowKind::Changed,
                        word_changes(&left_lines[l], &right_lines[r]),
                    ),
                    (_, Some(_), None) => (RowKind::LeftOnly, Default::default()),
                    _ => (RowKind::RightOnly, Default::default()),
                };
                rows.push(Row {
                    kind,
                    left,
                    right,
                    left_changed_words,
                    right_changed_words,
                });
            }
        }

        Self {
            left_lines,
            right_lines,
            rows,
        }
    }
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

/// Splits like [`TextDiff::from_lines`] does, so line indices match, but
/// drops the line endings.
fn split_lines(text: &str) -> Vec<String> {
    text.split_inclusive('\n')
        .map(|l| l.trim_end_matches(['\n', '\r']).to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    /// Draws the rows as text: a kind marker, then each side's 1-based line
    /// number and text, with changed words in brackets.
    fn render(diff: &SideBySide) -> String {
        let mut out = String::new();
        for row in &diff.rows {
            let marker = match row.kind {
                RowKind::Equal => ' ',
                RowKind::LeftOnly => '<',
                RowKind::RightOnly => '>',
                RowKind::Changed => '~',
            };
            let left = render_side(&diff.left_lines, row.left, &row.left_changed_words);
            let right = render_side(&diff.right_lines, row.right, &row.right_changed_words);
            out += format!("{marker} {left:<26}| {right}").trim_end();
            out += "\n";
        }
        out
    }

    /// One side of a rendered row: the 1-based line number and text, with
    /// the `changed` byte ranges in brackets. Empty if the side has no line.
    fn render_side(lines: &[String], line: Option<usize>, changed: &[Range<usize>]) -> String {
        let Some(l) = line else {
            return String::new();
        };
        let mut text = lines[l].clone();
        for r in changed.iter().rev() {
            text.insert(r.end, ']');
            text.insert(r.start, '[');
        }
        format!("{} {text}", l + 1)
    }

    #[test]
    fn pairs_replaced_lines_and_leaves_the_rest_unpaired() {
        let left = "keep\nlet x = 1;\nlet y = 2;\ngone\ntail\n";
        let right = "fresh\nkeep\nlet x = 10;\ntail\nextra\n";
        assert_snapshot!(render(&SideBySide::new(left, right)), @r"
        >                           | 1 fresh
          1 keep                    | 2 keep
        ~ 2 let x = [1];            | 3 let x = [10];
        < 3 let y = 2;              |
        < 4 gone                    |
          5 tail                    | 4 tail
        >                           | 5 extra
        ");
    }

    #[test]
    fn marks_changed_words_in_replaced_lines() {
        let left = "let total = a + b;\nfoo(one, two)\nsame\n";
        let right = "let sum = a + b + c;\nfoo(two)\nsame\n";
        assert_snapshot!(render(&SideBySide::new(left, right)), @r"
        ~ 1 let [total] = a + b;    | 1 let [sum] = a + b[ + c];
        ~ 2 foo([one, ]two)         | 2 foo(two)
          3 same                    | 3 same
        ");
    }

    #[test]
    fn keeps_dotted_names_as_one_word() {
        let left = "counts.entry(word.to_string())\n";
        let right = "counts.entry(word.to_lowercase())\n";
        assert_snapshot!(render(&SideBySide::new(left, right)), @r"
        ~ 1 counts.entry([word.to_string]())| 1 counts.entry([word.to_lowercase]())
        ");
    }

    #[test]
    fn marks_multibyte_words_on_char_boundaries() {
        let diff = SideBySide::new("café ok\n", "naïve ok\n");
        assert_snapshot!(render(&diff), @r"
        ~ 1 [café] ok               | 1 [naïve] ok
        ");
    }

    #[test]
    fn keeps_tabs_and_strips_crlf() {
        let left = "\tindented\r\nsame\r\n";
        let right = "\tindented!\r\nsame\r\n";
        let diff = SideBySide::new(left, right);
        assert_eq!(diff.left_lines, ["\tindented", "same"]);
        assert_eq!(diff.right_lines, ["\tindented!", "same"]);
    }

    #[test]
    fn empty_left_side_is_all_right_only() {
        assert_snapshot!(render(&SideBySide::new("", "a\nb")), @r"
        >                           | 1 a
        >                           | 2 b
        ");
    }
}
