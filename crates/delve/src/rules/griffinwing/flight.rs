//! The craft in the air: steering inside the window with a banking lean, the
//! free reticle that leads the shot, the tap and the hold-to-charge lock,
//! the roll, the spur and the brake, Starfall, and the wingmen who fly the
//! empty seats. Whole numbers, 256 to the unit, on the 30 Hz sim.
use super::*;

/// The keys, a bit each, for the presses (a key held across a tick is not a
/// new press).
pub const K_FIRE: u8 = 1;
pub const K_DASH: u8 = 2;
pub const K_BOMB: u8 = 4;
pub const K_SPUR: u8 = 8;

pub fn bits(i: Input) -> u8 {
    u8::from(i.fire)
        | u8::from(i.dash) << 1
        | u8::from(i.bomb) << 2
        | u8::from(i.interact) << 3
        | u8::from(i.swing) << 4
}

/// A seat's place in the formation: across and up, tenths of a unit.
pub fn slot(id: u32) -> (i32, i32) {
    match id {
        1 => (0, 0),
        2 => (dm(-22), dm(-4)),
        3 => (dm(22), dm(-4)),
        4 => (dm(-40), dm(-8)),
        5 => (dm(40), dm(-8)),
        _ => (0, dm(12)),
    }
}

/// A rider's place: across, up and ahead of the knights, so that what hangs
/// on a rider's tail is in front of a knight behind.
pub fn rider_slot(k: usize) -> (i32, i32, i32) {
    match k {
        0 => (dm(-30), dm(8), dm(60)),
        1 => (dm(30), dm(8), dm(60)),
        2 => (0, dm(18), dm(78)),
        _ => (dm(-62), dm(2), dm(50)),
    }
}

/// How fast a craft steers, across and up, and how quickly it gets there
/// (units a tick, in Q).
const V_X: i32 = 62;
const V_Z: i32 = 50;
const ACC: i32 = 14;
/// The reticle's pace, and how far it ranges from its craft at the far plane.
pub const RET_V: i32 = 100;
pub const RX_MAX: i32 = dm(120);
pub const RZ_MAX: i32 = dm(80);
/// A tap's rest, how long F is held before it is a charge, the rest after a
/// release.
const TAP_REST: u32 = 6;
const HOLD_AT: u32 = 7;
const REL_REST: u32 = 12;
/// How near the reticle's ray must pass a body to lock it, beyond its size.
pub const LOCK_R: i32 = dm(16);
/// The bolts' pace along their way (relative to the rail) and how long they
/// fly.
const V_BOLT: i32 = dm(22);
const V_STAR: i32 = dm(26);
const BOLT_LIFE: u16 = 26;
const STAR_LIFE: u16 = 46;
/// What a bolt and a starburst do: a starburst's least and its most (a full
/// charge).
pub const LANCE: u16 = 10;
pub const STAR_MIN: u32 = 16;
pub const STAR_MAX: u32 = 36;
/// The spur and the brake: how far they slide a craft along the rail, and
/// how quickly.
const BOOST_BACK: i32 = dm(55);
const BRAKE_BACK: i32 = dm(45);
const BACK_V: i32 = 28;
/// A roll's rest after it ends, and how hard it throws a craft aside.
const ROLL_REST: u32 = 12;
const ROLL_V: i32 = 84;
/// The window in which a second tap is a roll.
const TAP_WINDOW: u8 = 9;
/// The craft's body, for the bolts that hit it.
pub const CRAFT_R: i32 = dm(10);
/// How far from a craft a rolling shield turns a bolt round.
pub const REFLECT_R: i32 = dm(26);
/// What Starfall does to every foe it reaches.
pub const STARFALL: u32 = 120;

impl Craft {
    /// Whether its roll turns bolts round just now.
    pub fn reflecting(&self, reflect: u32) -> bool {
        self.roll > 0 && self.roll_len - self.roll < reflect
    }
}

impl Session {
    /// The rail point a craft is at.
    pub fn craft_s(&self, c: &Craft) -> i32 {
        self.anchor() + c.back
    }

    /// A craft by the id a bolt or a foe holds: a seat's, a rider's. Its
    /// place along, across and up, and whether it is flying.
    pub fn craft_at(&self, id: u8) -> Option<(i32, i32, i32, bool)> {
        let c = if (1..=6).contains(&id) {
            &self.seats.get(&u32::from(id))?.craft
        } else {
            let w = self.wing.iter().find(|w| w.id == id)?;
            return Some((
                self.craft_s(&w.craft),
                w.craft.x,
                w.craft.z,
                !w.lost && w.craft.down == 0,
            ));
        };
        Some((self.craft_s(c), c.x, c.z, c.flying()))
    }

    /// One knight's tick in the air.
    pub(super) fn fly(&mut self, id: u32, i: Input, n: &Nums) {
        let Some(seat) = self.seats.get(&id) else {
            return;
        };
        let mut c = seat.craft.clone();
        let old = c.keys;
        let edge = |key: u8, on: bool| on && old & key == 0;
        let (wx, wz) = (n.wx(), n.wz());
        c.hurt = c.hurt.saturating_sub(1);
        c.cool = c.cool.saturating_sub(1);
        c.rest = c.rest.saturating_sub(1);
        c.chain_ticks = c.chain_ticks.saturating_sub(1);
        if c.chain_ticks == 0 {
            c.chain = 0;
        }
        c.tap_ticks = c.tap_ticks.saturating_sub(1);
        if c.down > 0 {
            c.down -= 1;
            c.ds = PACE;
            c.held = 0;
            c.charge = 0;
            c.locks.clear();
            if c.down == 0 {
                // Back in the wing, mended a little, unhurt for a moment.
                let (x, z) = slot(id);
                c.x = x;
                c.z = z;
                c.vx = 0;
                c.vz = 0;
                c.shield = 60;
                c.hurt = 2 * HZ;
                c.roll = 0;
            }
            if let Some(s) = self.seats.get_mut(&id) {
                s.craft = c;
            }
            return;
        }
        // A second tap of a direction is a roll that way.
        let mut double = false;
        if i.move_x != 0 && c.last_x != i.move_x {
            if c.tap_dir == i.move_x && c.tap_ticks > 0 {
                double = true;
                c.tap_ticks = 0;
            } else {
                c.tap_dir = i.move_x;
                c.tap_ticks = TAP_WINDOW;
            }
        }
        c.last_x = i.move_x;
        // The roll: a spinning shield, and a throw to the side.
        if (edge(K_DASH, i.dash) || double) && c.rest == 0 && c.roll == 0 {
            c.dir = if i.move_x != 0 {
                i.move_x.signum()
            } else if c.vx.abs() > 8 {
                c.vx.signum() as i8
            } else {
                c.dir
            };
            c.roll = n.roll;
            c.roll_len = n.roll;
            c.rest = n.roll + ROLL_REST;
            c.vx = i32::from(c.dir) * ROLL_V;
            c.held = 0;
            c.charge = 0;
            c.locks.clear();
            self.sound("roll");
        }
        c.roll = c.roll.saturating_sub(1);
        // The reins.
        let (tx, tz) = (i32::from(i.move_x) * V_X, -i32::from(i.move_y) * V_Z);
        c.vx += (tx - c.vx).clamp(-ACC, ACC);
        c.vz += (tz - c.vz).clamp(-ACC, ACC);
        c.x += c.vx;
        c.z += c.vz;
        if c.x.abs() > wx {
            c.x = wx * c.x.signum();
            c.vx = 0;
        }
        if c.z.abs() > wz {
            c.z = wz * c.z.signum();
            c.vz = 0;
        }
        // The lean follows the sideways speed a little late.
        c.bank += (c.vx * Q / V_X - c.bank) / 4;
        c.lift += (c.vz * Q / V_Z - c.lift) / 4;
        // The spur on a short meter, and the brake.
        let drain = METER / n.spur.max(1) as i32;
        let refill = METER / (n.spur.max(1) as i32 * 5 / 2).max(1);
        c.boost = i.interact && !c.spent && c.meter > 0 && c.roll == 0;
        c.brake = i.swing && !c.boost;
        if c.boost {
            c.meter = (c.meter - drain).max(0);
            if c.meter == 0 {
                c.spent = true;
            }
        } else {
            c.meter = (c.meter + refill).min(METER);
            if c.spent && c.meter >= METER / 4 {
                c.spent = false;
            }
        }
        let want = if c.boost {
            BOOST_BACK
        } else if c.brake {
            -BRAKE_BACK
        } else {
            0
        };
        let back = c.back + (want - c.back).clamp(-BACK_V, BACK_V);
        c.ds = PACE + back - c.back;
        c.back = back;
        // The reticle: instant, and it stays where it is put.
        c.rx = (c.rx + i32::from(i.aim_x) * RET_V).clamp(-RX_MAX, RX_MAX);
        c.rz = (c.rz - i32::from(i.aim_y) * RET_V).clamp(-RZ_MAX, RZ_MAX);
        let s_c = self.craft_s(&c);
        // Fire: a tap is a twin bolt; a hold charges and locks; letting go
        // sends the locks.
        if c.roll == 0 && i.fire {
            if edge(K_FIRE, true) {
                c.held = 1;
                if c.cool == 0 {
                    self.tap(id as u8, &c, s_c);
                    c.cool = TAP_REST;
                }
            } else if c.held > 0 {
                c.held = (c.held + 1).min(30_000);
                if c.held > HOLD_AT {
                    c.charge = (c.charge + 1).min(n.charge);
                    self.acquire(&mut c, s_c);
                }
            }
        } else if c.held > 0 {
            if c.charge > 0 && c.roll == 0 {
                self.release(id as u8, &mut c, s_c, n);
            }
            c.held = 0;
            c.charge = 0;
            c.dwell = 0;
            c.locks.clear();
        }
        // Starfall.
        if edge(K_BOMB, i.bomb) && c.bombs > 0 && c.roll == 0 {
            c.bombs -= 1;
            self.starfall(id as u8, s_c, c.x, c.z);
        }
        if let Some(s) = self.seats.get_mut(&id) {
            s.craft = c;
        }
    }

    /// A tap: two bolts, side by side, both to where the reticle points.
    fn tap(&mut self, owner: u8, c: &Craft, s_c: i32) {
        let muzzle = s_c + dm(12);
        let to = (s_c + FAR, c.x + c.rx, c.z + c.rz);
        for side in [-1, 1] {
            let from = (muzzle, c.x + side * dm(4), c.z);
            let v = scale3((to.0 - from.0, to.1 - from.1, to.2 - from.2), V_BOLT);
            self.push_bolt(Bolt {
                shot: Arm::Lance,
                owner,
                s: from.0,
                x: from.1,
                z: from.2,
                vs: PACE + v.0,
                vx: v.1,
                vz: v.2,
                dmg: LANCE,
                life: BOLT_LIFE,
                aim: 0,
                src: 0,
                delay: 0,
                full: false,
            });
        }
        self.sound("shot_bolt");
    }

    /// Everything the reticle sweeps over while F is held is locked: a
    /// bracket on each, snapping on. A great body (a lamp) takes up to three
    /// if the reticle dwells on it.
    fn acquire(&mut self, c: &mut Craft, s_c: i32) {
        let alive: Vec<(u16, Kind, i32, i32, i32, bool)> = self
            .foes
            .iter()
            .map(|f| (f.id, f.kind, f.s, f.x, f.z, f.lockable()))
            .collect();
        c.locks.retain(|l| {
            alive.iter().any(|&(id, _, s, _, _, ok)| {
                id == l.foe && ok && (LOCK_MIN..=LOCK_MAX + dm(40)).contains(&(s - s_c))
            })
        });
        let mut total: usize = c.locks.iter().map(|l| usize::from(l.n)).sum();
        let mut dwelt = false;
        for &(id, kind, s, x, z, ok) in &alive {
            let ds = s - s_c;
            if !ok || !(LOCK_MIN..=LOCK_MAX).contains(&ds) {
                continue;
            }
            // The reticle's ray at that depth, against the body.
            let ax = (i64::from(c.rx) * i64::from(ds) / i64::from(FAR)) as i32;
            let az = (i64::from(c.rz) * i64::from(ds) / i64::from(FAR)) as i32;
            if flat(x - c.x, z - c.z, ax, az) > i64::from(kind.radius() + LOCK_R) {
                continue;
            }
            match c.locks.iter_mut().find(|l| l.foe == id) {
                Some(l) => {
                    if kind == Kind::Lamp && l.n < MAX_PER_BODY && total < MAX_LOCKS {
                        dwelt = true;
                        c.dwell += 1;
                        if c.dwell >= 8 {
                            l.n += 1;
                            c.dwell = 0;
                            total += 1;
                            self.sound("spoil_pickup");
                        }
                    }
                }
                None => {
                    if total < MAX_LOCKS {
                        c.locks.push(Lock { foe: id, n: 1 });
                        total += 1;
                        c.dwell = 0;
                        self.sound("spoil_pickup");
                        self.spark(Burst::Lock, s, x, z, total as u8);
                    }
                }
            }
        }
        if !dwelt {
            c.dwell = 0;
        }
    }

    /// Letting go: a starburst for every lock, a chain of pops two ticks
    /// apart, homing on their bodies.
    fn release(&mut self, owner: u8, c: &mut Craft, s_c: i32, n: &Nums) {
        let full = c.charge >= n.charge;
        let dmg = STAR_MIN + (STAR_MAX - STAR_MIN) * c.charge.min(n.charge) / n.charge.max(1);
        let mut k = 0i32;
        let locks = c.locks.clone();
        for l in &locks {
            for _ in 0..l.n {
                let spread =
                    [(-70, 40), (70, 40), (0, 90), (-40, -40), (40, -40), (0, 0)][(k % 6) as usize];
                self.push_bolt(Bolt {
                    shot: Arm::Star,
                    owner,
                    s: s_c + dm(12),
                    x: c.x,
                    z: c.z,
                    vs: PACE + 120,
                    vx: spread.0,
                    vz: spread.1,
                    dmg: dmg as u16,
                    life: STAR_LIFE,
                    aim: l.foe,
                    src: 0,
                    delay: (k * 2) as u8,
                    full,
                });
                k += 1;
            }
        }
        if k > 0 {
            c.cool = REL_REST;
            self.sound("shot_spark");
        }
    }

    pub(super) fn push_bolt(&mut self, b: Bolt) {
        if self.bolts.len() < MAX_BOLTS {
            self.bolts.push(b);
        }
    }

    /// The bolts' home pace (starbursts steer at it).
    pub fn star_speed() -> i32 {
        V_STAR
    }

    /// The wingmen: formation flying with a little drift, a bolt now and
    /// then at what is nearest ahead.
    pub(super) fn wing_fly(&mut self, _n: &Nums) {
        let anchor = self.anchor();
        let t = self.t;
        let targets: Vec<(i32, i32, i32)> = self
            .foes
            .iter()
            .filter(|f| f.lockable() && f.kind != Kind::Lamp)
            .map(|f| (f.s, f.x, f.z))
            .collect();
        for k in 0..self.wing.len() {
            let mut w = self.wing[k].clone();
            if w.lost {
                // Falling away, out of the picture.
                w.craft.z = (w.craft.z - 20).max(-dm(110));
                self.wing[k] = w;
                continue;
            }
            let (sx, sz, back) = rider_slot(k);
            let kk = k as u32;
            let tx = sx + tri(t + 41 * kk, 130 + 16 * kk, dm(16));
            let tz = sz + tri(t + 29 * kk, 96 + 12 * kk, dm(6));
            let (ox, oz) = (w.craft.x, w.craft.z);
            w.craft.x += (tx - w.craft.x) / 8;
            w.craft.z += (tz - w.craft.z) / 8;
            w.craft.vx = w.craft.x - ox;
            w.craft.vz = w.craft.z - oz;
            w.craft.bank += (w.craft.vx * Q / V_X - w.craft.bank) / 4;
            w.craft.lift += (w.craft.vz * Q / V_Z - w.craft.lift) / 4;
            // A rider coming back up from a fall leans no further than a knight.
            w.craft.bank = w.craft.bank.clamp(-Q, Q);
            w.craft.lift = w.craft.lift.clamp(-Q, Q);
            w.craft.back = back;
            w.craft.hurt = w.craft.hurt.saturating_sub(1);
            w.fire = w.fire.saturating_sub(1);
            let me = (anchor + back, w.craft.x, w.craft.z);
            if w.fire == 0 {
                // The nearest foe ahead, within range.
                let pick = targets
                    .iter()
                    .filter(|t| (dm(60)..=dm(400)).contains(&(t.0 - me.0)))
                    .min_by_key(|t| norm((t.0 - me.0, t.1 - me.1, t.2 - me.2)));
                if let Some(&to) = pick {
                    let v = scale3((to.0 - me.0, to.1 - me.1, to.2 - me.2), V_BOLT);
                    let owner = w.id;
                    self.push_bolt(Bolt {
                        shot: Arm::Ally,
                        owner,
                        s: me.0 + dm(12),
                        x: me.1,
                        z: me.2,
                        vs: PACE + v.0,
                        vx: v.1,
                        vz: v.2,
                        dmg: 6,
                        life: BOLT_LIFE,
                        aim: 0,
                        src: 0,
                        delay: 0,
                        full: false,
                    });
                    w.fire = 14 + 3 * kk;
                } else {
                    w.fire = 6;
                }
            }
            self.wing[k] = w;
        }
    }

    /// After the beat's blows: a craft with no shield falls and rises again
    /// a moment later; a rider with none is lost to the beat.
    pub(super) fn step_craft(&mut self, _n: &Nums) {
        let anchor = self.anchor();
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        for id in ids {
            let (down_now, at) = {
                let c = &self.seats[&id].craft;
                (c.shield <= 0 && c.down == 0, (anchor + c.back, c.x, c.z))
            };
            if down_now {
                if let Some(s) = self.seats.get_mut(&id) {
                    s.craft.down = RISE;
                    s.craft.shield = 0;
                    s.craft.roll = 0;
                    s.craft.held = 0;
                    s.craft.charge = 0;
                    s.craft.locks.clear();
                }
                self.spark(Burst::Fall, at.0, at.1, at.2, id as u8);
                self.sound("knight_fall");
                // Whatever hung on it lets go.
                for f in &mut self.foes {
                    if f.st == St::Latched && u32::from(f.on) == id {
                        f.st = St::Flee;
                        f.ticks = 40;
                    }
                }
            }
        }
        for k in 0..self.wing.len() {
            let (gone, at) = {
                let w = &self.wing[k];
                (
                    !w.lost && w.craft.shield <= 0,
                    (anchor + w.craft.back, w.craft.x, w.craft.z),
                )
            };
            if gone {
                let id = self.wing[k].id;
                self.wing[k].lost = true;
                self.beat.lost += 1;
                self.say(id, Line::Lost);
                self.spark(Burst::Fall, at.0, at.1, at.2, id);
                self.sound("knight_fall");
                for f in &mut self.foes {
                    if f.st == St::Latched && f.on == id {
                        f.st = St::Flee;
                        f.ticks = 40;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_triangle_wave_runs_between_its_ends() {
        let a = dm(10);
        let lo = (0..60).map(|t| tri(t, 60, a)).min().unwrap();
        let hi = (0..60).map(|t| tri(t, 60, a)).max().unwrap();
        assert!(lo <= -a + 20 && hi >= a - 20, "{lo} {hi}");
    }

    #[test]
    fn a_segment_hits_a_body_it_passes_through_and_misses_one_beside_it() {
        let (a, b) = ((0, 0, 0), (1000, 0, 0));
        assert!(near(a, b, (500, 100, 0), 200));
        assert!(!near(a, b, (500, 400, 0), 200));
        assert!(near(a, b, (1100, 0, 0), 200), "past the end, within reach");
        assert!(!near(a, b, (1300, 0, 0), 200));
        assert!(near(a, a, (50, 0, 0), 100), "a point is a segment too");
    }

    #[test]
    fn a_vector_is_scaled_to_its_length() {
        let v = scale3((300, 400, 0), 1000);
        assert_eq!(v, (600, 800, 0));
        assert_eq!(scale3((0, 0, 0), 5), (5, 0, 0));
    }
}
