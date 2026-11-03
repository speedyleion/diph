//! Side-by-side text diff view.

use diph_core::{Line, Row, RowKind, SideBySide};
use eframe::egui::{
    self, Align2, Color32, FontId, Painter, Rect, ScrollArea, Sense, Stroke, TextFormat, TextStyle,
    pos2, text::LayoutJob, vec2,
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

pub struct TextView {
    diff: SideBySide,
    left_label: String,
    right_label: String,
}

impl TextView {
    pub fn new(diff: SideBySide, left_label: String, right_label: String) -> Self {
        Self {
            diff,
            left_label,
            right_label,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("text_header").show(ui, |ui| {
            ui.columns(2, |cols| {
                cols[0].monospace(&self.left_label);
                cols[1].monospace(&self.right_label);
            });
        });
        egui::CentralPanel::default().show(ui, |ui| self.rows_ui(ui));
    }

    /// Both halves live in one scroll area so they always scroll together.
    fn rows_ui(&self, ui: &mut egui::Ui) {
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
        let diff = &self.diff;
        ScrollArea::vertical().auto_shrink(false).show_rows(
            ui,
            layout.row_height,
            diff.rows.len(),
            |ui, range| {
                for row in &diff.rows[range] {
                    layout.paint_row(ui, row);
                }
            },
        );
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
    fn paint_row(&self, ui: &mut egui::Ui, row: &Row) {
        let size = vec2(ui.available_width(), self.row_height);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let painter = ui.painter();
        let (left, right) = rect.split_left_right_at_x(rect.left() + self.half_width);
        let background = background(row);
        self.paint_half(ui, painter, left, row.left.as_ref(), background);
        self.paint_half(ui, painter, right, row.right.as_ref(), background);
        painter.vline(left.right(), rect.y_range(), Stroke::new(1.0, DIVIDER));
    }

    /// Paints one half of a row: the background, the line number, and the
    /// line text clipped to the half, with its changed words highlighted. A
    /// half without a line gets the filler.
    fn paint_half(
        &self,
        ui: &egui::Ui,
        painter: &Painter,
        half: Rect,
        line: Option<&Line>,
        background: Option<Color32>,
    ) {
        let Some(line) = line else {
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
        let galley = ui.fonts_mut(|f| f.layout_job(self.line_job(line)));
        let pos = pos2(gutter_right, mid_y - galley.size().y / 2.0);
        painter
            .with_clip_rect(text_rect)
            .galley(pos, galley, TEXT_FG);
    }

    /// Lays out a line with its changed words on [`CHANGED_WORD_BG`].
    fn line_job(&self, line: &Line) -> LayoutJob {
        let mut job = LayoutJob::default();
        let mut pos = 0;
        for range in &line.changed_words {
            let unchanged = &line.text[pos..range.start];
            job.append(unchanged, 0.0, self.format(Color32::TRANSPARENT));
            job.append(&line.text[range.clone()], 0.0, self.format(CHANGED_WORD_BG));
            pos = range.end;
        }
        job.append(&line.text[pos..], 0.0, self.format(Color32::TRANSPARENT));
        job
    }

    fn format(&self, background: Color32) -> TextFormat {
        TextFormat {
            font_id: self.font.clone(),
            color: TEXT_FG,
            background,
            ..Default::default()
        }
    }
}
