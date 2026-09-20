//! Coherent provider observations, separate from legacy numeric UI meters.
use std::collections::BTreeMap;
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};

mod projection;
pub(crate) use projection::AccountingReport;

pub(crate) const PATHS: [&str; 22] = [
    "prompt_tokens",
    "input_tokens",
    "completion_tokens",
    "output_tokens",
    "completion_tokens_details.reasoning_tokens",
    "output_tokens_details.reasoning_tokens",
    "cache_read_input_tokens",
    "prompt_tokens_details.cached_tokens",
    "input_tokens_details.cached_tokens",
    "prompt_cache_hit_tokens",
    "cache_write_input_tokens",
    "cache_creation_input_tokens",
    "prompt_tokens_details.cache_write_tokens",
    "input_tokens_details.cache_write_tokens",
    "_meta.usage.inputTokens",
    "_meta.usage.outputTokens",
    "_meta.usage.reasoningTokens",
    "response.usage.input_tokens",
    "response.usage.output_tokens",
    "response.usage.output_tokens_details.reasoning_tokens",
    "response.usage.input_tokens_details.cached_tokens",
    "response.usage.input_tokens_details.cache_write_tokens",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum CacheConvention {
    Included,
    // Synthetic contract coverage; no current provider reports this convention.
    #[cfg(test)]
    Separate,
    #[default]
    Unknown,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ReasoningConvention {
    Included,
    /// Direct xAI Chat Completions reports reasoning outside completion_tokens.
    Separate,
    #[default]
    Unknown,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct UsageContract {
    pub(crate) cache: CacheConvention,
    pub(crate) reasoning: ReasoningConvention,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct UsageObservation {
    /// Reported input/output/reasoning/cache-read/cache-write, without rewriting totals.
    pub(crate) raw: [Option<u64>; 5],
    /// Exact accepted wire aliases; arbitrary response keys never enter this array.
    pub(crate) paths: [Option<&'static str>; 5],
    pub(crate) contract: UsageContract,
}

impl UsageObservation {
    /// Generated tokens under the provider's explicit contract. Keep raw wire
    /// counters intact; missing components and overflow remain unknown.
    pub(crate) fn generation_output(self) -> Option<u64> {
        match self.contract.reasoning {
            ReasoningConvention::Included => self.raw[1],
            ReasoningConvention::Separate => self.raw[1]
                .zip(self.raw[2])
                .and_then(|(output, reasoning)| output.checked_add(reasoning)),
            ReasoningConvention::Unknown => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct FieldTotal {
    pub(crate) sum: u64,
    pub(crate) reports: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct AccountingSnapshot {
    pub(crate) attempts: u64,
    /// Raw five counters, ordinary input, total prompt, total generated output.
    pub(crate) fields: [FieldTotal; 8],
    pub(crate) cache_conventions: [u64; 3],
    pub(crate) reasoning_conventions: [u64; 3],
    pub(crate) path_reports: [u64; 22],
    pub(crate) unknown_path_fields: u64,
    pub(crate) inconsistent: u64,
    pub(crate) overflowed: bool,
}

fn checked_add(into: &mut u64, value: u64, overflow: &mut bool) {
    match into.checked_add(value) {
        Some(total) => *into = total,
        None => {
            *into = u64::MAX;
            *overflow = true;
        }
    }
}

impl AccountingSnapshot {
    pub(crate) fn record(&mut self, observation: Option<UsageObservation>) {
        checked_add(&mut self.attempts, 1, &mut self.overflowed);
        let Some(observation) = observation else {
            return;
        };
        let [input, output, reasoning, read, write] = observation.raw;
        let mut inconsistent = false;
        let prompt = match observation.contract.cache {
            CacheConvention::Included => {
                let combined = match (read, write) {
                    (Some(r), Some(w)) => r.checked_add(w),
                    _ => None,
                };
                inconsistent = input.is_some_and(|i| {
                    read.is_some_and(|r| r > i)
                        || write.is_some_and(|w| w > i)
                        || combined.is_some_and(|c| c > i)
                        || (read.is_some() && write.is_some() && combined.is_none())
                });
                input
            }
            #[cfg(test)]
            CacheConvention::Separate => match (input, read, write) {
                (Some(i), Some(r), Some(w)) => {
                    let result = i.checked_add(r).and_then(|v| v.checked_add(w));
                    self.overflowed |= result.is_none();
                    result
                }
                _ => None,
            },
            CacheConvention::Unknown => None,
        };
        let ordinary = match observation.contract.cache {
            CacheConvention::Included => match (input, read, write) {
                (Some(i), Some(r), Some(w)) => i.checked_sub(r).and_then(|v| v.checked_sub(w)),
                _ => None,
            },
            #[cfg(test)]
            CacheConvention::Separate => input,
            CacheConvention::Unknown => None,
        };
        let generation = match observation.contract.reasoning {
            ReasoningConvention::Included => {
                inconsistent |= output.is_some_and(|o| reasoning.is_some_and(|r| r > o));
                output
            }
            ReasoningConvention::Separate => match (output, reasoning) {
                (Some(o), Some(r)) => {
                    let result = o.checked_add(r);
                    self.overflowed |= result.is_none();
                    result
                }
                _ => None,
            },
            ReasoningConvention::Unknown => None,
        };
        if inconsistent {
            checked_add(&mut self.inconsistent, 1, &mut self.overflowed);
        }
        let convention = match observation.contract.cache {
            CacheConvention::Included => 0,
            #[cfg(test)]
            CacheConvention::Separate => 1,
            CacheConvention::Unknown => 2,
        };
        checked_add(
            &mut self.cache_conventions[convention],
            1,
            &mut self.overflowed,
        );
        let reasoning_convention = match observation.contract.reasoning {
            ReasoningConvention::Included => 0,
            ReasoningConvention::Separate => 1,
            ReasoningConvention::Unknown => 2,
        };
        checked_add(
            &mut self.reasoning_conventions[reasoning_convention],
            1,
            &mut self.overflowed,
        );
        let derived = if inconsistent {
            [None; 3]
        } else {
            [ordinary, prompt, generation]
        };
        for (field, value) in self
            .fields
            .iter_mut()
            .zip(observation.raw.into_iter().chain(derived))
        {
            if let Some(value) = value {
                checked_add(&mut field.sum, value, &mut self.overflowed);
                checked_add(&mut field.reports, 1, &mut self.overflowed);
            }
        }
        for (value, path) in observation.raw.into_iter().zip(observation.paths) {
            if value.is_none() {
                continue;
            }
            if let Some(index) =
                path.and_then(|p| PATHS.iter().position(|candidate| *candidate == p))
            {
                checked_add(&mut self.path_reports[index], 1, &mut self.overflowed);
            } else {
                checked_add(&mut self.unknown_path_fields, 1, &mut self.overflowed);
            }
        }
    }

    pub(crate) fn delta(self, before: Self) -> Self {
        let mut out = self;
        out.overflowed |= before.overflowed;
        let mut subtract = |a: u64, b: u64| match a.checked_sub(b) {
            Some(delta) => delta,
            None => {
                out.overflowed = true;
                0
            }
        };
        out.attempts = subtract(self.attempts, before.attempts);
        out.inconsistent = subtract(self.inconsistent, before.inconsistent);
        out.unknown_path_fields = subtract(self.unknown_path_fields, before.unknown_path_fields);
        for ((out, a), b) in out.fields.iter_mut().zip(self.fields).zip(before.fields) {
            out.sum = subtract(a.sum, b.sum);
            out.reports = subtract(a.reports, b.reports);
        }
        for ((out, a), b) in out
            .cache_conventions
            .iter_mut()
            .zip(self.cache_conventions)
            .zip(before.cache_conventions)
        {
            *out = subtract(a, b);
        }
        for ((out, a), b) in out
            .reasoning_conventions
            .iter_mut()
            .zip(self.reasoning_conventions)
            .zip(before.reasoning_conventions)
        {
            *out = subtract(a, b);
        }
        for ((out, a), b) in out
            .path_reports
            .iter_mut()
            .zip(self.path_reports)
            .zip(before.path_reports)
        {
            *out = subtract(a, b);
        }
        out
    }

    pub(crate) fn add(&mut self, other: Self) {
        self.overflowed |= other.overflowed;
        checked_add(&mut self.attempts, other.attempts, &mut self.overflowed);
        checked_add(
            &mut self.inconsistent,
            other.inconsistent,
            &mut self.overflowed,
        );
        checked_add(
            &mut self.unknown_path_fields,
            other.unknown_path_fields,
            &mut self.overflowed,
        );
        for (field, other) in self.fields.iter_mut().zip(other.fields) {
            checked_add(&mut field.sum, other.sum, &mut self.overflowed);
            checked_add(&mut field.reports, other.reports, &mut self.overflowed);
        }
        for (value, other) in self
            .cache_conventions
            .iter_mut()
            .zip(other.cache_conventions)
        {
            checked_add(value, other, &mut self.overflowed);
        }
        for (value, other) in self
            .reasoning_conventions
            .iter_mut()
            .zip(other.reasoning_conventions)
        {
            checked_add(value, other, &mut self.overflowed);
        }
        for (value, other) in self.path_reports.iter_mut().zip(other.path_reports) {
            checked_add(value, other, &mut self.overflowed);
        }
    }
}

pub(crate) struct AccountingCell {
    id: u64,
    state: Mutex<AccountingSnapshot>,
}
impl Default for AccountingCell {
    fn default() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let id = NEXT_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .unwrap_or(0);
        Self {
            id,
            state: Mutex::new(AccountingSnapshot::default()),
        }
    }
}
impl AccountingCell {
    pub(crate) fn attempt(&self) -> AccountingAttempt<'_> {
        AccountingAttempt {
            cell: self,
            formation_reservation: None,
            observation: None,
            trace_attempt: crate::harness::begin_provider_attempt(self.id),
            request_bytes: None,
            response_bytes: None,
        }
    }
    /// One short critical section publishes the entire completed attempt.
    /// No network, callback, or tool work is performed under this lock.
    pub(crate) fn record(&self, observation: Option<UsageObservation>) {
        if let Ok(mut state) = self.state.lock() {
            state.record(observation);
        }
    }
    pub(crate) fn view(&self) -> AccountingView {
        match self.state.lock() {
            Ok(state) if self.id != 0 => AccountingView {
                sources: [(self.id, *state)].into(),
                untracked: false,
            },
            _ => AccountingView::untracked(),
        }
    }
}

pub(crate) struct AccountingAttempt<'a> {
    cell: &'a AccountingCell,
    formation_reservation: Option<crate::harness::formation_budget::Reservation>,
    trace_attempt: Option<usize>,
    request_bytes: Option<u64>,
    response_bytes: Option<std::sync::Arc<AtomicU64>>,
    observation: Option<UsageObservation>,
}
/// Counts body bytes below buffering and decoding, including partial/error reads.
pub(crate) struct ResponseByteReader<R> {
    inner: R,
    count: std::sync::Arc<AtomicU64>,
}
impl<R: std::io::Read> std::io::Read for ResponseByteReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buffer)?;
        self.count.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}
impl AccountingAttempt<'_> {
    pub(crate) fn request_bytes(&mut self, bytes: usize) {
        self.request_bytes = Some(bytes as u64);
    }
    pub(crate) fn response_reader<R: std::io::Read>(&mut self, inner: R) -> ResponseByteReader<R> {
        let count = std::sync::Arc::new(AtomicU64::new(0));
        self.response_bytes = Some(std::sync::Arc::clone(&count));
        ResponseByteReader { inner, count }
    }

    /// Attach bookkeeping to this attempt. Settlement records overruns and
    /// unknown usage; neither affects retry or formation admission.
    pub(crate) fn reserve_formation(
        &mut self,
        reservation: crate::harness::formation_budget::Reservation,
    ) {
        self.formation_reservation = Some(reservation);
    }
    pub(crate) fn observe(&mut self, observation: Option<UsageObservation>) {
        if let Some(value) = observation {
            if let Some(reservation) = &self.formation_reservation {
                reservation.observe(value);
            }
            self.observation = observation;
        }
    }
}
impl Drop for AccountingAttempt<'_> {
    fn drop(&mut self) {
        if let Some(reservation) = self.formation_reservation.take() {
            reservation.settle(self.observation);
        }
        self.cell.record(self.observation);
        crate::harness::note_provider_bytes(
            self.trace_attempt,
            self.request_bytes,
            self.response_bytes
                .as_ref()
                .map(|count| count.load(Ordering::Relaxed)),
        );
        crate::harness::end_provider_attempt(self.trace_attempt, self.observation);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountingView {
    pub(crate) sources: BTreeMap<u64, AccountingSnapshot>,
    pub(crate) untracked: bool,
}
impl AccountingView {
    pub(crate) fn untracked() -> Self {
        Self {
            untracked: true,
            ..Self::default()
        }
    }
    pub(crate) fn extend(&mut self, other: Self) {
        self.untracked |= other.untracked;
        for (id, newer) in other.sources {
            self.sources
                .entry(id)
                .and_modify(|old| {
                    if newer.attempts > old.attempts {
                        *old = newer;
                    } else if newer.attempts == old.attempts && newer != *old {
                        self.untracked = true;
                    }
                })
                .or_insert(newer);
        }
    }
    pub(crate) fn delta(&self, before: &Self) -> AccountingReport {
        let mut total = AccountingSnapshot::default();
        let mut untracked = self.untracked || before.untracked;
        for (id, after) in &self.sources {
            total.add(after.delta(before.sources.get(id).copied().unwrap_or_default()));
        }
        untracked |= before
            .sources
            .keys()
            .any(|id| !self.sources.contains_key(id));
        AccountingReport::from_snapshot(total, untracked)
    }
}

#[cfg(test)]
mod tests;
