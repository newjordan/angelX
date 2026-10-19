//! The guardians' bounded, deterministic stage machines: the two unruly
//! ones' own (the Big Ripper's breath and rip, Grinwick's rails), and the
//! charge-ups any `.boss` file can name (`bosses::Windup`), built from the
//! same parts — a shown wind-up, then a dash, a slam, a beam, a leap or a
//! breath — and the caps that hold every guardian's shots to its stage of
//! the fight (`hold`).
use super::*;
use bosses::{ChargeUp, Stage, Windup};
use std::f32::consts::PI;
use foes::Deeds;

pub const RIPPER: &str = "the-big-ripper";
pub const ENGINE: &str = "grinwick";
pub const INHALE: u8 = 1;
pub const TELL: u8 = 2;
pub const DASH: u8 = 3;
pub const DAZED: u8 = 4;
pub const JUMP: u8 = 5;
pub const BREATH: u32 = 24;
pub const RIP_TELL: u32 = 21;
pub const RAIL_TELL: u32 = 30;
pub const RIP_SPEED: f32 = 40.0;
/// How fast a breath draws a knight in, units a second: a seventh of a
/// knight's walk, so walking away always gets clear.
pub const PULL_SPEED: f32 = 1.8;
const _: () = assert!(PULL_SPEED * 7.0 <= HERO_SPEED);
/// The nearest a guardian's breath draws a knight, past the edge of its
/// body, in units: clear of its touch (`hurts_by_touch`'s half unit).
pub const PULL_KEEP: f32 = 1.0;
pub const PULL_REACH: f32 = 11.0;
pub const WALL_AREA: f32 = 160.0;
pub const WALL_MAX: usize = 32;
pub const TENDER: u8 = 9;
/// A guardian winding up one of its file's charge-ups.
pub const WIND: u8 = 6;
/// Ticks a guardian stands spent after a charge-up: the knights' turn.
pub const RECOVER: u32 = 30;
/// Ticks a leap is in the air.
pub const FLIGHT: u32 = 15;
/// How far round where it lands a leap strikes.
pub const LANDING: f32 = 2.4;
/// The longest charge a lane is shown for, and the shortest worth turning
/// aside from the knight's line to find.
pub const LANE_MOST: f32 = 30.0;
pub const LANE_LEAST: f32 = 10.0;
/// Shot-ticks of allowance a guardian may bank: one full volley.
pub const BANK: u32 = bosses::VOLLEY_MOST * HZ;
/// How fast a charged beam sweeps its arc, in degrees a second: a knight
/// walking across its line outpaces it out to eighteen units away.
pub const BEAM_TURN: f32 = 40.0;
/// The least width of a slam's clear lane at its first ring, in units:
/// three knights abreast and room to spare.
pub const SLAM_GAP: f32 = 3.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rail {
    pub a: (f32, f32),
    pub b: (f32, f32),
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub active: bool,
    pub cycle: u32,
    pub left: u8,
    pub bounces: u8,
    pub rails: Vec<Rail>,
    pub rail: u8,
    pub jump_from: (f32, f32),
    pub babies: bool,
    /// Which of its file's charge-ups a guardian is winding up.
    #[serde(default)]
    pub windup: u8,
    /// Where a leap will land.
    #[serde(default)]
    pub mark: (f32, f32),
    /// Shot-ticks of its allowance spent (`hold`): each tick gives back its
    /// stage's shots a second.
    #[serde(default)]
    pub spent: u32,
    /// Its latest charge-up's tag (`tag`), carried by everything that
    /// charge-up strikes with.
    #[serde(default, skip_serializing_if = "zero")]
    pub blow: u32,
    /// Something of its latest charge-up is still about — a shot, its
    /// beam, a ring — so it winds up no other (`Run`, each tick).
    #[serde(skip)]
    pub lingering: bool,
}
impl State {
    pub fn quiet(&self) -> bool {
        self == &Self::default()
    }
    pub fn valid(&self) -> bool {
        self.left <= 3
            && self.bounces <= 2
            && self.rail <= 2
            && self.rails.len() <= 3
            && self
                .rails
                .iter()
                .all(|r| [r.a.0, r.a.1, r.b.0, r.b.1].into_iter().all(f32::is_finite))
            && self.jump_from.0.is_finite()
            && self.jump_from.1.is_finite()
            && usize::from(self.windup) < bosses::WINDUPS_MOST
            && self.mark.0.is_finite()
            && self.mark.1.is_finite()
            && self.spent <= BANK * 4
    }
}
pub fn special(boss: &Boss) -> bool {
    matches!(boss.id.as_str(), RIPPER | ENGINE)
}
/// It may shoot this tick: not mid-signature, nor winding up or spent
/// after a charge-up (an outlaw keeps its own stages).
pub fn shooting(enemy: &Enemy, boss: &Boss, wanted: bool) -> bool {
    if special(boss) {
        enemy.stage == 0
    } else {
        wanted || boss.windups.is_empty() || enemy.stage == 0
    }
}
/// The charge-ups a guardian fights with: its file's, or the two unruly
/// ones' own (the Ripper breathes in, then rips; Grinwick charges its
/// rails).
pub fn charge_ups(boss: &Boss) -> Vec<Windup> {
    match boss.id.as_str() {
        RIPPER => vec![Windup::Inhale, Windup::Charge],
        ENGINE => vec![Windup::Charge],
        _ => boss.windups.iter().map(|w| w.kind).collect(),
    }
}
/// The charge-up it will wind up next, if one is due in `stage`: its file's
/// lines from their stage on, taken in turn, each `every` ticks after the
/// last one ended (two thirds of that in its rage).
pub fn due(enemy: &Enemy, boss: &Boss, stage: Stage) -> Option<usize> {
    if special(boss) || enemy.stage != 0 || enemy.signature.lingering {
        return None;
    }
    let open: Vec<usize> = (0..boss.windups.len())
        .filter(|&k| boss.windups[k].from <= stage)
        .collect();
    let k = *open.get(enemy.signature.cycle as usize % open.len().max(1))?;
    let every = boss.windups[k].every;
    let every = if stage == Stage::Rage { every * 2 / 3 } else { every };
    (enemy.timer >= every).then_some(k)
}
/// What a shot weighs against the caps: itself, and the ring it bursts into.
pub fn weight(shot: &Projectile) -> u32 {
    match shot.traits.path.then {
        patterns::Then::Burst(n, _) => 1 + u32::from(n),
        _ => 1,
    }
}
/// Hold a guardian to its stage's caps (`bosses::Stage::caps`) for one
/// tick: its volleys this tick (`shots[from..]`) fly whole or not at all,
/// keeping back what a due charge-up will need; its charge-up's burst,
/// paid for as it began, is trimmed to the room left. `standing` is what
/// the room's hostile shots already weigh, `heads` the knights standing.
pub(super) fn hold(
    enemy: &mut Enemy,
    boss: &Boss,
    shots: &mut Vec<Projectile>,
    from: usize,
    bursts: &mut Vec<Projectile>,
    standing: u32,
    heads: usize,
) {
    let stage = boss.stage(enemy.hp, enemy.max_hp);
    let (per_second, on_screen) = stage.caps();
    let room = on_screen * heads.max(1) as u32;
    enemy.signature.spent = enemy.signature.spent.saturating_sub(per_second);
    let reserve = due(enemy, boss, stage).map_or(0, |k| boss.windups[k].burst() * HZ);
    let volley: u32 = shots[from.min(shots.len())..].iter().map(weight).sum();
    let spent = enemy.signature.spent;
    let kept = if volley == 0 {
        0
    } else if standing + volley > room || (spent > 0 && spent + volley * HZ + reserve > BANK) {
        shots.truncate(from);
        0
    } else {
        enemy.signature.spent = (spent + volley * HZ).min(BANK * 4);
        volley
    };
    let mut left = room.saturating_sub(standing + kept);
    for shot in bursts.drain(..) {
        let w = weight(&shot);
        if w <= left {
            left -= w;
            shots.push(shot);
        }
    }
}
pub fn swell(enemy: &Enemy) -> f32 {
    match enemy.stage {
        INHALE => match enemy.timer / BREATH {
            0 => 1.0,
            1 => 1.35,
            2 => 1.7,
            _ => 2.0,
        },
        TELL => 2.0,
        DASH => 2.0 - (enemy.timer as f32 / 33.0).min(1.0),
        _ => 1.0,
    }
}
/// A seeded candidate with the longest clear launch corridor. Sampling all
/// quadrants favours open room without turning the rip into aimed pursuit.
pub(super) fn rip_direction(enemy: &Enemy, grid: &Grid, seed: u64) -> (f32, f32) {
    let salt = mix(seed ^ u64::from(enemy.id) << 32 ^ u64::from(enemy.signature.cycle));
    let start = (salt % 65536) as f32 / 65536.0 * std::f32::consts::TAU;
    (0..8)
        .map(|k| {
            let angle = start + k as f32 * std::f32::consts::TAU / 8.0;
            let dir = (angle.cos(), angle.sin());
            let reach = (1..=16)
                .take_while(|&n| {
                    grid.clear(
                        enemy.x + dir.0 * n as f32,
                        enemy.y + dir.1 * n as f32,
                        enemy.radius(),
                        Mover::Walker,
                    )
                })
                .count();
            (reach, dir)
        })
        .max_by_key(|(reach, _)| *reach)
        .map(|(_, d)| d)
        .unwrap_or((1.0, 0.0))
}
/// What a guardian's charge-up strikes with carries this, for this
/// guardian and this charge-up: a knight it has struck carries it too
/// (`Hero::blow`), and is struck by nothing more of it.
fn tag(enemy: &Enemy) -> u32 {
    (enemy.id.rotate_left(16) ^ enemy.signature.cycle).max(1)
}
/// Ticks a charged beam burns sweeping its arc at `BEAM_TURN`.
pub fn beam_burn(up: &ChargeUp) -> u8 {
    (up.arc.abs() / BEAM_TURN * HZ as f32)
        .ceil()
        .clamp(f32::from(patterns::SWEEP_BURN), 255.0) as u8
}
/// A slam's clear lane, in degrees: its file's, or `SLAM_GAP` wide at the
/// first ring if that is wider.
pub fn slam_lane(up: &ChargeUp) -> f32 {
    up.arc.max((SLAM_GAP / tide::RING_GAP).to_degrees())
}
/// Ticks a guardian stands spent after the blow: its blow's own length (a
/// slam's later rings, a beam's sweep), then `RECOVER`.
pub fn spent_for(up: &ChargeUp) -> u32 {
    let blow = match up.kind {
        Windup::Slam => (up.shots.clamp(1, 8) - 1) * tide::step_for(up.speed) + 1,
        Windup::Beam => u32::from(beam_burn(up)) + 1,
        Windup::Charge | Windup::Leap | Windup::Inhale => 0,
    };
    blow + RECOVER
}
fn stage(enemy: &mut Enemy, next: u8) {
    enemy.stage = next;
    enemy.timer = 0;
}
fn begin_rip(enemy: &mut Enemy, grid: &Grid, seed: u64) {
    enemy.signature.cycle = enemy.signature.cycle.wrapping_add(1);
    enemy.dir = rip_direction(enemy, grid, seed);
    enemy.signature.bounces = 0;
    stage(enemy, TELL);
}
fn dash(enemy: &mut Enemy, grid: &Grid, speed: f32, bounce: bool, deeds: &mut Deeds) -> bool {
    dash_for(enemy, grid, speed, bounce, 24, 0, deeds)
}
fn dash_for(
    enemy: &mut Enemy,
    grid: &Grid,
    speed: f32,
    bounce: bool,
    damage: u32,
    blow: u32,
    deeds: &mut Deeds,
) -> bool {
    let from = (enemy.x, enemy.y);
    // Small collision steps leave a continuous trail and cannot tunnel stone.
    let stride = speed * DT / 4.0;
    let mut stopped = false;
    for _ in 0..4 {
        let next = (
            enemy.x + enemy.dir.0 * stride,
            enemy.y + enemy.dir.1 * stride,
        );
        if grid.clear(next.0, next.1, enemy.radius(), Mover::Walker) {
            (enemy.x, enemy.y) = next;
        } else if bounce && enemy.signature.bounces < 2 {
            let x_blocked = !grid.clear(next.0, enemy.y, enemy.radius(), Mover::Walker);
            let y_blocked = !grid.clear(enemy.x, next.1, enemy.radius(), Mover::Walker);
            if x_blocked {
                enemy.dir.0 = -enemy.dir.0;
            }
            if y_blocked {
                enemy.dir.1 = -enemy.dir.1;
            }
            if !x_blocked && !y_blocked {
                enemy.dir = (-enemy.dir.0, -enemy.dir.1);
            }
            enemy.signature.bounces += 1;
        } else {
            stopped = true;
            break;
        }
    }
    deeds.sweeps.push((
        from,
        (enemy.x, enemy.y),
        enemy.radius() + HERO_RADIUS,
        damage,
        enemy.dir,
        blow,
    ));
    stopped
}
pub(super) fn act(
    enemy: &mut Enemy,
    boss: &Boss,
    heroes: &[(f32, f32)],
    grid: &Grid,
    seed: u64,
    deeds: &mut Deeds,
) -> bool {
    if !special(boss) {
        return false;
    }
    enemy.signature.active = true;
    enemy.timer += 1;
    if boss.id == RIPPER {
        let raging = (enemy.hp as f32) < enemy.max_hp as f32 * 0.4;
        let radius = boss.radius * swell(enemy);
        // Each breath pushes his growing belly clear of nearby stone.
        if (radius > enemy.radius() || enemy.stage == TELL)
            && !grid.clear(enemy.x, enemy.y, radius, Mover::Walker)
        {
            let mut best = None;
            for dy in -8..=8 {
                for dx in -8..=8 {
                    let spot = (enemy.x + dx as f32 * 0.5, enemy.y + dy as f32 * 0.5);
                    if grid.clear(spot.0, spot.1, radius, Mover::Walker) {
                        let distance = dx * dx + dy * dy;
                        if best.is_none_or(|(d, _)| distance < d) {
                            best = Some((distance, spot));
                        }
                    }
                }
            }
            if let Some((_, spot)) = best {
                (enemy.x, enemy.y) = spot;
            }
        }
        enemy.size = Some(radius);
        if enemy.stage == 0 {
            let (ux, uy) = unit(
                grid.room.width() / 2.0 - enemy.x,
                grid.room.height() / 2.0 - enemy.y,
            );
            (enemy.x, enemy.y) = grid.slide(
                (enemy.x, enemy.y),
                (ux * 3.0 * DT, uy * 3.0 * DT),
                enemy.radius(),
                Mover::Walker,
            );
        }
        match enemy.stage {
            0 if enemy.timer >= 72 => {
                stage(enemy, INHALE);
            }
            INHALE => {
                deeds.pulls.push((enemy.x, enemy.y, 0.1));
                if enemy.timer >= BREATH * 3 {
                    enemy.size = Some(boss.radius * 2.0);
                    enemy.signature.left = if raging { 3 } else { 1 };
                    begin_rip(enemy, grid, seed);
                }
            }
            TELL if enemy.timer >= RIP_TELL => {
                stage(enemy, DASH);
                deeds.sounds.push("gas_release");
                deeds.shake = 12;
            }
            DASH => {
                let stop = dash(enemy, grid, RIP_SPEED, true, deeds);
                deeds.clouds.push(skills::Cloud {
                    x: enemy.x,
                    y: enemy.y,
                    vx: 0.0,
                    vy: 0.0,
                    age: 0,
                    owner: 0,
                    power: 4,
                    tint: if raging { 3 } else { 2 },
                });
                if stop || enemy.timer >= 33 {
                    enemy.signature.left = enemy.signature.left.saturating_sub(1);
                    if enemy.signature.left > 0 {
                        enemy.size = Some(boss.radius * 2.0);
                        begin_rip(enemy, grid, seed);
                    } else {
                        stage(enemy, DAZED);
                    }
                }
            }
            DAZED if enemy.timer >= 42 => {
                stage(enemy, 0);
            }
            _ => {}
        }
    } else {
        engine(enemy, boss, heroes, grid, seed, deeds);
    }
    true
}
/// The nearest of `heroes` to the guardian.
fn nearest(enemy: &Enemy, heroes: &[(f32, f32)]) -> Option<(f32, f32)> {
    heroes.iter().copied().min_by(|a, b| {
        (a.0 - enemy.x)
            .hypot(a.1 - enemy.y)
            .total_cmp(&(b.0 - enemy.x).hypot(b.1 - enemy.y))
    })
}
/// The clear spot nearest `to` where the guardian fits, if any near.
fn clear_near(enemy: &Enemy, grid: &Grid, to: (f32, f32)) -> Option<(f32, f32)> {
    let mut best = None;
    for dy in -8..=8 {
        for dx in -8..=8 {
            let spot = (to.0 + dx as f32 * 0.5, to.1 + dy as f32 * 0.5);
            if grid.clear(spot.0, spot.1, enemy.radius(), Mover::Walker) {
                let d = dx * dx + dy * dy;
                if best.is_none_or(|(b, _)| d < b) {
                    best = Some((d, spot));
                }
            }
        }
    }
    best.map(|(_, spot)| spot)
}
/// The charge-up it is winding up or striking with.
pub fn winding<'a>(enemy: &Enemy, boss: &'a Boss) -> Option<&'a ChargeUp> {
    (!special(boss) && matches!(enemy.stage, WIND | DASH | JUMP | DAZED))
        .then(|| boss.windups.get(usize::from(enemy.signature.windup)))
        .flatten()
}
/// A guardian's charge-ups from its file, one tick: when one is due (and
/// its burst is in its allowance) it stands and winds up where everyone can
/// see, then strikes, then stands spent. True while it is busy at one, so
/// it neither walks nor shoots.
pub(super) fn wind(
    enemy: &mut Enemy,
    boss: &Boss,
    heroes: &[(f32, f32)],
    grid: &Grid,
    seed: u64,
    deeds: &mut Deeds,
) -> bool {
    if special(boss) || boss.windups.is_empty() {
        return false;
    }
    enemy.timer += 1;
    let now = boss.stage(enemy.hp, enemy.max_hp);
    if enemy.stage == 0 {
        let Some(k) = due(enemy, boss, now) else {
            return false;
        };
        let up = &boss.windups[k];
        let cost = up.burst() * HZ;
        let Some(target) = nearest(enemy, heroes) else {
            return false;
        };
        if enemy.signature.spent + cost > BANK {
            // It gathers itself: what it will throw comes out of its
            // allowance first (`hold` keeps its volleys back meanwhile).
            return false;
        }
        enemy.signature.spent += cost;
        // Busy at a charge-up, its body does not strike (`hurts_by_touch`).
        enemy.signature.active = true;
        enemy.signature.windup = k as u8;
        enemy.signature.cycle = enemy.signature.cycle.wrapping_add(1);
        enemy.signature.blow = tag(enemy);
        begin(enemy, boss, up, target, grid, seed, deeds);
        stage(enemy, WIND);
        return true;
    }
    let Some(up) = boss.windups.get(usize::from(enemy.signature.windup)) else {
        stage(enemy, 0);
        return false;
    };
    match enemy.stage {
        WIND => {
            if up.kind == Windup::Inhale {
                deeds.pulls.push((enemy.x, enemy.y, enemy.radius() + PULL_KEEP));
            }
            if enemy.timer >= up.wind {
                strike(enemy, boss, up, deeds);
            }
        }
        DASH => {
            let stopped = dash_for(enemy, grid, up.speed, false, up.damage, enemy.signature.blow, deeds);
            let arrived = enemy
                .signature
                .rails
                .first()
                .is_none_or(|r| (r.b.0 - enemy.x).hypot(r.b.1 - enemy.y) < 1.0);
            if stopped || arrived || enemy.timer >= 45 {
                stage(enemy, DAZED);
            }
        }
        JUMP => {
            let t = (enemy.timer as f32 / FLIGHT as f32).min(1.0);
            let (a, b) = (enemy.signature.jump_from, enemy.signature.mark);
            enemy.x = a.0 + (b.0 - a.0) * t;
            enemy.y = a.1 + (b.1 - a.1) * t;
            if enemy.timer >= FLIGHT {
                (enemy.x, enemy.y) = b;
                // Its heading back the way it came: open floor, for the
                // knights it lands on to be thrown to (`signature_deeds`).
                let back = unit(a.0 - b.0, a.1 - b.1);
                deeds.sweeps.push((b, b, LANDING + HERO_RADIUS, up.damage, back, enemy.signature.blow));
                ring(enemy, boss, up, deeds);
                deeds.sounds.push("rock_land");
                deeds.shake = 10;
                stage(enemy, DAZED);
            }
        }
        DAZED => {
            if enemy.timer >= spent_for(up) {
                enemy.signature.rails.clear();
                stage(enemy, 0);
            }
        }
        _ => {
            stage(enemy, 0);
            return false;
        }
    }
    true
}
/// The wind-up's start: what it shows, and what is already on its way.
fn begin(
    enemy: &mut Enemy,
    boss: &Boss,
    up: &ChargeUp,
    target: (f32, f32),
    grid: &Grid,
    seed: u64,
    deeds: &mut Deeds,
) {
    let from = (enemy.x, enemy.y);
    enemy.dir = unit(target.0 - from.0, target.1 - from.1);
    deeds.sounds.push("boss_roar");
    match up.kind {
        Windup::Charge => {
            // The lane runs at the knight to the first stone, or a little
            // to one side where stone stands close; it is shown for the
            // whole wind-up.
            let aim = enemy.dir.1.atan2(enemy.dir.0);
            let lane = |angle: f32| {
                let dir = (angle.cos(), angle.sin());
                let mut b = from;
                for n in 1..=(LANE_MOST * 2.0) as i32 {
                    let p = (from.0 + dir.0 * n as f32 * 0.5, from.1 + dir.1 * n as f32 * 0.5);
                    if !grid.clear(p.0, p.1, enemy.radius(), Mover::Walker) {
                        break;
                    }
                    b = p;
                }
                (dir, b, (b.0 - from.0).hypot(b.1 - from.1))
            };
            let (dir, b, _) = [0.0, -0.35, 0.35, -0.7, 0.7, -1.2, 1.2, -1.8, 1.8, PI]
                .into_iter()
                .map(|turn| lane(aim + turn))
                .enumerate()
                .max_by(|(i, x), (j, y)| {
                    // The straight line wins unless stone cuts it short.
                    let score = |k: usize, reach: f32| reach.min(LANE_LEAST) * 10.0 - k as f32;
                    score(*i, x.2).total_cmp(&score(*j, y.2))
                })
                .map(|(_, lane)| lane)
                .unwrap_or(((1.0, 0.0), from, 0.0));
            enemy.dir = dir;
            enemy.signature.rails = vec![Rail { a: from, b }];
        }
        Windup::Slam => {
            // The floor ripples now; its rings burst as the wind-up ends.
            let mut slam = tide::Ravage::cast(
                from,
                up.shots,
                slam_lane(up),
                up.speed,
                up.damage,
                seed ^ u64::from(enemy.id) << 16 ^ u64::from(enemy.signature.cycle),
            );
            slam.blow = enemy.signature.blow;
            deeds.ravages.push(slam);
            deeds.cues.push("ravage");
        }
        Windup::Beam => {
            let aim = enemy.dir.1.atan2(enemy.dir.0);
            let arc = up.arc.to_radians();
            let way = if enemy.signature.cycle.is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };
            let mut beam = patterns::charged_beam(
                grid,
                from,
                aim - arc / 2.0 * way,
                arc * way,
                up.wind.min(255) as u8,
                beam_burn(up),
                up.bolt.shot(boss.only_in),
                up.damage,
            );
            beam.traits.blow = enemy.signature.blow;
            deeds.bursts.push(beam);
        }
        Windup::Leap => {
            enemy.signature.jump_from = from;
            enemy.signature.mark = clear_near(enemy, grid, target).unwrap_or(from);
        }
        Windup::Inhale => {}
    }
}
/// The wind-up is over: the blow.
fn strike(enemy: &mut Enemy, boss: &Boss, up: &ChargeUp, deeds: &mut Deeds) {
    match up.kind {
        Windup::Charge => {
            if let Some(r) = enemy.signature.rails.first() {
                enemy.dir = unit(r.b.0 - r.a.0, r.b.1 - r.a.1);
            }
            deeds.shake = 8;
            stage(enemy, DASH);
        }
        Windup::Slam => {
            deeds.shake = 10;
            stage(enemy, DAZED);
        }
        Windup::Beam => {
            deeds.sounds.push("spell_fire");
            stage(enemy, DAZED);
        }
        Windup::Leap => {
            stage(enemy, JUMP);
        }
        Windup::Inhale => {
            ring(enemy, boss, up, deeds);
            deeds.sounds.push("gas_release");
            deeds.shake = 6;
            stage(enemy, DAZED);
        }
    }
}
/// A burst of the charge-up's shots round the guardian, evenly spaced and
/// turned half a step off the way it faced, so the knight it was for
/// stands in a gap.
fn ring(enemy: &Enemy, boss: &Boss, up: &ChargeUp, deeds: &mut Deeds) {
    let n = up.shots;
    if n == 0 {
        return;
    }
    let step = std::f32::consts::TAU / n as f32;
    let start = enemy.dir.1.atan2(enemy.dir.0) + step / 2.0;
    for i in 0..n {
        let a = start + i as f32 * step;
        deeds.bursts.push(Projectile {
            x: enemy.x,
            y: enemy.y,
            vx: a.cos() * up.speed,
            vy: a.sin() * up.speed,
            hostile: true,
            look: None,
            kind: up.bolt.shot(boss.only_in),
            damage: up.damage,
            pierce: 0,
            last_hit: None,
            empowered: false,
            traits: ShotTraits {
                blow: enemy.signature.blow,
                ..Default::default()
            },
            ttl: 6 * HZ,
        });
    }
}
fn lay_rails(enemy: &mut Enemy, heroes: &[(f32, f32)], grid: &Grid, seed: u64) {
    let target = heroes
        .first()
        .copied()
        .unwrap_or((grid.room.width() / 2.0, grid.room.height() / 2.0));
    let aim = (target.1 - enemy.y).atan2(target.0 - enemy.x);
    let jitter = (mix(seed ^ u64::from(enemy.signature.cycle)) % 100) as f32 * 0.004 - 0.2;
    enemy.signature.rails.clear();
    for k in 0..3 {
        let a = (enemy.x, enemy.y);
        let angle = aim + jitter + (k as f32 - 1.0) * 0.72;
        let dir = (angle.cos(), angle.sin());
        // Rails follow traversable floor, ending before a wall or pillar.
        let mut b = a;
        for n in 1..=120 {
            let p = (a.0 + dir.0 * n as f32 * 0.5, a.1 + dir.1 * n as f32 * 0.5);
            if !grid.clear(p.0, p.1, enemy.radius(), Mover::Walker) {
                break;
            }
            b = p;
        }
        enemy.signature.rails.push(Rail { a, b });
    }
    // A larger engine can leave a side branch very short beside a wall.
    // Start on the longest clear corridor; each later branch is still warned.
    let reach = |r: &Rail| (r.b.0 - r.a.0).hypot(r.b.1 - r.a.1);
    enemy
        .signature
        .rails
        .sort_by(|a, b| reach(b).total_cmp(&reach(a)));
    enemy.signature.rail = 0;
    enemy.signature.left = if enemy.hp * 2 < enemy.max_hp { 3 } else { 1 };
    enemy.signature.cycle = enemy.signature.cycle.wrapping_add(1);
    stage(enemy, TELL);
}
fn engine(
    enemy: &mut Enemy,
    boss: &Boss,
    heroes: &[(f32, f32)],
    grid: &Grid,
    seed: u64,
    deeds: &mut Deeds,
) {
    if enemy.hp * 2 < enemy.max_hp && !enemy.signature.babies {
        enemy.signature.babies = true;
        for k in 0..4 {
            let a = k as f32 * std::f32::consts::FRAC_PI_2;
            deeds.spawns.push((
                EnemyKind::Spiderling,
                enemy.x + a.cos() * (enemy.radius() + 2.0),
                enemy.y + a.sin() * (enemy.radius() + 2.0),
                TENDER,
            ));
        }
    }
    match enemy.stage {
        0 => {
            let (tx, ty) = heroes.first().copied().unwrap_or((enemy.x, enemy.y));
            let (ux, uy) = unit(tx - enemy.x, ty - enemy.y);
            let wobble = (enemy.timer as f32 * 0.3).sin() * 0.45;
            enemy.dir = unit(ux - uy * wobble, uy + ux * wobble);
            (enemy.x, enemy.y) = grid.slide(
                (enemy.x, enemy.y),
                (enemy.dir.0 * boss.speed * DT, enemy.dir.1 * boss.speed * DT),
                enemy.radius(),
                Mover::Walker,
            );
            if enemy.timer == 36 {
                deeds.slams.push((enemy.x, enemy.y));
            }
            if enemy.timer >= 105 {
                lay_rails(enemy, heroes, grid, seed);
                deeds.sounds.push("engine_whistle");
            }
        }
        TELL if enemy.timer >= RAIL_TELL => {
            if let Some(r) = enemy.signature.rails.get(enemy.signature.rail as usize) {
                enemy.dir = unit(r.b.0 - r.a.0, r.b.1 - r.a.1);
            }
            stage(enemy, DASH);
            deeds.shake = 8;
        }
        DASH => {
            let stopped = dash(enemy, grid, 38.0, false, deeds);
            let arrived = enemy
                .signature
                .rails
                .get(enemy.signature.rail as usize)
                .is_none_or(|r| (r.b.0 - enemy.x).hypot(r.b.1 - enemy.y) < 1.4);
            if stopped || arrived || enemy.timer >= 45 {
                enemy.signature.left = enemy.signature.left.saturating_sub(1);
                if enemy.signature.left > 0 {
                    enemy.signature.rail = (enemy.signature.rail + 1).min(2);
                    enemy.signature.jump_from = (enemy.x, enemy.y);
                    stage(enemy, JUMP);
                } else {
                    stage(enemy, DAZED);
                }
            }
        }
        JUMP => {
            if let Some(r) = enemy.signature.rails.get(enemy.signature.rail as usize) {
                let t = (enemy.timer as f32 / 24.0).min(1.0);
                enemy.x = enemy.signature.jump_from.0 + (r.a.0 - enemy.signature.jump_from.0) * t;
                enemy.y = enemy.signature.jump_from.1 + (r.a.1 - enemy.signature.jump_from.1) * t;
            }
            if enemy.timer >= 24 {
                stage(enemy, TELL);
                deeds.sounds.push("engine_whistle");
            }
        }
        DAZED if enemy.timer >= 36 => {
            enemy.signature.rails.clear();
            stage(enemy, 0);
        }
        _ => {}
    }
}
pub fn rear_bonus(enemy: &Enemy, bosses: &[Boss], from: (f32, f32), damage: u32) -> u32 {
    if enemy.stage == DAZED
        && enemy
            .boss
            .and_then(|i| bosses.get(i as usize))
            .is_some_and(|b| b.id == RIPPER)
        && (from.0 - enemy.x) * enemy.dir.0 + (from.1 - enemy.y) * enemy.dir.1 < 0.0
    {
        damage / 2
    } else {
        0
    }
}
/// Gas trail area is bounded by the sum of whole segment disks, so overlaps
/// can only reduce coverage. Oldest walls yield first; player gas stays put.
pub fn add_wall(clouds: &mut Vec<skills::Cloud>, cloud: skills::Cloud) {
    let area = |c: &skills::Cloud| std::f32::consts::PI * c.wall_radius().powi(2);
    while clouds.iter().filter(|c| c.wall()).count() >= WALL_MAX
        || clouds.iter().filter(|c| c.wall()).map(area).sum::<f32>() + area(&cloud) > WALL_AREA
        || clouds.len() >= 48
    {
        let Some(i) = clouds.iter().position(skills::Cloud::wall) else {
            return;
        };
        clouds.remove(i);
    }
    clouds.push(cloud);
}
impl Run {
    pub(super) fn signature_deeds(
        &mut self,
        clouds: Vec<skills::Cloud>,
        pulls: Vec<(f32, f32, f32)>,
        sweeps: Vec<((f32, f32), (f32, f32), f32, u32, (f32, f32), u32)>,
    ) {
        for cloud in clouds {
            add_wall(&mut self.works.clouds, cloud);
        }
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        for (x, y, keep) in pulls {
            for h in self
                .players
                .values_mut()
                .filter(|h| h.hp > 0 && !h.stone && h.rolling().is_none())
            {
                let d = (h.x - x).hypot(h.y - y);
                if d < PULL_REACH && d > keep {
                    let (ux, uy) = unit(x - h.x, y - h.y);
                    (h.x, h.y) = grid.slide(
                        (h.x, h.y),
                        (ux * PULL_SPEED * DT, uy * PULL_SPEED * DT),
                        HERO_RADIUS,
                        Mover::Hero,
                    );
                }
            }
            for p in &mut self.projectiles {
                let d = (p.x - x).hypot(p.y - y);
                if d < PULL_REACH && d > 0.1 {
                    let (ux, uy) = unit(x - p.x, y - p.y);
                    p.vx += ux * 2.4 * DT;
                    p.vy += uy * 2.4 * DT;
                }
            }
        }
        for &(a, b, r, _, back, blow) in &sweeps {
            if blow == 0 || a != b {
                continue;
            }
            // A charge-up's landing throws the knights under it clear of
            // its body, as a slam's ring does, struck or not: straight out,
            // or, against stone, back the way the guardian came.
            for h in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
                let d = (h.x - b.0).hypot(h.y - b.1);
                if d >= r {
                    continue;
                }
                let throw = |from: (f32, f32), (ux, uy): (f32, f32), far: f32| {
                    // In steps, so stone stops it where it stands.
                    (0..8).fold(from, |at, _| {
                        grid.slide(at, (ux * far / 8.0, uy * far / 8.0), HERO_RADIUS, Mover::Hero)
                    })
                };
                let out = if d > 0.01 { unit(h.x - b.0, h.y - b.1) } else { back };
                let mut at = throw((h.x, h.y), out, r - d);
                if (at.0 - b.0).hypot(at.1 - b.1) < r - 0.05 && back != (0.0, 0.0) {
                    at = throw((h.x, h.y), back, r + d);
                }
                (h.x, h.y) = at;
                h.tossed = tide::TOSSED;
            }
        }
        for (a, b, r, damage, _, blow) in sweeps {
            for h in self
                .players
                .values_mut()
                .filter(|h| h.hp > 0 && !h.stone && h.invulnerable == 0)
            {
                if !hit_segment(a.0, a.1, b.0, b.1, h.x, h.y, r) {
                    continue;
                }
                // A charge-up's dash or landing strikes each knight once.
                if blow != 0 {
                    if h.blow(damage, blow) {
                        self.sounds.push("hero_hurt");
                    }
                } else {
                    h.hurt(damage);
                    self.sounds.push("hero_hurt");
                }
            }
        }
    }
}
#[cfg(test)]
#[path = "../../tests/rules/together_signatures__tests.rs"]
mod tests;
