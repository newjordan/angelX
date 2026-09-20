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
mod tests {
    use super::*;
    use crate::club::Club;
    use crate::openai_codex::{parse_responses_event, tests::club};

    #[test]
    fn all_terminal_paths_keep_cache_and_do_not_double_count_snapshots() {
        let club = club();
        for kind in [
            "response.completed",
            "response.failed",
            "response.incomplete",
        ] {
            let mut attempt = Attempt::new(
                &club,
                br#"{"model":"fixture","reasoning":{"effort":"low"}}"#,
            );
            let payload = format!(
                r#"data: {{"type":"{kind}","response":{{"usage":{{"input_tokens":100,"input_tokens_details":{{"cached_tokens":80}},"output_tokens":20,"output_tokens_details":{{"reasoning_tokens":12}}}}}}}}"#
            );
            let event = parse_responses_event(&payload);
            attempt.observe(&event);
            attempt.observe(&event);
        }
        let usage = club.token_usage().unwrap();
        assert_eq!(
            (
                usage.turns,
                usage.total_input,
                usage.total_output,
                usage.total_reasoning
            ),
            (3, 300, 60, 36)
        );
        let cache = club.cache_usage();
        assert_eq!(
            (cache.read_accounting_responses, cache.read_input_tokens),
            (3, 240)
        );
        let stats = club.shared.usage.lock().unwrap();
        assert_eq!(stats.attempts, 3);
        assert_eq!(stats.unknown_usage_attempts, 0);
        let outcomes: Vec<_> = stats.recent_attempts.iter().map(|r| r.outcome).collect();
        assert_eq!(outcomes, ["completed", "failed", "incomplete"]);
        assert!(
            stats
                .recent_attempts
                .iter()
                .all(|r| r.wire_effort.as_deref() == Some("low"))
        );
    }

    #[test]
    fn interrupted_and_cancelled_attempts_keep_observed_usage_or_explicit_unknown() {
        let club = club();
        {
            let mut attempt = Attempt::new(&club, br#"{"model":"fixture"}"#);
            attempt.outcome("interrupted");
            attempt.observe(&parse_responses_event(r#"data: {"type":"response.in_progress","response":{"usage":{"input_tokens":7,"output_tokens":2}}}"#));
        }
        {
            let mut attempt = Attempt::new(&club, br#"{"model":"fixture"}"#);
            attempt.outcome("cancelled");
        }
        let stats = club.shared.usage.lock().unwrap();
        assert_eq!((stats.attempts, stats.unknown_usage_attempts), (2, 1));
        assert_eq!(stats.recent_attempts[0].usage.unwrap().input, Some(7));
        assert!(stats.recent_attempts[1].usage.is_none());
        assert!(stats.recent_attempts[1].wire_effort.is_none());
    }

    #[test]
    fn cancellation_accounts_for_a_received_terminal_line_and_partial_fields_merge() {
        use std::io::BufRead;
        let club = club();
        {
            let mut attempt = Attempt::new(&club, br#"{"model":"fixture"}"#);
            attempt.receive(r#"data: {"type":"response.in_progress","response":{"usage":{"input_tokens":100,"output_tokens":10,"input_tokens_details":{"cached_tokens":80}}}}"#, false);
            let reader = std::io::Cursor::new(
                br#"data: {"type":"response.completed","response":{"usage":{"output_tokens":20}}}"#,
            );
            for line in reader.lines() {
                attempt.receive(&line.unwrap(), true);
            }
        }
        let stats = club.shared.usage.lock().unwrap();
        let receipt = stats.recent_attempts.back().unwrap();
        assert_eq!(receipt.outcome, "cancelled");
        let usage = receipt.usage.unwrap();
        assert_eq!(
            (usage.input, usage.output, usage.cached_input),
            (Some(100), Some(20), Some(80))
        );
        assert_eq!(
            (stats.unknown_usage_attempts, stats.partial_usage_attempts),
            (0, 0)
        );
    }

    #[test]
    fn cache_only_usage_keeps_ordinary_counts_unknown() {
        let club = club();
        {
            let mut attempt = Attempt::new(&club, br#"{"model":"fixture"}"#);
            attempt.receive(r#"data: {"type":"response.failed","response":{"usage":{"input_tokens_details":{"cached_tokens":80}}}}"#, false);
        }
        let stats = club.shared.usage.lock().unwrap();
        let usage = stats.recent_attempts.back().unwrap().usage.unwrap();
        assert!(usage.input.is_none() && usage.output.is_none());
        assert_eq!(stats.partial_usage_attempts, 1);
        assert_eq!(club.cache_usage().read_input_tokens, 80);
    }

    #[test]
    fn request_failures_are_bounded_and_unknown_is_not_reported_as_zero() {
        let club = club();
        for _ in 0..RECEIPT_CAP + 5 {
            let _attempt = Attempt::new(&club, br#"{"model":"fixture"}"#);
        }
        let stats = club.shared.usage.lock().unwrap();
        assert_eq!(stats.attempts, 69);
        assert_eq!(stats.unknown_usage_attempts, 69);
        assert_eq!(stats.recent_attempts.len(), RECEIPT_CAP);
        assert_eq!(stats.turns, 0);
        let json = serde_json::to_value(stats.recent_attempts.back().unwrap()).unwrap();
        assert!(json["usage"].is_null());
    }
}

#[cfg(test)]
mod usage_projection_tests {
    use super::*;
    #[test]
    fn usage_projection_codex_attempts_preserve_cache_only_zero_and_unknown() {
        use crate::club::Club;
        let club = crate::openai_codex::tests::club();
        let before = club.usage_accounting();
        {
            let mut attempt = Attempt::new(&club, br#"{"model":"fixture"}"#);
            attempt.receive(r#"data: {"type":"response.failed","response":{"usage":{"input_tokens_details":{"cached_tokens":80}}}}"#, false);
        }
        {
            let _unknown = Attempt::new(&club, br#"{"model":"fixture"}"#);
        }
        {
            let mut attempt = Attempt::new(&club, br#"{"model":"fixture"}"#);
            attempt.receive(r#"data: {"type":"response.completed","response":{"usage":{"input_tokens":0,"output_tokens":0}}}"#, false);
        }
        let report = crate::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
        assert_eq!(
            (
                report.attempts,
                report.input,
                report.output,
                report.reasoning,
                report.cache_read
            ),
            (3, Some(0), Some(0), None, Some(80))
        );
        assert_eq!(
            (
                report.reported_attempts.input,
                report.reported_attempts.cache_read
            ),
            (1, 1)
        );
        assert_eq!(
            report.raw_field_reports["response.usage.input_tokens_details.cached_tokens"],
            1
        );
        assert_eq!((report.uncached_input, report.cache_hit_pct), (None, None));
        assert!(!report.core_complete);
    }
}

#[cfg(test)]
mod non_http_usage_contract_fixture_tests {
    include!("non_http_usage_contract_fixture_tests.rs");
}

#[cfg(test)]
mod trajectory_tests {
    use super::*;
    use crate::club::Club;
    use std::io::Read;
    #[test]
    fn trajectory_c03c_oauth_attempt_bytes_and_usage_are_sealed_once() {
        let _lock = crate::tests::env_lock();
        let club = crate::openai_codex::tests::club();
        crate::harness::reset_turn_ledger(&club);
        crate::harness::note_timing_origin(Instant::now());
        crate::harness::begin_model_request();
        let wire = br#"{"model":"fixture"}"#;
        let response = "data: π\n\n";
        {
            let mut attempt = Attempt::new(&club, wire);
            let mut body = String::new();
            attempt
                .response_reader(response.as_bytes())
                .read_to_string(&mut body)
                .unwrap();
            attempt.receive(r#"data: {"type":"response.completed","response":{"usage":{"input_tokens":7,"output_tokens":2}}}"#, false);
        }
        let samples = crate::harness::provider_call_samples();
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0]["request_bytes"], wire.len());
        assert_eq!(samples[0]["response_bytes"], response.len());
        assert_eq!(samples[0]["usage"][0], 7);
        assert_eq!(club.token_usage().unwrap().turns, 1);
        crate::harness::end_model_request();
    }
}
