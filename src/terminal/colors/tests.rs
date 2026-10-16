use super::resolve;
use crate::terminal::{TerminalState, palette::TerminalPalette};
use alacritty_terminal::{
    term::color::Colors,
    vte::ansi::{Color, NamedColor, Rgb},
};

#[test]
fn named_and_indexed_ansi_colors_follow_each_palette() {
    let named = [
        NamedColor::Black,
        NamedColor::Red,
        NamedColor::Green,
        NamedColor::Yellow,
        NamedColor::Blue,
        NamedColor::Magenta,
        NamedColor::Cyan,
        NamedColor::White,
        NamedColor::BrightBlack,
        NamedColor::BrightRed,
        NamedColor::BrightGreen,
        NamedColor::BrightYellow,
        NamedColor::BrightBlue,
        NamedColor::BrightMagenta,
        NamedColor::BrightCyan,
        NamedColor::BrightWhite,
    ];
    let colors = Colors::default();
    for choice in TerminalPalette::ALL {
        let palette = choice.colors();
        for (index, named) in named.into_iter().enumerate() {
            assert_eq!(
                resolve(Color::Named(named), &colors, palette),
                palette.ansi[index]
            );
            assert_eq!(
                resolve(Color::Indexed(index as u8), &colors, palette),
                palette.ansi[index]
            );
        }
    }
}

#[test]
fn named_default_colors_follow_each_palette() {
    let colors = Colors::default();
    for choice in TerminalPalette::ALL {
        let palette = choice.colors();
        for (named, expected) in [
            (NamedColor::Foreground, palette.foreground),
            (NamedColor::BrightForeground, palette.foreground),
            (NamedColor::Background, palette.background),
            (NamedColor::Cursor, palette.cursor),
            (NamedColor::DimForeground, palette.dim_foreground),
            (NamedColor::DimRed, (palette.ansi[1] & 0xfefefe) >> 1),
        ] {
            assert_eq!(resolve(Color::Named(named), &colors, palette), expected);
        }
    }
}

#[test]
fn explicit_truecolor_is_independent_of_palette() {
    let color = Color::Spec(Rgb {
        r: 0x12,
        g: 0x34,
        b: 0x56,
    });
    for choice in TerminalPalette::ALL {
        assert_eq!(
            resolve(color, &Colors::default(), choice.colors()),
            0x123456
        );
    }
}

#[test]
fn color_cube_and_grayscale_indices_remain_standard() {
    for choice in TerminalPalette::ALL {
        for (index, expected) in [
            (16, 0x000000),
            (21, 0x0000ff),
            (59, 0x5f5f5f),
            (231, 0xffffff),
            (232, 0x080808),
            (255, 0xeeeeee),
        ] {
            assert_eq!(
                resolve(Color::Indexed(index), &Colors::default(), choice.colors()),
                expected
            );
        }
    }
}

#[test]
fn remote_osc_overrides_survive_palette_changes_until_reset() {
    let terminal = TerminalState::new(10, 2);
    terminal.feed(b"\x1b]4;1;#123456\x07\x1b]10;#aabbcc\x07\x1b]11;#112233\x07\x1b]12;#778899\x07");
    for choice in TerminalPalette::ALL {
        for (color, expected) in [
            (Color::Indexed(1), 0x123456),
            (Color::Named(NamedColor::Foreground), 0xaabbcc),
            (Color::Named(NamedColor::Background), 0x112233),
            (Color::Named(NamedColor::Cursor), 0x778899),
        ] {
            assert_eq!(
                resolve(color, &terminal.snapshot().colors, choice.colors()),
                expected
            );
        }
    }
    terminal.feed(b"\x1b]104;1\x07\x1b]110\x07\x1b]111\x07\x1b]112\x07");
    for choice in TerminalPalette::ALL {
        let palette = choice.colors();
        for (color, expected) in [
            (Color::Indexed(1), palette.ansi[1]),
            (Color::Named(NamedColor::Foreground), palette.foreground),
            (Color::Named(NamedColor::Background), palette.background),
            (Color::Named(NamedColor::Cursor), palette.cursor),
        ] {
            assert_eq!(
                resolve(color, &terminal.snapshot().colors, palette),
                expected
            );
        }
    }
}
