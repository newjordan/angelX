use crate::club::{CacheUsage, ChatMsg, Club, RouteIdentity, TokenUsage, ToolDef};
use crate::harness::{
    ToolRegistry, TurnEvent, TurnStopReason, run_turn_steered_checkpointed_observed,
};
use crate::session::Session;
use crate::steer::SteerQueue;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::Instant;

type TurnResult = Result<(Vec<ChatMsg>, String, RouteIdentity, TurnStopReason), String>;

const DEFAULT_STEER_IDLE_INTERRUPT_SECS: u64 = 60;

pub(crate) fn configured_turn_idle_timeout_secs() -> Option<u64> {
    std::env::var("ANGEL_TURN_IDLE_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
}

/// `ANGEL_STEER_IDLE_INTERRUPT_SECS` (default 60, `0` disables): when operator
/// steers are queued and the provider stream has made no progress for this
/// long, interrupt the stalled request (keeping the steers) so they are
/// delivered as the next message instead of waiting for the idle timeout.
fn configured_steer_idle_interrupt_secs() -> u64 {
    std::env::var("ANGEL_STEER_IDLE_INTERRUPT_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_STEER_IDLE_INTERRUPT_SECS)
}

/// Pure decision for the steer-idle interrupt: fire iff the knob is on, steers
/// are actually queued, and the stream has been silent for the whole threshold
/// window. Kept pure so the off / none-queued / under / over cases unit-test
/// without a live turn.
pub(crate) fn steer_interrupt_due(
    idle_secs: u64,
    queued_steers: usize,
    threshold_secs: u64,
) -> bool {
    threshold_secs > 0 && queued_steers > 0 && idle_secs >= threshold_secs
}

/// Keep a defect in a provider/parser/tool hop from silently dropping the turn
/// result channel. Mark the panic as contained so the terminal panic hook does
/// not dismantle the still-live cockpit; the UI receives a stable error and can
/// preserve the already-visible stream.
fn run_worker_guarded<T>(work: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    match crate::term::catch_background_unwind(work) {
        Ok(result) => result,
        Err(_) => Err(
            "agent worker panicked — turn stopped safely; retry or inspect provider/tool logs"
                .to_string(),
        ),
    }
}

/// Cumulative club meters captured before this turn's hops. The club reports
/// running totals, so `App::fold_turn_cache` subtracts this snapshot at drain.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SpawnUsageSnapshot {
    pub(crate) usage_before: Option<TokenUsage>,
    pub(crate) cache_before: CacheUsage,
}

#[cfg(test)]
pub(crate) fn published_spawn_usage(
    usage_before: Option<TokenUsage>,
    cache_before: CacheUsage,
) -> Arc<OnceLock<SpawnUsageSnapshot>> {
    let slot = Arc::new(OnceLock::new());
    let _ = slot.set(SpawnUsageSnapshot {
        usage_before,
        cache_before,
    });
    slot
}

/// An in-flight agent turn running off the UI thread.
pub(crate) struct Thinking {
    pub(crate) started: Instant,
    pub(crate) club_label: String,
    /// Soft interrupt: the UI sets this; `run_turn` checks it at hop boundaries.
    pub(crate) cancel: Arc<AtomicBool>,
    /// The exact club this turn was spawned with, kept for live UI telemetry. The
    /// selected bag can change while a turn is in flight, so meters must not look up
    /// the current in-hand tab and accidentally describe a different backend.
    pub(crate) club: Option<Arc<dyn Club>>,
    /// Club token/cache totals snapshotted inside the worker before hops.
    /// `token_usage` / `cache_usage` take club mutexes; doing that on the UI
    /// spawn path hitches Enter-after-echo. Live meters read `club` directly.
    /// Fold only needs this slot after the worker has started (and published).
    pub(crate) spawn_usage: Arc<OnceLock<SpawnUsageSnapshot>>,
    /// Route requested when the worker was spawned. Kept beside the resolved
    /// route returned by the worker so the UI can truthfully expose failover.
    pub(crate) requested_route: RouteIdentity,
    /// Worker returns the full updated conversation, final answer text, and the
    /// route that actually produced it. Capturing route identity on the worker
    /// closes the gap where the operator could change the selected Bag route
    /// before the UI harvested the answer.
    pub(crate) rx: mpsc::Receiver<TurnResult>,
    /// Live tool-call events from the worker, drained by the UI.
    pub(crate) event_rx: mpsc::Receiver<TurnEvent>,
    /// Last wall-clock instant we received ANY stream event from the worker.
    /// The turn watchdog uses this to detect a stuck turn (provider socket
    /// went silent and never closed) and abandon it instead of spinning the
    /// UI at 30fps forever.
    pub(crate) last_stream_at: Instant,
    /// Idle-watchdog deadline snapshotted for this turn. Reading the process
    /// environment here, rather than on every UI frame, lets a changed setting
    /// govern the next turn without taxing or mutating one already in flight.
    pub(crate) idle_timeout_secs: Option<u64>,
    /// One-shot idle-watchdog escalation stages (50% / 80% of the idle
    /// timeout) already announced for the CURRENT stall. Stream progress
    /// re-arms both (see [`Thinking::note_stream_progress`]), so each distinct
    /// stall escalates again; within one stall each stage speaks exactly once
    /// no matter how many UI frames observe it.
    pub(crate) idle_warned_50: bool,
    pub(crate) idle_warned_80: bool,
    /// Steer-idle interrupt threshold snapshotted for this turn
    /// (`ANGEL_STEER_IDLE_INTERRUPT_SECS`; `0` = off). Read at spawn, like the
    /// idle timeout, so a changed setting governs the next turn without a
    /// per-frame getenv.
    pub(crate) steer_idle_interrupt_secs: u64,
    /// The steer-idle interrupt already fired for the CURRENT stall. One
    /// interrupt per stall: only stream progress re-arms it (see
    /// [`Thinking::note_stream_progress`]).
    pub(crate) steer_interrupt_fired: bool,
    /// The operator has abandoned this turn, but its worker has not yet
    /// reached the result-channel quiescence boundary. While true, the UI
    /// suppresses every late event/result and keeps the shared foreground
    /// flight slot reserved so a replacement cannot overlap the same
    /// provider/tool lifecycle.
    pub(crate) draining: bool,
}

/// One due idle-watchdog escalation notice: how stalled the turn is right now
/// and which one-shot stage (50 or 80 percent of the idle timeout) this
/// crossing consumed.
pub(crate) struct IdleWarning {
    pub(crate) stage_pct: u8,
    pub(crate) idle_secs: u64,
    pub(crate) idle_timeout_secs: u64,
}

impl Drop for Thinking {
    fn drop(&mut self) {
        self.discard_buffered_stream();
    }
}

impl Thinking {
    /// Closing a cancelled stream may destroy a large queued burst. Move that
    /// destruction off the UI thread, including at the terminal result boundary.
    pub(crate) fn discard_buffered_stream(&mut self) {
        let Ok(first) = self.event_rx.try_recv() else {
            return;
        };
        let (_, empty) = mpsc::channel();
        let receiver = std::mem::replace(&mut self.event_rx, empty);
        let _ = std::thread::Builder::new()
            .name("retired-stream".into())
            .spawn(move || {
                drop(first);
                drop(receiver);
            });
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn spawn(
        club_label: String,
        club: Arc<dyn Club>,
        tools: Arc<ToolRegistry>,
        convo: Arc<[ChatMsg]>,
        steers: Arc<SteerQueue>,
        session: Session,
        requested_route: RouteIdentity,
        loop_owner: Option<String>,
    ) -> Self {
        let idle_timeout_secs = crate::club::model_defaults::budgets(
            &club.model_identity().unwrap_or_default(),
            club.env_namespace().unwrap_or(club.label()),
        )["turn_idle_timeout_secs"]
            .as_u64()
            .filter(|seconds| *seconds != 0);
        let steer_idle_interrupt_secs = configured_steer_idle_interrupt_secs();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let ui_club = Arc::clone(&club);
        let spawn_usage = Arc::new(OnceLock::new());
        let worker_spawn_usage = Arc::clone(&spawn_usage);
        let (tx, rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel::<TurnEvent>();
        let worker_tx = tx.clone();
        let worker_route = requested_route.clone();
        let spawn_result = std::thread::Builder::new()
            .name("agent-turn".to_string())
            .spawn(move || {
                // Ownership travels with this launch, never a global registry
                // pointer that a failed spawn could leave for a later turn.
                let _owner = crate::harness::run_identity::LiveTurnScope::enter(loop_owner);
                // One Vec clone of message headers; content bytes stay shared.
                // Taken here so Enter-after-echo does not pay a second snapshot.
                let _phase = crate::turn_phase::Scope::enter();
                crate::turn_phase::mark("worker_snapshot");
                let mut convo = convo.to_vec();
                // Hop cap is launch config. Read it here, not on the UI spawn
                // path, so a getenv cannot hitch Enter-after-echo.
                let max_hops = crate::harness::configured_max_hops();
                // Mutex snapshots of cumulative club meters. Folded at drain
                // (`App::fold_turn_cache`); live UI meters read `thinking.club`.
                // Taken here — before lease, vision sidecar, or hops — so
                // Enter-after-echo never waits on those locks.
                let _ = worker_spawn_usage.set(SpawnUsageSnapshot {
                    usage_before: club.token_usage(),
                    cache_before: club.cache_usage(),
                });
                // Foreground turns are unbounded unless the operator explicitly set
                // a positive ANGEL_MAX_HOPS. Semantic anti-spin/error watchdogs still
                // catch dead behavior without mistaking productive tool volume for a
                // completion metric. The steer queue is drained at hop boundaries so
                // the user can redirect long work without killing it.
                // Foreground lease wait stays off the UI thread so a contended
                // registry cannot hitch Enter-after-echo.
                crate::turn_phase::mark("foreground_lease");
                let foreground_lease =
                    if crate::backplane::mode() == crate::backplane::BackplaneMode::Legacy {
                        Ok(None)
                    } else {
                        let backplane = tools.backplane();
                        backplane
                            .resource_group_for_identity(&worker_route)
                            .map(|(route_id, resource_group)| {
                                backplane.acquire_scoped(
                                    &resource_group,
                                    crate::backplane::LeaseMode::Serve,
                                    crate::backplane::WorkloadRole::Foreground,
                                    Some(route_id),
                                    false,
                                )
                            })
                            .transpose()
                    };
                let result = match foreground_lease {
                    Err(error) => Err(format!("foreground route resource conflict: {error}")),
                    Ok(_lease) => run_worker_guarded(|| {
                        // Vision sidecar rewrite is network-bound. Fold it here
                        // on agent-turn before hop 1 so Enter-after-echo never
                        // waits on the VLM, and the text-only driver never sees
                        // bare image_url parts.
                        crate::turn_phase::mark("vision_preflight");
                        for notice in
                            crate::tools::vision::fold_vision_sidecar_into_convo(&*club, &mut convo)
                        {
                            let _ = event_tx.send(TurnEvent::Notice(notice));
                        }
                        // The harness invokes this immediately before every
                        // provider attempt and tool dispatch, after any steer,
                        // compaction, retry nudge, or tool result changed the
                        // exact model-facing thread. Persistence remains off
                        // the draw thread.
                        let checkpoint = |prefix: &[ChatMsg]| {
                            session
                                .checkpoint(prefix)
                                .map_err(|error| error.to_string())
                        };
                        run_turn_steered_checkpointed_observed(
                            &*club,
                            &tools,
                            &mut convo,
                            &worker_cancel,
                            max_hops,
                            &event_tx,
                            Some(&steers),
                            &checkpoint,
                        )
                        .map_err(|failure| failure.message)
                        .map(|outcome| {
                            let route = club.resolved_route_identity();
                            (convo, outcome.answer, route, outcome.stop_reason)
                        })
                    }),
                };
                crate::turn_phase::mark("worker_result");
                let _ = worker_tx.send(result);
            });
        if let Err(error) = spawn_result {
            let _ = tx.send(Err(format!("could not start agent worker: {error}")));
        }
        Self {
            started: Instant::now(),
            club_label,
            cancel,
            club: Some(ui_club),
            spawn_usage,
            requested_route,
            rx,
            event_rx,
            last_stream_at: Instant::now(),
            idle_timeout_secs,
            idle_warned_50: false,
            idle_warned_80: false,
            steer_idle_interrupt_secs,
            steer_interrupt_fired: false,
            draining: false,
        }
    }

    /// Retire the visible turn while retaining ownership until the worker's
    /// result sender settles. Idempotent so watchdog and operator races share
    /// one lifecycle transition.
    pub(crate) fn begin_draining(&mut self) {
        self.cancel
            .store(true, std::sync::atomic::Ordering::Release);
        self.draining = true;
    }

    pub(crate) fn is_draining(&self) -> bool {
        self.draining
    }

    /// Record live stream activity: refreshes the idle watchdog and re-arms
    /// its one-shot warnings (and the steer-idle interrupt) so a later,
    /// separate stall escalates again.
    pub(crate) fn note_stream_progress(&mut self) {
        self.last_stream_at = Instant::now();
        self.idle_warned_50 = false;
        self.idle_warned_80 = false;
        self.steer_interrupt_fired = false;
    }

    /// Which escalating idle-watchdog warning is due right now, if any,
    /// marking it consumed. The 80% stage subsumes 50% so a long frame gap can
    /// never double-announce; thresholds that truncate to zero (tiny timeouts)
    /// never warn — abandonment handles those on its own.
    pub(crate) fn idle_warning_due(&mut self) -> Option<IdleWarning> {
        let idle_timeout_secs = self.idle_timeout_secs?;
        let idle_secs = self.last_stream_at.elapsed().as_secs();
        let warn_80 = idle_timeout_secs.saturating_mul(4) / 5;
        let warn_50 = idle_timeout_secs / 2;
        if !self.idle_warned_80 && warn_80 > 0 && idle_secs >= warn_80 {
            self.idle_warned_80 = true;
            self.idle_warned_50 = true;
            return Some(IdleWarning {
                stage_pct: 80,
                idle_secs,
                idle_timeout_secs,
            });
        }
        if !self.idle_warned_50 && warn_50 > 0 && idle_secs >= warn_50 {
            self.idle_warned_50 = true;
            return Some(IdleWarning {
                stage_pct: 50,
                idle_secs,
                idle_timeout_secs,
            });
        }
        None
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn progress(&self) -> (f32, f32, &str) {
        let secs = self.started.elapsed().as_secs_f32();
        let progress = 1.0 - (-secs / 2.5_f32).exp();
        let label = if self.draining {
            "draining stopped worker"
        } else {
            self.club_label.as_str()
        };
        (progress, secs, label)
    }

    #[cfg(test)]
    pub(crate) fn pending_for_test(club_label: impl Into<String>) -> Self {
        let (_tx, rx) = mpsc::channel();
        let (_event_tx, event_rx) = mpsc::channel();
        Self {
            started: Instant::now(),
            club_label: club_label.into(),
            cancel: Arc::new(AtomicBool::new(false)),
            club: None,
            spawn_usage: published_spawn_usage(None, CacheUsage::default()),
            requested_route: RouteIdentity {
                driver: "practice".to_string(),
                model: None,
                reasoning_effort: None,
            },
            rx,
            event_rx,
            last_stream_at: Instant::now(),
            idle_timeout_secs: configured_turn_idle_timeout_secs(),
            idle_warned_50: false,
            idle_warned_80: false,
            steer_idle_interrupt_secs: configured_steer_idle_interrupt_secs(),
            steer_interrupt_fired: false,
            draining: false,
        }
    }
}

#[cfg(test)]
mod idle_timeout_tests {
    use super::*;

    #[test]
    fn worker_panic_returns_an_actionable_error_instead_of_disconnect() {
        let error = run_worker_guarded(|| -> Result<(), String> {
            panic!("synthetic provider panic");
        })
        .expect_err("panic must become a turn error");

        assert!(error.contains("worker panicked"), "{error}");
        assert!(error.contains("retry"), "{error}");
    }

    #[test]
    fn turn_idle_timeout_defaults_disables_and_falls_back() {
        let _env = crate::tests::env_lock();

        {
            let _unset = crate::tests::TestEnvGuard::unset("ANGEL_TURN_IDLE_TIMEOUT_SECS");
            assert_eq!(configured_turn_idle_timeout_secs(), None);
        }
        {
            let _disabled = crate::tests::TestEnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "0");
            assert_eq!(configured_turn_idle_timeout_secs(), None);
        }
        {
            let _invalid =
                crate::tests::TestEnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "not-seconds");
            assert_eq!(configured_turn_idle_timeout_secs(), None);
        }
    }

    #[test]
    fn steer_idle_interrupt_defaults_disables_and_falls_back() {
        let _env = crate::tests::env_lock();

        {
            let _unset = crate::tests::TestEnvGuard::unset("ANGEL_STEER_IDLE_INTERRUPT_SECS");
            assert_eq!(configured_steer_idle_interrupt_secs(), 60);
        }
        {
            let _disabled = crate::tests::TestEnvGuard::set("ANGEL_STEER_IDLE_INTERRUPT_SECS", "0");
            assert_eq!(configured_steer_idle_interrupt_secs(), 0);
        }
        {
            let _invalid =
                crate::tests::TestEnvGuard::set("ANGEL_STEER_IDLE_INTERRUPT_SECS", "not-seconds");
            assert_eq!(configured_steer_idle_interrupt_secs(), 60);
        }
    }

    #[test]
    fn steer_interrupt_due_covers_off_none_queued_under_and_over() {
        // Knob off: never, no matter how idle or queued.
        assert!(!steer_interrupt_due(10_000, 3, 0));
        // Nothing queued: never — there is nothing to deliver.
        assert!(!steer_interrupt_due(10_000, 0, 60));
        // Under the threshold: the stream may still be warming up.
        assert!(!steer_interrupt_due(59, 1, 60));
        // At and over the threshold with steers waiting: fire.
        assert!(steer_interrupt_due(60, 1, 60));
        assert!(steer_interrupt_due(600, 3, 60));
    }

    #[test]
    fn stream_progress_rearms_the_steer_interrupt_flag() {
        let mut thinking = Thinking::pending_for_test("practice");
        thinking.steer_interrupt_fired = true;
        thinking.note_stream_progress();
        assert!(
            !thinking.steer_interrupt_fired,
            "a fresh stall earns one new interrupt"
        );
        thinking.steer_interrupt_fired = true;
        assert!(thinking.steer_interrupt_fired, "otherwise it stays spent");
    }

    #[test]
    fn turn_idle_timeout_is_stable_in_flight_and_refreshes_next_turn() {
        let _env = crate::tests::env_lock();
        let _first_value = crate::tests::TestEnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "17");
        let first = Thinking::pending_for_test("first");

        {
            let _next_value = crate::tests::TestEnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "23");
            let second = Thinking::pending_for_test("second");
            assert_eq!(first.idle_timeout_secs, Some(17));
            assert_eq!(second.idle_timeout_secs, Some(23));
        }

        assert_eq!(
            first.idle_timeout_secs,
            Some(17),
            "later environment changes cannot rewrite an in-flight watchdog"
        );
    }

    fn aged(secs: u64) -> Instant {
        Instant::now()
            .checked_sub(std::time::Duration::from_secs(secs))
            .expect("monotonic clock supports backdating in tests")
    }

    #[test]
    fn idle_warnings_fire_exactly_once_per_stage_with_aged_stream() {
        let mut thinking = Thinking::pending_for_test("practice");
        thinking.idle_timeout_secs = Some(600);
        // Aged past 50% (300s) but not 80% (480s).
        thinking.last_stream_at = aged(301);
        let warning = thinking.idle_warning_due().expect("50% stage due");
        assert_eq!(warning.stage_pct, 50);
        assert!(warning.idle_secs >= 300);
        assert_eq!(warning.idle_timeout_secs, 600);
        assert!(
            thinking.idle_warning_due().is_none(),
            "50% is one-shot across frames"
        );
        // The stall deepens past 80%.
        thinking.last_stream_at = aged(481);
        let warning = thinking.idle_warning_due().expect("80% stage due");
        assert_eq!(warning.stage_pct, 80);
        assert!(
            thinking.idle_warning_due().is_none(),
            "80% is one-shot across frames"
        );
    }

    #[test]
    fn idle_warning_80_subsumes_50_and_progress_rearms_both() {
        let mut thinking = Thinking::pending_for_test("practice");
        thinking.idle_timeout_secs = Some(100);
        // First observation is already past 80%: exactly one notice.
        thinking.last_stream_at = aged(90);
        assert_eq!(thinking.idle_warning_due().expect("80% due").stage_pct, 80);
        assert!(
            thinking.idle_warning_due().is_none(),
            "the skipped 50% stage must not fire late"
        );
        // Stream progress re-arms; a fresh stall escalates from 50% again.
        thinking.note_stream_progress();
        assert!(
            thinking.idle_warning_due().is_none(),
            "fresh stream activity means no warning"
        );
        thinking.last_stream_at = aged(50);
        assert_eq!(thinking.idle_warning_due().expect("re-armed").stage_pct, 50);
    }

    #[test]
    fn idle_warnings_respect_disabled_and_tiny_timeouts() {
        let mut thinking = Thinking::pending_for_test("practice");
        thinking.idle_timeout_secs = None;
        thinking.last_stream_at = aged(10_000);
        assert!(
            thinking.idle_warning_due().is_none(),
            "disabled watchdog never warns"
        );
        // Thresholds that truncate to zero must never warn at idle 0.
        thinking.idle_timeout_secs = Some(1);
        thinking.last_stream_at = Instant::now();
        assert!(thinking.idle_warning_due().is_none());
    }
}

/// One completed turn's provider-reported cache economics, as folded into the
/// session meter. `reported` is false when the provider sent no cache
/// accounting at all that turn (local backends never do).
#[derive(Clone, Copy, Debug)]
pub(crate) struct TurnCacheSample {
    pub(crate) cache_read: u64,
    pub(crate) input: u64,
    pub(crate) reported: bool,
}

/// Session-aggregate prompt-cache hit accounting, folded from per-turn deltas
/// as each worker completes. Turns whose provider reported no cache fields are
/// excluded from the session sums so a local backend can't dilute a metered
/// club's rate — and until any turn reports, the display says "n/a", never a
/// fake 0%.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CacheMeter {
    pub(crate) session_cache_read: u64,
    pub(crate) session_input: u64,
    pub(crate) session_reported: bool,
    pub(crate) last_turn: Option<TurnCacheSample>,
}

impl CacheMeter {
    pub(crate) fn fold_turn(&mut self, cache_read: u64, input: u64, reported: bool) {
        if reported {
            self.session_cache_read = self.session_cache_read.saturating_add(cache_read);
            self.session_input = self.session_input.saturating_add(input);
            self.session_reported = true;
        }
        self.last_turn = Some(TurnCacheSample {
            cache_read,
            input,
            reported,
        });
    }

    /// One `/usage`-ready line: session aggregate + most recent turn.
    pub(crate) fn usage_line(&self) -> String {
        let session = rate_text(
            self.session_reported,
            self.session_cache_read,
            self.session_input,
        );
        let last = match self.last_turn {
            Some(turn) => rate_text(turn.reported, turn.cache_read, turn.input),
            None => "n/a".to_string(),
        };
        format!("hit {session} session · {last} last turn")
    }
}

fn rate_text(_reported: bool, _cache_read: u64, _input: u64) -> String {
    // These legacy UI totals carry no provider contract. Keep the visible
    // percentage unknown until CacheMeter consumes coherent accounting views.
    "n/a".to_string()
}

/// Percentage of prompt tokens served from the provider's cache. OpenAI-style
/// usage counts cached tokens inside the reported input (`prompt_tokens`);
/// Anthropic-style reports them disjointly — so when reads exceed the reported
/// input, the denominator is the recombined `input + read`.
pub(crate) fn cache_hit_pct(cache_read: u64, input: u64) -> Option<u64> {
    let denominator = if cache_read > input {
        input.saturating_add(cache_read)
    } else {
        input
    };
    if denominator == 0 {
        return None;
    }
    Some((cache_read as f64 / denominator as f64 * 100.0).round() as u64)
}

// ---------------------------------------------------------------------------
// Per-hop cache ledger (perf-cache-ledger): measurement-only receipts.
//
// The session `CacheMeter` above answers "how did the whole turn do?"; it
// cannot say WHICH hop lost the provider prefix or WHY. The ledger keeps one
// sample per provider request — deltas of the club's cumulative cache/token
// counters snapshotted around each `chat_streaming` call in the harness —
// tagged with the known prefix breakers that ran since the prior request
// (defs delta, aging/dedup receipts, compaction splice, steer injection…).
// Rows are folded per club label and rendered as extra `/usage` lines; a hop
// whose hit rate falls names its breakers in an operator Notice. Nothing here
// mutates a request: this is the receipt machine that gates the next wave's
// prefix-stability plays.
// ---------------------------------------------------------------------------

/// Denominators below this many prompt tokens are numerically too noisy for a
/// hit-rate comparison to indict a prefix breaker — tiny probe requests swing
/// tens of points on a handful of tokens.
const MIN_DROP_DENOMINATOR_TOKENS: u64 = 512;
/// Provider tokenizers and cache blocks can move by a small amount between
/// otherwise stable requests. A percentage fall is not a prefix loss unless
/// the absolute cached prefix also shrank by at least this much.
const MIN_PREFIX_LOSS_TOKENS: u64 = 256;

/// One provider request's prompt-cache economics: deltas of the club's
/// cumulative counters around a single streaming call. `reported` mirrors
/// [`TurnCacheSample`] semantics — false when the provider sent no cache
/// accounting for this hop, so a local backend never dilutes a metered rate.
#[derive(Clone, Debug, Default)]
pub(crate) struct HopCacheSample {
    pub(crate) hop: usize,
    pub(crate) input: u64,
    pub(crate) cache_read: u64,
    pub(crate) cache_write: u64,
    pub(crate) reported: bool,
    /// Known prefix breakers applied to the request since the previous hop.
    pub(crate) breakers: Vec<&'static str>,
}

/// Per-route (club label) fold of hop samples: one `/usage` table row plus the
/// miss-attribution counters behind the drop notices.
#[derive(Clone, Debug, Default)]
pub(crate) struct ClubCacheLedger {
    pub(crate) hops: u64,
    pub(crate) reported_hops: u64,
    pub(crate) input: u64,
    pub(crate) cache_read: u64,
    pub(crate) cache_write: u64,
    pub(crate) drops: u64,
    /// Hit-rate declines where the absolute cached prefix held or grew. These
    /// are healthy uncached-tail growth, not cache invalidation.
    pub(crate) dilutions: u64,
    /// breaker tag → hops on which it preceded a hit-rate drop.
    pub(crate) breaker_drops: BTreeMap<&'static str, u64>,
    /// Drops with no tagged breaker (provider-side eviction, schema drift…).
    pub(crate) unattributed_drops: u64,
    /// Hit rate of the most recent reported hop, the drop comparison baseline.
    /// Deliberately spans turn boundaries: under cache-first defaults the prior
    /// turn's prefix should still hit, so a boundary fall is a real finding.
    last_rate: Option<u64>,
    /// Absolute cached prefix on the most recent reported hop. Hit percentages
    /// alone are insufficient: appending a large new tool result can halve the
    /// ratio while preserving every cached prefix token.
    last_cache_read: Option<u64>,
}

impl ClubCacheLedger {
    /// Fold one hop sample. Returns the operator notice when this reported
    /// hop's hit rate fell at least `drop_threshold_pct` points (minimum 1 — a
    /// zero threshold still requires an actual fall) below the previous
    /// reported hop on this route. A decline with a stable absolute cache read
    /// is classified as tail dilution; only a material absolute read loss is a
    /// cache drop. Unreported hops accumulate token totals but never move either
    /// baseline.
    pub(crate) fn fold_hop(
        &mut self,
        label: &str,
        sample: &HopCacheSample,
        drop_threshold_pct: u64,
    ) -> Option<String> {
        self.hops = self.hops.saturating_add(1);
        self.input = self.input.saturating_add(sample.input);
        self.cache_read = self.cache_read.saturating_add(sample.cache_read);
        self.cache_write = self.cache_write.saturating_add(sample.cache_write);
        if !sample.reported {
            return None;
        }
        self.reported_hops = self.reported_hops.saturating_add(1);
        let rate = cache_hit_pct(sample.cache_read, sample.input)?;
        let prev = self.last_rate.replace(rate);
        let prev_cache_read = self.last_cache_read.replace(sample.cache_read);
        let (prev, prev_cache_read) = match (prev, prev_cache_read) {
            (Some(rate), Some(read)) => (rate, read),
            _ => return None,
        };
        let denominator = sample.input.max(sample.cache_read);
        if denominator < MIN_DROP_DENOMINATOR_TOKENS {
            return None;
        }
        if prev.saturating_sub(rate) < drop_threshold_pct.max(1) {
            return None;
        }
        let read_loss = prev_cache_read.saturating_sub(sample.cache_read);
        if read_loss < MIN_PREFIX_LOSS_TOKENS {
            self.dilutions = self.dilutions.saturating_add(1);
            return Some(format!(
                "cache ledger: {label} hop {} hit {prev}% → {rate}% from uncached tail growth; prefix read held {} → {}",
                sample.hop, prev_cache_read, sample.cache_read
            ));
        }
        self.drops = self.drops.saturating_add(1);
        let cause = if sample.breakers.is_empty() {
            self.unattributed_drops = self.unattributed_drops.saturating_add(1);
            "no tagged prefix breaker (provider-side or schema drift)".to_string()
        } else {
            for tag in &sample.breakers {
                *self.breaker_drops.entry(tag).or_default() += 1;
            }
            sample.breakers.join(" + ")
        };
        Some(format!(
            "cache ledger: {label} hop {} prefix read {} → {}; hit {prev}% → {rate}% after {cause}",
            sample.hop, prev_cache_read, sample.cache_read
        ))
    }

    /// One `/usage`-ready row for this route.
    pub(crate) fn usage_row(&self, label: &str) -> String {
        if self.reported_hops == 0 {
            return format!("{label} · {} hop(s) · cache n/a", self.hops);
        }
        let rate = rate_text(true, self.cache_read, self.input);
        let mut row = format!(
            "{label} · {} hop(s) ({} reported) · hit {rate} · in {} · read {}",
            self.hops,
            self.reported_hops,
            fmt_tokens(self.input),
            fmt_tokens(self.cache_read),
        );
        if self.cache_write > 0 {
            row.push_str(&format!(" · wrote {}", fmt_tokens(self.cache_write)));
        }
        if self.dilutions > 0 {
            row.push_str(&format!(" · {} tail dilution(s)", self.dilutions));
        }
        if self.drops > 0 {
            row.push_str(&format!(" · {} drop(s)", self.drops));
            let mut causes: Vec<String> = self
                .breaker_drops
                .iter()
                .map(|(tag, count)| format!("{tag} ×{count}"))
                .collect();
            if self.unattributed_drops > 0 {
                causes.push(format!("untagged ×{}", self.unattributed_drops));
            }
            row.push_str(&format!(" → {}", causes.join(", ")));
        }
        row
    }
}

/// Compact token counts for ledger rows: raw below 10k, then k / M.
fn fmt_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 10_000 {
        format!("{}k", tokens / 1_000)
    } else {
        tokens.to_string()
    }
}

/// Cheap fingerprint of the tool-definition set offered to the provider. Tool
/// schemas serialize ahead of the conversation in the request body, so any
/// change invalidates the whole provider prefix. Name list + description
/// length is a stable proxy: schema activation/deactivation always changes the
/// name set, and in-place schema edits mid-run do not occur in this codebase.
pub(crate) fn defs_fingerprint(defs: &[ToolDef]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    defs.len().hash(&mut hasher);
    for def in defs {
        def.name.hash(&mut hasher);
        def.description.len().hash(&mut hasher);
    }
    hasher.finish()
}

/// Master switch for the per-hop cache ledger. Measurement-only, so it
/// defaults on; `ANGEL_CACHE_HOP_LEDGER=0` silences both the sampling and the
/// drop notices (and `/usage` says so rather than showing nothing).
pub(crate) fn cache_hop_ledger_enabled() -> bool {
    crate::harness::env_flag("ANGEL_CACHE_HOP_LEDGER", true)
}

/// Percentage-point fall (vs the previous reported hop) that earns a drop
/// notice. Values ≤ 1 mean "any fall".
fn cache_ledger_drop_threshold_pct() -> u64 {
    crate::harness::env_usize("ANGEL_CACHE_LEDGER_DROP_PCT", 20) as u64
}

/// Process-wide per-route ledger. Deliberately outlives `/new`: it is a
/// measurement receipt across the whole cockpit process, like the club's own
/// cumulative counters it samples.
static CACHE_LEDGER: OnceLock<Mutex<BTreeMap<String, ClubCacheLedger>>> = OnceLock::new();

fn cache_ledger() -> &'static Mutex<BTreeMap<String, ClubCacheLedger>> {
    CACHE_LEDGER.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Fold one hop into the per-route ledger; returns the drop notice to emit,
/// if any. Called from the harness worker thread once per provider call.
pub(crate) fn cache_ledger_fold(label: &str, sample: HopCacheSample) -> Option<String> {
    let mut map = cache_ledger()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    map.entry(label.to_string()).or_default().fold_hop(
        label,
        &sample,
        cache_ledger_drop_threshold_pct(),
    )
}

/// `/usage` rows: one per route that made at least one provider call, or the
/// explicit "off" line when the operator disabled the ledger — a silenced gate
/// must still speak.
pub(crate) fn cache_ledger_lines() -> Vec<String> {
    if !cache_hop_ledger_enabled() {
        return vec!["per-hop ledger off (ANGEL_CACHE_HOP_LEDGER=0)".to_string()];
    }
    let map = cache_ledger()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    map.iter()
        .map(|(label, ledger)| ledger.usage_row(label))
        .collect()
}

#[cfg(test)]
mod cache_ledger_tests {
    use super::*;

    fn reported(hop: usize, input: u64, cache_read: u64) -> HopCacheSample {
        HopCacheSample {
            hop,
            input,
            cache_read,
            cache_write: 0,
            reported: true,
            breakers: Vec::new(),
        }
    }

    #[test]
    fn first_reported_hop_never_notices_and_totals_accumulate() {
        let mut ledger = ClubCacheLedger::default();
        let notice = ledger.fold_hop("glm", &reported(1, 10_000, 9_000), 20);
        assert!(notice.is_none(), "no baseline yet — nothing to drop from");
        assert_eq!(ledger.hops, 1);
        assert_eq!(ledger.reported_hops, 1);
        assert_eq!(ledger.input, 10_000);
        assert_eq!(ledger.cache_read, 9_000);
    }

    #[test]
    fn drop_names_the_tagged_breakers_and_counts_attribution() {
        let mut ledger = ClubCacheLedger::default();
        assert!(
            ledger
                .fold_hop("glm", &reported(1, 10_000, 9_000), 20)
                .is_none()
        );
        let mut broken = reported(2, 10_000, 3_000);
        broken.breakers = vec!["defs delta", "steer injection"];
        let notice = ledger
            .fold_hop("glm", &broken, 20)
            .expect("a 60-point fall past a 20-point threshold notices");
        assert_eq!(
            notice,
            "cache ledger: glm hop 2 prefix read 9000 → 3000; hit 90% → 30% after defs delta + steer injection"
        );
        assert_eq!(ledger.drops, 1);
        assert_eq!(ledger.breaker_drops.get("defs delta"), Some(&1));
        assert_eq!(ledger.breaker_drops.get("steer injection"), Some(&1));
        assert_eq!(ledger.unattributed_drops, 0);
    }

    #[test]
    fn untagged_drop_says_so_instead_of_staying_silent() {
        let mut ledger = ClubCacheLedger::default();
        assert!(
            ledger
                .fold_hop("glm", &reported(1, 10_000, 9_000), 20)
                .is_none()
        );
        let notice = ledger
            .fold_hop("glm", &reported(2, 10_000, 0), 20)
            .expect("a full miss notices");
        assert!(
            notice.contains("no tagged prefix breaker"),
            "unattributed drops must name their own uncertainty: {notice}"
        );
        assert_eq!(ledger.unattributed_drops, 1);
    }

    #[test]
    fn growing_uncached_tail_dilutes_ratio_without_claiming_prefix_loss() {
        let mut ledger = ClubCacheLedger::default();
        assert!(
            ledger
                .fold_hop("glm", &reported(1, 7_829, 7_808), 20)
                .is_none()
        );
        let notice = ledger
            .fold_hop("glm", &reported(2, 15_626, 7_808), 20)
            .expect("a significant dilution stays visible but is not a drop");
        assert_eq!(
            notice,
            "cache ledger: glm hop 2 hit 100% → 50% from uncached tail growth; prefix read held 7808 → 7808"
        );
        assert_eq!(ledger.drops, 0);
        assert_eq!(ledger.dilutions, 1);
        assert_eq!(ledger.unattributed_drops, 0);
        assert!(ledger.usage_row("glm").contains("1 tail dilution(s)"));
    }

    #[test]
    fn falls_below_threshold_and_rises_never_notice() {
        let mut ledger = ClubCacheLedger::default();
        assert!(
            ledger
                .fold_hop("glm", &reported(1, 10_000, 9_000), 20)
                .is_none()
        );
        // 90% → 80% is under the 20-point threshold.
        assert!(
            ledger
                .fold_hop("glm", &reported(2, 10_000, 8_000), 20)
                .is_none()
        );
        // Recovery is never a drop.
        assert!(
            ledger
                .fold_hop("glm", &reported(3, 10_000, 9_500), 20)
                .is_none()
        );
        assert_eq!(ledger.drops, 0);
    }

    #[test]
    fn zero_threshold_still_requires_an_actual_fall() {
        let mut ledger = ClubCacheLedger::default();
        assert!(
            ledger
                .fold_hop("glm", &reported(1, 10_000, 9_000), 0)
                .is_none()
        );
        // Identical rate: no fall, no notice even at threshold 0.
        assert!(
            ledger
                .fold_hop("glm", &reported(2, 10_000, 9_000), 0)
                .is_none()
        );
        // A 1-point fall does notice at threshold 0 (treated as 1).
        assert!(
            ledger
                .fold_hop("glm", &reported(3, 10_000, 8_900), 0)
                .is_some()
        );
    }

    #[test]
    fn tiny_denominators_are_too_noisy_to_indict_a_breaker() {
        let mut ledger = ClubCacheLedger::default();
        assert!(ledger.fold_hop("glm", &reported(1, 400, 380), 20).is_none());
        // 95% → 0% but over 400 tokens: numerically meaningless, exempt.
        assert!(ledger.fold_hop("glm", &reported(2, 400, 0), 20).is_none());
        assert_eq!(ledger.drops, 0);
    }

    #[test]
    fn unreported_hops_accumulate_totals_but_never_move_the_baseline() {
        let mut ledger = ClubCacheLedger::default();
        assert!(
            ledger
                .fold_hop("glm", &reported(1, 10_000, 9_000), 20)
                .is_none()
        );
        let mut blind = reported(2, 5_000, 0);
        blind.reported = false;
        assert!(ledger.fold_hop("glm", &blind, 20).is_none());
        assert_eq!(ledger.hops, 2);
        assert_eq!(ledger.reported_hops, 1);
        assert_eq!(ledger.input, 15_000, "blind tokens still count in totals");
        // The next reported hop compares against hop 1's 90%, not the blind hop.
        assert!(
            ledger
                .fold_hop("glm", &reported(3, 10_000, 3_000), 20)
                .is_some()
        );
    }

    #[test]
    fn usage_row_covers_blind_and_metered_routes() {
        let mut ledger = ClubCacheLedger::default();
        let mut blind = reported(1, 5_000, 0);
        blind.reported = false;
        assert!(ledger.fold_hop("nex2", &blind, 20).is_none());
        assert_eq!(ledger.usage_row("nex2"), "nex2 · 1 hop(s) · cache n/a");

        let mut metered = ClubCacheLedger::default();
        assert!(
            metered
                .fold_hop("glm", &reported(1, 10_000, 9_000), 20)
                .is_none()
        );
        let mut broken = reported(2, 10_000, 3_000);
        broken.breakers = vec!["compaction splice"];
        assert!(metered.fold_hop("glm", &broken, 20).is_some());
        assert!(
            metered
                .fold_hop("glm", &reported(3, 10_000, 0), 20)
                .is_some()
        );
        let row = metered.usage_row("glm");
        // Legacy counters have no qualified cache convention; raw totals do
        // not justify a percentage even when every hop reports numbers.
        assert_eq!(
            row,
            "glm · 3 hop(s) (3 reported) · hit n/a · in 30k · read 12k · 2 drop(s) \
             → compaction splice ×1, untagged ×1"
        );
    }

    #[test]
    fn fmt_tokens_scales_readably() {
        assert_eq!(fmt_tokens(0), "0");
        assert_eq!(fmt_tokens(9_999), "9999");
        assert_eq!(fmt_tokens(10_000), "10k");
        assert_eq!(fmt_tokens(999_999), "999k");
        assert_eq!(fmt_tokens(1_250_000), "1.2M");
    }

    #[test]
    fn defs_fingerprint_tracks_the_offered_schema_set() {
        let def = |name: &str, description: &str| ToolDef {
            name: name.to_string(),
            description: description.to_string(),
            params: serde_json::json!({}),
        };
        let base = vec![def("read_file", "read"), def("shell", "run")];
        assert_eq!(defs_fingerprint(&base), defs_fingerprint(&base.clone()));
        let renamed = vec![def("read_file", "read"), def("shell_v2", "run")];
        assert_ne!(defs_fingerprint(&base), defs_fingerprint(&renamed));
        let grown = vec![
            def("read_file", "read"),
            def("shell", "run"),
            def("lsp", "query"),
        ];
        assert_ne!(defs_fingerprint(&base), defs_fingerprint(&grown));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_reports_label_and_bounded_progress() {
        let thinking = Thinking::pending_for_test("practice");
        let (progress, secs, label) = thinking.progress();
        assert_eq!(label, "practice");
        assert!(progress >= 0.0);
        assert!(progress < 1.0);
        assert!(secs >= 0.0);
    }

    #[test]
    fn spawn_acquires_foreground_lease_inside_the_worker() {
        // Pin: acquire_scoped must not run on the UI thread in Thinking::spawn.
        // A contended leases mutex would otherwise hitch Enter-after-echo.
        // include_str is the non-flaky substitute for "spawn does not block on a
        // held lease" (acquire is fail-fast, so a held lease cannot stall spawn).
        let src = include_str!("turn.rs");
        let start = src
            .find("pub(crate) fn spawn(")
            .expect("Thinking::spawn present");
        let spawn = &src[start..];
        let end = spawn
            .find("\n    pub(crate) fn begin_draining")
            .expect("begin_draining follows spawn");
        let spawn = &spawn[..end];
        let worker = spawn
            .find(".spawn(move ||")
            .expect("worker closure in Thinking::spawn");
        let ui = &spawn[..worker];
        let worker = &spawn[worker..];
        assert!(
            !ui.contains("acquire_scoped") && !ui.contains("resource_group_for_identity"),
            "UI-thread spawn must not wait on a foreground lease:\n{ui}"
        );
        assert!(
            worker.contains("acquire_scoped"),
            "foreground lease must be acquired inside the worker:\n{worker}"
        );
        assert!(
            worker.contains("foreground route resource conflict"),
            "lease errors must still surface as the turn result:\n{worker}"
        );
        assert!(
            worker.contains("fold_vision_sidecar_into_convo"),
            "vision sidecar rewrite must run inside the worker before hop 1:\n{worker}"
        );
        assert!(
            !ui.contains("fold_vision_sidecar_into_convo")
                && !ui.contains("apply_vision_sidecar")
                && !ui.contains("describe_media"),
            "UI-thread spawn must not run the vision sidecar:\n{ui}"
        );
        assert!(
            !ui.contains("club.token_usage") && !ui.contains("club.cache_usage"),
            "UI-thread spawn must not snapshot club usage mutexes:\n{ui}"
        );
        assert!(
            worker.contains("club.token_usage") && worker.contains("club.cache_usage"),
            "club usage snapshots must be taken inside the worker:\n{worker}"
        );
        assert!(
            !ui.contains("convo.to_vec()"),
            "UI-thread spawn must not materialize the history Vec:\n{ui}"
        );
        assert!(
            worker.contains("convo.to_vec()"),
            "shared history Arc must become a Vec inside the worker:\n{worker}"
        );
        assert!(
            !ui.contains("configured_max_hops"),
            "UI-thread spawn must not getenv ANGEL_MAX_HOPS:\n{ui}"
        );
        assert!(
            worker.contains("configured_max_hops"),
            "hop cap must be resolved inside the worker:\n{worker}"
        );
        assert!(
            !ui.contains("club.route_identity"),
            "UI-thread spawn must take the precomputed route identity:\n{ui}"
        );
    }

    #[test]
    fn spawn_reports_a_held_foreground_lease_as_the_turn_result() {
        let _env = crate::tests::env_lock();
        let _backplane = crate::tests::TestEnvGuard::unset("ANGEL_BACKPLANE");
        let bag = crate::club::Bag::for_render_test(&[("spark", &[("swarm", true)])]);
        let mut registry = crate::harness::ToolRegistry::new();
        registry.set_backplane(crate::backplane::BackplaneRegistry::from_bag(&bag));
        let tools = Arc::new(registry);
        let club = bag.in_hand();
        let identity = club.route_identity();
        let backplane = tools.backplane();
        let (route_id, group) = backplane
            .resource_group_for_identity(&identity)
            .expect("render-test bag publishes a unique foreground route");
        let _held = backplane
            .acquire_scoped(
                &group,
                crate::backplane::LeaseMode::Serve,
                crate::backplane::WorkloadRole::Foreground,
                Some(route_id),
                false,
            )
            .expect("hold the foreground slot");

        let thinking = Thinking::spawn(
            club.label().to_string(),
            club.clone(),
            tools,
            Arc::from([ChatMsg::user("hello")]),
            Arc::new(crate::steer::SteerQueue::default()),
            crate::session::Session::disabled(),
            club.route_identity(),
            None,
        );
        let result = thinking
            .rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("worker must report the lease conflict without a provider hop");
        let error = result.expect_err("held foreground lease is a turn error");
        assert!(
            error.contains("foreground route resource conflict"),
            "{error}"
        );
    }

    #[test]
    fn spawn_snapshots_club_usage_inside_the_worker() {
        let _env = crate::tests::env_lock();
        let workspace =
            crate::tests::TestGitWorkspace::new("spawn_snapshots_club_usage_inside_the_worker");
        let _backplane = crate::tests::TestEnvGuard::unset("ANGEL_BACKPLANE");
        let _hops = crate::tests::TestEnvGuard::unset("ANGEL_MAX_HOPS");
        let _traj = crate::tests::TestEnvGuard::unset("ANGEL_TRAJECTORY_LOG");

        struct ProbeClub {
            usage_threads: Mutex<Vec<std::thread::ThreadId>>,
            cache_threads: Mutex<Vec<std::thread::ThreadId>>,
        }
        impl Club for ProbeClub {
            fn respond(&self, prompt: &str) -> Result<String, String> {
                Ok(format!("probe:{prompt}"))
            }
            fn label(&self) -> &str {
                "probe-usage"
            }
            fn token_usage(&self) -> Option<TokenUsage> {
                self.usage_threads
                    .lock()
                    .expect("usage thread log")
                    .push(std::thread::current().id());
                Some(TokenUsage {
                    turns: 2,
                    last_input: 40,
                    last_output: 5,
                    last_reasoning: 0,
                    total_input: 111,
                    total_output: 9,
                    total_reasoning: 0,
                })
            }
            fn cache_usage(&self) -> CacheUsage {
                self.cache_threads
                    .lock()
                    .expect("cache thread log")
                    .push(std::thread::current().id());
                CacheUsage {
                    read_input_tokens: 22,
                    read_accounting_responses: 1,
                    ..CacheUsage::default()
                }
            }
        }

        let club = Arc::new(ProbeClub {
            usage_threads: Mutex::new(Vec::new()),
            cache_threads: Mutex::new(Vec::new()),
        });
        let ui_thread = std::thread::current().id();
        let thinking = Thinking::spawn(
            club.label().to_string(),
            club.clone(),
            Arc::new(workspace.registry()),
            Arc::from([ChatMsg::user("hello")]),
            Arc::new(crate::steer::SteerQueue::default()),
            crate::session::Session::disabled(),
            club.route_identity(),
            None,
        );
        let _ = thinking
            .rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("probe turn must settle");
        let before = thinking
            .spawn_usage
            .get()
            .copied()
            .expect("worker publishes spawn usage before hops");
        assert_eq!(
            before.usage_before.map(|usage| usage.total_input),
            Some(111)
        );
        assert_eq!(before.cache_before.read_input_tokens, 22);
        let usage_threads = club.usage_threads.lock().expect("usage thread log");
        let cache_threads = club.cache_threads.lock().expect("cache thread log");
        assert!(
            !usage_threads.is_empty() && usage_threads.iter().all(|id| *id != ui_thread),
            "token_usage must run off the UI thread: {usage_threads:?}"
        );
        assert!(
            !cache_threads.is_empty() && cache_threads.iter().all(|id| *id != ui_thread),
            "cache_usage must run off the UI thread: {cache_threads:?}"
        );
    }

    #[test]
    fn spawn_materializes_the_shared_history_snapshot() {
        let _env = crate::tests::env_lock();
        let workspace =
            crate::tests::TestGitWorkspace::new("spawn_materializes_the_shared_history_snapshot");
        let _backplane = crate::tests::TestEnvGuard::unset("ANGEL_BACKPLANE");
        let _hops = crate::tests::TestEnvGuard::unset("ANGEL_MAX_HOPS");
        let _traj = crate::tests::TestEnvGuard::unset("ANGEL_TRAJECTORY_LOG");

        struct ProbeClub {
            seen: Mutex<Vec<Vec<(crate::club::ChatRole, String)>>>,
        }
        impl Club for ProbeClub {
            fn respond(&self, prompt: &str) -> Result<String, String> {
                Ok(format!("probe:{prompt}"))
            }
            fn label(&self) -> &str {
                "probe-history"
            }
            fn chat(
                &self,
                messages: &[ChatMsg],
                _tools: &[ToolDef],
            ) -> Result<crate::club::ClubReply, String> {
                self.seen.lock().expect("seen hop log").push(
                    messages
                        .iter()
                        .map(|message| (message.role.clone(), message.content.to_string()))
                        .collect(),
                );
                assert_eq!(
                    crate::harness::run_identity::live_turn().as_deref(),
                    Some("snapshot-loop-owner")
                );
                Ok(crate::club::ClubReply::Text("probe:ok".to_string()))
            }
        }

        let club = Arc::new(ProbeClub {
            seen: Mutex::new(Vec::new()),
        });
        let history: Arc<[ChatMsg]> = Arc::from(vec![
            ChatMsg::system("orchestrator prompt"),
            ChatMsg::user("same snapshot as persist"),
        ]);
        let thinking = Thinking::spawn(
            club.label().to_string(),
            club.clone(),
            Arc::new(workspace.registry()),
            Arc::clone(&history),
            Arc::new(crate::steer::SteerQueue::default()),
            crate::session::Session::disabled(),
            club.route_identity(),
            Some("snapshot-loop-owner".into()),
        );
        let (convo, answer, _, _) = thinking
            .rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("probe turn must settle")
            .expect("probe turn succeeds");
        assert_eq!(answer, "probe:ok");
        assert_eq!(convo.len(), history.len() + 1);
        assert_eq!(convo[0].role, history[0].role);
        assert_eq!(&*convo[0].content, &*history[0].content);
        assert_eq!(convo[1].role, history[1].role);
        assert_eq!(&*convo[1].content, &*history[1].content);
        let seen = club.seen.lock().expect("seen hop log");
        assert_eq!(seen.len(), 1);
        assert_eq!(
            seen[0],
            vec![
                (
                    crate::club::ChatRole::System,
                    "orchestrator prompt".to_string()
                ),
                (
                    crate::club::ChatRole::User,
                    "same snapshot as persist".to_string()
                ),
            ]
        );
    }

    #[test]
    fn cache_hit_pct_handles_both_provider_accounting_styles() {
        // OpenAI-style: cached tokens are inside prompt_tokens.
        assert_eq!(cache_hit_pct(75, 100), Some(75));
        // Anthropic-style: cache reads are disjoint from input_tokens.
        assert_eq!(cache_hit_pct(9_000, 1_000), Some(90));
        assert_eq!(cache_hit_pct(0, 100), Some(0));
        // Nothing reported at all → no rate, not 0%.
        assert_eq!(cache_hit_pct(0, 0), None);
    }

    #[test]
    fn cache_meter_says_na_until_a_provider_reports_cache_fields() {
        let mut meter = CacheMeter::default();
        assert_eq!(meter.usage_line(), "hit n/a session · n/a last turn");
        // A local backend turn: tokens maybe, but no cache accounting.
        meter.fold_turn(0, 5_000, false);
        assert_eq!(meter.usage_line(), "hit n/a session · n/a last turn");
        assert_eq!(meter.session_input, 0, "unreported turns never dilute");
    }

    #[test]
    fn cache_meter_folds_session_aggregate_and_last_turn() {
        let mut meter = CacheMeter::default();
        meter.fold_turn(50, 100, true);
        assert_eq!(meter.usage_line(), "hit n/a session · n/a last turn");
        meter.fold_turn(250, 300, true);
        // Session: 300/400; last turn: 250/300.
        assert_eq!(meter.usage_line(), "hit n/a session · n/a last turn");
        // A later cache-blind turn keeps the session rate but reports honestly.
        meter.fold_turn(0, 900, false);
        assert_eq!(meter.usage_line(), "hit n/a session · n/a last turn");
    }
}
