//! Bounded authored state machines. Preparation owns pixels; ticks inspect facts
//! and install a legacy appearance only when a state or provider changes.
use super::together_appearance::Appearance;
use angel_delve::{realm::appearance::Step, rules::Run};
use serde::Deserialize;
use std::collections::BTreeMap;

pub const CONTRACT: &str = "character-animation-graph-pack/1";
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Specification {
    initial: String,
    states: BTreeMap<String, State>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    steps: Vec<Step>,
    transitions: Vec<Transition>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Transition {
    to: String,
    when: BTreeMap<Fact, bool>,
    phase: Phase,
}
#[derive(Clone, Copy, Debug, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum Fact {
    Moving,
    Attacking,
    Shielding,
    Hurt,
    Rolling,
    Dead,
    Away,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Phase {
    Preserve,
    Restart,
}
#[derive(Clone, Debug)]
pub struct Graph {
    initial: String,
    states: BTreeMap<String, (Appearance, Vec<Transition>)>,
}
impl Specification {
    pub fn validate(&self) -> Result<(), String> {
        if self.states.is_empty()
            || self.states.len() > 8
            || !self.states.contains_key(&self.initial)
            || self
                .states
                .keys()
                .any(|id| !super::together_abilities::id_valid(id))
        {
            return Err("graph needs 1..8 named states and an existing initial state".into());
        }
        let mut edges = 0;
        for (id, state) in &self.states {
            angel_delve::realm::appearance::validate_steps(&state.steps, 8)?;
            edges += state.transitions.len();
            for edge in &state.transitions {
                if edge.to == *id || !self.states.contains_key(&edge.to) || edge.when.is_empty() {
                    return Err(
                        "graph transition needs another existing state and nonempty facts".into(),
                    );
                }
            }
        }
        if edges > 32 {
            return Err("graph exceeds 32 transitions".into());
        }
        Ok(())
    }
    pub fn prepare(&self, art: &Appearance) -> Result<Graph, String> {
        self.validate()?;
        let identity =
            angel_live::host::sha256_hex(format!("{}:{self:?}", art.fingerprint).as_bytes());
        let mut states = BTreeMap::new();
        for (id, state) in &self.states {
            let mut appearance = art.clone().with_clip(&state.steps)?;
            appearance.fingerprint = format!("{identity}:{id}");
            appearance.avatar.as_mut().unwrap().source_id = appearance.fingerprint.clone();
            states.insert(id.clone(), (appearance, state.transitions.clone()));
        }
        Ok(Graph {
            initial: self.initial.clone(),
            states,
        })
    }
}
pub const MAX_INTENT: u64 = 9_007_199_254_740_991;
#[derive(Debug)]
pub struct Intent {
    revision: u64,
    raid: u64,
    epoch: u64,
    present: bool,
    pub active: bool,
}
impl Intent {
    pub fn new(revision: u64, run: &Run, player: u32) -> Self {
        Self {
            revision,
            raid: run.raid_id,
            epoch: run.seat_epoch(player),
            present: run.players.contains_key(&player),
            active: run.players.contains_key(&player),
        }
    }
    pub fn owns_identity(&self, run: &Run, player: u32) -> bool {
        self.present
            && self.raid == run.raid_id
            && self.epoch == run.seat_epoch(player)
            && run.players.contains_key(&player)
    }
    pub fn admit(&mut self, revision: u64, run: &Run, player: u32) -> Result<bool, String> {
        if revision > MAX_INTENT {
            return Err("graph intent exceeds safe-integer bound".into());
        }
        if !self.owns_identity(run, player) {
            return Err(
                "seat owner changed; graph selection needs a leased current-seat context".into(),
            );
        }
        if revision < self.revision {
            return Err("stale graph selection intent".into());
        }
        let explicit = revision > self.revision;
        if explicit {
            self.revision = revision;
            self.active = true;
        }
        Ok(explicit)
    }
    pub fn retire(&mut self) {
        self.active = false;
    }
}
#[derive(Clone, Copy, Debug)]
struct Observation {
    tick: u64,
    room: usize,
    floor: (u32, u32),
    position: (f32, f32),
}
#[derive(Debug)]
pub struct Cursor {
    state: String,
    raid: u64,
    epoch: u64,
    installed: Option<String>,
    last_tick: Option<u64>,
    retired: bool,
    before: Option<Observation>,
}
impl Cursor {
    fn owns(&self, run: &Run, player: u32) -> bool {
        !self.retired
            && self.raid == run.raid_id
            && self.epoch == run.seat_epoch(player)
            && run.players.contains_key(&player)
    }
    fn matches_avatar(&self, run: &Run, player: u32) -> bool {
        run.players
            .get(&player)
            .and_then(|hero| hero.avatar.as_ref())
            .is_some_and(|avatar| Some(avatar.source_id.as_str()) == self.installed.as_deref())
    }
    /// Capture only an owned step boundary; no Run or frame data is cloned.
    pub fn before_step(&mut self, run: &Run, player: u32) {
        self.before = if self.owns(run, player) && self.matches_avatar(run, player) {
            let hero = &run.players[&player];
            Some(Observation {
                tick: run.tick,
                room: run.at,
                floor: (run.dungeon.depth, run.dungeon.world_revision),
                position: (hero.x, hero.y),
            })
        } else {
            None
        };
    }
    pub fn current_owner(&self, run: &Run, player: u32) -> bool {
        self.owns(run, player) && self.matches_avatar(run, player)
    }
    pub fn is_retired(&self) -> bool {
        self.retired
    }
    pub fn retire_explicitly(&mut self) {
        self.retired = true;
        // The explicit avatar API transferred intent even if bytes are identical.
        self.installed = None;
        self.before = None;
    }
    pub fn clear(&self, run: &mut Run, player: u32) {
        // Matching content is insufficient authority to mutate a replacement seat.
        if self.raid != run.raid_id || self.epoch != run.seat_epoch(player) {
            return;
        }
        let Some(installed) = &self.installed else {
            return;
        };
        if let Some(hero) = run.players.get_mut(&player) {
            if hero
                .avatar
                .as_ref()
                .is_some_and(|a| &a.source_id == installed)
            {
                hero.avatar = None;
            }
        }
    }
}
impl Graph {
    // Called only at activation. An existing cursor never silently adopts a new seat.
    pub fn activate(
        &self,
        mut previous: Option<Cursor>,
        run: &mut Run,
        player: u32,
        explicit: bool,
    ) -> Cursor {
        if let Some(old) = previous.as_mut() {
            let same_owner = old.raid == run.raid_id
                && old.epoch == run.seat_epoch(player)
                && run.players.contains_key(&player);
            if !same_owner
                || (!explicit && (!old.owns(run, player) || !old.matches_avatar(run, player)))
            {
                old.retired = true;
                old.clear(run, player);
                return previous.unwrap();
            }
        }
        let state = previous
            .as_ref()
            .filter(|old| old.owns(run, player) || explicit)
            .map(|old| &old.state)
            .filter(|state| self.states.contains_key(*state))
            .cloned()
            .unwrap_or_else(|| self.initial.clone());
        let mut cursor = Cursor {
            state,
            raid: run.raid_id,
            epoch: run.seat_epoch(player),
            installed: None,
            last_tick: previous
                .as_ref()
                .and_then(|old| old.last_tick)
                .or(Some(run.tick)),
            before: previous.as_ref().and_then(|old| old.before),
            retired: false,
        };
        self.install(&mut cursor, run, player, Phase::Preserve);
        cursor
    }
    fn install(&self, cursor: &mut Cursor, run: &mut Run, player: u32, phase: Phase) {
        let appearance = &self.states[&cursor.state].0;
        if !cursor.owns(run, player) {
            return;
        }
        appearance.install(run);
        if matches!(phase, Phase::Restart) {
            if let Some(animation) = run
                .players
                .get_mut(&player)
                .and_then(|h| h.avatar.as_mut())
                .and_then(|a| a.animation.as_mut())
            {
                animation.phase_offset =
                    ((u64::from(animation.period()) - run.tick % u64::from(animation.period()))
                        % u64::from(animation.period())) as u32;
            }
        }
        cursor.installed = Some(appearance.fingerprint.clone());
    }
    pub fn advance(&self, cursor: &mut Cursor, run: &mut Run, player: u32) {
        if !cursor.owns(run, player) || !cursor.matches_avatar(run, player) {
            cursor.retired = true;
            cursor.before = None;
            cursor.clear(run, player);
            return;
        }
        let before = cursor.before.take();
        if cursor.last_tick == Some(run.tick) {
            return;
        }
        cursor.last_tick = Some(run.tick);
        let hero = &run.players[&player];
        // Footstep phase wraps to zero while walking; it is not a movement fact.
        // Match the simulation's teleport exclusion, but retain real slow motion.
        let moving = before.is_some_and(|before| {
            let distance = (hero.x - before.position.0).hypot(hero.y - before.position.1);
            before.tick.checked_add(1) == Some(run.tick)
                && before.room == run.at
                && before.floor == (run.dungeon.depth, run.dungeon.world_revision)
                && distance.is_finite()
                && distance > 0.0
                && distance < 2.5
        });
        // Facts come from authoritative simulation, never client presentation state.
        let fact = |fact: Fact| match fact {
            Fact::Moving => moving,
            Fact::Attacking => hero.swing > 0 || hero.fire_cooldown > 0,
            Fact::Shielding => hero.shielding,
            Fact::Hurt => hero.struck > 0,
            Fact::Rolling => hero.rolling().is_some(),
            Fact::Dead => hero.hp == 0,
            Fact::Away => hero.away || hero.stone,
        };
        if let Some(edge) = self.states[&cursor.state]
            .1
            .iter()
            .find(|edge| edge.when.iter().all(|(key, value)| fact(*key) == *value))
        {
            cursor.state = edge.to.clone();
            self.install(cursor, run, player, edge.phase);
        }
    }
}
