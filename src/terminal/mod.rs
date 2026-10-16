//! Independent Alacritty terminal state with native GPUI display and input.

mod colors;
mod input;
mod input_handler;
mod paint;
mod palette;
mod state;
mod view;

pub use palette::{Palette, TerminalPalette};
pub use state::TerminalState;
pub use view::{TerminalInput, TerminalView};
