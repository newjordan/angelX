//! SwarmClub construction, role-call plumbing, and the Club impl.

use super::*;
use crate::club::{CacheUsage, EffortGateUsage, TruncationUsage};
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, atomic::AtomicUsize};

const SWARM_WORKER_INFLIGHT_LIMIT: usize = 64;
static SWARM_WORKERS_INFLIGHT: AtomicUsize = AtomicUsize::new(0);

struct SwarmWorkerPermit {
    counter: &'static AtomicUsize,
}

impl Drop for SwarmWorkerPermit {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::AcqRel);
    }
}

fn reserve_swarm_workers(
    counter: &'static AtomicUsize,
    workers: usize,
    limit: usize,
) -> Result<Vec<SwarmWorkerPermit>, String> {
    let limit = limit.max(1);
    let mut current = counter.load(Ordering::Acquire);
    loop {
        let Some(next) = current.checked_add(workers) else {
            return Err("swarm worker counter overflow".to_string());
        };
        if next > limit {
            return Err(format!(
                "swarm capacity exhausted: {current} worker(s) are still running, {workers} requested, process limit {limit}"
            ));
        }
        match counter.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => {
                return Ok((0..workers)
                    .map(|_| SwarmWorkerPermit { counter })
                    .collect());
            }
            Err(observed) => current = observed,
        }
    }
}

// ---------------------------------------------------------------------------
// SwarmClub
// ---------------------------------------------------------------------------

/// A model seat paired with an optional liveness bit from the fleet prober: when
/// the bit is present and reads false, the seat is skipped rather than dialed.
pub(crate) type SeatExtra = (Arc<dyn Club>, Option<Arc<AtomicBool>>);

/// The fleet club each swarm role runs on. By default every role points at the
/// same inner club — a homogeneous mixture-of-agents (today's behavior). Routing
/// a role to a *different* fleet model (via `ANGEL_SWARM_{PROPOSE,JUDGE,VERIFY,
/// AGG}_CLUB`) turns the swarm into a heterogeneous / symbiotic mixture: e.g.
/// gemma proposes, siq judges, qwen verifies. `aggregate` is also the club used
/// for the single-pass fallback and the final streamed synthesis.
#[derive(Clone)]
pub(crate) struct RoleClubs {
    pub(crate) propose: Arc<dyn Club>,
    /// Additional heterogeneous proposer seats. Each optional availability bit
    /// comes from the fleet prober, so a dead local breadth model is omitted
    /// from the wave instead of turning into a slow or metered fallback call.
    pub(crate) propose_extra: Vec<SeatExtra>,
    pub(crate) judge: Arc<dyn Club>,
    pub(crate) judge_extra: Vec<SeatExtra>,
    pub(crate) verify: Arc<dyn Club>,
    pub(crate) verify_extra: Vec<SeatExtra>,
    pub(crate) aggregate: Arc<dyn Club>,
    pub(crate) aggregate_extra: Vec<SeatExtra>,
    /// Optional live-research scout. This does not occupy or replace any MoA role
    /// seat; it only supplies fresh context for the proposers when requested.
    pub(crate) research: Option<Arc<dyn Club>>,
}

/// A [`Club`] that answers by fanning out a mixture-of-agents over its role clubs.
type ToolPreflightOutcome = Result<Arc<str>, Arc<str>>;

pub struct SwarmClub {
    pub(crate) name: String,
    /// The worker model(s) each role runs on (all gemma4 in the default homogeneous
    /// case; heterogeneous when role-club env vars are set).
    pub(crate) clubs: RoleClubs,
    pub(crate) k: Knobs,
    /// Quota failover bench: every configured SOTA link, in build order. When a
    /// role call fails with a quota-exhausted error (weekly/monthly plan cap —
    /// hours-to-days, not a transient 429), the call is retried on the next
    /// spendable link here instead of failing the turn. Empty for local swarms:
    /// a local fleet model has no metered quota, so there's nothing to route
    /// around and behavior is unchanged.
    pub(crate) fallbacks: Vec<Arc<dyn Club>>,
    /// One terminal formation outcome per active user turn. Successful advice
    /// is replayed into every coordinator hop so provider history stays
    /// prefix-stable. A failure is memoized too: retrying the same outer task
    /// turn must not buy the entire failed formation again.
    pub(crate) tool_preflight: Arc<Mutex<Option<(u64, ToolPreflightOutcome)>>>,
}

impl SwarmClub {
    fn aggregation_fanin_from_env() -> usize {
        match env_usize("ANGEL_MOA_AGG_FANIN", 0) {
            0 => usize::MAX,
            configured => configured.max(2),
        }
    }

    pub(crate) fn knobs_from_env() -> Knobs {
        let max = env_flag_or("ANGEL_SWARM_MAX", false);
        let width = env_usize("ANGEL_SWARM_WIDTH", 4).clamp(2, roster_len());
        let max_width = env_usize("ANGEL_SWARM_MAX_WIDTH", 32)
            .clamp(2, roster_len())
            .max(width);
        Knobs {
            width,
            max_width,
            max_waves: env_usize("ANGEL_SWARM_MAX_WAVES", 3).max(1),
            wave_growth: env_f64("ANGEL_SWARM_WAVE_GROWTH", 2.0).max(1.0),
            novelty_floor: env_f64("ANGEL_SWARM_NOVELTY_FLOOR", 0.35).clamp(0.0, 1.0),
            dissent_epsilon: env_f64("ANGEL_SWARM_DISSENT_EPSILON", 0.05).max(0.0),
            hard_factor: env_usize("ANGEL_SWARM_HARD_FACTOR", 2).max(1),
            refine_width: env_usize("ANGEL_SWARM_REFINE_WIDTH", 6).clamp(1, roster_len()),
            layers: env_usize("ANGEL_SWARM_LAYERS", if max { 3 } else { 2 }).max(1),
            always: env_flag_or("ANGEL_SWARM_ALWAYS", max),
            engaged: false,
            // Local fleet calls are free — the heuristic gate stays the default.
            mode: MoaMode::from_env_var("ANGEL_SWARM_MODE", MoaMode::Auto),
            reflect: env_flag_or("ANGEL_SWARM_REFLECT", max),
            judge: env_flag_or("ANGEL_SWARM_JUDGE", max),
            judge_panel: env_usize("ANGEL_SWARM_JUDGE_PANEL", if max { 3 } else { 1 }).max(1),
            dims: env_flag_or("ANGEL_SWARM_JUDGE_DIMS", max),
            keep: env_usize("ANGEL_SWARM_KEEP", 0),
            samples: env_usize("ANGEL_SWARM_SAMPLES", if max { 3 } else { 1 }).max(1),
            verify: env_usize("ANGEL_SWARM_VERIFY", if max { 2 } else { 0 }),
            verify_guard: env_flag_or("ANGEL_SWARM_VERIFY_GUARD", max),
            research: env_flag_or("ANGEL_SWARM_RESEARCH", max),
            cite: env_flag_or("ANGEL_SWARM_CITE", max),
            hedge: env_flag_or("ANGEL_SWARM_HEDGE", max),
            search_url: std::env::var("ANGEL_SWARM_SEARCH_URL")
                .unwrap_or_else(|_| default_search_url()),
            delegate: env_flag_or("ANGEL_SWARM_DELEGATE", max),
            max_tests: env_usize("ANGEL_SWARM_MAX_TESTS", 3),
            dissent_gate: env_flag_or("ANGEL_MOA_DISSENT_GATE", false),
            scale_threshold: env_usize("ANGEL_MOA_SCALE_THRESHOLD", 6).max(1),
            cluster: env_flag_or("ANGEL_MOA_CLUSTER", false),
            cluster_sim: env_f64("ANGEL_MOA_CLUSTER_SIM", 0.55).clamp(0.0, 1.0),
            rep_mode: std::env::var("ANGEL_MOA_REP_MODE").unwrap_or_else(|_| "detail".to_string()),
            pod_size: env_usize("ANGEL_MOA_POD_SIZE", 6).max(1),
            pod_keep: env_usize("ANGEL_MOA_POD_KEEP", 2).max(1),
            judge_fanout: env_usize("ANGEL_MOA_JUDGE_FANOUT", 18).max(1),
            judge_weights: env_flag_or("ANGEL_MOA_JUDGE_WEIGHTS", false),
            agg_fanin: Self::aggregation_fanin_from_env(),
            dominant_relax: env_f64("ANGEL_MOA_DOMINANT_RELAX", 0.67).clamp(0.0, 1.0),
            eclusters_hi: env_f64("ANGEL_MOA_ECLUSTERS_HI", 4.0).max(1.0),
            // Local fleet calls are free and their backends mostly ignore
            // effort; only the SOTA roster carries per-seat efforts.
            seat_efforts: SeatEfforts::default(),
        }
    }

    pub(crate) fn sota_knobs_from_env() -> Knobs {
        if env_flag_or("ANGEL_SOTA_MOA_USE_SWARM_KNOBS", false) {
            return Self::knobs_from_env();
        }
        let max = env_flag_or("ANGEL_SOTA_MOA_MAX", false);
        let width =
            env_usize("ANGEL_SOTA_MOA_WIDTH", if max { 3 } else { 2 }).clamp(2, roster_len());
        let max_width = env_usize("ANGEL_SOTA_MOA_MAX_WIDTH", width)
            .clamp(2, roster_len())
            .max(width);
        Knobs {
            // SOTA/provider MOA pays real network/API latency per wave. When a
            // prompt routes to MOA, keep the default to one proposer wave plus
            // one streamed synthesis wave.
            width,
            max_width,
            max_waves: env_usize("ANGEL_SOTA_MOA_MAX_WAVES", 1).max(1),
            wave_growth: env_f64("ANGEL_SWARM_WAVE_GROWTH", 2.0).max(1.0),
            novelty_floor: env_f64("ANGEL_SWARM_NOVELTY_FLOOR", 0.35).clamp(0.0, 1.0),
            dissent_epsilon: env_f64("ANGEL_SWARM_DISSENT_EPSILON", 0.05).max(0.0),
            hard_factor: env_usize("ANGEL_SWARM_HARD_FACTOR", 2).max(1),
            refine_width: env_usize("ANGEL_SWARM_REFINE_WIDTH", 6).clamp(1, roster_len()),
            layers: env_usize("ANGEL_SOTA_MOA_LAYERS", if max { 2 } else { 1 }).max(1),
            always: env_flag_or("ANGEL_SOTA_MOA_ALWAYS", false),
            engaged: false,
            // SOTA links are metered: default MANUAL — the coordinator answers
            // solo at standard token spend until a turn opens with `moa:`.
            mode: MoaMode::from_env_var("ANGEL_SOTA_MOA_MODE", MoaMode::Manual),
            reflect: env_flag_or("ANGEL_SOTA_MOA_REFLECT", max),
            judge: env_flag_or("ANGEL_SOTA_MOA_JUDGE", false),
            judge_panel: env_usize("ANGEL_SOTA_MOA_JUDGE_PANEL", if max { 2 } else { 1 }).max(1),
            dims: env_flag_or("ANGEL_SOTA_MOA_JUDGE_DIMS", false),
            keep: env_usize("ANGEL_SOTA_MOA_KEEP", 0),
            samples: env_usize("ANGEL_SOTA_MOA_SAMPLES", if max { 2 } else { 1 }).max(1),
            verify: env_usize("ANGEL_SOTA_MOA_VERIFY", if max { 1 } else { 0 }),
            verify_guard: env_flag_or("ANGEL_SOTA_MOA_VERIFY_GUARD", max),
            research: env_flag_or("ANGEL_SOTA_MOA_RESEARCH", false),
            cite: env_flag_or("ANGEL_SOTA_MOA_CITE", false),
            hedge: env_flag_or("ANGEL_SOTA_MOA_HEDGE", max),
            search_url: std::env::var("ANGEL_SOTA_MOA_SEARCH_URL")
                .or_else(|_| std::env::var("ANGEL_SWARM_SEARCH_URL"))
                .unwrap_or_else(|_| default_search_url()),
            delegate: env_flag_or("ANGEL_SOTA_MOA_DELEGATE", false),
            max_tests: env_usize("ANGEL_SOTA_MOA_MAX_TESTS", 2),
            dissent_gate: env_flag_or(
                "ANGEL_SOTA_MOA_DISSENT_GATE",
                env_flag_or("ANGEL_MOA_DISSENT_GATE", false),
            ),
            scale_threshold: env_usize("ANGEL_MOA_SCALE_THRESHOLD", 6).max(1),
            cluster: env_flag_or("ANGEL_MOA_CLUSTER", false),
            cluster_sim: env_f64("ANGEL_MOA_CLUSTER_SIM", 0.55).clamp(0.0, 1.0),
            rep_mode: std::env::var("ANGEL_MOA_REP_MODE").unwrap_or_else(|_| "detail".to_string()),
            pod_size: env_usize("ANGEL_MOA_POD_SIZE", 6).max(1),
            pod_keep: env_usize("ANGEL_MOA_POD_KEEP", 2).max(1),
            judge_fanout: env_usize("ANGEL_MOA_JUDGE_FANOUT", 18).max(1),
            judge_weights: env_flag_or("ANGEL_MOA_JUDGE_WEIGHTS", false),
            agg_fanin: Self::aggregation_fanin_from_env(),
            dominant_relax: env_f64("ANGEL_MOA_DOMINANT_RELAX", 0.67).clamp(0.0, 1.0),
            eclusters_hi: env_f64("ANGEL_MOA_ECLUSTERS_HI", 4.0).max(1.0),
            seat_efforts: SeatEfforts {
                propose: env_effort("ANGEL_SOTA_MOA_PROPOSE_EFFORT"),
                judge: env_effort("ANGEL_SOTA_MOA_JUDGE_EFFORT"),
                verify: env_effort("ANGEL_SOTA_MOA_VERIFY_EFFORT"),
                aggregate: env_effort("ANGEL_SOTA_MOA_AGG_EFFORT"),
            },
        }
    }

    /// Build from env. Base: `ANGEL_SWARM_WIDTH` (wave-0 size, clamped 2..=36),
    /// `ANGEL_SWARM_MAX_WIDTH` (32), `ANGEL_SWARM_MAX_WAVES` (3),
    /// `ANGEL_SWARM_LAYERS` (2), `ANGEL_SWARM_ALWAYS`. Autoresearch:
    /// `ANGEL_SWARM_REFLECT`, `ANGEL_SWARM_JUDGE` (+ `ANGEL_SWARM_JUDGE_PANEL`,
    /// `ANGEL_SWARM_JUDGE_DIMS`), `ANGEL_SWARM_KEEP`, `ANGEL_SWARM_SAMPLES`,
    /// `ANGEL_SWARM_VERIFY` (+ `ANGEL_SWARM_VERIFY_GUARD`), `ANGEL_SWARM_RESEARCH`
    /// (+ `ANGEL_SWARM_SEARCH_URL`, `ANGEL_SWARM_CITE`), `ANGEL_SWARM_HEDGE`.
    /// `ANGEL_SWARM_MAX` turns the whole stack on;
    /// individual vars still override it (e.g. `ANGEL_SWARM_VERIFY=0` under MAX).
    ///
    /// `resolve` maps a fleet label (e.g. "gemma","turbo","atlas") to a club so the
    /// `ANGEL_SWARM_{PROPOSE,JUDGE,VERIFY,AGG}_CLUB` env vars can route each role to
    /// a different model — a cross-model (symbiotic) mixture. Any role whose env var
    /// is unset or unresolvable falls back to `inner`, so the default is the old
    /// homogeneous single-inner swarm. Pass `|_| None` for an inner-only swarm.
    pub fn from_env(name: impl Into<String>, inner: Arc<dyn Club>) -> Self {
        // No resolver -> role-club env vars never resolve, so every role stays on
        // `inner` (the homogeneous default).
        Self::from_env_with(name, inner, |_| None)
    }

    pub fn from_env_with(
        name: impl Into<String>,
        inner: Arc<dyn Club>,
        resolve: impl Fn(&str) -> Option<Arc<dyn Club>>,
    ) -> Self {
        let name = name.into();
        let k = Self::knobs_from_env();
        // Resolve each role to a fleet club, defaulting to `inner`. An env var that
        // names a label we can't resolve is a likely typo — warn, then fall back.
        let role = |var: &str| -> Arc<dyn Club> {
            match std::env::var(var) {
                Ok(lbl) if !lbl.is_empty() => match resolve(&lbl) {
                    Some(c) => c,
                    None => {
                        eprintln!(
                            "[swarm:{name}] {var}={lbl:?} did not resolve to a fleet club; using {}",
                            inner.label()
                        );
                        Arc::clone(&inner)
                    }
                },
                _ => Arc::clone(&inner),
            }
        };
        let clubs = RoleClubs {
            propose: role("ANGEL_SWARM_PROPOSE_CLUB"),
            propose_extra: Vec::new(),
            judge: role("ANGEL_SWARM_JUDGE_CLUB"),
            judge_extra: Vec::new(),
            verify: role("ANGEL_SWARM_VERIFY_CLUB"),
            verify_extra: Vec::new(),
            aggregate: role("ANGEL_SWARM_AGG_CLUB"),
            aggregate_extra: Vec::new(),
            research: grok_research_role(),
        };
        if let Some(research) = &clubs.research {
            static RESEARCH_NOTICE: std::sync::Once = std::sync::Once::new();
            RESEARCH_NOTICE.call_once(|| {
                eprintln!("[swarm:{name}] research scout: {}", research.label());
            });
        }
        Self {
            name,
            clubs,
            k,
            fallbacks: Vec::new(),
            tool_preflight: Default::default(),
        }
    }

    pub fn from_env_roles(
        name: impl Into<String>,
        propose: Arc<dyn Club>,
        judge: Arc<dyn Club>,
        verify: Arc<dyn Club>,
        aggregate: Arc<dyn Club>,
    ) -> Self {
        let name = name.into();
        let clubs = RoleClubs {
            propose,
            propose_extra: Vec::new(),
            judge,
            judge_extra: Vec::new(),
            verify,
            verify_extra: Vec::new(),
            aggregate,
            aggregate_extra: Vec::new(),
            research: grok_research_role(),
        };
        if let Some(research) = &clubs.research {
            static RESEARCH_NOTICE: std::sync::Once = std::sync::Once::new();
            RESEARCH_NOTICE.call_once(|| {
                eprintln!("[swarm:{name}] research scout: {}", research.label());
            });
        }
        Self {
            name,
            clubs,
            k: Self::knobs_from_env(),
            fallbacks: Vec::new(),
            tool_preflight: Default::default(),
        }
    }

    /// Announce a heterogeneous role map only when this exact swarm is actually
    /// used. The Bag eagerly constructs every named formation at startup; doing
    /// this in the constructors falsely advertised inactive OpenAI/Grok profiles
    /// during a strictly GLM-routed task.
    pub(super) fn announce_active_mixture(&self) {
        let labels = [
            self.clubs.propose.label(),
            self.clubs.judge.label(),
            self.clubs.verify.label(),
            self.clubs.aggregate.label(),
        ];
        if labels.windows(2).all(|pair| pair[0] == pair[1]) {
            return;
        }
        let signature = format!("{}\0{}", self.name, labels.join("\0"));
        static ANNOUNCED: std::sync::OnceLock<Mutex<std::collections::HashSet<String>>> =
            std::sync::OnceLock::new();
        let announced = ANNOUNCED.get_or_init(|| Mutex::new(std::collections::HashSet::new()));
        if announced
            .lock()
            .is_ok_and(|mut seen| seen.insert(signature))
        {
            eprintln!(
                "[swarm:{}] active mixture: propose={} judge={} verify={} aggregate={}",
                self.name, labels[0], labels[1], labels[2], labels[3]
            );
        }
    }

    /// Dedicated Math God club: one Sol@ultra head proposes and synthesizes,
    /// GLM-5.3 and DeepSeek v4 Pro are extra proposers, Grok weighs in at xhigh.
    /// Leanstral is a tool, not a seat. The bag copy is **manual** so Tab/loop
    /// cannot silently fan it out; `/moa math` engagement still amplifies
    /// deliberate answers via [`Self::with_engaged_formation`] (short answers
    /// stay coordinator-only). An explicit `ANGEL_DRIVER=mathgod` pin
    /// is the other Always path.
    pub fn mathgod(
        head: Arc<dyn Club>,
        judge: Arc<dyn Club>,
        extra_proposers: Vec<SeatExtra>,
    ) -> Self {
        let aggregate = Arc::clone(&head);
        let verify = Arc::clone(&head);
        let mut club = Self::from_env_roles("mathgod", head, judge, verify, aggregate);
        club.k = Self::mathgod_knobs();
        club.clubs.propose_extra = extra_proposers;
        club
    }

    fn mathgod_driver_pinned() -> bool {
        std::env::var("ANGEL_DRIVER")
            .ok()
            .map(|s| s.trim().to_ascii_lowercase().replace(['_', ' '], "-"))
            .is_some_and(|s| matches!(s.as_str(), "math" | "math-god" | "mathgod"))
    }

    fn mathgod_knobs() -> Knobs {
        let mut k = Self::sota_knobs_from_env();
        k.width = 3;
        k.max_width = 3;
        k.layers = 1;
        k.samples = 1;
        k.max_waves = 1;
        let pinned = Self::mathgod_driver_pinned();
        k.always = pinned;
        k.mode = if pinned {
            MoaMode::Always
        } else {
            MoaMode::Manual
        };
        k.judge = true;
        k.judge_panel = 1;
        k.verify = 0;
        k.verify_guard = false;
        k.reflect = false;
        k.research = pinned;
        k.seat_efforts = SeatEfforts {
            propose: Some("ultra".into()),
            judge: Some("xhigh".into()),
            verify: None,
            aggregate: Some("ultra".into()),
        };
        k
    }

    pub fn from_sota_env_roles(
        name: impl Into<String>,
        propose: Arc<dyn Club>,
        judge: Arc<dyn Club>,
        verify: Arc<dyn Club>,
        aggregate: Arc<dyn Club>,
    ) -> Self {
        let mut club = Self::from_env_roles(name, propose, judge, verify, aggregate);
        club.k = Self::sota_knobs_from_env();
        if club.k.mode == MoaMode::Manual {
            static MANUAL_NOTICE: std::sync::Once = std::sync::Once::new();
            MANUAL_NOTICE.call_once(|| {
                eprintln!(
                    "[swarm] sota-moa amplify: manual — coordinator-only; start a turn with \"moa:\" to fan out"
                );
            });
        }
        club
    }

    /// Merge per-role effort overrides (a staged formation roster) over the
    /// env-derived seat policy: a `Some` wins its seat, `None` keeps env.
    pub(crate) fn with_role_effort_overrides(mut self, overrides: SeatEfforts) -> Self {
        let seats = &mut self.k.seat_efforts;
        if overrides.propose.is_some() {
            seats.propose = overrides.propose;
        }
        if overrides.judge.is_some() {
            seats.judge = overrides.judge;
        }
        if overrides.verify.is_some() {
            seats.verify = overrides.verify;
        }
        if overrides.aggregate.is_some() {
            seats.aggregate = overrides.aggregate;
        }
        self
    }

    /// A formation assembled explicitly by the cockpit roster board is already
    /// operator-authorized for every call made through that temporary wrapper.
    /// Carry that fact in route state so tools, steers, and compaction cannot
    /// erase it, and no synthetic `moa:` text has to impersonate User input.
    /// Engagement authorizes the mixture, it is not `always`: the router and
    /// the short-answer skip still apply, so the formation amplifies
    /// deliberate/synthesis answers instead of every final reply.
    pub(crate) fn with_engaged_formation(mut self) -> Self {
        self.k.engaged = true;
        self
    }

    /// Add heterogeneous proposer-only breadth. These seats never judge,
    /// verify, aggregate, drive tools, or enter the quota-failover bench.
    pub(crate) fn with_extra_proposers(mut self, extras: Vec<SeatExtra>) -> Self {
        self.clubs.propose_extra = extras;
        self
    }

    pub(crate) fn with_extra_judges(mut self, extras: Vec<SeatExtra>) -> Self {
        self.clubs.judge_extra = extras;
        self
    }

    pub(crate) fn with_extra_verifiers(mut self, extras: Vec<SeatExtra>) -> Self {
        self.clubs.verify_extra = extras;
        self
    }

    pub(crate) fn with_extra_aggregators(mut self, extras: Vec<SeatExtra>) -> Self {
        self.clubs.aggregate_extra = extras;
        self
    }

    pub(crate) fn with_research_role(mut self, research: Option<Arc<dyn Club>>) -> Self {
        self.clubs.research = research;
        self
    }

    /// Primary proposer followed by currently-live roster seats. Duplicates are
    /// intentional: choosing A/A/B for P1/P2/P3 must not silently become A/B/A.
    /// The primary remains usable even when every fleet availability bit is
    /// cold; optional seats retain their live availability gates.
    pub(crate) fn available_proposers(&self) -> Vec<Arc<dyn Club>> {
        available_role_roster(&self.clubs.propose, &self.clubs.propose_extra)
    }

    pub(crate) fn available_judges(&self) -> Vec<Arc<dyn Club>> {
        available_role_roster(&self.clubs.judge, &self.clubs.judge_extra)
    }

    pub(crate) fn available_verifiers(&self) -> Vec<Arc<dyn Club>> {
        available_role_roster(&self.clubs.verify, &self.clubs.verify_extra)
    }

    pub(crate) fn available_aggregators(&self) -> Vec<Arc<dyn Club>> {
        available_role_roster(&self.clubs.aggregate, &self.clubs.aggregate_extra)
    }

    /// Attach the quota failover bench (see the `fallbacks` field). Builder-style
    /// so the SOTA wiring in `club.rs` stays a single expression.
    pub fn with_quota_fallbacks(mut self, fallbacks: Vec<Arc<dyn Club>>) -> Self {
        self.fallbacks = fallbacks;
        self
    }

    /// A per-turn view of this swarm under different knobs: same role clubs
    /// (Arc clones) and failover bench, gated `k`. This is how the dissent gate
    /// overrides stage knobs for one turn — the stage methods keep reading
    /// `self.k`, no parameter threading.
    pub(crate) fn gated(&self, k: Knobs) -> SwarmClub {
        SwarmClub {
            name: self.name.clone(),
            clubs: self.clubs.clone(),
            k,
            fallbacks: self.fallbacks.clone(),
            tool_preflight: Arc::clone(&self.tool_preflight),
        }
    }

    /// One blocking call to a specific role `club` with `system` in front of
    /// `msgs` (an empty system — the cache-aligned score-stage shape — sends the
    /// messages bare). This is a MoA *stage* call: tools are never offered, since
    /// the stages reason over evidence the tool-capable driver hop
    /// ([`Self::drive_with_tools`]) already surfaced. If the club's provider quota
    /// is exhausted, the call fails over across the bench (see
    /// [`Self::quota_failover_order`]).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn call_on(
        &self,
        club: &dyn Club,
        system: &str,
        msgs: &[ChatMsg],
    ) -> Result<String, String> {
        self.call_on_with_effort(club, system, msgs, None)
    }

    /// [`Self::call_on`] carrying the seat's per-call reasoning-effort request
    /// (see [`SeatEfforts`]). The effort rides the call — and its failover —
    /// so a club shared across seats never has roster state written into it.
    pub(crate) fn call_on_with_effort(
        &self,
        club: &dyn Club,
        system: &str,
        msgs: &[ChatMsg],
        effort: Option<&str>,
    ) -> Result<String, String> {
        stage_call_on(&self.name, &self.fallbacks, club, system, msgs, effort)
    }

    pub(crate) fn text_reply(
        reply: Result<ClubReply, String>,
        label: &str,
    ) -> Result<String, String> {
        match reply? {
            ClubReply::Text(t) if crate::club::contains_raw_tool_markup(&t) => {
                Err(raw_tool_markup_text_error(label))
            }
            ClubReply::Text(t) => Ok(t),
            ClubReply::Calls(_) => Err(tool_call_text_stage_error(label)),
        }
    }

    pub(crate) fn checked_stream_text_reply(
        &self,
        club: &dyn Club,
        full: &[ChatMsg],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<String, String> {
        let reply = self.stream_with_failover(
            club,
            full,
            cancel,
            &mut |d| match d {
                StreamDelta::Content(_) => {}
                StreamDelta::Reasoning(r) => on_delta(StreamDelta::Reasoning(r)),
                StreamDelta::Heartbeat => on_delta(StreamDelta::Heartbeat),
            },
            true,
        );
        let text = Self::text_reply(reply, club.label())?;
        if !text.is_empty() {
            on_delta(StreamDelta::Content(&text));
        }
        Ok(text)
    }

    /// Blocking chat that reroutes a quota-exhausted call across the bench: try
    /// the assigned club, and on a plan-cap error (never a transient one) walk
    /// the remaining spendable links until one answers. The last error stands
    /// when the whole bench is exhausted.
    pub(crate) fn chat_with_failover(
        &self,
        club: &dyn Club,
        full: &[ChatMsg],
    ) -> Result<ClubReply, String> {
        // The tool-capable driver hop is the operator's interactive club, not a
        // roster seat: it keeps the club's own THINK-deck/env effort state.
        stage_chat_with_failover(&self.name, &self.fallbacks, club, full, None)
    }

    /// Streaming twin of [`Self::chat_with_failover`]. Failover is only safe
    /// while nothing has streamed to the caller — a quota gate trips before the
    /// first byte, so that's the normal case; once content has flowed, a retry
    /// would duplicate text in the transcript, so the error surfaces instead.
    pub(crate) fn stream_with_failover(
        &self,
        club: &dyn Club,
        full: &[ChatMsg],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
        allow_buffered_raw_markup_retry: bool,
    ) -> Result<ClubReply, String> {
        let full = text_only_history(full);
        let emitted = std::cell::Cell::new(false);
        let mut attempt = |c: &dyn Club| {
            let mut tap = |d: StreamDelta<'_>| {
                if matches!(d, StreamDelta::Content(_)) {
                    emitted.set(true);
                }
                on_delta(d);
            };
            guard_text_stage_reply(c.chat_streaming(&full, &[], cancel, &mut tap), c.label())
        };
        let mut result = attempt(club);
        let mut tried = vec![club.label().to_string()];
        loop {
            let Err(err) = &result else { return result };
            let raw_markup_retry =
                allow_buffered_raw_markup_retry && error_indicates_raw_tool_markup_text(err);
            if (emitted.get() && !raw_markup_retry) || !error_can_failover_text_stage(err) {
                return result;
            }
            let Some(next) = self.quota_failover_order(&tried).next().cloned() else {
                return result;
            };
            self.log_text_stage_failover(
                tried.last().map(String::as_str).unwrap_or("?"),
                &*next,
                err,
            );
            tried.push(next.label().to_string());
            result = attempt(&*next);
        }
    }

    fn log_text_stage_failover(&self, from: &str, next: &dyn Club, err: &str) {
        stage_log_failover(&self.name, from, next, err);
    }

    /// The bench, filtered for one failing call: skip every club already tried
    /// this call (each attempt gets exactly one shot, so two gate-less links can
    /// never ping-pong) and any link known to be cooling down (its gate would
    /// fail the call without network anyway — no point spending a slot on it).
    fn quota_failover_order<'a>(
        &'a self,
        tried: &'a [String],
    ) -> impl Iterator<Item = &'a Arc<dyn Club>> {
        stage_quota_failover_order(&self.fallbacks, tried)
    }

    /// Default-role call: the aggregate club (classify/critique/synthesize/revise).
    pub(crate) fn call(&self, system: &str, msgs: &[ChatMsg]) -> Result<String, String> {
        self.call_on_with_effort(
            &*self.clubs.aggregate,
            system,
            msgs,
            self.k.seat_efforts.aggregate.as_deref(),
        )
    }

    /// The bounded fan-out under every stage barrier. `clubs[i]` owns seat `i`;
    /// `make(i)` builds the `(system, messages)` for worker `i` on this thread,
    /// then each worker blocks in its own thread so the backends see the wave's
    /// simultaneous requests. Workers run *detached* (owned payloads, results
    /// over a channel) instead of scope-joined, so the barrier stops waiting at
    /// the first of:
    ///   - all `n` workers returned (the common case — nothing changes),
    ///   - the wave deadline (`ANGEL_SWARM_WAVE_DEADLINE`, default 240s — one
    ///     wedged endpoint used to hold a wave through its full 3×180s retry
    ///     budget while N−1 finished results sat waiting),
    ///   - quorum + grace: once `ANGEL_SWARM_QUORUM` (default 0.75) of the wave
    ///     has landed, stragglers get `ANGEL_SWARM_GRACE_SECS` (default 20s)
    ///     and are then cut — the mixture already provides redundancy. A
    ///     strict-width SOTA-MoA run disables this early cut and waits for all
    ///     requested seats, still bounded by the wave deadline,
    ///   - `cancel` (user interrupt) — the wave stops consuming immediately.
    ///
    /// SOTA-MoA additionally admits at most `ANGEL_SOTA_MOA_MAX_PARALLEL`
    /// requests at once (default 8). The remaining requested seats queue inside
    /// this same bounded wave and still count toward its exact width. This keeps
    /// 12-seat cloud formations from bursting through a provider's concurrency
    /// window without silently shrinking the formation.
    ///
    /// A cut worker's HTTP call winds down in the background; its send lands in
    /// a closed channel and vanishes. A queued worker is never launched after
    /// the barrier closes. Cut slots surface as `Err`, which every stage already
    /// tolerates as a partial wave.
    pub(crate) fn fan_out_across_on_cancel<F>(
        &self,
        clubs: &[Arc<dyn Club>],
        cancel: Option<&AtomicBool>,
        effort: Option<&str>,
        make: F,
    ) -> Vec<Result<String, String>>
    where
        F: Fn(usize) -> (String, Vec<ChatMsg>),
    {
        if let Some(budget) = crate::harness::formation_budget::current() {
            budget.set_unstarted(clubs.len().max(1) as u64);
        }
        self.fan_out_across_on_cancel_with_admission(
            clubs,
            cancel,
            effort,
            &SWARM_WORKERS_INFLIGHT,
            SWARM_WORKER_INFLIGHT_LIMIT,
            make,
        )
    }

    fn fan_out_across_on_cancel_with_admission<F>(
        &self,
        clubs: &[Arc<dyn Club>],
        cancel: Option<&AtomicBool>,
        effort: Option<&str>,
        inflight: &'static AtomicUsize,
        inflight_limit: usize,
        make: F,
    ) -> Vec<Result<String, String>>
    where
        F: Fn(usize) -> (String, Vec<ChatMsg>),
    {
        use std::time::{Duration, Instant};
        let n = clubs.len();
        if n == 0 {
            return Vec::new();
        }
        let deadline = Duration::from_secs(env_usize("ANGEL_SWARM_WAVE_DEADLINE", 0) as u64);
        let quorum_frac = env_f64("ANGEL_SWARM_QUORUM", 0.75).clamp(0.0, 1.0);
        let require_full_width = self.name.eq_ignore_ascii_case("sota-moa")
            && env_flag_or("ANGEL_SOTA_MOA_REQUIRE_FULL_WIDTH", false);
        let quorum = if require_full_width {
            n
        } else {
            (((n as f64) * quorum_frac).ceil() as usize).clamp(1, n)
        };
        let grace = Duration::from_secs(env_usize("ANGEL_SWARM_GRACE_SECS", 20) as u64);
        // Cut provider calls can ignore the wave deadline and continue in a
        // detached thread. Admit one complete wave atomically, then retain its
        // permits until the actual calls exit so repeated cut waves cannot grow
        // the process without bound.
        let permits = match reserve_swarm_workers(inflight, n, inflight_limit.max(n)) {
            Ok(permits) => permits,
            Err(error) => return (0..n).map(|_| Err(error.clone())).collect(),
        };
        let (tx, rx) = std::sync::mpsc::channel::<(usize, Result<String, String>)>();
        let max_parallel = if self.name.eq_ignore_ascii_case("sota-moa") {
            env_usize("ANGEL_SOTA_MOA_MAX_PARALLEL", 8).clamp(1, n)
        } else {
            n
        };
        let mut permits = permits.into_iter().map(Some).collect::<Vec<_>>();
        let launch =
            |i: usize,
             permit: SwarmWorkerPermit,
             tx: &std::sync::mpsc::Sender<(usize, Result<String, String>)>| {
                let (system, msgs) = make(i);
                let club = Arc::clone(&clubs[i]);
                let name = self.name.clone();
                let fallbacks = self.fallbacks.clone();
                // Owned copy: the seat's effort request crosses the 'static
                // worker boundary with the rest of the stage context.
                let effort = effort.map(str::to_string);
                let tx = tx.clone();
                let budget = crate::harness::formation_budget::current();
                let role = crate::harness::formation_budget::role();
                std::thread::spawn(move || {
                    let _budget_scope = crate::harness::formation_budget::enter(budget);
                    let _seat_role = crate::harness::formation_budget::enter_role(&role);
                    let out =
                        stage_call_on(&name, &fallbacks, &*club, &system, &msgs, effort.as_deref());
                    // A landed result must imply capacity is already available to
                    // the next wave; timed-out calls retain this permit because
                    // they have not reached this boundary yet.
                    drop(permit);
                    let _ = tx.send((i, out));
                });
            };
        let mut tx = Some(tx);
        let mut launched = 0usize;
        while launched < max_parallel {
            launch(
                launched,
                permits[launched].take().expect("one permit per swarm seat"),
                tx.as_ref().expect("launcher channel remains open"),
            );
            launched += 1;
        }
        if launched == n {
            drop(tx.take());
        }
        let started = Instant::now();
        let mut slots: Vec<Option<Result<String, String>>> = (0..n).map(|_| None).collect();
        let mut returned = 0usize;
        let mut quorum_at: Option<Instant> = None;
        while returned < n {
            if (!deadline.is_zero() && started.elapsed() >= deadline)
                || quorum_at.is_some_and(|q| q.elapsed() >= grace)
                || cancel.is_some_and(|c| c.load(Ordering::Relaxed))
            {
                break;
            }
            let wait = Duration::from_millis(250);
            match rx.recv_timeout(wait) {
                Ok((i, r)) => {
                    if slots[i].is_none() {
                        returned += 1;
                        // Seat telemetry for the miniworld muster/portal pips:
                        // one lock-and-set on the barrier thread per landing,
                        // nothing on the workers' streaming path.
                        crate::agentviz::stage_seat_update(
                            i,
                            if r.is_ok() {
                                crate::agentviz::SeatState::Returned
                            } else {
                                crate::agentviz::SeatState::Failed
                            },
                        );
                    }
                    slots[i] = Some(r);
                    if returned >= quorum && quorum < n && quorum_at.is_none() {
                        quorum_at = Some(Instant::now());
                    }
                    // A landed worker released its global permit before sending
                    // this result. Fill that request slot with the next queued
                    // seat while the wave is still live.
                    let barrier_open = (deadline.is_zero() || started.elapsed() < deadline)
                        && quorum_at.is_none_or(|q| q.elapsed() < grace)
                        && !cancel.is_some_and(|c| c.load(Ordering::Relaxed));
                    if launched < n && barrier_open {
                        launch(
                            launched,
                            permits[launched]
                                .take()
                                .expect("one permit per queued swarm seat"),
                            tx.as_ref().expect("queued seats retain launcher channel"),
                        );
                        launched += 1;
                        if launched == n {
                            drop(tx.take());
                        }
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        // Closing the root sender ensures queued seats can never be launched by
        // later cleanup after the deadline/cancel barrier has returned.
        drop(tx.take());
        // Anything still pending was cut at the deadline/quorum/cancel
        // barrier — say so on the seat telemetry too, not just in the Err.
        for (i, slot) in slots.iter().enumerate() {
            if slot.is_none() {
                crate::agentviz::stage_seat_update(i, crate::agentviz::SeatState::Cut);
            }
        }
        slots
            .into_iter()
            .map(|r| {
                r.unwrap_or_else(|| {
                    Err(
                        "cut at wave deadline/quorum (straggler abandoned to background)"
                            .to_string(),
                    )
                })
            })
            .collect()
    }

    /// Default-role fan-out: the aggregate club (layer re-aggregation, synthesis).
    pub(crate) fn fan_out<F>(&self, n: usize, make: F) -> Vec<Result<String, String>>
    where
        F: Fn(usize) -> (String, Vec<ChatMsg>),
    {
        let pool = self.available_aggregators();
        let assigned = (0..n)
            .map(|index| Arc::clone(&pool[index % pool.len()]))
            .collect::<Vec<_>>();
        self.fan_out_across_on_cancel(
            &assigned,
            None,
            self.k.seat_efforts.aggregate.as_deref(),
            make,
        )
    }

    /// Tool-capable driver hop. This is what makes the MoA *usable* as the
    /// top-level brain instead of a text-only oracle that strips the agent of
    /// every tool.
    ///
    /// The move: run one real, tool-offered pass on the driver (`aggregate`)
    /// club against the true conversation (tool protocol intact). If it wants a
    /// tool, hand the calls straight back to the harness — the agent works,
    /// gathers evidence, loops. Only when the driver produces a *final text
    /// answer* do we optionally amplify it with the full MoA fan-out — and only
    /// for reasoning-dominant/research questions ([`wants_synthesis`]), because a
    /// tool-grounded action answer is already backed by real output and a
    /// text-only re-synthesis can only regress it. `ANGEL_SOTA_MOA_ALWAYS`
    /// amplifies every turn; a short answer (a clarifying question, an ack) is
    /// never amplified.
    pub(crate) fn drive_with_tools(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
        stream: bool,
    ) -> Result<ClubReply, String> {
        let problem = messages
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_default();
        // Whether the *final answer* earns MoA amplification. Tool passthrough is
        // unconditional; this only decides whether to re-derive the answer.
        // Manual (the sota-moa default): coordinator-only at standard spend
        // until the user summons the mixture with a leading `moa:` — or an
        // armed formation (`engaged`) authorizes the same deliberate answers
        // without making every short reply a fan-out.
        let summoned = summons_moa(&problem);
        let explicitly_requested = self.k.always || self.k.mode == MoaMode::Always || summoned;
        let mut amplify = match self.k.mode {
            MoaMode::Always => true,
            MoaMode::Manual => self.k.always || self.k.engaged || summoned,
            MoaMode::Auto => {
                self.k.always
                    || summoned
                    || self.should_research_route(&problem)
                    || wants_synthesis(&problem)
            }
        };
        let preflight_requested = explicitly_requested || self.k.engaged;
        let preflight_enabled = tool_preflight_enabled(
            preflight_requested && self.name.eq_ignore_ascii_case("sota-moa"),
        );
        // A tool-capable MoA historically did all real work through the single
        // aggregate seat and waited until the coordinator's final prose before
        // fanning out. Long coding tasks that ended at max_hops therefore ran
        // zero formation calls while still presenting themselves as Fanout-8,
        // Council, etc. An armed or explicitly summoned formation now runs one
        // bounded planning wave before the first tool hop by default; operators
        // can explicitly disable it. Its output is untrusted advice, folded back
        // into the coordinator history; later tool hops see the existing tool
        // transcript and never pay for the preflight twice.
        let preflight_key = tool_preflight_key(messages);
        let preflight_memo = if preflight_enabled {
            preflight_key.and_then(|key| {
                self.tool_preflight
                    .lock()
                    .ok()
                    .and_then(|memo| memo.as_ref().filter(|(saved, _)| *saved == key).cloned())
                    .map(|(_, outcome)| outcome)
            })
        } else {
            None
        };
        let mut preflight_advice = preflight_memo
            .as_ref()
            .and_then(|outcome| outcome.as_ref().ok().cloned());
        if let Some(error) = preflight_memo
            .as_ref()
            .and_then(|outcome| outcome.as_ref().err())
            && tool_preflight_required()
        {
            return Err(error.to_string());
        }
        if preflight_memo.is_none()
            && preflight_requested
            && preflight_enabled
            && first_tool_hop_for_latest_user(messages)
        {
            self.emit_progress(
                stream,
                on_delta,
                "preflight",
                "running formation before the first tool action",
            );
            let planning = tool_preflight_history(messages);
            let outcome: ToolPreflightOutcome =
                match self.run(&planning, cancel, &mut |_| {}, false) {
                    Ok(advice) if !advice.trim().is_empty() => {
                        Ok(cap_tool_preflight_advice(advice.trim()).into())
                    }
                    Ok(_) => Err("SOTA-MOA tool preflight returned an empty advisory".into()),
                    Err(error) => Err(format!("SOTA-MOA tool preflight failed: {error}").into()),
                };
            if let (Some(key), Ok(mut memo)) = (preflight_key, self.tool_preflight.lock()) {
                *memo = Some((key, outcome.clone()));
            }
            match outcome {
                Ok(advice) => preflight_advice = Some(advice),
                Err(error) if tool_preflight_required() => return Err(error.to_string()),
                Err(_) => {}
            }
        }
        // Competition agents need the declared formation's planning diversity,
        // but their final answer is already grounded in the tool transcript. A
        // second full proposer/judge/verify wave only rewrites that report and
        // can multiply both latency and the now-large history tokens. Suppress
        // that duplicate wave only after a real preflight produced advice; if
        // an optional preflight never ran, ordinary amplification remains the
        // fallback that still exercises the requested formation.
        if tool_preflight_only() && preflight_advice.is_some() {
            amplify = false;
        }
        let advised_messages = preflight_advice.map(|advice| {
            let mut with_advice = messages.to_vec();
            let advisory = ChatMsg::harness(format!(
                "Formation preflight advisory (planning hypotheses only; no command, edit, \
                 benchmark, or verification has run yet). Check every claim with tools before \
                 acting:\n\n{advice}"
            ));
            // Keep the advisory immediately after the user request on every
            // hop. Appending it after a growing tool transcript would preserve
            // the text but destroy the provider's reusable request prefix.
            let insert_at = with_advice
                .iter()
                .rposition(|message| message.role == ChatRole::User)
                .map_or(with_advice.len(), |latest_user| latest_user + 1);
            with_advice.insert(insert_at, advisory);
            with_advice
        });
        let driver_messages = advised_messages.as_deref().unwrap_or(messages);
        // When we intend to amplify, suppress the driver pass's answer content —
        // the MoA output replaces it, so streaming it would double up. Reasoning
        // (the model's thinking) always flows through.
        let forward_content = stream && !amplify;

        let _seat_role = crate::harness::formation_budget::enter_role("coordinator");
        let driver = self.stream_tools_with_failover(
            &*self.clubs.aggregate,
            driver_messages,
            tools,
            cancel,
            on_delta,
            forward_content,
        );
        let driver_answer = match driver {
            Ok(ClubReply::Calls(calls)) => return Ok(ClubReply::Calls(calls)),
            Ok(ClubReply::Text(answer)) => Some(answer),
            // A dead driver pass can still be recovered by the MoA (own failover +
            // single-pass floor) when we were going to amplify anyway — but never a
            // botched tool call: the model wanted to *act*, and a text-only MoA
            // synthesis would fabricate an answer describing work that never ran.
            // Surface that error to the turn instead.
            Err(err) if error_indicates_raw_tool_markup_text(&err) => return Err(err),
            Err(_) if amplify => None,
            Err(err) => return Err(err),
        };

        // Raw tool markup in the driver answer is a tool call the whole bench
        // failed to repair. Never amplify it — the MoA would launder it into
        // confident prose about work that never happened. Return it as-is so the
        // harness's false-start guard suppresses it and forces a real structured
        // call (bounded by its own nudge cap).
        let raw_markup = driver_answer
            .as_ref()
            .is_some_and(|a| crate::club::contains_raw_tool_markup(a));
        let short = driver_answer
            .as_ref()
            .is_some_and(|a| a.trim().chars().count() < MIN_AMPLIFY_CHARS);
        if !amplify || (short && !explicitly_requested) || raw_markup {
            let answer = driver_answer.unwrap_or_default();
            // We suppressed the driver's content (amplify path) but then declined
            // to amplify (too short) — emit it now so the user still sees it.
            if stream && !forward_content && !answer.is_empty() {
                on_delta(StreamDelta::Content(&answer));
            }
            return Ok(ClubReply::Text(answer));
        }

        // Amplify: the MoA fan-out over the (now evidence-rich) conversation,
        // with the driver's answer seeded into the pool as draft 0 — a full
        // generation the judge/synthesis can keep instead of discarding it.
        // Any failure falls back to the driver's own answer so a flaky fan-out
        // never loses a good direct reply.
        match self.run_seeded(messages, cancel, on_delta, stream, driver_answer.as_deref()) {
            Ok(better) => {
                // A synthesized formation answer has no single certified seat.
                crate::harness::run_identity::publish_answer_route(
                    self as *const Self as usize,
                    Default::default(),
                );
                Ok(ClubReply::Text(better))
            }
            Err(moa_err) => match driver_answer {
                Some(answer) if !answer.trim().is_empty() => {
                    if stream {
                        on_delta(StreamDelta::Content(&answer));
                    }
                    Ok(ClubReply::Text(answer))
                }
                _ => Err(moa_err),
            },
        }
    }

    /// Streaming twin of [`Self::chat_with_failover`] that *keeps tools* and the
    /// real tool-protocol history — the tool-capable driver hop, not a text-only
    /// stage. Tool calls come back as `ClubReply::Calls` (success, passed through);
    /// raw tool markup in a would-be text answer, a quota cap, or an auth reject
    /// reroutes to the next spendable SOTA link **only while nothing has been shown
    /// to the user yet** (a committed partial can't be replayed). `forward_content`
    /// gates whether answer tokens stream live: off while we intend to amplify, so
    /// the MoA output isn't double-emitted.
    pub(crate) fn stream_tools_with_failover(
        &self,
        club: &dyn Club,
        full: &[ChatMsg],
        tools: &[ToolDef],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
        forward_content: bool,
    ) -> Result<ClubReply, String> {
        // `shown` = visible content the user can't un-see (only set when we
        // actually forward it). A buffered probe (forward_content=false) leaves it
        // clear, so a botched draft can still fail over.
        let shown = std::cell::Cell::new(false);
        // The last raw-markup text a link produced. If *every* link botches the
        // call the same way, the bench exhausts on a raw-markup error — surfacing
        // that markup (instead of swallowing it into an Err the amplify path
        // recovers) lets the harness guard force a structured retry.
        let last_raw_markup = std::cell::RefCell::new(None::<String>);
        let identity_key = self as *const Self as usize;
        crate::harness::run_identity::publish_answer_route(identity_key, Default::default());
        let mut attempt = |c: &dyn Club| {
            let mut tap = |d: StreamDelta<'_>| match d {
                StreamDelta::Content(t) => {
                    if forward_content {
                        shown.set(true);
                        on_delta(StreamDelta::Content(t));
                    }
                }
                StreamDelta::Reasoning(r) => on_delta(StreamDelta::Reasoning(r)),
                StreamDelta::Heartbeat => on_delta(StreamDelta::Heartbeat),
            };
            match c.chat_streaming(full, tools, cancel, &mut tap) {
                // A tool call is the goal here — pass it through untouched.
                Ok(ClubReply::Calls(calls)) => {
                    crate::harness::run_identity::publish_answer_route(
                        identity_key,
                        c.resolved_route_identity(),
                    );
                    Ok(ClubReply::Calls(calls))
                }
                // Raw tool markup in text means the model tried to call a tool in
                // prose (the LongCat brownout). If nothing was shown, treat it as a
                // failed hop and reroute to a link that can emit a structured call;
                // if it already streamed, let the harness guard catch it.
                Ok(ClubReply::Text(t))
                    if !shown.get() && crate::club::contains_raw_tool_markup(&t) =>
                {
                    let err = raw_tool_markup_text_error(c.label());
                    *last_raw_markup.borrow_mut() = Some(t);
                    Err(err)
                }
                other => {
                    if other.is_ok() {
                        crate::harness::run_identity::publish_answer_route(
                            identity_key,
                            c.resolved_route_identity(),
                        );
                    }
                    other
                }
            }
        };
        let mut result = attempt(club);
        let mut tried = vec![club.label().to_string()];
        loop {
            let Err(err) = &result else { return result };
            if shown.get() || !error_can_failover_tool_stage(err) {
                return result;
            }
            let Some(next) = self.quota_failover_order(&tried).next().cloned() else {
                // Bench exhausted. If the standing failure is raw tool markup,
                // hand the markup itself back to the harness: its false-start
                // guard suppresses the text and forces a real structured call,
                // bounded by its nudge cap. Swallowing it here is how the agent
                // "finished work" without ever executing a tool.
                if error_indicates_raw_tool_markup_text(err)
                    && let Some(markup) = last_raw_markup.borrow_mut().take()
                {
                    eprintln!(
                        "[{}] every SOTA link printed raw tool markup — \
                             surfacing it to the harness guard instead of \
                             synthesizing a text-only answer",
                        self.name
                    );
                    return Ok(ClubReply::Text(markup));
                }
                return result;
            };
            self.log_text_stage_failover(
                tried.last().map(String::as_str).unwrap_or("?"),
                &*next,
                err,
            );
            tried.push(next.label().to_string());
            result = attempt(&*next);
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/swarm__club_admission_tests.rs"]
mod worker_admission_tests;

impl SwarmClub {
    /// Visit every distinct underlying club once — raw fallback links first
    /// (cheap-role wrappers expose the same whole chain), then role seats,
    /// extras, and research — deduped by label so wrapper multiplicity can't
    /// multiply provider usage. The one enumeration behind every roster-summed
    /// usage report (tokens, truncation, cache, effort gate).
    fn for_each_unique_club(&self, mut f: impl FnMut(&dyn Club)) {
        let mut seen: Vec<String> = Vec::new();
        for club in self
            .fallbacks
            .iter()
            .chain([
                &self.clubs.propose,
                &self.clubs.judge,
                &self.clubs.verify,
                &self.clubs.aggregate,
            ])
            .chain(self.clubs.propose_extra.iter().map(|(club, _)| club))
            .chain(self.clubs.judge_extra.iter().map(|(club, _)| club))
            .chain(self.clubs.verify_extra.iter().map(|(club, _)| club))
            .chain(self.clubs.aggregate_extra.iter().map(|(club, _)| club))
            .chain(self.clubs.research.iter())
        {
            if seen.iter().any(|label| label == club.label()) {
                continue;
            }
            seen.push(club.label().to_string());
            f(club.as_ref());
        }
    }
}

fn available_role_roster(primary: &Arc<dyn Club>, extras: &[SeatExtra]) -> Vec<Arc<dyn Club>> {
    let mut out = vec![Arc::clone(primary)];
    out.extend(
        extras
            .iter()
            .filter(|(_, available)| {
                available
                    .as_ref()
                    .is_none_or(|gate| gate.load(Ordering::Relaxed))
            })
            .map(|(club, _)| Arc::clone(club)),
    );
    out
}

impl Club for SwarmClub {
    fn model_identity(&self) -> Option<String> {
        self.resolved_route_identity().model
    }

    fn resolved_route_identity(&self) -> crate::club::RouteIdentity {
        crate::harness::run_identity::answer_route(self as *const Self as usize).unwrap_or_default()
    }

    fn supports_formation_budget(&self) -> bool {
        let mut supported = !self.k.delegate;
        self.for_each_unique_club(|club| supported &= club.supports_formation_budget());
        supported
    }
    fn label(&self) -> &str {
        &self.name
    }

    /// The swarm is only reachable if its underlying model endpoint is — it fans
    /// out over the aggregate club, so delegate the readiness probe to it. When
    /// the aggregate is down (a quota-exhausted link reports unavailable), any
    /// spendable failover link keeps the swarm in play — role calls reroute at
    /// run time. `any` short-circuits, so the extra probing is one link in the
    /// common case; gated links answer instantly without a network probe.
    fn is_available(&self) -> bool {
        self.clubs.aggregate.is_available() || self.fallbacks.iter().any(|c| c.is_available())
    }

    /// A swarm answer is a distilled multi-agent synthesis — exactly the kind of
    /// artifact worth filing to the palace (through the Librarian).
    fn reports_to_palace(&self) -> bool {
        true
    }

    fn usage_accounting(&self) -> crate::club::AccountingView {
        let mut view = crate::club::AccountingView::default();
        for club in self
            .fallbacks
            .iter()
            .chain([
                &self.clubs.propose,
                &self.clubs.judge,
                &self.clubs.verify,
                &self.clubs.aggregate,
            ])
            .chain(self.clubs.propose_extra.iter().map(|(club, _)| club))
            .chain(self.clubs.judge_extra.iter().map(|(club, _)| club))
            .chain(self.clubs.verify_extra.iter().map(|(club, _)| club))
            .chain(self.clubs.aggregate_extra.iter().map(|(club, _)| club))
            .chain(self.clubs.research.iter())
        {
            view.extend(club.usage_accounting());
        }
        view
    }

    fn token_usage(&self) -> Option<TokenUsage> {
        let mut out = TokenUsage::default();
        self.for_each_unique_club(|club| {
            let Some(usage) = club.token_usage() else {
                return;
            };
            out.turns += usage.turns;
            out.last_input += usage.last_input;
            out.last_output += usage.last_output;
            out.last_reasoning += usage.last_reasoning;
            out.total_input += usage.total_input;
            out.total_output += usage.total_output;
            out.total_reasoning += usage.total_reasoning;
        });
        (out.turns > 0).then_some(out)
    }

    fn truncation_usage(&self) -> TruncationUsage {
        let mut out = TruncationUsage::default();
        self.for_each_unique_club(|club| {
            let usage = club.truncation_usage();
            out.episodes = out.episodes.saturating_add(usage.episodes);
            out.retained_partials = out
                .retained_partials
                .saturating_add(usage.retained_partials);
            out.retries = out.retries.saturating_add(usage.retries);
            out.recoveries = out.recoveries.saturating_add(usage.recoveries);
            out.failures = out.failures.saturating_add(usage.failures);
        });
        out
    }

    fn cache_usage(&self) -> CacheUsage {
        let mut out = CacheUsage::default();
        self.for_each_unique_club(|club| {
            let usage = club.cache_usage();
            out.control_requests = out.control_requests.saturating_add(usage.control_requests);
            out.read_input_tokens = out
                .read_input_tokens
                .saturating_add(usage.read_input_tokens);
            out.write_input_tokens = out
                .write_input_tokens
                .saturating_add(usage.write_input_tokens);
            out.read_accounting_responses = out
                .read_accounting_responses
                .saturating_add(usage.read_accounting_responses);
            out.write_accounting_responses = out
                .write_accounting_responses
                .saturating_add(usage.write_accounting_responses);
        });
        out
    }

    /// Tool-driving hops are sent through the aggregate club. Preserve that
    /// backend's byte-exact prefix-cache capability at the wrapper boundary so
    /// the outer turn loop does not rewrite old tool results between calls and
    /// invalidate a cache the actual model supports.
    fn prompt_cache_capable(&self) -> bool {
        self.clubs.aggregate.prompt_cache_capable()
    }

    /// A seat's withheld or rejected effort must reach the turn loop's voice
    /// even though the seat call happened deep inside a stage fan-out — the
    /// swarm is the driver club the harness deltas, so it speaks for its whole
    /// roster.
    fn effort_gate_usage(&self) -> EffortGateUsage {
        let mut out = EffortGateUsage::default();
        self.for_each_unique_club(|club| {
            let usage = club.effort_gate_usage();
            out.withheld = out.withheld.saturating_add(usage.withheld);
            out.rejections = out.rejections.saturating_add(usage.rejections);
            if usage.last.is_some() {
                out.last = usage.last;
            }
        });
        out
    }

    fn respond(&self, prompt: &str) -> Result<String, String> {
        let history = [ChatMsg::user(prompt)];
        self.run(&history, &AtomicBool::new(false), &mut |_| {}, false)
    }

    /// With no tools offered (benches, `respond`, control-bench, any text-only
    /// caller) the swarm is a pure MoA think. With tools (the real agent loop) it
    /// drives them: [`Self::drive_with_tools`] passes tool calls through so the
    /// agent actually works, and amplifies only the final answer.
    fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
        if tools.is_empty() {
            let answer = self.run(messages, &AtomicBool::new(false), &mut |_| {}, false)?;
            crate::harness::run_identity::publish_answer_route(
                self as *const Self as usize,
                Default::default(),
            );
            return Ok(ClubReply::Text(self.maybe_advise(
                messages,
                answer,
                &mut |_| {},
            )));
        }
        self.drive_with_tools(messages, tools, &AtomicBool::new(false), &mut |_| {}, false)
    }

    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        if tools.is_empty() {
            let answer = self.run(messages, cancel, on_delta, true)?;
            crate::harness::run_identity::publish_answer_route(
                self as *const Self as usize,
                Default::default(),
            );
            return Ok(ClubReply::Text(
                self.maybe_advise(messages, answer, on_delta),
            ));
        }
        self.drive_with_tools(messages, tools, cancel, on_delta, true)
    }
}

fn grok_research_role() -> Option<Arc<dyn Club>> {
    crate::club::GrokResearchClub::from_env().map(|club| Arc::new(club) as Arc<dyn Club>)
}

/// A driver answer shorter than this is usually a clarifying question,
/// acknowledgment, or one-liner the automatic MoA route can only bloat.
/// Explicit operator engagement bypasses this heuristic.
const MIN_AMPLIFY_CHARS: usize = 240;
const TOOL_PREFLIGHT_ADVICE_CHARS: usize = 16_000;

fn tool_preflight_enabled(explicitly_requested: bool) -> bool {
    env_flag_or("ANGEL_SOTA_MOA_TOOL_PREFLIGHT", explicitly_requested)
}

fn tool_preflight_required() -> bool {
    env_flag_or("ANGEL_SOTA_MOA_TOOL_PREFLIGHT_REQUIRED", false)
}

fn tool_preflight_only() -> bool {
    env_flag_or("ANGEL_SOTA_MOA_PREFLIGHT_ONLY", false)
}

fn cap_tool_preflight_advice(text: &str) -> String {
    let mut chars = text.chars();
    let head = chars
        .by_ref()
        .take(TOOL_PREFLIGHT_ADVICE_CHARS)
        .collect::<String>();
    if chars.next().is_some() {
        format!("{head}\n[formation advisory truncated]")
    } else {
        head
    }
}

fn tool_preflight_key(messages: &[ChatMsg]) -> Option<u64> {
    let latest_user = messages
        .iter()
        .rposition(|message| message.role == ChatRole::User)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for message in &messages[..=latest_user] {
        match message.role {
            ChatRole::System => 0u8,
            ChatRole::User => 1,
            ChatRole::Harness => 2,
            ChatRole::Assistant => 3,
            ChatRole::Tool => 4,
        }
        .hash(&mut hasher);
        message.content.hash(&mut hasher);
        message.tool_call_id.hash(&mut hasher);
        for call in message.tool_calls.iter() {
            call.id.hash(&mut hasher);
            call.name.hash(&mut hasher);
            call.args.to_string().hash(&mut hasher);
        }
    }
    Some(hasher.finish())
}

fn first_tool_hop_for_latest_user(messages: &[ChatMsg]) -> bool {
    let Some(latest_user) = messages
        .iter()
        .rposition(|message| message.role == ChatRole::User)
    else {
        return false;
    };
    messages[latest_user + 1..]
        .iter()
        .all(|message| message.role != ChatRole::Tool && message.tool_calls.is_empty())
}

fn tool_preflight_history(messages: &[ChatMsg]) -> Vec<ChatMsg> {
    let mut planning = text_only_history(messages);
    if let Some(message) = planning
        .iter_mut()
        .rev()
        .find(|message| message.role == ChatRole::User)
    {
        message.content = format!(
            "{}\n\n[FORMATION PREFLIGHT: Produce an independent, concrete action plan and \
             risk review for the coordinator. This is planning only: no tool is available, \
             nothing has executed, and you must not claim edits, tests, or measurements.]",
            message.content
        )
        .into();
    }
    planning
}

const RAW_TOOL_MARKUP_TEXT_ERR: &str = "printed raw tool markup as text";
const TOOL_CALL_TEXT_STAGE_ERR: &str = "requested a tool in a text-only stage";

/// Sent back to a seat that answered a text-only stage with a tool call. Names
/// the exact dialects seen live so the correction is unambiguous to the model
/// that just used one.
const TEXT_ONLY_CORRECTION: &str = "Your previous reply was a tool call. No tools are available in \
this stage and nothing was executed — a tool call here is discarded. Answer now in plain prose, \
from what you already know: no tool calls and no tool-call markup of any kind (`[TOOL_CALLS]`, \
`<tool_call>`, `<function=…>`, `<SHELL>{…}`).";

/// Remove the driver's tool-protocol instructions from a text-only stage's
/// system prompt, and say plainly that no tools exist here.
///
/// The driver prompt tells the model to "use built-in tools for local work,
/// `skill(name)` for reusable playbooks" — correct for the driver, actively
/// harmful for a proposer seat, which has no tools wired and whose reply is
/// discarded if it emits a call. Seats that follow instructions closely obey the
/// system prompt over a trailing stage instruction: Leanstral answered every
/// turn with `[TOOL_CALLS]skill[ARGS]{"name": "quantize-model"}` rather than
/// analysis. Only whole paragraphs that are *about* calling tools are dropped;
/// workspace identity, posture, and repo context all survive.
pub(crate) fn text_only_system(base_sys: &str) -> String {
    const TOOL_PARAGRAPH_MARKERS: &[&str] = &[
        "tool protocol:",
        "skill(name)",
        "structured tool-call interface",
    ];
    let kept: Vec<&str> = base_sys
        .split("\n\n")
        .filter(|paragraph| {
            let lowered = paragraph.to_ascii_lowercase();
            !TOOL_PARAGRAPH_MARKERS
                .iter()
                .any(|marker| lowered.contains(marker))
        })
        .collect();
    let mut out = kept.join("\n\n");
    let trimmed_len = out.trim_end().len();
    out.truncate(trimmed_len);
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(
        "This is an analysis-only stage. You have no tools here and no tool call will execute — \
         a reply containing tool-call markup is discarded unread. Answer in prose from what you \
         already know.",
    );
    out
}

fn is_text_stage_tool_markup_error(err: &str) -> bool {
    err.contains(RAW_TOOL_MARKUP_TEXT_ERR) || err.contains(TOOL_CALL_TEXT_STAGE_ERR)
}

fn raw_tool_markup_text_error(label: &str) -> String {
    format!("swarm worker on {label} {RAW_TOOL_MARKUP_TEXT_ERR} (no tool executed)")
}

fn tool_call_text_stage_error(label: &str) -> String {
    format!("swarm worker on {label} {TOOL_CALL_TEXT_STAGE_ERR} (no tool executed)")
}

/// Prose written *before* a model slipped into tool-call markup, when there is
/// enough of it to stand on its own as a draft.
///
/// A seat that writes a page of analysis and then tacks on a `<SHELL>{…}` line
/// used to have its entire contribution discarded — which, in a two-corner
/// formation, silently reduced the mixture to one model (and, under a strict
/// quorum, killed the turn outright). The markup itself is still never honored:
/// no tool runs, and the tail is cut away. This only rescues the analysis.
///
/// Anything shorter than the floor is treated as the dangerous case it is — a
/// reply that is essentially *only* a tool request — and still fails closed.
fn salvage_text_before_tool_markup(text: &str) -> Option<String> {
    const MIN_SALVAGE_CHARS: usize = 200;
    const MARKERS: &[&str] = &[
        "<tool_call",
        "<function=",
        "</function>",
        "[tool_calls]",
        "<longcat_tool_call",
        "<longcat_arg_key>",
        "<parameter=",
    ];
    let lowered = text.to_ascii_lowercase();
    let mut cut = MARKERS
        .iter()
        .filter_map(|marker| lowered.find(marker))
        .min();
    // The shouted-tag dialect (`<SHELL>{`) has no fixed spelling, so find the
    // first uppercase tag that opens a JSON object.
    let bytes = text.as_bytes();
    for (index, _) in text.match_indices('<') {
        let after = &text[index + 1..];
        let name_len = after
            .chars()
            .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
            .map(char::len_utf8)
            .sum::<usize>();
        if name_len >= 2 && after[name_len..].starts_with('>') {
            let rest = after[name_len + 1..].trim_start();
            if rest.starts_with('{') && bytes.get(index).is_some() {
                cut = Some(cut.map_or(index, |current: usize| current.min(index)));
                break;
            }
        }
    }
    let prose = text[..cut?].trim();
    (prose.chars().count() >= MIN_SALVAGE_CHARS).then(|| prose.to_string())
}

fn guard_text_stage_reply(
    reply: Result<ClubReply, String>,
    label: &str,
) -> Result<ClubReply, String> {
    match reply {
        Ok(ClubReply::Text(t)) if crate::club::contains_raw_tool_markup(&t) => {
            match salvage_text_before_tool_markup(&t) {
                Some(prose) => {
                    eprintln!(
                        "[swarm] {label} emitted tool markup in a text-only stage — kept {} chars of analysis, discarded the call",
                        prose.chars().count()
                    );
                    Ok(ClubReply::Text(prose))
                }
                None => {
                    if env_flag_or("ANGEL_SWARM_DRAFT_DEBUG", false) {
                        let preview: String = t.trim().chars().take(400).collect();
                        eprintln!(
                            "[swarm:draft] {label} rejected reply ({} chars) | {}",
                            t.chars().count(),
                            preview.replace('\n', "⏎")
                        );
                    }
                    Err(raw_tool_markup_text_error(label))
                }
            }
        }
        Ok(ClubReply::Calls(_)) => Err(tool_call_text_stage_error(label)),
        other => other,
    }
}

// ---------------------------------------------------------------------------
// Stage-call machinery as free functions: a detached fan-out worker crosses a
// 'static thread boundary, so it carries an owned (name, fallback bench) pair
// instead of borrowing &SwarmClub. The SwarmClub methods delegate here — one
// body serves both the serial stage calls and the bounded fan-out.
// ---------------------------------------------------------------------------

/// Parse one optional per-seat effort knob: trimmed, lowercased, empty = unset.
fn env_effort(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
}

pub(crate) fn stage_call_on(
    name: &str,
    fallbacks: &[Arc<dyn Club>],
    club: &dyn Club,
    system: &str,
    msgs: &[ChatMsg],
    effort: Option<&str>,
) -> Result<String, String> {
    let mut full = Vec::with_capacity(msgs.len() + 1);
    if !system.trim().is_empty() {
        full.push(ChatMsg::system(system));
    }
    full.extend_from_slice(msgs);
    SwarmClub::text_reply(
        stage_chat_with_failover(name, fallbacks, club, &full, effort),
        club.label(),
    )
}

pub(crate) fn stage_chat_with_failover(
    name: &str,
    fallbacks: &[Arc<dyn Club>],
    club: &dyn Club,
    full: &[ChatMsg],
    effort: Option<&str>,
) -> Result<ClubReply, String> {
    let full = text_only_history(full);
    let mut result =
        guard_text_stage_reply(club.chat_with_effort(&full, &[], effort), club.label());
    // A seat that answers a text-only stage with a tool call is usually not
    // incapable — it is imitating the tool protocol the driver's system prompt
    // documents (Mistral-family seats emit `[TOOL_CALLS]` especially readily).
    // Correcting it in place recovers the draft; failing over to another link
    // costs the same call while losing this seat's angle entirely, which is how
    // a two-corner formation quietly became a one-model answer.
    if result
        .as_ref()
        .err()
        .is_some_and(|err| is_text_stage_tool_markup_error(err))
    {
        // Only the offending seat pays for this: the first attempt keeps the
        // conversation's own system prefix so provider prompt caches stay hot
        // across stages, and only the retry rewrites it.
        let mut corrected = full.clone();
        if let Some(system) = corrected
            .iter_mut()
            .find(|msg| msg.role == ChatRole::System && !msg.content.trim().is_empty())
        {
            system.content = text_only_system(&system.content).into();
        }
        corrected.push(ChatMsg::user(TEXT_ONLY_CORRECTION));
        let retry =
            guard_text_stage_reply(club.chat_with_effort(&corrected, &[], effort), club.label());
        if retry.is_ok() {
            eprintln!(
                "[swarm] {} answered with tool markup in a text-only stage; recovered on correction",
                club.label()
            );
            result = retry;
        }
    }
    let mut tried = vec![club.label().to_string()];
    loop {
        let Err(err) = &result else { return result };
        if !error_can_failover_text_stage(err) {
            return result;
        }
        let Some(next) = stage_quota_failover_order(fallbacks, &tried)
            .next()
            .cloned()
        else {
            return result;
        };
        stage_log_failover(
            name,
            tried.last().map(String::as_str).unwrap_or("?"),
            &*next,
            err,
        );
        tried.push(next.label().to_string());
        result = guard_text_stage_reply(next.chat_with_effort(&full, &[], effort), next.label());
    }
}

fn stage_quota_failover_order<'a>(
    fallbacks: &'a [Arc<dyn Club>],
    tried: &'a [String],
) -> impl Iterator<Item = &'a Arc<dyn Club>> {
    fallbacks.iter().filter(move |c| {
        !tried.iter().any(|t| t.eq_ignore_ascii_case(c.label())) && c.quota_cooldown().is_none()
    })
}

fn stage_log_failover(name: &str, from: &str, next: &dyn Club, err: &str) {
    let reason = if crate::club::error_indicates_quota_exhausted(err) {
        "quota exhausted"
    } else if crate::club::error_indicates_auth_failed(err) {
        "auth failed"
    } else if error_indicates_raw_tool_markup_text(err) {
        "raw tool markup"
    } else if error_indicates_tool_call_text_stage(err) {
        "tool call in text-only stage"
    } else {
        "text-stage error"
    };
    eprintln!(
        "[{name}] {from} {reason} — failing over to {}",
        next.label()
    );
    // Keep the stderr line above (operator-visible), and also durably record
    // the failover for the experience ledger. Both the text-stage and the
    // tool-stage reroute loops funnel through here, so this one sink captures
    // every failover — including the LongCat raw-tool-markup brownout that
    // this whole subsystem exists to diagnose. Drained into the turn record.
    crate::experience::note_failover(from, next.label(), reason);
}

fn error_indicates_raw_tool_markup_text(err: &str) -> bool {
    err.contains(RAW_TOOL_MARKUP_TEXT_ERR)
}

fn error_indicates_tool_call_text_stage(err: &str) -> bool {
    err.contains(TOOL_CALL_TEXT_STAGE_ERR)
}

fn error_can_failover_text_stage(err: &str) -> bool {
    crate::club::error_indicates_quota_exhausted(err)
        || crate::club::error_indicates_auth_failed(err)
        || error_indicates_raw_tool_markup_text(err)
        || error_indicates_tool_call_text_stage(err)
}

/// Failover triggers for the tool-capable driver hop. Unlike the text-only
/// stages, a *tool call* is the desired outcome (never a failover reason); only a
/// spent quota, a rejected key, or raw tool markup surfacing as text (a botched
/// call) reroutes to the next SOTA link.
fn error_can_failover_tool_stage(err: &str) -> bool {
    crate::club::error_indicates_quota_exhausted(err)
        || crate::club::error_indicates_auth_failed(err)
        || error_indicates_raw_tool_markup_text(err)
}
