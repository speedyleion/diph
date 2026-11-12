use std::path::PathBuf;

use diph_core::Theme;
use eframe::egui;

use crate::menu::{MenuAction, MenuBar};
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
    save_message: Option<String>,
}

impl App {
    pub(crate) fn new(
        mut view: TextView,
        files: [SideFile; 2],
        paths: [PathBuf; 2],
        storage: Option<&dyn eframe::Storage>,
        ctx: &egui::Context,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let theme = storage
            .and_then(|storage| storage.get_string(THEME_KEY))
            .and_then(|name| Theme::named(&name))
            .unwrap_or_default();
        let menu = MenuBar::new(theme.name(), wake);
        view.set_theme(ctx, theme);
        view.highlight_in_background(paths, ctx.clone());
        Self {
            view,
            files,
            menu,
            save_message: None,
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
            );
        }
        self.view.show(ui, self.save_message.as_deref());
        if let Some(request) = self.view.take_save_request() {
            handle_save_request(&self.files, &mut self.save_message, Some(request));
            ctx.request_repaint();
        }
    }

    fn persist_egui_memory(&self) -> bool {
        // We use false here, because the scroll position will affect opening subsequent files.
        // Initial diff is on line 1 of file_a. We open a diff for file_b and
        // the first change is on line 400. When this data is stored file_b will scroll to line 400.
        // Without persisting file_b will appear to open at line 400.
        false
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(THEME_KEY, self.view.theme().name().to_owned());
    }
}

fn handle_menu_action(
    action: MenuAction,
    ctx: &egui::Context,
    view: &mut TextView,
    files: &[SideFile; 2],
    save_message: &mut Option<String>,
) {
    match action {
        MenuAction::Save => handle_save_request(files, save_message, view.save_request()),
        MenuAction::Theme(name) => {
            if let Some(theme) = Theme::named(&name) {
                view.set_theme(ctx, theme);
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Instant;

    use diph_core::{Buffer, SideBySide, Theme};
    use eframe::egui;

    use crate::menu::MenuAction;
    use crate::text_view::{SaveRequest, TextView};

    use super::{SideFile, handle_menu_action, handle_save_request};

    fn test_view() -> TextView {
        TextView::new(
            SideBySide::new([Buffer::new("left\n"), Buffer::new("right\n")]),
            [PathBuf::from("left.rs"), PathBuf::from("right.rs")],
            Instant::now(),
        )
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
    fn theme_menu_action_updates_the_view() {
        let mut view = test_view();
        let name = Theme::names()
            .find(|&name| name != Theme::default().name())
            .expect("there is more than one theme");
        let ctx = egui::Context::default();

        let files = test_files(None);
        let mut save_message = None;
        handle_menu_action(
            MenuAction::Theme(name.to_owned()),
            &ctx,
            &mut view,
            &files,
            &mut save_message,
        );

        assert_eq!(view.theme().name(), name);
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

        handle_menu_action(
            MenuAction::Save,
            &egui::Context::default(),
            &mut view,
            &files,
            &mut save_message,
        );

        assert_eq!(std::fs::read_to_string(&file).unwrap(), "right\n");
        assert_eq!(save_message, Some(format!("saved {}", file.display())));
    }
}
