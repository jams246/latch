use alacritty_terminal::{term::color::Colors, vte::ansi::Color};

const ANSI: [u32; 16] = [
    0x20242d, 0xe06c75, 0x98c379, 0xe5c07b, 0x61afef, 0xc678dd, 0x56b6c2, 0xabb2bf, 0x5c6370,
    0xff7a85, 0xb5e890, 0xffd98f, 0x81c8ff, 0xe49bff, 0x76d6e2, 0xffffff,
];

pub(super) fn resolve(color: Color, colors: &Colors) -> u32 {
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
        0..=15 => ANSI[index],
        16..=231 => {
            let n = index - 16;
            let channel = |value| if value == 0 { 0 } else { 55 + value * 40 };
            ((channel(n / 36) << 16) | (channel(n / 6 % 6) << 8) | channel(n % 6)) as u32
        }
        232..=255 => (8 + (index as u32 - 232) * 10) * 0x010101,
        257 => 0x151820,
        259..=266 => (ANSI[index - 259] & 0xfefefe) >> 1,
        268 => 0x858b98,
        _ => 0xd7dce5,
    }
}
