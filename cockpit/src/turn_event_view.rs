use crate::glyphs::Glyph;
use crate::media::Media;

pub fn route_label(route: &crate::club::RouteIdentity) -> String {
    let mut label = match route.model.as_deref() {
        Some(model) if !model.eq_ignore_ascii_case(&route.driver) => {
            format!("{}/{model}", route.driver)
        }
        Some(model) => model.to_string(),
        None => route.driver.clone(),
    };
    if let Some(effort) = route.reasoning_effort.as_deref() {
        label.push('@');
        label.push_str(effort);
    }
    label
}

fn same_route(a: &crate::club::RouteIdentity, b: &crate::club::RouteIdentity) -> bool {
    a.driver.eq_ignore_ascii_case(&b.driver)
        && match (a.model.as_deref(), b.model.as_deref()) {
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
            (None, None) => true,
            _ => false,
        }
        && match (a.reasoning_effort.as_deref(), b.reasoning_effort.as_deref()) {
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
            (None, None) => true,
            _ => false,
        }
}

fn compact_duration(ms: u64) -> String {
    if ms < 1_000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else {
        format!("{}m{:02}s", ms / 60_000, (ms / 1_000) % 60)
    }
}

/// Text-only provenance receipt placed beside a user-visible answer. It carries
/// route metadata, first-output latency, and total elapsed time, never
/// prompt/response content or secrets.
pub fn answer_receipt_text(
    requested: &crate::club::RouteIdentity,
    resolved: &crate::club::RouteIdentity,
    first_output_ms: u64,
    total_ms: u64,
) -> String {
    let route = if same_route(requested, resolved) {
        route_label(resolved)
    } else {
        format!(
            "{} → {} · fallback",
            route_label(requested),
            route_label(resolved)
        )
    };
    let first_output_ms = first_output_ms.min(total_ms);
    format!(
        "{route} · first {} · total {}",
        compact_duration(first_output_ms),
        compact_duration(total_ms)
    )
}

pub fn tool_call_text(name: &str, args_summary: &str) -> String {
    format!("{} {name}({args_summary})", Glyph::Tool.token())
}

pub fn tool_result_text(name: &str, summary: &str) -> String {
    format!("  {} {name}: {summary}", Glyph::Result.token())
}

pub fn notice_text(note: &str) -> String {
    format!("{} {note}", Glyph::Idle.token())
}

/// Repeat count at which a gauge row's background fill saturates. The fill is
/// a visual severity meter: a guard firing this many times in one turn has the
/// operator's full attention (and the harness treadmill stop is already near).
pub const NOTICE_GAUGE_FULL: usize = 10;

/// One-slot transcript row for a repeating guard notice: the *latest* text
/// plus a running repeat count. Repeats rewrite the row in place instead of
/// stacking; the draw layer turns the count into a progressive background
/// fill.
pub fn notice_gauge_text(note: &str, count: usize) -> String {
    format!("{} {note} ×{count}", Glyph::Idle.token())
}

/// The notice body of an Activity row (glyph prefix stripped, gauge count
/// suffix removed), or `None` for non-notice rows (trace `T:`/`R:` lines,
/// tool tallies).
pub fn activity_notice_body(text: &str) -> Option<&str> {
    let body = text.strip_prefix(Glyph::Idle.token())?.trim_start();
    match activity_gauge_count(body) {
        Some(_) => body.rsplit_once(" ×").map(|(base, _)| base),
        None => Some(body),
    }
}

/// Parse the repeat count off a gauge row (`… ×N`, N ≥ 2). Plain rows and
/// first occurrences carry no suffix and return `None`.
pub fn activity_gauge_count(text: &str) -> Option<usize> {
    let (_, suffix) = text.rsplit_once(" ×")?;
    let digits = suffix.split(" · ").next()?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse::<usize>().ok().filter(|count| *count >= 2)
}

/// Parsed receipt state lives in the row itself, so scrollback eviction and
/// transcript replacement cannot leave a stale index or unbounded side table.
pub(crate) struct ActionReceipt<'a> {
    pub head: &'a str,
    pub key: String,
    pub last: u128,
    pub min: u128,
    pub max: u128,
    pub reason: Option<&'a str>,
}

pub(crate) fn action_receipt(note: &str) -> Option<ActionReceipt<'_>> {
    let mut parts = note.split(" · ");
    if parts.next()? != "action receipt" {
        return None;
    }
    let head = parts.next()?.split(" ×").next()?;
    let (tool, verb) = head.split_once(' ')?;
    let class = [
        "dispatch error",
        "applied",
        "ran",
        "denied",
        "timeout",
        "verifier",
    ]
    .into_iter()
    .find(|class| verb == *class || verb.starts_with(&format!("{class} ")))?;
    let timing = parts.next()?;
    let last = timing.strip_prefix("last ").unwrap_or(timing);
    let last = last.strip_suffix(" ms")?.parse().ok()?;
    let mut receipt = ActionReceipt {
        head,
        key: format!("receipt:{tool}:{class}"),
        last,
        min: last,
        max: last,
        reason: None,
    };
    if let Some(next) = parts.next() {
        if let Some((min, max)) = next.strip_suffix(" ms").and_then(|s| s.split_once('–')) {
            receipt.min = min.parse().ok()?;
            receipt.max = max.parse().ok()?;
            receipt.reason = parts.next();
        } else {
            receipt.reason = Some(next);
        }
    }
    Some(receipt)
}

pub(crate) fn receipt_gauge_text(previous: &str, note: &str) -> Option<String> {
    let old = action_receipt(previous.strip_prefix(Glyph::Idle.token())?.trim_start())?;
    let new = action_receipt(note)?;
    if old.key != new.key {
        return None;
    }
    let count = activity_gauge_count(previous)
        .unwrap_or(1)
        .saturating_add(1);
    let head = notice_gauge_text(&format!("action receipt · {}", new.head), count);
    let reason = new
        .reason
        .map(|reason| format!(" · {reason}"))
        .unwrap_or_default();
    Some(format!(
        "{head} · last {} ms · {}–{} ms{reason}",
        new.last,
        old.min.min(new.last),
        old.max.max(new.last)
    ))
}

/// Routine harness murmurs ride the live tool strip's note line instead of
/// stacking in the scrollback (Conversation mode). Anything unrecognized — and
/// anything that reads as a failure — keeps its transcript line, so a new or
/// misbehaving gate can never go silent (`/trace` always restores the full
/// stream).
pub fn notice_rides_strip(note: &str) -> bool {
    // "denying" is listed beside "denied": the verification gate phrases itself
    // in the present tense, slipped past the failure marks on that technicality,
    // and so spent its life collapsed into the strip's "(N notes)" counter —
    // retracting finished answers with no visible reason.
    // "timed out" is timeout_note's spelling; "timed-out" is the original
    // fail-open spec, and "timeout"/"timeouts" catch idle-timeout receipts
    // that never say "timed out". Present-tense "failing" is the same hole.
    const FAILURE_MARKS: [&str; 12] = [
        "error",
        "failed",
        "failing",
        "denied",
        "denying",
        "rejected",
        "discarded",
        "vanished",
        "timed out",
        "timed-out",
        "timeout",
        "timeouts",
    ];
    if FAILURE_MARKS.iter().any(|mark| note.contains(mark)) {
        return false;
    }
    // Operator decision (2026-08-26): Conversation scrollback is for the
    // conversation. Every routine harness/policy murmur — capsule previews,
    // cadence arming, verifier one-shots, broker/recall/cache receipts —
    // rides the strip, where mixed families stay visible via
    // `notice_strip_prefix` and `/trace` restores the full stream. The
    // 2026-08-25 wave moved these into scrollback so its own loop could see
    // them; that buried real answers under internal telemetry. Failures
    // (checked above) always keep their transcript line.
    const ROUTINE_PREFIXES: [&str; 27] = [
        "action capsule",
        "action receipt",
        "first-write guard",
        "no-edit answer guard",
        "assistant announced work",
        "assistant printed raw tool markup",
        "post-green",
        "green verifier",
        "self-authored verifier",
        "effort gate",
        "steer delivered",
        "trimmed ",
        "recalled ",
        "knowledge broker",
        "compacting context",
        "context compacted",
        "compaction summarizing",
        "ctx:",
        "final-mile",
        "peripheral fan-out",
        "mutation thrash",
        "task acceptance",
        "competition action cadence armed",
        "cache-first: compaction budget",
        "cache ledger:",
        // Turn-boundary cache-stable receipt (2026-09-05): it arrives after the
        // answer finished streaming; in scrollback it flushed the draft and the
        // committed answer then rendered a second time.
        "cache-stable:",
        // Sliding-window duplicate suppressions: one in-place strip line plus
        // "(N notes)", not a wall of identical `.. storm:` scrollback rows.
        "storm:",
    ];
    ROUTINE_PREFIXES
        .iter()
        .any(|prefix| note.starts_with(prefix))
}

/// Compact category for a strip note: colon-head when present, else the first
/// token. Mixed murmurs share one row, so the operator needs a label shorter
/// than the latest full sentence.
pub fn notice_strip_prefix(note: &str) -> &str {
    let note = note.trim();
    if let Some(head) = note.split_once(':').map(|(head, _)| head.trim())
        && !head.is_empty()
    {
        return head;
    }
    note.split_whitespace().next().unwrap_or(note)
}

/// Stable category for a repeatable cadence guard. Dynamic verdict suffixes
/// deliberately share one key: the transcript needs one visible receipt for
/// the guard, while the live strip carries its latest exact state and count.
pub fn notice_coalesce_key(note: &str) -> Option<&'static str> {
    if note.starts_with("unproductive streak:") {
        Some("unproductive-streak")
    } else if note.starts_with("in-flight idle failure (") {
        Some("in-flight-idle")
    } else if note.starts_with("always-be-improving: prepped candidate is sitting (") {
        Some("prepped-candidate-sitting")
    } else if note.starts_with("runner waste: local preflight required") {
        Some("runner-preflight")
    } else if note.starts_with("passive wait blocked") {
        Some("passive-wait")
    } else {
        None
    }
}

/// Sub-agent tools whose results are conversation, not chrome: their summaries
/// surface as `agents` blocks in the transcript.
pub fn is_council_tool(name: &str) -> bool {
    matches!(name, "spawn" | "delegate" | "integrate")
}

pub fn media_card(kind: &str, label: &str, url: &str) -> Media {
    match kind {
        "image" => Media::Image {
            label: label.to_string(),
            path: url.to_string(),
        },
        "video" => Media::Video {
            label: label.to_string(),
            path: url.to_string(),
        },
        "graph" => Media::Graph {
            label: label.to_string(),
            url: url.to_string(),
        },
        "resource" => Media::Resource {
            label: label.to_string(),
            url: url.to_string(),
        },
        _ => Media::Link {
            label: label.to_string(),
            url: url.to_string(),
        },
    }
}

pub fn media_delivery_text(label: &str, preview_result: Result<String, String>) -> String {
    match preview_result {
        Ok(msg) if !msg.is_empty() => {
            format!(
                "{} delivered to carousel: {label} · {msg}",
                Glyph::Transfer.token()
            )
        }
        Ok(_) => format!("{} delivered to carousel: {label}", Glyph::Transfer.token()),
        Err(e) => format!(
            "{} delivered to carousel: {label} · {} preview unavailable: {e}",
            Glyph::Transfer.token(),
            Glyph::Warning.token()
        ),
    }
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/turn_event_view__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/cockpit/app/turn_event_view__receipt_tests.rs"]
mod receipt_tests;
