use tokio::sync::{mpsc, watch};

use super::TransferEvent;

pub(super) struct Reporter {
    events: mpsc::Sender<TransferEvent>,
    latest: watch::Sender<Option<TransferEvent>>,
    name: String,
    completed: u64,
    total: Option<u64>,
    aggregate: bool,
}

impl Reporter {
    pub(super) fn new(
        events: mpsc::Sender<TransferEvent>,
        latest: watch::Sender<Option<TransferEvent>>,
    ) -> Self {
        Self {
            events,
            latest,
            name: String::new(),
            completed: 0,
            total: None,
            aggregate: false,
        }
    }

    pub(super) fn total(&mut self, total: Option<u64>) {
        self.aggregate = true;
        self.total = total;
    }

    pub(super) fn file(&mut self, name: String, total: Option<u64>) {
        self.name = name;
        if !self.aggregate {
            self.total = total;
        }
        self.report();
    }

    pub(super) fn advance(&mut self, length: usize) {
        self.completed = self.completed.saturating_add(length as u64);
        self.report();
    }

    fn report(&self) {
        let event = TransferEvent::Progress {
            name: self.name.clone(),
            completed: self.completed,
            total: self.total,
        };
        self.latest.send_replace(Some(event.clone()));
        let _ = self.events.try_send(event);
    }
}
