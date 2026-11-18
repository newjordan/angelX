//! Smooth motion between ticks, and a client's own knight ahead of them.
//!
//! A client keeps the last few ticks of the host's run, as the engine sends
//! them (`crate::wire`). Between two ticks a view may stand anywhere: `Pose`
//! remembers where things stood on the tick before, and `drawn_with` places
//! them part of the way on (and the client's own knight where its keys have
//! taken it), so 30 ticks a second draw as smooth frames.

use super::*;

#[derive(Clone, Debug, Default)]
pub struct Pose {
    tick: u64,
    at: usize,
    players: Vec<(u32, f32, f32)>,
    enemies: Vec<(u32, f32, f32)>,
    squires: Vec<(u32, u32, f32, f32)>,
    cameras: Vec<(u32, Option<(f32, f32)>)>,
    outside: BTreeMap<u32, usize>,
}

impl Run {
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
            squires: self.lane_squires.iter().map(|s| (s.owner, s.recruit.id, s.x, s.y)).collect(),
        }
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn wire_copy(&self) -> Run {
        crate::wire::read(&crate::wire::publish(self).expect("a run makes a tree")).expect("a tree reads back")
    }

    pub fn drawn_with<T>(
        &mut self,
        before: &Pose,
        alpha: f32,
        own: Option<(u32, &Hero)>,
        f: impl FnOnce(&Run) -> T,
    ) -> T {
        self.drawn_with_shots(before, alpha, own, None, f)
    }

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
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        self.shoulders.seat_riders(&grid, &mut self.players);
        let mut mine: Vec<(usize, Projectile)> = Vec::new();
        let theirs = self.projectiles.len();
        if let (Some((id, _)), Some(drawn)) = (own, shots) {
            let mut seen = 0;
            let mut places = Vec::new();
            let taken: Vec<Projectile> = self
                .projectiles
                .extract_if(.., |p| {
                    seen += 1;
                    let own = p.loosed_by(id);
                    if own {
                        places.push(seen - 1);
                    }
                    own
                })
                .collect();
            mine.extend(places.into_iter().zip(taken));
            self.projectiles.extend(drawn.iter().cloned());
        }
        let out = f(self);
        if shots.is_some() && own.is_some() {
            self.projectiles.truncate(theirs - mine.len());
            if !mine.is_empty() {
                let mut rest = std::mem::take(&mut self.projectiles).into_iter();
                let mut mine = mine.into_iter().peekable();
                self.projectiles.reserve(theirs);
                while self.projectiles.len() < theirs {
                    let next = match mine.next_if(|(at, _)| *at == self.projectiles.len()) {
                        Some((_, shot)) => Some(shot),
                        None => rest.next(),
                    };
                    match next {
                        Some(shot) => self.projectiles.push(shot),
                        None => break,
                    }
                }
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
        for &(owner, id, x, y) in &now.squires {
            if let Some(squire) = self.lane_squires.iter_mut().find(|s| s.owner == owner && s.recruit.id == id) {
                (squire.x, squire.y) = (x, y);
            }
        }
        for (shot, &(x, y)) in self.projectiles.iter_mut().zip(shots) {
            (shot.x, shot.y) = (x, y);
        }
    }

    pub fn predictable(&self, id: u32) -> bool {
        let at = self.exits.outside.get(&id).map_or(self.at, |e| e.at);
        self.active()
            && self.crossroads.is_none()
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
            && !self.shoulders.carried(id)
    }

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
        for cd in &mut hero.spell_cooldowns { *cd = cd.saturating_sub(1); }
        let ((dx, dy, speed), _) = hero.own_step(walk, &grid, tune.roll_speed, |h| {
            fx::ground_pace(h, webs, song, roads.as_deref(), tune)
        });
        let speed = speed * self.shoulders.pace(id) * self.hands.pace(id);
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
        let flying = volley_in_flight(&self.projectiles, id);
        let Some(hero) = self.players.get_mut(&id).filter(|h| h.possessed.is_none()) else {
            return false;
        };
        let input = own_input(hero, input);
        heroes::focus_aim(hero, enemies);
        heroes::predict_stance(hero, &self.book, input);
        if !hero.hero_kit.charging { hero.hero_kit.charge = 0; }
        let was = hero.fire_cooldown;
        hero.fire_cooldown = hero.fire_cooldown.saturating_sub(1);
        let (ax, ay) = unit(input.aim_x as f32, input.aim_y as f32);
        let guarded = hero.dash_ticks > 0 || hero.shielding || hero.walling || hero.spin > 0 || hero.champ.busy() || hero.hero_kit.charging;
        if arcade || !(input.fire || ax != 0.0 || ay != 0.0) || hero.fire_cooldown != 0 || guarded {
            return true;
        }
        let target = enemies.iter().filter(|e| e.hp > 0).min_by(|a, b| {
            (a.x - hero.x).hypot(a.y - hero.y).total_cmp(&(b.x - hero.x).hypot(b.y - hero.y))
        });
        let melee = hero.forged.as_ref().and_then(|weapon| {
            weapon.melee.filter(|m| {
                (weapon.bolt.is_none() && weapon.throw.is_none())
                    || target.is_some_and(|e| (e.x - hero.x).hypot(e.y - hero.y) <= melee::slash_reach(m.reach) + e.radius())
            })
        });
        if hero.arm.as_deref() == Some("flask-bandolier") {
            hero.fire_cooldown = hero.cooldown(champions::FLASK_REARM);
        } else if hero.unarmed() {
            hero.fire_cooldown = 9;
            hero.swing = 5;
            hero.sword = false;
        } else if let Some(melee) = melee {
            if let Some(enemy) = target.filter(|e| (e.x - hero.x).hypot(e.y - hero.y) <= melee::slash_reach(melee.reach) + e.radius()) {
                (hero.aim_x, hero.aim_y) = unit(enemy.x - hero.x, enemy.y - hero.y);
            }
            let every = if hero.champ.fury > 0 { melee.every / 2 } else { melee.every };
            hero.fire_cooldown = hero.cooldown(every);
            hero.swing = 6;
            hero.sword = false;
        } else {
            let from = shots.len();
            let burning = burning && hero.forged.is_none() && hero.weapon == Weapon::Bow;
            if let Some(Loosed::Arm(_, (ax, ay))) = hero.loose(id, &self.book, burning, rng, flying, shots)
                && hero.weapon == Weapon::Handgonne
            {
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
        let now = hero.fire_cooldown;
        if tune.fire_rate != 100 && now != was && now > was.saturating_sub(1) {
            hero.fire_cooldown = tune.cooldown(now);
        }
        true
    }

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
        for &(owner, id, x, y) in &before.squires {
            if let Some(squire) = view.lane_squires.iter_mut().find(|s| s.owner == owner && s.recruit.id == id) {
                (squire.x, squire.y) = (back(squire.x, x), back(squire.y, y));
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
    pub fn loosed_by(&self, id: u32) -> bool {
        !self.hostile && !self.traits.squire && u32::from(self.traits.owner) == id
    }
}

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

const BLEND_TICKS: u64 = 2;
const JUMP: f32 = 4.0;

#[cfg(test)]
#[path = "../../tests/rules/together_mirror__tests.rs"]
mod tests;
