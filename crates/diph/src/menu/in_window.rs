use diph_core::Theme;
use eframe::egui;

use crate::menu::MenuAction;

pub struct MenuBar {
    theme: &'static str,
}

impl MenuBar {
    pub fn new(theme: &str, _wake: impl Fn() + Send + Sync + 'static) -> Self {
        let theme = Theme::names()
            .find(|&name| name == theme)
            .unwrap_or_else(|| Theme::names().next().expect("there is a default theme"));
        Self { theme }
    }

    /// Shows the bar at the top of `ui` and returns its selected actions.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Vec<MenuAction> {
        let mut actions = Vec::new();
        egui::Panel::top("menu_bar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Save").clicked() {
                        actions.push(MenuAction::Save);
                        ui.close();
                    }
                    if ui.button("Reload").clicked() {
                        actions.push(MenuAction::Reload);
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.menu_button("Themes", |ui| {
                        if let Some(action) = self.theme_buttons(ui, self.theme) {
                            actions.push(action);
                        }
                    });
                });
            })
        });
        actions
    }

    fn theme_buttons(&mut self, ui: &mut egui::Ui, theme: &str) -> Option<MenuAction> {
        for name in Theme::names() {
            if ui.selectable_label(name == theme, name).clicked() {
                self.theme = name;
                return Some(MenuAction::Theme(name.to_owned()));
            }
        }
        None
    }
}
