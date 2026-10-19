//! The Swamp of Horrors: a delve of its own behind a plate in Sir Damp's
//! Marsh (the realm's Swamp, on the Wetland), three floors of mire, and the
//! white horse the Mire King penned in his hall at the bottom.
//!
//! - **The hook.** A notice post by the plate (E reads it), and Wren's
//!   bounty for the horse (`bounties`).
//! - **The way in.** The plate (`world::ENTRANCES`, `Dest::Delve(Swamp)`)
//!   goes straight down to floor one, whatever landing Wren drew.
//! - **The ground.** Bog (`Tile::Bog`) holds the feet of knights and walking
//!   monsters to `BOG_PACE`; shots and wings pass over it. Reed islands
//!   stand dry with their clumps on them, gas seeps bubble (`perils`), and
//!   fog lies in patches (`fog`): the Lights Out mode's darkness, local.
//! - **The horrors** (`horrors`): bog hands, drowned knights, will-o'-wisps.
//! - **The Mire King** (`the-mire-king.boss`, `where swamp`) guards the last
//!   floor's hall, the white horse penned behind him. While he stands the
//!   pen holds; his fall opens it (hold E, as at any cage), and once she is
//!   out the light leads home.
//! - **After.** She joins the stable's herd (`settle`), a white mare by name,
//!   and grazes in the Flower Fields; she races and jousts as any herd horse
//!   does. Riding her is a later step.
//!
//! The numbers are the named constants below; nothing here draws from the
//! run's dice.

use super::countryside::Country;
use super::*;
use crate::realm::chivalry::{Bloodline, Horse, HorseCoat, Stable};

/// The swamp's floors: the third is the Mire King's.
pub const FLOORS: u32 = 3;
/// How fast feet go in the bog, of their pace on dry ground.
pub const BOG_PACE: f32 = 0.55;
/// The share of a swamp room's open ground (percent) that is bog, by floor.
pub const BOG_SHARE: [usize; 3] = [35, 45, 50];
/// Fog patches a swamp room holds, by floor, and how far one spreads (units).
pub const FOG_PATCHES: [usize; 3] = [1, 2, 2];
pub const FOG_REACH: f32 = 7.0;
/// Fog keeps this far off a doorway, so nobody walks in blind.
const FOG_DOOR_CLEAR: f32 = 8.0;

/// The plate in Sir Damp's Marsh (tiles), and the notice post beside it.
pub const PLATE: (i32, i32, i32, i32) = (13, 8, 2, 1);
pub const NOTICE: (f32, f32) = (15.8, 8.0);
/// A knight this close to the post (tiles) reads it.
pub const NOTICE_REACH: f32 = 2.0;

/// The captive's id in the pen, and the mark her rescue leaves.
pub const HORSE: &str = "white-horse";
pub const RESCUE_MARK: &str = "rescue:white-horse";
/// The notice read: Wren's notice has been seen.
pub const NOTICE_MARK: &str = "country:swamp:notice";
/// The moment Wren's bounty counts (`cue:white_horse`).
pub const MOMENT: &str = "white_horse";
/// The boss file of the swamp's guardian.
pub const KING: &str = "the-mire-king";

/// Her name, the yard she is kept in, and her blood: quick and brave, a
/// horse who lived through the swamp. Fixed, as Helios's is.
pub const HORSE_NAME: &str = "Blanchefleur";
pub const HORSE_HOUSE: &str = "keep";
pub const HORSE_BLOOD: Bloodline = Bloodline {
    pace: 8,
    stamina: 7,
    corner: 8,
    nerve: 9,
};

/// The swamp's marks the private checkpoint journal keeps (`countryside`).
pub const INTENT_MARKS: [&str; 2] = [NOTICE_MARK, RESCUE_MARK];

/// What a knight at the pen hears while the Mire King stands.
pub const PEN_HELD: &str = "THE MIRE KING HOLDS THE PEN.";
/// What the notice post says.
pub const NOTICE_TEXT: &str =
    "NOTICE: A WHITE HORSE WAS TAKEN INTO THE SWAMP. THE PLATE GOES DOWN. BRING HER HOME.";

/// The white horse, as she joins the herd.
pub fn white_horse() -> Horse {
    Horse {
        name: HORSE_NAME.into(),
        house: HORSE_HOUSE.into(),
        blood: HORSE_BLOOD,
        mare: true,
        coat: Some(HorseCoat::White),
        ..Horse::default()
    }
}

/// Whether she is in this stable's herd.
pub fn in_herd(stable: &Stable) -> bool {
    stable
        .herd
        .iter()
        .any(|h| h.name == HORSE_NAME && h.coat == Some(HorseCoat::White))
}

/// A room's own salt for the mire and the fog: its place on its floor.
pub fn salt(cell: (i32, i32), depth: u32) -> u64 {
    mix(u64::from(depth).wrapping_mul(0x9e37_79b9)
        ^ (cell.0 as u64).wrapping_mul(0x51_7cc1)
        ^ (cell.1 as u64).wrapping_mul(0x2545_f491)
        ^ 0x5a3b)
}

fn tier(depth: u32) -> usize {
    (depth.clamp(1, FLOORS) - 1) as usize
}

/// Smooth noise on a grid of three tiles, from the salt alone.
fn wet(x: usize, y: usize, salt: u64) -> f32 {
    let corner = |gx: usize, gy: usize| {
        (mix(salt ^ (gx as u64) << 20 ^ (gy as u64) << 40) & 0xffff) as f32 / 65535.0
    };
    let (fx, fy) = (x as f32 / 3.0, y as f32 / 3.0);
    let (gx, gy) = (fx.floor() as usize, fy.floor() as usize);
    let (tx, ty) = (fx - gx as f32, fy - gy as f32);
    let top = corner(gx, gy) * (1.0 - tx) + corner(gx + 1, gy) * tx;
    let bottom = corner(gx, gy + 1) * (1.0 - tx) + corner(gx + 1, gy + 1) * tx;
    top * (1.0 - ty) + bottom * ty
}

/// The mire over one quadrant of a swamp room (`layout`'s mirrored
/// template): the wettest `BOG_SHARE` of its open ground goes to bog
/// (`,`). The door lanes and the crossing in the middle (its last two
/// columns and rows) stay dry, and so does the ground round every reed
/// clump: a reed island.
pub fn mire(quadrant: &mut [Vec<u8>], depth: u32, salt: u64) {
    let h = quadrant.len();
    let w = quadrant.first().map_or(0, Vec::len);
    let reeds = |q: &[Vec<u8>], x: usize, y: usize| {
        (y.saturating_sub(1)..=(y + 1).min(h - 1))
            .any(|yy| (x.saturating_sub(1)..=(x + 1).min(w - 1)).any(|xx| q[yy][xx] == b'#'))
    };
    let mut open: Vec<(f32, usize, usize)> = Vec::new();
    for y in 0..h.saturating_sub(2) {
        for x in 0..w.saturating_sub(2) {
            if quadrant[y][x] == b'.' && !reeds(quadrant, x, y) {
                open.push((wet(x, y, salt), x, y));
            }
        }
    }
    let all = (0..h).map(|y| (0..w).filter(|&x| quadrant[y][x] == b'.').count()).sum::<usize>();
    let want = (all * BOG_SHARE[tier(depth)] / 100).min(open.len());
    open.sort_by(|a, b| b.0.total_cmp(&a.0).then((a.1, a.2).cmp(&(b.1, b.2))));
    for &(_, x, y) in open.iter().take(want) {
        quadrant[y][x] = b',';
    }
}

/// Where the fog lies in a swamp room: patch centres (arena units), each
/// `FOG_REACH` across, on the bog and clear of the doorways. None outside
/// the swamp's fights.
pub fn fog(pack: Pack, room: &Room, depth: u32) -> Vec<(f32, f32)> {
    if pack != Pack::Swamp || !matches!(room.kind, RoomKind::Fight | RoomKind::Stairs) {
        return Vec::new();
    }
    let (w, h) = (room.width(), room.height());
    let doors: Vec<(f32, f32)> = [(w / 2.0, 0.0), (w, h / 2.0), (w / 2.0, h), (0.0, h / 2.0)]
        .into_iter()
        .enumerate()
        .filter(|&(side, _)| room.doors[side])
        .map(|(_, at)| at)
        .collect();
    let salt = salt(room.cell, depth) ^ 0xf06;
    let mut spots: Vec<(u64, f32, f32)> = Vec::new();
    for row in 2..room.rows.saturating_sub(2) {
        for col in 2..room.cols.saturating_sub(2) {
            if room.tile(col as i32, row as i32) != Tile::Bog {
                continue;
            }
            let (x, y) = ((col as f32 + 0.5) * TILE_UNITS, (row as f32 + 0.5) * TILE_UNITS);
            if doors.iter().any(|&(dx, dy)| (dx - x).hypot(dy - y) < FOG_DOOR_CLEAR) {
                continue;
            }
            spots.push((mix(salt ^ (col as u64) << 16 ^ (row as u64) << 32), x, y));
        }
    }
    spots.sort_by_key(|s| s.0);
    let mut patches: Vec<(f32, f32)> = Vec::new();
    for (_, x, y) in spots {
        if patches.len() >= FOG_PATCHES[tier(depth)] {
            break;
        }
        if patches.iter().all(|&(px, py)| (px - x).hypot(py - y) >= FOG_REACH * 2.0) {
            patches.push((x, y));
        }
    }
    patches
}

/// Whether a point lies in one of the room's fog patches.
pub fn in_fog(patches: &[(f32, f32)], x: f32, y: f32) -> bool {
    patches
        .iter()
        .any(|&(px, py)| (px - x).hypot(py - y) < FOG_REACH)
}

/// Where the pen stands in the Mire King's hall: by a wall the party does
/// not come in by, off the middle where the King himself rises (arena
/// units, the middle of a tile).
pub fn pen_spot(room: &Room) -> (f32, f32) {
    let (w, h) = (room.width(), room.height());
    let sides = [
        (w * 0.72, 9.0),
        (w - 7.0, h * 0.72),
        (w * 0.28, h - 7.0),
        (7.0, h * 0.28),
    ];
    let (x, y) = (0..4)
        .find(|&side| !room.doors[side])
        .map_or(sides[0], |side| sides[side]);
    (
        ((x / TILE_UNITS).floor() + 0.5) * TILE_UNITS,
        ((y / TILE_UNITS).floor() + 0.5) * TILE_UNITS,
    )
}

/// The pen stands on dry ground: the three by three tiles round it.
fn pen_ground(room: &mut Room, (x, y): (f32, f32)) {
    let (col, row) = ((x / TILE_UNITS) as usize, (y / TILE_UNITS) as usize);
    for r in row - 1..=row + 1 {
        for c in col - 1..=col + 1 {
            room.set(c, r, Tile::Floor);
        }
    }
}

/// The realm's half: the notice read, and the white horse home. She joins
/// the herd if the yard has room; if not, she waits at the ranch until it
/// has. Returns whether the realm changed, and what is said.
pub fn settle(
    country: &mut Country,
    stable: &mut Stable,
    marks: &BTreeMap<String, u32>,
) -> (bool, Vec<String>) {
    let mut changed = false;
    let mut said = Vec::new();
    if marks.contains_key(NOTICE_MARK) && !country.swamp_heard {
        country.swamp_heard = true;
        changed = true;
    }
    let fresh = marks.contains_key(RESCUE_MARK);
    if fresh && country.white_horse == 0 {
        country.white_horse = 1;
        changed = true;
    }
    if country.white_horse == 1 {
        if in_herd(stable) {
            country.white_horse = 2;
            changed = true;
        } else if stable.herd.len() < crate::realm::chivalry::HERD_CAP {
            stable.herd.push(white_horse());
            country.white_horse = 2;
            changed = true;
            said.push(format!(
                "{HORSE_NAME}, the white horse out of the swamp, joins the herd: she grazes in the Flower Fields"
            ));
        } else if fresh {
            said.push(format!(
                "The herd yard is full: {HORSE_NAME} waits at the ranch until there is room"
            ));
        }
    }
    (changed, said)
}

impl Run {
    /// The floor this delve counts toward the realm's depth records (the
    /// deepest floor reached, a bounty to reach a floor, the Map station's
    /// landings): the floor it is on, but 0 in the swamp, a delve of its
    /// own whose three floors lie beside the keep's and count for none of
    /// them.
    pub fn record_floor(&self) -> u32 {
        if self.dungeon.pack == Pack::Swamp {
            0
        } else {
            self.dungeon.depth
        }
    }

    /// The notice post by the plate in the Marsh: E reads it.
    pub(super) fn tick_swamp_notice(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.room().kind != RoomKind::Marsh {
            return;
        }
        let reader = self.players.iter().find_map(|(&id, h)| {
            (h.hp > 0
                && !h.stone
                && h.privy == 0
                && inputs.get(&id).is_some_and(|i| i.valid() && i.interact)
                && (h.x - NOTICE.0 * TILE_UNITS).hypot(h.y - NOTICE.1 * TILE_UNITS)
                    < NOTICE_REACH * TILE_UNITS)
                .then_some(id)
        });
        let Some(reader) = reader else {
            return;
        };
        if !self.home.countryside.swamp_heard && !self.marks.contains_key(NOTICE_MARK) {
            self.sounds.push("play_card");
        }
        self.marks.insert(NOTICE_MARK.into(), 1);
        self.found = Some((self.tick, reader, NOTICE_TEXT.into()));
    }

    /// A swamp floor: on the last, the white horse in the Mire King's hall,
    /// unless the realm already has her.
    pub(super) fn pen_the_horse(&mut self) {
        self.captive = None;
        if self.dungeon.pack != Pack::Swamp
            || self.dungeon.depth < FLOORS
            || self.home.countryside.white_horse != 0
            || in_herd(&self.home.stable)
        {
            return;
        }
        let Some(room) = self
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Stairs)
        else {
            return;
        };
        let (x, y) = pen_spot(&self.dungeon.rooms[room]);
        pen_ground(&mut self.dungeon.rooms[room], (x, y));
        self.captive = Some(rescues::Captive {
            who: HORSE.into(),
            room,
            x,
            y,
            freed: None,
        });
    }

    /// The Mire King's hall on the swamp's last floor.
    pub fn in_mire_kings_hall(&self) -> bool {
        self.dungeon.pack == Pack::Swamp
            && self.dungeon.depth >= FLOORS
            && self.room().kind == RoomKind::Stairs
    }

    /// The Mire King is fallen: his floor is reclaimed and banked, and the
    /// pen opens. The light home comes once the horse is out.
    pub(super) fn mire_king_falls(&mut self) {
        self.reclaimed.push(Pack::Swamp);
        self.cues.push("mire_king_falls".into());
        self.sounds.push("boss_fall");
        self.fx.push(Fx::Shake(30));
        self.bank_all(true, "the Mire King is fallen");
        let penned = self
            .captive
            .as_ref()
            .is_some_and(|c| c.room == self.at && c.freed.is_none());
        self.found = Some((
            self.tick,
            0,
            if penned {
                "THE MIRE KING IS FALLEN. HOLD E AT THE PEN.".into()
            } else {
                "THE MIRE KING IS FALLEN. THE LIGHT LEADS HOME.".into()
            },
        ));
        if !penned {
            self.light_the_way_home();
        }
    }

    /// The light home in the middle of the hall: stood in, the delve is won.
    pub(super) fn light_the_way_home(&mut self) {
        if self.light.is_some() {
            return;
        }
        let room = self.room();
        self.light = Some((room.width() / 2.0, room.height() / 2.0));
        self.stairs_held = true;
        self.cues.push("homeward_light".into());
    }

    /// She is out of the pen.
    pub(super) fn horse_freed(&mut self, by: u32) {
        self.cues.push(MOMENT.into());
        self.notice("white_horse");
        self.found = Some((self.tick, by, format!("{} IS FREE!", HORSE_NAME.to_uppercase())));
        if self.dungeon.rooms[self.at].cleared {
            self.light_the_way_home();
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_swamp__tests.rs"]
mod tests;
