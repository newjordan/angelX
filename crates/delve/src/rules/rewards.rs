//! The moments a knight gets stronger, and the road ahead (the operator,
//! 2026-10-28: "ensure highest quality experience and player retention").
//!
//! Progress in the Delve comes in steps: a level reached, a wish's ladder
//! climbed a tier, the knight drawn a rung finer on the resolution ladder, a
//! floor deeper than the realm has ever stood on, a relic taken off the
//! floor. Each is a `Step`, kept on the run for the seconds its moment plays
//! (`Kind::life`) and then let go, so the host, every friend's mirror and a
//! browser guest draw the same moment from the same run
//! (`render::overworld::arena::rewards`). A knight's steps play one after
//! another, never on top of each other, and those that come with a new
//! floor wait for its title plate to clear.
//!
//! The run keeps what each knight earned this delve (`Earned`): experience,
//! levels, wishes and the highest tier, relics, gold, the rung. At the run's
//! end the results board turns to the road ahead: what each knight earned,
//! and what is next for them (`ahead`: the next level, rung and tier) and for
//! the party (`road`: the next village of the Deep, the sky).
//!
//! And the realm remembers. Every wish granted lights a star in its sky
//! (`Sky`, on `Home::sky`), and the stars fill constellations one after
//! another. A constellation once whole is lit for good, named for the
//! knight whose wish finished it, and gives the next delve a wish more for
//! every knight (`GIFT`). Nothing is ever lost for staying away and nothing
//! runs on the clock: the sky only grows, and the next constellation is
//! always a few wishes off (the knob: `ANGEL_DELVE_SKY=off`).

use super::*;
use std::sync::OnceLock;

/// The most steps the run keeps at once.
pub const MOST_STEPS: usize = 12;
/// The most constellations the realm keeps a name for.
pub const MOST_LIT: usize = 64;
/// Wishes each knight takes into the next delve from a lit constellation,
/// and the most the sky holds for it: a constellation lit while one is
/// already waiting adds nothing more.
pub const GIFT: u8 = 1;
/// The constellations, in the order the stars fill them. After the last the
/// names come round again, numbered (`constellation`).
pub const CONSTELLATIONS: [&str; 8] =
    ["THE LAMP", "THE CROWN", "THE GRAIL", "THE DRAGON", "THE FLAMINGO", "THE GNOME", "THE KEY", "THE COMET"];
/// The sounds the steps land with (`Kind::sound`): the host's mixer and the
/// page play each by name.
pub const SOUNDS: [&str; 6] = ["level_up", "rung_up", "relic_find", "deepest_yet", "sky_lit", "wish_grant"];
/// Ticks a second step of a knight's waits after the one before it, and
/// after the party's own banner, so they never play on top of each other.
const AFTER_BANNER: u64 = 24;

/// A step up, as its moment shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// The knight reached level `level` (`talents::LEVELS`): a lesson waits
    /// with Sir Ector.
    Level { level: u32 },
    /// The knight's wish `wish` climbed from tier `from` to tier `tier`
    /// (from 0): two at once for a super wish.
    Tier { wish: String, from: u8, tier: u8 },
    /// The knight's figure climbed the resolution ladder to `px` pixels.
    Rung { px: u32 },
    /// The party stands deeper than the realm ever has: floor `floor`.
    Record { floor: u32 },
    /// The knight took a relic off the floor.
    Relic { name: String },
    /// Constellation `index` of the realm's sky is whole: lit by `by`.
    Lit { index: u32, by: String },
    /// The sky's gift: every knight starts the delve with `wishes` more.
    Gift { wishes: u8 },
}

impl Kind {
    /// Ticks its moment plays.
    pub fn life(&self) -> u64 {
        match self {
            Kind::Level { .. } => 90,
            Kind::Tier { .. } => 60,
            Kind::Rung { .. } => 105,
            Kind::Record { .. } => 120,
            Kind::Relic { .. } => 75,
            Kind::Lit { .. } => 150,
            Kind::Gift { .. } => 105,
        }
    }

    /// Ticks before the knight's next step may begin: its moment's front,
    /// before it settles to read.
    fn front(&self) -> u64 {
        match self {
            Kind::Level { .. } => 54,
            Kind::Tier { .. } => 32,
            Kind::Rung { .. } => 66,
            Kind::Relic { .. } => 40,
            Kind::Record { .. } | Kind::Lit { .. } | Kind::Gift { .. } => AFTER_BANNER,
        }
    }

    /// The sound it lands with, when it starts: a tier has the wish's own
    /// landing (`overclass::land_sound`).
    pub fn sound(&self) -> Option<&'static str> {
        match self {
            Kind::Level { .. } => Some("level_up"),
            Kind::Tier { .. } => None,
            Kind::Rung { .. } => Some("rung_up"),
            Kind::Record { .. } => Some("deepest_yet"),
            Kind::Relic { .. } => Some("relic_find"),
            Kind::Lit { .. } => Some("sky_lit"),
            Kind::Gift { .. } => Some("wish_grant"),
        }
    }

    /// Whether it is the party's, shown across the view (else it plays
    /// over its knight).
    pub fn banner(&self) -> bool {
        matches!(self, Kind::Record { .. } | Kind::Lit { .. } | Kind::Gift { .. })
    }
}

/// A step up: when its moment starts, whose it is (0: the party's) and
/// what it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub at: u64,
    pub knight: u32,
    pub kind: Kind,
}

impl Step {
    /// Ticks into its moment at `tick`; none before it starts or once it
    /// has played.
    pub fn age(&self, tick: u64) -> Option<u64> {
        tick.checked_sub(self.at).filter(|&a| a < self.kind.life())
    }
}

/// What a knight earned this delve.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Earned {
    pub xp: u32,
    /// The level they came down at, and the level they stand at now.
    pub from_level: u32,
    pub level: u32,
    /// Wishes granted them, and the highest tier one reached: the wish's
    /// name and its tier (from 0).
    pub wishes: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best: Option<(String, u8)>,
    pub relics: u32,
    pub gold: u32,
    /// The finest rung they were drawn at, in pixels.
    pub rung: u32,
}

/// The moments playing (`Run::rewards`), sent to every seat.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rewards {
    /// Steps up playing, or about to, oldest first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Step>,
    /// What the watcher saw last tick: the server's own, never saved nor
    /// sent (a save picks up where it stands, cheering nothing).
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub seen: Seen,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// What each knight earned this delve (`Run::ledger`): saved with the run,
/// the server's own while it is played (`wire::PRIVATE`: it plays nothing on
/// a seat), and handed to every seat once, with the run's last results
/// (`floor_stats::Results::ledger`), for the road ahead.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ledger {
    /// By knight.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub earned: BTreeMap<u32, Earned>,
    /// The deepest floor this delve stood on, and whether it was deeper
    /// than the realm had ever been.
    #[serde(skip_serializing_if = "is_zero")]
    pub deepest: u32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub record: bool,
}

impl Ledger {
    pub fn quiet(&self) -> bool {
        *self == Ledger::default()
    }

    pub fn valid(&self) -> bool {
        self.earned.len() <= MAX_PLAYERS as usize
            && self.earned.keys().all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.earned.values().all(|e| e.best.as_ref().is_none_or(|(w, t)| w.len() <= 96 && usize::from(*t) < phrasebook::MAX_TIERS))
    }

    /// A wish granted knight `id` at tier `tier`, counted.
    fn granted(&mut self, id: u32, wish: &str, tier: u8) {
        let earned = self.earned.entry(id).or_default();
        earned.wishes = earned.wishes.saturating_add(1);
        if earned.best.as_ref().is_none_or(|(_, best)| tier > *best) {
            earned.best = Some((wish.chars().take(48).collect(), tier));
        }
    }
}

/// The watcher's memory (`Rewards::seen`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Seen {
    /// The floor the party stood on (none before the first look).
    pub floor: Option<u32>,
    /// The deepest floor the realm had stood on before this one.
    pub best: u32,
    /// Each knight as last seen: whose knight, their experience, level,
    /// rung and purse.
    pub knights: BTreeMap<u32, Watched>,
    /// Steps that came with a new floor, waiting for its plate to clear.
    pub held: Vec<(u32, Kind)>,
    /// The sky's gift waits for the delve's first floor a knight may wish on.
    pub gift_due: bool,
    /// The steps whose sound has played (when, whose).
    pub sounded: Vec<(u64, u32)>,
}

/// A knight as the watcher last saw them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Watched {
    pub key: String,
    pub xp: u32,
    pub level: u32,
    pub rung: u32,
    pub gold: u32,
}

impl Rewards {
    pub fn quiet(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn valid(&self) -> bool {
        let text = |s: &str| s.len() <= 96;
        self.steps.len() <= MOST_STEPS
            && self.steps.iter().all(|s| match &s.kind {
                Kind::Tier { wish, from, tier } => text(wish) && from <= tier && usize::from(*tier) < phrasebook::MAX_TIERS,
                Kind::Relic { name } => text(name),
                Kind::Lit { by, index } => text(by) && *index < 1 << 16,
                _ => true,
            })
    }

    /// The steps playing at `tick`, oldest first.
    pub fn playing(&self, tick: u64) -> impl Iterator<Item = (&Step, u64)> {
        self.steps.iter().filter_map(move |s| s.age(tick).map(|age| (s, age)))
    }

    /// Put a step up at `now`: after the knight's last one has passed its
    /// front, and after the party's banner for a knight's own.
    fn push(&mut self, now: u64, knight: u32, kind: Kind) {
        let after = self
            .steps
            .iter()
            .filter(|s| s.knight == knight || (s.kind.banner() && !kind.banner()))
            .map(|s| s.at + s.kind.front())
            .max();
        let at = after.map_or(now, |a| a.max(now));
        if self.steps.len() >= MOST_STEPS {
            self.steps.remove(0);
        }
        self.steps.push(Step { at, knight, kind });
    }

    /// A wish granted knight `id` at tier `tier` (from 0): a tier above
    /// the first is a step. A super wish's two grants in one tick are one
    /// step, from the first tier to the last.
    fn granted(&mut self, now: u64, id: u32, wish: &str, tier: u8) {
        if tier == 0 {
            return;
        }
        let waiting = self
            .steps
            .iter_mut()
            .rev()
            .find(|s| s.knight == id && s.at >= now && matches!(&s.kind, Kind::Tier { wish: w, .. } if w == wish));
        if let Some(Step { kind: Kind::Tier { tier: t, .. }, .. }) = waiting {
            *t = (*t).max(tier);
            return;
        }
        self.push(now, id, Kind::Tier { wish: wish.chars().take(48).collect(), from: tier - 1, tier });
    }

}

/// Knight `id` took relic `name` off the floor at `now` (`Run::gather`):
/// counted, and a step.
pub(super) fn relic(rewards: &mut Rewards, ledger: &mut Ledger, now: u64, id: u32, name: &str) {
    let earned = ledger.earned.entry(id).or_default();
    earned.relics = earned.relics.saturating_add(1);
    rewards.push(now, id, Kind::Relic { name: name.chars().take(48).collect() });
}

impl Rewards {
}

// ─── the rung ────────────────────────────────────────────────────────────────

pub use super::resolution::Climb;

/// The ladder's rungs, in pixels top to toe (`resolution::Rung::ALL`).
pub const RUNG_PIXELS: [u32; 4] = [16, 32, 64, 128];

/// The rung knight `id` is drawn at, in pixels top to toe, by the
/// resolution ladder's rule in force (`rules::resolution`, docs/LADDER.md):
/// what the rung-up's moment and the road ahead read.
pub fn rung_px(run: &Run, id: u32) -> u32 {
    super::resolution::rung(run, id).px()
}

/// The rung above knight `id`'s, and where it begins; none at the top, or
/// with one rung fixed for everyone.
pub fn next_rung(run: &Run, id: u32) -> Option<(u32, Climb)> {
    super::resolution::next(run, id).map(|(rung, at)| (rung.px(), at))
}

// ─── the sky ─────────────────────────────────────────────────────────────────

/// The realm's sky (`Home::sky`): a star for every wish granted, filling
/// the constellations in turn.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sky {
    #[serde(skip_serializing_if = "is_zero")]
    pub stars: u32,
    /// Wishes waiting for each knight at the next delve's start (`GIFT`).
    #[serde(skip_serializing_if = "is_zero8")]
    pub gift: u8,
    /// Whose wish finished each lit constellation, in order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lit_by: Vec<String>,
}

fn is_zero8(n: &u8) -> bool {
    *n == 0
}

impl Sky {
    pub fn is_empty(&self) -> bool {
        *self == Sky::default()
    }

    pub fn valid(&self) -> bool {
        self.gift <= GIFT && self.lit_by.len() <= MOST_LIT && self.lit_by.iter().all(|n| n.len() <= 48)
    }

    /// The constellation the stars are filling: its index, the stars
    /// placed in it so far, and how many it takes.
    pub fn filling(&self) -> (u32, u32, u32) {
        let mut left = self.stars;
        let mut i = 0;
        loop {
            let size = size(i);
            if left < size {
                return (i, left, size);
            }
            left -= size;
            i += 1;
        }
    }

    /// Constellations lit.
    pub fn lit(&self) -> u32 {
        self.filling().0
    }
}

/// Stars constellation `i` takes: ten, four more each after, at most
/// forty-eight.
pub fn size(i: u32) -> u32 {
    (10 + 4 * i.min(16)).min(48)
}

/// Constellation `i`'s name: the eight in turn, then again with a numeral.
pub fn constellation(i: u32) -> String {
    let n = CONSTELLATIONS.len() as u32;
    let name = CONSTELLATIONS[(i % n) as usize];
    match i / n {
        0 => name.to_string(),
        round => format!("{name} {}", ["II", "III", "IV", "V", "VI", "VII", "VIII", "IX"].get(round as usize - 1).copied().unwrap_or("+")),
    }
}

/// The sky's knob: `ANGEL_DELVE_SKY=off` (or `0`) keeps no stars and gives
/// no gift; anything else, or nothing, keeps the sky.
pub fn sky_on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| !matches!(std::env::var("ANGEL_DELVE_SKY").ok().as_deref().map(str::trim), Some("off" | "0" | "no")))
}

// ─── the road ahead ──────────────────────────────────────────────────────────

/// What is next for a knight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Next {
    /// Level `level`, `xp` experience away.
    Level { level: u32, xp: u32 },
    /// Every level reached.
    TopLevel,
    /// Lessons earned and waiting with Sir Ector.
    Lessons(u32),
    /// The next rung, `px`, and where it begins.
    Rung { px: u32, at: Climb },
    /// Tier `tier` (from 0) of wish `wish`, opening on floor `floor` (any
    /// floor when none).
    Tier { wish: String, tier: u8, floor: Option<u32> },
}

/// Roman numerals for tiers (from 0).
pub fn numeral(tier: u8) -> &'static str {
    ["I", "II", "III", "IV", "V"].get(usize::from(tier)).copied().unwrap_or("+")
}

impl Next {
    /// The few words the road ahead shows it in.
    pub fn line(&self) -> String {
        match self {
            Next::Level { level, xp } => format!("LV {level} IN {xp} XP"),
            Next::TopLevel => "EVERY LEVEL".to_string(),
            Next::Lessons(1) => "A LESSON WAITS".to_string(),
            Next::Lessons(n) => format!("{n} LESSONS WAIT"),
            Next::Rung { px, at: Climb::Floor(f) } => format!("{px} PX AT FLOOR {f}"),
            Next::Rung { px, at: Climb::Level(l) } => format!("{px} PX AT LV {l}"),
            Next::Tier { tier, floor: Some(f), .. } => format!("TIER {} AT FLOOR {f}", numeral(*tier)),
            Next::Tier { tier, floor: None, .. } => format!("TIER {} ANY FLOOR", numeral(*tier)),
        }
    }
}

/// What lies ahead for the party.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Road {
    /// Floor `floor` was the realm's deepest yet.
    Record { floor: u32 },
    /// The Deep Road opens once the realm has stood on floor `floor`.
    DeepRoad { floor: u32 },
    /// The next village of the Deep: on floor `floor`, the town of a people.
    Village { floor: u32, town: &'static str, people: &'static str },
    /// The sky: constellation `index` with `placed` of its `size` stars; a
    /// gift already waiting for the next delve.
    Sky { index: u32, placed: u32, size: u32, gift: bool },
}

impl Road {
    pub fn line(&self) -> String {
        match self {
            Road::Record { floor } => format!("FLOOR {floor}: THE REALM'S DEEPEST YET"),
            Road::DeepRoad { floor } => format!("STAND ON FLOOR {floor}: THE DEEP ROAD OPENS"),
            Road::Village { floor, town, people } => format!("NEXT VILLAGE: {town} OF {people}, FLOOR {floor}"),
            Road::Sky { gift: true, index, .. } => {
                format!("{} SHINES: A WISH MORE EACH NEXT DELVE", constellation(index.saturating_sub(1)))
            }
            Road::Sky { index, placed, size, .. } => {
                let left = size - placed;
                format!("{}: {left} WISH{} MORE LIGHT IT", constellation(*index), if left == 1 { "" } else { "ES" })
            }
        }
    }
}

/// The experience that is level `level`'s.
fn level_of(xp: u32) -> u32 {
    talents::Prowess { xp, learned: Vec::new() }.level()
}

impl Run {
    /// Knight `id`'s experience as it stands: what the realm keeps and what
    /// this run has marked since it last settled.
    pub fn experience(&self, id: u32) -> Option<(String, u32)> {
        let hero = self.players.get(&id)?;
        let key = talents::profile_key(hero);
        let kept = self.home.knights.get(&key).map_or(0, |p| p.xp);
        Some((key, kept.saturating_add(self.marks.get("xp").copied().unwrap_or(0))))
    }

    /// The ledger a seat reads: the run's last results carry it to every
    /// seat once the run is over; the host has its own.
    pub fn ledger_shown(&self) -> &Ledger {
        self.floor_results.as_ref().and_then(|r| r.ledger.as_ref()).unwrap_or(&self.ledger)
    }

    /// What is next for knight `id`: the next level and the lessons
    /// waiting, the next rung, the next tier of their best wish.
    pub fn ahead(&self, id: u32) -> Vec<Next> {
        let mut out = Vec::new();
        let Some((key, xp)) = self.experience(id) else {
            return out;
        };
        let level = level_of(xp);
        match talents::LEVELS.get(level as usize - 1) {
            Some(&at) => out.push(Next::Level { level: level + 1, xp: at - xp }),
            None => out.push(Next::TopLevel),
        }
        let learned = self.home.knights.get(&key).map_or(0, |p| p.learned.len() as u32);
        let waiting = (level - 1).saturating_sub(learned).min(talents::LESSONS.len() as u32);
        if waiting > 0 {
            out.push(Next::Lessons(waiting));
        }
        if let Some((px, at)) = next_rung(self, id) {
            out.push(Next::Rung { px, at });
        }
        if let Some((wish, tier)) = self.ledger_shown().earned.get(&id).and_then(|e| e.best.clone())
            && usize::from(tier) + 1 < phrasebook::MAX_TIERS
        {
            let next = tier + 1;
            out.push(Next::Tier { wish, tier: next, floor: overclass::tier_floor(usize::from(next)) });
        }
        out
    }

    /// What lies ahead for the party: a record set, the Deep's next
    /// village (or its road), the sky.
    pub fn road(&self) -> Vec<Road> {
        let mut out = Vec::new();
        let ledger = self.ledger_shown();
        if ledger.record {
            out.push(Road::Record { floor: ledger.deepest });
        }
        if !self.home.deep_road() {
            out.push(Road::DeepRoad { floor: DEEPEST });
        } else {
            let reached = self.home.deep.deepest.max(self.deep_floor().unwrap_or(0));
            let floor = (reached / deep::CHAPTER + 1) * deep::CHAPTER;
            let culture = self.home.deep.culture_at(floor);
            out.push(Road::Village { floor, town: culture.town(deep::chapter(floor)), people: culture.people() });
        }
        if sky_on() {
            let (index, placed, size) = self.home.sky.filling();
            out.push(Road::Sky { index, placed, size, gift: self.home.sky.gift > 0 });
        }
        out
    }

    /// A wish was granted knight `id` at tier `tier` (from 0) of `wish`
    /// (`Run::grant`): the ledger counts it, a tier above the first is a
    /// step, and the realm's sky takes a star.
    pub(super) fn rewarded_grant(&mut self, id: u32, wish: &str, tier: usize) {
        let tier = tier.min(phrasebook::MAX_TIERS - 1) as u8;
        self.ledger.granted(id, wish, tier);
        self.rewards.granted(self.tick, id, wish, tier);
        if !sky_on() {
            return;
        }
        let before = self.home.sky.filling().0;
        self.home.sky.stars = self.home.sky.stars.saturating_add(1);
        let after = self.home.sky.filling().0;
        if after > before {
            let by = self.players.get(&id).map_or_else(String::new, |h| h.shown_name()).chars().take(48).collect::<String>();
            if self.home.sky.lit_by.len() < MOST_LIT {
                self.home.sky.lit_by.push(by.clone());
            }
            self.home.sky.gift = GIFT;
            self.rewards.push(self.tick, 0, Kind::Lit { index: before, by });
        }
    }

    /// Whether a moment that came with a new floor may play: its title plate
    /// gone from every seat, a village's falling star landed and named.
    fn floor_settled(&self) -> bool {
        self.floor_plates.is_empty()
            && !self
                .deep
                .as_ref()
                .and_then(|d| d.village.as_ref())
                .is_some_and(|v| self.tick < v.arrived + deep::village::FALL + 150)
    }

    /// A tick of the watcher (`Run::step`): a level reached, a rung
    /// climbed, a new floor and its record, the sky's gift handed out, the
    /// ledger kept, and the steps that have played let go.
    pub(super) fn watch_rewards(&mut self) {
        let now = self.tick;
        let floor = self.floor();
        let realm_best = self.home.deepest.max(self.home.deep.deepest);
        let mut seen = std::mem::take(&mut self.rewards.seen);
        match seen.floor {
            None => seen.best = realm_best.max(floor),
            Some(was) if was != floor => {
                if was == 0 && floor >= 1 {
                    // A new delve: its ledger starts over.
                    self.ledger = Ledger::default();
                    seen.gift_due = sky_on() && self.home.sky.gift > 0;
                }
                // A landing off the Winding Stair is a floor stood on before,
                // except the Deep Road's first: that one counts.
                if floor >= 2 && floor > seen.best {
                    seen.held.push((0, Kind::Record { floor }));
                    self.ledger.record = true;
                }
                seen.best = seen.best.max(floor);
                if floor >= 1 {
                    self.ledger.deepest = self.ledger.deepest.max(floor);
                }
            }
            Some(_) => seen.best = seen.best.max(realm_best),
        }
        seen.floor = Some(floor);
        // The sky's gift, on the delve's first floor a knight wishes on (the
        // identity start's first floor gives its wishes a room at a time).
        if seen.gift_due && floor >= 1 && !(self.identity && self.dungeon.depth == 1 && self.deep.is_none()) {
            seen.gift_due = false;
            let wishes = std::mem::take(&mut self.home.sky.gift);
            if wishes > 0 {
                for hero in self.players.values_mut().filter(|h| !h.absent) {
                    hero.wishes = hero.wishes.saturating_add(wishes).min(overclass::MOST_WISHES);
                }
                seen.held.push((0, Kind::Gift { wishes }));
            }
        }
        let delving = floor >= 1;
        let ids: Vec<u32> = self.players.iter().filter(|(_, h)| !h.absent).map(|(&id, _)| id).collect();
        for id in ids {
            let Some((key, xp)) = self.experience(id) else {
                continue;
            };
            let level = level_of(xp);
            let rung = rung_px(self, id);
            let gold = self.players[&id].carried.get(crate::realm::Spoil::Gold);
            let was = seen.knights.get(&id).filter(|w| w.key == key).cloned();
            if delving {
                let earned = self.ledger.earned.entry(id).or_insert_with(|| Earned {
                    from_level: was.as_ref().map_or(level, |w| w.level),
                    level,
                    rung,
                    ..Earned::default()
                });
                if let Some(w) = &was {
                    earned.xp = earned.xp.saturating_add(xp.saturating_sub(w.xp));
                    earned.gold = earned.gold.saturating_add(gold.saturating_sub(w.gold));
                }
                earned.level = level;
                earned.rung = earned.rung.max(rung);
            }
            if let Some(w) = &was {
                if level > w.level {
                    self.rewards.push(now, id, Kind::Level { level });
                }
                if rung > w.rung {
                    seen.held.push((id, Kind::Rung { px: rung }));
                }
            }
            seen.knights.insert(id, Watched { key, xp, level, rung, gold });
        }
        seen.knights.retain(|id, _| self.players.contains_key(id));
        // What came with the floor plays once the floor's plate has cleared:
        // the party's banner, then each knight's.
        if !seen.held.is_empty() && self.floor_settled() {
            for (knight, kind) in std::mem::take(&mut seen.held) {
                self.rewards.push(now, knight, kind);
            }
        }
        self.rewards.steps.retain(|s| now < s.at + s.kind.life());
        // Each step lands with its sound as it starts (a step put up between
        // ticks, a tick late).
        for s in self.rewards.steps.iter().filter(|s| s.at <= now) {
            if !seen.sounded.contains(&(s.at, s.knight)) {
                seen.sounded.push((s.at, s.knight));
                if let Some(sound) = s.kind.sound() {
                    self.sounds.push(sound);
                }
            }
        }
        let steps = &self.rewards.steps;
        seen.sounded.retain(|&(at, knight)| steps.iter().any(|s| s.at == at && s.knight == knight));
        self.rewards.seen = seen;
    }
}

#[cfg(test)]
#[path = "../../tests/rules/rewards__tests.rs"]
mod tests;
