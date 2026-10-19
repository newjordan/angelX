//! POWDER KEG — one shared courtyard of hay bales, dropped kegs and cross
//! blasts; every knight in the heat plays in it at the same time.
//!
//! # Powder Keg
//!
//! ## The feeling
//! The panic of being boxed in by your own keg's cross of fire, and the glee
//! of trapping a friend against a stone post. The first ten seconds: a keg
//! you dropped a heartbeat ago starts spitting sparks, and you are still
//! standing in one of its four arms — and a friend's keg is burning too.
//!
//! ## The loop (one sentence)
//! Knights walk one shared grid courtyard of hay bales and stone posts, F
//! drops a powder keg that blows a cross after a short fuse, bales burn away
//! and some leave a charm (a longer blast, one more keg, quicker boots), a
//! knight caught in any blast — their own or a friend's — is sooted out of
//! the heat, and the last knight standing wins — or, alone, clear eight
//! bales before the sixty-second clock.
//!
//! ## Words on the title card
//! DROP KEGS · LAST STANDS
//!
//! ## Who plays
//! Every seated knight, at once, in ONE shared courtyard: a keg can trap
//! and soot a friend. That is the point.
//!
//! ## Controls
//! Move, F. Nothing else.
//!
//! ## The five numbers (fixed integers; HZ = 30)
//! 1. FUSE = 90 ticks (3 s) — long enough to walk out of your own arm and
//!    think about a friend's, short enough to sweat.
//! 2. BLAST = 3 tiles — how far each arm of the cross reaches; one charm
//!    steps it to 6. The whole map is readable from this one number.
//! 3. SPEED = 176 — a knight's step, in 256ths of a tile per tick; the
//!    boots charm steps it to 240.
//! 4. KEGS = 1 — how many lit kegs a knight may have at once; the keg charm
//!    steps it to 2.
//! 5. FIRE = 45 — a second and a half of standing flame after the blast,
//!    long enough to read the cross; it soots a knight who walks into it.
//!
//! (Supporting fixed numbers: courtyard 11×9 flagstones at tile (6,2), a
//! stone post on every other tile the way the trope has them, bales on the
//! rest with a lane kept open at each corner, 60 s clock, purse
//! `progress × 75 + time × 5 − mistakes × 25`, clamped 25–1000.)
//!
//! ## Fail and retry
//! A blast soots a knight out of the heat and plants them back on their own
//! start pad, ash-faced but whole, in the same tick (under a second: no
//! corpse timer). A lone knight who runs out of stock or clock simply steps
//! the pad again.
//!
//! ## Version 2 is
//! One shared courtyard, one verb (drop and dodge), sooting a friend, a
//! readable floor (flagstones, posts on every other tile, corner lanes), a
//! screen, flame tongues along the arms, and scorch marks that fade. Left
//! out for a third brief: kicked kegs, foes, riding a blast, a second map.
//!
//! ## Pays
//! The pastime purse: a finish pays 25–1000 gold into the shared score,
//! the heat winner wears the laurel, the rest wear the game's booby prize,
//! and the host saves a best to beat.

use super::*;
use crate::rules::MAX_PLAYERS;
use serde::{Deserialize, Serialize};

/// The courtyard, tiles: 11 wide, 9 tall, origin (6, 2) in room tiles —
/// the middle of the Colosseum's Powder Keg yard, the bench and its start
/// mark to the west (`colosseum`).
pub const W: u8 = 11;
pub const H: u8 = 9;
pub const X0: u8 = 6;
pub const Y0: u8 = 2;
/// The five numbers (see the brief above).
pub const FUSE: u8 = 90;
pub const BLAST: u8 = 3;
pub const BLAST_CHARM: u8 = 6;
pub const SPEED: u16 = 176;
pub const SPEED_BOOTS: u16 = 240;
pub const KEGS: u8 = 1;
pub const KEGS_CHARM: u8 = 2;
pub const STOCK: u8 = 8;
pub const STOCK_CHARM: u8 = 12;
/// How long the cross of flame stays on screen, in ticks. One of the five
/// numbers: raised from 10 so the cross can be read.
pub const FIRE: u8 = 45;
/// How long a scorch mark lingers after its fire, in ticks.
pub const SCORCH: u8 = 200;
/// The four start corners of the shared courtyard, tiles: a knight per
/// corner, each on a lane kept clear of bales and posts.
pub const CORNERS: [(u8, u8); 4] = [(1, 1), (9, 7), (9, 1), (1, 7)];
/// The stone posts: the classic pillar grid, only in the interior (odd
/// rows one step in), so the outer ring stays a walkable lane and the four
/// corners stand clear.
pub const POSTS: [(u8, u8); 10] = [
    (3, 3), (5, 3), (7, 3),
    (3, 5), (5, 5), (7, 5),
    (3, 7), (5, 7), (7, 7),
    (5, 1),
];
/// The hay bales: every other free interior tile, keeping a lane open at
/// each corner. Twenty in all, well past the eight needed to win.
pub const BALES: [(u8, u8); 20] = [
    (2, 2), (4, 2), (6, 2), (8, 2),
    (2, 4), (4, 4), (6, 4), (8, 4),
    (2, 6), (4, 6), (6, 6), (8, 6),
    (2, 8), (4, 8), (6, 8), (8, 8),
    (7, 1), (1, 5), (9, 5), (3, 8),
];

/// One lit keg in the shared courtyard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keg {
    pub col: u8,
    pub row: u8,
    pub left: u8,
    /// Whose keg this is (its blast's reach comes from that knight's charm).
    pub owner: u32,
}

/// One tile of fire while it shows: tile and ticks left.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flame {
    pub col: u8,
    pub row: u8,
    pub left: u8,
    /// The arm's direction from the keg (0 E, 1 W, 2 S, 3 N, 4 the keg).
    /// The tongues are drawn along it.
    pub arm: u8,
}

/// A scorch mark on a tile where fire has been, fading.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scorch {
    pub col: u8,
    pub row: u8,
    pub left: u8,
}

/// A charm lying where a bale burned away.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Charm {
    pub col: u8,
    pub row: u8,
    /// 0 blast, 1 boots, 2 keg.
    pub kind: u8,
}

impl Charm {
    pub fn glyph(self) -> char {
        match self.kind {
            0 => 'r',
            1 => 'W',
            _ => 'k',
        }
    }
}

/// One knight in the shared courtyard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Knight {
    pub id: u32,
    /// Position, in 256ths of a tile, courtyard-relative.
    pub x: u16,
    pub y: u16,
    /// True while this knight stands in the heat.
    pub alive: bool,
    pub stock: u8,
    /// Charms taken: 0 blast, 1 boots, 2 keg.
    pub blast: u8,
    pub boots: bool,
    pub spare: bool,
    /// Ticks of being caught in a blast, for the soot flash.
    pub soot: u8,
}

impl Knight {
    /// The knight's tile, courtyard-relative.
    pub fn tile(&self) -> (u8, u8) {
        ((self.x / 256 % u16::from(W)) as u8, (self.y / 256 % u16::from(H)) as u8)
    }
}

/// The one shared courtyard every knight in the heat plays in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Powder {
    /// The knights standing in this heat.
    pub knights: Vec<Knight>,
    pub kegs: Vec<Keg>,
    pub flames: Vec<Flame>,
    pub scorch: Vec<Scorch>,
    pub charms: Vec<Charm>,
    /// The bales still standing, tiles `row * W + col`.
    pub bales: Vec<u8>,
    /// A fixed seed for the charm drops, so all seats agree.
    pub seed: u8,
    /// How many bales each seat has burned (their progress), by id.
    pub burned: BTreeMap<u32, u8>,
}

impl Powder {
    pub fn new(ids: &[u32], seed: u8) -> Self {
        Self {
            knights: ids
                .iter()
                .enumerate()
                .map(|(i, &id)| {
                    let (c, r) = CORNERS[i % CORNERS.len()];
                    Knight {
                        id,
                        x: u16::from(c) * 256 + 128,
                        y: u16::from(r) * 256 + 128,
                        alive: true,
                        stock: STOCK,
                        blast: BLAST,
                        boots: false,
                        spare: false,
                        soot: 0,
                    }
                })
                .collect(),
            kegs: Vec::new(),
            flames: Vec::new(),
            scorch: Vec::new(),
            charms: Vec::new(),
            bales: BALES.iter().map(|&(c, r)| r * W + c).collect(),
            seed,
            burned: ids.iter().map(|&id| (id, 0)).collect(),
        }
    }

    /// The seats still standing.
    pub fn alive(&self) -> Vec<u32> {
        self.knights.iter().filter(|k| k.alive).map(|k| k.id).collect()
    }

    fn charm_at(&self, col: u8, row: u8) -> Option<usize> {
        self.charms.iter().position(|c| c.col == col && c.row == row)
    }

    fn post(col: u8, row: u8) -> bool {
        POSTS.iter().any(|&(c, r)| c == col && r == row)
    }

    pub fn solid(&self, col: u8, row: u8) -> bool {
        Self::post(col, row)
            || self.bales.iter().any(|&t| t == row * W + col)
            || self.kegs.iter().any(|k| k.col == col && k.row == row)
    }

    /// Charm drops are seed-fixed on the shared grid: every third bale,
    /// taken in reading order, from a start the seed picks. All seats see
    /// the same charms at the same tiles.
    fn drops(&self) -> Vec<u8> {
        let mut v = Vec::new();
        for (i, &t) in self.bales.iter().enumerate() {
            let j = i + usize::from(self.seed);
            if j % 3 == 1 {
                v.push(t);
            }
        }
        v
    }

    /// One 30 Hz tick of the shared courtyard. Returns (sooted ids, a bale
    /// burned, a blast went off, a charm was picked up by whom).
    pub fn step(
        &mut self,
        inputs: &BTreeMap<u32, crate::rules::Input>,
        tapped: &[u32],
    ) -> (Vec<u32>, bool, bool, Vec<u32>) {
        let mut sooted_ids = Vec::new();
        let mut picked = Vec::new();
        let mut out = (false, false, false);
        // Knights pass through one another: the glee is the keg, not the
        // shove. Compute each knight's step against the yard as it stood,
        // then apply, so no knight is blocked by a friend's mid-tick ghost.
        let mut moves: Vec<(u32, u16, u16)> = Vec::new();
        let mut drops: Vec<(u32, u8, u8)> = Vec::new();
        for k in self.knights.iter() {
            if !k.alive {
                continue;
            }
            let input = inputs
                .get(&k.id)
                .filter(|i| i.valid())
                .copied()
                .unwrap_or_default();
            let speed = if k.boots { SPEED_BOOTS } else { SPEED };
            let (mut x, mut y) = (k.x, k.y);
            if input.valid() {
                for (axis, dir) in [(0, input.move_x), (1, input.move_y)] {
                    if dir == 0 {
                        continue;
                    }
                    let (nx, ny) = match axis {
                        0 => (
                            x.saturating_add_signed(i16::from(dir) * (speed as i16 / 8)),
                            y,
                        ),
                        _ => (
                            x,
                            y.saturating_add_signed(i16::from(dir) * (speed as i16 / 8)),
                        ),
                    };
                    let (nc, nr) = (
                        (nx / 256 % u16::from(W)) as u8,
                        (ny / 256 % u16::from(H)) as u8,
                    );
                    let (tc, tr) = ((x / 256 % u16::from(W)) as u8, (y / 256 % u16::from(H)) as u8);
                    // A keg just dropped at your feet does not trap you on
                    // it: you may step off, and only then does it block.
                    let own_feet = (nc, nr) == (tc, tr);
                    let fire_free = !self
                        .flames
                        .iter()
                        .any(|f| f.col == nc && f.row == nr && f.left > FIRE - 8);
                    if own_feet || (!self.solid(nc, nr) && fire_free) {
                        x = nx.min((u16::from(W) - 1) * 256 + 255);
                        y = ny.min((u16::from(H) - 1) * 256 + 255);
                    }
                }
            }
            moves.push((k.id, x, y));
            if tapped.contains(&k.id)
                && k.stock > 0
                && self.kegs.len() + drops.len()
                    < usize::from(if k.spare { KEGS_CHARM } else { KEGS })
                        * self.knights.iter().filter(|n| n.alive).count()
            {
                let (c, r) = ((x / 256 % u16::from(W)) as u8, (y / 256 % u16::from(H)) as u8);
                if !self.solid(c, r)
                    && !self
                        .flames
                        .iter()
                        .any(|f| f.col == c && f.row == r && f.left > FIRE - 8)
                {
                    drops.push((k.id, c, r));
                }
            }
        }
        for k in self.knights.iter_mut() {
            if let Some(&(_, x, y)) = moves.iter().find(|m| m.0 == k.id) {
                k.x = x;
                k.y = y;
            }
        }
        let _alive = self.knights.iter().filter(|n| n.alive).count();
        for (id, c, r) in drops {
            self.kegs.push(Keg {
                col: c,
                row: r,
                left: FUSE,
                owner: id,
            });
            if let Some(k) = self.knights.iter_mut().find(|k| k.id == id) {
                k.stock -= 1;
            }
            out.2 = true;
        }
        for k in self.knights.iter_mut() {
            k.soot = k.soot.saturating_sub(1);
        }
        // Fuses run.
        let mut boomed: Vec<Keg> = Vec::new();
        for kg in self.kegs.iter_mut() {
            kg.left -= 1;
        }
        self.kegs.retain(|kg| {
            if kg.left == 0 {
                boomed.push(*kg);
                false
            } else {
                true
            }
        });
        if !boomed.is_empty() {
            out.2 = true;
        }
        let mut burned: Vec<(u8, u32)> = Vec::new();
        for kg in boomed {
            let reach = self
                .knights
                .iter()
                .find(|k| k.id == kg.owner)
                .map(|k| k.blast)
                .unwrap_or(BLAST);
            // The burst frame: the whole cross at full reach for an instant.
            let mut arms: Vec<Flame> = vec![Flame {
                col: kg.col,
                row: kg.row,
                left: FIRE,
                arm: 4,
            }];
            for (dc, dr) in [(1i16, 0i16), (-1, 0), (0, 1), (0, -1)] {
                for step in 1..=reach {
                    let (c, r) = (
                        u8::try_from(i16::from(kg.col) + dc * i16::from(step)).unwrap_or(W),
                        u8::try_from(i16::from(kg.row) + dr * i16::from(step)).unwrap_or(H),
                    );
                    if c >= W || r >= H {
                        break;
                    }
                    let arm = if dc == 1 {
                        0
                    } else if dc == -1 {
                        1
                    } else if dr == 1 {
                        2
                    } else {
                        3
                    };
                    if Self::post(c, r) {
                        // A stone post stops the arm cold, the trope's hard
                        // block: flame on the post, nothing beyond.
                        arms.push(Flame {
                            col: c,
                            row: r,
                            left: FIRE,
                            arm,
                        });
                        break;
                    }
                    if let Some(&t) = self.bales.iter().find(|&&t| t == r * W + c) {
                        burned.push((t, kg.owner));
                        arms.push(Flame {
                            col: c,
                            row: r,
                            left: FIRE,
                            arm,
                        });
                        break;
                    }
                    if self.kegs.iter().any(|k2| k2.col == c && k2.row == r) {
                        // A chain: the other keg goes off next tick.
                        if let Some(k2) =
                            self.kegs.iter_mut().find(|k2| k2.col == c && k2.row == r)
                        {
                            k2.left = k2.left.min(1);
                        }
                        arms.push(Flame {
                            col: c,
                            row: r,
                            left: FIRE,
                            arm,
                        });
                        break;
                    }
                    arms.push(Flame {
                        col: c,
                        row: r,
                        left: FIRE,
                        arm,
                    });
                }
            }
            // Scorch marks where the fire stood; they fade.
            for f in &arms {
                if !self.scorch.iter().any(|s| s.col == f.col && s.row == f.row) {
                    self.scorch.push(Scorch {
                        col: f.col,
                        row: f.row,
                        left: SCORCH,
                    });
                }
            }
            self.flames.extend(arms.iter().copied());
        }
        if !burned.is_empty() {
            out.1 = true;
            let drops = self.drops();
            for &(t, owner) in &burned {
                self.bales.retain(|&b| b != t);
                *self.burned.entry(owner).or_insert(0) += 1;
                if drops.contains(&t) {
                    let (c, r) = (t % W, t / W);
                    self.charms.push(Charm {
                        col: c,
                        row: r,
                        kind: (t / 7) % 3,
                    });
                }
            }
        }
        // Standing fire soots any knight standing in it — your own blast, a
        // friend's blast, or a flame you walked into. The glee of trapping
        // a friend lives here.
        let catches: Vec<(u32, bool)> = self
            .knights
            .iter()
            .filter(|k| k.alive)
            .map(|k| {
                let (tc, tr) = k.tile();
                (
                    k.id,
                    self.flames
                        .iter()
                        .any(|f| f.col == tc && f.row == tr && f.left > FIRE - 8),
                )
            })
            .collect();
        let takes: Vec<(u32, usize)> = self
            .knights
            .iter()
            .filter(|k| k.alive)
            .filter_map(|k| {
                let (tc, tr) = k.tile();
                self.charm_at(tc, tr).map(|i| (k.id, i))
            })
            .collect();
        for (id, caught) in catches {
            if caught {
                if let Some(k) = self.knights.iter_mut().find(|k| k.id == id) {
                    k.alive = false;
                    k.soot = 30;
                }
                sooted_ids.push(id);
            }
        }
        for (id, i) in takes.iter().rev() {
            let charm = self.charms.remove(*i);
            if let Some(k) = self.knights.iter_mut().find(|k| k.id == *id) {
                match charm.kind {
                    0 => k.blast = BLAST_CHARM,
                    1 => k.boots = true,
                    _ => {
                        k.spare = true;
                        k.stock = STOCK_CHARM;
                    }
                }
                picked.push(*id);
            }
        }

        for f in self.flames.iter_mut() {
            f.left -= 1;
        }
        self.flames.retain(|f| f.left > 0);
        for s in self.scorch.iter_mut() {
            s.left -= 1;
        }
        self.scorch.retain(|s| s.left > 0);
        (sooted_ids, out.1, out.2, picked)
    }

    pub fn valid(&self) -> bool {
        !self.knights.is_empty()
            && self.knights.len() <= MAX_PLAYERS as usize
            && self.knights.iter().all(|k| {
                (1..=MAX_PLAYERS).contains(&k.id)
                    && k.x < u16::from(W) * 256
                    && k.y < u16::from(H) * 256
                    && k.stock <= STOCK_CHARM
                    && k.blast <= BLAST_CHARM
                    && k.blast >= BLAST
                    && k.soot <= 30
            })
            && self.kegs.len() <= 4 * MAX_PLAYERS as usize
            && self
                .kegs
                .iter()
                .all(|k| k.col < W && k.row < H && k.left > 0 && k.left <= FUSE && (1..=MAX_PLAYERS).contains(&k.owner))
            && self.flames.len() <= 256
            && self.flames.iter().all(|f| f.col < W && f.row < H && f.left <= FIRE)
            && self.scorch.len() <= 128
            && self.scorch.iter().all(|s| s.col < W && s.row < H && s.left <= SCORCH)
            && self.charms.len() <= BALES.len()
            && self.charms.iter().all(|c| c.col < W && c.row < H && c.kind < 3)
            && self.bales.len() <= BALES.len()
    }

    /// The charms left to find, for the board's live cue.
    pub fn charms_left(&self) -> u8 {
        u8::try_from(
            self.drops()
                .iter()
                .filter(|&&t| self.bales.iter().any(|&b| b == t))
                .count(),
        )
        .unwrap_or(0)
    }
}

#[allow(dead_code)]
fn dr_only(dc: i16, dr: i16) -> (i16, i16) {
    (dc, dr)
}

impl Run {
    /// One tick of the shared courtyard, driving the starter's round.
    /// Returns false when that seat's round is over.
    pub(super) fn tick_kegs(
        &mut self,
        round: &mut Round,
        inputs: &BTreeMap<u32, Input>,
        eligible: &[(u32, f32, f32)],
        tapped: &[u32],
    ) -> bool {
        let Some(p) = round.powder.as_mut() else {
            return false;
        };
        // The shared yard stands while any knight of the heat is still in
        // the room; the round's voice is the first knight still present.
        let Some(owner) = p
            .knights
            .iter()
            .filter(|k| eligible.iter().any(|&(id, _, _)| id == k.id))
            .find(|k| k.alive)
            .or_else(|| {
                p.knights
                    .iter()
                    .find(|k| eligible.iter().any(|&(id, _, _)| id == k.id))
            })
            .map(|k| k.id)
        else {
            self.found = Some((self.tick, 0, "The sappers left. Kegs parked.".into()));
            return false;
        };
        let (sooted, burned, boom, picked) = p.step(inputs, tapped);
        // Each knight is drawn where they stand in the yard; a sooted knight
        // goes back to the bench by the start mark to watch.
        for (i, k) in p.knights.iter().enumerate() {
            if let Some(hero) = self.players.get_mut(&k.id) {
                (hero.x, hero.y) = if k.alive {
                    (
                        (f32::from(X0) + f32::from(k.x) / 256.0) * TILE_UNITS,
                        (f32::from(Y0) + f32::from(k.y) / 256.0 + 0.3) * TILE_UNITS,
                    )
                } else {
                    (2.5 * TILE_UNITS, (7.0 + 1.1 * i as f32) * TILE_UNITS)
                };
            }
        }
        // The yard's sounds are the delve's own: a keg's burst is a bomb's,
        // a bale's whoosh a vent's fire, a friend sooted a blast's hit.
        if boom {
            self.sounds.push("bomb");
        }
        if burned {
            self.sounds.push("vent_fire");
        }
        for &id in &picked {
            let _ = id;
            self.sounds.push("spoil_pickup");
        }
        // Each seat's progress is the bales burned while they stand.
        round.progress = p.burned.get(&owner).copied().unwrap_or(0).min(Game::Powder.need());
        if !sooted.is_empty() {
            self.sounds.push("hit_blast");
            let who = if sooted.contains(&owner) {
                format!("Knight {}", owner)
            } else {
                format!("a friend ({})", sooted.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","))
            };
            self.found = Some((self.tick, 0, format!("Sooted! {who} out of this heat.")));
        }
        // Last knight standing: everyone else in the heat is done and this
        // seat is still going. The win is the round's.
        if let Some(heat) = self.pastimes.heat.as_ref() {
            if heat.game == Game::Powder
                && heat.seats.len() > 1
                && heat.done.len() + 1 == heat.seats.len()
            {
                round.progress = round.progress.max(round.game.need());
            }
        }
        let stock_left = p
            .knights
            .iter()
            .find(|k| k.id == owner)
            .map(|k| k.stock)
            .unwrap_or(0);
        let kegs_left = p.kegs.len();
        if stock_left == 0
            && kegs_left == 0
            && p.bales.len() > BALES.len() - usize::from(Game::Powder.need())
            && p.alive().len() <= 1
        {
            self.found = Some((self.tick, owner, "Out of kegs!".into()));
            return false;
        }
        // This seat's knight sooted out of the shared yard: their round is
        // done (a friend's round carries the yard on).
        p.alive().contains(&owner)
    }
}
