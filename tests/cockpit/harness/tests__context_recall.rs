//! Context tool, compaction budget knobs, auto-recall, and maybe_compact coverage.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory.
//! Uses parent fixtures: SummarizerClub, long_history, registry_with_store, tool_hop_history.

use super::*;

// --- context / recall suite ---

#[test]
fn turn_budget_off_by_default_else_expires_at_deadline() {
    assert!(!turn_expired(99_999, 0), "0 = off, never expires (default)");
    assert!(!turn_expired(9, 10));
    assert!(turn_expired(10, 10), "at the deadline");
    assert!(turn_expired(11, 10), "past the deadline");
}

#[test]
fn bm25_ranks_by_relevance() {
    let docs = vec![
        (0, "read a file from disk".to_string()),
        (1, "search the web for pages".to_string()),
        (2, "fetch a url and extract text".to_string()),
    ];
    let hits = bm25_rank("web search", &docs, 3);
    assert_eq!(
        hits.first(),
        Some(&1),
        "web-search doc ranks first: {hits:?}"
    );
    // No overlap → no hits.
    assert!(bm25_rank("quantum", &docs, 3).is_empty());
    assert!(bm25_rank("", &docs, 3).is_empty());
}

fn quadratic_bm25_reference(query: &str, docs: &[(usize, String)], limit: usize) -> Vec<usize> {
    let query = tokenize(query);
    if query.is_empty() || docs.is_empty() {
        return Vec::new();
    }
    let tokenized = docs
        .iter()
        .map(|(id, text)| (*id, tokenize(text)))
        .collect::<Vec<_>>();
    let document_count = tokenized.len() as f64;
    let average_length = (tokenized
        .iter()
        .map(|(_, tokens)| tokens.len())
        .sum::<usize>() as f64
        / document_count)
        .max(1.0);
    let mut scored = Vec::new();
    for (id, document) in &tokenized {
        let document_length = document.len() as f64;
        let mut score = 0.0;
        for term in &query {
            let term_frequency = document.iter().filter(|word| *word == term).count() as f64;
            if term_frequency == 0.0 {
                continue;
            }
            let document_frequency = tokenized
                .iter()
                .filter(|(_, candidate)| candidate.contains(term))
                .count() as f64;
            let inverse_frequency =
                (((document_count - document_frequency + 0.5) / (document_frequency + 0.5)) + 1.0)
                    .ln();
            score += inverse_frequency * (term_frequency * 2.5)
                / (term_frequency + 1.5 * (1.0 - 0.75 + 0.75 * document_length / average_length));
        }
        if score > 0.0 {
            scored.push((*id, score));
        }
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(limit).map(|(id, _)| id).collect()
}

#[test]
fn bm25_linear_index_matches_reference_and_visits_each_document_token_once() {
    let docs = (0..1024)
        .map(|index| {
            let topic = match index % 5 {
                0 => "compile rust cargo test diagnostics",
                1 => "search remote web research citations",
                2 => "read file source definition references",
                3 => "delegate review agent verification",
                _ => "image inspect visual screenshot",
            };
            (
                index,
                format!("tool_{index} {topic} shared shared bucket_{}", index % 17),
            )
        })
        .collect::<Vec<_>>();
    let query = "search shared rust search image absent";
    let expected_visits = docs
        .iter()
        .map(|(_, document)| tokenize(document).len())
        .sum::<usize>();

    let expected = quadratic_bm25_reference(query, &docs, 50);
    for limit in [1, 7, 50] {
        let (actual, visits) = count_bm25_index_token_visits(|| bm25_rank(query, &docs, limit));
        assert_eq!(
            actual,
            expected[..limit],
            "indexed ranking changed at limit {limit}"
        );
        assert_eq!(
            visits, expected_visits,
            "BM25 indexing must visit each document token exactly once"
        );
    }
}

#[test]
fn deferred_tools_hidden_from_defs_but_dispatchable() {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(ReverseTool)); // advertised
    reg.register_deferred(Box::new(WordCountTool)); // hidden until searched
    let names: Vec<String> = reg.defs().iter().map(|d| d.name.clone()).collect();
    assert!(names.contains(&"reverse".to_string()));
    assert!(
        !names.contains(&"word_count".to_string()),
        "deferred is hidden"
    );
    // Still dispatchable directly by name.
    assert!(
        reg.dispatch("word_count", &serde_json::json!({"text": "a b c"}))
            .is_ok()
    );
    // tool_search appears once enabled and finds the deferred tool.
    reg.enable_tool_search();
    let names: Vec<String> = reg.defs().iter().map(|d| d.name.clone()).collect();
    assert!(names.contains(&"tool_search".to_string()));
    let out = reg
        .dispatch("tool_search", &serde_json::json!({"query": "count words"}))
        .unwrap();
    assert!(
        out.contains("word_count"),
        "search surfaces the deferred tool: {out}"
    );
}

#[test]
fn context_tool_reports_usage_and_budget() {
    let gauge = Arc::new(ContextGauge::default());
    let tool = ContextTool::new(Arc::clone(&gauge));
    assert_eq!(tool.def().name, "get_context_remaining");
    // No budget set → usage-only message.
    gauge.used_tokens.store(1234, Ordering::Relaxed);
    let out = tool.call(&serde_json::json!({})).unwrap();
    assert!(
        out.contains("1234") && out.contains("No context budget"),
        "got: {out}"
    );
    // With a budget → reports remaining + percent.
    gauge.budget_tokens.store(2000, Ordering::Relaxed);
    let out = tool.call(&serde_json::json!({})).unwrap();
    assert!(out.contains("766 remaining"), "got: {out}"); // 2000-1234
    assert!(out.contains("62%"), "got: {out}"); // 1234/2000
}

struct MetaClub {
    label: &'static str,
    context_window: usize,
}
impl Club for MetaClub {
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok("ok".to_string())
    }
    fn label(&self) -> &str {
        self.label
    }
    fn metadata(&self) -> Option<Metadata> {
        (self.context_window > 0).then_some(Metadata {
            context_window: self.context_window,
            supports_cache: false,
            supports_reasoning: None,
            supports_tools: true,
        })
    }
}

#[test]
fn compaction_budget_defaults_to_333k_and_caps_to_model_window() {
    let _guard = crate::tests::env_lock();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_NO_AUTOCOMPACT") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_CONTEXT_SOFT_CAP") };

    let unknown = MetaClub {
        label: "unknown",
        context_window: 0,
    };
    assert_eq!(
        compaction_budget(&unknown, 0),
        DEFAULT_CONTEXT_BUDGET_TOKENS
    );

    let large = MetaClub {
        label: "large",
        context_window: 1_000_000,
    };
    assert_eq!(
        compaction_budget(&large, 0),
        DEFAULT_CONTEXT_BUDGET_TOKENS,
        "333k target caps a larger model window"
    );

    let small = MetaClub {
        label: "small",
        context_window: 64_000,
    };
    assert_eq!(
        compaction_budget(&small, 0),
        51_200,
        "usable reported window keeps 20% headroom and caps below 333k"
    );

    assert_eq!(
        compaction_budget(&small, 123_456),
        123_456,
        "explicit env budget remains a hard override"
    );

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_CONTEXT_SOFT_CAP", "111000") };
    assert_eq!(compaction_budget(&large, 0), 111_000);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_NO_AUTOCOMPACT", "1") };
    assert_eq!(compaction_budget(&large, 0), 0);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_CONTEXT_SOFT_CAP") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_NO_AUTOCOMPACT") };
}

#[test]
fn compaction_budget_sota_links_default_to_cost_sized_target() {
    let _guard = crate::tests::env_lock();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_NO_AUTOCOMPACT") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_CONTEXT_SOFT_CAP") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_SOTA_CONTEXT_BUDGET") };

    // A metered label budgets for cost, not context fit.
    let sota = MetaClub {
        label: "openrouter",
        context_window: 0,
    };
    assert_eq!(
        compaction_budget(&sota, 0),
        DEFAULT_SOTA_CONTEXT_BUDGET_TOKENS
    );

    // Env override reshapes the metered target.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_SOTA_CONTEXT_BUDGET", "50000") };
    assert_eq!(compaction_budget(&sota, 0), 50_000);

    // An explicit budget still beats everything.
    assert_eq!(compaction_budget(&sota, 123_456), 123_456);

    // 0 disables the special case → back on the 333k context-fit path.
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_SOTA_CONTEXT_BUDGET", "0") };
    assert_eq!(compaction_budget(&sota, 0), DEFAULT_CONTEXT_BUDGET_TOKENS);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_SOTA_CONTEXT_BUDGET") };

    // Local labels never see the metered target.
    let local = MetaClub {
        label: "atlas",
        context_window: 0,
    };
    assert_eq!(compaction_budget(&local, 0), DEFAULT_CONTEXT_BUDGET_TOKENS);

    // The metered target is still min'd against the model window −20%.
    let small_sota = MetaClub {
        label: "deepseek",
        context_window: 64_000,
    };
    assert_eq!(compaction_budget(&small_sota, 0), 51_200);
}

/// One `hops`-deep tool-calling tail: each hop is an assistant tool-call batch
/// followed by its result carrying `payload`.

#[test]
fn auto_recall_runs_once_per_distinct_project_query() {
    use crate::knowledge::memory::store::{Drawer, MemoryStore};
    struct RecallStore {
        seen: Mutex<Vec<(String, usize, Option<String>)>>,
    }
    impl MemoryStore for RecallStore {
        fn deposit(&self, _drawer: &Drawer) -> Result<String, String> {
            Ok(String::new())
        }
        fn search(
            &self,
            query: &str,
            limit: usize,
            wing: Option<&str>,
        ) -> Result<Vec<String>, String> {
            self.seen
                .lock()
                .unwrap()
                .push((query.to_string(), limit, wing.map(str::to_string)));
            Ok(vec![format!("memory hit for {query}")])
        }
        fn status(&self) -> Result<String, String> {
            Ok(String::new())
        }
    }

    let _guard = crate::tests::env_lock();
    let _legacy = crate::tests::TestEnvGuard::set("ANGEL_BACKPLANE", "0");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_AUTO_RECALL") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_AUTO_RECALL_K") };

    let store = Arc::new(RecallStore {
        seen: Mutex::new(Vec::new()),
    });
    let alpha_workspace = std::path::PathBuf::from("/tmp/angel-recall-alpha/repo");
    let beta_workspace = std::path::PathBuf::from("/tmp/angel-recall-beta/repo");
    let mut registry = ToolRegistry::with_team(alpha_workspace.clone(), Vec::new());
    registry.set_memory_store(store.clone());
    let (tx, _rx) = mpsc::channel();
    let mut history = vec![ChatMsg::system("sys"), ChatMsg::user("first task")];

    maybe_auto_recall(
        &registry,
        &mut history,
        DEFAULT_CONTEXT_BUDGET_TOKENS,
        &[],
        &tx,
    );
    maybe_auto_recall(
        &registry,
        &mut history,
        DEFAULT_CONTEXT_BUDGET_TOKENS,
        &[],
        &tx,
    );
    history.push(ChatMsg::user("second task"));
    maybe_auto_recall(
        &registry,
        &mut history,
        DEFAULT_CONTEXT_BUDGET_TOKENS,
        &[],
        &tx,
    );

    let mut beta_registry = ToolRegistry::with_team(beta_workspace.clone(), Vec::new());
    beta_registry.set_memory_store(store.clone());
    let mut beta_history = vec![ChatMsg::system("sys"), ChatMsg::user("first task")];
    maybe_auto_recall(
        &beta_registry,
        &mut beta_history,
        DEFAULT_CONTEXT_BUDGET_TOKENS,
        &[],
        &tx,
    );

    let seen = store.seen.lock().unwrap();
    assert_eq!(
        seen.len(),
        3,
        "same project/query recalled only once; another project recalls independently"
    );
    assert_eq!(seen[0].0, "first task");
    assert_eq!(seen[0].1, DEFAULT_AUTO_RECALL_K);
    assert_eq!(seen[1].0, "second task");
    assert_eq!(seen[2].0, "first task");
    assert_ne!(
        seen[0].2, seen[2].2,
        "unrelated repositories with the same basename need disjoint palace wings"
    );
    assert_eq!(
        seen[0].2.as_deref(),
        Some(crate::agent::compaction::project_wing_for(&alpha_workspace).as_str())
    );
    assert_eq!(
        seen[2].2.as_deref(),
        Some(crate::agent::compaction::project_wing_for(&beta_workspace).as_str())
    );
    assert!(
        history
            .iter()
            .any(|m| m.content.contains("memory hit for second task"))
    );
    let recalls: Vec<_> = history
        .iter()
        .filter(|m| m.content.contains("memory hit for"))
        .collect();
    assert_eq!(recalls.len(), 1, "new recall replaces stale recall context");
    assert_eq!(
        recalls[0].role,
        ChatRole::Harness,
        "recalled memory must use the internal background carrier"
    );
    assert!(recalls[0].content.starts_with(AUTO_RECALL_NOTE_PREFIX));
    assert_eq!(
        history
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::User)
            .map(|m| m.content.as_ref()),
        Some("second task"),
        "latest real user task stays the active request"
    );
}

#[test]
fn timed_out_uninterruptible_auto_recall_stays_single_flight_until_return() {
    use crate::knowledge::memory::store::{Drawer, MemoryStore};

    struct BlockingRecallStore {
        calls: Arc<AtomicUsize>,
        release: Arc<AtomicBool>,
    }
    impl MemoryStore for BlockingRecallStore {
        fn deposit(&self, _drawer: &Drawer) -> Result<String, String> {
            Ok(String::new())
        }
        fn search(
            &self,
            query: &str,
            _limit: usize,
            _wing: Option<&str>,
        ) -> Result<Vec<String>, String> {
            self.calls.fetch_add(1, Ordering::AcqRel);
            while !self.release.load(Ordering::Acquire) {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            Ok(vec![format!("released {query}")])
        }
        fn status(&self) -> Result<String, String> {
            Ok(String::new())
        }
    }

    struct ReleaseOnDrop(Arc<AtomicBool>);
    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    let _guard = crate::tests::env_lock();
    let calls = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(AtomicBool::new(false));
    let _release_on_drop = ReleaseOnDrop(Arc::clone(&release));
    let store: Arc<dyn MemoryStore> = Arc::new(BlockingRecallStore {
        calls: Arc::clone(&calls),
        release: Arc::clone(&release),
    });

    assert!(
        auto_recall_search_with_timeout(
            Arc::clone(&store),
            "first".into(),
            5,
            "wing".to_string(),
            std::time::Duration::from_millis(20),
        )
        .is_none()
    );
    let started_deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    while calls.load(Ordering::Acquire) == 0 && std::time::Instant::now() < started_deadline {
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!(calls.load(Ordering::Acquire), 1);

    assert!(
        auto_recall_search_with_timeout(
            Arc::clone(&store),
            "must not spawn".into(),
            5,
            "wing".to_string(),
            std::time::Duration::from_millis(20),
        )
        .is_none()
    );
    assert_eq!(calls.load(Ordering::Acquire), 1);

    release.store(true, Ordering::Release);
    let released_deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    let result = loop {
        if let Some(result) = auto_recall_search_with_timeout(
            Arc::clone(&store),
            "after release".into(),
            5,
            "wing".to_string(),
            std::time::Duration::from_millis(100),
        ) {
            break Some(result);
        }
        if std::time::Instant::now() >= released_deadline {
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    assert_eq!(result, Some(vec!["released after release".to_string()]));
    assert_eq!(calls.load(Ordering::Acquire), 2);
}

#[test]
fn maybe_compact_noop_when_summarizer_fails() {
    let mut h = long_history();
    let n0 = h.len();
    let (tx, _rx) = mpsc::channel();
    let club = SummarizerClub(""); // respond -> Err
    let reg = registry_with_store(std::sync::Arc::new(
        crate::knowledge::memory::store::NullStore,
    ));
    assert!(!maybe_compact(&club, &mut h, 30, 5, 0, &[], &reg, &tx));
    assert_eq!(h.len(), n0, "history untouched when the summary call fails");
}

#[test]
fn auto_recall_bounds_pinned_memory_before_the_first_model_hop() {
    use crate::knowledge::memory::store::{Drawer, MemoryStore};
    struct GiantRecallStore;
    impl MemoryStore for GiantRecallStore {
        fn deposit(&self, _drawer: &Drawer) -> Result<String, String> {
            Ok(String::new())
        }
        fn search(
            &self,
            _query: &str,
            _limit: usize,
            _wing: Option<&str>,
        ) -> Result<Vec<String>, String> {
            Ok(vec![
                format!("highest-ranked {}", "a".repeat(360)),
                format!("second-ranked {}", "b".repeat(360)),
                format!("third-ranked {}", "c".repeat(360)),
            ])
        }
        fn status(&self) -> Result<String, String> {
            Ok(String::new())
        }
    }

    let _guard = crate::tests::env_lock();
    let _legacy = crate::tests::TestEnvGuard::set("ANGEL_BACKPLANE", "0");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_AUTO_RECALL") };
    let mut registry = ToolRegistry::new();
    registry.set_memory_store(Arc::new(GiantRecallStore));
    let (tx, _rx) = mpsc::channel();
    let mut history = vec![ChatMsg::system("sys"), ChatMsg::user("find prior decision")];
    let budget = 500;

    maybe_auto_recall(&registry, &mut history, budget, &[], &tx);

    let recall = history
        .iter()
        .find(|message| message.content.starts_with("[Relevant notes recalled"))
        .expect("a complete top-ranked recall block should fit");
    assert!(
        recall.content.contains("highest-ranked"),
        "{}",
        recall.content
    );
    assert!(
        recall.content.contains("omitted to fit context"),
        "the oversized lower-ranked blocks need an explicit marker: {}",
        recall.content
    );
    assert!(
        context_tokens(&history, &[]) <= budget,
        "pinned recall must not overflow the first request: {} > {budget}",
        context_tokens(&history, &[])
    );
}

#[test]
fn maybe_compact_with_live_store_compacts_and_splices() {
    use crate::knowledge::memory::store::{Drawer, MemoryStore};
    use std::sync::Mutex;
    struct LiveRec(Mutex<usize>);
    impl MemoryStore for LiveRec {
        fn deposit(&self, _d: &Drawer) -> Result<String, String> {
            *self.0.lock().unwrap() += 1;
            Ok(String::new())
        }
        fn search(&self, _q: &str, _l: usize, _w: Option<&str>) -> Result<Vec<String>, String> {
            Ok(Vec::new())
        }
        fn status(&self) -> Result<String, String> {
            Ok(String::new())
        }
        // is_live() defaults to true → exercises the deposit/"filing" branch.
    }
    let mut h = long_history();
    let n0 = h.len();
    let (tx, _rx) = mpsc::channel();
    let club = SummarizerClub("## Task\n- ship it\n## Files\n- harness.rs");
    let reg = registry_with_store(std::sync::Arc::new(LiveRec(Mutex::new(0))));
    assert!(maybe_compact(&club, &mut h, 30, 5, 0, &[], &reg, &tx));
    assert!(h.len() < n0, "history shrank");
    assert!(
        h.iter().any(|m| m.content.contains("ship it")),
        "structured inline note spliced in"
    );
    let note = h
        .iter()
        .find(|m| m.content.contains("ship it"))
        .expect("structured inline note");
    assert_eq!(
        note.role,
        ChatRole::Harness,
        "live-store compaction summary must use the internal background carrier"
    );
    assert!(crate::agent::compaction::is_compaction_note(note));
    // Deposits are fire-and-forget (off-thread); we don't assert the count here
    // to avoid a race — the deterministic outcome is the compaction + splice.
}

#[test]
fn report_topic_uses_latest_user_ask() {
    // Picks the *last* user message's first non-empty line.
    let h = vec![
        ChatMsg::user("an earlier question"),
        ChatMsg::assistant("answer"),
        ChatMsg::user("\n  how does the swarm file reports?  \n"),
        ChatMsg::assistant("the synthesis"),
    ];
    assert_eq!(report_topic(&h), "how does the swarm file reports?");
    // Long asks are truncated with an ellipsis.
    let long = "x".repeat(100);
    let h2 = vec![ChatMsg::user(long.clone())];
    let topic = report_topic(&h2);
    assert!(topic.ends_with('…'));
    assert_eq!(topic.chars().count(), 65); // 64 chars + ellipsis
    // No user turn → generic fallback.
    assert_eq!(
        report_topic(&[ChatMsg::system("preamble")]),
        "Swarm synthesis"
    );
}

#[test]
fn recall_tool_validates_args_and_formats_hits() {
    use crate::knowledge::memory::store::{Drawer, MemoryStore};
    struct StubStore;
    impl MemoryStore for StubStore {
        fn deposit(&self, _d: &Drawer) -> Result<String, String> {
            Ok(String::new())
        }
        fn search(
            &self,
            query: &str,
            limit: usize,
            _wing: Option<&str>,
        ) -> Result<Vec<String>, String> {
            match query {
                "boom" => Err("backend down".to_string()),
                "empty" => Ok(Vec::new()),
                q => Ok(vec![format!("[w/Facts] hit:{q} limit:{limit}")]),
            }
        }
        fn status(&self) -> Result<String, String> {
            Ok(String::new())
        }
    }
    let tool = RecallTool::new(
        std::sync::Arc::new(StubStore),
        std::path::Path::new("/tmp/recall-project"),
    );
    assert!(
        tool.call(&serde_json::json!({"query": "   "})).is_err(),
        "blank query"
    );
    assert!(tool.call(&serde_json::json!({})).is_err(), "missing query");
    let hit = tool
        .call(&serde_json::json!({"query": "deadlock", "limit": 3}))
        .unwrap();
    assert!(hit.contains("hit:deadlock") && hit.contains("limit:3"));
    assert_eq!(
        tool.call(&serde_json::json!({"query": "empty"})).unwrap(),
        "(no relevant notes in long-term memory)"
    );
    assert!(tool.call(&serde_json::json!({"query": "boom"})).is_err());
    // limit 0 → clamped to 1.
    let clamped = tool
        .call(&serde_json::json!({"query": "x", "limit": 0}))
        .unwrap();
    assert!(clamped.contains("limit:1"), "got: {clamped}");
}
