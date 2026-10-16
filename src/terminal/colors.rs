use alacritty_terminal::{term::color::Colors, vte::ansi::Color};

use super::palette::Palette;

#[cfg(test)]
mod tests;

pub(super) fn resolve(color: Color, colors: &Colors, palette: &Palette) -> u32 {
    let index = match color {
        Color::Spec(rgb) => {
            return (u32::from(rgb.r) << 16) | (u32::from(rgb.g) << 8) | u32::from(rgb.b);
        }
        Color::Indexed(index) => usize::from(index),
        Color::Named(named) => named as usize,
    };
    if let Some(rgb) = colors[index] {
        return (u32::from(rgb.r) << 16) | (u32::from(rgb.g) << 8) | u32::from(rgb.b);
    }
    match index {
        0..=15 => palette.ansi[index],
        16..=231 => {
            let n = index - 16;
            let channel = |value| if value == 0 { 0 } else { 55 + value * 40 };
            ((channel(n / 36) << 16) | (channel(n / 6 % 6) << 8) | channel(n % 6)) as u32
        }
        232..=255 => (8 + (index as u32 - 232) * 10) * 0x010101,
        257 => palette.background,
        258 => palette.cursor,
        259..=266 => (palette.ansi[index - 259] & 0xfefefe) >> 1,
        268 => palette.dim_foreground,
        _ => palette.foreground,
    }
}
