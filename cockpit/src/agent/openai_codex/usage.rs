//! Usage accounting and rate-limit formatting (module-breakup: token
//! accounting, extracted from `openai_codex.rs`).
//!
//! The session usage ledger and the pure formatters the /status report and
//! the limit lines use. `CodexSharedState` (still in the parent) owns the
//! mutex; `events.rs` consumes [`Usage`] for its Done payloads.

use std::collections::{BTreeMap, BTreeSet};

/// Token counts the Responses API reports for one turn (`response.usage`).
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub(crate) struct Usage {
    pub(crate) input: Option<u64>,
    pub(crate) output: Option<u64>,
    pub(crate) reasoning: Option<u64>,
    pub(crate) cached_input: Option<u64>,
    pub(crate) cache_write: Option<u64>,
}

/// Running usage for the session, plus the most recent rate-limit headers (raw
/// name→value; the ChatGPT backend's exact header set isn't contractual, so we
/// capture whatever it sends rather than hardcoding guesses).
#[derive(Default)]
pub(crate) struct UsageStats {
    pub(crate) attempts: u64,
    pub(crate) unknown_usage_attempts: u64,
    pub(crate) partial_usage_attempts: u64,
    pub(crate) recent_attempts: std::collections::VecDeque<super::attempts::AttemptReceipt>,
    pub(crate) turns: u64,
    pub(crate) last: Usage,
    pub(crate) total_input: u64,
    pub(crate) total_output: u64,
    pub(crate) total_reasoning: u64,
    pub(crate) rate_limits: Vec<(String, String)>,
}

/// Render accumulated usage for `/status`. `None` until a turn has reported usage
/// (and no rate-limit headers seen) — so a never-used codex tab adds no noise.
pub(crate) fn format_usage(s: &UsageStats) -> Option<String> {
    if s.attempts == 0 && s.turns == 0 && s.rate_limits.is_empty() {
        return None;
    }
    let mut out = String::from("openai usage · this session");
    if s.turns > 0 {
        let reasoning = if s.last.reasoning.unwrap_or(0) > 0 {
            format!(" · reasoning {}", fmt_count(s.last.reasoning.unwrap_or(0)))
        } else {
            String::new()
        };
        let total = s.total_input + s.total_output;
        let in_pct = percent(s.total_input, total);
        let out_pct = percent(s.total_output, total);
        out.push_str(&format!(
            "\n    turns     {}\n    last      in {} · out {}{}\n    session   in {} · out {} · total {}\n    mix       in {} {:>3}% · out {} {:>3}%",
            s.turns,
            s.last.input.map(fmt_count).unwrap_or_else(|| "unknown".into()),
            s.last.output.map(fmt_count).unwrap_or_else(|| "unknown".into()),
            reasoning,
            fmt_count(s.total_input),
            fmt_count(s.total_output),
            fmt_count(total),
            usage_bar(in_pct, 12),
            in_pct,
            usage_bar(out_pct, 12),
            out_pct,
        ));
    }
    if s.attempts > 0 {
        out.push_str(&format!(
            "\n    attempts  {} · usage unknown {} · partial {}",
            s.attempts, s.unknown_usage_attempts, s.partial_usage_attempts
        ));
        // A bounded inspection tail; session counters cover evicted receipts.
        for receipt in s.recent_attempts.iter().rev().take(5).rev() {
            if let Ok(json) = serde_json::to_string(receipt) {
                out.push_str("\n    attempt   ");
                out.push_str(&json);
            }
        }
    }
    if !s.rate_limits.is_empty() {
        let lines = format_limit_lines(&s.rate_limits);
        if !lines.is_empty() {
            out.push_str("\n    plan");
            for line in lines {
                out.push_str("\n      ");
                out.push_str(&line);
            }
        }
    }
    Some(out)
}

pub(crate) fn format_limit_lines(rate_limits: &[(String, String)]) -> Vec<String> {
    let mut map = BTreeMap::new();
    for (k, v) in rate_limits {
        let key = limit_label(k);
        let val = v.trim();
        if !val.is_empty() {
            map.insert(key, val.to_string());
        }
    }
    let mut used = BTreeSet::new();
    let mut lines = Vec::new();

    let plan_type = take(&map, &mut used, "plan type");
    let active = take(&map, &mut used, "active limit");
    let bfox = take(&map, &mut used, "bengalfox limit name");
    if plan_type.is_some() || active.is_some() || bfox.is_some() {
        let mut parts = Vec::new();
        if let Some(v) = plan_type {
            parts.push(v);
        }
        if let Some(v) = active {
            parts.push(format!("active {v}"));
        }
        if let Some(v) = bfox {
            parts.push(format!("limit {v}"));
        }
        lines.push(format!("type      {}", parts.join(" · ")));
    }

    for (label, prefix) in [
        ("primary  ", "primary"),
        ("secondary", "secondary"),
        ("bfox pri ", "bengalfox primary"),
        ("bfox sec ", "bengalfox secondary"),
    ] {
        if let Some(line) = limit_window_line(&map, &mut used, label, prefix) {
            lines.push(line);
        }
    }

    let has_credits = take(&map, &mut used, "credits has credits");
    let unlimited = take(&map, &mut used, "credits unlimited");
    let balance = take(&map, &mut used, "credits balance");
    if has_credits.is_some() || unlimited.is_some() || balance.is_some() {
        let credit_state = match has_credits.as_deref() {
            Some(v) if truthy(v) => "has credits",
            Some(_) => "no credits",
            None => "credits unknown",
        };
        let unlimited_state = match unlimited.as_deref() {
            Some(v) if truthy(v) => "unlimited",
            Some(_) => "metered",
            None => "limit unknown",
        };
        let mut line = format!("credits   {credit_state} · {unlimited_state}");
        if let Some(balance) = balance {
            line.push_str(&format!(" · balance {balance}"));
        }
        lines.push(line);
    }

    let unknown: Vec<String> = map
        .iter()
        .filter(|(k, _)| !used.contains(k.as_str()))
        .map(|(k, v)| format!("{k} {v}"))
        .collect();
    if lines.is_empty() && !unknown.is_empty() {
        lines.push(format!("limits    {}", unknown.join(" · ")));
    }
    lines
}

pub(crate) fn limit_window_line(
    map: &BTreeMap<String, String>,
    used: &mut BTreeSet<String>,
    label: &str,
    prefix: &str,
) -> Option<String> {
    let pct = take(map, used, &format!("{prefix} used percent"));
    let reset_after = take(map, used, &format!("{prefix} reset after seconds"));
    used.insert(format!("{prefix} reset at"));
    let window = take(map, used, &format!("{prefix} window minutes"));
    let over = take(map, used, &format!("{prefix} over secondary limit percent"));
    if pct.is_none() && reset_after.is_none() && window.is_none() && over.is_none() {
        return None;
    }
    let pct_num = pct
        .as_deref()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0)
        .min(100);
    let mut parts = vec![format!("{} {:>3}%", usage_bar(pct_num, 12), pct_num)];
    if let Some(v) = reset_after.and_then(|v| v.parse::<u64>().ok()) {
        parts.push(format!("reset {}", fmt_seconds(v)));
    }
    if let Some(v) = window.and_then(|v| v.parse::<u64>().ok()) {
        parts.push(format!("window {}", fmt_minutes(v)));
    }
    if let Some(v) = over {
        parts.push(format!("over-sec {v}%"));
    }
    Some(format!("{label} {}", parts.join(" · ")))
}

pub(crate) fn take(
    map: &BTreeMap<String, String>,
    used: &mut BTreeSet<String>,
    key: &str,
) -> Option<String> {
    used.insert(key.to_string());
    map.get(key).cloned()
}

pub(crate) fn limit_label(k: &str) -> String {
    k.strip_prefix("x-codex-").unwrap_or(k).replace('-', " ")
}

pub(crate) fn usage_bar(percent: u64, width: usize) -> String {
    let pct = percent.min(100) as usize;
    let filled = (pct * width + 50) / 100;
    format!("[{}{}]", "#".repeat(filled), ".".repeat(width - filled))
}

pub(crate) fn percent(part: u64, total: u64) -> u64 {
    if total == 0 {
        0
    } else {
        ((part as u128 * 100 + (total / 2) as u128) / total as u128) as u64
    }
}

pub(crate) fn fmt_count(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

pub(crate) fn fmt_seconds(secs: u64) -> String {
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let minutes = (secs % 3_600) / 60;
    if days > 0 {
        format!("{days}d{hours}h")
    } else if hours > 0 {
        format!("{hours}h{minutes}m")
    } else {
        format!("{minutes}m")
    }
}

pub(crate) fn fmt_minutes(minutes: u64) -> String {
    if minutes >= 60 * 24 {
        format!("{}d", minutes / (60 * 24))
    } else if minutes >= 60 {
        format!("{}h", minutes / 60)
    } else {
        format!("{minutes}m")
    }
}

pub(crate) fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Pull plan rate-limit headers off a response (raw name→value). The ChatGPT
/// backend's header set isn't a stable contract, so match defensively: anything
/// vendor-prefixed (`x-codex-…`) or with `ratelimit`/`rate-limit` in the name.
pub(crate) fn collect_rate_limits(resp: &ureq::Response) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for name in resp.headers_names() {
        let lower = name.to_ascii_lowercase();
        let looks_like_limit = lower.starts_with("x-codex")
            || lower.contains("ratelimit")
            || lower.contains("rate-limit");
        if looks_like_limit && let Some(val) = resp.header(&name) {
            out.push((lower, val.trim().to_string()));
        }
    }
    out.sort();
    out
}

// ---------------------------------------------------------------------------
// SSE decoding for the Responses API
// ---------------------------------------------------------------------------
