use eframe::egui::{Key, KeyboardShortcut, Modifiers};

pub(crate) fn save_shortcut() -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::COMMAND, Key::S)
}

pub(crate) fn reload_shortcut() -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::COMMAND, Key::R)
}
