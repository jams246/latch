#[cfg(test)]
mod tests;
mod values;

use gpui::{App, Global};

pub use crate::storage::TerminalTheme as TerminalPalette;

impl Global for TerminalPalette {}

impl TerminalPalette {
    /// All available terminal presets.
    pub const ALL: [Self; 5] = [
        Self::Classic,
        Self::Carbon,
        Self::Glacier,
        Self::Ember,
        Self::Midnight,
    ];

    /// Display name for the preset.
    pub fn name(self) -> &'static str {
        match self {
            Self::Classic => "Classic",
            Self::Carbon => "Carbon",
            Self::Glacier => "Glacier",
            Self::Ember => "Ember",
            Self::Midnight => "Midnight",
        }
    }

    /// Current application-wide terminal preset, or Classic before selection.
    pub fn current(cx: &App) -> Self {
        cx.try_global::<Self>().copied().unwrap_or_default()
    }

    /// Color values for this preset.
    pub fn colors(self) -> &'static Palette {
        match self {
            Self::Classic => &values::CLASSIC,
            Self::Carbon => &values::CARBON,
            Self::Glacier => &values::GLACIER,
            Self::Ember => &values::EMBER,
            Self::Midnight => &values::MIDNIGHT,
        }
    }
}

/// Terminal color roles, each encoded as 24-bit RGB.
#[derive(Clone, Copy)]
pub struct Palette {
    /// Standard ANSI colors followed by their eight bright variants.
    pub ansi: [u32; 16],
    /// Default text color, also used for bright default text.
    pub foreground: u32,
    /// Default terminal canvas and cell background.
    pub background: u32,
    /// Cursor color before any remote override.
    pub cursor: u32,
    /// Selected cell background.
    pub selection: u32,
    /// Default text color for the dim named foreground.
    pub dim_foreground: u32,
}
