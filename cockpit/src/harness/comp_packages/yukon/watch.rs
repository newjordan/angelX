use crate::harness::comp_watch::{
    ConfiguredWatchSource, SlotSnapshot, StatusSource,
};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

pub(crate) struct YukonStatusSource {
    benchmark: String,
    id: String,
    pending: Option<Receiver<Result<super::status::SubmissionStatus, String>>>,
    last: Option<super::status::SubmissionStatus>,
    next_poll: Instant,
    error_notice: Option<String>,
}

impl YukonStatusSource {
    fn new(benchmark: String, id: String) -> Self {
        Self {
            benchmark,
            id,
            pending: None,
            last: None,
            next_poll: Instant::now(),
            error_notice: None,
        }
    }
}

impl StatusSource for YukonStatusSource {
    fn probe(&mut self, id: &str) -> Result<SlotSnapshot, String> {
        if id != self.id {
            return Err("Yukon watch slot identity mismatch".into());
        }
        if let Some(receiver) = &self.pending {
            let result = match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    Some(Err("Yukon status worker disconnected".into()))
                }
            };
            if let Some(result) = result {
                self.pending = None;
                self.next_poll = Instant::now() + Duration::from_secs(15);
                match result {
                    Ok(receipt) => self.last = Some(receipt),
                    Err(error) => {
                        self.error_notice = Some(format!(
                            "Yukon status refresh failed; last observation is stale: {error}"
                        ))
                    }
                }
            }
        }
        if self.pending.is_none() && Instant::now() >= self.next_poll {
            let (sender, receiver) = mpsc::sync_channel(1);
            let benchmark = self.benchmark.clone();
            let submission = self.id.clone();
            match std::thread::Builder::new()
                .name("angel-yukon-status".into())
                .spawn(move || {
                    let _ = sender.send(super::status::fetch(&benchmark, &submission));
                }) {
                Ok(_) => self.pending = Some(receiver),
                Err(_) => {
                    self.next_poll = Instant::now() + Duration::from_secs(15);
                    self.error_notice =
                        Some("Yukon status worker could not start; observation unavailable".into());
                }
            }
        }
        let receipt = self
            .last
            .as_ref()
            .ok_or("Yukon status observation pending")?;
        Ok(SlotSnapshot {
            id: receipt.id.clone(),
            status: receipt.status.clone(),
            score: receipt.official_score.clone(),
            rejection_reason: None,
        })
    }

    fn receipt_note(&self) -> Option<String> {
        self.last.as_ref().map(|r| r.receipt_note())
    }
    fn take_error_notice(&mut self) -> Option<String> {
        self.error_notice.take()
    }
}

/// Only operator configuration can adopt a live slot. An ID echoed by a tool
/// never selects which authenticated response we observe.
pub(crate) fn configured_yukon_watch() -> Result<Option<(String, ConfiguredWatchSource)>, String> {
    let benchmark = std::env::var("ANGEL_WATCH_YUKON_BENCHMARK").ok();
    let id = std::env::var("ANGEL_WATCH_YUKON_SUBMISSION").ok();
    match (benchmark, id) {
        (None, None) => Ok(None),
        (Some(benchmark), Some(id))
            if super::status::valid_uuid(&benchmark) && super::status::valid_uuid(&id) => {
                Ok(Some((id.clone(), ConfiguredWatchSource::Yukon(Box::new(YukonStatusSource::new(benchmark, id))))))
            }
        _ => Err("Yukon watcher requires both ANGEL_WATCH_YUKON_BENCHMARK and ANGEL_WATCH_YUKON_SUBMISSION as full lowercase UUIDs".into()),
    }
}
