use eframe::egui::{Key, KeyboardShortcut, Modifiers};

pub(super) fn save_shortcut() -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::COMMAND, Key::S)
}
