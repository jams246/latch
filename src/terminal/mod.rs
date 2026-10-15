//! Independent Alacritty terminal state with native GPUI display and input.

mod colors;
mod input;
mod input_handler;
mod paint;
mod state;
mod view;

pub use state::TerminalState;
pub use view::{TerminalInput, TerminalView};
