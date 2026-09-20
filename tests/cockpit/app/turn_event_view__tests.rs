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
    let poll = notice_coalesce_key("in-flight idle failure (FailPollOnly); watcher owns status");
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
