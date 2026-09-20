//! The adventure model (Zelda overworld Z1): pure state + events.
//!
//! The self-improvement loop becomes a quest. `LoopState` is diffed against a
//! [`LoopMirror`] once per frame and folded into [`AdventureEvent`]s; those
//! events drive the [`Quest`] state machine (region, danger, loot, banners,
//! party). Everything here is deterministic in `(state, event, tick)` — no
//! clocks, no maps, no randomness — so Z2's renderers can key frames on it.
//!
//! Nothing in this module draws anything. It is the single authority for
//! "where is the hero and how much trouble are they in".

use super::{Building, RealmActivity};
use crate::drive::loop_ctl::{EscalationTier, LoopState, LoopStatus};

/// How many ticks a measurement banner ("treasure!" / "empty chest") and the
/// submission banners stay up.
const BANNER_TICKS: u64 = 40;
/// `LoopFinished{ok:true}` walks the loot home for this long, then the quest
/// idles back in town.
const HOMECOMING_TICKS: u64 = 120;
/// How many recent tool classifications feed `loop_kind_for`'s tie-break.
const TOOL_MIX_WINDOW: usize = 12;
/// The room camera's exact historical horizon. Keep an anchor plus at most
/// one pace change per tick; no session-length allocation survives reset.
pub(super) const WALK_LOOKBACK: u64 = 12;
const WALK_SEGMENTS: usize = WALK_LOOKBACK as usize + 1;

/// The places an adventure goes. Assigned by loop kind and harness state —
/// never by the model's prose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Region {
    /// Idle / ordinary turns; the knight walks the town landmarks.
    CastleTown,
    /// Competition, podrace, and coding loops: the measured-candidates path.
    TheMines,
    /// Research and exploration loops; unknown territory.
    DarkForest,
    /// Stalled loops, blocked verifiers, awaiting approval.
    Swamp,
    /// A submission in flight or an acceptance gate judging the work.
    DragonKeep,
    /// A finished loop walking the loot home.
    Homecoming,
}

impl Region {
    /// The place's name as the HUD, the pane title and `/world quest` say it.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Region::CastleTown => "Castle Town",
            Region::TheMines => "The Mines",
            Region::DarkForest => "Dark Forest",
            Region::Swamp => "The Swamp",
            Region::DragonKeep => "Dragon Keep",
            Region::Homecoming => "Homecoming",
        }
    }

    /// One-cell sigil that opens the quest line. Every glyph is display
    /// width 1 so the HUD's cell budget is the character count.
    ///
    /// Castle Town has no sigil of its own in the Z4 spec — the chess king
    /// is chosen to sit beside the keep's rook, and the town line is only
    /// ever raised by a live banner anyway.
    pub(crate) fn glyph(self) -> char {
        match self {
            Region::CastleTown => '\u{2654}',
            Region::TheMines => '\u{2694}',
            Region::DarkForest => '\u{2663}',
            Region::Swamp => '\u{2248}',
            Region::DragonKeep => '\u{265C}',
            Region::Homecoming => '\u{2302}',
        }
    }
}

/// Stall depth, 0..=3 — fog density, monster count, hero pace for Z2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Danger(u8);

impl Danger {
    pub(crate) fn calm() -> Self {
        Danger(0)
    }

    /// Clamp into the legal 0..=3 band.
    fn new(level: u8) -> Self {
        Danger(level.min(3))
    }

    pub(crate) fn level(self) -> u8 {
        self.0
    }

    /// `stale_count * 3 / pivot.max(1)`, clamped to 0..=3.
    pub(crate) fn from_stall(stale: usize, pivot: usize) -> Self {
        Danger::new((stale * 3 / pivot.max(1)).min(3) as u8)
    }
}

/// What kind of adventure a loop is. Fixed at `LoopStarted` from the task
/// text, podrace mode, and the recent tool mix.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum LoopKind {
    Competition,
    Research,
    Coding,
    #[default]
    Unknown,
}

impl LoopKind {
    /// How `/world quest` names the adventure.
    pub(crate) fn label(self) -> &'static str {
        match self {
            LoopKind::Competition => "competition",
            LoopKind::Research => "research",
            LoopKind::Coding => "coding",
            LoopKind::Unknown => "unclassified",
        }
    }
}

/// Harness facts folded into the quest, in the order the mirror emits them.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AdventureEvent {
    LoopStarted {
        kind: LoopKind,
        task: String,
    },
    Iteration {
        n: usize,
    },
    /// Rising or falling; carries the new level.
    Stall {
        level: u8,
    },
    /// The loop escalated its approach (tier change) — a fresh direction.
    Pivot,
    /// A measurement receipt was observed; objective comparison is unavailable.
    MeasurementObserved,
    /// A trusted comparable measurement supplied an improvement verdict.
    // Trusted comparison fixtures only; the live mirror emits MeasurementObserved.
    #[cfg(test)]
    Measured {
        improved: bool,
    },
    Submitted,
    /// The judge returned on in-flight work (acceptance gate verdict).
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
    /// Live formation size (1 = solo hero).
    Party {
        size: u8,
    },
}

/// The quest: everything the renderer needs about the current adventure.
///
/// Session state only — deliberately not persisted in world-rewards. Loot
/// accumulates across the session's loops; banners and the homecoming walk
/// expire on the world tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Quest {
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
    /// True from `LoopStarted` to `LoopFinished`.
    on_adventure: bool,
    /// A submission is in flight / the judge holds the work: stay in the
    /// Dragon Keep until an `Iteration` resumes or the loop finishes.
    submission_latch: bool,
    /// Last world tick the quest observed (event or `observe_tick`).
    last_tick: u64,
    /// The region [`Quest::region`] last reported, and the tick it changed.
    /// Z2's renderer fades the screen over the change, so the change needs a
    /// clock of its own — `since_tick` belongs to the banner.
    seen_region: Region,
    region_since: u64,
    /// Tick the party size last changed — the followers' sparkle clock.
    party_since: u64,
    /// Cumulative anchor and bounded recent pace changes, in half-speed ticks.
    walk_clock: Vec<WalkPace>,
    /// Region folds within one world tick are a batch. Retain just the original
    /// epoch so an unpainted round trip can restore it (not a per-region cache).
    region_rollback: Option<RegionRollback>,
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
    // Keep fresh quest values independent on the pinned rustc 1.95: in release
    // builds, inlining this constructor lets MIR reuse a by-value argument that
    // the first scripted run mutates. The existing determinism test reproduces
    // treasures=1/2 without this guard (including in a standalone executable).
    #[inline(never)]
    pub(crate) fn idle() -> Self {
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
        }
    }

    /// Fold one harness fact in. Deterministic in `(state, event, tick)`.
    pub(crate) fn apply(&mut self, ev: AdventureEvent, tick: u64) {
        // Motion is prospective, but event/banner/homecoming timestamps keep
        // the harness fact's original tick (including late facts).
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
                // The judge released the work; the party rides back to the
                // adventure's own grounds.
                self.submission_latch = false;
                self.settle();
            }
            AdventureEvent::Stall { level } => {
                self.danger = Danger::new(level);
                self.settle();
            }
            AdventureEvent::Pivot => {
                // A structural pivot is fresh ground: the fog lifts.
                self.danger = Danger::calm();
                self.settle();
            }
            AdventureEvent::MeasurementObserved => {
                self.measurements_observed = self.measurements_observed.saturating_add(1);
                self.flash("measurement recorded", tick);
                self.settle();
            }
            #[cfg(test)]
            AdventureEvent::Measured { improved } => {
                // Reward-never-punish: a regression is an empty chest, never
                // danger or a lost treasure.
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
                    // The banner stays raised over the keep until an
                    // iteration resumes or the loop finishes.
                    self.submission_latch = true;
                    self.flash("banner raised", tick);
                } else {
                    // Back to the home grounds; no penalty.
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
                // Tool traffic walks the town landmarks; inside an adventure
                // the region stands. The rolling mix lives on `World`.
            }
            AdventureEvent::TurnStarted | AdventureEvent::TurnEnded { .. } => {
                // Ordinary turns are Castle Town life; loops own the region.
            }
            AdventureEvent::Party { size } => {
                let size = size.clamp(1, 8);
                if size != self.party {
                    self.party_since = tick;
                }
                self.party = size;
            }
        }
        if reset_walk {
            // Explicit loop/iteration boundaries must never resurrect an old
            // epoch, even when a later fold returns to its region this tick.
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
        // Late facts affect motion prospectively without changing the existing
        // event/banner timestamps or historical positions.
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

    /// Accumulated progress independent of map pace: two units per calm tick,
    /// one per stalled tick. Integer arithmetic preserves fractional tiles and
    /// is safe even at u64::MAX; renderers supply their authored ticks per tile.
    pub(crate) fn walk_half_ticks(&self, tick: u64) -> u128 {
        let end = self.walk_clock.partition_point(|pace| pace.tick <= tick);
        if end == 0 {
            // Older than the retained horizon: clamp to the cumulative anchor.
            // Exact replay is only promised for last_tick-12..=last_tick (and
            // prospective lookups); before a fresh epoch its anchor is zero.
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

    /// The world clock advanced without an event (banner / homecoming expiry).
    pub(crate) fn observe_tick(&mut self, tick: u64) {
        let tick = tick.max(self.last_tick);
        self.advance_walk_history(tick);
        self.mark_region(tick);
    }

    /// Genuine region entry starts a new walk. Same-tick folds are one visual
    /// batch: returning to the batch's original region restores its progress
    /// and fade clock. A different tick or explicit iteration commits the new
    /// epoch; there is deliberately no cross-tick per-region journey cache.
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

    /// Ticks elapsed since the visible region last changed — the renderer's
    /// fade phase.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn region_since(&self) -> u64 {
        self.region_since
    }

    /// Tick the party size last changed — followers sparkle in and out.
    #[allow(dead_code)]
    pub(crate) fn party_since(&self) -> u64 {
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

    pub(crate) fn region(&self) -> Region {
        if self.region == Region::Homecoming && self.last_tick >= self.home_return_until {
            return Region::CastleTown;
        }
        self.region
    }

    pub(crate) fn danger(&self) -> Danger {
        self.danger
    }

    pub(crate) fn party(&self) -> u8 {
        self.party
    }

    pub(crate) fn treasures(&self) -> u32 {
        self.treasures
    }

    pub(crate) fn banner_is_some(&self) -> bool {
        self.banner_live()
    }

    /// The live banner, or `None` once its ticks are spent.
    pub(crate) fn banner(&self) -> Option<&'static str> {
        self.banner.filter(|_| self.banner_live())
    }

    /// The loop's iteration count — the walk's clock. Z2/Z3 turn it into the
    /// waypoint the hero stands on (`world3d::region::waypoint`).
    pub(crate) fn iteration(&self) -> usize {
        self.iteration
    }

    pub(crate) fn empty_chests(&self) -> u32 {
        self.empty_chests
    }

    pub(crate) fn kind(&self) -> LoopKind {
        self.kind
    }

    /// HUD-ready banner text: the homecoming banner carries the loot count.
    pub(crate) fn banner_text(&self) -> Option<String> {
        match self.banner()? {
            "homecoming" => Some(format!("homecoming · {} treasure(s)", self.treasures)),
            other => Some(other.to_string()),
        }
    }
}

/// Where the loop's home grounds are, by kind.
fn home_region(kind: LoopKind, measured: usize) -> Region {
    match kind {
        LoopKind::Competition | LoopKind::Coding => Region::TheMines,
        LoopKind::Research => Region::DarkForest,
        // An unclassified loop that is measuring candidates is mining ore,
        // not surveying woods (design table: TheMines = measured-candidates
        // path).
        LoopKind::Unknown if measured > 0 => Region::TheMines,
        LoopKind::Unknown => Region::DarkForest,
    }
}

/// The region snapshot for a loop state. Pure; the `Quest` state machine
/// applies the same rules through its events.
///
/// Precedence: no loop → town; a submission the judge holds → Dragon Keep
/// (only an iteration or finishing dislodges it); danger ≥ 2 → Swamp; else
/// the kind's home grounds. `AwaitingApproval` forces danger ≥ 2.
pub(crate) fn region_for(
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

/// Which lean a tool call gives the loop-kind classifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolLean {
    Neither,
    /// Shell / benchmark / submission work: the forge and the gate.
    Competitive,
    /// Read / search / research work: the scriptorium and the observatory.
    Research,
}

/// Rolling classification of the last [`TOOL_MIX_WINDOW`] tool calls. Breaks
/// `Unknown` ties toward the work the agent actually does; it can never
/// demote a kind the task text already named.
#[derive(Clone, Debug)]
pub(crate) struct ToolMix {
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
    pub(crate) fn push(&mut self, building: Building, _activity: RealmActivity) {
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

    /// Promote an `Unknown` kind by strict majority of the filled window.
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

/// Classify a loop by its task text, podrace mode, and recent tool mix.
pub(crate) fn loop_kind_for(task: &str, podrace: bool, tool_mix: &ToolMix) -> LoopKind {
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

/// Last-seen loop facts kept on `App`; diffed once per frame against
/// `loop_ctl` to emit [`AdventureEvent`]s. Runs whether or not the world pane
/// is visible — the quest is session state, like the village pulses.
#[derive(Clone, Debug)]
pub(crate) struct LoopMirror {
    status: LoopStatus,
    iteration: usize,
    stale_level: u8,
    measured_candidates: usize,
    submissions: usize,
    tier: EscalationTier,
    party: u8,
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

    pub(crate) fn stall_level(st: &LoopState) -> u8 {
        if st.verifier_blocked.is_some() || st.status == LoopStatus::AwaitingApproval {
            return 2;
        }
        if !st.podrace {
            return Danger::from_stall(st.stale_count, st.pivot).level();
        }
        // Podrace stale_count is lack of authoritative objective comparison,
        // not proof that the fast/deep workers are idle. Render observed
        // inactivity or execution failure; never punish missing metric authority.
        if st
            .log
            .last()
            .is_some_and(|entry| entry.tool_errors > 0 && entry.tool_errors >= entry.tool_calls)
        {
            return 2;
        }
        let inactive = st
            .log
            .iter()
            .rev()
            .take_while(|entry| {
                entry.tool_calls <= entry.tool_errors
                    && !entry.workspace_changed
                    && entry.new_findings == 0
                    && entry.outcome_progress == 0
                    && entry.novel_outcome_actions == 0
                    && entry.verified_outcome_actions == 0
            })
            .count();
        Danger::from_stall(inactive, st.pivot).level()
    }

    /// Diff and drain. Event order: LoopStarted, Iteration, Stall, Pivot,
    /// Measured, Submitted, SubmissionSettled, LoopFinished, Party.
    pub(crate) fn drain(
        &mut self,
        st: &LoopState,
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
            // Escalation is a structural pivot: more of the engine on the
            // problem, a fresh direction.
            events.push(AdventureEvent::Pivot);
        }
        if st.measured_candidates > self.measured_candidates {
            events.push(AdventureEvent::MeasurementObserved);
        }
        let entered_verifying =
            st.status == LoopStatus::Verifying && self.status != LoopStatus::Verifying;
        if st.submissions > self.submissions || entered_verifying {
            // Entering `Verifying` puts the work in front of the judge — the
            // same in-flight state a submitted competition candidate has.
            events.push(AdventureEvent::Submitted);
        }
        if self.status == LoopStatus::Verifying && st.status != LoopStatus::Verifying {
            // The acceptance gate returned: `Done` is the accepted verdict,
            // any return to the arm/stop ladder is a rejection (no penalty).
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
#[path = "../../../../tests/cockpit/world_viz/adventure__tests.rs"]
mod tests;
