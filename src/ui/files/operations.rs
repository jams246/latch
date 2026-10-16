use super::rows::values::display_size;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

#[cfg(test)]
mod tests;
mod view;

pub(super) struct Metrics {
    name: String,
    completed: u64,
    total: Option<u64>,
    samples: VecDeque<(Instant, u64)>,
    paused: bool,
}

impl Metrics {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            name: String::new(),
            completed: 0,
            total: None,
            samples: VecDeque::from([(now, 0)]),
            paused: false,
        }
    }

    pub(super) fn set_paused(&mut self, paused: bool, now: Instant) {
        if self.paused != paused {
            self.paused = paused;
            self.samples.clear();
            self.samples.push_back((now, self.completed));
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
        if self.paused {
            return;
        }
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
        if self.paused {
            return 0.;
        }
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
        if self.paused {
            return None;
        }
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
