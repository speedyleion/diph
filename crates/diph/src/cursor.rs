//! The cursor: a character on one side of a diff.

use diph_core::SideBySide;

/// A character on one side of a diff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    /// 0 for the left side, 1 for the right.
    pub side: usize,
    /// Index of the line in its side's text.
    pub line: usize,
    /// Index of the character in its line.
    pub column: usize,
}

/// Where one side's line sits in a diff.
struct LinePlace {
    /// Index into the diff's rows.
    row: usize,
    /// Characters in the line.
    chars: usize,
}

/// A cursor on a character of one side of a diff. It moves within the
/// side's lines, skipping the rows where the side has none, and it keeps
/// its column across lines too short to reach it.
pub struct Cursor {
    /// The lines of the left and right side, in order.
    lines: [Vec<LinePlace>; 2],
    side: usize,
    line: usize,
    /// The column to show on lines long enough to reach it.
    column: usize,
}

impl Cursor {
    /// A cursor at the start of the right side's first line, or of the left
    /// side's when the right side has none. `None` when neither side has
    /// lines.
    pub fn new(diff: &SideBySide) -> Option<Self> {
        let lines = [side_lines(diff, 0), side_lines(diff, 1)];
        let side = [1, 0].into_iter().find(|&s| !lines[s].is_empty())?;
        Some(Self {
            lines,
            side,
            line: 0,
            column: 0,
        })
    }

    /// Where the cursor is, with its column limited to the line's last
    /// character.
    pub fn position(&self) -> Position {
        let last_column = self.place().chars.saturating_sub(1);
        Position {
            side: self.side,
            line: self.line,
            column: self.column.min(last_column),
        }
    }

    /// The index into the diff's rows of the cursor's line.
    pub fn row(&self) -> usize {
        self.place().row
    }

    fn place(&self) -> &LinePlace {
        &self.lines[self.side][self.line]
    }

    /// Moves to `position`, which must be a line of its side.
    pub fn set(&mut self, position: Position) {
        Position {
            side: self.side,
            line: self.line,
            column: self.column,
        } = position;
    }

    /// Moves `delta` characters right or left, stopping at either end of
    /// the line.
    pub fn move_column(&mut self, delta: isize) {
        let position = self.position();
        let last_column = self.place().chars.saturating_sub(1);
        self.column = position
            .column
            .saturating_add_signed(delta)
            .min(last_column);
    }

    /// Moves `delta` lines down or up, stopping at the first and last line.
    pub fn move_line(&mut self, delta: isize) {
        self.go_to_line(self.line.saturating_add_signed(delta));
    }

    /// Moves to `line`, or to the last line when `line` is past it.
    pub fn go_to_line(&mut self, line: usize) {
        self.line = line.min(self.lines[self.side].len() - 1);
    }

    /// Moves to the first line of `side` in or below `row`, or to its last
    /// line when none is. Stays put when `side` has no lines.
    pub fn go_to_row(&mut self, side: usize, row: usize) {
        let lines = &self.lines[side];
        if lines.is_empty() {
            return;
        }
        self.side = side;
        self.line = lines.partition_point(|l| l.row < row).min(lines.len() - 1);
    }
}

/// Where each of `side`'s lines sits in `diff`, in order.
fn side_lines(diff: &SideBySide, side: usize) -> Vec<LinePlace> {
    (diff.rows.iter().enumerate())
        .filter_map(|(row, r)| [&r.left, &r.right][side].as_ref().map(|l| (row, l)))
        .map(|(row, l)| LinePlace {
            row,
            chars: l.text.chars().count(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cursor on a diff of `left` and `right`, moved to `start`.
    fn cursor_at(left: &str, right: &str, start: Position) -> Cursor {
        let mut cursor = Cursor::new(&SideBySide::new(left, right)).unwrap();
        cursor.set(start);
        cursor
    }

    fn position(side: usize, line: usize, column: usize) -> Position {
        Position { side, line, column }
    }

    #[yare::parameterized(
        on_the_right_side = { "a\n", "b\n", 1 },
        on_the_left_side_when_the_right_has_no_lines = { "a\n", "", 0 },
    )]
    fn starts_at_the_first_line(left: &str, right: &str, side: usize) {
        let cursor = Cursor::new(&SideBySide::new(left, right)).unwrap();
        assert_eq!(cursor.position(), position(side, 0, 0));
    }

    #[test]
    fn there_is_none_without_lines() {
        assert!(Cursor::new(&SideBySide::new("", "")).is_none());
    }

    #[yare::parameterized(
        right_moves_a_character = { 1, 3 },
        left_moves_a_character = { -1, 1 },
        right_stops_at_the_last_character = { 9, 4 },
        left_stops_at_the_first_character = { -9, 0 },
    )]
    fn move_column(delta: isize, column: usize) {
        let mut cursor = cursor_at("", "hello\n", position(1, 0, 2));
        cursor.move_column(delta);
        assert_eq!(cursor.position().column, column);
    }

    #[yare::parameterized(
        down_moves_a_line = { 1, 2 },
        up_moves_a_line = { -1, 0 },
        down_stops_at_the_last_line = { 9, 3 },
        up_stops_at_the_first_line = { -9, 0 },
    )]
    fn move_line(delta: isize, line: usize) {
        let mut cursor = cursor_at("", "a\nb\nc\nd\n", position(1, 1, 0));
        cursor.move_line(delta);
        assert_eq!(cursor.position().line, line);
    }

    #[yare::parameterized(
        a_shorter_line_shows_its_last_character = { 1, 1 },
        an_empty_line_shows_the_first_column = { 2, 0 },
        a_long_enough_line_gets_the_column_back = { 3, 4 },
    )]
    fn the_column_is_kept_across_lines(delta: isize, column: usize) {
        let mut cursor = cursor_at("", "hello\nab\n\nworld\n", position(1, 0, 4));
        cursor.move_line(delta);
        assert_eq!(cursor.position().column, column);
    }

    #[test]
    fn moving_on_a_short_line_starts_from_the_column_shown() {
        let mut cursor = cursor_at("", "hello\nab\n", position(1, 0, 4));
        cursor.move_line(1);
        cursor.move_column(-1);
        assert_eq!(cursor.position().column, 0);
    }

    #[test]
    fn moving_down_skips_rows_without_a_line() {
        // The right side has no line opposite "b" and "c".
        let mut cursor = cursor_at("a\nb\nc\nd\n", "a\nd\n", position(1, 0, 0));
        cursor.move_line(1);
        assert_eq!((cursor.position().line, cursor.row()), (1, 3));
    }

    #[yare::parameterized(
        a_row_with_a_line_goes_to_it = { 0, 2, position(0, 2, 0) },
        a_row_without_one_goes_to_the_line_below = { 1, 1, position(1, 1, 0) },
        a_row_past_the_last_line_goes_to_it = { 0, 9, position(0, 3, 0) },
    )]
    fn go_to_row(side: usize, row: usize, expected: Position) {
        let mut cursor = cursor_at("a\nb\nc\nd\n", "a\nd\n", position(0, 0, 0));
        cursor.go_to_row(side, row);
        assert_eq!(cursor.position(), expected);
    }

    #[test]
    fn go_to_row_stays_put_on_a_side_without_lines() {
        let mut cursor = cursor_at("", "a\nb\n", position(1, 1, 0));
        cursor.go_to_row(0, 0);
        assert_eq!(cursor.position(), position(1, 1, 0));
    }
}
