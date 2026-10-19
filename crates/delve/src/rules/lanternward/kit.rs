//! The knight: a body, a lock-on, a sword, a shield, a lantern and a hop.
//! Every seat has all of it. The verbs are the classic lock-on adventure's,
//! changed only for the camera: the view is from above, so a hop is a hop
//! along the ground and a jump attack lands where the knight comes down.
//!
//! Buttons (the Delve's own, nothing new): WASD walk, the arrows face, R
//! (a pad's left shoulder) holds the lock, Space holds the shield, Q is the
//! sword, E is the one context button (and the roll or the hop when there is
//! nothing to do with it), F is the lantern, X lays a bomb.
use super::world::*;
use super::*;

pub const B_LOCK: u8 = 1;
pub const B_GUARD: u8 = 2;
pub const B_SWORD: u8 = 4;
pub const B_CTX: u8 = 8;
pub const B_LAMP: u8 = 16;
pub const B_BOMB: u8 = 32;

/// The seven buttons the adventure reads, as one byte.
pub fn buttons(i: &Input) -> u8 {
    (if i.ult { B_LOCK } else { 0 })
        | (if i.dash { B_GUARD } else { 0 })
        | (if i.swing { B_SWORD } else { 0 })
        | (if i.interact { B_CTX } else { 0 })
        | (if i.fire { B_LAMP } else { 0 })
        | (if i.bomb { B_BOMB } else { 0 })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Act {
    Free,
    /// A slash: `Seat::n` is which of the three.
    Slash,
    Lunge,
    Spin,
    Roll,
    /// A hop: `Seat::n` is 0 side, 1 back, 2 forward.
    Hop,
    /// The jump attack coming down.
    Plunge,
    Hurt,
    /// Holding an item overhead.
    Item,
    Down,
    /// Over a pit and going down.
    Fall,
}

/// What a lock holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lock {
    Foe(u8),
    Prop(u8),
    /// The Warden's lamp-heart.
    Boss,
}

/// What the context button would do just now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ctx {
    None,
    Open(u8),
    Read(u8),
    Lift(u8),
    Throw,
    Flame,
    Light(u8),
    Unlock(u8),
    Revive(u8),
    Relight,
    Talk(u8),
    Pull(u8),
}

impl Ctx {
    pub fn label(self) -> &'static str {
        match self {
            Ctx::None => "",
            Ctx::Open(_) => "OPEN",
            Ctx::Read(_) => "READ",
            Ctx::Lift(_) => "LIFT",
            Ctx::Throw => "THROW",
            Ctx::Flame => "TAKE FLAME",
            Ctx::Light(_) => "LIGHT",
            Ctx::Unlock(_) => "UNLOCK",
            Ctx::Revive(_) => "REVIVE",
            Ctx::Relight => "RELIGHT",
            Ctx::Talk(_) => "TALK",
            Ctx::Pull(_) => "PULL",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Carry {
    Nothing,
    Pot,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seat {
    /// Which seat this is.
    pub id: u8,
    /// The tick of the run this knight sat down at (0: from the start).
    pub joined: u32,
    pub x: i32,
    pub y: i32,
    /// Height off the ground while hopping, in `Q` pixels.
    pub z: i32,
    pub face: (i32, i32),
    /// Quarter hearts.
    pub hp: u32,
    pub max_hp: u32,
    pub gems: u32,
    pub bombs: u32,
    /// Ticks of flare left in the lantern.
    pub flame: u32,
    /// Ticks left of an ember's flame carried to a brazier.
    pub taper: u32,
    pub act: Act,
    pub t: u32,
    pub n: u8,
    /// Where the act carries the body each tick, in `Q` pixels.
    pub mv: (i32, i32),
    /// A blow's push, a few ticks.
    pub push: (i32, i32),
    pub pusht: u8,
    /// The next slash in the chain, and ticks left to start it.
    pub step: u8,
    pub chain: u32,
    pub queued: bool,
    /// Ticks the sword button has been held.
    pub charge: u32,
    /// Ticks the shield has been up (1 on the first), 0 when down.
    pub shield: u32,
    pub guard_cd: u32,
    pub lock: Option<Lock>,
    pub lock_t: u32,
    pub unlock_t: u32,
    pub relock: u32,
    pub inv: u32,
    /// Ticks left of the blink a hit leaves.
    pub ward: u32,
    /// The knight's own hit-stop: a blade that landed holds its owner.
    pub freeze: u32,
    pub swing: u32,
    pub lamp: u32,
    pub lamp_cd: u32,
    pub carry: Carry,
    pub ctx: Ctx,
    /// Sign being read, and how long it stays up.
    pub read: Option<(u8, u32)>,
    /// The aim keys last tick, to catch a fresh press (cycling a lock).
    pub aim: (i8, i8),
    pub held: u8,
    pub safe: (i32, i32),
    pub cam: (i32, i32),
    /// Revive progress while down.
    pub rev: u32,
    /// Ticks the knight has been in no one's way: for the beep.
    pub beep: u32,
    /// The item held up, while `Act::Item`.
    pub item: Option<Item>,
}

impl Seat {
    pub fn new(id: u32, joined: u32, at: (i32, i32)) -> Seat {
        Seat {
            id: id as u8,
            joined,
            x: at.0,
            y: at.1,
            z: 0,
            face: (0, Q),
            hp: START_HP,
            max_hp: START_HP,
            gems: 0,
            bombs: 0,
            flame: 0,
            taper: 0,
            act: Act::Free,
            t: 0,
            n: 0,
            mv: (0, 0),
            push: (0, 0),
            pusht: 0,
            step: 0,
            chain: 0,
            queued: false,
            charge: 0,
            shield: 0,
            guard_cd: 0,
            lock: None,
            lock_t: 0,
            unlock_t: 0,
            relock: 0,
            inv: SPAWN_INV,
            ward: 0,
            freeze: 0,
            swing: 0,
            lamp: 0,
            lamp_cd: 0,
            carry: Carry::Nothing,
            ctx: Ctx::None,
            read: None,
            aim: (0, 0),
            held: 0,
            safe: at,
            cam: at,
            rev: 0,
            beep: 0,
            item: None,
        }
    }

    /// Whether every number is one a save or the wire may carry.
    pub fn valid(&self, id: u32, elapsed: u32, w: i32, h: i32) -> bool {
        u32::from(self.id) == id
            && self.joined <= elapsed
            && (0..=w).contains(&self.x)
            && (0..=h).contains(&self.y)
            && (0..=(HOP_HEIGHT + 2) * Q).contains(&self.z)
            && dist(self.face.0, self.face.1) <= Q + 8
            && self.max_hp <= 12 * HEART
            && self.hp <= self.max_hp
            && self.gems <= 100_000
            && self.bombs <= BOMB_CAP
            && self.flame <= 300
            && self.taper <= TAPER
            && self.t <= 4096
            && self.n <= 3
            && self.step <= 2
            && self.chain <= 64
            && self.charge <= 255
            && self.shield <= 255
            && self.guard_cd <= 64
            && self.lock_t <= 255
            && self.unlock_t <= 64
            && self.relock <= 16
            && self.inv <= 128
            && self.ward <= HURT_INV
            && self.freeze <= 16
            && self.lamp <= 255
            && self.lamp_cd <= 64
            && self.rev <= 128
            && self.pusht <= 16
            && (-(8 * Q)..=8 * Q).contains(&self.mv.0)
            && (-(8 * Q)..=8 * Q).contains(&self.mv.1)
            && (-(8 * Q)..=8 * Q).contains(&self.push.0)
            && (-(8 * Q)..=8 * Q).contains(&self.push.1)
            && (0..=w).contains(&self.cam.0)
            && (0..=h).contains(&self.cam.1)
            && (0..=w).contains(&self.safe.0)
            && (0..=h).contains(&self.safe.1)
            && self.read.is_none_or(|(_, t)| t <= READ_TICKS)
    }

    /// Standing and awake: can be hurt, can hurt, can be a foe's target.
    pub fn up(&self) -> bool {
        self.hp > 0 && self.act != Act::Down
    }

    pub fn flaring(&self) -> bool {
        self.lamp >= FLARE_AFTER && self.flame > 0 && self.act != Act::Down
    }

    /// Fully wound for the spin.
    pub fn charged(&self) -> bool {
        self.charge >= CHARGE_TICKS
    }

    pub fn airborne(&self) -> bool {
        self.z > 0
    }
}

const SLASH_WIND: [u32; 3] = [2, 2, 4];
const SLASH_ACTIVE: [u32; 3] = [3, 3, 4];
const SLASH_REST: [u32; 3] = [6, 6, 12];
const SLASH_REACH: [i32; 3] = [18, 18, 22];
/// The cosine of each slash's half-angle, in `Q`.
const SLASH_COS: [i32; 3] = [66, 66, -44];
const SLASH_DAMAGE: [u32; 3] = [2, 2, 4];

impl Session {
    /// One tick of one knight.
    pub(super) fn step_seat(&mut self, id: u32, inp: Input, r: &Rules, ev: &mut Events) {
        let Some(mut k) = self.seats.remove(&id) else {
            return;
        };
        let mask = buttons(&inp);
        let press = mask & !k.held;
        k.held = mask;
        k.inv = k.inv.saturating_sub(1);
        k.ward = k.ward.saturating_sub(1);
        k.taper = k.taper.saturating_sub(1);
        k.guard_cd = k.guard_cd.saturating_sub(1);
        k.lamp_cd = k.lamp_cd.saturating_sub(1);
        k.chain = k.chain.saturating_sub(1);
        if k.chain == 0 && !k.queued {
            k.step = 0;
        }
        k.unlock_t = k.unlock_t.saturating_sub(1);
        k.lock_t = (k.lock_t + 1).min(255);
        if let Some((_, t)) = &mut k.read {
            *t = t.saturating_sub(1);
        }
        if k.read.is_some_and(|(_, t)| t == 0) {
            k.read = None;
        }
        self.refill(&mut k, r);
        // A blade that landed holds its owner a moment: the knight's own
        // hit-stop, so six knights never freeze each other.
        if k.freeze > 0 {
            k.freeze -= 1;
            self.camera(&mut k, r);
            self.seats.insert(id, k);
            return;
        }
        match k.act {
            Act::Down => {
                k.t = (k.t + 1).min(4000);
                k.lock = None;
                k.shield = 0;
            }
            Act::Item => {
                k.t += 1;
                if k.t >= ITEM_TICKS {
                    k.act = Act::Free;
                    k.item = None;
                    k.t = 0;
                }
            }
            Act::Fall => self.falling(&mut k),
            Act::Hurt => self.hurting(&mut k),
            _ => {
                self.lock_on(id, &mut k, mask, press, &inp, r, ev);
                self.face_and_move(&mut k, &inp, r);
                self.verbs(id, &mut k, mask, press, &inp, r, ev);
            }
        }
        self.upkeep(&mut k, ev);
        self.camera(&mut k, r);
        self.seats.insert(id, k);
    }

    /// The lantern's flame comes back: slowly anywhere, quickly at a
    /// resting brazier.
    fn refill(&self, k: &mut Seat, r: &Rules) {
        if !self.lantern_owned() {
            return;
        }
        let max = r.flame;
        if k.flame > max {
            k.flame = max;
        }
        if k.flaring() {
            return;
        }
        let near_rest = self
            .props
            .iter()
            .any(|p| p.kind == Pk::Rest && dist(p.x - k.x, p.y - k.y) < BRAZIER_R * Q);
        // A friend's flare kindles yours: stand by it and the tank fills.
        let kindled = self.seats.values().any(|o| {
            o.id != k.id && o.flaring() && dist(o.x - k.x, o.y - k.y) < KINDLE_R * Q
        });
        let up = if near_rest {
            3
        } else if kindled {
            2
        } else if self.elapsed.is_multiple_of(2) {
            1
        } else {
            0
        };
        k.flame = (k.flame + up).min(max);
    }

    pub(super) fn lantern_owned(&self) -> bool {
        self.inv.lantern
    }

    /// Things that go on whatever the knight is doing.
    fn upkeep(&mut self, k: &mut Seat, ev: &mut Events) {
        if k.act == Act::Free && k.z == 0 && !self.pit(k.x, k.y) {
            let (tx, ty) = (tile_of(k.x), tile_of(k.y));
            let p = plan(usize::from(self.room));
            let ledge = (-1..=1).any(|dy| (-1..=1).any(|dx| p.at(tx + dx, ty + dy) == b'='));
            if !ledge && self.clear(k.x, k.y, KNIGHT_R) {
                k.safe = (k.x, k.y);
            }
        }
        // The heart beep: a low knight chirps now and then.
        if k.up() && k.hp <= LOW_HP {
            k.beep = (k.beep + 1) % (BEEP_EVERY * 4);
            if k.beep % BEEP_EVERY == 1 {
                ev.sound("mana_empty");
            }
        } else {
            k.beep = 0;
        }
        k.ctx = if matches!(k.act, Act::Free) {
            self.context(k)
        } else {
            Ctx::None
        };
    }

    // ----- lock-on -------------------------------------------------------

    /// Everything a lock may hold: where it is, and whether it is a foe.
    fn targets(&self) -> Vec<(Lock, i32, i32, bool)> {
        let mut out = Vec::new();
        for f in &self.foes {
            if f.alive() {
                out.push((Lock::Foe(f.id), f.x, f.y, true));
            }
        }
        if let Some(w) = &self.boss
            && w.hp > 0
        {
            let (hx, hy) = w.heart();
            out.push((Lock::Boss, hx, hy, true));
        }
        for (i, p) in self.props.iter().enumerate() {
            if p.target() {
                out.push((Lock::Prop(i as u8), p.x, p.y, false));
            }
        }
        out
    }

    pub fn lock_point(&self, l: Lock) -> Option<(i32, i32)> {
        match l {
            Lock::Foe(id) => self.foes.iter().find(|f| f.id == id && f.alive()).map(|f| (f.x, f.y)),
            Lock::Boss => self.boss.as_ref().filter(|w| w.hp > 0).map(|w| w.heart()),
            Lock::Prop(i) => self
                .props
                .get(usize::from(i))
                .filter(|p| p.target())
                .map(|p| (p.x, p.y)),
        }
    }

    /// Whether the knight's lock is of something still there and in reach.
    fn lock_holds(&self, k: &Seat, l: Lock, r: &Rules) -> bool {
        self.lock_point(l)
            .is_some_and(|(x, y)| dist(x - k.x, y - k.y) <= r.lock * 3 / 2)
    }

    /// The nearest thing in front of the knight, a foe before a switch; with
    /// nothing in front, the nearest at all.
    fn pick_target(&self, k: &Seat, r: &Rules, skip: Option<Lock>) -> Option<Lock> {
        let mut best: Option<((u8, u8, i32), Lock)> = None;
        for (l, x, y, hostile) in self.targets() {
            if Some(l) == skip {
                continue;
            }
            let d = dist(x - k.x, y - k.y);
            if d > r.lock || !self.line_open((k.x, k.y), (x, y)) {
                continue;
            }
            let front = dot(k.face, unit(x - k.x, y - k.y)) >= 0;
            let key = (u8::from(!hostile), u8::from(!front), d);
            if best.is_none_or(|(b, _)| key < b) {
                best = Some((key, l));
            }
        }
        best.map(|(_, l)| l)
    }

    /// The next target in the direction the knight pressed, as seen from the
    /// current one: up and the lock jumps to what is above it on the screen.
    fn cycle_target(&self, k: &Seat, r: &Rules, dir: (i32, i32)) -> Option<Lock> {
        let cur = k.lock.and_then(|l| self.lock_point(l))?;
        let want = unit(dir.0, dir.1);
        let mut best: Option<(i32, Lock)> = None;
        for (l, x, y, _) in self.targets() {
            if Some(l) == k.lock || dist(x - k.x, y - k.y) > r.lock || !self.line_open((k.x, k.y), (x, y)) {
                continue;
            }
            let v = (x - cur.0, y - cur.1);
            let d = dist(v.0, v.1);
            if d > 0 && dot(unit(v.0, v.1), want) > Q / 2 && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, l));
            }
        }
        best.map(|(_, l)| l)
    }

    #[allow(clippy::too_many_arguments)]
    fn lock_on(
        &mut self,
        _id: u32,
        k: &mut Seat,
        mask: u8,
        press: u8,
        inp: &Input,
        r: &Rules,
        ev: &mut Events,
    ) {
        let holding = mask & B_LOCK != 0;
        if !holding || !k.up() {
            if k.lock.is_some() {
                k.unlock_t = EASE_OUT;
            }
            k.lock = None;
            k.relock = 0;
            k.aim = (inp.aim_x, inp.aim_y);
            return;
        }
        if let Some(l) = k.lock
            && !self.lock_holds(k, l, r)
        {
            k.lock = None;
            k.relock = RELOCK;
        }
        // A fresh press of an aim key while locked cycles to the next target
        // that way.
        let fresh = (inp.aim_x != 0 || inp.aim_y != 0) && (inp.aim_x, inp.aim_y) != k.aim;
        k.aim = (inp.aim_x, inp.aim_y);
        if k.lock.is_some() && fresh {
            if let Some(l) = self.cycle_target(k, r, (i32::from(inp.aim_x), i32::from(inp.aim_y))) {
                self.take_lock(k, l, ev);
            }
        } else if k.lock.is_none() {
            if press & B_LOCK != 0 || k.relock == 0 {
                if let Some(l) = self.pick_target(k, r, None) {
                    self.take_lock(k, l, ev);
                } else if press & B_LOCK != 0 {
                    // Nothing to lock: the view squares up behind the knight.
                    k.unlock_t = EASE_OUT;
                }
            } else {
                k.relock -= 1;
            }
        }
    }

    fn take_lock(&mut self, k: &mut Seat, l: Lock, ev: &mut Events) {
        k.lock = Some(l);
        k.lock_t = 0;
        if let Some((x, y)) = self.lock_point(l) {
            // The wick-spark settles over the target, and the knight swings
            // round to face it at once.
            self.fx(Fxk::Tick, x, y, 0);
            let f = unit(x - k.x, y - k.y);
            if f != (0, 0) {
                k.face = f;
            }
        }
        ev.sound("card_pickup");
    }

    // ----- facing and walking -------------------------------------------

    fn face_and_move(&mut self, k: &mut Seat, inp: &Input, r: &Rules) {
        let walking = (i32::from(inp.move_x), i32::from(inp.move_y));
        if let Some(l) = k.lock
            && let Some((tx, ty)) = self.lock_point(l)
        {
            let f = unit(tx - k.x, ty - k.y);
            if f != (0, 0) {
                k.face = f;
            }
        } else if inp.aim_x != 0 || inp.aim_y != 0 {
            k.face = unit(i32::from(inp.aim_x), i32::from(inp.aim_y));
        } else if walking != (0, 0) && k.shield == 0 && k.act == Act::Free {
            k.face = unit(walking.0, walking.1);
        }
        match k.act {
            Act::Free => {}
            _ => return,
        }
        if k.pusht > 0 {
            k.pusht -= 1;
            let p = k.push;
            self.shove(k, p.0, p.1);
        }
        if walking == (0, 0) {
            return;
        }
        let speed = if k.shield > 0 {
            GUARD_WALK
        } else if k.carry == Carry::Pot || k.charge > CHARGE_WALK_AFTER {
            CARRY_WALK
        } else if k.lock.is_some() {
            LOCKED_WALK
        } else {
            WALK
        };
        let v = unit(walking.0, walking.1);
        let (vx, vy) = (v.0 * speed / Q, v.1 * speed / Q);
        match k.lock.and_then(|l| self.lock_point(l)) {
            Some((tx, ty)) => self.orbit(k, (tx, ty), (vx, vy)),
            None => self.shove(k, vx, vy),
        }
        let _ = r;
    }

    /// Walk about a locked target: the sideways part of the step goes round
    /// it at the same distance, the rest closes or opens it.
    fn orbit(&mut self, k: &mut Seat, t: (i32, i32), v: (i32, i32)) {
        let (rx, ry) = (k.x - t.0, k.y - t.1);
        let len = dist(rx, ry);
        if len < Q {
            return self.shove(k, v.0, v.1);
        }
        let rhat = unit(rx, ry);
        let that = (-rhat.1, rhat.0);
        let (mut radial, mut tangent) = (dot(v, rhat), dot(v, that));
        // A keyboard cannot aim at a tangent: input that is mostly sideways
        // to the foe is all sideways, and mostly toward or away is all that,
        // so a held key really does circle.
        let speed = dist(v.0, v.1);
        if tangent.abs() * 10 >= speed * 8 {
            radial = 0;
        } else if radial.abs() * 10 >= speed * 8 {
            tangent = 0;
        }
        let want = (len + radial).max(ORBIT_MIN * Q);
        let off = (
            (i64::from(rhat.0) * i64::from(len) / i64::from(Q)) as i32 + tangent * that.0 / Q,
            (i64::from(rhat.1) * i64::from(len) / i64::from(Q)) as i32 + tangent * that.1 / Q,
        );
        let u = unit(off.0, off.1);
        let to = (
            t.0 + (i64::from(u.0) * i64::from(want) / i64::from(Q)) as i32,
            t.1 + (i64::from(u.1) * i64::from(want) / i64::from(Q)) as i32,
        );
        self.shove(k, to.0 - k.x, to.1 - k.y);
    }

    /// Move a body by `(dx, dy)`, a whole axis at a time so it slides along
    /// a wall; leans on a block it meets; falls into a pit it walks into.
    pub(super) fn shove(&mut self, k: &mut Seat, dx: i32, dy: i32) {
        let airborne = k.airborne() || matches!(k.act, Act::Roll | Act::Hop | Act::Plunge);
        let rr = KNIGHT_R;
        if dx != 0 {
            if self.clear(k.x + dx, k.y, rr) {
                k.x += dx;
            } else {
                self.lean(k, dx.signum(), 0);
            }
        }
        if dy != 0 {
            if self.clear(k.x, k.y + dy, rr) {
                k.y += dy;
            } else {
                self.lean(k, 0, dy.signum());
            }
        }
        if !airborne && self.pit(k.x, k.y) && k.act == Act::Free {
            k.act = Act::Fall;
            k.t = 0;
            k.lock = None;
            k.lamp = 0;
            k.shield = 0;
        }
    }

    /// A body pressing on a block: after a moment of leaning, it slides.
    fn lean(&mut self, k: &Seat, sx: i32, sy: i32) {
        let reach = (k.x + sx * (KNIGHT_R + 3 * Q), k.y + sy * (KNIGHT_R + 3 * Q));
        let Some(b) = self
            .props
            .iter()
            .position(|p| p.kind == Pk::Block && p.on == 0 && p.blocks(reach.0, reach.1))
        else {
            return;
        };
        let p = &mut self.props[b];
        // Two for each tick leaned, one lost each tick not: a push is a
        // push held for a moment.
        p.t = p.t.saturating_add(2);
        if p.t >= PUSH_LEAN {
            let way = match (sx, sy) {
                (1, _) => 1,
                (-1, _) => 2,
                (_, 1) => 3,
                _ => 4,
            };
            p.t = 0;
            p.on = way;
            self.start_slide(b);
        }
    }

    fn falling(&mut self, k: &mut Seat) {
        k.t += 1;
        if k.t >= FALL_TICKS {
            k.hp = k.hp.saturating_sub(FALL_DAMAGE).max(1);
            k.x = k.safe.0;
            k.y = k.safe.1;
            k.act = Act::Free;
            k.t = 0;
            k.inv = HURT_INV;
        }
    }

    fn hurting(&mut self, k: &mut Seat) {
        k.t += 1;
        if k.pusht > 0 {
            k.pusht -= 1;
            let p = k.push;
            self.shove(k, p.0, p.1);
        }
        if k.t >= HURT_TICKS {
            k.act = Act::Free;
            k.t = 0;
        }
    }

    // ----- the verbs ------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn verbs(
        &mut self,
        id: u32,
        k: &mut Seat,
        mask: u8,
        press: u8,
        inp: &Input,
        r: &Rules,
        ev: &mut Events,
    ) {
        // The shield: held on the ground with a free arm.
        let can_guard = k.carry == Carry::Nothing && k.z == 0;
        if mask & B_GUARD != 0 && matches!(k.act, Act::Free) && can_guard && k.guard_cd == 0 {
            if k.shield == 0 {
                ev.sound("shield_up");
            }
            k.shield = (k.shield + 1).min(255);
        } else {
            if k.shield > 0 && k.act == Act::Free {
                k.guard_cd = GUARD_REST;
            }
            k.shield = 0;
        }
        // The sword button held builds the spin.
        if mask & B_SWORD != 0 && k.shield == 0 {
            k.charge = (k.charge + 1).min(255);
            if k.charge == CHARGE_TICKS {
                ev.sound("spell_cast");
                self.fx(Fxk::Spark, k.x, k.y - 8 * Q, 1);
            }
        }
        let released = k.held_before_sword(mask);
        let _ = released;
        // A swing's recovery gives way to a dodge or the shield: the knight
        // is never locked into a blow while something is coming.
        if self.recovering(k) && (press & B_CTX != 0 || mask & B_GUARD != 0) {
            k.act = Act::Free;
            k.t = 0;
            k.chain = 0;
            k.step = 0;
            k.queued = false;
        }
        match k.act {
            Act::Free => self.free(id, k, mask, press, inp, r, ev),
            Act::Slash | Act::Lunge | Act::Spin => self.swinging(id, k, mask, press, inp, r, ev),
            Act::Roll => self.rolling(k),
            Act::Hop => self.hopping(id, k, press, r, ev),
            Act::Plunge => self.plunging(id, k, r, ev),
            _ => {}
        }
        // Letting the sword go ends the winding; a wound spin goes off.
        if mask & B_SWORD == 0 {
            if k.charged() && k.act == Act::Free && k.shield == 0 {
                self.begin_spin(k, ev);
            }
            k.charge = 0;
        }
        // The lantern.
        self.lantern(id, k, mask, press, r, ev);
    }

    /// Whether a slash or a lunge has struck and is only coming back.
    fn recovering(&self, k: &Seat) -> bool {
        match k.act {
            Act::Slash => {
                let n = usize::from(k.n.min(2));
                k.t > SLASH_WIND[n] + SLASH_ACTIVE[n]
            }
            Act::Lunge => k.t >= LUNGE_WIND + LUNGE_ACTIVE,
            _ => false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn free(
        &mut self,
        id: u32,
        k: &mut Seat,
        _mask: u8,
        press: u8,
        inp: &Input,
        r: &Rules,
        ev: &mut Events,
    ) {
        if k.shield > 0 {
            // A shield up trades the sword and the hop for a wall; E still
            // meets the world.
            if press & B_CTX != 0 {
                self.use_context(id, k, r, ev);
            }
            return;
        }
        if press & B_SWORD != 0 && k.carry == Carry::Nothing {
            let forward = k.lock.and_then(|l| self.lock_point(l)).is_some_and(|(tx, ty)| {
                let v = unit(i32::from(inp.move_x), i32::from(inp.move_y));
                v != (0, 0) && dot(v, unit(tx - k.x, ty - k.y)) >= LUNGE_FORWARD
            });
            if forward && k.chain == 0 {
                self.begin_lunge(k, ev);
            } else {
                let step = if k.chain > 0 { k.step } else { 0 };
                self.begin_slash(k, step, r, ev);
            }
            return;
        }
        if press & B_CTX != 0 {
            if !self.use_context(id, k, r, ev) {
                self.begin_dodge(k, inp, ev);
            }
            return;
        }
        if press & B_BOMB != 0 && k.bombs > 0 && k.carry == Carry::Nothing {
            k.bombs -= 1;
            let (x, y) = (k.x + k.face.0 * 12, k.y + k.face.1 * 12);
            if self.fuses.len() < FUSE_CAP {
                self.fuses.push(Fuse { x, y, t: FUSE_TICKS, owner: id as u8 });
            }
            ev.sound("bomb_toss");
        }
    }

    // ----- sword ----------------------------------------------------------

    fn begin_slash(&mut self, k: &mut Seat, step: u8, _r: &Rules, ev: &mut Events) {
        k.act = Act::Slash;
        k.t = 0;
        k.n = step.min(2);
        k.swing += 1;
        k.queued = false;
        k.mv = (0, 0);
        ev.sound("swing");
    }

    fn begin_lunge(&mut self, k: &mut Seat, ev: &mut Events) {
        k.act = Act::Lunge;
        k.t = 0;
        k.swing += 1;
        k.chain = 0;
        k.step = 0;
        k.mv = (k.face.0 * LUNGE_STEP / Q, k.face.1 * LUNGE_STEP / Q);
        ev.sound("swing");
    }

    fn begin_spin(&mut self, k: &mut Seat, ev: &mut Events) {
        k.act = Act::Spin;
        k.t = 0;
        k.swing += 1;
        k.charge = 0;
        k.chain = 0;
        k.step = 0;
        ev.sound("axe_spin");
    }

    #[allow(clippy::too_many_arguments)]
    fn swinging(
        &mut self,
        id: u32,
        k: &mut Seat,
        mask: u8,
        press: u8,
        _inp: &Input,
        r: &Rules,
        ev: &mut Events,
    ) {
        k.t += 1;
        let t = k.t;
        match k.act {
            Act::Slash => {
                let n = usize::from(k.n.min(2));
                let (wind, active, rest) = (SLASH_WIND[n], SLASH_ACTIVE[n], SLASH_REST[n]);
                if press & B_SWORD != 0 {
                    k.queued = true;
                }
                if t > wind && t <= wind + active {
                    let hit = self.sweep(id, k, SLASH_REACH[n], SLASH_COS[n], SLASH_DAMAGE[n], n == 2, ev);
                    if hit {
                        k.freeze = k.freeze.max(if n == 2 { 4 } else { 2 });
                    }
                }
                if t == wind + active {
                    k.chain = rest + r.combo;
                    k.step = if n == 2 { 0 } else { n as u8 + 1 };
                }
                if t >= wind + active + rest {
                    k.act = Act::Free;
                    k.t = 0;
                    if k.queued && k.step > 0 && k.chain > 0 {
                        let step = k.step;
                        self.begin_slash(k, step, r, ev);
                    }
                }
            }
            Act::Lunge => {
                if (LUNGE_WIND..LUNGE_WIND + LUNGE_ACTIVE).contains(&t) {
                    let (mx, my) = k.mv;
                    self.shove(k, mx, my);
                    let hit = self.lunge_hit(id, k, ev);
                    if hit {
                        k.freeze = k.freeze.max(3);
                    }
                }
                if t >= LUNGE_WIND + LUNGE_ACTIVE + LUNGE_REST {
                    k.act = Act::Free;
                    k.t = 0;
                }
            }
            Act::Spin => {
                if (2..SPIN_TICKS - 2).contains(&t) {
                    let hit = self.sweep(id, k, SPIN_REACH, -Q, SPIN_DAMAGE, true, ev);
                    if hit {
                        k.freeze = k.freeze.max(3);
                    }
                }
                if t >= SPIN_TICKS {
                    k.act = Act::Free;
                    k.t = 0;
                }
            }
            _ => {}
        }
        let _ = mask;
    }

    /// A swing's arc: every foe, pot, blade-cut thing and iron bolt inside
    /// it is struck once. True when something was hurt.
    #[allow(clippy::too_many_arguments)]
    fn sweep(
        &mut self,
        id: u32,
        k: &Seat,
        reach: i32,
        cos: i32,
        dmg: u32,
        heavy: bool,
        ev: &mut Events,
    ) -> bool {
        let mut landed = false;
        let seat = (id as usize - 1).min(5);
        let inside = |x: i32, y: i32, extra: i32| {
            let d = dist(x - k.x, y - k.y);
            d <= reach * Q + extra && (cos <= -Q || d < 6 * Q || dot(unit(x - k.x, y - k.y), k.face) >= cos)
        };
        for i in 0..self.foes.len() {
            let f = &self.foes[i];
            if f.hit[seat] == k.swing || !f.alive() || !inside(f.x, f.y, f.radius()) {
                continue;
            }
            self.foes[i].hit[seat] = k.swing;
            if self.hurt_foe(i, dmg, (k.x, k.y), ev) {
                landed = true;
            }
        }
        if self.boss.as_ref().is_some_and(|w| w.hp > 0) {
            let (bx, by, br) = self.boss.as_ref().map(|w| (w.x, w.y, w.radius())).unwrap();
            if inside(bx, by, br) && self.boss.as_ref().unwrap().hit[seat] != k.swing {
                self.boss.as_mut().unwrap().hit[seat] = k.swing;
                if self.hurt_boss(dmg, (k.x, k.y), id, ev) {
                    landed = true;
                }
            }
        }
        for i in 0..self.props.len() {
            let p = &self.props[i];
            let cuttable = matches!(p.kind, Pk::Pot | Pk::Grass | Pk::Crystal) && p.on == 0;
            if cuttable && inside(p.x, p.y, 6 * Q) {
                let kind = p.kind;
                let (x, y) = (p.x, p.y);
                let _ = (x, y);
                match kind {
                    Pk::Crystal => self.light_crystal(i, ev),
                    _ => self.shatter(i, ev),
                }
            }
        }
        let mut cut = false;
        for b in &mut self.bolts {
            if b.hostile() && inside(b.x, b.y, 4 * Q) {
                b.ttl = 0;
                cut = true;
            }
        }
        if cut {
            ev.sound("clang");
            self.fx(Fxk::Spark, k.x + k.face.0 * 18, k.y + k.face.1 * 18, 0);
        }
        if landed {
            ev.stop(Stop::Heavy);
        }
        if landed && heavy {
            self.shake = self.shake.max(SHAKE_HEAVY);
            ev.sound("hit_axe");
        }
        landed
    }

    /// A lunge's line: from the knight out ahead, narrow and long.
    fn lunge_hit(&mut self, id: u32, k: &Seat, ev: &mut Events) -> bool {
        let seat = (id as usize - 1).min(5);
        let a = (k.x, k.y);
        let b = (k.x + k.face.0 * LUNGE_REACH, k.y + k.face.1 * LUNGE_REACH);
        let line = Danger::Capsule { ax: a.0, ay: a.1, bx: b.0, by: b.1, r: 5 * Q };
        let mut landed = false;
        for i in 0..self.foes.len() {
            let f = &self.foes[i];
            if f.hit[seat] == k.swing || !f.alive() || !line.hits(f.x, f.y, f.radius()) {
                continue;
            }
            self.foes[i].hit[seat] = k.swing;
            if self.hurt_foe(i, LUNGE_DAMAGE, (k.x, k.y), ev) {
                landed = true;
            }
        }
        if let Some(w) = &self.boss
            && w.hp > 0
            && w.hit[seat] != k.swing
            && line.hits(w.x, w.y, w.radius())
        {
            let swing = k.swing;
            self.boss.as_mut().unwrap().hit[seat] = swing;
            if self.hurt_boss(LUNGE_DAMAGE, (k.x, k.y), id, ev) {
                landed = true;
            }
        }
        if landed {
            ev.stop(Stop::Heavy);
        }
        landed
    }

    // ----- dodges ---------------------------------------------------------

    /// E with nothing to do: a roll along the walk, or, locked on, the hop
    /// the stick asks for.
    fn begin_dodge(&mut self, k: &mut Seat, inp: &Input, ev: &mut Events) {
        let walk = unit(i32::from(inp.move_x), i32::from(inp.move_y));
        let locked = k.lock.and_then(|l| self.lock_point(l));
        if let Some((tx, ty)) = locked {
            let fwd = unit(tx - k.x, ty - k.y);
            let side = (-fwd.1, fwd.0);
            let (f, s) = if walk == (0, 0) { (-Q, 0) } else { (dot(walk, fwd), dot(walk, side)) };
            let (kind, v, speed) = if s.abs() > f.abs() && s.abs() > Q / 4 {
                let sign = s.signum();
                (0u8, (side.0 * sign, side.1 * sign), HOP_SIDE)
            } else if f > Q / 4 {
                (2u8, fwd, HOP_FORWARD)
            } else {
                (1u8, (-fwd.0, -fwd.1), HOP_BACK)
            };
            k.act = Act::Hop;
            k.n = kind;
            k.t = 0;
            k.mv = (v.0 * speed / Q, v.1 * speed / Q);
            k.lock_t = 0;
            ev.sound("roll");
        } else {
            let v = if walk == (0, 0) { k.face } else { walk };
            k.act = Act::Roll;
            k.t = 0;
            k.n = 0;
            k.mv = (v.0 * ROLL_SPEED / Q, v.1 * ROLL_SPEED / Q);
            k.face = v;
            ev.sound("roll");
        }
        k.chain = 0;
        k.step = 0;
        k.charge = 0;
    }

    fn rolling(&mut self, k: &mut Seat) {
        k.t += 1;
        if k.t < ROLL_MOVE {
            let (mx, my) = k.mv;
            self.shove(k, mx, my);
        }
        if k.t >= ROLL_TICKS {
            k.act = Act::Free;
            k.t = 0;
            if self.pit(k.x, k.y) {
                k.act = Act::Fall;
            }
        }
        k.inv = k.inv.max(u32::from((ROLL_IFRAMES.0..=ROLL_IFRAMES.1).contains(&k.t)));
    }

    fn hopping(&mut self, id: u32, k: &mut Seat, press: u8, r: &Rules, ev: &mut Events) {
        k.t += 1;
        let t = k.t as i32;
        let total = HOP_TICKS as i32;
        if t < total - 2 {
            let (mx, my) = k.mv;
            self.shove(k, mx, my);
        }
        // A parabola, peaking mid-hop.
        k.z = (HOP_HEIGHT * Q * 4 * t * (total - t) / (total * total)).max(0);
        if (HOP_IFRAMES.0..=HOP_IFRAMES.1).contains(&k.t) {
            k.inv = k.inv.max(1);
        }
        if press & B_SWORD != 0 && k.z > 2 * Q {
            // The jump attack: straight down on whatever is below.
            k.act = Act::Plunge;
            k.t = 0;
            k.mv = (0, 0);
            k.swing += 1;
            ev.sound("swing");
            return;
        }
        if k.t >= HOP_TICKS {
            k.z = 0;
            k.act = Act::Free;
            k.t = 0;
            self.fx(Fxk::Dust, k.x, k.y, 0);
            if self.pit(k.x, k.y) {
                k.act = Act::Fall;
            }
        }
        let _ = (id, r);
    }

    fn plunging(&mut self, id: u32, k: &mut Seat, _r: &Rules, ev: &mut Events) {
        k.t += 1;
        k.inv = k.inv.max(1);
        k.z = (k.z - PLUNGE_FALL * Q).max(0);
        if k.z == 0 {
            // Down: the blow lands in a ring about the knight.
            let hit = self.sweep(id, k, PLUNGE_REACH, -Q, PLUNGE_DAMAGE, true, ev);
            self.fx(Fxk::Ring, k.x, k.y, 24);
            ev.sound("rock_land");
            self.shake = self.shake.max(SHAKE_HEAVY);
            if hit {
                k.freeze = 3;
            }
            k.act = Act::Free;
            k.t = 0;
            k.guard_cd = PLUNGE_REST;
            if self.pit(k.x, k.y) {
                k.act = Act::Fall;
            }
        }
    }

    // ----- the lantern ----------------------------------------------------

    fn lantern(&mut self, id: u32, k: &mut Seat, mask: u8, _press: u8, r: &Rules, ev: &mut Events) {
        if !self.lantern_owned() || !matches!(k.act, Act::Free | Act::Slash | Act::Hop) {
            k.lamp = 0;
            return;
        }
        if mask & B_LAMP != 0 {
            k.lamp = (k.lamp + 1).min(255);
            if k.lamp == FLARE_AFTER && k.flame > 0 {
                ev.sound("spell_cast");
            }
            if k.flaring() {
                k.flame = k.flame.saturating_sub(1);
                if k.flame == 0 {
                    // Sputters out: the knight cannot flare again for a bit.
                    k.lamp = 0;
                    k.lamp_cd = SPUTTER;
                    ev.sound("mana_empty");
                }
            }
        } else {
            // A tap throws a bolt of light on the release.
            if k.lamp > 0 && k.lamp < FLARE_AFTER && k.lamp_cd == 0 {
                let cost = r.flame / BOLT_COST_PARTS;
                if k.flame >= cost {
                    k.flame -= cost;
                    k.lamp_cd = LAMP_REST;
                    let aim = k
                        .lock
                        .and_then(|l| self.lock_point(l))
                        .map_or(k.face, |(tx, ty)| unit(tx - k.x, ty - k.y));
                    if self.bolts.len() < BOLT_CAP {
                        self.bolts.push(Bolt {
                            kind: Bk::Light,
                            x: k.x + aim.0 * 8,
                            y: k.y + aim.1 * 8,
                            vx: aim.0 * LIGHT_SPEED / Q,
                            vy: aim.1 * LIGHT_SPEED / Q,
                            ttl: LIGHT_LIFE,
                            owner: id as u8,
                        });
                    }
                    ev.sound("shot_spark");
                } else {
                    ev.sound("mana_empty");
                }
            }
            k.lamp = 0;
        }
        let _ = r;
    }

    // ----- camera -----------------------------------------------------------

    /// The view eases to the knight, leaning toward a lock; a fresh lock
    /// swings it in fast, a let-go eases it back slowly.
    fn camera(&mut self, k: &mut Seat, _r: &Rules) {
        let mut target = (k.x, k.y);
        if let Some((tx, ty)) = k.lock.and_then(|l| self.lock_point(l)) {
            target = (k.x + (tx - k.x) * LOCK_LEAN / 100, k.y + (ty - k.y) * LOCK_LEAN / 100);
        }
        // The Warden stands head and shoulders over a knight: while it is up
        // and not the lock, the view leans toward it so its body clears the
        // top of the picture (and the bar), up to BOSS_PULL pixels.
        if !matches!(k.lock, Some(Lock::Boss))
            && let Some(w) = self.boss.as_ref().filter(|w| w.hp > 0 && w.st != Ws::Asleep)
        {
            let (dx, dy) = (w.x - target.0, w.y - target.1);
            let d = dist(dx, dy).max(1);
            let pull = (d * 70 / 100).min(BOSS_PULL * Q);
            target.0 += (i64::from(dx) * i64::from(pull) / i64::from(d)) as i32;
            target.1 += (i64::from(dy) * i64::from(pull) / i64::from(d)) as i32;
        }
        // A door or a switch that came right: every view looks at it a moment.
        let mut cued = false;
        if let Some((cx, cy, t)) = self.cue {
            let on = if t > CUE_TICKS - 8 {
                (CUE_TICKS - t) as i32 * 100 / 8
            } else if t > 8 {
                100
            } else {
                t as i32 * 100 / 8
            };
            target = (target.0 + (cx - target.0) * on / 100, target.1 + (cy - target.1) * on / 100);
            cued = on > 0;
        }
        let rate = if cued {
            3
        } else if k.lock.is_some() && k.lock_t < SNAP_TICKS {
            6
        } else if k.lock.is_some() {
            3
        } else if k.unlock_t > 0 {
            2
        } else {
            4
        };
        k.cam.0 += (target.0 - k.cam.0) * rate / 8;
        k.cam.1 += (target.1 - k.cam.1) * rate / 8;
        let p = plan(usize::from(self.room));
        let (hw, hh) = (VIEW_W / 2 * Q, VIEW_H / 2 * Q);
        let (rw, rh) = (p.w() * TILE * Q, p.h() * TILE * Q);
        k.cam.0 = if rw <= 2 * hw { rw / 2 } else { k.cam.0.clamp(hw, rw - hw) };
        k.cam.1 = if rh <= 2 * hh { rh / 2 } else { k.cam.1.clamp(hh, rh - hh) };
    }
}

impl Seat {
    /// Whether the sword button was held a tick ago (unused by the verbs,
    /// kept so the release rule reads in one place).
    fn held_before_sword(&self, mask: u8) -> bool {
        mask & B_SWORD != 0
    }
}
