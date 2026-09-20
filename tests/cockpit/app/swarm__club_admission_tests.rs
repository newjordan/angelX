use super::*;
use std::time::{Duration, Instant};

static TEST_WORKERS_INFLIGHT: AtomicUsize = AtomicUsize::new(0);
static TEST_WAVE_INFLIGHT: AtomicUsize = AtomicUsize::new(0);

struct BlockingClub {
    calls: Arc<AtomicUsize>,
    release: Arc<AtomicBool>,
}

struct ImmediateClub;
struct CacheCapableClub;

struct PeakConcurrencyClub {
    calls: Arc<AtomicUsize>,
    active: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
}

impl Club for ImmediateClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok("landed".to_string())
    }

    fn label(&self) -> &str {
        "immediate-swarm-worker"
    }
}

impl Club for CacheCapableClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok("landed".to_string())
    }

    fn label(&self) -> &str {
        "cache-capable-swarm-worker"
    }

    fn prompt_cache_capable(&self) -> bool {
        true
    }
}

impl Club for PeakConcurrencyClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        let active = self.active.fetch_add(1, Ordering::AcqRel) + 1;
        self.peak.fetch_max(active, Ordering::AcqRel);
        std::thread::sleep(Duration::from_millis(25));
        self.active.fetch_sub(1, Ordering::AcqRel);
        Ok("landed".to_string())
    }

    fn label(&self) -> &str {
        "peak-concurrency-swarm-worker"
    }
}

impl Club for BlockingClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        while !self.release.load(Ordering::Acquire) {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        Ok("released".to_string())
    }

    fn label(&self) -> &str {
        "blocking-swarm-worker"
    }
}

fn test_swarm(club: Arc<dyn Club>) -> SwarmClub {
    SwarmClub {
        name: "worker-admission-fixture".to_string(),
        clubs: RoleClubs {
            propose: Arc::clone(&club),
            propose_extra: Vec::new(),
            judge: Arc::clone(&club),
            judge_extra: Vec::new(),
            verify: Arc::clone(&club),
            verify_extra: Vec::new(),
            aggregate: club,
            aggregate_extra: Vec::new(),
            research: None,
        },
        k: Knobs::default(),
        fallbacks: Vec::new(),
        tool_preflight: Default::default(),
    }
}

#[test]
fn swarm_forwards_aggregate_prompt_cache_capability() {
    let swarm = test_swarm(Arc::new(CacheCapableClub));

    assert!(swarm.prompt_cache_capable());
}

#[test]
fn sota_moa_parallel_cap_queues_every_requested_seat() {
    let _guard = crate::tests::env_lock();
    let _parallel = crate::tests::TestEnvGuard::set("ANGEL_SOTA_MOA_MAX_PARALLEL", "2");
    TEST_WAVE_INFLIGHT.store(0, Ordering::Release);
    let calls = Arc::new(AtomicUsize::new(0));
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let club: Arc<dyn Club> = Arc::new(PeakConcurrencyClub {
        calls: Arc::clone(&calls),
        active: Arc::clone(&active),
        peak: Arc::clone(&peak),
    });
    let mut swarm = test_swarm(Arc::clone(&club));
    swarm.name = "sota-moa".to_string();
    let clubs = (0..5).map(|_| Arc::clone(&club)).collect::<Vec<_>>();

    let results = swarm.fan_out_across_on_cancel_with_admission(
        &clubs,
        None,
        None,
        &TEST_WAVE_INFLIGHT,
        8,
        |_| (String::new(), vec![ChatMsg::user("draft")]),
    );

    assert!(results.iter().all(Result::is_ok), "{results:?}");
    assert_eq!(calls.load(Ordering::Acquire), 5);
    assert_eq!(peak.load(Ordering::Acquire), 2);
    assert_eq!(active.load(Ordering::Acquire), 0);
    assert_eq!(TEST_WAVE_INFLIGHT.load(Ordering::Acquire), 0);
}

#[test]
fn sota_moa_parallel_queue_does_not_launch_after_cancel() {
    let _guard = crate::tests::env_lock();
    let _parallel = crate::tests::TestEnvGuard::set("ANGEL_SOTA_MOA_MAX_PARALLEL", "1");
    TEST_WAVE_INFLIGHT.store(0, Ordering::Release);
    let calls = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(AtomicBool::new(false));
    let club: Arc<dyn Club> = Arc::new(BlockingClub {
        calls: Arc::clone(&calls),
        release: Arc::clone(&release),
    });
    let mut swarm = test_swarm(Arc::clone(&club));
    swarm.name = "sota-moa".to_string();
    let clubs = (0..3).map(|_| Arc::clone(&club)).collect::<Vec<_>>();
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_worker = Arc::clone(&cancel);
    let calls_worker = Arc::clone(&calls);
    let canceller = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(1);
        while calls_worker.load(Ordering::Acquire) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        cancel_worker.store(true, Ordering::Release);
        release.store(true, Ordering::Release);
    });

    let results = swarm.fan_out_across_on_cancel_with_admission(
        &clubs,
        Some(cancel.as_ref()),
        None,
        &TEST_WAVE_INFLIGHT,
        4,
        |_| (String::new(), vec![ChatMsg::user("draft")]),
    );
    canceller.join().unwrap();
    let release_deadline = Instant::now() + Duration::from_secs(1);
    while TEST_WAVE_INFLIGHT.load(Ordering::Acquire) != 0 && Instant::now() < release_deadline {
        std::thread::sleep(Duration::from_millis(2));
    }

    assert_eq!(calls.load(Ordering::Acquire), 1);
    assert!(results[1..].iter().all(Result::is_err));
    assert_eq!(TEST_WAVE_INFLIGHT.load(Ordering::Acquire), 0);
}

#[test]
fn cut_swarm_worker_permits_stay_owned_until_actual_return() {
    TEST_WORKERS_INFLIGHT.store(0, Ordering::Release);
    let mut first = reserve_swarm_workers(&TEST_WORKERS_INFLIGHT, 2, 2).unwrap();
    assert_eq!(TEST_WORKERS_INFLIGHT.load(Ordering::Acquire), 2);
    assert!(reserve_swarm_workers(&TEST_WORKERS_INFLIGHT, 1, 2).is_err());

    drop(first.pop());
    assert_eq!(TEST_WORKERS_INFLIGHT.load(Ordering::Acquire), 1);
    let replacement = reserve_swarm_workers(&TEST_WORKERS_INFLIGHT, 1, 2).unwrap();
    assert_eq!(TEST_WORKERS_INFLIGHT.load(Ordering::Acquire), 2);

    drop(first);
    drop(replacement);
    assert_eq!(TEST_WORKERS_INFLIGHT.load(Ordering::Acquire), 0);
}

#[test]
fn repeated_cut_waves_fail_fast_until_actual_workers_return() {
    // This spawn-heavy test shares TEST_WAVE_INFLIGHT with the other waves.
    let _guard = crate::tests::env_lock();
    TEST_WAVE_INFLIGHT.store(0, Ordering::Release);
    let calls = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(AtomicBool::new(false));
    let club: Arc<dyn Club> = Arc::new(BlockingClub {
        calls: Arc::clone(&calls),
        release: Arc::clone(&release),
    });
    let swarm = test_swarm(Arc::clone(&club));
    let clubs = vec![Arc::clone(&club), club];
    let cancelled = AtomicBool::new(true);
    let make = |_| (String::new(), vec![ChatMsg::user("draft")]);

    let first = swarm.fan_out_across_on_cancel_with_admission(
        &clubs,
        Some(&cancelled),
        None,
        &TEST_WAVE_INFLIGHT,
        2,
        make,
    );
    assert!(first.iter().all(Result::is_err));
    let started_deadline = Instant::now() + Duration::from_secs(1);
    while calls.load(Ordering::Acquire) < 2 && Instant::now() < started_deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(calls.load(Ordering::Acquire), 2);
    assert_eq!(TEST_WAVE_INFLIGHT.load(Ordering::Acquire), 2);

    let second = swarm.fan_out_across_on_cancel_with_admission(
        &clubs,
        Some(&cancelled),
        None,
        &TEST_WAVE_INFLIGHT,
        2,
        make,
    );
    assert!(second.iter().all(|result| {
        result
            .as_ref()
            .is_err_and(|error| error.contains("swarm capacity exhausted"))
    }));
    assert_eq!(calls.load(Ordering::Acquire), 2);

    release.store(true, Ordering::Release);
    let released_deadline = Instant::now() + Duration::from_secs(1);
    while TEST_WAVE_INFLIGHT.load(Ordering::Acquire) != 0 && Instant::now() < released_deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(TEST_WAVE_INFLIGHT.load(Ordering::Acquire), 0);

    let reopened = swarm.fan_out_across_on_cancel_with_admission(
        &clubs,
        None,
        None,
        &TEST_WAVE_INFLIGHT,
        2,
        make,
    );
    assert!(reopened.iter().all(Result::is_ok), "{reopened:?}");
    assert_eq!(TEST_WAVE_INFLIGHT.load(Ordering::Acquire), 0);
}

#[test]
fn strict_width_waits_past_the_partial_quorum_grace_cut() {
    let _guard = crate::tests::env_lock();
    let _strict = crate::tests::TestEnvGuard::set("ANGEL_SOTA_MOA_REQUIRE_FULL_WIDTH", "1");
    let _quorum = crate::tests::TestEnvGuard::set("ANGEL_SWARM_QUORUM", "0.75");
    let _grace = crate::tests::TestEnvGuard::set("ANGEL_SWARM_GRACE_SECS", "0");
    let _deadline = crate::tests::TestEnvGuard::set("ANGEL_SWARM_WAVE_DEADLINE", "10");
    TEST_WAVE_INFLIGHT.store(0, Ordering::Release);

    let immediate: Arc<dyn Club> = Arc::new(ImmediateClub);
    let calls = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(AtomicBool::new(false));
    let delayed: Arc<dyn Club> = Arc::new(BlockingClub {
        calls: Arc::clone(&calls),
        release: Arc::clone(&release),
    });
    let mut swarm = test_swarm(Arc::clone(&immediate));
    swarm.name = "sota-moa".to_string();
    let clubs = vec![
        Arc::clone(&immediate),
        Arc::clone(&immediate),
        immediate,
        delayed,
    ];
    let releaser = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        release.store(true, Ordering::Release);
    });

    let results = swarm.fan_out_across_on_cancel_with_admission(
        &clubs,
        None,
        None,
        &TEST_WAVE_INFLIGHT,
        4,
        |_| (String::new(), vec![ChatMsg::user("draft")]),
    );
    releaser.join().unwrap();

    assert_eq!(calls.load(Ordering::Acquire), 1);
    assert!(results.iter().all(Result::is_ok), "{results:?}");
    assert_eq!(TEST_WAVE_INFLIGHT.load(Ordering::Acquire), 0);
}

struct OverallocatedClub;
impl Club for OverallocatedClub {
    fn label(&self) -> &str {
        "overallocated-swarm-seat"
    }
    fn supports_formation_budget(&self) -> bool {
        true
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        crate::agent::harness::formation_budget::current()
            .unwrap()
            .reserve(self.label(), 20, 20)?
            .settle(Some(crate::agent::club::UsageObservation {
                raw: [Some(20), Some(20), Some(0), Some(0), Some(0)],
                contract: crate::agent::club::UsageContract {
                    cache: crate::agent::club::CacheConvention::Included,
                    reasoning: crate::agent::club::ReasoningConvention::Included,
                },
                ..Default::default()
            }));
        Ok("complete overallocated seat".into())
    }
}

/// Operator-ordered F01 contract: every queued swarm seat runs after allocation is exceeded.
#[test]
fn formation_budget_swarm_queued_seats_report_overrun_and_all_complete() {
    let _guard = crate::tests::env_lock();
    let _parallel = crate::tests::TestEnvGuard::set("ANGEL_SOTA_MOA_MAX_PARALLEL", "1");
    let budget = crate::agent::harness::formation_budget::Budget::new(Some(1), None);
    let _scope = crate::agent::harness::formation_budget::enter(Some(budget.clone()));
    let club: Arc<dyn Club> = Arc::new(OverallocatedClub);
    let mut swarm = test_swarm(club.clone());
    swarm.name = "sota-moa".into();
    let clubs = vec![club; 5];
    let results = swarm.fan_out_across_on_cancel_with_admission(
        &clubs,
        None,
        None,
        &TEST_WAVE_INFLIGHT,
        8,
        |_| (String::new(), vec![ChatMsg::user("draft")]),
    );
    assert_eq!(results.len(), 5);
    assert!(results.iter().all(Result::is_ok), "{results:?}");
    let receipt = budget.snapshot();
    assert_eq!(receipt["spent"], 200);
    assert_eq!(receipt["over_allocation_tokens"], 199);
    assert_eq!(receipt["remaining"], -199);
    assert_eq!(receipt["reserved"], 0);
}
