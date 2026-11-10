//! The colors of the view, from its color theme.

use diph_core::Theme;
use eframe::egui::{self, Color32};

/// Change map mark of one-sided lines, and the tint of their background.
pub const ONE_SIDED_MARK: Color32 = Color32::from_rgb(0x2f, 0x8f, 0x55);
/// Change map mark of changed lines, and the tint of their background.
pub const CHANGED_MARK: Color32 = Color32::from_rgb(0x3d, 0x6f, 0xc4);

/// The colors the view paints with.
#[derive(Clone, Copy)]
pub struct Palette {
    pub text: Color32,
    /// Background of equal lines and of the panels around them.
    pub background: Color32,
    pub line_number: Color32,
    pub divider: Color32,
    pub cursor: Color32,
    /// Color of the character under the cursor.
    pub cursor_text: Color32,
    /// Background opposite a one-sided line.
    pub filler: Color32,
    /// Background of a line with nothing opposite it.
    pub one_sided: Color32,
    /// Background of a line that differs from the line opposite it.
    pub changed: Color32,
    /// Background of the words that differ within a changed line.
    pub changed_word: Color32,
    /// Change map cover over the rows in view.
    pub in_view_cover: Color32,
}

impl Palette {
    /// The colors of `theme`, with the diff's backgrounds tinted onto its
    /// background.
    pub fn new(theme: &Theme) -> Self {
        let rgb = |[r, g, b]: [u8; 3]| Color32::from_rgb(r, g, b);
        let text = rgb(theme.foreground());
        let background = rgb(theme.background());
        let [r, g, b] = theme.foreground();
        Self {
            text,
            background,
            line_number: theme
                .gutter_foreground()
                .map_or(background.lerp_to_gamma(text, 0.45), rgb),
            divider: background.lerp_to_gamma(text, 0.2),
            cursor: theme.caret().map_or(text, rgb),
            cursor_text: background,
            filler: background.lerp_to_gamma(text, 0.06),
            one_sided: background.lerp_to_gamma(ONE_SIDED_MARK, 0.3),
            changed: background.lerp_to_gamma(CHANGED_MARK, 0.3),
            changed_word: background.lerp_to_gamma(CHANGED_MARK, 0.6),
            in_view_cover: Color32::from_rgba_unmultiplied(r, g, b, 24),
        }
    }

    /// Makes egui draw its panels, menus, and text in these colors.
    pub fn apply(&self, ctx: &egui::Context) {
        let (theme, mut visuals) = if self.background.intensity() < 0.5 {
            (egui::Theme::Dark, egui::Visuals::dark())
        } else {
            (egui::Theme::Light, egui::Visuals::light())
        };
        visuals.panel_fill = self.background;
        visuals.window_fill = self.background;
        visuals.override_text_color = Some(self.text);
        ctx.set_theme(theme);
        ctx.set_visuals_of(theme, visuals);
    }
}
