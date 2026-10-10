//! Dame Fortune's Wheel: the delve's game modes.
//!
//! East of the Undercroft is Fortune's hall, and in it her wheel — the
//! Rota Fortunae that Arthur dreamt of, run the way the Delve's Herald runs
//! everything: as a show, with an audience. A knight at its lever holds F
//! and the wheel spins once a delve; where it stops is how the delve goes.
//! More wedges open as Fortune's ladder is bought.
//!
//! Every mode is a rule over the same dungeon: a darker one, bigger
//! monsters, floors that fall, a horde, a gauntlet of guardians. The riskier
//! ones pay more spoils.

use super::home::Station;
use super::*;
use crate::drive::together_realm::Spoils;

/// How long the wheel spins before it stops.
pub(crate) const SPIN_TICKS: u32 = 4 * HZ;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Mode {
    /// The delve as it always was.
    #[default]
    LongWayDown,
    /// Dark: light only round knights and fires.
    LightsOut,
    /// Every floor falls in after a while: get down before it does.
    Collapse,
    /// Every monster is a giant: bigger, tougher, and generous.
    GiantsFeast,
    /// One great hall a floor, and a horde coming up the stair.
    HoldTheStair,
    /// Chests in every won room, and half of them bite.
    MimicFair,
    /// Everyone hits twice as hard. Everyone.
    GlassJaw,
    /// No small talk: a hall, then the guardian, every floor.
    Gauntlet,
    /// Turbo: the whole delve runs a third faster.
    Turbo,
    /// A rune in every fight, and a fresh one while it lasts.
    RuneRush,
    /// All Random: every knight carries a stranger's ultimate.
    AllRandom,
    /// No second winds at all (the Pit's Talisman aside).
    Ironman,
    /// Every floor hides a vault behind a cracked wall.
    HollowWalls,
    /// Sponsors: the audience counts double, so the boxes rain.
    Sponsors,
}

impl Mode {
    /// The wheel's wedges, clockwise from the top.
    pub(crate) const WHEEL: [Mode; 14] = [
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
        Mode::Sponsors,
    ];

    pub(crate) fn name(self) -> &'static str {
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
            Mode::Sponsors => "Sponsors' Night",
        }
    }

    /// The word in a cue: `wheel:lights_out`.
    pub(crate) fn word(self) -> &'static str {
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
            Mode::Sponsors => "sponsors",
        }
    }

    /// What it does, in a line.
    pub(crate) fn says(self) -> &'static str {
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
            Mode::Sponsors => "the audience counts double",
        }
    }

    /// The rung of Fortune's ladder that opens this wedge.
    pub(crate) fn rung(self) -> u8 {
        match self {
            Mode::LongWayDown | Mode::LightsOut | Mode::GiantsFeast | Mode::MimicFair => 0,
            Mode::Collapse | Mode::GlassJaw => 1,
            Mode::HoldTheStair | Mode::Gauntlet => 2,
            Mode::Turbo | Mode::RuneRush | Mode::AllRandom => 3,
            Mode::Ironman | Mode::HollowWalls | Mode::Sponsors => 4,
        }
    }

    /// What the spoils carried home are worth, as `(times, over)`.
    pub(crate) fn spoils(self) -> (u32, u32) {
        match self {
            Mode::LightsOut | Mode::GiantsFeast => (3, 2),
            Mode::Collapse | Mode::GlassJaw | Mode::HoldTheStair | Mode::Turbo => (2, 1),
            Mode::LongWayDown
            | Mode::MimicFair
            | Mode::Gauntlet
            | Mode::RuneRush
            | Mode::AllRandom
            | Mode::HollowWalls
            | Mode::Sponsors => (1, 1),
            Mode::Ironman => (3, 1),
        }
    }

    /// The seconds a floor stands before it collapses.
    pub(crate) fn collapse_secs(depth: u32) -> u32 {
        120 + 30 * depth
    }
}

/// A spin of the wheel: when it began, where it began, and the wedge it
/// stops on (decided at the pull, played out over `SPIN_TICKS`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Spin {
    pub(crate) started: u64,
    pub(crate) from: f32,
    pub(crate) wedge: u8,
    pub(crate) turns: u8,
    /// Who pulled the lever.
    pub(crate) by: u32,
}

impl Spin {
    /// How far the wheel has turned (radians, clockwise) at `tick`.
    pub(crate) fn angle(&self, tick: u64) -> f32 {
        let t = (tick.saturating_sub(self.started) as f32 / SPIN_TICKS as f32).min(1.0);
        let ease = 1.0 - (1.0 - t).powi(3);
        let target = f32::from(self.turns) * std::f32::consts::TAU + rest_angle(self.wedge);
        self.from + (target - self.from) * ease
    }

    pub(crate) fn done(&self, tick: u64) -> bool {
        tick.saturating_sub(self.started) >= u64::from(SPIN_TICKS)
    }
}

/// The wheel's turn that brings wedge `wedge` under the pointer at the top.
pub(crate) fn rest_angle(wedge: u8) -> f32 {
    let w = std::f32::consts::TAU / Mode::WHEEL.len() as f32;
    std::f32::consts::TAU - (f32::from(wedge) + 0.5) * w
}

/// Where the hall's plates are, in tiles: the lever before the wheel, and
/// Fortune's own ledger beside it.
pub(crate) const LEVER: (i32, i32, i32, i32) = (11, 8, 2, 1);
pub(crate) const FORTUNE_PLATE: (i32, i32, i32, i32) = (16, 8, 2, 1);
/// The Herald's coffer on its pedestal by the west door, and its plate.
pub(crate) const COFFER: (i32, i32, i32, i32) = (3, 3, 2, 2);
pub(crate) const COFFER_PLATE: (i32, i32, i32, i32) = (3, 5, 2, 1);
/// The wheel on its stand against the north wall, and the benches.
pub(crate) const WHEEL_STAND: (i32, i32, i32, i32) = (9, 1, 6, 6);
pub(crate) const BENCHES: [(i32, i32, i32, i32); 2] = [(3, 10, 7, 2), (14, 10, 7, 2)];
/// Where Dame Fortune stands, in tiles.
pub(crate) const FORTUNE_AT: (f32, f32) = (7.6, 7.2);

fn inside((col, row, w, h): (i32, i32, i32, i32), x: f32, y: f32) -> bool {
    let (c, r) = (x / TILE_UNITS, y / TILE_UNITS);
    c >= col as f32 && c < (col + w) as f32 && r >= row as f32 && r < (row + h) as f32
}

/// The wedges Fortune's ladder has opened.
pub(crate) fn open_wedges(level: u8) -> Vec<u8> {
    (0..Mode::WHEEL.len() as u8)
        .filter(|&i| Mode::WHEEL[usize::from(i)].rung() <= level)
        .collect()
}

impl Run {
    /// Fortune's hall: the lever spins the wheel (once a delve), her plate
    /// reads her ledger, and the wheel comes to rest on the delve's mode.
    pub(super) fn tick_fortune(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.room().kind != RoomKind::Fortune {
            return;
        }
        let mut pulled = None;
        let mut asked = Vec::new();
        for (&id, hero) in self.players.iter_mut() {
            let fire = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .is_some_and(|i| i.fire);
            if !fire {
                hero.buy_spent = false;
            }
            let standing = hero.hp > 0 && !hero.stone;
            let on_lever = standing && inside(LEVER, hero.x, hero.y);
            let on_coffer = standing && inside(COFFER_PLATE, hero.x, hero.y);
            let on_plate = standing && (inside(FORTUNE_PLATE, hero.x, hero.y) || on_coffer);
            if (on_lever || on_plate) && fire && !hero.buy_spent {
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
        // Fortune greets whoever walks up to her, once per approach.
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
            self.shake = self.shake.max(10);
        }
    }

    /// All Random: down the stair, each knight is handed one of the other
    /// knights' ultimates for the delve; any other way down, their own.
    pub(super) fn deal_ults(&mut self) {
        let random = self.mode == Mode::AllRandom;
        for hero in self.players.values_mut() {
            hero.ult = None;
            if random {
                let own = ults::Ult::of(hero.knight.as_deref());
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

    /// Ironman: no knight keeps a second wind (the Talisman is the Pit Tyrant's
    /// own, and still saves).
    pub(super) fn iron(&mut self) {
        if self.mode != Mode::Ironman {
            return;
        }
        for hero in self.players.values_mut() {
            hero.winds = 0;
        }
    }

    /// All Random: every floor begins with the ultimates half charged.
    pub(super) fn half_charge(&mut self) {
        if self.mode != Mode::AllRandom {
            return;
        }
        for hero in self.players.values_mut() {
            hero.ult_charge = hero.ult_charge.max(ults::ULT_FULL / 2);
        }
    }

    /// Turbo: down in the delve, one tick in four comes free, so every
    /// three steps of play run four ticks of the delve.
    pub(super) fn turbo(&self) -> bool {
        self.mode == Mode::Turbo && self.dungeon.depth > 0 && self.tick % 4 == 3
    }

    /// Glass Jaw: the knights hit and are hit twice as hard.
    pub(super) fn harden(&mut self) {
        let glass = self.mode == Mode::GlassJaw;
        for hero in self.players.values_mut() {
            hero.glass = glass;
        }
    }

    /// The Collapse: past a floor's time, the ceiling comes down wherever
    /// the party stands, faster and faster, until they take the stairs.
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
            self.shake = self.shake.max(20);
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
            });
        }
    }

    /// Mimic Fair: a won room leaves a chest in its middle.
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

    /// Seconds left before this floor collapses, while it stands.
    pub(crate) fn collapse_in(&self) -> Option<u32> {
        let at = self.collapse_at.filter(|_| self.mode == Mode::Collapse)?;
        (!self.at_home_now()).then(|| (at.saturating_sub(self.tick) / u64::from(HZ)) as u32)
    }
}

impl Spoils {
    /// Each spoil times `times / over`, rounded up: a riskier delve's pay.
    pub(crate) fn scaled(&self, (times, over): (u32, u32)) -> Spoils {
        Spoils(
            self.0
                .iter()
                .map(|(&s, &n)| (s, (n * times).div_ceil(over.max(1))))
                .collect(),
        )
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_fortune__tests.rs"]
mod tests;
