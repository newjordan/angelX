use super::*;
use crate::club::Metadata;

/// Minimal club with a controllable label / window / prefix-cache capability
/// (mirrors the MetaClub fixture in harness/tests, plus the cache flag).
struct CacheMetaClub {
    label: &'static str,
    context_window: usize,
    cache_capable: bool,
}
impl Club for CacheMetaClub {
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok("ok".to_string())
    }
    fn label(&self) -> &str {
        self.label
    }
    fn prompt_cache_capable(&self) -> bool {
        self.cache_capable
    }
    fn metadata(&self) -> Option<Metadata> {
        (self.context_window > 0).then_some(Metadata {
            context_window: self.context_window,
            supports_cache: self.cache_capable,
            supports_reasoning: None,
            supports_tools: true,
        })
    }
}

fn capable(label: &'static str, context_window: usize) -> CacheMetaClub {
    CacheMetaClub {
        label,
        context_window,
        cache_capable: true,
    }
}

fn scrub_env() {
    for key in [
        "ANGEL_NO_AUTOCOMPACT",
        "ANGEL_CONTEXT_SOFT_CAP",
        "ANGEL_SOTA_CONTEXT_BUDGET",
        "ANGEL_SOTA_CACHE_BUDGET_LIFT",
        "ANGEL_CACHE_STABLE",
        "ANGEL_COMPACT_CHUNK_TOKENS",
    ] {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
    }
    reset_cache_budget_lift_notice_for_test();
}

#[test]
fn auto_compaction_uses_the_utility_chunk_limit_not_the_large_route_budget() {
    let _guard = crate::tests::env_lock();
    scrub_env();

    assert_eq!(
        auto_compact_chunk_threshold(DEFAULT_SOTA_CONTEXT_BUDGET_TOKENS),
        12_000,
        "the 120k paid-route budget must not become a 60k Qwen request"
    );
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_COMPACT_CHUNK_TOKENS", "64000") };
    assert_eq!(auto_compact_chunk_threshold(120_000), 60_000);
    assert_eq!(
        auto_compact_chunk_threshold(20_000),
        10_000,
        "half of a small route budget remains the hard ceiling"
    );
    scrub_env();
}

#[test]
fn cache_capable_sota_keeps_cost_budget_until_lift_is_opted_in() {
    let _guard = crate::tests::env_lock();
    scrub_env();

    // Cache hits are still transmitted, counted, and billed, so a
    // cache-capable paid route keeps the request-volume target by default.
    assert_eq!(
        compaction_budget(&capable("deepseek", 0), 0),
        DEFAULT_SOTA_CONTEXT_BUDGET_TOKENS
    );
    let (tx, rx) = mpsc::channel();
    speak_cache_budget_lift(&tx);
    assert!(
        rx.try_recv().is_err(),
        "the default must not announce a lift"
    );

    // The former cache-first behavior remains available as an explicit
    // operator choice.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_SOTA_CACHE_BUDGET_LIFT", "1") };
    assert_eq!(
        compaction_budget(&capable("deepseek", 0), 0),
        DEFAULT_CONTEXT_BUDGET_TOKENS
    );
    // The lift speaks once, at a compaction boundary, naming the club.
    speak_cache_budget_lift(&tx);
    match rx.try_recv() {
        Ok(TurnEvent::Notice(msg)) => assert_eq!(
            msg, "cache-first: compaction budget 120k -> 333k on deepseek",
            "notice names the move and the club"
        ),
        other => panic!("expected the lift notice, got {other:?}"),
    }
    // Drained → nothing further, and a later qualifying resolve doesn't
    // re-queue: one notice per session.
    speak_cache_budget_lift(&tx);
    assert!(rx.try_recv().is_err(), "notice is one-shot");
    assert_eq!(
        compaction_budget(&capable("glm", 1_000_000), 0),
        DEFAULT_CONTEXT_BUDGET_TOKENS,
        "large window still caps at the 333k target"
    );
    speak_cache_budget_lift(&tx);
    assert!(rx.try_recv().is_err(), "no second notice per session");

    // A small model window caps the lifted target exactly like the base
    // path: window −20% binds below both targets, so nothing was raised and
    // nothing is announced.
    reset_cache_budget_lift_notice_for_test();
    assert_eq!(
        compaction_budget(&capable("kimi", 64_000), 0),
        51_200,
        "window −20% still caps the lifted target"
    );
    speak_cache_budget_lift(&tx);
    assert!(
        rx.try_recv().is_err(),
        "a window-capped non-raise must not announce a lift"
    );
    scrub_env();
}

#[test]
fn non_cache_sota_keeps_cost_sized_target_and_opt_outs_hold() {
    let _guard = crate::tests::env_lock();
    scrub_env();
    let (tx, rx) = mpsc::channel();

    // No detected prefix cache → the historical 120k cost target holds.
    let plain = CacheMetaClub {
        label: "openrouter",
        context_window: 0,
        cache_capable: false,
    };
    assert_eq!(
        compaction_budget(&plain, 0),
        DEFAULT_SOTA_CONTEXT_BUDGET_TOKENS
    );

    // Explicit opt-out keeps a cache-capable route on the cost target.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_SOTA_CACHE_BUDGET_LIFT", "0") };
    assert_eq!(
        compaction_budget(&capable("deepseek", 0), 0),
        DEFAULT_SOTA_CONTEXT_BUDGET_TOKENS,
        "ANGEL_SOTA_CACHE_BUDGET_LIFT=0 opts out"
    );
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_SOTA_CACHE_BUDGET_LIFT") };

    // An explicitly set SOTA budget is an operator cost order — it wins
    // over the lift in both directions.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_SOTA_CONTEXT_BUDGET", "200000") };
    assert_eq!(
        compaction_budget(&capable("deepseek", 0), 0),
        200_000,
        "explicit ANGEL_SOTA_CONTEXT_BUDGET beats the lift"
    );
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_SOTA_CONTEXT_BUDGET") };

    // Cache-stable mode off (per-hop trimming active) → prefix cache can't
    // hold anyway, so the cost target stays.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_CACHE_STABLE", "0") };
    assert_eq!(
        compaction_budget(&capable("deepseek", 0), 0),
        DEFAULT_SOTA_CONTEXT_BUDGET_TOKENS,
        "the lift requires cache-stable mode"
    );
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_CACHE_STABLE") };

    // Local labels never took the SOTA target and are untouched by the lift.
    assert_eq!(
        compaction_budget(&capable("atlas", 0), 0),
        DEFAULT_CONTEXT_BUDGET_TOKENS
    );

    // None of the retained paths queued a notice.
    speak_cache_budget_lift(&tx);
    assert!(
        rx.try_recv().is_err(),
        "retained 120k paths must not announce a lift"
    );
    scrub_env();
}

/// A fixed-summary club, like the parent tests' SummarizerClub.
struct LiftSummarizer;
impl Club for LiftSummarizer {
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok("LIFT NOTES".to_string())
    }
    fn label(&self) -> &str {
        "lift-summarizer"
    }
}

#[test]
fn bg_compact_arming_point_moves_with_the_lifted_budget() {
    let _guard = crate::tests::env_lock();
    scrub_env();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_SOTA_CACHE_BUDGET_LIFT", "1") };

    // Resolve both budgets through the real resolver: the legacy cost
    // target on a plain SOTA link, the explicitly lifted target on a
    // cache-first one.
    let plain = CacheMetaClub {
        label: "openrouter",
        context_window: 0,
        cache_capable: false,
    };
    let legacy = compaction_budget(&plain, 0);
    let lifted = compaction_budget(&capable("deepseek", 0), 0);
    assert_eq!(legacy, DEFAULT_SOTA_CONTEXT_BUDGET_TOKENS);
    assert_eq!(lifted, DEFAULT_CONTEXT_BUDGET_TOKENS);

    // Usage past 80% of the legacy budget but well under 80% of the lifted
    // one: ~100k estimated tokens (bytes/4).
    let mut history = vec![ChatMsg::system("sys"), ChatMsg::user("task")];
    for i in 0..20 {
        history.push(ChatMsg::assistant(format!("{i} {}", "x".repeat(20_000))));
    }
    let used = estimate_tokens(&history);
    assert!(used > legacy * 80 / 100 && used < lifted * 80 / 100);

    let (tx, rx) = mpsc::channel();
    let mut reg = ToolRegistry::new();
    reg.set_memory_store(std::sync::Arc::new(crate::memory_store::NullStore));
    reg.set_aux_clubs(vec![std::sync::Arc::new(LiftSummarizer)]);

    // Under the lifted budget the 80% arming point has moved: no bg pass.
    maybe_start_bg_compact(&mut history, lifted, 5, 0, &[], &reg, &tx);
    assert!(
        !bg_compact_inflight(&reg),
        "arming point follows the lifted budget"
    );
    // The boundary spoke the queued lift notice even without arming.
    match rx.try_recv() {
        Ok(TurnEvent::Notice(msg)) => assert_eq!(
            msg, "cache-first: compaction budget 120k -> 333k on deepseek",
            "boundary drains the lift notice"
        ),
        other => panic!("expected the lift notice at the boundary, got {other:?}"),
    }

    // The same usage against the legacy budget is past its arming point.
    maybe_start_bg_compact(&mut history, legacy, 5, 0, &[], &reg, &tx);
    assert!(
        bg_compact_inflight(&reg),
        "same usage arms at 80% of the legacy budget"
    );
    // Drain the worker so no permit/slot leaks into other tests.
    let deadline = Instant::now() + Duration::from_secs(5);
    while bg_compact_inflight(&reg) {
        assert!(Instant::now() < deadline, "bg pass never landed");
        try_splice_bg_compact(&reg, &mut history, &tx);
        std::thread::sleep(Duration::from_millis(5));
    }
    scrub_env();
}
