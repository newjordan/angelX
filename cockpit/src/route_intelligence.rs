//! Privacy-safe route evidence distilled from the append-only experience ledger.
//!
//! Operational evidence (clean completion, failover, latency, token spend) and
//! explicit user quality labels are kept as separate channels. Neither channel
//! inspects prompt or response text, and a clean completion is never silently
//! treated as a useful answer. The reader scans only a bounded recent tail and
//! skips malformed historical rows, so opening the Brain Route deck stays fast
//! even after years of sessions.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::club::RouteChoice;

const DEFAULT_RECENT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_RECENT_BYTES: u64 = 16 * 1024 * 1024;
pub(crate) const OPERATIONAL_PICK_MIN_SAMPLES: u32 = 5;
pub(crate) const USER_QUALITY_PICK_MIN_SAMPLES: u32 = 5;

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct RouteKey {
    driver: String,
    model: Option<String>,
    effort: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RouteEvidence {
    pub(crate) driver: String,
    pub(crate) model: Option<String>,
    pub(crate) effort: Option<String>,
    pub(crate) samples: u32,
    pub(crate) clean_answers: u32,
    pub(crate) recovered_answers: u32,
    pub(crate) failures: u32,
    pub(crate) p50_latency_ms: u64,
    pub(crate) mean_tokens: u64,
    pub(crate) user_useful: u32,
    pub(crate) user_miss: u32,
    pub(crate) last_ts: u64,
    pub(crate) quality_last_ts: u64,
}

impl RouteEvidence {
    pub(crate) fn clean_percent(&self) -> u32 {
        (self.clean_answers * 100 + self.samples / 2)
            .checked_div(self.samples)
            .unwrap_or(0)
    }

    /// Laplace-smoothed clean-completion rate in basis points. This is only an
    /// operational ranking signal; it says nothing about answer correctness.
    pub(crate) fn operational_score(&self) -> Option<u32> {
        (self.samples >= OPERATIONAL_PICK_MIN_SAMPLES)
            .then(|| ((self.clean_answers + 1) * 10_000) / (self.samples + 2))
    }

    pub(crate) fn user_quality_samples(&self) -> u32 {
        self.user_useful.saturating_add(self.user_miss)
    }

    pub(crate) fn useful_percent(&self) -> u32 {
        let samples = self.user_quality_samples();
        (self.user_useful * 100 + samples / 2)
            .checked_div(samples)
            .unwrap_or(0)
    }

    /// 95% Wilson lower confidence bound for explicit user usefulness labels,
    /// in basis points. Unlike a raw percentage, this rewards evidence depth:
    /// five useful labels do not outrank twenty equally-perfect labels.
    pub(crate) fn explicit_quality_score(&self) -> Option<u32> {
        let samples = self.user_quality_samples();
        if samples < USER_QUALITY_PICK_MIN_SAMPLES {
            return None;
        }
        let n = f64::from(samples);
        let p = f64::from(self.user_useful) / n;
        let z = 1.96_f64;
        let z2 = z * z;
        let centre = p + z2 / (2.0 * n);
        let margin = z * ((p * (1.0 - p) / n) + z2 / (4.0 * n * n)).sqrt();
        let lower = (centre - margin) / (1.0 + z2 / n);
        Some((lower.clamp(0.0, 1.0) * 10_000.0).round() as u32)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RouteEvidenceSnapshot {
    pub(crate) routes: Vec<RouteEvidence>,
    pub(crate) scanned_lines: usize,
    pub(crate) malformed_lines: usize,
    pub(crate) truncated: bool,
}

impl RouteEvidenceSnapshot {
    pub(crate) fn find(
        &self,
        driver: &str,
        model: &str,
        effort: Option<&str>,
    ) -> Option<&RouteEvidence> {
        let exact = self.routes.iter().find(|row| {
            row.driver.eq_ignore_ascii_case(driver)
                && row
                    .model
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case(model))
                && match (row.effort.as_deref(), effort) {
                    (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                    (None, None) => true,
                    _ => false,
                }
        });
        exact.or_else(|| {
            // Legacy/non-model clubs can still contribute when their driver is
            // the concrete route label. Never smear an old `openai` row across
            // every modern cached OpenAI model.
            self.routes.iter().find(|row| {
                row.model.is_none()
                    && row.driver.eq_ignore_ascii_case(driver)
                    && driver.eq_ignore_ascii_case(model)
            })
        })
    }

    /// Reflect a just-recorded label immediately in the open deck. A later
    /// disk reload replaces this snapshot, so this cannot double-count.
    pub(crate) fn apply_user_verdict(
        &mut self,
        route: &crate::club::RouteIdentity,
        verdict: crate::experience::RouteVerdict,
        completed_ms: u64,
    ) {
        let Some(driver) = normalized(Some(&route.driver)) else {
            return;
        };
        let key = RouteKey {
            driver,
            model: normalized(route.model.as_deref()),
            effort: normalized(route.reasoning_effort.as_deref()),
        };
        let index = self
            .routes
            .iter()
            .position(|row| route_matches(row, &key))
            .unwrap_or_else(|| {
                self.routes.push(RouteEvidence {
                    driver: key.driver.clone(),
                    model: key.model.clone(),
                    effort: key.effort.clone(),
                    ..RouteEvidence::default()
                });
                self.routes.len() - 1
            });
        let row = &mut self.routes[index];
        match verdict {
            crate::experience::RouteVerdict::Useful => {
                row.user_useful = row.user_useful.saturating_add(1)
            }
            crate::experience::RouteVerdict::Miss => {
                row.user_miss = row.user_miss.saturating_add(1)
            }
        }
        row.quality_last_ts = row.quality_last_ts.max(completed_ms / 1_000);
    }
}

fn route_matches(row: &RouteEvidence, key: &RouteKey) -> bool {
    row.driver.eq_ignore_ascii_case(&key.driver)
        && row.model == key.model
        && row.effort == key.effort
}

/// Best sufficiently-observed reachable route for operational reliability,
/// with enough context reserve for another useful turn. Unknown windows remain
/// eligible; trusted windows at 80%+ do not. This never mutates the Bag or
/// makes an answer-quality claim; the operator still confirms with Enter.
pub(crate) fn operational_pick_for_context(
    snapshot: &RouteEvidenceSnapshot,
    choices: &[RouteChoice],
    context_used: usize,
) -> Option<usize> {
    choices
        .iter()
        .enumerate()
        .filter(|(_, choice)| choice.available && has_recommendation_headroom(choice, context_used))
        .filter_map(|(index, choice)| {
            let evidence = snapshot.find(
                &choice.driver,
                &choice.model,
                choice.reasoning_effort.as_deref(),
            )?;
            Some((index, evidence, evidence.operational_score()?))
        })
        .max_by(cmp_operational)
        .map(|(index, _, _)| index)
}

/// Best sufficiently-labelled reachable route by explicit user usefulness,
/// with the same context reserve as the operational channel. Quality evidence
/// never overrides a known inability to carry the current thread safely. `q`
/// may move the UI cursor here, but selection still requires Enter.
pub(crate) fn quality_pick_for_context(
    snapshot: &RouteEvidenceSnapshot,
    choices: &[RouteChoice],
    context_used: usize,
) -> Option<usize> {
    choices
        .iter()
        .enumerate()
        .filter(|(_, choice)| choice.available && has_recommendation_headroom(choice, context_used))
        .filter_map(|(index, choice)| {
            let evidence = snapshot.find(
                &choice.driver,
                &choice.model,
                choice.reasoning_effort.as_deref(),
            )?;
            Some((index, evidence, evidence.explicit_quality_score()?))
        })
        .max_by(cmp_quality)
        .map(|(index, _, _)| index)
}

/// Best sufficiently-observed reasoning level for one concrete route. This is
/// operational evidence only and never mutates the backend-owned effort.
pub(crate) fn operational_effort_pick(
    snapshot: &RouteEvidenceSnapshot,
    choice: &RouteChoice,
    levels: &[String],
) -> Option<usize> {
    levels
        .iter()
        .enumerate()
        .filter_map(|(index, effort)| {
            let evidence = snapshot.find(&choice.driver, &choice.model, Some(effort))?;
            Some((index, evidence, evidence.operational_score()?))
        })
        .max_by(cmp_operational)
        .map(|(index, _, _)| index)
}

/// Best sufficiently-labelled reasoning level by explicit user usefulness.
/// Kept separate from operational completion evidence by construction.
pub(crate) fn quality_effort_pick(
    snapshot: &RouteEvidenceSnapshot,
    choice: &RouteChoice,
    levels: &[String],
) -> Option<usize> {
    levels
        .iter()
        .enumerate()
        .filter_map(|(index, effort)| {
            let evidence = snapshot.find(&choice.driver, &choice.model, Some(effort))?;
            Some((index, evidence, evidence.explicit_quality_score()?))
        })
        .max_by(cmp_quality)
        .map(|(index, _, _)| index)
}

fn cmp_operational(
    (_, a, a_score): &(usize, &RouteEvidence, u32),
    (_, b, b_score): &(usize, &RouteEvidence, u32),
) -> std::cmp::Ordering {
    a_score
        .cmp(b_score)
        .then_with(|| b.p50_latency_ms.cmp(&a.p50_latency_ms))
        .then_with(|| a.samples.cmp(&b.samples))
        .then_with(|| a.last_ts.cmp(&b.last_ts))
}

fn cmp_quality(
    (_, a, a_score): &(usize, &RouteEvidence, u32),
    (_, b, b_score): &(usize, &RouteEvidence, u32),
) -> std::cmp::Ordering {
    a_score
        .cmp(b_score)
        .then_with(|| a.user_quality_samples().cmp(&b.user_quality_samples()))
        .then_with(|| {
            a.operational_score()
                .unwrap_or(0)
                .cmp(&b.operational_score().unwrap_or(0))
        })
        .then_with(|| a.quality_last_ts.cmp(&b.quality_last_ts))
}

fn has_recommendation_headroom(choice: &RouteChoice, context_used: usize) -> bool {
    choice
        .metadata
        .context_usage_percent(context_used)
        .is_none_or(|percent| percent < 80)
}

#[derive(Default)]
struct Accumulator {
    samples: u32,
    clean_answers: u32,
    recovered_answers: u32,
    failures: u32,
    latencies: Vec<u64>,
    total_tokens: u64,
    user_useful: u32,
    user_miss: u32,
    last_ts: u64,
    quality_last_ts: u64,
}

fn normalized(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn tokens_in(outcome: &serde_json::Value) -> u64 {
    outcome
        .get("tokens")
        .and_then(serde_json::Value::as_object)
        .map(|tokens| {
            tokens
                .values()
                .map(|usage| {
                    usage
                        .get("in")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0)
                        + usage
                            .get("out")
                            .and_then(serde_json::Value::as_u64)
                            .unwrap_or(0)
                })
                .sum()
        })
        .unwrap_or(0)
}

fn route_key(record: &serde_json::Value) -> Option<RouteKey> {
    let driver = record
        .pointer("/route/driver")
        .and_then(serde_json::Value::as_str)
        .or_else(|| record.get("driver").and_then(serde_json::Value::as_str));
    Some(RouteKey {
        driver: normalized(driver)?,
        model: normalized(
            record
                .pointer("/route/model")
                .and_then(serde_json::Value::as_str),
        ),
        effort: normalized(
            record
                .pointer("/route/reasoning_effort")
                .and_then(serde_json::Value::as_str),
        ),
    })
}

pub(crate) fn parse_recent_jsonl(raw: &str, truncated: bool) -> RouteEvidenceSnapshot {
    let mut snapshot = RouteEvidenceSnapshot {
        truncated,
        ..RouteEvidenceSnapshot::default()
    };
    let mut grouped: BTreeMap<RouteKey, Accumulator> = BTreeMap::new();
    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        snapshot.scanned_lines += 1;
        let Ok(record) = serde_json::from_str::<serde_json::Value>(line) else {
            snapshot.malformed_lines += 1;
            continue;
        };
        let Some(key) = route_key(&record) else {
            continue;
        };
        if record.get("kind").and_then(serde_json::Value::as_str) == Some("route_verdict") {
            // Only explicit user labels enter this channel. Future automated
            // graders must get their own provenance and score, not masquerade
            // as the operator's judgment.
            if record.get("source").and_then(serde_json::Value::as_str) != Some("user") {
                continue;
            }
            let verdict = record.get("verdict").and_then(serde_json::Value::as_str);
            if !matches!(verdict, Some("useful" | "miss")) {
                continue;
            }
            let acc = grouped.entry(key).or_default();
            match verdict {
                Some("useful") => acc.user_useful = acc.user_useful.saturating_add(1),
                Some("miss") => acc.user_miss = acc.user_miss.saturating_add(1),
                _ => unreachable!("verdict was validated above"),
            }
            acc.quality_last_ts = acc.quality_last_ts.max(
                record
                    .get("ts")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
            );
            continue;
        }
        if record.get("kind").and_then(serde_json::Value::as_str) != Some("turn") {
            continue;
        }
        let Some(outcome) = record.get("outcome") else {
            continue;
        };
        let ok = outcome
            .get("ok")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let answer = outcome.get("stop").and_then(serde_json::Value::as_str) == Some("answer");
        let recovered = outcome
            .get("failovers")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|failovers| !failovers.is_empty());
        let acc = grouped.entry(key).or_default();
        acc.samples = acc.samples.saturating_add(1);
        if ok && answer && !recovered {
            acc.clean_answers = acc.clean_answers.saturating_add(1);
        } else if ok && answer {
            acc.recovered_answers = acc.recovered_answers.saturating_add(1);
        } else {
            acc.failures = acc.failures.saturating_add(1);
        }
        if let Some(latency) = outcome
            .get("latency_ms")
            .and_then(serde_json::Value::as_u64)
        {
            acc.latencies.push(latency);
        }
        acc.total_tokens = acc.total_tokens.saturating_add(tokens_in(outcome));
        acc.last_ts = acc.last_ts.max(
            record
                .get("ts")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
        );
    }

    snapshot.routes = grouped
        .into_iter()
        .map(|(key, mut acc)| {
            acc.latencies.sort_unstable();
            let p50_latency_ms = acc
                .latencies
                .get(acc.latencies.len().saturating_sub(1) / 2)
                .copied()
                .unwrap_or(0);
            RouteEvidence {
                driver: key.driver,
                model: key.model,
                effort: key.effort,
                samples: acc.samples,
                clean_answers: acc.clean_answers,
                recovered_answers: acc.recovered_answers,
                failures: acc.failures,
                p50_latency_ms,
                mean_tokens: acc.total_tokens / u64::from(acc.samples.max(1)),
                user_useful: acc.user_useful,
                user_miss: acc.user_miss,
                last_ts: acc.last_ts,
                quality_last_ts: acc.quality_last_ts,
            }
        })
        .collect();
    snapshot
}

pub(crate) fn load_recent(path: &Path, max_bytes: u64) -> RouteEvidenceSnapshot {
    let Ok(mut file) = std::fs::File::open(path) else {
        return RouteEvidenceSnapshot::default();
    };
    let Ok(meta) = file.metadata() else {
        return RouteEvidenceSnapshot::default();
    };
    let cap = max_bytes.clamp(1, MAX_RECENT_BYTES);
    let truncated = meta.len() > cap;
    let start = meta.len().saturating_sub(cap);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return RouteEvidenceSnapshot::default();
    }
    let mut bytes = Vec::with_capacity((meta.len() - start) as usize);
    if file.read_to_end(&mut bytes).is_err() {
        return RouteEvidenceSnapshot::default();
    }
    if truncated {
        if let Some(first_newline) = bytes.iter().position(|byte| *byte == b'\n') {
            bytes.drain(..=first_newline);
        } else {
            bytes.clear();
        }
    }
    parse_recent_jsonl(&String::from_utf8_lossy(&bytes), truncated)
}

pub(crate) fn load_default() -> RouteEvidenceSnapshot {
    if cfg!(test) {
        return RouteEvidenceSnapshot::default();
    }
    let max_bytes = std::env::var("ANGEL_ROUTE_EVIDENCE_BYTES")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_RECENT_BYTES);
    load_recent(&crate::experience::ledger_path(), max_bytes)
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/route_intelligence__tests.rs"]
mod tests;
