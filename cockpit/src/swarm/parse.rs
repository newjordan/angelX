//! Free helpers: prompt assembly, score parsing, env plumbing.

use super::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Split a conversation into (merged system text, non-system messages).
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

/// Prepend `primary`, then fold in the conversation's own system prompt (if any).
pub(crate) fn compose(primary: &str, base_sys: &str) -> String {
    if base_sys.trim().is_empty() {
        primary.to_string()
    } else {
        format!("{primary}\n\n{base_sys}")
    }
}

/// Whether stage calls are shaped for provider prefix caching (default yes;
/// `ANGEL_MOA_CACHE_ALIGN=0` restores the legacy stage-prompt-first shape).
pub(crate) fn cache_align_enabled() -> bool {
    env_flag_or("ANGEL_MOA_CACHE_ALIGN", true)
}

/// Assemble one stage call as `(system, messages)`.
///
/// Cache-aligned (default): the system slot carries only the conversation's own
/// stable `base_sys` (or nothing, when a stage passes `""`), and the stage or
/// persona instruction rides at the END, folded into the trailing task message.
/// Every call then opens with the byte-identical `[system][conversation...]`
/// prefix that provider prefix caches key on (DeepSeek/OpenAI-compatible
/// provider caches, vLLM APC / llama.cpp locally) — so the re-sent history becomes a
/// cache hit across the fan-out, across stages, and across turns, instead of a
/// full-price cold prefill on every call.
///
/// Legacy (`ANGEL_MOA_CACHE_ALIGN=0`): the stage prompt leads the system slot
/// (`compose(stage_sys, base_sys)`), byte-identical to the historical shape —
/// no prefix ever repeats between two calls.
pub(crate) fn stage_parts(
    stage_sys: &str,
    base_sys: &str,
    ctx: &[ChatMsg],
    task: Option<String>,
) -> (String, Vec<ChatMsg>) {
    stage_parts_at(cache_align_enabled(), stage_sys, base_sys, ctx, task)
}

/// [`stage_parts`] with the alignment mode explicit (testable without env).
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

/// Char budget for the conversation context sent to score-only stages (judge,
/// critic, chooser, verifier). The default keeps the newest ~24,000 chars —
/// score seats re-read the whole history on every call, which is where most
/// MoA input spend hides. `ANGEL_MOA_AUX_CONTEXT_CHARS=0` is the explicit
/// full-history opt-in.
pub(crate) fn aux_context_chars() -> usize {
    env_usize("ANGEL_MOA_AUX_CONTEXT_CHARS", 24_000)
}

/// Bounded conversation tail for score-only stages: the newest messages that fit
/// the budget, newest-inclusive — the final message (the problem, or the
/// delegate evidence right behind it) always survives, tail-kept with a leading
/// marker line when it alone busts the budget, so the most recent user ask and
/// tool results are what a scorer sees. Answer-producing stages keep the full
/// conversation.
pub(crate) fn aux_context(rest: &[ChatMsg]) -> Vec<ChatMsg> {
    aux_context_at(rest, aux_context_chars())
}

/// [`aux_context`] with the budget explicit (testable without env).
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
                    format!("[context trimmed: {omitted} chars omitted]\n{tail}").into();
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

/// The engaged formation's display name — written by the roster deck when it
/// arms a formation (`Formation::apply_sota_env`), cleared on disengage — so
/// the MoA ledgers can attribute a turn's economics to the formation that
/// spent them. `None` when no formation owns the process.
pub(crate) fn engaged_formation_name() -> Option<String> {
    std::env::var("ANGEL_SOTA_MOA_FORMATION")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Convert a full Angel transcript into plain chat messages for text-only MoA
/// worker calls. Providers still validate tool protocol fields even when
/// `tools=[]`, so stale or duplicated tool IDs from earlier turns must be prose.
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
            tool_receipt: None,
            recovery_context: m.recovery_context.clone(),
        },
        ChatRole::Harness => ChatMsg {
            role: ChatRole::Harness,
            content: m.content.clone(),
            attachments: m.attachments.clone(),
            tool_calls: Vec::new().into(),
            tool_call_id: None,
            private_reasoning: None,
            tool_receipt: None,
            recovery_context: m.recovery_context.clone(),
        },
        ChatRole::Assistant => {
            let mut parts = Vec::new();
            if !m.content.trim().is_empty() {
                parts.push(m.content.to_string());
            }
            if !m.tool_calls.is_empty() {
                parts.push("Assistant requested tools:".to_string());
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
            ChatMsg::harness(format!("[tool result for {id}]\n{}", m.content))
        }
    };
    message.recovery_context = m.recovery_context.clone();
    message
}

/// Number a set of texts as `### Head N` blocks for embedding in a prompt, each
/// bounded to `max_chars` (0 = unbounded) so a giant draft can't flood a stage's
/// input tokens.
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
                format!(" ({weight} of {total} drafts converged here)")
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
        // Default: each draft a stage copies into its payload is bounded to
        // 12k chars (tail trimmed with a marker) — the aggregator fan-out
        // multiplies every draft by every seat, so one unbounded draft
        // dominates a turn's input spend. Explicit `0` via env unbounds it;
        // route context still decides when hierarchical reduction is needed.
        .unwrap_or(12_000)
}

pub(crate) fn moa_final_target_chars() -> usize {
    std::env::var("ANGEL_MOA_FINAL_TARGET_CHARS")
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0)
}

fn final_output_guidance() -> String {
    match moa_final_target_chars() {
        0 => "Use all internal evidence needed. Deliver a complete answer in the format and \
              level of detail the user requested. Be direct and non-repetitive; remove \
              repetition before evidence, caveats, code, or required detail. Do not shorten \
              merely to meet an unstated length."
            .to_string(),
        target => format!(
            "Use all internal evidence needed, but shape the delivered answer rather than \
             constraining the panel's reasoning. Aim for at most about {target} characters \
             unless completeness or an explicit user format requires more; remove repetition \
             before removing evidence, caveats, or required detail."
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
    format!("{head}\n\n[MOA draft truncated before synthesis: {omitted} chars omitted]")
}

/// The aggregator's task text: a synthesis instruction carrying the numbered
/// drafts to fuse. Intermediate layers synthesize *concise* material (kept
/// fast); only the final layer writes the full user-facing answer.
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
        "Merge the responses below into a single, stronger set of dense bullet points. Keep \
         what is correct, drop what is wrong, reconcile conflicts, and add what they missed. \
         This is intermediate material for a later step, not the final answer — no preamble, \
         no prose."
            .to_string()
    } else {
        format!(
            "Synthesize the single strongest answer to the problem above, drawing on the responses \
         below. Keep what is correct, discard what is wrong, reconcile conflicts, and cover \
         anything a single response missed. Answer the user directly and in full — do not \
         mention this synthesis step or the responses. {}",
            final_output_guidance()
        )
    };
    format!(
        "{instruction}\n\n{} independent responses:\n\n{}",
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
        "Merge the responses below into a single, stronger set of dense bullet points. Keep \
         what is correct, drop what is wrong, reconcile conflicts, and add what they missed. \
         This is intermediate material for a later step, not the final answer — no preamble, \
         no prose. If the inputs conflict, state both positions; do not resolve that conflict \
         at this stage."
            .to_string()
    } else {
        format!(
            "Synthesize the single strongest answer to the problem above, drawing on the responses \
         below. Keep what is correct, discard what is wrong, reconcile conflicts, and cover \
         anything a single response missed. Answer the user directly and in full — do not \
         mention this synthesis step or the responses. {}",
            final_output_guidance()
        )
    };
    format!(
        "{instruction}\n\n{} independent responses represented by {} item(s):\n\n{}",
        total,
        drafts.len(),
        numbered_weighted_bounded(drafts, "Response", per_draft_chars, true)
    )
}

/// Keep the non-empty successful drafts, dropping workers that errored or
/// returned blank.
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

// ---------------------------------------------------------------------------
// Near-duplicate detection — local text similarity, zero model calls
// ---------------------------------------------------------------------------

/// Jaccard threshold above which two drafts count as the same answer.
/// The default (0.92) collapses near-identical drafts — proposers that
/// converged add nothing to the mixture while every surviving copy costs real
/// tokens in judge/synthesis payloads — and lets the free chooser skip fire
/// when all candidates agree. `ANGEL_MOA_DEDUP_SIM=0` disables it.
pub(crate) fn dedup_similarity() -> f64 {
    std::env::var("ANGEL_MOA_DEDUP_SIM")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(0.92)
}

/// Lowercased word 3-gram shingle set, hashed for cheap set ops.
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

/// Whether two drafts are near-identical: word-3-gram Jaccard at or above
/// `threshold`. Texts too short to shingle fall back to normalized equality.
pub(crate) fn near_identical_at(a: &str, b: &str, threshold: f64) -> bool {
    if threshold <= 0.0 {
        return false; // dedup disabled
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

/// [`near_identical_at`] with the env-configured threshold.
pub(crate) fn near_identical(a: &str, b: &str) -> bool {
    near_identical_at(a, b, dedup_similarity())
}

/// Whether every candidate is near-identical to the first — when they all
/// converged, any pick is the same answer and a chooser call buys nothing.
pub(crate) fn all_near_identical(candidates: &[String]) -> bool {
    let threshold = dedup_similarity();
    candidates
        .split_first()
        .map(|(first, rest)| rest.iter().all(|c| near_identical_at(first, c, threshold)))
        .unwrap_or(true)
}

/// Collapse near-duplicate drafts, keeping the first of each group (stable
/// persona order). Every duplicate that survives to a later stage costs real
/// tokens in judge/synthesis payloads while adding nothing to the mixture —
/// this drops them with pure local compute, no model calls. O(n²) pairwise on
/// a fan-out of ≤6 drafts.
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

/// First two numbers on a line → `(index, score)`, e.g. `"2: 8.5"` → `(2, 8.5)`.
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

/// Parse one reviewer's score sheet into `(draft_index, score)` pairs: 0-based,
/// in range, and the first score per draft wins (a reviewer who double-scores a
/// draft doesn't get two votes).
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

/// Pull up to `max` numeric runs (digits / `.`) from a line, in order.
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

/// Parse one reviewer's per-dimension sheet — `index` then one 0–10 score per
/// [`JUDGE_DIMS`] entry, in that order — into `(draft_index, weighted_composite)`.
/// A line missing any dimension is skipped; first row per draft wins (a reviewer
/// who double-scores a draft doesn't get two votes), matching `parse_scores`.
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

/// Fuse a panel of reviewers into one ranking: each draft is scored by its
/// *median* across the reviewers that scored it, then sorted strongest-first.
/// Ties keep draft order (the input is grouped by ascending index), so the
/// result is deterministic regardless of reviewer arrival order.
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

/// Median of a score list (empty → 0.0, but panel callers never pass empty).
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

/// First run of digits in `text` parsed as a number.
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

/// Whether the verifier signalled the answer is clean.
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

/// Truthy-env flag with an explicit default (so `MAX` can default a knob on while
/// an explicit `=0`/`false` still turns it off).
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

/// Which personas are allowed to request verifying tests when delegation is on.
pub(crate) fn is_delegator(key: &str) -> bool {
    matches!(
        key.split('+').next().unwrap_or(key),
        "red-team" | "empiricist"
    )
}
