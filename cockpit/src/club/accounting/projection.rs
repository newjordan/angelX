use super::{AccountingSnapshot, PATHS};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct FieldCoverage {
    pub(crate) input: u64,
    pub(crate) output: u64,
    pub(crate) reasoning: u64,
    pub(crate) cache_read: u64,
    pub(crate) cache_write: u64,
    pub(crate) uncached_input: u64,
    pub(crate) total_prompt: u64,
    pub(crate) generation_output: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct AccountingReport {
    pub(crate) schema: &'static str,
    /// Coherent per source, but not exact per-turn attribution under concurrency.
    pub(crate) attribution: &'static str,
    pub(crate) attempts: u64,
    pub(crate) input: Option<u64>,
    pub(crate) output: Option<u64>,
    pub(crate) reasoning: Option<u64>,
    pub(crate) cache_read: Option<u64>,
    pub(crate) cache_write: Option<u64>,
    pub(crate) uncached_input: Option<u64>,
    pub(crate) total_prompt: Option<u64>,
    pub(crate) generation_output: Option<u64>,
    pub(crate) cache_hit_pct: Option<u64>,
    /// Number of completed attempts actually reporting each field.
    pub(crate) reported_attempts: FieldCoverage,
    pub(crate) core_complete: bool,
    pub(crate) untracked_sources: bool,
    pub(crate) inconsistent_attempts: u64,
    pub(crate) overflowed: bool,
    /// Raw input sums may mix these conventions; total_prompt is normalized per attempt.
    pub(crate) cache_convention_attempts: BTreeMap<&'static str, u64>,
    /// Observed inclusion contracts, separate from whether reasoning was reported.
    /// Receiptless attempts have no convention observation and are not counted.
    pub(crate) reasoning_convention_attempts: BTreeMap<&'static str, u64>,
    /// Exact accepted wire paths and their observed-field counts in this window.
    pub(crate) raw_field_reports: BTreeMap<&'static str, u64>,
    pub(crate) unknown_path_fields: u64,
}

impl AccountingReport {
    pub(crate) fn from_snapshot(snapshot: AccountingSnapshot, untracked: bool) -> Self {
        let values: [Option<u64>; 8] = snapshot
            .fields
            .map(|field| (field.reports > 0 && !snapshot.overflowed).then_some(field.sum));
        let [
            input,
            output,
            reasoning,
            cache_read,
            cache_write,
            uncached_input,
            total_prompt,
            generation_output,
        ] = values;
        let [i, o, r, c, w, u, p, g] = snapshot.fields.map(|field| field.reports);
        let complete = |reports| {
            snapshot.attempts > 0
                && reports == snapshot.attempts
                && !untracked
                && !snapshot.overflowed
        };
        let cache_hit_pct = match (cache_read, total_prompt) {
            (Some(read), Some(prompt))
                if prompt > 0
                    && read <= prompt
                    && complete(c)
                    && complete(p)
                    && snapshot.inconsistent == 0 =>
            {
                Some(
                    ((u128::from(read) * 100 + u128::from(prompt) / 2) / u128::from(prompt)) as u64,
                )
            }
            _ => None,
        };
        Self {
            schema: "angel.usage.v2",
            attribution: "shared-cumulative-window",
            attempts: snapshot.attempts,
            input,
            output,
            reasoning,
            cache_read,
            cache_write,
            uncached_input,
            total_prompt,
            generation_output,
            cache_hit_pct,
            reported_attempts: FieldCoverage {
                input: i,
                output: o,
                reasoning: r,
                cache_read: c,
                cache_write: w,
                uncached_input: u,
                total_prompt: p,
                generation_output: g,
            },
            core_complete: complete(i) && complete(o) && snapshot.inconsistent == 0,
            untracked_sources: untracked,
            inconsistent_attempts: snapshot.inconsistent,
            overflowed: snapshot.overflowed,
            cache_convention_attempts: ["included", "separate", "unknown"]
                .into_iter()
                .zip(snapshot.cache_conventions)
                .filter(|(_, n)| *n > 0)
                .collect(),
            reasoning_convention_attempts: ["included", "separate", "unknown"]
                .into_iter()
                .zip(snapshot.reasoning_conventions)
                .filter(|(_, n)| *n > 0)
                .collect(),
            raw_field_reports: PATHS
                .into_iter()
                .zip(snapshot.path_reports)
                .filter(|(_, n)| *n > 0)
                .collect(),
            unknown_path_fields: snapshot.unknown_path_fields,
        }
    }
}
