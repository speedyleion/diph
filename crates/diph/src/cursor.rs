//! The cursor: a character in a buffer.

use diph_core::{Buffer, Position};

/// A cursor on a character of a buffer, which each call names. It keeps
/// its column across lines too short to reach it. In a buffer without
/// lines it stays at the start.
#[derive(Default)]
pub struct Cursor {
    line: usize,
    /// The column to show on lines long enough to reach it.
    column: usize,
}

impl Cursor {
    /// Where the cursor is in `buffer`, with its column limited to the
    /// line's last character.
    pub fn position(&self, buffer: &Buffer) -> Position {
        Position {
            line: self.line,
            column: self.column.min(last_column(buffer, self.line)),
        }
    }

    /// Moves to `position` in `buffer`, stopping at its last line and the
    /// last character of the line.
    pub fn set(&mut self, buffer: &Buffer, position: Position) {
        self.go_to_line(buffer, position.line);
        self.column = position.column.min(last_column(buffer, self.line));
    }

    /// Moves `delta` characters right or left, stopping at either end of
    /// the line.
    pub fn move_column(&mut self, buffer: &Buffer, delta: isize) {
        let column = self.position(buffer).column.saturating_add_signed(delta);
        self.column = column.min(last_column(buffer, self.line));
    }

    /// Moves `delta` lines down or up, stopping at the first and last line.
    pub fn move_line(&mut self, buffer: &Buffer, delta: isize) {
        self.go_to_line(buffer, self.line.saturating_add_signed(delta));
    }

    /// Moves to `line`, or to the last line when `line` is past it.
    pub fn go_to_line(&mut self, buffer: &Buffer, line: usize) {
        self.line = line.min(buffer.lines().len().saturating_sub(1));
    }
}

/// The column of the last character of `line` in `buffer`, or 0 when the
/// line is empty or missing.
fn last_column(buffer: &Buffer, line: usize) -> usize {
    let chars = buffer.lines().get(line).map_or(0, |l| l.chars().count());
    chars.saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A buffer of `text`, and a cursor on it moved to `start`.
    fn cursor_at(text: &str, start: Position) -> (Cursor, Buffer) {
        let buffer = Buffer::new(text);
        let mut cursor = Cursor::default();
        cursor.set(&buffer, start);
        (cursor, buffer)
    }

    #[test]
    fn set_stops_at_the_last_line() {
        let (cursor, buffer) = cursor_at("hello\nab\n", Position { line: 9, column: 0 });
        assert_eq!(cursor.position(&buffer), Position { line: 1, column: 0 });
    }

    #[test]
    fn set_keeps_a_column_past_the_end_as_the_last_character() {
        let (mut cursor, buffer) = cursor_at("hello\nab\n", Position { line: 1, column: 9 });
        cursor.move_line(&buffer, -1);
        assert_eq!(cursor.position(&buffer).column, 1);
    }

    #[test]
    fn stays_at_the_start_of_a_buffer_without_lines() {
        let (mut cursor, buffer) = cursor_at("", Position { line: 0, column: 0 });
        cursor.move_line(&buffer, 1);
        cursor.move_column(&buffer, 1);
        assert_eq!(cursor.position(&buffer), Position { line: 0, column: 0 });
    }

    #[yare::parameterized(
        right_moves_a_character = { 1, 3 },
        left_moves_a_character = { -1, 1 },
        right_stops_at_the_last_character = { 9, 4 },
        left_stops_at_the_first_character = { -9, 0 },
    )]
    fn move_column(delta: isize, column: usize) {
        let (mut cursor, buffer) = cursor_at("hello\n", Position { line: 0, column: 2 });
        cursor.move_column(&buffer, delta);
        assert_eq!(cursor.position(&buffer).column, column);
    }

    #[yare::parameterized(
        down_moves_a_line = { 1, 2 },
        up_moves_a_line = { -1, 0 },
        down_stops_at_the_last_line = { 9, 3 },
        up_stops_at_the_first_line = { -9, 0 },
    )]
    fn move_line(delta: isize, line: usize) {
        let (mut cursor, buffer) = cursor_at("a\nb\nc\nd\n", Position { line: 1, column: 0 });
        cursor.move_line(&buffer, delta);
        assert_eq!(cursor.position(&buffer).line, line);
    }

    #[yare::parameterized(
        a_shorter_line_shows_its_last_character = { 1, 1 },
        an_empty_line_shows_the_first_column = { 2, 0 },
        a_long_enough_line_gets_the_column_back = { 3, 4 },
    )]
    fn the_column_is_kept_across_lines(delta: isize, column: usize) {
        let text = "hello\nab\n\nworld\n";
        let (mut cursor, buffer) = cursor_at(text, Position { line: 0, column: 4 });
        cursor.move_line(&buffer, delta);
        assert_eq!(cursor.position(&buffer).column, column);
    }

    #[test]
    fn moving_on_a_short_line_starts_from_the_column_shown() {
        let (mut cursor, buffer) = cursor_at("hello\nab\n", Position { line: 0, column: 4 });
        cursor.move_line(&buffer, 1);
        cursor.move_column(&buffer, -1);
        assert_eq!(cursor.position(&buffer).column, 0);
    }
}
