//! The fight: the foes' flights and tells, the bolts and what they hit (a
//! swept test in rail space, never free 3D collision), the roll that turns
//! a bolt round, the kites that latch on, and Starfall.
use super::flight::{CRAFT_R, REFLECT_R, STARFALL};
use super::*;

/// How a kite closes on its victim, and how far behind the craft it hangs.
const HUNT_V: i32 = dm(10);
const LATCH_BACK: i32 = dm(18);
/// What a hit of each hostile bolt takes off a shield, and how big they are.
const GLOAM: i32 = 25;
const SPARK: i32 = 10;
const GLOAM_R: i32 = dm(6);
const SPARK_R: i32 = dm(5);
const BOLT_R: i32 = dm(5);
/// What a turned bolt does, and its pace home.
pub const TURNED: u16 = 60;
const V_TURN: i32 = dm(24);
/// A dive-bomber's blow.
const RAM: i32 = 35;
/// How long a shield stays down after a hit lands on it, in ticks.
pub const HURT: u32 = 36;

impl Session {
    /// Put a foe on the rail, its health provisioned for the knights at the
    /// table; what it is worth is offered to the medal.
    pub(super) fn spawn_foe(
        &mut self,
        kind: Kind,
        at: (i32, i32, i32),
        vel: (i32, i32, i32),
        st: St,
        mode: u8,
        phase: u8,
    ) -> Option<u16> {
        if self.foes.len() >= MAX_FOES {
            return None;
        }
        let seats = self.pressure();
        let full = kind.health() * u32::from(seats);
        let id = self.next_id();
        self.foes.push(Foe {
            id,
            kind,
            s: at.0,
            x: at.1,
            z: at.2,
            vs: vel.0,
            vx: vel.1,
            vz: vel.2,
            hp: full,
            full,
            seats,
            st,
            ticks: 0,
            fx: 0,
            fz: 0,
            on: 0,
            flash: 0,
            mode,
            phase,
        });
        self.beat.offered += kind.worth();
        Some(id)
    }

    /// A craft's shield takes a blow. A knight is spared for a moment after
    /// one; a rider is only worn down (by what latches on). Whether it
    /// landed.
    pub(super) fn hurt_craft(&mut self, id: u8, dmg: i32) -> bool {
        let at = self.craft_at(id);
        if (1..=6).contains(&id) {
            let Some(seat) = self.seats.get_mut(&u32::from(id)) else {
                return false;
            };
            let c = &mut seat.craft;
            if c.hurt > 0 || c.down > 0 {
                return false;
            }
            c.shield = (c.shield - dmg).max(0);
            c.hurt = HURT;
            if let Some((s, x, z, _)) = at {
                self.spark(Burst::Hurt, s, x, z, id);
            }
            self.sound(if dmg >= 20 {
                "knight_hit_big"
            } else {
                "hero_hurt"
            });
            if self.seats[&u32::from(id)].craft.shield <= 30 {
                self.say(id, Line::Low);
            }
            true
        } else if let Some(w) = self.wing.iter_mut().find(|w| w.id == id) {
            if w.lost {
                return false;
            }
            w.craft.shield = (w.craft.shield - dmg).max(0);
            true
        } else {
            false
        }
    }

    /// A drip from a latched kite: no spare moment, no flash.
    fn drain(&mut self, id: u8, amount: i32) {
        if (1..=6).contains(&id) {
            if let Some(s) = self.seats.get_mut(&u32::from(id)) {
                if s.craft.down == 0 {
                    s.craft.shield = (s.craft.shield - amount).max(0);
                }
            }
        } else if let Some(w) = self.wing.iter_mut().find(|w| w.id == id && !w.lost) {
            w.craft.shield = (w.craft.shield - amount).max(0);
        }
    }

    /// Whether some kite hangs on this craft.
    pub fn latched(&self, id: u8) -> bool {
        self.foes.iter().any(|f| f.st == St::Latched && f.on == id)
    }

    /// Who a hunting kite goes for: a rider not yet latched, else a knight
    /// not yet latched, else anyone flying. The same hunter always finds the
    /// same victim.
    fn victim(&self, hunter: u16) -> Option<u8> {
        let free = |id: u8| self.craft_at(id).is_some_and(|c| c.3) && !self.latched(id);
        let riders: Vec<u8> = self
            .wing
            .iter()
            .map(|w| w.id)
            .filter(|&id| free(id))
            .collect();
        if !riders.is_empty() {
            return Some(riders[usize::from(hunter) % riders.len()]);
        }
        let knights: Vec<u8> = self
            .seats
            .keys()
            .map(|&id| id as u8)
            .filter(|&id| free(id))
            .collect();
        if !knights.is_empty() {
            return Some(knights[usize::from(hunter) % knights.len()]);
        }
        self.seats
            .keys()
            .map(|&id| id as u8)
            .find(|&id| self.craft_at(id).is_some_and(|c| c.3))
    }

    /// Who a turret or the Wyrm fires at: the knights flying, in turn.
    pub(super) fn target(&self, turn: usize) -> Option<(i32, i32)> {
        let flying: Vec<(i32, i32)> = self
            .seats
            .values()
            .filter(|s| s.craft.flying())
            .map(|s| (s.craft.x, s.craft.z))
            .collect();
        if flying.is_empty() {
            None
        } else {
            Some(flying[turn % flying.len()])
        }
    }

    /// A bolt from `o` that meets the formation's plane at `to` (across, up)
    /// as the rail brings the plane to it, closing at `closing` a tick.
    pub(super) fn plane_shot(
        &self,
        o: (i32, i32, i32),
        to: (i32, i32),
        closing: i32,
    ) -> (i32, i32, i32) {
        let gap = (o.0 - self.anchor()).max(dm(40));
        let ticks = (gap / closing.max(1)).max(1);
        (PACE - closing, (to.0 - o.1) / ticks, (to.1 - o.2) / ticks)
    }

    /// The foes' tick: flights, tells, latching, dives.
    pub(super) fn step_foes(&mut self, n: &Nums) {
        let anchor = self.anchor();
        let mut foes = std::mem::take(&mut self.foes);
        for f in &mut foes {
            self.step_foe(f, anchor, n);
        }
        self.foes = foes;
        // Those that have gone by, or fled out of sight, are gone.
        self.foes.retain(|f| {
            (f.kind == Kind::Lamp)
                || (f.hp > 0 && f.s > anchor - dm(300) && f.s < anchor + dm(1500))
        });
        // A bomber that was thrown at a craft.
        self.bombers_ram();
    }

    fn step_foe(&mut self, f: &mut Foe, anchor: i32, n: &Nums) {
        let t = self.t;
        // A hit stops the foe a moment where it is, each foe by itself (the
        // shared sim never pauses): the hit lands.
        if f.flash >= 4 && matches!(f.st, St::Fly | St::Hunt) {
            return;
        }
        match (f.kind, f.st) {
            (Kind::Wisp | Kind::Kite, St::Fly) => {
                f.s += f.vs;
                f.x += f.vx;
                f.z += f.vz;
                if f.mode == 1 && (t + u32::from(f.phase)) % 34 == 0 {
                    f.vx = -f.vx;
                }
                // A shooting wisp: a short tell, then a slow spark.
                if f.mode == 3 && f.kind == Kind::Wisp {
                    f.ticks = f.ticks.saturating_sub(1);
                    let ahead = f.s - anchor;
                    if f.ticks == 0 && (dm(200)..=dm(460)).contains(&ahead) {
                        if let Some(to) = self.target(usize::from(f.id)) {
                            f.fx = to.0;
                            f.fz = to.1;
                            f.st = St::Tell;
                            f.ticks = n.tell;
                        }
                    }
                }
            }
            (Kind::Wisp, St::Tell) => {
                f.s += f.vs;
                f.x += f.vx;
                f.z += f.vz;
                f.ticks = f.ticks.saturating_sub(1);
                if f.ticks == 0 {
                    let v = self.plane_shot((f.s, f.x, f.z), (f.fx, f.fz), dm(4));
                    self.push_bolt(Bolt {
                        shot: Arm::Spark,
                        owner: 0,
                        s: f.s,
                        x: f.x,
                        z: f.z,
                        vs: v.0,
                        vx: v.1,
                        vz: v.2,
                        dmg: SPARK as u16,
                        life: 220,
                        aim: 0,
                        src: f.id,
                        delay: 0,
                        full: false,
                    });
                    f.st = St::Fly;
                    f.ticks = 100;
                }
            }
            (Kind::Kite, St::Hunt) => self.hunt(f),
            (Kind::Kite, St::Latched) => self.hang(f),
            (_, St::Flee) => {
                f.s += f.vs;
                f.x += f.vx;
                f.z += f.vz;
                f.ticks = f.ticks.saturating_sub(1);
                if f.ticks == 0 {
                    f.hp = 0;
                }
            }
            (Kind::Turret, St::Rest) => {
                f.ticks = f.ticks.saturating_sub(1);
                let ahead = f.s - anchor;
                if f.ticks == 0 && (dm(150)..=dm(560)).contains(&ahead) {
                    if let Some(to) = self.target(usize::from(f.id) + usize::from(f.phase)) {
                        f.fx = to.0;
                        f.fz = to.1;
                        f.st = St::Tell;
                        f.ticks = n.tell;
                        self.sound("vent_fire");
                    }
                }
            }
            (Kind::Turret, St::Tell) => {
                f.ticks = f.ticks.saturating_sub(1);
                if f.ticks == 0 {
                    self.fire_marks(f);
                    f.phase = f.phase.wrapping_add(1);
                    f.st = St::Rest;
                    f.ticks = 26;
                }
            }
            (Kind::Bomber, St::Hide) => {
                if f.s - anchor <= dm(330) {
                    if let Some(to) = self.target(usize::from(f.id)) {
                        f.fx = to.0;
                        f.fz = to.1;
                        f.st = St::Tell;
                        f.ticks = n.tell;
                    }
                }
            }
            (Kind::Bomber, St::Tell) => {
                // It rises over the ridge, then dives down the locked lane.
                if f.z < f.fz.max(dm(10)) + dm(24) {
                    f.z += dm(1);
                }
                f.ticks = f.ticks.saturating_sub(1);
                if f.ticks == 0 {
                    let gap = (f.s - anchor).max(dm(60));
                    let ticks = 32;
                    let closing = gap / ticks;
                    f.vs = PACE - closing;
                    f.vx = (f.fx - f.x) / ticks;
                    f.vz = (f.fz - f.z) / ticks;
                    f.st = St::Dive;
                }
            }
            (Kind::Bomber, St::Dive) => {
                f.s += f.vs;
                f.x += f.vx;
                f.z += f.vz;
            }
            _ => {}
        }
    }

    /// The marks a turret's shot will land on, across and up: where it
    /// locked, and a pair more to each side for every three knights beyond
    /// the first.
    pub fn marks(f: &Foe, seats: u8) -> Vec<(i32, i32)> {
        // A pair side by side even for one knight, and a pair more to each
        // side for every three knights beyond the first.
        let extra = i32::from(seats.saturating_sub(1)) / 3;
        let mut out = vec![(f.fx - dm(8), f.fz), (f.fx + dm(8), f.fz)];
        for k in 1..=extra {
            out.push((f.fx - dm(8) - k * dm(20), f.fz));
            out.push((f.fx + dm(8) + k * dm(20), f.fz));
        }
        out
    }

    /// A turret fires on its locked marks.
    fn fire_marks(&mut self, f: &Foe) {
        for to in Session::marks(f, self.pressure()) {
            let v = self.plane_shot((f.s, f.x, f.z + dm(12)), to, dm(5));
            self.push_bolt(Bolt {
                shot: Arm::Gloam,
                owner: 0,
                s: f.s,
                x: f.x,
                z: f.z + dm(12),
                vs: v.0,
                vx: v.1,
                vz: v.2,
                dmg: GLOAM as u16,
                life: 260,
                aim: 0,
                src: f.id,
                delay: 0,
                full: false,
            });
        }
        self.sound("shot_crossbow");
    }

    /// A kite going for its victim: it picks one, closes, and latches onto
    /// the tail.
    fn hunt(&mut self, f: &mut Foe) {
        let held = f.on != 0 && self.craft_at(f.on).is_some_and(|c| c.3);
        if !held {
            match self.victim(f.id) {
                Some(v) => f.on = v,
                None => {
                    f.st = St::Flee;
                    f.ticks = 40;
                    f.vz = 60;
                    return;
                }
            }
        }
        let Some((vs, vx, vz, _)) = self.craft_at(f.on) else {
            return;
        };
        let to = (vs - LATCH_BACK, vx, vz + dm(4));
        let d = (to.0 - f.s, to.1 - f.x, to.2 - f.z);
        if norm(d) <= i64::from(dm(9)) {
            f.st = St::Latched;
            f.ticks = 0;
            f.s = to.0;
            f.x = to.1;
            f.z = to.2;
            let who = f.on;
            self.say(who, Line::Latched);
            self.sound("clang");
        } else {
            let v = scale3(d, HUNT_V);
            f.vs = v.0;
            f.vx = v.1;
            f.vz = v.2;
            f.s += v.0;
            f.x += v.1;
            f.z += v.2;
        }
    }

    /// A latched kite hangs on its victim's tail, draining its shield, until
    /// it is shot, or shaken off by a roll.
    fn hang(&mut self, f: &mut Foe) {
        let Some((vs, vx, vz, ok)) = self.craft_at(f.on) else {
            f.st = St::Flee;
            f.ticks = 40;
            return;
        };
        let rolling = (1..=6).contains(&f.on)
            && self
                .seats
                .get(&u32::from(f.on))
                .is_some_and(|s| s.craft.roll > 0);
        if !ok || rolling {
            f.st = St::Flee;
            f.ticks = 40;
            f.vs = 0;
            f.vx = if vx >= 0 { 90 } else { -90 };
            f.vz = 70;
            return;
        }
        f.s = vs - LATCH_BACK;
        f.x = vx;
        f.z = vz + dm(4);
        f.ticks = (f.ticks + 1) % 9_996;
        if f.ticks % 12 == 0 {
            let amount = if f.on <= 6 { 2 } else { 3 };
            self.drain(f.on, amount);
        }
    }

    /// Dive-bombers meet craft: a blow for the knight, and the bomber is
    /// spent.
    fn bombers_ram(&mut self) {
        let crafts: Vec<(u8, i32, i32, i32)> = self
            .seats
            .iter()
            .filter(|(_, s)| s.craft.flying())
            .map(|(&id, s)| (id as u8, self.craft_s(&s.craft), s.craft.x, s.craft.z))
            .collect();
        let mut spent = Vec::new();
        for f in &self.foes {
            if f.kind != Kind::Bomber || f.st != St::Dive {
                continue;
            }
            let from = (f.s - f.vs, f.x - f.vx, f.z - f.vz);
            for &(id, s, x, z) in &crafts {
                if near(from, (f.s, f.x, f.z), (s, x, z), f.radius() + CRAFT_R) {
                    spent.push((f.id, id));
                    break;
                }
            }
        }
        for (fid, id) in spent {
            if self.hurt_craft(id, RAM) {
                if let Some(i) = self.foes.iter().position(|f| f.id == fid) {
                    let f = self.foes.remove(i);
                    self.spark(Burst::Pop, f.s, f.x, f.z, 0);
                    self.sound("bomb_toss");
                }
            }
        }
    }

    /// A foe takes `dmg` from `by`; whether it fell.
    pub(super) fn hit_foe(&mut self, fi: usize, dmg: u32, by: u8) -> bool {
        let f = &mut self.foes[fi];
        f.hp = f.hp.saturating_sub(dmg);
        f.flash = 5;
        if f.hp > 0 {
            self.sound("hit_magic");
            return false;
        }
        if f.kind == Kind::Lamp {
            self.break_lamp(fi, by);
            return true;
        }
        self.kill(fi, by);
        true
    }

    /// A foe pops into cold smoke and a spark of returned light. A kite that
    /// had latched on is a rescue; a knight's chain of pops is counted, and
    /// pays more the longer it runs.
    fn kill(&mut self, fi: usize, by: u8) {
        let f = self.foes.remove(fi);
        let mut gain = f.kind.worth();
        self.beat.kills += 1;
        self.beat.base += gain;
        if f.st == St::Latched {
            gain += 150;
            self.beat.saved += 1;
            self.say(f.on, Line::Thanks);
            if let Some(s) = self.seats.get_mut(&u32::from(by)) {
                s.craft.saved += 1;
            }
        }
        let mut chain = 0;
        if let Some(s) = self.seats.get_mut(&u32::from(by)) {
            let c = &mut s.craft;
            c.chain += 1;
            c.chain_ticks = 75;
            c.hits += 1;
            chain = c.chain;
            if c.chain >= 3 {
                gain += 10 * (u32::from(c.chain) - 2);
            }
            c.pts += gain;
        }
        self.beat.pts += gain;
        self.spark(Burst::Pop, f.s, f.x, f.z, chain.min(99) as u8);
        self.sound("kill");
        if chain == 5 {
            self.say(by, Line::Chain);
        }
    }

    /// A lamp's flame is out: points, a flash, and the Wyrm reels (the Wyrm's
    /// tick sees it).
    fn break_lamp(&mut self, fi: usize, by: u8) {
        let f = &mut self.foes[fi];
        f.st = St::Broken;
        f.hp = 0;
        let (s, x, z) = (f.s, f.x, f.z);
        let gain = Kind::Lamp.worth();
        self.beat.pts += gain;
        self.beat.base += gain;
        self.beat.kills += 1;
        if let Some(seat) = self.seats.get_mut(&u32::from(by)) {
            seat.craft.pts += gain;
            seat.craft.hits += 1;
        }
        self.spark(Burst::Lamp, s, x, z, 2);
        self.sound("boss_rage");
    }

    /// Starfall: every hostile bolt is cleared, and everything small on the
    /// way is struck for what it can bear.
    pub(super) fn starfall(&mut self, by: u8, s_c: i32, _x: i32, _z: i32) {
        self.spark(Burst::Starfall, s_c, 0, 0, by);
        self.sound("bomb");
        self.bolts
            .retain(|b| !matches!(b.shot, Arm::Spark | Arm::Gloam));
        let anchor = self.anchor();
        let mut i = 0;
        while i < self.foes.len() {
            let f = &self.foes[i];
            let reach = (-dm(50)..=dm(600)).contains(&(f.s - anchor));
            if reach && f.lockable() && f.kind != Kind::Lamp {
                if !self.hit_foe(i, STARFALL, by) {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
    }

    /// The bolts' tick: they fly and home, then land.
    pub(super) fn step_bolts(&mut self, n: &Nums) {
        let anchor = self.anchor();
        let count = self.bolts.len();
        type Seg = ((i32, i32, i32), (i32, i32, i32));
        let mut seg: Vec<Option<Seg>> = vec![None; count];
        let mut dead = vec![false; count];
        for i in 0..count {
            let mut b = self.bolts[i].clone();
            if b.delay > 0 {
                // A released starburst waits at its craft's muzzle for its turn.
                match self.craft_at(b.owner) {
                    Some((s, x, z, true)) => {
                        b.s = s + dm(12);
                        b.x = x;
                        b.z = z;
                        b.delay -= 1;
                    }
                    _ => dead[i] = true,
                }
                self.bolts[i] = b;
                continue;
            }
            // Homing: a starburst and a turned bolt close on their body.
            if b.aim != 0 && matches!(b.shot, Arm::Star | Arm::Turned) {
                let pick = self
                    .foes
                    .iter()
                    .find(|f| f.id == b.aim && f.alive())
                    .map(|f| (f.s, f.x, f.z));
                match pick {
                    Some(p) => {
                        let speed = if b.shot == Arm::Turned {
                            V_TURN
                        } else {
                            Session::star_speed()
                        };
                        let want = scale3((p.0 - b.s, p.1 - b.x, p.2 - b.z), speed);
                        b.vs += (want.0 - b.vs) / 3;
                        b.vx += (want.1 - b.vx) / 3;
                        b.vz += (want.2 - b.vz) / 3;
                    }
                    None => {
                        // Its body is gone: on to the nearest ahead.
                        b.aim = self
                            .foes
                            .iter()
                            .filter(|f| f.lockable() && f.kind != Kind::Lamp)
                            .filter(|f| (dm(20)..=dm(500)).contains(&(f.s - b.s)))
                            .min_by_key(|f| norm((f.s - b.s, f.x - b.x, f.z - b.z)))
                            .map_or(0, |f| f.id);
                    }
                }
            }
            let from = (b.s, b.x, b.z);
            b.s += b.vs;
            b.x += b.vx;
            b.z += b.vz;
            b.life = b.life.saturating_sub(1);
            seg[i] = Some((from, (b.s, b.x, b.z)));
            self.bolts[i] = b;
        }
        for i in 0..count {
            let Some((from, to)) = seg[i] else { continue };
            if dead[i] {
                continue;
            }
            let b = self.bolts[i].clone();
            let flies = match b.shot {
                Arm::Spark | Arm::Gloam => {
                    let mut turned = b;
                    let flies = self.hostile_bolt(&mut turned, from, to, n);
                    self.bolts[i] = turned;
                    flies
                }
                _ => self.friendly_bolt(&b, from, to, &mut dead),
            };
            if !flies {
                dead[i] = true;
            }
        }
        let mut k = 0;
        self.bolts.retain(|b| {
            let keep = !dead[k] && b.life > 0 && b.s >= anchor - dm(80) && b.s <= anchor + dm(1200);
            k += 1;
            keep
        });
    }

    /// A friendly bolt against the foes and the hostile sparks; whether it
    /// flies on.
    fn friendly_bolt(
        &mut self,
        b: &Bolt,
        from: (i32, i32, i32),
        to: (i32, i32, i32),
        dead: &mut [bool],
    ) -> bool {
        let mut best: Option<(usize, i64)> = None;
        for (i, f) in self.foes.iter().enumerate() {
            if !f.alive() || f.st == St::Hide {
                continue;
            }
            // A released starburst pops the body it was locked on, so a chain
            // of locks is a chain of pops.
            if b.shot == Arm::Star && b.aim != 0 && f.id != b.aim {
                continue;
            }
            if near(from, to, (f.s, f.x, f.z), f.radius() + BOLT_R) {
                let d = norm((f.s - from.0, f.x - from.1, f.z - from.2));
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((i, d));
                }
            }
        }
        if let Some((i, _)) = best {
            self.land(i, b);
            return false;
        }
        // A bolt shoots a snuffer's spark out of the air.
        let hit = self.bolts.iter().enumerate().position(|(j, h)| {
            !dead[j] && h.shot == Arm::Spark && near(from, to, (h.s, h.x, h.z), SPARK_R + BOLT_R)
        });
        if let Some(j) = hit {
            dead[j] = true;
            let h = &self.bolts[j];
            let (s, x, z) = (h.s, h.x, h.z);
            self.spark(Burst::Hit, s, x, z, 0);
            return false;
        }
        true
    }

    /// A friendly bolt lands on foe `i`.
    fn land(&mut self, i: usize, b: &Bolt) {
        let (kind, st, id) = (self.foes[i].kind, self.foes[i].st, self.foes[i].id);
        let at = (self.foes[i].s, self.foes[i].x, self.foes[i].z);
        if kind == Kind::Lamp {
            let active = self
                .boss
                .as_ref()
                .is_some_and(|boss| boss.lamps[usize::from(boss.phase.min(2))] == id);
            match (b.shot, st) {
                // A turned bolt opens the active lamp's cold-light shield.
                (Arm::Turned, St::Shut) if active => {
                    self.foes[i].st = St::Open;
                    self.foes[i].ticks = boss::OPEN;
                    self.spark(Burst::Lamp, at.0, at.1, at.2, 1);
                    self.sound("shield_up");
                    let who = b.owner;
                    self.say(who, Line::Open);
                }
                (Arm::Turned, St::Open) => {
                    self.foes[i].ticks = boss::OPEN;
                }
                // Only a charged starburst breaks it open.
                (Arm::Star, St::Open) if b.full && active => {
                    self.hit_foe(i, u32::from(b.dmg), b.owner);
                    self.spark(Burst::Hit, at.0, at.1, at.2, 1);
                }
                _ => {
                    self.foes[i].flash = 4;
                    self.spark(Burst::Hit, at.0, at.1, at.2, 0);
                    self.sound("clang");
                }
            }
            return;
        }
        let dmg = if b.shot == Arm::Turned {
            u32::from(TURNED)
        } else {
            u32::from(b.dmg)
        };
        self.spark(Burst::Hit, at.0, at.1, at.2, 0);
        if self.hit_foe(i, dmg, b.owner) && b.shot == Arm::Star {
            // The chain of pops counts a starburst's own kills.
        }
    }

    /// A hostile bolt against the knights: a roll's shield turns it round, a
    /// bare craft takes it. Whether it flies on.
    fn hostile_bolt(
        &mut self,
        b: &mut Bolt,
        from: (i32, i32, i32),
        to: (i32, i32, i32),
        n: &Nums,
    ) -> bool {
        let r = if b.shot == Arm::Gloam {
            GLOAM_R
        } else {
            SPARK_R
        };
        let crafts: Vec<(u32, i32, i32, i32, bool, bool)> = self
            .seats
            .iter()
            .filter(|(_, s)| s.craft.flying())
            .map(|(&id, s)| {
                (
                    id,
                    self.craft_s(&s.craft),
                    s.craft.x,
                    s.craft.z,
                    s.craft.reflecting(n.reflect()),
                    s.craft.hurt > 0,
                )
            })
            .collect();
        for (id, s, x, z, reflecting, unhurt_for_now) in crafts {
            if reflecting && near(from, to, (s, x, z), REFLECT_R + r) {
                self.turn(b, id, (s, x, z));
                return true;
            }
            if !unhurt_for_now && near(from, to, (s, x, z), CRAFT_R + r) {
                self.hurt_craft(id as u8, i32::from(b.dmg));
                self.spark(Burst::Hit, b.s, b.x, b.z, 0);
                return false;
            }
        }
        true
    }

    /// A rolling craft's shield turns a bolt round: it is the roller's now,
    /// faster, and heads for what fired it (or the Wyrm's lit lamp).
    fn turn(&mut self, b: &mut Bolt, by: u32, at: (i32, i32, i32)) {
        let target = self
            .foes
            .iter()
            .find(|f| f.id == b.src && f.alive())
            .map(|f| f.id)
            .or_else(|| {
                self.boss.as_ref().and_then(|boss| {
                    let id = boss.lamps[usize::from(boss.phase.min(2))];
                    self.foes
                        .iter()
                        .find(|f| f.id == id && f.alive())
                        .map(|f| f.id)
                })
            })
            .or_else(|| {
                self.foes
                    .iter()
                    .filter(|f| f.lockable() && f.kind != Kind::Lamp && f.s > at.0)
                    .min_by_key(|f| norm((f.s - at.0, f.x - at.1, f.z - at.2)))
                    .map(|f| f.id)
            });
        b.shot = Arm::Turned;
        b.owner = by as u8;
        b.dmg = TURNED;
        b.life = 90;
        b.aim = target.unwrap_or(0);
        let v = target
            .and_then(|id| self.foes.iter().find(|f| f.id == id))
            .map_or((V_TURN, 0, 0), |f| {
                scale3((f.s - b.s, f.x - b.x, f.z - b.z), V_TURN)
            });
        (b.vs, b.vx, b.vz) = v;
        if let Some(s) = self.seats.get_mut(&by) {
            s.craft.pts += 25;
        }
        self.beat.pts += 25;
        self.spark(Burst::Turn, b.s, b.x, b.z, by as u8);
        self.sound("shield_block");
    }
}
