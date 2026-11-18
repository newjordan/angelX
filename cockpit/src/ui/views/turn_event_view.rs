use crate::ui::glyphs::Glyph;
use crate::ui::media::Media;

pub fn route_label(route: &crate::agent::club::RouteIdentity) -> String {
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

fn same_route(
    a: &crate::agent::club::RouteIdentity,
    b: &crate::agent::club::RouteIdentity,
) -> bool {
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

pub fn answer_receipt_text(
    requested: &crate::agent::club::RouteIdentity,
    resolved: &crate::agent::club::RouteIdentity,
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

pub const NOTICE_GAUGE_FULL: usize = 10;

pub fn notice_gauge_text(note: &str, count: usize) -> String {
    format!("{} {note} ×{count}", Glyph::Idle.token())
}

pub fn activity_notice_body(text: &str) -> Option<&str> {
    let body = text.strip_prefix(Glyph::Idle.token())?.trim_start();
    match activity_gauge_count(body) {
        Some(_) => body.rsplit_once(" ×").map(|(base, _)| base),
        None => Some(body),
    }
}

pub fn activity_gauge_count(text: &str) -> Option<usize> {
    let (_, suffix) = text.rsplit_once(" ×")?;
    let digits = suffix.split(" · ").next()?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse::<usize>().ok().filter(|count| *count >= 2)
}

pub fn notice_rides_strip(note: &str) -> bool {
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
        "cache-stable:",
        "storm:",
    ];
    ROUTINE_PREFIXES
        .iter()
        .any(|prefix| note.starts_with(prefix))
}

pub fn notice_strip_prefix(note: &str) -> &str {
    let note = note.trim();
    if let Some(head) = note.split_once(':').map(|(head, _)| head.trim())
        && !head.is_empty()
    {
        return head;
    }
    note.split_whitespace().next().unwrap_or(note)
}

pub fn notice_coalesce_key(note: &str) -> Option<&'static str> {
    if note.starts_with(&crate::agent::harness::book::l_loops::STREAK.cells()) {
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
#[path = "../../../../tests/cockpit/app/turn_event_view__tests.rs"]
mod tests;
