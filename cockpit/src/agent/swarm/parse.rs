//! Free helpers: prompt assembly, score parsing, env plumbing. The words of
//! the stage tasks and the labels on their payload are pages on `⠐`
//! (`book::d5_frames`); the drafts, counts and ids ride beside them as data.

use super::*;
use crate::agent::harness::book::d3_roles::pages;
use crate::agent::harness::book::d5_frames::{GUIDANCE, LABELS, MERGE, SYNTHESIS};

pub(crate) fn split_system(history: &[ChatMsg]) -> (String, Vec<ChatMsg>) {
    let mut sys = String::new();
    let mut rest = Vec::new();
    for m in history {
        if m.role == ChatRole::System {
            if !sys.is_empty() {
                sys.push_str("\n\n");
            }
            sys.push_str(&m.content);
        } else {
            rest.push(m.clone());
        }
    }
    (sys, rest)
}

pub(crate) fn compose(primary: &str, base_sys: &str) -> String {
    if base_sys.trim().is_empty() {
        primary.to_string()
    } else {
        format!("{primary}\n\n{base_sys}")
    }
}

pub(crate) fn cache_align_enabled() -> bool {
    env_flag_or("ANGEL_MOA_CACHE_ALIGN", true)
}

pub(crate) fn stage_parts(
    stage_sys: &str,
    base_sys: &str,
    ctx: &[ChatMsg],
    task: Option<String>,
) -> (String, Vec<ChatMsg>) {
    stage_parts_at(cache_align_enabled(), stage_sys, base_sys, ctx, task)
}

pub(crate) fn stage_parts_at(
    aligned: bool,
    stage_sys: &str,
    base_sys: &str,
    ctx: &[ChatMsg],
    task: Option<String>,
) -> (String, Vec<ChatMsg>) {
    let mut msgs = ctx.to_vec();
    if aligned {
        let mut tail = stage_sys.trim().to_string();
        if let Some(t) = task {
            if !tail.is_empty() {
                tail.push_str("\n\n");
            }
            tail.push_str(&t);
        }
        if !tail.is_empty() {
            msgs.push(ChatMsg::harness(tail));
        }
        (base_sys.to_string(), msgs)
    } else {
        if let Some(t) = task {
            msgs.push(ChatMsg::harness(t));
        }
        (compose(stage_sys, base_sys), msgs)
    }
}

pub(crate) fn aux_context_chars() -> usize {
    env_usize("ANGEL_MOA_AUX_CONTEXT_CHARS", 24_000)
}

pub(crate) fn aux_context(rest: &[ChatMsg]) -> Vec<ChatMsg> {
    aux_context_at(rest, aux_context_chars())
}

pub(crate) fn aux_context_at(rest: &[ChatMsg], budget: usize) -> Vec<ChatMsg> {
    if budget == 0 {
        return rest.to_vec();
    }
    let mut kept: Vec<ChatMsg> = Vec::new();
    let mut used = 0usize;
    for m in rest.iter().rev() {
        let len = m.content.chars().count();
        if kept.is_empty() {
            if len > budget {
                let mut bounded = m.clone();
                let keep = budget.saturating_sub(64).max(32);
                let tail: String = m.content.chars().skip(len - keep).collect();
                let omitted = len - keep;
                bounded.content =
                    format!("{} omitted={omitted}\n{tail}", pages(LABELS, [5])).into();
                kept.push(bounded);
                used = budget;
            } else {
                kept.push(m.clone());
                used += len;
            }
            continue;
        }
        if used + len > budget {
            break;
        }
        kept.push(m.clone());
        used += len;
    }
    kept.reverse();
    kept
}

pub(crate) fn engaged_formation_name() -> Option<String> {
    std::env::var("ANGEL_SOTA_MOA_FORMATION")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub(crate) fn text_only_history(history: &[ChatMsg]) -> Vec<ChatMsg> {
    history.iter().map(text_only_message).collect()
}

fn text_only_message(m: &ChatMsg) -> ChatMsg {
    let mut message = match m.role {
        ChatRole::System => ChatMsg::system(m.content.clone()),
        ChatRole::User => ChatMsg {
            role: ChatRole::User,
            content: m.content.clone(),
            attachments: m.attachments.clone(),
            tool_calls: Vec::new().into(),
            tool_call_id: None,
            private_reasoning: None,
            responses_replay: None,
            tool_receipt: None,
            recovery_context: m.recovery_context.clone(),
            book_memory: m.book_memory.clone(),
        },
        ChatRole::Harness => ChatMsg {
            role: ChatRole::Harness,
            content: m.content.clone(),
            attachments: m.attachments.clone(),
            tool_calls: Vec::new().into(),
            tool_call_id: None,
            private_reasoning: None,
            responses_replay: None,
            tool_receipt: None,
            recovery_context: m.recovery_context.clone(),
            book_memory: m.book_memory.clone(),
        },
        ChatRole::Assistant => {
            let mut parts = Vec::new();
            if !m.content.trim().is_empty() {
                parts.push(m.content.to_string());
            }
            if !m.tool_calls.is_empty() {
                parts.push(pages(LABELS, [6]));
                for call in m.tool_calls.iter() {
                    parts.push(format!("- {} id={} args={}", call.name, call.id, call.args));
                }
            }
            ChatMsg::assistant(parts.join("\n"))
        }
        ChatRole::Tool => {
            let id = m
                .tool_call_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("unknown");
            ChatMsg::harness(format!("{} id={id}\n{}", pages(LABELS, [7]), m.content))
        }
    };
    message.recovery_context = m.recovery_context.clone();
    message.book_memory = m.book_memory.clone();
    message
}

pub(crate) fn numbered_bounded(items: &[String], head: &str, max_chars: usize) -> String {
    items
        .iter()
        .enumerate()
        .map(|(i, d)| {
            format!(
                "### {head} {}\n{}",
                i + 1,
                bound_moa_draft(d.trim(), max_chars)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn numbered_weighted_bounded(
    items: &[(String, usize)],
    head: &str,
    max_chars: usize,
    show_weights: bool,
) -> String {
    let total: usize = items.iter().map(|(_, w)| *w).sum();
    items
        .iter()
        .enumerate()
        .map(|(i, (d, weight))| {
            let suffix = if show_weights && total > items.len() {
                format!(" {} weight={weight} total={total}", pages(LABELS, [3]))
            } else {
                String::new()
            };
            format!(
                "### {head} {}{suffix}\n{}",
                i + 1,
                bound_moa_draft(d.trim(), max_chars)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn moa_draft_max_chars() -> usize {
    std::env::var("ANGEL_MOA_DRAFT_MAX_CHARS")
        .or_else(|_| std::env::var("ANGEL_SOTA_MOA_DRAFT_MAX_CHARS"))
        .or_else(|_| std::env::var("ANGEL_SWARM_DRAFT_MAX_CHARS"))
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(12_000)
}

pub(crate) fn moa_final_target_chars() -> usize {
    std::env::var("ANGEL_MOA_FINAL_TARGET_CHARS")
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0)
}

fn final_synthesis() -> String {
    match moa_final_target_chars() {
        0 => SYNTHESIS.cells(),
        target => format!(
            "{}{} target={target}",
            pages(SYNTHESIS, 1..=3),
            GUIDANCE.cells()
        ),
    }
}

pub(crate) fn bound_moa_draft(text: &str, max_chars: usize) -> String {
    if max_chars == 0 || text.chars().count() <= max_chars {
        return text.to_string();
    }
    let keep = max_chars.saturating_sub(96).max(32);
    let head: String = text.chars().take(keep).collect();
    let omitted = text.chars().count().saturating_sub(keep);
    format!("{head}\n\n{} omitted={omitted}", pages(LABELS, [4]))
}

#[cfg(test)]
pub(crate) fn agg_task(drafts: &[String], concise: bool) -> String {
    agg_task_with_cap(drafts, concise, moa_draft_max_chars())
}

pub(crate) fn agg_task_with_cap(
    drafts: &[String],
    concise: bool,
    per_draft_chars: usize,
) -> String {
    let instruction = if concise {
        pages(MERGE, 1..=3)
    } else {
        final_synthesis()
    };
    format!(
        "{instruction}\n\n{} n={}\n\n{}",
        pages(LABELS, [1]),
        drafts.len(),
        numbered_bounded(drafts, "Response", per_draft_chars)
    )
}

pub(crate) fn agg_task_weighted(
    drafts: &[(String, usize)],
    concise: bool,
    per_draft_chars: usize,
) -> String {
    let total: usize = drafts.iter().map(|(_, w)| *w).sum();
    let instruction = if concise {
        MERGE.cells()
    } else {
        final_synthesis()
    };
    format!(
        "{instruction}\n\n{} total={} n={}\n\n{}",
        pages(LABELS, [2]),
        total,
        drafts.len(),
        numbered_weighted_bounded(drafts, "Response", per_draft_chars, true)
    )
}

pub(crate) fn keep_text(results: Vec<Result<String, String>>) -> Vec<String> {
    collect_text(results).drafts
}

pub(crate) struct DraftSet {
    pub(crate) drafts: Vec<String>,
    errors: Vec<String>,
    blanks: usize,
}

pub(crate) fn collect_text(results: Vec<Result<String, String>>) -> DraftSet {
    let mut drafts = Vec::new();
    let mut errors = Vec::new();
    let mut blanks = 0usize;
    for result in results {
        match result {
            Ok(text) if text.trim().is_empty() => blanks += 1,
            Ok(text) => drafts.push(text),
            Err(err) => errors.push(err),
        }
    }
    DraftSet {
        drafts,
        errors,
        blanks,
    }
}

impl DraftSet {
    pub(crate) fn failure_summary(&self) -> String {
        let mut parts = Vec::new();
        if self.blanks > 0 {
            parts.push(format!("blank drafts: {}", self.blanks));
        }
        for (idx, err) in self.errors.iter().take(3).enumerate() {
            parts.push(format!("error {}: {}", idx + 1, compact_error(err, 220)));
        }
        if self.errors.len() > 3 {
            parts.push(format!("{} additional error(s)", self.errors.len() - 3));
        }
        if parts.is_empty() {
            "No proposer error payload was reported.".to_string()
        } else {
            parts.join("; ")
        }
    }
}

pub(crate) fn compact_error(err: &str, max_chars: usize) -> String {
    let mut out = String::with_capacity(err.len().min(max_chars));
    let mut prev_space = false;
    for ch in err.chars() {
        let ch = if ch.is_whitespace() { ' ' } else { ch };
        if ch == ' ' && prev_space {
            continue;
        }
        prev_space = ch == ' ';
        out.push(ch);
        if out.chars().count() >= max_chars {
            break;
        }
    }
    let out = out.trim();
    if err.chars().count() > max_chars {
        format!("{out}...[truncated]")
    } else {
        out.to_string()
    }
}

pub(crate) fn dedup_similarity() -> f64 {
    std::env::var("ANGEL_MOA_DEDUP_SIM")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(0.92)
}

fn shingles(text: &str) -> std::collections::HashSet<u64> {
    use std::hash::{Hash, Hasher};
    let words: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    words
        .windows(3)
        .map(|w| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            w.hash(&mut h);
            h.finish()
        })
        .collect()
}

pub(crate) fn near_identical_at(a: &str, b: &str, threshold: f64) -> bool {
    if threshold <= 0.0 {
        return false;
    }
    let (sa, sb) = (shingles(a), shingles(b));
    if sa.is_empty() || sb.is_empty() {
        return a.split_whitespace().collect::<Vec<_>>()
            == b.split_whitespace().collect::<Vec<_>>();
    }
    let inter = sa.intersection(&sb).count() as f64;
    let union = (sa.len() + sb.len()) as f64 - inter;
    union > 0.0 && inter / union >= threshold
}

pub(crate) fn near_identical(a: &str, b: &str) -> bool {
    near_identical_at(a, b, dedup_similarity())
}

pub(crate) fn all_near_identical(candidates: &[String]) -> bool {
    let threshold = dedup_similarity();
    candidates
        .split_first()
        .map(|(first, rest)| rest.iter().all(|c| near_identical_at(first, c, threshold)))
        .unwrap_or(true)
}

pub(crate) fn collapse_near_duplicates(drafts: Vec<String>) -> Vec<String> {
    let threshold = dedup_similarity();
    if threshold <= 0.0 {
        return drafts;
    }
    let mut kept: Vec<String> = Vec::with_capacity(drafts.len());
    for d in drafts {
        if !kept.iter().any(|k| near_identical_at(k, &d, threshold)) {
            kept.push(d);
        }
    }
    kept
}

pub(crate) fn first_two_numbers(line: &str) -> Option<(usize, f64)> {
    let mut nums: Vec<String> = Vec::new();
    let mut cur = String::new();
    for ch in line.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            cur.push(ch);
        } else if !cur.is_empty() {
            nums.push(std::mem::take(&mut cur));
            if nums.len() == 2 {
                break;
            }
        }
    }
    if nums.len() < 2 && !cur.is_empty() {
        nums.push(cur);
    }
    if nums.len() < 2 {
        return None;
    }
    let idx: usize = nums[0].split('.').next()?.parse().ok()?;
    let score: f64 = nums[1].parse().ok()?;
    Some((idx, score))
}

pub(crate) fn parse_scores(text: &str, n_drafts: usize) -> Vec<(usize, f64)> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (i, s) in text.lines().filter_map(first_two_numbers) {
        if i >= 1 && i <= n_drafts && seen.insert(i) {
            out.push((i - 1, s));
        }
    }
    out
}

pub(crate) fn scan_numbers(line: &str, max: usize) -> Vec<f64> {
    let mut nums = Vec::new();
    let mut cur = String::new();
    for ch in line.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            cur.push(ch);
        } else if !cur.is_empty() {
            if let Ok(n) = cur.parse::<f64>() {
                nums.push(n);
            }
            cur.clear();
            if nums.len() == max {
                return nums;
            }
        }
    }
    if !cur.is_empty()
        && nums.len() < max
        && let Ok(n) = cur.parse::<f64>()
    {
        nums.push(n);
    }
    nums
}

pub(crate) fn parse_dim_scores(text: &str, n_drafts: usize) -> Vec<(usize, f64)> {
    let want = JUDGE_DIMS.len();
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let nums = scan_numbers(line, want + 1);
        if nums.len() < want + 1 {
            continue;
        }
        let idx = nums[0] as usize;
        if idx < 1 || idx > n_drafts || !seen.insert(idx) {
            continue;
        }
        let composite: f64 = JUDGE_DIMS
            .iter()
            .zip(&nums[1..=want])
            .map(|((_, w), s)| w * s)
            .sum();
        out.push((idx - 1, composite));
    }
    out
}

pub(crate) fn median_rank(reviews: &[Vec<(usize, f64)>]) -> Vec<(usize, f64)> {
    let mut by_draft: std::collections::BTreeMap<usize, Vec<f64>> =
        std::collections::BTreeMap::new();
    for review in reviews {
        for &(idx, score) in review {
            by_draft.entry(idx).or_default().push(score);
        }
    }
    let mut ranked: Vec<(usize, f64)> = by_draft
        .into_iter()
        .map(|(idx, scores)| (idx, median(scores)))
        .collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    ranked
}

pub(crate) fn median(mut scores: Vec<f64>) -> f64 {
    if scores.is_empty() {
        return 0.0;
    }
    scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = scores.len() / 2;
    if scores.len() % 2 == 1 {
        scores[mid]
    } else {
        (scores[mid - 1] + scores[mid]) / 2.0
    }
}

pub(crate) fn first_int(text: &str) -> Option<usize> {
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            cur.push(ch);
        } else if !cur.is_empty() {
            break;
        }
    }
    cur.parse().ok()
}

pub(crate) fn is_ok(v: &str) -> bool {
    let t = v.trim().to_ascii_uppercase();
    t == "OK"
        || t.starts_with("OK.")
        || t.starts_with("OK ")
        || t.starts_with("OK\n")
        || t.contains("NO ISSUES")
        || t.contains("NO PROBLEMS")
}

pub(crate) fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

pub(crate) fn env_flag_or(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            !(v.is_empty() || v == "0" || v == "false" || v == "no" || v == "off")
        }
        Err(_) => default,
    }
}

pub(crate) fn default_search_url() -> String {
    "http://127.0.0.1:8888/search".to_string()
}

pub(crate) fn is_delegator(key: &str) -> bool {
    matches!(
        key.split('+').next().unwrap_or(key),
        "red-team" | "empiricist"
    )
}
