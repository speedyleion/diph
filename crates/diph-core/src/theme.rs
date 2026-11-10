//! Color themes.

use two_face::theme::{EmbeddedLazyThemeSet, EmbeddedThemeName};

/// The colors of syntax highlighting and of the text and background around
/// it.
pub struct Theme {
    name: EmbeddedThemeName,
    pub(crate) syntect: syntect::highlighting::Theme,
}

impl Default for Theme {
    fn default() -> Self {
        Self::load(EmbeddedThemeName::Base16OceanDark)
    }
}

impl Theme {
    /// The names of the bundled themes, in the order to list them.
    pub fn names() -> impl Iterator<Item = &'static str> {
        themes().map(EmbeddedThemeName::as_name)
    }

    /// The bundled theme called `name`, one of [`Self::names`].
    pub fn named(name: &str) -> Option<Self> {
        themes().find(|t| t.as_name() == name).map(Self::load)
    }

    fn load(name: EmbeddedThemeName) -> Self {
        Self {
            name,
            syntect: two_face::theme::extra().get(name).clone(),
        }
    }

    pub fn name(&self) -> &'static str {
        self.name.as_name()
    }

    /// The color of text that has no syntax color.
    pub fn foreground(&self) -> [u8; 3] {
        let color = self.syntect.settings.foreground;
        color.map_or([0xff; 3], |c| [c.r, c.g, c.b])
    }

    pub fn background(&self) -> [u8; 3] {
        let color = self.syntect.settings.background;
        color.map_or([0; 3], |c| [c.r, c.g, c.b])
    }

    /// The color of the cursor, if the theme gives one.
    pub fn caret(&self) -> Option<[u8; 3]> {
        let color = self.syntect.settings.caret;
        color.map(|c| [c.r, c.g, c.b])
    }

    /// The color of line numbers, if the theme gives one.
    pub fn gutter_foreground(&self) -> Option<[u8; 3]> {
        let color = self.syntect.settings.gutter_foreground;
        color.map(|c| [c.r, c.g, c.b])
    }
}

/// The bundled themes, without the ones that hold terminal palette indices
/// in place of colors.
fn themes() -> impl Iterator<Item = EmbeddedThemeName> {
    let terminal = [
        EmbeddedThemeName::Ansi,
        EmbeddedThemeName::Base16,
        EmbeddedThemeName::Base16_256,
    ];
    let names = EmbeddedLazyThemeSet::theme_names().iter().copied();
    names.filter(move |name| !terminal.contains(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[yare::parameterized(
        a_bundled_theme = { "Nord", true },
        a_terminal_palette_theme = { "ansi", false },
        an_unknown_name = { "no such theme", false },
    )]
    fn named(name: &str, found: bool) {
        assert_eq!(Theme::named(name).is_some(), found);
    }
}
