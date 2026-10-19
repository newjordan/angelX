//! A bot's hands: what a scripted knight presses to fly Griffinwing. It reads
//! only what a player can see (the foes, the bolts coming, the footprints
//! locked for the tell) and presses only the buttons a player has: it steers
//! round what is coming and through the gates, keeps the reticle on its
//! target, taps at lone foes, holds and sweeps a flight of them and lets go,
//! rolls into a bolt that would hit it (the even seats do it first), charges
//! a lamp that is open, brakes for a shutter that is shut, and throws
//! Starfall when its shield is low and the sky is full.
use super::flight::{K_BOMB, K_DASH, K_FIRE, RET_V, RX_MAX, RZ_MAX};
use super::things::shutter_gap;
use super::*;

/// How well a bot flies: a sure hand, or a novice's (late on the roll, loose
/// on the aim, letting go of a charge too soon, missing a reflect now and
/// then): the tuning rounds fly both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skill {
    Sure,
    Novice,
}

/// A steady scatter in `-span..=span` from a knight, a tick and a salt.
fn noise(id: u32, t: u32, salt: u32, span: i32) -> i32 {
    let mut h =
        id.wrapping_mul(0x9E37_79B1) ^ t.wrapping_mul(0x85EB_CA6B) ^ salt.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 13;
    (h % (2 * span as u32 + 1)) as i32 - span
}

impl Session {
    /// What knight `id` presses this tick, with a sure hand.
    pub fn bot_input(&self, id: u32, _hero: &Hero) -> Input {
        self.bot_with(id, &Nums::default(), Skill::Sure)
    }

    /// What knight `id` presses this tick, for the numbers `n` stand at.
    pub fn bot_with(&self, id: u32, n: &Nums, skill: Skill) -> Input {
        let novice = skill == Skill::Novice;
        let Some(seat) = self.seats.get(&id) else {
            return Input::default();
        };
        let c = &seat.craft;
        if self.mode == Mode::Tally {
            // A fresh press now and then leaves.
            return Input {
                interact: self.mode_t % 2 == 0,
                ..Input::default()
            };
        }
        if self.mode == Mode::Retry || c.down > 0 {
            return Input::default();
        }
        let s_c = self.craft_s(c);
        let mut input = Input::default();
        // A kite on this knight's own tail: roll it off.
        let on_me = self.latched(id as u8);

        // Where to be: round what is coming, through a gate, away from a
        // footprint, to the middle of an opening.
        let mut want: Option<(i32, i32)> = None;
        let mut rolls = on_me && c.rest == 0 && c.roll == 0;
        let mut brake = false;
        // A friend with a kite on their tail, level with this knight: drop
        // back until the chaser is in front, and shoot it.
        if let Some(chaser) = self
            .foes
            .iter()
            .find(|f| f.st == St::Latched && f.on <= 6 && u32::from(f.on) != id && f.on != 0)
        {
            if chaser.s - s_c < dm(30) {
                brake = true;
            }
        }
        let threat = self.threat(c, s_c);
        let seek = self.reflector(id).then(|| self.intercept(s_c)).flatten();
        if let Some((t_hit, px, pz)) = seek {
            // The Wyrm's lamp is shut: be where its spit will be, and roll.
            want = Some((px, pz));
            // A novice meets the Wyrm's spit half the time.
            let willing = !novice || noise(id, self.t / 60, 5, 1) >= 0;
            rolls = willing && c.rest == 0 && c.roll == 0 && t_hit <= if novice { 6 } else { 8 };
        } else if let Some((t_hit, px, pz, gloam)) = threat {
            let reflect = c.rest == 0 && c.roll == 0;
            // The even seats roll at a Gloam bolt; the odd ones dodge it
            // unless it is on them, or no even seat is there to roll.
            let eager = self.reflector(id) || t_hit <= 6;
            let slip = novice && noise(id, self.t / 45, 9, 2) == 0;
            if reflect && gloam && eager && t_hit <= if novice { 6 } else { 9 } && !slip {
                rolls = true;
            } else if rolls {
                // Already rolling a kite off.
            } else if t_hit <= if novice { 22 } else { 40 } {
                want = Some(Session::room_from(c, px, pz, &n));
            }
        }
        if want.is_none() {
            want = self.footprint_away(c, &n);
        }
        if want.is_none() {
            want = self.boss_corridor(c, &n);
        }
        if want.is_none() {
            // The next thing on the rail to thread or take.
            let ahead = |p: &Prop| (0..=dm(300)).contains(&(p.s - s_c));
            for p in self.props.iter().filter(|p| ahead(p)) {
                match p.thing {
                    Thing::Arch if (p.s - s_c) < dm(260) => {
                        want = Some((p.x, p.z));
                        break;
                    }
                    Thing::Gate | Thing::Ring
                        if p.taken & bit(id) == 0 && (p.thing == Thing::Gate || c.shield < 80) =>
                    {
                        want = Some((p.x, p.z));
                        break;
                    }
                    Thing::Shutter if (p.s - s_c) < dm(220) => {
                        want = Some((p.x, c.z));
                        let ticks = ((p.s - s_c) / PACE).max(0) as u32;
                        if shutter_gap(p, self.t + ticks, n.wx()) < dm(16) {
                            brake = true;
                        }
                        break;
                    }
                    Thing::Ridge if (p.x - c.x).abs() < p.w + dm(16) && (p.s - s_c) < dm(200) => {
                        want = Some((-p.x.signum() * dm(20), c.z));
                        break;
                    }
                    _ => {}
                }
            }
        }
        let (tx, tz) = want.unwrap_or((0, 0));
        let steer = |from: i32, to: i32, dead: i32| -> i8 {
            if (to - from).abs() <= dead {
                0
            } else {
                (to - from).signum() as i8
            }
        };
        input.move_x = steer(c.x, tx, dm(8));
        input.move_y = -steer(c.z, tz, dm(8));
        input.swing = brake;
        if rolls && on_me && threat.is_none() {
            input.dash = c.keys & K_DASH == 0;
            return input;
        }
        if rolls {
            input.dash = c.keys & K_DASH == 0;
            input.move_x = if threat.is_some_and(|t| t.1 >= c.x) {
                -1
            } else {
                1
            };
            return input;
        }

        // What to shoot: a kite on a friend's tail, a lamp that is open, a
        // flight to sweep, the nearest foe.
        let lockable: Vec<&Foe> = self
            .foes
            .iter()
            .filter(|f| {
                let near = if f.st == St::Latched { dm(15) } else { dm(40) };
                f.lockable() && (near..=dm(480)).contains(&(f.s - s_c))
            })
            .collect();
        let lamp_open = lockable.iter().find(|f| f.kind == Kind::Lamp);
        let latched = lockable
            .iter()
            .filter(|f| f.st == St::Latched)
            .min_by_key(|f| f.s);
        let fliers: Vec<&&Foe> = lockable.iter().filter(|f| f.kind != Kind::Lamp).collect();
        let cluster = fliers.len() >= 3;
        let held = c.held > HOLD_AT_BOT;
        let in_locks = |f: &Foe| c.locks.iter().any(|l| l.foe == f.id);
        // The target the reticle seeks.
        let target: Option<&Foe> = if let Some(f) = latched {
            Some(*f)
        } else if let Some(f) = lamp_open {
            Some(*f)
        } else if held {
            // Sweeping: on to the nearest one not yet locked.
            fliers
                .iter()
                .map(|f| **f)
                .filter(|f| !in_locks(f))
                .min_by_key(|f| (f.x - c.x).abs() + (f.z - c.z).abs())
        } else {
            fliers.iter().map(|f| **f).min_by_key(|f| (f.s - s_c, f.id))
        };
        if let Some(f) = target {
            let ds = (f.s - s_c).max(dm(30));
            // A novice's aim wanders a little (up to a unit and a half).
            let (jx, jz) = if novice {
                (
                    noise(id, self.t / 14, 1, dm(15)),
                    noise(id, self.t / 14, 2, dm(10)),
                )
            } else {
                (0, 0)
            };
            let rx = ((i64::from(f.x - c.x + jx) * i64::from(FAR) / i64::from(ds)) as i32)
                .clamp(-RX_MAX, RX_MAX);
            let rz = ((i64::from(f.z - c.z + jz) * i64::from(FAR) / i64::from(ds)) as i32)
                .clamp(-RZ_MAX, RZ_MAX);
            let dead = RET_V * 3 / 4;
            input.aim_x = steer(c.rx, rx, dead);
            input.aim_y = -steer(c.rz, rz, dead);
        } else {
            // Nothing to aim at: the reticle comes home.
            input.aim_x = steer(c.rx, 0, RET_V);
            input.aim_y = -steer(c.rz, 0, RET_V);
        }
        // The hands on the fire key.
        let total: usize = c.locks.iter().map(|l| usize::from(l.n)).sum();
        let want_locks = if lamp_open.is_some() && latched.is_none() {
            MAX_PER_BODY as usize
        } else {
            fliers.len().min(MAX_LOCKS)
        };
        if held {
            // Hold until the locks are in or the charge is full, then let go.
            // A lamp wants the whole charge; a flight, whatever is in hand.
            // A novice lets go early now and then, a lamp's charge unfinished.
            let early = novice && noise(id, self.t / 90, 3, 2) == 0;
            let done = if lamp_open.is_some() && latched.is_none() {
                (c.charge >= n.charge && total >= 2) || (early && c.charge >= n.charge / 2)
            } else {
                (total >= want_locks && c.charge >= n.charge / 2) || c.charge >= n.charge
            };
            input.fire = !done;
        } else if c.held > 0 {
            input.fire = true;
        } else if lamp_open.is_some() || cluster {
            input.fire = c.keys & K_FIRE == 0;
        } else if target.is_some() {
            // A tap: down on one tick and up on the next.
            input.fire = c.keys & K_FIRE == 0;
        }
        // Starfall when worn down in a crowded sky.
        let crowd = fliers.iter().filter(|f| (f.s - s_c) < dm(260)).count();
        if c.shield <= 35 && crowd >= 3 && c.bombs > 0 && c.keys & K_BOMB == 0 {
            input.bomb = true;
        }
        input
    }

    /// Whether this knight is one of the rollers: the even seats, and any
    /// knight at all when no even seat is flying.
    fn reflector(&self, id: u32) -> bool {
        id % 2 == 0
            || !self
                .seats
                .iter()
                .any(|(&k, s)| k % 2 == 0 && s.craft.flying())
    }

    /// While the lit lamp is shut, the soonest of the Wyrm's spits that is
    /// owed to it: ticks to it and where it will cross.
    fn intercept(&self, s_c: i32) -> Option<(u32, i32, i32)> {
        let boss = self.boss.as_ref()?;
        let lit = boss.lamps[usize::from(boss.phase.min(2))];
        if !self.foes.iter().any(|f| f.id == lit && f.st == St::Shut) {
            return None;
        }
        self.bolts
            .iter()
            .filter(|b| b.shot == Arm::Gloam && b.src == lit && b.s > s_c)
            .map(|b| {
                let t = ((b.s - s_c) / (PACE - b.vs).max(1)).max(0);
                (t as u32, b.x + b.vx * t, b.z + b.vz * t)
            })
            .min_by_key(|x| x.0)
    }

    /// The soonest hostile bolt that will pass near this craft's plane:
    /// ticks to it, where it will cross, and whether it is a Gloam bolt.
    fn threat(&self, c: &Craft, s_c: i32) -> Option<(u32, i32, i32, bool)> {
        let mut best: Option<(u32, i32, i32, bool)> = None;
        for b in &self.bolts {
            if !matches!(b.shot, Arm::Spark | Arm::Gloam) {
                continue;
            }
            let gap = b.s - s_c;
            let closing = (PACE - b.vs).max(1);
            if gap <= 0 || gap > dm(220) {
                continue;
            }
            let t = (gap / closing).max(0);
            let (px, pz) = (b.x + b.vx * t, b.z + b.vz * t);
            if flat(px, pz, c.x, c.z) < i64::from(dm(26)) && best.is_none_or(|x| (t as u32) < x.0) {
                best = Some((t as u32, px, pz, b.shot == Arm::Gloam));
            }
        }
        best
    }

    /// Where to go to be clear of the point `(ox, oz)`: four ways out of it
    /// (left, right, up, down), those the steer box allows, the one that ends
    /// farthest from it, so a knight at the edge goes up or down and not into
    /// the wall.
    fn room_from(c: &Craft, ox: i32, oz: i32, n: &Nums) -> (i32, i32) {
        let (wx, wz) = (n.wx(), n.wz());
        [(dm(40), 0), (dm(-40), 0), (0, dm(30)), (0, dm(-30))]
            .into_iter()
            .map(|(dx, dz)| ((c.x + dx).clamp(-wx, wx), (c.z + dz).clamp(-wz, wz)))
            .max_by_key(|&(x, z)| flat(x, z, ox, oz))
            .unwrap_or((c.x, c.z))
    }

    /// A footprint locked on this craft (a turret's, a bomber's lane): the
    /// place to be instead.
    fn footprint_away(&self, c: &Craft, n: &Nums) -> Option<(i32, i32)> {
        for f in &self.foes {
            if f.st == St::Tell && matches!(f.kind, Kind::Turret | Kind::Bomber | Kind::Wisp) {
                for (mx, mz) in Session::marks(f, self.pressure()) {
                    if flat(c.x, c.z, mx, mz) < i64::from(dm(24)) {
                        return Some(Session::room_from(c, mx, mz, n));
                    }
                }
            }
        }
        None
    }

    /// The Wyrm's coil wants the gap, its roar wants the column left.
    fn boss_corridor(&self, c: &Craft, n: &Nums) -> Option<(i32, i32)> {
        let b = self.boss.as_ref()?;
        if !matches!(b.st, Wyrm::Tell | Wyrm::Coil | Wyrm::Roar) {
            return None;
        }
        match b.atk {
            1 => Some((b.gap, c.z)),
            2 => {
                if (c.x - b.col).abs() < boss::COL_HALF + dm(16) {
                    let away = if c.x >= b.col { 1 } else { -1 };
                    let to = (b.col + away * (boss::COL_HALF + dm(30))).clamp(-n.wx(), n.wx());
                    Some((to, c.z))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// How long F is held before it is a charge (the flight's own number).
const HOLD_AT_BOT: u32 = 7;
