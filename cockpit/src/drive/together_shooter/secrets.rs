//! Secret rooms, the way the old dungeons kept them: now and then a fight
//! room has a cracked wall, and behind it a vault no map shows. A knight's
//! bomb in that room brings the wall down, and so does a sapper's keg or a
//! hob's bomb going off beside the crack. The Herald did not know about it,
//! and is furious.
//!
//! And the small secrets: a summoning card hidden under something on a
//! floor, lifted once by a deliberate act at its place (a vigil, a raised
//! shield, a bomb, a draught drunk, a spoil offered). Passing by only earns
//! the Herald's hint, once. A lift counts for Wren's bounty as a vault does.

use super::*;
use crate::drive::together_realm::{Spoil, Spoils};
use std::collections::VecDeque;

/// A keg or a hob's bomb this close to the crack brings it down too.
pub(crate) const CRACK_REACH: f32 = 7.0;
/// How close a knight comes before Snibbet pays up, and how many piles.
const SNIBBET_REACH: f32 = 5.0;
const HUSH_MONEY: usize = 8;

/// Where Snibbet sits in his vault: its middle, a little north.
pub(crate) fn snibbet_at(room: &Room) -> (f32, f32) {
    (room.width() / 2.0, room.height() / 2.0 - 2.0)
}

/// West of the centre stair pad, on floor if the generator left any.
/// The pad is a 2×2 of stair tiles; standing on it goes down.
pub(crate) fn stair_landing(room: &Room) -> (f32, f32) {
    let cx = room.cols as i32 / 2 - 1;
    let cy = room.rows as i32 / 2 - 1;
    for (dc, dr) in [(-1, 0), (-1, 1), (-2, 0), (-2, 1), (2, 0), (0, -1), (0, 2)] {
        let (c, r) = (cx + dc, cy + dr);
        if room.tile(c, r) == Tile::Floor {
            return ((c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS);
        }
    }
    (
        (cx as f32 - 0.5) * TILE_UNITS,
        (cy as f32 + 0.5) * TILE_UNITS,
    )
}

/// The north lip of the Pit, off the beast's perch in the middle.
pub(crate) fn sunk_reed_at(room: &Room) -> (f32, f32) {
    (room.width() / 2.0, room.height() / 2.0 - 4.0)
}

/// The Low Wick: the side-on ledge's far east nook, past its second pit,
/// where a knight stands on the last step (a body's middle, `FOOT` above
/// the stone it stands on). Reached only by a jump, never on the way in.
pub(crate) fn low_wick_at(room: &Room) -> (f32, f32) {
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    for c in (cols / 2..cols - 1).rev() {
        for r in (1..rows - 1).rev() {
            let stone = matches!(room.tile(c, r + 1), Tile::Block | Tile::Wall);
            if room.tile(c, r) == Tile::Floor && stone {
                let x = (c as f32 + 0.5) * TILE_UNITS;
                return (x, (r + 1) as f32 * TILE_UNITS - ledge::FOOT);
            }
        }
    }
    (room.width() - 3.0, room.height() / 2.0)
}

/// The Damp Sill: the wet stone at a hall's crossing.
pub(crate) fn damp_sill_at(room: &Room) -> (f32, f32) {
    (room.width() / 2.0, room.height() / 2.0)
}

/// The room of `kind` farthest from a floor's entrance through its doors
/// (the first of them, when two are as far): the Salt Niche's fight room
/// and the Damp Sill's hall, one a floor.
pub(crate) fn farthest(floor: &Floor, kind: RoomKind) -> Option<usize> {
    let mut dist = vec![usize::MAX; floor.rooms.len()];
    dist[0] = 0;
    let mut queue = VecDeque::from([0]);
    while let Some(i) = queue.pop_front() {
        for dir in 0..4 {
            if let Some(j) = floor.neighbour(i, dir)
                && dist[j] == usize::MAX
            {
                dist[j] = dist[i] + 1;
                queue.push_back(j);
            }
        }
    }
    (0..floor.rooms.len())
        .filter(|&i| floor.rooms[i].kind == kind && dist[i] != usize::MAX)
        .max_by_key(|&i| (dist[i], std::cmp::Reverse(i)))
}

/// What lifts a small secret, done within its reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Act {
    /// A knight keeping vigil.
    Vigil,
    /// A knight keeping vigil while carrying this spoil, one of which the
    /// secret takes.
    Offer(Spoil),
    /// A shield held up.
    Shield,
    /// A knight's bomb going off.
    Bomb,
    /// A healing card played (a potion drunk).
    Draught,
}

/// A small secret waiting in the room the party is in.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Waiting {
    /// Its key's word (`cup:{depth}`), and its cues' (`secret_found:cup`,
    /// `secret_hint:cup`).
    pub(crate) name: &'static str,
    pub(crate) card: &'static str,
    pub(crate) at: (f32, f32),
    pub(crate) reach: f32,
    pub(crate) act: Act,
    /// What the notice line says when it lifts.
    pub(crate) says: &'static str,
    /// The Herald may hint at it now (one hint at a time at one altar).
    pub(crate) hints: bool,
}

impl Run {
    /// A blast in this room: a knight's bomb (`at` none: it fills the
    /// room) or a keg's or a hob's (near the crack, or nothing).
    pub(super) fn blast_wall(&mut self, at: Option<(f32, f32)>) {
        let Some((host, cx, cy)) = self.dungeon.crack() else {
            return;
        };
        if host != self.at || at.is_some_and(|(x, y)| (x - cx).hypot(y - cy) > CRACK_REACH) {
            return;
        }
        if !self.dungeon.open_secret() {
            return;
        }
        self.cues.push("secret_found".into());
        self.sounds.push("rock_land");
        self.sounds.push("door_open");
        self.shake = self.shake.max(16);
        if self.blasts.len() < 16 {
            self.blasts.push((cx, cy, BLAST_TICKS));
        }
        self.found = Some((self.tick, 0, "A secret room!".into()));
        self.notice("secret_room");
        self.thrill(40);
    }

    /// Snibbet's vault: the first knight to walk up to him is paid to
    /// forget he was ever here, gold heaped at their feet.
    pub(super) fn tick_snibbet(&mut self) {
        let Some(secret) = self.dungeon.secret.filter(|s| s.snibbet && !s.paid) else {
            return;
        };
        if self.at != secret.vault {
            return;
        }
        let (sx, sy) = snibbet_at(self.room());
        let near = self
            .players
            .values()
            .any(|h| h.hp > 0 && !h.stone && (h.x - sx).hypot(h.y - sy) < SNIBBET_REACH);
        if !near {
            return;
        }
        if let Some(s) = self.dungeon.secret.as_mut() {
            s.paid = true;
        }
        for k in 0..HUSH_MONEY {
            let a = k as f32 / HUSH_MONEY as f32 * std::f32::consts::TAU;
            self.drop_item(
                "gold".into(),
                sx + a.cos() * 6.0,
                sy + 7.0 + a.sin() * 3.0,
                None,
            );
        }
        self.cues.push("snibbet".into());
        self.sounds.push("card_pickup");
        self.found = Some((self.tick, 0, "It's a secret to everybody.".into()));
        self.thrill(25);
    }

    /// The small secrets waiting in this room, whether or not lifted.
    pub(crate) fn waiting_secrets(&self) -> Vec<Waiting> {
        let (room, depth) = (self.room(), self.dungeon.depth);
        let at_altar = |name, card, act, says, hints| Waiting {
            name,
            card,
            at: room.altar(),
            reach: 3.5,
            act,
            says,
            hints,
        };
        let mut here = Vec::new();
        match room.kind {
            // The Empty Cup, and below it on the deeper floors the Glass
            // Altar, which is hinted at once the cup is lifted.
            RoomKind::Sanctuary if depth >= 2 => {
                here.push(at_altar(
                    "cup",
                    "hearth-bell",
                    Act::Vigil,
                    "Under the empty cup: a Hearth Bell.",
                    true,
                ));
                if depth >= 3 {
                    let cup = self.lifted.contains(&format!("cup:{depth}"));
                    here.push(at_altar(
                        "glass",
                        "glass-needle",
                        Act::Shield,
                        "Under the altar glass: a Glass Needle.",
                        cup,
                    ));
                }
            }
            // The Unbroken Seal: a chest nobody has opened yet.
            RoomKind::Treasure if depth >= 2 => {
                if let Some(chest) = room.chest.filter(|c| !c.open) {
                    here.push(Waiting {
                        name: "seal",
                        card: "rime-whistle",
                        at: (chest.x, chest.y),
                        reach: 6.0,
                        act: Act::Shield,
                        says: "The seal holds. Beside it: a Rime Whistle.",
                        hints: true,
                    });
                }
            }
            // The Salt Niche: the heart of the floor's farthest fight.
            RoomKind::Fight
                if depth >= 2
                    && room.cleared
                    && farthest(&self.dungeon, RoomKind::Fight) == Some(self.at) =>
            {
                here.push(Waiting {
                    name: "salt",
                    card: "salt-thread",
                    at: (room.width() / 2.0, room.height() / 2.0),
                    reach: 3.0,
                    act: Act::Bomb,
                    says: "Salt cracks out of the floor: a Salted Thread.",
                    hints: true,
                });
            }
            // Banked Coal: the heart of the dragon's lair, once it is slain.
            RoomKind::Lair if room.cleared => here.push(Waiting {
                name: "coal",
                card: "cinder-wick",
                at: (room.width() / 2.0, room.height() / 2.0),
                reach: 3.0,
                act: Act::Bomb,
                says: "The banked coal wakes: a Cinder Wick.",
                hints: true,
            }),
            // The Stair Lantern: the landing beside a won stair.
            RoomKind::Stairs if depth >= 2 && room.cleared => here.push(Waiting {
                name: "lantern",
                card: "lantern-mote",
                at: stair_landing(room),
                reach: 3.5,
                act: Act::Draught,
                says: "The stair lantern drinks: a Lantern Mote.",
                hints: true,
            }),
            // The Sunk Reed: the lip of the Pit, its beast slain.
            RoomKind::Pit if depth >= 2 && room.cleared => here.push(Waiting {
                name: "reed",
                card: "reed-flute",
                at: sunk_reed_at(room),
                reach: 3.5,
                act: Act::Draught,
                says: "Up from the pit: a Reed Flute.",
                hints: true,
            }),
            // The Low Wick: the side-on ledge's far nook, with wax, on a
            // floor that sheds it (spoils are banked at every stair).
            RoomKind::Ledge if self.dungeon.pack.kin() == Pack::Crypt => here.push(Waiting {
                name: "wick",
                card: "choir-crumb",
                at: low_wick_at(room),
                reach: 3.0,
                act: Act::Offer(Spoil::Wax),
                says: "The low wick takes your wax: a Choir Crumb.",
                hints: true,
            }),
            // The Damp Sill: the floor's farthest hall, with ore, on a
            // floor of the Mines' kin.
            RoomKind::Hall
                if self.dungeon.pack.kin() == Pack::Cavern
                    && farthest(&self.dungeon, RoomKind::Hall) == Some(self.at) =>
            {
                here.push(Waiting {
                    name: "sill",
                    card: "marrow-sip",
                    at: damp_sill_at(room),
                    reach: 3.0,
                    act: Act::Offer(Spoil::Ore),
                    says: "Off the damp sill: a Marrow Sip.",
                    hints: true,
                })
            }
            _ => {}
        }
        here
    }

    /// Each tick of a delve: a small secret here lifts for its act done
    /// within its reach (`booms`: where knights' bombs went off, `draughts`:
    /// where healing cards were played), or hints once for a knight near it.
    pub(super) fn small_secrets(&mut self, booms: &[(f32, f32)], draughts: &[(f32, f32)]) {
        if self.dungeon.depth == 0 {
            return;
        }
        let depth = self.dungeon.depth;
        for secret in self.waiting_secrets() {
            let key = format!("{}:{depth}", secret.name);
            if self.lifted.contains(&key) {
                continue;
            }
            let (x, y) = secret.at;
            let near = |(px, py): (f32, f32)| (px - x).hypot(py - y) <= secret.reach;
            let by = |hero: &Hero, act: Act| {
                hero.hp > 0
                    && near((hero.x, hero.y))
                    && match act {
                        Act::Vigil => hero.vigil,
                        Act::Offer(spoil) => hero.vigil && hero.carried.get(spoil) > 0,
                        Act::Shield => hero.shielding,
                        Act::Bomb | Act::Draught => false,
                    }
            };
            let done = match secret.act {
                Act::Bomb => booms.iter().any(|&p| near(p)),
                Act::Draught => draughts.iter().any(|&p| near(p)),
                act => self.players.values().any(|hero| by(hero, act)),
            };
            if done {
                if let Act::Offer(spoil) = secret.act
                    && let Some(hero) = self.players.values_mut().find(|h| by(h, secret.act))
                {
                    let mut one = Spoils::default();
                    one.add(spoil, 1);
                    hero.carried.take(&one);
                }
                self.lift(key, &secret);
            } else if secret.hints
                && !self.hinted.contains(&key)
                && self
                    .players
                    .values()
                    .any(|hero| hero.hp > 0 && near((hero.x, hero.y)))
            {
                self.hinted.push(key);
                self.cues.push(format!("secret_hint:{}", secret.name));
            }
        }
    }

    /// A small secret lifts: its card on open floor beside its place, out
    /// of every knight's reach so it is seen before it is taken; the notice
    /// line; and the cue Wren's bounty and the Herald hear. A card the book
    /// cannot hold is no secret to find.
    fn lift(&mut self, key: String, secret: &Waiting) {
        if self.book.get(secret.card).is_none() {
            return;
        }
        let (x, y) = secret.at;
        let grid = Grid {
            room: self.room(),
            barred: true,
        };
        let spots: Vec<(f32, f32)> = [(2.2, 0.0), (-2.2, 0.0), (0.0, 2.2), (0.0, -2.2)]
            .into_iter()
            .map(|(dx, dy)| (x + dx, y + dy))
            .filter(|&(ix, iy)| grid.clear(ix, iy, 0.5, Mover::Hero))
            .collect();
        let clear_of_knights = |&(ix, iy): &(f32, f32)| {
            self.players
                .values()
                .all(|h| (h.x - ix).hypot(h.y - iy) > PICKUP_REACH + 0.5)
        };
        let (ix, iy) = spots
            .iter()
            .copied()
            .find(clear_of_knights)
            .or(spots.first().copied())
            .unwrap_or((x, y));
        let at = self.at;
        self.dungeon.rooms[at].items.push(Item {
            card: secret.card.into(),
            x: ix,
            y: iy,
            held_off: None,
        });
        self.lifted.push(key);
        self.cues.push(format!("secret_found:{}", secret.name));
        self.sounds.push("card_pickup");
        self.found = Some((self.tick, 0, secret.says.into()));
        self.thrill(15);
    }

    /// The first walk into the room with the crack: the Herald notices the
    /// wall.
    pub(super) fn notice_crack(&mut self, first_visit: bool) {
        if first_visit
            && self
                .dungeon
                .crack()
                .is_some_and(|(host, ..)| host == self.at)
        {
            self.cues.push("crack_seen".into());
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_secrets__tests.rs"]
mod tests;
