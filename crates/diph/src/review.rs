//! The file to hold review comments

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use diph_core::Position;
use serde::{Deserialize, Serialize};

use crate::comment::{Comment, CommentRange};
use crate::selection::SelectionKind;

#[derive(Serialize, Deserialize)]
struct ReviewFile {
    version: u32,
    comments: Vec<ReviewComment>,
}

/// A comment as the review file stores it.
#[derive(Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Side {
    Left,
    Right,
}

/// A place in a side's text, counting lines and columns from 1.
#[derive(Serialize, Deserialize)]
struct Place {
    line: usize,
    /// Left out for a comment on whole lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

/// Writes `comments` to the review `file`
/// Comments for `paths` will replace any comments for paths that exist in `file`, if `file`
/// already contains comments.
/// Comments for other files will remain unmodified.
pub(crate) fn save(
    file: &Path,
    paths: [&Path; 2],
    comments: Vec<ReviewComment>,
) -> anyhow::Result<()> {
    let mut review = read(file)?;
    review
        .comments
        .retain(|comment| !paths.contains(&comment.path.as_path()));
    review.comments.extend(comments);
    let json = serde_json::to_string_pretty(&review)? + "\n";
    std::fs::write(file, json).with_context(|| format!("writing {}", file.display()))
}

fn read(file: &Path) -> anyhow::Result<ReviewFile> {
    match std::fs::read(file) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).with_context(|| format!("reading {}", file.display()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(ReviewFile {
            version: 1,
            comments: Vec::new(),
        }),
        Err(error) => Err(error).with_context(|| format!("reading {}", file.display())),
    }
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

    fn review_comment(path: &str, body: &str) -> ReviewComment {
        let comment = comment(SelectionKind::Line, body);
        ReviewComment::new(&comment, Path::new(path), "quote".to_owned())
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
    fn saving_replaces_the_comments_for_the_diff_and_keeps_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("review.json");
        let kept = vec![review_comment("other.rs", "kept")];
        save(&file, [Path::new("other.rs"); 2], kept).unwrap();
        save(
            &file,
            [Path::new("a.rs"); 2],
            vec![review_comment("a.rs", "old")],
        )
        .unwrap();

        save(
            &file,
            [Path::new("a.rs"); 2],
            vec![review_comment("a.rs", "new")],
        )
        .unwrap();

        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        let bodies: Vec<_> = saved["comments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|comment| comment["body"].as_str().unwrap())
            .collect();
        assert_eq!(saved["version"].as_u64(), Some(1));
        assert_eq!(bodies, vec!["kept", "new"]);
    }
}
