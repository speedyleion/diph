//! The text of one side of a diff.

use std::ops::Range;

/// A character in a buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    /// Index of the line in the buffer.
    pub line: usize,
    /// Index of the character in its line.
    pub column: usize,
}

/// A place in a buffer, as a line and a byte index in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BytePosition {
    pub line: usize,
    pub byte: usize,
}

/// A change to a buffer: the text from `start` up to `deleted_end` was
/// replaced by text that ends at `inserted_end`. `deleted_end` is where
/// the deleted text ended before the change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edit {
    pub start: BytePosition,
    pub deleted_end: BytePosition,
    pub inserted_end: BytePosition,
}

/// The lines of one side's text, without their line endings.
#[derive(Default)]
pub struct Buffer {
    lines: Vec<String>,
    /// Whether lines end in CRLF rather than LF.
    crlf: bool,
    /// Whether the last line has a line ending.
    ends_with_line_ending: bool,
}

impl Buffer {
    /// Both LF and CRLF end a line, and a last line without a line ending
    /// still counts.
    pub fn new(text: &str) -> Self {
        let lines: Vec<_> = text.split_inclusive('\n').collect();
        Self {
            crlf: lines.first().is_some_and(|l| l.ends_with("\r\n")),
            ends_with_line_ending: text.ends_with('\n'),
            lines: lines
                .iter()
                .map(|l| l.trim_end_matches(['\n', '\r']).to_owned())
                .collect(),
        }
    }

    /// The buffer's text, with each line ending in the line ending of the
    /// text's first line, and the last line ending in one if the text's
    /// last line did.
    pub fn text(&self) -> String {
        let line_ending = if self.crlf { "\r\n" } else { "\n" };
        let mut text = self.lines.join(line_ending);
        if self.ends_with_line_ending {
            text.push_str(line_ending);
        }
        text
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Inserts `text` at `at`, splitting the line at each newline in
    /// `text`. `at` stops at the end of its line, a line past the last stops
    /// at the end of the buffer, and a buffer without lines gets one.
    pub fn insert(&mut self, at: Position, text: &str) -> Edit {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        let start = self.byte_position(at);
        let tail = self.lines[start.line].split_off(start.byte);
        let mut pieces = text.split('\n');
        self.lines[start.line].push_str(pieces.next().unwrap_or_default());
        let new_lines: Vec<_> = pieces.map(str::to_owned).collect();
        let last = start.line + new_lines.len();
        self.lines.splice(start.line + 1..start.line + 1, new_lines);
        let inserted_end = BytePosition {
            line: last,
            byte: self.lines[last].len(),
        };
        self.lines[last].push_str(&tail);
        Edit {
            start,
            deleted_end: start,
            inserted_end,
        }
    }

    /// Removes the text from `range.start` up to `range.end`, joining their
    /// lines. Each end stops at the end of its line, and a line past the
    /// last stops at the end of the buffer. Removes nothing when `range.end`
    /// is not after `range.start`.
    pub fn delete(&mut self, range: Range<Position>) -> Edit {
        let start = self.byte_position(range.start);
        let unchanged = Edit {
            start,
            deleted_end: start,
            inserted_end: start,
        };
        if self.lines.is_empty() {
            return unchanged;
        }
        let end = self.byte_position(range.end);
        if end <= start {
            return unchanged;
        }
        let tail = self.lines[end.line].split_off(end.byte);
        self.lines[start.line].truncate(start.byte);
        self.lines[start.line].push_str(&tail);
        self.lines.drain(start.line + 1..=end.line);
        Edit {
            deleted_end: end,
            ..unchanged
        }
    }

    /// The character position of `at`, which must be in the buffer.
    pub fn position(&self, at: BytePosition) -> Position {
        let column = self.lines[at.line][..at.byte].chars().count();
        Position {
            line: at.line,
            column,
        }
    }

    /// Where `position` is in bytes, stopping at the end of its line. A line
    /// past the last stops at the end of the buffer, and a buffer without
    /// lines has only the start.
    fn byte_position(&self, position: Position) -> BytePosition {
        let Some(last) = self.lines.len().checked_sub(1) else {
            return BytePosition { line: 0, byte: 0 };
        };
        if position.line > last {
            let byte = self.lines[last].len();
            return BytePosition { line: last, byte };
        }
        let text = &self.lines[position.line];
        let byte = text.char_indices().nth(position.column);
        let byte = byte.map_or(text.len(), |(i, _)| i);
        BytePosition {
            line: position.line,
            byte,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[yare::parameterized(
        lf_ends_a_line = { "a\nb\n", &["a", "b"] },
        crlf_ends_a_line = { "a\r\nb\r\n", &["a", "b"] },
        a_last_line_without_an_ending_counts = { "a\nb", &["a", "b"] },
        empty_text_has_no_lines = { "", &[] },
        tabs_are_kept = { "\tindented\n", &["\tindented"] },
    )]
    fn splits_lines(text: &str, lines: &[&str]) {
        assert_eq!(Buffer::new(text).lines(), lines);
    }

    #[yare::parameterized(
        in_the_middle_of_a_line = { "hello\nworld\n", Position { line: 0, column: 2 }, "XY", &["heXYllo", "world"] },
        at_the_end_of_a_line = { "hello\nworld\n", Position { line: 0, column: 5 }, "!", &["hello!", "world"] },
        past_the_end_of_a_line_at_its_end = { "hello\nworld\n", Position { line: 0, column: 9 }, "!", &["hello!", "world"] },
        past_the_last_line_at_the_end_of_the_buffer = { "hello\nworld\n", Position { line: 9, column: 0 }, ">", &["hello", "world>"] },
        a_newline_splits_the_line = { "hello\nworld\n", Position { line: 0, column: 2 }, "\n", &["he", "llo", "world"] },
        several_lines_between_two = { "hello\nworld\n", Position { line: 1, column: 0 }, "a\nb\n", &["hello", "a", "b", "world"] },
        at_a_column_counted_in_characters = { "héllo\n", Position { line: 0, column: 2 }, "-", &["hé-llo"] },
        into_a_buffer_without_lines = { "", Position { line: 0, column: 0 }, "a", &["a"] },
    )]
    fn insert(text: &str, at: Position, inserted: &str, lines: &[&str]) {
        let mut buffer = Buffer::new(text);
        buffer.insert(at, inserted);
        assert_eq!(buffer.lines(), lines);
    }

    #[yare::parameterized(
        on_one_line = { "hello\n", Position { line: 0, column: 2 }, "XY", BytePosition { line: 0, byte: 2 }, BytePosition { line: 0, byte: 4 } },
        ending_on_the_last_inserted_line = { "hello\n", Position { line: 0, column: 2 }, "a\nbc", BytePosition { line: 0, byte: 2 }, BytePosition { line: 1, byte: 2 } },
        from_where_a_column_past_the_end_stopped = { "hello\n", Position { line: 0, column: 9 }, "!", BytePosition { line: 0, byte: 5 }, BytePosition { line: 0, byte: 6 } },
        in_bytes = { "héllo\n", Position { line: 0, column: 2 }, "é", BytePosition { line: 0, byte: 3 }, BytePosition { line: 0, byte: 5 } },
    )]
    fn insert_reports_the_change(
        text: &str,
        at: Position,
        inserted: &str,
        start: BytePosition,
        inserted_end: BytePosition,
    ) {
        let mut buffer = Buffer::new(text);
        let edit = buffer.insert(at, inserted);
        let deleted_end = start;
        assert_eq!(
            edit,
            Edit {
                start,
                deleted_end,
                inserted_end
            }
        );
    }

    #[yare::parameterized(
        within_a_line = { "hello\nworld\n", Position { line: 0, column: 1 }..Position { line: 0, column: 3 }, &["hlo", "world"] },
        across_lines_joining_them = { "hello\nworld\n", Position { line: 0, column: 3 }..Position { line: 1, column: 2 }, &["helrld"] },
        a_line_break_joining_its_lines = { "hello\nworld\n", Position { line: 0, column: 5 }..Position { line: 1, column: 0 }, &["helloworld"] },
        up_to_the_end_of_the_line_past_it = { "hello\nworld\n", Position { line: 0, column: 3 }..Position { line: 0, column: 9 }, &["hel", "world"] },
        up_to_the_end_of_the_buffer_past_its_last_line = { "hello\nworld\n", Position { line: 0, column: 3 }..Position { line: 9, column: 0 }, &["hel"] },
        nothing_when_the_end_is_before_the_start = { "hello\nworld\n", Position { line: 0, column: 3 }..Position { line: 0, column: 1 }, &["hello", "world"] },
        columns_counted_in_characters = { "héllo\n", Position { line: 0, column: 1 }..Position { line: 0, column: 2 }, &["hllo"] },
        nothing_from_a_buffer_without_lines = { "", Position { line: 0, column: 0 }..Position { line: 0, column: 1 }, &[] },
    )]
    fn delete(text: &str, range: Range<Position>, lines: &[&str]) {
        let mut buffer = Buffer::new(text);
        buffer.delete(range);
        assert_eq!(buffer.lines(), lines);
    }

    #[yare::parameterized(
        within_a_line = { "hello\n", Position { line: 0, column: 1 }..Position { line: 0, column: 3 }, BytePosition { line: 0, byte: 1 }, BytePosition { line: 0, byte: 3 } },
        across_lines = { "hello\nworld\n", Position { line: 0, column: 3 }..Position { line: 1, column: 2 }, BytePosition { line: 0, byte: 3 }, BytePosition { line: 1, byte: 2 } },
        in_bytes = { "héllo\n", Position { line: 0, column: 1 }..Position { line: 0, column: 2 }, BytePosition { line: 0, byte: 1 }, BytePosition { line: 0, byte: 3 } },
        nothing_when_the_end_is_before_the_start = { "hello\n", Position { line: 0, column: 3 }..Position { line: 0, column: 1 }, BytePosition { line: 0, byte: 3 }, BytePosition { line: 0, byte: 3 } },
    )]
    fn delete_reports_the_change(
        text: &str,
        range: Range<Position>,
        start: BytePosition,
        deleted_end: BytePosition,
    ) {
        let mut buffer = Buffer::new(text);
        let edit = buffer.delete(range);
        let inserted_end = start;
        assert_eq!(
            edit,
            Edit {
                start,
                deleted_end,
                inserted_end
            }
        );
    }

    #[test]
    fn position_counts_the_characters_before_a_byte() {
        let buffer = Buffer::new("héllo\n");
        let at = BytePosition { line: 0, byte: 3 };
        assert_eq!(buffer.position(at), Position { line: 0, column: 2 });
    }

    #[yare::parameterized(
        lf = { "a\nb\n" },
        crlf = { "a\r\nb\r\n" },
        without_an_ending_on_the_last_line = { "a\nb" },
        empty = { "" },
    )]
    fn text_is_the_text_read(text: &str) {
        assert_eq!(Buffer::new(text).text(), text);
    }

    #[test]
    fn inserted_lines_end_like_the_others() {
        let mut buffer = Buffer::new("a\r\nb\r\n");
        buffer.insert(Position { line: 0, column: 1 }, "\nc");
        assert_eq!(buffer.text(), "a\r\nc\r\nb\r\n");
    }
}
