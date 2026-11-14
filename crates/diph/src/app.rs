use std::path::PathBuf;
use std::time::{Duration, Instant};

use diph_core::Theme;
use eframe::egui;

use crate::menu::{MenuAction, MenuBar};
use crate::palette::Palette;
use crate::text_view::{SaveTarget, TextView, ViewEvent};

pub(crate) struct SideFile {
    /// The name shown above the side.
    pub(crate) label: PathBuf,
    /// Where the side is read from and saved to.
    pub(crate) path: PathBuf,
    /// Whether the source text can be saved without replacing invalid bytes.
    pub(crate) saveable: bool,
}

/// The key the name of the picked theme is saved under between runs.
const THEME_KEY: &str = "theme";

pub(crate) struct App {
    view: TextView,
    files: [SideFile; 2],
    menu: MenuBar,
    theme: &'static str,
    save_message: Option<String>,
    started: Instant,
    startup_time: Option<Duration>,
}

impl App {
    pub(crate) fn new(
        mut view: TextView,
        files: [SideFile; 2],
        paths: [PathBuf; 2],
        started: Instant,
        storage: Option<&dyn eframe::Storage>,
        ctx: &egui::Context,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let theme = storage
            .and_then(|storage| storage.get_string(THEME_KEY))
            .and_then(|name| Theme::named(&name))
            .unwrap_or_default();
        let theme_name = theme.name();
        let menu = MenuBar::new(theme_name, wake);
        Palette::new(&theme).apply(ctx);
        view.set_theme(theme);
        view.highlight_in_background(paths, ctx.clone());
        Self {
            view,
            files,
            menu,
            theme: theme_name,
            save_message: None,
            started,
            startup_time: None,
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        for action in self.menu.ui(ui) {
            handle_menu_action(
                action,
                &ctx,
                &mut self.view,
                &mut self.files,
                &mut self.save_message,
                &mut self.theme,
            );
        }
        for event in self.view.process_input(ui) {
            match event {
                ViewEvent::Save => {
                    handle_save_target(
                        &self.files,
                        &mut self.save_message,
                        self.view.save_target(),
                    );
                }
                ViewEvent::Reload => {
                    handle_reload(&mut self.files, &mut self.view, &mut self.save_message);
                }
            }
        }
        let started = self.started;
        let startup_time = *self.startup_time.get_or_insert_with(|| started.elapsed());
        self.view
            .show_header(ui, [&self.files[0].label, &self.files[1].label]);
        show_status(
            ui,
            self.view.is_insert_mode(),
            self.save_message.as_deref(),
            startup_time,
        );
        self.view.show_diff(ui);
    }

    fn persist_egui_memory(&self) -> bool {
        // We use false here, because the scroll position will affect opening subsequent files.
        // Initial diff is on line 1 of file_a. We open a diff for file_b and
        // the first change is on line 400. When this data is stored file_b will scroll to line 400.
        // Without persisting file_b will appear to open at line 400.
        false
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        save_theme(storage, self.theme);
    }
}

fn handle_menu_action(
    action: MenuAction,
    ctx: &egui::Context,
    view: &mut TextView,
    files: &mut [SideFile; 2],
    save_message: &mut Option<String>,
    selected_theme: &mut &'static str,
) {
    match action {
        MenuAction::Save => handle_save_target(files, save_message, view.save_target()),
        MenuAction::Reload => handle_reload(files, view, save_message),
        MenuAction::Theme(name) => {
            if let Some(theme) = Theme::named(&name) {
                *selected_theme = theme.name();
                Palette::new(&theme).apply(ctx);
                view.set_theme(theme);
            }
        }
    }
}

fn handle_reload(
    files: &mut [SideFile; 2],
    view: &mut TextView,
    save_message: &mut Option<String>,
) {
    let [left, right] = [crate::read(&files[0].path), crate::read(&files[1].path)];
    let bytes = match (left, right) {
        (Ok(left), Ok(right)) => [left, right],
        (Err(error), _) | (_, Err(error)) => {
            *save_message = Some(format!("couldn't reload: {error}"));
            return;
        }
    };
    if bytes.iter().any(|bytes| crate::is_binary(bytes)) {
        *save_message = Some("couldn't reload: binary files differ".to_owned());
        return;
    }

    let diff = crate::text_diff(bytes.each_ref().map(Vec::as_slice));
    for (file, bytes) in files.iter_mut().zip(bytes) {
        file.saveable = crate::can_save(&file.path, &bytes);
    }
    view.reload(diff);
    *save_message = Some("reloaded".to_owned());
}

fn handle_save_target(
    files: &[SideFile; 2],
    save_message: &mut Option<String>,
    target: Option<SaveTarget>,
) {
    let Some(target) = target else {
        return;
    };
    let file = &files[target.side];
    *save_message = Some(match file.saveable {
        false => format!("{} can't be saved", file.label.display()),
        true => match std::fs::write(&file.path, target.text) {
            Ok(()) => format!("saved {}", file.path.display()),
            Err(error) => format!("couldn't save {}: {error}", file.path.display()),
        },
    });
}

fn show_status(
    ui: &mut egui::Ui,
    insert_mode: bool,
    save_message: Option<&str>,
    startup_time: Duration,
) {
    egui::Panel::bottom("status_bar").show(ui, |ui| {
        ui.horizontal(|ui| {
            if insert_mode {
                ui.strong("-- INSERT --");
            }
            if let Some(message) = save_message {
                ui.label(message);
            }
            ui.weak(format!("started in {} ms", startup_time.as_millis()));
        });
    });
}

fn save_theme(storage: &mut dyn eframe::Storage, theme: &str) {
    storage.set_string(THEME_KEY, theme.to_owned());
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use diph_core::{Buffer, SideBySide, Theme};
    use eframe::egui;
    use egui_kittest::{Harness, kittest::Queryable as _};

    use crate::menu::MenuAction;
    use crate::palette::Palette;
    use crate::text_view::{SaveTarget, TextView};

    use super::{SideFile, handle_menu_action, handle_reload, handle_save_target, show_status};

    fn test_view() -> TextView {
        TextView::new(SideBySide::new([
            Buffer::new("left\n"),
            Buffer::new("right\n"),
        ]))
    }

    fn test_files(right_path: Option<PathBuf>) -> [SideFile; 2] {
        let right_saveable = right_path.is_some();
        [
            SideFile {
                label: "left.rs".into(),
                path: "left.rs".into(),
                saveable: false,
            },
            SideFile {
                label: "right.rs".into(),
                path: right_path.unwrap_or_else(|| "right.rs".into()),
                saveable: right_saveable,
            },
        ]
    }

    #[test]
    fn theme_menu_action_updates_app_and_view_theme_state() {
        let mut view = test_view();
        let name = Theme::names()
            .find(|&name| name != Theme::default().name())
            .expect("there is more than one theme");
        let expected_background = Palette::new(&Theme::named(name).unwrap()).background;
        let ctx = egui::Context::default();
        let mut selected_theme = Theme::default().name();

        let mut files = test_files(None);
        let mut save_message = None;
        handle_menu_action(
            MenuAction::Theme(name.to_owned()),
            &ctx,
            &mut view,
            &mut files,
            &mut save_message,
            &mut selected_theme,
        );

        assert_eq!(selected_theme, name);
        let egui_theme = if expected_background.intensity() < 0.5 {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        assert_eq!(
            ctx.style_of(egui_theme).visuals.panel_fill,
            expected_background
        );
    }

    #[test]
    fn status_bar_shows_mode_feedback_and_startup_time() {
        let mut harness = Harness::new_ui_state(
            |ui, ()| show_status(ui, true, Some("saved right.rs"), Duration::from_millis(12)),
            (),
        );

        harness.run();

        harness.get_by_label("-- INSERT --");
        harness.get_by_label("saved right.rs");
        harness.get_by_label("started in 12 ms");
    }

    #[test]
    fn save_target_writes_the_requested_side() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("right.rs");
        let files = test_files(Some(file.clone()));
        let mut save_message = None;

        handle_save_target(
            &files,
            &mut save_message,
            Some(SaveTarget {
                side: 1,
                text: "edited right\n".to_owned(),
            }),
        );

        assert_eq!(std::fs::read_to_string(&file).unwrap(), "edited right\n");
        assert_eq!(save_message, Some(format!("saved {}", file.display())));
    }

    #[test]
    fn save_target_reports_an_unsavable_side() {
        let files = test_files(None);
        let mut save_message = None;

        handle_save_target(
            &files,
            &mut save_message,
            Some(SaveTarget {
                side: 1,
                text: "edited right\n".to_owned(),
            }),
        );

        assert_eq!(save_message.as_deref(), Some("right.rs can't be saved"));
    }

    #[test]
    fn save_target_reports_a_write_failure() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("missing/right.rs");
        let files = test_files(Some(file.clone()));
        let mut save_message = None;

        handle_save_target(
            &files,
            &mut save_message,
            Some(SaveTarget {
                side: 1,
                text: "edited right\n".to_owned(),
            }),
        );

        assert!(save_message.as_deref().is_some_and(|message| {
            message.starts_with(&format!("couldn't save {}:", file.display()))
        }));
    }

    #[test]
    fn menu_save_uses_the_app_save_handler() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("right.rs");
        let mut files = test_files(Some(file.clone()));
        let mut view = test_view();
        let mut save_message = None;
        let mut selected_theme = Theme::default().name();

        handle_menu_action(
            MenuAction::Save,
            &egui::Context::default(),
            &mut view,
            &mut files,
            &mut save_message,
            &mut selected_theme,
        );

        assert_eq!(std::fs::read_to_string(&file).unwrap(), "right\n");
        assert_eq!(save_message, Some(format!("saved {}", file.display())));
    }

    #[test]
    fn menu_reload_reads_the_source_files_into_the_view() {
        let dir = tempfile::tempdir().unwrap();
        let left = dir.path().join("left.rs");
        let right = dir.path().join("right.rs");
        std::fs::write(&left, "original left\n").unwrap();
        std::fs::write(&right, "original right\n").unwrap();
        let mut files = [
            SideFile {
                label: "left.rs".into(),
                path: left,
                saveable: false,
            },
            SideFile {
                label: "right.rs".into(),
                path: right.clone(),
                saveable: true,
            },
        ];
        let mut view = TextView::new(SideBySide::new([
            Buffer::new("original left\n"),
            Buffer::new("original right\n"),
        ]));
        std::fs::write(&files[0].path, "reloaded left\n").unwrap();
        std::fs::write(&files[1].path, "reloaded right\n").unwrap();
        let mut save_message = None;
        let mut selected_theme = Theme::default().name();

        handle_menu_action(
            MenuAction::Reload,
            &egui::Context::default(),
            &mut view,
            &mut files,
            &mut save_message,
            &mut selected_theme,
        );

        assert_eq!(view.save_target().unwrap().text, "reloaded right\n");
        assert_eq!(save_message.as_deref(), Some("reloaded"));
        assert!(files[0].saveable);
    }

    #[yare::parameterized(
        missing_source = { None, "couldn't reload:" },
        binary_source = { Some(b"binary\0data"), "couldn't reload: binary files differ" },
    )]
    fn menu_reload_errors_leave_the_open_diff_unchanged(
        right_contents: Option<&[u8]>,
        expected_message: &str,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let left = dir.path().join("left.rs");
        let right = dir.path().join("right.rs");
        std::fs::write(&left, "original left\n").unwrap();
        std::fs::write(&right, "original right\n").unwrap();
        let mut files = [
            SideFile {
                label: "left.rs".into(),
                path: left.clone(),
                saveable: true,
            },
            SideFile {
                label: "right.rs".into(),
                path: right.clone(),
                saveable: true,
            },
        ];
        let mut view = TextView::new(SideBySide::new([
            Buffer::new("original left\n"),
            Buffer::new("original right\n"),
        ]));
        std::fs::write(&files[0].path, "changed left\n").unwrap();
        match right_contents {
            Some(contents) => std::fs::write(&right, contents).unwrap(),
            None => std::fs::remove_file(&right).unwrap(),
        }
        let mut save_message = None;

        handle_reload(&mut files, &mut view, &mut save_message);

        assert_eq!(view.save_target().unwrap().text, "original right\n");
        assert!(
            save_message
                .as_deref()
                .is_some_and(|message| message.starts_with(expected_message))
        );
    }

    #[test]
    fn menu_reload_disables_saving_for_non_utf8_sources() {
        let dir = tempfile::tempdir().unwrap();
        let left = dir.path().join("left.rs");
        let right = dir.path().join("right.rs");
        std::fs::write(&left, "original left\n").unwrap();
        std::fs::write(&right, "original right\n").unwrap();
        let mut files = [
            SideFile {
                label: "left.rs".into(),
                path: left.clone(),
                saveable: true,
            },
            SideFile {
                label: "right.rs".into(),
                path: right.clone(),
                saveable: true,
            },
        ];
        let mut view = TextView::new(SideBySide::new([
            Buffer::new("original left\n"),
            Buffer::new("original right\n"),
        ]));
        std::fs::write(&files[1].path, b"reloaded \xff\n").unwrap();
        let mut save_message = None;

        handle_reload(&mut files, &mut view, &mut save_message);

        assert_eq!(view.save_target().unwrap().text, "reloaded \u{fffd}\n");
        assert!(!files[1].saveable);
    }
}
