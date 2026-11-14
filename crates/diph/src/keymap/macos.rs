use eframe::egui::{Key, KeyboardShortcut, Modifiers};

pub(super) fn save_shortcut() -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::COMMAND, Key::S)
}

pub(super) fn reload_shortcut() -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::COMMAND, Key::R)
}
