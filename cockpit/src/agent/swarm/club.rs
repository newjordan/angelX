//! SwarmClub construction, role-call plumbing, and the Club impl.

use super::*;
use crate::agent::club::{CacheUsage, EffortGateUsage, TruncationUsage};
use crate::agent::harness::book::{d3_roles, d5_frames, st_connected};
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

pub(crate) type SeatExtra = (Arc<dyn Club>, Option<Arc<AtomicBool>>);

#[derive(Clone)]
pub(crate) struct RoleClubs {
    pub(crate) propose: Arc<dyn Club>,
    pub(crate) propose_extra: Vec<SeatExtra>,
    pub(crate) judge: Arc<dyn Club>,
    pub(crate) judge_extra: Vec<SeatExtra>,
    pub(crate) verify: Arc<dyn Club>,
    pub(crate) verify_extra: Vec<SeatExtra>,
    pub(crate) aggregate: Arc<dyn Club>,
    pub(crate) aggregate_extra: Vec<SeatExtra>,
    pub(crate) research: Option<Arc<dyn Club>>,
}

type ToolPreflightOutcome = Result<Arc<str>, Arc<str>>;

pub struct SwarmClub {
    pub(crate) name: String,
    pub(crate) clubs: RoleClubs,
    pub(crate) k: Knobs,
    pub(crate) fallbacks: Vec<Arc<dyn Club>>,
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

    pub fn from_env(name: impl Into<String>, inner: Arc<dyn Club>) -> Self {
        Self::from_env_with(name, inner, |_| None)
    }

    pub fn from_env_with(
        name: impl Into<String>,
        inner: Arc<dyn Club>,
        resolve: impl Fn(&str) -> Option<Arc<dyn Club>>,
    ) -> Self {
        let name = name.into();
        let k = Self::knobs_from_env();
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

    pub(crate) fn with_engaged_formation(mut self) -> Self {
        self.k.engaged = true;
        self
    }

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

    pub fn with_quota_fallbacks(mut self, fallbacks: Vec<Arc<dyn Club>>) -> Self {
        self.fallbacks = fallbacks;
        self
    }

    pub(crate) fn gated(&self, k: Knobs) -> SwarmClub {
        SwarmClub {
            name: self.name.clone(),
            clubs: self.clubs.clone(),
            k,
            fallbacks: self.fallbacks.clone(),
            tool_preflight: Arc::clone(&self.tool_preflight),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn call_on(
        &self,
        club: &dyn Club,
        system: &str,
        msgs: &[ChatMsg],
    ) -> Result<String, String> {
        self.call_on_with_effort(club, system, msgs, None)
    }

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
            ClubReply::Text(t) if crate::agent::club::contains_raw_tool_markup(&t) => {
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

    pub(crate) fn chat_with_failover(
        &self,
        club: &dyn Club,
        full: &[ChatMsg],
    ) -> Result<ClubReply, String> {
        stage_chat_with_failover(&self.name, &self.fallbacks, club, full, None)
    }

    pub(crate) fn stream_with_failover(
        &self,
        club: &dyn Club,
        full: &[ChatMsg],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
        allow_buffered_raw_markup_retry: bool,
    ) -> Result<ClubReply, String> {
        let full = text_only_history(full);
        let workspace = crate::agent::harness::book::connect::workspace();
        let emitted = std::cell::Cell::new(false);
        let mut attempt = |c: &dyn Club| {
            let mut tap = |d: StreamDelta<'_>| {
                if matches!(d, StreamDelta::Content(_)) {
                    emitted.set(true);
                }
                on_delta(d);
            };
            let reply = crate::agent::harness::book::connect::converse(
                &workspace,
                &full,
                |history, tools| c.chat_streaming(history, tools, cancel, &mut tap),
            );
            guard_text_stage_reply(reply, c.label())
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

    fn quota_failover_order<'a>(
        &'a self,
        tried: &'a [String],
    ) -> impl Iterator<Item = &'a Arc<dyn Club>> {
        stage_quota_failover_order(&self.fallbacks, tried)
    }

    pub(crate) fn call(&self, system: &str, msgs: &[ChatMsg]) -> Result<String, String> {
        self.call_on_with_effort(
            &*self.clubs.aggregate,
            system,
            msgs,
            self.k.seat_efforts.aggregate.as_deref(),
        )
    }

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
        if let Some(budget) = crate::agent::harness::formation_budget::current() {
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
        use std::time::Duration;
        let n = clubs.len();
        if n == 0 {
            return Vec::new();
        }
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
                let effort = effort.map(str::to_string);
                let tx = tx.clone();
                let budget = crate::agent::harness::formation_budget::current();
                let role = crate::agent::harness::formation_budget::role();
                let ledger = crate::agent::harness::book::connect::workspace();
                let book_catalog = crate::agent::harness::book::catalog::current();
                std::thread::spawn(move || {
                    let _book_catalog = book_catalog.map(crate::agent::harness::book::catalog::enter);
                    let _budget_scope = crate::agent::harness::formation_budget::enter(budget);
                    let _seat_role = crate::agent::harness::formation_budget::enter_role(&role);
                    let _ledger = crate::agent::harness::book::connect::enter(&ledger);
                    let out =
                        stage_call_on(&name, &fallbacks, &*club, &system, &msgs, effort.as_deref());
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
        let mut slots: Vec<Option<Result<String, String>>> = (0..n).map(|_| None).collect();
        let mut returned = 0usize;
        while returned < n {
            if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                break;
            }
            let wait = Duration::from_millis(250);
            match rx.recv_timeout(wait) {
                Ok((i, r)) => {
                    if slots[i].is_none() {
                        returned += 1;
                        crate::ui::viz::agentviz::stage_seat_update(
                            i,
                            if r.is_ok() {
                                crate::ui::viz::agentviz::SeatState::Returned
                            } else {
                                crate::ui::viz::agentviz::SeatState::Failed
                            },
                        );
                    }
                    slots[i] = Some(r);
                    let barrier_open = !cancel.is_some_and(|c| c.load(Ordering::Relaxed));
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
        drop(tx.take());
        for (i, slot) in slots.iter().enumerate() {
            if slot.is_none() {
                crate::ui::viz::agentviz::stage_seat_update(
                    i,
                    crate::ui::viz::agentviz::SeatState::Cut,
                );
            }
        }
        slots
            .into_iter()
            .map(|r| {
                r.unwrap_or_else(|| Err("cancelled (seat abandoned to background)".to_string()))
            })
            .collect()
    }

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
        let routing = route_decisions(&problem);
        let summoned = routing.moa;
        let explicitly_requested = self.k.always || self.k.mode == MoaMode::Always || summoned;
        let mut amplify = match self.k.mode {
            MoaMode::Always => true,
            MoaMode::Manual => self.k.always || self.k.engaged || summoned,
            MoaMode::Auto => {
                self.k.always
                    || summoned
                    || self.research_route_for(routing.research)
                    || routing.synthesis
            }
        };
        let preflight_requested = explicitly_requested || self.k.engaged;
        let preflight_enabled = tool_preflight_enabled(
            preflight_requested && self.name.eq_ignore_ascii_case("sota-moa"),
        );
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
        if tool_preflight_only() && preflight_advice.is_some() {
            amplify = false;
        }
        let advised_messages = preflight_advice.map(|advice| {
            let mut with_advice = messages.to_vec();
            let advisory = ChatMsg::harness(format!(
                "{}\n\n{advice}",
                crate::agent::harness::book::ing_drivers::PREFLIGHT.cells()
            ));
            let insert_at = with_advice
                .iter()
                .rposition(|message| message.role == ChatRole::User)
                .map_or(with_advice.len(), |latest_user| latest_user + 1);
            with_advice.insert(insert_at, advisory);
            with_advice
        });
        let driver_messages = advised_messages.as_deref().unwrap_or(messages);
        let forward_content = stream && !amplify;

        let _seat_role = crate::agent::harness::formation_budget::enter_role("coordinator");
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
            Err(err) if error_indicates_raw_tool_markup_text(&err) => return Err(err),
            Err(_) if amplify => None,
            Err(err) => return Err(err),
        };

        let raw_markup = driver_answer
            .as_ref()
            .is_some_and(|a| crate::agent::club::contains_raw_tool_markup(a));
        let short = driver_answer
            .as_ref()
            .is_some_and(|a| a.trim().chars().count() < MIN_AMPLIFY_CHARS);
        if !amplify || (short && !explicitly_requested) || raw_markup {
            let answer = driver_answer.unwrap_or_default();
            if stream && !forward_content && !answer.is_empty() {
                on_delta(StreamDelta::Content(&answer));
            }
            return Ok(ClubReply::Text(answer));
        }

        match self.run_seeded(messages, cancel, on_delta, stream, driver_answer.as_deref()) {
            Ok(better) => {
                crate::agent::harness::run_identity::publish_answer_route(
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

    pub(crate) fn stream_tools_with_failover(
        &self,
        club: &dyn Club,
        full: &[ChatMsg],
        tools: &[ToolDef],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
        forward_content: bool,
    ) -> Result<ClubReply, String> {
        let shown = std::cell::Cell::new(false);
        let last_raw_markup = std::cell::RefCell::new(None::<String>);
        let identity_key = self as *const Self as usize;
        crate::agent::harness::run_identity::publish_answer_route(identity_key, Default::default());
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
                Ok(ClubReply::Calls(calls)) => {
                    crate::agent::harness::run_identity::publish_answer_route(
                        identity_key,
                        c.resolved_route_identity(),
                    );
                    Ok(ClubReply::Calls(calls))
                }
                Ok(ClubReply::Text(t))
                    if !shown.get() && crate::agent::club::contains_raw_tool_markup(&t) =>
                {
                    let err = raw_tool_markup_text_error(c.label());
                    *last_raw_markup.borrow_mut() = Some(t);
                    Err(err)
                }
                other => {
                    if other.is_ok() {
                        crate::agent::harness::run_identity::publish_answer_route(
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
#[path = "../../../../tests/cockpit/app/swarm__club_admission_tests.rs"]
mod worker_admission_tests;

impl SwarmClub {
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

    fn resolved_route_identity(&self) -> crate::agent::club::RouteIdentity {
        crate::agent::harness::run_identity::answer_route(self as *const Self as usize)
            .unwrap_or_default()
    }

    fn supports_formation_budget(&self) -> bool {
        let mut supported = !self.k.delegate;
        self.for_each_unique_club(|club| supported &= club.supports_formation_budget());
        supported
    }
    fn label(&self) -> &str {
        &self.name
    }

    fn is_available(&self) -> bool {
        self.clubs.aggregate.is_available() || self.fallbacks.iter().any(|c| c.is_available())
    }

    fn reports_to_palace(&self) -> bool {
        true
    }

    fn usage_accounting(&self) -> crate::agent::club::AccountingView {
        let mut view = crate::agent::club::AccountingView::default();
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

    fn prompt_cache_capable(&self) -> bool {
        self.clubs.aggregate.prompt_cache_capable()
    }

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

    fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
        if tools.is_empty() {
            let answer = self.run(messages, &AtomicBool::new(false), &mut |_| {}, false)?;
            crate::agent::harness::run_identity::publish_answer_route(
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
            crate::agent::harness::run_identity::publish_answer_route(
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
    crate::agent::club::GrokResearchClub::from_env().map(|club| Arc::new(club) as Arc<dyn Club>)
}

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
        format!("{head}\n{}", d3_roles::pages(d5_frames::PREFLIGHT, [3]))
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
            "{}\n\n{}",
            message.content,
            d3_roles::pages(d5_frames::PREFLIGHT, [1, 2])
        )
        .into();
    }
    planning
}

const RAW_TOOL_MARKUP_TEXT_ERR: &str = "printed raw tool markup as text";
const TOOL_CALL_TEXT_STAGE_ERR: &str = "requested a tool in a text-only stage";

fn text_only_correction() -> String {
    d3_roles::pages(st_connected::TEXT_ONLY, [4, 5, 6])
}

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
    out.push_str(&d3_roles::pages(st_connected::TEXT_ONLY, [1, 2, 3]));
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
        Ok(ClubReply::Text(t)) if crate::agent::club::contains_raw_tool_markup(&t) => {
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
    let workspace = crate::agent::harness::book::connect::workspace();
    let connected = |club: &dyn Club, full: &[ChatMsg]| {
        let reply =
            crate::agent::harness::book::connect::converse(&workspace, full, |history, tools| {
                club.chat_with_effort(history, tools, effort)
            });
        guard_text_stage_reply(reply, club.label())
    };
    let mut result = connected(club, &full);
    if result
        .as_ref()
        .err()
        .is_some_and(|err| is_text_stage_tool_markup_error(err))
    {
        let mut corrected = full.clone();
        if let Some(system) = corrected
            .iter_mut()
            .find(|msg| msg.role == ChatRole::System && !msg.content.trim().is_empty())
        {
            system.content = text_only_system(&system.content).into();
        }
        corrected.push(ChatMsg::user(text_only_correction()));
        let retry = connected(club, &corrected);
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
        result = connected(&*next, &full);
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
    let reason = if crate::agent::club::error_indicates_quota_exhausted(err) {
        "quota exhausted"
    } else if crate::agent::club::error_indicates_auth_failed(err) {
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
    crate::knowledge::experience::note_failover(from, next.label(), reason);
}

fn error_indicates_raw_tool_markup_text(err: &str) -> bool {
    err.contains(RAW_TOOL_MARKUP_TEXT_ERR)
}

fn error_indicates_tool_call_text_stage(err: &str) -> bool {
    err.contains(TOOL_CALL_TEXT_STAGE_ERR)
}

fn error_can_failover_text_stage(err: &str) -> bool {
    crate::agent::club::error_indicates_quota_exhausted(err)
        || crate::agent::club::error_indicates_auth_failed(err)
        || error_indicates_raw_tool_markup_text(err)
        || error_indicates_tool_call_text_stage(err)
}

fn error_can_failover_tool_stage(err: &str) -> bool {
    crate::agent::club::error_indicates_quota_exhausted(err)
        || crate::agent::club::error_indicates_auth_failed(err)
        || error_indicates_raw_tool_markup_text(err)
}
