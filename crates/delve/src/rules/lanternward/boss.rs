//! The Warden: a great lantern-bearer whose lamp-heart is shuttered. A
//! thrown bolt of light, or a flare held on it, opens the shutter for a
//! while (from the second third of its health, two knights' light at once);
//! a parried beam knocks it open too. Then the blade counts. Three phases,
//! every tell at least 40 ticks, every footprint shown for the whole tell.
use super::blow::Hit;
use super::foes::*;
use super::world::*;
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ws {
    Asleep,
    Rest,
    Tell,
    Strike,
    Recover,
    /// Its beam parried: shutter wide open and a double blow waiting.
    Stagger,
    Dying,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Atk {
    /// A beam swept across a fan (two, in the last phase).
    Beam,
    /// A ring along the ground: hop it or roll through it.
    Slam,
    /// Snuffers climb out of the fissures.
    Summon,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warden {
    pub x: i32,
    pub y: i32,
    pub hp: u32,
    pub full: u32,
    pub prov: u8,
    /// 0, 1 or 2: thirds of its health.
    pub phase: u8,
    pub st: Ws,
    pub t: u16,
    pub atk: Atk,
    pub n: u8,
    pub danger: Danger,
    /// The beam's middle, and how far it has swept.
    pub aim: i32,
    pub sweep: i32,
    /// The tick each seat last had light on the heart.
    pub lit: [u32; 6],
    pub hit: [u32; 6],
    pub struck: u8,
    pub dead: u16,
    /// Its health when the phase began: where a retry starts it.
    pub start: u32,
    pub face: (i32, i32),
    pub flinch: u8,
}

impl Warden {
    pub fn new(x: i32, y: i32, seats: u32) -> Warden {
        let full = WARDEN_HP * seats.max(1);
        Warden {
            x,
            y,
            hp: full,
            full,
            prov: seats.clamp(1, MAX_PLAYERS) as u8,
            phase: 0,
            st: Ws::Asleep,
            t: 0,
            atk: Atk::Slam,
            n: 0,
            danger: Danger::None,
            aim: 90,
            sweep: 0,
            lit: [0; 6],
            hit: [0; 6],
            struck: 0,
            dead: 0,
            start: full,
            face: (0, Q),
            flinch: 0,
        }
    }

    pub fn radius(&self) -> i32 {
        WARDEN_R * Q
    }

    /// Whether every number is one a save or the wire may carry.
    pub fn valid(&self) -> bool {
        self.hp <= self.full
            && self.full <= WARDEN_HP * MAX_PLAYERS
            && (1..=MAX_PLAYERS as u8).contains(&self.prov)
            && self.phase <= 2
            && self.t <= 600
            && (-360..=360).contains(&self.sweep)
            && dist(self.face.0, self.face.1) <= Q + 8
            && self.dead <= 600
            && self.start <= self.full
    }

    /// The lamp-heart's place: in its chest, toward where it faces.
    pub fn heart(&self) -> (i32, i32) {
        (self.x + self.face.0 * 9, self.y + self.face.1 * 9)
    }

    /// How many knights' light the shutter wants.
    pub fn need(&self, seats: u32) -> u32 {
        if self.phase >= 1 { seats.min(2) } else { 1 }
    }

    /// Whether the shutter is open at tick `now`.
    pub fn exposed(&self, now: u32, seats: u32) -> bool {
        if self.st == Ws::Stagger {
            return true;
        }
        let lit = self.lit.iter().filter(|&&t| t > 0 && t + EXPOSE_TICKS > now).count() as u32;
        lit >= self.need(seats)
    }

    /// How long the shutter has left open, 0 if shut.
    pub fn open_for(&self, now: u32, seats: u32) -> u32 {
        if !self.exposed(now, seats) {
            return 0;
        }
        if self.st == Ws::Stagger {
            return u32::from(self.t);
        }
        self.lit
            .iter()
            .filter(|&&t| t > 0)
            .map(|&t| (t + EXPOSE_TICKS).saturating_sub(now))
            .min()
            .unwrap_or(0)
    }
}

impl Session {
    /// Light fell on the lamp-heart from seat `owner`.
    pub(super) fn light_heart(&mut self, owner: u32) {
        let now = self.elapsed;
        if let Some(w) = self.boss.as_mut()
            && (1..=MAX_PLAYERS).contains(&owner)
        {
            w.lit[owner as usize - 1] = now;
        }
    }

    pub fn boss_open(&self) -> bool {
        self.boss_exposed()
    }

    pub(super) fn boss_exposed(&self) -> bool {
        self.boss
            .as_ref()
            .is_some_and(|w| w.exposed(self.elapsed, self.seats.len() as u32))
    }

    /// A blade or a pot on the Warden. Only the open heart can be hurt.
    pub(super) fn hurt_boss(&mut self, dmg: u32, from: (i32, i32), _owner: u32, ev: &mut Events) -> bool {
        if !self.boss_exposed() {
            let (bx, by) = self.boss.as_ref().map_or((0, 0), |w| (w.x, w.y));
            let _ = from;
            self.fx(Fxk::Ring, bx + (from.0 - bx) / 6, by + (from.1 - by) / 6, 12);
            ev.sound("clang");
            return false;
        }
        let Some(w) = self.boss.as_mut() else { return false };
        if w.hp == 0 || w.st == Ws::Dying {
            return false;
        }
        let dmg = if w.st == Ws::Stagger { dmg * 2 } else { dmg };
        w.hp = w.hp.saturating_sub(dmg);
        w.flinch = FLINCH_TICKS;
        let (hx, hy) = (w.x, w.y);
        let before = w.phase;
        let third = w.full / 3;
        let phase = if w.hp <= third {
            2
        } else if w.hp <= third * 2 {
            1
        } else {
            0
        };
        if w.hp == 0 {
            w.st = Ws::Dying;
            w.dead = 0;
            w.danger = Danger::None;
            ev.sound("boss_death");
            ev.stop(Stop::Boss);
            self.fx(Fxk::Puff, hx, hy, 0);
            self.shake = SHAKE_HEAVY * 2;
            // Its summons go out with it.
            for f in &mut self.foes {
                if f.alive() {
                    f.hp = 0;
                    f.st = Fs::Dying;
                    f.dead = 0;
                }
            }
            return true;
        }
        if phase > before {
            w.phase = phase;
            w.start = w.hp;
            // A new phase: it rears, the shutter slams.
            w.st = Ws::Recover;
            w.t = 48;
            w.danger = Danger::None;
            w.lit = [0; 6];
            ev.sound("boss_rage");
            if phase == 2 {
                // The Gloam falls: every lamp in the arena is snuffed.
                for q in self.props.iter_mut().filter(|q| q.kind == Pk::Brazier) {
                    q.on = 0;
                    q.t = 0;
                }
                ev.sound("step_ash");
            }
            ev.stop(Stop::Phase);
            self.shake = SHAKE_HEAVY;
            self.flash = self.flash.max(6);
        } else {
            ev.sound("hit_axe");
        }
        let (hx, hy) = self.boss.as_ref().map_or((0, 0), |w| w.heart());
        self.fx(Fxk::Spark, hx, hy, 0);
        true
    }

    /// A bomb's blast near the Warden: it counts only on an open heart.
    pub(super) fn blast_boss(&mut self, at: (i32, i32), ev: &mut Events) {
        let Some(w) = self.boss.as_ref() else { return };
        if dist(w.x - at.0, w.y - at.1) <= BLAST_R * Q + w.radius() {
            self.hurt_boss(BLAST_DAMAGE, at, 0, ev);
        }
    }

    /// A retry puts the Warden back at the start of the phase it was in, at
    /// the health the table gave it (a leaver does not make it weaker).
    pub(super) fn reset_boss_phase(&mut self) {
        let (px, py) = plan(4)
            .find(b'W')
            .first()
            .map_or((0, 0), |&(tx, ty)| (middle(tx), middle(ty)));
        if let Some(w) = self.boss.as_mut()
            && w.hp > 0
        {
            w.hp = w.start.min(w.full).max(1);
            w.st = Ws::Rest;
            w.t = 60;
            w.x = px;
            w.y = py;
            w.danger = Danger::None;
            w.lit = [0; 6];
            w.struck = 0;
            w.n = 0;
        }
    }

    /// One tick of the Warden.
    pub(super) fn step_boss(&mut self, r: &Rules, ev: &mut Events) {
        let seats = self.seats.len() as u32;
        let now = self.elapsed;
        // Flares held on the heart keep it lit.
        if let Some(w) = &self.boss {
            let (hx, hy) = w.heart();
            let lit: Vec<u32> = self
                .seats
                .iter()
                .filter(|(_, k)| {
                    k.flaring()
                        && k.up()
                        && dist(k.x - hx, k.y - hy) <= HEART_FLARE_R * Q
                        && self.line_open((k.x, k.y), (hx, hy))
                })
                .map(|(&id, _)| id)
                .collect();
            for id in lit {
                self.light_heart(id);
            }
        }
        let Some(mut w) = self.boss.take() else { return };
        w.flinch = w.flinch.saturating_sub(1);
        let target = self.nearest_knight(w.x, w.y);
        match w.st {
            Ws::Dying => {
                w.dead += 1;
                if w.dead == 40 {
                    self.drop_at(Dk::Container, w.x, w.y);
                    self.stage = 2;
                }
            }
            Ws::Asleep => {
                let woke = target.is_some_and(|(_, _, _, d)| d < WARDEN_WAKE * Q) || self.stage_t > WARDEN_NAP;
                if woke {
                    w.st = Ws::Rest;
                    w.t = 70;
                    self.stage = 1;
                    ev.sound("boss_roar");
                    self.shake = SHAKE_HEAVY;
                }
            }
            Ws::Rest => {
                if let Some((_, tx, ty, d)) = target {
                    w.face = unit(tx - w.x, ty - w.y);
                    // Close the gap, never into the knight's face.
                    if d > w.radius() + 26 * Q {
                        let (nx, ny) = (w.x + w.face.0 * WARDEN_WALK / Q, w.y + w.face.1 * WARDEN_WALK / Q);
                        if self.clear(nx, ny, w.radius() / 2) {
                            w.x = nx;
                            w.y = ny;
                        }
                    }
                    w.t = w.t.saturating_sub(1);
                    if w.t == 0 {
                        self.begin_attack(&mut w, (tx, ty), r);
                    }
                }
            }
            Ws::Tell => {
                w.t = w.t.saturating_sub(1);
                if w.t == 0 {
                    w.st = Ws::Strike;
                    w.struck = 0;
                    w.t = match w.atk {
                        Atk::Beam => BEAM_TICKS,
                        Atk::Slam => SLAM_TICKS,
                        Atk::Summon => 12,
                    };
                    w.sweep = 0;
                    if w.atk == Atk::Summon {
                        self.summon(&w, ev);
                    }
                    ev.sound(match w.atk {
                        Atk::Beam => "shot_arcane",
                        Atk::Slam => "rock_land",
                        Atk::Summon => "crack_open",
                    });
                }
            }
            Ws::Strike => {
                let total = match w.atk {
                    Atk::Beam => BEAM_TICKS,
                    Atk::Slam => SLAM_TICKS,
                    Atk::Summon => 12,
                };
                let elapsed = i32::from(total - w.t);
                let mut parried = false;
                match w.atk {
                    Atk::Beam => {
                        let span = BEAM_HALF * 2;
                        let angle = w.aim - BEAM_HALF + span * elapsed / i32::from(BEAM_TICKS);
                        let beams = if w.phase >= 2 { 2 } else { 1 };
                        for b in 0..beams {
                            let a = angle + 180 * b;
                            let d = dir(a);
                            let tip = (w.x + d.0 * BEAM_LEN, w.y + d.1 * BEAM_LEN);
                            let line = Danger::Capsule { ax: w.x, ay: w.y, bx: tip.0, by: tip.1, r: 6 * Q };
                            if b == 0 {
                                w.danger = line;
                            }
                            parried |= self.warden_hits(&mut w, line, BEAM_DAMAGE, Hit::Beam, r, ev);
                        }
                    }
                    Atk::Slam => {
                        let radius = (12 + SLAM_SPEED * elapsed) * Q;
                        let ring = Danger::Ring { x: w.x, y: w.y, r: radius, w: 6 * Q };
                        w.danger = Danger::Circle { x: w.x, y: w.y, r: SLAM_REACH * Q };
                        parried |= self.warden_hits(&mut w, ring, SLAM_DAMAGE, Hit::Low, r, ev);
                        if elapsed == 0 {
                            self.fx(Fxk::Wave, w.x, w.y, 0);
                        }
                    }
                    Atk::Summon => {}
                }
                if parried {
                    w.st = Ws::Stagger;
                    w.t = STAGGER_BOSS;
                    w.danger = Danger::None;
                    w.lit = [0; 6];
                    ev.sound("boss_rage");
                    ev.stop(Stop::Phase);
                    self.shake = SHAKE_HEAVY;
                } else {
                    w.t = w.t.saturating_sub(1);
                    if w.t == 0 {
                        w.st = Ws::Recover;
                        w.t = if w.atk == Atk::Summon { 30 } else { RECOVER_TICKS };
                        w.danger = Danger::None;
                    }
                }
            }
            Ws::Recover => {
                w.t = w.t.saturating_sub(1);
                if w.t == 0 {
                    w.st = Ws::Rest;
                    w.t = REST_TICKS - 12 * u16::from(w.phase);
                    w.n = w.n.wrapping_add(1);
                }
            }
            Ws::Stagger => {
                w.t = w.t.saturating_sub(1);
                if w.t == 0 {
                    w.st = Ws::Rest;
                    w.t = 30;
                }
            }
        }
        // It is a body: a knight cannot stand in it.
        if w.st != Ws::Dying {
            let ids: Vec<u32> = self.seats.keys().copied().collect();
            for id in ids {
                let k = self.seats.get_mut(&id).unwrap();
                let (dx, dy) = (k.x - w.x, k.y - w.y);
                let d = dist(dx, dy);
                let min = w.radius() + KNIGHT_R;
                if k.up() && d < min && d > 0 {
                    let u = unit(dx, dy);
                    k.x = w.x + u.0 * min / Q;
                    k.y = w.y + u.1 * min / Q;
                }
            }
        }
        let _ = (seats, now);
        self.boss = Some(w);
    }

    /// Choose the Warden's next attack and show where it will land.
    fn begin_attack(&mut self, w: &mut Warden, at: (i32, i32), r: &Rules) {
        let seats = self.seats.len() as u32;
        let pattern: &[Atk] = match w.phase {
            0 => &[Atk::Slam, Atk::Beam, Atk::Beam, Atk::Slam],
            1 => &[Atk::Beam, Atk::Summon, Atk::Slam, Atk::Beam],
            _ => &[Atk::Beam, Atk::Slam, Atk::Summon, Atk::Beam, Atk::Slam],
        };
        w.atk = pattern[usize::from(w.n) % pattern.len()];
        w.st = Ws::Tell;
        w.t = r.tell_for(seats) as u16;
        w.face = unit(at.0 - w.x, at.1 - w.y);
        w.struck = 0;
        match w.atk {
            Atk::Beam => {
                w.aim = angle_of(at.0 - w.x, at.1 - w.y);
                w.danger = Danger::Fan { x: w.x, y: w.y, deg: w.aim, half: BEAM_HALF + 6, r: BEAM_LEN * Q };
            }
            Atk::Slam => {
                w.danger = Danger::Circle { x: w.x, y: w.y, r: SLAM_REACH * Q };
            }
            Atk::Summon => {
                w.danger = Danger::None;
            }
        }
    }

    /// The Warden's blow on every knight in `line` it has not yet struck. A
    /// parry returns true.
    fn warden_hits(&mut self, w: &mut Warden, line: Danger, dmg: u32, kind: Hit, r: &Rules, ev: &mut Events) -> bool {
        let mut parried = false;
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        for id in ids {
            let Some(k) = self.seats.get(&id) else { continue };
            let bit = 1u8 << (id.min(7) as u8);
            if w.struck & bit != 0 || !k.up() || !line.hits(k.x, k.y, KNIGHT_R) {
                continue;
            }
            match self.blow(id, dmg, (w.x, w.y), kind, r, ev) {
                Blow::Dodged => {}
                Blow::Parried => {
                    w.struck |= bit;
                    parried = true;
                }
                _ => w.struck |= bit,
            }
        }
        parried
    }

    /// Snuffers climb out of the fissures (hooded, in the last phase).
    fn summon(&mut self, w: &Warden, ev: &mut Events) {
        let seats = self.seats.len().max(1) as u32;
        let n = (2 + (seats - 1) / 2).min(4) as usize;
        let hooded = w.phase >= 2;
        let marks = plan(4).find(b'f');
        for (tx, ty) in marks.into_iter().take(n) {
            self.spawn_foe(Fk::Snuffer, middle(tx), middle(ty), hooded);
            if let Some(i) = self.prop_at(Pk::Fissure, tx, ty) {
                self.props[i].t = 40;
            }
            if let Some(f) = self.foes.last_mut() {
                f.st = Fs::Drift;
                f.t = 30;
            }
        }
        let _ = ev;
    }

    fn prop_at(&self, kind: Pk, tx: i32, ty: i32) -> Option<usize> {
        self.props
            .iter()
            .position(|p| p.kind == kind && i32::from(p.tx) == tx && i32::from(p.ty) == ty)
    }

    /// The last room: the heart container, then the Beacon.
    pub(super) fn stage_warden(&mut self, _r: &Rules, ev: &mut Events) {
        if self.stage == 3 {
            let holders = self
                .seats
                .values()
                .filter(|k| k.up() && k.act == Act::Free && k.held & super::kit::B_CTX != 0 && k.ctx == Ctx::Relight)
                .count() as u16;
            if let Some(i) = self.props.iter().position(|p| p.kind == Pk::Beacon) {
                let p = &mut self.props[i];
                if holders > 0 {
                    p.t = p.t.saturating_add(holders);
                } else {
                    p.t = p.t.saturating_sub(1);
                }
                if p.t >= BEACON_TICKS && p.on == 0 {
                    p.on = 1;
                    let (x, y) = (p.x, p.y);
                    self.stage = 4;
                    self.done = Some(self.elapsed);
                    self.flash = 12;
                    self.shake = SHAKE_HEAVY;
                    self.fx(Fxk::Solved, x, y, 1);
                    ev.sound("circle_toll");
                    ev.sound("wish_grant");
                    ev.won = true;
                }
            }
        }
    }
}
