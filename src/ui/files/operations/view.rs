use super::{super::Files, display_size, remaining_label};
use crate::ui::{button, theme::CARBON};
use gpui::{prelude::*, *};
use std::time::Instant;

impl Files {
    pub(in crate::ui) fn operations_bar(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let bar = div()
            .id("operations-bar")
            .debug_selector(|| "operations-bar".into());
        let Some(transfer) = &self.transfer else {
            return bar;
        };
        let paused = transfer.is_paused();
        let now = Instant::now();
        let (name, size, speed, remaining, fraction) = self.metrics.as_ref().map_or_else(
            || {
                (
                    "Preparing transfer…".into(),
                    "0 B / --".into(),
                    "0 B/s".into(),
                    "-- remaining".into(),
                    0.,
                )
            },
            |metrics| {
                (
                    if metrics.name.is_empty() {
                        "Preparing transfer…".into()
                    } else {
                        metrics.name.clone()
                    },
                    metrics.size_label(),
                    format!("{}/s", display_size(metrics.speed(now) as u64)),
                    remaining_label(metrics.remaining(now)),
                    metrics.fraction(),
                )
            },
        );
        bar.h(px(48.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_3()
            .text_xs()
            .text_color(rgb(CARBON.muted))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("transfer-filename")
                                    .debug_selector(|| "transfer-filename".into())
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(rgb(CARBON.text))
                                    .child(name),
                            )
                            .child("|")
                            .child(
                                div()
                                    .id("transfer-size")
                                    .debug_selector(|| "transfer-size".into())
                                    .flex_shrink_0()
                                    .child(size),
                            )
                            .child("|")
                            .child(
                                div()
                                    .id("transfer-speed")
                                    .debug_selector(|| "transfer-speed".into())
                                    .flex_shrink_0()
                                    .child(speed),
                            )
                            .child("|")
                            .child(
                                div()
                                    .id("transfer-remaining")
                                    .debug_selector(|| "transfer-remaining".into())
                                    .flex_shrink_0()
                                    .child(if paused { "Paused".into() } else { remaining }),
                            ),
                    )
                    .child(
                        div()
                            .id("transfer-progress")
                            .debug_selector(|| "transfer-progress".into())
                            .w_full()
                            .h(px(4.))
                            .rounded_sm()
                            .overflow_hidden()
                            .bg(rgb(CARBON.control))
                            .child(div().w(relative(fraction)).h_full().bg(rgb(CARBON.accent))),
                    ),
            )
            .child(
                button("pause-transfer", if paused { "Resume" } else { "Pause" })
                    .debug_selector(|| "pause-transfer".into())
                    .flex_shrink_0()
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(transfer) = &this.transfer {
                            if transfer.is_paused() {
                                transfer.resume();
                            } else {
                                transfer.pause();
                            }
                            if let Some(metrics) = &mut this.metrics {
                                metrics.set_paused(transfer.is_paused(), Instant::now());
                            }
                            cx.notify();
                        }
                    })),
            )
            .child(
                button("cancel-transfer", "Cancel")
                    .debug_selector(|| "cancel-transfer".into())
                    .flex_shrink_0()
                    .on_click(cx.listener(|this, _, _, _| {
                        if let Some(transfer) = &this.transfer {
                            transfer.cancel();
                        }
                    })),
            )
    }
}
