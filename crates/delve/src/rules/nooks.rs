//! Nooks: lived-in rooms below ground. Each floor of a delve hangs one off
//! a fight room, a dead end with its own furniture and its own small
//! business, played by the whole party at once:
//!
//! - **Grubb's Kitchen** (Cavern, Fungal): steal the stew while the goblin
//!   cook's back is turned; anyone who moves while he looks gets the ladle,
//!   and he shouts for his cousins.
//! - **The Quiet Room** (Archive, Unknown): read the books on the lecterns
//!   for coin; three of them are mimics.
//! - **The Great Bellows** (Cavern, Hellforge): pump the two bellows in
//!   turn until the forge roars and tempers every arm; the heat wakes imps.
//! - **The Bone Dominoes** (Crypt, Unknown): topple a winding line of
//!   bones; its gaps want a knight standing in them to fall over in turn.
//!   A stalled line raises skeletons.
//! - **Mother Mort's Still** (Hellforge, Fungal): two spoils from the
//!   knight's own pack, two jars, one brew; the same jar twice is a frog.
//! - **The Bell Loft** (Crypt, Archive): the ghost choirmaster rings a
//!   tune, the party rings it back; a wrong bell calls a wraith.
//!
//! Nooks only add: their room is extra, never a fight room taken away, and
//! what they raise is on top of the floor's own monsters. Placement draws
//! nothing from the dice, so every other room stays as the seed laid it.
//! Every number is fixed.
use super::*;

mod bellows;
mod kitchen;
mod loft;
mod ossuary;
mod reading;
mod still;
pub use bellows::{HANDLES, HEAT_FULL, HEAT_IMPS};
pub use kitchen::{Cook, POT, SCOOPS, cook};
pub use loft::{BELLS, SEQS, Show, show};
pub use ossuary::{GAPS, PATH, at as domino_at, is_gap};
pub use reading::{LECTERNS, stand as lectern_stand};
pub use still::{CAULDRON, JARS, MORT as STILL_MORT};
pub use bellows::FORGE as BELLOWS_FORGE;
pub use ossuary::COFFER as OSSUARY_COFFER;
pub use kitchen::STOVE as KITCHEN_STOVE;
pub use loft::under as loft_under;

/// The six nooks, as room kinds.
pub const KINDS: [RoomKind; 6] = [
    RoomKind::Kitchen,
    RoomKind::Reading,
    RoomKind::Bellows,
    RoomKind::Ossuary,
    RoomKind::Still,
    RoomKind::Loft,
];

impl RoomKind {
    /// A nook below ground.
    pub fn nook(self) -> bool {
        KINDS.contains(&self)
    }
}

/// The two nooks each pack keeps; a floor takes one by its depth.
pub fn of_pack(pack: Pack) -> [RoomKind; 2] {
    match pack {
        Pack::Crypt => [RoomKind::Ossuary, RoomKind::Loft],
        Pack::Cavern => [RoomKind::Kitchen, RoomKind::Bellows],
        Pack::Hellforge => [RoomKind::Bellows, RoomKind::Still],
        Pack::Archive => [RoomKind::Reading, RoomKind::Loft],
        Pack::Fungal => [RoomKind::Still, RoomKind::Kitchen],
        Pack::Unknown => [RoomKind::Reading, RoomKind::Ossuary],
        Pack::Swamp => [RoomKind::Still, RoomKind::Ossuary],
    }
}

/// The nook a floor of `pack` at `depth` keeps.
pub fn for_floor(pack: Pack, depth: u32) -> RoomKind {
    of_pack(pack)[(depth % 2) as usize]
}

/// How many monsters one nook may raise, all told.
pub const RAISE_CAP: u8 = 4;
/// How near (arena units) a knight stands to use a pot, lectern, jar.
pub const REACH: f32 = 1.8;

pub(super) fn fill(room: &mut Room, (c, r, w, h): (i32, i32, i32, i32), tile: Tile) {
    for y in r..r + h {
        for x in c..c + w {
            room.set(x as usize, y as usize, tile);
        }
    }
}

/// A nook's room: walls, a plain floor, its furniture, one doorway.
pub fn room(kind: RoomKind, cell: (i32, i32), door: usize) -> Room {
    let mut room = Room::solid_rock(cell);
    room.kind = kind;
    fill(&mut room, (1, 1, COLS as i32 - 2, ROWS as i32 - 2), Tile::Floor);
    let blocks: Vec<(i32, i32, i32, i32)> = match kind {
        RoomKind::Kitchen => kitchen::BLOCKS.to_vec(),
        RoomKind::Reading => reading::blocks(),
        RoomKind::Bellows => bellows::BLOCKS.to_vec(),
        RoomKind::Ossuary => ossuary::BLOCKS.to_vec(),
        RoomKind::Still => still::BLOCKS.to_vec(),
        _ => loft::blocks(),
    };
    for rect in blocks {
        fill(&mut room, rect, Tile::Block);
    }
    room.open_door(door);
    room
}

/// Hang the floor's nook off its last fight room with a free cell beside
/// it, touching nothing else if it can. No dice; true if one was added.
pub fn add_nook(floor: &mut Floor) -> bool {
    if floor.depth == 0
        || floor.depth >= DEEPEST
        || floor.rooms.len() >= 63
        || floor.rooms.iter().any(|r| r.kind.nook())
    {
        return false;
    }
    let kind = for_floor(floor.pack, floor.depth);
    let taken = |c: (i32, i32)| floor.rooms.iter().any(|r| r.cell == c);
    // A fight room first, else a passage, the treasure, the entrance; the
    // last such room first (the deepest, as the floor was laid out). The
    // bellows' forge fills the north wall, so their doorway is never north.
    let hosts: [&[RoomKind]; 4] = [
        &[RoomKind::Fight],
        &[RoomKind::Hall],
        &[RoomKind::Treasure],
        &[RoomKind::Start, RoomKind::Sanctuary],
    ];
    let dirs: &[usize] = if kind == RoomKind::Bellows { &[1, 3, 0] } else { &[1, 3, 0, 2] };
    for want in hosts {
        for lonely in [true, false] {
            for i in (0..floor.rooms.len()).rev() {
                let host = &floor.rooms[i];
                if !want.contains(&host.kind) || host.great() || host.cols != COLS || host.rows != ROWS {
                    continue;
                }
                for &dir in dirs {
                    let (dx, dy) = DIRS[dir];
                    let cell = (host.cell.0 + dx, host.cell.1 + dy);
                    if taken(cell) || !(-16..=16).contains(&cell.0) || !(-16..=16).contains(&cell.1) {
                        continue;
                    }
                    let touching = DIRS.iter().filter(|(ex, ey)| taken((cell.0 + ex, cell.1 + ey))).count();
                    if lonely && touching > 1 {
                        continue;
                    }
                    let mut opened = host.clone();
                    opened.open_door(dir);
                    if !layout::connected(&opened) {
                        continue;
                    }
                    floor.rooms[i] = opened;
                    floor.rooms.push(room(kind, cell, (dir + 2) % 4));
                    return true;
                }
            }
        }
    }
    false
}

/// The floor's nook at play: which room, what is under way there, who
/// has done what. One struct for all six; the fields a nook does not use
/// stay at their defaults.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Nook {
    pub room: usize,
    /// Ticks since the party first stepped in.
    pub t: u32,
    pub done: bool,
    /// Knights holding E last tick (a bit each): a press, never a hold.
    pub held: u8,
    /// What each knight has done: scoops, pages, pumps, falls, brews, rings.
    pub score: BTreeMap<u32, u16>,
    /// Monsters raised so far.
    pub raised: u8,
    /// Kitchen: scoops left, ticks into the cook's beat, which beat, and
    /// who has been caught this look.
    pub pot: u8,
    pub cook: u32,
    pub beat: u8,
    pub caught: u8,
    /// Quiet Room: books opened, and which are mimics.
    pub opened: u16,
    pub mimics: u16,
    /// Bellows: the heat, and the bellows last pumped (1 or 2).
    pub heat: u32,
    pub last: u8,
    /// Dominoes: how many have fallen, whether the line is falling, ticks
    /// to the next, ticks before a stalled line stands again.
    pub fallen: u8,
    pub running: bool,
    pub next: u32,
    pub stalled: u32,
    /// Still: each knight's first jar (1 to 3), and brews brewed.
    pub jars: BTreeMap<u32, u8>,
    pub brews: BTreeMap<u32, u8>,
    /// Loft: the round, ticks into its showing, bells rung right so far,
    /// and the bell last lit with ticks left.
    pub round: u8,
    pub shown: u32,
    pub pos: u8,
    pub lit: Option<(u8, u32)>,
}

impl Nook {
    pub fn valid(&self, rooms: usize) -> bool {
        let seats = |m: &BTreeMap<u32, u16>| m.len() <= MAX_PLAYERS as usize && m.keys().all(|id| (1..=MAX_PLAYERS).contains(id));
        self.room < rooms
            && seats(&self.score)
            && self.raised <= RAISE_CAP
            && self.pot <= SCOOPS
            && self.beat < kitchen::BEATS.len() as u8
            && self.cook <= kitchen::beat_len(usize::from(self.beat))
            && self.opened < 1 << LECTERNS.len()
            && self.mimics < 1 << LECTERNS.len()
            && self.heat <= HEAT_FULL
            && self.last <= 2
            && usize::from(self.fallen) <= PATH.len()
            && self.next <= ossuary::FALL
            && self.stalled <= ossuary::STAND_AGAIN
            && self.jars.len() <= MAX_PLAYERS as usize
            && self.jars.iter().all(|(id, &j)| (1..=MAX_PLAYERS).contains(id) && (1..=3).contains(&j))
            && self.brews.len() <= MAX_PLAYERS as usize
            && self.brews.iter().all(|(id, &n)| (1..=MAX_PLAYERS).contains(id) && n <= still::BREWS)
            && usize::from(self.round) <= SEQS.len()
            && self.pos as usize <= SEQS.get(usize::from(self.round)).map_or(0, |s| s.len())
            && self.shown <= loft::show_len(usize::from(self.round.min(SEQS.len() as u8 - 1)))
            && self.lit.is_none_or(|(b, t)| usize::from(b) < BELLS.len() && t <= loft::LIT)
    }
}

impl Run {
    /// A new floor: find its nook and set it up.
    pub(super) fn ready_nook(&mut self) {
        self.nook = self
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind.nook())
            .map(|room| {
                let kind = self.dungeon.rooms[room].kind;
                Nook {
                    room,
                    pot: SCOOPS,
                    mimics: if kind == RoomKind::Reading { reading::mimics(&self.dungeon) } else { 0 },
                    ..Default::default()
                }
            });
    }

    /// The knights who can play: standing, not stone, not in the privy.
    pub(super) fn nook_players(&self) -> Vec<u32> {
        self.players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone && h.privy == 0)
            .map(|(&id, _)| id)
            .collect()
    }

    /// Knight `id` stands within reach of tile point `at`.
    pub(super) fn nook_near(&self, id: u32, at: (f32, f32), reach: f32) -> bool {
        self.players
            .get(&id)
            .is_some_and(|h| (h.x - at.0 * TILE_UNITS).hypot(h.y - at.1 * TILE_UNITS) < reach)
    }

    /// Raise a monster at tile point `at`, while the nook has any to raise.
    pub(super) fn nook_raise(&mut self, nook: &mut Nook, kind: EnemyKind, at: (f32, f32)) {
        if nook.raised >= RAISE_CAP {
            return;
        }
        nook.raised += 1;
        self.spawn_at(kind, at.0 * TILE_UNITS, at.1 * TILE_UNITS);
    }

    /// The knight who did most is crowned, when friends played too.
    pub(super) fn nook_crown(&mut self, nook: &Nook) -> Option<u32> {
        let best = nook.score.iter().max_by_key(|&(&id, &n)| (n, std::cmp::Reverse(id))).map(|(&id, _)| id)?;
        if self.party_len() > 1 {
            for (&id, hero) in self.players.iter_mut() {
                hero.laurel = id == best;
            }
        }
        Some(best)
    }

    pub(super) fn tick_nook(&mut self, inputs: &BTreeMap<u32, Input>) {
        let Some(mut nook) = self.nook.take() else {
            return;
        };
        let held = self
            .players
            .keys()
            .filter(|id| inputs.get(id).is_some_and(|i| i.valid() && i.interact))
            .fold(0u8, |bits, &id| bits | 1 << (id & 7));
        let pressed = held & !nook.held;
        nook.held = held;
        if self.at == nook.room && !nook.done && nook.room < self.dungeon.rooms.len() {
            nook.t = nook.t.saturating_add(1);
            match self.dungeon.rooms[nook.room].kind {
                RoomKind::Kitchen => self.tick_kitchen(&mut nook, inputs, pressed),
                RoomKind::Reading => self.tick_reading(&mut nook, pressed),
                RoomKind::Bellows => self.tick_bellows(&mut nook, pressed),
                RoomKind::Ossuary => self.tick_ossuary(&mut nook, pressed),
                RoomKind::Still => self.tick_still(&mut nook, pressed),
                RoomKind::Loft => self.tick_loft(&mut nook, pressed),
                _ => {}
            }
        }
        self.nook = Some(nook);
    }
}

/// Whether knight `id`'s bit is in `bits`.
pub(super) fn has(bits: u8, id: u32) -> bool {
    bits & 1 << (id & 7) != 0
}

#[cfg(test)]
#[path = "../../tests/rules/together_nooks__tests.rs"]
mod tests;
