use super::*;
use crate::agent::club::Metadata;

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

    unsafe { std::env::set_var("ANGEL_SOTA_CACHE_BUDGET_LIFT", "1") };
    assert_eq!(
        compaction_budget(&capable("deepseek", 0), 0),
        DEFAULT_CONTEXT_BUDGET_TOKENS
    );
    speak_cache_budget_lift(&tx);
    match rx.try_recv() {
        Ok(TurnEvent::Notice(msg)) => assert_eq!(
            msg, "cache-first: compaction budget 120k -> 333k on deepseek",
            "notice names the move and the club"
        ),
        other => panic!("expected the lift notice, got {other:?}"),
    }
    speak_cache_budget_lift(&tx);
    assert!(rx.try_recv().is_err(), "notice is one-shot");
    assert_eq!(
        compaction_budget(&capable("glm", 1_000_000), 0),
        DEFAULT_CONTEXT_BUDGET_TOKENS,
        "large window still caps at the 333k target"
    );
    speak_cache_budget_lift(&tx);
    assert!(rx.try_recv().is_err(), "no second notice per session");

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

    let plain = CacheMetaClub {
        label: "openrouter",
        context_window: 0,
        cache_capable: false,
    };
    assert_eq!(
        compaction_budget(&plain, 0),
        DEFAULT_SOTA_CONTEXT_BUDGET_TOKENS
    );

    unsafe { std::env::set_var("ANGEL_SOTA_CACHE_BUDGET_LIFT", "0") };
    assert_eq!(
        compaction_budget(&capable("deepseek", 0), 0),
        DEFAULT_SOTA_CONTEXT_BUDGET_TOKENS,
        "ANGEL_SOTA_CACHE_BUDGET_LIFT=0 opts out"
    );
    unsafe { std::env::remove_var("ANGEL_SOTA_CACHE_BUDGET_LIFT") };

    unsafe { std::env::set_var("ANGEL_SOTA_CONTEXT_BUDGET", "200000") };
    assert_eq!(
        compaction_budget(&capable("deepseek", 0), 0),
        200_000,
        "explicit ANGEL_SOTA_CONTEXT_BUDGET beats the lift"
    );
    unsafe { std::env::remove_var("ANGEL_SOTA_CONTEXT_BUDGET") };

    unsafe { std::env::set_var("ANGEL_CACHE_STABLE", "0") };
    assert_eq!(
        compaction_budget(&capable("deepseek", 0), 0),
        DEFAULT_SOTA_CONTEXT_BUDGET_TOKENS,
        "the lift requires cache-stable mode"
    );
    unsafe { std::env::remove_var("ANGEL_CACHE_STABLE") };

    assert_eq!(
        compaction_budget(&capable("atlas", 0), 0),
        DEFAULT_CONTEXT_BUDGET_TOKENS
    );

    speak_cache_budget_lift(&tx);
    assert!(
        rx.try_recv().is_err(),
        "retained 120k paths must not announce a lift"
    );
    scrub_env();
}

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
    unsafe { std::env::set_var("ANGEL_SOTA_CACHE_BUDGET_LIFT", "1") };

    let plain = CacheMetaClub {
        label: "openrouter",
        context_window: 0,
        cache_capable: false,
    };
    let legacy = compaction_budget(&plain, 0);
    let lifted = compaction_budget(&capable("deepseek", 0), 0);
    assert_eq!(legacy, DEFAULT_SOTA_CONTEXT_BUDGET_TOKENS);
    assert_eq!(lifted, DEFAULT_CONTEXT_BUDGET_TOKENS);

    let mut history = vec![ChatMsg::system("sys"), ChatMsg::user("task")];
    for i in 0..20 {
        history.push(ChatMsg::assistant(format!("{i} {}", "x".repeat(20_000))));
    }
    let used = estimate_tokens(&history);
    assert!(used > legacy * 80 / 100 && used < lifted * 80 / 100);

    let (tx, rx) = mpsc::channel();
    let mut reg = ToolRegistry::new();
    reg.set_memory_store(std::sync::Arc::new(
        crate::knowledge::memory::store::NullStore,
    ));
    reg.set_aux_clubs(vec![std::sync::Arc::new(LiftSummarizer)]);

    maybe_start_bg_compact(&mut history, lifted, 5, 0, &[], &reg, &tx);
    assert!(
        !bg_compact_inflight(&reg),
        "arming point follows the lifted budget"
    );
    match rx.try_recv() {
        Ok(TurnEvent::Notice(msg)) => assert_eq!(
            msg, "cache-first: compaction budget 120k -> 333k on deepseek",
            "boundary drains the lift notice"
        ),
        other => panic!("expected the lift notice at the boundary, got {other:?}"),
    }

    maybe_start_bg_compact(&mut history, legacy, 5, 0, &[], &reg, &tx);
    assert!(
        bg_compact_inflight(&reg),
        "same usage arms at 80% of the legacy budget"
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while bg_compact_inflight(&reg) {
        assert!(Instant::now() < deadline, "bg pass never landed");
        try_splice_bg_compact(&reg, &mut history, &tx);
        std::thread::sleep(Duration::from_millis(5));
    }
    scrub_env();
}

struct ProviderBudgetClub {
    context_window: usize,
}
impl Club for ProviderBudgetClub {
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok("ok".to_string())
    }
    fn label(&self) -> &str {
        "deepseek-flash"
    }
    fn prompt_cache_capable(&self) -> bool {
        true
    }
    fn provider_compaction_budget(&self) -> Option<usize> {
        Some(678_464)
    }
    fn metadata(&self) -> Option<Metadata> {
        (self.context_window > 0).then_some(Metadata {
            context_window: self.context_window,
            supports_cache: true,
            supports_reasoning: None,
            supports_tools: true,
        })
    }
}

#[test]
fn a_provider_harness_trigger_sets_the_budget_until_the_operator_sets_one() {
    let _guard = crate::tests::env_lock();
    scrub_env();
    assert_eq!(
        compaction_budget(
            &ProviderBudgetClub {
                context_window: 1_000_000
            },
            0
        ),
        678_464
    );
    assert_eq!(
        compaction_budget(
            &ProviderBudgetClub {
                context_window: 400_000
            },
            0
        ),
        320_000
    );
    unsafe { std::env::set_var("ANGEL_SOTA_CONTEXT_BUDGET", "90000") };
    assert_eq!(
        compaction_budget(
            &ProviderBudgetClub {
                context_window: 1_000_000
            },
            0
        ),
        90_000
    );
    scrub_env();
    assert_eq!(
        compaction_budget(
            &ProviderBudgetClub {
                context_window: 1_000_000
            },
            50_000
        ),
        50_000
    );
    let seat = crate::agent::club::HttpClub::new(
        "deepseek-flash",
        "https://api.deepseek.com/v1",
        "deepseek-flash",
        None,
    );
    assert_eq!(seat.provider_compaction_budget(), Some(678_464));
    let gateway = crate::agent::club::HttpClub::new(
        "deepseek-flash",
        "https://gateway.example/v1",
        "deepseek-flash",
        None,
    );
    assert_eq!(gateway.provider_compaction_budget(), None);
}
