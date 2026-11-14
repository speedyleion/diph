mod app;
mod cursor;
mod keymap;
mod menu;
mod palette;
mod selection;
mod text_view;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::{CommandFactory as _, Parser, error::ErrorKind};
use eframe::egui;
use winit::event_loop::{EventLoop, EventLoopProxy};

use app::{App, FileLoadError, SideFile};
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

/// The name to show for a side: "(none)" for `/dev/null`, else the label if
/// given, else the path.
fn shown(path: &Path, label: Option<&Path>) -> PathBuf {
    match label {
        _ if app::is_null(path) => PathBuf::from("(none)"),
        Some(label) => label.to_path_buf(),
        None => path.to_path_buf(),
    }
}

/// The label if given, else `path`, or `other` when `path` is `/dev/null`.
fn side_path(path: &Path, label: Option<&Path>, other: &Path) -> PathBuf {
    let path = if app::is_null(path) { other } else { path };
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

fn main() -> anyhow::Result<()> {
    let started = Instant::now();
    let args = Args::try_parse_args(std::env::args_os()).unwrap_or_else(|e| e.exit());
    let paths = args.side_paths();
    let name = diff_name(&paths);
    let mut files = [
        SideFile {
            label: shown(&args.left, args.label(0)),
            path: args.left.clone(),
            saveable: false,
        },
        SideFile {
            label: shown(&args.right, args.label(1)),
            path: args.right.clone(),
            saveable: false,
        },
    ];
    let diff = match app::load_files(&mut files) {
        Ok(diff) => diff,
        Err(FileLoadError::Binary) => {
            eprintln!("diph: {name}: binary files differ");
            return Ok(());
        }
        Err(FileLoadError::Read(error)) => return Err(error),
    };
    let view = TextView::new(diff);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1400.0, 900.0]),
        ..Default::default()
    };
    let event_loop = EventLoop::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let mut app = eframe::create_native(
        "diph",
        options,
        Box::new(move |cc| {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Title(format!("{name} — diph")));
            Ok(Box::new(App::new(
                view,
                files,
                paths,
                started,
                cc.storage,
                &cc.egui_ctx,
                wake_event_loop(proxy),
            )))
        }),
        &event_loop,
    );
    event_loop.run_app(&mut app)?;
    Ok(())
}

fn wake_event_loop(proxy: EventLoopProxy<eframe::UserEvent>) -> impl Fn() + Send + Sync + 'static {
    move || {
        let _ = proxy.send_event(eframe::UserEvent::RequestRepaint {
            viewport_id: egui::ViewportId::ROOT,
            when: Instant::now(),
            cumulative_pass_nr: 0,
        });
    }
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
}
