//! Side-by-side text diff view.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use diph_core::{Highlighter, Line, Row, RowKind, SideBySide, Span};
use eframe::egui::{
    self, Align2, Color32, FontId, InputState, Key, Painter, Rect, ScrollArea, Sense, Stroke,
    TextFormat, TextStyle, pos2, text::LayoutJob, vec2,
};
use itertools::{
    EitherOrBoth::{Both, Left, Right},
    Itertools as _,
};

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

pub struct TextView {
    diff: SideBySide,
    left_label: String,
    right_label: String,
    /// Index into `diff.changes` of the change last jumped to.
    current_change: Option<usize>,
    /// Row to scroll to on the next frame.
    scroll_to_row: Option<usize>,
    /// Syntax colors of the left and right side, filled in from the top as
    /// the background highlighter delivers them.
    colors: [SideColors; 2],
    colors_rx: Option<Receiver<ColorsChunk>>,
}

impl TextView {
    pub fn new(diff: SideBySide, left_label: String, right_label: String) -> Self {
        let mut view = Self {
            diff,
            left_label,
            right_label,
            current_change: None,
            scroll_to_row: None,
            colors: Default::default(),
            colors_rx: None,
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
        let rows = &self.diff.rows;
        let sides = [
            side_texts(rows.iter().filter_map(|r| r.left.as_ref())),
            side_texts(rows.iter().filter_map(|r| r.right.as_ref())),
        ];
        std::thread::spawn(move || highlight_sides(&paths, &sides, &tx, &ctx));
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        if let Some(rx) = &self.colors_rx {
            for chunk in rx.try_iter() {
                self.colors[chunk.side].lines.extend(chunk.lines);
            }
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let step = ui.input(change_step);
        if step != 0 {
            self.step_change(step);
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
        egui::CentralPanel::default().show(ui, |ui| self.rows_ui(ui));
    }

    /// Moves `delta` changes forward or back, stopping at the first and last.
    fn step_change(&mut self, delta: isize) {
        let Some(last) = self.diff.changes.len().checked_sub(1) else {
            return;
        };
        let next = match self.current_change {
            None => 0,
            Some(c) => c.saturating_add_signed(delta).min(last),
        };
        self.current_change = Some(next);
        self.scroll_to_row = Some(self.diff.changes[next]);
    }

    /// Both halves live in one scroll area so they always scroll together.
    fn rows_ui(&mut self, ui: &mut egui::Ui) {
        let font = TextStyle::Monospace.resolve(ui.style());
        let char_width = ui.fonts_mut(|f| f.glyph_width(&font, '0'));
        let rows = &self.diff.rows;
        let sides = rows.iter().flat_map(|r| [&r.left, &r.right]);
        let last_number = sides.flatten().map(|l| l.index + 1).max().unwrap_or(1);
        let digits = last_number.ilog10() as f32 + 1.0;
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
        if let Some(row) = self.scroll_to_row.take() {
            // Clamped so changes near the end don't overscroll and bounce back.
            let row_height = layout.row_height;
            let max_offset = rows.len() as f32 * row_height - ui.available_height();
            let offset = row.saturating_sub(CONTEXT_ROWS) as f32 * row_height;
            area = area.vertical_scroll_offset(offset.min(max_offset).max(0.0));
        }
        area.show_rows(ui, layout.row_height, rows.len(), |ui, range| {
            for row in &rows[range] {
                layout.paint_row(ui, &self.colors, row);
            }
        });
    }
}

/// The texts of one side's lines, in order.
fn side_texts<'a>(lines: impl Iterator<Item = &'a Line>) -> Vec<String> {
    lines.map(|l| l.text.clone()).collect()
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

/// The change step the pressed keys ask for: -1 for p or Shift+F7, 1 for n
/// or F7, otherwise 0.
fn change_step(input: &InputState) -> isize {
    let f7 = input.key_pressed(Key::F7);
    if input.key_pressed(Key::P) || (f7 && input.modifiers.shift) {
        -1
    } else if input.key_pressed(Key::N) || f7 {
        1
    } else {
        0
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

/// Where the parts of a row go.
struct RowLayout {
    font: FontId,
    char_width: f32,
    gutter_width: f32,
    row_height: f32,
    half_width: f32,
}

impl RowLayout {
    /// Paints `row` across both halves, with a divider between them.
    fn paint_row(&self, ui: &mut egui::Ui, colors: &[SideColors; 2], row: &Row) {
        let size = vec2(ui.available_width(), self.row_height);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let painter = ui.painter();
        let (left, right) = rect.split_left_right_at_x(rect.left() + self.half_width);
        let background = background(row);
        let left_line = row.left.as_ref().map(|l| (l, colors[0].line(l)));
        let right_line = row.right.as_ref().map(|l| (l, colors[1].line(l)));
        self.paint_half(ui, painter, left, left_line, background);
        self.paint_half(ui, painter, right, right_line, background);
        painter.vline(left.right(), rect.y_range(), Stroke::new(1.0, DIVIDER));
    }

    /// Paints one half of a row: the background, the line number, and the
    /// line text in the colors of its spans, clipped to the half, with its
    /// changed words highlighted. A half without a line gets the filler.
    fn paint_half(
        &self,
        ui: &egui::Ui,
        painter: &Painter,
        half: Rect,
        line: Option<(&Line, &[Span])>,
        background: Option<Color32>,
    ) {
        let Some((line, spans)) = line else {
            painter.rect_filled(half, 0.0, FILLER_BG);
            return;
        };
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
        let galley = ui.fonts_mut(|f| f.layout_job(self.line_job(line, spans)));
        let pos = pos2(gutter_right, mid_y - galley.size().y / 2.0);
        painter
            .with_clip_rect(text_rect)
            .galley(pos, galley, TEXT_FG);
    }

    /// Lays out `line` in the colors of `spans`, with its changed words on
    /// [`CHANGED_WORD_BG`].
    fn line_job(&self, line: &Line, spans: &[Span]) -> LayoutJob {
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
            self.append_piece(&mut job, &line.text[start..end], color, background);
            start = end;
            match event {
                Left((_, c)) => color = c,
                Right((_, b)) => background = b,
                Both((_, c), (_, b)) => (color, background) = (c, b),
            }
        }
        self.append_piece(&mut job, &line.text[start..], color, background);
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
            SideBySide::new(&left, &right),
            "left".into(),
            "right".into(),
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

    #[test]
    fn escape_closes_the_window() {
        let mut harness = Harness::new_ui_state(|ui, view| view.show(ui), three_changes());
        harness.run();
        // Only the key down, so the next frame's output is the one that
        // handles it.
        harness.key_down(Key::Escape);
        harness.step();
        let commands = &harness.output().viewport_output[&egui::ViewportId::ROOT].commands;
        assert!(commands.contains(&egui::ViewportCommand::Close));
    }
}
