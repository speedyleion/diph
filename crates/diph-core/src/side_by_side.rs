//! Aligns two texts line by line for a side-by-side view.

use std::ops::Range;

use similar::utils::TextDiffRemapper;
use similar::{ChangeTag, DiffTag, TextDiff};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    Equal,
    /// The line exists only on the old side.
    Delete,
    /// The line exists only on the new side.
    Insert,
    /// An old and a new line paired up inside a replaced block.
    Replace,
}

/// One row of the side-by-side view: the old and new lines shown opposite
/// each other. A side is `None` where the other side has a line with nothing
/// opposite it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub kind: RowKind,
    /// Index of the line in the old text, counting from zero.
    pub old: Option<usize>,
    /// Index of the line in the new text, counting from zero.
    pub new: Option<usize>,
    /// For [`RowKind::Replace`] rows, byte ranges in the old line of the
    /// words that are not in the new line. Empty for other kinds.
    pub deleted_words: Vec<Range<usize>>,
    /// For [`RowKind::Replace`] rows, byte ranges in the new line of the
    /// words that are not in the old line. Empty for other kinds.
    pub added_words: Vec<Range<usize>>,
}

pub struct SideBySide {
    /// The old text's lines, without line endings.
    pub old_lines: Vec<String>,
    /// The new text's lines, without line endings.
    pub new_lines: Vec<String>,
    pub rows: Vec<Row>,
    /// Number of lines only in the new text, like git's insertions count.
    pub added: usize,
    /// Number of lines only in the old text, like git's deletions count.
    pub removed: usize,
}

impl SideBySide {
    pub fn new(old_text: &str, new_text: &str) -> Self {
        let diff = TextDiff::from_lines(old_text, new_text);
        let old_lines = split_lines(old_text);
        let new_lines = split_lines(new_text);

        let mut rows = Vec::new();
        let (mut added, mut removed) = (0, 0);
        for op in diff.ops() {
            let (tag, old, new) = op.as_tag_tuple();
            if tag != DiffTag::Equal {
                removed += old.len();
                added += new.len();
            }
            // Pair lines from the top of each block. In a replaced block the
            // longer side's leftovers become plain deletes or inserts.
            for i in 0..old.len().max(new.len()) {
                let old = (i < old.len()).then_some(old.start + i);
                let new = (i < new.len()).then_some(new.start + i);
                let (kind, (deleted_words, added_words)) = match (tag, old, new) {
                    (DiffTag::Equal, ..) => (RowKind::Equal, Default::default()),
                    (_, Some(o), Some(n)) => {
                        (RowKind::Replace, word_changes(&old_lines[o], &new_lines[n]))
                    }
                    (_, Some(_), None) => (RowKind::Delete, Default::default()),
                    _ => (RowKind::Insert, Default::default()),
                };
                rows.push(Row {
                    kind,
                    old,
                    new,
                    deleted_words,
                    added_words,
                });
            }
        }

        Self {
            old_lines,
            new_lines,
            rows,
            added,
            removed,
        }
    }
}

/// Byte ranges of the words deleted from `old` and added in `new`.
fn word_changes(old: &str, new: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    let diff = TextDiff::configure().diff_unicode_words(old, new);
    let remapper = TextDiffRemapper::from_text_diff(&diff, old, new);
    let slices: Vec<_> = diff
        .ops()
        .iter()
        .flat_map(|op| remapper.iter_slices(op))
        .collect();
    (
        ranges_in(old, &slices, ChangeTag::Delete),
        ranges_in(new, &slices, ChangeTag::Insert),
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
                RowKind::Delete => '-',
                RowKind::Insert => '+',
                RowKind::Replace => '~',
            };
            let old = render_side(&diff.old_lines, row.old, &row.deleted_words);
            let new = render_side(&diff.new_lines, row.new, &row.added_words);
            out += format!("{marker} {old:<26}| {new}").trim_end();
            out += "\n";
        }
        out += &format!("+{} -{}", diff.added, diff.removed);
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
        let old = "keep\nlet x = 1;\nlet y = 2;\ngone\ntail\n";
        let new = "fresh\nkeep\nlet x = 10;\ntail\nextra\n";
        assert_snapshot!(render(&SideBySide::new(old, new)), @r"
        +                           | 1 fresh
          1 keep                    | 2 keep
        ~ 2 let x = [1];            | 3 let x = [10];
        - 3 let y = 2;              |
        - 4 gone                    |
          5 tail                    | 4 tail
        +                           | 5 extra
        +3 -3
        ");
    }

    #[test]
    fn marks_changed_words_in_replaced_lines() {
        let old = "let total = a + b;\nfoo(one, two)\nsame\n";
        let new = "let sum = a + b + c;\nfoo(two)\nsame\n";
        assert_snapshot!(render(&SideBySide::new(old, new)), @r"
        ~ 1 let [total] = a + b;    | 1 let [sum] = a + b[ + c];
        ~ 2 foo([one, ]two)         | 2 foo(two)
          3 same                    | 3 same
        +2 -2
        ");
    }

    #[test]
    fn keeps_dotted_names_as_one_word() {
        let old = "counts.entry(word.to_string())\n";
        let new = "counts.entry(word.to_lowercase())\n";
        assert_snapshot!(render(&SideBySide::new(old, new)), @r"
        ~ 1 counts.entry([word.to_string]())| 1 counts.entry([word.to_lowercase]())
        +1 -1
        ");
    }

    #[test]
    fn marks_multibyte_words_on_char_boundaries() {
        let diff = SideBySide::new("café ok\n", "naïve ok\n");
        assert_snapshot!(render(&diff), @r"
        ~ 1 [café] ok               | 1 [naïve] ok
        +1 -1
        ");
    }

    #[test]
    fn keeps_tabs_and_strips_crlf() {
        let old = "\tindented\r\nsame\r\n";
        let new = "\tindented!\r\nsame\r\n";
        let diff = SideBySide::new(old, new);
        assert_eq!(diff.old_lines, ["\tindented", "same"]);
        assert_eq!(diff.new_lines, ["\tindented!", "same"]);
    }

    #[test]
    fn empty_old_side_is_all_inserts() {
        assert_snapshot!(render(&SideBySide::new("", "a\nb")), @r"
        +                           | 1 a
        +                           | 2 b
        +2 -0
        ");
    }
}
