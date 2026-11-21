//! Side-by-side text diff view.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, OnceLock};

use diph_core::{
    Buffer, Edit, Highlighter, Line, Position, Row, RowKind, SideBySide, SideColors, Span, Theme,
};
use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, FontId, Galley, Layout, Painter, Pos2, Rect,
    ScrollArea, Sense, Stroke, StrokeKind, TextFormat, TextStyle, UiBuilder, epaint::text::Glyph,
    pos2, text::LayoutJob, vec2,
};
use itertools::{
    EitherOrBoth::{Both, Left, Right},
    Itertools as _,
};

use crate::comment::{Comment, CommentRange};
use crate::cursor::Cursor;
use crate::keymap::{
    Action, InsertAt, Keymap, OpenLineAt, comment_keymap, insert_keymap, normal_keymap, pressed_key,
};
use crate::palette::{CHANGED_MARK, COMMENT_MARK, ONE_SIDED_MARK, Palette};
use crate::review::ReviewComment;
use crate::selection::{Selection, SelectionKind, SelectionRange};
use crate::view_rows::{Gap, ViewRow, ViewRows};

/// Width of the cursor in insert mode, a bar before its column.
const CURSOR_BAR_WIDTH: f32 = 2.0;

/// Width of the gutter mark of a commented line.
const COMMENT_MARK_WIDTH: f32 = 3.0;

/// Width of the change map beside the rows.
const MAP_WIDTH: f32 = 16.0;
/// Least height of a change map mark, so one row of a long diff still shows.
const MIN_MARK_HEIGHT: f32 = 3.0;

/// Rows of unchanged context kept above a change when jumping to it.
const CONTEXT_ROWS: usize = 3;
/// Lines highlighted per message from the background highlighter, so the top
/// of a large file gets colors without waiting for the rest.
const HIGHLIGHT_CHUNK: usize = 1000;

/// Syntax colors for a run of lines on one side.
struct ColorsChunk {
    /// 0 for the left side, 1 for the right.
    side: usize,
    /// Which highlighting of the side the colors come from.
    run: usize,
    lines: Vec<Vec<Span>>,
}

/// Which keymap keys go to, as in vim.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Normal,
    Insert,
    /// Writing a comment, whose box takes the typed text.
    Comment,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ViewEvent {
    Save,
    Reload,
    Yank(String),
    /// A comment was saved.
    CommentsChanged,
}

/// What the view did with an input event.
enum Handled {
    /// Took the event, with what it asks of the app, if anything.
    Taken(Option<ViewEvent>),
    /// Left the event to the widgets.
    Left,
}

/// Which comment a comment box shows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shows {
    /// The saved comment at this index.
    Saved(usize),
    Draft,
}

/// A comment box under the last line its comment is on.
struct CommentBox {
    gap: Gap,
    /// 0 for the left side, 1 for the right.
    side: usize,
    shows: Shows,
}

/// The text a comment box holds.
enum BoxText<'a> {
    Saved(&'a str),
    Draft(&'a mut String),
}

pub(crate) struct SaveTarget {
    pub(crate) side: usize,
    pub(crate) text: String,
}

pub struct TextView {
    diff: SideBySide,
    /// Index into `diff.changes()` of the change last jumped to.
    current_change: Option<usize>,
    /// Row of the view to scroll to its top on the next frame.
    scroll_to_top_row: Option<usize>,
    /// Top row of the view wholly in view in the last frame.
    top_row: usize,
    /// Rows of the view wholly in view in the last frame.
    rows_in_view: usize,
    /// How far the text of both sides is scrolled left, in points.
    scroll_x: f32,
    /// The cursor position the view last scrolled horizontally to show.
    scrolled_to_cursor: Option<(usize, Position)>,
    /// The side with the cursor, 0 for the left and 1 for the right. `None`
    /// when neither side has lines.
    cursor_side: Option<usize>,
    /// The cursor in the buffer of `cursor_side`.
    cursor: Cursor,
    selection: Option<Selection>,
    comments: Vec<Comment>,
    /// The comment being written in comment mode.
    draft: Option<Comment>,
    /// The index of the saved comment the draft edits, which shows the
    /// draft's box instead of its own while it does.
    editing: Option<usize>,
    /// The gaps under comment boxes in the last frame.
    gaps: Vec<Gap>,
    mode: Mode,
    /// Whether the last key pressed was bound to an action, so the text it
    /// typed is taken and dropped.
    key_handled: bool,
    normal_keys: Keymap<Action>,
    insert_keys: Keymap<Action>,
    comment_keys: Keymap<Action>,
    /// Syntax colors of the left and right side, filled in from the top as
    /// the background highlighter delivers them.
    colors: [SideColors; 2],
    colors_rx: Option<Receiver<ColorsChunk>>,
    /// The current highlighting of the left and right side. Colors from an
    /// earlier one are for text since edited or in an earlier theme, so
    /// they are dropped.
    highlight_runs: Arc<[AtomicUsize; 2]>,
    /// Lines of each side's colors from its current highlighting. The
    /// colors past them are from before the side's last edit or theme
    /// change.
    colors_received: [usize; 2],
    /// What highlighting a side needs, once highlighting has started.
    highlighting: Option<Highlighting>,
    /// The theme the sides are highlighted in.
    theme: Arc<Theme>,
    /// The colors of `theme` the view paints with.
    palette: Palette,
}

impl TextView {
    /// A view of `diff`.
    pub fn new(diff: SideBySide, comments: Vec<Comment>) -> Self {
        let buffers = diff.buffers();
        // The right side, unless only the left has lines.
        let cursor_side = [1, 0].into_iter().find(|&s| !buffers[s].lines().is_empty());
        let theme = Theme::default();
        let mut view = Self {
            cursor_side,
            cursor: Cursor::default(),
            selection: None,
            comments,
            draft: None,
            editing: None,
            gaps: Vec::new(),
            diff,
            current_change: None,
            scroll_to_top_row: None,
            top_row: 0,
            rows_in_view: 0,
            scroll_x: 0.0,
            scrolled_to_cursor: None,
            mode: Mode::Normal,
            key_handled: false,
            normal_keys: normal_keymap(),
            insert_keys: insert_keymap(),
            comment_keys: comment_keymap(),
            colors: Default::default(),
            colors_rx: None,
            highlight_runs: Default::default(),
            colors_received: [0; 2],
            highlighting: None,
            palette: Palette::new(&theme),
            theme: Arc::new(theme),
        };
        view.reset_diff_state();
        if !view.diff.changes().is_empty() {
            view.jump_to_change(0);
        }
        view
    }

    /// Replaces the compared text, keeping the cursor within the new text.
    pub(crate) fn reload(&mut self, diff: SideBySide) {
        let cursor_row = self
            .cursor_position()
            .map(|(side, position)| self.view_row_of(side, position.line));
        let top_row = self.scroll_to_top_row.unwrap_or(self.top_row);
        let cursor_offset = cursor_row.map(|row| {
            row.saturating_sub(top_row)
                .min(self.rows_in_view.saturating_sub(1))
        });
        self.diff = diff;
        self.cursor_side = self
            .cursor_side
            .filter(|&side| !self.diff.buffers()[side].lines().is_empty())
            .or_else(|| {
                [1, 0]
                    .into_iter()
                    .find(|&side| !self.diff.buffers()[side].lines().is_empty())
            });
        if let Some(side) = self.cursor_side {
            self.cursor.clamp(&self.diff.buffers()[side]);
        }
        self.scroll_to_top_row = self.cursor_position().map(|(side, position)| {
            self.view_row_of(side, position.line)
                .saturating_sub(cursor_offset.unwrap_or(CONTEXT_ROWS))
        });
        self.reset_diff_state();
        self.rehighlight(0);
        self.rehighlight(1);
    }

    fn reset_diff_state(&mut self) {
        self.current_change = None;
        self.top_row = 0;
        self.rows_in_view = 0;
        self.mode = Mode::Normal;
        self.selection = None;
        self.draft = None;
        self.editing = None;
        self.key_handled = false;
        self.normal_keys.clear();
        self.insert_keys.clear();
        self.comment_keys.clear();
        self.colors = Default::default();
        self.colors_received = [0; 2];
    }

    /// Starts highlighting both sides on background threads, picking each
    /// side's language from its path. Lines show uncolored until their
    /// colors arrive.
    pub fn highlight_in_background(&mut self, paths: [PathBuf; 2], ctx: egui::Context) {
        let (tx, rx) = channel();
        self.colors_rx = Some(rx);
        self.highlighting = Some(Highlighting {
            paths,
            highlighter: Arc::default(),
            runs: self.highlight_runs.clone(),
            tx,
            ctx,
        });
        self.highlight(0);
        self.highlight(1);
    }

    /// Highlights `side` on a background thread, if highlighting has
    /// started.
    fn highlight(&self, side: usize) {
        let Some(highlighting) = self.highlighting.clone() else {
            return;
        };
        let run = self.highlight_runs[side].load(Ordering::Relaxed);
        let lines = self.diff.buffers()[side].lines().to_vec();
        let theme = self.theme.clone();
        std::thread::spawn(move || highlighting.highlight_side(side, run, &theme, &lines));
    }

    /// Starts highlighting `side` again. Its colors are replaced from the
    /// top as the new ones arrive.
    fn rehighlight(&mut self, side: usize) {
        self.highlight_runs[side].fetch_add(1, Ordering::Relaxed);
        self.colors_received[side] = 0;
        self.highlight(side);
    }

    /// Highlights both sides again and paints the view in `theme`'s colors.
    pub(crate) fn set_theme(&mut self, theme: Theme) {
        self.palette = Palette::new(&theme);
        self.theme = Arc::new(theme);
        self.rehighlight(0);
        self.rehighlight(1);
    }

    /// What the status bar shows for the mode, if anything.
    pub(crate) fn mode_label(&self) -> Option<&'static str> {
        match self.mode {
            Mode::Normal => None,
            Mode::Insert => Some("-- INSERT --"),
            Mode::Comment => Some("-- COMMENT --"),
        }
    }

    /// Handles this frame's input, leaving the widgets only the events the
    /// view did not take, such as the text typed into a comment box.
    pub(crate) fn process_input(&mut self, ui: &mut egui::Ui) -> Vec<ViewEvent> {
        let events = ui.input(|i| i.events.clone());
        let mut view_events = Vec::new();
        let mut untaken = Vec::new();
        for event in events {
            match self.handle(ui.ctx(), &event) {
                Handled::Taken(view_event) => view_events.extend(view_event),
                Handled::Left => untaken.push(event),
            }
        }
        ui.input_mut(|i| i.events = untaken);
        view_events
    }

    pub(crate) fn show_header(&mut self, ui: &mut egui::Ui, labels: [&Path; 2]) {
        egui::Panel::top("text_header").show(ui, |ui| {
            ui.horizontal(|ui| {
                // Without keyboard focus, which Tab would give and Space or
                // Enter would press with, since keys go to the keymaps.
                let button = |text| egui::Button::new(text).sense(Sense::CLICK);
                if ui
                    .add(button("Prev"))
                    .on_hover_text("p / Shift+F7")
                    .clicked()
                {
                    self.step_change(-1);
                }
                if ui.add(button("Next")).on_hover_text("n / F7").clicked() {
                    self.step_change(1);
                }
                let current = self
                    .current_change
                    .map_or("–".to_owned(), |c| (c + 1).to_string());
                ui.label(format!("change {current} of {}", self.diff.changes().len()));
            });
            ui.columns(2, |cols| {
                for (column, label) in cols.iter_mut().zip(labels) {
                    let font = TextStyle::Monospace.resolve(column.style());
                    let char_width = column.fonts_mut(|f| f.glyph_width(&font, '0'));
                    let label = truncate_label(label, column.available_width(), char_width);
                    column.monospace(label);
                }
            });
        });
    }

    /// Shows the diff, returning what it asks of the app, if anything.
    pub(crate) fn show_diff(&mut self, ui: &mut egui::Ui) -> Option<ViewEvent> {
        if let Some(rx) = &self.colors_rx {
            for chunk in rx.try_iter() {
                let side = chunk.side;
                if chunk.run != self.highlight_runs[side].load(Ordering::Relaxed) {
                    continue;
                }
                // Replaces the colors from before the last edit.
                let colors = &mut self.colors[side].lines;
                let start = self.colors_received[side];
                let end = (start + chunk.lines.len()).min(colors.len());
                self.colors_received[side] += chunk.lines.len();
                colors.splice(start..end, chunk.lines);
            }
        }
        egui::CentralPanel::default()
            .show(ui, |ui| self.diff_ui(ui))
            .inner
    }

    /// The rows, beside a map of the whole diff that marks each side's
    /// changes in a column of its own and covers the rows in view. Returns
    /// what the view asks of the app, if anything.
    fn diff_ui(&mut self, ui: &mut egui::Ui) -> Option<ViewEvent> {
        let rect = ui.available_rect_before_wrap();
        let (rows_rect, map) = rect.split_left_right_at_x(rect.right() - MAP_WIDTH);
        let rows_builder = UiBuilder::new().max_rect(rows_rect);
        let (in_view, action) = ui.scope_builder(rows_builder, |ui| self.rows_ui(ui)).inner;

        let view_rows = self.view_rows();
        let y = map_y(map, view_rows.len(), in_view.end - in_view.start);
        let diff_row_y = |row: f32| y(view_rows.view_row(row as usize) as f32);
        let painter = ui.painter();
        painter.rect_filled(map, 0.0, self.palette.filler);
        for (mark, color) in change_marks(self.diff.rows(), map, diff_row_y) {
            painter.rect_filled(mark, 0.0, color);
        }
        let comments = self.comments.iter().filter(|c| self.has_line(c.range));
        let comment_rows =
            comments.map(|c| self.diff.row_of(c.range.side, c.range.range.start.line));
        for mark in comment_marks(comment_rows, map, diff_row_y) {
            painter.rect_filled(mark, 0.0, COMMENT_MARK);
        }
        let cover = Rect::from_x_y_ranges(map.x_range(), y(in_view.start)..=y(in_view.end));
        painter.rect_filled(cover, 0.0, self.palette.in_view_cover);
        let action = action?;
        // Shows what the action changed, such as the mode, without waiting
        // for more input.
        ui.ctx().request_repaint();
        self.perform(ui.ctx(), action, None)
    }

    /// Performs the action of a pressed key, or in insert mode inserts typed
    /// text that a bound key did not type. Takes the text a bound key typed.
    fn handle(&mut self, ctx: &egui::Context, event: &egui::Event) -> Handled {
        if let Some(key) = pressed_key(event) {
            let keymap = match self.mode {
                Mode::Normal => &mut self.normal_keys,
                Mode::Insert => &mut self.insert_keys,
                Mode::Comment => &mut self.comment_keys,
            };
            let pressed = keymap.press(key);
            self.key_handled = pressed.is_some();
            if let Some((action, count)) = pressed {
                return Handled::Taken(self.perform(ctx, action, count));
            }
        } else if let egui::Event::Text(text) = event {
            if std::mem::take(&mut self.key_handled) {
                return Handled::Taken(None);
            }
            if self.mode == Mode::Insert {
                self.type_text(text);
                return Handled::Taken(None);
            }
        }
        Handled::Left
    }

    /// Performs `action`, with the count typed before its keys.
    fn perform(
        &mut self,
        ctx: &egui::Context,
        action: Action,
        count: Option<usize>,
    ) -> Option<ViewEvent> {
        // The index of the line a count numbers from 1.
        let counted_line = count.map(|line| line - 1);
        let count = count.unwrap_or(1).try_into().unwrap_or(isize::MAX);
        match action {
            Action::PrevChange => self.step_change(-1),
            Action::NextChange => self.step_change(1),
            Action::CursorBottom => {
                self.align_cursor(|row, rows| row.saturating_add(1).saturating_sub(rows))
            }
            Action::CursorTop => self.align_cursor(|row, _| row),
            Action::CursorCenter => {
                self.align_cursor(|row, rows| row.saturating_sub(rows.saturating_sub(1) / 2))
            }
            Action::CharLeft => self.move_cursor(|c, b| c.move_column(b, -count)),
            Action::CharRight => self.move_cursor(|c, b| c.move_column(b, count)),
            Action::LineDown => self.move_cursor(|c, b| c.move_line(b, count)),
            Action::LineUp => self.move_cursor(|c, b| c.move_line(b, -count)),
            Action::HalfPageDown => {
                self.move_page((self.rows_in_view / 2).max(1) as isize, count);
            }
            Action::HalfPageUp => {
                self.move_page(-((self.rows_in_view / 2).max(1) as isize), count);
            }
            Action::PageDown => {
                self.move_page(self.rows_in_view.max(1) as isize, count);
            }
            Action::PageUp => {
                self.move_page(-(self.rows_in_view.max(1) as isize), count);
            }
            Action::FirstLine => {
                self.move_cursor(|c, b| c.go_to_line(b, counted_line.unwrap_or(0)));
            }
            Action::LastLine => {
                self.move_cursor(|c, b| c.go_to_line(b, counted_line.unwrap_or(usize::MAX)));
            }
            Action::FirstColumn => self.move_cursor(|c, b| c.move_column(b, isize::MIN)),
            Action::LineStart => self.move_cursor(|c, b| c.go_to_line_start(b)),
            Action::LineEnd => self.move_cursor(|c, b| c.go_to_line_end(b)),
            Action::LeftSide => self.switch_side(0),
            Action::RightSide => self.switch_side(1),
            Action::Close => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Action::Save => return Some(ViewEvent::Save),
            Action::Reload => return Some(ViewEvent::Reload),
            Action::Yank => {
                let selection = self.selection.take()?;
                let (side, active) = self.cursor_position()?;
                if side != selection.side {
                    return None;
                }
                let buffer = &self.diff.buffers()[side];
                let text = selected_text(buffer, selection.kind, selection.range(active))?;
                return Some(ViewEvent::Yank(text));
            }
            Action::StartCharacterSelection => self.start_selection(SelectionKind::Character),
            Action::StartLineSelection => self.start_selection(SelectionKind::Line),
            Action::CancelSelection => self.selection = None,
            Action::Comment => self.start_comment(),
            Action::NextComment => self.step_comment(count),
            Action::PrevComment => self.step_comment(-count),
            Action::SaveComment => return self.save_comment(),
            Action::DiscardComment => {
                self.draft = None;
                self.editing = None;
                self.mode = Mode::Normal;
            }
            Action::Insert(at) => self.insert(at),
            Action::OpenLine(at) => self.open_line(at),
            Action::NormalMode => self.leave_insert(),
            Action::NewLine => self.type_text("\n"),
            Action::Tab => self.type_text("\t"),
            Action::Backspace => self.backspace(),
        }
        None
    }

    /// Edits the buffer of `side` with `edit` and starts highlighting the
    /// side again. Until the colors arrive, the side's colors move with the
    /// text the edit changed. Returns the change `edit` made.
    fn edit(&mut self, side: usize, edit: impl FnOnce(&mut Buffer) -> Edit) -> Edit {
        let edit = self.diff.edit(side, edit);
        self.colors[side].apply(&edit);
        for comment in &mut self.comments {
            comment.range.apply(side, &edit);
        }
        self.rehighlight(side);
        edit
    }

    /// The saved comments as the review file stores them, with the left
    /// and right side shown as `paths`.
    pub(crate) fn review_comments(&self, paths: [&Path; 2]) -> Vec<ReviewComment> {
        let comments = self.comments.iter();
        comments
            .map(|comment| self.review_comment(comment, paths))
            .collect()
    }

    /// `comment` as the review file stores it.
    fn review_comment(&self, comment: &Comment, paths: [&Path; 2]) -> ReviewComment {
        let CommentRange { side, kind, range } = comment.range;
        let quote = selected_text(&self.diff.buffers()[side], kind, range);
        ReviewComment::new(comment, paths[side], quote.unwrap_or_default())
    }

    pub(crate) fn save_target(&self) -> Option<SaveTarget> {
        let side = self.cursor_side?;
        Some(SaveTarget {
            side,
            text: self.diff.buffers()[side].text(),
        })
    }

    /// Inserts `text` at the cursor and moves the cursor past it.
    fn type_text(&mut self, text: &str) {
        let Some(side) = self.cursor_side else {
            return;
        };
        let at = self.cursor.position(&self.diff.buffers()[side]);
        let edit = self.edit(side, |b| b.insert(at, text));
        self.move_cursor(|c, b| c.set(b, b.position(edit.inserted_end)));
    }

    /// Deletes the character before the cursor, or at the start of a line
    /// joins it to the line above, and moves the cursor back.
    fn backspace(&mut self) {
        let Some(side) = self.cursor_side else {
            return;
        };
        let buffer = &self.diff.buffers()[side];
        let at = self.cursor.position(buffer);
        let start = match (at.line, at.column) {
            (0, 0) => return,
            (line, 0) => Position {
                line: line - 1,
                column: buffer.lines()[line - 1].chars().count(),
            },
            (line, column) => Position {
                line,
                column: column - 1,
            },
        };
        self.edit(side, |b| b.delete(start..at));
        self.move_cursor(|c, b| c.set(b, start));
    }

    /// Switches to insert mode with the cursor at `at`.
    fn insert(&mut self, at: InsertAt) {
        self.selection = None;
        self.mode = Mode::Insert;
        self.move_cursor(|c, b| match at {
            InsertAt::BeforeCursor => {
                c.set_between_characters(b, true);
            }
            InsertAt::AfterCursor => {
                c.set_between_characters(b, true);
                c.move_column(b, 1);
            }
            InsertAt::LineStart => {
                c.go_to_line_start(b);
                c.set_between_characters(b, true);
            }
            InsertAt::LineEnd => {
                c.set_between_characters(b, true);
                c.go_to_line_end(b);
            }
        });
    }

    fn open_line(&mut self, at: OpenLineAt) {
        let Some(side) = self.cursor_side else {
            return;
        };
        self.selection = None;
        let buffer = &self.diff.buffers()[side];
        let current = self.cursor.position(buffer);
        let (line, column) = match at {
            OpenLineAt::Above => (current.line, 0),
            OpenLineAt::Below => (current.line + 1, usize::MAX),
        };
        let insertion = Position {
            line: current.line,
            column,
        };
        self.edit(side, |b| b.insert(insertion, "\n"));
        self.mode = Mode::Insert;
        self.move_cursor(|c, b| {
            c.set(b, Position { line, column: 0 });
            c.set_between_characters(b, true);
        });
    }

    /// Switches to normal mode with the cursor on the character before it,
    /// as vim does.
    fn leave_insert(&mut self) {
        self.mode = Mode::Normal;
        self.move_cursor(|c, b| {
            c.move_column(b, -1);
            c.set_between_characters(b, false);
        });
    }

    /// The rows the view shows.
    fn view_rows(&self) -> ViewRows {
        ViewRows::new(self.diff.rows().len(), self.gaps.clone())
    }

    /// The row of the view that shows `line` of `side`.
    fn view_row_of(&self, side: usize, line: usize) -> usize {
        self.view_rows().view_row(self.diff.row_of(side, line))
    }

    /// The boxes of the saved comments and the draft, in the order of the
    /// rows they follow.
    fn comment_boxes(&self, ui: &egui::Ui, layout: &RowLayout) -> Vec<CommentBox> {
        let saved = self.comments.iter().enumerate();
        let saved = saved.map(|(index, comment)| (comment, Shows::Saved(index)));
        // Comments past the end of their side's text, such as after a reload
        // of shorter text, have no line to go under.
        let saved = saved.filter(|(comment, _)| self.has_line(comment.range));
        let saved = saved.filter(|(_, shows)| self.editing.map(Shows::Saved) != Some(*shows));
        let draft = self.draft.iter().map(|draft| (draft, Shows::Draft));
        let mut boxes: Vec<_> = saved
            .chain(draft)
            .map(|(comment, shows)| self.comment_box(ui, layout, comment, shows))
            .collect();
        boxes.sort_by_key(|comment_box| comment_box.gap.after);
        boxes
    }

    /// Whether `range`'s last line is in its side's text.
    fn has_line(&self, range: CommentRange) -> bool {
        range.range.end.line < self.diff.buffers()[range.side].lines().len()
    }

    /// The box of `comment`, under its last line.
    fn comment_box(
        &self,
        ui: &egui::Ui,
        layout: &RowLayout,
        comment: &Comment,
        shows: Shows,
    ) -> CommentBox {
        let CommentRange { side, range, .. } = comment.range;
        let button_rows = match shows {
            Shows::Draft => 1,
            Shows::Saved(_) => 0,
        };
        let gap = Gap {
            after: self.diff.row_of(side, range.end.line),
            rows: layout.comment_rows(ui, &comment.text) + button_rows,
        };
        CommentBox { gap, side, shows }
    }

    /// The side with the cursor, and where the cursor is in its buffer.
    fn cursor_position(&self) -> Option<(usize, Position)> {
        let side = self.cursor_side?;
        Some((side, self.cursor.position(&self.diff.buffers()[side])))
    }

    fn start_selection(&mut self, kind: SelectionKind) {
        if let Some((side, anchor)) = self.cursor_position() {
            self.selection = Some(Selection::new(kind, side, anchor));
        }
    }

    /// Opens a comment box on the selection, or on the cursor's line without
    /// one, and switches to comment mode. Without a selection, a comment
    /// already on the cursor's line opens to edit instead.
    fn start_comment(&mut self) {
        let Some((side, active)) = self.cursor_position() else {
            return;
        };
        self.mode = Mode::Comment;
        self.editing = match self.selection {
            Some(_) => None,
            None => self
                .comments
                .iter()
                .position(|comment: &Comment| comment.range.is_on(side, active.line)),
        };
        if let Some(index) = self.editing {
            self.draft = Some(self.comments[index].clone());
            return;
        }
        let selection = self.selection.take();
        let selection = selection.unwrap_or(Selection::new(SelectionKind::Line, side, active));
        let range = CommentRange {
            side,
            kind: selection.kind,
            range: selection.range(active),
        };
        self.draft = Some(Comment {
            range,
            text: String::new(),
        });
    }

    /// Moves the cursor to the first line of the `delta`th comment below its
    /// row, or above it when `delta` is negative. Stays put when there are
    /// not that many.
    fn step_comment(&mut self, delta: isize) {
        let Some((side, position)) = self.cursor_position() else {
            return;
        };
        let cursor_row = self.diff.row_of(side, position.line);
        let start_row = |range: &CommentRange| self.diff.row_of(range.side, range.range.start.line);
        let mut ranges: Vec<_> = self.comments.iter().map(|comment| comment.range).collect();
        ranges.retain(|range| self.has_line(*range));
        ranges.sort_by_key(start_row);
        let steps = delta.unsigned_abs() - 1;
        let target = match delta > 0 {
            true => ranges
                .iter()
                .filter(|r| start_row(r) > cursor_row)
                .nth(steps),
            false => ranges
                .iter()
                .rev()
                .filter(|r| start_row(r) < cursor_row)
                .nth(steps),
        };
        let Some(&CommentRange { side, range, .. }) = target else {
            return;
        };
        if self.cursor_side != Some(side) {
            self.selection = None;
        }
        self.cursor_side = Some(side);
        self.move_cursor(|c, b| c.go_to_line(b, range.start.line));
        let view_rows = self.view_rows();
        let start_row = view_rows.view_row(self.diff.row_of(side, range.start.line));
        let end_row = view_rows
            .view_row(self.diff.row_of(side, range.end.line) + 1)
            .saturating_sub(1);
        let rows_in_view = self.rows_in_view.max(1);
        self.scroll_to_top_row = Some(
            self.scroll_to_top_row
                .unwrap_or(self.top_row)
                .max(end_row.saturating_sub(rows_in_view.saturating_sub(1)))
                .min(start_row),
        );
    }

    /// Saves the draft and leaves comment mode, replacing the comment it
    /// edits. An empty draft deletes the comment it edits, and is not saved.
    fn save_comment(&mut self) -> Option<ViewEvent> {
        self.mode = Mode::Normal;
        let draft = self.draft.take()?;
        match (self.editing.take(), draft.text.is_empty()) {
            (Some(index), true) => _ = self.comments.remove(index),
            (Some(index), false) => self.comments[index] = draft,
            (None, false) => self.comments.push(draft),
            (None, true) => return None,
        }
        Some(ViewEvent::CommentsChanged)
    }

    /// Moves the cursor to the line of `side` across from it, keeping its
    /// column. Stays put when `side` has no lines.
    fn switch_side(&mut self, side: usize) {
        let Some((from, position)) = self.cursor_position() else {
            return;
        };
        let row = self.diff.row_of(from, position.line);
        let Some(line) = self.diff.line_at(side, row) else {
            return;
        };
        if from != side {
            self.selection = None;
        }
        self.cursor_side = Some(side);
        self.move_cursor(|c, b| c.go_to_line(b, line));
    }

    /// Moves the cursor in its buffer with `move_it`, then scrolls the least
    /// that brings its row into view.
    fn move_cursor(&mut self, move_it: impl FnOnce(&mut Cursor, &Buffer)) {
        let Some(side) = self.cursor_side else {
            return;
        };
        move_it(&mut self.cursor, &self.diff.buffers()[side]);
        let line = self.cursor.position(&self.diff.buffers()[side]).line;
        self.scroll_to_show(self.view_row_of(side, line));
    }

    /// Scrolls the least that brings row `row` of the view into view.
    fn scroll_to_show(&mut self, row: usize) {
        let rows_in_view = self.rows_in_view.max(1);
        let top_row = self.scroll_to_top_row.unwrap_or(self.top_row);
        if row < top_row {
            self.scroll_to_top_row = Some(row);
        } else if row >= top_row + rows_in_view {
            self.scroll_to_top_row = Some(row + 1 - rows_in_view);
        }
    }

    /// Moves the view by `rows` multiplied by `count`, placing the cursor at
    /// the new top or bottom row.
    fn move_page(&mut self, rows: isize, count: isize) {
        let Some(side) = self.cursor_side else {
            return;
        };
        let Some(position) = self.cursor_position().map(|(_, position)| position) else {
            return;
        };
        let rows_in_view = self.rows_in_view.max(1);
        let view_rows = self.view_rows();
        let max_top = view_rows.len().saturating_sub(rows_in_view);
        let old_top = self.scroll_to_top_row.unwrap_or(self.top_row);
        let top = self
            .scroll_to_top_row
            .unwrap_or(self.top_row)
            .saturating_add_signed(rows.saturating_mul(count))
            .min(max_top);
        self.scroll_to_top_row = Some(top);
        let cursor_row = self.view_row_of(side, position.line);
        let row = match top == old_top {
            true => cursor_row
                .saturating_add_signed(rows.saturating_mul(count))
                .min(view_rows.len().saturating_sub(1)),
            false => cursor_row.clamp(top, top + rows_in_view.saturating_sub(1)),
        };
        let Some(line) = self.diff.line_at(side, view_rows.diff_row(row)) else {
            return;
        };
        self.move_cursor(|c, b| c.go_to_line(b, line));
    }

    /// Scrolls the view so the cursor row is at the position returned by
    /// `top_row`.
    fn align_cursor(&mut self, top_row: impl FnOnce(usize, usize) -> usize) {
        let Some((side, position)) = self.cursor_position() else {
            return;
        };
        let rows_in_view = self.rows_in_view.max(1);
        let row = self.view_row_of(side, position.line);
        self.scroll_to_top_row = Some(top_row(row, rows_in_view));
    }

    /// Moves to the change before or after the cursor, if there is one.
    fn step_change(&mut self, delta: isize) {
        let Some((side, position)) = self.cursor_position() else {
            return;
        };
        let cursor_row = self.diff.row_of(side, position.line);
        let changes = self.diff.changes();
        let next = match delta > 0 {
            true => changes.iter().position(|&row| row > cursor_row),
            false => match self.diff.rows()[cursor_row].kind {
                RowKind::Equal => changes
                    .partition_point(|&row| row < cursor_row)
                    .checked_sub(1),
                _ => changes
                    .partition_point(|&row| row <= cursor_row)
                    .checked_sub(2),
            },
        };
        let Some(next) = next else {
            return;
        };
        self.jump_to_change(next);
    }

    /// Moves the cursor to `index` of the changes and scrolls it into view.
    fn jump_to_change(&mut self, index: usize) {
        self.current_change = Some(index);
        let row = self.diff.changes()[index];
        let view_row = self.view_rows().view_row(row);
        self.scroll_to_top_row = Some(view_row.saturating_sub(CONTEXT_ROWS));
        if let Some(side) = self.cursor_side
            && let Some(line) = self.diff.line_at(side, row)
        {
            self.cursor.go_to_line(&self.diff.buffers()[side], line);
        }
    }

    /// Both halves live in one scroll area so they always scroll together.
    /// Returns the rows in view, in fractions of a row, and the action they
    /// ask for, if any.
    fn rows_ui(&mut self, ui: &mut egui::Ui) -> (Range<f32>, Option<Action>) {
        let font = TextStyle::Monospace.resolve(ui.style());
        let char_width = ui.fonts_mut(|f| f.glyph_width(&font, '0'));
        let line_count = self
            .diff
            .buffers()
            .iter()
            .map(|b| b.lines().len())
            .max()
            .unwrap_or(0);
        let digits = line_count.max(1).ilog10() as f32 + 1.0;
        let layout = RowLayout {
            row_height: ui.fonts_mut(|f| f.row_height(&font)).ceil(),
            half_width: ui.available_width() / 2.0,
            // A space on each side of the line number.
            gutter_width: (digits + 2.0) * char_width,
            char_width,
            font,
            bar_cursor: self.mode == Mode::Insert,
            palette: self.palette,
            scroll_x: 0.0,
        };
        let layout = RowLayout {
            scroll_x: self.scroll_x(ui, &layout),
            ..layout
        };

        ui.spacing_mut().item_spacing.y = 0.0;
        let boxes = self.comment_boxes(ui, &layout);
        let gaps: Vec<_> = boxes.iter().map(|comment_box| comment_box.gap).collect();
        if gaps != self.gaps {
            self.gaps = gaps;
            // Keeps the comment box in view as it opens and grows.
            let draft_box = boxes.iter().find(|b| b.shows == Shows::Draft);
            if let Some(gap) = draft_box.map(|comment_box| comment_box.gap) {
                self.scroll_to_show(self.view_rows().view_row(gap.after) + gap.rows);
            }
        }
        let view_rows = self.view_rows();
        let mut area = ScrollArea::vertical().auto_shrink(false);
        if let Some(row) = self.scroll_to_top_row.take() {
            // Clamped so rows near the end don't overscroll and bounce back.
            let row_height = layout.row_height;
            let max_offset = view_rows.len() as f32 * row_height - ui.available_height();
            let offset = row as f32 * row_height;
            area = area.vertical_scroll_offset(offset.min(max_offset).max(0.0));
        }
        let rows = self.diff.rows();
        let comments = &self.comments;
        let marks = Marks {
            cursor: self.cursor_position(),
            selection: self.selection,
            comments,
        };
        let draft = &mut self.draft;
        let mut action = None;
        let output = area.show_rows(ui, layout.row_height, view_rows.len(), |ui, range| {
            let first_in_view = range.start;
            // `last` paints every row, keeping the click if one was clicked.
            range
                .filter_map(|view_row| match view_rows.at(view_row) {
                    ViewRow::Diff(row) => {
                        layout.paint_row(ui, self.diff.buffers(), &self.colors, &rows[row], marks)
                    }
                    ViewRow::Gap { index, row } => {
                        let paint_box = row == 0 || view_row == first_in_view;
                        let text = match boxes[index].shows {
                            Shows::Saved(index) => BoxText::Saved(&comments[index].text),
                            Shows::Draft => BoxText::Draft(&mut draft.as_mut()?.text),
                        };
                        action =
                            action.or(layout.paint_gap(ui, &boxes[index], row, text, paint_box));
                        None
                    }
                })
                .last()
        });
        if let Some((side, clicked)) = output.inner {
            if self
                .selection
                .is_some_and(|selection| selection.side != side)
            {
                self.selection = None;
            }
            self.cursor_side = Some(side);
            self.cursor.set(&self.diff.buffers()[side], clicked);
        }
        let top = output.state.clamped_offset().y / layout.row_height;
        let bottom = top + output.inner_rect.height() / layout.row_height;
        self.top_row = top.ceil() as usize;
        self.rows_in_view = (bottom.floor() as usize).saturating_sub(self.top_row);
        (top..bottom, action)
    }

    /// Updates and returns how far the text is scrolled left: by the
    /// horizontal mouse wheel over the rows, up to past the end of the
    /// widest line in view, and the least that shows the cursor once it
    /// has moved.
    fn scroll_x(&mut self, ui: &egui::Ui, layout: &RowLayout) -> f32 {
        let text_width = layout.half_width - layout.gutter_width;
        let hovered = ui.rect_contains_pointer(ui.available_rect_before_wrap());
        let wheel = ui.input(|i| i.smooth_scroll_delta.x);
        if hovered && wheel != 0.0 {
            let widest = self.widest_line_in_view(ui, layout);
            let max_scroll = (widest + layout.char_width - text_width).max(self.scroll_x);
            self.scroll_x = (self.scroll_x - wheel).min(max_scroll).max(0.0);
        }
        let cursor = self.cursor_position();
        if cursor != self.scrolled_to_cursor {
            self.scrolled_to_cursor = cursor;
            if let Some((side, position)) = cursor {
                let text = &self.diff.buffers()[side].lines()[position.line];
                let galley = layout.layout_line(ui, text);
                let glyphs = galley.rows.first().map_or(&[][..], |r| &r.glyphs);
                let (x, width) = layout.cursor_span(glyphs, position.column);
                self.scroll_x = self.scroll_x.max(x + width - text_width).min(x);
            }
        }
        self.scroll_x
    }

    /// Width of the widest line on the rows in view in the last frame.
    fn widest_line_in_view(&self, ui: &egui::Ui, layout: &RowLayout) -> f32 {
        let view_rows = self.view_rows();
        let rows = self.diff.rows();
        let buffers = self.diff.buffers();
        // Includes the rows partly in view above and below.
        let end = (self.top_row + self.rows_in_view + 1).min(view_rows.len());
        (self.top_row.saturating_sub(1)..end)
            .filter_map(|view_row| match view_rows.at(view_row) {
                ViewRow::Diff(row) => Some(&rows[row]),
                ViewRow::Gap { .. } => None,
            })
            .flat_map(|row| [&row.left, &row.right].into_iter().enumerate())
            .filter_map(|(side, line)| Some(&buffers[side].lines()[line.as_ref()?.index]))
            .map(|text| layout.layout_line(ui, text).size().x)
            .fold(0.0, f32::max)
    }
}

/// Lays out the Cancel and Comment buttons at the right of `rect`, returning
/// the action of the one clicked.
fn comment_buttons(ui: &mut egui::Ui, rect: Rect) -> Option<Action> {
    let builder = UiBuilder::new()
        .max_rect(rect)
        .layout(Layout::right_to_left(Align::Center));
    let mut ui = ui.new_child(builder);
    // Without keyboard focus, which would take it from the comment's text.
    let button = |text| egui::Button::new(text).sense(Sense::CLICK);
    let comment = ui.add(button("Comment")).on_hover_text("Enter");
    let cancel = ui.add(button("Cancel")).on_hover_text("Escape");
    let comment = comment.clicked().then_some(Action::SaveComment);
    comment.or(cancel.clicked().then_some(Action::DiscardComment))
}

/// Shortens `label` to fit `width`, keeping its filename when possible.
fn truncate_label(path: &Path, width: f32, char_width: f32) -> String {
    let label = path.to_string_lossy();
    let max_chars = (width / char_width).floor() as usize;
    if label.chars().count() <= max_chars {
        return label.into_owned();
    }

    let filename = path
        .file_name()
        .and_then(|filename| filename.to_str())
        .unwrap_or(&label);
    let filename_label = format!(".../{filename}");
    if filename_label.chars().count() <= max_chars {
        return filename_label;
    }

    let tail = label.chars().rev().take(max_chars.saturating_sub(3));
    format!(
        "...{}",
        tail.collect::<String>().chars().rev().collect::<String>()
    )
}

/// What highlighting a side on a background thread needs.
#[derive(Clone)]
struct Highlighting {
    /// The paths whose names pick the left and right side's language.
    paths: [PathBuf; 2],
    /// Loaded by the first highlighting to need it.
    highlighter: Arc<OnceLock<Highlighter>>,
    /// The current highlighting of each side.
    runs: Arc<[AtomicUsize; 2]>,
    tx: Sender<ColorsChunk>,
    ctx: egui::Context,
}

impl Highlighting {
    /// Highlights `side`'s `lines` in `theme` as its highlighting `run`,
    /// sending the colors in chunks and requesting a repaint after each.
    /// Stops once the side has a later highlighting or the window has
    /// closed.
    fn highlight_side(&self, side: usize, run: usize, theme: &Theme, lines: &[String]) {
        let highlighter = self.highlighter.get_or_init(Highlighter::default);
        let current = || self.runs[side].load(Ordering::Relaxed) == run;
        let path = &self.paths[side];
        let mut spans = highlighter.highlight(theme, path, lines).peekable();
        while spans.peek().is_some() && current() {
            // Stops partway through a chunk too, once it is no longer current.
            let chunk = spans.by_ref().take(HIGHLIGHT_CHUNK);
            let lines = chunk.take_while(|_| current()).collect();
            // Fails only once the window has closed.
            if self.tx.send(ColorsChunk { side, run, lines }).is_err() {
                return;
            }
            self.ctx.request_repaint();
        }
    }
}

/// Background in `palette` of the sides of `row` that have a line, if the
/// row differs.
fn background(row: &Row, palette: &Palette) -> Option<Color32> {
    match row.kind {
        RowKind::Equal => None,
        RowKind::LeftOnly | RowKind::RightOnly => Some(palette.one_sided),
        RowKind::Changed => Some(palette.changed),
    }
}

/// The y on `map` of the top of a row of a diff of `rows` rows, with
/// `rows_in_view` rows fitting in the view. A diff shorter than the view
/// keeps the view's spacing.
fn map_y(map: Rect, rows: usize, rows_in_view: f32) -> impl Fn(f32) -> f32 {
    let row_height = map.height() / (rows as f32).max(rows_in_view);
    move |row| map.top() + row * row_height
}

/// The marks of each side's differing lines, in the left and right column
/// of `map`.
fn change_marks(rows: &[Row], map: Rect, y: impl Fn(f32) -> f32) -> Vec<(Rect, Color32)> {
    let center = map.center().x;
    let left = map.with_max_x(center);
    let right = map.with_min_x(center);
    let mut marks = column_marks(rows, left, &y, RowKind::LeftOnly);
    marks.extend(column_marks(rows, right, &y, RowKind::RightOnly));
    marks
}

/// A mark across `column` for each run of rows of one kind that are
/// changed or `one_sided`, from the `y` of its first row to the `y` past
/// its last, and at least [`MIN_MARK_HEIGHT`] tall.
fn column_marks(
    rows: &[Row],
    column: Rect,
    y: impl Fn(f32) -> f32,
    one_sided: RowKind,
) -> Vec<(Rect, Color32)> {
    let mut marks = Vec::new();
    let mut start = 0;
    for (count, color) in rows
        .iter()
        .map(|r| mark_color(r.kind, one_sided))
        .dedup_with_count()
    {
        if let Some(color) = color {
            let top = y(start as f32);
            let bottom = y((start + count) as f32).max(top + MIN_MARK_HEIGHT);
            marks.push((Rect::from_x_y_ranges(column.x_range(), top..=bottom), color));
        }
        start += count;
    }
    marks
}

/// A mark across `map` from the `y` of each of `rows`, the rows of the
/// diff comments start on, [`MIN_MARK_HEIGHT`] tall.
fn comment_marks(
    rows: impl Iterator<Item = usize>,
    map: Rect,
    y: impl Fn(f32) -> f32,
) -> Vec<Rect> {
    let mark = |row: usize| {
        let top = y(row as f32);
        Rect::from_x_y_ranges(map.x_range(), top..=top + MIN_MARK_HEIGHT)
    };
    rows.map(mark).collect()
}

/// The change map color of a row of `kind` in the column whose side's
/// one-sided rows are `one_sided`, if the column marks it.
fn mark_color(kind: RowKind, one_sided: RowKind) -> Option<Color32> {
    match kind {
        RowKind::Changed => Some(CHANGED_MARK),
        _ if kind == one_sided => Some(ONE_SIDED_MARK),
        _ => None,
    }
}

/// Where the parts of a row go.
struct RowLayout {
    font: FontId,
    char_width: f32,
    gutter_width: f32,
    row_height: f32,
    half_width: f32,
    /// Whether the cursor is a bar before its column, as in insert mode.
    bar_cursor: bool,
    palette: Palette,
    /// How far the text is scrolled left, past the line numbers.
    scroll_x: f32,
}

struct HalfLine<'a> {
    rect: Rect,
    line: &'a Line,
    text: &'a str,
    spans: &'a [Span],
    background: Option<Color32>,
    selection: Option<Selection>,
    active: Option<Position>,
    /// Whether a comment is on the line.
    commented: bool,
}

/// What a row shows on its lines besides their text.
#[derive(Clone, Copy)]
struct Marks<'a> {
    /// The side and position of the cursor.
    cursor: Option<(usize, Position)>,
    selection: Option<Selection>,
    comments: &'a [Comment],
}

impl RowLayout {
    /// Paints `row` across both halves, with each side's text from its
    /// buffer, a divider between them, and the `marks` on the row's lines.
    /// Returns the side and position of the character a click on a line's
    /// half was on.
    fn paint_row(
        &self,
        ui: &mut egui::Ui,
        buffers: &[Buffer; 2],
        colors: &[SideColors; 2],
        row: &Row,
        marks: Marks<'_>,
    ) -> Option<(usize, Position)> {
        let Marks {
            cursor,
            selection,
            comments,
        } = marks;
        let size = vec2(ui.available_width(), self.row_height);
        let (rect, response) = ui.allocate_exact_size(size, Sense::CLICK);
        let response = response.on_hover_cursor(CursorIcon::Text);
        let click = response
            .interact_pointer_pos()
            .filter(|_| response.clicked());
        let painter = ui.painter();
        let (left, right) = rect.split_left_right_at_x(rect.left() + self.half_width);
        let background = background(row, &self.palette);
        let mut clicked = None;
        for (side, (half, line)) in [(left, &row.left), (right, &row.right)]
            .into_iter()
            .enumerate()
        {
            let Some(line) = line else {
                painter.rect_filled(half, 0.0, self.palette.filler);
                continue;
            };
            let text = &buffers[side].lines()[line.index];
            let spans = colors[side].line(line);
            let selection = selection.filter(|selection| selection.side == side);
            let active = cursor
                .filter(|(cursor_side, _)| *cursor_side == side)
                .map(|(_, p)| p);
            let (pos, galley) = self.paint_half(
                ui,
                HalfLine {
                    rect: half,
                    line,
                    text,
                    spans,
                    background,
                    selection,
                    active,
                    commented: comments.iter().any(|c| c.range.is_on(side, line.index)),
                },
            );
            let glyphs = galley.rows.first().map_or(&[][..], |r| &r.glyphs);
            if let Some((_, c)) = cursor.filter(|(s, c)| *s == side && c.line == line.index) {
                self.paint_cursor(painter, half, pos, glyphs, c.column);
            }
            if let Some(click) = click.filter(|p| half.contains(*p)) {
                // A click on the line number is on the first column in view.
                let text_left = half.left() + self.gutter_width;
                let column = column_at(glyphs, click.x.max(text_left) - pos.x);
                let line = line.index;
                clicked = Some((side, Position { line, column }));
            }
        }
        let divider = Stroke::new(1.0, self.palette.divider);
        painter.vline(left.right(), rect.y_range(), divider);
        clicked
    }

    /// Rows of a comment box holding `text`: its lines, wrapped to fit the
    /// box, and half a row above and below them.
    fn comment_rows(&self, ui: &egui::Ui, text: &str) -> usize {
        let width = self.half_width - self.gutter_width - 3.0 * self.char_width;
        let font = self.font.clone();
        let galley = ui.fonts_mut(|f| f.layout(text.to_owned(), font, Color32::PLACEHOLDER, width));
        galley.rows.len() + 1
    }

    /// Paints row `row` of the gap `comment_box` takes, with filler on the
    /// other side. When `paint_box`, also paints the box over the whole gap,
    /// holding `text`, with Cancel and Comment buttons under a draft's text.
    /// Returns the action the box asks for, if any.
    fn paint_gap(
        &self,
        ui: &mut egui::Ui,
        comment_box: &CommentBox,
        row: usize,
        text: BoxText<'_>,
        paint_box: bool,
    ) -> Option<Action> {
        let size = vec2(ui.available_width(), self.row_height);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let (left, right) = rect.split_left_right_at_x(rect.left() + self.half_width);
        let (commented, other) = match comment_box.side {
            0 => (left, right),
            _ => (right, left),
        };
        let divider = Stroke::new(1.0, self.palette.divider);
        let painter = ui.painter();
        painter.rect_filled(other, 0.0, self.palette.filler);
        painter.vline(left.right(), rect.y_range(), divider);
        if !paint_box {
            return None;
        }
        let top = rect.top() - row as f32 * self.row_height;
        let bottom = top + comment_box.gap.rows as f32 * self.row_height;
        let x = commented.left() + self.gutter_width..=commented.right() - self.char_width;
        let box_rect = Rect::from_x_y_ranges(x, top..=bottom);
        let (fill, stroke) = (self.palette.filler, StrokeKind::Inside);
        painter.rect(box_rect, 2.0, fill, divider, stroke);
        let text_rect = box_rect.shrink2(vec2(self.char_width, self.row_height / 2.0));
        match text {
            BoxText::Saved(text) => {
                let (font, color) = (self.font.clone(), self.palette.text);
                let galley = painter.layout(text.to_owned(), font, color, text_rect.width());
                painter.galley(text_rect.min, galley, color);
                None
            }
            BoxText::Draft(text) => {
                let buttons_top = text_rect.bottom() - self.row_height;
                self.edit_comment(ui, text_rect.with_max_y(buttons_top), text);
                let buttons_rect = text_rect.with_min_y(buttons_top);
                comment_buttons(ui, buttons_rect.with_max_y(box_rect.bottom()))
            }
        }
    }

    /// Lays out a text box over `rect` that edits `text` and keeps the
    /// keyboard focus.
    fn edit_comment(&self, ui: &mut egui::Ui, rect: Rect, text: &mut String) {
        let edit = egui::TextEdit::multiline(text)
            .id_salt("comment")
            .hint_text("Comment")
            .font(self.font.clone())
            .frame(egui::Frame::NONE)
            .margin(0.0)
            .desired_width(rect.width())
            .desired_rows(1);
        let response = ui.new_child(UiBuilder::new().max_rect(rect)).add(edit);
        if !response.has_focus() {
            response.request_focus();
        }
    }

    /// Paints the cursor at `column` of a line in `half` whose `glyphs` are
    /// drawn from `pos`: a bar before the column in insert mode, else a
    /// block over its character, or over the line's start when it has none.
    fn paint_cursor(
        &self,
        painter: &Painter,
        half: Rect,
        pos: Pos2,
        glyphs: &[Glyph],
        column: usize,
    ) {
        let glyph = glyphs.get(column);
        let (x, width) = self.cursor_span(glyphs, column);
        let x = pos.x + x;
        let block = Rect::from_x_y_ranges(x..=x + width, half.y_range());
        let text_left = half.left() + self.gutter_width;
        let text_rect = Rect::from_min_max(pos2(text_left, half.top()), half.max);
        let painter = painter.with_clip_rect(text_rect);
        painter.rect_filled(block, 0.0, self.palette.cursor);
        if let Some(glyph) = glyph.filter(|_| !self.bar_cursor) {
            let (align, font) = (Align2::LEFT_CENTER, self.font.clone());
            let color = self.palette.cursor_text;
            painter.text(block.left_center(), align, glyph.chr, font, color);
        }
    }

    /// The x from the line's start and the width of the cursor at `column`
    /// of a line whose layout has `glyphs`.
    fn cursor_span(&self, glyphs: &[Glyph], column: usize) -> (f32, f32) {
        let glyph = glyphs.get(column);
        let end_x = glyphs.last().map_or(0.0, Glyph::max_x);
        let x = glyph.map_or(end_x, |g| g.pos.x);
        let width = match (self.bar_cursor, glyph) {
            (true, _) => CURSOR_BAR_WIDTH,
            (false, Some(g)) => g.advance_width,
            (false, None) => self.char_width,
        };
        (x, width)
    }

    /// Lays out `text` in the font of lines, without their formatting.
    fn layout_line(&self, ui: &egui::Ui, text: &str) -> Arc<Galley> {
        let font = self.font.clone();
        ui.fonts_mut(|f| f.layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER))
    }

    /// Paints one half of a row that has a line: the background, the line
    /// number, and the line's `text` in the colors of its spans, clipped to
    /// the half, with its changed words and selection highlighted. Returns
    /// where the text starts, and its layout.
    fn paint_half(&self, ui: &egui::Ui, half: HalfLine<'_>) -> (Pos2, Arc<Galley>) {
        let HalfLine {
            rect: half,
            line,
            text,
            spans,
            background,
            selection,
            active,
            commented,
        } = half;
        let painter = ui.painter();
        let mid_y = half.center().y;
        let gutter_right = half.left() + self.gutter_width;
        let selected_line = selection.zip(active).is_some_and(|(selection, active)| {
            selection.kind == SelectionKind::Line
                && (selection.range(active).start.line..=selection.range(active).end.line)
                    .contains(&line.index)
        });
        if let Some(background) = background {
            painter.rect_filled(half, 0.0, background);
        }
        if selected_line {
            painter.rect_filled(
                half,
                0.0,
                self.selection_background(background.unwrap_or(self.palette.background)),
            );
        }
        if commented {
            let x = half.left()..=half.left() + COMMENT_MARK_WIDTH;
            let mark = Rect::from_x_y_ranges(x, half.y_range()).shrink2(vec2(0.0, 2.0));
            painter.rect_filled(mark, 1.0, COMMENT_MARK);
        }
        let number = (line.index + 1).to_string();
        let pos = pos2(gutter_right - self.char_width, mid_y);
        painter.text(
            pos,
            Align2::RIGHT_CENTER,
            number,
            self.font.clone(),
            self.palette.line_number,
        );

        let text_rect = Rect::from_min_max(pos2(gutter_right, half.top()), half.max);
        let galley = ui.fonts_mut(|f| {
            f.layout_job(self.line_job(
                line,
                text,
                spans,
                background.unwrap_or(self.palette.background),
                selection,
                active,
            ))
        });
        let pos = pos2(gutter_right - self.scroll_x, mid_y - galley.size().y / 2.0);
        painter
            .with_clip_rect(text_rect)
            .galley(pos, galley.clone(), self.palette.text);
        (pos, galley)
    }

    /// Lays out `line`'s `text` with syntax, diff, and selection formatting.
    fn line_job(
        &self,
        line: &Line,
        text: &str,
        spans: &[Span],
        row_background: Color32,
        selection: Option<Selection>,
        active: Option<Position>,
    ) -> LayoutJob {
        let selected = selection
            .zip(active)
            .and_then(|(selection, active)| selected_bytes(selection, active, line, text));
        let color_starts = spans.iter().map(|span| {
            let [r, g, b] = span.rgb;
            (span.range.start, Color32::from_rgb(r, g, b))
        });
        let background_starts = background_starts(
            &line.changed_words,
            selected,
            self.palette.changed_word,
            self.selection_background(row_background),
            self.selection_background(self.palette.changed_word),
        );
        let mut job = LayoutJob::default();
        let (mut start, mut color, mut background) = (0, self.palette.text, Color32::TRANSPARENT);
        for event in color_starts.merge_join_by(background_starts, |a, b| a.0.cmp(&b.0)) {
            let end = event.as_ref().reduce(|a, _| a).0;
            self.append_piece(&mut job, &text[start..end], color, background);
            start = end;
            match event {
                Left((_, new_color)) => color = new_color,
                Right((_, new_background)) => background = new_background,
                Both((_, new_color), (_, new_background)) => {
                    (color, background) = (new_color, new_background)
                }
            }
        }
        self.append_piece(&mut job, &text[start..], color, background);
        job
    }

    fn selection_background(&self, background: Color32) -> Color32 {
        background.lerp_to_gamma(self.palette.selection, 0.5)
    }

    /// Appends `text` to `job` in `color` on `background`, unless it is empty.
    fn append_piece(&self, job: &mut LayoutJob, text: &str, color: Color32, background: Color32) {
        if text.is_empty() {
            return;
        }
        let format = TextFormat {
            font_id: self.font.clone(),
            color,
            background,
            ..Default::default()
        };
        job.append(text, 0.0, format);
    }
}

fn background_starts<'a>(
    changed_words: &'a [Range<usize>],
    selection: Option<Range<usize>>,
    changed_word_background: Color32,
    selected_background: Color32,
    selected_changed_word_background: Color32,
) -> impl Iterator<Item = (usize, Color32)> + 'a {
    let changed_word_starts = changed_words
        .iter()
        .flat_map(|range| [(range.start, true), (range.end, false)]);
    let selection_starts = selection
        .into_iter()
        .flat_map(|range| [(range.start, true), (range.end, false)]);
    changed_word_starts
        .merge_join_by(selection_starts, |a, b| a.0.cmp(&b.0))
        .scan((false, false), move |(changed_word, selected), event| {
            let position = match event {
                Left((position, active)) => {
                    *changed_word = active;
                    position
                }
                Right((position, active)) => {
                    *selected = active;
                    position
                }
                Both((position, changed_word_active), (_, selected_active)) => {
                    *changed_word = changed_word_active;
                    *selected = selected_active;
                    position
                }
            };
            let background = match (*changed_word, *selected) {
                (true, true) => selected_changed_word_background,
                (true, false) => changed_word_background,
                (false, true) => selected_background,
                (false, false) => Color32::TRANSPARENT,
            };
            Some((position, background))
        })
}

fn selected_bytes(
    selection: Selection,
    active: Position,
    line: &Line,
    text: &str,
) -> Option<Range<usize>> {
    let range = selection.range(active);
    if !(range.start.line..=range.end.line).contains(&line.index) {
        return None;
    }
    if selection.kind == SelectionKind::Line {
        return Some(0..text.len());
    }
    let start_column = if line.index == range.start.line {
        range.start.column
    } else {
        0
    };
    let end_column = if line.index == range.end.line {
        range.end.column.saturating_add(1)
    } else {
        text.chars().count()
    };
    let start = text.char_indices().nth(start_column)?.0;
    let end = text
        .char_indices()
        .nth(end_column)
        .map_or(text.len(), |(byte, _)| byte);
    Some(start..end)
}

fn selected_text(buffer: &Buffer, kind: SelectionKind, range: SelectionRange) -> Option<String> {
    let lines = buffer.lines();
    let start_line = lines.get(range.start.line)?;
    let end_line = lines.get(range.end.line)?;
    let text = buffer.text();
    let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let line_start = |index: usize| {
        lines[..index]
            .iter()
            .map(|line| line.len() + line_ending.len())
            .sum::<usize>()
    };
    let start_line_start = line_start(range.start.line);
    let end_line_start = line_start(range.end.line);
    let (start, end) = match kind {
        SelectionKind::Line => {
            let end = end_line_start + end_line.len();
            let end = end + usize::from(text[end..].starts_with(line_ending)) * line_ending.len();
            (start_line_start, end)
        }
        SelectionKind::Character => {
            let start_column = start_line.char_indices().nth(range.start.column);
            let start = start_line_start + start_column.map_or(start_line.len(), |(byte, _)| byte);
            let end_column = end_line.char_indices().nth(range.end.column);
            let end = end_line_start + end_column.map_or(end_line.len(), |(byte, _)| byte);
            let end = match end_line.get(end - end_line_start..)?.chars().next() {
                Some(character) => end + character.len_utf8(),
                None => end + usize::from(text[end..].starts_with(line_ending)) * line_ending.len(),
            };
            (start, end)
        }
    };
    Some(text.get(start..end)?.to_owned())
}

/// The index of the glyph of `glyphs`, laid out from 0, that `x` is on, or
/// of the last glyph when `x` is past it.
fn column_at(glyphs: &[Glyph], x: f32) -> usize {
    let column = glyphs.iter().take_while(|g| g.max_x() <= x).count();
    column.min(glyphs.len().saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Key, Modifiers, accesskit::Role, text::ByteIndex};
    use egui_kittest::{Harness, kittest::Queryable as _};

    fn show_view(ui: &mut egui::Ui, view: &mut TextView) {
        view.process_input(ui);
        view.show_header(ui, [Path::new("left"), Path::new("right")]);
        view.show_diff(ui);
    }

    #[yare::parameterized(
        full_label_fits_at_its_boundary = { "src/main.rs", 11, "src/main.rs" },
        full_label_falls_back_to_tail_at_its_boundary = { "src/main.rs", 10, "...main.rs" },
        filename_form_fits_at_its_boundary = { "src/very/long/main.rs", 11, ".../main.rs" },
        filename_form_falls_back_to_tail_at_its_boundary = {
            "src/very/long/main.rs", 10, "...main.rs"
        },
        tail_form_has_one_character_before_ellipsis = { "src/main.rs", 4, "...s" },
        tail_form_becomes_ellipsis_at_its_boundary = { "src/main.rs", 3, "..." },
        ellipsis_remains_at_narrower_width = { "src/main.rs", 2, "..." },
    )]
    fn truncate_label(label: &str, width: usize, expected: &str) {
        assert_eq!(
            super::truncate_label(Path::new(label), width as f32, 1.0),
            expected
        );
    }

    /// Three separate one-line changes with unchanged lines between them.
    fn three_changes() -> TextView {
        let left: String = (0..30).map(|i| format!("line {i}\n")).collect();
        let right = left
            .replace("line 5\n", "line five\n")
            .replace("line 15\n", "line fifteen\n")
            .replace("line 25\n", "line twenty-five\n");
        TextView::new(
            SideBySide::new([Buffer::new(&left), Buffer::new(&right)]),
            Vec::new(),
        )
    }

    #[test]
    fn keys_step_through_changes_and_stop_at_the_ends() {
        let mut harness = Harness::new_ui_state(show_view, three_changes());
        harness.run();
        harness.get_by_label("change 1 of 3");

        harness.key_press(Key::P);
        harness.run();
        harness.get_by_label("change 1 of 3");

        for _ in 0..3 {
            harness.key_press(Key::N);
        }
        harness.run();
        harness.get_by_label("change 3 of 3");

        harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::F7);
        harness.run();
        harness.get_by_label("change 2 of 3");
    }

    #[test]
    fn next_change_uses_the_cursor_position() {
        let mut view = three_changes();
        view.cursor.set(
            &view.diff.buffers()[1],
            Position {
                line: 20,
                column: 0,
            },
        );

        view.step_change(1);

        assert_eq!(view.current_change, Some(2));
    }

    #[test]
    fn previous_change_uses_the_cursor_position() {
        let mut view = three_changes();
        view.step_change(1);
        view.step_change(1);
        view.cursor.set(
            &view.diff.buffers()[1],
            Position {
                line: 10,
                column: 0,
            },
        );

        view.step_change(-1);

        assert_eq!(view.current_change, Some(0));
    }

    #[test]
    fn previous_change_skips_the_change_under_the_cursor() {
        let left: String = (0..30).map(|i| format!("line {i}\n")).collect();
        let right = left
            .replace("line 5\n", "line five\n")
            .replace("line 15\nline 16\n", "line fifteen\nline sixteen\n")
            .replace("line 25\n", "line twenty-five\n");
        let mut view = TextView::new(
            SideBySide::new([Buffer::new(&left), Buffer::new(&right)]),
            Vec::new(),
        );
        view.step_change(1);
        view.step_change(1);
        view.cursor.set(
            &view.diff.buffers()[1],
            Position {
                line: 16,
                column: 0,
            },
        );

        view.step_change(-1);

        assert_eq!(view.current_change, Some(0));
    }

    /// The view of 200 equal lines, after pressing `keys` one per frame.
    /// `keys` are separated by spaces, each a character, uppercase with
    /// Shift, a key name such as `Down`, or either with `C-` for Ctrl or
    /// `S-` for Shift.
    fn typed_in_unchanged_file(keys: &str) -> Harness<'static, TextView> {
        let mut harness = Harness::new_ui_state(show_view, unchanged_file());
        harness.run();
        press(&mut harness, keys);
        harness
    }

    /// A view of 200 equal lines, "line 0" to "line 199".
    fn unchanged_file() -> TextView {
        let text: String = (0..200).map(|i| format!("line {i}\n")).collect();
        let diff = SideBySide::new([Buffer::new(&text), Buffer::new(&text)]);
        TextView::new(diff, Vec::new())
    }

    /// Presses `keys`, in [`typed_in_unchanged_file`]'s notation, one per
    /// frame. A key with Shift or Ctrl comes after a press of that modifier
    /// key, as a keyboard sends it.
    fn press<State>(harness: &mut Harness<'_, State>, keys: &str) {
        for key in keys.split_whitespace() {
            let (modifiers, pressed) = key_press(key);
            if modifiers.shift {
                harness.key_press(Key::ShiftLeft);
            }
            if modifiers.ctrl {
                harness.key_press(Key::ControlLeft);
            }
            harness.key_press_modifiers(modifiers, pressed);
            if key.chars().count() == 1 {
                harness.event(egui::Event::Text(key.to_owned()));
            }
            harness.run();
        }
    }

    /// The modifiers and key of one key in [`typed_in_unchanged_file`]'s
    /// notation.
    fn key_press(key: &str) -> (Modifiers, Key) {
        let (ctrl, name) = match key.strip_prefix("C-") {
            // Ctrl is also the command key outside macOS.
            Some(name) => (Modifiers::CTRL | Modifiers::COMMAND, name),
            None => (Modifiers::NONE, key),
        };
        let (shifted, name) = match name.strip_prefix("S-") {
            Some(name) => (true, name),
            None => (false, name),
        };
        let shift = if shifted
            || matches!(name, "$" | "^")
            || (name.len() == 1 && name != name.to_lowercase())
        {
            Modifiers::SHIFT
        } else {
            Modifiers::NONE
        };
        let key = match name {
            "$" => Some(Key::Num4),
            "^" => Some(Key::Num6),
            _ => Key::from_name(&name.to_uppercase()).or(Key::from_name(name)),
        };
        (ctrl | shift, key.unwrap())
    }

    #[yare::parameterized(
        l_moves_right = { "l l l", 1, 0, 3 },
        h_moves_left = { "l l l h", 1, 0, 2 },
        j_moves_down = { "j j j", 1, 3, 0 },
        k_moves_up = { "j j j k", 1, 2, 0 },
        arrows_move = { "Down Down Right Up Right Right Left", 1, 1, 2 },
        a_count_before_h_moves_left = { "j 5 l 2 h", 1, 1, 3 },
        a_count_before_l_moves_right = { "3 l", 1, 0, 3 },
        a_count_before_j_moves_down = { "5 j", 1, 5, 0 },
        a_count_before_k_moves_up = { "j j j 2 k", 1, 1, 0 },
        gg_moves_to_the_first_line = { "j j g g", 1, 0, 0 },
        shift_g_moves_to_the_last_line = { "G", 1, 199, 0 },
        zero_moves_to_the_first_column = { "l l l 0", 1, 0, 0 },
        caret_moves_to_the_start_of_the_line = { "^", 1, 0, 0 },
        dollar_moves_to_the_end_of_the_line = { "$", 1, 0, 5 },
        ctrl_w_h_moves_to_the_left_side = { "j l C-w h", 0, 1, 1 },
        ctrl_w_ctrl_h_moves_to_the_left_side = { "C-w C-h", 0, 0, 0 },
        ctrl_w_l_moves_to_the_right_side = { "C-w h j C-w l", 1, 1, 0 },
        another_key_cancels_a_sequence_and_is_dropped = { "j j g j g", 1, 2, 0 },
        a_new_sequence_starts_after_a_cancelled_one = { "j g j g g", 1, 0, 0 },
        a_count_before_gg_moves_to_its_line = { "1 0 g g", 1, 9, 0 },
        a_count_before_shift_g_moves_to_its_line = { "1 0 G", 1, 9, 0 },
        a_count_past_the_last_line_moves_to_the_last_line = { "9 9 9 g g", 1, 199, 0 },
        a_count_is_used_once = { "5 g g j g g", 1, 0, 0 },
        a_cancelled_sequence_drops_its_count = { "5 g j g g", 1, 0, 0 },
        zero_alone_is_not_a_count = { "j 0 G", 1, 199, 0 },
    )]
    fn keys_move_the_cursor(keys: &str, side: usize, line: usize, column: usize) {
        let harness = typed_in_unchanged_file(keys);
        let cursor = harness.state().cursor_position();
        assert_eq!(cursor, Some((side, Position { line, column })));
    }

    #[test]
    fn ctrl_d_moves_down_half_a_view() {
        let harness = typed_in_unchanged_file("C-d");
        let rows_in_view = harness.state().rows_in_view;
        assert_eq!(
            harness.state().cursor_position().unwrap().1.line,
            rows_in_view / 2
        );
        assert_eq!(harness.state().top_row, rows_in_view / 2);
    }

    #[test]
    fn ctrl_u_moves_up_half_a_view() {
        let rows_in_view = typed_in_unchanged_file("").state().rows_in_view;
        let harness = typed_in_unchanged_file("G C-u");
        assert_eq!(
            harness.state().cursor_position().unwrap().1.line,
            199 - rows_in_view / 2
        );
        assert_eq!(
            harness.state().top_row,
            (199 - rows_in_view / 2).saturating_sub(rows_in_view - 1)
        );
    }

    #[test]
    fn ctrl_u_keeps_moving_cursor_after_view_reaches_the_top() {
        let rows_in_view = typed_in_unchanged_file("").state().rows_in_view;
        let half_view = (rows_in_view / 2).max(1);
        let moves_to_top = (200 - rows_in_view).div_ceil(half_view);
        let keys = format!("G {}", "C-u ".repeat(moves_to_top + 1));
        let harness = typed_in_unchanged_file(&keys);

        assert_eq!(harness.state().top_row, 0);
        assert_eq!(
            harness.state().cursor_position().unwrap().1.line,
            rows_in_view - 1 - rows_in_view / 2
        );
    }

    #[test]
    fn ctrl_f_moves_down_a_view() {
        let harness = typed_in_unchanged_file("C-f");
        let rows_in_view = harness.state().rows_in_view;
        assert_eq!(
            harness.state().cursor_position().unwrap().1.line,
            rows_in_view
        );
        assert_eq!(harness.state().top_row, rows_in_view);
    }

    #[test]
    fn ctrl_f_after_ctrl_b_moves_to_the_next_row() {
        let rows_in_view = typed_in_unchanged_file("").state().rows_in_view;
        let harness = typed_in_unchanged_file("G C-b C-f");

        assert_eq!(
            harness.state().cursor_position().unwrap().1.line,
            199 - rows_in_view + 1
        );
        assert_eq!(harness.state().top_row, 199 - rows_in_view + 1);
    }

    #[test]
    fn ctrl_d_keeps_moving_cursor_after_view_reaches_the_bottom() {
        let rows_in_view = typed_in_unchanged_file("").state().rows_in_view;
        let half_view = (rows_in_view / 2).max(1);
        let moves_to_bottom = (200 - rows_in_view).div_ceil(half_view);
        let keys = "C-d ".repeat(moves_to_bottom + 1);
        let harness = typed_in_unchanged_file(&keys);

        assert_eq!(harness.state().top_row, 200 - rows_in_view);
        assert_eq!(
            harness.state().cursor_position().unwrap().1.line,
            200 - rows_in_view + rows_in_view / 2
        );
    }

    #[test]
    fn ctrl_b_moves_up_a_view() {
        let rows_in_view = typed_in_unchanged_file("").state().rows_in_view;
        let harness = typed_in_unchanged_file("G C-b");
        assert_eq!(
            harness.state().cursor_position().unwrap().1.line,
            199 - rows_in_view
        );
        assert_eq!(
            harness.state().top_row,
            (199 - rows_in_view).saturating_sub(rows_in_view - 1)
        );
    }

    #[yare::parameterized(
        moving_to_the_bottom_row_in_view_keeps_the_view = { 0, 0 },
        moving_past_it_scrolls_down_as_far = { 3, 3 },
    )]
    fn moving_down_scrolls_once_the_cursor_leaves_the_view(past_bottom: usize, top_row: usize) {
        let rows_in_view = typed_in_unchanged_file("").state().rows_in_view;
        let keys = "j ".repeat(rows_in_view - 1 + past_bottom);
        let harness = typed_in_unchanged_file(&keys);
        assert_eq!(harness.state().top_row, top_row);
    }

    #[test]
    fn moving_up_past_the_top_of_the_view_scrolls_it_to_the_cursor() {
        let rows_in_view = typed_in_unchanged_file("").state().rows_in_view;
        // G moves the cursor to the last line, at the bottom of the view.
        let keys = format!("G {}", "k ".repeat(rows_in_view + 2));
        let harness = typed_in_unchanged_file(&keys);
        assert_eq!(harness.state().top_row, 199 - (rows_in_view + 2));
    }

    #[test]
    fn shift_g_scrolls_to_where_moving_down_stops() {
        let bottom = typed_in_unchanged_file(&"j ".repeat(250)).state().top_row;
        let harness = typed_in_unchanged_file("G");
        assert_eq!(harness.state().top_row, bottom);
    }

    #[test]
    fn zt_scrolls_the_cursor_to_the_top() {
        let harness = typed_in_unchanged_file(&format!("{}z t", "j ".repeat(50)));
        assert_eq!(harness.state().top_row, 50);
    }

    #[test]
    fn zb_scrolls_the_cursor_to_the_bottom() {
        let harness = typed_in_unchanged_file(&format!("{}z b", "j ".repeat(50)));
        let rows_in_view = harness.state().rows_in_view;
        assert_eq!(harness.state().top_row, 51 - rows_in_view);
    }

    #[test]
    fn zz_scrolls_the_cursor_to_the_center() {
        let harness = typed_in_unchanged_file(&format!("{}z z", "j ".repeat(50)));
        let rows_in_view = harness.state().rows_in_view;
        assert_eq!(
            harness.state().top_row,
            50 - rows_in_view.saturating_sub(1) / 2
        );
    }

    /// A view of a line too long to fit beside a short one, with the cursor
    /// at the start of the long line.
    fn long_line() -> Harness<'static, TextView> {
        let text = format!("{}\nshort\n", "x".repeat(500));
        let diff = SideBySide::new([Buffer::new(&text), Buffer::new(&text)]);
        let mut harness = Harness::new_ui_state(show_view, TextView::new(diff, Vec::new()));
        harness.run();
        harness
    }

    #[yare::parameterized(
        moving_to_the_end_scrolls_right = { "$", true },
        moving_back_to_the_start_scrolls_left = { "$ ^", false },
        moving_back_to_the_first_column_scrolls_left = { "$ 0", false },
    )]
    fn the_view_scrolls_horizontally_to_keep_the_cursor_in_view(keys: &str, scrolled: bool) {
        let mut harness = long_line();
        press(&mut harness, keys);
        assert_eq!(harness.state().scroll_x > 0.0, scrolled);
    }

    #[test]
    fn the_cursor_at_the_end_of_a_long_line_is_in_view() {
        let mut harness = long_line();
        press(&mut harness, "$");
        let font = TextStyle::Monospace.resolve(&harness.ctx.global_style());
        let char_width = harness.ctx.fonts_mut(|f| f.glyph_width(&font, '0'));
        let view = harness.state();
        let x = view.cursor_position().unwrap().1.column as f32 * char_width - view.scroll_x;
        // Each half's text is under half the window wide.
        let half_width = harness.ctx.content_rect().width() / 2.0;
        assert!((0.0..half_width).contains(&x), "{x} out of 0..{half_width}");
    }

    #[test]
    fn a_horizontal_wheel_scrolls_the_text() {
        let mut harness = long_line();
        harness.hover_at(harness.ctx.content_rect().center());
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(-100.0, 0.0),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
            modifiers: Modifiers::NONE,
        });
        harness.run();
        assert!(harness.state().scroll_x > 0.0);
    }

    #[test]
    fn stepping_to_a_change_moves_the_cursor_to_it() {
        let mut harness = Harness::new_ui_state(show_view, three_changes());
        harness.run();
        harness.key_press(Key::N);
        harness.run();
        let cursor = harness.state().cursor_position();
        assert_eq!(
            cursor,
            Some((
                1,
                Position {
                    line: 15,
                    column: 0
                }
            ))
        );
    }

    #[yare::parameterized(
        on_the_right_side = { "a\n", "b\n", Some(1) },
        on_the_left_side_when_the_right_has_no_lines = { "a\n", "", Some(0) },
        on_no_side_when_neither_has_lines = { "", "", None },
    )]
    fn the_cursor_starts(left: &str, right: &str, side: Option<usize>) {
        let buffers = [Buffer::new(left), Buffer::new(right)];
        let view = TextView::new(SideBySide::new(buffers), Vec::new());
        assert_eq!(view.cursor_side, side);
    }

    #[yare::parameterized(
        a_run_sits_at_its_share_of_the_map = { 200, 50..60, 20.0, 25.0..=30.0 },
        a_one_row_change_gets_the_least_height = { 800, 400..401, 20.0, 50.0..=53.0 },
        a_diff_shorter_than_the_view_keeps_its_spacing = { 10, 5..7, 50.0, 10.0..=14.0 },
    )]
    fn change_marks(
        lines: usize,
        changed: Range<usize>,
        rows_in_view: f32,
        y_range: std::ops::RangeInclusive<f32>,
    ) {
        let left: Vec<_> = (0..lines).map(|i| format!("line {i}\n")).collect();
        let mut right = left.clone();
        right[changed.clone()].fill("changed\n".to_owned());
        let diff = SideBySide::new([Buffer::new(&left.concat()), Buffer::new(&right.concat())]);
        let map = Rect::from_min_size(pos2(0.0, 0.0), vec2(10.0, 100.0));

        let y = map_y(map, diff.rows().len(), rows_in_view);
        let marks = super::change_marks(diff.rows(), map, y);

        let left = Rect::from_x_y_ranges(0.0..=5.0, y_range.clone());
        let right = Rect::from_x_y_ranges(5.0..=10.0, y_range);
        assert_eq!(marks, vec![(left, CHANGED_MARK), (right, CHANGED_MARK)]);
    }

    #[yare::parameterized(
        deleted_lines_mark_the_left_column = { "a\nb\nc\nd\n", "a\nd\n", 0.0..=5.0 },
        added_lines_mark_the_right_column = { "a\nd\n", "a\nb\nc\nd\n", 5.0..=10.0 },
    )]
    fn one_sided_marks(left: &str, right: &str, x_range: std::ops::RangeInclusive<f32>) {
        let diff = SideBySide::new([Buffer::new(left), Buffer::new(right)]);
        let map = Rect::from_min_size(pos2(0.0, 0.0), vec2(10.0, 100.0));

        let marks = super::change_marks(diff.rows(), map, map_y(map, diff.rows().len(), 4.0));

        let mark = Rect::from_x_y_ranges(x_range, 25.0..=75.0);
        assert_eq!(marks, vec![(mark, ONE_SIDED_MARK)]);
    }

    #[test]
    fn shift_zz_closes_the_window() {
        let mut harness = Harness::new_ui_state(show_view, three_changes());
        harness.run();
        harness.key_press_modifiers(Modifiers::SHIFT, Key::Z);
        harness.run();
        // Only the key down, so the one frame `step` runs is the one that
        // handles it.
        harness.event(egui::Event::Key {
            key: Key::Z,
            pressed: true,
            modifiers: Modifiers::SHIFT,
            repeat: false,
            physical_key: None,
        });
        harness.step();
        let commands = &harness.output().viewport_output[&egui::ViewportId::ROOT].commands;
        assert!(commands.contains(&egui::ViewportCommand::Close));
    }

    #[yare::parameterized(
        i_keeps_the_cursor_on_its_column = { "l l i", 2 },
        a_moves_it_after_its_character = { "l l a", 3 },
        a_on_the_last_character_moves_past_the_end = { "l l l l l a", 6 },
        shift_i_moves_to_the_start_of_the_line = { "l l I", 0 },
        shift_a_moves_to_the_end_of_the_line = { "A", 6 },
        escape_moves_it_back_a_character = { "l l a Escape", 2 },
        escape_at_the_start_of_a_line_keeps_it_there = { "i Escape", 0 },
        insert_mode_arrows_reach_past_the_end = { "i Right Right Right Right Right Right Right", 6 },
    )]
    fn mode_keys_move_the_cursor(keys: &str, column: usize) {
        let harness = typed_in_unchanged_file(keys);
        let cursor = harness.state().cursor_position();
        assert_eq!(cursor, Some((1, Position { line: 0, column })));
    }

    #[yare::parameterized(
        open_below = { "o x Escape", "line 0", "x" },
        open_above = { "O x Escape", "x", "line 0" },
    )]
    fn opening_a_line_inserts_text_on_the_new_line(
        keys: &str,
        first_line: &str,
        second_line: &str,
    ) {
        let harness = typed_in_unchanged_file(keys);
        let lines = &harness.state().diff.buffers()[1].lines()[..2];
        assert_eq!(lines, [first_line, second_line]);
    }

    #[yare::parameterized(
        open_below = { "o x Escape", 1, 0 },
        open_above = { "O x Escape", 0, 0 },
    )]
    fn opening_a_line_leaves_the_cursor_on_its_text(keys: &str, line: usize, column: usize) {
        let harness = typed_in_unchanged_file(keys);
        assert_eq!(
            harness.state().cursor_position(),
            Some((1, Position { line, column }))
        );
    }

    #[yare::parameterized(
        insert_after_i = { "i", Some("-- INSERT --") },
        insert_after_a = { "a", Some("-- INSERT --") },
        comment_after_c = { "c", Some("-- COMMENT --") },
        none_after_escape = { "i Escape", None },
        none_before_i = { "", None },
    )]
    fn mode_label_is_exposed_to_the_app(keys: &str, label: Option<&str>) {
        let harness = typed_in_unchanged_file(keys);
        assert_eq!(harness.state().mode_label(), label);
    }

    #[yare::parameterized(
        insert_mode_types_at_the_cursor = { "l i x y", "lxyine 0" },
        shift_i_types_at_the_start_of_the_line = { "l l I x", "xline 0" },
        shift_a_types_at_the_end_of_the_line = { "A x", "line 0x" },
        the_key_entering_insert_mode_types_nothing = { "a", "line 0" },
        tab_types_a_tab = { "i Tab", "\tline 0" },
        backspace_deletes_the_character_before_the_cursor = { "l l i Backspace", "lne 0" },
        backspace_at_the_start_of_a_line_joins_it_to_the_one_above = { "j i Backspace", "line 0line 1" },
        enter_ends_the_line_at_the_cursor = { "l l i Enter", "li" },
        normal_mode_types_nothing = { "x", "line 0" },
        escape_stops_typing = { "i Escape x", "line 0" },
    )]
    fn typing_edits_the_first_line(keys: &str, first_line: &str) {
        let harness = typed_in_unchanged_file(keys);
        assert_eq!(harness.state().diff.buffers()[1].lines()[0], first_line);
    }

    #[yare::parameterized(
        past_typed_text = { "i x y", 0, 2 },
        to_the_start_of_the_line_enter_makes = { "l l i Enter", 1, 0 },
        to_where_backspace_joined_the_lines = { "j i Backspace", 0, 6 },
        to_the_line_enter_makes_after_a_tab = { "i Tab Enter", 1, 0 },
    )]
    fn typing_moves_the_cursor(keys: &str, line: usize, column: usize) {
        let harness = typed_in_unchanged_file(keys);
        let cursor = harness.state().cursor_position();
        assert_eq!(cursor, Some((1, Position { line, column })));
    }

    #[test]
    fn typing_shows_the_change_it_makes() {
        let harness = typed_in_unchanged_file("i x");
        harness.get_by_label("change – of 1");
    }

    #[test]
    fn typing_keeps_the_colors_of_the_text_it_moves() {
        let mut view = unchanged_file();
        view.colors[1] = colors_per_character(&view.diff.buffers()[1]);
        let mut harness = Harness::new_ui_state(show_view, view);
        harness.run();
        press(&mut harness, "l i Enter");
        let lines = &harness.state().colors[1].lines;
        // "line 0" split into "l" and "ine 0", then "line 1".
        assert_eq!([lines[0].len(), lines[1].len(), lines[2].len()], [1, 5, 6]);
    }

    #[test]
    fn highlighting_after_an_edit_replaces_the_colors_from_the_top() {
        let mut view = unchanged_file();
        view.colors[1] = colors_per_character(&view.diff.buffers()[1]);
        let (tx, rx) = channel();
        view.colors_rx = Some(rx);
        let mut harness = Harness::new_ui_state(show_view, view);
        harness.run();
        press(&mut harness, "i x");
        tx.send(ColorsChunk {
            side: 1,
            run: 1,
            lines: vec![Vec::new(); 2],
        })
        .unwrap();
        harness.run();
        let lines = &harness.state().colors[1].lines;
        assert_eq!([lines.len(), lines[1].len(), lines[2].len()], [200, 0, 6]);
    }

    /// Colors for `buffer` with a span for each character, as highlighting
    /// can leave them.
    fn colors_per_character(buffer: &Buffer) -> SideColors {
        let line_colors = |line: &String| {
            (0..line.len())
                .map(|c| Span {
                    range: c..c + 1,
                    rgb: [9; 3],
                })
                .collect()
        };
        SideColors {
            lines: buffer.lines().iter().map(line_colors).collect(),
        }
    }

    #[test]
    fn colors_for_text_before_an_edit_are_dropped() {
        let mut view = unchanged_file();
        let (tx, rx) = channel();
        view.colors_rx = Some(rx);
        let mut harness = Harness::new_ui_state(show_view, view);
        harness.run();
        press(&mut harness, "i x");
        let lines = vec![Vec::new(); 200];
        tx.send(ColorsChunk {
            side: 1,
            run: 0,
            lines,
        })
        .unwrap();
        harness.run();
        assert!(harness.state().colors[1].lines.is_empty());
    }

    #[test]
    fn colors_in_an_earlier_theme_are_dropped() {
        let mut view = unchanged_file();
        let (tx, rx) = channel();
        view.colors_rx = Some(rx);
        let mut harness = Harness::new_ui_state(show_view, view);
        harness.run();
        harness.state_mut().set_theme(Theme::named("Nord").unwrap());
        let lines = vec![Vec::new(); 200];
        tx.send(ColorsChunk {
            side: 1,
            run: 0,
            lines,
        })
        .unwrap();
        harness.run();
        assert!(harness.state().colors[1].lines.is_empty());
    }

    #[yare::parameterized(
        in_normal_mode = { "i x Escape C-s" },
        in_insert_mode = { "i x C-s" },
    )]
    fn ctrl_s_emits_a_save_event_for_the_side_with_the_cursor(keys: &str) {
        let mut harness = Harness::new_ui_state(
            |ui, (view, events)| {
                events.extend(view.process_input(ui));
                view.show_header(ui, [Path::new("left"), Path::new("right")]);
                view.show_diff(ui);
            },
            (unchanged_file(), Vec::new()),
        );
        harness.run();
        press(&mut harness, keys);

        let (view, events) = harness.state();
        assert_eq!(events, &[ViewEvent::Save]);
        let target = view.save_target().unwrap();
        assert_eq!(target.side, 1);
        let lines: String = (0..200).map(|i| format!("line {i}\n")).collect();
        assert_eq!(target.text, format!("x{lines}"));
    }

    #[test]
    fn reload_shortcut_emits_a_reload_event() {
        let mut harness = Harness::new_ui_state(
            |ui, (view, events)| {
                events.extend(view.process_input(ui));
                view.show_header(ui, [Path::new("left"), Path::new("right")]);
                view.show_diff(ui);
            },
            (unchanged_file(), Vec::new()),
        );
        harness.run();
        #[cfg(target_os = "macos")]
        press(&mut harness, "C-r");
        #[cfg(not(target_os = "macos"))]
        press(&mut harness, "F5");

        let (_, events) = harness.state();
        assert_eq!(events, &[ViewEvent::Reload]);
    }

    #[test]
    fn reload_replaces_buffers_and_resets_view_state() {
        let mut view = three_changes();
        view.step_change(1);
        view.step_change(1);
        view.mode = Mode::Insert;
        view.selection = Some(Selection::new(
            SelectionKind::Character,
            1,
            Position { line: 0, column: 0 },
        ));
        view.normal_keys
            .press(egui::KeyboardShortcut::new(Modifiers::NONE, Key::Z));
        view.set_theme(Theme::named("Nord").unwrap());

        view.reload(SideBySide::new([
            Buffer::new("fresh left\n"),
            Buffer::new("fresh right\n"),
        ]));

        assert_eq!(view.diff.buffers()[0].lines(), ["fresh left"]);
        assert_eq!(view.diff.buffers()[1].lines(), ["fresh right"]);
        assert_eq!(view.cursor_side, Some(1));
        assert_eq!(view.current_change, None);
        assert!(view.mode == Mode::Normal);
        assert_eq!(view.selection, None);
        assert_eq!(view.theme.name(), "Nord");
        assert!(
            view.normal_keys
                .press(egui::KeyboardShortcut::new(Modifiers::NONE, Key::T))
                .is_none()
        );
    }

    #[test]
    fn reload_preserves_cursor_position_when_new_buffer_is_large_enough() {
        let mut view = unchanged_file();
        view.cursor_side = Some(0);
        view.cursor.set(
            &view.diff.buffers()[0],
            Position {
                line: 80,
                column: 3,
            },
        );

        let diff = unchanged_file().diff;
        view.reload(diff);

        assert_eq!(
            view.cursor_position(),
            Some((
                0,
                Position {
                    line: 80,
                    column: 3,
                }
            ))
        );
    }

    #[test]
    fn reload_preserves_cursor_row_in_view() {
        let mut view = unchanged_file();
        view.cursor_side = Some(0);
        view.cursor.set(
            &view.diff.buffers()[0],
            Position {
                line: 79,
                column: 0,
            },
        );
        view.top_row = 60;
        view.rows_in_view = 20;

        view.reload(unchanged_file().diff);

        let cursor_row = view.diff.row_of(0, view.cursor_position().unwrap().1.line);
        assert_eq!(view.scroll_to_top_row, Some(60));
        assert_eq!(cursor_row - view.scroll_to_top_row.unwrap(), 19);
    }

    #[test]
    fn reload_clamps_cursor_to_the_end_of_a_shorter_buffer() {
        let mut view = unchanged_file();
        view.cursor_side = Some(0);
        view.cursor.set(
            &view.diff.buffers()[0],
            Position {
                line: 80,
                column: 5,
            },
        );

        view.reload(SideBySide::new([
            Buffer::new("a\nxy"),
            Buffer::new("right\n"),
        ]));

        assert_eq!(
            view.cursor_position(),
            Some((0, Position { line: 1, column: 1 }))
        );
    }

    #[yare::parameterized(
        character_selection_extends_right = {
            "v l",
            SelectionKind::Character,
            Position { line: 0, column: 0 },
            Position { line: 0, column: 1 },
        },
        character_selection_extends_to_first_column = {
            "l l l v 0",
            SelectionKind::Character,
            Position { line: 0, column: 3 },
            Position { line: 0, column: 0 },
        },
        line_selection_extends_down = {
            "V j",
            SelectionKind::Line,
            Position { line: 0, column: 0 },
            Position { line: 1, column: 0 },
        },
    )]
    fn visual_selection_keeps_its_anchor_while_motions_move_the_cursor(
        keys: &str,
        kind: SelectionKind,
        anchor: Position,
        active: Position,
    ) {
        let harness = typed_in_unchanged_file(keys);

        assert_eq!(
            harness.state().selection,
            Some(Selection::new(kind, 1, anchor))
        );
        assert_eq!(harness.state().cursor_position(), Some((1, active)));
    }

    #[test]
    fn escape_cancels_visual_selection() {
        let harness = typed_in_unchanged_file("v l Escape");

        assert_eq!(harness.state().selection, None);
    }

    #[test]
    fn switching_sides_cancels_visual_selection() {
        let harness = typed_in_unchanged_file("v C-w h");

        assert_eq!(harness.state().selection, None);
        assert_eq!(harness.state().cursor_side, Some(0));
    }

    #[test]
    fn entering_insert_mode_cancels_visual_selection() {
        let harness = typed_in_unchanged_file("v i");

        assert_eq!(harness.state().selection, None);
        assert!(harness.state().mode == Mode::Insert);
    }

    fn row_layout() -> RowLayout {
        RowLayout {
            font: FontId::monospace(14.0),
            char_width: 8.0,
            gutter_width: 0.0,
            row_height: 18.0,
            half_width: 100.0,
            bar_cursor: false,
            palette: Palette::new(&Theme::default()),
            scroll_x: 0.0,
        }
    }

    #[test]
    fn character_selection_highlights_inclusive_unicode_glyphs_over_changed_words() {
        let layout = row_layout();
        let line = Line {
            index: 0,
            changed_words: std::iter::once(1..5).collect(),
        };
        let spans = [
            (0..1, [255, 0, 0]),
            (1..3, [0, 255, 0]),
            (3..5, [0, 0, 255]),
        ]
        .into_iter()
        .map(|(range, rgb)| Span { range, rgb })
        .collect::<Vec<_>>();
        let selection =
            Selection::new(SelectionKind::Character, 1, Position { line: 0, column: 2 });
        let active = Position { line: 0, column: 1 };

        let job = layout.line_job(
            &line,
            "aébc",
            &spans,
            layout.palette.changed,
            Some(selection),
            Some(active),
        );
        let selected_background = layout.selection_background(layout.palette.changed_word);
        let selected: Vec<_> = job
            .sections
            .iter()
            .filter(|section| section.format.background == selected_background)
            .map(|section| (section.byte_range.clone(), section.format.color))
            .collect();

        assert_eq!(
            selected,
            vec![
                (ByteIndex(1)..ByteIndex(3), Color32::from_rgb(0, 255, 0)),
                (ByteIndex(3)..ByteIndex(4), Color32::from_rgb(0, 0, 255)),
            ]
        );
        assert!(job.sections.iter().any(|section| {
            section.byte_range == (ByteIndex(4)..ByteIndex(5))
                && section.format.background == layout.palette.changed_word
        }));
    }

    #[test]
    fn line_selection_covers_complete_lines() {
        let layout = row_layout();
        let selection = Selection::new(SelectionKind::Line, 1, Position { line: 0, column: 0 });
        let active = Position { line: 1, column: 1 };
        let line = Line {
            index: 1,
            changed_words: Vec::new(),
        };

        let selected = selected_bytes(selection, active, &line, "last");
        let job = layout.line_job(
            &line,
            "last",
            &[],
            layout.palette.background,
            Some(selection),
            Some(active),
        );

        assert_eq!(selected, Some(0..4));
        assert_eq!(job.sections[0].byte_range, ByteIndex(0)..ByteIndex(4));
        assert_eq!(
            job.sections[0].format.background,
            layout.selection_background(layout.palette.background)
        );
        assert_eq!(
            selected_bytes(
                selection,
                active,
                &Line {
                    index: 2,
                    changed_words: Vec::new()
                },
                "next"
            ),
            None
        );
    }

    #[test]
    fn overlapping_selection_blends_with_changed_word_background() {
        let layout = row_layout();
        let changed_words = std::iter::once(2..6).collect::<Vec<_>>();
        let backgrounds = background_starts(
            &changed_words,
            Some(4..8),
            layout.palette.changed_word,
            layout.selection_background(layout.palette.changed),
            layout.selection_background(layout.palette.changed_word),
        )
        .collect::<Vec<_>>();

        assert_eq!(
            backgrounds,
            vec![
                (2, layout.palette.changed_word),
                (4, layout.selection_background(layout.palette.changed_word)),
                (6, layout.selection_background(layout.palette.changed)),
                (8, Color32::TRANSPARENT),
            ]
        );
    }

    #[test]
    fn adjacent_changed_word_ranges_keep_their_background() {
        let layout = row_layout();
        let backgrounds = background_starts(
            &[1..3, 3..4],
            None,
            layout.palette.changed_word,
            layout.selection_background(layout.palette.changed),
            layout.selection_background(layout.palette.changed_word),
        )
        .collect::<Vec<_>>();

        assert_eq!(
            backgrounds,
            vec![
                (1, layout.palette.changed_word),
                (3, Color32::TRANSPARENT),
                (3, layout.palette.changed_word),
                (4, Color32::TRANSPARENT),
            ]
        );
    }

    #[test]
    fn separate_selection_and_changed_word_ranges_keep_their_backgrounds() {
        let layout = row_layout();
        let changed_words = std::iter::once(1..3).collect::<Vec<_>>();
        let backgrounds = background_starts(
            &changed_words,
            Some(5..7),
            layout.palette.changed_word,
            layout.selection_background(layout.palette.changed),
            layout.selection_background(layout.palette.changed_word),
        )
        .collect::<Vec<_>>();

        assert_eq!(
            backgrounds,
            vec![
                (1, layout.palette.changed_word),
                (3, Color32::TRANSPARENT),
                (5, layout.selection_background(layout.palette.changed)),
                (7, Color32::TRANSPARENT),
            ]
        );
    }

    #[yare::parameterized(
        inclusive_unicode_characters = {
            "aébc\n",
            SelectionKind::Character,
            Position { line: 0, column: 1 },
            Position { line: 0, column: 2 },
            "éb",
        },
        reversed_multiline_crlf = {
            "abc\r\ndef\r\n",
            SelectionKind::Character,
            Position { line: 1, column: 1 },
            Position { line: 0, column: 1 },
            "bc\r\nde",
        },
        empty_line_characterwise = {
            "a\n\nb\n",
            SelectionKind::Character,
            Position { line: 1, column: 0 },
            Position { line: 1, column: 0 },
            "\n",
        },
        final_line_without_terminator = {
            "one\ntwó",
            SelectionKind::Character,
            Position { line: 1, column: 0 },
            Position { line: 1, column: 2 },
            "twó",
        },
        linewise_includes_crlf_terminator = {
            "one\r\ntwo\r\nthree",
            SelectionKind::Line,
            Position { line: 0, column: 0 },
            Position { line: 1, column: 1 },
            "one\r\ntwo\r\n",
        },
        linewise_final_line_without_terminator = {
            "one\ntwo",
            SelectionKind::Line,
            Position { line: 1, column: 0 },
            Position { line: 1, column: 0 },
            "two",
        },
    )]
    fn selected_text_respects_kind_and_buffer_endings(
        text: &str,
        kind: SelectionKind,
        anchor: Position,
        active: Position,
        expected: &str,
    ) {
        let buffer = Buffer::new(text);
        let selection = Selection::new(kind, 1, anchor);

        assert_eq!(
            selected_text(&buffer, kind, selection.range(active)).as_deref(),
            Some(expected)
        );
    }

    #[test]
    fn yank_emits_selected_text_and_clears_the_selection() {
        let mut view = TextView::new(
            SideBySide::new([Buffer::new("left"), Buffer::new("aébc")]),
            Vec::new(),
        );
        view.cursor
            .set(&view.diff.buffers()[1], Position { line: 0, column: 1 });
        view.selection = Some(Selection::new(
            SelectionKind::Character,
            1,
            Position { line: 0, column: 0 },
        ));
        let mut harness = Harness::new_ui_state(
            |ui, (view, events)| {
                events.extend(view.process_input(ui));
                view.show_header(ui, [Path::new("left"), Path::new("right")]);
                view.show_diff(ui);
            },
            (view, Vec::new()),
        );
        harness.run();
        harness.key_press(Key::Y);
        harness.run();

        let (view, events) = harness.state();
        assert_eq!(events, &[ViewEvent::Yank("aé".to_owned())]);
        assert_eq!(view.selection, None);
        assert!(view.mode == Mode::Normal);
    }

    #[test]
    fn typing_in_comment_mode_writes_the_comment_and_leaves_the_text() {
        let harness = typed_in_unchanged_file("c i x");

        let view = harness.state();
        let comment = view.draft.as_ref().map(|draft| draft.text.as_str());
        assert_eq!(
            (comment, view.diff.buffers()[1].lines()[0].as_str()),
            (Some("ix"), "line 0")
        );
    }

    #[yare::parameterized(
        the_cursor_line_without_a_selection = {
            "j c", SelectionKind::Line, Position { line: 1, column: 0 }, Position { line: 1, column: 0 }
        },
        the_lines_of_a_line_selection = {
            "V j c", SelectionKind::Line, Position { line: 0, column: 0 }, Position { line: 1, column: 0 }
        },
        the_characters_of_a_character_selection = {
            "l v l c", SelectionKind::Character, Position { line: 0, column: 1 }, Position { line: 0, column: 2 }
        },
    )]
    fn c_comments_on(keys: &str, kind: SelectionKind, start: Position, end: Position) {
        let harness = typed_in_unchanged_file(keys);

        let range = harness.state().draft.as_ref().map(|draft| draft.range);
        let expected = CommentRange {
            side: 1,
            kind,
            range: SelectionRange { start, end },
        };
        assert_eq!(range, Some(expected));
    }

    #[test]
    fn escape_discards_the_comment_and_returns_to_normal_mode() {
        let harness = typed_in_unchanged_file("c x Escape j");

        let view = harness.state();
        let line = view.cursor_position().map(|(_, position)| position.line);
        assert_eq!((view.draft.is_none(), line), (true, Some(1)));
    }

    #[yare::parameterized(
        an_empty_box_takes_three_rows_under_the_last_line = { "V j c", Gap { after: 1, rows: 3 } },
        each_line_of_the_comment_adds_a_row = { "c x S-Enter y", Gap { after: 0, rows: 4 } },
    )]
    fn the_comment_box_takes_a_gap(keys: &str, gap: Gap) {
        let harness = typed_in_unchanged_file(keys);

        assert_eq!(harness.state().gaps, vec![gap]);
    }

    #[yare::parameterized(
        enter_saves_the_comment = { "c x Enter", vec!["x"] },
        an_empty_comment_is_not_saved = { "c Enter", vec![] },
        escape_does_not_save_the_comment = { "c x Escape", vec![] },
        c_on_a_commented_line_edits_its_comment = { "c x Enter c y Enter", vec!["xy"] },
        emptying_a_comment_deletes_it = { "c x Enter c Backspace Enter", vec![] },
        escape_leaves_an_edited_comment_as_it_was = { "c x Enter c y Escape", vec!["x"] },
        c_with_a_selection_adds_a_comment = { "c x Enter V c y Enter", vec!["x", "y"] },
        shift_enter_starts_a_new_line = { "c x S-Enter y Enter", vec!["x\ny"] },
    )]
    fn saved_comments(keys: &str, texts: Vec<&str>) {
        let harness = typed_in_unchanged_file(keys);

        let view = harness.state();
        let saved: Vec<_> = view.comments.iter().map(|c| c.text.as_str()).collect();
        assert_eq!((saved, view.mode_label()), (texts, None));
    }

    #[yare::parameterized(
        a_saved_comment_keeps_its_box_beside_the_draft = {
            "c x Enter j c", vec![Gap { after: 0, rows: 2 }, Gap { after: 1, rows: 3 }]
        },
        boxes_follow_the_order_of_their_lines = {
            "j c x Enter k c", vec![Gap { after: 0, rows: 3 }, Gap { after: 1, rows: 2 }]
        },
        an_edited_comment_shows_only_the_draft_box = {
            "c x Enter c", vec![Gap { after: 0, rows: 3 }]
        },
        boxes_under_one_line_follow_each_other = {
            "c x S-Enter y Enter V c", vec![Gap { after: 0, rows: 3 }, Gap { after: 0, rows: 3 }]
        },
    )]
    fn comment_boxes(keys: &str, gaps: Vec<Gap>) {
        let harness = typed_in_unchanged_file(keys);

        assert_eq!(harness.state().gaps, gaps);
    }

    #[yare::parameterized(
        saving_a_comment = { "c x Enter", vec![ViewEvent::CommentsChanged] },
        not_saving_an_empty_comment = { "c Enter", vec![] },
        deleting_a_comment = {
            "c x Enter c Backspace Enter",
            vec![ViewEvent::CommentsChanged, ViewEvent::CommentsChanged]
        },
    )]
    fn comments_changed_event(keys: &str, expected: Vec<ViewEvent>) {
        let mut harness = Harness::new_ui_state(
            |ui, (view, events)| {
                events.extend(view.process_input(ui));
                view.show_diff(ui);
            },
            (unchanged_file(), Vec::new()),
        );
        harness.run();
        press(&mut harness, keys);

        assert_eq!(harness.state().1, expected);
    }

    #[yare::parameterized(
        comment_saves_the_comment = { "Comment", vec!["x"] },
        cancel_discards_the_comment = { "Cancel", vec![] },
    )]
    fn clicking_a_comment_box_button(label: &str, texts: Vec<&str>) {
        let mut harness = typed_in_unchanged_file("c x");

        harness.get_by_role_and_label(Role::Button, label).click();
        harness.run();

        let view = harness.state();
        let saved: Vec<_> = view.comments.iter().map(|c| c.text.as_str()).collect();
        assert_eq!((saved, view.mode_label()), (texts, None));
    }

    #[test]
    fn clicking_comment_emits_a_comments_changed_event() {
        let mut harness = Harness::new_ui_state(
            |ui, (view, events): &mut (TextView, Vec<ViewEvent>)| {
                view.process_input(ui);
                events.extend(view.show_diff(ui));
            },
            (unchanged_file(), Vec::new()),
        );
        harness.run();
        press(&mut harness, "c x");

        harness
            .get_by_role_and_label(Role::Button, "Comment")
            .click();
        harness.run();

        assert_eq!(harness.state().1, [ViewEvent::CommentsChanged]);
    }

    #[test]
    fn review_comments_quote_the_commented_lines_of_their_side() {
        let harness = typed_in_unchanged_file("j V j c x Enter");

        let paths = [Path::new("old.rs"), Path::new("new.rs")];
        let comments = harness.state().review_comments(paths);

        let json = serde_json::to_value(comments).unwrap();
        let expected = serde_json::json!([{
            "path": "new.rs",
            "side": "right",
            "start": { "line": 2 },
            "end": { "line": 3 },
            "quote": "line 1\nline 2\n",
            "body": "x",
        }]);
        assert_eq!(json, expected);
    }

    #[test]
    fn a_comment_past_the_end_of_its_side_has_no_box() {
        let position = Position { line: 5, column: 0 };
        let range = CommentRange {
            side: 1,
            kind: SelectionKind::Line,
            range: SelectionRange {
                start: position,
                end: position,
            },
        };
        let comment = Comment {
            range,
            text: "gone".to_owned(),
        };
        let diff = SideBySide::new([Buffer::new("a\n"), Buffer::new("b\n")]);
        let mut harness = Harness::new_ui_state(show_view, TextView::new(diff, vec![comment]));

        harness.run();

        assert_eq!(harness.state().gaps, vec![]);
    }

    /// A line comment on `line` of `side`.
    fn line_comment(side: usize, line: usize) -> Comment {
        let position = Position { line, column: 0 };
        let range = SelectionRange {
            start: position,
            end: position,
        };
        let kind = SelectionKind::Line;
        Comment {
            range: CommentRange { side, kind, range },
            text: format!("on line {line}"),
        }
    }

    #[yare::parameterized(
        next_goes_to_the_first_comment_below = { "] c", 1, 2 },
        next_crosses_to_the_other_side = { "] c ] c", 0, 5 },
        next_takes_a_count = { "2 ] c", 0, 5 },
        next_stays_put_after_the_last = { "] c ] c ] c ] c", 1, 9 },
        prev_goes_to_the_first_comment_above = { "G [ c", 1, 9 },
        prev_stays_put_before_the_first = { "] c [ c", 1, 2 },
    )]
    fn bracket_c_steps_between_comments(keys: &str, side: usize, line: usize) {
        let comments = vec![line_comment(1, 9), line_comment(1, 2), line_comment(0, 5)];
        let view = TextView::new(unchanged_file().diff, comments);
        let mut harness = Harness::new_ui_state(show_view, view);
        harness.run();

        press(&mut harness, keys);

        let cursor = harness.state().cursor_position();
        assert_eq!(cursor, Some((side, Position { line, column: 0 })));
    }

    #[yare::parameterized(
        a_fitting_comment_keeps_its_last_line_in_view = { 90, 95, true },
        a_tall_comment_keeps_its_first_line_in_view = { 20, 80, false },
    )]
    fn bracket_c_keeps_comment_lines_in_view(
        start_line: usize,
        end_line: usize,
        last_line_in_view: bool,
    ) {
        let range = SelectionRange {
            start: Position {
                line: start_line,
                column: 0,
            },
            end: Position {
                line: end_line,
                column: 0,
            },
        };
        let comment = Comment {
            range: CommentRange {
                side: 1,
                kind: SelectionKind::Line,
                range,
            },
            text: String::new(),
        };
        let view = TextView::new(unchanged_file().diff, vec![comment]);
        let mut harness = Harness::new_ui_state(show_view, view);
        harness.run();

        press(&mut harness, "] c");

        let view = harness.state();
        let start_row = view.view_row_of(1, start_line);
        let end_row = view.view_row_of(1, end_line);
        let bottom_row = view.top_row + view.rows_in_view;
        assert!(start_row >= view.top_row);
        assert_eq!(end_row < bottom_row, last_line_in_view);
    }

    #[test]
    fn bracket_c_keeps_the_first_comment_box_in_view_from_the_top() {
        let comment_line = 90;
        let comments = vec![line_comment(1, comment_line), line_comment(1, 160)];
        let view = TextView::new(unchanged_file().diff, comments);
        let mut harness = Harness::new_ui_state(show_view, view);
        harness.run();

        press(&mut harness, "] c");

        let view = harness.state();
        let comment_row = view.view_row_of(1, comment_line);
        let comment_end = view
            .view_rows()
            .view_row(view.diff.row_of(1, comment_line) + 1);
        assert!(comment_row >= view.top_row);
        assert!(comment_end <= view.top_row + view.rows_in_view);
    }

    #[test]
    fn a_comment_marks_the_change_map_at_its_share_of_the_map() {
        let map = Rect::from_min_size(pos2(0.0, 0.0), vec2(10.0, 100.0));

        let marks = comment_marks([50].into_iter(), map, map_y(map, 200, 20.0));

        assert_eq!(marks, vec![Rect::from_x_y_ranges(0.0..=10.0, 25.0..=28.0)]);
    }

    #[test]
    fn a_comment_moves_with_lines_typed_above_it() {
        let harness = typed_in_unchanged_file("j c x Enter k O Escape");

        let lines: Vec<_> = harness
            .state()
            .comments
            .iter()
            .map(|c| c.range.range.start.line)
            .collect();

        assert_eq!(lines, vec![2]);
    }
}
