mod cursor;
mod keymap;
mod text_view;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::Context as _;
use clap::{CommandFactory as _, Parser, error::ErrorKind};
use diph_core::SideBySide;
use eframe::egui;

use text_view::TextView;

/// A side-by-side diff tool.
///
/// Use it as a git difftool with:
///
///   git config --global difftool.diph.cmd 'diph "$LOCAL" "$REMOTE" --label "$MERGED" --label "$MERGED"'
#[derive(Parser)]
#[command(version)]
struct Args {
    /// The file shown on the left, or `/dev/null` for none.
    left: PathBuf,
    /// The file shown on the right, or `/dev/null` for none.
    right: PathBuf,
    /// The path to show for a side instead of its file, such as the file's
    /// path in the repository when git passes temporary files. The first
    /// `--label` is for the left side and the second for the right. Its name
    /// also picks the side's syntax highlighting.
    #[arg(long)]
    label: Vec<PathBuf>,
}

impl Args {
    /// Parses `args`, a command line starting with the program name, allowing
    /// at most one `--label` per side.
    fn try_parse_args(
        args: impl IntoIterator<Item = impl Into<OsString> + Clone>,
    ) -> Result<Self, clap::Error> {
        let args = Self::try_parse_from(args)?;
        if args.label.len() > 2 {
            let message = "--label can be given at most twice, once per side";
            return Err(Self::command().error(ErrorKind::TooManyValues, message));
        }
        Ok(args)
    }

    /// The label given for `side`: 0 for left, 1 for right.
    fn label(&self, side: usize) -> Option<&Path> {
        self.label.get(side).map(PathBuf::as_path)
    }

    /// The paths the left and right side stand for, whose names pick their
    /// syntax: each side's label if given, else its path, or the other side's
    /// when it is `/dev/null`.
    fn side_paths(&self) -> [PathBuf; 2] {
        [
            side_path(
                &self.left,
                self.label(0),
                self.label(1).unwrap_or(&self.right),
            ),
            side_path(
                &self.right,
                self.label(1),
                self.label(0).unwrap_or(&self.left),
            ),
        ]
    }
}

/// Git passes `/dev/null` for the missing side of an added or deleted
/// file, even on Windows where no such file exists.
fn is_null(path: &Path) -> bool {
    path == Path::new("/dev/null")
}

impl eframe::App for TextView {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}

fn read(path: &Path) -> anyhow::Result<Vec<u8>> {
    if is_null(path) {
        return Ok(Vec::new());
    }
    std::fs::read(path).with_context(|| format!("reading {}", path.display()))
}

/// The name to show for a side: "(none)" for `/dev/null`, else the label if
/// given, else the path.
fn shown(path: &Path, label: Option<&Path>) -> String {
    match label {
        _ if is_null(path) => "(none)".to_owned(),
        Some(label) => label.display().to_string(),
        None => path.display().to_string(),
    }
}

/// The label if given, else `path`, or `other` when `path` is `/dev/null`.
fn side_path(path: &Path, label: Option<&Path>, other: &Path) -> PathBuf {
    let path = if is_null(path) { other } else { path };
    label.unwrap_or(path).to_path_buf()
}

/// The name of the comparison: the one path both sides stand for, or both
/// paths when they differ.
fn diff_name([left, right]: &[PathBuf; 2]) -> String {
    let left = left.as_path();
    let right = right.as_path();
    let left_name = left.file_name().unwrap_or(left.as_os_str()).to_str();
    let right_name = right.file_name().unwrap_or(right.as_os_str()).to_str();

    match (left_name, right_name) {
        (Some(left), Some(right)) if left != right => format!("{left} ↔ {right}"),
        (left, right) => left.or(right).unwrap_or_default().into(),
    }
}

/// Git's test: a NUL byte in the first 8000 bytes means binary.
fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8000).any(|&b| b == 0)
}

fn main() -> anyhow::Result<()> {
    let started = Instant::now();
    let args = Args::try_parse_args(std::env::args_os()).unwrap_or_else(|e| e.exit());
    let paths = args.side_paths();
    let name = diff_name(&paths);
    let (left, right) = (read(&args.left)?, read(&args.right)?);
    if is_binary(&left) || is_binary(&right) {
        eprintln!("diph: {name}: binary files differ");
        return Ok(());
    }

    // Non-UTF-8 text shows U+FFFD in place of the bytes it can't decode.
    let text = |bytes| String::from_utf8_lossy(bytes).into_owned();
    let diff = SideBySide::new(&text(&left), &text(&right));
    let (left_label, right_label) = (
        shown(&args.left, args.label(0)),
        shown(&args.right, args.label(1)),
    );
    let mut view = TextView::new(diff, left_label, right_label, started);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1400.0, 900.0]),
        ..Default::default()
    };
    eframe::run_native(
        "diph",
        options,
        Box::new(move |cc| {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Title(format!("{name} — diph")));
            // The diff colors are picked for a dark background.
            cc.egui_ctx.set_theme(egui::Theme::Dark);
            view.highlight_in_background(paths, cc.egui_ctx.clone());
            Ok(Box::new(view))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[yare::parameterized(
        each_label_names_its_own_side = {
            &["a.tmp", "b.tmp", "--label", "old.rs", "--label", "new.py"],
            ["old.rs", "new.py"],
        },
        unlabelled_dev_null_side_goes_by_the_other_sides_path = {
            &["/dev/null", "b.rs"],
            ["b.rs", "b.rs"],
        },
    )]
    fn side_paths(args: &[&str], expected: [&str; 2]) {
        let args = Args::try_parse_args(std::iter::once(&"diph").chain(args)).unwrap();
        assert_eq!(args.side_paths(), expected.map(PathBuf::from));
    }

    #[test]
    fn a_third_label_is_an_error() {
        let args = [
            "diph", "a", "b", "--label", "x", "--label", "y", "--label", "z",
        ];
        let error = Args::try_parse_args(args).err().unwrap();
        assert_eq!(error.kind(), ErrorKind::TooManyValues);
    }

    #[yare::parameterized(
        shows_a_shared_file_once = { ["src/a.rs", "src/a.rs"], "a.rs" },
        shows_a_shared_file_different_path_once = { ["foo/a.rs", "bar/a.rs"], "a.rs" },
        shows_both_files_when_they_differ = { ["old.rs", "new.py"], "old.rs ↔ new.py" },
        shows_both_files_when_paths_differ = { ["foo/old.rs", "bar/new.py"], "old.rs ↔ new.py" },
    )]
    fn diff_name(paths: [&str; 2], expected: &str) {
        assert_eq!(super::diff_name(&paths.map(PathBuf::from)), expected);
    }

    #[test]
    fn dev_null_reads_as_empty_even_where_it_does_not_exist() {
        assert_eq!(read(Path::new("/dev/null")).unwrap(), b"");
    }

    #[yare::parameterized(
        nul_byte_within_the_first_8000_bytes = { b"text\0more".to_vec(), true },
        text_without_a_nul_byte = { "café\r\n".as_bytes().to_vec(), false },
        nul_byte_after_the_first_8000_bytes = { [vec![b'a'; 8000], vec![0]].concat(), false },
    )]
    fn is_binary(bytes: Vec<u8>, binary: bool) {
        assert_eq!(super::is_binary(&bytes), binary);
    }
}
