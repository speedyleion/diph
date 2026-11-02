//! Aligns two texts line by line for a side-by-side view.

use similar::{DiffTag, TextDiff};

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
                let kind = match (tag, old, new) {
                    (DiffTag::Equal, ..) => RowKind::Equal,
                    (_, Some(_), Some(_)) => RowKind::Replace,
                    (_, Some(_), None) => RowKind::Delete,
                    _ => RowKind::Insert,
                };
                rows.push(Row { kind, old, new });
            }
        }

        Self {
            old_lines: split_lines(old_text),
            new_lines: split_lines(new_text),
            rows,
            added,
            removed,
        }
    }
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
    /// number and text.
    fn render(diff: &SideBySide) -> String {
        let side = |lines: &[String], line: Option<usize>| {
            line.map_or(String::new(), |l| format!("{} {}", l + 1, lines[l]))
        };
        let mut out = String::new();
        for row in &diff.rows {
            let marker = match row.kind {
                RowKind::Equal => ' ',
                RowKind::Delete => '-',
                RowKind::Insert => '+',
                RowKind::Replace => '~',
            };
            let old = side(&diff.old_lines, row.old);
            let new = side(&diff.new_lines, row.new);
            out += format!("{marker} {old:<26}| {new}").trim_end();
            out += "\n";
        }
        out += &format!("+{} -{}", diff.added, diff.removed);
        out
    }

    #[test]
    fn pairs_replaced_lines_and_leaves_the_rest_unpaired() {
        let old = "keep\nlet x = 1;\nlet y = 2;\ngone\ntail\n";
        let new = "fresh\nkeep\nlet x = 10;\ntail\nextra\n";
        assert_snapshot!(render(&SideBySide::new(old, new)), @r"
        +                           | 1 fresh
          1 keep                    | 2 keep
        ~ 2 let x = 1;              | 3 let x = 10;
        - 3 let y = 2;              |
        - 4 gone                    |
          5 tail                    | 4 tail
        +                           | 5 extra
        +3 -3
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
