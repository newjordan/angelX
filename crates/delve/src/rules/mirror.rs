//! Smooth motion between ticks, and a client's own knight ahead of them.
//!
//! A client keeps the last few ticks of the host's run, as the engine sends
//! them (`crate::wire`). Between two ticks a view may stand anywhere: `Pose`
//! remembers where things stood on the tick before, and `drawn_with` places
//! them part of the way on (and the client's own knight where its keys have
//! taken it), so 30 ticks a second draw as smooth frames.

use super::*;

/// Where everything stood on one tick, for drawing part of the way on.
#[derive(Clone, Debug, Default)]
pub struct Pose {
    tick: u64,
    at: usize,
    players: Vec<(u32, f32, f32)>,
    enemies: Vec<(u32, f32, f32)>,
    cameras: Vec<(u32, Option<(f32, f32)>)>,
    outside: BTreeMap<u32, usize>,
}

impl Run {
    /// Where everything stands now.
    pub fn pose(&self) -> Pose {
        Pose {
            tick: self.tick,
            at: self.at,
            outside: self
                .exits
                .outside
                .iter()
                .map(|(&id, e)| (id, e.at))
                .collect(),
            cameras: self.players.iter().map(|(&id, h)| (id, h.camera)).collect(),
            players: self.players.iter().map(|(&id, h)| (id, h.x, h.y)).collect(),
            enemies: self.enemies.iter().map(|e| (e.id, e.x, e.y)).collect(),
        }
    }

    /// The run as a client reads it from the engine's wire
    /// (`crate::wire`): what a friend's screen holds of this tick.
    #[cfg(any(test, feature = "testing"))]
    pub fn wire_copy(&self) -> Run {
        crate::wire::read(&crate::wire::publish(self).expect("a run makes a tree")).expect("a tree reads back")
    }

    /// `f` with everyone but knight `own.0` drawn part of the way from
    /// `before`, and that knight drawn as `own.1` (a client's own knight:
    /// the newest tick's, walked on by its own keys); then put back as it
    /// was, with no copy of the whole run per frame.
    pub fn drawn_with<T>(
        &mut self,
        before: &Pose,
        alpha: f32,
        own: Option<(u32, &Hero)>,
        f: impl FnOnce(&Run) -> T,
    ) -> T {
        self.drawn_with_shots(before, alpha, own, None, f)
    }

    /// `drawn_with`, and with `shots` given, knight `own.0`'s own shots
    /// (`Projectile::loosed_by`) drawn as those rather than as this tick
    /// has them: a client's own shots, drawn ahead with its knight. Then
    /// all put back as they were.
    pub fn drawn_with_shots<T>(
        &mut self,
        before: &Pose,
        alpha: f32,
        own: Option<(u32, &Hero)>,
        shots: Option<&[Projectile]>,
        f: impl FnOnce(&Run) -> T,
    ) -> T {
        let now = self.pose();
        let places: Vec<(f32, f32)> = self.projectiles.iter().map(|p| (p.x, p.y)).collect();
        self.blend(before, alpha);
        let swapped = own.map(|(id, hero)| (id, self.players.insert(id, hero.clone())));
        // The knight's own shots out (each with its place in the list), and
        // the ones given in, after everyone else's.
        let mut mine: Vec<(usize, Projectile)> = Vec::new();
        let theirs = self.projectiles.len();
        if let (Some((id, _)), Some(drawn)) = (own, shots) {
            let mut i = 0;
            while i < self.projectiles.len() {
                if self.projectiles[i].loosed_by(id) {
                    mine.push((i + mine.len(), self.projectiles.remove(i)));
                } else {
                    i += 1;
                }
            }
            self.projectiles.extend(drawn.iter().cloned());
        }
        let out = f(self);
        if shots.is_some() && own.is_some() {
            self.projectiles.truncate(theirs - mine.len());
            for (at, shot) in mine {
                self.projectiles.insert(at, shot);
            }
        }
        if let Some((id, was)) = swapped {
            match was {
                Some(hero) => self.players.insert(id, hero),
                None => self.players.remove(&id),
            };
        }
        self.put_back(&now, &places);
        out
    }

    /// Everything `drawn_*` moved, back where `now` and `shots` had it.
    fn put_back(&mut self, now: &Pose, shots: &[(f32, f32)]) {
        for (id, x, y) in &now.players {
            if let Some(hero) = self.players.get_mut(id) {
                (hero.x, hero.y) = (*x, *y);
            }
        }
        for (id, camera) in &now.cameras {
            if let Some(hero) = self.players.get_mut(id) {
                hero.camera = *camera;
            }
        }
        for (id, x, y) in &now.enemies {
            if let Some(enemy) = self.enemies.iter_mut().find(|e| e.id == *id) {
                (enemy.x, enemy.y) = (*x, *y);
            }
        }
        for (shot, &(x, y)) in self.projectiles.iter_mut().zip(shots) {
            (shot.x, shot.y) = (x, y);
        }
    }

    /// Whether knight `id`'s going is its own to draw ahead: the run is
    /// live, no ride holds the reins and Turbo is not doubling ticks; the
    /// knight is standing and not stone, in a top-down room, and in none of
    /// the states something other than its keys moves it (the privy and the
    /// pit stop, a Bladewind, a Ravage's throw). A client draws a knight
    /// that is not from the server's ticks, blended like everyone else.
    pub fn predictable(&self, id: u32) -> bool {
        let at = self.exits.outside.get(&id).map_or(self.at, |e| e.at);
        self.active()
            && !self.hitstop.frozen()
            && !self.turbo_on()
            && !super::activity::RIDES.iter().any(|ride| (ride.on)(self))
            && !matches!(self.dungeon.rooms[at].kind, RoomKind::Ledge | RoomKind::Wanted)
            && self.players.get(&id).is_some_and(|h| {
                h.hp > 0
                    && !h.stone
                    && h.privy == 0
                    && h.pit.stop.is_none()
                    && h.pit.result.is_none()
                    && h.slashes == 0
                    && h.tossed == 0
            })
    }

    /// Step knight `id` one tick on by `input`, as the sim steps it
    /// (`Run::play`, through the same `Hero::own_keys` and
    /// `Hero::own_step`): the roll's rearm runs down, the guard key acts
    /// (a roll starts and runs, a shield or a wall slows the walk, a spin
    /// turns), a dagger's blink lands, and the knight slides along stone at
    /// the ground's pace. Nothing but the knight's own going moves. False,
    /// and nothing done, when it is not `predictable`.
    pub fn predict(&mut self, id: u32, input: Input) -> bool {
        if !self.predictable(id) {
            return false;
        }
        let outside = self.exits.outside.get(&id);
        let at = outside.map_or(self.at, |e| e.at);
        let barred = outside.is_none() && self.barred();
        let (webs, song) = if outside.is_some() {
            (&[][..], None)
        } else {
            (&self.webs[..], self.song.as_deref())
        };
        let roads = (self.dungeon.rooms[at].kind == RoomKind::Kingdom)
            .then(|| map::geography(&self.kingdom.wishes));
        let shield_free = !self.works.flights.iter().any(|f| f.owner == id);
        let tune = &self.tune;
        let grid = Grid {
            room: &self.dungeon.rooms[at],
            barred,
        };
        let Some(hero) = self.players.get_mut(&id) else {
            return false;
        };
        let input = own_input(hero, input);
        let (walk, _) = hero.own_keys(&input, false, shield_free);
        let ((dx, dy, speed), _) = hero.own_step(walk, &grid, tune.roll_speed, |h| {
            fx::ground_pace(h, webs, song, roads.as_deref(), tune)
        });
        // In the Wax Maze, the lanes' glide, as the sim steps it.
        let glided = self
            .folly
            .as_mut()
            .filter(|m| m.room == at && outside.is_none())
            .and_then(|m| {
                let way = if walk != (0.0, 0.0) { walk } else { (dx, dy) };
                m.glide(id, grid.room, (hero.x, hero.y), way, speed * DT)
            });
        if let Some(to) = glided {
            let moved = (to.0 - hero.x, to.1 - hero.y);
            (hero.x, hero.y) = to;
            if input.aim_x == 0 && input.aim_y == 0 && moved.0.abs().max(moved.1.abs()) > 1e-4 {
                (hero.aim_x, hero.aim_y) = unit(moved.0, moved.1);
            }
            return true;
        }
        (hero.x, hero.y) = grid.slide(
            (hero.x, hero.y),
            (dx * speed * DT, dy * speed * DT),
            HERO_RADIUS,
            Mover::Hero,
        );
        true
    }

    /// Knight `id`'s fire key on one tick, as the sim fires it (`Run::play`,
    /// through the same `Hero::loose`), for a client drawing its own shots
    /// ahead of the server: called after `predict` of the same input. The
    /// fire cooldown runs down; with the fire key or an aim held, the arm
    /// ready and no guard up, the cooldown is set again (and moved by the
    /// fire-rate knob) and the shots loosed are pushed to `shots` where
    /// they leave the knight, before their first step. A handgonne kicks
    /// the knight back. A punch or a forged weapon's swing sets the
    /// cooldown and looses nothing, and neither does a festival's F.
    /// `tick` is the tick the command plays on: a stream strays by it.
    /// False, and nothing done, when the knight is not `predictable`.
    pub fn predict_fire(&mut self, id: u32, input: Input, tick: u64, shots: &mut Vec<Projectile>) -> bool {
        if !self.predictable(id) {
            return false;
        }
        let arcade = self.arcade();
        let burning = self.works.kits.get(&id).is_some_and(|k| k.burning > 0);
        let rng = mix(self.seed ^ tick.wrapping_mul(0x9e37) ^ u64::from(id) << 48);
        let tune = self.tune;
        let outside = self.exits.outside.get(&id);
        let at = outside.map_or(self.at, |e| e.at);
        let barred = outside.is_none() && self.barred();
        let grid = Grid {
            room: &self.dungeon.rooms[at],
            barred,
        };
        let enemies: &[Enemy] = if outside.is_none() { &self.enemies } else { &[] };
        // What the server holds in flight of this knight's: a volley past
        // its budget leaves its middle, as the server's does.
        let flying = in_flight(&self.projectiles, id);
        // A borrowed body's volleys are the server's to draw (`bodies`).
        let Some(hero) = self.players.get_mut(&id).filter(|h| h.possessed.is_none()) else {
            return false;
        };
        let input = own_input(hero, input);
        let was = hero.fire_cooldown;
        hero.fire_cooldown = hero.fire_cooldown.saturating_sub(1);
        let (ax, ay) = unit(input.aim_x as f32, input.aim_y as f32);
        let guarded = hero.dash_ticks > 0 || hero.shielding || hero.walling || hero.spin > 0;
        if arcade || !(input.fire || ax != 0.0 || ay != 0.0) || hero.fire_cooldown != 0 || guarded {
            return true;
        }
        let target = enemies.iter().filter(|e| e.hp > 0).min_by(|a, b| {
            (a.x - hero.x).hypot(a.y - hero.y).total_cmp(&(b.x - hero.x).hypot(b.y - hero.y))
        });
        let melee = hero.forged.as_ref().and_then(|weapon| {
            weapon.melee.filter(|m| {
                (weapon.bolt.is_none() && weapon.throw.is_none())
                    || target.is_some_and(|e| (e.x - hero.x).hypot(e.y - hero.y) <= m.reach + e.radius())
            })
        });
        if hero.unarmed() {
            hero.fire_cooldown = 9;
            hero.swing = 5;
            hero.sword = false;
        } else if let Some(melee) = melee {
            if let Some(enemy) = target.filter(|e| (e.x - hero.x).hypot(e.y - hero.y) <= melee.reach + e.radius()) {
                (hero.aim_x, hero.aim_y) = unit(enemy.x - hero.x, enemy.y - hero.y);
            }
            hero.fire_cooldown = hero.cooldown(melee.every);
            hero.swing = 6;
            hero.sword = false;
        } else {
            let from = shots.len();
            let burning = burning && hero.forged.is_none() && hero.weapon == Weapon::Bow;
            if let Some(Loosed::Arm(_, (ax, ay))) = hero.loose(id, &self.book, burning, rng, flying, shots)
                && hero.weapon == Weapon::Handgonne
            {
                // The kick: back along the aim, never through stone.
                let kick = (-ax * feel::RECOIL, -ay * feel::RECOIL);
                (hero.x, hero.y) = grid.slide((hero.x, hero.y), kick, HERO_RADIUS, Mover::Hero);
            }
            if tune.shot_speed != 1.0 {
                for shot in &mut shots[from..] {
                    shot.vx *= tune.shot_speed;
                    shot.vy *= tune.shot_speed;
                }
            }
        }
        // The attack-rate knob, as the sim moves a cooldown set this tick
        // (`tune::tune_knights`).
        let now = hero.fire_cooldown;
        if tune.fire_rate != 100 && now != was && now > was.saturating_sub(1) {
            hero.fire_cooldown = tune.cooldown(now);
        }
        true
    }

    /// Where knight `id`'s own `shot` is drawn `ticks` of flight on from
    /// `from`, for a client drawing its shots ahead of the server: along its
    /// course and no further than its life, stopped at stone (one that
    /// bounces is held there, any other is gone) and gone at the first live
    /// monster on the way unless it pierces (one that forks stops there
    /// too: what it splits into is the server's to send). `None` when it is
    /// gone.
    pub fn flown(&self, id: u32, shot: &Projectile, from: (f32, f32), ticks: f32) -> Option<(f32, f32)> {
        if ticks > shot.ttl as f32 {
            return None;
        }
        let to = (from.0 + shot.vx * DT * ticks, from.1 + shot.vy * DT * ticks);
        let outside = self.exits.outside.get(&id);
        let at = outside.map_or(self.at, |e| e.at);
        let grid = Grid {
            room: &self.dungeon.rooms[at],
            barred: outside.is_none() && self.barred(),
        };
        let to = match grid
            .shot_stop_body(from, to, patterns::body(shot))
            .filter(|_| shot.traits.back != 2)
        {
            Some(stop) if shot.traits.bounce > 0 => stop,
            Some(_) => return None,
            None => to,
        };
        let stops = shot.pierce == 0;
        let struck = || {
            self.enemies
                .iter()
                .any(|e| e.hp > 0 && hit_segment(from.0, from.1, to.0, to.1, e.x, e.y, e.radius()))
        };
        if stops && outside.is_none() && struck() {
            return None;
        }
        Some(to)
    }

    fn blend(&mut self, before: &Pose, alpha: f32) {
        let view = self;
        let gap = view.tick.wrapping_sub(before.tick);
        if view.hitstop.frozen() || !(1..=BLEND_TICKS).contains(&gap) || before.at != view.at {
            return;
        }
        let k = 1.0 - alpha.clamp(0.0, 1.0);
        let gap = gap as f32;
        let back = |now: f32, then: f32| {
            // A jump of more than a few units a tick is a teleport (a pit,
            // a door): it is not drawn sliding.
            if (now - then).abs() > JUMP * gap {
                now
            } else {
                now - (now - then) * k
            }
        };
        for (id, x, y) in &before.players {
            if before.outside.get(id).copied() != view.exits.outside.get(id).map(|e| e.at) {
                continue;
            }
            if let Some(hero) = view.players.get_mut(id) {
                (hero.x, hero.y) = (back(hero.x, *x), back(hero.y, *y));
            }
        }
        for (id, camera) in &before.cameras {
            if before.outside.get(id).copied() != view.exits.outside.get(id).map(|e| e.at) {
                continue;
            }
            if let Some(hero) = view.players.get_mut(id)
                && let (Some((cx, cy)), Some((px, py))) = (hero.camera, *camera)
            {
                hero.camera = Some((back(cx, px), back(cy, py)));
            }
        }
        for (id, x, y) in &before.enemies {
            if let Some(enemy) = view.enemies.iter_mut().find(|e| e.id == *id) {
                (enemy.x, enemy.y) = (back(enemy.x, *x), back(enemy.y, *y));
            }
        }
        for shot in &mut view.projectiles {
            shot.x -= shot.vx * DT * k * gap;
            shot.y -= shot.vy * DT * k * gap;
        }
    }
}

impl Projectile {
    /// Whether knight `id` loosed this shot (its arm's, a forged weapon's,
    /// or a phantom's copy of them).
    pub fn loosed_by(&self, id: u32) -> bool {
        !self.hostile && u32::from(self.traits.owner) == id
    }
}

/// A seat's keys as the sim reads them (`Run::play`): a command it would
/// refuse is none, and a frog only hops.
fn own_input(hero: &Hero, input: Input) -> Input {
    let input = Some(input).filter(|i| i.valid()).unwrap_or_default();
    if hero.frog() {
        Input {
            move_x: input.move_x,
            move_y: input.move_y,
            ..Input::default()
        }
    } else {
        input
    }
}

/// Two poses this many ticks apart or fewer slide from one to the other:
/// a host that sends every other tick (15 snapshots a second at the 30 Hz
/// tick) is drawn as smoothly as one that sends them all. A longer gap is
/// drawn as it stands.
const BLEND_TICKS: u64 = 2;
/// A thing that moved further than this in a tick jumped.
const JUMP: f32 = 4.0;

#[cfg(test)]
#[path = "../../tests/rules/together_mirror__tests.rs"]
mod tests;
