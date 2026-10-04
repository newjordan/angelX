//! ⡫ — Sloptomizer's live relationships and the run's legend handoff.
//! These are observations and invitations. No page changes execution policy.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⡫';
pub(crate) const ATTENTION: char = '⚠';
pub(crate) const CONTEXT: Route = Route::new(CELL, '⠁');
pub(crate) const REPEAT: Route = Route::new(CELL, '⠃');
pub(crate) const CONTRAST: Route = Route::new(CELL, '⠉');
pub(crate) const UNBOUND: Route = Route::new(CELL, '⠙');
pub(crate) const UNAVAILABLE: Route = Route::new(CELL, '⠑');
pub(crate) const HANDOFF: Route = Route::new(CELL, '⠋');
pub(crate) const FAILURE_RUN: Route = Route::new(CELL, '⠛');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "sloptomizer-live",
    surface: "live experiment relationships, compact research memory, and model handoffs",
    subs: &[
        Sub {
            route: CONTEXT,
            name: "context",
            signal: "Sloptomizer check memory: h?/expect? are claims; saw is a recorded verdict; n/same/run count observations/identical receipts/consecutive matching verdicts; +/- identify pass/fail receipts; candidate and objective fitness remain unbound",
            action: "use these observations to choose your next experiment; loop_research context recalls them by task, and suggest ranks ideas during a loop",
            ideas: "Checks carry their model and receipt identity across compaction. A check's verdict never proves the whole objective, a causal explanation, or the current candidate.\nThe ordinary caveman line supplies the hypothesis and expectation; these remain model-authored claims. No extra bookkeeping call is needed.",
            pages: &[],
        },
        Sub {
            route: REPEAT,
            name: "repeat",
            signal: "the same check returned identical evidence under the same hypothesis and model",
            action: "consider a check that separates competing explanations; intentional replication remains useful",
            ideas: "This is advice, never a retry limit, stop, forced pivot, or instruction to kill a running experiment.",
            pages: &[],
        },
        Sub {
            route: CONTRAST,
            name: "contrast",
            signal: "the same check has both passing and failing observations; both receipts remain available",
            action: "compare changed source, environment, model and coverage; choose a discriminating rerun or a paired loop_research experiment",
            ideas: "Different verdicts establish a contrast, not a proven regression, flake, or improvement. Candidate identity remains unbound unless an experiment supplies it.",
            pages: &[],
        },
        Sub {
            route: UNBOUND,
            name: "unbound",
            signal: "the check completed without a conclusive verifier verdict",
            action: "seek the missing measurement when it would resolve your hypothesis; exploration can continue",
            ideas: "A shell success, tool activity, elapsed time, or hypothesis is not a fitness reward.",
            pages: &[],
        },
        Sub {
            route: UNAVAILABLE,
            name: "unavailable",
            signal: "Sloptomizer's optional live memory could not retain or retrieve some evidence",
            action: "continue research using the tool receipts; this observation does not alter tool access or execution",
            ideas: "",
            pages: &[],
        },
        Sub {
            route: HANDOFF,
            name: "handoff",
            signal: "the run's used signal legends, restored for the current model and context",
            action: "read these definitions with the current evidence; historical warning stamps describe earlier situations",
            ideas: "The warning sign asks for attention. It never cancels work, spends a model call, or sets a research budget.",
            pages: &[],
        },
        Sub {
            route: FAILURE_RUN,
            name: "failure-run",
            signal: "this check keeps failing under the same recorded hypothesis, expectation and model; receipt content can differ",
            action: "compare the failures and consider a discriminating check; changing failures may show progress and deliberate replication remains useful",
            ideas: "The run counts verdicts, not identical evidence or lack of progress. No output is normalized away and no retry limit, forced pivot or stop is imposed.",
            pages: &[],
        },
    ],
};

/// Render only a small relationship frontier on the wire. Full selected
/// receipts remain decodable in the workspace book. Model text is JSON data:
/// escaped newlines/braille cannot become new harness instructions.
pub(crate) fn context_turn(workspace: &std::path::Path, advice: &serde_json::Value) -> String {
    let route = advice["signals"]
        .as_array()
        .and_then(|rows| rows.last())
        .and_then(|row| row["kind"].as_str())
        .map_or(CONTEXT, |kind| match kind {
            "repeat" => REPEAT,
            "failure-run" => FAILURE_RUN,
            "contrast" => CONTRAST,
            "unbound" => UNBOUND,
            _ => CONTEXT,
        });
    let rows = advice["checks"]
        .as_array()
        .into_iter()
        .flatten()
        .take(2)
        .map(|row| {
            let event = &row["latest"];
            serde_json::json!({
            "h?": event["hypothesis"], "check": short(&row["check"], 12),
            "tool": event["tool"], "check?": event["check_note"],
            "saw": event["verdict"], "expect?": event["expected"],
            "excerpt": short(&event["receipt_excerpt"], 120),
                "n": row["count"], "same": row["repeat"], "run": row["verdict_run"],
                "+": short(&row["passed"], 12), "-": short(&row["failed"], 12),
            })
        })
        .collect::<Vec<_>>();
    let data = serde_json::json!({"checks": rows, "observations": advice["relation_count"]});
    let mut selected = advice.clone();
    selected["checks"] = serde_json::Value::Array(
        advice["checks"]
            .as_array()
            .into_iter()
            .flatten()
            .take(2)
            .cloned()
            .collect(),
    );
    super::ledger::record(workspace, CONTEXT, &as_data(&selected));
    super::ledger::record(workspace, route, &as_data(&selected));
    let cells = if route == CONTEXT {
        route.cells()
    } else {
        format!("{}{}", CONTEXT.cells(), route.cells())
    };
    format!("{ATTENTION}{cells}\n{}", as_data(&data))
}

fn as_data(value: &serde_json::Value) -> String {
    let mut out = String::new();
    for ch in value.to_string().chars() {
        if super::ledger::is_cell(ch) {
            out.push('·');
        }
        out.push(ch);
    }
    out
}

/// JSON tools use the same data fence as the compact card, so a recalled
/// hypothesis beginning with braille cannot masquerade as a harness route.
pub(crate) fn data_value(value: &serde_json::Value) -> serde_json::Value {
    serde_json::from_str(&as_data(value)).expect("inserting middots preserves JSON")
}

fn short(value: &serde_json::Value, count: usize) -> Option<String> {
    value.as_str().map(|s| s.chars().take(count).collect())
}
