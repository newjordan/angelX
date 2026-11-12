use super::*;
use crate::agent::club::{ChatMsg, Club, ClubReply, StreamDelta, ToolDef};
use crate::agent::harness::tests::EnvGuard;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;

struct Root(PathBuf);
impl Root {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "angel-errors-leg-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("errors")).unwrap();
        Self(root)
    }
    fn module(&self) -> PathBuf {
        self.0.join("errors")
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The module exactly as shipped.
fn install_shipped(root: &Root) {
    let package = Path::new(env!("CARGO_MANIFEST_DIR")).join("live/errors");
    for entry in std::fs::read_dir(package).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), root.module().join(entry.file_name())).unwrap();
    }
}

/// A plain JavaScript leg, for answers the shipped policy would never give.
fn install_js(root: &Root, render: &str) {
    std::fs::write(
        root.module().join("module.json"),
        r#"{"api_version":1,"id":"errors","kind":"policy","entry":"index.js","probe":{}}"#,
    )
    .unwrap();
    std::fs::write(
        root.module().join("index.js"),
        format!("module.exports={{apiVersion:1,render:input=>{render}}}"),
    )
    .unwrap();
}

/// What the host does on a reload: publish the snapshot's legs to the registry.
fn publish(modules: &mut angel_live::Registry, registry: &ToolRegistry) {
    modules.refresh(true);
    registry.set_live_policies(modules.snapshot().policies);
}

fn live_registry(root: &Root) -> (angel_live::Registry, ToolRegistry) {
    let mut modules = angel_live::Registry::new(root.0.clone(), crate::agent::hotload::evaluate);
    let registry = ToolRegistry::new();
    publish(&mut modules, &registry);
    registry.pin_live_policies();
    (modules, registry)
}

fn context(attempt: usize, budget: Option<usize>) -> FailureContext {
    FailureContext {
        emitted_answer: false,
        output_cap_exhausted: false,
        attempt,
        budget,
        backoff_ms: 500,
        elapsed: Duration::from_secs(3),
    }
}

const QUOTA_429: &str = r#"HTTP 429: {"error":{"code":"1310","message":"Weekly/Monthly Limit Exhausted. Your limit will reset at 2026-07-03 11:33:39"}}"#;

/// Real error strings from the native classifier's own tests and the club's
/// error envelopes, grouped by the class the native policy gives them.
fn corpus() -> Vec<(&'static str, FailureClass)> {
    use FailureClass::*;
    let stalled = "stream stalled: server kept the connection alive but sent no data for 60s";
    let mut cases: Vec<(&'static str, FailureClass)> = vec![
        // Permanent: status, credentials, config, veto, certificate.
        ("HTTP 400: malformed messages", Permanent),
        ("HTTP 401: Unauthorized", Permanent),
        ("HTTP 402: payment required", Permanent),
        ("HTTP 403: invalid_api_key", Permanent),
        ("HTTP 404: model not found", Permanent),
        ("HTTP 406: unacceptable response format", Permanent),
        ("HTTP 415: unsupported media type", Permanent),
        ("HTTP 422: unsupported parameter", Permanent),
        ("HTTP 424: failed dependency", Permanent),
        ("HTTP 503 [x-should-retry: false]: overloaded", Permanent),
        ("HTTP 429 [x-should-retry: false]: rate limited", Permanent),
        (
            "HTTP 500 [x-should-retry: false]: request content",
            Permanent,
        ),
        ("HTTP 408 [x-should-retry: false]: read timeout", Permanent),
        ("HTTP 425 [x-should-retry: false]: ", Permanent),
        (
            "HTTP 529 [x-should-retry: false]: {\"error\":\"overloaded: retry later\"}",
            Permanent,
        ),
        (
            "transport error after 1 attempt(s): https://api.example/v1/chat/completions: Connection Failed: tls connection init failed: invalid peer certificate: UnknownIssuer",
            Permanent,
        ),
        (
            "muse responses: transport: Connection Failed: tls connection init failed: invalid peer certificate: NotValidForName",
            Permanent,
        ),
        (
            "tls connection init failed: invalid peer certificate: Expired",
            Permanent,
        ),
        (
            "transport error after 1 attempt(s): [permanent transport configuration] https://bad: invalid url",
            Permanent,
        ),
        (
            "HTTP 401 Unauthorized: invalid api key; upstream reported stream stalled: server kept the connection alive but sent no data for 60s",
            Permanent,
        ),
        (
            r#"api error: {"code":"1000","message":"身份验证失败"}"#,
            Permanent,
        ),
        ("does not exist or you do not have access to it", Permanent),
        ("unknown model gpt-9", Permanent),
        // Quota: a long window that retrying cannot clear.
        (QUOTA_429, Quota),
        ("HTTP 429: weekly quota exhausted", Quota),
        ("HTTP 429: monthly quota exhausted", Quota),
        (
            "quota exhausted (cooling down, next probe in 12m): Weekly limit",
            Quota,
        ),
        ("quota exhausted: 429 insufficient_quota", Quota),
        // Context overflow, which the turn compacts and rebuilds first.
        (
            "HTTP 413: context_length_exceeded: payload too large",
            Context,
        ),
        ("HTTP 413: payload too large", Context),
        ("context_length_exceeded", Context),
        ("maximum context length is 32768 tokens", Context),
        ("input token count exceeds the model limit", Context),
        ("request exceeds the context window", Context),
        (
            "HTTP 400: request (104239 tokens) exceeds the available context size (102400 tokens); type=exceed_context_size_error",
            Context,
        ),
        ("HTTP 500: exceed_context_size_error", Context),
        // Transient: everything the retry loop exists for.
        ("HTTP 408: timeout", Transient),
        ("HTTP 425: too early", Transient),
        ("HTTP 429: rate limited", Transient),
        ("HTTP 500: internal error", Transient),
        ("HTTP 502: bad gateway", Transient),
        ("HTTP 503: overloaded", Transient),
        ("HTTP 504: gateway timeout", Transient),
        ("HTTP 529: overloaded", Transient),
        (
            "HTTP 503: [x-should-retry: false] upstream temporarily busy",
            Transient,
        ),
        (
            "HTTP 503: upstream unavailable\nHTTP 503 [x-should-retry: false]: copied diagnostic",
            Transient,
        ),
        ("connection reset by peer", Transient),
        (
            "transport error after 2 attempt(s): Connection Failed: Connect error: Connection refused (os error 111)",
            Transient,
        ),
        (
            "transport error after 2 attempt(s): Dns Failed: resolve dns name 'api.example:443': failed to lookup address information",
            Transient,
        ),
        (
            "club returned an empty reply (no text and no tool calls)",
            Transient,
        ),
        ("club stream produced no data", Transient),
        ("context cache temporarily unavailable", Transient),
        ("provider returned empty stdout", Transient),
        ("response incomplete: max_output_tokens", Transient),
        (crate::agent::club::TRUNCATED_OUTPUT_ERR, Transient),
        (stalled, Transient),
    ];
    cases.push((
        Box::leak(
            format!(
                "{}: partial answer text",
                crate::agent::club::INCOMPLETE_STREAM_ERR
            )
            .into_boxed_str(),
        ),
        Transient,
    ));
    cases
}

fn contexts() -> Vec<FailureContext> {
    let mut all = Vec::new();
    for (answered, capped) in [(false, false), (true, false), (false, true), (true, true)] {
        for (attempt, budget) in [
            (0, None),
            (3, None),
            (0, Some(0)),
            (2, Some(3)),
            (3, Some(3)),
            (40, Some(3)),
        ] {
            for backoff_ms in [0, 500, 3_000] {
                all.push(FailureContext {
                    emitted_answer: answered,
                    output_cap_exhausted: capped,
                    attempt,
                    budget,
                    backoff_ms,
                    elapsed: Duration::from_millis(4_321),
                });
            }
        }
    }
    all
}

#[test]
fn native_verdict_names_each_corpus_case_by_the_class_the_turn_gave_it() {
    let cases = corpus();
    for (error, class) in &cases {
        let verdict = native_verdict(error, &context(0, None));
        assert_eq!(verdict.class, *class, "{error}");
        if matches!(class, FailureClass::Permanent | FailureClass::Quota) {
            assert!(!verdict.retry, "{error}");
            assert_eq!(verdict.delay_ms, 0, "{error}");
        }
    }
    // The ladder: base * (replays + 1), capped.
    let ladder: Vec<_> = [0, 1, 2, 3, 9]
        .iter()
        .map(|attempt| native_verdict("HTTP 503: overloaded", &context(*attempt, None)).delay_ms)
        .collect();
    assert_eq!(ladder, [500, 1_000, 1_500, 2_000, 2_000]);
    assert!(!native_verdict("HTTP 503: overloaded", &context(3, Some(3))).retry);
}

#[test]
fn the_shipped_bend_leg_reproduces_the_native_decision_over_the_corpus() {
    let root = Root::new("parity");
    install_shipped(&root);
    let (_modules, registry) = live_registry(&root);
    let digest = registry
        .live_policy("errors")
        .expect("shipped leg")
        .digest()
        .to_string();
    let (mut checked, mut replays) = (0, 0);
    for (error, _) in corpus() {
        for ctx in contexts() {
            let decision = decide_failure(&registry, error, &ctx);
            let native = native_verdict(error, &ctx);
            assert_eq!(
                decision.leg,
                LegSource::Live {
                    digest: digest.clone()
                },
                "{error}"
            );
            assert_eq!(
                decision.verdict,
                native,
                "{error} attempt={} budget={:?} answered={} capped={} backoff={}",
                ctx.attempt,
                ctx.budget,
                ctx.emitted_answer,
                ctx.output_cap_exhausted,
                ctx.backoff_ms
            );
            assert!(!decision.declined);
            checked += 1;
            replays += usize::from(native.retry);
        }
    }
    eprintln!("errors leg parity: {checked} decisions equal native ({replays} replays)");
    assert!(checked >= 4_000 && replays >= 200, "{checked} {replays}");
}

#[test]
fn a_leg_that_asks_for_more_than_native_falls_back_with_the_native_decision() {
    let permanent = "HTTP 401: Unauthorized";
    let transient = "HTTP 503: overloaded";
    let plain = "connection reset by peer";
    // (render body, error, attempt, budget): each answer is looser than native.
    let looser: [(&str, &str, usize, Option<usize>); 9] = [
        (
            "({class:'transient',retry:true,delay_ms:0})",
            permanent,
            0,
            None,
        ),
        (
            "({class:'transient',retry:true,delay_ms:2001})",
            transient,
            0,
            None,
        ),
        (
            "({class:'transient',retry:true,delay_ms:0})",
            transient,
            3,
            Some(3),
        ),
        ("({class:'context',retry:false,delay_ms:0})", plain, 0, None),
        (
            "({class:'permanent',retry:true,delay_ms:0})",
            transient,
            0,
            None,
        ),
        (
            "({class:'quota',retry:true,delay_ms:0})",
            transient,
            0,
            None,
        ),
        ("({class:'transient',retry:true})", transient, 0, None),
        (
            "({class:'doomed',retry:false,delay_ms:0})",
            transient,
            0,
            None,
        ),
        (
            "({class:'transient',retry:true,delay_ms:0,extra:1})",
            transient,
            0,
            None,
        ),
    ];
    for (render, error, attempt, budget) in looser {
        let root = Root::new("looser");
        install_js(&root, render);
        let (_modules, registry) = live_registry(&root);
        let ctx = context(attempt, budget);
        let decision = decide_failure(&registry, error, &ctx);
        assert_eq!(decision.verdict, native_verdict(error, &ctx), "{render}");
        assert!(
            matches!(decision.leg, LegSource::LiveFallback { .. }),
            "{render}: {:?}",
            decision.leg
        );
        assert_eq!(leg_parts(&decision.leg).0, "live-fallback");
    }
    // Throwing is a fallback too, never a failed turn.
    let root = Root::new("throws");
    install_js(
        &root,
        "(input.kind === undefined ? ({class:'transient',retry:false,delay_ms:0}) : (() => { throw Error('edit in progress') })())",
    );
    let (_modules, registry) = live_registry(&root);
    let ctx = context(0, None);
    let decision = decide_failure(&registry, "HTTP 503: overloaded", &ctx);
    assert_eq!(
        decision.verdict,
        native_verdict("HTTP 503: overloaded", &ctx)
    );
    assert!(matches!(decision.leg, LegSource::LiveFallback { .. }));
}

#[test]
fn a_stricter_leg_is_honored_and_a_missing_leg_is_native() {
    let root = Root::new("stricter");
    install_js(
        &root,
        "({class: input.status === 503 ? 'permanent' : 'transient', retry: input.status !== 503 && input.attempt < 1, delay_ms: 5})",
    );
    let (_modules, registry) = live_registry(&root);
    let ctx = context(0, None);
    let first = decide_failure(&registry, "HTTP 503: overloaded", &ctx);
    assert!(matches!(first.leg, LegSource::Live { .. }));
    assert_eq!(
        first.verdict,
        FailureVerdict {
            class: FailureClass::Permanent,
            retry: false,
            delay_ms: 5
        }
    );
    assert!(first.declined);
    let second = decide_failure(&registry, "connection reset by peer", &ctx);
    assert!(second.verdict.retry && !second.declined);
    assert_eq!(second.verdict.delay_ms, 5);
    // Native keeps its 500 ms ladder; only a live leg may ask for less.
    let bare = ToolRegistry::new();
    let native = decide_failure(&bare, "HTTP 503: overloaded", &ctx);
    assert_eq!(native.leg, LegSource::Native);
    assert_eq!(native.verdict.delay_ms, 500);
    assert_eq!(leg_note(&native.leg), "");
    assert!(leg_note(&first.leg).starts_with(" [errors leg live:"));
}

#[test]
fn an_edited_leg_serves_the_next_turn_not_the_one_in_flight() {
    let root = Root::new("edit");
    install_shipped(&root);
    let (mut modules, registry) = live_registry(&root);
    let before = registry.live_policy("errors").unwrap().digest().to_string();
    let ctx = context(1, None);
    let error = "HTTP 503: overloaded";
    assert!(decide_failure(&registry, error, &ctx).verdict.retry);
    // An adapter edit: the shipped policy, held to one replay.
    std::fs::write(
        root.module().join("index.js"),
        std::fs::read_to_string(root.module().join("index.js"))
            .unwrap()
            .replace(
                "retry: verdict.retry,",
                "retry: verdict.retry && input.attempt < 1,",
            ),
    )
    .unwrap();
    publish(&mut modules, &registry);
    // The turn that pinned the old generation keeps it.
    assert_eq!(registry.live_policy("errors").unwrap().digest(), before);
    assert!(decide_failure(&registry, error, &ctx).verdict.retry);
    registry.pin_live_policies();
    let after = registry.live_policy("errors").unwrap();
    assert_ne!(after.digest(), before);
    let next = decide_failure(&registry, error, &ctx);
    assert!(!next.verdict.retry && next.declined);
    assert_eq!(
        next.leg,
        LegSource::Live {
            digest: after.digest().to_string()
        }
    );
    assert!(
        decide_failure(&registry, error, &context(0, None))
            .verdict
            .retry
    );
}

#[test]
fn the_policy_costs_microseconds_per_decision() {
    let root = Root::new("cost");
    install_shipped(&root);
    let (_modules, registry) = live_registry(&root);
    let program = registry.live_policy("errors").unwrap();
    let ctx = context(2, Some(5));
    let facts = failure_facts("HTTP 503: overloaded", &ctx);
    for _ in 0..20 {
        program.render(&facts).unwrap();
    }
    let mut micros: Vec<u128> = (0..400)
        .map(|_| {
            let started = Instant::now();
            program.render(&facts).unwrap();
            started.elapsed().as_micros()
        })
        .collect();
    micros.sort_unstable();
    eprintln!(
        "errors leg Program::render: median={}us p95={}us max={}us over {} calls",
        micros[200],
        micros[380],
        micros[399],
        micros.len()
    );
    let started = Instant::now();
    for _ in 0..400 {
        std::hint::black_box(native_verdict("HTTP 503: overloaded", &ctx));
    }
    eprintln!(
        "native verdict: {}ns per decision",
        started.elapsed().as_nanos() / 400
    );
    assert!(micros[200] < 100_000, "median {}us", micros[200]);
}

struct FailingClub {
    calls: AtomicUsize,
    error: &'static str,
}
impl Club for FailingClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok("unused".into())
    }
    fn label(&self) -> &str {
        "status-failure"
    }
    fn chat_streaming(
        &self,
        _messages: &[ChatMsg],
        _tools: &[ToolDef],
        _cancel: &AtomicBool,
        _on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(self.error.to_string())
    }
}

/// One whole turn against a club that always fails: provider calls made, the
/// terminal message, the notices and the ledger's `provider_unavailable` record.
fn failing_turn(
    registry: &ToolRegistry,
    error: &'static str,
) -> (usize, String, Vec<String>, serde_json::Value) {
    let club = FailingClub {
        calls: AtomicUsize::new(0),
        error,
    };
    let (events, receiver) = mpsc::channel();
    let failure = run_turn_observed(
        &club,
        registry,
        &mut vec![ChatMsg::user("hello")],
        &AtomicBool::new(false),
        Some(4),
        &events,
    )
    .expect_err("an always-failing provider ends the turn");
    assert_eq!(failure.stop_reason, TurnStopReason::ProviderError);
    let notices = receiver
        .try_iter()
        .filter_map(|event| match event {
            TurnEvent::Notice(text) if text.contains("; retrying") => Some(text),
            _ => None,
        })
        .collect();
    let record = trajectory::progress_ledger_snapshot()["escalations"]
        .as_array()
        .unwrap()
        .iter()
        .rfind(|entry| entry["kind"] == "provider_unavailable")
        .cloned()
        .expect("the stop is recorded");
    (
        club.calls.load(Ordering::SeqCst),
        failure.message,
        notices,
        record,
    )
}

fn turn_env() -> [EnvGuard; 3] {
    [
        EnvGuard::set("ANGEL_PROVIDER_RETRIES", "3"),
        EnvGuard::set("ANGEL_PROVIDER_RETRY_BACKOFF_MS", "0"),
        EnvGuard::set("ANGEL_TEACHER_WATCH", "0"),
    ]
}

#[test]
fn a_turn_retries_through_the_shipped_leg_exactly_as_it_did_natively() {
    let _guard = crate::tests::env_lock();
    let _env = turn_env();
    let root = Root::new("turn");
    install_shipped(&root);
    let (_modules, live) = live_registry(&root);
    let digest = live.live_policy("errors").unwrap().digest()[..8].to_string();
    for (error, calls) in [
        ("HTTP 503: overloaded", 4),
        ("HTTP 429: rate limited", 4),
        ("HTTP 401: Unauthorized", 1),
        ("HTTP 503 [x-should-retry: false]: overloaded", 1),
        (
            "tls connection init failed: invalid peer certificate: UnknownIssuer",
            1,
        ),
        ("HTTP 429: weekly quota exhausted", 1),
    ] {
        let (native_calls, native_message, native_notices, native_record) =
            failing_turn(&ToolRegistry::new(), error);
        let (live_calls, message, notices, record) = failing_turn(&live, error);
        assert_eq!((native_calls, live_calls), (calls, calls), "{error}");
        let tag = format!(" [errors leg live:{digest}]");
        assert_eq!(message, format!("{native_message}{tag}"), "{error}");
        assert_eq!(notices.len(), native_notices.len(), "{error}");
        assert!(
            notices.iter().all(|notice| notice.ends_with(&tag)),
            "{error}"
        );
        assert_eq!(native_record["leg"], "native", "{error}");
        assert_eq!(record["leg"], "live", "{error}");
        assert_eq!(record["digest"], digest, "{error}");
        assert_eq!(record["class"], native_record["class"], "{error}");
    }
}

#[test]
fn a_turn_ignores_a_looser_leg_and_obeys_a_stricter_one() {
    let _guard = crate::tests::env_lock();
    let _env = turn_env();
    // Looser: replays a permanent failure. Native stops, with the receipt saying so.
    let root = Root::new("turn-looser");
    install_js(&root, "({class:'transient',retry:true,delay_ms:0})");
    let (_modules, registry) = live_registry(&root);
    let (calls, message, _, record) = failing_turn(&registry, "HTTP 401: Unauthorized");
    assert_eq!(calls, 1);
    assert!(
        message.contains("not retried: permanent provider error"),
        "{message}"
    );
    assert!(message.contains(" [errors leg live-fallback:"), "{message}");
    assert_eq!(record["leg"], "live-fallback");
    assert_eq!(record["class"], "permanent");
    // Stricter: one replay, then stop, and the receipt names the leg.
    let root = Root::new("turn-stricter");
    install_js(
        &root,
        "({class:'transient',retry:input.attempt < 1,delay_ms:0})",
    );
    let (_modules, registry) = live_registry(&root);
    let (calls, message, notices, record) = failing_turn(&registry, "HTTP 503: overloaded");
    assert_eq!(calls, 2);
    assert_eq!(notices.len(), 1);
    assert!(
        message.contains("not retried: the errors leg stopped this transient failure"),
        "{message}"
    );
    assert_eq!(record["leg"], "live");
}

#[test]
fn an_overflow_classed_by_the_leg_is_compacted_and_a_context_claim_without_flags_is_refused() {
    // The leg may only name `context` where the flags show an overflow.
    let root = Root::new("context");
    install_js(&root, "({class:'context',retry:false,delay_ms:0})");
    let (_modules, registry) = live_registry(&root);
    let ctx = context(0, None);
    let overflow = decide_failure(&registry, "HTTP 413: payload too large", &ctx);
    assert_eq!(overflow.verdict.class, FailureClass::Context);
    assert!(matches!(overflow.leg, LegSource::Live { .. }));
    let claim = decide_failure(&registry, "HTTP 503: overloaded", &ctx);
    assert_eq!(claim.verdict.class, FailureClass::Transient);
    assert!(matches!(claim.leg, LegSource::LiveFallback { .. }));
}

#[test]
#[ignore = "requires pinned optional Bend developer compiler; run explicitly"]
fn a_bend_source_edit_changes_the_next_turns_retries_and_a_broken_law_is_refused() {
    let root = Root::new("bend-edit");
    install_shipped(&root);
    let (mut modules, registry) = live_registry(&root);
    let source = std::fs::read_to_string(root.module().join("errors.bend")).unwrap();
    let status = |modules: &angel_live::Registry| {
        modules
            .snapshot()
            .statuses
            .into_iter()
            .find(|status| status.id == "errors")
            .unwrap()
    };
    let error = "HTTP 503: overloaded";
    let first = registry.live_policy("errors").unwrap().digest().to_string();
    assert!(
        decide_failure(&registry, error, &context(5, None))
            .verdict
            .retry
    );
    // An edit that keeps the laws: no more than two replays.
    let capped = source.replace(
        "    case False{}:\n      True{}\n    case True{}:\n      (attempt < budget : U32)",
        "    case False{}:\n      (attempt < 2 : U32)\n    case True{}:\n      (attempt < budget : U32)",
    );
    assert_ne!(capped, source);
    std::fs::write(root.module().join("errors.bend"), &capped).unwrap();
    publish(&mut modules, &registry);
    assert_eq!(status(&modules).generation, 2, "{:?}", status(&modules));
    assert!(
        decide_failure(&registry, error, &context(5, None))
            .verdict
            .retry,
        "pinned turn"
    );
    registry.pin_live_policies();
    let next = decide_failure(&registry, error, &context(5, None));
    assert!(
        !next.verdict.retry && next.declined,
        "{next:?}",
        next = next.verdict
    );
    assert!(
        decide_failure(&registry, error, &context(1, None))
            .verdict
            .retry
    );
    assert_ne!(registry.live_policy("errors").unwrap().digest(), first);
    // An edit that retries a rejected certificate cannot be proven and never ships.
    std::fs::write(
        root.module().join("errors.bend"),
        capped.replace("veto || config || cert || auth", "veto || config || auth"),
    )
    .unwrap();
    publish(&mut modules, &registry);
    let refused = status(&modules);
    assert_eq!(refused.generation, 2);
    assert!(
        refused
            .error
            .as_deref()
            .is_some_and(|error| error.contains("SOME PROOFS FAIL")),
        "{refused:?}"
    );
    registry.pin_live_policies();
    let cert = "tls connection init failed: invalid peer certificate: UnknownIssuer";
    assert!(
        !decide_failure(&registry, cert, &context(0, None))
            .verdict
            .retry
    );
}

#[test]
fn policy_control_errors_withdrawal_preserves_pinned_bend_and_next_turn_uses_native_fallback() {
    let root=Root::new("withdrawal");install_shipped(&root);
    let path=root.module().join("index.js");
    let original=std::fs::read_to_string(&path).unwrap();
    assert!(original.contains("delay_ms: verdict.delay"));
    std::fs::write(&path,original.replace("delay_ms: verdict.delay","delay_ms: 5")).unwrap();
    let (mut modules,registry)=live_registry(&root);
    let ctx=context(1,None);let error="HTTP 503: overloaded";
    let before=decide_failure(&registry,error,&ctx);
    assert_eq!(before.verdict.delay_ms,5);assert!(matches!(before.leg,LegSource::Live { .. }));
    let snapshot=modules.snapshot();let status=snapshot.statuses.iter().find(|s|s.id=="errors").unwrap();
    assert!(status.build.is_some(),"fixture did not load receipt-checked packaged Bend");
    modules.withdraw(vec![("errors".into(),status.generation)]).unwrap();drop(snapshot);
    registry.set_live_policies(modules.snapshot().policies);
    let held=decide_failure(&registry,error,&ctx);
    assert_eq!(held.leg,before.leg);assert_eq!(held.verdict,before.verdict);
    registry.pin_live_policies();
    let native=decide_failure(&registry,error,&ctx);
    assert_eq!(native.leg,LegSource::Native);assert_eq!(native.verdict.delay_ms,1000);
    assert_eq!(native.verdict,native_verdict(error,&ctx));
    std::fs::write(&path,original.replace("delay_ms: verdict.delay","delay_ms: 7")).unwrap();
    modules.enable(vec!["errors".into()]).unwrap();
    registry.set_live_policies(modules.snapshot().policies);
    assert_eq!(decide_failure(&registry,error,&ctx).leg,LegSource::Native,"re-enable altered a pinned native-fallback turn");
    registry.pin_live_policies();let returned=decide_failure(&registry,error,&ctx);
    assert!(matches!(returned.leg,LegSource::Live { .. }));assert_eq!(returned.verdict.delay_ms,7);
}
