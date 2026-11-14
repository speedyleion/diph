use std::path::PathBuf;
use std::time::{Duration, Instant};

use diph_core::Theme;
use eframe::egui;

use crate::menu::{MenuAction, MenuBar};
use crate::palette::Palette;
use crate::text_view::{SaveRequest, TextView};

pub(crate) struct SideFile {
    /// The name shown above the side.
    pub(crate) label: PathBuf,
    /// Where the side saves to; none when it can't be saved.
    pub(crate) save_path: Option<PathBuf>,
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
                &self.files,
                &mut self.save_message,
                &mut self.theme,
            );
        }
        self.view.process_input(ui);
        if let Some(request) = self.view.take_save_request() {
            handle_save_request(&self.files, &mut self.save_message, Some(request));
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
    files: &[SideFile; 2],
    save_message: &mut Option<String>,
    selected_theme: &mut &'static str,
) {
    match action {
        MenuAction::Save => handle_save_request(files, save_message, view.save_request()),
        MenuAction::Theme(name) => {
            if let Some(theme) = Theme::named(&name) {
                *selected_theme = theme.name();
                Palette::new(&theme).apply(ctx);
                view.set_theme(theme);
            }
        }
    }
}

pub(crate) fn handle_save_request(
    files: &[SideFile; 2],
    save_message: &mut Option<String>,
    request: Option<SaveRequest>,
) {
    let Some(request) = request else {
        return;
    };
    let file = &files[request.side];
    *save_message = Some(match &file.save_path {
        None => format!("{} can't be saved", file.label.display()),
        Some(path) => match std::fs::write(path, request.text) {
            Ok(()) => format!("saved {}", path.display()),
            Err(error) => format!("couldn't save {}: {error}", path.display()),
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
    use crate::text_view::{SaveRequest, TextView};

    use super::{SideFile, handle_menu_action, handle_save_request, show_status};

    fn test_view() -> TextView {
        TextView::new(SideBySide::new([
            Buffer::new("left\n"),
            Buffer::new("right\n"),
        ]))
    }

    fn test_files(right_save_path: Option<PathBuf>) -> [SideFile; 2] {
        [
            SideFile {
                label: "left.rs".into(),
                save_path: None,
            },
            SideFile {
                label: "right.rs".into(),
                save_path: right_save_path,
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

        let files = test_files(None);
        let mut save_message = None;
        handle_menu_action(
            MenuAction::Theme(name.to_owned()),
            &ctx,
            &mut view,
            &files,
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
    fn save_request_writes_the_requested_side() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("right.rs");
        let files = test_files(Some(file.clone()));
        let mut save_message = None;

        handle_save_request(
            &files,
            &mut save_message,
            Some(SaveRequest {
                side: 1,
                text: "edited right\n".to_owned(),
            }),
        );

        assert_eq!(std::fs::read_to_string(&file).unwrap(), "edited right\n");
        assert_eq!(save_message, Some(format!("saved {}", file.display())));
    }

    #[test]
    fn save_request_reports_an_unsavable_side() {
        let files = test_files(None);
        let mut save_message = None;

        handle_save_request(
            &files,
            &mut save_message,
            Some(SaveRequest {
                side: 1,
                text: "edited right\n".to_owned(),
            }),
        );

        assert_eq!(save_message.as_deref(), Some("right.rs can't be saved"));
    }

    #[test]
    fn save_request_reports_a_write_failure() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("missing/right.rs");
        let files = test_files(Some(file.clone()));
        let mut save_message = None;

        handle_save_request(
            &files,
            &mut save_message,
            Some(SaveRequest {
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
        let files = test_files(Some(file.clone()));
        let mut view = test_view();
        let mut save_message = None;
        let mut selected_theme = Theme::default().name();

        handle_menu_action(
            MenuAction::Save,
            &egui::Context::default(),
            &mut view,
            &files,
            &mut save_message,
            &mut selected_theme,
        );

        assert_eq!(std::fs::read_to_string(&file).unwrap(), "right\n");
        assert_eq!(save_message, Some(format!("saved {}", file.display())));
    }
}
