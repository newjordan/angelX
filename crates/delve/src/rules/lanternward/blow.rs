//! What blows do: a foe's on a knight (the shield blocks, the shield in the
//! first few ticks parries), a knight's on a foe, and the flight of every
//! bolt: iron turned back by a shield, light thrown from a lantern.
use super::foes::*;
use super::world::*;
use super::*;

/// How a blow comes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Melee,
    Bolt,
    Beam,
    /// Along the ground: a body in the air clears it.
    Low,
}

/// What came of it for the knight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blow {
    Hit,
    Blocked,
    Parried,
    Dodged,
}

impl Session {
    /// A blow lands on knight `id`, from `from`, for `dmg` quarter hearts.
    pub(super) fn blow(
        &mut self,
        id: u32,
        dmg: u32,
        from: (i32, i32),
        kind: Hit,
        r: &Rules,
        ev: &mut Events,
    ) -> Blow {
        let Some(k) = self.seats.get(&id) else {
            return Blow::Dodged;
        };
        if !k.up() || k.inv > 0 || matches!(k.act, Act::Item | Act::Fall) {
            return Blow::Dodged;
        }
        if kind == Hit::Low && k.z >= LOW_CLEAR * Q {
            return Blow::Dodged;
        }
        let (kx, ky, face) = (k.x, k.y, k.face);
        let toward = unit(from.0 - kx, from.1 - ky);
        let guarded = k.shield > 0 && k.act == Act::Free && dot(face, toward) >= SHIELD_COS;
        if guarded {
            let parried = k.shield <= r.parry;
            let at = (kx + face.0 * SHIELD_REACH, ky + face.1 * SHIELD_REACH);
            let k = self.seats.get_mut(&id).unwrap();
            if parried {
                k.freeze = k.freeze.max(PARRY_FREEZE);
                self.fx(Fxk::Parry, at.0, at.1, 0);
                self.flash = self.flash.max(PARRY_FLASH);
                ev.sound("holy_water");
                ev.stop(Stop::Phase);
                return Blow::Parried;
            }
            let away = (-toward.0 * BLOCK_PUSH / Q, -toward.1 * BLOCK_PUSH / Q);
            k.push = away;
            k.pusht = 4;
            self.fx(Fxk::Ring, at.0, at.1, 10);
            ev.sound("shield_block");
            return Blow::Blocked;
        }
        let k = self.seats.get_mut(&id).unwrap();
        k.hp = k.hp.saturating_sub(dmg);
        k.inv = HURT_INV;
        k.ward = HURT_INV;
        k.act = if k.hp == 0 { Act::Down } else { Act::Hurt };
        k.t = 0;
        k.chain = 0;
        k.step = 0;
        k.charge = 0;
        k.shield = 0;
        k.lamp = 0;
        k.queued = false;
        k.z = 0;
        let away = unit(kx - from.0, ky - from.1);
        k.push = (away.0 * HURT_PUSH / Q, away.1 * HURT_PUSH / Q);
        k.pusht = 6;
        let down = k.hp == 0;
        if down {
            k.lock = None;
            k.rev = 0;
            if k.carry == Carry::Pot {
                k.carry = Carry::Nothing;
            }
        }
        self.fx(Fxk::Spark, kx, ky - 6 * Q, 0);
        self.shake = self.shake.max(SHAKE_HIT);
        ev.sound(if dmg >= 4 { "knight_hit_big" } else { "knight_hit" });
        if down {
            ev.sound("knight_fall");
        }
        Blow::Hit
    }

    /// A blade, a bolt or a blast on foe `i`. True when it hurt (a blade
    /// passes through a hooded snuffer; nothing hurts the turret).
    pub(super) fn hurt_foe(&mut self, i: usize, dmg: u32, from: (i32, i32), ev: &mut Events) -> bool {
        let f = &mut self.foes[i];
        if !f.alive() {
            return false;
        }
        let (x, y) = (f.x, f.y);
        match f.kind {
            Fk::Turret => {
                ev.sound("clang");
                self.fx(Fxk::Spark, x, y, 0);
                return false;
            }
            Fk::Snuffer if !f.vulnerable() => {
                // The hood: the blade passes through shadow.
                ev.sound("step_ash");
                self.fx(Fxk::Ring, x, y, 8);
                return false;
            }
            _ => {}
        }
        let dmg = if f.open {
            f.open = false;
            dmg * 2
        } else {
            dmg
        };
        f.hp = f.hp.saturating_sub(dmg);
        f.flinch = FLINCH_TICKS;
        f.freeze = HIT_FREEZE;
        let away = unit(x - from.0, y - from.1);
        f.kb = (away.0 * FOE_PUSH / Q, away.1 * FOE_PUSH / Q);
        f.kbt = 5;
        if f.st == Fs::Dormant {
            f.st = Fs::Drift;
            f.t = 20;
        }
        if f.hp == 0 {
            f.st = Fs::Dying;
            f.dead = 0;
            f.danger = Danger::None;
            f.kbt = 0;
            self.fx(Fxk::Puff, x, y, 0);
            self.fx(Fxk::Light, x, y, 0);
            self.scatter(Pk::Fissure, x, y);
            ev.sound("kill");
            ev.stop(Stop::Kill);
        } else {
            self.fx(Fxk::Spark, x, y, 0);
            ev.sound("hit_blade");
        }
        true
    }

    /// Light on foe `i`: a snuffer's hood burns off for a while, and it
    /// takes a little.
    fn light_foe(&mut self, i: usize, ev: &mut Events) {
        let f = &mut self.foes[i];
        if f.kind != Fk::Snuffer || !f.alive() {
            return;
        }
        f.lit = LIT_HOLD * 2;
        f.flinch = FLINCH_TICKS;
        if f.st == Fs::Dormant {
            f.st = Fs::Drift;
            f.t = 20;
        }
        let (x, y) = (f.x, f.y);
        self.fx(Fxk::Light, x, y, 0);
        ev.sound("hit_magic");
    }

    /// Fly every bolt one tick.
    pub(super) fn step_bolts(&mut self, r: &Rules, ev: &mut Events) {
        for i in 0..self.bolts.len() {
            if self.bolts[i].ttl == 0 {
                continue;
            }
            // Fast bolts go in two half steps so none skips a body.
            for _ in 0..2 {
                if self.bolts[i].ttl == 0 {
                    break;
                }
                self.bolts[i].x += self.bolts[i].vx / 2;
                self.bolts[i].y += self.bolts[i].vy / 2;
                self.bolt_meets(i, r, ev);
            }
            let b = &mut self.bolts[i];
            b.ttl = b.ttl.saturating_sub(1);
        }
        self.bolts.retain(|b| b.ttl > 0);
    }

    fn bolt_meets(&mut self, i: usize, r: &Rules, ev: &mut Events) {
        let b = self.bolts[i].clone();
        let (px, py) = (b.x, b.y);
        let p = plan(usize::from(self.room));
        if p.at(tile_of(px), tile_of(py)) == b'#' || p.at(tile_of(px), tile_of(py)) == 0 {
            self.bolt_ends(i, ev);
            return;
        }
        // Things that stand in the way.
        for j in 0..self.props.len() {
            let q = &self.props[j];
            if !q.blocks(px, py) || q.lets_bolts() {
                continue;
            }
            match (q.kind, b.kind) {
                (Pk::Crystal, Bk::Light | Bk::Bright) if q.on == 0 => self.light_crystal(j, ev),
                (Pk::Brazier, Bk::Light | Bk::Bright) if !q.lit() => {
                    self.props[j].on = 1;
                    let (x, y) = (self.props[j].x, self.props[j].y);
                    self.fx(Fxk::Light, x, y, 0);
                    ev.sound("spell_fire");
                }
                (Pk::Great, Bk::Light | Bk::Bright) if q.on == 0 => {
                    self.props[j].on = 1;
                    let (x, y) = (self.props[j].x, self.props[j].y);
                    self.solved(x, y, ev);
                }
                (Pk::Sigil, _) => {}
                _ => {}
            }
            self.bolt_ends(i, ev);
            return;
        }
        match b.kind {
            Bk::Iron => {
                let ids: Vec<u32> = self.seats.keys().copied().collect();
                for id in ids {
                    let Some(k) = self.seats.get(&id) else { continue };
                    if !k.up() || dist(k.x - px, k.y - py) > KNIGHT_R + 3 * Q {
                        continue;
                    }
                    match self.blow(id, BOLT_DAMAGE, (px - b.vx, py - b.vy), Hit::Bolt, r, ev) {
                        Blow::Dodged => continue,
                        Blow::Hit => {
                            self.bolts[i].ttl = 0;
                        }
                        out => self.turn_bolt(i, id, out == Blow::Parried, ev),
                    }
                    return;
                }
            }
            Bk::Light | Bk::Bright | Bk::Dim | Bk::Pot => {
                for j in 0..self.foes.len() {
                    let f = &self.foes[j];
                    if !f.alive() || dist(f.x - px, f.y - py) > f.radius() + 2 * Q {
                        continue;
                    }
                    let from = (px - b.vx, py - b.vy);
                    match b.kind {
                        Bk::Light => self.light_foe(j, ev),
                        Bk::Bright => {
                            if self.foes[j].kind == Fk::Turret {
                                let f = &mut self.foes[j];
                                f.st = Fs::Stagger;
                                f.t = TURRET_STUN;
                                f.danger = Danger::None;
                                f.open = true;
                                let (x, y) = (f.x, f.y);
                                self.fx(Fxk::Parry, x, y, 0);
                                ev.sound("clang");
                            } else {
                                self.hurt_foe(j, BRIGHT_DAMAGE, from, ev);
                            }
                        }
                        Bk::Dim => {
                            self.hurt_foe(j, DIM_DAMAGE, from, ev);
                        }
                        _ => {
                            self.hurt_foe(j, POT_DAMAGE, from, ev);
                        }
                    }
                    self.bolt_ends(i, ev);
                    return;
                }
                if let Some(w) = &self.boss
                    && w.hp > 0
                {
                    let (hx, hy) = w.heart();
                    let near_heart = dist(hx - px, hy - py) <= HEART_R * Q;
                    let near_body = dist(w.x - px, w.y - py) <= w.radius();
                    if near_heart || near_body {
                        let owner = u32::from(b.owner);
                        match b.kind {
                            Bk::Light | Bk::Bright if near_heart => self.light_heart(owner),
                            Bk::Pot => {
                                self.hurt_boss(POT_DAMAGE, (px - b.vx, py - b.vy), owner, ev);
                            }
                            _ => {}
                        }
                        self.bolt_ends(i, ev);
                    }
                }
            }
        }
    }

    /// A bolt is done: a pot breaks, light flickers, iron goes out.
    fn bolt_ends(&mut self, i: usize, ev: &mut Events) {
        let b = &mut self.bolts[i];
        b.ttl = 0;
        let (x, y, kind) = (b.x, b.y, b.kind);
        match kind {
            Bk::Pot => {
                self.fx(Fxk::Dust, x, y, 0);
                self.scatter(Pk::Pot, x, y);
                ev.sound("rock_land");
            }
            Bk::Light | Bk::Bright => self.fx(Fxk::Spark, x, y, 0),
            _ => self.fx(Fxk::Dust, x, y, 0),
        }
    }

    /// A shield turns an iron bolt: back where the knight's lock holds, else
    /// where the shield faces. A parry sends it bright and quick; a late
    /// block sends it back dim and short.
    fn turn_bolt(&mut self, i: usize, id: u32, parried: bool, ev: &mut Events) {
        let Some(k) = self.seats.get(&id) else { return };
        let aim = k
            .lock
            .and_then(|l| self.lock_point(l))
            .map(|(tx, ty)| unit(tx - k.x, ty - k.y))
            .filter(|&d| dot(d, k.face) >= REFLECT_COS)
            .unwrap_or(k.face);
        let at = (k.x + k.face.0 * SHIELD_REACH, k.y + k.face.1 * SHIELD_REACH);
        let speed = if parried { BOLT_SPEED * 3 / 2 } else { BOLT_SPEED };
        let b = &mut self.bolts[i];
        b.kind = if parried { Bk::Bright } else { Bk::Dim };
        b.owner = id as u8;
        b.x = at.0;
        b.y = at.1;
        b.vx = aim.0 * speed / Q;
        b.vy = aim.1 * speed / Q;
        b.ttl = if parried { BOLT_LIFE } else { DIM_LIFE };
        let _ = ev;
    }

    /// Bombs: tick the fuses; a burst opens cracked walls, breaks pots and
    /// grass and hurts the Gloam, never the knights.
    pub(super) fn step_fuses(&mut self, ev: &mut Events) {
        for f in &mut self.fuses {
            f.t = f.t.saturating_sub(1);
        }
        let burst: Vec<Fuse> = self.fuses.iter().filter(|f| f.t == 0).cloned().collect();
        self.fuses.retain(|f| f.t > 0);
        for f in burst {
            self.fx(Fxk::Blast, f.x, f.y, 0);
            self.shake = self.shake.max(SHAKE_HEAVY);
            ev.sound("bomb");
            ev.stop(Stop::Heavy);
            for i in 0..self.foes.len() {
                let (fx, fy) = (self.foes[i].x, self.foes[i].y);
                if self.foes[i].alive() && dist(fx - f.x, fy - f.y) <= BLAST_R * Q {
                    if self.foes[i].kind == Fk::Turret {
                        let t = &mut self.foes[i];
                        t.st = Fs::Stagger;
                        t.t = TURRET_STUN;
                        t.danger = Danger::None;
                    } else {
                        self.foes[i].lit = LIT_HOLD;
                        self.hurt_foe(i, BLAST_DAMAGE, (f.x, f.y), ev);
                    }
                }
            }
            for i in 0..self.props.len() {
                let (kind, on, x, y) = {
                    let p = &self.props[i];
                    (p.kind, p.on, p.x, p.y)
                };
                if dist(x - f.x, y - f.y) > BLAST_R * Q {
                    continue;
                }
                match kind {
                    Pk::Cracked if on == 0 => {
                        self.props[i].on = 1;
                        self.fx(Fxk::Dust, x, y, 0);
                        self.solved(x, y, ev);
                        ev.sound("crack_open");
                    }
                    Pk::Pot | Pk::Grass if on == 0 => self.shatter(i, ev),
                    _ => {}
                }
            }
            if self.boss.is_some() {
                self.blast_boss((f.x, f.y), ev);
            }
        }
    }
}
