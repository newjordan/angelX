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
mod tests {
    use super::*;

    #[test]
    fn gate_denials_keep_their_transcript_line() {
        // Present-tense "denying" used to miss the failure marks and match the
        // routine prefix instead, so a retracted answer only ever showed up in
        // the strip's collapsed "(N notes)" counter.
        assert!(!notice_rides_strip(
            "edited workspace is not yet verified; denying unsupported completion (2/2)"
        ));
        assert!(!notice_rides_strip(
            "no-edit answer guard: denied completion without a workspace mutation"
        ));
        assert!(!notice_rides_strip(
            "verification gate released by operator; accepting the completion unverified"
        ));
        // Routine policy murmurs ride the strip; a failure mark always wins
        // over a routine prefix (capsule *denied* keeps its transcript line).
        assert!(notice_rides_strip(
            "action capsule · 3 scoped operation(s) ready · local preview only, no model call"
        ));
        assert!(!notice_rides_strip(
            "action capsule denied · 3 scoped operation(s) were not dispatched"
        ));
        assert!(notice_rides_strip(
            "action receipt · write_file applied · 12 ms"
        ));
        assert!(notice_rides_strip("context compacted"));
        assert!(notice_rides_strip(
            "competition action cadence armed (trigger: competition-loop) — mutate + local preflight → SUBMIT → watcher owns the slot → improve next candidate"
        ));
        assert!(notice_rides_strip(
            "final-mile reserve active with 6 bounded hop(s) remaining"
        ));
        assert!(notice_rides_strip(
            "cache-first: compaction budget 120k -> 333k on deepseek"
        ));
        assert!(notice_rides_strip(
            "cache-first: compaction budget 120k -> 333k on deepseek-v4-pro"
        ));
        assert!(notice_rides_strip(
            "knowledge broker selected 2 source(s); 1 omitted"
        ));
        assert!(notice_rides_strip(
            "recalled notes skipped: no complete note fits the active context budget"
        ));
        assert!(notice_rides_strip(
            "recalled 3 note(s) from long-term memory"
        ));
        assert!(notice_rides_strip(
            "recalled 2 note(s) from long-term memory; 1 omitted to fit context"
        ));
        assert!(notice_rides_strip(
            "cache ledger: glm hop 2 hit 90% → 30% after defs delta + steer injection"
        ));
        assert!(notice_rides_strip(
            "cache ledger: deepseek-v4-pro hop 11 hit 88% → 68%"
        ));
        assert!(notice_rides_strip(
            "storm: suppressed duplicate shell call (x3)"
        ));
        assert!(notice_rides_strip(
            "trimmed 3 recent tool result(s) to fit the active context window"
        ));
        assert!(notice_rides_strip("compacting context…"));
        assert!(notice_rides_strip(
            "compaction summarizing on local deepseek (in-hand deepseek stays free)"
        ));
        assert!(notice_rides_strip(
            "effort gate: high withheld; model has no reasoning_effort"
        ));
        assert!(notice_rides_strip(
            "task acceptance command was already green; deterministic auto-completion disabled"
        ));
        assert!(notice_rides_strip(
            "green verifier achieved; continue any remaining requested work"
        ));
        assert!(notice_rides_strip(
            "post-green guard disarmed: the workspace changed after the green"
        ));
        assert!(notice_rides_strip(
            "self-authored verifier guard: green check only covers agent-written tests"
        ));
        assert!(notice_rides_strip("post-green grace batch 1/2"));
        assert!(notice_rides_strip(
            "first-write guard: 3 inspection call(s); mutation or board wait/poll required next"
        ));
        assert!(notice_rides_strip("peripheral fan-out"));
        assert!(notice_rides_strip("mutation thrash"));
        assert_eq!(
            notice_strip_prefix("storm: suppressed duplicate shell call (x3)"),
            "storm"
        );
        assert_eq!(
            notice_strip_prefix("trimmed 3 recent tool result(s) to fit the active context window"),
            "trimmed"
        );
        assert_eq!(notice_strip_prefix("context compacted"), "context");
    }

    #[test]
    fn cache_stable_boundary_receipt_rides_the_strip() {
        assert!(notice_rides_strip(
            "cache-stable: history rewrites held across 1 hop(s) to keep the request prefix \
             byte-stable; at the turn boundary aged 0 result(s) (−0 bytes), shrank 0 tool-call \
             argument(s) (−0 bytes), and deduped 0 duplicate result(s) (−0 bytes)"
        ));
    }

    #[test]
    fn notice_rides_strip_keeps_timeout_notices() {
        // "timed out" (space) was already a failure mark. Hyphenated
        // "timed-out", "timeout"/"timeouts", and present-tense "failing"
        // still matched a routine prefix and vanished into the strip.
        let timeout_note = crate::harness::timeout_note(120, None);
        assert_eq!(
            timeout_note,
            "\n[timed out after 120s — process killed; raise/disable via ANGEL_TOOL_TIMEOUT]"
        );
        assert!(!notice_rides_strip(&timeout_note));
        assert!(!notice_rides_strip(
            "background model compaction via deepseek timed out; the local latency guard will compact if needed"
        ));
        assert!(!notice_rides_strip(
            "turn idle timeout (1s with no stream progress)"
        ));
        assert!(!notice_rides_strip(
            "[angel-hook-blocked/v1] tool 'reverse' blocked by PreToolUse hook: hook timed out"
        ));
        assert!(!notice_rides_strip("code_mode: execution timed out"));
        assert!(!notice_rides_strip("acceptance check kept failing"));
        // Routine prefixes stay on the strip unless they also carry a failure mark.
        assert!(notice_rides_strip(
            "trimmed 3 recent tool result(s) to fit the active context window"
        ));
        assert!(notice_rides_strip("compacting context…"));
        assert!(notice_rides_strip(
            "compacting context locally (model-free latency guard)…"
        ));
        assert!(notice_rides_strip(
            "compaction summarizing on local deepseek (in-hand deepseek stays free)"
        ));
        assert!(notice_rides_strip(
            "storm: suppressed duplicate shell call (x3)"
        ));
        assert!(!notice_rides_strip(&format!(
            "trimmed 3 recent tool result(s) to fit the active context window{timeout_note}"
        )));
        assert!(!notice_rides_strip(
            "ctx: club=deepseek window=unknown budget=333000; turn idle timeout (1s with no stream progress)"
        ));
        assert!(!notice_rides_strip("storm: acceptance check kept failing"));
        assert!(!notice_rides_strip(
            "compaction summarizing on local deepseek (in-hand deepseek stays free); timed-out"
        ));
    }

    #[test]
    fn only_known_repeatable_cadence_failures_have_coalesce_keys() {
        let recon =
            notice_coalesce_key("in-flight idle failure (FailReconThrash); watcher owns status");
        let poll =
            notice_coalesce_key("in-flight idle failure (FailPollOnly); watcher owns status");
        assert_eq!(recon, Some("in-flight-idle"));
        assert_eq!(
            poll, recon,
            "changing verdicts share one transcript receipt"
        );
        assert_eq!(
            notice_coalesce_key("runner waste: local preflight required before runner dispatch"),
            Some("runner-preflight")
        );
        assert_eq!(
            notice_coalesce_key(
                "edited workspace is not yet verified; denying unsupported completion (1/2)"
            ),
            None
        );
        assert_eq!(
            notice_coalesce_key("provider error: connection reset"),
            None
        );
        assert_eq!(
            notice_coalesce_key(
                "passive wait blocked: status/sleep calls were not started; \
                 advance the candidate before checking again"
            ),
            Some("passive-wait")
        );
    }

    #[test]
    fn gauge_rows_round_trip_count_and_body() {
        let note = "passive wait blocked: status/sleep calls were not started";
        let row = notice_gauge_text(note, 7);
        assert!(row.ends_with("×7"), "{row}");
        assert_eq!(activity_gauge_count(&row), Some(7));
        assert_eq!(activity_notice_body(&row), Some(note));
        // First occurrences carry no suffix; body still extracts.
        let first = notice_text(note);
        assert_eq!(activity_gauge_count(&first), None);
        assert_eq!(activity_notice_body(&first), Some(note));
        // Non-notice activity rows (trace lines, tallies) are never gauges.
        assert_eq!(activity_notice_body("T: read(file=a)"), None);
        assert_eq!(
            activity_gauge_count("⚒ 14 tools · shell×9 read×3 · 38s"),
            None
        );
        assert_eq!(activity_gauge_count(".. storm note (x3)"), None);
    }

    #[test]
    fn formats_tool_activity_lines() {
        assert_eq!(tool_call_text("read", "file=a"), "T: read(file=a)");
        assert_eq!(tool_result_text("read", "ok"), "  R: read: ok");
        assert_eq!(notice_text("done"), ".. done");
    }

    #[test]
    fn answer_receipt_is_exact_and_marks_resolved_failover() {
        let requested = crate::club::RouteIdentity {
            driver: "openai".to_string(),
            model: Some("gpt-5.6-sol".to_string()),
            reasoning_effort: Some("ultra".to_string()),
        };
        assert_eq!(
            answer_receipt_text(&requested, &requested, 240, 1_250),
            "openai/gpt-5.6-sol@ultra · first 240ms · total 1.2s"
        );
        let resolved = crate::club::RouteIdentity {
            driver: "practice".to_string(),
            model: None,
            reasoning_effort: None,
        };
        assert_eq!(
            answer_receipt_text(&requested, &resolved, 61_234, 61_234),
            "openai/gpt-5.6-sol@ultra → practice · fallback · first 1m01s · total 1m01s"
        );
        assert_eq!(
            answer_receipt_text(&requested, &requested, 2_000, 1_250),
            "openai/gpt-5.6-sol@ultra · first 1.2s · total 1.2s"
        );
    }

    #[test]
    fn builds_media_cards_by_kind() {
        assert!(matches!(
            media_card("image", "avatar", "/tmp/a.png"),
            Media::Image { .. }
        ));
        assert!(matches!(
            media_card("graph", "plot", "http://x"),
            Media::Graph { .. }
        ));
        assert!(matches!(
            media_card("resource", "doc", "http://x"),
            Media::Resource { .. }
        ));
        assert!(matches!(
            media_card("other", "link", "http://x"),
            Media::Link { .. }
        ));
    }

    #[test]
    fn formats_media_delivery_preview_status() {
        assert_eq!(
            media_delivery_text("card", Ok("showing image".to_string())),
            "<> delivered to carousel: card · showing image"
        );
        assert_eq!(
            media_delivery_text("card", Ok(String::new())),
            "<> delivered to carousel: card"
        );
        assert_eq!(
            media_delivery_text("card", Err("missing".to_string())),
            "<> delivered to carousel: card · W! preview unavailable: missing"
        );
    }
}

#[cfg(test)]
mod receipt_tests {
    use super::*;
    #[test]
    fn receipt_keys_range_and_latest_reason() {
        let mut row =
            notice_text("action receipt · shell dispatch error · 2694 ms · helper: ENOENT");
        for ms in [43, 1503] {
            row = receipt_gauge_text(
                &row,
                &format!(
                    "action receipt · shell dispatch error · {ms} ms · sandbox: mount not confined"
                ),
            )
            .unwrap();
        }
        assert_eq!(
            row,
            ".. action receipt · shell dispatch error ×3 · last 1503 ms · 43–2694 ms · sandbox: mount not confined"
        );
        for class in [
            "applied",
            "ran",
            "dispatch error",
            "denied",
            "timeout",
            "verifier failed",
        ] {
            let note = format!("action receipt · shell {class} · 4 ms");
            let parsed = action_receipt(&note).unwrap();
            assert_eq!(
                parsed.key,
                format!(
                    "receipt:shell:{}",
                    if class == "verifier failed" {
                        "verifier"
                    } else {
                        class
                    }
                )
            );
        }
        assert!(receipt_gauge_text(&row, "action receipt · shell ran · 2 ms").is_none());
        assert!(action_receipt("action receipt · shell ran · bad ms").is_none());
        println!(
            "receipt key classes=6; latest=1503 ms min=43 ms max=2694 ms; latest reason retained"
        );
    }
}
