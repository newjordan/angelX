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
#[path = "../../tests/cockpit/app/turn__idle_timeout_tests.rs"]
mod idle_timeout_tests;

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
#[path = "../../tests/cockpit/app/turn__cache_ledger_tests.rs"]
mod cache_ledger_tests;

#[cfg(test)]
#[path = "../../tests/cockpit/app/turn__tests.rs"]
mod tests;
