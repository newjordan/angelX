//! Shoulder rides: knights climb on each other's shoulders and fight as a
//! tower, and a carrier can throw a friend off the top down the hall.
//!
//! - **Climbing on.** The ride key (C) next to another knight climbs onto
//!   their shoulders. Whoever is already up there, a new rider climbs to the
//!   top: one carrier holds up to five (`MAX_RIDERS`), a party of six in one
//!   tower. A rider does not walk, roll or meet the world: they go where the
//!   carrier goes and only aim, shoot, swing, cast and play cards.
//! - **Hopping off.** A rider's ride key hops down to clear ground beside
//!   the tower. Riders are thrown off when the carrier is downed, turns to
//!   stone, is tossed by a Ravage, goes into the privy, is hexed into a
//!   frog, goes side-on (a ledge hall's pits) or onto a ride, or leaves the
//!   room for one the riders do not go to. Through an ordinary doorway,
//!   down the stairs and across an open floor the tower stays whole: the
//!   riders are carried.
//! - **Throwing.** The carrier's ride key throws the top rider along the
//!   carrier's aim: up in a high arc, over monsters, props, pits and shots,
//!   untouchable all the way, landing one hallway on (`THROW_REACH`, the
//!   open floors' passage measured) or short of the first wall or doorway.
//!   The landing is a slam (`SLAM_REACH`): it strikes the monsters round it
//!   as one stroke (`aoe`), throws them back and stuns them a moment, and
//!   pops the monsters' shots there. A carrier throws once every
//!   `THROW_REARM`; the thrown knight lands with `LAND_WARD` of grace.
//!
//! **Who takes the hits.** A tower is one body to the monsters: only the
//! carrier can be struck (riders are not `Hero::targetable`, like a knight
//! mid-throw). Each rider takes `RIDER_SHARE` percent of every hit the
//! carrier takes, and their own grace after it (`Hero::hit_ward`), so a
//! rider is hurt at most once a ward. The carrier slows by `PACE_PER_RIDER`
//! for each rider (a five-high tower walks at 70%): a tower is a gun
//! platform, not a free win.
//!
//! The state is the run's (`Run::shoulders`) and goes over the wire with
//! the rest: a client draws its own carrier ahead with the riders sat on it
//! (`Run::drawn_with`), and a rider or a thrown knight is drawn from the
//! server's ticks like a knight on a ride, the tower always in one piece.

use super::*;
use std::collections::BTreeSet;

pub const MAX_RIDERS: usize = MAX_PLAYERS as usize - 1;
pub const RIDE_REACH: f32 = 2.8;
pub const PACE_PER_RIDER: f32 = 0.06;
pub const RIDER_SHARE: u32 = 33;
pub const THROW_REACH: f32 = 12.0 * TILE_UNITS;
pub const THROW_TICKS: u32 = HZ * 3 / 4;
const THROW_LEAST: u32 = HZ / 4;
pub const THROW_PEAK: f32 = 6.0;
pub const THROW_REARM: u32 = 3 * HZ;
pub const LAND_WARD: u32 = HZ / 2;
pub const SLAM_REACH: f32 = 4.5;
pub const SLAM_DAMAGE: u32 = 45;
pub const SLAM_KNOCK: f32 = 4.0;
pub const SLAM_STUN: u32 = HZ * 2 / 5;
pub const SLAM_SHOWN: u32 = 14;
pub const SOUND_CLIMB: &str = "shoulder_climb";
pub const SOUND_HOP: &str = "shoulder_hop";
pub const SOUND_THROW: &str = "shoulder_throw";
pub const SOUND_SLAM: &str = "shoulder_slam";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shoulders {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub stacks: BTreeMap<u32, Vec<u32>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flights: Vec<Flight>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub rearm: BTreeMap<u32, u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub slams: Vec<Slam>,
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    keys: BTreeSet<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Flight {
    pub knight: u32,
    pub by: u32,
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub age: u32,
    pub ticks: u32,
    pub at: usize,
    pub depth: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slam {
    pub x: f32,
    pub y: f32,
    pub tick: u64,
}

impl Flight {
    pub fn progress(&self) -> f32 {
        (self.age as f32 / self.ticks.max(1) as f32).clamp(0.0, 1.0)
    }

    pub fn height_at(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        let reach = (self.to.0 - self.from.0).hypot(self.to.1 - self.from.1);
        let peak = THROW_PEAK * (0.45 + 0.55 * (reach / THROW_REACH).min(1.0));
        4.0 * peak * t * (1.0 - t)
    }

    pub fn progress_at(&self, (x, y): (f32, f32)) -> f32 {
        let (dx, dy) = (self.to.0 - self.from.0, self.to.1 - self.from.1);
        let len = dx * dx + dy * dy;
        if len < 1e-4 {
            return self.progress();
        }
        (((x - self.from.0) * dx + (y - self.from.1) * dy) / len).clamp(0.0, 1.0)
    }

    fn place(&self) -> (f32, f32) {
        let t = self.progress();
        (self.from.0 + (self.to.0 - self.from.0) * t, self.from.1 + (self.to.1 - self.from.1) * t)
    }
}

impl Shoulders {
    pub fn quiet(&self) -> bool {
        self.stacks.is_empty() && self.flights.is_empty() && self.rearm.is_empty() && self.slams.is_empty()
    }

    pub fn carrier_of(&self, id: u32) -> Option<u32> {
        self.stacks.iter().find(|(_, riders)| riders.contains(&id)).map(|(&c, _)| c)
    }

    pub fn riding(&self, id: u32) -> bool {
        self.carrier_of(id).is_some()
    }

    pub fn level(&self, id: u32) -> usize {
        self.stacks
            .values()
            .find_map(|riders| riders.iter().position(|&r| r == id))
            .map_or(0, |i| i + 1)
    }

    pub fn riders(&self, id: u32) -> &[u32] {
        self.stacks.get(&id).map_or(&[], Vec::as_slice)
    }

    pub fn flight(&self, id: u32) -> Option<&Flight> {
        self.flights.iter().find(|f| f.knight == id)
    }

    pub fn carried(&self, id: u32) -> bool {
        self.riding(id) || self.flight(id).is_some()
    }

    pub fn pace(&self, id: u32) -> f32 {
        1.0 - PACE_PER_RIDER * self.riders(id).len() as f32
    }

    pub(super) fn seat_riders(&self, grid: &Grid, players: &mut BTreeMap<u32, Hero>) {
        for (carrier, riders) in &self.stacks {
            let Some((x, y)) = players.get(carrier).map(|h| (h.x, h.y)) else {
                continue;
            };
            for rider in riders {
                if let Some(hero) = players.get_mut(rider) {
                    let was = (hero.x, hero.y);
                    (hero.x, hero.y) = grid.across_bar(was, (x, y));
                }
            }
        }
    }

    pub fn valid(&self) -> bool {
        let knight = |id: &u32| (1..=MAX_PLAYERS).contains(id);
        let mut seen = BTreeSet::new();
        self.stacks.len() <= MAX_PLAYERS as usize
            && self.stacks.iter().all(|(c, riders)| {
                knight(c) && riders.len() <= MAX_RIDERS && riders.iter().all(knight) && seen.insert(*c)
                    && riders.iter().all(|r| seen.insert(*r))
            })
            && self.flights.len() <= MAX_PLAYERS as usize
            && self.flights.iter().all(|f| {
                knight(&f.knight) && f.ticks > 0 && f.age <= f.ticks
                    && [f.from.0, f.from.1, f.to.0, f.to.1].iter().all(|v| v.is_finite())
            })
            && self.slams.len() <= 2 * MAX_PLAYERS as usize
    }
}

pub(super) struct Ledger {
    carried: Vec<(u32, u32)>,
}

impl Run {
    pub(super) fn shoulders_begin(&mut self, inputs: &BTreeMap<u32, Input>) -> (Option<BTreeMap<u32, Input>>, Ledger) {
        let quiet = self.shoulders.quiet() && !inputs.values().any(|i| i.ride) && self.shoulders.keys.is_empty();
        if quiet {
            for hero in self.players.values_mut() {
                hero.aloft = false;
            }
            return (None, Ledger { carried: Vec::new() });
        }
        self.topple();
        self.fly_on();
        self.ride_keys(inputs);
        let s = &self.shoulders;
        for (&id, hero) in &mut self.players {
            hero.aloft = s.carried(id);
        }
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        self.shoulders.seat_riders(&grid, &mut self.players);
        let mut out = inputs.clone();
        for (&id, input) in &mut out {
            if self.shoulders.carried(id) {
                *input = Input {
                    move_x: 0,
                    move_y: 0,
                    dash: false,
                    interact: false,
                    take: false,
                    vigil: false,
                    ..*input
                };
            }
        }
        let carried = self
            .shoulders
            .stacks
            .keys()
            .filter_map(|&c| self.players.get(&c).map(|h| (c, h.hp)))
            .collect();
        (Some(out), Ledger { carried })
    }

    pub(super) fn shoulders_end(&mut self, ledger: Ledger) {
        for (carrier, was) in ledger.carried {
            let Some(now) = self.players.get(&carrier).map(|h| h.hp) else {
                continue;
            };
            let lost = was.saturating_sub(now);
            if lost == 0 {
                continue;
            }
            let riders = self.shoulders.riders(carrier).to_vec();
            for rider in riders {
                let Some(hero) = self.players.get_mut(&rider) else {
                    continue;
                };
                if hero.hp == 0 || hero.invulnerable > 0 || hero.stone {
                    continue;
                }
                let share = (lost * RIDER_SHARE).div_ceil(100).max(1);
                let before = hero.hp;
                hero.hp = hero.hp.saturating_sub(share);
                hero.invulnerable = hero.hit_ward;
                if self.dungeon.depth > 0 {
                    let line = self.floor_tally.line(rider);
                    line.taken = line.taken.saturating_add(before - hero.hp);
                    line.falls += u32::from(hero.hp == 0);
                }
                if hero.hp == 0 {
                    self.cues.push("knight_down".into());
                }
                self.room_hurt = true;
            }
        }
        self.topple();
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        self.shoulders.seat_riders(&grid, &mut self.players);
        for t in self.shoulders.rearm.values_mut() {
            *t = t.saturating_sub(1);
        }
        self.shoulders.rearm.retain(|_, t| *t > 0);
        let tick = self.tick;
        self.shoulders.slams.retain(|s| tick.saturating_sub(s.tick) < u64::from(SLAM_SHOWN));
    }

    fn steady(&self, id: u32) -> bool {
        !self.exits.outside.contains_key(&id)
            && self.players.get(&id).is_some_and(|h| {
                h.hp > 0
                    && !h.stone
                    && !h.frog()
                    && h.privy == 0
                    && h.pit.stop.is_none()
                    && h.pit.result.is_none()
                    && h.tossed == 0
                    && h.slashes == 0
                    && h.flee_ticks == 0
                    && h.possessed.is_none()
                    && !h.stocked
                    && h.soak == 0
                    && !h.sitting
            })
    }

    fn towers_stand(&self) -> bool {
        !self.side_on() && !activity::RIDES.iter().any(|ride| (ride.on)(self))
    }

    fn topple(&mut self) {
        let stands = self.towers_stand();
        let carriers: Vec<u32> = self.shoulders.stacks.keys().copied().collect();
        for carrier in carriers {
            if !stands || !self.steady(carrier) {
                let riders = self.shoulders.stacks.remove(&carrier).unwrap_or_default();
                self.set_down(carrier, &riders);
                continue;
            }
            let riders = self.shoulders.stacks.get(&carrier).cloned().unwrap_or_default();
            let fallen: Vec<u32> = riders.iter().copied().filter(|&r| !self.steady(r)).collect();
            if !fallen.is_empty() {
                if let Some(stack) = self.shoulders.stacks.get_mut(&carrier) {
                    stack.retain(|r| !fallen.contains(r));
                }
                self.set_down(carrier, &fallen);
            }
        }
        self.shoulders.stacks.retain(|_, riders| !riders.is_empty());
        let (at, depth) = (self.at, self.dungeon.depth);
        let gone: Vec<u32> = self
            .shoulders
            .flights
            .iter()
            .filter(|f| f.at != at || f.depth != depth || !stands || !self.players.contains_key(&f.knight))
            .map(|f| f.knight)
            .collect();
        self.shoulders.flights.retain(|f| !gone.contains(&f.knight));
        for id in gone {
            if let Some(hero) = self.players.get_mut(&id) {
                hero.aloft = false;
            }
        }
    }

    pub(super) fn set_down(&mut self, carrier: u32, riders: &[u32]) {
        let Some(at) = self.players.get(&carrier).map(|h| (h.x, h.y)) else {
            return;
        };
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
        let mut taken: Vec<(f32, f32)> = vec![at];
        for &rider in riders {
            let Some(hero) = self.players.get_mut(&rider) else {
                continue;
            };
            let prefer = if (hero.aim_x, hero.aim_y) == (0.0, 0.0) { (1.0, 0.0) } else { (hero.aim_x, hero.aim_y) };
            let spot = beside(&grid, at, prefer, &taken);
            taken.push(spot);
            let was = (hero.x, hero.y);
            (hero.x, hero.y) = grid.halted(was, (spot.0 - was.0, spot.1 - was.1));
            hero.aloft = false;
        }
    }

    fn fly_on(&mut self) {
        let barred = self.barred();
        let at = self.at;
        let mut landed = Vec::new();
        for flight in &mut self.shoulders.flights {
            flight.age = (flight.age + 1).min(flight.ticks);
            let place = flight.place();
            if let Some(hero) = self.players.get_mut(&flight.knight) {
                let room = &self.dungeon.rooms[at];
                let grid = Grid { room, barred };
                let was = (hero.x, hero.y);
                (hero.x, hero.y) = grid.halted(was, (place.0 - was.0, place.1 - was.1));
            }
            if flight.age >= flight.ticks {
                landed.push(*flight);
            }
        }
        self.shoulders.flights.retain(|f| f.age < f.ticks);
        for flight in landed {
            self.slam(flight);
        }
    }

    fn slam(&mut self, flight: Flight) {
        let (x, y) = flight.to;
        let id = flight.knight;
        let barred = self.barred();
        let at = self.at;
        if let Some(hero) = self.players.get_mut(&id) {
            let room = &self.dungeon.rooms[at];
            let grid = Grid { room, barred };
            let was = (hero.x, hero.y);
            (hero.x, hero.y) = grid.halted(was, (x - was.0, y - was.1));
            hero.aloft = false;
            hero.invulnerable = hero.invulnerable.max(LAND_WARD);
        }
        self.shoulders.slams.push(Slam { x, y, tick: self.tick });
        let over = self.shoulders.slams.len().saturating_sub(2 * MAX_PLAYERS as usize);
        self.shoulders.slams.drain(..over);
        self.sounds.push(SOUND_SLAM);
        self.feel.dust(x, y, (0.0, 0.0), feel::LAND);
        self.fx.push(Fx::Shake(feel::SHAKE_KILL + 2));
        let tick = self.tick;
        let struck = aoe::strike(&mut self.enemies, (x, y), SLAM_DAMAGE, |e| {
            !e.dormant && (e.x - x).hypot(e.y - y) <= SLAM_REACH + e.radius()
        });
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
        let mut heavy = false;
        for (i, dealt) in struck {
            let enemy = &mut self.enemies[i];
            enemy.wits.foe = id;
            if dealt > 0 {
                self.feel.credit(id, enemy.id, dealt, tick);
            }
            heavy |= enemy.is_boss();
            let anchored = enemy.is_boss() || matches!(enemy.kind, EnemyKind::Dummy | EnemyKind::Ward);
            if enemy.hp == 0 || anchored {
                continue;
            }
            enemy.frozen = enemy.frozen.max(SLAM_STUN);
            let (ux, uy) = if (enemy.x - x).hypot(enemy.y - y) < 1e-3 {
                (1.0, 0.0)
            } else {
                unit(enemy.x - x, enemy.y - y)
            };
            let mover = if enemy.kind.flies() { Mover::Flier } else { Mover::Walker };
            (enemy.x, enemy.y) = grid.slide((enemy.x, enemy.y), (ux * SLAM_KNOCK, uy * SLAM_KNOCK), enemy.radius(), mover);
        }
        if heavy {
            self.hitstop.request(&self.tune, hitstop::Impact::Heavy);
        }
        let feel = &mut self.feel;
        self.projectiles.retain(|p| {
            let kept = !p.hostile || (p.x - x).hypot(p.y - y) > SLAM_REACH;
            if !kept {
                feel.impact(p.x, p.y, p.kind, (0.0, 0.0));
            }
            kept
        });
    }

    fn ride_keys(&mut self, inputs: &BTreeMap<u32, Input>) {
        let down: BTreeSet<u32> = inputs
            .iter()
            .filter(|(_, i)| i.valid() && i.ride)
            .map(|(&id, _)| id)
            .collect();
        let pressed: Vec<u32> = down.difference(&self.shoulders.keys).copied().collect();
        self.shoulders.keys = down;
        if pressed.is_empty() || !self.towers_stand() {
            return;
        }
        for id in pressed {
            if !self.steady(id) || self.shoulders.flight(id).is_some() {
                continue;
            }
            if let Some(carrier) = self.shoulders.carrier_of(id) {
                if let Some(stack) = self.shoulders.stacks.get_mut(&carrier) {
                    stack.retain(|&r| r != id);
                }
                self.shoulders.stacks.retain(|_, riders| !riders.is_empty());
                self.set_down(carrier, &[id]);
                self.sounds.push(SOUND_HOP);
            } else if !self.shoulders.riders(id).is_empty() {
                self.throw_top(id);
            } else if let Some(base) = self.climbable(id) {
                let stack = self.shoulders.stacks.entry(base).or_default();
                stack.push(id);
                if let Some(hero) = self.players.get_mut(&id) {
                    (hero.dash_ticks, hero.shielding, hero.walling, hero.spin) = (0, false, false, 0);
                }
                self.sounds.push(SOUND_CLIMB);
            }
        }
    }

    fn climbable(&self, id: u32) -> Option<u32> {
        let me = self.players.get(&id)?;
        self.players
            .iter()
            .filter(|&(&other, _)| other != id && self.steady(other) && self.shoulders.flight(other).is_none())
            .map(|(&other, h)| ((h.x - me.x).hypot(h.y - me.y), other))
            .filter(|&(d, _)| d <= RIDE_REACH)
            .map(|(d, other)| (d, self.shoulders.carrier_of(other).unwrap_or(other)))
            .filter(|&(_, base)| base != id && self.shoulders.riders(base).len() < MAX_RIDERS)
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(_, base)| base)
    }

    fn throw_top(&mut self, id: u32) {
        if self.shoulders.rearm.contains_key(&id) {
            return;
        }
        let Some(&top) = self.shoulders.riders(id).last() else {
            return;
        };
        let Some(carrier) = self.players.get(&id) else {
            return;
        };
        let from = (carrier.x, carrier.y);
        let way = if carrier.aim_x.hypot(carrier.aim_y) < 1e-3 { (1.0, 0.0) } else { unit(carrier.aim_x, carrier.aim_y) };
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
        let to = landing(&grid, from, way, THROW_REACH);
        let reach = (to.0 - from.0).hypot(to.1 - from.1);
        let ticks = ((THROW_TICKS as f32 * reach / THROW_REACH).round() as u32).clamp(THROW_LEAST, THROW_TICKS);
        if let Some(stack) = self.shoulders.stacks.get_mut(&id) {
            stack.pop();
        }
        self.shoulders.stacks.retain(|_, riders| !riders.is_empty());
        self.shoulders.flights.push(Flight { knight: top, by: id, from, to, age: 0, ticks, at: self.at, depth: self.dungeon.depth });
        self.shoulders.rearm.insert(id, THROW_REARM);
        if let Some(hero) = self.players.get_mut(&top) {
            (hero.aim_x, hero.aim_y) = way;
        }
        self.sounds.push(SOUND_THROW);
    }
}

fn ground_at(grid: &Grid, (x, y): (f32, f32)) -> bool {
    let r = HERO_RADIUS;
    let (w, h) = (grid.room.width(), grid.room.height());
    [(-r, -r), (r, -r), (-r, r), (r, r)].into_iter().all(|(dx, dy)| {
        let (px, py) = (x + dx, y + dy);
        px > 0.0 && py > 0.0 && px < w && py < h && grid.tile_at(px, py).ground()
    })
}

pub(super) fn landing(grid: &Grid, from: (f32, f32), way: (f32, f32), reach: f32) -> (f32, f32) {
    let steps = (reach / 0.25).ceil().max(1.0) as u32;
    let (w, h) = (grid.room.width(), grid.room.height());
    let mut best = from;
    for i in 1..=steps {
        let d = reach * i as f32 / steps as f32;
        let at = (from.0 + way.0 * d, from.1 + way.1 * d);
        if at.0 <= 0.0 || at.1 <= 0.0 || at.0 >= w || at.1 >= h {
            break;
        }
        if matches!(grid.tile_at(at.0, at.1), Tile::Wall | Tile::Door) {
            break;
        }
        if ground_at(grid, at) {
            best = at;
        }
    }
    best
}

fn beside(grid: &Grid, at: (f32, f32), prefer: (f32, f32), taken: &[(f32, f32)]) -> (f32, f32) {
    let start = prefer.1.atan2(prefer.0);
    for reach in [1.6f32, 2.4, 3.2] {
        for k in 0..8 {
            let turn = ((k + 1) / 2) as f32 * if k % 2 == 1 { 1.0 } else { -1.0 } * std::f32::consts::FRAC_PI_4;
            let a = start + turn;
            let spot = (at.0 + a.cos() * reach, at.1 + a.sin() * reach);
            if ground_at(grid, spot) && taken.iter().all(|t| (t.0 - spot.0).hypot(t.1 - spot.1) > 1.0) {
                return spot;
            }
        }
    }
    at
}

#[cfg(test)]
#[path = "../../tests/rules/together_shoulders__tests.rs"]
mod tests;
