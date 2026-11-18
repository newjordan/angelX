//! Dame Fortune's Wheel: the delve's game modes.
//!
//! East of the Undercroft is Fortune's hall, and in it her wheel — the
//! Rota Fortunae that Arthur dreamt of, run the way the Delve's Herald runs
//! the paths below: a different road with each turn. A knight at its lever holds E
//! and the wheel spins once a delve; where it stops is how the delve goes.
//! More wedges open as Fortune's ladder is bought.
//!
//! Every mode is a rule over the same dungeon: a darker one, bigger
//! monsters, floors that fall, a horde, a gauntlet of guardians. The riskier
//! ones pay more spoils.

use super::home::Station;
use super::*;
use crate::realm::Spoils;

pub const WIND: u32 = 12;
pub const RUN: u32 = 104;
pub const HANG: u32 = 26;
pub const DROP: u32 = 10;
pub const SPIN_TICKS: u32 = WIND + RUN + HANG + DROP;
const BACK: f32 = 0.28;
const NEAR: f32 = 0.10;
const CREEP: f32 = 0.04;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Luck {
    Plain,
    Fair,
    Grand,
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    #[serde(alias = "sponsors")]
    LongWayDown,
    LightsOut,
    Collapse,
    GiantsFeast,
    HoldTheStair,
    MimicFair,
    GlassJaw,
    Gauntlet,
    Turbo,
    RuneRush,
    AllRandom,
    Ironman,
    HollowWalls,
    OpenFloors,
}

impl Mode {
    pub const WHEEL: [Mode; 14] = [
        Mode::LongWayDown,
        Mode::LightsOut,
        Mode::Collapse,
        Mode::GiantsFeast,
        Mode::HoldTheStair,
        Mode::MimicFair,
        Mode::GlassJaw,
        Mode::Gauntlet,
        Mode::Turbo,
        Mode::RuneRush,
        Mode::AllRandom,
        Mode::Ironman,
        Mode::HollowWalls,
        Mode::OpenFloors,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Mode::LongWayDown => "The Long Way Down",
            Mode::LightsOut => "Lights Out",
            Mode::Collapse => "The Collapse",
            Mode::GiantsFeast => "Giant's Feast",
            Mode::HoldTheStair => "Hold the Stair",
            Mode::MimicFair => "Mimic Fair",
            Mode::GlassJaw => "Glass Jaw",
            Mode::Gauntlet => "Gauntlet of Guardians",
            Mode::Turbo => "Turbo",
            Mode::RuneRush => "Rune Rush",
            Mode::AllRandom => "All Random",
            Mode::Ironman => "Ironman",
            Mode::HollowWalls => "Hollow Walls",
            Mode::OpenFloors => "Open Floors",
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Mode::LongWayDown => "long_way_down",
            Mode::LightsOut => "lights_out",
            Mode::Collapse => "collapse",
            Mode::GiantsFeast => "giants_feast",
            Mode::HoldTheStair => "hold_the_stair",
            Mode::MimicFair => "mimic_fair",
            Mode::GlassJaw => "glass_jaw",
            Mode::Gauntlet => "gauntlet",
            Mode::Turbo => "turbo",
            Mode::RuneRush => "rune_rush",
            Mode::AllRandom => "all_random",
            Mode::Ironman => "ironman",
            Mode::HollowWalls => "hollow_walls",
            Mode::OpenFloors => "open_floors",
        }
    }

    pub fn says(self) -> &'static str {
        match self {
            Mode::LongWayDown => "the delve as it always was",
            Mode::LightsOut => "dark but for lanterns: 1.5x spoils",
            Mode::Collapse => "floors fall in: 2x spoils",
            Mode::GiantsFeast => "giant monsters: 1.5x spoils",
            Mode::HoldTheStair => "a horde a floor: 2x spoils",
            Mode::MimicFair => "a chest in every won room",
            Mode::GlassJaw => "all hits double: 2x spoils",
            Mode::Gauntlet => "straight to the guardians",
            Mode::Turbo => "everything a third faster: 2x spoils",
            Mode::RuneRush => "a rune every fight, and more",
            Mode::AllRandom => "a stranger's ultimate, half charged",
            Mode::Ironman => "no second winds: 3x spoils",
            Mode::HollowWalls => "a vault on every floor",
            Mode::OpenFloors => "no doors between fights: explore",
        }
    }

    pub fn headline(self) -> &'static str {
        match self {
            Mode::LongWayDown => "LONG WAY DOWN",
            Mode::LightsOut => "LIGHTS OUT",
            Mode::Collapse => "COLLAPSE",
            Mode::GiantsFeast => "GIANT'S FEAST",
            Mode::HoldTheStair => "HOLD THE STAIR",
            Mode::MimicFair => "MIMIC FAIR",
            Mode::GlassJaw => "GLASS JAW",
            Mode::Gauntlet => "GAUNTLET",
            Mode::Turbo => "TURBO",
            Mode::RuneRush => "RUNE RUSH",
            Mode::AllRandom => "ALL RANDOM",
            Mode::Ironman => "IRONMAN",
            Mode::HollowWalls => "HOLLOW WALLS",
            Mode::OpenFloors => "OPEN FLOORS",
        }
    }

    pub fn gist(self) -> &'static str {
        match self {
            Mode::LongWayDown => "THE PLAIN ROAD",
            Mode::LightsOut => "LANTERNS ONLY",
            Mode::Collapse => "FLOORS FALL IN",
            Mode::GiantsFeast => "GIANT FOES",
            Mode::HoldTheStair => "HORDES COME UP",
            Mode::MimicFair => "CHESTS THAT BITE",
            Mode::GlassJaw => "ALL HITS DOUBLE",
            Mode::Gauntlet => "GUARDIANS ONLY",
            Mode::Turbo => "A THIRD FASTER",
            Mode::RuneRush => "RUNES EVERY FIGHT",
            Mode::AllRandom => "RANDOM ULTIMATES",
            Mode::Ironman => "NO SECOND WINDS",
            Mode::HollowWalls => "VAULT EVERY FLOOR",
            Mode::OpenFloors => "ONE OPEN MAP",
        }
    }

    pub fn pay(self) -> Option<&'static str> {
        match self.spoils() {
            (3, 2) => Some("1.5X SPOILS"),
            (2, 1) => Some("2X SPOILS"),
            (3, 1) => Some("3X SPOILS"),
            _ => None,
        }
    }

    pub fn luck(self) -> Luck {
        match self {
            Mode::Collapse | Mode::GlassJaw | Mode::HoldTheStair | Mode::Turbo | Mode::Ironman => Luck::Grand,
            Mode::LightsOut
            | Mode::GiantsFeast
            | Mode::MimicFair
            | Mode::RuneRush
            | Mode::AllRandom
            | Mode::HollowWalls => Luck::Fair,
            Mode::LongWayDown | Mode::Gauntlet | Mode::OpenFloors => Luck::Plain,
        }
    }

    pub fn rung(self) -> u8 {
        match self {
            Mode::LongWayDown | Mode::LightsOut | Mode::GiantsFeast | Mode::MimicFair | Mode::OpenFloors => 0,
            Mode::Collapse | Mode::GlassJaw => 1,
            Mode::HoldTheStair | Mode::Gauntlet => 2,
            Mode::Turbo | Mode::RuneRush | Mode::AllRandom => 3,
            Mode::Ironman | Mode::HollowWalls => 4,
        }
    }

    pub fn spoils(self) -> (u32, u32) {
        match self {
            Mode::LightsOut | Mode::GiantsFeast => (3, 2),
            Mode::Collapse | Mode::GlassJaw | Mode::HoldTheStair | Mode::Turbo => (2, 1),
            Mode::LongWayDown
            | Mode::MimicFair
            | Mode::Gauntlet
            | Mode::RuneRush
            | Mode::AllRandom
            | Mode::HollowWalls
            | Mode::OpenFloors => (1, 1),
            Mode::Ironman => (3, 1),
        }
    }

    pub fn collapse_secs(depth: u32) -> u32 {
        120 + 30 * depth
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spin {
    #[serde(default)]
    pub settled: bool,
    pub started: u64,
    pub from: f32,
    #[serde(deserialize_with = "saved_wedge")]
    pub wedge: u8,
    pub turns: u8,
    pub by: u32,
}

fn saved_wedge<'de, D: serde::Deserializer<'de>>(de: D) -> Result<u8, D::Error> {
    let wedge = u8::deserialize(de)?;
    Ok(if usize::from(wedge) < Mode::WHEEL.len() { wedge } else { 0 })
}

impl Spin {
    pub fn angle(&self, tick: u64) -> f32 {
        let n = if self.settled { SPIN_TICKS } else { tick.saturating_sub(self.started).min(u64::from(SPIN_TICKS)) as u32 };
        self.angle_at(n)
    }

    pub fn angle_at(&self, n: u32) -> f32 {
        use crate::motion::ease;
        let w = std::f32::consts::TAU / Mode::WHEEL.len() as f32;
        let target = f32::from(self.turns) * std::f32::consts::TAU + rest_angle(self.wedge);
        if n >= SPIN_TICKS {
            return target;
        }
        let start = self.from - BACK;
        let hang = target - (0.5 + NEAR) * w;
        let (run_at, hang_at, drop_at) = (WIND, WIND + RUN, WIND + RUN + HANG);
        if n < run_at {
            return self.from - BACK * ease::out(n as f32 / WIND as f32);
        }
        if n < hang_at {
            let u = (n - run_at) as f32 / RUN as f32;
            let g = ease::out(u) * ease::both(u / 0.06);
            return start + (hang - start) * g;
        }
        if n < drop_at {
            let h = (n - hang_at) as f32 / HANG as f32;
            return hang + CREEP * w * ease::in_quad(h);
        }
        let crawl = hang + CREEP * w;
        let d = (n - drop_at) as f32 / DROP as f32;
        crawl + (target - crawl) * ease::elastic(d)
    }

    pub fn beat(&self, n: u32) -> Beat {
        match n {
            n if n < WIND => Beat::Wind,
            n if n < WIND + RUN => Beat::Run,
            n if n < WIND + RUN + HANG => Beat::Hang,
            n if n < SPIN_TICKS => Beat::Drop,
            _ => Beat::Landed,
        }
    }

    pub fn speed(&self, n: u32) -> f32 {
        let w = std::f32::consts::TAU / Mode::WHEEL.len() as f32;
        (self.angle_at(n) - self.angle_at(n.saturating_sub(1))) / w
    }

    pub fn pegs(&self, n: u32) -> i32 {
        let w = std::f32::consts::TAU / Mode::WHEEL.len() as f32;
        (self.angle_at(n) / w).floor() as i32
    }

    pub fn clicked(&self, n: u32) -> bool {
        n > 0 && n < SPIN_TICKS && self.pegs(n) != self.pegs(n - 1)
    }

    pub fn neighbour(&self) -> u8 {
        ((usize::from(self.wedge) + 1) % Mode::WHEEL.len()) as u8
    }

    pub fn done(&self, tick: u64) -> bool {
        self.settled || tick.saturating_sub(self.started) >= u64::from(SPIN_TICKS)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Beat {
    Wind,
    Run,
    Hang,
    Drop,
    Landed,
}

pub fn rest_angle(wedge: u8) -> f32 {
    let w = std::f32::consts::TAU / Mode::WHEEL.len() as f32;
    std::f32::consts::TAU - (f32::from(wedge) + 0.5) * w
}

pub const LEVER: (i32, i32, i32, i32) = (11, 8, 2, 1);
pub const FORTUNE_PLATE: (i32, i32, i32, i32) = (16, 8, 2, 1);
pub const COFFER: (i32, i32, i32, i32) = (3, 3, 2, 2);
pub const COFFER_PLATE: (i32, i32, i32, i32) = (3, 5, 2, 1);
pub const WHEEL_STAND: (i32, i32, i32, i32) = (9, 1, 6, 6);
pub const BENCHES: [(i32, i32, i32, i32); 2] = [(3, 10, 7, 2), (14, 10, 7, 2)];
pub const FORTUNE_AT: (f32, f32) = (7.6, 7.2);

fn inside((col, row, w, h): (i32, i32, i32, i32), x: f32, y: f32) -> bool {
    let (c, r) = (x / TILE_UNITS, y / TILE_UNITS);
    c >= col as f32 && c < (col + w) as f32 && r >= row as f32 && r < (row + h) as f32
}

pub fn open_wedges(level: u8) -> Vec<u8> {
    (0..Mode::WHEEL.len() as u8)
        .filter(|&i| Mode::WHEEL[usize::from(i)].rung() <= level)
        .collect()
}

impl Run {
    pub(super) fn tick_fortune(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.room().kind != RoomKind::Fortune {
            return;
        }
        let mut pulled = None;
        let mut asked = Vec::new();
        for (&id, hero) in self.players.iter_mut() {
            let pressing = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .is_some_and(|i| i.interact);
            if !pressing {
                hero.buy_spent = false;
            }
            let standing = hero.hp > 0 && !hero.stone;
            let on_lever = standing && inside(LEVER, hero.x, hero.y);
            let on_coffer = standing && inside(COFFER_PLATE, hero.x, hero.y);
            let on_plate = standing && (inside(FORTUNE_PLATE, hero.x, hero.y) || on_coffer);
            if (on_lever || on_plate) && pressing && !hero.buy_spent {
                hero.buying = hero.buying.saturating_add(1);
                if hero.buying >= home::BUY_HOLD {
                    hero.buying = 0;
                    hero.buy_spent = true;
                    if on_lever {
                        pulled = pulled.or(Some(id));
                    } else {
                        asked.push(home::Order {
                            knight: id,
                            station: if on_coffer {
                                Station::Coffer
                            } else {
                                Station::Wheel
                            },
                        });
                    }
                }
            } else {
                hero.buying = 0;
            }
        }
        self.orders.extend(asked);
        let (fx, fy) = (FORTUNE_AT.0 * TILE_UNITS, FORTUNE_AT.1 * TILE_UNITS);
        let near = self
            .players
            .values()
            .any(|h| h.hp > 0 && !h.stone && (h.x - fx).hypot(h.y - fy) < 4.0);
        const FORTUNE_BIT: u8 = 1 << 7;
        if near && self.greeted & FORTUNE_BIT == 0 {
            self.cues.push("npc:fortune".into());
        }
        self.greeted = if near {
            self.greeted | FORTUNE_BIT
        } else {
            self.greeted & !FORTUNE_BIT
        };
        if let Some(id) = pulled {
            if self.spin.is_some() {
                self.found = Some((self.tick, id, "Fortune spins once a delve".into()));
                self.cues.push("wheel_again".into());
            } else {
                let open = open_wedges(self.home.level(Station::Wheel));
                let wedge = open[self.rng.below(open.len())];
                self.spin = Some(Spin {
                    settled: false,
                    started: self.tick,
                    from: 0.0,
                    wedge,
                    turns: 3 + self.rng.below(2) as u8,
                    by: id,
                });
                self.cues.push("wheel_spin".into());
                self.sounds.push("wheel_spin");
                self.notice("spin");
            }
        }
        if let Some(spin) = self.spin.filter(|s| !s.done(self.tick)) {
            let n = self.tick.saturating_sub(spin.started) as u32;
            if spin.clicked(n) && spin.speed(n).abs() < 1.0 {
                self.sounds.push("step_wood");
            }
        }
        if let Some(spin) = self.spin
            && spin.done(self.tick)
            && self.mode_said != Some(spin.started)
        {
            let mode = Mode::WHEEL[usize::from(spin.wedge)];
            self.mode = mode;
            self.mode_said = Some(spin.started);
            self.found = Some((self.tick, 0, format!("Fortune's Wheel: {}", mode.name())));
            self.cues.push(format!("wheel:{}", mode.word()));
            self.sounds.push("wheel_land");
            if mode.luck() != Luck::Plain {
                self.sounds.push("gold");
            }
            self.fx.push(Fx::Shake(match mode.luck() {
                Luck::Grand => 18,
                _ => 12,
            }));
            self.hold_for(juice::Weight::Finale);
        }
    }

    pub(super) fn deal_ults(&mut self) {
        let random = self.mode == Mode::AllRandom;
        let owns: BTreeMap<u32, ults::Ult> = self.players.iter().map(|(&id, h)| (id, self.own_ult(h.knight.as_deref()))).collect();
        for (id, hero) in self.players.iter_mut() {
            hero.ult = hero.knight.as_deref().filter(|k| recruit_ults::recruit(k)).map(|_| owns[id]);
            if random {
                let own = owns[id];
                let others: Vec<ults::Ult> = ults::Ult::ALL
                    .iter()
                    .copied()
                    .filter(|&u| u != own)
                    .collect();
                hero.ult = Some(others[self.rng.below(others.len())]);
            }
        }
        if random {
            self.cues.push("all_random".into());
        }
    }

    pub(super) fn iron(&mut self) {
        if self.mode != Mode::Ironman {
            return;
        }
        for hero in self.players.values_mut() {
            hero.winds = 0;
        }
    }

    pub(super) fn half_charge(&mut self) {
        if self.mode != Mode::AllRandom {
            return;
        }
        for hero in self.players.values_mut() {
            hero.ult_charge = hero.ult_charge.max(ults::ULT_FULL / 2);
        }
    }

    pub(super) fn turbo(&self) -> bool {
        self.turbo_on() && self.tick % 4 == 3
    }

    pub(super) fn turbo_on(&self) -> bool {
        self.mode == Mode::Turbo && self.dungeon.depth > 0
    }

    pub(super) fn harden(&mut self) {
        let glass = self.mode == Mode::GlassJaw;
        for hero in self.players.values_mut() {
            hero.glass = glass;
        }
    }

    pub(super) fn tick_collapse(&mut self) {
        if self.mode != Mode::Collapse || self.at_home_now() {
            return;
        }
        let Some(at) = self.collapse_at else {
            return;
        };
        let warn = at.saturating_sub(u64::from(30 * HZ));
        if self.tick == warn {
            self.cues.push("collapse_warn".into());
        }
        if self.tick < at || !self.active() {
            return;
        }
        if self.tick == at {
            self.cues.push("collapse".into());
            self.fx.push(Fx::Shake(20));
        }
        let living: Vec<(f32, f32)> = self
            .players
            .values()
            .filter(|h| h.hp > 0 && !h.stone)
            .map(|h| (h.x, h.y))
            .collect();
        let past = (self.tick - at) / u64::from(HZ);
        let every = 24u64.saturating_sub(past).max(8);
        if !living.is_empty() && self.tick.is_multiple_of(every) && self.rocks.len() < 10 {
            let (hx, hy) = living[self.rng.below(living.len())];
            let (w, h) = (self.room().width(), self.room().height());
            let jitter = |rng: &mut Rng| (rng.below(81) as f32 / 10.0) - 4.0;
            let x = (hx + jitter(&mut self.rng)).clamp(3.0, w - 3.0);
            let y = (hy + jitter(&mut self.rng)).clamp(3.0, h - 3.0);
            self.rocks.push(hazards::Rock {
                x,
                y,
                fall: hazards::ROCK_FALL,
                dust: 0,
                ember: false,
            });
        }
    }

    pub(super) fn fair_chest(&mut self) {
        if self.mode != Mode::MimicFair {
            return;
        }
        let room = &mut self.dungeon.rooms[self.at];
        if room.kind != RoomKind::Fight || room.chest.is_some() {
            return;
        }
        let (x, y) = (room.width() / 2.0, room.height() / 2.0);
        if room.tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32) == Tile::Floor {
            room.chest = Some(layout::Chest { x, y, open: false });
        }
    }

    pub fn collapse_in(&self) -> Option<u32> {
        let at = self.collapse_at.filter(|_| self.mode == Mode::Collapse)?;
        (!self.at_home_now()).then(|| (at.saturating_sub(self.tick) / u64::from(HZ)) as u32)
    }
}

impl Spoils {
    pub fn scaled(&self, (times, over): (u32, u32)) -> Spoils {
        Spoils(
            self.0
                .iter()
                .map(|(&s, &n)| (s, (n * times).div_ceil(over.max(1))))
                .collect(),
        )
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_fortune__tests.rs"]
mod tests;
