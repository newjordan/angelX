//! The Wanted hall: on every floor a side-on showdown with an outlaw of
//! the pack's own roster, in the manner of a Sunset Riders stage.
//!
//! The party walks into a wide hall of balconies and awnings, crates and
//! barrels, and the WANTED poster goes up: the outlaw's likeness, its name
//! and what is offered for it. Then it moves between its posts (the ground,
//! either balcony, the beam in the middle, the cover of a crate), takes
//! its shots on the beats its own file gives it (each preceded by a tell),
//! calls minions down on ropes, taunts when it has spent itself, and grows
//! desperate as any boss does. When it falls it falls hard, the bounty is
//! paid, and the room is clear. Knights run, jump twice and duck here.
//!
//! The hall hangs off the floor like the Pit does and is never on the way to
//! the stairs: it is the floor's showdown for those who go looking, with the
//! Boss Key's door and the route chiefs the floor's way down.
//!
//! Every number is a named constant, up top.

use super::bosses::Pattern;
use super::*;

/// The hall, row by row, from the west. `#` stone, `=` plank, `D` the one
/// doorway, `.` air: balconies at either end, awnings under them, a beam
/// over the middle, crates for cover and barrels to leap.
pub const STAGE: [&str; ROWS] = [
    "################################################",
    "#..............................................#",
    "#..............................................#",
    "#..............................................#",
    "#..............................................#",
    "#..............................................#",
    "#...========........................========...#",
    "#..............................................#",
    "#....................======....................#",
    "D.====.........##..............##.........====.#",
    "D..............##......##......##..............#",
    "################################################",
    "################################################",
    "################################################",
];

/// Where the outlaw stands, as `(column, row of the surface it stands on)`.
pub const GROUND_WEST: (i32, i32) = (9, 11);
pub const GROUND_EAST: (i32, i32) = (39, 11);
pub const BALCONY_WEST: (i32, i32) = (8, 6);
pub const BALCONY_EAST: (i32, i32) = (40, 6);
pub const BEAM: (i32, i32) = (23, 8);
/// The crates' middles (arena units across), for taking cover behind.
const CRATES: [f32; 2] = [32.0, 64.0];
/// How far from a crate's middle the outlaw crouches.
const COVER_SIDE: f32 = 3.4;

/// The poster is up this long, and the knights stand untouched while it is.
pub const POSTER: u32 = 3 * HZ + 10;
/// The fall: how long the outlaw takes to die and the bounty to be counted.
pub const FALL: u32 = 2 * HZ + 20;
/// The bounty offered: a base and so much a floor, rounded to fifty. Each
/// knight standing carries home `BOUNTY_GOLD` gold besides, and the score
/// takes the whole.
pub const BOUNTY_BASE: u32 = 400;
pub const BOUNTY_PER_FLOOR: u32 = 300;
pub const BOUNTY_GOLD: u32 = 2;

/// Its states and how long each lasts (a range, picked by the dice of the
/// moment): strafing on the ground, leaping, perched, in cover, calling
/// minions, taunting.
pub const HOLD: u8 = 0;
pub const STRAFE: u8 = 1;
pub const LEAP: u8 = 2;
pub const PERCH: u8 = 3;
pub const COVER: u8 = 4;
pub const CALL: u8 = 5;
pub const TAUNT: u8 = 6;
const STRAFE_FOR: (u32, u32) = (110, 90);
const PERCH_FOR: (u32, u32) = (140, 80);
const COVER_FOR: (u32, u32) = (100, 40);
pub const CALL_FOR: u32 = 70;
pub const TAUNT_FOR: u32 = 48;
pub const LEAP_FOR: u32 = 28;
/// The apex of a leap above the straight line between its ends.
const LEAP_ARC: f32 = 4.6;
/// Its stride on the ground, before the boss file's own pace.
const WALK: f32 = 3.2;
/// How near it likes to keep to a knight when strafing.
const KEEP: f32 = 13.0;
/// A tell shows this many ticks before a volley.
pub const TELL: u32 = 12;
/// Minions: the calls it has in a fight, how many come at a call, how many
/// may stand at once, and how long the rope shows before one drops.
const CALLS: u8 = 2;
const CALLED: usize = 2;
const MINIONS_UP: usize = 4;
pub const ROPE: u32 = 24;
/// The minion is dropped from here (arena units down from the top).
const CEILING: f32 = 3.0;

/// One minion on its rope.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rope {
    pub x: f32,
    pub left: u32,
    pub kind: EnemyKind,
}

/// The outlaw's death throes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fall {
    pub x: f32,
    pub y: f32,
    pub at: u64,
    pub boss: u8,
    pub bounty: u32,
}

/// The showdown as it stands: who is wanted, the poster's tick, the ropes,
/// the death throes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Showdown {
    /// The tick the poster went up (the hall entered with the outlaw alive).
    pub began: Option<u64>,
    pub boss: Option<u8>,
    pub bounty: u32,
    pub calls: u8,
    pub ropes: Vec<Rope>,
    pub fall: Option<Fall>,
    pub paid: bool,
}

impl Showdown {
    pub fn valid(&self) -> bool {
        self.ropes.len() <= 8
            && self.ropes.iter().all(|r| r.x.is_finite())
            && self.fall.is_none_or(|f| f.x.is_finite() && f.y.is_finite())
    }
}

/// Whether `enemy` is the hall's outlaw, steering itself (gravity and all).
pub(super) fn own_gravity(enemy: &Enemy, grid: &Grid) -> bool {
    grid.room.kind == RoomKind::Wanted && enemy.boss.is_some()
}

/// The bounty offered for an outlaw on floor `depth`.
pub fn bounty(boss: &Boss, depth: u32) -> u32 {
    let raw = BOUNTY_BASE + BOUNTY_PER_FLOOR * depth + boss.hp / 4;
    raw / 50 * 50
}

fn anchor((col, row): (i32, i32), radius: f32) -> (f32, f32) {
    (
        (col as f32 + 0.5) * TILE_UNITS,
        row as f32 * TILE_UNITS - radius,
    )
}

/// A pick among `n` by the dice of the moment.
fn pick(h: u64, n: usize) -> usize {
    (h % n.max(1) as u64) as usize
}

fn span((base, spread): (u32, u32), h: u64) -> u32 {
    base + (h >> 20) as u32 % spread.max(1)
}

/// The outlaw's step: its posts, its leaps and its shots.
pub(super) fn act(
    enemy: &mut Enemy,
    boss: &Boss,
    heroes: &[(f32, f32)],
    grid: &Grid,
    seed: u64,
    shots: &mut Vec<Projectile>,
    deeds: &mut foes::Deeds,
) {
    let room = grid.room;
    let r = enemy.radius();
    let target = heroes.iter().copied().min_by(|a, b| {
        (a.0 - enemy.x)
            .hypot(a.1 - enemy.y)
            .total_cmp(&(b.0 - enemy.x).hypot(b.1 - enemy.y))
    });
    let (tx, ty) = target.unwrap_or((enemy.x - 1.0, enemy.y));
    let h = mix(seed ^ u64::from(enemy.id) << 24 ^ u64::from(enemy.age) << 3);
    if enemy.stage == HOLD && enemy.age >= POSTER {
        enemy.stage = STRAFE;
        enemy.timer = 0;
    }
    enemy.timer += 1;
    let ground = |col: i32| anchor((col, 11), r);
    let state = enemy.stage;
    match state {
        HOLD => {
            // Posing for the poster: back in its spot, one foot up.
            let (x, y) = anchor(GROUND_EAST, r);
            (enemy.x, enemy.y) = (x, y);
            return;
        }
        STRAFE => {
            // Keep a gun's distance: close when far, back off when near.
            let gap = enemy.x - tx;
            let want = if gap.abs() > KEEP + 3.0 {
                -gap.signum()
            } else if gap.abs() < KEEP - 4.0 {
                gap.signum()
            } else {
                0.0
            };
            walk(enemy, grid, want, boss.speed.max(1.0) + WALK);
            if enemy.timer >= span(STRAFE_FOR, h) {
                choose(enemy, room, tx, h);
            }
        }
        PERCH => {
            if enemy.timer >= span(PERCH_FOR, h) {
                // Down from the perch: to the ground, or across.
                let across = pick(h >> 8, 10) < 3;
                let (col, row) = if across {
                    if enemy.x < room.width() / 2.0 {
                        BALCONY_EAST
                    } else {
                        BALCONY_WEST
                    }
                } else if tx < room.width() / 2.0 {
                    GROUND_EAST
                } else {
                    GROUND_WEST
                };
                set_leap(enemy, anchor((col, row), r));
            }
        }
        COVER => {
            if enemy.timer >= span(COVER_FOR, h) {
                enemy.stage = STRAFE;
                enemy.timer = 0;
            }
        }
        CALL => {
            if enemy.timer >= CALL_FOR {
                enemy.stage = STRAFE;
                enemy.timer = 0;
            }
        }
        TAUNT => {
            if enemy.timer >= TAUNT_FOR {
                enemy.stage = STRAFE;
                enemy.timer = 0;
            }
        }
        _ => {
            // LEAP: along an arc from where it left to where it lands.
            let t = (enemy.timer as f32 / LEAP_FOR as f32).min(1.0);
            let (x0, y0) = (enemy.origin_x, enemy.origin_y);
            let (x1, y1) = enemy.dir;
            enemy.x = x0 + (x1 - x0) * t;
            enemy.y = y0 + (y1 - y0) * t - (t * std::f32::consts::PI).sin() * LEAP_ARC;
            if enemy.timer >= LEAP_FOR {
                (enemy.x, enemy.y) = (x1, y1);
                let high = y1 < ground(0).1 - 2.0;
                let cover = CRATES
                    .iter()
                    .any(|c| ((c - x1).abs() - COVER_SIDE).abs() < 0.2);
                enemy.stage = if cover {
                    COVER
                } else if high {
                    PERCH
                } else {
                    STRAFE
                };
                enemy.timer = 0;
            }
            return;
        }
    }
    fire(enemy, boss, (tx, ty), seed, shots, state);
    desperate::last_stand(enemy, target, shots, deeds);
}

/// Along the ground by `dir` (-1, 0 or 1) at `speed`, stopping at stone.
fn walk(enemy: &mut Enemy, grid: &Grid, dir: f32, speed: f32) {
    if dir == 0.0 {
        return;
    }
    let nx = enemy.x + dir * speed * DT;
    if grid.clear(nx, enemy.y, enemy.radius() * 0.9, Mover::Walker) {
        enemy.x = nx;
    }
}

fn set_leap(enemy: &mut Enemy, to: (f32, f32)) {
    (enemy.origin_x, enemy.origin_y) = (enemy.x, enemy.y);
    enemy.dir = to;
    enemy.stage = LEAP;
    enemy.timer = 0;
}

/// What it does next after strafing.
fn choose(enemy: &mut Enemy, room: &Room, tx: f32, h: u64) {
    let r = enemy.radius();
    let roll = pick(h >> 10, 100);
    match roll {
        0..=34 => {
            // Up to a balcony, the nearer one to a knight on that side.
            let (col, row) = if tx < room.width() / 2.0 {
                BALCONY_WEST
            } else {
                BALCONY_EAST
            };
            set_leap(enemy, anchor((col, row), r));
        }
        35..=49 => set_leap(enemy, anchor(BEAM, r)),
        50..=74 => {
            // Behind the crate that stands between it and the nearest knight.
            let crate_x = CRATES
                .iter()
                .copied()
                .min_by(|a, b| (a - enemy.x).abs().total_cmp(&(b - enemy.x).abs()))
                .unwrap_or(CRATES[0]);
            let side = if tx < crate_x { 1.0 } else { -1.0 };
            let x = (crate_x + side * COVER_SIDE).clamp(4.0, room.width() - 4.0);
            let y = anchor(GROUND_EAST, r).1;
            set_leap(enemy, (x, y));
        }
        75..=89 => {
            enemy.stage = CALL;
            enemy.timer = 0;
        }
        _ => {
            enemy.stage = TAUNT;
            enemy.timer = 0;
        }
    }
}

/// The states an outlaw shoots in.
pub(super) fn shooting(stage: u8) -> bool {
    matches!(stage, STRAFE | PERCH | COVER)
}

pub(super) fn pattern_pace(stage: u8) -> u32 {
    if stage == COVER { 2 } else { 1 }
}

/// Fire what its file gives it on their beats, in the states that shoot:
/// strafing and perched at the full pace, in cover at half.
fn fire(
    enemy: &mut Enemy,
    boss: &Boss,
    (tx, ty): (f32, f32),
    seed: u64,
    shots: &mut Vec<Projectile>,
    state: u8,
) {
    if !matches!(state, STRAFE | PERCH | COVER) {
        return;
    }
    let age = enemy.age;
    let raging = boss.rage > 0.0 && (enemy.hp as f32) < boss.rage * enemy.max_hp as f32;
    let aim = (ty - enemy.y).atan2(tx - enemy.x);
    let spin = age as f32 * 0.035 + (seed % 17) as f32 * 0.1;
    for (index, attack) in boss.attacks.iter().enumerate() {
        if matches!(attack.pattern, Pattern::Ravage | Pattern::Named(_)) {
            // A named pattern is fired with the monsters' (`patterns`).
            continue;
        }
        let every = if raging {
            (attack.every * 2 / 3).max(4)
        } else {
            attack.every.max(4)
        };
        let every = enemy.beat(every) * if state == COVER { 2 } else { 1 };
        if !(age + index as u32 * 11).is_multiple_of(every.max(4)) {
            continue;
        }
        let kind = attack.bolt.shot(boss.only_in);
        let n = attack.shots.max(1);
        let angles: Vec<f32> = match attack.pattern {
            Pattern::Aimed => (0..n)
                .map(|i| aim + (i as f32 - (n - 1) as f32 / 2.0) * 0.06)
                .collect(),
            Pattern::Fan => {
                let arc = attack.arc.to_radians();
                (0..n)
                    .map(|i| aim + arc * (i as f32 / (n - 1).max(1) as f32 - 0.5))
                    .collect()
            }
            Pattern::Ring => (0..n)
                .map(|i| spin + i as f32 * std::f32::consts::TAU / n as f32)
                .collect(),
            Pattern::Spiral => (0..n)
                .map(|i| spin * 3.0 + i as f32 * std::f32::consts::TAU / n as f32)
                .collect(),
            Pattern::Ravage | Pattern::Named(_) => Vec::new(),
        };
        for angle in angles {
            shots.push(Projectile {
                x: enemy.x,
                y: enemy.y - enemy.radius() * 0.3,
                vx: angle.cos() * attack.speed,
                vy: angle.sin() * attack.speed,
                hostile: true,
                look: None,
                kind,
                damage: attack.damage,
                pierce: 0,
                last_hit: None,
                empowered: false,
                traits: Default::default(),
                ttl: 8 * HZ,
            });
        }
    }
}

/// How close the outlaw's next volley is, from 0 (not yet) to 1 (now), for
/// the tell drawn on it.
pub fn tell(enemy: &Enemy, boss: &Boss) -> f32 {
    if !matches!(enemy.stage, STRAFE | PERCH | COVER) {
        return 0.0;
    }
    let raging = boss.rage > 0.0 && (enemy.hp as f32) < boss.rage * enemy.max_hp as f32;
    boss.attacks
        .iter()
        .enumerate()
        .filter(|(_, a)| a.pattern != Pattern::Ravage)
        .map(|(index, attack)| {
            let every = if raging {
                (attack.every * 2 / 3).max(4)
            } else {
                attack.every.max(4)
            };
            let every = (enemy.beat(every) * if enemy.stage == COVER { 2 } else { 1 }).max(4);
            let next = every - (enemy.age + index as u32 * 11) % every;
            if next <= TELL {
                1.0 - (next as f32 / TELL as f32)
            } else {
                0.0
            }
        })
        .fold(0.0, f32::max)
}

impl Run {
    /// Raise the hall's outlaw as the party walks in, and put up its poster.
    pub(super) fn raise_outlaw(&mut self) {
        self.sync_works();
        if self.room().kind != RoomKind::Wanted || self.room().cleared {
            return;
        }
        let pack = self.dungeon.pack;
        // Not one of this floor's chiefs or its apex, if the pack has another,
        // nor a signature boss: those are kept for the stairs.
        let used: Vec<u8> = self
            .boss_gates
            .as_ref()
            .map(|g| {
                g.leaders
                    .iter()
                    .map(|l| l.boss)
                    .chain(g.guardian_boss)
                    .collect()
            })
            .unwrap_or_default();
        let fresh: Vec<usize> = (0..self.bosses.len())
            .filter(|&i| self.bosses[i].only_in == pack && self.bosses[i].id != "shoggoth-heart")
            .filter(|&i| !used.contains(&(i as u8)) && !signatures::special(&self.bosses[i]))
            .collect();
        let any: Vec<usize> = (0..self.bosses.len())
            .filter(|&i| self.bosses[i].only_in == pack)
            .collect();
        let pool = if fresh.is_empty() { any } else { fresh };
        let Some(&index) = pool.get(pick(
            mix(self.seed
                ^ self.raid_id.rotate_left(9)
                ^ u64::from(self.dungeon.depth) << 32
                ^ 0xdead),
            pool.len(),
        )) else {
            return;
        };
        self.spawn_boss(index);
        let Some(enemy) = self
            .enemies
            .last_mut()
            .filter(|e| e.boss == Some(index as u8))
        else {
            return;
        };
        let (x, y) = anchor(GROUND_EAST, enemy.radius());
        (enemy.x, enemy.y) = (x, y);
        (enemy.origin_x, enemy.origin_y) = (x, y);
        enemy.stage = HOLD;
        let boss = &self.bosses[index];
        let offered = bounty(boss, self.dungeon.depth);
        self.works.outlaw = Showdown {
            began: Some(self.tick),
            boss: Some(index as u8),
            bounty: offered,
            ..Showdown::default()
        };
        self.cues.push(format!("outlaw_rise:{}", boss.id));
        self.cues.push("outlaw_rise".into());
        self.sounds.push("boss_rise");
    }

    /// Whether the poster is up now.
    pub fn poster_up(&self) -> bool {
        self.room().kind == RoomKind::Wanted
            && self
                .works
                .outlaw
                .began
                .is_some_and(|at| self.tick.saturating_sub(at) < u64::from(POSTER))
    }

    /// The outlaw has fallen: its company runs, the fall is recorded and
    /// the bounty paid, once. The hall's tick calls it, and so does the
    /// reaping of the dead, so a blow landed later in the tick still pays.
    pub(super) fn pay_outlaw(&mut self) {
        if self.room().kind != RoomKind::Wanted || self.works.outlaw.paid {
            return;
        }
        let Some((x, y, index, radius)) = self
            .enemies
            .iter()
            .find(|e| e.boss.is_some())
            .filter(|e| e.hp == 0)
            .map(|e| (e.x, e.y, e.boss.unwrap_or(0), e.radius()))
        else {
            return;
        };
        self.works.outlaw.paid = true;
        let offered = self.works.outlaw.bounty;
        self.works.outlaw.fall = Some(Fall {
            x,
            y: y + radius,
            at: self.tick,
            boss: index,
            bounty: offered,
        });
        self.score += offered;
        for hero in self.players.values_mut().filter(|h| h.targetable()) {
            hero.carried.add(Spoil::Gold, BOUNTY_GOLD);
        }
        self.enemies.retain(|e| e.boss.is_some());
        self.works.outlaw.ropes.clear();
        self.cues.push("outlaw_fall".into());
        self.cues.push("bounty_paid".into());
        self.sounds.push("boss_fall");
        self.fx.push(Fx::Shake(20));
    }

    /// The Wanted hall's tick: ropes drop minions, the poster guards the
    /// knights, the outlaw's fall is recorded and the bounty paid.
    pub(super) fn tick_outlaw(&mut self) {
        if self.room().kind != RoomKind::Wanted {
            return;
        }
        if self.poster_up() {
            for hero in self.players.values_mut() {
                hero.invulnerable = hero.invulnerable.max(2);
            }
        }
        let outlaw = self
            .enemies
            .iter()
            .find(|e| e.boss.is_some())
            .map(|e| (e.hp, e.stage, e.timer));
        let Some((hp, stage, timer)) = outlaw else {
            return;
        };
        if hp == 0 {
            self.pay_outlaw();
            return;
        }
        match stage {
            TAUNT if timer == 1 => self.cues.push("outlaw_taunt".into()),
            CALL if timer == 1 => {
                let minions = self.enemies.iter().filter(|e| e.boss.is_none()).count();
                let pending = self.works.outlaw.ropes.len();
                if self.works.outlaw.calls < CALLS && minions + pending < MINIONS_UP {
                    self.works.outlaw.calls += 1;
                    let room_w = self.room().width();
                    let n = CALLED + usize::from(self.party_len() >= 3);
                    let roster = self.dungeon.pack.roster_at(self.dungeon.depth);
                    let walkers: Vec<(EnemyKind, u32)> = roster
                        .iter()
                        .copied()
                        .filter(|(k, _)| !k.elite() && !k.flies() && *k != EnemyKind::Sapper)
                        .collect();
                    for i in 0..n {
                        let kind = if walkers.is_empty() {
                            EnemyKind::Skeleton
                        } else {
                            self.rng.pick(&walkers)
                        };
                        let spread = (i as f32 + 1.0) / (n as f32 + 1.0);
                        let side = if self.works.outlaw.calls % 2 == 1 {
                            0.0
                        } else {
                            0.25
                        };
                        self.works.outlaw.ropes.push(Rope {
                            x: room_w * (0.2 + 0.6 * ((spread + side) % 1.0)),
                            left: ROPE + 6 * i as u32,
                            kind,
                        });
                    }
                    self.cues.push("outlaw_call".into());
                } else if let Some(e) = self.enemies.iter_mut().find(|e| e.boss.is_some()) {
                    // Nothing left to call: it taunts instead.
                    e.stage = TAUNT;
                    e.timer = 0;
                }
            }
            _ => {}
        }
        // Ropes pay out: the minion drops in from above.
        let mut landed: Vec<(f32, EnemyKind)> = Vec::new();
        for rope in &mut self.works.outlaw.ropes {
            rope.left = rope.left.saturating_sub(1);
            if rope.left == 0 {
                landed.push((rope.x, rope.kind));
            }
        }
        self.works.outlaw.ropes.retain(|r| r.left > 0);
        for (x, kind) in landed {
            if self.spawn_body(kind, x, CEILING).is_some()
                && let Some(enemy) = self.enemies.last_mut()
            {
                enemy.age = TELEGRAPH;
            }
        }
    }
}
