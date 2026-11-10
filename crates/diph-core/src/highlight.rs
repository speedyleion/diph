//! Syntax highlighting with syntect.

use std::ops::Range;
use std::path::Path;

use syntect::easy::HighlightLines;
use syntect::parsing::{SyntaxReference, SyntaxSet};

use crate::{Edit, Line, Theme};

/// A highlighted run of a line.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    /// Byte range within the line.
    pub range: Range<usize>,
    pub rgb: [u8; 3],
}

/// Syntax colors of one side's lines, by line index.
#[derive(Default)]
pub struct SideColors {
    pub lines: Vec<Vec<Span>>,
}

impl SideColors {
    /// The colors of `line`; empty until highlighted.
    pub fn line(&self, line: &Line) -> &[Span] {
        self.lines.get(line.index).map_or(&[], Vec::as_slice)
    }

    /// Moves the colors with the text `edit` changed. Text before and
    /// after the change keeps its colors, text inserted within a line takes
    /// the color of the character before it, and other inserted text has
    /// none.
    pub fn apply(&mut self, edit: &Edit) {
        let Edit {
            start,
            deleted_end,
            inserted_end,
        } = *edit;
        if start.line >= self.lines.len() {
            return;
        }
        let mut before: Vec<_> = clipped(&self.lines[start.line], 0..start.byte).collect();
        if let Some(span) = before.last_mut()
            && span.range.end == start.byte
            && start.line == inserted_end.line
        {
            span.range.end = inserted_end.byte;
        }
        let shift = |byte: usize| byte - deleted_end.byte + inserted_end.byte;
        let after = self
            .lines
            .get(deleted_end.line)
            .map_or(&[][..], Vec::as_slice);
        let after = clipped(after, deleted_end.byte..usize::MAX).map(|span| Span {
            range: shift(span.range.start)..shift(span.range.end),
            ..span
        });
        let mut lines = vec![Vec::new(); inserted_end.line - start.line + 1];
        lines[0] = before;
        lines.last_mut().expect("one line or more").extend(after);
        let end = (deleted_end.line + 1).min(self.lines.len());
        self.lines.splice(start.line..end, lines);
    }
}

/// The parts of `spans` within `bytes`.
fn clipped(spans: &[Span], bytes: Range<usize>) -> impl Iterator<Item = Span> {
    spans
        .iter()
        .map(move |span| Span {
            range: span.range.start.max(bytes.start)..span.range.end.min(bytes.end),
            rgb: span.rgb,
        })
        .filter(|span| !span.range.is_empty())
}

pub struct Highlighter {
    syntaxes: SyntaxSet,
}

impl Default for Highlighter {
    fn default() -> Self {
        Self {
            syntaxes: two_face::syntax::extra_newlines(),
        }
    }
}

impl Highlighter {
    /// Highlights `lines` (without line endings) in the colors of `theme`
    /// as the language picked from `path`'s name, or else from the first
    /// line, e.g. a `#!` line. The iterator is lazy, so callers can take
    /// lines in chunks.
    pub fn highlight<'a>(
        &'a self,
        theme: &'a Theme,
        path: &Path,
        lines: &'a [String],
    ) -> impl Iterator<Item = Vec<Span>> + 'a {
        let mut state = HighlightLines::new(self.syntax(path, lines), &theme.syntect);
        lines
            .iter()
            .map(move |line| self.highlight_line(&mut state, line))
    }

    /// Highlights `line` as the next line of the file `state` is parsing.
    fn highlight_line(&self, state: &mut HighlightLines, line: &str) -> Vec<Span> {
        // The syntaxes expect each line to end in a newline; the spans are
        // cut back to the line without it.
        let with_newline = format!("{line}\n");
        let pieces = state
            .highlight_line(&with_newline, &self.syntaxes)
            .unwrap_or_default();
        pieces
            .into_iter()
            .filter_map(|(style, piece)| {
                Some((with_newline.substr_range(piece)?, style.foreground))
            })
            .map(|(range, c)| Span {
                range: range.start..range.end.min(line.len()),
                rgb: [c.r, c.g, c.b],
            })
            .filter(|span| !span.range.is_empty())
            .collect()
    }

    fn syntax(&self, path: &Path, lines: &[String]) -> &SyntaxReference {
        let by_name = self.syntaxes.find_syntax_for_file(path).ok().flatten();
        let by_first_line = || {
            let first = lines.first()?;
            self.syntaxes.find_syntax_by_first_line(first)
        };
        by_name
            .or_else(by_first_line)
            .unwrap_or_else(|| self.syntaxes.find_syntax_plain_text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Buffer, Position};

    /// The distinct colors of each line, as the text they cover.
    fn colored_words(path: &str, text: &str) -> Vec<Vec<String>> {
        let lines: Vec<String> = text.lines().map(str::to_owned).collect();
        let highlighter = Highlighter::default();
        let theme = Theme::default();
        highlighter
            .highlight(&theme, Path::new(path), &lines)
            .zip(&lines)
            .map(|(spans, line)| {
                spans
                    .iter()
                    .map(|span| line[span.range.clone()].to_owned())
                    .collect()
            })
            .collect()
    }

    #[yare::parameterized(
        rust_file_is_highlighted = { "main.rs", "fn main() {}", true },
        kotlin_file_is_highlighted = { "Main.kt", "fun main() {}", true },
        unknown_file_is_plain = { "notes", "fn main() {}", false },
    )]
    fn picks_the_language_from_the_file_name(path: &str, code: &str, highlighted: bool) {
        let words = colored_words(path, code);
        assert_eq!(words[0].concat(), code);
        assert_eq!(words[0].len() > 1, highlighted, "{words:?}");
    }

    #[test]
    fn falls_back_to_the_first_line() {
        let words = colored_words("script", "#!/usr/bin/env python\nimport os");
        assert!(words[1].len() > 1, "{words:?}");
    }

    #[yare::parameterized(
        inside_an_open_comment = { "/* start", false },
        after_a_closed_comment = { "/* done */", true },
    )]
    fn carries_state_across_lines(first_line: &str, highlighted: bool) {
        let words = colored_words("a.rs", &format!("{first_line}\nfn still_a_comment"));
        assert_eq!(words[1].len() > 1, highlighted, "{words:?}");
    }

    /// Colors with a span for each run of one digit in `tags`, a character
    /// per byte of each line, whose color is the digit; `.` has none.
    fn colors_from_tags(tags: &[&str]) -> SideColors {
        SideColors {
            lines: tags.iter().map(|tags| line_colors(tags)).collect(),
        }
    }

    /// A line's spans from its tags, as [`colors_from_tags`] takes them.
    fn line_colors(tags: &str) -> Vec<Span> {
        let bytes: Vec<_> = tags.bytes().enumerate().collect();
        let runs = bytes
            .chunk_by(|a, b| a.1 == b.1)
            .map(|run| (run[0], run.len()));
        runs.filter(|((_, tag), _)| *tag != b'.')
            .map(|((start, tag), len)| Span {
                range: start..start + len,
                rgb: [tag; 3],
            })
            .collect()
    }

    /// The tags, as [`colors_from_tags`] takes them, of `colors` on
    /// `buffer`'s lines.
    fn tags(colors: &SideColors, buffer: &Buffer) -> Vec<String> {
        let lines = colors.lines.iter().zip(buffer.lines());
        lines.map(|(spans, text)| line_tags(spans, text)).collect()
    }

    /// The tags of `spans` on `text`.
    fn line_tags(spans: &[Span], text: &str) -> String {
        let mut tags = vec![b'.'; text.len()];
        for span in spans {
            tags[span.range.clone()].fill(span.rgb[0]);
        }
        String::from_utf8(tags).unwrap()
    }

    #[yare::parameterized(
        text_typed_in_a_span_takes_its_color = { |b| b.insert(Position { line: 0, column: 1 }, "X"), &["111233", "44"] },
        text_typed_after_a_span_takes_its_color = { |b| b.insert(Position { line: 0, column: 2 }, "X"), &["111233", "44"] },
        text_typed_at_the_start_of_a_line_has_none = { |b| b.insert(Position { line: 0, column: 0 }, "X"), &[".11233", "44"] },
        a_split_line_keeps_its_colors = { |b| b.insert(Position { line: 0, column: 2 }, "\n"), &["11", "233", "44"] },
        inserted_lines_have_none = { |b| b.insert(Position { line: 0, column: 2 }, "X\nY"), &["11.", ".233", "44"] },
        deleting_keeps_the_colors_around_it = { |b| b.delete(Position { line: 0, column: 1 }..Position { line: 0, column: 4 }), &["13", "44"] },
        joined_lines_keep_their_colors = { |b| b.delete(Position { line: 0, column: 5 }..Position { line: 1, column: 0 }), &["1123344"] },
    )]
    fn colors_follow_the_edited_text(edit: fn(&mut Buffer) -> Edit, after: &[&str]) {
        let mut buffer = Buffer::new("ab cd\nef\n");
        let mut colors = colors_from_tags(&["11233", "44"]);
        colors.apply(&edit(&mut buffer));
        assert_eq!(tags(&colors, &buffer), after);
    }
}
