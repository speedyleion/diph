//! Key sequences, such as vim's `gg`, bound to actions.

use eframe::egui::{Event, Key, KeyboardShortcut};

/// The digit keys, in order from 0.
const DIGITS: [Key; 10] = [
    Key::Num0,
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num6,
    Key::Num7,
    Key::Num8,
    Key::Num9,
];

/// The modifier keys, whose own presses are not keys of a sequence.
const MODIFIER_KEYS: [Key; 8] = [
    Key::ShiftLeft,
    Key::ShiftRight,
    Key::ControlLeft,
    Key::ControlRight,
    Key::AltLeft,
    Key::AltRight,
    Key::SuperLeft,
    Key::SuperRight,
];

/// Actions bound to sequences of key presses, which may span several frames.
/// A key that neither completes nor continues a sequence cancels it, along
/// with the keys pressed before it.
pub struct Keymap<A> {
    bindings: Vec<Binding<A>>,
    /// Whether digits typed before a sequence give it a count.
    counts: bool,
    /// The count typed so far before the pending sequence.
    count: Option<usize>,
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
            counts: false,
            count: None,
            pending: Vec::new(),
        }
    }

    /// Lets digits typed before a sequence give it a count, as in vim's
    /// `10gg`. A 0 is part of a count only after another digit, so a 0 on
    /// its own stays free to bind.
    pub fn with_counts(self) -> Self {
        Self {
            counts: true,
            ..self
        }
    }

    /// Adds `key` to the pending sequence, returning the action it completes
    /// and the count typed before the sequence, if any, which is never 0.
    pub fn press(&mut self, key: KeyboardShortcut) -> Option<(A, Option<usize>)> {
        if let Some(digit) = self.count_digit(&key) {
            let count = self.count.unwrap_or(0).saturating_mul(10);
            self.count = Some(count.saturating_add(digit));
            return None;
        }
        self.pending.push(key);
        let continued: Vec<_> = self
            .bindings
            .iter()
            .filter(|b| b.continues(&self.pending))
            .collect();
        let completed = continued
            .iter()
            .find(|b| b.keys.len() == self.pending.len())
            .map(|b| b.action);
        if completed.is_some() || continued.is_empty() {
            self.pending.clear();
            let count = self.count.take();
            return completed.map(|action| (action, count));
        }
        None
    }

    /// Whether `key` can add to the count: a key without modifiers typed
    /// before a sequence's first key, in a keymap with counts.
    fn can_count(&self, key: &KeyboardShortcut) -> bool {
        self.counts && self.pending.is_empty() && key.modifiers.is_none()
    }

    /// The digit `key` adds to the count, if it adds to one.
    fn count_digit(&self, key: &KeyboardShortcut) -> Option<usize> {
        if !self.can_count(key) {
            return None;
        }
        let digit = DIGITS.iter().position(|d| *d == key.logical_key)?;
        (digit > 0 || self.count.is_some()).then_some(digit)
    }
}

/// The key and modifiers of a key press, including repeats from holding it.
/// None for a press of a modifier key itself.
pub fn pressed_key(event: &Event) -> Option<KeyboardShortcut> {
    match event {
        Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } if !MODIFIER_KEYS.contains(key) => Some(KeyboardShortcut::new(*modifiers, *key)),
        _ => None,
    }
}
