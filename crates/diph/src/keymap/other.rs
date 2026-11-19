use eframe::egui::{Key, KeyboardShortcut, Modifiers};

pub(crate) fn save_shortcut() -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::CTRL, Key::S)
}

pub(crate) fn reload_shortcut() -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::NONE, Key::F5)
}
