use std::sync::mpsc::{Receiver, channel};

use diph_core::Theme;
use eframe::egui;
use muda::{
    CheckMenuItem, IsMenuItem, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
    accelerator::{Key as MudaKey, KeyAccelerator, Modifiers},
};

use crate::{keymap, menu::MenuAction};

/// The app's menus in the system menu bar.
pub struct MenuBar {
    /// Keeps the menus alive.
    _bar: muda::Menu,
    /// The item of each theme, whose id is the theme's name.
    themes: Vec<CheckMenuItem>,
    events: Receiver<MenuEvent>,
}

impl MenuBar {
    /// Replaces the app's menus in the system menu bar, with `theme` picked.
    /// Choosing an item wakes the application event loop.
    pub fn new(theme: &str, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let themes: Vec<_> = Theme::names()
            .map(|name| CheckMenuItem::with_id(name, name, true, name == theme, None))
            .collect();
        let bar = menu_bar(&themes).expect("menus are built from items without a parent");
        bar.init_for_nsapp();

        let (tx, events) = channel();
        MenuEvent::set_event_handler(Some(move |event| {
            let _ = tx.send(event);
            wake();
        }));
        Self {
            _bar: bar,
            themes,
            events,
        }
    }

    /// Returns all recognized menu actions picked since the last call.
    pub fn ui(&mut self, _ui: &mut egui::Ui) -> Vec<MenuAction> {
        let picked: Vec<_> = self
            .events
            .try_iter()
            .filter_map(|event| match event.id.0.as_str() {
                "save" => Some(MenuAction::Save),
                "reload" => Some(MenuAction::Reload),
                id => Theme::names()
                    .find(|&name| name == id)
                    .map(|name| MenuAction::Theme(name.to_owned())),
            })
            .collect();
        picked
            .into_iter()
            .map(|action| match action {
                MenuAction::Theme(picked) => {
                    // Choosing an item toggles it, so a chosen theme that was already
                    // checked would be unchecked.
                    for item in &self.themes {
                        item.set_checked(*item.id() == picked);
                    }
                    MenuAction::Theme(picked)
                }
                action => action,
            })
            .collect()
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
    let save = MenuItem::with_id("save", "Save", true, None);
    save.set_key_accelerator(accelerator(keymap::save_shortcut()))?;
    let reload = MenuItem::with_id("reload", "Reload", true, None);
    reload.set_key_accelerator(accelerator(keymap::reload_shortcut()))?;
    let file = Submenu::with_items("File", true, &[&save, &reload])?;
    muda::Menu::with_items(&[&app, &file, &view])
}

fn accelerator(shortcut: egui::KeyboardShortcut) -> Option<KeyAccelerator> {
    let modifiers = [
        (shortcut.modifiers.command, Modifiers::META),
        (shortcut.modifiers.ctrl, Modifiers::CONTROL),
        (shortcut.modifiers.alt, Modifiers::ALT),
        (shortcut.modifiers.shift, Modifiers::SHIFT),
    ]
    .into_iter()
    .filter_map(|(pressed, modifier)| pressed.then_some(modifier))
    .fold(Modifiers::empty(), |modifiers, modifier| {
        modifiers | modifier
    });
    let key = match shortcut.logical_key {
        egui::Key::S => MudaKey::Character("s".to_owned()),
        egui::Key::R => MudaKey::Character("r".to_owned()),
        egui::Key::F => MudaKey::Character("f".to_owned()),
        _ => return None,
    };
    Some(KeyAccelerator::new(modifiers, key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accelerator_maps_combined_modifiers_and_logical_key() {
        let shortcut = egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::ALT,
            egui::Key::F,
        );

        assert_eq!(
            accelerator(shortcut),
            Some(KeyAccelerator::new(
                Modifiers::META | Modifiers::ALT,
                MudaKey::Character("f".to_owned()),
            )),
        );
    }
}
