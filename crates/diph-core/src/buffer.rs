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

/// The lines of one side's text, without their line endings.
pub struct Buffer {
    lines: Vec<String>,
}

impl Buffer {
    /// Both LF and CRLF end a line, and a last line without a line ending
    /// still counts.
    pub fn new(text: &str) -> Self {
        let lines = text
            .split_inclusive('\n')
            .map(|l| l.trim_end_matches(['\n', '\r']).to_owned())
            .collect();
        Self { lines }
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Inserts `text` at `at`, splitting the line at each newline in
    /// `text`. `at` stops at the end of its line, a line past the last stops
    /// at the end of the buffer, and a buffer without lines gets one.
    /// Returns the position just past the inserted text.
    pub fn insert(&mut self, at: Position, text: &str) -> Position {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        let (line, byte) = self.byte_position(at);
        let tail = self.lines[line].split_off(byte);
        let mut pieces = text.split('\n');
        self.lines[line].push_str(pieces.next().unwrap_or_default());
        let new_lines: Vec<_> = pieces.map(str::to_owned).collect();
        let last = line + new_lines.len();
        self.lines.splice(line + 1..line + 1, new_lines);
        let column = self.lines[last].chars().count();
        self.lines[last].push_str(&tail);
        Position { line: last, column }
    }

    /// Removes the text from `range.start` up to `range.end`, joining their
    /// lines. Each end stops at the end of its line, and a line past the
    /// last stops at the end of the buffer. Removes nothing when `range.end`
    /// is not after `range.start`.
    pub fn delete(&mut self, range: Range<Position>) {
        if self.lines.is_empty() {
            return;
        }
        let (start_line, start_byte) = self.byte_position(range.start);
        let (end_line, end_byte) = self.byte_position(range.end);
        if (end_line, end_byte) <= (start_line, start_byte) {
            return;
        }
        let tail = self.lines[end_line].split_off(end_byte);
        self.lines[start_line].truncate(start_byte);
        self.lines[start_line].push_str(&tail);
        self.lines.drain(start_line + 1..=end_line);
    }

    /// The line of `position` and the byte index in it of `position`'s
    /// column, stopping at the line's end. A line past the last stops at the
    /// end of the buffer. The buffer must have lines.
    fn byte_position(&self, position: Position) -> (usize, usize) {
        let last = self.lines.len() - 1;
        if position.line > last {
            return (last, self.lines[last].len());
        }
        let text = &self.lines[position.line];
        let byte = text.char_indices().nth(position.column);
        (position.line, byte.map_or(text.len(), |(i, _)| i))
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
        on_one_line = { Position { line: 0, column: 2 }, "XY", Position { line: 0, column: 4 } },
        on_the_last_inserted_line = { Position { line: 0, column: 2 }, "a\nbc", Position { line: 1, column: 2 } },
        from_where_a_column_past_the_end_stopped = { Position { line: 0, column: 9 }, "!", Position { line: 0, column: 6 } },
    )]
    fn insert_returns_the_position_past_the_text(at: Position, inserted: &str, past: Position) {
        let mut buffer = Buffer::new("hello\nworld\n");
        assert_eq!(buffer.insert(at, inserted), past);
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
}
