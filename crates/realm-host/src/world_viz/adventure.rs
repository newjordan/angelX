//! The adventure model (Zelda overworld Z1): pure state + events.
//!
//! The self-improvement loop becomes a quest. A `LoopSummary` of the loop is
//! diffed against a [`LoopMirror`] once per frame and folded into [`AdventureEvent`]s; those
//! events drive the [`Quest`] state machine (region, danger, loot, banners,
//! party). Everything here is deterministic in `(state, event, tick)` — no
//! clocks, no maps, no randomness — so Z2's renderers can key frames on it.
//!
//! Nothing in this module draws anything. It is the single authority for
//! "where is the hero and how much trouble are they in".

use super::{Building, RealmActivity};
use crate::loop_summary::{EscalationTier, LoopStatus, LoopSummary};

const BANNER_TICKS: u64 = 40;
const HOMECOMING_TICKS: u64 = 120;
const TOOL_MIX_WINDOW: usize = 12;
pub(super) const WALK_LOOKBACK: u64 = 12;
const WALK_SEGMENTS: usize = WALK_LOOKBACK as usize + 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Region {
    CastleTown,
    TheMines,
    DarkForest,
    Swamp,
    DragonKeep,
    Homecoming,
}

impl Region {
    pub fn label(self) -> &'static str {
        match self {
            Region::CastleTown => "Castle Town",
            Region::TheMines => "The Mines",
            Region::DarkForest => "Dark Forest",
            Region::Swamp => "The Swamp",
            Region::DragonKeep => "Dragon Keep",
            Region::Homecoming => "Homecoming",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Danger(u8);

impl Danger {
    pub fn calm() -> Self {
        Danger(0)
    }

    fn new(level: u8) -> Self {
        Danger(level.min(3))
    }

    pub fn level(self) -> u8 {
        self.0
    }

    pub fn from_stall(stale: usize, pivot: usize) -> Self {
        Danger::new((stale * 3 / pivot.max(1)).min(3) as u8)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LoopKind {
    Competition,
    Research,
    Coding,
    #[default]
    Unknown,
}

impl LoopKind {
    pub fn label(self) -> &'static str {
        match self {
            LoopKind::Competition => "competition",
            LoopKind::Research => "research",
            LoopKind::Coding => "coding",
            LoopKind::Unknown => "unclassified",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AdventureEvent {
    LoopStarted {
        kind: LoopKind,
        task: String,
    },
    Iteration {
        n: usize,
    },
    Stall {
        level: u8,
    },
    Pivot,
    MeasurementObserved,
    #[cfg(any(test, feature = "testing"))]
    Measured {
        improved: bool,
    },
    Submitted,
    SubmissionSettled {
        accepted: bool,
    },
    LoopFinished {
        ok: bool,
    },
    Tool(Building, RealmActivity),
    TurnStarted,
    TurnEnded {
        ok: bool,
    },
    Party {
        size: u8,
    },
    RecordPromoted {
        score: Option<String>,
    },
    PromotionFailed {
        score: Option<String>,
    },
}

pub const RECORD_FIGHT: u64 = 40 * 180;
pub const RETREAT_FIGHT: u64 = 40 * 120;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quest {
    region: Region,
    danger: Danger,
    iteration: usize,
    treasures: u32,
    empty_chests: u32,
    measurements_observed: u32,
    party: u8,
    banner: Option<&'static str>,
    since_tick: u64,
    kind: LoopKind,
    home_return_until: u64,
    on_adventure: bool,
    submission_latch: bool,
    last_tick: u64,
    seen_region: Region,
    region_since: u64,
    party_since: u64,
    walk_clock: Vec<WalkPace>,
    region_rollback: Option<RegionRollback>,
    records: u32,
    retreats: u32,
    record_at: Option<u64>,
    record_won: bool,
    record_score: Option<String>,
    measured_at: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RegionRollback {
    tick: u64,
    region: Region,
    since: u64,
    clock: Vec<WalkPace>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WalkPace {
    tick: u64,
    half_ticks: u128,
    slow: bool,
}

impl Quest {
    #[inline(never)]
    pub fn idle() -> Self {
        Quest {
            region: Region::CastleTown,
            danger: Danger::calm(),
            iteration: 0,
            treasures: 0,
            empty_chests: 0,
            measurements_observed: 0,
            party: 1,
            banner: None,
            since_tick: 0,
            kind: LoopKind::Unknown,
            home_return_until: 0,
            on_adventure: false,
            submission_latch: false,
            last_tick: 0,
            seen_region: Region::CastleTown,
            region_since: 0,
            party_since: 0,
            walk_clock: Vec::new(),
            region_rollback: None,
            records: 0,
            retreats: 0,
            record_at: None,
            record_won: true,
            record_score: None,
            measured_at: None,
        }
    }

    pub fn apply(&mut self, ev: AdventureEvent, tick: u64) {
        let walk_tick = tick.max(self.last_tick);
        self.advance_walk_history(walk_tick);
        let reset_walk = matches!(
            ev,
            AdventureEvent::LoopStarted { .. } | AdventureEvent::Iteration { .. }
        );
        match ev {
            AdventureEvent::LoopStarted { kind, .. } => {
                self.kind = kind;
                self.iteration = 0;
                self.danger = Danger::calm();
                self.home_return_until = 0;
                self.submission_latch = false;
                self.on_adventure = true;
                self.settle();
            }
            AdventureEvent::Iteration { n } => {
                self.iteration = n;
                self.submission_latch = false;
                self.settle();
            }
            AdventureEvent::Stall { level } => {
                self.danger = Danger::new(level);
                self.settle();
            }
            AdventureEvent::Pivot => {
                self.danger = Danger::calm();
                self.settle();
            }
            AdventureEvent::MeasurementObserved => {
                self.measurements_observed = self.measurements_observed.saturating_add(1);
                self.measured_at = Some(tick);
                self.flash("measurement recorded", tick);
                self.settle();
            }
            #[cfg(any(test, feature = "testing"))]
            AdventureEvent::Measured { improved } => {
                if improved {
                    self.treasures = self.treasures.saturating_add(1);
                    self.flash("treasure!", tick);
                } else {
                    self.empty_chests = self.empty_chests.saturating_add(1);
                    self.flash("empty chest", tick);
                }
            }
            AdventureEvent::Submitted => {
                self.submission_latch = true;
                self.settle();
            }
            AdventureEvent::SubmissionSettled { accepted } => {
                if accepted {
                    self.submission_latch = true;
                    self.flash("banner raised", tick);
                } else {
                    self.submission_latch = false;
                }
                self.settle();
            }
            AdventureEvent::LoopFinished { ok } => {
                self.on_adventure = false;
                self.submission_latch = false;
                self.danger = Danger::calm();
                if ok {
                    self.region = Region::Homecoming;
                    self.home_return_until = tick.saturating_add(HOMECOMING_TICKS);
                    self.since_tick = tick;
                    self.banner = Some("homecoming");
                } else {
                    self.region = Region::CastleTown;
                    self.home_return_until = 0;
                    self.flash("retreat", tick);
                }
            }
            AdventureEvent::Tool(..) => {
            }
            AdventureEvent::TurnStarted | AdventureEvent::TurnEnded { .. } => {
            }
            AdventureEvent::Party { size } => {
                let size = size.clamp(1, 8);
                if size != self.party {
                    self.party_since = tick;
                }
                self.party = size;
            }
            AdventureEvent::RecordPromoted { score } => {
                self.records += 1;
                self.record_at = Some(tick);
                self.record_won = true;
                self.record_score = score;
            }
            AdventureEvent::PromotionFailed { score } => {
                self.retreats += 1;
                self.record_at = Some(tick);
                self.record_won = false;
                self.record_score = score;
            }
        }
        if reset_walk {
            self.region_rollback = None;
            if self.seen_region != self.region() {
                self.seen_region = self.region();
                self.region_since = tick;
            }
            self.reset_walk_clock(tick);
        } else {
            self.mark_region(walk_tick);
            self.note_walk_pace(walk_tick);
        }
    }

    fn reset_walk_clock(&mut self, tick: u64) {
        let tick = tick.max(self.last_tick);
        self.walk_clock = Vec::with_capacity(WALK_SEGMENTS);
        self.walk_clock.push(WalkPace {
            tick,
            half_ticks: 0,
            slow: self.danger.level() >= 1,
        });
    }

    fn note_walk_pace(&mut self, tick: u64) {
        let tick = tick.max(self.last_tick);
        let slow = self.danger.level() >= 1;
        if self.walk_clock.last().is_some_and(|pace| pace.slow == slow) {
            return;
        }
        let half_ticks = self.walk_half_ticks(tick);
        if self.walk_clock.last().is_some_and(|pace| pace.tick == tick) {
            self.walk_clock.pop();
        }
        if self.walk_clock.capacity() < WALK_SEGMENTS {
            self.walk_clock
                .reserve_exact(WALK_SEGMENTS - self.walk_clock.len());
        }
        self.walk_clock.push(WalkPace {
            tick,
            half_ticks,
            slow,
        });
        debug_assert!(self.walk_clock.len() <= WALK_SEGMENTS);
    }

    pub fn walk_half_ticks(&self, tick: u64) -> u128 {
        let end = self.walk_clock.partition_point(|pace| pace.tick <= tick);
        if end == 0 {
            return self.walk_clock.first().map_or(0, |pace| pace.half_ticks);
        }
        let pace = &self.walk_clock[end - 1];
        pace.half_ticks + u128::from(tick - pace.tick) * if pace.slow { 1 } else { 2 }
    }

    fn advance_walk_history(&mut self, tick: u64) {
        if tick > self.last_tick {
            self.last_tick = tick;
            self.region_rollback = None;
        }
        let cutoff = self.last_tick.saturating_sub(WALK_LOOKBACK);
        let end = self.walk_clock.partition_point(|pace| pace.tick <= cutoff);
        if end > 0 {
            let anchor = WalkPace {
                tick: cutoff,
                half_ticks: self.walk_half_ticks(cutoff),
                slow: self.walk_clock[end - 1].slow,
            };
            self.walk_clock.drain(..end - 1);
            self.walk_clock[0] = anchor;
        }
    }

    pub fn observe_tick(&mut self, tick: u64) {
        let tick = tick.max(self.last_tick);
        self.advance_walk_history(tick);
        self.mark_region(tick);
    }

    fn mark_region(&mut self, tick: u64) {
        let now = self.region();
        if now == self.seen_region {
            return;
        }
        if self
            .region_rollback
            .as_ref()
            .is_some_and(|saved| saved.tick == tick && saved.region == now)
        {
            let saved = self.region_rollback.take().unwrap();
            self.walk_clock = saved.clock;
            self.region_since = saved.since;
        } else {
            if self.region_rollback.is_none() {
                self.region_rollback = Some(RegionRollback {
                    tick,
                    region: self.seen_region,
                    since: self.region_since,
                    clock: self.walk_clock.clone(),
                });
            }
            self.region_since = tick;
            self.reset_walk_clock(tick);
        }
        self.seen_region = now;
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn region_since(&self) -> u64 {
        self.region_since
    }

    #[allow(dead_code)]
    pub fn party_since(&self) -> u64 {
        self.party_since
    }

    fn flash(&mut self, banner: &'static str, tick: u64) {
        self.banner = Some(banner);
        self.since_tick = tick;
    }

    fn settle(&mut self) {
        if !self.on_adventure {
            self.region = Region::CastleTown;
            return;
        }
        let measured = self
            .treasures
            .saturating_add(self.empty_chests)
            .saturating_add(self.measurements_observed) as usize;
        let submissions = usize::from(self.submission_latch);
        self.region = region_for(
            self.kind,
            &LoopStatus::Running,
            self.danger,
            measured,
            submissions,
            false,
        );
    }

    fn banner_ticks(text: &str) -> u64 {
        if text == "homecoming" {
            HOMECOMING_TICKS
        } else {
            BANNER_TICKS
        }
    }

    fn banner_live(&self) -> bool {
        self.banner.is_some_and(|text| {
            self.last_tick.saturating_sub(self.since_tick) < Self::banner_ticks(text)
        })
    }

    pub fn region(&self) -> Region {
        if self.region == Region::Homecoming && self.last_tick >= self.home_return_until {
            return Region::CastleTown;
        }
        self.region
    }

    pub fn danger(&self) -> Danger {
        self.danger
    }

    pub fn party(&self) -> u8 {
        self.party
    }

    pub fn treasures(&self) -> u32 {
        self.treasures
    }

    pub fn banner_is_some(&self) -> bool {
        self.banner_live()
    }

    pub fn banner(&self) -> Option<&'static str> {
        self.banner.filter(|_| self.banner_live())
    }

    pub fn iteration(&self) -> usize {
        self.iteration
    }

    pub fn empty_chests(&self) -> u32 {
        self.empty_chests
    }

    pub fn kind(&self) -> LoopKind {
        self.kind
    }

    pub fn record_fight(&self, tick: u64) -> Option<(u32, bool)> {
        let into = tick.checked_sub(self.record_at?)?;
        let lasts = if self.record_won {
            RECORD_FIGHT
        } else {
            RETREAT_FIGHT
        };
        (into < lasts).then_some((into as u32, self.record_won))
    }

    pub fn record_score(&self) -> Option<&str> {
        self.record_score.as_deref()
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn records(&self) -> u32 {
        self.records
    }

    pub fn fresh_measurement(&self, tick: u64) -> Option<u32> {
        let since = tick.checked_sub(self.measured_at?)?;
        (since < 400).then_some(since as u32)
    }

    pub fn bouts(&self) -> u32 {
        self.records + self.retreats
    }

    pub fn banner_text(&self) -> Option<String> {
        match self.banner()? {
            "homecoming" => Some(format!("homecoming · {} treasure(s)", self.treasures)),
            other => Some(other.to_string()),
        }
    }
}

fn home_region(kind: LoopKind, measured: usize) -> Region {
    match kind {
        LoopKind::Competition | LoopKind::Coding => Region::TheMines,
        LoopKind::Research => Region::DarkForest,
        LoopKind::Unknown if measured > 0 => Region::TheMines,
        LoopKind::Unknown => Region::DarkForest,
    }
}

pub fn region_for(
    kind: LoopKind,
    status: &LoopStatus,
    danger: Danger,
    measured: usize,
    submissions: usize,
    verifying: bool,
) -> Region {
    let active = matches!(
        status,
        LoopStatus::Baselining
            | LoopStatus::Running
            | LoopStatus::Verifying
            | LoopStatus::AwaitingApproval
    );
    if !active {
        return Region::CastleTown;
    }
    if *status == LoopStatus::Verifying || verifying || submissions > 0 {
        return Region::DragonKeep;
    }
    let danger = if *status == LoopStatus::AwaitingApproval {
        Danger::new(danger.level().max(2))
    } else {
        danger
    };
    if danger.level() >= 2 {
        return Region::Swamp;
    }
    home_region(kind, measured)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolLean {
    Neither,
    Competitive,
    Research,
}

#[derive(Clone, Debug)]
pub struct ToolMix {
    ring: Vec<ToolLean>,
    next: usize,
    competitive: usize,
    research: usize,
}

impl Default for ToolMix {
    fn default() -> Self {
        ToolMix {
            ring: vec![ToolLean::Neither; TOOL_MIX_WINDOW],
            next: 0,
            competitive: 0,
            research: 0,
        }
    }
}

impl ToolMix {
    pub fn push(&mut self, building: Building, _activity: RealmActivity) {
        let lean = match building {
            Building::Smithy | Building::Gatehouse => ToolLean::Competitive,
            Building::Scriptorium | Building::Observatory => ToolLean::Research,
            Building::Keep | Building::Rookery | Building::Chapel | Building::RoundTable => {
                ToolLean::Neither
            }
        };
        let evicted = self.ring[self.next];
        self.ring[self.next] = lean;
        self.next = (self.next + 1) % TOOL_MIX_WINDOW;
        self.competitive = self.competitive - usize::from(evicted == ToolLean::Competitive)
            + usize::from(lean == ToolLean::Competitive);
        self.research = self.research - usize::from(evicted == ToolLean::Research)
            + usize::from(lean == ToolLean::Research);
    }

    fn promote(&self, kind: LoopKind) -> LoopKind {
        if kind != LoopKind::Unknown {
            return kind;
        }
        if self.competitive * 2 > TOOL_MIX_WINDOW {
            LoopKind::Competition
        } else if self.research * 2 > TOOL_MIX_WINDOW {
            LoopKind::Research
        } else {
            LoopKind::Unknown
        }
    }
}

pub fn loop_kind_for(task: &str, podrace: bool, tool_mix: &ToolMix) -> LoopKind {
    if podrace {
        return LoopKind::Competition;
    }
    let text = task.to_ascii_lowercase();
    if [
        "benchmark",
        "submission",
        "leaderboard",
        "kernel",
        "optimi",
        "faster",
    ]
    .iter()
    .any(|needle| text.contains(needle))
    {
        return LoopKind::Competition;
    }
    if ["research", "survey", "read", "explore", "find out", "why"]
        .iter()
        .any(|needle| text.contains(needle))
    {
        return LoopKind::Research;
    }
    if ["fix", "implement", "refactor", "build", "test", "write"]
        .iter()
        .any(|needle| text.contains(needle))
    {
        return LoopKind::Coding;
    }
    tool_mix.promote(LoopKind::Unknown)
}

#[derive(Clone, Debug)]
pub struct LoopMirror {
    status: LoopStatus,
    iteration: usize,
    stale_level: u8,
    measured_candidates: usize,
    submissions: usize,
    tier: EscalationTier,
    party: u8,
    records: usize,
    failures: usize,
    primed: bool,
}

impl Default for LoopMirror {
    fn default() -> Self {
        LoopMirror {
            status: LoopStatus::Idle,
            iteration: 0,
            stale_level: 0,
            measured_candidates: 0,
            submissions: 0,
            tier: EscalationTier::Local,
            party: 1,
            records: 0,
            failures: 0,
            primed: false,
        }
    }
}

impl LoopMirror {
    fn is_active(status: LoopStatus) -> bool {
        matches!(
            status,
            LoopStatus::Baselining
                | LoopStatus::Running
                | LoopStatus::Verifying
                | LoopStatus::AwaitingApproval
        )
    }

    pub fn stall_level(st: &LoopSummary) -> u8 {
        if st.verifier_blocked.is_some() || st.status == LoopStatus::AwaitingApproval {
            return 2;
        }
        if !st.podrace {
            return Danger::from_stall(st.stale_count, st.pivot).level();
        }
        if st.last_log_all_errors {
            return 2;
        }
        Danger::from_stall(st.idle_log_tail, st.pivot).level()
    }

    pub fn drain(
        &mut self,
        st: &LoopSummary,
        tool_mix: &ToolMix,
        party: u8,
    ) -> Vec<AdventureEvent> {
        let mut events = Vec::new();
        let active = Self::is_active(st.status);
        let was_active = Self::is_active(self.status);

        if active && !was_active {
            events.push(AdventureEvent::LoopStarted {
                kind: loop_kind_for(&st.task, st.podrace, tool_mix),
                task: st.task.chars().take(240).collect(),
            });
        }
        if st.iteration > self.iteration {
            events.push(AdventureEvent::Iteration { n: st.iteration });
        }
        let level = Self::stall_level(st);
        if active && level != self.stale_level {
            events.push(AdventureEvent::Stall { level });
        }
        if st.tier != self.tier {
            events.push(AdventureEvent::Pivot);
        }
        if st.measured_candidates > self.measured_candidates {
            events.push(AdventureEvent::MeasurementObserved);
        }
        let entered_verifying =
            st.status == LoopStatus::Verifying && self.status != LoopStatus::Verifying;
        if st.submissions > self.submissions || entered_verifying {
            events.push(AdventureEvent::Submitted);
        }
        if self.status == LoopStatus::Verifying && st.status != LoopStatus::Verifying {
            events.push(AdventureEvent::SubmissionSettled {
                accepted: st.status == LoopStatus::Done,
            });
        }
        let finished_now = matches!(
            st.status,
            LoopStatus::Done | LoopStatus::Stopped | LoopStatus::Failed
        ) && !matches!(
            self.status,
            LoopStatus::Done | LoopStatus::Stopped | LoopStatus::Failed
        );
        if finished_now {
            events.push(AdventureEvent::LoopFinished {
                ok: st.status == LoopStatus::Done,
            });
        }
        if party != self.party {
            events.push(AdventureEvent::Party { size: party });
        }
        if self.primed && st.promotions > self.records {
            events.push(AdventureEvent::RecordPromoted {
                score: st.last_promotion_score.clone(),
            });
        } else if self.primed && st.promotion_failures > self.failures {
            events.push(AdventureEvent::PromotionFailed {
                score: st.last_promotion_failure_score.clone(),
            });
        }
        self.records = st.promotions;
        self.failures = st.promotion_failures;
        self.primed = true;

        self.status = st.status;
        self.iteration = st.iteration;
        self.stale_level = level;
        self.measured_candidates = st.measured_candidates;
        self.submissions = st.submissions;
        self.tier = st.tier;
        self.party = party;
        events
    }
}

#[cfg(test)]
#[path = "../../tests/world_viz/adventure__tests.rs"]
mod tests;
