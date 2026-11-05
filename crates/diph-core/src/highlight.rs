//! Syntax highlighting with syntect.

use std::ops::Range;
use std::path::Path;

use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

/// A highlighted run of a line.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    /// Byte range within the line.
    pub range: Range<usize>,
    pub rgb: [u8; 3],
}

pub struct Highlighter {
    syntaxes: SyntaxSet,
    theme: Theme,
}

impl Default for Highlighter {
    fn default() -> Self {
        let mut themes = ThemeSet::load_defaults();
        Self {
            syntaxes: SyntaxSet::load_defaults_newlines(),
            theme: themes
                .themes
                .remove("base16-ocean.dark")
                .expect("bundled theme"),
        }
    }
}

impl Highlighter {
    /// Highlights `lines` (without line endings) as the language picked from
    /// `path`'s name, or else from the first line, e.g. a `#!` line. The
    /// iterator is lazy, so callers can take lines in chunks.
    pub fn highlight<'a>(
        &'a self,
        path: &Path,
        lines: &'a [String],
    ) -> impl Iterator<Item = Vec<Span>> + 'a {
        let mut state = HighlightLines::new(self.syntax(path, lines), &self.theme);
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

    /// The distinct colors of each line, as the text they cover.
    fn colored_words(path: &str, text: &str) -> Vec<Vec<String>> {
        let lines: Vec<String> = text.lines().map(str::to_owned).collect();
        let highlighter = Highlighter::default();
        highlighter
            .highlight(Path::new(path), &lines)
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
        rust_file_is_highlighted = { "main.rs", true },
        unknown_file_is_plain = { "notes", false },
    )]
    fn picks_the_language_from_the_file_name(path: &str, highlighted: bool) {
        let words = colored_words(path, "fn main() {}");
        assert_eq!(words[0].concat(), "fn main() {}");
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
}
