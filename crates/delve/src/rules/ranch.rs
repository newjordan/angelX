//! The horse ranch beside the stables, and the flower fields beyond it.
//!
//! The ranch keeps the realm's working horses: a draft at the hitching rail,
//! brushed till he shines, and a paddock of war horses. Its pride is
//! **Helios**, a Belgian draft, brown with a white star and a yellow mane.
//! Nobody rides Helios and nobody buys him. In the morning he runs the
//! flower fields, and with the low sun behind him his mane and tail look
//! like fire.
//!
//! The one way to have something of him: bring a herd mare to the stud
//! book in front of his stall. The fee is paid every time
//! and he is particular: a foal comes once in a long while. A foal grows up
//! in the fields as the realm wins its delves.
//!
//! Like the stables, the run only marks what was asked (`ranch:breed`); the
//! cockpit settles it into the realm (`settle`), paying from the treasury.
use super::world::{Dest, Entrance};
use super::*;
use crate::realm::chivalry::{Bloodline, Horse, Stable};
use crate::realm::{Spoil, Spoils};

pub const ROOMS: [RoomKind; 2] = [RoomKind::Ranch, RoomKind::Pasture];

pub fn is_room(kind: RoomKind) -> bool {
    ROOMS.contains(&kind)
}

pub fn label(kind: RoomKind) -> &'static str {
    match kind {
        RoomKind::Pasture => "The Flower Fields",
        _ => "The Horse Ranch",
    }
}

pub const CELLS: [(RoomKind, (i32, i32)); 2] = [
    (RoomKind::Ranch, (-1, -9)),
    (RoomKind::Pasture, (-1, -10)),
];

pub const STABLES_GATE: (i32, i32, i32, i32) = (1, 11, 2, 1);
pub const RANCH_OUT: (i32, i32, i32, i32) = (1, 12, 2, 1);
pub const FIELDS_GATE: (i32, i32, i32, i32) = (19, 1, 3, 1);
pub const FIELDS_OUT: (i32, i32, i32, i32) = (6, 12, 2, 1);

pub const DOORS: [Entrance; 4] = [
    Entrance {
        id: "ranch-gate",
        room: RoomKind::Stables,
        at: STABLES_GATE,
        to: Dest::Room(RoomKind::Ranch, (3.0, 10.5)),
        label: "THE HORSE RANCH",
        needs: None,
    },
    Entrance {
        id: "ranch-out",
        room: RoomKind::Ranch,
        at: RANCH_OUT,
        to: Dest::Room(RoomKind::Stables, (3.5, 9.5)),
        label: "THE STABLES",
        needs: None,
    },
    Entrance {
        id: "fields-gate",
        room: RoomKind::Ranch,
        at: FIELDS_GATE,
        to: Dest::Room(RoomKind::Pasture, (12.0, 10.5)),
        label: "THE FLOWER FIELDS",
        needs: None,
    },
    Entrance {
        id: "fields-out",
        room: RoomKind::Pasture,
        at: FIELDS_OUT,
        to: Dest::Room(RoomKind::Ranch, (20.5, 3.0)),
        label: "THE RANCH",
        needs: None,
    },
];

pub const BARN: (i32, i32, i32, i32) = (2, 1, 8, 4);
pub const STALL: (i32, i32, i32, i32) = (12, 1, 5, 3);
pub const STUD_BOOK: (i32, i32, i32, i32) = (13, 4, 3, 1);
pub const LECTERN: (i32, i32, i32, i32) = (16, 4, 1, 1);
pub const RAIL: (i32, i32, i32, i32) = (3, 7, 5, 2);
pub const HAY: (i32, i32, i32, i32) = (10, 10, 2, 1);
pub const PADDOCK: (i32, i32, i32, i32) = (14, 7, 9, 5);
pub const GROOM_AT: (f32, f32) = (9.0, 8.0);
pub const DRAFT_AT: (f32, f32) = (5.5, 8.0);

pub const OAK: (i32, i32, i32, i32) = (3, 2, 3, 3);
pub const TROUGH: (i32, i32, i32, i32) = (18, 10, 2, 1);

pub const FEE: u32 = 400;
pub const HELIOS_BLOOD: Bloodline = Bloodline {
    pace: 9, stamina: 9, corner: 8, nerve: 9,
};
pub const ODDS: u64 = 25;
pub const YEARLING: u32 = 2;
pub const GROWN: u32 = 5;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Ranch {
    pub coverings: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foal: Option<Foal>,
    pub helios_bred: bool,
    pub wins: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Foal {
    pub name: String,
    pub filly: bool,
    pub born: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Age {
    Foal,
    Yearling,
    Grown,
}

pub fn age(foal: &Horse, raids_won: u32) -> Age {
    match raids_won.saturating_sub(foal.born.unwrap_or(0)) {
        n if n >= GROWN => Age::Grown,
        n if n >= YEARLING => Age::Yearling,
        _ => Age::Foal,
    }
}

pub fn herd_foal(stable: &Stable) -> Option<&Horse> {
    stable.herd.iter().find(|h| h.sire == "Helios" && h.born.is_some())
}

pub fn helios_foal(name: &str, mare: bool, born: u32, dam: &Horse) -> Horse {
    Horse {
        name: name.into(), house: dam.house.clone(), sire: "Helios".into(),
        dam: dam.name.clone(), blood: HELIOS_BLOOD, mare, born: Some(born),
        ..Horse::default()
    }
}

pub fn migrate(ranch: &mut Ranch, stable: &mut Stable) -> bool {
    let Some(old) = ranch.foal.take() else { return false; };
    ranch.helios_bred = true;
    if !stable.herd.iter().any(|h| h.name == old.name && h.sire == "Helios") {
        let dam = Horse { name: "Mist".into(), house: "keep".into(), mare: true, ..Horse::default() };
        stable.herd.push(helios_foal(&old.name, old.filly, old.born, &dam));
        stable.foals = stable.foals.saturating_add(1);
    }
    true
}

impl Ranch {
    pub fn valid(&self) -> bool {
        self.foal.as_ref().is_none_or(|f| f.name.len() <= 24)
    }
}

fn lucky(coverings: u32, gold: u32, raids_won: u32) -> bool {
    let mut x = (u64::from(coverings) << 40) ^ (u64::from(gold) << 8) ^ u64::from(raids_won);
    x ^= 0x9e37_79b9_7f4a_7c15;
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x % ODDS == 0
}

const NOT_TODAY: [&str; 4] = [
    "Helios flicked an ear at the mare and went back to the clover",
    "Helios and the mare grazed side by side, and that was all",
    "Helios tossed his mane in the sun and trotted off",
    "Not today: Helios had the whole field to run",
];

pub fn settle(
    ranch: &mut Ranch,
    treasury: &mut Spoils,
    stable: &mut Stable,
    raids_won: u32,
    marks: &BTreeMap<String, u32>,
) -> (bool, Vec<String>) {
    let mut said = Vec::new();
    let grew = migrate(ranch, stable) || ranch.wins != raids_won;
    ranch.wins = raids_won;
    if !marks.contains_key("ranch:breed") {
        return (grew, said);
    }
    if ranch.helios_bred || herd_foal(stable).is_some() {
        said.push(herd_foal(stable).map_or_else(
            || "Helios has already given this realm his foal".into(),
            |foal| format!("{} is already in the fields", foal.name)));
        return (grew, said);
    }
    let Some(dam) = stable.mare(raids_won).cloned() else {
        said.push("Bring a grown mare from your herd: buy one at the horse market first".into());
        return (grew, said);
    };
    if stable.herd.len() >= 12 {
        said.push("The herd yard is full: make room for a foal first".into());
        return (grew, said);
    }
    let mut fee = Spoils::default();
    fee.add(Spoil::Gold, FEE);
    if !treasury.covers(&fee) {
        said.push(format!("The stud fee is {FEE} gold"));
        return (grew, said);
    }
    let gold = treasury.get(Spoil::Gold);
    treasury.take(&fee);
    ranch.coverings += 1;
    if lucky(ranch.coverings, gold, raids_won) {
        let filly = ranch.coverings % 2 == 0;
        let name = if filly { "Aurora" } else { "Phaethon" };
        stable.herd.push(helios_foal(name, filly, raids_won, &dam));
        stable.foals = stable.foals.saturating_add(1);
        ranch.helios_bred = true;
        said.push(format!(
            "A {} for {} and Helios: {name}, with his star and his yellow mane",
            if filly { "filly" } else { "colt" }, dam.name
        ));
    } else {
        let n = (ranch.coverings as usize - 1) % NOT_TODAY.len();
        said.push(format!("{} ({FEE} gold)", NOT_TODAY[n]));
    }
    (true, said)
}

pub fn helios(tick: u64) -> ((f32, f32), bool, Option<u32>) {
    const LAP: u64 = 7 * HZ as u64;
    const REST: u64 = 3 * HZ as u64;
    let t = tick % (LAP + REST);
    let (cx, cy, rx, ry) = (12.0f32, 6.8f32, 8.0f32, 3.6f32);
    let at = |phase: f32| {
        let a = phase * std::f32::consts::TAU;
        (cx + rx * a.cos(), cy + ry * a.sin())
    };
    if t < LAP {
        let phase = t as f32 / LAP as f32;
        let a = phase * std::f32::consts::TAU;
        let east = a.sin() < 0.0;
        (at(phase), east, Some((t / 3) as u32))
    } else {
        (at(0.0), false, None)
    }
}

pub fn grazers(tick: u64) -> [((f32, f32), bool); 3] {
    let drift = |seed: f32, base: (f32, f32)| {
        let t = tick as f32 / (HZ as f32 * 9.0) + seed;
        let x = base.0 + 1.6 * t.sin();
        let y = base.1 + 0.6 * (t * 0.7).cos();
        ((x, y), (t.cos()) > 0.0)
    };
    [
        drift(0.0, (6.5, 9.6)),
        drift(2.1, (17.5, 4.0)),
        drift(4.4, (19.5, 8.2)),
    ]
}

pub fn room(kind: RoomKind, cell: (i32, i32)) -> Room {
    let mut r = Room::solid_rock(cell);
    r.kind = kind;
    for y in 1..ROWS - 1 {
        for x in 1..COLS - 1 {
            r.set(x, y, Tile::Floor);
        }
    }
    let mut fill = |(x, y, w, h): (i32, i32, i32, i32), tile: Tile| {
        for yy in y..y + h {
            for xx in x..x + w {
                r.set(xx as usize, yy as usize, tile);
            }
        }
    };
    match kind {
        RoomKind::Ranch => {
            for b in [BARN, STALL, LECTERN, RAIL, HAY, PADDOCK] {
                fill(b, Tile::Block);
            }
        }
        _ => {
            fill(OAK, Tile::Block);
            fill(TROUGH, Tile::Block);
        }
    }
    for e in DOORS.iter().filter(|e| e.room == kind) {
        fill(e.at, Tile::Stairs);
    }
    r
}

fn inside((col, row, w, h): (i32, i32, i32, i32), x: f32, y: f32) -> bool {
    let (c, r) = (x / TILE_UNITS, y / TILE_UNITS);
    c >= col as f32 && c < (col + w) as f32 && r >= row as f32 && r < (row + h) as f32
}

fn near(h: &Hero, at: (f32, f32), tiles: f32) -> bool {
    h.hp > 0
        && !h.stone
        && h.privy == 0
        && (h.x - at.0 * TILE_UNITS).hypot(h.y - at.1 * TILE_UNITS) < tiles * TILE_UNITS
}

const GROOM: [&str; 4] = [
    "That's Helios in the fields. Belgian. Not for sale, not for riding",
    "Old Barley here gets brushed twice a day. Look at him shine",
    "Bring a herd mare to the stud book if you feel lucky. He's particular",
    "War horses in the paddock. Mind the black one, he bites",
];
const HELIOS: [&str; 3] = [
    "Helios won't be caught. Nobody rides Helios",
    "Helios snorts, shakes out that yellow mane, and is gone",
    "He lets you close enough to see the star, then he runs",
];

impl Run {
    pub(super) fn tick_ranch(&mut self, inputs: &BTreeMap<u32, Input>) {
        let kind = self.room().kind;
        if !is_room(kind) {
            return;
        }
        let helios_at = helios(self.tick).0;
        let mut asked = false;
        let mut talk: Option<String> = None;
        for (&id, hero) in self.players.iter_mut() {
            let pressing = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .is_some_and(|i| i.interact);
            if !pressing {
                hero.buy_spent = false;
                hero.buying = 0;
                continue;
            }
            if hero.buy_spent || hero.hp == 0 || hero.stone {
                continue;
            }
            if kind == RoomKind::Ranch && inside(STUD_BOOK, hero.x, hero.y) {
                hero.buying = hero.buying.saturating_add(1);
                if hero.buying >= super::home::BUY_HOLD {
                    hero.buying = 0;
                    hero.buy_spent = true;
                    asked = true;
                }
                continue;
            }
            let said = if kind == RoomKind::Ranch && near(hero, GROOM_AT, 2.4) {
                Some(GROOM[(self.tick / 7) as usize % GROOM.len()])
            } else if kind == RoomKind::Ranch && near(hero, DRAFT_AT, 2.6) {
                Some("Old Barley, the ranch's draft: braided, brushed, and very pleased with himself")
            } else if kind == RoomKind::Pasture && near(hero, helios_at, 3.0) {
                Some(HELIOS[(self.tick / 5) as usize % HELIOS.len()])
            } else {
                None
            };
            if let Some(line) = said {
                hero.buy_spent = true;
                talk = Some(line.into());
            }
        }
        if asked {
            *self.marks.entry("ranch:breed".into()).or_default() += 1;
            self.sounds.push("play_card");
            let mare = self.home.stable.mare(self.home.ranch.wins)
                .map(|h| h.name.as_str()).unwrap_or("A herd mare");
            self.found = Some((self.tick, 0, format!("{mare} is led out to meet Helios")));
        }
        if let Some(line) = talk {
            self.found = Some((self.tick, 0, line));
            self.cues.push("npc:groom".into());
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_ranch__tests.rs"]
mod tests;
