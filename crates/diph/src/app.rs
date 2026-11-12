use std::path::PathBuf;

use diph_core::Theme;
use eframe::egui;

use crate::menu::{MenuAction, MenuBar};
use crate::text_view::TextView;

/// The key the name of the picked theme is saved under between runs.
const THEME_KEY: &str = "theme";

pub(crate) struct App {
    view: TextView,
    menu: MenuBar,
}

impl App {
    pub(crate) fn new(
        mut view: TextView,
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
        Self { view, menu }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        for action in self.menu.ui(ui) {
            handle_menu_action(action, &ctx, &mut self.view);
        }
        self.view.show(ui);
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

fn handle_menu_action(action: MenuAction, ctx: &egui::Context, view: &mut TextView) {
    match action {
        MenuAction::Save => view.save(),
        MenuAction::Theme(name) => {
            if let Some(theme) = Theme::named(&name) {
                view.set_theme(ctx, theme);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Instant;

    use diph_core::{Buffer, SideBySide, Theme};
    use eframe::egui;

    use crate::menu::MenuAction;
    use crate::text_view::{SideFile, TextView};

    use super::handle_menu_action;

    #[test]
    fn theme_menu_action_updates_the_view() {
        let mut view = TextView::new(
            SideBySide::new([Buffer::new("left\n"), Buffer::new("right\n")]),
            SideFile {
                label: PathBuf::from("left.rs"),
                save_path: None,
            },
            SideFile {
                label: PathBuf::from("right.rs"),
                save_path: None,
            },
            Instant::now(),
        );
        let name = Theme::names()
            .find(|&name| name != Theme::default().name())
            .expect("there is more than one theme");
        let ctx = egui::Context::default();

        handle_menu_action(MenuAction::Theme(name.to_owned()), &ctx, &mut view);

        assert_eq!(view.theme().name(), name);
    }
}
