//! The review file: review comments as JSON.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use diph_core::Position;
use serde::Serialize;

use crate::comment::{Comment, CommentRange};
use crate::selection::SelectionKind;

#[derive(Serialize)]
struct ReviewFile {
    version: u32,
    comments: Vec<ReviewComment>,
}

/// A comment as the review file stores it.
#[derive(Serialize)]
pub(crate) struct ReviewComment {
    /// The commented side's path, as shown above it.
    path: PathBuf,
    side: Side,
    start: Place,
    end: Place,
    /// The commented text.
    quote: String,
    body: String,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Side {
    Left,
    Right,
}

/// A place in a side's text, counting lines and columns from 1.
#[derive(Serialize)]
struct Place {
    line: usize,
    /// Left out for a comment on whole lines.
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<usize>,
}

impl ReviewComment {
    /// `comment` on the side shown as `path`, whose commented text is
    /// `quote`.
    pub(crate) fn new(comment: &Comment, path: &Path, quote: String) -> Self {
        let CommentRange { side, kind, range } = comment.range;
        let place = |position: Position| Place {
            line: position.line + 1,
            column: (kind == SelectionKind::Character).then_some(position.column + 1),
        };
        Self {
            path: path.to_owned(),
            side: match side {
                0 => Side::Left,
                _ => Side::Right,
            },
            start: place(range.start),
            end: place(range.end),
            quote,
            body: comment.text.clone(),
        }
    }
}

/// Writes `comments` to the review file `file`.
pub(crate) fn save(file: &Path, comments: Vec<ReviewComment>) -> anyhow::Result<()> {
    let review = ReviewFile {
        version: 1,
        comments,
    };
    let json = serde_json::to_string_pretty(&review)? + "\n";
    std::fs::write(file, json).with_context(|| format!("writing {}", file.display()))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use diph_core::Position;
    use serde_json::json;

    use super::{ReviewComment, save};
    use crate::comment::{Comment, CommentRange};
    use crate::selection::{SelectionKind, SelectionRange};

    fn comment(kind: SelectionKind, text: &str) -> Comment {
        let range = SelectionRange {
            start: Position { line: 2, column: 4 },
            end: Position { line: 3, column: 0 },
        };
        Comment {
            range: CommentRange {
                side: 1,
                kind,
                range,
            },
            text: text.to_owned(),
        }
    }

    #[yare::parameterized(
        a_line_comment_leaves_out_columns = {
            SelectionKind::Line, json!({ "line": 3 }), json!({ "line": 4 })
        },
        a_character_comment_counts_columns_from_1 = {
            SelectionKind::Character,
            json!({ "line": 3, "column": 5 }),
            json!({ "line": 4, "column": 1 })
        },
    )]
    fn review_comment_json(kind: SelectionKind, start: serde_json::Value, end: serde_json::Value) {
        let comment = comment(kind, "body");

        let review_comment = ReviewComment::new(&comment, Path::new("a.rs"), "text".to_owned());

        let expected = json!({
            "path": "a.rs",
            "side": "right",
            "start": start,
            "end": end,
            "quote": "text",
            "body": "body",
        });
        assert_eq!(serde_json::to_value(review_comment).unwrap(), expected);
    }

    #[test]
    fn saving_writes_the_comments_to_a_version_1_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("review.json");
        let comment = comment(SelectionKind::Line, "body");
        let comments = vec![ReviewComment::new(
            &comment,
            Path::new("a.rs"),
            "text".to_owned(),
        )];

        save(&file, comments).unwrap();

        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        let bodies = saved["comments"].as_array().map(|comments| comments.len());
        assert_eq!((saved["version"].as_u64(), bodies), (Some(1), Some(1)));
    }
}
