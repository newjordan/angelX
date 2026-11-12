//! The Deep Road: the floors between the villages, with things on it.
//!
//! Every other floor of a stretch of road (`stretch`) holds one meeting, in
//! a room or chamber of its own, drawn in the room (`deep_village::road`):
//! a lost patrol of one of the peoples, a caravan with odd goods, a blind
//! hermit with a riddle, captives to free once their captors are down, a
//! drowned shrine, an envoy of a rival people. E beside them opens a
//! two-page talk as a village's does, and the answer on 1, 2 or 3 is a
//! choice with a cost and a consequence that the rest of the Deep keeps:
//!
//! - **Favour**: each people's regard for the realm (`Ledger`, on
//!   `Home::deep`). A village whose people are friends lights a step more
//!   as the star lands and pays its errands more; one that is wary keeps
//!   the star's own light back (`Run::road_news`).
//! - **Errands**: a lamp lit, a relic found or bought counts on the
//!   chapter's open errands (`missions`), and the kills of a fight count
//!   on its hunts.
//! - **Cargo**: a gift, a word or a letter the party carries to the next
//!   village (`Descent::cargo`), delivered as the star lands.
//! - **Dread**: a plundered shrine stirs the dread at the next village's
//!   stair (`Descent::stir`): more of it, and angrier, when it is met.
//!
//! Nothing here draws from the run's dice: a floor's meeting is where its
//! own rooms and number say. What a card, a sign or a talk line says is
//! what the choice does.

use super::cultures::WHEEL;
use super::missions::{hosts, spot};
use super::*;
use crate::realm::{Spoil, Spoils};

/// How near (arena units) a knight stands to speak to a meeting.
pub const REACH: f32 = 4.4;
/// How near (arena units) a knight comes before captors rise.
pub const RISE_REACH: f32 = 16.0;
/// The most parcels the party carries, foes a meeting raises and stir the
/// dread takes.
pub const MOST_CARGO: usize = 4;
pub const MOST_FOES: usize = 12;
pub const MOST_STIR: u8 = 3;
/// A people's regard runs from `-FAVOUR_MOST` to `FAVOUR_MOST`; at `FRIEND`
/// they are friends, at `WARY` wary.
pub const FAVOUR_MOST: i8 = 9;
pub const FRIEND: i8 = 3;
pub const WARY: i8 = -3;
/// What a village of friends pays on top of an errand's purse, each.
pub const FRIEND_PURSE: u32 = 25;

/// The prices and prizes of the road's choices, in gold unless said.
pub const OIL: u32 = 50;
pub const GIFT: u32 = 60;
pub const DRAUGHT: u32 = 40;
pub const OFFERING: u32 = 80;
pub const HOARD_GOLD: u32 = 80;
pub const HOARD_GEMS: u32 = 2;
pub const TAKEN_GEMS: u32 = 2;
pub const BOUNTY: u32 = 100;
pub const RIDDLE_GEMS: u32 = 2;
/// How much of their health a mend gives each knight, in percent.
pub const MEND_PERCENT: u32 = 50;
/// Foes raised, however many knights: the patrol's pursuers, the captors,
/// what comes up the shore at the bell, the dark that answers a wrong
/// answer. Each comes as strong as the floor and the party make it.
pub const PURSUERS: usize = 3;
pub const CAPTORS: usize = 4;
pub const SHORE: usize = 4;
pub const AMBUSH: usize = 3;

/// How long after it was settled an envoy sent home, and captives let out
/// of their cage, are gone from the room.
pub const ENVOY_LEAVES: u64 = 90;
pub const CAPTIVES_LEAVE: u64 = 80;

/// What stands on the road.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Patrol,
    Caravan,
    Hermit,
    Captives,
    Shrine,
    Envoy,
}

impl Kind {
    pub const ALL: [Kind; 6] = [Kind::Patrol, Kind::Caravan, Kind::Hermit, Kind::Captives, Kind::Shrine, Kind::Envoy];

    /// How the tracker names it.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Patrol => "A LOST PATROL",
            Kind::Caravan => "A DEEP CARAVAN",
            Kind::Hermit => "A BLIND HERMIT",
            Kind::Captives => "CAPTIVE DELVERS",
            Kind::Shrine => "A DROWNED SHRINE",
            Kind::Envoy => "AN ENVOY",
        }
    }
}

/// Where a meeting stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Captives in their cage, the captors not yet risen.
    Waiting,
    /// Foes up: the captors, the pursuers, what comes up the shore.
    Fighting,
    /// Ready to talk.
    Open,
    /// Done: what is left is what it says.
    Settled,
}

/// A knight's talk with a meeting: 0 their greeting, 1 what they ask.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Talk {
    pub page: u8,
    pub opened: u64,
}

/// One meeting on a floor of the road.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Meeting {
    pub kind: Kind,
    pub room: usize,
    /// Its tile in that room.
    pub col: u8,
    pub row: u8,
    /// Whose it is: the patrol's, the captives', the envoy's, the trader's
    /// and the shrine's people.
    pub people: Culture,
    /// The hermit's riddle, and where its right answer stands (0 to 2).
    pub riddle: u8,
    pub key: u8,
    pub stage: Stage,
    /// The answer taken, once there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<u8>,
    /// The caravan's wares sold, a bit each.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub sold: u8,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub talks: BTreeMap<u32, Talk>,
    /// The foes it raised.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub foes: Vec<u32>,
    /// The tick it was settled, for its flare.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<u64>,
}

impl Meeting {
    pub fn valid(&self, rooms: usize) -> bool {
        self.room < rooms
            && self.talks.len() <= MAX_PLAYERS as usize
            && self.talks.iter().all(|(id, t)| (1..=MAX_PLAYERS).contains(id) && t.page <= 1)
            && self.foes.len() <= MOST_FOES
            && self.riddle < RIDDLES.len() as u8
            && self.key < 3
            && self.sold < 8
            && self.choice.is_none_or(|c| (1..=3).contains(&c))
    }

    /// Its tile point: the middle of its tile.
    pub fn tile_point(&self) -> (f32, f32) {
        (f32::from(self.col) + 0.5, f32::from(self.row) + 0.5)
    }

    /// Its point in arena units.
    pub fn point(&self) -> (f32, f32) {
        let (x, y) = self.tile_point();
        (x * TILE_UNITS, y * TILE_UNITS)
    }

    pub fn settled(&self) -> bool {
        self.stage == Stage::Settled
    }

    /// Whether they have left the room by tick `tick`: an envoy sent home,
    /// captives let out. Nobody is there to speak to.
    pub fn gone(&self, tick: u64) -> bool {
        let since = self.at.map_or(0, |at| tick.saturating_sub(at));
        match self.kind {
            Kind::Envoy => self.choice == Some(3) && since > ENVOY_LEAVES,
            Kind::Captives => self.stage == Stage::Settled && since > CAPTIVES_LEAVE,
            _ => false,
        }
    }
}

/// What a party carries down the road to the next village.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Parcel {
    /// Goods bought for the village.
    Gift,
    /// Word from freed captives.
    Word,
    /// An envoy's letter of peace.
    Peace,
    /// An envoy's challenge.
    Challenge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cargo {
    pub parcel: Parcel,
    /// The people who sent it.
    pub from: Culture,
}

/// How a people stand with the realm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Wary,
    Known,
    Friend,
}

/// Each people's regard for the realm: one place on the wheel each, kept
/// on `Home::deep` and left out of a save while it is all even.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ledger {
    pub favour: [i8; 8],
}

fn slot(culture: Culture) -> usize {
    WHEEL.iter().position(|&c| c == culture).unwrap_or(0)
}

impl Ledger {
    pub fn is_empty(&self) -> bool {
        *self == Ledger::default()
    }

    pub fn valid(&self) -> bool {
        self.favour.iter().all(|f| f.abs() <= FAVOUR_MOST)
    }

    pub fn of(&self, culture: Culture) -> i8 {
        self.favour[slot(culture)]
    }

    /// `culture`'s regard moved by `by`, held to its bounds.
    pub fn shift(&mut self, culture: Culture, by: i8) {
        let f = &mut self.favour[slot(culture)];
        *f = (*f + by).clamp(-FAVOUR_MOST, FAVOUR_MOST);
    }

    pub fn tier(&self, culture: Culture) -> Tier {
        match self.of(culture) {
            f if f >= FRIEND => Tier::Friend,
            f if f <= WARY => Tier::Wary,
            _ => Tier::Known,
        }
    }
}

/// Where floor `floor` stands on its stretch of road, counting from 0 at
/// the stretch's first floor; none for a village or a floor above the Deep.
pub fn stretch(floor: u32) -> Option<u32> {
    if floor < ROAD || is_village(floor) {
        return None;
    }
    let first = if chapter(floor) == 0 { ROAD } else { chapter(floor) * CHAPTER + 1 };
    Some(floor - first)
}

/// Whether floor `floor` of the Deep holds a meeting: every other floor
/// of its stretch, the first included.
pub fn meets(floor: u32) -> bool {
    stretch(floor).is_some_and(|n| n.is_multiple_of(2))
}

/// The village at the end of floor `floor`'s stretch.
pub fn ahead(floor: u32) -> u32 {
    (chapter(floor) + 1) * CHAPTER
}

/// The kind a stretch lays as its `m`th meeting: the six turn in an order
/// of the delve's own, a shrine never on the floor of the dread it would
/// stir.
pub fn kind_for(seed: u64, c: u32, m: u32, guard: bool) -> Kind {
    let mut order = Kind::ALL;
    order.sort_by_key(|k| mix(seed ^ (*k as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15)));
    let mut i = (m + c) as usize % order.len();
    while guard && order[i] == Kind::Shrine {
        i = (i + 1) % order.len();
    }
    order[i]
}

/// The riddles: a question, its right answer and two wrong ones.
pub const RIDDLES: [(&str, &str, [&str; 2]); 6] = [
    (
        "I AM LIT BY ONE HAND AND SEEN BY A HUNDRED. I GROW WHEN I AM GIVEN AWAY. WHAT AM I?",
        "HOPE",
        ["A TORCH", "A DEBT"],
    ),
    (
        "I HAVE A MOUTH AND NEVER EAT, A TONGUE OF IRON, AND I TELL THE HOURS TO THE DROWNED. WHAT AM I?",
        "A BELL",
        ["A WELL", "A FORGE"],
    ),
    (
        "I FALL FROM A CEILING I CANNOT TOUCH, AND BUILD A TOWER ONE DROP AT A TIME. WHAT AM I?",
        "A STALAGMITE",
        ["A STAR", "A SPIDER"],
    ),
    (
        "THE MORE OF ME YOU TAKE, THE MORE YOU LEAVE BEHIND. WHAT AM I?",
        "FOOTSTEPS",
        ["GOLD", "LAMP OIL"],
    ),
    (
        "NO SUN FEEDS ME AND I HAVE NO LEAF, YET THE DEEP FOLK PLANT ME AND REAP ME. WHAT AM I?",
        "A MUSHROOM",
        ["A SUNFLOWER", "A WILLOW"],
    ),
    (
        "A BREATH CAN PUT ME OUT AND A SPARK CAN WAKE ME. I AM THE ROAD'S ONLY HONEST SIGN. WHAT AM I?",
        "A FLAME",
        ["A SHADOW", "A ROCK"],
    ),
];

impl Culture {
    /// The captain of the people's watch, who leads a lost patrol.
    pub fn watch(self) -> &'static str {
        match self {
            Culture::Capfolk => "SERGEANT BUTTON",
            Culture::Glimmerkin => "WARDEN SLATE",
            Culture::Emberfolk => "CAPTAIN RIVET",
            Culture::Lampwick => "WICK-WARDEN OAR",
            Culture::Roostfolk => "RIDER-LEAD KESTREL",
            Culture::Rootkin => "WARDEN SPLINT",
            Culture::Chimefolk => "BELL-WARDEN CLAPPER",
            Culture::Saltkin => "SALT-WARDEN GRIT",
        }
    }

    /// The people's envoy.
    pub fn envoy(self) -> &'static str {
        match self {
            Culture::Capfolk => "ENVOY PUFFBALL",
            Culture::Glimmerkin => "ENVOY PRISM",
            Culture::Emberfolk => "ENVOY SMELT",
            Culture::Lampwick => "ENVOY GUTTERWICK",
            Culture::Roostfolk => "ENVOY SWOOP",
            Culture::Rootkin => "ENVOY BRIAR",
            Culture::Chimefolk => "ENVOY CAROL",
            Culture::Saltkin => "ENVOY BRACKISH",
        }
    }

    /// The people's trader.
    pub fn trader(self) -> &'static str {
        match self {
            Culture::Capfolk => "TRADER TUFTED",
            Culture::Glimmerkin => "TRADER GRAVEL",
            Culture::Emberfolk => "TRADER SLAG",
            Culture::Lampwick => "TRADER DREGS",
            Culture::Roostfolk => "TRADER FEATHERWEIGHT",
            Culture::Rootkin => "TRADER TWIG",
            Culture::Chimefolk => "TRADER KNELL",
            Culture::Saltkin => "TRADER GRAIN",
        }
    }
}

/// A figure on the talk board: whose portrait to draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Folk(Culture, usize),
    Hermit,
    Shrine,
}

/// What knight `id`'s talk shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub name: String,
    pub people: String,
    pub face: Face,
    pub words: String,
    pub answers: Vec<String>,
}

/// One answer: what it says, what it costs, and why it cannot be taken if
/// it cannot.
struct Opt {
    label: String,
    /// Gold it costs the knight who answers, or pays each knight.
    cost: Option<u32>,
    gain: Option<u32>,
    lack: Option<String>,
}

impl Opt {
    fn new(label: impl Into<String>) -> Opt {
        Opt { label: label.into(), cost: None, gain: None, lack: None }
    }

    fn costs(label: &str, gold: u32, have: u32) -> Opt {
        let lack = (have < gold).then(|| format!("NEED {gold} GOLD"));
        Opt { label: label.into(), cost: Some(gold), gain: None, lack }
    }

    fn pays(label: &str, gold: u32) -> Opt {
        Opt { label: label.into(), cost: None, gain: Some(gold), lack: None }
    }

    /// As the board shows it.
    fn shown(&self) -> String {
        match (&self.lack, self.cost, self.gain) {
            (Some(lack), _, _) => format!("{} ({lack})", self.label),
            (None, Some(gold), _) => format!("{} ({gold} GOLD)", self.label),
            (None, None, Some(gold)) => format!("{} (+{gold} GOLD)", self.label),
            _ => self.label.clone(),
        }
    }
}

/// Everything a meeting says, for the knight `id` looking at it.
struct Script {
    name: String,
    face: Face,
    hello: String,
    ask: String,
    options: Vec<Opt>,
    after: String,
}

fn gold(run: &Run, id: u32) -> u32 {
    run.players.get(&id).map_or(0, |h| h.carried.get(Spoil::Gold))
}

/// The dread ahead stirred by `by`, or settled by a negative.
fn stir(deep: &mut Descent, by: i8) {
    deep.stir = (i16::from(deep.stir) + i16::from(by)).clamp(0, i16::from(MOST_STIR)) as u8;
}

impl Run {
    /// The village a stretch leads to: its people, as the realm will meet
    /// them.
    fn town_ahead(&self, floor: u32) -> (Culture, &'static str) {
        let v = ahead(floor);
        let culture = self.home.deep.culture_at(v);
        (culture, culture.town(chapter(v)))
    }

    /// An open errand of the chapter below the village behind that a lamp
    /// lit or a relic found would count on: its index, its kind.
    fn helped_errand(&self, floor: u32, want: Errand) -> Option<usize> {
        self.home.deep.open_on(floor).find(|(_, m)| m.errand == want).map(|(i, _)| i)
    }

    /// Lay floor `floor`'s meeting, if its place on the stretch has one.
    pub(super) fn lay_meeting(&mut self, floor: u32) {
        if let Some(d) = &mut self.deep {
            d.meeting = None;
        }
        if !meets(floor) {
            return;
        }
        let c = chapter(floor);
        let n = stretch(floor).unwrap_or(0);
        let guard = dread::guards(floor).is_some();
        let delve = self.raid_id ^ u64::from(self.home.deep.wheel).rotate_left(23);
        let kind = kind_for(delve ^ u64::from(c).wrapping_mul(0x2545_f491_4f6c_dd1d), c, n / 2, guard);
        let salt = delve ^ u64::from(floor).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x7a11;
        let signs: Vec<(usize, (f32, f32))> =
            self.deep.as_ref().map_or_else(Vec::new, |d| d.signs.iter().map(|s| (s.room, s.point())).collect());
        let mut placed = None;
        // Not a hall that hosts a game of its own (a folly, a gantry, a race
        // hall), nor the room with a cage in it.
        let caged = self.captive.as_ref().map(|c| c.room);
        for (i, (room, near)) in hosts(&self.dungeon, salt).into_iter().enumerate() {
            if self.hosts_game(room) || caged == Some(room) {
                continue;
            }
            let Some((col, row)) = spot(&self.dungeon.rooms[room], near, salt ^ ((i as u64 + 1) * 0x51)) else {
                continue;
            };
            let at = ((f32::from(col) + 0.5) * TILE_UNITS, (f32::from(row) + 0.5) * TILE_UNITS);
            if signs.iter().all(|&(r, p)| r != room || (p.0 - at.0).hypot(p.1 - at.1) > 8.0) {
                placed = Some((room, col, row));
                break;
            }
        }
        let Some((room, col, row)) = placed else {
            return;
        };
        let (ahead_people, _) = self.town_ahead(floor);
        let pick = mix(salt ^ 0x61);
        let people = match kind {
            Kind::Patrol | Kind::Shrine | Kind::Hermit => ahead_people,
            Kind::Captives if c >= 1 => self.home.deep.culture_at(c * CHAPTER),
            Kind::Captives => ahead_people,
            Kind::Caravan => WHEEL[(pick % WHEEL.len() as u64) as usize],
            Kind::Envoy => WHEEL[(slot(ahead_people) + 1 + (pick % (WHEEL.len() as u64 - 1)) as usize) % WHEEL.len()],
        };
        let meeting = Meeting {
            kind,
            room,
            col,
            row,
            people,
            riddle: (pick >> 8) as u8 % RIDDLES.len() as u8,
            key: ((pick >> 16) % 3) as u8,
            stage: if kind == Kind::Captives { Stage::Waiting } else { Stage::Open },
            choice: None,
            sold: 0,
            talks: BTreeMap::new(),
            foes: Vec::new(),
            at: None,
        };
        if let Some(d) = &mut self.deep {
            d.meeting = Some(meeting);
        }
    }

    /// Which chamber of an open map holds tile point `at`.
    fn chamber_at(&self, at: (f32, f32)) -> Option<u8> {
        let open = self.dungeon.open.as_ref()?;
        open.chambers.iter().position(|c| c.holds(at.0 * TILE_UNITS, at.1 * TILE_UNITS, 0.0)).map(|i| i as u8)
    }

    /// Whether the meeting's room is an open floor's map.
    fn on_map(&self, room: usize) -> bool {
        self.dungeon.open.as_ref().is_some_and(|o| o.map == room)
    }

    /// One tick of the road: the party's talk with a meeting, its captors
    /// rising and falling.
    pub(super) fn tick_road(&mut self, deep: &mut Descent, pressed: u8) {
        let Some(mut m) = deep.meeting.take() else {
            return;
        };
        if m.room != self.at {
            // Away from it: nobody talks, and what rose in a room that was
            // left fell back with the room.
            m.talks.clear();
            if m.stage == Stage::Fighting && !self.on_map(m.room) {
                m.foes.clear();
                m.stage = if m.choice.is_some() { Stage::Open } else { Stage::Waiting };
                m.choice = None;
            }
        } else {
            self.tick_meeting(deep, &mut m, pressed);
        }
        deep.meeting = Some(m);
    }

    fn tick_meeting(&mut self, deep: &mut Descent, m: &mut Meeting, pressed: u8) {
        let tp = m.tile_point();
        // Whoever has walked off has ended their talk.
        let gone: Vec<u32> = m.talks.keys().copied().filter(|&id| !self.nook_near(id, tp, REACH)).collect();
        for id in gone {
            m.talks.remove(&id);
        }
        match m.stage {
            Stage::Waiting => {
                let near = self.players.keys().any(|&id| self.nook_near(id, tp, RISE_REACH) && self.players[&id].hp > 0);
                if near && !self.fight_on() {
                    m.foes = self.raise_foes(tp, CAPTORS);
                    m.stage = Stage::Fighting;
                    self.sounds.push("boss_rise");
                    self.cues.push("deep_captors".into());
                    self.found = Some((self.tick, 0, "THE CAPTORS RISE.".into()));
                }
            }
            Stage::Fighting => {
                let up = m.foes.iter().any(|f| self.enemies.iter().any(|e| e.id == *f && e.hp > 0));
                if !up {
                    match m.choice {
                        None => {
                            m.stage = Stage::Open;
                            self.sounds.push("door_open");
                            self.found = Some((self.tick, 0, "THE CAPTORS ARE DOWN. THE CAGED LOOK UP.".into()));
                        }
                        Some(pick) => self.prize(deep, m, pick),
                    }
                }
            }
            Stage::Open | Stage::Settled => {}
        }
        for id in self.nook_players() {
            if !super::super::nooks::has(pressed, id) {
                continue;
            }
            if let Some(talk) = m.talks.get(&id).copied() {
                // E on an open talk: on to what they ask, then away.
                if talk.page == 0 && m.stage == Stage::Open {
                    m.talks.insert(id, Talk { page: 1, ..talk });
                } else {
                    m.talks.remove(&id);
                }
                continue;
            }
            if !self.nook_near(id, tp, REACH) || !matches!(m.stage, Stage::Open | Stage::Settled) || m.gone(self.tick) {
                continue;
            }
            if self.fight_on() {
                self.found = Some((self.tick, id, "THEY WAIT FOR THE FIGHTING TO STOP.".into()));
                continue;
            }
            m.talks.insert(id, Talk { page: 0, opened: self.tick });
            self.sounds.push("play_card");
        }
    }

    /// Raise `n` foes round tile point `tp`, out of the floor's own
    /// company, on clear ground away from the knights.
    fn raise_foes(&mut self, tp: (f32, f32), n: usize) -> Vec<u32> {
        let roster = self.dungeon.pack.roster_at(self.dungeon.depth);
        let mut pool: Vec<EnemyKind> = roster
            .iter()
            .map(|&(k, _)| k)
            .filter(|k| !matches!(k, EnemyKind::Bat | EnemyKind::Slime | EnemyKind::Spiderling | EnemyKind::Dragon | EnemyKind::Boss | EnemyKind::Dummy))
            .collect();
        if pool.is_empty() {
            pool.push(EnemyKind::Skeleton);
        }
        let (cx, cy) = (tp.0 * TILE_UNITS, tp.1 * TILE_UNITS);
        let home = self.chamber_at(tp);
        let salt = self.tick ^ self.raid_id;
        let mut ids = Vec::new();
        let mut tried = 0u64;
        while ids.len() < n.min(MOST_FOES) && tried < 96 {
            let h = mix(salt ^ tried.wrapping_mul(0x9e37_79b9));
            tried += 1;
            let angle = (h % 360) as f32 / 360.0 * std::f32::consts::TAU;
            let radius = 6.0 + ((h >> 12) % 5) as f32 * 1.4;
            let (x, y) = (cx + angle.cos() * radius, cy + angle.sin() * radius * 0.8);
            let (col, row) = ((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32);
            let room = &self.dungeon.rooms[self.at];
            let clear = room.tile(col, row).ground() && room.tile(col + 1, row).ground() && room.tile(col, row + 1).ground();
            let apart = self.enemies.iter().all(|e| (e.x - x).hypot(e.y - y) > 2.0)
                && self.players.values().all(|p| (p.x - x).hypot(p.y - y) > 4.0);
            if !clear || !apart {
                continue;
            }
            let kind = pool[(h >> 20) as usize % pool.len()];
            if let Some(id) = self.spawn_at(kind, x, y) {
                if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
                    e.home = home;
                }
                ids.push(id);
            }
        }
        ids
    }

    /// The words, the answers and the portrait of meeting `m`, as knight
    /// `id` sees them.
    fn script(&self, deep: &Descent, m: &Meeting, id: u32) -> Script {
        let floor = deep.floor;
        let (ahead_people, town) = self.town_ahead(floor);
        let p = m.people;
        let (lamp, relic, _) = p.sign_words();
        let people = p.people();
        let have = gold(self, id);
        let gem = self.players.get(&id).map_or(0, |h| h.carried.get(Spoil::Gem));
        match m.kind {
            Kind::Patrol => {
                let options = vec![
                    Opt::costs("SHARE YOUR LIGHT", OIL, have),
                    Opt::new(format!("TAKE THEIR GEMS ({TAKEN_GEMS} EACH)")),
                    Opt::new("SEE THEM TO THE STAIR (A FIGHT)"),
                ];
                Script {
                    name: p.watch().into(),
                    face: Face::Folk(p, 0),
                    hello: format!("A PATROL OF {people}, FOUR DAYS FROM {town}. OUR {lamp} IS COLD, STAR-KNIGHT, AND THE DARK HAS BEEN COUNTING US."),
                    ask: "SPARE US LIGHT AND WE WALK HOME BY IT. OR TAKE WHAT WE CARRY: WE WON'T STOP YOU. OR SEE US TO THE STAIR, FOR SOMETHING HAS FOLLOWED US DOWN.".into(),
                    options,
                    after: match m.choice {
                        Some(1) => format!("YOUR LIGHT IS IN OUR {lamp} NOW. {town} WILL HEAR WHO LIT IT."),
                        Some(2) => "TAKE THEM. WE WILL REMEMBER WHO ASKED.".into(),
                        _ => "WE WALK HOME WITH THE STAIR AT OUR BACKS. THANK YOU.".into(),
                    },
                }
            }
            Kind::Caravan => {
                let wanted = self.helped_errand(floor, Errand::Gather).is_some();
                let first = if wanted { format!("BUY A {relic}") } else { format!("A GIFT FOR {town}") };
                let mut options = vec![
                    Opt::costs(&first, GIFT, have),
                    Opt::costs("A DRAUGHT OF DEEP-HONEY", DRAUGHT, have),
                    Opt::pays("SELL A GEM", Spoil::Gem.worth()),
                ];
                if gem == 0 {
                    options[2].lack = Some("NO GEM".into());
                }
                for (i, opt) in options.iter_mut().enumerate() {
                    if m.sold & 1 << i != 0 {
                        opt.lack = Some("SOLD".into());
                    }
                }
                Script {
                    name: p.trader().into(),
                    face: Face::Folk(p, 3),
                    hello: format!("A CARAVAN OF {people}, AND A LANTERN-BEAST LADEN WITH ODD GOODS: {relic}S, DEEP-HONEY, WHATEVER THE ROAD HAS LEFT THEM."),
                    ask: "EVERYTHING HERE HAS COME UP A STAIR OR DOWN ONE. WHAT WILL IT BE, STAR-KNIGHT?".into(),
                    options,
                    after: "THE LANTERN-BEAST IS LOADED AGAIN AND SO AM I. MIND THE ROAD.".into(),
                }
            }
            Kind::Hermit => {
                let (question, right, wrong) = RIDDLES[usize::from(m.riddle)];
                let mut wrongs = wrong.iter();
                let options = (0..3u8)
                    .map(|k| Opt::new(if k == m.key { right } else { wrongs.next().copied().unwrap_or("NOTHING") }))
                    .collect();
                Script {
                    name: "THE BLIND HERMIT".into(),
                    face: Face::Hermit,
                    hello: "I CANNOT SEE YOU. I CAN SMELL THE SUN ON YOU, THOUGH, AND THAT IS RARE. SIT. ANSWER ME ONE THING AND I WILL LIGHT MY LAMP FOR YOU.".into(),
                    ask: question.into(),
                    options,
                    after: if m.choice == Some(m.key + 1) {
                        "THE LAMP IS LIT, AND I CAN FEEL IT. GO WELL. MIND THE DARK AT THE STAIR.".into()
                    } else {
                        "THE DARK DOES NOT LIKE TO BE ANSWERED WRONG. GO WELL, IF YOU CAN.".into()
                    },
                }
            }
            Kind::Captives => {
                let digs = self.helped_errand(floor, Errand::Gather).is_some();
                let second = if digs {
                    format!("FREE THEM, THEY DIG OUT A {relic}")
                } else {
                    "FREE THEM, THEY TEND YOUR WOUNDS".into()
                };
                let options = vec![
                    Opt::new("FREE THEM, TAKE THEIR PURSE"),
                    Opt::new(second),
                    Opt::new(format!("FREE THEM, SEND WORD TO {town}")),
                ];
                Script {
                    name: "THE ELDEST CAPTIVE".into(),
                    face: Face::Folk(p, 4),
                    hello: format!("YOU CAME IN ON THE NOISE. WE PRAYED IT WAS YOURS. THREE DELVERS OF {people}, TAKEN ON THE ROAD, AND KEPT FOR WHAT WE KNOW OF THE SEAMS."),
                    ask: format!("OPEN THE CAGE AND WE ARE FREE. WE CAN PAY WHAT THE CAPTORS LEFT US, HELP YOU WITH WHAT WE KNOW OF THE SEAMS, OR CARRY WORD AHEAD TO {town} THAT YOU CAME."),
                    options,
                    after: "THE CAGE STANDS OPEN AND WE ARE HOME ON THE WAY. THE DARK WILL HAVE TO FIND ITS OWN MINERS.".into(),
                }
            }
            Kind::Shrine => {
                let options = vec![
                    Opt::new("TAKE THE HOARD (WAKES THE DREAD)"),
                    Opt::costs("LEAVE AN OFFERING", OFFERING, have),
                    Opt::new("RING THE DROWNED BELL (A FIGHT)"),
                ];
                Script {
                    name: "THE DROWNED SHRINE".into(),
                    face: Face::Shrine,
                    hello: format!("A SHRINE OF {people}, HALF DROWNED, ITS {lamp} STILL BURNING UNDER THE WATER. A HOARD LIES AT ITS FOOT, LEFT FOR THE DROWNED. SOMETHING BELOW WATCHES IT."),
                    ask: format!("THE WATER WAITS. TAKE THE HOARD AND THE DREAD OF {town} WAKES THE STRONGER. LEAVE AN OFFERING AND THE WATER GOES STILL. OR RING THE BELL AND SEE WHAT ANSWERS."),
                    options,
                    after: match m.choice {
                        Some(1) => "THE WATER IS BLACK AND EMPTY.".into(),
                        Some(2) => format!("THE {lamp} BURNS STEADY UNDER THE WATER."),
                        _ => "THE BELL HANGS STILL. THE WATER IS ONLY WATER.".into(),
                    },
                }
            }
            Kind::Envoy => {
                let options = vec![
                    Opt::new("CARRY THEIR PEACE-LETTER"),
                    Opt::pays("CARRY THEIR CHALLENGE", BOUNTY),
                    Opt::new("SEND THEM HOME"),
                ];
                let against = ahead_people.people();
                Script {
                    name: p.envoy().into(),
                    face: Face::Folk(p, 0),
                    hello: format!("{people} AND {against} HAVE QUARRELLED OVER THE LAMP-ROADS FOR FORTY YEARS. I HAVE A LETTER FOR {town}, AND NO GUARD LEFT TO CARRY IT."),
                    ask: "A LETTER OF PEACE, IF YOU WILL TAKE IT. OR A CHALLENGE, AND I WILL PAY WELL TO HAVE IT ARRIVE. OR TELL ME TO FIND MY OWN WAY.".into(),
                    options,
                    after: "THE LETTER IS YOURS TO CARRY NOW, AND I AM LIGHTER.".into(),
                }
            }
        }
    }

    /// What knight `id`'s open talk with the road's meeting shows.
    pub fn road_view(&self, id: u32) -> Option<View> {
        let deep = self.deep.as_ref()?;
        let m = deep.meeting.as_ref().filter(|m| m.room == self.at && !m.gone(self.tick))?;
        let talk = m.talks.get(&id)?;
        let s = self.script(deep, m, id);
        let (words, answers) = if m.stage == Stage::Settled {
            (s.after, vec!["E: FAREWELL".to_string()])
        } else if talk.page == 0 {
            (s.hello, vec!["E: GO ON".to_string()])
        } else if self.fight_on() {
            (s.ask, vec!["THEY WAIT FOR THE FIGHTING TO STOP.".to_string()])
        } else {
            let answers = s
                .options
                .iter()
                .enumerate()
                .map(|(i, o)| format!("{}: {}", i + 1, o.shown()))
                .collect();
            (s.ask, answers)
        };
        let people = if m.kind == Kind::Hermit { "OF THE DEEP ROAD".to_string() } else { m.people.people().to_string() };
        Some(View { name: s.name, people, face: s.face, words, answers })
    }

    /// The knights with a talk open on the road.
    pub(super) fn road_talking(&self) -> Vec<u32> {
        self.deep
            .as_ref()
            .and_then(|d| d.meeting.as_ref())
            .filter(|m| m.room == self.at)
            .map(|m| m.talks.keys().copied().collect())
            .unwrap_or_default()
    }

    /// Knight `id` answers a meeting with key `pick` (1 to 3).
    pub(super) fn answer_road(&mut self, id: u32, pick: u8) {
        let Some(mut deep) = self.deep.take() else {
            return;
        };
        if let Some(mut m) = deep.meeting.take() {
            self.answer_meeting(&mut deep, &mut m, id, pick);
            deep.meeting = Some(m);
        }
        self.deep = Some(deep);
    }

    fn answer_meeting(&mut self, deep: &mut Descent, m: &mut Meeting, id: u32, pick: u8) {
        let Some(talk) = m.talks.get(&id).copied() else {
            return;
        };
        if talk.page == 0 || m.stage != Stage::Open || !(1..=3).contains(&pick) || self.fight_on() {
            return;
        }
        let script = self.script(deep, m, id);
        let Some(opt) = script.options.get(usize::from(pick) - 1) else {
            return;
        };
        if let Some(lack) = &opt.lack {
            self.found = Some((self.tick, id, format!("{}: {lack}.", opt.label)));
            return;
        }
        self.settle(deep, m, id, pick);
    }

    fn pay(&mut self, id: u32, amount: u32) {
        let price = Spoils(BTreeMap::from([(Spoil::Gold, amount)]));
        if let Some(h) = self.players.get_mut(&id) {
            h.carried.take(&price);
        }
        self.sounds.push("gold");
    }

    /// Every standing knight gains `n` of `spoil`.
    fn each(&mut self, spoil: Spoil, n: u32) {
        for h in self.players.values_mut().filter(|h| h.hp > 0) {
            h.carried.add(spoil, n);
        }
    }

    fn mend_all(&mut self) {
        for h in self.players.values_mut().filter(|h| h.hp > 0) {
            h.hp = (h.hp + h.max_hp * MEND_PERCENT / 100).min(h.max_hp);
        }
        self.sounds.push("potion");
    }

    /// Count `n` on an open errand of `want`, and say so if it finished.
    fn help_errand(&mut self, floor: u32, want: Errand, n: u16) -> Option<usize> {
        let i = self.helped_errand(floor, want)?;
        self.count_errand(i, n, floor);
        Some(i)
    }

    /// The answer taken: its cost paid, its consequence done, the meeting
    /// settled or its fight begun.
    fn settle(&mut self, deep: &mut Descent, m: &mut Meeting, id: u32, pick: u8) {
        let floor = deep.floor;
        let (ahead_people, town) = self.town_ahead(floor);
        let p = m.people;
        let (lamp, relic, _) = p.sign_words();
        let people = p.people();
        let purse = purse(floor);
        let mut say: String;
        let mut fight = None;
        match (m.kind, pick) {
            (Kind::Patrol, 1) => {
                self.pay(id, OIL);
                self.home.deep.road.shift(p, 2);
                let lit = self.help_errand(floor, Errand::Beacons, 1).is_some();
                say = if lit {
                    format!("{people} WALK HOME BY YOUR LIGHT, AND LIGHT A {lamp} FOR YOU. ERRAND +1. FAVOUR WITH {people} +2.")
                } else {
                    format!("{people} WALK HOME BY YOUR LIGHT. FAVOUR WITH {people} +2.")
                };
                self.sounds.push("wish_grant");
            }
            (Kind::Patrol, 2) => {
                self.each(Spoil::Gem, TAKEN_GEMS);
                self.home.deep.road.shift(p, -2);
                say = format!("{TAKEN_GEMS} GEMS EACH, FROM THE LOST. FAVOUR WITH {people} -2.");
                self.sounds.push("spoil_pickup");
            }
            (Kind::Patrol, _) => {
                fight = Some(PURSUERS);
                say = "SOMETHING COMES UP THE ROAD AFTER THEM.".into();
            }
            (Kind::Caravan, k) => {
                let k = k - 1;
                match k {
                    0 if self.helped_errand(floor, Errand::Gather).is_some() => {
                        self.pay(id, GIFT);
                        self.help_errand(floor, Errand::Gather, 1);
                        say = format!("YOU BUY A {relic} OFF THE BEAST'S BACK. ERRAND +1.");
                    }
                    0 => {
                        self.pay(id, GIFT);
                        if deep.cargo.len() < MOST_CARGO {
                            deep.cargo.push(Cargo { parcel: Parcel::Gift, from: p });
                        }
                        say = format!("YOU TAKE A GIFT OF {relic}S FOR {town}. IT REACHES THEM WHEN YOU DO.");
                    }
                    1 => {
                        self.pay(id, DRAUGHT);
                        self.mend_all();
                        say = "A DRAUGHT OF DEEP-HONEY FOR EVERY KNIGHT: HALF THEIR HEALTH BACK.".into();
                    }
                    _ => {
                        let price = Spoils(BTreeMap::from([(Spoil::Gem, 1)]));
                        if let Some(h) = self.players.get_mut(&id) {
                            h.carried.take(&price);
                            h.carried.add(Spoil::Gold, Spoil::Gem.worth());
                        }
                        say = format!("THE TRADER WEIGHS YOUR GEM AND PAYS {} GOLD.", Spoil::Gem.worth());
                        self.sounds.push("gold");
                    }
                }
                m.sold |= 1 << k;
                self.sounds.push("card_pickup");
            }
            (Kind::Hermit, k) => {
                if k - 1 == m.key {
                    self.mend_all();
                    self.each(Spoil::Gem, RIDDLE_GEMS);
                    self.home.deep.road.shift(ahead_people, 1);
                    // The dread at this floor's own stair is already risen.
                    let calmed = deep.stir > 0 && dread::guards(floor).is_none();
                    if calmed {
                        stir(deep, -1);
                    }
                    say = format!("THE LAMP CATCHES. EVERY KNIGHT MENDS HALF THEIR HEALTH AND TAKES {RIDDLE_GEMS} GEMS. HE SENDS A BAT AHEAD: FAVOUR WITH {} +1.", ahead_people.people());
                    if calmed {
                        say.push_str(&format!(" THE DREAD OF {town} IS LESS STIRRED."));
                    }
                    self.sounds.push("wish_grant");
                } else {
                    say = "WRONG. THE LAMP DIES AND THE DARK ANSWERS.".into();
                    let tp = m.tile_point();
                    self.raise_foes(tp, AMBUSH);
                    self.sounds.push("boss_rise");
                }
            }
            (Kind::Captives, 1) => {
                self.each(Spoil::Gold, purse);
                self.each(Spoil::Gem, 1);
                self.home.deep.road.shift(p, 3);
                say = format!("THE CAGE OPENS. {people} PAY {purse} GOLD AND A GEM EACH. FAVOUR WITH {people} +3.");
                self.sounds.push("spoil_pickup");
            }
            (Kind::Captives, 2) => {
                self.home.deep.road.shift(p, 2);
                let dug = self.help_errand(floor, Errand::Gather, 1).is_some();
                say = if dug {
                    format!("THE CAGE OPENS. THEY DIG OUT A {relic} FOR YOU. ERRAND +1. FAVOUR WITH {people} +2.")
                } else {
                    self.mend_all();
                    format!("THE CAGE OPENS. THEY TEND YOUR WOUNDS: EVERY KNIGHT MENDS HALF THEIR HEALTH. FAVOUR WITH {people} +2.")
                };
            }
            (Kind::Captives, _) => {
                self.home.deep.road.shift(p, 1);
                if deep.cargo.len() < MOST_CARGO {
                    deep.cargo.push(Cargo { parcel: Parcel::Word, from: p });
                }
                say = format!("THE CAGE OPENS. THEY GO AHEAD TO {town} WITH WORD OF YOU. FAVOUR WITH {people} +1.");
            }
            (Kind::Shrine, 1) => {
                self.each(Spoil::Gold, HOARD_GOLD);
                self.each(Spoil::Gem, HOARD_GEMS);
                self.home.deep.road.shift(p, -3);
                let before = deep.stir;
                stir(deep, 1);
                let woke = if deep.stir > before { "WAKES STRONGER" } else { "IS AS STIRRED AS IT CAN BE" };
                say = format!("{HOARD_GEMS} GEMS AND {HOARD_GOLD} GOLD EACH. THE WATER SHUDDERS. THE DREAD OF {town} {woke}. FAVOUR WITH {people} -3.");
                self.sounds.push("spoil_pickup");
            }
            (Kind::Shrine, 2) => {
                self.pay(id, OFFERING);
                self.mend_all();
                self.home.deep.road.shift(p, 2);
                let calmed = deep.stir > 0;
                stir(deep, -1);
                say = format!("THE WATER GOES STILL. EVERY KNIGHT MENDS HALF THEIR HEALTH. FAVOUR WITH {people} +2.");
                if calmed {
                    say.push_str(&format!(" THE DREAD OF {town} SETTLES."));
                }
                self.sounds.push("wish_grant");
            }
            (Kind::Shrine, _) => {
                fight = Some(SHORE);
                say = "THE BELL RINGS UNDER THE WATER, AND THINGS COME UP THE SHORE.".into();
            }
            (Kind::Envoy, 1) => {
                if deep.cargo.len() < MOST_CARGO {
                    deep.cargo.push(Cargo { parcel: Parcel::Peace, from: p });
                }
                say = format!("YOU TAKE THE LETTER OF PEACE. IT REACHES {town} WHEN YOU DO.");
                self.sounds.push("card_pickup");
            }
            (Kind::Envoy, 2) => {
                self.each(Spoil::Gold, BOUNTY);
                if deep.cargo.len() < MOST_CARGO {
                    deep.cargo.push(Cargo { parcel: Parcel::Challenge, from: p });
                }
                say = format!("{BOUNTY} GOLD EACH. THE CHALLENGE REACHES {town} WHEN YOU DO.");
                self.sounds.push("gold");
            }
            (Kind::Envoy, _) => {
                self.home.deep.road.shift(p, -1);
                say = format!("THE ENVOY GOES BACK THE WAY THEY CAME. FAVOUR WITH {people} -1.");
            }
        }
        // A caravan stays open until its wares are gone; everything else is
        // settled, or on to its fight.
        m.choice = Some(pick);
        if m.kind == Kind::Caravan {
            m.choice = None;
            if m.sold == 0b111 {
                m.stage = Stage::Settled;
                m.at = Some(self.tick);
            }
        } else if let Some(n) = fight {
            m.foes = self.raise_foes(m.tile_point(), n);
            m.stage = Stage::Fighting;
            m.talks.clear();
            self.sounds.push("boss_rise");
        } else {
            m.stage = Stage::Settled;
            m.at = Some(self.tick);
            m.talks.clear();
        }
        // An errand that finished says so too.
        let finished = self.found.as_ref().filter(|f| f.0 == self.tick && f.2.starts_with("ERRAND DONE")).map(|f| f.2.clone());
        if let Some(done) = finished {
            say = format!("{say} {done}");
        }
        self.found = Some((self.tick, 0, say));
        self.cues.push("deep_road".into());
    }

    /// A fight a choice began is won: what it promised is paid.
    fn prize(&mut self, deep: &mut Descent, m: &mut Meeting, pick: u8) {
        let floor = deep.floor;
        let p = m.people;
        let people = p.people();
        let (_, relic, _) = p.sign_words();
        let say = match (m.kind, pick) {
            (Kind::Patrol, _) => {
                let purse = purse(floor);
                self.each(Spoil::Gold, purse);
                self.home.deep.road.shift(p, 3);
                format!("THE ROAD IS CLEAR. {people} PAY {purse} GOLD EACH. FAVOUR WITH {people} +3.")
            }
            _ => {
                self.each(Spoil::Gem, 1);
                self.home.deep.road.shift(p, 1);
                let found = self.help_errand(floor, Errand::Gather, 1).is_some();
                if found {
                    format!("THE SHORE IS QUIET. A GEM EACH, AND A {relic} FROM THE BELL'S FOOT. ERRAND +1. FAVOUR WITH {people} +1.")
                } else {
                    format!("THE SHORE IS QUIET. A GEM EACH, FROM THE BELL'S FOOT. FAVOUR WITH {people} +1.")
                }
            }
        };
        m.stage = Stage::Settled;
        m.at = Some(self.tick);
        m.talks.clear();
        self.sounds.push("wish_grant");
        self.found = Some((self.tick, 0, say));
    }

    /// The party comes down into the village on floor `floor`: what it
    /// carried is delivered, its people's regard read, the stir the road
    /// gave the dread spent. Returns the steps of hope the star brings
    /// beyond its own, whether the star's own light is held back, and what
    /// the village says.
    pub(super) fn road_news(&mut self, floor: u32, culture: Culture, lit: bool) -> (u8, bool, String) {
        let Some(mut deep) = self.deep.take() else {
            return (0, false, String::new());
        };
        let town = culture.town(chapter(floor));
        let cargo = std::mem::take(&mut deep.cargo);
        deep.stir = 0;
        self.deep = Some(deep);
        let mut boon = 0u8;
        let mut lines: Vec<String> = Vec::new();
        for c in cargo {
            let (_, relic, _) = c.from.sign_words();
            match c.parcel {
                Parcel::Gift => {
                    self.home.deep.road.shift(culture, 1);
                    boon += 1;
                    lines.push(if lit { format!("YOUR GIFT OF {relic}S REACHES {town}.") } else { format!("YOUR GIFT OF {relic}S LIGHTS {town} A STEP.") });
                }
                Parcel::Word => {
                    self.home.deep.road.shift(culture, 1);
                    boon += 1;
                    lines.push(if lit { format!("THE WORD YOU SENT REACHES {town}.") } else { format!("THE WORD YOU SENT LIGHTS {town} A STEP.") });
                }
                Parcel::Peace => {
                    self.home.deep.road.shift(culture, 1);
                    self.home.deep.road.shift(c.from, 1);
                    boon += 1;
                    lines.push(if lit {
                        format!("THE LETTER OF PEACE FROM {} IS READ IN {town}.", c.from.people())
                    } else {
                        format!("THE LETTER OF PEACE FROM {} IS READ IN {town}, AND LIGHTS IT A STEP.", c.from.people())
                    });
                }
                Parcel::Challenge => {
                    self.home.deep.road.shift(culture, -2);
                    self.home.deep.road.shift(c.from, 1);
                    lines.push(format!("THE CHALLENGE FROM {} IS READ IN {town}. IT IS NOT TAKEN KINDLY.", c.from.people()));
                }
            }
        }
        let tier = self.home.deep.road.tier(culture);
        let cold = tier == Tier::Wary;
        if tier == Tier::Friend {
            if !lit {
                boon += 1;
            }
            lines.push(format!("{town} KNOWS YOUR NAME FROM THE ROAD."));
        } else if cold {
            lines.push(format!("{town} KEEPS ITS LAMPS LOW FOR YOU."));
        }
        let news: String = lines.join(" ").chars().take(400).collect();
        (boon.min(3), cold && !lit, news)
    }

    /// What a village's elder says first of the party, if its people have
    /// made up their minds about it on the road.
    pub fn road_greeting(&self, culture: Culture) -> &'static str {
        match self.home.deep.road.tier(culture) {
            Tier::Friend => "WE KNOW YOU FROM THE ROAD, STAR-KNIGHT. ",
            Tier::Wary => "THE ROAD HAS TOLD US OF YOU, AND NOT KINDLY. ",
            Tier::Known => "",
        }
    }

    /// What the tracker says of the road on this floor, a line each.
    pub fn road_notes(&self) -> Vec<String> {
        let Some(d) = self.deep.as_ref().filter(|d| d.village.is_none()) else {
            return Vec::new();
        };
        let mut notes = Vec::new();
        if let Some(m) = d.meeting.as_ref().filter(|m| !m.settled()) {
            notes.push(format!("ON THIS FLOOR: {}", m.kind.name()));
        }
        let (culture, town) = self.town_ahead(d.floor);
        for c in &d.cargo {
            let what = match c.parcel {
                Parcel::Gift => "A GIFT",
                Parcel::Word => "WORD",
                Parcel::Peace => "A PEACE LETTER",
                Parcel::Challenge => "A CHALLENGE",
            };
            notes.push(format!("CARRYING: {what} FOR {town}"));
        }
        if d.stir > 0 {
            notes.push(format!("THE DREAD OF {town} IS STIRRED"));
        }
        match self.home.deep.road.tier(culture) {
            Tier::Friend => notes.push(format!("{town}: FRIENDS")),
            Tier::Wary => notes.push(format!("{town}: WARY")),
            Tier::Known => {}
        }
        notes
    }
}

#[cfg(test)]
#[path = "../../../tests/rules/deep_road__tests.rs"]
mod tests;
