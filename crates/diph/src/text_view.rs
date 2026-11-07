//! Side-by-side text diff view.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use diph_core::{Buffer, Highlighter, Line, Position, Row, RowKind, SideBySide, Span};
use eframe::egui::{
    self, Align2, Color32, CursorIcon, FontId, Galley, Key, KeyboardShortcut, Modifiers, Painter,
    Pos2, Rect, ScrollArea, Sense, Stroke, TextFormat, TextStyle, UiBuilder, epaint::text::Glyph,
    pos2, text::LayoutJob, vec2,
};
use itertools::{
    EitherOrBoth::{Both, Left, Right},
    Itertools as _,
};

use crate::cursor::Cursor;
use crate::keymap::Keymap;

/// Background of a line with nothing opposite it.
const ONE_SIDED_BG: Color32 = Color32::from_rgb(0x1b, 0x3d, 0x2a);
/// Background of a line that differs from the line opposite it.
const CHANGED_BG: Color32 = Color32::from_rgb(0x1f, 0x30, 0x4a);
/// Background of the words that differ within a changed line.
const CHANGED_WORD_BG: Color32 = Color32::from_rgb(0x2f, 0x55, 0x8f);
/// Background opposite a one-sided line.
const FILLER_BG: Color32 = Color32::from_rgb(0x22, 0x24, 0x28);
const GUTTER_FG: Color32 = Color32::from_gray(110);
const TEXT_FG: Color32 = Color32::from_gray(210);
const DIVIDER: Color32 = Color32::from_gray(60);
const CURSOR_BG: Color32 = TEXT_FG;
/// Color of the character under the cursor.
const CURSOR_FG: Color32 = Color32::from_gray(20);
/// Change map mark of one-sided lines.
const ONE_SIDED_MARK: Color32 = Color32::from_rgb(0x2f, 0x8f, 0x55);
/// Change map mark of changed lines.
const CHANGED_MARK: Color32 = Color32::from_rgb(0x3d, 0x6f, 0xc4);
/// Change map cover over the rows in view.
const IN_VIEW_COVER: Color32 = Color32::from_rgba_premultiplied(24, 24, 24, 24);

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
    lines: Vec<Vec<Span>>,
}

/// What a key sequence asks the view to do.
#[derive(Clone, Copy)]
enum Action {
    PrevChange,
    NextChange,
    CharLeft,
    CharRight,
    LineDown,
    LineUp,
    FirstLine,
    LastLine,
    LeftSide,
    RightSide,
    Close,
}

/// p or Shift+F7 and n or F7 step through changes. h, j, k, and l or the
/// arrow keys move the cursor a character or a line, gg and G move it to
/// the first and last line, and Ctrl+W h and Ctrl+W l move it to the left
/// and right side. ZZ closes the window.
fn keymap() -> Keymap<Action> {
    let key = |key| KeyboardShortcut::new(Modifiers::NONE, key);
    let shift = |key| KeyboardShortcut::new(Modifiers::SHIFT, key);
    let ctrl = |key| KeyboardShortcut::new(Modifiers::CTRL, key);
    Keymap::new([
        (vec![key(Key::P)], Action::PrevChange),
        (vec![shift(Key::F7)], Action::PrevChange),
        (vec![key(Key::N)], Action::NextChange),
        (vec![key(Key::F7)], Action::NextChange),
        (vec![key(Key::H)], Action::CharLeft),
        (vec![key(Key::ArrowLeft)], Action::CharLeft),
        (vec![key(Key::L)], Action::CharRight),
        (vec![key(Key::ArrowRight)], Action::CharRight),
        (vec![key(Key::J)], Action::LineDown),
        (vec![key(Key::ArrowDown)], Action::LineDown),
        (vec![key(Key::K)], Action::LineUp),
        (vec![key(Key::ArrowUp)], Action::LineUp),
        (vec![key(Key::G), key(Key::G)], Action::FirstLine),
        (vec![shift(Key::G)], Action::LastLine),
        (vec![ctrl(Key::W), key(Key::H)], Action::LeftSide),
        (vec![ctrl(Key::W), ctrl(Key::H)], Action::LeftSide),
        (vec![ctrl(Key::W), key(Key::L)], Action::RightSide),
        (vec![ctrl(Key::W), ctrl(Key::L)], Action::RightSide),
        (vec![shift(Key::Z), shift(Key::Z)], Action::Close),
    ])
}

pub struct TextView {
    diff: SideBySide,
    left_label: String,
    right_label: String,
    /// Index into `diff.changes` of the change last jumped to.
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
    keymap: Keymap<Action>,
    /// Syntax colors of the left and right side, filled in from the top as
    /// the background highlighter delivers them.
    colors: [SideColors; 2],
    colors_rx: Option<Receiver<ColorsChunk>>,
    /// When diph started.
    started: Instant,
    /// Time from `started` to the first frame.
    startup_time: Option<Duration>,
}

impl TextView {
    /// A view of `diff` whose status bar shows the time from `started` to
    /// the first frame.
    pub fn new(
        diff: SideBySide,
        left_label: String,
        right_label: String,
        started: Instant,
    ) -> Self {
        let buffers = diff.buffers();
        // The right side, unless only the left has lines.
        let cursor_side = [1, 0].into_iter().find(|&s| !buffers[s].lines().is_empty());
        let mut view = Self {
            cursor_side,
            cursor: Cursor::default(),
            diff,
            left_label,
            right_label,
            current_change: None,
            scroll_to_top_row: None,
            top_row: 0,
            rows_in_view: 0,
            keymap: keymap(),
            colors: Default::default(),
            colors_rx: None,
            started,
            startup_time: None,
        };
        view.step_change(1);
        view
    }

    /// Starts highlighting both sides on background threads, picking each
    /// side's language from its path. Lines show uncolored until their
    /// colors arrive.
    pub fn highlight_in_background(&mut self, paths: [PathBuf; 2], ctx: egui::Context) {
        let (tx, rx) = channel();
        self.colors_rx = Some(rx);
        let sides = self.diff.buffers().each_ref().map(|b| b.lines().to_vec());
        std::thread::spawn(move || highlight_sides(&paths, &sides, &tx, &ctx));
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let started = self.started;
        let startup_time = *self.startup_time.get_or_insert_with(|| started.elapsed());
        if let Some(rx) = &self.colors_rx {
            for chunk in rx.try_iter() {
                self.colors[chunk.side].lines.extend(chunk.lines);
            }
        }
        let actions = ui.input(|i| self.keymap.actions(i));
        for action in actions {
            self.perform(ui.ctx(), action);
        }

        egui::Panel::top("text_header").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Prev").on_hover_text("p / Shift+F7").clicked() {
                    self.step_change(-1);
                }
                if ui.button("Next").on_hover_text("n / F7").clicked() {
                    self.step_change(1);
                }
                let current = self
                    .current_change
                    .map_or("–".to_owned(), |c| (c + 1).to_string());
                ui.label(format!("change {current} of {}", self.diff.changes.len()));
            });
            ui.columns(2, |cols| {
                cols[0].monospace(&self.left_label);
                cols[1].monospace(&self.right_label);
            });
        });
        egui::Panel::bottom("status_bar").show(ui, |ui| {
            ui.weak(format!("started in {} ms", startup_time.as_millis()));
        });
        egui::CentralPanel::default().show(ui, |ui| self.diff_ui(ui));
    }

    /// The rows, beside a map of the whole diff that marks each side's
    /// changes in a column of its own and covers the rows in view.
    fn diff_ui(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let (rows_rect, map) = rect.split_left_right_at_x(rect.right() - MAP_WIDTH);
        let rows_builder = UiBuilder::new().max_rect(rows_rect);
        let in_view = ui.scope_builder(rows_builder, |ui| self.rows_ui(ui)).inner;

        let y = map_y(map, self.diff.rows.len(), in_view.end - in_view.start);
        let painter = ui.painter();
        painter.rect_filled(map, 0.0, FILLER_BG);
        for (mark, color) in change_marks(&self.diff.rows, map, &y) {
            painter.rect_filled(mark, 0.0, color);
        }
        let cover = Rect::from_x_y_ranges(map.x_range(), y(in_view.start)..=y(in_view.end));
        painter.rect_filled(cover, 0.0, IN_VIEW_COVER);
    }

    fn perform(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::PrevChange => self.step_change(-1),
            Action::NextChange => self.step_change(1),
            Action::CharLeft => self.move_cursor(|c, b| c.move_column(b, -1)),
            Action::CharRight => self.move_cursor(|c, b| c.move_column(b, 1)),
            Action::LineDown => self.move_cursor(|c, b| c.move_line(b, 1)),
            Action::LineUp => self.move_cursor(|c, b| c.move_line(b, -1)),
            Action::FirstLine => self.move_cursor(|c, b| c.go_to_line(b, 0)),
            Action::LastLine => self.move_cursor(|c, b| c.go_to_line(b, usize::MAX)),
            Action::LeftSide => self.switch_side(0),
            Action::RightSide => self.switch_side(1),
            Action::Close => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    /// The side with the cursor, and where the cursor is in its buffer.
    fn cursor_position(&self) -> Option<(usize, Position)> {
        let side = self.cursor_side?;
        Some((side, self.cursor.position(&self.diff.buffers()[side])))
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

    /// Moves `delta` changes forward or back, stopping at the first and last,
    /// and moves the cursor to the change on its side.
    fn step_change(&mut self, delta: isize) {
        let Some(last) = self.diff.changes.len().checked_sub(1) else {
            return;
        };
        let next = match self.current_change {
            None => 0,
            Some(c) => c.saturating_add_signed(delta).min(last),
        };
        self.current_change = Some(next);
        let row = self.diff.changes[next];
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
        let rows = &self.diff.rows;
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

/// Highlights both sides at once, sending each side's colors over `tx` in
/// chunks and requesting a repaint after each. Stops once the window has
/// closed.
fn highlight_sides(
    paths: &[PathBuf; 2],
    sides: &[Vec<String>; 2],
    tx: &Sender<ColorsChunk>,
    ctx: &egui::Context,
) {
    let highlighter = &Highlighter::default();
    std::thread::scope(|scope| {
        for (side, (path, lines)) in paths.iter().zip(sides).enumerate() {
            scope.spawn(move || highlight_side(highlighter, path, lines, side, tx, ctx));
        }
    });
}

/// Highlights one side's `lines`, sending them over `tx` tagged with `side`.
fn highlight_side(
    highlighter: &Highlighter,
    path: &Path,
    lines: &[String],
    side: usize,
    tx: &Sender<ColorsChunk>,
    ctx: &egui::Context,
) {
    let mut spans = highlighter.highlight(path, lines).peekable();
    while spans.peek().is_some() {
        let lines = spans.by_ref().take(HIGHLIGHT_CHUNK).collect();
        // Fails only once the window has closed.
        if tx.send(ColorsChunk { side, lines }).is_err() {
            return;
        }
        ctx.request_repaint();
    }
}

/// Background of the sides of `row` that have a line, if the row differs.
fn background(row: &Row) -> Option<Color32> {
    match row.kind {
        RowKind::Equal => None,
        RowKind::LeftOnly | RowKind::RightOnly => Some(ONE_SIDED_BG),
        RowKind::Changed => Some(CHANGED_BG),
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
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let response = response.on_hover_cursor(CursorIcon::Text);
        let click = response
            .interact_pointer_pos()
            .filter(|_| response.clicked());
        let painter = ui.painter();
        let (left, right) = rect.split_left_right_at_x(rect.left() + self.half_width);
        let background = background(row);
        let mut clicked = None;
        for (side, (half, line)) in [(left, &row.left), (right, &row.right)]
            .into_iter()
            .enumerate()
        {
            let Some(line) = line else {
                painter.rect_filled(half, 0.0, FILLER_BG);
                continue;
            };
            let text = &buffers[side].lines()[line.index];
            let spans = colors[side].line(line);
            let (pos, galley) = self.paint_half(ui, half, line, text, spans, background);
            let glyphs = galley.rows.first().map_or(&[][..], |r| &r.glyphs);
            if let Some((_, c)) = cursor.filter(|(s, c)| *s == side && c.line == line.index) {
                self.paint_cursor(painter, half, pos, glyphs.get(c.column));
            }
            if let Some(click) = click.filter(|p| half.contains(*p)) {
                let column = column_at(glyphs, click.x - pos.x);
                let line = line.index;
                clicked = Some((side, Position { line, column }));
            }
        }
        painter.vline(left.right(), rect.y_range(), Stroke::new(1.0, DIVIDER));
        clicked
    }

    /// Paints the cursor as a block over `glyph`, a character of a line in
    /// `half` drawn from `pos`, or over the line's start when it has none.
    fn paint_cursor(&self, painter: &Painter, half: Rect, pos: Pos2, glyph: Option<&Glyph>) {
        let (x, width) = glyph.map_or((0.0, self.char_width), |g| (g.pos.x, g.advance_width));
        let block = Rect::from_x_y_ranges(pos.x + x..=pos.x + x + width, half.y_range());
        let text_rect = Rect::from_min_max(pos2(pos.x, half.top()), half.max);
        let painter = painter.with_clip_rect(text_rect);
        painter.rect_filled(block, 0.0, CURSOR_BG);
        if let Some(glyph) = glyph {
            let (align, font) = (Align2::LEFT_CENTER, self.font.clone());
            painter.text(block.left_center(), align, glyph.chr, font, CURSOR_FG);
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
            GUTTER_FG,
        );

        let text_rect = Rect::from_min_max(pos2(gutter_right, half.top()), half.max);
        let galley = ui.fonts_mut(|f| f.layout_job(self.line_job(line, text, spans)));
        let pos = pos2(gutter_right, mid_y - galley.size().y / 2.0);
        painter
            .with_clip_rect(text_rect)
            .galley(pos, galley.clone(), TEXT_FG);
        (pos, galley)
    }

    /// Lays out `line`'s `text` in the colors of `spans`, with its changed
    /// words on [`CHANGED_WORD_BG`].
    fn line_job(&self, line: &Line, text: &str, spans: &[Span]) -> LayoutJob {
        let color_starts = spans.iter().map(|span| {
            let [r, g, b] = span.rgb;
            (span.range.start, Color32::from_rgb(r, g, b))
        });
        let background_starts = (line.changed_words.iter())
            .flat_map(|r| [(r.start, CHANGED_WORD_BG), (r.end, Color32::TRANSPARENT)]);
        let mut job = LayoutJob::default();
        let (mut start, mut color, mut background) = (0, TEXT_FG, Color32::TRANSPARENT);
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

/// Syntax colors of one side's lines, by line index.
#[derive(Default)]
struct SideColors {
    lines: Vec<Vec<Span>>,
}

impl SideColors {
    /// The colors of `line`; empty until highlighted.
    fn line(&self, line: &Line) -> &[Span] {
        self.lines.get(line.index).map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::{Harness, kittest::Queryable as _};

    /// Three separate one-line changes with unchanged lines between them.
    fn three_changes() -> TextView {
        let left: String = (0..30).map(|i| format!("line {i}\n")).collect();
        let right = left
            .replace("line 5\n", "line five\n")
            .replace("line 15\n", "line fifteen\n")
            .replace("line 25\n", "line twenty-five\n");
        TextView::new(
            SideBySide::new([Buffer::new(&left), Buffer::new(&right)]),
            "left".into(),
            "right".into(),
            Instant::now(),
        )
    }

    #[test]
    fn keys_step_through_changes_and_stop_at_the_ends() {
        let mut harness = Harness::new_ui_state(|ui, view| view.show(ui), three_changes());
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
        let text: String = (0..200).map(|i| format!("line {i}\n")).collect();
        let diff = SideBySide::new([Buffer::new(&text), Buffer::new(&text)]);
        let view = TextView::new(diff, "left".into(), "right".into(), Instant::now());
        let mut harness = Harness::new_ui_state(|ui, view| view.show(ui), view);
        harness.run();
        for key in keys.split_whitespace() {
            let (modifiers, key) = key_press(key);
            harness.key_press_modifiers(modifiers, key);
            harness.run();
        }
        harness
    }

    /// The modifiers and key of one key in [`typed_in_unchanged_file`]'s
    /// notation.
    fn key_press(key: &str) -> (Modifiers, Key) {
        let (ctrl, name) = match key.strip_prefix("C-") {
            Some(name) => (Modifiers::CTRL, name),
            None => (Modifiers::NONE, key),
        };
        let shift = if name.len() == 1 && name != name.to_lowercase() {
            Modifiers::SHIFT
        } else {
            Modifiers::NONE
        };
        let key = Key::from_name(&name.to_uppercase()).or(Key::from_name(name));
        (ctrl | shift, key.unwrap())
    }

    #[yare::parameterized(
        l_moves_right = { "l l l", 1, 0, 3 },
        h_moves_left = { "l l l h", 1, 0, 2 },
        j_moves_down = { "j j j", 1, 3, 0 },
        k_moves_up = { "j j j k", 1, 2, 0 },
        arrows_move = { "Down Down Right Up Right Right Left", 1, 1, 2 },
        gg_moves_to_the_first_line = { "j j g g", 1, 0, 0 },
        shift_g_moves_to_the_last_line = { "G", 1, 199, 0 },
        ctrl_w_h_moves_to_the_left_side = { "j l C-w h", 0, 1, 1 },
        ctrl_w_ctrl_h_moves_to_the_left_side = { "C-w C-h", 0, 0, 0 },
        ctrl_w_l_moves_to_the_right_side = { "C-w h j C-w l", 1, 1, 0 },
        another_key_cancels_a_sequence_and_is_dropped = { "j j g j g", 1, 2, 0 },
        a_new_sequence_starts_after_a_cancelled_one = { "j g j g g", 1, 0, 0 },
    )]
    fn keys_move_the_cursor(keys: &str, side: usize, line: usize, column: usize) {
        let harness = typed_in_unchanged_file(keys);
        let cursor = harness.state().cursor_position();
        assert_eq!(cursor, Some((side, Position { line, column })));
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
    fn stepping_to_a_change_moves_the_cursor_to_it() {
        let mut harness = Harness::new_ui_state(|ui, view| view.show(ui), three_changes());
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
        let view = TextView::new(
            SideBySide::new(buffers),
            "left".into(),
            "right".into(),
            Instant::now(),
        );
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

        let y = map_y(map, diff.rows.len(), rows_in_view);
        let marks = super::change_marks(&diff.rows, map, y);

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

        let marks = super::change_marks(&diff.rows, map, map_y(map, diff.rows.len(), 4.0));

        let mark = Rect::from_x_y_ranges(x_range, 25.0..=75.0);
        assert_eq!(marks, vec![(mark, ONE_SIDED_MARK)]);
    }

    #[test]
    fn shift_zz_closes_the_window() {
        let mut harness = Harness::new_ui_state(|ui, view| view.show(ui), three_changes());
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
}
