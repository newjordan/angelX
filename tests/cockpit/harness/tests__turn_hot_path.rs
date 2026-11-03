//! Manual 200-hop hot-path measurement.
//!
//! `cargo test --profile fast turn_hot_path -- --ignored --nocapture`
//!
//! The timings are diagnostic, not a gate. They keep the history-growing work
//! on a model-hop boundary visible: request assembly, every full-history token
//! estimate, the background-output scan, the continuity handoff, and the
//! per-attempt session checkpoint. The request-bytes test beside it is a real
//! gate: it pins the wire prefix so a latency fix can never silently spend the
//! provider's prefix cache.

use super::*;
use crate::agent::club::HttpClub;
use crate::agent::harness::auxiliary::AuxiliaryTracker;
use crate::agent::harness::book::continuity::Continuity;
use crate::agent::harness::book::introduction;
use crate::agent::harness::turn::background;
use std::hint::black_box;

const HOT_HOPS: usize = 200;
const HOT_ITERS: usize = 8;

fn hot_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "read_file".into(),
            description: "Read a workspace file, numbered, with its book stamps. ⠇⠙".into(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "workspace-relative path"},
                    "offset": {"type": "integer"},
                    "limit": {"type": "integer"}
                },
                "required": ["path"]
            }),
        },
        ToolDef {
            name: "str_replace".into(),
            description: "Replace one exact snippet in a workspace file. ⠇⠑".into(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "old": {"type": "string"},
                    "new": {"type": "string"}
                },
                "required": ["path", "old", "new"]
            }),
        },
        ToolDef {
            name: "shell".into(),
            description: "Run one shell command in the sandbox. ⠇⠎".into(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                    "timeout_ms": {"type": "integer"}
                },
                "required": ["command"]
            }),
        },
    ]
}

/// A ~4 KiB inspection result carrying the trailing book receipt real tool
/// results wear (the byte accounting subtracts it).
fn hot_result(hop: usize) -> String {
    use std::fmt::Write as _;
    let mut body = String::with_capacity(4 * 1024 + 64);
    for line in 0..64 {
        let _ = writeln!(
            body,
            "src/module_{hop}.rs:{}: pub(crate) fn item_{line}() -> usize {{ {hop} + {line} }}",
            line + 1
        );
    }
    body.push('\n');
    body.push_str(&book::d467_receipts::MIDDLE_LINES.cells());
    body
}

fn hot_history(hops: usize) -> Vec<ChatMsg> {
    let mut history = vec![
        ChatMsg::system("Harness policy preamble, permissions, and workspace rules. ".repeat(40)),
        ChatMsg::user("Walk the modules, make the change, and run the focused tests."),
    ];
    for hop in 0..hops {
        history.push(ChatMsg::assistant_calls(vec![ToolCall {
            id: format!("call-{hop}"),
            name: "read_file".into(),
            args: serde_json::json!({ "path": format!("src/module_{hop}.rs") }),
        }]));
        history.push(ChatMsg::tool(format!("call-{hop}"), hot_result(hop)));
        history.push(ChatMsg::assistant(format!(
            "hop {hop}: module_{hop} read; item {hop} sits in the dispatch table. \
             Next I will patch it and rerun the suite."
        )));
    }
    history
}

fn time_stage(label: &str, iters: usize, mut op: impl FnMut()) {
    op();
    let started = Instant::now();
    for _ in 0..iters {
        op();
    }
    let per_op = started.elapsed().as_secs_f64() * 1000.0 / iters as f64;
    eprintln!("TURN_HOT_PATH {label:<46} {per_op:>9.3} ms/op");
}

#[test]
fn no_op_boundary_passes_do_not_rescan_background_outputs() {
    let _guard = crate::tests::env_lock();
    let _sync = EnvGuard::set("ANGEL_COMPACT_SYNC_LLM", "0");
    let mut history = hot_history(4);
    let tools = hot_tools();
    let meter = background::Meter::default();
    let _scope = meter.enter(Instant::now());
    for hop in 0..4 {
        let bytes = hot_result(hop).len() as u64;
        background::produced(&format!("call-{hop}"), bytes, bytes);
    }
    let (events, _rx) = mpsc::channel();
    let registry = ToolRegistry::new();
    let summarizer = SummarizerClub("unused");
    let ((), scans) = background::count_observe_scans(|| {
        prune_history(&mut history, 0);
        fit_tool_results_to_budget(&mut history, 10_000_000, &tools);
        maybe_compact_for_turn(
            &summarizer,
            &mut history,
            10_000_000,
            20,
            20_000,
            &tools,
            &registry,
            &events,
        );
        maybe_start_bg_compact(
            &mut history,
            10_000_000,
            20,
            20_000,
            &tools,
            &registry,
            &events,
        );
        try_splice_bg_compact(&registry, &mut history, &events);
    });
    assert_eq!(
        scans, 0,
        "under-budget/no-op boundary passes must not rescan every tool output"
    );
}

#[test]
#[ignore = "manual 200-hop hot-path measurement; timings are diagnostic, not a gate"]
fn turn_hot_path_200_hop_measurement() {
    let _guard = crate::tests::env_lock();
    let _sync = EnvGuard::set("ANGEL_COMPACT_SYNC_LLM", "0");
    let history = hot_history(HOT_HOPS);
    let tools = hot_tools();
    let content_bytes: usize = history.iter().map(|m| m.content.len()).sum();
    eprintln!(
        "TURN_HOT_PATH hops={HOT_HOPS} messages={} content_bytes={content_bytes} estimated_tokens={} tool_tokens={}",
        history.len(),
        estimate_tokens(&history),
        estimate_tool_tokens(&tools)
    );

    let club = HttpClub::new(
        "hotpath",
        "https://api.example.com/v1",
        "fixture-model",
        Some("test-key".into()),
    );
    time_stage("history.clone()", HOT_ITERS, || {
        black_box(history.clone());
    });
    time_stage("request build + serialize", HOT_ITERS, || {
        let body = club
            .build_body_with_effort(black_box(&history), &tools, false, None)
            .expect("request body");
        black_box(serde_json::to_vec(&body).expect("serialized request"));
    });
    time_stage("messages_to_json", HOT_ITERS, || {
        black_box(crate::agent::club::messages_to_json(
            black_box(&history),
            true,
        ));
    });

    let mut roll = HistoryTokenRoll::default();
    let settled = roll.recompute(&history);
    time_stage("token roll observe (steady)", HOT_ITERS, || {
        black_box(roll.observe(black_box(&history)));
    });
    time_stage("token roll recompute (full)", HOT_ITERS, || {
        black_box(roll.recompute(black_box(&history)));
    });
    time_stage("estimate_tokens (full)", HOT_ITERS, || {
        black_box(estimate_tokens(black_box(&history)));
    });
    assert_eq!(settled, estimate_tokens(&history));

    let mut fit_history = history.clone();
    time_stage("fit_tool_results under budget", HOT_ITERS, || {
        black_box(fit_tool_results_to_budget(
            black_box(&mut fit_history),
            10_000_000,
            &tools,
        ));
    });
    time_stage("prune_history (cap off)", HOT_ITERS, || {
        prune_history(black_box(&mut fit_history), 0);
    });

    let registry = ToolRegistry::new();
    let (events, _rx) = mpsc::channel();
    let club = SummarizerClub("unused");
    let mut compact_history = history.clone();
    time_stage("maybe_compact_for_turn under budget", HOT_ITERS, || {
        black_box(maybe_compact_for_turn(
            &club,
            black_box(&mut compact_history),
            10_000_000,
            20,
            20_000,
            &tools,
            &registry,
            &events,
        ));
    });
    time_stage("maybe_start_bg_compact under budget", HOT_ITERS, || {
        maybe_start_bg_compact(
            black_box(&mut compact_history),
            10_000_000,
            20,
            20_000,
            &tools,
            &registry,
            &events,
        );
    });

    let meter = background::Meter::default();
    let _meter_scope = meter.enter(Instant::now());
    for hop in 0..HOT_HOPS {
        let bytes = hot_result(hop).len() as u64;
        background::produced(&format!("call-{hop}"), bytes, bytes);
    }
    time_stage("background::observe (one call)", HOT_ITERS, || {
        background::observe(black_box(&history));
    });
    time_stage("background::observe x4", HOT_ITERS, || {
        for _ in 0..4 {
            background::observe(black_box(&history));
        }
    });

    let route = crate::agent::club::RouteIdentity {
        driver: "scripted".into(),
        model: Some("fixture-model".into()),
        reasoning_effort: None,
    };
    let mut continuity = Continuity::new(&history);
    let mut prepared = history.clone();
    time_stage("continuity.prepare", HOT_ITERS, || {
        black_box(continuity.prepare(black_box(&mut prepared), &route));
    });
    time_stage("introductions (request scan)", HOT_ITERS, || {
        black_box(introduction::introductions(black_box(&history), None));
    });

    let auxiliary = AuxiliaryTracker::default();
    let scope = auxiliary.enter();
    let mut seen = std::collections::HashSet::new();
    time_stage("auxiliary.observe_recovery_context", HOT_ITERS, || {
        scope.observe_recovery_context(black_box(&history), &mut seen);
    });

    let session_dir = scratch("turn_hot_path_session");
    let session = crate::knowledge::session::Session::at_for(
        session_dir.join("sessions"),
        "hotpath".into(),
        &session_dir,
    );
    let _ = std::fs::create_dir_all(session_dir.join("sessions"));
    time_stage("session checkpoint (full snapshot)", 3, || {
        session
            .checkpoint(black_box(&history))
            .expect("session checkpoint");
    });
    let mut snapshot = serde_json::to_value(&history).expect("history value");
    let serialized = crate::knowledge::session::prior_snapshot_bytes(&history);
    time_stage("session serialize history -> Value", 3, || {
        black_box(serde_json::to_value(black_box(&history)).expect("history value"));
    });
    time_stage("session redact_value(history)", 3, || {
        crate::platform::secrets::redact_value(black_box(&mut snapshot));
    });
    time_stage("session Value -> bytes", 3, || {
        black_box(serde_json::to_vec(black_box(&snapshot)).expect("history json"));
    });
    time_stage("session parse prior snapshot (SessionRecord)", 3, || {
        black_box(crate::knowledge::session::parse_prior_snapshot(black_box(
            &serialized,
        )));
    });
    let prior_path = session_dir.join("prior.json");
    std::fs::write(&prior_path, &serialized).expect("prior snapshot fixture");
    time_stage("session read + parse prior snapshot", 3, || {
        let bytes = std::fs::read(black_box(&prior_path)).expect("read prior snapshot");
        black_box(crate::knowledge::session::parse_prior_snapshot(black_box(
            &bytes,
        )));
    });
    time_stage("secret_values()", 3, || {
        black_box(crate::platform::secrets::secret_values());
    });

    let tiny_dir = scratch("turn_hot_path_session_tiny");
    let tiny_session = crate::knowledge::session::Session::at_for(
        tiny_dir.join("sessions"),
        "tiny".into(),
        &tiny_dir,
    );
    let _ = std::fs::create_dir_all(tiny_dir.join("sessions"));
    let tiny = vec![ChatMsg::user("one short turn")];
    tiny_session.checkpoint(&tiny).expect("prime tiny session");
    time_stage("session checkpoint (2 messages)", 3, || {
        tiny_session
            .checkpoint(black_box(&tiny))
            .expect("tiny checkpoint");
    });
    let _ = std::fs::remove_dir_all(&tiny_dir);
    let _ = std::fs::remove_dir_all(&session_dir);
}

/// Golden wire bytes for the synthetic request. The prefix cache is only worth
/// anything if a latency change leaves the serialized request untouched; if
/// this digest must move, that is a deliberate cache-invalidating decision.
/// Produced on base `2bdd1a9a709257075ac375fa5af18cbc648be56c`; regenerate by
/// running the test with `--nocapture` and pasting the printed `sha256`:
/// `cd cockpit && cargo test --profile fast turn_hot_path_request_bytes_stay_identical -- --nocapture`
#[test]
fn turn_hot_path_request_bytes_stay_identical() {
    const GOLDEN_SHA256: &str = "f17ea5c70d633ad155c2eaf68133cbbf380be381cae92a21736f64ebdbf8d508";
    let _guard = crate::tests::env_lock();
    let _caveman = EnvGuard::set("ANGEL_SOTA_CAVEMAN", "0");
    let _intro = EnvGuard::set("ANGEL_BOOK_INTRO", "1");
    let _cache = EnvGuard::set("ANGEL_PROMPT_CACHE", "0");
    let _usage = EnvGuard::set("ANGEL_STREAM_USAGE", "0");
    let history = hot_history(8);
    let tools = hot_tools();
    let club = HttpClub::new(
        "hotpath-golden",
        "https://api.example.com/v1",
        "fixture-model",
        Some("test-key".into()),
    );
    let body = club
        .build_body_with_effort(&history, &tools, true, None)
        .expect("request body");
    let wire = serde_json::to_vec(&body).expect("serialized request");
    let digest = crate::knowledge::cut::sha256_hex(&wire);
    eprintln!(
        "TURN_HOT_PATH request wire sha256={digest} bytes={} messages={}",
        wire.len(),
        body["messages"].as_array().map_or(0, Vec::len)
    );
    assert_eq!(
        digest, GOLDEN_SHA256,
        "the serialized request bytes changed; this invalidates the provider prefix cache"
    );
}
