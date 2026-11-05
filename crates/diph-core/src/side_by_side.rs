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
    pub left: Option<Line>,
    pub right: Option<Line>,
}

/// One side's line in a row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// Index of the line in its text, counting from zero.
    pub index: usize,
    /// The line, without its line ending.
    pub text: String,
    /// For [`RowKind::Changed`] rows, byte ranges of the words that are not in
    /// the line opposite. Empty for other kinds.
    pub changed_words: Vec<Range<usize>>,
}

/// The rows of a side-by-side view. Each side's lines appear in order, so
/// walking the rows visits every line of each text once.
pub struct SideBySide {
    pub rows: Vec<Row>,
    /// Indices into `rows` where each run of differing rows starts.
    pub changes: Vec<usize>,
}

impl SideBySide {
    /// Line endings are not compared, so CRLF against LF, or a missing
    /// newline at the end of the file, does not mark lines as changed.
    pub fn new(left_text: &str, right_text: &str) -> Self {
        let left_lines = split_lines(left_text);
        let right_lines = split_lines(right_text);
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
                    left: left.map(|l| line(&left_lines, l, left_words)),
                    right: right.map(|r| line(&right_lines, r, right_words)),
                });
            }
        }

        Self { rows, changes }
    }
}

/// The `index`th of `lines`, with its `changed_words`.
fn line(lines: &[&str], index: usize, changed_words: Vec<Range<usize>>) -> Line {
    Line {
        index,
        text: lines[index].to_owned(),
        changed_words,
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

/// Splits into lines without their line endings.
fn split_lines(text: &str) -> Vec<&str> {
    text.split_inclusive('\n')
        .map(|l| l.trim_end_matches(['\n', '\r']))
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
            let left = render_side(row.left.as_ref());
            let right = render_side(row.right.as_ref());
            out += format!("{marker} {left:<26}| {right}").trim_end();
            out += "\n";
        }
        out += &format!("changes at {:?}", diff.changes);
        out
    }

    /// One side of a rendered row: the 1-based line number and text, with
    /// the changed words in brackets. Empty if the side has no line.
    fn render_side(line: Option<&Line>) -> String {
        let Some(line) = line else {
            return String::new();
        };
        let mut text = line.text.clone();
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
        assert_snapshot!(render(&SideBySide::new(left, right)), @r"
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
        assert_snapshot!(render(&SideBySide::new(left, right)), @r"
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
        assert_snapshot!(render(&SideBySide::new(left, right)), @r"
        ~ 1 counts.entry([word.to_string]())| 1 counts.entry([word.to_lowercase]())
        changes at [0]
        ");
    }

    #[test]
    fn marks_multibyte_words_on_char_boundaries() {
        let diff = SideBySide::new("café ok\n", "naïve ok\n");
        assert_snapshot!(render(&diff), @r"
        ~ 1 [café] ok               | 1 [naïve] ok
        changes at [0]
        ");
    }

    #[test]
    fn keeps_tabs_and_strips_crlf() {
        let left = "\tindented\r\nsame\r\n";
        let right = "\tindented!\r\nsame\r\n";
        let diff = SideBySide::new(left, right);
        let left: Vec<_> = diff.rows.iter().filter_map(|r| r.left.as_ref()).collect();
        let right: Vec<_> = diff.rows.iter().filter_map(|r| r.right.as_ref()).collect();
        assert_eq!(
            left.iter().map(|l| &l.text).collect::<Vec<_>>(),
            ["\tindented", "same"]
        );
        assert_eq!(
            right.iter().map(|l| &l.text).collect::<Vec<_>>(),
            ["\tindented!", "same"]
        );
    }

    #[test]
    fn ignores_line_ending_differences() {
        let diff = SideBySide::new("a\r\nb\r\nc", "a\nb\nc\n");
        assert!(diff.rows.iter().all(|r| r.kind == RowKind::Equal));
    }

    #[test]
    fn empty_left_side_is_all_right_only() {
        assert_snapshot!(render(&SideBySide::new("", "a\nb")), @r"
        >                           | 1 a
        >                           | 2 b
        changes at [0]
        ");
    }
}
