//! Starfall (the operator, 2026-10-28: "people LOVE the wish system ...
//! lets add new ways to interact with the mechanic through world
//! exploration ... we bring hope ... like comets or shooting stars").
//!
//! Once or twice a floor, in a quiet moment, a shooting star crosses the
//! dark over the room the party stands in. Which quiet moments is the
//! floor's own (`Run::star_plan`): never the first room the party clears,
//! never in the middle of a fight, never in a guardian's room or while a
//! boss is awake, never at home, in a safe room or in a village of the
//! Deep. A moment is a room (or an open floor's chamber) the party has
//! cleared, by fighting it or by walking into one with nothing to fight;
//! the star comes once the room has been still a little while after it
//! (`QUIET`).
//!
//! - **Wish upon a star.** Every knight standing under it as it appears
//!   catches its wish (`Hero::star_wish`): one wish, free, spent before any
//!   of their own, at one step of a ladder (a new wish at its first tier).
//!   It goes on the ordinary scroll: T opens the wish line, a known wish is
//!   granted at once and new words go to the host's learner as any wish's
//!   do. A star's wish is said under its own sky: it fades when the party
//!   leaves the room it fell in (a scroll already open with it is kept),
//!   and with the floor.
//! - **The fallen star.** It comes down somewhere the party can walk to
//!   and lies there as a shard of starlight for half a minute
//!   (`SHARD_LIFE`). The first knight to touch it takes it: a wish of their
//!   own, one more to hold. Nobody's, it fades.
//!
//! Nothing is rolled: where the star crosses and lands is hashed from the
//! run's seed, the floor and the tick it came, so the host and every
//! friend's mirror draw the same star.

use super::*;

/// The first quiet moment a star may come at: the second room cleared on
/// a floor. The first one never (a floor's first fight is its own).
pub const FIRST_MOMENT: usize = 2;
/// Ticks the room must stay still before the star comes.
pub const QUIET: u64 = 5 * HZ as u64 / 2;
/// The star's beats, in ticks from its first glint: it gathers in the
/// dark, crosses the sky, dives, and comes down (`LANDS`, a little over
/// four seconds).
pub const GLINT: u64 = 14;
pub const DIVE: u64 = 98;
pub const LANDS: u64 = 124;
/// How long the banner calls it, from the glint.
pub const BANNER: u64 = 5 * HZ as u64;
/// How long the shard lies before it fades, and the fading itself.
pub const SHARD_LIFE: u64 = 30 * HZ as u64;
pub const SHARD_FADE: u64 = 3 * HZ as u64;
/// How near a knight comes to take the shard, in arena units.
pub const REACH: f32 = 1.9;
/// How far from every knight a star comes down, in arena units.
pub const CLEAR_OF_KNIGHTS: f32 = 4.5;
/// How long a catch, and a star wish's grant, are shown.
pub const CAUGHT_TICKS: u64 = 2 * HZ as u64;
pub const GRANT_TICKS: u64 = 5 * HZ as u64 / 2;
/// The most star wishes' grants remembered at once.
const MOST_GRANTED: usize = 8;

/// Where the star is in its fall.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fall {
    /// In the sky, falling.
    Sky,
    /// Down, a shard lying where it landed.
    Lying,
    /// Taken by knight `knight` at `tick`.
    Taken { knight: u32, tick: u64 },
    /// Faded, nobody's.
    Gone,
}

/// A shooting star: when it first glinted, the room it crosses, where it
/// comes down (arena units), the seed its path is drawn from, and where it
/// is in its fall.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Star {
    pub tick: u64,
    pub room: usize,
    pub land: (f32, f32),
    pub seed: u32,
    pub fall: Fall,
}

impl Star {
    /// Ticks since it glinted.
    pub fn age(&self, tick: u64) -> u64 {
        tick.saturating_sub(self.tick)
    }

    /// Still in the sky at `tick`.
    pub fn in_sky(&self, tick: u64) -> bool {
        self.fall == Fall::Sky && self.age(tick) < LANDS
    }

    /// It crosses from the left of the view (else the right).
    pub fn from_left(&self) -> bool {
        self.seed & 1 == 0
    }
}

/// A star wish granted: to whom, when.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Granted {
    pub knight: u32,
    pub tick: u64,
}

/// The floor's stars: how many have fallen, the tick the next one is due
/// once the room is still, the star now (or last), and the star wishes
/// lately granted.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Starfall {
    /// The floor (depth) these stars fell on.
    #[serde(default)]
    pub floor: u32,
    #[serde(default)]
    pub fell: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub star: Option<Star>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub granted: Vec<Granted>,
}

impl Starfall {
    /// No star has come near: saves and the wire leave it out.
    pub fn omit(v: &Starfall) -> bool {
        v.fell == 0 && v.due.is_none() && v.star.is_none() && v.granted.is_empty()
    }

    pub fn valid(&self, rooms: usize) -> bool {
        let seat = |id: u32| (1..=MAX_PLAYERS).contains(&id);
        self.granted.len() <= MOST_GRANTED
            && self.granted.iter().all(|g| seat(g.knight))
            && self.star.as_ref().is_none_or(|s| {
                s.room < rooms
                    && s.land.0.is_finite()
                    && s.land.1.is_finite()
                    && match s.fall {
                        Fall::Taken { knight, .. } => seat(knight),
                        _ => true,
                    }
            })
    }

    /// The star whose shard lies at `tick`, and how far through its life.
    pub fn shard(&self, tick: u64) -> Option<(&Star, f32)> {
        let s = self.star.as_ref().filter(|s| s.fall == Fall::Lying)?;
        let since = s.age(tick).saturating_sub(LANDS);
        Some((s, since as f32 / SHARD_LIFE as f32))
    }
}

/// The kinds of room a quiet moment is counted in, and a star may cross:
/// the halls a party fights and walks through, and an open floor's map.
fn counted(kind: RoomKind) -> bool {
    matches!(kind, RoomKind::Fight | RoomKind::Treasure | RoomKind::Hall)
}

fn open_sky(kind: RoomKind) -> bool {
    counted(kind) || kind == RoomKind::Depths
}

impl Run {
    /// This floor's stars: how many (one, or two on a third of floors),
    /// and the quiet moment each comes at — the second to fourth room
    /// cleared for the first, two to four rooms on for the second.
    pub fn star_plan(&self) -> (u8, [usize; 2]) {
        let h = mix(self.seed ^ u64::from(self.dungeon.depth).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x5741_2fa1);
        let count = if h % 3 == 0 { 2 } else { 1 };
        let first = FIRST_MOMENT + (h >> 8) as usize % 3;
        let second = first + 2 + (h >> 16) as usize % 3;
        (count, [first, second])
    }

    /// The quiet moments the party has had on this floor: rooms (and an
    /// open floor's chambers) cleared, fought or walked into empty.
    pub fn quiet_moments(&self) -> usize {
        let rooms = self.dungeon.rooms.iter().filter(|r| r.visited && r.cleared && counted(r.kind)).count();
        let chambers = self.dungeon.open.as_ref().map_or(0, |o| o.chambers.iter().filter(|c| c.cleared && counted(c.kind)).count());
        rooms + chambers
    }

    /// Whether the sky over the party is still enough for a star: a hall of
    /// a delve, nothing fighting, no boss awake, no game under way in it.
    fn sky_quiet(&self) -> bool {
        self.dungeon.depth >= 1
            && open_sky(self.room().kind)
            && !(self.identity && self.dungeon.depth == 1)
            && self.phase == Phase::Exploring
            && !self.fight_on()
            && !self.floor_boss_here()
            && !self.boss_near()
            && !self.folly_running()
            && self.nook.as_ref().is_none_or(|n| n.room != self.at)
            && self.floor_plates.is_empty()
            && self.players.values().any(|h| h.hp > 0 && !h.absent)
    }

    /// A boss awake, or one asleep within a stride of waking near the
    /// party (an open floor's guardian in its chamber): a fight is coming.
    fn boss_near(&self) -> bool {
        self.enemies.iter().any(|e| {
            e.hp > 0
                && e.is_boss()
                && (!e.dormant
                    || self.players.values().any(|h| h.hp > 0 && (h.x - e.x).hypot(h.y - e.y) < 2.5 * depths::WAKE_REACH))
        })
    }

    /// Knight `id` holds a star's wish.
    pub fn star_wish_held(&self, id: u32) -> bool {
        self.players.get(&id).is_some_and(|h| h.star_wish)
    }

    /// Knight `id`'s star wish is spent.
    pub(super) fn spend_star_wish(&mut self, id: u32) -> bool {
        let Some(hero) = self.players.get_mut(&id).filter(|h| h.star_wish) else {
            return false;
        };
        hero.star_wish = false;
        if let Some(window) = self.window.as_mut() {
            window.wished.push(id);
        }
        true
    }

    /// Knight `id`'s star wish landed: its moment is drawn over them, and
    /// the fairies' chimes ring for it.
    pub(super) fn star_wish_granted(&mut self, id: u32) {
        let s = &mut self.starfall;
        if s.granted.len() >= MOST_GRANTED {
            s.granted.remove(0);
        }
        s.granted.push(Granted { knight: id, tick: self.tick });
        self.sounds.push("fairy_wish");
        self.cues.push("star_wish".into());
    }

    /// A new floor: its stars are its own, and a star's wish not said is
    /// gone with the last one.
    pub(super) fn starfall_new_floor(&mut self) {
        self.starfall = Starfall::default();
        for hero in self.players.values_mut() {
            hero.star_wish = false;
        }
    }

    /// One tick of the floor's stars: the one falling comes down, its shard
    /// is taken or fades, its wishes fade once the party has left its sky,
    /// and in a quiet moment the next one comes.
    pub(super) fn tick_starfall(&mut self, _inputs: &BTreeMap<u32, Input>) {
        if self.dungeon.depth == 0 || (!Starfall::omit(&self.starfall) && self.starfall.floor != self.dungeon.depth) {
            if !Starfall::omit(&self.starfall) || self.players.values().any(|h| h.star_wish) {
                self.starfall_new_floor();
            }
            return;
        }
        let tick = self.tick;
        self.starfall.granted.retain(|g| tick.saturating_sub(g.tick) < GRANT_TICKS);
        self.tick_star();
        self.watch_the_sky();
    }

    /// The star now: down at `LANDS`, its shard taken by the first knight
    /// to touch it or faded at the end of its life, and its wishes gone
    /// with the party out of its room.
    fn tick_star(&mut self) {
        let Some(star) = self.starfall.star.clone() else {
            return;
        };
        let age = star.age(self.tick);
        let here = star.room == self.at;
        if star.fall == Fall::Sky && age >= LANDS {
            self.set_fall(Fall::Lying);
            if here {
                self.sounds.push("impact_holy");
                self.shake = self.shake.max(3);
            }
        }
        if self.starfall.star.as_ref().is_some_and(|s| s.fall == Fall::Lying) {
            if age >= LANDS + SHARD_LIFE {
                self.set_fall(Fall::Gone);
            } else if here && let Some(id) = self.shard_toucher(star.land) {
                self.catch_star(id);
            }
        }
        // A star's wish is said under its own sky.
        if !here {
            let fading: Vec<u32> = self
                .players
                .iter()
                .filter(|(_, h)| h.star_wish && !h.offer.as_ref().is_some_and(|o| o.star))
                .map(|(&id, _)| id)
                .collect();
            for id in fading {
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.star_wish = false;
                }
                self.tell(id, "The star's sky is behind you: its wish fades.");
            }
        }
    }

    fn set_fall(&mut self, fall: Fall) {
        if let Some(s) = self.starfall.star.as_mut() {
            s.fall = fall;
        }
    }

    /// The knight touching the shard at `at`, nearest first: standing, and
    /// with room for one more wish.
    fn shard_toucher(&self, at: (f32, f32)) -> Option<u32> {
        self.players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.absent && !h.stone && h.wishes < overclass::MOST_WISHES)
            .map(|(&id, h)| (id, (h.x - at.0).hypot(h.y - at.1)))
            .filter(|&(_, d)| d <= REACH)
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
            .map(|(id, _)| id)
    }

    /// Knight `id` takes the fallen star: a wish of their own.
    fn catch_star(&mut self, id: u32) {
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        hero.wishes = hero.wishes.saturating_add(1).min(overclass::MOST_WISHES);
        self.set_fall(Fall::Taken { knight: id, tick: self.tick });
        self.sounds.push("wish_land");
        self.cues.push("star_caught".into());
        self.tell(id, "The star is yours: one more wish to hold.");
    }

    /// The floor's next star, if it has one to come: due once the room has
    /// been still `QUIET` ticks at its quiet moment, put off while anything
    /// stirs.
    fn watch_the_sky(&mut self) {
        let (count, at) = self.star_plan();
        let fell = usize::from(self.starfall.fell);
        if fell >= usize::from(count) || self.quiet_moments() < at[fell.min(1)] {
            return;
        }
        // One star at a time: the last one's shard and its catch first.
        if self.starfall.star.as_ref().is_some_and(|s| match s.fall {
            Fall::Sky | Fall::Lying => true,
            Fall::Taken { tick, .. } => self.tick.saturating_sub(tick) < CAUGHT_TICKS,
            Fall::Gone => false,
        }) {
            return;
        }
        if !self.sky_quiet() {
            self.starfall.due = None;
            return;
        }
        match self.starfall.due {
            None => {
                self.starfall.floor = self.dungeon.depth;
                self.starfall.due = Some(self.tick + QUIET);
            }
            Some(due) if self.tick >= due => self.star_falls(),
            Some(_) => {}
        }
    }

    /// A shooting star crosses the room: every knight under it catches its
    /// wish, and it comes down somewhere they can walk to.
    pub(super) fn star_falls(&mut self) {
        let depth = self.dungeon.depth;
        let seed = (mix(self.seed ^ self.tick.rotate_left(21) ^ u64::from(depth).wrapping_mul(0x51a7)) >> 16) as u32;
        let land = self.star_landing(seed);
        let s = &mut self.starfall;
        s.floor = depth;
        s.fell = s.fell.saturating_add(1);
        s.due = None;
        s.star = Some(Star { tick: self.tick, room: self.at, land, seed, fall: Fall::Sky });
        let under: Vec<u32> = self.players.iter().filter(|(_, h)| h.hp > 0 && !h.absent).map(|(&id, _)| id).collect();
        for &id in &under {
            if let Some(hero) = self.players.get_mut(&id) {
                hero.star_wish = true;
            }
            self.tell(id, "A shooting star! Make a wish: T, and this one is free.");
        }
        self.sounds.push("fairy_wish");
        self.cues.push("starfall".into());
    }

    /// Where a star with `seed` comes down: open floor a few strides from
    /// the knight nearest the party's middle, on a straight walk from them,
    /// off the walls and clear of every knight; that knight's own feet
    /// when the room has no such place.
    fn star_landing(&self, seed: u32) -> (f32, f32) {
        let standing: Vec<(f32, f32)> = self.players.values().filter(|h| h.hp > 0 && !h.absent).map(|h| (h.x, h.y)).collect();
        let n = standing.len().max(1) as f32;
        let mid = standing.iter().fold((0.0, 0.0), |(x, y), p| (x + p.0 / n, y + p.1 / n));
        let from = standing
            .iter()
            .copied()
            .min_by(|a, b| (a.0 - mid.0).hypot(a.1 - mid.1).total_cmp(&(b.0 - mid.0).hypot(b.1 - mid.1)))
            .unwrap_or(mid);
        let room = self.room();
        let grid = Grid { room, barred: false };
        let (w, h) = (room.width(), room.height());
        let a0 = (seed % 3600) as f32 / 3600.0 * std::f32::consts::TAU;
        for r in [8.0f32, 6.5, 10.0, 5.0, 12.0, 3.5] {
            for k in 0..12 {
                let a = a0 + k as f32 * std::f32::consts::TAU / 12.0;
                let p = (from.0 + a.cos() * r, from.1 + a.sin() * r * 0.75);
                // Off the walls, and low enough in the room to be seen
                // under the HUD and the room's top wall.
                if p.0 < 3.0 * TILE_UNITS || p.0 > w - 3.0 * TILE_UNITS || p.1 < 3.5 * TILE_UNITS || p.1 > h - 4.0 * TILE_UNITS {
                    continue;
                }
                if grid.tile_at(p.0, p.1) != Tile::Floor || !grid.clear(p.0, p.1, 1.0, Mover::Hero) {
                    continue;
                }
                // Never at a knight's feet: someone walks to it.
                if standing.iter().any(|k| (k.0 - p.0).hypot(k.1 - p.1) < CLEAR_OF_KNIGHTS) {
                    continue;
                }
                let reached = grid.halted(from, (p.0 - from.0, p.1 - from.1));
                if (reached.0 - p.0).hypot(reached.1 - p.1) < 0.2 {
                    return p;
                }
            }
        }
        from
    }
}

#[cfg(any(test, feature = "testing"))]
impl Run {
    /// A star falls now, wherever the party stands (the shots' and the
    /// rules' fixtures).
    pub fn star_falls_for_test(&mut self) {
        self.star_falls();
    }
}

#[cfg(test)]
#[path = "../../tests/rules/starfall__tests.rs"]
mod tests;
