use crate::ui::Latch;
use gpui::{AppContext, Context, Entity};
use latch::terminal::{TerminalState, TerminalView};
use std::sync::Arc;

pub(super) fn new(cx: &mut Context<Latch>) -> Entity<TerminalView> {
    let state = TerminalState::new(48, 10);
    state.feed(concat!(
        "\x1b[1;32muser@server\x1b[0m:\x1b[1;34m~/project\x1b[0m$ git status\r\n",
        "On branch \x1b[36mmain\x1b[0m\r\n",
        "\x1b[32m  modified:   src/main.rs\x1b[0m\r\n",
        "\x1b[33mwarning:\x1b[0m local preview only\r\n",
        "\x1b[31merror:\x1b[0m sample diagnostic\r\n",
        "\x1b[35mnotes\x1b[0m  \x1b[36minfo\x1b[0m  \x1b[1mbright text\x1b[0m\r\n",
        "\x1b[41m   \x1b[42m   \x1b[43m   \x1b[44m   \x1b[45m   \x1b[46m   \x1b[47m   \x1b[0m\r\n",
        "\x1b[1;32muser@server\x1b[0m:\x1b[1;34m~/project\x1b[0m$ "
    ).as_bytes());
    cx.new(|cx| TerminalView::new(state, Arc::new(|_| {}), cx))
}
