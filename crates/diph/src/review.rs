//! The file to hold review comments

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use diph_core::Position;
use serde::{Deserialize, Serialize};

use crate::comment::{Comment, CommentRange};
use crate::selection::{SelectionKind, SelectionRange};

#[derive(Serialize, Deserialize)]
pub(crate) struct ReviewFile {
    pub(crate) version: u32,
    pub(crate) comments: Vec<ReviewComment>,
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
#[derive(Clone, Copy, Serialize, Deserialize)]
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

    /// The stored comment if it's for one of the provided `paths`
    fn comment(self, paths: [&Path; 2]) -> Option<Comment> {
        let side = match self.side {
            Side::Left => 0,
            Side::Right => 1,
        };
        if paths[side] != self.path {
            return None;
        }
        let kind = match self.start.column {
            Some(_) => SelectionKind::Character,
            None => SelectionKind::Line,
        };
        let position = |place: Place| Position {
            line: place.line.saturating_sub(1),
            column: place.column.unwrap_or(1).saturating_sub(1),
        };
        let range = SelectionRange {
            start: position(self.start),
            end: position(self.end),
        };
        let range = CommentRange { side, kind, range };
        Some(Comment {
            range,
            text: self.body,
        })
    }
}

/// The comments in the review file `file` on the left and right side shown
/// as `paths`. None when the file does not exist.
pub(crate) fn load(file: &Path, paths: [&Path; 2]) -> anyhow::Result<Vec<Comment>> {
    let comments = read(file)?.comments.into_iter();
    Ok(comments
        .filter_map(|comment| comment.comment(paths))
        .collect())
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
    let text = toml::to_string_pretty(&review)? + "\n";
    std::fs::write(file, text).with_context(|| format!("writing {}", file.display()))
}

fn read(file: &Path) -> anyhow::Result<ReviewFile> {
    match std::fs::read_to_string(file) {
        Ok(text) => toml::from_str(&text).with_context(|| format!("reading {}", file.display())),
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

    use super::{ReviewComment, ReviewFile, load, save};
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
            SelectionKind::Line,
            "quote = \"text\"\nbody = \"body\"\n\n[comments.start]\nline = 3\n\n[comments.end]\nline = 4\n",
        },
        a_character_comment_counts_columns_from_1 = {
            SelectionKind::Character,
            "quote = \"text\"\nbody = \"body\"\n\n[comments.start]\nline = 3\ncolumn = 5\n\n[comments.end]\nline = 4\ncolumn = 1\n",
        },
    )]
    fn review_comment_toml(kind: SelectionKind, positions: &str) {
        let comment = comment(kind, "body");
        let review_comment = ReviewComment::new(&comment, Path::new("a.rs"), "text".to_owned());
        let review = ReviewFile {
            version: 1,
            comments: vec![review_comment],
        };

        let serialized = toml::to_string_pretty(&review).unwrap();

        let expected =
            format!("version = 1\n\n[[comments]]\npath = \"a.rs\"\nside = \"right\"\n{positions}");
        assert_eq!(serialized, expected);
    }

    #[test]
    fn saving_replaces_the_comments_for_the_diff_and_keeps_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("review.toml");
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

        let saved: ReviewFile = toml::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        let bodies: Vec<_> = saved.comments.iter().map(|c| c.body.as_str()).collect();
        assert_eq!(saved.version, 1);
        assert_eq!(bodies, vec!["kept", "new"]);
    }

    #[yare::parameterized(
        a_line_comment_from_its_lines_first_column = { SelectionKind::Line, 0 },
        a_character_comment = { SelectionKind::Character, 4 },
    )]
    fn loading_reads_back_saved_comments(kind: SelectionKind, start_column: usize) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("review.toml");
        let mut comment = comment(kind, "body");
        comment.range.range.start.column = start_column;
        let review_comment = ReviewComment::new(&comment, Path::new("a.rs"), "text".to_owned());
        save(&file, [Path::new("a.rs"); 2], vec![review_comment]).unwrap();

        let loaded = load(&file, [Path::new("a.rs"); 2]).unwrap();

        assert_eq!(loaded, vec![comment]);
    }

    #[yare::parameterized(
        on_the_same_path = { ["old.rs", "new.rs"], 1 },
        on_the_path_of_the_other_side = { ["new.rs", "old.rs"], 0 },
        on_another_path = { ["old.rs", "other.rs"], 0 },
    )]
    fn loading_keeps_comments_on_their_sides_path(paths: [&str; 2], loaded: usize) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("review.toml");
        let comments = vec![review_comment("new.rs", "body")];
        save(&file, [Path::new("new.rs"); 2], comments).unwrap();

        let comments = load(&file, paths.map(Path::new)).unwrap();

        assert_eq!(comments.len(), loaded);
    }

    #[test]
    fn loading_a_missing_file_reads_no_comments() {
        let dir = tempfile::tempdir().unwrap();

        let comments = load(&dir.path().join("review.json"), [Path::new("a.rs"); 2]).unwrap();

        assert_eq!(comments, vec![]);
    }
}
