use super::{Files, rows::values::display_size};
use crate::ui::button;
use gpui::{prelude::*, *};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

#[cfg(test)]
mod tests;

pub(super) struct Metrics {
    name: String,
    completed: u64,
    total: Option<u64>,
    samples: VecDeque<(Instant, u64)>,
}

impl Metrics {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            name: String::new(),
            completed: 0,
            total: None,
            samples: VecDeque::from([(now, 0)]),
        }
    }

    pub(super) fn update(
        &mut self,
        name: String,
        completed: u64,
        total: Option<u64>,
        now: Instant,
    ) {
        self.name = name;
        self.completed = completed;
        self.total = total;
        if self.samples.back().is_none_or(|(time, _)| {
            now.saturating_duration_since(*time) >= Duration::from_millis(250)
        }) {
            self.samples.push_back((now, completed));
        }
        while self.samples.len() > 1
            && self.samples.front().is_some_and(|(time, _)| {
                now.saturating_duration_since(*time) > Duration::from_secs(3)
            })
        {
            self.samples.pop_front();
        }
    }

    fn speed(&self, now: Instant) -> f64 {
        let Some((time, completed)) = self
            .samples
            .iter()
            .find(|(time, _)| now.saturating_duration_since(*time) <= Duration::from_secs(3))
        else {
            return 0.;
        };
        let seconds = now.saturating_duration_since(*time).as_secs_f64();
        if seconds == 0. {
            0.
        } else {
            self.completed.saturating_sub(*completed) as f64 / seconds
        }
    }

    fn remaining(&self, now: Instant) -> Option<u64> {
        let remaining = self.total?.saturating_sub(self.completed);
        if remaining == 0 {
            return Some(0);
        }
        let speed = self.speed(now);
        (speed > 0.).then(|| (remaining as f64 / speed).ceil() as u64)
    }

    fn fraction(&self) -> f32 {
        self.total.map_or(0., |total| {
            if total == 0 {
                1.
            } else {
                (self.completed as f64 / total as f64).clamp(0., 1.) as f32
            }
        })
    }

    fn size_label(&self) -> String {
        format!(
            "{} / {}",
            display_size(self.completed),
            self.total.map_or_else(|| "--".into(), display_size)
        )
    }
}

fn remaining_label(seconds: Option<u64>) -> String {
    match seconds {
        None => "-- remaining".into(),
        Some(seconds) if seconds < 60 => format!("{seconds}s remaining"),
        Some(seconds) if seconds < 3600 => format!("{}m {}s remaining", seconds / 60, seconds % 60),
        Some(seconds) => format!("{}h {}m remaining", seconds / 3600, seconds % 3600 / 60),
    }
}

impl Files {
    pub(in crate::ui) fn operations_bar(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let bar = div()
            .id("operations-bar")
            .debug_selector(|| "operations-bar".into())
            .h(px(48.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_3()
            .text_xs()
            .text_color(rgb(0xa8c1d3));
        if self.transfer.is_none() {
            return bar.child("Double-click folders or links to open. Dates in UTC.");
        }
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
        bar.child(
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
                                .text_color(rgb(0xe7f1fa))
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
                                .child(remaining),
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
                        .bg(rgb(0x263c4d))
                        .child(div().w(relative(fraction)).h_full().bg(rgb(0x7fd1b9))),
                ),
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
