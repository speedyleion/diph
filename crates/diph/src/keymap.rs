//! Key sequences, such as vim's `gg`, bound to actions.

use eframe::egui::{Event, InputState, KeyboardShortcut};

/// Actions bound to sequences of key presses, which may span several frames.
/// A key that neither completes nor continues a sequence cancels it, along
/// with the keys pressed before it.
pub struct Keymap<A> {
    bindings: Vec<Binding<A>>,
    /// Keys pressed so far of a sequence not yet complete.
    pending: Vec<KeyboardShortcut>,
}

struct Binding<A> {
    keys: Vec<KeyboardShortcut>,
    action: A,
}

impl<A> Binding<A> {
    /// Whether `pressed` is the start of this binding's keys, or all of them.
    fn continues(&self, pressed: &[KeyboardShortcut]) -> bool {
        self.keys.len() >= pressed.len()
            && (self.keys.iter().zip(pressed)).all(|(key, pressed)| is_press_of(pressed, key))
    }
}

/// Whether `pressed` is `key` with exactly its modifiers.
fn is_press_of(pressed: &KeyboardShortcut, key: &KeyboardShortcut) -> bool {
    pressed.logical_key == key.logical_key && pressed.modifiers.matches_exact(key.modifiers)
}

impl<A: Copy> Keymap<A> {
    /// Binds each sequence of keys to its action.
    pub fn new(bindings: impl IntoIterator<Item = (Vec<KeyboardShortcut>, A)>) -> Self {
        let bindings = bindings
            .into_iter()
            .map(|(keys, action)| Binding { keys, action })
            .collect();
        Self {
            bindings,
            pending: Vec::new(),
        }
    }

    /// The actions of the sequences that the keys pressed this frame
    /// complete, in order.
    pub fn actions(&mut self, input: &InputState) -> Vec<A> {
        (input.events.iter())
            .filter_map(pressed_key)
            .filter_map(|key| self.press(key))
            .collect()
    }

    /// Adds `key` to the pending sequence, returning the action it completes.
    fn press(&mut self, key: KeyboardShortcut) -> Option<A> {
        self.pending.push(key);
        let continued: Vec<_> = (self.bindings.iter())
            .filter(|b| b.continues(&self.pending))
            .collect();
        let completed = (continued.iter())
            .find(|b| b.keys.len() == self.pending.len())
            .map(|b| b.action);
        if completed.is_some() || continued.is_empty() {
            self.pending.clear();
        }
        completed
    }
}

/// The key and modifiers of a key press, including repeats from holding it.
fn pressed_key(event: &Event) -> Option<KeyboardShortcut> {
    match event {
        Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => Some(KeyboardShortcut::new(*modifiers, *key)),
        _ => None,
    }
}
