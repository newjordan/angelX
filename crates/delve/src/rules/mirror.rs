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
        let now = self.pose();
        let shots: Vec<(f32, f32)> = self.projectiles.iter().map(|p| (p.x, p.y)).collect();
        self.blend(before, alpha);
        let swapped = own.map(|(id, hero)| (id, self.players.insert(id, hero.clone())));
        let out = f(self);
        if let Some((id, was)) = swapped {
            match was {
                Some(hero) => self.players.insert(id, hero),
                None => self.players.remove(&id),
            };
        }
        self.put_back(&now, &shots);
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
        // As the sim reads a seat's keys: a command it would refuse is none,
        // and a frog only hops.
        let input = Some(input).filter(|i| i.valid()).unwrap_or_default();
        let input = if hero.frog() {
            Input {
                move_x: input.move_x,
                move_y: input.move_y,
                ..Input::default()
            }
        } else {
            input
        };
        let (walk, _) = hero.own_keys(&input, false, shield_free);
        let ((dx, dy, speed), _) = hero.own_step(walk, &grid, tune.roll_speed, |h| {
            fx::ground_pace(h, webs, song, roads.as_deref(), tune)
        });
        (hero.x, hero.y) = grid.slide(
            (hero.x, hero.y),
            (dx * speed * DT, dy * speed * DT),
            HERO_RADIUS,
            Mover::Hero,
        );
        true
    }

    fn blend(&mut self, before: &Pose, alpha: f32) {
        let view = self;
        let gap = view.tick.wrapping_sub(before.tick);
        if !(1..=BLEND_TICKS).contains(&gap) || before.at != view.at {
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
