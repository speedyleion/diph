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
    self, Align2, Color32, CursorIcon, FontId, Galley, Painter, Pos2, Rect, ScrollArea, Sense,
    Stroke, TextFormat, TextStyle, UiBuilder, epaint::text::Glyph, pos2, text::LayoutJob, vec2,
};
use itertools::{
    EitherOrBoth::{Both, Left, Right},
    Itertools as _,
};

use crate::cursor::Cursor;
use crate::keymap::{
    Action, InsertAt, Keymap, OpenLineAt, insert_keymap, normal_keymap, pressed_key,
};
use crate::palette::{CHANGED_MARK, ONE_SIDED_MARK, Palette};
use crate::selection::{Selection, SelectionKind};

/// Width of the cursor in insert mode, a bar before its column.
const CURSOR_BAR_WIDTH: f32 = 2.0;

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ViewEvent {
    Save,
    Reload,
}

pub(crate) struct SaveTarget {
    pub(crate) side: usize,
    pub(crate) text: String,
}

pub struct TextView {
    diff: SideBySide,
    /// Index into `diff.changes()` of the change last jumped to.
    current_change: Option<usize>,
    /// Row to scroll to the top of the view on the next frame.
    scroll_to_top_row: Option<usize>,
    /// Top row wholly in view in the last frame.
    top_row: usize,
    /// Rows wholly in view in the last frame.
    rows_in_view: usize,
    /// The side with the cursor, 0 for the left and 1 for the right. `None`
    /// when neither side has lines.
    cursor_side: Option<usize>,
    /// The cursor in the buffer of `cursor_side`.
    cursor: Cursor,
    selection: Option<Selection>,
    mode: Mode,
    /// Whether the last key pressed was bound to an action, so the text it
    /// typed is not inserted.
    key_handled: bool,
    normal_keys: Keymap<Action>,
    insert_keys: Keymap<Action>,
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
    pub fn new(diff: SideBySide) -> Self {
        let buffers = diff.buffers();
        // The right side, unless only the left has lines.
        let cursor_side = [1, 0].into_iter().find(|&s| !buffers[s].lines().is_empty());
        let theme = Theme::default();
        let mut view = Self {
            cursor_side,
            cursor: Cursor::default(),
            selection: None,
            diff,
            current_change: None,
            scroll_to_top_row: None,
            top_row: 0,
            rows_in_view: 0,
            mode: Mode::Normal,
            key_handled: false,
            normal_keys: normal_keymap(),
            insert_keys: insert_keymap(),
            colors: Default::default(),
            colors_rx: None,
            highlight_runs: Default::default(),
            colors_received: [0; 2],
            highlighting: None,
            palette: Palette::new(&theme),
            theme: Arc::new(theme),
        };
        view.reset_diff_state();
        view.step_change(1);
        view
    }

    /// Replaces the compared text, keeping the cursor within the new text.
    pub(crate) fn reload(&mut self, diff: SideBySide) {
        let cursor_row = self
            .cursor_position()
            .map(|(side, position)| self.diff.row_of(side, position.line));
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
            self.diff
                .row_of(side, position.line)
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
        self.key_handled = false;
        self.normal_keys.clear();
        self.insert_keys.clear();
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

    pub(crate) fn is_insert_mode(&self) -> bool {
        self.mode == Mode::Insert
    }

    pub(crate) fn process_input(&mut self, ui: &mut egui::Ui) -> Vec<ViewEvent> {
        let events = ui.input(|i| i.events.clone());
        let mut view_events = Vec::new();
        view_events.extend(events.iter().filter_map(|e| self.handle(ui.ctx(), e)));
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

    pub(crate) fn show_diff(&mut self, ui: &mut egui::Ui) {
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
        egui::CentralPanel::default().show(ui, |ui| self.diff_ui(ui));
    }

    /// The rows, beside a map of the whole diff that marks each side's
    /// changes in a column of its own and covers the rows in view.
    fn diff_ui(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let (rows_rect, map) = rect.split_left_right_at_x(rect.right() - MAP_WIDTH);
        let rows_builder = UiBuilder::new().max_rect(rows_rect);
        let in_view = ui.scope_builder(rows_builder, |ui| self.rows_ui(ui)).inner;

        let y = map_y(map, self.diff.rows().len(), in_view.end - in_view.start);
        let painter = ui.painter();
        painter.rect_filled(map, 0.0, self.palette.filler);
        for (mark, color) in change_marks(self.diff.rows(), map, &y) {
            painter.rect_filled(mark, 0.0, color);
        }
        let cover = Rect::from_x_y_ranges(map.x_range(), y(in_view.start)..=y(in_view.end));
        painter.rect_filled(cover, 0.0, self.palette.in_view_cover);
    }

    /// Performs the action of a pressed key, or in insert mode inserts typed
    /// text that a bound key did not type.
    fn handle(&mut self, ctx: &egui::Context, event: &egui::Event) -> Option<ViewEvent> {
        if let Some(key) = pressed_key(event) {
            let keymap = match self.mode {
                Mode::Normal => &mut self.normal_keys,
                Mode::Insert => &mut self.insert_keys,
            };
            let pressed = keymap.press(key);
            self.key_handled = pressed.is_some();
            if let Some((action, count)) = pressed {
                return self.perform(ctx, action, count);
            }
        } else if let egui::Event::Text(text) = event
            && self.mode == Mode::Insert
            && !std::mem::take(&mut self.key_handled)
        {
            self.type_text(text);
        }
        None
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
            Action::LineStart => self.move_cursor(|c, b| c.go_to_line_start(b)),
            Action::LineEnd => self.move_cursor(|c, b| c.go_to_line_end(b)),
            Action::LeftSide => self.switch_side(0),
            Action::RightSide => self.switch_side(1),
            Action::Close => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Action::Save => return Some(ViewEvent::Save),
            Action::Reload => return Some(ViewEvent::Reload),
            Action::StartCharacterSelection => self.start_selection(SelectionKind::Character),
            Action::StartLineSelection => self.start_selection(SelectionKind::Line),
            Action::CancelSelection => self.selection = None,
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
        self.rehighlight(side);
        edit
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
        let row = self.diff.row_of(side, line);
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
        let max_top = self.diff.rows().len().saturating_sub(rows_in_view);
        let old_top = self.scroll_to_top_row.unwrap_or(self.top_row);
        let top = self
            .scroll_to_top_row
            .unwrap_or(self.top_row)
            .saturating_add_signed(rows.saturating_mul(count))
            .min(max_top);
        self.scroll_to_top_row = Some(top);
        let row = match top == old_top {
            true => self
                .diff
                .row_of(side, position.line)
                .saturating_add_signed(rows.saturating_mul(count))
                .min(self.diff.rows().len().saturating_sub(1)),
            false => self
                .diff
                .row_of(side, position.line)
                .clamp(top, top + rows_in_view.saturating_sub(1)),
        };
        let Some(line) = self.diff.line_at(side, row) else {
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
        let row = self.diff.row_of(side, position.line);
        self.scroll_to_top_row = Some(top_row(row, rows_in_view));
    }

    /// Moves `delta` changes forward or back, stopping at the first and last,
    /// and moves the cursor to the change on its side.
    fn step_change(&mut self, delta: isize) {
        let Some(last) = self.diff.changes().len().checked_sub(1) else {
            return;
        };
        let next = match self.current_change {
            None => 0,
            Some(c) => c.saturating_add_signed(delta).min(last),
        };
        self.current_change = Some(next);
        let row = self.diff.changes()[next];
        self.scroll_to_top_row = Some(row.saturating_sub(CONTEXT_ROWS));
        if let Some(side) = self.cursor_side
            && let Some(line) = self.diff.line_at(side, row)
        {
            self.cursor.go_to_line(&self.diff.buffers()[side], line);
        }
    }

    /// Both halves live in one scroll area so they always scroll together.
    /// Returns the rows in view, in fractions of a row.
    fn rows_ui(&mut self, ui: &mut egui::Ui) -> Range<f32> {
        let font = TextStyle::Monospace.resolve(ui.style());
        let char_width = ui.fonts_mut(|f| f.glyph_width(&font, '0'));
        let rows = self.diff.rows();
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
        };

        ui.spacing_mut().item_spacing.y = 0.0;
        let mut area = ScrollArea::vertical().auto_shrink(false);
        if let Some(row) = self.scroll_to_top_row.take() {
            // Clamped so rows near the end don't overscroll and bounce back.
            let row_height = layout.row_height;
            let max_offset = rows.len() as f32 * row_height - ui.available_height();
            let offset = row as f32 * row_height;
            area = area.vertical_scroll_offset(offset.min(max_offset).max(0.0));
        }
        let cursor = self.cursor_position();
        let output = area.show_rows(ui, layout.row_height, rows.len(), |ui, range| {
            // `last` paints every row, keeping the click if one was clicked.
            rows[range]
                .iter()
                .filter_map(|row| {
                    layout.paint_row(ui, self.diff.buffers(), &self.colors, row, cursor)
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
        top..bottom
    }
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
}

impl RowLayout {
    /// Paints `row` across both halves, with each side's text from its
    /// buffer, a divider between them, and the cursor if it is on one of the
    /// row's lines, given as its side and position. Returns the side and
    /// position of the character a click on a line's half was on.
    fn paint_row(
        &self,
        ui: &mut egui::Ui,
        buffers: &[Buffer; 2],
        colors: &[SideColors; 2],
        row: &Row,
        cursor: Option<(usize, Position)>,
    ) -> Option<(usize, Position)> {
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
            let (pos, galley) = self.paint_half(ui, half, line, text, spans, background);
            let glyphs = galley.rows.first().map_or(&[][..], |r| &r.glyphs);
            if let Some((_, c)) = cursor.filter(|(s, c)| *s == side && c.line == line.index) {
                self.paint_cursor(painter, half, pos, glyphs, c.column);
            }
            if let Some(click) = click.filter(|p| half.contains(*p)) {
                let column = column_at(glyphs, click.x - pos.x);
                let line = line.index;
                clicked = Some((side, Position { line, column }));
            }
        }
        let divider = Stroke::new(1.0, self.palette.divider);
        painter.vline(left.right(), rect.y_range(), divider);
        clicked
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
        let end_x = glyphs.last().map_or(0.0, Glyph::max_x);
        let x = pos.x + glyph.map_or(end_x, |g| g.pos.x);
        let width = match (self.bar_cursor, glyph) {
            (true, _) => CURSOR_BAR_WIDTH,
            (false, Some(g)) => g.advance_width,
            (false, None) => self.char_width,
        };
        let block = Rect::from_x_y_ranges(x..=x + width, half.y_range());
        let text_rect = Rect::from_min_max(pos2(pos.x, half.top()), half.max);
        let painter = painter.with_clip_rect(text_rect);
        painter.rect_filled(block, 0.0, self.palette.cursor);
        if let Some(glyph) = glyph.filter(|_| !self.bar_cursor) {
            let (align, font) = (Align2::LEFT_CENTER, self.font.clone());
            let color = self.palette.cursor_text;
            painter.text(block.left_center(), align, glyph.chr, font, color);
        }
    }

    /// Paints one half of a row that has a line: the background, the line
    /// number, and the line's `text` in the colors of its spans, clipped to
    /// the half, with its changed words highlighted. Returns where the text
    /// starts, and its layout.
    fn paint_half(
        &self,
        ui: &egui::Ui,
        half: Rect,
        line: &Line,
        text: &str,
        spans: &[Span],
        background: Option<Color32>,
    ) -> (Pos2, Arc<Galley>) {
        let painter = ui.painter();
        let mid_y = half.center().y;
        let gutter_right = half.left() + self.gutter_width;
        if let Some(background) = background {
            painter.rect_filled(half, 0.0, background);
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
        let galley = ui.fonts_mut(|f| f.layout_job(self.line_job(line, text, spans)));
        let pos = pos2(gutter_right, mid_y - galley.size().y / 2.0);
        painter
            .with_clip_rect(text_rect)
            .galley(pos, galley.clone(), self.palette.text);
        (pos, galley)
    }

    /// Lays out `line`'s `text` in the colors of `spans`, with its changed
    /// words on the palette's changed word background.
    fn line_job(&self, line: &Line, text: &str, spans: &[Span]) -> LayoutJob {
        let color_starts = spans.iter().map(|span| {
            let [r, g, b] = span.rgb;
            (span.range.start, Color32::from_rgb(r, g, b))
        });
        let changed_word = self.palette.changed_word;
        let background_starts = (line.changed_words.iter())
            .flat_map(|r| [(r.start, changed_word), (r.end, Color32::TRANSPARENT)]);
        let mut job = LayoutJob::default();
        let (mut start, mut color, mut background) = (0, self.palette.text, Color32::TRANSPARENT);
        // Each event starts a new color, a new background, or both.
        for event in color_starts.merge_join_by(background_starts, |a, b| a.0.cmp(&b.0)) {
            let end = event.as_ref().reduce(|a, _| a).0;
            self.append_piece(&mut job, &text[start..end], color, background);
            start = end;
            match event {
                Left((_, c)) => color = c,
                Right((_, b)) => background = b,
                Both((_, c), (_, b)) => (color, background) = (c, b),
            }
        }
        self.append_piece(&mut job, &text[start..], color, background);
        job
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

/// The index of the glyph of `glyphs`, laid out from 0, that `x` is on, or
/// of the last glyph when `x` is past it.
fn column_at(glyphs: &[Glyph], x: f32) -> usize {
    let column = glyphs.iter().take_while(|g| g.max_x() <= x).count();
    column.min(glyphs.len().saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Key, Modifiers};
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
        TextView::new(SideBySide::new([Buffer::new(&left), Buffer::new(&right)]))
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

    /// The view of 200 equal lines, after pressing `keys` one per frame.
    /// `keys` are separated by spaces, each a character, uppercase with
    /// Shift, a key name such as `Down`, or either with `C-` for Ctrl.
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
        TextView::new(diff)
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
        let shift = if matches!(name, "$" | "^") || (name.len() == 1 && name != name.to_lowercase())
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
        let view = TextView::new(SideBySide::new(buffers));
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
        after_i = { "i", true },
        after_a = { "a", true },
        not_after_escape = { "i Escape", false },
        not_before_i = { "", false },
    )]
    fn insert_mode_state_is_exposed_to_the_app(keys: &str, insert_mode: bool) {
        let harness = typed_in_unchanged_file(keys);
        assert_eq!(harness.state().is_insert_mode(), insert_mode);
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
        assert!(harness.state().is_insert_mode());
    }
}
