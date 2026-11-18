//! The Gloam's small things: the snuffer, a hooded wisp that lunges along a
//! line it shows first, and the turret that looses an iron bolt along one.
//! Every attack shows its footprint for its whole tell and strikes only
//! there.
use super::world::*;
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fk {
    Snuffer,
    Turret,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fs {
    Dormant,
    Drift,
    Tell,
    Strike,
    Recover,
    Stagger,
    Dying,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Danger {
    None,
    Capsule { ax: i32, ay: i32, bx: i32, by: i32, r: i32 },
    Circle { x: i32, y: i32, r: i32 },
    Fan { x: i32, y: i32, deg: i32, half: i32, r: i32 },
    Ring { x: i32, y: i32, r: i32, w: i32 },
}

fn seg_dist2(p: (i64, i64), a: (i64, i64), b: (i64, i64)) -> i64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0 {
        0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) * 1024 / len2).clamp(0, 1024)
    };
    let (cx, cy) = (a.0 + dx * t / 1024, a.1 + dy * t / 1024);
    (p.0 - cx) * (p.0 - cx) + (p.1 - cy) * (p.1 - cy)
}

impl Danger {
    pub fn hits(self, x: i32, y: i32, rad: i32) -> bool {
        match self {
            Danger::None => false,
            Danger::Capsule { ax, ay, bx, by, r } => {
                let reach = i64::from(r + rad);
                seg_dist2(
                    (i64::from(x), i64::from(y)),
                    (i64::from(ax), i64::from(ay)),
                    (i64::from(bx), i64::from(by)),
                ) <= reach * reach
            }
            Danger::Circle { x: cx, y: cy, r } => dist(x - cx, y - cy) <= r + rad,
            Danger::Fan { x: cx, y: cy, deg, half, r } => {
                let d = dist(x - cx, y - cy);
                d <= r + rad && (d < rad || dot(unit(x - cx, y - cy), dir(deg)) >= cos_q(half))
            }
            Danger::Ring { x: cx, y: cy, r, w } => (dist(x - cx, y - cy) - r).abs() <= w + rad,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Foe {
    pub id: u8,
    pub kind: Fk,
    pub x: i32,
    pub y: i32,
    pub hp: u32,
    pub full: u32,
    pub prov: u8,
    pub st: Fs,
    pub t: u16,
    pub fx: i32,
    pub fy: i32,
    pub danger: Danger,
    pub hooded: bool,
    pub lit: u16,
    pub flinch: u8,
    pub freeze: u8,
    pub dead: u8,
    pub open: bool,
    pub hit: [u32; 6],
    pub struck: u8,
    pub kb: (i32, i32),
    pub kbt: u8,
    pub n: u8,
}

impl Foe {
    pub fn new(id: u8, kind: Fk, x: i32, y: i32, seats: u32) -> Foe {
        let per = match kind {
            Fk::Snuffer => SNUFFER_HP,
            Fk::Turret => TURRET_HP,
        };
        let full = per * seats.max(1);
        Foe {
            id,
            kind,
            x,
            y,
            hp: full,
            full,
            prov: seats.clamp(1, MAX_PLAYERS) as u8,
            st: Fs::Dormant,
            t: 0,
            fx: 0,
            fy: Q,
            danger: Danger::None,
            hooded: false,
            lit: 0,
            flinch: 0,
            freeze: 0,
            dead: 0,
            open: false,
            hit: [0; 6],
            struck: 0,
            kb: (0, 0),
            kbt: 0,
            n: 0,
        }
    }

    pub fn alive(&self) -> bool {
        self.hp > 0 && self.st != Fs::Dying
    }

    pub fn radius(&self) -> i32 {
        match self.kind {
            Fk::Snuffer => 6 * Q,
            Fk::Turret => 8 * Q,
        }
    }

    pub fn tell(&self, r: &Rules, seats: u32) -> u16 {
        match self.kind {
            Fk::Snuffer => SNUFFER_TELL,
            Fk::Turret => r.tell_for(seats) as u16,
        }
    }

    pub fn vulnerable(&self) -> bool {
        match self.kind {
            Fk::Snuffer => !(self.hooded && self.lit == 0),
            Fk::Turret => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Bk {
    Iron,
    Light,
    Pot,
    Bright,
    Dim,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bolt {
    pub kind: Bk,
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub ttl: u16,
    pub owner: u8,
}

impl Bolt {
    pub fn light(&self) -> bool {
        matches!(self.kind, Bk::Light | Bk::Bright)
    }
    pub fn hostile(&self) -> bool {
        self.kind == Bk::Iron
    }
}

impl Session {
    pub(super) fn spawn_foe(&mut self, kind: Fk, x: i32, y: i32, hooded: bool) {
        if self.foes.len() >= FOE_CAP {
            return;
        }
        let id = self.next_foe;
        self.next_foe = self.next_foe.wrapping_add(1);
        let mut f = Foe::new(id, kind, x, y, self.seats.len() as u32);
        f.hooded = hooded;
        self.foes.push(f);
    }

    pub(super) fn provision(&mut self) {
        let now = self.seats.len().clamp(1, MAX_PLAYERS as usize) as u32;
        for f in &mut self.foes {
            let had = u32::from(f.prov);
            if now > had && f.hp > 0 {
                f.hp = (u64::from(f.hp) * u64::from(now) / u64::from(had)) as u32;
                f.full = f.full * now / had;
                f.prov = now as u8;
            }
        }
        if let Some(w) = self.boss.as_mut() {
            let had = u32::from(w.prov);
            if now > had && w.hp > 0 {
                w.hp = (u64::from(w.hp) * u64::from(now) / u64::from(had)) as u32;
                w.full = w.full * now / had;
                w.start = (u64::from(w.start) * u64::from(now) / u64::from(had)) as u32;
                w.prov = now as u8;
            }
        }
    }

    pub(super) fn nearest_knight(&self, x: i32, y: i32) -> Option<(u32, i32, i32, i32)> {
        self.seats
            .iter()
            .filter(|(_, k)| k.up())
            .map(|(&id, k)| (dist(k.x - x, k.y - y), id, k.x, k.y))
            .min()
            .map(|(d, id, kx, ky)| (id, kx, ky, d))
    }

    pub(super) fn step_foes(&mut self, r: &Rules, ev: &mut Events) {
        let seats = self.seats.len() as u32;
        let mut strikes: Vec<(usize, Danger, u32)> = Vec::new();
        let mut shots: Vec<Bolt> = Vec::new();
        for i in 0..self.foes.len() {
            let (fx, fy) = (self.foes[i].x, self.foes[i].y);
            let target = self.nearest_knight(fx, fy);
            let burning = self.burning(fx, fy);
            let open_ground = {
                let f = &self.foes[i];
                (f.x, f.y)
            };
            let f = &mut self.foes[i];
            if f.st == Fs::Dying {
                f.dead = f.dead.saturating_add(1);
                continue;
            }
            if f.freeze > 0 {
                f.freeze -= 1;
                continue;
            }
            f.flinch = f.flinch.saturating_sub(1);
            if burning {
                f.lit = LIT_HOLD;
            } else {
                f.lit = f.lit.saturating_sub(1);
            }
            if f.kbt > 0 {
                f.kbt -= 1;
                f.x += f.kb.0;
                f.y += f.kb.1;
            }
            let _ = open_ground;
            match f.kind {
                Fk::Snuffer => step_snuffer(f, target, i, &mut strikes),
                Fk::Turret => step_turret(f, target, r, seats, &mut shots, ev),
            }
        }
        for i in 0..self.foes.len() {
            let f = &self.foes[i];
            if f.kind == Fk::Snuffer && !self.clear(f.x, f.y, 4 * Q) {
                let (cx, cy) = (f.home_x(), f.home_y());
                let _ = (cx, cy);
                let (x, y) = self.nudge(f.x, f.y, 4 * Q);
                self.foes[i].x = x;
                self.foes[i].y = y;
            }
        }
        for s in shots {
            if self.bolts.len() < BOLT_CAP {
                self.bolts.push(s);
            }
        }
        for (i, danger, dmg) in strikes {
            self.strike_with(i, danger, dmg, r, ev);
        }
        self.foes.retain(|f| !(f.st == Fs::Dying && f.dead >= DEATH_TICKS));
    }

    pub(super) fn nudge(&self, x: i32, y: i32, r: i32) -> (i32, i32) {
        for ring in 1..=8 {
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)] {
                let (px, py) = (x + dx * ring * 4 * Q, y + dy * ring * 4 * Q);
                if self.clear(px, py, r) {
                    return (px, py);
                }
            }
        }
        (x, y)
    }

    fn strike_with(&mut self, i: usize, danger: Danger, dmg: u32, r: &Rules, ev: &mut Events) {
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        for id in ids {
            let Some(k) = self.seats.get(&id) else { continue };
            let bit = 1u8 << (id.min(7) as u8);
            if self.foes[i].struck & bit != 0 || !k.up() || !danger.hits(k.x, k.y, KNIGHT_R) {
                continue;
            }
            self.foes[i].struck |= bit;
            let from = (self.foes[i].x, self.foes[i].y);
            match self.blow(id, dmg, from, Hit::Melee, r, ev) {
                Blow::Parried => {
                    let f = &mut self.foes[i];
                    f.st = Fs::Stagger;
                    f.t = STAGGER_TICKS;
                    f.open = true;
                    f.danger = Danger::None;
                    f.freeze = 3;
                    ev.stop(Stop::Phase);
                }
                Blow::Blocked => {
                    let f = &mut self.foes[i];
                    f.kb = (-f.fx / 40, -f.fy / 40);
                    f.kbt = 6;
                }
                _ => {}
            }
        }
    }
}

impl Foe {
    fn home_x(&self) -> i32 {
        self.x
    }
    fn home_y(&self) -> i32 {
        self.y
    }
}

fn step_snuffer(
    f: &mut Foe,
    target: Option<(u32, i32, i32, i32)>,
    index: usize,
    strikes: &mut Vec<(usize, Danger, u32)>,
) {
    let Some((_, tx, ty, d)) = target else {
        return;
    };
    match f.st {
        Fs::Dormant => {
            if d < AGGRO * Q || f.hp < f.full {
                f.st = Fs::Drift;
                f.t = 18 + u16::from(f.id % 5) * 4;
            }
        }
        Fs::Drift => {
            let (ux, uy) = unit(tx - f.x, ty - f.y);
            (f.fx, f.fy) = (ux, uy);
            let want = RING * Q;
            let (mx, my) = if d > want + 4 * Q {
                (ux, uy)
            } else if d < want - 4 * Q {
                (-ux, -uy)
            } else if f.id.is_multiple_of(2) {
                (-uy, ux)
            } else {
                (uy, -ux)
            };
            f.x += mx * SNUFFER_DRIFT / Q;
            f.y += my * SNUFFER_DRIFT / Q;
            f.t = f.t.saturating_sub(1);
            if f.t == 0 && d < STRIKE_FROM * Q {
                let (lx, ly) = unit(tx - f.x, ty - f.y);
                (f.fx, f.fy) = (lx, ly);
                f.danger = Danger::Capsule {
                    ax: f.x,
                    ay: f.y,
                    bx: f.x + lx * LUNGE_LEN,
                    by: f.y + ly * LUNGE_LEN,
                    r: 7 * Q,
                };
                f.st = Fs::Tell;
                f.t = SNUFFER_TELL;
                f.struck = 0;
            } else if f.t == 0 {
                f.t = 8;
            }
        }
        Fs::Tell => {
            if f.flinch > 0 {
                f.st = Fs::Drift;
                f.danger = Danger::None;
                f.t = 30;
                return;
            }
            f.t = f.t.saturating_sub(1);
            if f.t == 0 {
                f.st = Fs::Strike;
                f.t = LUNGE_TICKS;
            }
        }
        Fs::Strike => {
            f.x += f.fx * LUNGE_SPEED / Q;
            f.y += f.fy * LUNGE_SPEED / Q;
            strikes.push((index, f.danger, SNUFFER_DAMAGE));
            f.t = f.t.saturating_sub(1);
            if f.t == 0 {
                f.st = Fs::Recover;
                f.t = SNUFFER_RECOVER;
                f.danger = Danger::None;
            }
        }
        Fs::Recover => {
            f.t = f.t.saturating_sub(1);
            if f.t == 0 {
                f.st = Fs::Drift;
                f.t = 22 + u16::from(f.id % 4) * 6 + u16::from(f.n % 3) * 4;
                f.n = f.n.wrapping_add(1);
            }
        }
        Fs::Stagger => {
            f.t = f.t.saturating_sub(1);
            if f.t == 0 {
                f.st = Fs::Drift;
                f.open = false;
                f.t = 24;
            }
        }
        Fs::Dying => {}
    }
}

fn step_turret(
    f: &mut Foe,
    target: Option<(u32, i32, i32, i32)>,
    r: &Rules,
    seats: u32,
    shots: &mut Vec<Bolt>,
    ev: &mut Events,
) {
    match f.st {
        Fs::Dormant | Fs::Drift => {
            let Some((_, tx, ty, d)) = target else {
                return;
            };
            f.t = f.t.saturating_sub(1);
            if d < TURRET_RANGE * Q && f.t == 0 {
                let (ux, uy) = unit(tx - f.x, ty - f.y);
                (f.fx, f.fy) = (ux, uy);
                f.danger = Danger::Capsule {
                    ax: f.x,
                    ay: f.y,
                    bx: f.x + ux * TURRET_RANGE,
                    by: f.y + uy * TURRET_RANGE,
                    r: 5 * Q,
                };
                f.st = Fs::Tell;
                f.t = f.tell(r, seats);
            }
        }
        Fs::Tell => {
            f.t = f.t.saturating_sub(1);
            if f.t == 0 {
                f.st = Fs::Strike;
                f.t = 6;
                shots.push(Bolt {
                    kind: Bk::Iron,
                    x: f.x + f.fx * 10,
                    y: f.y + f.fy * 10,
                    vx: f.fx * BOLT_SPEED / Q,
                    vy: f.fy * BOLT_SPEED / Q,
                    ttl: BOLT_LIFE,
                    owner: 0,
                });
                ev.sound("shot_bolt");
            }
        }
        Fs::Strike => {
            f.t = f.t.saturating_sub(1);
            if f.t == 0 {
                f.st = Fs::Recover;
                f.t = TURRET_RECOVER;
                f.danger = Danger::None;
            }
        }
        Fs::Recover => {
            f.t = f.t.saturating_sub(1);
            if f.t == 0 {
                f.st = Fs::Dormant;
                f.t = 0;
            }
        }
        Fs::Stagger => {
            f.t = f.t.saturating_sub(1);
            if f.t == 0 {
                f.st = Fs::Dormant;
                f.open = false;
            }
        }
        Fs::Dying => {}
    }
}
