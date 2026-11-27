//! Word boundaries, word motions, and word text objects.

use std::iter;
use std::ops::Range;

use diph_core::{Buffer, Position};

/// The class of character for Vim word boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CharKind {
    Keyword,
    Punctuation,
    Whitespace,
}

/// The character class of `c`.
pub(crate) fn char_kind(c: char) -> CharKind {
    if c.is_alphanumeric() || c == '_' {
        CharKind::Keyword
    } else if c.is_whitespace() {
        CharKind::Whitespace
    } else {
        CharKind::Punctuation
    }
}

/// The character column range of the word or whitespace span under `column`
/// on `line`. None when `column` is past the line's characters.
#[allow(dead_code)]
pub(crate) fn inner_word_range(line: &str, column: usize) -> Option<Range<usize>> {
    let chars: Vec<char> = line.chars().collect();
    let (first, second) = chars.split_at_checked(column)?;
    let target = char_kind(*second.first()?);
    let start = first
        .iter()
        .rposition(|&c| char_kind(c) != target)
        .map_or(0, |idx| idx + 1);
    let end = second
        .iter()
        .position(|&c| char_kind(c) != target)
        .map_or(chars.len(), |idx| column + idx);
    Some(start..end)
}

/// The character column range of a word under `column` on `line`, including
/// its trailing whitespace, or leading whitespace if there is no trailing
/// whitespace on the line. When `column` is on whitespace, includes the word
/// following it. None when `column` is past the line's characters.
#[allow(dead_code)]
pub(crate) fn a_word_range(line: &str, column: usize) -> Option<Range<usize>> {
    let chars: Vec<char> = line.chars().collect();
    let inner = inner_word_range(line, column)?;
    let word_kind = char_kind(*chars.get(column)?);
    let (trailing_kind, leading_kind) = match word_kind {
        CharKind::Whitespace => (
            chars.get(inner.end).copied().map(char_kind),
            inner.start.checked_sub(1).map(|i| char_kind(chars[i])),
        ),
        _ => (Some(CharKind::Whitespace), Some(CharKind::Whitespace)),
    };
    let trailing = trailing_kind.map_or(0, |kind| {
        chars[inner.end..]
            .iter()
            .take_while(|&&c| char_kind(c) == kind)
            .count()
    });
    if trailing > 0 {
        return Some(inner.start..inner.end + trailing);
    }
    let leading = leading_kind.map_or(0, |kind| {
        chars[..inner.start]
            .iter()
            .rev()
            .take_while(|&&c| char_kind(c) == kind)
            .count()
    });
    if leading > 0 {
        return Some(inner.start - leading..inner.end);
    }
    Some(inner)
}

/// The destination of moving forward `count` word starts from `from` in
/// `buffer`.
pub(crate) fn find_word_forward(buffer: &Buffer, from: Position, count: usize) -> Position {
    let from = buffer.clamp(from);
    iter::successors(Some(from), |&pos| {
        let next = step_word_forward(buffer, pos);
        (next != pos).then_some(next)
    })
    .take(count + 1)
    .last()
    .unwrap_or(from)
}

/// The destination of moving backward `count` word starts from `from` in
/// `buffer`.
pub(crate) fn find_word_backward(buffer: &Buffer, from: Position, count: usize) -> Position {
    let from = buffer.clamp(from);
    iter::successors(Some(from), |&pos| {
        let next = step_word_backward(buffer, pos);
        (next != pos).then_some(next)
    })
    .take(count + 1)
    .last()
    .unwrap_or(from)
}

/// The destination of moving forward to the end of `count` words from `from`
/// in `buffer`.
pub(crate) fn find_word_end(buffer: &Buffer, from: Position, count: usize) -> Position {
    let from = buffer.clamp(from);
    iter::successors(Some(from), |&pos| {
        let next = step_word_end(buffer, pos);
        (next != pos).then_some(next)
    })
    .take(count + 1)
    .last()
    .unwrap_or(from)
}

fn step_word_forward(buffer: &Buffer, pos: Position) -> Position {
    let lines = buffer.lines();
    if lines.is_empty() {
        return Position { line: 0, column: 0 };
    }

    let pos = buffer.clamp(pos);
    let line_chars: Vec<char> = lines[pos.line].chars().collect();
    if pos.column >= line_chars.len() {
        return next_line_first_word(buffer, pos.line + 1);
    }

    let start_kind = char_kind(line_chars[pos.column]);
    let mut col = pos.column;

    // Skip to end of current word/type
    if start_kind != CharKind::Whitespace {
        col += line_chars[col..]
            .iter()
            .take_while(|&&c| char_kind(c) == start_kind)
            .count();
    }

    // Skip whitespace
    col += line_chars[col..]
        .iter()
        .take_while(|&&c| char_kind(c) == CharKind::Whitespace)
        .count();

    if col < line_chars.len() {
        return Position {
            line: pos.line,
            column: col,
        };
    }

    next_line_first_word(buffer, pos.line + 1)
}

fn next_line_first_word(buffer: &Buffer, from_line: usize) -> Position {
    let lines = buffer.lines();
    for (line_idx, line) in lines.iter().enumerate().skip(from_line) {
        if line.is_empty() {
            return Position {
                line: line_idx,
                column: 0,
            };
        }
        if let Some(col) = line.chars().position(|c| !c.is_whitespace()) {
            return Position {
                line: line_idx,
                column: col,
            };
        }
    }
    buffer.clamp(Position {
        line: from_line,
        column: 0,
    })
}

fn step_word_end(buffer: &Buffer, pos: Position) -> Position {
    let lines = buffer.lines();
    if lines.is_empty() {
        return Position { line: 0, column: 0 };
    }

    let pos = buffer.clamp(pos);
    let Some(target) = next_non_whitespace(
        buffer,
        Position {
            line: pos.line,
            column: pos.column + 1,
        },
    ) else {
        return buffer.clamp(Position {
            line: lines.len(),
            column: 0,
        });
    };
    let chars: Vec<char> = lines[target.line].chars().collect();
    let target_kind = char_kind(chars[target.column]);
    let col = target.column
        + chars[target.column + 1..]
            .iter()
            .take_while(|&&c| char_kind(c) == target_kind)
            .count();

    Position {
        line: target.line,
        column: col,
    }
}

/// The column of the first non-whitespace character in `line` at or after
/// `start`, or None when there is none.
fn first_non_whitespace(line: &str, start: usize) -> Option<usize> {
    line.chars()
        .skip(start)
        .position(|c| !c.is_whitespace())
        .map(|i| start + i)
}

/// The position of the first non-whitespace character at or after `from`,
/// scanning forward across lines, or None when there is none.
fn next_non_whitespace(buffer: &Buffer, from: Position) -> Option<Position> {
    buffer
        .lines()
        .iter()
        .enumerate()
        .skip(from.line)
        .find_map(|(line_idx, line)| {
            let start = if line_idx == from.line {
                from.column
            } else {
                0
            };
            first_non_whitespace(line, start).map(|column| Position {
                line: line_idx,
                column,
            })
        })
}

fn step_word_backward(buffer: &Buffer, pos: Position) -> Position {
    let lines = buffer.lines();
    if lines.is_empty() {
        return Position { line: 0, column: 0 };
    }

    let pos = buffer.clamp(pos);
    let start_line = if pos.column == 0 {
        let Some(prev) = pos.line.checked_sub(1) else {
            return Position { line: 0, column: 0 };
        };
        prev
    } else {
        pos.line
    };

    (0..=start_line)
        .rev()
        .find_map(|line_idx| {
            let line = &lines[line_idx];
            if line_idx < pos.line && line.is_empty() {
                return Some(Position {
                    line: line_idx,
                    column: 0,
                });
            }
            let chars: Vec<char> = line.chars().collect();
            let search_end = if line_idx == pos.line {
                pos.column
            } else {
                chars.len()
            };
            let col = chars[..search_end]
                .iter()
                .rposition(|&c| !c.is_whitespace())?;
            let target_kind = char_kind(chars[col]);
            let column = chars[..=col]
                .iter()
                .rposition(|&c| char_kind(c) != target_kind)
                .map_or(0, |p| p + 1);
            Some(Position {
                line: line_idx,
                column,
            })
        })
        .unwrap_or(Position { line: 0, column: 0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[yare::parameterized(
        alpha = { 'a', CharKind::Keyword },
        multibyte_alpha = { 'é', CharKind::Keyword },
        digit = { '9', CharKind::Keyword },
        underscore = { '_', CharKind::Keyword },
        space = { ' ', CharKind::Whitespace },
        tab = { '\t', CharKind::Whitespace },
        punctuation = { '.', CharKind::Punctuation },
        paren = { '(', CharKind::Punctuation },
    )]
    fn char_kind_classifies_characters(c: char, expected: CharKind) {
        assert_eq!(char_kind(c), expected);
    }

    #[yare::parameterized(
        word_start = { "hello world", 0, 0..5 },
        word_middle = { "hello world", 2, 0..5 },
        word_end = { "hello world", 4, 0..5 },
        second_word = { "hello world", 6, 6..11 },
        whitespace = { "hello   world", 6, 5..8 },
        punctuation = { "foo->bar", 3, 3..5 },
        single_char = { "a", 0, 0..1 },
        single_char_in_line = { "a + b", 2, 2..3 },
        multibyte = { "café ok", 3, 0..4 },
    )]
    fn inner_word_range_finds_boundaries(line: &str, col: usize, expected: Range<usize>) {
        assert_eq!(inner_word_range(line, col), Some(expected));
    }

    #[yare::parameterized(
        empty_line = { "", 0 },
        at_end_of_line = { "hello", 5 },
        past_end_of_line = { "hello", 10 },
    )]
    fn inner_word_range_out_of_bounds_is_none(line: &str, col: usize) {
        assert_eq!(inner_word_range(line, col), None);
    }

    #[yare::parameterized(
        trailing_whitespace = { "hello world", 0, 0..6 },
        trailing_multiple_spaces = { "hello   world", 2, 0..8 },
        no_trailing_uses_leading = { "hello world", 8, 5..11 },
        only_word_on_line = { "hello", 2, 0..5 },
        single_char = { "a", 0, 0..1 },
        punctuation_with_trailing = { "foo, bar", 3, 3..5 },
        whitespace_takes_following = { "hello world", 5, 5..11 },
        whitespace_before_punctuation_stops_at_kind_change = { "foo (bar)", 3, 3..5 },
        trailing_whitespace_only_uses_preceding_word = { "foo   ", 4, 0..6 },
        whitespace_only_line = { "   ", 1, 0..3 },
        stops_on_kind_change = { "fn foo(arg: type)", 3, 2..6 },
        multibyte = { "café ok", 3, 0..5 },
    )]
    fn a_word_range_includes_surrounding_whitespace(
        line: &str,
        col: usize,
        expected: Range<usize>,
    ) {
        assert_eq!(a_word_range(line, col), Some(expected));
    }

    #[yare::parameterized(
        empty_line = { "", 0 },
        at_end_of_line = { "hello", 5 },
        past_end_of_line = { "hello", 10 },
    )]
    fn a_word_range_out_of_bounds_is_none(line: &str, col: usize) {
        assert_eq!(a_word_range(line, col), None);
    }

    #[yare::parameterized(
        empty_buffer = { "", Position { line: 0, column: 0 }, 1, Position { line: 0, column: 0 } },
        count_zero = { "one two", Position { line: 0, column: 2 }, 0, Position { line: 0, column: 2 } },
        to_next_word = { "one two three", Position { line: 0, column: 0 }, 1, Position { line: 0, column: 4 } },
        from_word_middle = { "one two three", Position { line: 0, column: 1 }, 1, Position { line: 0, column: 4 } },
        with_count = { "one two three", Position { line: 0, column: 0 }, 2, Position { line: 0, column: 8 } },
        count_exceeds_words = { "one two three", Position { line: 0, column: 0 }, 3, Position { line: 0, column: 12 } },
        count_exceeds_words_more = { "one two three", Position { line: 0, column: 0 }, 4, Position { line: 0, column: 12 } },
        across_punctuation = { "foo(bar)", Position { line: 0, column: 0 }, 1, Position { line: 0, column: 3 } },
        from_punctuation = { "foo(bar)", Position { line: 0, column: 3 }, 1, Position { line: 0, column: 4 } },
        across_lines = { "one\ntwo\n", Position { line: 0, column: 0 }, 1, Position { line: 1, column: 0 } },
        across_empty_line = { "one\n\ntwo\n", Position { line: 0, column: 0 }, 1, Position { line: 1, column: 0 } },
        whitespace_only_buffer = { "   ", Position { line: 0, column: 0 }, 1, Position { line: 0, column: 2 } },
        trailing_whitespace_on_last_line = { "one   ", Position { line: 0, column: 0 }, 1, Position { line: 0, column: 5 } },
        trailing_empty_line = { "one\n\n", Position { line: 0, column: 0 }, 2, Position { line: 1, column: 0 } },
        out_of_bounds_position = { "one two", Position { line: 5, column: 9 }, 1, Position { line: 0, column: 6 } },
        multibyte = { "café naïve", Position { line: 0, column: 0 }, 1, Position { line: 0, column: 5 } },
    )]
    fn word_forward_moves_to_next_start(
        text: &str,
        from: Position,
        count: usize,
        expected: Position,
    ) {
        let buffer = Buffer::new(text);
        assert_eq!(find_word_forward(&buffer, from, count), expected);
    }

    #[yare::parameterized(
        empty_buffer = { "", Position { line: 0, column: 0 }, 1, Position { line: 0, column: 0 } },
        count_zero = { "one two", Position { line: 0, column: 4 }, 0, Position { line: 0, column: 4 } },
        to_prev_word = { "one two three", Position { line: 0, column: 4 }, 1, Position { line: 0, column: 0 } },
        to_prev_word_with_high_count = { "one two three", Position { line: 0, column: 4 }, 8, Position { line: 0, column: 0 } },
        from_word_middle = { "one two three", Position { line: 0, column: 6 }, 1, Position { line: 0, column: 4 } },
        with_count = { "one two three", Position { line: 0, column: 8 }, 2, Position { line: 0, column: 0 } },
        across_punctuation = { "foo(bar)", Position { line: 0, column: 4 }, 1, Position { line: 0, column: 3 } },
        across_lines = { "one\ntwo\n", Position { line: 1, column: 0 }, 1, Position { line: 0, column: 0 } },
        at_buffer_start = { "one", Position { line: 0, column: 0 }, 1, Position { line: 0, column: 0 } },
        whitespace_only_buffer = { "   ", Position { line: 0, column: 2 }, 1, Position { line: 0, column: 0 } },
        leading_whitespace_on_first_line = { "   foo", Position { line: 0, column: 3 }, 1, Position { line: 0, column: 0 } },
        across_leading_whitespace_to_prev_line = { "one\n  two", Position { line: 1, column: 2 }, 1, Position { line: 0, column: 0 } },
        across_whitespace_only_line = { "one\n   \ntwo", Position { line: 2, column: 0 }, 1, Position { line: 0, column: 0 } },
        across_empty_line = { "one\n\ntwo", Position { line: 2, column: 0 }, 1, Position { line: 1, column: 0 } },
        whitespace_only_line_before_word = { "   \nfoo", Position { line: 1, column: 0 }, 1, Position { line: 0, column: 0 } },
        out_of_bounds_position = { "one two", Position { line: 5, column: 9 }, 1, Position { line: 0, column: 4 } },
        multibyte = { "café naïve", Position { line: 0, column: 5 }, 1, Position { line: 0, column: 0 } },
    )]
    fn word_backward_moves_to_prev_start(
        text: &str,
        from: Position,
        count: usize,
        expected: Position,
    ) {
        let buffer = Buffer::new(text);
        assert_eq!(find_word_backward(&buffer, from, count), expected);
    }

    #[yare::parameterized(
        empty_buffer = { "", Position { line: 0, column: 0 }, 1, Position { line: 0, column: 0 } },
        count_zero = { "one two", Position { line: 0, column: 0 }, 0, Position { line: 0, column: 0 } },
        single_char_buffer = { "a", Position { line: 0, column: 0 }, 1, Position { line: 0, column: 0 } },
        to_word_end = { "one two three", Position { line: 0, column: 0 }, 1, Position { line: 0, column: 2 } },
        from_word_end = { "one two three", Position { line: 0, column: 2 }, 1, Position { line: 0, column: 6 } },
        with_count = { "one two three", Position { line: 0, column: 0 }, 2, Position { line: 0, column: 6 } },
        count_exceeds_buffer = { "one two three", Position { line: 0, column: 0 }, 4, Position { line: 0, column: 12 } },
        punctuation_end = { "foo(bar)", Position { line: 0, column: 2 }, 1, Position { line: 0, column: 3 } },
        across_lines = { "one\ntwo\n", Position { line: 0, column: 2 }, 1, Position { line: 1, column: 2 } },
        whitespace_only_buffer = { "   ", Position { line: 0, column: 0 }, 1, Position { line: 0, column: 2 } },
        trailing_whitespace = { "foo   ", Position { line: 0, column: 2 }, 1, Position { line: 0, column: 5 } },
        trailing_empty_line = { "foo\n\n", Position { line: 0, column: 2 }, 1, Position { line: 1, column: 0 } },
        out_of_bounds_position = { "one two", Position { line: 5, column: 9 }, 1, Position { line: 0, column: 6 } },
        multibyte = { "café naïve", Position { line: 0, column: 0 }, 1, Position { line: 0, column: 3 } },
    )]
    fn word_end_moves_to_word_end(text: &str, from: Position, count: usize, expected: Position) {
        let buffer = Buffer::new(text);
        assert_eq!(find_word_end(&buffer, from, count), expected);
    }
    #[yare::parameterized(
        next_word = { "one two three", Position { line: 0, column: 0 }, Position { line: 0, column: 4 } },
        next_word_at_end = { "one two three", Position { line: 0, column: 2 }, Position { line: 0, column: 4 } },
        next_word_on_space = { "one two three", Position { line: 0, column: 3 }, Position { line: 0, column: 4 } },
        to_last_word = { "one two three", Position { line: 0, column: 6 }, Position { line: 0, column: 8 } },
        space_before_last_word = { "one two three", Position { line: 0, column: 6 }, Position { line: 0, column: 8 } },
        on_last_word = { "one two three", Position { line: 0, column: 8 }, Position { line: 0, column: 12 } },
        at_end_of_buffer = { "one two three", Position { line: 0, column: 12 }, Position { line: 0, column: 12 } },
        punctuation = { "foo(bar)", Position { line: 0, column: 2 }, Position { line: 0, column: 3 } },
        across_lines = { "one\ntwo\n", Position { line: 0, column: 2 }, Position { line: 1, column: 0 } },
    )]
    fn step_word_forward_moves_to_next_or_last_word(
        text: &str,
        from: Position,
        expected: Position,
    ) {
        let buffer = Buffer::new(text);
        assert_eq!(step_word_forward(&buffer, from), expected);
    }
}
