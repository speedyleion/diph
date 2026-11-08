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
    /// Whether the cursor sits between characters, before the one at its
    /// column, instead of on that character. Between characters it can
    /// also sit after a line's last character.
    between_characters: bool,
}

impl Cursor {
    /// Where the cursor is in `buffer`, with its column limited to the
    /// line's last column.
    pub fn position(&self, buffer: &Buffer) -> Position {
        Position {
            line: self.line,
            column: self.column.min(self.last_column(buffer)),
        }
    }

    /// Moves to `position` in `buffer`, stopping at its last line and the
    /// last column of the line.
    pub fn set(&mut self, buffer: &Buffer, position: Position) {
        self.go_to_line(buffer, position.line);
        self.column = position.column.min(self.last_column(buffer));
    }

    /// Places the cursor between characters or on one, at the column it
    /// shows.
    pub fn set_between_characters(&mut self, buffer: &Buffer, between: bool) {
        self.set(buffer, self.position(buffer));
        self.between_characters = between;
    }

    /// Moves `delta` characters right or left, stopping at either end of
    /// the line.
    pub fn move_column(&mut self, buffer: &Buffer, delta: isize) {
        let column = self.position(buffer).column.saturating_add_signed(delta);
        self.column = column.min(self.last_column(buffer));
    }

    /// Moves `delta` lines down or up, stopping at the first and last line.
    pub fn move_line(&mut self, buffer: &Buffer, delta: isize) {
        self.go_to_line(buffer, self.line.saturating_add_signed(delta));
    }

    /// Moves to `line`, or to the last line when `line` is past it.
    pub fn go_to_line(&mut self, buffer: &Buffer, line: usize) {
        self.line = line.min(buffer.lines().len().saturating_sub(1));
    }

    /// Moves to the first non-whitespace character on the current line.
    pub fn go_to_line_start(&mut self, buffer: &Buffer) {
        let line = &buffer.lines()[self.line];
        let column = line
            .chars()
            .position(|character| !character.is_whitespace())
            .unwrap_or(0);
        self.set(
            buffer,
            Position {
                line: self.line,
                column,
            },
        );
    }

    /// Moves past the last character on the current line.
    pub fn go_to_line_end(&mut self, buffer: &Buffer) {
        self.set(
            buffer,
            Position {
                line: self.line,
                column: usize::MAX,
            },
        );
    }

    /// The last column the cursor can take on its line in `buffer`.
    fn last_column(&self, buffer: &Buffer) -> usize {
        let chars = buffer
            .lines()
            .get(self.line)
            .map_or(0, |l| l.chars().count());
        if self.between_characters {
            chars
        } else {
            chars.saturating_sub(1)
        }
    }
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

    #[yare::parameterized(
        on_the_last_character = { "hello\n", 0, 4 },
        on_a_line_shorter_than_its_column = { "hello\nab\n", 1, 1 },
    )]
    fn placing_between_characters_keeps_the_column_shown(text: &str, delta: isize, column: usize) {
        let (mut cursor, buffer) = cursor_at(text, Position { line: 0, column: 4 });
        cursor.move_line(&buffer, delta);
        cursor.set_between_characters(&buffer, true);
        assert_eq!(cursor.position(&buffer).column, column);
    }

    #[test]
    fn between_characters_it_moves_past_the_last_one() {
        let (mut cursor, buffer) = cursor_at("hello\n", Position { line: 0, column: 4 });
        cursor.set_between_characters(&buffer, true);
        cursor.move_column(&buffer, 1);
        assert_eq!(cursor.position(&buffer).column, 5);
    }

    #[test]
    fn placed_on_a_character_from_past_the_last_it_moves_onto_the_last() {
        let (mut cursor, buffer) = cursor_at("hello\n", Position { line: 0, column: 4 });
        cursor.set_between_characters(&buffer, true);
        cursor.move_column(&buffer, 1);
        cursor.set_between_characters(&buffer, false);
        assert_eq!(cursor.position(&buffer).column, 4);
    }
}
