//! The Mirage Duelist: the fast, evasive fencer among the champions. Where
//! the Phantom Lancer is a swarm of weak images, the Duelist keeps few and
//! moves a great deal.
//!
//! | | F | 1 | 2 | 3 | R |
//! |---|---|---|---|---|---|
//! | Mirage Duelist | Mirage Foil: fast narrow thrusts | Swashbuckle: a dash that bounces off walls, then four slashes | Mirror Swap: change places with the nearest image | Echo Strike: the next three thrusts each strike twice | Hall of Mirrors |
//!
//! The images are the Phantom Lancer's own (`ults::Phantom`), and the foil's
//! thrust is mirrored by them the same way (`ults::phantom_strikes`). The
//! dodge roll leaves a decoy image behind. Every number is one fixed
//! number, and nothing here rolls a die: a replay plays the same.

use super::super::ults::{self, Phantom};
use super::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

// ─── Swashbuckle ─────────────────────────────────────────────────────────────

/// How far the dash runs before it meets a wall (arena units: 5.5 tiles),
/// and what the first wall it meets makes of that (the whole dash's budget
/// is then this many times as long, once).
pub const SWASH_REACH: f32 = 11.0;
pub const SWASH_TRIPLE: f32 = 3.0;
/// How fast it runs (arena units a second), and the longest step it takes
/// at a time: a wall is never skipped.
pub const SWASH_SPEED: f32 = 26.0;
const SWASH_STEP: f32 = 0.1;
/// How many times a wall turns the dash, the most it may be steered a tick
/// (90 degrees a second) and how long (ticks) the knight is untouchable at
/// most, the dash over or not.
pub const SWASH_REBOUNDS: u8 = 2;
pub const SWASH_TURN: f32 = FRAC_PI_2 * DT;
pub const SWASH_INVULN: u32 = HZ;
/// The flurry that follows: how many slashes, the ticks between them, and
/// each one's reach and arc, and its blow (the foil's, times 12 in 10).
pub const SWASH_SLASHES: u8 = 4;
pub const SWASH_EVERY: u32 = 8;
pub const SWASH_SLASH_REACH: f32 = 4.0;
pub const SWASH_SLASH_ARC: f32 = 30.0;
/// How long the image left where the dash began stands.
pub const SWASH_IMAGE: u32 = 4 * HZ;
/// The foil's blow when its card is lost to the knight, for the slashes.
const FOIL_DAMAGE: u32 = 14;

/// A dash and its flurry, on the knight: where the dash is heading and how
/// much of its way is left, and the slashes still to come. Left out of a
/// save while the knight is still.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Swash {
    /// The whole distance the dash may run (zero when it is not running),
    /// and how far it has run.
    pub budget: f32,
    pub run: f32,
    /// How many walls have turned it, and whether the first has tripled it.
    pub bounces: u8,
    pub tripled: bool,
    /// The way it is heading (a unit vector).
    pub hx: f32,
    pub hy: f32,
    /// The aim held this tick, if any: where the dash is steered.
    pub steer: (f32, f32),
    /// Ticks of the dash the knight has been untouchable for.
    pub shielded: u32,
    /// The flurry: slashes to come and ticks until the next.
    pub slashes: u8,
    pub wait: u32,
}

impl Swash {
    pub fn idle(&self) -> bool {
        *self == Swash::default()
    }

    /// The dash is running.
    pub fn dashing(&self) -> bool {
        self.budget > 0.0
    }
}

/// Whether the dash may stand at `(x, y)`: floor the knight fits on, in the
/// room, and not a door or stairs, which would take the knight out of it.
fn dash_clear(grid: &Grid, x: f32, y: f32) -> bool {
    let r = HERO_RADIUS;
    grid.clear(x, y, r, Mover::Hero)
        && x >= r
        && y >= r
        && x <= grid.room.width() - r
        && y <= grid.room.height() - r
        && [(-r, -r), (r, -r), (-r, r), (r, r)]
            .into_iter()
            .all(|(dx, dy)| !matches!(grid.tile_at(x + dx, y + dy), Tile::Door | Tile::Stairs))
}

impl Hero {
    /// One tick of the dash, in place of the walk (`own_step`): the heading
    /// turned a little toward the aim held, the knight untouchable, and the
    /// dash run in steps no longer than `SWASH_STEP`. A wall turns it: the
    /// heading is reflected off it (the axis it met flipped, both at a
    /// corner), and the first wall to do so makes the dash three times as
    /// long, what has run counting against that. A third wall, or the
    /// budget spent, ends it where the knight stands, on open floor, and
    /// the flurry begins.
    pub(in crate::rules) fn swash_run(&mut self, grid: &Grid) {
        let mut s = self.champ.swash;
        if s.steer != (0.0, 0.0) {
            let cur = s.hy.atan2(s.hx);
            let want = s.steer.1.atan2(s.steer.0);
            let turn = ((want - cur + PI).rem_euclid(TAU) - PI).clamp(-SWASH_TURN, SWASH_TURN);
            (s.hx, s.hy) = ((cur + turn).cos(), (cur + turn).sin());
        }
        if s.shielded < SWASH_INVULN {
            // Held through this tick's hits; the next tick's turn takes it
            // back down unless the dash still has its second to spend.
            self.invulnerable = self.invulnerable.max(1);
            s.shielded += 1;
        }
        let (mut x, mut y) = (self.x, self.y);
        let mut left = (SWASH_SPEED * DT).min(s.budget - s.run);
        let mut over = false;
        while left > 1e-4 {
            let hop = left.min(SWASH_STEP);
            left -= hop;
            let (dx, dy) = (s.hx * hop, s.hy * hop);
            if dash_clear(grid, x + dx, y + dy) {
                (x, y) = (x + dx, y + dy);
                s.run += hop;
                continue;
            }
            if s.bounces >= SWASH_REBOUNDS {
                over = true;
                break;
            }
            s.bounces += 1;
            if !s.tripled {
                s.tripled = true;
                s.budget = SWASH_REACH * SWASH_TRIPLE;
            }
            // Which axis the wall stands across; a corner, or a clip of one
            // that neither axis alone meets, turns both.
            let (along_x, along_y) = (!dash_clear(grid, x + dx, y), !dash_clear(grid, x, y + dy));
            if along_x == along_y {
                (s.hx, s.hy) = (-s.hx, -s.hy);
            } else if along_x {
                s.hx = -s.hx;
            } else {
                s.hy = -s.hy;
            }
        }
        (self.x, self.y) = (x, y);
        if over || s.run >= s.budget - 1e-4 {
            s.budget = 0.0;
            s.slashes = SWASH_SLASHES;
            s.wait = 0;
        }
        // Facing: where it heads, unless the knight aims.
        if s.steer == (0.0, 0.0) {
            (self.aim_x, self.aim_y) = (s.hx, s.hy);
        }
        self.champ.swash = s;
    }
}

// ─── Echo Strike ─────────────────────────────────────────────────────────────

/// Echo Strike: how many thrusts, how long they have to be made, how long
/// after a thrust its echo lands (ticks), and the share of the thrust's
/// blow the echo deals (percent).
pub const ECHO_THRUSTS: u8 = 3;
pub const ECHO_TICKS: u32 = 6 * HZ;
pub const ECHO_DELAY: u32 = 9;
pub const ECHO_SHARE: u32 = 50;

/// A thrust to be struck again from where it was made.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Echo {
    pub owner: u32,
    pub at: (f32, f32),
    pub aim: (f32, f32),
    pub reach: f32,
    pub arc: f32,
    pub damage: u32,
    pub wait: u32,
}

/// Echo Strike: the knight's thrust, made with the stance up, is struck
/// once more from the same place after `ECHO_DELAY` ticks.
pub(in crate::rules) fn echo_thrust(
    echoes: &mut Vec<Echo>,
    hero: &mut Hero,
    id: u32,
    (reach, arc): (f32, f32),
    damage: u32,
) {
    if hero.champ.echoes == 0 || hero.champ.echo_ticks == 0 {
        return;
    }
    hero.champ.echoes -= 1;
    if hero.champ.echoes == 0 {
        hero.champ.echo_ticks = 0;
    }
    echoes.push(Echo {
        owner: id,
        at: (hero.x, hero.y),
        aim: (hero.aim_x, hero.aim_y),
        reach,
        arc,
        damage: (damage * ECHO_SHARE / 100).max(1),
        // Struck on the tick `ECHO_DELAY` after the thrust (the field's tick
        // that pushes it counts one).
        wait: ECHO_DELAY + 1,
    });
}

// ─── the dodge, the swap and the hall ────────────────────────────────────────

/// The decoy a dodge roll leaves: how long it stands.
pub const DECOY_TICKS: u32 = HZ * 3 / 2;

/// Mirror Swap: how far the blink goes when there is no image to change
/// places with, how long the image it leaves stands, and how long the knight
/// is untouchable after either (ticks, counting the one it is cast on).
pub const SWAP_BLINK: f32 = 4.0;
pub const SWAP_IMAGE: u32 = 4 * HZ;
pub const SWAP_SHIELD: u32 = 12;

/// Hall of Mirrors: how many images, how long they stand, how fast they run,
/// how near a foe they strike from, the ticks between one image's strikes,
/// the share of a thrust's blow each deals (percent), and how far round the
/// knight they start.
pub const HALL_IMAGES: usize = 3;
pub const HALL_TICKS: u32 = 6 * HZ;
pub const HALL_SPEED: f32 = 14.0;
pub const HALL_REACH: f32 = 2.4;
pub const HALL_EVERY: u32 = 10;
pub const HALL_SHARE: u32 = 70;
const HALL_RING: f32 = 2.4;

/// Whether the knight's dodge roll leaves a decoy, and a place for one.
pub(in crate::rules) fn decoy(phantoms: &mut Vec<Phantom>, hero: &Hero, id: u32) {
    if of(hero) == Some(Champion::MirageDuelist) && phantoms.len() < spells::IMPOSTURE_MAX {
        phantoms.push(Phantom::standing(id, (hero.x, hero.y), DECOY_TICKS));
    }
}

/// The blow of the knight's thrust: the foil's, as forged on the arm.
fn thrust_damage(hero: &Hero) -> u32 {
    hero.scaled(hero.forged.as_ref().and_then(|w| w.melee).map_or(FOIL_DAMAGE, |m| m.damage))
}

fn unit_or(ax: f32, ay: f32, fallback: (f32, f32)) -> (f32, f32) {
    let (ux, uy) = unit(ax, ay);
    if ux == 0.0 && uy == 0.0 { fallback } else { (ux, uy) }
}

impl Run {
    /// Swashbuckle, cast: the dash is set running along the aim (it moves
    /// from the next tick, `Hero::swash_run`), an image is left where it
    /// began, and the voice goes up once.
    pub(in crate::rules) fn swashbuckle(&mut self, id: u32) {
        let Some(hero) = self.players.get_mut(&id).filter(|h| h.hp > 0) else {
            return;
        };
        let (ax, ay) = unit_or(hero.aim_x, hero.aim_y, (0.0, -1.0));
        hero.dash_ticks = 0;
        hero.champ.swash = Swash { budget: SWASH_REACH, hx: ax, hy: ay, ..Swash::default() };
        let at = (hero.x, hero.y);
        ults::add_image(&mut self.phantoms, Phantom::standing(id, at, SWASH_IMAGE));
        // Once a cast, at the start: the Duelist's "bing bong" (`sfx/mirage_bing_bong.mp3`).
        self.sounds.push("mirage_bing_bong");
    }

    /// The slashes that follow the dash: one every `SWASH_EVERY` ticks,
    /// each along the aim held that tick, struck at every foe in its arc,
    /// with the images' share beside it.
    pub(in crate::rules) fn swash_flurry(&mut self, id: u32) {
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        let s = &mut hero.champ.swash;
        if s.dashing() || s.slashes == 0 {
            return;
        }
        if s.wait > 0 {
            s.wait -= 1;
            return;
        }
        s.slashes -= 1;
        s.wait = SWASH_EVERY - 1;
        // The last slash leaves nothing of the dash on the knight.
        if s.slashes == 0 {
            hero.champ.swash = Swash::default();
        }
        hero.swing = 6;
        hero.sword = false;
        let (ax, ay) = unit_or(hero.aim_x, hero.aim_y, (0.0, -1.0));
        let (hx, hy) = (hero.x, hero.y);
        let blow = thrust_damage(hero);
        let damage = (blow * 12 + 5) / 10;
        let (reach, arc) = (SWASH_SLASH_REACH, SWASH_SLASH_ARC);
        if let Some(hero) = self.players.get(&id) {
            self.feel.melee.swing(id, hero, melee::Stroke::Slash, reach, arc);
        }
        let angle = ay.atan2(ax);
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
        let caught: Vec<usize> = self
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                let (dx, dy) = (e.x - hx, e.y - hy);
                let delta = (dy.atan2(dx) - angle).sin().atan2((dy.atan2(dx) - angle).cos()).abs();
                e.hp > 0
                    && dx.hypot(dy) <= reach + e.radius()
                    && delta <= arc.to_radians() / 2.0
                    && grid.open((hx + e.x) / 2.0, (hy + e.y) / 2.0, Mover::Hero)
            })
            .map(|(i, _)| i)
            .collect();
        let images = ults::phantom_strikes(&self.phantoms, id, &mut self.enemies, &grid, (ax, ay), (reach, arc), damage);
        for (i, dealt) in images {
            self.enemies[i].wits.foe = id;
            let foe = self.enemies[i].id;
            self.feel.credit(id, foe, dealt, self.tick);
            if let Some(hero) = self.players.get_mut(&id) {
                hero.ult_charge = (hero.ult_charge + dealt / 2).min(ults::ULT_FULL);
            }
        }
        for i in caught {
            self.smite(id, i, damage, 0, true);
        }
        self.sounds.push("swing_spear");
    }

    /// Mirror Swap: the knight changes places with their nearest image (the
    /// image takes the spot the knight left and stands there), or with none
    /// to change with, blinks a short way along the aim and leaves one. The
    /// knight is untouchable a moment after either.
    pub(in crate::rules) fn mirror_swap(&mut self, id: u32) {
        let Some(hero) = self.players.get(&id).filter(|h| h.hp > 0) else {
            return;
        };
        let (hx, hy) = (hero.x, hero.y);
        let (ax, ay) = unit_or(hero.aim_x, hero.aim_y, (0.0, -1.0));
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
        let nearest = self
            .phantoms
            .iter()
            .enumerate()
            .filter(|(_, p)| p.owner == id && p.left > 0 && dash_clear(&grid, p.x, p.y))
            .min_by(|a, b| {
                let d = |p: &Phantom| (p.x - hx).hypot(p.y - hy);
                d(a.1).total_cmp(&d(b.1))
            })
            .map(|(k, _)| k);
        let to = match nearest {
            Some(k) => {
                let image = &mut self.phantoms[k];
                let to = (image.x, image.y);
                // It stands where the knight stood, and no longer charges.
                (image.x, image.y, image.dx, image.dy) = (hx, hy, 0.0, 0.0);
                image.fixed = true;
                image.charge = false;
                to
            }
            None => {
                let to = grid.halted((hx, hy), (ax * SWAP_BLINK, ay * SWAP_BLINK));
                ults::add_image(&mut self.phantoms, Phantom::standing(id, (hx, hy), SWAP_IMAGE));
                to
            }
        };
        if fx::sparks_room(&self.sparks, &self.fx) {
            self.sparks.push(Spark { from: (hx, hy), to, ttl: 7 });
        }
        if let Some(hero) = self.players.get_mut(&id) {
            (hero.x, hero.y) = to;
            hero.dash_ticks = 0;
            hero.invulnerable = hero.invulnerable.max(SWAP_SHIELD);
        }
    }

    /// Hall of Mirrors, cast: three images ring the knight, and run at the
    /// nearest foe (`tick_mirrors`). A recast replaces them.
    pub(in crate::rules) fn hall_of_mirrors(&mut self, id: u32) {
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        hero.champ.cast = CAST_POSE;
        hero.champ.spell = 4;
        let (hx, hy) = (hero.x, hero.y);
        let (ax, ay) = unit_or(hero.aim_x, hero.aim_y, (0.0, -1.0));
        self.phantoms.retain(|p| p.owner != id);
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
        for k in 0..HALL_IMAGES {
            let a = ay.atan2(ax) + TAU * (k as f32 + 0.5) / HALL_IMAGES as f32;
            for r in [HALL_RING, HALL_RING * 0.7, HALL_RING * 0.4] {
                let (dx, dy) = (a.cos() * r, a.sin() * r);
                if grid.clear(hx + dx, hy + dy, HERO_RADIUS, Mover::Hero) {
                    let mut image = Phantom::standing(id, (hx + dx, hy + dy), HALL_TICKS);
                    image.charge = true;
                    (image.dx, image.dy) = (ax, ay);
                    ults::add_image(&mut self.phantoms, image);
                    break;
                }
            }
        }
    }

    /// The Hall's images, a tick: each runs at the nearest foe and, near it,
    /// strikes it every `HALL_EVERY` ticks for a share of the knight's blow.
    pub(in crate::rules) fn tick_mirrors(&mut self) {
        if !self.phantoms.iter().any(|p| p.charge) {
            return;
        }
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
        let mut blows = Vec::new();
        for k in 0..self.phantoms.len() {
            let mut p = self.phantoms[k];
            p.strike = p.strike.saturating_sub(1);
            if p.charge && p.left > 0 {
                let Some(blow) = self.players.get(&p.owner).filter(|h| h.hp > 0).map(thrust_damage) else {
                    self.phantoms[k] = p;
                    continue;
                };
                // The nearest foe, by the same ordering every blow on a crowd uses.
                if let Some(&i) = aoe::reached(&self.enemies, (p.x, p.y), |_| true).first() {
                    let e = &self.enemies[i];
                    let (dx, dy) = (e.x - p.x, e.y - p.y);
                    let gap = dx.hypot(dy);
                    let (ux, uy) = unit_or(dx, dy, (p.dx, p.dy));
                    (p.dx, p.dy) = (ux, uy);
                    if gap > HALL_REACH + e.radius() {
                        let step = (HALL_SPEED * DT).min(gap - HALL_REACH);
                        (p.x, p.y) = grid.slide((p.x, p.y), (ux * step, uy * step), HERO_RADIUS, Mover::Hero);
                    } else if (self.tick + 3 * k as u64) % u64::from(HALL_EVERY) == 0 {
                        p.strike = 6;
                        blows.push((p.owner, i, (blow * HALL_SHARE / 100).max(1)));
                    }
                }
            }
            self.phantoms[k] = p;
        }
        for (owner, i, damage) in blows {
            self.smite(owner, i, damage, 0, true);
        }
    }

    /// The echoes of Echo Strike, a tick: each waits its beat, then strikes
    /// from where the thrust was made.
    pub(in crate::rules) fn tick_echoes(&mut self) {
        if self.champions.echoes.is_empty() {
            return;
        }
        let mut due = Vec::new();
        self.champions.echoes.retain_mut(|e| {
            e.wait = e.wait.saturating_sub(1);
            if e.wait == 0 {
                due.push(*e);
            }
            e.wait > 0
        });
        for e in due {
            let Some(hero) = self.players.get(&e.owner).filter(|h| h.hp > 0) else {
                continue;
            };
            let mut at = hero.clone();
            (at.x, at.y, at.aim_x, at.aim_y) = (e.at.0, e.at.1, e.aim.0, e.aim.1);
            self.feel.melee.swing(e.owner, &at, melee::Stroke::Thrust, e.reach, e.arc);
            let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
            let angle = e.aim.1.atan2(e.aim.0);
            let struck = aoe::strike(&mut self.enemies, e.at, e.damage, |foe| {
                let (dx, dy) = (foe.x - e.at.0, foe.y - e.at.1);
                let delta = (dy.atan2(dx) - angle).sin().atan2((dy.atan2(dx) - angle).cos()).abs();
                dx.hypot(dy) <= e.reach + foe.radius()
                    && delta <= e.arc.to_radians() / 2.0
                    && grid.open((e.at.0 + foe.x) / 2.0, (e.at.1 + foe.y) / 2.0, Mover::Hero)
            });
            for (i, dealt) in struck {
                self.enemies[i].wits.foe = e.owner;
                let foe = self.enemies[i].id;
                self.feel.credit(e.owner, foe, dealt, self.tick);
                self.sounds.push("hit");
                if let Some(hero) = self.players.get_mut(&e.owner) {
                    hero.ult_charge = (hero.ult_charge + dealt / 2).min(ults::ULT_FULL);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/rules/together_mirage__tests.rs"]
mod tests;
