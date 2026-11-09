//! The menu bar, whose View > Themes menu picks the color theme. On macOS
//! it is the system menu bar at the top of the screen, elsewhere a bar at
//! the top of the window.

#[cfg(not(target_os = "macos"))]
pub use in_window::MenuBar;
#[cfg(target_os = "macos")]
pub use native::MenuBar;

#[cfg(not(target_os = "macos"))]
mod in_window {
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
}

#[cfg(target_os = "macos")]
mod native {
    use std::sync::mpsc::{Receiver, channel};

    use diph_core::Theme;
    use eframe::egui;
    use muda::{CheckMenuItem, IsMenuItem, MenuEvent, PredefinedMenuItem, Submenu};

    /// The app's menus in the system menu bar.
    pub struct MenuBar {
        /// Keeps the menus alive.
        _bar: muda::Menu,
        /// The item of each theme, whose id is the theme's name.
        themes: Vec<CheckMenuItem>,
        events: Receiver<MenuEvent>,
    }

    impl MenuBar {
        /// Replaces the app's menus in the system menu bar, with `theme`
        /// picked. Choosing an item requests a repaint of `ctx`.
        pub fn new(ctx: &egui::Context, theme: &str) -> Self {
            let themes: Vec<_> = Theme::names()
                .map(|name| CheckMenuItem::with_id(name, name, true, name == theme, None))
                .collect();
            let bar = menu_bar(&themes).expect("menus are built from items without a parent");
            bar.init_for_nsapp();

            let (tx, events) = channel();
            let ctx = ctx.clone();
            // egui only draws a frame when something asks it to.
            MenuEvent::set_event_handler(Some(move |event| {
                let _ = tx.send(event);
                ctx.request_repaint();
            }));
            Self {
                _bar: bar,
                themes,
                events,
            }
        }

        /// The theme picked since the last call, if any.
        pub fn show(&mut self, _ui: &mut egui::Ui, _theme: &str) -> Option<&'static str> {
            let picked = self
                .events
                .try_iter()
                .filter_map(|event| Theme::names().find(|&name| event.id == name))
                .last()?;
            // Choosing an item toggles it, so a chosen theme that was already
            // checked would be unchecked.
            for item in &self.themes {
                item.set_checked(*item.id() == picked);
            }
            Some(picked)
        }
    }

    /// The app menu macOS expects first, then View > Themes with `themes`.
    fn menu_bar(themes: &[CheckMenuItem]) -> muda::Result<muda::Menu> {
        let separator = PredefinedMenuItem::separator;
        let app = Submenu::with_items(
            "diph",
            true,
            &[
                &PredefinedMenuItem::about(None, None),
                &separator(),
                &PredefinedMenuItem::services(None),
                &separator(),
                &PredefinedMenuItem::hide(None),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &separator(),
                &PredefinedMenuItem::quit(None),
            ],
        )?;
        let theme_items: Vec<&dyn IsMenuItem> =
            themes.iter().map(|item| item as &dyn IsMenuItem).collect();
        let themes = Submenu::with_items("Themes", true, &theme_items)?;
        let view = Submenu::with_items("View", true, &[&themes])?;
        muda::Menu::with_items(&[&app, &view])
    }
}
