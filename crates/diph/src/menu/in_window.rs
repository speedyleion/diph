use diph_core::Theme;
use eframe::egui;

pub struct MenuBar;

impl MenuBar {
    pub fn new(_ctx: &egui::Context, _theme: &str) -> Self {
        Self
    }

    /// Shows the bar at the top of `ui`, with `theme` picked. Returns
    /// the theme picked, if any.
    pub fn show(&mut self, ui: &mut egui::Ui, theme: &str) -> Option<&'static str> {
        let bar = egui::Panel::top("menu_bar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                let view = ui.menu_button("View", |ui| {
                    let themes = ui.menu_button("Themes", |ui| theme_buttons(ui, theme));
                    themes.inner.flatten()
                });
                view.inner.flatten()
            })
        });
        bar.inner.inner
    }
}

/// A radio button per theme, with `theme`'s selected. Returns the theme
/// clicked, if any.
fn theme_buttons(ui: &mut egui::Ui, theme: &str) -> Option<&'static str> {
    Theme::names()
        .filter(|&name| ui.radio(name == theme, name).clicked())
        .last()
}
