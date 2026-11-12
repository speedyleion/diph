//! Key sequences, such as vim's `gg`, bound to actions.

use eframe::egui::{Event, Key, KeyboardShortcut, Modifiers};

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

/// What a key sequence asks the view to do.
#[derive(Clone, Copy)]
pub(crate) enum Action {
    PrevChange,
    NextChange,
    CursorBottom,
    CursorTop,
    CursorCenter,
    CharLeft,
    CharRight,
    LineDown,
    LineUp,
    HalfPageDown,
    HalfPageUp,
    PageDown,
    PageUp,
    /// The line of the count, or the first line without one.
    FirstLine,
    /// The line of the count, or the last line without one.
    LastLine,
    LineStart,
    LineEnd,
    LeftSide,
    RightSide,
    Close,
    Save,
    Insert(InsertAt),
    OpenLine(OpenLineAt),
    NormalMode,
    NewLine,
    Tab,
    Backspace,
}

/// Where insert mode puts the cursor.
#[derive(Clone, Copy)]
pub(crate) enum InsertAt {
    /// Before the character under the cursor.
    BeforeCursor,
    /// After the character under the cursor.
    AfterCursor,
    /// At the first non-whitespace character on the line.
    LineStart,
    /// After the last character on the line.
    LineEnd,
}

#[derive(Clone, Copy)]
pub(crate) enum OpenLineAt {
    Above,
    Below,
}

pub(crate) fn normal_keymap() -> Keymap<Action> {
    let key = |key| KeyboardShortcut::new(Modifiers::NONE, key);
    let shift = |key| KeyboardShortcut::new(Modifiers::SHIFT, key);
    let ctrl = |key| KeyboardShortcut::new(Modifiers::CTRL, key);
    let command = |key| KeyboardShortcut::new(Modifiers::COMMAND, key);
    Keymap::new([
        (vec![key(Key::P)], Action::PrevChange),
        (vec![shift(Key::F7)], Action::PrevChange),
        (vec![key(Key::N)], Action::NextChange),
        (vec![key(Key::F7)], Action::NextChange),
        (vec![key(Key::Z), key(Key::B)], Action::CursorBottom),
        (vec![key(Key::Z), key(Key::T)], Action::CursorTop),
        (vec![key(Key::Z), key(Key::Z)], Action::CursorCenter),
        (vec![key(Key::H)], Action::CharLeft),
        (vec![key(Key::ArrowLeft)], Action::CharLeft),
        (vec![key(Key::L)], Action::CharRight),
        (vec![key(Key::ArrowRight)], Action::CharRight),
        (vec![key(Key::J)], Action::LineDown),
        (vec![key(Key::ArrowDown)], Action::LineDown),
        (vec![key(Key::K)], Action::LineUp),
        (vec![key(Key::ArrowUp)], Action::LineUp),
        (vec![ctrl(Key::D)], Action::HalfPageDown),
        (vec![ctrl(Key::U)], Action::HalfPageUp),
        (vec![ctrl(Key::F)], Action::PageDown),
        (vec![ctrl(Key::B)], Action::PageUp),
        (vec![key(Key::G), key(Key::G)], Action::FirstLine),
        (vec![shift(Key::G)], Action::LastLine),
        (vec![shift(Key::Num6)], Action::LineStart),
        (vec![shift(Key::Num4)], Action::LineEnd),
        (vec![ctrl(Key::W), key(Key::H)], Action::LeftSide),
        (vec![ctrl(Key::W), ctrl(Key::H)], Action::LeftSide),
        (vec![ctrl(Key::W), key(Key::L)], Action::RightSide),
        (vec![ctrl(Key::W), ctrl(Key::L)], Action::RightSide),
        (vec![shift(Key::Z), shift(Key::Z)], Action::Close),
        (vec![command(Key::S)], Action::Save),
        (vec![key(Key::I)], Action::Insert(InsertAt::BeforeCursor)),
        (vec![key(Key::A)], Action::Insert(InsertAt::AfterCursor)),
        (vec![shift(Key::I)], Action::Insert(InsertAt::LineStart)),
        (vec![shift(Key::A)], Action::Insert(InsertAt::LineEnd)),
        (vec![shift(Key::O)], Action::OpenLine(OpenLineAt::Above)),
        (vec![key(Key::O)], Action::OpenLine(OpenLineAt::Below)),
    ])
    .with_counts()
}

pub(crate) fn insert_keymap() -> Keymap<Action> {
    let key = |key| KeyboardShortcut::new(Modifiers::NONE, key);
    let command = |key| KeyboardShortcut::new(Modifiers::COMMAND, key);
    Keymap::new([
        (vec![key(Key::ArrowLeft)], Action::CharLeft),
        (vec![key(Key::ArrowRight)], Action::CharRight),
        (vec![key(Key::ArrowDown)], Action::LineDown),
        (vec![key(Key::ArrowUp)], Action::LineUp),
        (vec![key(Key::Escape)], Action::NormalMode),
        (vec![key(Key::Enter)], Action::NewLine),
        (vec![key(Key::Tab)], Action::Tab),
        (vec![key(Key::Backspace)], Action::Backspace),
        (vec![command(Key::S)], Action::Save),
    ])
}

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

    /// Whether `key` can add to the count.
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
