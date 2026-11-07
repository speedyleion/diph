//! The text of one side of a diff.

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
}
