//! One receipt per HTTP generation attempt, including every early return.
//! Unknown usage stays absent; no estimate is substituted for provider counts.
use super::{CodexClub, ResponseEvent, Usage};
use serde::{Deserialize, Serialize};
use std::time::Instant;

pub(super) const RECEIPT_CAP: usize = 64;

#[derive(Debug, Serialize)]
pub(crate) struct AttemptReceipt {
    pub model: String,
    pub wire_effort: Option<String>,
    pub outcome: &'static str,
    pub elapsed_ms: u64,
    pub usage: Option<Usage>,
}

pub(super) struct Attempt<'a> {
    club: &'a CodexClub,
    started: Instant,
    receipt: Option<AttemptReceipt>,
    accounting: crate::club::AccountingAttempt<'a>,
}

impl<'a> Attempt<'a> {
    pub(super) fn new(club: &'a CodexClub, wire: &[u8]) -> Self {
        #[derive(Default, Deserialize)]
        struct Reasoning {
            effort: Option<String>,
        }
        #[derive(Default, Deserialize)]
        struct Identity {
            model: String,
            #[serde(default)]
            reasoning: Reasoning,
        }
        // Deserialize only identity from the actual serialized request, after
        // optional transformations. Never retain prompt text or credentials.
        let identity = serde_json::from_slice::<Identity>(wire).unwrap_or_default();
        let mut accounting = club.shared.accounting.attempt();
        accounting.request_bytes(wire.len());
        Self {
            accounting,
            club,
            started: Instant::now(),
            receipt: Some(AttemptReceipt {
                model: identity.model,
                wire_effort: identity.reasoning.effort,
                outcome: "request_failed",
                elapsed_ms: 0,
                usage: None,
            }),
        }
    }

    pub(super) fn response_reader<R: std::io::Read>(
        &mut self,
        reader: R,
    ) -> crate::club::ResponseByteReader<R> {
        self.accounting.response_reader(reader)
    }

    pub(super) fn outcome(&mut self, outcome: &'static str) {
        self.receipt.as_mut().unwrap().outcome = outcome;
    }

    pub(super) fn receive(&mut self, line: &str, cancelled: bool) -> ResponseEvent {
        let event = super::parse_responses_event(line);
        self.observe(&event);
        if cancelled {
            self.outcome("cancelled");
        }
        event
    }

    pub(super) fn observe(&mut self, event: &ResponseEvent) {
        let receipt = self.receipt.as_mut().unwrap();
        let usage = match event {
            ResponseEvent::Done(usage) => {
                receipt.outcome = "completed";
                usage.as_ref()
            }
            ResponseEvent::Failed(_, usage) => {
                receipt.outcome = "failed";
                usage.as_ref()
            }
            ResponseEvent::Incomplete(_, usage) => {
                receipt.outcome = "incomplete";
                usage.as_ref()
            }
            ResponseEvent::Usage(usage) => Some(usage),
            _ => None,
        };
        // Provider snapshots are cumulative. Replace, never sum repeated frames.
        if let Some(usage) = usage {
            let prior = receipt.usage.unwrap_or_default();
            receipt.usage = Some(Usage {
                input: usage.input.or(prior.input),
                output: usage.output.or(prior.output),
                reasoning: usage.reasoning.or(prior.reasoning),
                cached_input: usage.cached_input.or(prior.cached_input),
                cache_write: usage.cache_write.or(prior.cache_write),
            });
        }
    }
}

impl Drop for Attempt<'_> {
    fn drop(&mut self) {
        let Some(mut receipt) = self.receipt.take() else {
            return;
        };
        receipt.elapsed_ms = self.started.elapsed().as_millis().min(u64::MAX as u128) as u64;
        self.accounting
            .observe(receipt.usage.map(super::accounting_observation));
        if let Some(usage) = receipt.usage {
            self.club.record_usage(usage);
        }
        if let Ok(mut stats) = self.club.shared.usage.lock() {
            stats.attempts = stats.attempts.saturating_add(1);
            if receipt.usage.is_none() {
                stats.unknown_usage_attempts = stats.unknown_usage_attempts.saturating_add(1);
            }
            if receipt
                .usage
                .is_some_and(|u| u.input.is_none() || u.output.is_none())
            {
                stats.partial_usage_attempts = stats.partial_usage_attempts.saturating_add(1);
            }
            if stats.recent_attempts.len() == RECEIPT_CAP {
                stats.recent_attempts.pop_front();
            }
            stats.recent_attempts.push_back(receipt);
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/openai_codex__attempts__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/openai_codex__attempts__usage_projection_tests.rs"]
mod usage_projection_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/openai_codex__attempts__non_http_usage_contract_fixture_tests.rs"]
mod non_http_usage_contract_fixture_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/openai_codex__attempts__trajectory_tests.rs"]
mod trajectory_tests;
