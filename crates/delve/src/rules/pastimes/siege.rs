//! THE SIEGE TRAIL — a medieval homage to the lobbing artillery games and
//! the marching trail games, in our own name with our own knights.
//!
//! ## Brief (written first)
//! The party marches a trail of three legs to three castles that get
//! harder: higher walls, more defenders, stronger wind. Each leg is one
//! fixed event — a ford, wolves, a bad stew — and the knights answer with
//! one key (F) or march on; every man lost gets a tiny grave and a
//! one-line epitaph in our own words. At each castle every knight plays
//! their own siege on their own side-on field: aim with up and down, power
//! with left and right, F to loose. The wind changes each volley. The
//! castle shoots back — a pot tipped over the wall — and a hit kills crew.
//! Soldiers are the clock and the life: the crew haul the counterweight
//! (each a little body leaning on the rope; fewer men, more frames of
//! straining), the sling is loaded with whatever is to hand by fixed
//! rotation (boulder, cow, beehive, pig, a barrel of eels — each with its
//! own splat), and with two crew or fewer a soldier stays tangled in the
//! sling and is flung with the shot, legs windmilling, then limps back
//! next volley. Bring the keep's banner down (undermine the middle of the
//! wall) and the party marches on; a knight out of soldiers or stones
//! waits until the next castle, where recruits join. The most damage with
//! the most crew outlasted takes the crown; the others wear the game's
//! booby until they win one back.
//!
//! ## The five fixed numbers
//! 1. `SOLDIERS` — soldiers a knight sets out with.
//! 2. `MISSING_CREW_TICKS` — reload ticks added per missing soldier.
//! 3. `VOLLEY_HZ` — the shared volley clock, in ticks.
//! 4. `WIND_MAX` — the wind's top strength.
//! 5. `EVENT_LOSS` — soldiers lost to the mild side of a trail event.
//!
//! ## Determinism
//! Host-authoritative at 30 Hz, fixed integers only. The arc is
//! fixed-point with a fixed gravity step; the wind is a fixed table read
//! per volley; every trail event is a fixed table, never a roll.

use super::Round;
use crate::rules::{Input, Run, HZ, MAX_PLAYERS};
use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

fn ser_craters<S: serde::Serializer>(c: &[u8; 48], s: S) -> Result<S::Ok, S::Error> {
    s.serialize_some(&c[..])
}
fn de_craters<'de, D: serde::Deserializer<'de>>(d: D) -> Result<[u8; 48], D::Error> {
    let v = <Option<Vec<u8>>>::deserialize(d)?;
    let mut c = [0u8; 48];
    if let Some(v) = v {
        for (dst, src) in c.iter_mut().zip(v) {
            *dst = src;
        }
    }
    Ok(c)
}

// ─── the five fixed numbers ─────────────────────────────────────────────────

/// 1. Soldiers a knight sets out with.
pub const SOLDIERS: u8 = 7;
/// 2. Reload ticks added per missing soldier: fewer men, funnier reload.
pub const MISSING_CREW_TICKS: u32 = 12;
/// 3. The shared volley clock: every loaded engine fires when it ends, so
/// nobody waits a turn and a lone knight still has a game.
pub const VOLLEY_HZ: u32 = 60;
/// 4. The wind's top strength (sixteenths of a pixel a tick).
pub const WIND_MAX: i32 = 8;
/// 5. Soldiers lost to the mild side of a trail event.
pub const EVENT_LOSS: u8 = 1;

// ─── the other fixed numbers ────────────────────────────────────────────────

/// Soldiers lost to the rash side of a trail event.
pub const EVENT_RASH_LOSS: u8 = 2;
/// Legs between castles: five legs, three castles (a castle falls on legs
/// 2, 3 and 5).
pub const CASTLE_LEGS: [u8; 3] = [2, 3, 5];
/// Which castle a leg besieges, if any (1..=3).
pub fn castle_of(leg: u8) -> Option<u8> {
    match leg {
        2 => Some(1),
        3 => Some(2),
        5 => Some(3),
        _ => None,
    }
}
/// Ticks a splat frame lasts on the ground after a shot lands.
pub const SPLAT: u32 = HZ / 5;
/// Ticks a felled crewman lies on the ground before rising.
pub const FELLED: u32 = HZ;
/// Recruits who join at each new castle.
pub const RECRUITS: u8 = 2;
/// Stones in the magazine at each castle.
pub const STONES: u8 = 9;
/// Food set out with; each leg eats one, and an empty larder eats men.
pub const FOOD: u8 = 3;
/// Hits a wall block takes from a boulder.
pub const HIT_DAMAGE: u8 = 3;
/// Chip damage the blocks above and below a hit take.
pub const SPLASH_DAMAGE: u8 = 1;
/// Wall rows in the first keep; each later keep one row taller.
pub const KEEP_ROWS: u8 = 3;
/// Defenders in the first keep; each later keep one more.
pub const DEFENDERS: u8 = 2;
/// Ticks a trail event waits for the party's key.
pub const EVENT_WAIT: u32 = 2 * HZ;
/// Ticks the notice board stands after an event settles.
pub const MOURN: u32 = HZ + HZ / 2;
/// Ticks the cheer at a banner down.
pub const CHEER: u32 = HZ + HZ / 2;
/// Ticks a beaten besieger waits at the walls before marching on.
pub const RETREAT: u32 = HZ;
/// Ticks the defenders wave a bare backside at a miss.
pub const MOON: u32 = HZ + HZ / 2;
/// Ticks the defenders spend tipping their pot as they fire.
pub const POT: u32 = HZ / 3;
/// The fixed gravity step (sixteenths a tick a tick).
pub const G_STEP: i32 = 8;
/// A shot's base speed, and per power step (sixteenths a tick).
pub const SHOT_V0: i32 = 20;
pub const SHOT_VP: i32 = 14;
/// A shot's base lift, and per aim step (sixteenths a tick).
pub const SHOT_L0: i32 = 30;
pub const SHOT_LA: i32 = 10;
/// The aim and power dials: 0..=DIAL each.
pub const DIAL: u8 = 10;
/// The design field, and where everything stands on it (pixels).
pub const FIELD_W: i32 = 384;
pub const GROUND_LINE: i32 = 176;
pub const ENGINE_X: i32 = 36;
pub const CASTLE_X: i32 = 264;
pub const BLOCK_W: i32 = 12;
pub const BLOCK_H: i32 = 10;
pub const KEEP_COLS: usize = 3;
/// The rolling ground, fixed: hill height of each 8-pixel segment.
pub const GROUND: [u8; 48] = [
    6, 7, 8, 9, 10, 11, 12, 11, 10, 9, 10, 11, 12, 13, 12, 11, 10, 9, 8, 9, 10, 11, 12, 13, 14,
    13, 12, 11, 10, 9, 8, 7, 8, 9, 10, 11, 12, 11, 10, 9, 8, 7, 6, 6, 7, 8, 9, 8,
];
/// Crater depth a landing digs (pixels of hill).
pub const CRATER: u8 = 3;
/// The wind table: castle 1 gentle, castle 3 wild (capped at `WIND_MAX`).
pub const WINDS: [i32; 3] = [WIND_MAX / 4, WIND_MAX / 2, WIND_MAX];
/// How far each of the keep's pot-shots lands off the engine (pixels),
/// fixed, so a knight can learn to dread the fourth.
pub const POT_LANDS: [i32; 5] = [0, 12, -9, 20, -15];
/// Near enough to the engine that a pot's landing kills crew (pixels).
pub const POT_WOUND: i32 = 14;
/// The notice board's fixed lines, by index.
pub const NOTICES: [&str; 12] = [
    "THE FORD",
    "THE WOLVES",
    "THE BAD STEW",
    "THE FLUX",
    "THE DESERTER",
    "The river took its toll.",
    "The wolves took their due.",
    "The stew took its revenge.",
    "The larders were bare. The march ate men.",
    "The party marches on.",
    "He died of the flux. The privy knows the truth.",
    "One walked home in the night. The road keeps its own.",
];
pub const NOTICE_FLUX: u8 = 8;
pub const NOTICE_DESERTER: u8 = 9;
/// The ford crossed dry-shod (the stone's line).
pub const NOTICE_FORD_PAID: u8 = 6;
pub const NOTICE_MARCHES: u8 = 7;
pub const NOTICE_NONE: u8 = 255;
pub const GRAVE_NONE: u8 = 255;

/// What the sling holds, by fixed rotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ammo {
    Boulder,
    Cow,
    Beehive,
    Pig,
    Eels,
}

impl Ammo {
    pub const ROTATION: [Ammo; 5] =
        [Ammo::Boulder, Ammo::Cow, Ammo::Beehive, Ammo::Pig, Ammo::Eels];
    pub fn from_index(i: u8) -> Ammo {
        Ammo::ROTATION[usize::from(i % 5)]
    }
    pub fn name(self) -> &'static str {
        match self {
            Ammo::Boulder => "BOULDER",
            Ammo::Cow => "COW",
            Ammo::Beehive => "BEEHIVE",
            Ammo::Pig => "PIG",
            Ammo::Eels => "EEL BARREL",
        }
    }
    /// Each lands with its own splat (short, our own words).
    pub fn splat(self) -> &'static str {
        match self {
            Ammo::Boulder => "CRUNCH!",
            Ammo::Cow => "MOO-THUD!",
            Ammo::Beehive => "BUZZ-SPLAT!",
            Ammo::Pig => "OINK-THUMP!",
            Ammo::Eels => "SLITHER-SPLAT!",
        }
    }
    /// The damage it does to a block it meets: the boulder is the
    /// workhorse, live stock softer.
    pub fn damage(self) -> u8 {
        match self {
            Ammo::Boulder => HIT_DAMAGE,
            _ => HIT_DAMAGE - 1,
        }
    }
}

/// The trail's dead, in order of loss: a name and our own one line.
pub const EPITAPHS: [(&str, &str); 10] = [
    ("Wat", "He ate the stew."),
    ("Colin", "He fought the wolves."),
    ("Bert", "The ford was cold."),
    ("Odo", "The wolves knew his name."),
    ("Marn", "The flux was quicker."),
    ("Dole", "He kept walking home."),
    ("Piers", "He waved at the castle."),
    ("Hob", "He trusted the stew."),
    ("Ned", "He lent the toll money."),
    ("Aldous", "He ducked too slowly."),
];

/// The trail: five legs over three castles. What marching on costs,
/// what F buys, and the notice board's word when it takes a man.
pub const LEGS: [( &str, &str, &str, &str ); 5] = [
    ("THE FORD", "MARCH ON: LOSE 1", "F: PAY A STONE", "The river took"),
    ("THE WOLVES", "STAND: LOSE 1", "F: FLEE: LOSE 2", "The wolves took"),
    ("THE BAD STEW", "SKIP: LOSE 1", "F: EAT: LOSE 2", "The stew took"),
    ("THE FLUX", "REST: LOSE 1", "F: MARCH ON: LOSE 1", "The flux took"),
    ("THE DESERTER", "LET HIM GO: LOSE 1", "F: CHASE: LOSE 2", "The road took"),
];

/// One side-on keep: breakable blocks in three columns, defenders, banner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keep {
    /// Damage each block can still take: 3 columns x `rows`, column-major,
    /// top row first. Zero is rubble.
    pub blocks: Vec<u8>,
    pub rows: u8,
    /// Defenders behind the wall (they keep firing the pot).
    pub defenders: u8,
    /// The banner stands while the middle column's top block does.
    pub banner: bool,
    /// Ticks to the keep's next pot-shot.
    pub shot_left: u32,
    /// Pot-shots fired: picks the next landing from the fixed table.
    pub volleys: u8,
}

impl Keep {
    pub fn new(castle: u8) -> Self {
        let rows = KEEP_ROWS + castle.saturating_sub(1);
        Keep {
            blocks: vec![HIT_DAMAGE; KEEP_COLS * usize::from(rows)],
            rows,
            defenders: DEFENDERS + castle.saturating_sub(1),
            banner: true,
            shot_left: VOLLEY_HZ + VOLLEY_HZ / 2,
            volleys: 0,
        }
    }
    /// The top standing row of column `col` (0 = top), or `rows` if the
    /// column is rubble.
    pub fn top_of(&self, col: usize) -> u8 {
        let rows = usize::from(self.rows);
        for row in 0..rows {
            if self.blocks[col * rows + row] > 0 {
                return row as u8;
            }
        }
        self.rows
    }
    /// A hit at column `col`, row-from-top `row`, for `dmg`: the damage
    /// done, with a chip to the blocks above and below.
    pub fn hit(&mut self, col: usize, row: u8, dmg: u8) -> u8 {
        let rows = usize::from(self.rows);
        let i = col * rows + usize::from(row);
        let mut done = 0;
        if i < self.blocks.len() {
            let before = self.blocks[i];
            self.blocks[i] = before.saturating_sub(dmg);
            done += before - self.blocks[i];
            for &n in &[i.wrapping_sub(rows), i + rows] {
                if n < self.blocks.len() && self.blocks[n] > 0 {
                    let b = self.blocks[n];
                    self.blocks[n] = b.saturating_sub(SPLASH_DAMAGE);
                    done += b - self.blocks[n];
                }
            }
        }
        // The banner hangs over the great gate: the middle column's door.
        // Break the door and the gate comes down, banner and all — a wall
        // is only as tall as its door.
        let rows = usize::from(self.rows);
        if self.blocks[2 * rows - 1] == 0 {
            self.banner = false;
        }
        done
    }
}

/// A shot in the air: fixed-point pixels (sixteenths).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shot {
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub wind: i32,
    pub ammo: Ammo,
    /// A soldier tangled in the sling, legs windmilling.
    pub flung: bool,
}

impl Shot {
    pub fn new(loose_from: (i32, i32), power: u8, aim: u8, wind: i32, ammo: Ammo, flung: bool) -> Self {
        Shot {
            x: loose_from.0 * 16,
            y: loose_from.1 * 16,
            vx: SHOT_V0 + i32::from(power) * SHOT_VP,
            vy: -(SHOT_L0 + i32::from(aim) * SHOT_LA),
            wind,
            ammo,
            flung,
        }
    }
}

/// One knight's own march and siege.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Siege {
    pub owner: u32,
    /// False on the trail, true at the walls.
    pub fighting: bool,
    /// The leg being marched (1..=3); the castle besieged equals it.
    pub leg: u8,
    /// Castles taken (the round's progress), and the graves left on the
    /// trail behind the party: each an index into `EPITAPHS`.
    pub graves: [u8; 5],
    pub keep: Keep,
    pub soldiers: u8,
    pub stones: u8,
    pub food: u8,
    pub aim: u8,
    pub power: u8,
    /// Ticks left on the reload clock and the volley clock.
    pub reload: u32,
    pub volley_left: u32,
    /// The knight's shot in the air, and the keep's pot-shot.
    pub shot: Option<Shot>,
    pub castle_shot: Option<Shot>,
    /// Where the keep's pot-shot will land (pixels).
    pub pot_land: i32,
    /// The wind on the current volley (sixteenths a tick).
    pub wind: i32,
    /// Volleys loosed: the sling's load follows by rotation.
    pub volleys: u8,
    /// Damage done to standing masonry (score) and soldiers lost over the
    /// whole trail.
    pub damage: u16,
    pub lost: u8,
    /// Castles taken (the round's progress).
    pub taken: u8,
    /// A soldier tangled in the sling, and ticks of him limping home.
    pub flung: bool,
    pub limp: u32,
    /// Craters dug into the rolling ground: each entry is a column's depth.
    #[serde(
        serialize_with = "ser_craters",
        deserialize_with = "de_craters"
    )]
    pub craters: [u8; 48],
    /// The trail event in play: ticks left, and whether F answered.
    pub event_left: u32,
    pub event_f: bool,
    /// The notice board's line (index into a fixed table; 255 = none).
    pub notice: u8,
    /// The grave it reports: index into `EPITAPHS` (255 = none).
    pub grave: u8,
    /// The splat a landed shot leaves (ticks left, and the load that fell),
    /// and the felled crewman a pot-shot knocked down.
    pub splat: u32,
    pub splat_x: i32,
    pub splat_miss: bool,
    pub felled: u32,
    /// Clocks for the jokes: mourning, the cheer, a beaten retreat, the
    /// defenders' bare backside, and their tipping pot.
    pub mourn: u32,
    pub cheer: u32,
    pub retreat: u32,
    pub moon: u32,
    pub pot: u32,
}

impl Siege {
    pub fn new(owner: u32) -> Self {
        Siege {
            owner,
            fighting: false,
            leg: 1,
            graves: [GRAVE_NONE; 5],
            keep: Keep::new(1),
            soldiers: SOLDIERS,
            stones: STONES,
            food: FOOD,
            aim: 5,
            power: 5,
            reload: 0,
            volley_left: VOLLEY_HZ,
            shot: None,
            castle_shot: None,
            pot_land: ENGINE_X,
            wind: WINDS[0],
            volleys: 0,
            damage: 0,
            lost: 0,
            taken: 0,
            flung: false,
            limp: 0,
            craters: [0; 48],
            event_left: EVENT_WAIT,
            event_f: false,
            notice: 0,
            grave: GRAVE_NONE,
            splat: 0,
            splat_x: ENGINE_X,
            splat_miss: false,
            felled: 0,
            mourn: 0,
            cheer: 0,
            retreat: 0,
            moon: 0,
            pot: 0,
        }
    }

    /// The reload clock: full crew is quick, and every missing soldier
    /// adds his own frames of straining.
    pub fn reload_ticks(&self) -> u32 {
        RELOAD_TICKS_BASE + u32::from(SOLDIERS.saturating_sub(self.soldiers)) * MISSING_CREW_TICKS
    }

    /// The crew hauling the counterweight: pose frame 0..=3, stepped off
    /// the round's clock. Fewer men means a shorter cycle held in the
    /// strain — more frames of leaning on the rope.
    pub fn crew_pose(&self, tick: u32) -> u8 {
        let missing = SOLDIERS.saturating_sub(self.soldiers).min(3);
        let span = 12 - u32::from(missing) * 3;
        ((tick / 2) % span) as u8 % 4
    }

    /// The ground's standing height at field x (pixels above the line),
    /// craters and all: where a falling shot stops.
    pub fn ground_at(&self, x: i32) -> i32 {
        let seg = (x / 8).clamp(0, 47) as usize;
        i32::from(GROUND[seg].saturating_sub(self.craters[seg]))
    }

    /// The wind for the coming volley, from the fixed table: the sign
    /// flips each volley, so no two ride the same air.
    pub fn next_wind(&self) -> i32 {
        let w = WINDS[usize::from(self.leg - 1).min(2)];
        if self.volleys % 2 == 1 { -w } else { w }
    }

    /// Loose: the sling's load by fixed rotation, the arc by the dials.
    fn loose(&mut self) -> Shot {
        let ammo = Ammo::from_index(self.volleys);
        self.volleys += 1;
        let flung = self.flung && self.limp == 0;
        if flung {
            self.flung = false;
        }
        Shot::new(
            (ENGINE_X + 8, GROUND_LINE - 26 - self.ground_at(ENGINE_X)),
            self.power,
            self.aim,
            self.wind,
            ammo,
            flung,
        )
    }

    /// One tick of the knight's own march and siege. `Some(false)`: this
    /// castle is lost to them but the round goes on; `None`: the round is
    /// over.
    pub fn step(&mut self, input: Input, tapped: bool) -> Option<bool> {
        if self.mourn > 0 {
            self.mourn -= 1;
            if self.mourn == 0 {
                self.to_the_walls();
            }
            return Some(true);
        }
        if self.cheer > 0 {
            self.cheer -= 1;
            if self.cheer == 0 {
                self.march_on();
            }
            return Some(true);
        }
        if self.retreat > 0 {
            self.retreat -= 1;
            if self.retreat == 0 {
                self.march_on();
            }
            return Some(true);
        }
        if !self.fighting {
            self.step_trail(tapped);
            return Some(true);
        }
        // Aim with up and down, power with left and right.
        if input.aim_y < 0 || input.move_y < 0 {
            self.aim = (self.aim + 1).min(DIAL);
        }
        if input.aim_y > 0 || input.move_y > 0 {
            self.aim = self.aim.saturating_sub(1);
        }
        if input.aim_x > 0 || input.move_x > 0 {
            self.power = (self.power + 1).min(DIAL);
        }
        if input.aim_x < 0 || input.move_x < 0 {
            self.power = self.power.saturating_sub(1);
        }
        // The reload crew haul; with two or fewer, one tangles in the sling.
        if self.reload > 0 {
            self.reload -= 1;
            if self.reload == 0 && self.soldiers <= 2 && self.soldiers > 0 && !self.flung {
                self.flung = true;
            }
        }
        if self.limp > 0 {
            self.limp -= 1;
        }
        if self.splat > 0 {
            self.splat -= 1;
        }
        if self.moon > 0 {
            self.moon -= 1;
        }
        if self.felled > 0 {
            self.felled -= 1;
        }
        // The shared volley clock turns the wind for everyone at once; only
        // F loosens, so an idle engine never wins a war by itself.
        self.volley_left -= 1;
        let volley = self.volley_left == 0;
        if volley {
            self.volley_left = VOLLEY_HZ;
            self.wind = self.next_wind();
        }
        let loaded = self.reload == 0 && self.shot.is_none() && self.stones > 0;
        if tapped && loaded {
            self.shot = Some(self.loose());
            self.stones -= 1;
            self.reload = self.reload_ticks();
        }
        self.step_shot();
        self.step_keep_shot();
        // Out of soldiers or stones: off to the next castle — unless this
        // was the last, when the trail is lost.
        let spent = self.soldiers == 0 || (self.stones == 0 && self.shot.is_none());
        if spent {
            if castle_of(self.leg) == Some(3) {
                return None;
            }
            self.retreat = RETREAT;
            self.shot = None;
            self.castle_shot = None;
        }
        Some(true)
    }

    /// The trail event: wait for the key, then settle it, fixed.
    fn step_trail(&mut self, tapped: bool) {
        if self.event_left > 0 {
            self.event_left -= 1;
            if tapped {
                self.event_f = true;
                self.event_left = 0;
            } else {
                return;
            }
        }
        let (_, march, keyed, took) = LEGS[(usize::from(self.leg) - 1).min(LEGS.len() - 1)];
        let _ = (march, keyed);
        let mut loss = if self.event_f { EVENT_RASH_LOSS } else { EVENT_LOSS };
        // The ford's keyed side pays a stone instead of a man, if one is
        // spare.
        let paid = self.leg == 1 && self.event_f && self.stones > 0;
        if paid {
            self.stones -= 1;
            loss = 0;
        }
        // An empty larder eats men on the march.
        let hungry = self.food == 0;
        if hungry {
            loss += EVENT_LOSS;
        }
        self.food = self.food.saturating_sub(1);
        let before = self.soldiers;
        self.soldiers = self.soldiers.saturating_sub(loss);
        let gone = before - self.soldiers;
        self.lost += gone;
        let leg_notice = match self.leg {
            1 => 3,
            2 => 4,
            3 => 5,
            4 => NOTICE_FLUX,
            _ => NOTICE_DESERTER,
        };
        self.notice = if paid {
            NOTICE_FORD_PAID
        } else if hungry && gone > 0 {
            6
        } else if gone > 0 {
            leg_notice
        } else {
            NOTICE_MARCHES
        };
        let _ = took;
        self.grave = if gone > 0 { self.lost.min(7) } else { GRAVE_NONE };
        if gone > 0 {
            // A grave is left on the trail behind the party, one line each.
            let line = match self.leg {
                1 => 0,
                2 => 1,
                3 => 2,
                4 => 3,
                _ => 4,
            };
            self.graves[usize::from(self.leg - 1).min(4)] = self.lost.min(7).saturating_sub(1);
            let _ = line;
        }
        self.mourn = MOURN;
        self.event_f = false;
    }

    /// The walls at last: recruits and a fresh magazine arrive.
    fn to_the_walls(&mut self) {
        self.fighting = true;
        self.keep = Keep::new(castle_of(self.leg).unwrap_or(1));
        self.soldiers = (self.soldiers + RECRUITS).min(SOLDIERS);
        self.stones = STONES;
        self.reload = 0;
        self.volley_left = VOLLEY_HZ;
        self.wind = self.next_wind();
        self.shot = None;
        self.castle_shot = None;
        self.grave = GRAVE_NONE;
        self.notice = NOTICE_NONE;
    }

    /// The banner is down (or the party is spent): on to the next leg.
    fn march_on(&mut self) {
        self.fighting = false;
        self.leg += 1;
        self.event_left = EVENT_WAIT;
        // The board ahead: NOTICES 0..=2 name the castles' roads, 3 and 4
        // the flux's and the deserter's.
        self.notice = match castle_of(self.leg + 1) {
            Some(_) => (self.leg - 1).min(2),
            None => match self.leg {
                3 => 3,
                _ => 4,
            },
        };
        self.grave = GRAVE_NONE;
        self.splat = 0;
        self.felled = 0;
    }

    /// The knight's shot: a fixed arc, the wall, the ground.
    fn step_shot(&mut self) {
        let Some(mut s) = self.shot.take() else { return };
        s.vy += G_STEP;
        s.x += s.vx + s.wind;
        s.y += s.vy;
        let (px, py) = (s.x / 16, s.y / 16);
        // The wall: three columns of blocks on the keep's face.
        if px >= CASTLE_X && px < CASTLE_X + BLOCK_W * KEEP_COLS as i32 {
            let col = (px - CASTLE_X) / BLOCK_W;
            let base = GROUND_LINE - self.ground_at(CASTLE_X + col * BLOCK_W + BLOCK_W / 2);
            let rows = i32::from(self.keep.rows);
            if py < base {
                let from_top = (base - py + BLOCK_H - 1) / BLOCK_H - 1;
                if from_top >= 0 && from_top < rows {
                    let row = from_top as u8;
                    let i = usize::try_from(col).unwrap_or(0) * usize::from(self.keep.rows)
                        + usize::from(row);
                    if i < self.keep.blocks.len() && self.keep.blocks[i] > 0 {
                        let done = self
                            .keep
                            .hit(usize::try_from(col).unwrap_or(0), row, s.ammo.damage());
                        self.damage += u16::from(done);
                        if s.flung {
                            self.limp = VOLLEY_HZ;
                        }
                        self.landed(&s, true);
                        if !self.keep.banner && self.cheer == 0 {
                            self.taken += 1;
                            self.cheer = CHEER;
                        }
                        return;
                    }
                }
            }
        }
        // The ground: a crater, and the defenders' commentary on a short.
        let ground = GROUND_LINE - self.ground_at(px);
        if py >= ground || px >= FIELD_W {
            self.dig(px);
            if s.flung {
                self.limp = VOLLEY_HZ;
            }
            let short = px < CASTLE_X;
            self.landed(&s, short);
            return;
        }
        self.shot = Some(s);
    }

    /// A landing's noise and the defenders' manners.
    fn landed(&mut self, s: &Shot, short: bool) {
        self.moon = if short { MOON } else { 0 };
        self.splat = SPLAT;
        self.splat_x = s.x / 16;
        self.splat_miss = short;
    }

    fn dig(&mut self, px: i32) {
        // The castle stands on its own footings: no crater under the wall.
        if px >= CASTLE_X - BLOCK_W {
            return;
        }
        let seg = (px / 8).clamp(0, 47) as usize;
        self.craters[seg] = (self.craters[seg] + CRATER).min(GROUND[seg]);
    }

    /// The keep's pot-shot: fixed landings off the engine, learnt by heart.
    fn step_keep_shot(&mut self) {
        if self.pot > 0 {
            self.pot -= 1;
        }
        if self.castle_shot.is_none() {
            if self.keep.defenders == 0 || !self.keep.banner {
                return;
            }
            self.keep.shot_left = self.keep.shot_left.saturating_sub(1);
            if self.keep.shot_left == 0 {
                self.keep.shot_left = VOLLEY_HZ + VOLLEY_HZ / 2;
                let off = POT_LANDS[usize::from(self.keep.volleys % 5)];
                self.keep.volleys += 1;
                self.pot = POT;
                self.pot_land = ENGINE_X + off;
                // The throw that lands `off` from the engine: fixed speed,
                // fixed lift, the offset bought with the aim.
                let dist = CASTLE_X + BLOCK_W - self.pot_land;
                let vx = -dist * 16 / 45;
                self.castle_shot = Some(Shot {
                    x: (CASTLE_X + BLOCK_W) * 16,
                    y: (GROUND_LINE - 50) * 16,
                    vx,
                    vy: -180,
                    wind: 0,
                    ammo: Ammo::Boulder,
                    flung: false,
                });
            }
            return;
        }
        let mut s = self.castle_shot.take().unwrap();
        s.vy += G_STEP;
        s.x += s.vx;
        s.y += s.vy;
        let (px, py) = (s.x / 16, s.y / 16);
        if py >= GROUND_LINE - self.ground_at(px) || px <= 0 {
            if (px - self.pot_land).abs() <= POT_WOUND && self.soldiers > 0 {
                self.soldiers -= 1;
                self.lost += 1;
                self.felled = FELLED;
                // A tiny grave is left for him later, on the trail behind.
                let slot = usize::from(self.taken).min(4);
                self.graves[slot] = self.lost.min(7).saturating_sub(1);
            }
            return;
        }
        self.castle_shot = Some(s);
    }

    pub fn valid(&self, r: &Round) -> bool {
        (1..=MAX_PLAYERS).contains(&self.owner)
            && (1..=5).contains(&self.leg)
            && self.soldiers <= SOLDIERS
            && self.stones <= STONES
            && self.food <= FOOD
            && self.aim <= DIAL
            && self.power <= DIAL
            && self.reload
                <= RELOAD_TICKS_BASE + u32::from(SOLDIERS) * MISSING_CREW_TICKS
            && self.volley_left <= VOLLEY_HZ
            && self.volleys <= 120
            && self.damage <= 1000
            && self.lost <= SOLDIERS * 4
            && self.taken <= 3
            && self.mourn <= MOURN
            && self.cheer <= CHEER
            && self.retreat <= RETREAT
            && self.moon <= MOON
            && self.pot <= POT
            && self.event_left <= EVENT_WAIT
            && self.limp <= VOLLEY_HZ
            && self.splat <= SPLAT
            && self.felled <= FELLED
            && self.craters.iter().all(|&c| c <= 15)
            && self.keep.rows
                == KEEP_ROWS + castle_of(self.leg).unwrap_or(1).saturating_sub(1)
            && self.keep.blocks.len() == KEEP_COLS * usize::from(self.keep.rows)
            && self.keep.blocks.iter().all(|&b| b <= HIT_DAMAGE)
            && self.keep.defenders <= DEFENDERS + 2
            && self.keep.volleys <= 120
            && u16::from(r.progress) <= u16::from(self.taken)
            && (self.keep.banner == (self.keep.top_of(1) == 0))
            && (self.fighting || self.event_left > 0 || self.mourn > 0)
    }
}

/// The reload clock's base at full crew (the sixth fixed number, named).
pub const RELOAD_TICKS_BASE: u32 = 20;

impl Run {
    /// One tick of one besieger's round. False once it is lost.
    pub(super) fn tick_siege(
        &mut self,
        r: &mut Round,
        inputs: &BTreeMap<u32, Input>,
        eligible: &[(u32, f32, f32)],
    ) -> bool {
        let b = r.siege.as_mut().unwrap();
        if !eligible.iter().any(|&(id, _, _)| id == b.owner) {
            self.found = Some((self.tick, 0, "ENGINE ABANDONED.".into()));
            return false;
        }
        let input = inputs
            .get(&b.owner)
            .filter(|i| i.valid())
            .copied()
            .unwrap_or_default();
        let tapped = inputs.get(&b.owner).is_some_and(|i| i.valid() && i.fire);
        // The engine is a feverish greedy loop on the pastimes' fire-latch:
        // F here answers the volley clock, so we use the raw key edge we are
        // given, through the round's own taps. The latched set is cleared by
        // the caller; a held key must not loose every tick.
        let alive = b.step(input, tapped).unwrap_or(false);
        r.progress = b.taken;
        if b.shot.is_some() || b.castle_shot.is_some() {
            // keep the round's "something in the air" cue fresh
            r.hazard_left = 4;
        }
        if b.cheer == CHEER && b.taken == r.progress {
            self.sounds.push("spoil_pickup");
        }
        if b.mourn == MOURN {
            self.sounds.push("hero_hurt");
        }
        if b.moon == MOON {
            self.sounds.push("clang");
        }
        alive
    }
}
