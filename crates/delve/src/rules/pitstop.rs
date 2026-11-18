//! The Sanctuary's privy, run like a pit stop.
//!
//! Every floor the party needs the privy: one knight's visit counts for
//! all. Leaving a floor with nobody having gone costs each knight a quarter
//! of their health (never the last point). Beating the whole
//! Delve without once going is the Great Unwashed.
//!
//! The visit has six stages, each against the clock, and the clock is the
//! score:
//!
//! 1. **Strip.** Three straps hold the armour on. A marker sweeps a bar; hit
//!    an action key (fire, bomb, guard or sword) with it in the green. A
//!    clean hit lets the strap go at once, a loose one costs a moment, a
//!    miss locks the hands for longer. The marker quickens each strap.
//! 2. **Run in.** In smallclothes, to the privy's door.
//! 3. **Choose.** Left for a pee, right for a poop. The poop takes longer
//!    and pays more. (A choice has to wait for the stick to be let go of, so
//!    a knight walking in is not choosing by accident.)
//! 4. **Inside.** The door shuts; OCCUPIED.
//! 5. **Run out.** Cross the chequered line for a burst and a split time.
//! 6. **Dress.** The armour flies back on as it came off: one bar for both
//!    (the operator, 2026-10-28: "just one bar game, not two ... on and off
//!    at the same skillshot"), then a short job-quality celebration.
//!
//! Two clear stalls run independently; further knights undo their straps
//! while hopping in their own queue spots. Admission is oldest-first per
//! stall, with seat order breaking ties. Animation ages use simulation ticks.
//! New saved fields default when reading stops from older versions.
//!
//! The total is kept as the knight's time; the party's best sit on the
//! board beside the privy. A quick stop earns a damage buff, a poop a
//! speed buff, and either mends and refills mana as the privy always has.

use super::*;

pub const STRAPS: u8 = 3;
pub const PEE_TICKS: u32 = 36;
pub const POOP_TICKS: u32 = 96;
pub const RUN_TICKS: u32 = 24;
pub const CHOOSE_WAIT: u32 = 5 * HZ;
pub const PATIENCE: u32 = 8 * HZ;
pub const PERFECT_BAND: f32 = 0.07;
pub const GOOD_BAND: f32 = 0.18;
const LOCK_GOOD: u32 = 6;
const LOCK_MISS: u32 = 16;
pub const QUICK_PEE: u32 = 8 * HZ;
pub const QUICK_POOP: u32 = 11 * HZ;
pub const SPEED_TICKS: u32 = 12 * HZ;
pub const DAMAGE_TICKS: u32 = 10 * HZ;
pub const BUFF: u32 = 15;
pub const MEND: u32 = 10;
pub const POOP_MEND: u32 = 10;
pub const UNWASHED_COST: u32 = 25;
const BOX_DISTANCE: f32 = 4.6;
pub const FLIGHT_TICKS: u32 = 18;
pub const FINISH_PROGRESS: f32 = 0.45;
pub const RESULT_TICKS: u32 = 3 * HZ;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Strip,
    RunIn,
    Choose,
    Inside,
    RunOut,
    Dress,
}

impl Stage {
    pub const ALL: [Stage; 6] = [
        Stage::Strip,
        Stage::RunIn,
        Stage::Choose,
        Stage::Inside,
        Stage::RunOut,
        Stage::Dress,
    ];

    pub fn index(self) -> usize {
        Stage::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    pub fn name(self) -> &'static str {
        match self {
            Stage::Strip => "ARMOUR OFF",
            Stage::RunIn => "RUN IN",
            Stage::Choose => "CHOOSE",
            Stage::Inside => "INSIDE",
            Stage::RunOut => "RUN OUT",
            Stage::Dress => "ARMOUR ON",
        }
    }

    fn straps(self) -> bool {
        matches!(self, Stage::Strip)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grade {
    Clean,
    Loose,
    Missed,
}

impl Grade {
    pub fn word(self) -> &'static str {
        match self {
            Grade::Clean => "CLEAN!",
            Grade::Loose => "LOOSE",
            Grade::Missed => "FUMBLED",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Royal,
    Fresh,
    Whiffy,
    Disaster,
}

impl Quality {
    pub fn word(self) -> &'static str {
        match self {
            Self::Royal => "ROYAL FLUSH",
            Self::Fresh => "FRESH START",
            Self::Whiffy => "BIT WHIFFY",
            Self::Disaster => "PAPER TRAIL",
        }
    }
}

pub fn quality(total: u32, poop: bool, loose: u32, misses: u32) -> Quality {
    let quick = if poop { QUICK_POOP } else { QUICK_PEE };
    if total <= quick && misses == 0 && loose <= 1 {
        Quality::Royal
    } else if total <= quick + 3 * HZ && misses <= 2 && loose <= 4 {
        Quality::Fresh
    } else if total <= quick + 8 * HZ && misses <= 5 {
        Quality::Whiffy
    } else {
        Quality::Disaster
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Result {
    pub quality: Quality,
    pub total: u32,
    pub left: u32,
    #[serde(default)]
    pub party_record: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stop {
    pub stage: Stage,
    pub t: u32,
    pub total: u32,
    pub straps: u8,
    pub lock: u32,
    pub phase: f32,
    pub last: Option<(Grade, u32)>,
    pub times: [u32; 6],
    pub poop: bool,
    pub ready: bool,
    pub pressed: bool,
    pub from: (f32, f32),
    pub door: (f32, f32),
    #[serde(default)]
    pub lane: u8,
    #[serde(default)]
    pub waiting: bool,
    #[serde(default)]
    pub tossed: [Option<u32>; 3],
    #[serde(default)]
    pub fitted: [Option<u32>; 3],
    #[serde(default)]
    pub loose: u32,
    #[serde(default)]
    pub misses: u32,
    #[serde(default)]
    pub finish: Option<u32>,
}

impl Stop {
    fn begin(from: (f32, f32), door: (f32, f32)) -> Stop {
        Stop {
            stage: Stage::Strip,
            t: 0,
            total: 0,
            straps: 0,
            lock: 0,
            phase: 0.0,
            last: None,
            times: [0; 6],
            poop: false,
            ready: false,
            pressed: false,
            from,
            door,
            lane: 0,
            waiting: false,
            tossed: [None; 3],
            fitted: [None; 3],
            loose: 0,
            misses: 0,
            finish: None,
        }
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn finish_point(&self) -> (f32, f32) {
        (
            self.door.0 + (self.from.0 - self.door.0) * FINISH_PROGRESS,
            self.door.1 + (self.from.1 - self.door.1) * FINISH_PROGRESS,
        )
    }

    pub fn marker(&self) -> f32 {
        if self.phase < 1.0 {
            self.phase
        } else {
            2.0 - self.phase
        }
    }

    pub fn period(straps: u8) -> f32 {
        [36.0, 30.0, 25.0][usize::from(straps.min(2))]
    }

    pub fn clock(&self) -> u32 {
        self.total
    }

    pub fn progress(&self) -> f32 {
        match self.stage {
            Stage::Inside => self.t as f32 / self.stay() as f32,
            Stage::Choose => self.t as f32 / CHOOSE_WAIT as f32,
            Stage::RunIn | Stage::RunOut => self.t as f32 / RUN_TICKS as f32,
            _ => self.straps as f32 / f32::from(STRAPS),
        }
        .clamp(0.0, 1.0)
    }

    pub fn stay(&self) -> u32 {
        if self.poop { POOP_TICKS } else { PEE_TICKS }
    }

    pub fn bare(&self) -> Option<u8> {
        Some(match self.stage {
            Stage::Strip => match self.straps {
                0 => return None,
                1 => 1,
                _ => 2,
            },
            Stage::Dress => match self.straps {
                0 | 1 => 2,
                2 => 1,
                _ => return None,
            },
            _ => 2,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pit {
    pub stop: Option<Stop>,
    pub visits: u32,
    pub speed: u32,
    pub damage: u32,
    #[serde(default)]
    pub result: Option<Result>,
}

impl Pit {
    pub fn tick(&mut self) {
        if self.result.is_none() {
            self.speed = self.speed.saturating_sub(1);
            self.damage = self.damage.saturating_sub(1);
        }
        if let Some(result) = &mut self.result {
            result.left = result.left.saturating_sub(1);
            if result.left == 0 {
                self.result = None;
            }
        }
    }

    pub fn pace(&self) -> f32 {
        if self.speed > 0 {
            1.0 + BUFF as f32 / 100.0
        } else {
            1.0
        }
    }

    pub fn hit(&self) -> u32 {
        if self.damage > 0 { BUFF } else { 0 }
    }

    pub fn bare(&self) -> Option<u8> {
        self.stop.as_ref().and_then(Stop::bare)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub seat: u32,
    pub name: String,
    pub best: u32,
    pub last: u32,
    pub stops: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Board {
    pub entries: Vec<Entry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Record {
    None,
    Personal,
    Party,
}

impl Board {
    pub fn ranked(&self) -> Vec<&Entry> {
        let mut all: Vec<&Entry> = self.entries.iter().collect();
        all.sort_by_key(|e| (e.best, e.seat));
        all
    }

    pub fn best(&self) -> Option<u32> {
        self.entries.iter().map(|e| e.best).min()
    }

    pub fn set(&mut self, seat: u32, name: &str, ticks: u32) -> Record {
        let party = self.best();
        match self.entries.iter_mut().find(|e| e.seat == seat) {
            Some(entry) => {
                entry.last = ticks;
                entry.stops += 1;
                entry.name = name.to_string();
                let faster = ticks < entry.best;
                if faster {
                    entry.best = ticks;
                }
                match (faster, party) {
                    (true, Some(p)) if ticks < p => Record::Party,
                    (true, None) => Record::Party,
                    (true, _) => Record::Personal,
                    _ => Record::None,
                }
            }
            None => {
                self.entries.push(Entry {
                    seat,
                    name: name.to_string(),
                    best: ticks,
                    last: ticks,
                    stops: 1,
                });
                if party.is_none_or(|p| ticks < p) {
                    Record::Party
                } else {
                    Record::Personal
                }
            }
        }
    }
}

pub(super) enum Event {
    Stage(Stage),
    Press(Grade),
    Finish,
    Done {
        total: u32,
        poop: bool,
        quality: Quality,
    },
}

pub fn seconds(ticks: u32) -> String {
    format!("{}.{:02}", ticks / HZ, (ticks % HZ) * 100 / HZ)
}

pub fn grade(marker: f32) -> Grade {
    let off = (marker - 0.5).abs();
    if off <= PERFECT_BAND {
        Grade::Clean
    } else if off <= GOOD_BAND {
        Grade::Loose
    } else {
        Grade::Missed
    }
}

fn is_press(input: &Input) -> bool {
    input.fire || input.interact || input.bomb || input.dash || input.swing
}

impl Room {
    pub fn privy_stalls(&self) -> Vec<(f32, f32)> {
        let Some(door) = self.privy() else {
            return Vec::new();
        };
        let mut stalls = vec![door];
        let other = (door.0 + 4.0, door.1);
        let floor = |x: f32, y: f32| {
            self.tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32) == Tile::Floor
        };
        if [-1.0, 1.0].into_iter().all(|dx| {
            [-3.0, -1.0, 1.0, 3.0, 5.0]
                .into_iter()
                .all(|dy| floor(other.0 + dx, other.1 + dy))
        }) {
            stalls.push(other);
        }
        stalls
    }

    pub fn pit_box(&self) -> Option<(f32, f32)> {
        let (px, py) = self.privy()?;
        let clear = |x: f32, y: f32| {
            let grid = Grid {
                room: self,
                barred: true,
            };
            grid.clear(x, y, HERO_RADIUS + 0.2, Mover::Hero)
        };
        [
            (0.0, BOX_DISTANCE),
            (3.0, BOX_DISTANCE - 0.6),
            (-3.0, BOX_DISTANCE - 0.6),
            (0.0, 3.4),
            (4.0, 0.0),
            (0.0, 2.2),
        ]
        .into_iter()
        .map(|(dx, dy)| (px + dx, py + dy))
        .find(|&(x, y)| clear(x, y))
    }
}

pub(super) fn begin(hero: &mut Hero, from: (f32, f32), door: (f32, f32)) {
    hero.pit.result = None;
    hero.pit.stop = Some(Stop::begin(from, door));
    (hero.x, hero.y) = from;
    hero.dash_ticks = 0;
    hero.shielding = false;
    hero.walling = false;
    hero.aim_x = 0.0;
    hero.aim_y = -1.0;
}

pub(super) fn step(hero: &mut Hero, input: Input, admitted: bool, events: &mut Vec<Event>) {
    let Some(stop) = hero.pit.stop.as_mut() else {
        hero.privy = hero.privy.saturating_sub(1);
        return;
    };
    stop.total += 1;
    stop.t += 1;
    stop.lock = stop.lock.saturating_sub(1);
    let press = is_press(&input);
    let edge = press && !stop.pressed;
    stop.pressed = press;
    let mut next = false;
    match stop.stage {
        Stage::Dress => {
            if stop.straps < STRAPS {
                for piece in &mut stop.fitted {
                    piece.get_or_insert(stop.total);
                }
                stop.straps = STRAPS;
            }
            next = stop
                .fitted
                .iter()
                .flatten()
                .all(|&t| stop.total.saturating_sub(t) >= FLIGHT_TICKS);
        }
        Stage::Strip => {
            stop.phase = (stop.phase + 2.0 / Stop::period(stop.straps)).rem_euclid(2.0);
            if edge && stop.lock == 0 && stop.straps < STRAPS {
                let g = grade(stop.marker());
                stop.last = Some((g, stop.total));
                events.push(Event::Press(g));
                if g != Grade::Missed {
                    let piece = if stop.stage == Stage::Strip {
                        stop.straps
                    } else {
                        STRAPS - 1 - stop.straps
                    } as usize;
                    if stop.stage == Stage::Strip {
                        stop.tossed[piece] = Some(stop.total);
                    } else {
                        stop.fitted[piece] = Some(stop.total);
                    }
                }
                match g {
                    Grade::Clean => stop.straps += 1,
                    Grade::Loose => {
                        stop.loose += 1;
                        stop.straps += 1;
                        stop.lock = LOCK_GOOD;
                    }
                    Grade::Missed => {
                        stop.misses += 1;
                        stop.lock = LOCK_MISS;
                    }
                }
            } else if stop.t >= PATIENCE && stop.straps < STRAPS {
                stop.last = Some((Grade::Missed, stop.total));
                stop.misses += 6;
                for piece in 0..3 {
                    if stop.stage == Stage::Strip {
                        stop.tossed[piece].get_or_insert(stop.total);
                    } else {
                        stop.fitted[piece].get_or_insert(stop.total);
                    }
                }
                stop.straps = STRAPS;
            }
            stop.waiting = stop.stage == Stage::Strip && stop.straps >= STRAPS && !admitted;
            next = stop.straps >= STRAPS
                && if stop.stage == Stage::Strip {
                    admitted
                        && stop
                            .tossed
                            .iter()
                            .flatten()
                            .all(|&t| stop.total.saturating_sub(t) >= FLIGHT_TICKS)
                } else {
                    stop.fitted
                        .iter()
                        .flatten()
                        .all(|&t| stop.total.saturating_sub(t) >= FLIGHT_TICKS)
                };
        }
        Stage::RunIn | Stage::RunOut => {
            let k = (stop.t as f32 / RUN_TICKS as f32).clamp(0.0, 1.0);
            let (a, b) = if stop.stage == Stage::RunIn {
                (stop.from, stop.door)
            } else {
                (stop.door, stop.from)
            };
            (hero.x, hero.y) = (a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k);
            let (dx, dy) = unit(b.0 - a.0, b.1 - a.1);
            (hero.aim_x, hero.aim_y) = (dx, dy);
            if stop.stage == Stage::RunOut && stop.finish.is_none() && k >= FINISH_PROGRESS {
                stop.finish = Some(stop.total);
                events.push(Event::Finish);
            }
            next = stop.t >= RUN_TICKS;
        }
        Stage::Choose => {
            let side = i32::from(input.move_x) + i32::from(input.aim_x);
            if side == 0 {
                stop.ready = true;
            }
            if stop.ready && side != 0 {
                stop.poop = side > 0;
                next = true;
            } else if stop.t >= CHOOSE_WAIT {
                next = true;
            }
        }
        Stage::Inside => {
            hero.privy = stop.stay().saturating_sub(stop.t).max(1);
            next = stop.t >= stop.stay();
        }
    }
    if !next {
        return;
    }
    stop.times[stop.stage.index()] = stop.t;
    let after = match stop.stage {
        Stage::Strip => Some(Stage::RunIn),
        Stage::RunIn => Some(Stage::Choose),
        Stage::Choose => Some(Stage::Inside),
        Stage::Inside => Some(Stage::RunOut),
        Stage::RunOut => Some(Stage::Dress),
        Stage::Dress => None,
    };
    match after {
        Some(stage) => {
            stop.stage = stage;
            stop.waiting = false;
            stop.t = 0;
            stop.straps = 0;
            stop.lock = 0;
            stop.phase = 0.0;
            if stage == Stage::Inside {
                hero.privy = stop.stay();
            } else {
                hero.privy = 0;
            }
            if stage == Stage::RunOut {
                (hero.x, hero.y) = stop.door;
            }
            events.push(Event::Stage(stage));
        }
        None => {
            events.push(Event::Done {
                total: stop.total,
                poop: stop.poop,
                quality: quality(stop.total, stop.poop, stop.loose, stop.misses),
            });
            hero.pit.stop = None;
            hero.privy = 0;
        }
    }
}

pub const WALL: usize = 8;
const SCRAWLS: [&str; 6] = [
    "{} WAS HERE",
    "{} SAT HERE AND PONDERED",
    "BEWARE THE SOUP - {}",
    "{} + THIS PRIVY 4EVER",
    "{} WAS FASTER",
    "HONK - {}",
];

impl Run {
    fn pits_scrawled(&self) -> usize {
        self.home.privy_wall.len()
    }

    pub(super) fn pit_hud(&self, id: u32, keys: &hud::Keys) -> Option<hud::PitHud> {
        let stop = self.players.get(&id)?.pit.stop.as_ref()?;
        let strap = stop.stage.straps();
        let hint = match stop.stage {
            Stage::Strip if stop.waiting => "YOUR TURN SOON".to_string(),
            Stage::Strip => format!("{} ON GREEN", keys.fire),
            Stage::Choose => "PICK A SIDE".to_string(),
            Stage::Inside => "OCCUPIED".to_string(),
            _ => "GO GO".to_string(),
        };
        Some(hud::PitHud {
            stage: stop.stage.name().to_string(),
            straps: (stop.straps, if strap { STRAPS } else { 0 }),
            total: seconds(stop.clock()),
            stage_time: seconds(stop.t),
            marker: strap.then(|| stop.marker()),
            zone: (GOOD_BAND, PERFECT_BAND),
            hint,
            board: self
                .pits
                .ranked()
                .into_iter()
                .take(4)
                .map(|e| (e.name.clone(), seconds(e.best)))
                .collect(),
        })
    }

    pub(super) fn pit_admissions(&self) -> std::collections::BTreeSet<u32> {
        let mut owners = BTreeMap::new();
        for (&id, hero) in &self.players {
            if hero.hp == 0 || hero.stone {
                continue;
            }
            let Some(s) = &hero.pit.stop else {
                continue;
            };
            if s.stage == Stage::Dress {
                continue;
            }
            let priority = (s.stage != Stage::Strip, s.total, std::cmp::Reverse(id));
            let entry = owners.entry(s.lane).or_insert((priority, id));
            if priority > entry.0 {
                *entry = (priority, id);
            }
        }
        owners.values().map(|&(_, id)| id).collect()
    }

    pub(super) fn visit_privy(&mut self) {
        if self.phase != Phase::Exploring {
            return;
        }
        let stalls = self.room().privy_stalls();
        if stalls.is_empty() {
            return;
        }
        let Some(spot) = self.room().pit_box() else {
            return;
        };
        let (depth, here) = (self.dungeon.depth, self.at);
        let mut lanes = vec![0usize; stalls.len()];
        let mut occupied = Vec::new();
        for h in self.players.values() {
            if let Some(s) = &h.pit.stop {
                lanes[usize::from(s.lane).min(stalls.len() - 1)] += 1;
                occupied.push(s.from);
            }
        }
        let grid = Grid {
            room: self.room(),
            barred: true,
        };
        let candidates: Vec<_> = (0..8)
            .flat_map(|row| {
                (0..stalls.len())
                    .map(move |lane| (spot.0 + lane as f32 * 4.0, spot.1 + row as f32 * 2.6))
            })
            .filter(|&(x, y)| grid.clear(x, y, HERO_RADIUS + 0.2, Mover::Hero))
            .collect();
        let mut went = false;
        for hero in self.players.values_mut() {
            if hero.hp > 0
                && !hero.stone
                && hero.privy == 0
                && hero.pit.stop.is_none()
                && hero.relieved != Some((depth, here))
                && stalls
                    .iter()
                    .any(|&(px, py)| (hero.x - px).hypot(hero.y - py) < PRIVY_REACH)
            {
                let lane = (0..lanes.len()).min_by_key(|&i| (lanes[i], i)).unwrap();
                let Some(&from) = candidates.iter().find(|p| !occupied.contains(p)) else {
                    continue;
                };
                begin(hero, from, stalls[lane]);
                hero.pit.stop.as_mut().unwrap().lane = lane as u8;
                lanes[lane] += 1;
                occupied.push(from);
                went = true;
            }
        }
        if went {
            self.sounds.push("door_seal");
            self.cues.push("privy".into());
        }
    }

    pub(super) fn pit_events(&mut self, events: Vec<(u32, Event)>) {
        let (depth, here) = (self.dungeon.depth, self.at);
        for (id, event) in events {
            match event {
                Event::Stage(Stage::Inside) => self.sounds.push("door_seal"),
                Event::Stage(Stage::RunOut) => self.sounds.push("door_open"),
                Event::Stage(_) => self.sounds.push("roll"),
                Event::Press(Grade::Clean) => self.sounds.push("play_card"),
                Event::Press(Grade::Loose) => self.sounds.push("card_pickup"),
                Event::Press(Grade::Missed) => self.sounds.push("mana_empty"),
                Event::Finish => {
                    self.sounds.push("play_card");
                    self.cues.push("privy_finish".into());
                }
                Event::Done {
                    total,
                    poop,
                    quality,
                    ..
                } => {
                    let Some(hero) = self.players.get_mut(&id) else {
                        continue;
                    };
                    hero.mana = MAX_MANA;
                    let mend = MEND + if poop { POOP_MEND } else { 0 };
                    hero.mend(mend, super::stakes::Mend::Rest);
                    hero.relieved = Some((depth, here));
                    hero.pit.visits += 1;
                    let name = hero.shown_name();
                    let record = self.pits.set(id, &name, total);
                    hero.pit.result = Some(Result {
                        quality,
                        total,
                        left: RESULT_TICKS,
                        party_record: record == Record::Party,
                    });
                    let quick = total <= if poop { QUICK_POOP } else { QUICK_PEE };
                    if poop {
                        hero.pit.speed = SPEED_TICKS;
                    }
                    if quick {
                        hero.pit.damage = DAMAGE_TICKS;
                    }
                    let first: String = name
                        .split_whitespace()
                        .last()
                        .unwrap_or("A KNIGHT")
                        .to_uppercase()
                        .chars()
                        .take(10)
                        .collect();
                    let scrawl = SCRAWLS[(self.pits_scrawled() + id as usize) % SCRAWLS.len()]
                        .replace("{}", &first);
                    let wall = &mut self.home.privy_wall;
                    wall.push(scrawl);
                    if wall.len() > WALL {
                        wall.remove(0);
                    }
                    let time = seconds(total);
                    let line = match record {
                        Record::Party => format!("{name}: {time} at the privy, a party record"),
                        Record::Personal => format!("{name}: {time} at the privy, a personal best"),
                        Record::None if quick => {
                            format!("{name}: {time} at the privy, a quick one")
                        }
                        Record::None => format!("{name}: {time} at the privy"),
                    };
                    self.found = Some((self.tick, 0, line));
                    self.sounds.push("door_open");
                    self.cues.push(if record == Record::None {
                        "privy_done".into()
                    } else {
                        "privy_record".into()
                    });
                }
            }
        }
    }

    pub(super) fn unwashed_toll(&mut self) {
        let depth = self.dungeon.depth;
        if self.players.values().any(|h| h.relieved.map(|(d, _)| d) == Some(depth)) {
            return;
        }
        let mut docked = false;
        for hero in self.players.values_mut() {
            if hero.hp == 0 || hero.stone {
                continue;
            }
            if hero.relieved.map(|(d, _)| d) != Some(depth) {
                if hero.scrubbed {
                    hero.scrubbed = false;
                    continue;
                }
                let cost = hero.max_hp * UNWASHED_COST / 100;
                hero.hp = hero.hp.saturating_sub(cost).max(1);
                docked = true;
            }
        }
        if docked {
            self.cues.push("unwashed_toll".into());
            self.found = Some((
                self.tick,
                0,
                "Down the stairs unrelieved: a quarter of the health goes".into(),
            ));
        }
    }

    pub(super) fn unwashed_check(&mut self) {
        if self.players.values().any(|h| h.hp > 0 && h.pit.visits == 0) {
            self.notice("unwashed");
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_pitstop__tests.rs"]
mod tests;
