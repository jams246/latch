use super::CARBON;

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

fn contrast(foreground: u32, background: u32) -> f64 {
    let foreground = luminance(foreground);
    let background = luminance(background);
    (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05)
}

#[test]
fn carbon_text_meets_normal_text_contrast_on_every_surface() {
    for (role, foreground) in [
        ("text", CARBON.text),
        ("muted", CARBON.muted),
        ("accent", CARBON.accent),
        ("danger", CARBON.danger),
    ] {
        for background in [
            CARBON.background,
            CARBON.panel,
            CARBON.surface,
            CARBON.control,
            CARBON.hover,
            CARBON.selected,
        ] {
            let ratio = contrast(foreground, background);
            assert!(
                ratio >= 4.5,
                "Carbon {role} on #{background:06x}: {ratio:.2}:1"
            );
        }
    }
}
