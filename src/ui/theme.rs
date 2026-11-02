#[cfg(test)]
mod tests;

use gpui::WindowAppearance;
use latch::storage::AppTheme;
use std::cell::Cell;

// All GPUI windows and their paint callbacks share the profile on the UI thread.
thread_local! { static ACTIVE: Cell<Palette> = const { Cell::new(GRAPHITE) }; }

pub(crate) fn current() -> Palette {
    ACTIVE.get()
}

pub(super) fn apply(theme: AppTheme, appearance: WindowAppearance) {
    ACTIVE.set(palette(theme, appearance));
}

pub(super) fn palette(theme: AppTheme, appearance: WindowAppearance) -> Palette {
    let dark = matches!(
        appearance,
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    );
    match (theme, dark) {
        (AppTheme::Graphite, true) => GRAPHITE,
        (AppTheme::Graphite, false) => PAPER,
        (AppTheme::Citron, true) => CITRON,
        (AppTheme::Citron, false) => CITRON_LIGHT,
    }
}

const GRAPHITE: Palette = Palette {
    background: 0x191a1b,
    panel: 0x202123,
    surface: 0x242628,
    control: 0x303236,
    hover: 0x303236,
    selected: 0x343639,
    border: 0x414346,
    text: 0xf1f1f1,
    muted: 0xb4b6bb,
    accent: 0xe1e3e6,
    danger: 0xffb4ab,
    overlay: 0x050608aa,
};
const PAPER: Palette = Palette {
    background: 0xfafafa,
    panel: 0xf1f1f1,
    surface: 0xeeeeef,
    control: 0xe4e6e8,
    hover: 0xeaecee,
    selected: 0xdfe2e4,
    border: 0xcbcdcf,
    text: 0x202123,
    muted: 0x505459,
    accent: 0x25282b,
    danger: 0x8d2523,
    overlay: 0x14161966,
};
const CITRON: Palette = Palette {
    background: 0x1b1c19,
    panel: 0x252622,
    surface: 0x292b24,
    control: 0x303327,
    hover: 0x333b21,
    selected: 0x353d22,
    border: 0x464a3d,
    text: 0xf2f3e7,
    muted: 0xb9bea9,
    accent: 0xd4ef57,
    ..GRAPHITE
};
const CITRON_LIGHT: Palette = Palette {
    background: 0xfbfbf6,
    panel: 0xf0f1e8,
    surface: 0xeef0e4,
    control: 0xe7e9dd,
    hover: 0xeaefd0,
    selected: 0xe5ecbe,
    border: 0xcacfbc,
    text: 0x26281d,
    muted: 0x505544,
    accent: 0x4a5a00,
    ..PAPER
};

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub(crate) background: u32,
    pub(crate) panel: u32,
    pub(crate) surface: u32,
    pub(crate) control: u32,
    pub(crate) hover: u32,
    pub(crate) selected: u32,
    pub(crate) border: u32,
    pub(crate) text: u32,
    pub(crate) muted: u32,
    pub(crate) accent: u32,
    pub(crate) danger: u32,
    pub(crate) overlay: u32,
}
