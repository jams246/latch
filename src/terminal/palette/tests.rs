use super::TerminalPalette;

fn luminance(color: u32) -> f64 {
    let [red, green, blue] = [16, 8, 0].map(|shift| {
        let channel = f64::from((color >> shift) & 0xff) / 255.;
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    });
    red * 0.2126 + green * 0.7152 + blue * 0.0722
}

fn contrast(first: u32, second: u32) -> f64 {
    let first = luminance(first);
    let second = luminance(second);
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

#[test]
fn classic_preserves_original_terminal_colors() {
    let palette = TerminalPalette::default().colors();
    assert_eq!(TerminalPalette::default(), TerminalPalette::Classic);
    assert_eq!(
        palette.ansi,
        [
            0x20242d, 0xe06c75, 0x98c379, 0xe5c07b, 0x61afef, 0xc678dd, 0x56b6c2, 0xabb2bf,
            0x5c6370, 0xff7a85, 0xb5e890, 0xffd98f, 0x81c8ff, 0xe49bff, 0x76d6e2, 0xffffff,
        ]
    );
    assert_eq!(
        [
            palette.foreground,
            palette.background,
            palette.cursor,
            palette.selection,
            palette.dim_foreground,
        ],
        [0xd7dce5, 0x151820, 0xd7dce5, 0x365578, 0x858b98]
    );
}

#[test]
fn palettes_keep_default_text_and_cursor_readable() {
    for choice in TerminalPalette::ALL {
        let palette = choice.colors();
        for (role, foreground, background) in [
            ("foreground", palette.foreground, palette.background),
            ("selection text", palette.foreground, palette.selection),
            ("dim foreground", palette.dim_foreground, palette.background),
            ("cursor", palette.cursor, palette.background),
        ] {
            let ratio = contrast(foreground, background);
            assert!(ratio >= 4.5, "{choice:?} {role}: {ratio:.2}:1");
        }
        assert!(
            contrast(palette.selection, palette.background) >= 1.5,
            "{choice:?} selection must remain distinct from the canvas"
        );
    }
}

#[test]
fn palette_values_fit_24_bit_rgb() {
    for choice in TerminalPalette::ALL {
        let palette = choice.colors();
        for color in palette.ansi.into_iter().chain([
            palette.foreground,
            palette.background,
            palette.cursor,
            palette.selection,
            palette.dim_foreground,
        ]) {
            assert!(color <= 0xffffff, "{choice:?}: #{color:x}");
        }
    }
}
