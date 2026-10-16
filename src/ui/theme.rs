#[cfg(test)]
mod tests;

pub(crate) const CARBON: Palette = Palette {
    background: 0x0d1012,
    panel: 0x151a1d,
    surface: 0x1c2327,
    control: 0x252e33,
    hover: 0x303c42,
    selected: 0x24444c,
    border: 0x46575f,
    text: 0xe1ebee,
    muted: 0xacbdc3,
    accent: 0x79d8eb,
    danger: 0xffb4ab,
    overlay: 0x05080baa,
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
