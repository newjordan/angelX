//! The realm's makings, met in the world (`rite`; the operator, 2026-11-07:
//! "everytime you play you add to the world"). What a knight brought to life
//! in the Creator is kept in `Home::rites`; this is where the world takes it
//! in, all of it a pure reading of the record plus a few small marks:
//!
//! - **Folk and heroes** stand or wander at their post, the Sozzled Griffin's
//!   taproom or the Delve's courtyard, where they are is a function of the
//!   record and the tick ([`placed`]); E near one makes it say its line
//!   ([`Run::talk_to_made`]), shown over it for a few seconds.
//! - **A minion** turns up as the party's ally on the first room of a delve
//!   floor, when the ally's place is free, the floor's turn picking which.
//! - **A monster or a boss** stands among the foes of a floor's fighting rooms,
//!   one boss and two monsters a floor at most, health bounded by its
//!   recipe's, its maker's name on the health bar (`Enemy::made`).
//!
//! Each kind shows its newest few (`rite::MOST_*`); the rest roll off the
//! display, never the record.

use super::creation::{Calling, Creation, Habit, Post, Role};
use super::hireling::{AllyKind, Hireling};
use super::rite::{self, Kind, Rite};
use super::*;

/// Who made a made thing in the world, and which it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    /// Its place in the realm's record (`Home::rites`).
    pub rite: u32,
    pub by: String,
    pub name: String,
}

/// What the world has noted of the makings: the floor they were last met on
/// and how many made foes it has, and who last heard a folk's line.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// The floor last met (its depth and raid), and the foes met on it.
    #[serde(default)]
    pub floor: u64,
    #[serde(default)]
    pub foes: u8,
    #[serde(default)]
    pub boss: bool,
    /// Knight -> the tick they last heard a made thing speak, and its place in the record.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub said: BTreeMap<u32, (u64, u32)>,
}

impl State {
    pub fn quiet(&self) -> bool {
        *self == Self::default()
    }

    /// Whether the record's `rite` is speaking now, and the tick it began.
    pub fn speaking(&self, rite: usize, tick: u64) -> Option<u64> {
        self.said
            .values()
            .filter(|(at, n)| *n as usize == rite && tick.saturating_sub(*at) < SAID_TICKS)
            .map(|(at, _)| *at)
            .max()
    }
}

/// How long a line shows over the one who said it, and how near a knight
/// must be (tiles) and how often E may speak to the same one.
pub const SAID_TICKS: u64 = 4 * HZ as u64;
pub const REACH: f32 = 2.6;
const AGAIN: u64 = 3 * HZ as u64 / 2;

/// A made thing of the world's people, standing in a room.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    /// Its place in the record.
    pub rite: usize,
    pub kind: Kind,
    /// Where it is, in tiles of the room, and which way it faces (-1 or 1).
    pub at: (f32, f32),
    pub facing: f32,
    pub walking: bool,
}

/// Where the makings stand: in the Griffin's taproom, tiles.
const GRIFFIN_SPOTS: [(f32, f32); MOST_SPOTS] = [(5.0, 11.6), (8.5, 12.6), (19.0, 11.4), (22.5, 12.0)];
/// In the courtyard's lawn, tiles.
const YARD_SPOTS: [(f32, f32); MOST_SPOTS] = [(5.5, 10.8), (8.0, 9.4), (10.0, 11.8), (3.5, 8.8)];
const MOST_SPOTS: usize = rite::MOST_FOLK;
/// How far a wanderer paces about its spot, tiles, and how fast.
const PACE: f32 = 1.6;

fn room_post(kind: RoomKind) -> Option<Post> {
    match kind {
        RoomKind::Alehouse => Some(Post::Griffin),
        RoomKind::Gate => Some(Post::Courtyard),
        _ => None,
    }
}

/// The people the record puts in this room at `tick`: its folk at their post
/// (the newest few of each place), and in the courtyard its heroes walking.
pub fn placed(run: &Run, kind: RoomKind, tick: u64) -> Vec<Placed> {
    let Some(post) = room_post(kind) else {
        return Vec::new();
    };
    let spots = if post == Post::Griffin { &GRIFFIN_SPOTS } else { &YARD_SPOTS };
    let mut at_post: Vec<(usize, &Rite)> = run
        .home
        .rites
        .newest(Kind::Folk, 64)
        .into_iter()
        .filter(|(_, r)| r.creature().is_some_and(|c| c.calling.post == post))
        .collect();
    at_post.drain(..at_post.len().saturating_sub(rite::MOST_FOLK));
    let mut out = Vec::new();
    for (k, (n, r)) in at_post.iter().enumerate() {
        let calling = r.creature().map(|c| c.calling.clone()).unwrap_or_default();
        out.push(stand(spots[k % spots.len()], *n, Kind::Folk, &calling, tick, k));
    }
    if post == Post::Courtyard {
        for (k, (n, _)) in run.home.rites.newest(Kind::Hero, rite::MOST_HEROES).into_iter().enumerate() {
            let calling = Calling { habit: Habit::Wanders, ..Calling::default() };
            out.push(stand(YARD_SPOTS[(k + 1) % YARD_SPOTS.len()], n, Kind::Hero, &calling, tick, k + 7));
        }
    }
    out
}

fn stand(spot: (f32, f32), rite: usize, kind: Kind, calling: &Calling, tick: u64, k: usize) -> Placed {
    if calling.habit == Habit::Stands {
        let facing = if k % 2 == 0 { 1.0 } else { -1.0 };
        return Placed { rite, kind, at: spot, facing, walking: false };
    }
    // Paces to and fro along its spot, each on its own beat.
    let phase = (tick as f32 / (HZ as f32 * 2.2)) + k as f32 * 0.37;
    let swing = (phase * std::f32::consts::TAU).sin();
    let slope = (phase * std::f32::consts::TAU).cos();
    Placed { rite, kind, at: (spot.0 + swing * PACE, spot.1), facing: if slope >= 0.0 { 1.0 } else { -1.0 }, walking: true }
}

/// What a made foe fights as: what its recipe is good at.
fn foe_kind(c: &Creation) -> EnemyKind {
    let s = c.stats();
    let caster = s.magic > s.melee || !c.spells.is_empty();
    match (c.role, caster) {
        (Role::Boss, true) => EnemyKind::Demon,
        (Role::Boss, false) => EnemyKind::Warboar,
        (_, true) => EnemyKind::Shaman,
        _ => EnemyKind::Hob,
    }
}

impl Run {
    /// E near one of the world's made people makes it speak.
    pub(super) fn talk_to_made(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.home.rites.is_empty() {
            return;
        }
        let kind = self.room().kind;
        if room_post(kind).is_none() {
            return;
        }
        let tick = self.tick;
        let here = placed(self, kind, tick);
        if here.is_empty() {
            return;
        }
        let mut said = Vec::new();
        for (&id, input) in inputs {
            if !input.valid() || !input.interact {
                continue;
            }
            let Some(hero) = self.players.get(&id).filter(|h| h.hp > 0 && !h.absent) else {
                continue;
            };
            if self.made.said.get(&id).is_some_and(|(at, _)| tick.saturating_sub(*at) < AGAIN) {
                continue;
            }
            let (hx, hy) = (hero.x / TILE_UNITS, hero.y / TILE_UNITS);
            let near = here
                .iter()
                .map(|p| (p, (p.at.0 - hx).hypot(p.at.1 - hy)))
                .filter(|(_, d)| *d <= REACH)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((p, _)) = near {
                said.push((id, p.rite as u32));
            }
        }
        for (id, rite) in said {
            self.made.said.insert(id, (tick, rite));
            self.cues.push("made_speaks".into());
        }
        // The notes of those who have gone quiet are dropped.
        self.made.said.retain(|_, (at, _)| tick.saturating_sub(*at) < 8 * SAID_TICKS);
    }

    /// What the made thing in the record at `rite` says: a folk's line, a
    /// hero's greeting.
    pub fn made_line(&self, rite: usize) -> Option<String> {
        let r = self.home.rites.kept.get(rite)?;
        Some(match r.kind {
            Kind::Folk => r.creature()?.calling.line.clone(),
            _ => format!("{} of the realm. Made by {}.", r.name, r.knight),
        })
    }

    /// A delve floor's first fighting room (or its first room): the realm's
    /// makings turn up. A minion joins the party as its ally when that place
    /// is free; a monster or boss is among the room's foes.
    pub(super) fn meet_made(&mut self, index: usize, first_visit: bool) {
        if self.home.rites.is_empty() || self.dungeon.depth == 0 || !first_visit {
            return;
        }
        let key = (u64::from(self.dungeon.depth) << 40) ^ self.raid_id.wrapping_add(1);
        if self.made.floor != key {
            self.made.floor = key;
            self.made.foes = 0;
            self.made.boss = false;
            self.meet_minion(index);
        }
        self.meet_foe(index);
    }

    fn meet_minion(&mut self, index: usize) {
        let minions = self.home.rites.newest(Kind::Minion, rite::MOST_MINIONS);
        if minions.is_empty() || self.hireling.is_some() {
            return;
        }
        let pick = (self.dungeon.depth as usize + self.raid_id as usize) % minions.len();
        let (n, r) = minions[pick];
        let (Some(c), Some(lead)) = (
            r.creature(),
            self.players.iter().find(|(_, h)| h.hp > 0 && !h.stone).map(|(&id, h)| (id, (h.x, h.y))),
        ) else {
            return;
        };
        let Some(grid) = self.dungeon.rooms.get(index).map(|room| Grid { room, barred: false }) else {
            return;
        };
        let at = super::hireling::beside(&grid, lead.1);
        let mut ally = Hireling::minion(AllyKind::Hound, 0, self.dungeon.depth, at, lead.0);
        // A little friend: its recipe's health, nothing more.
        let hp = u32::from(c.stats().hp).clamp(20, 80);
        (ally.hp, ally.max_hp) = (hp, hp);
        ally.made = Some(Tag { rite: n as u32, by: r.knight.clone(), name: r.name.clone() });
        let line = format!("{}, {}'s minion, joins the party.", r.name, r.knight);
        self.hireling = Some(ally);
        self.cues.push("summoned:made".into());
        if self.notices.len() < 8 {
            self.notices.push(line);
        }
    }

    fn meet_foe(&mut self, index: usize) {
        let fighting = !self.dungeon.rooms[index].cleared
            && !self.enemies.is_empty()
            && !self.enemies.iter().any(|e| e.boss.is_some() || matches!(e.kind, EnemyKind::Dragon | EnemyKind::PitTyrant | EnemyKind::Boss));
        if !fighting || usize::from(self.made.foes) >= rite::MOST_FOES {
            return;
        }
        let mut pool: Vec<(usize, &Rite)> = self.home.rites.newest(Kind::Monster, 6);
        if self.dungeon.depth >= 2 && !self.made.boss {
            pool.extend(self.home.rites.newest(Kind::Boss, 3));
        }
        if pool.is_empty() {
            return;
        }
        let pick = (self.dungeon.depth as usize * 7 + index + self.raid_id as usize) % pool.len();
        let (n, r) = pool[pick];
        let Some(c) = r.creature().cloned() else {
            return;
        };
        let (by, name, boss) = (r.knight.clone(), r.name.clone(), r.kind == Kind::Boss);
        let before = self.enemies.len();
        self.spawn(foe_kind(&c));
        if self.enemies.len() == before {
            return;
        }
        let party = self.party_len().max(1) as u32;
        let hp = rite::made_hp(&c) * party;
        if let Some(e) = self.enemies.last_mut() {
            (e.hp, e.max_hp, e.seen_hp, e.base_hp) = (hp, hp, hp, Some(hp / party));
            if boss {
                e.size = Some(e.kind.radius() * 1.5);
            }
            e.made = Some(Tag { rite: n as u32, by, name });
        }
        self.made.foes += 1;
        self.made.boss |= boss;
        self.cues.push(format!("made_foe:{}", if boss { "boss" } else { "monster" }));
    }
}
