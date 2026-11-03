//! The shots in flight on a tick (`Run::play`): beams and lobs, pattern
//! shots, thrown blades and rails, walls and shields, and what each shot
//! strikes.

use super::*;
use super::arms::Sortie;

/// What the shots struck on a tick, for what follows them
/// (`Run::aftermath`): chains, bursts and fart clouds to set off, mends to
/// give, the knights hexed and the leaps made, and the arms' synergies.
/// A chain hit to spark on: the monster struck, where, the damage each link
/// deals, how many links, the knight's id, and the forking shot that set it
/// off (its forks fly from the first link).
pub(super) type Chain = (u32, f32, f32, u32, u8, u8, Option<Box<Projectile>>);

/// The two shots a forking `shot` splits into at `at`, turned either way off
/// `heading`, flying on past monster `hit`.
pub(super) fn fork_children(shot: &Projectile, heading: (f32, f32), at: (f32, f32), hit: u32) -> [Projectile; 2] {
    [-FORK_TURN, FORK_TURN].map(|turn| {
        let (c, s) = (turn.cos(), turn.sin());
        let mut child = shot.clone();
        (child.vx, child.vy) = (heading.0 * c - heading.1 * s, heading.0 * s + heading.1 * c);
        (child.x, child.y) = at;
        child.ttl = shot.ttl.max(FORK_TTL);
        child.last_hit = Some(hit);
        child.traits.fork = shot.traits.fork.saturating_sub(1);
        child.traits.struck = 0;
        child.traits.forked = true;
        child
    })
}

impl Run {
    /// A knight's forks and splits join the air while that knight has fewer
    /// than `KNIGHT_CHILDREN` children aloft (their own volley does not
    /// count against it), and under `MAX_PROJECTILES`.
    pub(super) fn adopt_children(&mut self, children: impl IntoIterator<Item = Projectile>) {
        let mut flying = [0usize; 256];
        for p in self.projectiles.iter().filter(|p| !p.hostile && p.traits.forked) {
            flying[usize::from(p.traits.owner)] += 1;
        }
        let room = MAX_PROJECTILES.saturating_sub(self.projectiles.len());
        self.projectiles.extend(
            children
                .into_iter()
                .filter(|s| {
                    let owner = usize::from(s.traits.owner);
                    if s.hostile || owner == 0 {
                        return true;
                    }
                    flying[owner] += 1;
                    flying[owner] <= KNIGHT_CHILDREN
                })
                .take(room),
        );
    }
}

pub(super) struct Struck {
    pub(super) chains: Vec<Chain>,
    pub(super) bursts: Vec<(u32, f32, f32, u32, u8)>,
    pub(super) farts: Vec<(f32, f32, u8, u8)>,
    pub(super) mends: BTreeMap<u8, u32>,
    pub(super) hexed: Vec<u32>,
    pub(super) leaps: u32,
    pub(super) overkill: bool,
    pub(super) burns: Vec<(u32, u8, u8)>,
    pub(super) poisons: Vec<(u32, u8, u8)>,
    pub(super) turned: Vec<u8>,
    pub(super) kegs: Vec<(usize, u8)>,
}

/// Living foes binned by where they stand, so a shot tests only bodies its
/// path could reach. A shove rebuilds the bins before the next shot: within
/// one shot only the foe already visited moves, and the rest are tested
/// where they stood.
struct FoeBins {
    cell: f32,
    ox: f32,
    oy: f32,
    cols: i32,
    rows: i32,
    /// Widest living body this tick. A query grows by this plus the shot's
    /// own reach, so a body the segment could touch is never left out.
    max_r: f32,
    head: Vec<u32>,
    next: Vec<u32>,
    /// Non-finite positions, always tested. An empty grid means no living foe.
    extra: Vec<usize>,
}

const FOE_CELL: f32 = 4.0;
/// Ticks between two of one family's spell breaks: a ring's dozen bolts
/// through a crowd ring out as a few, not a dozen.
const SPELL_HIT_EVERY: u64 = 4;

thread_local! {
    static BIN_HEAD: std::cell::RefCell<Vec<u32>> = std::cell::RefCell::new(Vec::new());
    static BIN_NEXT: std::cell::RefCell<Vec<u32>> = std::cell::RefCell::new(Vec::new());
}

fn take_bins(cells: usize, foes: usize) -> (Vec<u32>, Vec<u32>) {
    let mut head = BIN_HEAD.with(|slot| std::mem::take(&mut *slot.borrow_mut()));
    let mut next = BIN_NEXT.with(|slot| std::mem::take(&mut *slot.borrow_mut()));
    head.clear();
    head.resize(cells, u32::MAX);
    next.clear();
    next.resize(foes, u32::MAX);
    (head, next)
}

fn give_bins(head: Vec<u32>, next: Vec<u32>) {
    BIN_HEAD.with(|slot| {
        let mut kept = slot.borrow_mut();
        if head.capacity() > kept.capacity() {
            *kept = head;
        }
    });
    BIN_NEXT.with(|slot| {
        let mut kept = slot.borrow_mut();
        if next.capacity() > kept.capacity() {
            *kept = next;
        }
    });
}

impl Drop for FoeBins {
    fn drop(&mut self) {
        give_bins(std::mem::take(&mut self.head), std::mem::take(&mut self.next));
    }
}

impl FoeBins {
    fn build(enemies: &[Enemy]) -> Self {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        let mut max_r = 0.0f32;
        let mut extra = Vec::new();
        let mut any = false;
        for (i, e) in enemies.iter().enumerate() {
            if e.hp == 0 || e.buried() {
                continue;
            }
            if !e.x.is_finite() || !e.y.is_finite() {
                extra.push(i);
                continue;
            }
            // A foe knocked absurdly far past the room breaks the span and
            // buys nothing: it rides in `extra`, tested by hand, like the
            // infinite ones. 2026-11-02.
            if e.x.abs() > 1.0e9 || e.y.abs() > 1.0e9 {
                extra.push(i);
                continue;
            }
            any = true;
            min_x = min_x.min(e.x);
            min_y = min_y.min(e.y);
            max_x = max_x.max(e.x);
            max_y = max_y.max(e.y);
            max_r = max_r.max(e.radius());
        }
        if !any {
            return Self {
                cell: FOE_CELL,
                ox: 0.0,
                oy: 0.0,
                cols: 0,
                rows: 0,
                max_r,
                head: Vec::new(),
                next: Vec::new(),
                extra,
            };
        }
        // A foe knocked far past the room can make the span too wide for i32.
        // That is not a floor: the caller falls back to testing everyone.
        let cols = cells_across(max_x, min_x);
        let rows = cells_across(max_y, min_y);
        let cols = cols.max(1);
        let rows = rows.max(1);
        // A map this wide is not a floor. Test everyone; do not allocate a
        // grid that cannot pay for itself.
        if cols > 512 || rows > 512 {
            extra.extend(enemies.iter().enumerate().filter_map(|(i, e)| {
                (e.hp > 0 && !e.buried() && e.x.is_finite() && e.y.is_finite()).then_some(i)
            }));
            return Self {
                cell: FOE_CELL,
                ox: 0.0,
                oy: 0.0,
                cols: 0,
                rows: 0,
                max_r,
                head: Vec::new(),
                next: Vec::new(),
                extra,
            };
        }
        let (mut head, mut next) = take_bins((cols * rows) as usize, enemies.len());
        for (i, e) in enemies.iter().enumerate() {
            if e.hp == 0 || e.buried() || !e.x.is_finite() || !e.y.is_finite() {
                continue;
            }
            let c = Self::cell_at(min_x, min_y, cols, rows, e.x, e.y);
            next[i] = head[c];
            head[c] = i as u32;
        }
        Self { cell: FOE_CELL, ox: min_x, oy: min_y, cols, rows, max_r, head, next, extra }
    }

    fn cell_at(ox: f32, oy: f32, cols: i32, rows: i32, x: f32, y: f32) -> usize {
        let cx = ((x - ox) / FOE_CELL).floor() as i32;
        let cy = ((y - oy) / FOE_CELL).floor() as i32;
        (cy.clamp(0, rows - 1) * cols + cx.clamp(0, cols - 1)) as usize
    }

    /// Foes whose cells meet the shot's segment grown by `pad`.
    fn gather(&self, x0: f32, y0: f32, x1: f32, y1: f32, pad: f32, order: &mut Vec<usize>) {
        order.clear();
        order.extend_from_slice(&self.extra);
        if self.cols == 0 {
            return;
        }
        let min_x = x0.min(x1) - pad;
        let max_x = x0.max(x1) + pad;
        let min_y = y0.min(y1) - pad;
        let max_y = y0.max(y1) + pad;
        let c0 = ((min_x - self.ox) / self.cell).floor() as i32;
        let c1 = ((max_x - self.ox) / self.cell).floor() as i32;
        let r0 = ((min_y - self.oy) / self.cell).floor() as i32;
        let r1 = ((max_y - self.oy) / self.cell).floor() as i32;
        if c1 < 0 || r1 < 0 || c0 >= self.cols || r0 >= self.rows {
            return;
        }
        let c0 = c0.clamp(0, self.cols - 1);
        let c1 = c1.clamp(0, self.cols - 1);
        let r0 = r0.clamp(0, self.rows - 1);
        let r1 = r1.clamp(0, self.rows - 1);
        for row in r0..=r1 {
            for col in c0..=c1 {
                let mut i = self.head[(row * self.cols + col) as usize];
                while i != u32::MAX {
                    order.push(i as usize);
                    i = self.next[i as usize];
                }
            }
        }
    }
}

impl Run {
    /// The shots in flight: this tick's join the air, then every shot
    /// moves, steers, and meets walls, shields, images, knights and
    /// monsters; what they struck is gathered for what follows.
    pub(super) fn shots_fly(&mut self, sortie: &mut Sortie) -> Struck {
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: sortie.barred };
        let side = sortie.side;
        let loosed = self.projectiles.len();
        let mut fresh = std::mem::take(&mut sortie.shots);
        // Fewer, faster monster bullets (`Tune::foe_shots`, `foe_shot_speed`):
        // of each tick's new hostile shots, an even share flies, picked by
        // a running count so a volley thins evenly and the sim stays the same
        // on every seat.
        if self.tune.foe_shots < 100 || self.tune.foe_shot_speed != 1.0 {
            let (keep, speed) = (u64::from(self.tune.foe_shots), self.tune.foe_shot_speed);
            let mut n = self.hostile_shots;
            fresh.retain_mut(|shot| {
                if !shot.hostile {
                    return true;
                }
                n = n.wrapping_add(1);
                if n * keep / 100 == (n - 1) * keep / 100 {
                    return false;
                }
                // Faster, not farther: the same reach in fewer ticks.
                shot.vx *= speed;
                shot.vy *= speed;
                shot.ttl = ((shot.ttl as f32 / speed).ceil() as u32).max(1);
                true
            });
        }
        let room = MAX_PROJECTILES.saturating_sub(self.projectiles.len());
        if fresh.len() > room {
            fresh.truncate(room);
        }
        let mut released = [false; 256];
        for shot in fresh.iter().filter(|p| p.in_flight() && p.traits.path.beam.is_none()) {
            let owner = usize::from(shot.traits.owner);
            if shot.hostile || !released[owner] {
                self.feel.ranged(shot, ranged::Mark::Release);
                released[owner] = true;
            }
        }
        self.projectiles.extend(fresh.drain(..));
        super::scratch::give_shots(fresh);
        self.hostile_shots += hostile_past(&self.projectiles, loosed);
        // Swept segments prevent fast shots tunnelling through small targets.
        let walls: Vec<((f32, f32), (f32, f32), u32)> = self
            .players
            .values()
            .filter(|h| h.walling && h.hp > 0 && !h.stone)
            .filter_map(|h| h.wall_span().map(|(a, b)| (a, b, h.wall_empower())))
            .collect();
        let mut chains: Vec<Chain> = Vec::new();
        let mut bursts: Vec<(u32, f32, f32, u32, u8)> = Vec::new();
        // Where Fart Bombs hit this tick: the spot, the knight, how strong.
        let mut farts: Vec<(f32, f32, u8, u8)> = Vec::new();
        // Each knight whose shot mends hit something this tick, and by how
        // much: a tick's hits mend once, however many they were.
        let mut mends: BTreeMap<u8, u32> = BTreeMap::new();
        let mut hexed: Vec<u32> = Vec::new();
        // Who Rimeleap can leap to: the knights standing.
        let standing: Vec<(u32, f32, f32)> = self
            .players
            .iter()
            .filter(|(_, h)| h.targetable())
            .map(|(&id, h)| (id, h.x, h.y))
            .collect();
        let mut leaps = 0u32;
        let hands: Vec<(u32, f32, f32)> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0)
            .map(|(&id, h)| (id, h.x, h.y))
            .collect();
        // What a borrowed shot turns after: the monsters (`bodies`).
        let mut foes_at = super::scratch::take_foes_at();
        foes_at.extend(self.enemies.iter().filter(|e| e.hp > 0 && !e.buried()).map(|e| (e.id, e.x, e.y)));
        let spheres = self.spheres.clone();
        let mut overkill = false;
        // What pattern shots burst into this tick.
        let mut pops: Vec<Projectile> = Vec::new();
        // The arms' feel this tick (`feel`): rails split and turned, foes
        // set burning, clouds a ball lit (the knights grazed are the
        // sortie's).
        let mut splits: Vec<Projectile> = Vec::new();
        let mut burns: Vec<(u32, u8, u8)> = Vec::new();
        let mut poisons: Vec<(u32, u8, u8)> = Vec::new();
        let mut turned: Vec<u8> = Vec::new();
        let mut kegs: Vec<(usize, u8)> = Vec::new();
        // The knights whose spell's shots broke on a monster this tick, and
        // as what (a bolt, a ball): each sounds its family's break once.
        let mut spell_hits: Vec<(u8, Shot)> = Vec::new();
        let shields: Vec<(f32, f32)> = self.works.flights.iter().map(|f| (f.x, f.y)).collect();
        let clouds: Vec<(f32, f32, f32)> = self
            .works
            .clouds
            .iter()
            .map(|c| (c.x, c.y, c.radius()))
            .collect();
        let tick = self.tick;
        let kingdom = self.room().kind == RoomKind::Kingdom;
        let holding = self.holding();
        let mut bins = FoeBins::build(&self.enemies);
        let mut order: Vec<usize> = Vec::new();
        let mut foes_moved = false;
        for bullet in &mut self.projectiles {
            // Time stands still in a Stillhour for the monsters' shots, and
            // in a room holding for its party.
            if bullet.hostile
                && (holding
                    || spheres
                        .iter()
                        .any(|s| (bullet.x - s.x).hypot(bullet.y - s.y) < s.r))
            {
                continue;
            }
            // A beam warns, then burns down its line; it never flies. A
            // borrowed one burns the monsters (`bodies`).
            if bullet.traits.path.beam.is_some() && bullet.traits.path.wait == 0 {
                if bullet.traits.borrowed {
                    let owner = u32::from(bullet.traits.owner);
                    for (i, dealt) in patterns::shine_foes(bullet, &mut self.enemies, &grid) {
                        let enemy = &mut self.enemies[i];
                        enemy.wits.foe = owner;
                        sortie.credits.push((owner, dealt));
                        self.feel.impact(enemy.x, enemy.y, bullet.kind, (0.0, 0.0));
                        self.sounds.push("hit");
                    }
                } else {
                    patterns::shine_guarded(bullet, &mut self.players, &grid, &mut self.sounds, &mut self.feel, &mut self.minions.raised);
                    // A beam has already been clipped to stone and the
                    // party's raised shields. Do not give six companions
                    // six bits in the knights' shared beam-struck mask.
                    if bullet.hostile
                        && let Some(beam) = bullet.traits.path.beam
                        && beam.burning(bullet.traits.path.age)
                    {
                        for s in self.squire_field.companions.values_mut().filter(|s| s.targetable()) {
                            if beam.crosses((bullet.x, bullet.y), (s.actor.x, s.actor.y)) {
                                s.actor.hurt(bullet.damage * patterns::BEAM_SHARE / 100);
                            }
                        }
                    }
                }
                continue;
            }
            // A lob flies over everything and comes down on its mark; a
            // borrowed one on the monsters there.
            if bullet.traits.path.form == patterns::Form::Lob && !bullet.traits.path.waiting() {
                if bullet.traits.borrowed {
                    if let Some(struck) = patterns::lob_foes(bullet, &mut self.enemies) {
                        self.feel.impact(bullet.x, bullet.y, bullet.kind, (0.0, 0.0));
                        let owner = u32::from(bullet.traits.owner);
                        for (i, dealt) in struck {
                            self.enemies[i].wits.foe = owner;
                            sortie.credits.push((owner, dealt));
                            self.sounds.push("hit");
                        }
                    }
                } else if patterns::lob(bullet, &mut self.players, &mut self.sounds) {
                    // A lob cannot be caught mid-air, even by a shield.
                    for s in self.squire_field.companions.values_mut().filter(|s| s.targetable()) {
                        if (s.actor.x - bullet.x).hypot(s.actor.y - bullet.y) <= HERO_RADIUS + patterns::LOB_SPLASH {
                            s.actor.hurt(bullet.damage);
                        }
                    }
                    // It bursts where it came down.
                    self.feel.impact(bullet.x, bullet.y, bullet.kind, (0.0, 0.0));
                }
                continue;
            }
            let old_x = bullet.x;
            let old_y = bullet.y;
            // How near stone and props its body comes (`patterns::body`).
            let body = patterns::body(bullet);
            // A pattern shot waits, or changes speed, turns, weaves, stops
            // or bursts, before it moves.
            let marks = if bullet.traits.borrowed { &foes_at } else { &standing };
            let waiting = bullet.traits.path.waiting();
            if patterns::steer(bullet, marks, &mut pops) == patterns::Steer::Hold {
                if bullet.ttl == 0 {
                    self.feel.boss_impact(bullet);
                }
                continue;
            }
            if waiting && bullet.in_flight() { self.feel.ranged(bullet, ranged::Mark::Release); }
            // A thrown blade: out for its range, then home to the hand.
            match bullet.traits.back {
                1 => {
                    bullet.traits.out = bullet.traits.out.saturating_sub(1);
                    if bullet.traits.out == 0 {
                        bullet.traits.back = 2;
                        bullet.last_hit = None;
                    }
                }
                2 => match hands.iter().find(|h| h.0 == u32::from(bullet.traits.owner)) {
                    Some(&(_, hx, hy)) => {
                        let (dx, dy) = (hx - bullet.x, hy - bullet.y);
                        let d = dx.hypot(dy);
                        if d < 1.2 {
                            bullet.ttl = 0;
                            continue;
                        }
                        let speed = wishcraft::home_speed(bullet, BLADE_SPEED);
                        (bullet.vx, bullet.vy) = (dx / d * speed, dy / d * speed);
                    }
                    None => bullet.ttl = 0,
                },
                _ => {}
            }
            // A homing shot turns toward the nearest monster, a little a tick.
            if !bullet.hostile
                && bullet.traits.homing > 0
                && let Some(target) = self.enemies.iter().filter(|e| e.hp > 0).min_by(|a, b| {
                    (a.x - bullet.x)
                        .hypot(a.y - bullet.y)
                        .total_cmp(&(b.x - bullet.x).hypot(b.y - bullet.y))
                })
            {
                let speed = bullet.vx.hypot(bullet.vy);
                let now = bullet.vy.atan2(bullet.vx);
                let want = (target.y - bullet.y).atan2(target.x - bullet.x);
                let turn = (want - now).sin().atan2((want - now).cos());
                let most = HOMING_TURN * f32::from(bullet.traits.homing) * DT;
                let next = now + turn.clamp(-most, most);
                (bullet.vx, bullet.vy) = (next.cos() * speed, next.sin() * speed);
            }
            bullet.x += bullet.vx * DT;
            bullet.y += bullet.vy * DT;
            let stone = if !bullet.hostile && bullet.traits.back != 2 {
                grid.shot_stop_body((old_x, old_y), (bullet.x, bullet.y), body)
            } else {
                None
            };
            if let Some((x, y)) = stone {
                (bullet.x, bullet.y) = (x, y);
            }
            bullet.ttl = bullet.ttl.saturating_sub(1);
            let rail = !bullet.hostile && bullet.kind == Shot::Bolt && bullet.traits.arm;
            if rail {
                self.feel.tracer(
                    (old_x, old_y),
                    (bullet.x, bullet.y),
                    bullet.traits.temper >= 1,
                );
                // Ricochet: a rail meeting a flying shield turns off it to
                // the nearest monster it has not struck.
                if !bullet.traits.once
                    && let Some(&(sx, sy)) = shields.iter().find(|&&(sx, sy)| {
                        hit_segment(old_x, old_y, bullet.x, bullet.y, sx, sy, 0.9)
                    })
                    && let Some(foe) = self
                        .enemies
                        .iter()
                        .filter(|e| e.hp > 0 && !bullet.traits.pierced.contains(&e.id))
                        .min_by(|a, b| {
                            (a.x - sx)
                                .hypot(a.y - sy)
                                .total_cmp(&(b.x - sx).hypot(b.y - sy))
                        })
                {
                    let speed = bullet.vx.hypot(bullet.vy);
                    let (ux, uy) = unit(foe.x - sx, foe.y - sy);
                    (bullet.x, bullet.y) = (sx, sy);
                    (bullet.vx, bullet.vy) = (ux * speed, uy * speed);
                    bullet.traits.once = true;
                    bullet.ttl = bullet.ttl.max(HZ / 2);
                    turned.push(bullet.traits.owner);
                }
            }
            // Fire and embers flash gas too, including the Ripper's walls.
            let fire = matches!(bullet.kind, Shot::Ember | Shot::Flame) || bullet.traits.fire;
            if fire {
                for (i, &(cx, cy, r)) in clouds.iter().enumerate() {
                    if hit_segment(old_x, old_y, bullet.x, bullet.y, cx, cy, r)
                        && !kegs.iter().any(|k| k.0 == i)
                    {
                        kegs.push((i, bullet.traits.owner));
                    }
                }
            }
            let powder = !bullet.hostile && bullet.kind == Shot::Ball && bullet.traits.arm;
            if !bullet.hostile && !fire && !powder {
                if let Some(c) = self.works.clouds.iter().find(|c| {
                    c.wall() && hit_segment(old_x, old_y, bullet.x, bullet.y, c.x, c.y, c.radius())
                }) {
                    if c.age < HZ / 2 {
                        bullet.ttl = 0;
                        continue;
                    }
                    bullet.x = old_x + (bullet.x - old_x) * 0.55;
                    bullet.y = old_y + (bullet.y - old_y) * 0.55;
                }
            }
            // Powder Keg: a handgonne ball through a cloud of gas lights it.
            if !bullet.hostile && bullet.kind == Shot::Ball && bullet.traits.arm {
                for (i, &(cx, cy, r)) in clouds.iter().enumerate() {
                    let inside = hit_segment(old_x, old_y, bullet.x, bullet.y, cx, cy, r);
                    if inside && !kegs.iter().any(|k| k.0 == i) {
                        kegs.push((i, bullet.traits.owner));
                    }
                }
            }
            // A wall stops what monsters throw and lets friends' shots
            // through, stronger.
            if let Some(&(_, _, empower)) = walls
                .iter()
                .find(|&&(a, b, _)| crosses((old_x, old_y), (bullet.x, bullet.y), a, b))
            {
                if bullet.hostile {
                    bullet.ttl = 0;
                    self.sounds.push("shield_block");
                    continue;
                } else if !bullet.empowered {
                    bullet.empowered = true;
                    bullet.damage = bullet.damage * (100 + empower) / 100;
                }
            }
            if bullet.hostile && self.scarecrows.iter().any(|d| d.active(self.tick)
                && hit_segment(old_x, old_y, bullet.x, bullet.y, d.x, d.y, peasant::DECOY_RADIUS))
            {
                bullet.ttl = 0;
                continue;
            }
            if bullet.hostile
                && let Some(phantom) = self.phantoms.iter_mut().find(|p| {
                    p.left > 0 && hit_segment(old_x, old_y, bullet.x, bullet.y, p.x, p.y, 0.6)
                })
            {
                // An image takes the shot meant for its knight, and is gone.
                phantom.left = 0;
                bullet.ttl = 0;
                continue;
            }
            // A friend's shot at a knight in the stocks is a tomato.
            if !bullet.hostile
                && kingdom
                && let Some(hero) = self.players.iter_mut().find_map(|(&id, h)| {
                    (h.stocked
                        && id != u32::from(bullet.traits.owner)
                        && hit_segment(old_x, old_y, bullet.x, bullet.y, h.x, h.y, HERO_RADIUS + 0.16))
                    .then_some(h)
                })
            {
                hero.splats = hero.splats.saturating_add(1).min(99);
                bullet.ttl = 0;
                self.sounds.push("hit");
                self.score = self.score.saturating_add(1);
                continue;
            }
            if bullet.hostile {
                // The knight it meets first along its step: at a raised
                // shield's face, or at the body (`shield`). A Peasant
                // deserting is no one's mark (`peasant`).
                let (from, to) = ((old_x, old_y), (bullet.x, bullet.y));
                let mut first: Option<(u32, shield::Meet)> = None;
                for (&id, hero) in self.players.iter().filter(|(_, h)| h.targetable()) {
                    // Rimeleap never strikes the knight it just left.
                    if bullet.kind == Shot::Frost && bullet.last_hit == Some(id) {
                        continue;
                    }
                    let crouch = if self.works.kits.get(&id).is_some_and(|k| k.duck) {
                        skills::DUCK_DROP
                    } else {
                        0.0
                    };
                    // A dart strikes close, a heavy orb wide (`patterns::girth`).
                    let centre = (hero.x, hero.y + crouch);
                    if let Some(meet) = shield::meets(hero, centre, from, to, HERO_RADIUS + patterns::girth(bullet))
                        && first.is_none_or(|(_, f)| meet.t < f.t)
                    {
                        first = Some((id, meet));
                    }
                }
                let guard = self.minions.raised.iter().enumerate()
                    .filter(|(_, m)| m.kind == hireling::AllyKind::Skeleton && !m.down())
                    .filter_map(|(i, m)| shield::entry(from, to, (m.x, m.y), minions::WALL_RADIUS + patterns::girth(bullet)).map(|t| (i, t)))
                    .filter(|&(_, t)| first.is_none_or(|(_, meet)| t <= meet.t))
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                // Squires and skeleton shields compete with the nearest hero
                // on this same swept path, not on aftermath's endpoint overlap.
                let companion = self.players.iter()
                    .filter(|(id, h)| !h.absent && !h.away && !h.stone && !self.exits.outside.contains_key(id))
                    .filter_map(|(&id, h)| {
                        let s = h.squire.as_ref().filter(|s| !s.down() && s.vanished == 0
                            && s.owner == super::squires_lodge::owner_key(h))?;
                        shield::entry(from, to, (s.body.x, s.body.y), s.shot_radius() + patterns::girth(bullet))
                            .filter(|&t| first.is_none_or(|(_, meet)| t <= meet.t))
                            .map(|t| (id, t))
                    })
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                // Squires have bodies too. Resolve swept entry before
                // the knight behind them, not after shots have already hit
                // that knight. A shield-bearer pays half from its own health.
                let squire = self.squire_field.companions.iter()
                    .filter(|(_, s)| s.targetable())
                    .filter_map(|(&id, s)| {
                        let radius = if s.squire.kind == squire_company::Kind::Shield { 0.85 } else { HERO_RADIUS };
                        shield::entry(from, to, (s.actor.x, s.actor.y), radius + patterns::girth(bullet)).map(|t| (id, t))
                    })
                    .filter(|&(_, t)| first.is_none_or(|(_, meet)| t <= meet.t)
                        && guard.is_none_or(|(_, gt)| t <= gt) && companion.is_none_or(|(_, ct)| t <= ct))
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                // Companions meet a shot along its whole step, not at its
                // final point in aftermath. Compare both kinds of guards and
                // the knight, so a body behind the knight cannot save them.
                let players = &self.players;
                let outside = &self.exits.outside;
                let mut companions: Vec<_> = self.squire_hall.companions.iter().enumerate()
                    .filter(|(_, c)| players.get(&c.owner).is_some_and(|h| !h.absent) && !outside.contains_key(&c.owner) && !c.body.down() && c.smoke == 0)
                    .filter_map(|(i, c)| {
                        let radius = if c.squire.kind == crate::realm::squires::Kind::Shield { 1.3 } else { 0.7 };
                        shield::entry(from, to, (c.body.x, c.body.y), radius + patterns::girth(bullet)).map(|t| (i, t))
                    })
                    .filter(|&(_, t)| first.is_none_or(|(_, meet)| t <= meet.t))
                    .filter(|&(_, t)| guard.is_none_or(|(_, g)| t < g) && companion.is_none_or(|(_, ct)| t < ct))
                    .collect();
                companions.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
                for (i, t) in companions {
                    // The combined company's first body stops the shot. A
                    // hall companion behind it cannot be hit by that shot.
                    if squire.is_some_and(|(_, company_t)| company_t <= t) { break; }
                    let c = &mut self.squire_hall.companions[i];
                    let shield = c.squire.kind == crate::realm::squires::Kind::Shield;
                    c.body.hurt(if shield { bullet.damage.div_ceil(2) } else { bullet.damage });
                    self.sounds.push(if shield { "shield_block" } else { "hit" });
                    if shield {
                        bullet.ttl = 0;
                        break;
                    }
                }
                if bullet.ttl == 0 { continue; }
                if let Some((id, _)) = squire {
                    let s = self.squire_field.companions.get_mut(&id).unwrap();
                    if s.squire.kind == squire_company::Kind::Shield {
                        s.actor.hp = s.actor.hp.saturating_sub(bullet.damage.div_ceil(2));
                        self.sounds.push("shield_block");
                    } else if bullet.kind != Shot::Hex {
                        s.actor.hurt(bullet.damage);
                    }
                    bullet.ttl = 0;
                    continue;
                }
                if let Some((i, _)) = guard.filter(|&(_, t)| companion.is_none_or(|(_, c)| t <= c)) {
                    let skeleton = &mut self.minions.raised[i];
                    skeleton.hp = skeleton.hp.saturating_sub(bullet.damage.div_ceil(2));
                    bullet.ttl = 0;
                    self.sounds.push("shield_block");
                    continue;
                }
                // A company shield catches the swept shot before its knight,
                // not afterwards in the ally AI (fast arrows must not tunnel).
                let squire_guard = self.lane_squires.iter().enumerate()
                    .filter(|(_, s)| s.recruit.kind == lane_squires::Kind::Shield && s.hp > 0)
                    .filter_map(|(i, s)| shield::entry(from, to, (s.x, s.y), 0.85 + patterns::girth(bullet)).map(|t| (i, t)))
                    .filter(|&(_, t)| first.is_none_or(|(_, meet)| t <= meet.t))
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((i, _)) = squire_guard.filter(|&(_, t)| companion.is_none_or(|(_, c)| t <= c)) {
                    let squire = &mut self.lane_squires[i];
                    if squire.grace == 0 {
                        let damage = if squire.recruit.level >= 2 { bullet.damage.div_ceil(2) } else { bullet.damage };
                        squire.hp = squire.hp.saturating_sub(damage);
                        squire.grace = HZ / 2;
                    }
                    bullet.ttl = 0;
                    self.sounds.push("shield_block");
                    continue;
                }
                if let Some((id, t)) = companion {
                    self.players.get_mut(&id).unwrap().squire.as_mut().unwrap().take_shot(bullet.damage);
                    bullet.x = from.0 + (to.0 - from.0) * t;
                    bullet.y = from.1 + (to.1 - from.1) * t;
                    bullet.ttl = 0;
                    self.sounds.push("shield_block");
                    continue;
                }
                // A graze: close, and past, once a shot (`feel`).
                for (&id, hero) in self.players.iter().filter(|(_, h)| h.targetable()) {
                    if bullet.traits.once || first.is_some_and(|(met, _)| met == id) {
                        continue;
                    }
                    if bullet.kind == Shot::Frost && bullet.last_hit == Some(id) {
                        continue;
                    }
                    let crouch = if self.works.kits.get(&id).is_some_and(|k| k.duck) {
                        skills::DUCK_DROP
                    } else {
                        0.0
                    };
                    let reach = HERO_RADIUS + patterns::girth(bullet);
                    let (x, y) = (hero.x, hero.y + crouch);
                    if hero.invulnerable == 0
                        && !hit_segment(old_x, old_y, bullet.x, bullet.y, x, y, reach)
                        && hit_segment(old_x, old_y, bullet.x, bullet.y, x, y, reach + feel::GRAZE)
                    {
                        bullet.traits.once = true;
                        sortie.grazes.push(id);
                    }
                }
                if let Some((id, meet)) = first {
                    bullet.ttl = 0;
                    if meet.shield {
                        // Stopped at the shield's face, never in the knight.
                        (bullet.x, bullet.y) = meet.at;
                    }
                    // A boss's shot bursts in its own bank where it lands.
                    self.feel.boss_impact(bullet);
                    if meet.shield {
                        let way = unit(bullet.vx, bullet.vy);
                        let out = self
                            .players
                            .get(&id)
                            .map_or((-way.0, -way.1), |h| (meet.at.0 - h.x, meet.at.1 - h.y));
                        let weight = shield::weight(bullet);
                        self.sounds.push("shield_block");
                        self.feel.block(id, meet.at, out, way, bullet.kind, weight);
                        if let Some(hero) = self.players.get_mut(&id)
                            && hero.shield_turned(bullet.damage)
                        {
                            self.sounds.push("clang");
                        }
                    } else if let Some(hero) = self.players.get_mut(&id) {
                        // An evaded projectile cannot chill, hex or chain.
                        if hero.hero_kit.windrun > 0 { continue; }
                        if hero.invulnerable == 0 && bullet.kind == Shot::Hex {
                            hexed.push(id);
                        } else if bullet.kind == Shot::Frost {
                            if hero.invulnerable == 0 {
                                hero.attack_hurt(bullet.damage);
                                self.sounds.push("hero_hurt");
                            }
                            if hero.immune == 0 {
                                hero.chilled = lich::CHILL;
                            }
                            // Rimeleap leaps on to the nearest other knight.
                            if bullet.pierce > 0
                                && let Some((_, nx, ny)) =
                                    lich::next_link(&standing, id, (hero.x, hero.y))
                            {
                                let speed = bullet.vx.hypot(bullet.vy).max(1.0);
                                let (ux, uy) = unit(nx - hero.x, ny - hero.y);
                                (bullet.x, bullet.y) = (hero.x, hero.y);
                                (bullet.vx, bullet.vy) = (ux * speed, uy * speed);
                                bullet.pierce -= 1;
                                bullet.last_hit = Some(id);
                                bullet.ttl = 4 * HZ;
                                leaps += 1;
                            }
                        } else if bullet.traits.blow != 0 {
                            if hero.blow(bullet.damage, bullet.traits.blow) {
                                self.sounds.push("hero_hurt");
                            }
                        } else if hero.invulnerable == 0 {
                            hero.attack_hurt(bullet.damage);
                            self.sounds.push("hero_hurt");
                        }
                    }
                }
            } else {
                // A fast rail may cross several bodies in one tick. Visit
                // them in flight order and keep every foe it has touched.
                // Only bodies the segment could reach are tested; the rest
                // cannot meet `hit_segment`, so the first strike is the same.
                if foes_moved {
                    bins = FoeBins::build(&self.enemies);
                    foes_moved = false;
                }
                let thick_pad = if rail && bullet.traits.temper >= 1 { 0.35 } else { 0.0 };
                let pad = bins.max_r + thick_pad + bullet.reach() + 1.0e-3;
                bins.gather(old_x, old_y, bullet.x, bullet.y, pad, &mut order);
                if rail {
                    order.sort_by(|&a, &b| {
                        let along =
                            |e: &Enemy| (e.x - old_x) * bullet.vx + (e.y - old_y) * bullet.vy;
                        along(&self.enemies[a]).total_cmp(&along(&self.enemies[b]))
                    });
                } else {
                    order.sort_unstable();
                }
                // How wide it strikes, worked out once, not for each monster.
                let reach = bullet.reach();
                for &i in &order {
                    let enemy = &mut self.enemies[i];
                    if enemy.hp == 0 || enemy.buried() {
                        continue;
                    }
                    // A tempered rail is thicker.
                    let thick = if rail && bullet.traits.temper >= 1 {
                        0.35
                    } else {
                        0.0
                    };
                    if bullet.last_hit != Some(enemy.id)
                        && (!rail || !bullet.traits.pierced.contains(&enemy.id))
                        && hit_segment(
                            old_x,
                            old_y,
                            bullet.x,
                            bullet.y,
                            enemy.x,
                            enemy.y,
                            enemy.radius() + thick + reach,
                        )
                    {
                        if critters::blocks(enemy, bullet.vx, bullet.vy) {
                            // A tower shield turns it, but each shot it
                            // turns tells: enough and it is knocked aside.
                            bullet.ttl = 0;
                            if critters::guard_struck(enemy, bullet.damage) {
                                self.sounds.push("clang");
                            } else {
                                self.sounds.push("shield_block");
                            }
                            break;
                        }
                        // A shot that passes through counts the monsters it
                        // meets, as one stroke (`aoe`).
                        let dealt = aoe::part(bullet.damage, usize::from(bullet.traits.struck));
                        let dealt = dealt
                            + signatures::rear_bonus(enemy, &self.bosses, (old_x, old_y), dealt);
                        // A curio that likes one kind of monster and not the rest
                        // (`curios`).
                        let dealt = match self.players.get(&u32::from(bullet.traits.owner)) {
                            Some(owner) if !owner.pockets.items.is_empty() && dealt > 0 => {
                                (dealt * owner.curio_versus(enemy.kind) / 100).max(1)
                            }
                            _ => dealt,
                        };
                        let dealt = if self.squire_mark.as_ref().is_some_and(|m| m.foe == enemy.id && m.ticks > 0) {
                            dealt.saturating_add((dealt / 4).max(1))
                        } else { dealt };
                        let first_hit = bullet.traits.struck == 0;
                        bullet.traits.struck = bullet.traits.struck.saturating_add(1);
                        if rail {
                            bullet.traits.pierced.push(enemy.id);
                        }
                        let actual = dealt.min(enemy.hp);
                        enemy.hp = enemy.hp.saturating_sub(dealt);
                        if actual > 0 && enemy.is_boss()
                            && (bullet.empowered || matches!(bullet.kind, Shot::Sacred | Shot::Missile))
                        {
                            self.hitstop.request(&self.tune, hitstop::Impact::Heavy);
                        }
                        bullet.last_hit = Some(enemy.id);
                        if bullet.traits.owner > 0 {
                            let owner = u32::from(bullet.traits.owner);
                            sortie.credits.push((owner, actual));
                            // It turns on whoever hurt it (`press`).
                            enemy.wits.foe = owner;
                            self.floor_tally.line(owner).hits +=
                                u32::from(first_hit && (bullet.traits.arm || bullet.look.is_some()));
                            if feel::ledger_on() {
                                self.feel.wounded.entry(enemy.id).or_insert(tick);
                                let tally = self.feel.ledger.entry(owner).or_default();
                                tally.hits += u32::from(
                                    first_hit && (bullet.traits.arm || bullet.look.is_some()),
                                );
                            }
                        }
                        // The hit lands: sparks, a jolt, a shove (`feel`); a
                        // Bard's note bursts into notes.
                        let (ux, uy) = unit(bullet.vx, bullet.vy);
                        let note = !bullet.hostile && bullet.look.as_ref().is_some_and(|(_, shape)| shape == "note");
                        self.feel.impact_as(
                            enemy.x - ux * enemy.radius(),
                            enemy.y - uy * enemy.radius(),
                            bullet.kind,
                            (ux, uy),
                            note,
                        );
                        if actual > 0 {
                            self.feel.ranged(bullet, ranged::Mark::Flesh);
                            if let Some(hit) = self.feel.ranged.last_mut() {
                                hit.x = enemy.x - ux * enemy.radius();
                                hit.y = enemy.y - uy * enemy.radius();
                            }
                            if !bullet.hostile
                                && bullet.traits.owner > 0
                                && matches!(bullet.kind, Shot::Bolt | Shot::Ball)
                                && !spell_hits.contains(&(bullet.traits.owner, bullet.kind))
                                && spell_hits.len() < 8
                            {
                                spell_hits.push((bullet.traits.owner, bullet.kind));
                            }
                        }
                        let anchored = enemy.asleep(self.tick) || enemy.is_boss()
                            || matches!(enemy.kind, EnemyKind::Dummy | EnemyKind::Ward);
                        if enemy.hp > 0 && !anchored {
                            if enemy.wits.flinch == 0 {
                                enemy.frozen = enemy.frozen.max(feel::JOLT_HIT);
                                enemy.wits.flinch = feel::FLINCH;
                            }
                            // A Peasant's stone shoves its own way; else a
                            // wish's knock (`wishcraft`).
                            let push = if peasant::stone(bullet) { peasant::STONE_KNOCK } else { wishcraft::knock(bullet) };
                            if push > 0.0 {
                                let (ux, uy) = unit(bullet.vx, bullet.vy);
                                let mover = if enemy.kind.flies() {
                                    Mover::Flier
                                } else {
                                    Mover::Walker
                                };
                                let shove = (ux * push, if side { 0.0 } else { uy * push });
                                (enemy.x, enemy.y) =
                                    grid.slide((enemy.x, enemy.y), shove, enemy.radius(), mover);
                                foes_moved = true;
                            }
                        }
                        if enemy.hp == 0
                            && matches!(bullet.kind, Shot::Bolt | Shot::Ball | Shot::Sacred)
                        {
                            self.fx.push(Fx::Shake(feel::SHAKE_KILL));
                        }
                        // A knight's shot killed it: the finisher's blow (`gore`).
                        if enemy.hp == 0 && actual > 0 && !bullet.hostile && bullet.traits.owner > 0 {
                            let burst = (bullet.kind == Shot::Ball && (!bullet.traits.arm || bullet.traits.temper >= 3))
                                || bullet.traits.burst > 0;
                            self.feel.blow(
                                enemy.id,
                                u32::from(bullet.traits.owner),
                                gore::Blow::Shot { shot: bullet.kind, dir: (ux, uy), burst },
                            );
                        }
                        if bullet.traits.fire || bullet.traits.burn > 0 {
                            burns.push((enemy.id, bullet.traits.owner, bullet.traits.burn.max(1)));
                        }
                        wishcraft::chill(enemy, bullet);
                        if bullet.traits.poison > 0 {
                            poisons.push((enemy.id, bullet.traits.owner, bullet.traits.poison));
                        }
                        // A twice-tempered rail splits on its first kill.
                        if rail && enemy.hp == 0 && bullet.traits.temper >= 2 {
                            bullet.traits.temper = 1;
                            for turn in [-0.45f32, 0.45] {
                                let (c, s) = (turn.cos(), turn.sin());
                                let mut child = bullet.clone();
                                (child.vx, child.vy) =
                                    (bullet.vx * c - bullet.vy * s, bullet.vx * s + bullet.vy * c);
                                (child.x, child.y) = (enemy.x, enemy.y);
                                child.damage = bullet.damage / 2;
                                child.ttl = HZ / 2;
                                child.traits.once = true;
                                child.traits.forked = true;
                                splits.push(child);
                            }
                        }
                        if bullet.damage >= 300 {
                            overkill = true;
                        }
                        if bullet.traits.stun > 0 {
                            enemy.frozen = enemy.frozen.max(u32::from(bullet.traits.stun));
                        }
                        self.sounds.push("hit");
                        // A burst and a spark follow what the hit dealt.
                        if bullet.kind == Shot::Ball
                            && (!bullet.traits.arm || bullet.traits.temper >= 3)
                        {
                            // A thrice-tempered handgonne's ball bursts: half
                            // to all around.
                            bursts.push((
                                enemy.id,
                                enemy.x,
                                enemy.y,
                                dealt / 2,
                                bullet.traits.owner,
                            ));
                        } else if bullet.traits.burst > 0 {
                            let part = dealt * u32::from(bullet.traits.burst) / 100;
                            bursts.push((
                                enemy.id,
                                enemy.x,
                                enemy.y,
                                part.max(1),
                                bullet.traits.owner,
                            ));
                        }
                        if bullet.traits.chain > 0 {
                            chains.push((
                                enemy.id,
                                enemy.x,
                                enemy.y,
                                dealt / 2,
                                bullet.traits.chain,
                                bullet.traits.owner,
                                (bullet.traits.fork > 0 && bullet.traits.back == 0 && !bullet.hostile)
                                    .then(|| Box::new(bullet.clone())),
                            ));
                        }
                        if bullet.traits.mend > 0 {
                            let mend = mends.entry(bullet.traits.owner).or_default();
                            *mend = (*mend).max(u32::from(bullet.traits.mend));
                        }
                        if bullet.traits.fart > 0 && bullet.traits.owner > 0 {
                            farts.push((enemy.x, enemy.y, bullet.traits.owner, bullet.traits.fart));
                        }
                        if bullet.pierce == 0 && wishcraft::turn_home(bullet) {
                            // A boomerang turns home off what it struck.
                            break;
                        } else if bullet.pierce == 0 {
                            // Fork: where it stops, two fly on at an angle,
                            // past the one it struck, as new shots.
                            if bullet.traits.fork > 0 && bullet.traits.back == 0 {
                                splits.extend(fork_children(bullet, (bullet.vx, bullet.vy), (enemy.x, enemy.y), enemy.id));
                            }
                            bullet.ttl = 0;
                        } else {
                            bullet.pierce -= 1;
                        }
                        if !rail || bullet.ttl == 0 {
                            break;
                        }
                    }
                }
            }
            if stone.is_some()
                || (bullet.traits.back != 2 && grid.stops_body(bullet.x, bullet.y, body))
                || grid.stops_shot(bullet.x, bullet.y)
            {
                if bullet.traits.back == 1 {
                    // A thrown blade turns home off the stone; coming back
                    // it passes over everything.
                    (bullet.x, bullet.y) = (old_x, old_y);
                    bullet.traits.back = 2;
                    bullet.last_hit = None;
                } else if bullet.traits.back == 2 {
                } else if bullet.traits.bounce > 0 && bullet.ttl > 0 {
                    // Off the stone it came against: across, along, or both
                    // in a corner. It may hit the same monster again. A
                    // monster's bouncing pattern shot comes off the same way.
                    let across = grid.stops_body(bullet.x, old_y, body);
                    let along = grid.stops_body(old_x, bullet.y, body);
                    if across || !along {
                        bullet.vx = -bullet.vx;
                    }
                    if along || !across {
                        bullet.vy = -bullet.vy;
                    }
                    (bullet.x, bullet.y) = (old_x, old_y);
                    bullet.traits.bounce -= 1;
                    bullet.last_hit = None;
                } else {
                    self.feel.ranged(bullet, ranged::Mark::Wall);
                    // Arrows and bolts stick in the stone till the room ends.
                    if !bullet.hostile
                        && matches!(bullet.kind, Shot::Arrow | Shot::Bolt)
                        && bullet.look.is_none()
                    {
                        self.feel
                            .stick(bullet.x, bullet.y, (bullet.vx, bullet.vy), bullet.kind);
                    } else if bullet.hostile && bosses::Art::of_shot(bullet).is_some() {
                        // A library guardian's shot breaks in its own bank.
                        self.feel.boss_impact(bullet);
                    } else {
                        // A monster's shot breaks on stone with a spark too.
                        self.feel.impact(bullet.x, bullet.y, bullet.kind, (bullet.vx, bullet.vy));
                    }
                    bullet.ttl = 0;
                }
            }
        }
        // A spell's shot breaks in its family's way (`vfx::Voice`), a crowd
        // of them no faster than `SPELL_HIT_EVERY`.
        for (owner, kind) in spell_hits {
            let sound = self
                .players
                .get(&u32::from(owner))
                .and_then(|hero| hero.spell_lately(&self.book, kind))
                .map(vfx::impact_sound);
            if let Some(sound) = sound {
                self.sound_every(sound, SPELL_HIT_EVERY);
            }
        }
        for shot in self.projectiles.iter().filter(|p| p.ttl == 0) { self.feel.ranged(shot, ranged::Mark::Pop); }
        self.projectiles.retain(|p| p.ttl > 0);
        let room = MAX_PROJECTILES.saturating_sub(self.projectiles.len());
        let popped = self.projectiles.len();
        self.projectiles.extend(pops.into_iter().take(room));
        self.adopt_children(splits);
        self.hostile_shots += hostile_past(&self.projectiles, popped);
        // The party's shots keep to their share of the screen (`cover`).
        let view = cover::view(self.room());
        cover::hold(&mut self.projectiles, view);
        super::scratch::give_foes_at(foes_at);
        Struck {
            chains,
            bursts,
            farts,
            mends,
            hexed,
            leaps,
            overkill,
            burns,
            poisons,
            turned,
            kegs,
        }
    }
}

/// How many cells cover `min..=max`. A non-finite span, or one that does not
/// fit in `i32`, counts as too wide (`i32::MAX`); the caller then skips the grid.
fn cells_across(max: f32, min: f32) -> i32 {
    let span = (max - min) / FOE_CELL;
    if !span.is_finite() || span < 0.0 {
        return i32::MAX;
    }
    (span.floor() as i32).saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::FoeBins;

    #[test]
    fn knocked_far_foe_does_not_overflow_the_grid() {
        let mut run = Run::new(1, 1, None);
        let id = run
            .spawn_body(EnemyKind::Slime, 10.0, 10.0)
            .expect("a foe");
        let foe = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
        foe.x = 1.0e20;
        foe.y = -1.0e20;
        let bins = FoeBins::build(&run.enemies);
        // A span past the room is not a grid *to search*: the far foe rides
        // in `extra` (tested by hand, out of the bins), and with no foe left
        // to bin the grid itself is empty. 2026-11-02: keep the foe that
        // made the span absurd out of the bins altogether, as the infinite
        // one above.
        let far = bins.extra.contains(&(0usize)) || bins.cols == 0;
        assert!(far, "the far foe is not binned: cols={} extra={:?}", bins.cols, bins.extra);
        let near = run
            .spawn_body(EnemyKind::Slime, 12.0, 12.0)
            .expect("a second foe");
        assert_ne!(near, id);
        let bins = FoeBins::build(&run.enemies);
        // The near foe bins alone on a small grid; the far one still rides
        // in `extra`, never blowing the span up.
        assert!(bins.cols > 0 && bins.cols <= 512, "a small grid: {}", bins.cols);
        assert!(bins.extra.contains(&0), "the far foe stays out of the bins");
    }
}
