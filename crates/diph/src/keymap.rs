//! Key sequences, such as vim's `gg`, bound to actions.

use eframe::egui::{Event, Key, KeyboardShortcut, Modifiers};

#[cfg(target_os = "macos")]
#[path = "keymap/macos.rs"]
mod platform;
#[cfg(not(target_os = "macos"))]
#[path = "keymap/other.rs"]
mod platform;

pub(crate) use platform::{reload_shortcut, save_shortcut};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    FirstColumn,
    LineStart,
    LineEnd,
    WordForward,
    WordBackward,
    WordEnd,
    LeftSide,
    RightSide,
    Close,
    Save,
    Reload,
    Yank,
    StartCharacterSelection,
    StartLineSelection,
    CancelSelection,
    /// Opens a comment box on the selection or the cursor's line.
    Comment,
    NextComment,
    PrevComment,
    SaveComment,
    DiscardComment,
    Insert(InsertAt),
    OpenLine(OpenLineAt),
    NormalMode,
    NewLine,
    Tab,
    Backspace,
    /// Sets the named mark on the cursor's side and position.
    SetMark(char),
    /// Clears the named mark on the cursor's side.
    DeleteMark(char),
    /// Copies the change at the cursor from the other side to the cursor's.
    DiffGet,
    /// Copies the change at the cursor from the cursor's side to the other.
    DiffPut,
    /// Moves the cursor to the other side, toggling.
    ToggleSide,
    /// Moves the cursor to the named mark's line and column.
    GoToMark(char),
    /// Moves the cursor to the first non-whitespace character on the named
    /// mark's line.
    GoToMarkLine(char),
    /// Enters operator-pending mode, or applies the operator to an active
    /// selection.
    EnterOperator(Operator),
    /// Applies the pending operator to a motion or text object.
    OperatorTarget(Target),
    /// Repeats an operator key, acting linewise when it matches the pending
    /// operator.
    OperatorKey(Operator),
    /// Cancels operator-pending mode.
    CancelOperator,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operator {
    Delete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Motion {
    CharLeft,
    CharRight,
    LineDown,
    LineUp,
    FirstLine,
    LastLine,
    FirstColumn,
    LineStart,
    LineEnd,
    WordForward,
    WordBackward,
    WordEnd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextObject {
    InnerWord,
    AWord,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Line,
    Motion(Motion),
    TextObject(TextObject),
    Selection,
}

/// Where insert mode puts the cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OpenLineAt {
    Above,
    Below,
}

fn letter_key(c: char) -> Key {
    Key::from_name(&c.to_uppercase().to_string()).unwrap()
}

pub(crate) fn normal_keymap() -> Keymap<Action> {
    let key = |key| KeyboardShortcut::new(Modifiers::NONE, key);
    let shift = |key| KeyboardShortcut::new(Modifiers::SHIFT, key);
    let ctrl = |key| KeyboardShortcut::new(Modifiers::CTRL, key);
    let mark_bindings = ('a'..='z').flat_map(|c| {
        let set = (vec![key(Key::M), key(letter_key(c))], Action::SetMark(c));
        let go_to = (
            vec![key(Key::Backtick), key(letter_key(c))],
            Action::GoToMark(c),
        );
        let go_to_line = (
            vec![key(Key::Quote), key(letter_key(c))],
            Action::GoToMarkLine(c),
        );
        [set, go_to, go_to_line]
    });
    Keymap::new(
        [
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
            (vec![key(Key::Num0)], Action::FirstColumn),
            (vec![shift(Key::Num6)], Action::LineStart),
            (vec![shift(Key::Num4)], Action::LineEnd),
            (vec![key(Key::W)], Action::WordForward),
            (vec![key(Key::B)], Action::WordBackward),
            (vec![key(Key::E)], Action::WordEnd),
            (vec![ctrl(Key::W), key(Key::H)], Action::LeftSide),
            (vec![ctrl(Key::W), ctrl(Key::H)], Action::LeftSide),
            (vec![ctrl(Key::W), key(Key::L)], Action::RightSide),
            (vec![ctrl(Key::W), ctrl(Key::L)], Action::RightSide),
            (vec![ctrl(Key::W), ctrl(Key::W)], Action::ToggleSide),
            (vec![shift(Key::Z), shift(Key::Z)], Action::Close),
            (vec![save_shortcut()], Action::Save),
            (vec![reload_shortcut()], Action::Reload),
            (vec![key(Key::Y)], Action::Yank),
            (vec![key(Key::V)], Action::StartCharacterSelection),
            (vec![shift(Key::V)], Action::StartLineSelection),
            (vec![key(Key::Escape)], Action::CancelSelection),
            (vec![key(Key::C)], Action::Comment),
            (
                vec![key(Key::CloseBracket), key(Key::C)],
                Action::NextComment,
            ),
            (
                vec![key(Key::OpenBracket), key(Key::C)],
                Action::PrevComment,
            ),
            (vec![key(Key::I)], Action::Insert(InsertAt::BeforeCursor)),
            (vec![key(Key::A)], Action::Insert(InsertAt::AfterCursor)),
            (vec![shift(Key::I)], Action::Insert(InsertAt::LineStart)),
            (vec![shift(Key::A)], Action::Insert(InsertAt::LineEnd)),
            (vec![shift(Key::O)], Action::OpenLine(OpenLineAt::Above)),
            (vec![key(Key::O)], Action::OpenLine(OpenLineAt::Below)),
            (vec![key(Key::D)], Action::EnterOperator(Operator::Delete)),
        ]
        .into_iter()
        .chain(mark_bindings),
    )
    .with_counts()
}

pub(crate) fn operator_keymap() -> Keymap<Action> {
    let key = |key| KeyboardShortcut::new(Modifiers::NONE, key);
    let shift = |key| KeyboardShortcut::new(Modifiers::SHIFT, key);
    let delete_mark_bindings =
        ('a'..='z').map(|c| (vec![key(Key::M), key(letter_key(c))], Action::DeleteMark(c)));
    Keymap::new(
        [
            (vec![key(Key::D)], Action::OperatorKey(Operator::Delete)),
            (vec![key(Key::O)], Action::DiffGet),
            (vec![key(Key::P)], Action::DiffPut),
            (vec![key(Key::Escape)], Action::CancelOperator),
            (
                vec![key(Key::H)],
                Action::OperatorTarget(Target::Motion(Motion::CharLeft)),
            ),
            (
                vec![key(Key::ArrowLeft)],
                Action::OperatorTarget(Target::Motion(Motion::CharLeft)),
            ),
            (
                vec![key(Key::L)],
                Action::OperatorTarget(Target::Motion(Motion::CharRight)),
            ),
            (
                vec![key(Key::ArrowRight)],
                Action::OperatorTarget(Target::Motion(Motion::CharRight)),
            ),
            (
                vec![key(Key::J)],
                Action::OperatorTarget(Target::Motion(Motion::LineDown)),
            ),
            (
                vec![key(Key::ArrowDown)],
                Action::OperatorTarget(Target::Motion(Motion::LineDown)),
            ),
            (
                vec![key(Key::K)],
                Action::OperatorTarget(Target::Motion(Motion::LineUp)),
            ),
            (
                vec![key(Key::ArrowUp)],
                Action::OperatorTarget(Target::Motion(Motion::LineUp)),
            ),
            (
                vec![key(Key::G), key(Key::G)],
                Action::OperatorTarget(Target::Motion(Motion::FirstLine)),
            ),
            (
                vec![shift(Key::G)],
                Action::OperatorTarget(Target::Motion(Motion::LastLine)),
            ),
            (
                vec![key(Key::Num0)],
                Action::OperatorTarget(Target::Motion(Motion::FirstColumn)),
            ),
            (
                vec![shift(Key::Num6)],
                Action::OperatorTarget(Target::Motion(Motion::LineStart)),
            ),
            (
                vec![shift(Key::Num4)],
                Action::OperatorTarget(Target::Motion(Motion::LineEnd)),
            ),
            (
                vec![key(Key::W)],
                Action::OperatorTarget(Target::Motion(Motion::WordForward)),
            ),
            (
                vec![key(Key::B)],
                Action::OperatorTarget(Target::Motion(Motion::WordBackward)),
            ),
            (
                vec![key(Key::E)],
                Action::OperatorTarget(Target::Motion(Motion::WordEnd)),
            ),
            (
                vec![key(Key::I), key(Key::W)],
                Action::OperatorTarget(Target::TextObject(TextObject::InnerWord)),
            ),
            (
                vec![key(Key::A), key(Key::W)],
                Action::OperatorTarget(Target::TextObject(TextObject::AWord)),
            ),
        ]
        .into_iter()
        .chain(delete_mark_bindings),
    )
    .with_counts()
}

pub(crate) fn insert_keymap() -> Keymap<Action> {
    let key = |key| KeyboardShortcut::new(Modifiers::NONE, key);
    Keymap::new([
        (vec![key(Key::ArrowLeft)], Action::CharLeft),
        (vec![key(Key::ArrowRight)], Action::CharRight),
        (vec![key(Key::ArrowDown)], Action::LineDown),
        (vec![key(Key::ArrowUp)], Action::LineUp),
        (vec![key(Key::Escape)], Action::NormalMode),
        (vec![key(Key::Enter)], Action::NewLine),
        (vec![key(Key::Tab)], Action::Tab),
        (vec![key(Key::Backspace)], Action::Backspace),
        (vec![save_shortcut()], Action::Save),
    ])
}

pub(crate) fn comment_keymap() -> Keymap<Action> {
    let key = |key| KeyboardShortcut::new(Modifiers::NONE, key);
    Keymap::new([
        (vec![key(Key::Enter)], Action::SaveComment),
        (vec![key(Key::Escape)], Action::DiscardComment),
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

    /// Clears the count and any pending sequence.
    pub fn clear(&mut self) {
        self.count = None;
        self.pending.clear();
    }

    /// Whether no count or pending key sequence is active.
    pub fn is_idle(&self) -> bool {
        self.count.is_none() && self.pending.is_empty()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn shortcut(modifiers: Modifiers, key: Key) -> KeyboardShortcut {
        KeyboardShortcut::new(modifiers, key)
    }

    #[yare::parameterized(
        repeat_delete_key = { vec![shortcut(Modifiers::NONE, Key::D)], Action::OperatorKey(Operator::Delete), None },
        diff_get = { vec![shortcut(Modifiers::NONE, Key::O)], Action::DiffGet, None },
        diff_put = { vec![shortcut(Modifiers::NONE, Key::P)], Action::DiffPut, None },
        delete_mark = { vec![shortcut(Modifiers::NONE, Key::M), shortcut(Modifiers::NONE, Key::A)], Action::DeleteMark('a'), None },
        inner_word = { vec![shortcut(Modifiers::NONE, Key::I), shortcut(Modifiers::NONE, Key::W)], Action::OperatorTarget(Target::TextObject(TextObject::InnerWord)), None },
        a_word = { vec![shortcut(Modifiers::NONE, Key::A), shortcut(Modifiers::NONE, Key::W)], Action::OperatorTarget(Target::TextObject(TextObject::AWord)), None },
        line_end = { vec![shortcut(Modifiers::SHIFT, Key::Num4)], Action::OperatorTarget(Target::Motion(Motion::LineEnd)), None },
        zero_alone_is_first_column = { vec![shortcut(Modifiers::NONE, Key::Num0)], Action::OperatorTarget(Target::Motion(Motion::FirstColumn)), None },
        counted_word_forward = { vec![shortcut(Modifiers::NONE, Key::Num2), shortcut(Modifiers::NONE, Key::W)], Action::OperatorTarget(Target::Motion(Motion::WordForward)), Some(2) },
        counted_first_line = { vec![shortcut(Modifiers::NONE, Key::Num1), shortcut(Modifiers::NONE, Key::Num0), shortcut(Modifiers::NONE, Key::G), shortcut(Modifiers::NONE, Key::G)], Action::OperatorTarget(Target::Motion(Motion::FirstLine)), Some(10) },
        escape_cancels = { vec![shortcut(Modifiers::NONE, Key::Escape)], Action::CancelOperator, None },
    )]
    fn operator_keymap_resolves_sequences(
        keys: Vec<KeyboardShortcut>,
        expected_action: Action,
        expected_count: Option<usize>,
    ) {
        let mut map = operator_keymap();
        let results: Vec<_> = keys.into_iter().map(|key| map.press(key)).collect();
        let expected: Vec<_> = std::iter::repeat_n(None, results.len().saturating_sub(1))
            .chain(std::iter::once(Some((expected_action, expected_count))))
            .collect();
        assert_eq!(results, expected);
        assert!(map.is_idle());
    }

    #[test]
    fn operator_keymap_stays_active_during_partial_sequence_and_clears_on_mismatch() {
        let mut map = operator_keymap();
        assert_eq!(map.press(shortcut(Modifiers::NONE, Key::I)), None);
        assert!(!map.is_idle());

        assert_eq!(map.press(shortcut(Modifiers::NONE, Key::X)), None);
        assert!(map.is_idle());
    }
}
