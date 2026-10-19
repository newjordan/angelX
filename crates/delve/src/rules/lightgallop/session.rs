//! The party's ride: every seated knight on the Moonwell Road at once, one
//! Dimming behind them, one set of foes and lanterns ahead.
//!
//! Each knight has the whole set of moves (`rider.rs`) and their own camera
//! and spark count; the road is shared: a foe popped is popped for everyone
//! (and re-forms a while later, so a pack behind has something to chain), a
//! lantern post any knight lights is lit for all and holds the Dimming back,
//! and a fallen knight is carried to the last lit post. More knights at the
//! table means more foes on the road and a quicker Dimming, never weaker foes.
use super::level::{HOLD, level};
use super::math::{self, Q};
use super::rider::*;
use super::world::*;
use crate::rules::{BTreeMap, HZ, Hero, Input, MAX_PLAYERS};
use serde::{Deserialize, Serialize};

/// A run never runs past an hour; a longer one is not a run to play.
pub const LIMIT: u32 = 60 * 60 * HZ;
/// A popped foe re-forms after this many ticks if no knight is near.
pub const REFORM: u16 = 180;
/// After the first bell, the rest of the pack has this long to ring it.
pub const GRACE: u32 = 300;
/// The warden's hits for one knight; each more knight adds one.
pub const WARDEN_BASE: u8 = 3;
/// Ticks the warden shrugs off blows after one lands (shared by all).
pub const WARDEN_INV: u8 = 22;
/// Sparks each knight is given when the warden falls.
pub const WARDEN_LIGHT: u32 = 10;
/// Ticks the warden's fall is shown for.
pub const WARDEN_FALL: u16 = 90;
/// Hay bale lifts.
pub const SPRING_UP: i32 = 2200;
pub const SPRING_DIAG: (i32, i32) = (1700, 2000);
/// A boost pad's pace.
pub const PAD_SPEED: i32 = 1920;
/// Spur gauge from a lock-chain link (times the link, up to five), a plain
/// kill, a spark, a lantern post.
pub const CHAIN_GAUGE: i32 = 140;
pub const KILL_GAUGE: i32 = 60;
pub const SPARK_GAUGE: i32 = 12;
pub const POST_GAUGE: i32 = 200;
/// The Dimming's pace at each act, percent of the number.
pub const ACT_PACE: [i32; 4] = [90, 110, 130, 150];
/// The most the Gloam lets the rearmost knight lead it by, pixels: a pack
/// that outruns it by more is gained on at once, so the chase is always on.
pub const LEAD_MAX: i32 = 1200;
/// The Dimming's pace gains this much more for each knight past the first,
/// percent; and a lit post holds it this many pixels less far back.
pub const SEAT_PACE: i32 = 8;
pub const SEAT_HOLD: i32 = 25;
/// The slowest and fastest time that scores, in seconds.
pub const FAST: u32 = 100;
pub const SLOW: u32 = 300;
/// The ranks' lines: S, A, B.
pub const RANKS: [u32; 3] = [850, 650, 420];
/// A tally takes this long to tick up before an E starts the ride again.
pub const TALLY_HOLD: u32 = 45;

/// One knight at the table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seat {
    /// The tick of the run this knight sat down at (0: from the start).
    pub joined: u32,
    pub rider: Rider,
}

/// The end of the ride: what the party scored and how it ranks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    /// The tick the tally began, for the numbers to tick up from.
    pub at: u32,
    /// The pack's time (the last bell), in ticks.
    pub time: u32,
    /// Sparks gathered, the pack's average.
    pub sparks: u32,
    /// The longest lance-lock chain any knight made.
    pub chain: u32,
    /// Lantern posts lit by the pack.
    pub posts: u32,
    pub falls: u32,
    pub points: u32,
    /// 0 C, 1 B, 2 A, 3 S.
    pub rank: u8,
    /// The knight who rode best (the laurel).
    pub best: u32,
    pub scores: BTreeMap<u32, u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub seats: BTreeMap<u32, Seat>,
    pub elapsed: u32,
    /// The front of the Dimming, 1/256 pixel along the road.
    pub dim: i32,
    /// Lantern posts lit, a bit each; and when each was (tick + 1).
    pub posts: u32,
    pub post_t: Vec<u32>,
    /// The most knights who have sat down; foes are woken for this many.
    pub provisioned: u32,
    pub foes: Vec<FoeState>,
    /// When each spring and pad last fired (tick + 1), for the picture.
    pub spring_t: Vec<u32>,
    pub pad_t: Vec<u32>,
    pub warden_inv: u8,
    /// The tick the first bell rang, if it has.
    pub bell_t: Option<u32>,
    pub tally: Option<Tally>,
    /// The purse has been paid for this ride.
    pub paid: bool,
    /// A knight asked to ride again at the tally.
    pub again: bool,
}

/// A rider's ground, set at the shared post or the gate.
fn ride_from(post: Option<usize>, id: u32) -> (usize, i32) {
    let lvl = level();
    let (path, s) = lvl.spawn(post);
    match post {
        // Side by side at the gate, a few pixels apart.
        None => (path, s - (id as i32 - 1) * 12 * Q),
        Some(_) => (path, s),
    }
}

impl Session {
    pub fn new(players: &BTreeMap<u32, Hero>) -> Self {
        let lvl = level();
        let mut s = Session {
            seats: BTreeMap::new(),
            elapsed: 0,
            dim: -700 * Q,
            posts: 0,
            post_t: vec![0; lvl.posts.len()],
            provisioned: 1,
            foes: lvl
                .foes
                .iter()
                .map(|f| FoeState { hp: if f.kind == FoeKind::Warden { WARDEN_BASE } else { 1 }, pop: 0, flinch: 0 })
                .collect(),
            spring_t: vec![0; lvl.springs.len()],
            pad_t: vec![0; lvl.pads.len()],
            warden_inv: 0,
            bell_t: None,
            tally: None,
            paid: false,
            again: false,
        };
        for (&id, _) in players.iter().filter(|(_, h)| !h.away && !h.stone) {
            let seat = s.new_seat(id, 0, None);
            s.seats.insert(id, seat);
        }
        s.provisioned = (s.seats.len() as u32).max(1);
        s.grow_warden(s.provisioned - 1);
        s
    }

    fn new_seat(&self, id: u32, joined: u32, post: Option<usize>) -> Seat {
        let lvl = level();
        let (path, at) = ride_from(post, id);
        let mut rider = Rider::new_on(&lvl.terrain, path, at);
        rider.snap_camera(rider.x / Q, rider.y / Q);
        Seat { joined, rider }
    }

    /// How far back post `n` holds the Dimming when it is lit, pixels: less
    /// for a fuller table.
    pub fn hold(&self, n: usize) -> i32 {
        level().posts[n].hold - SEAT_HOLD * (self.seats.len() as i32 - 1).max(0)
    }

    /// The highest post lit, if any.
    pub fn last_post(&self) -> Option<usize> {
        (0..level().posts.len()).rev().find(|&n| self.posts >> n & 1 == 1)
    }

    /// Which act the road is in at column `x` (pixels).
    pub fn act_at(x: i32) -> usize {
        let a = level().acts;
        (0..4).rev().find(|&n| x >= a[n]).unwrap_or(0)
    }

    /// Whether foe `i` is on the road for this many knights.
    fn awake(&self, i: usize) -> bool {
        level().foes[i].min_seats <= self.provisioned
    }

    /// The warden takes one more hit for each knight who sits down; hits
    /// already taken stay taken.
    fn grow_warden(&mut self, add: u32) {
        for (i, f) in level().foes.iter().enumerate() {
            if f.kind == FoeKind::Warden && self.foes[i].hp > 0 {
                self.foes[i].hp = (u32::from(self.foes[i].hp) + add).min(u32::from(WARDEN_BASE) + MAX_PLAYERS - 1) as u8;
            }
        }
    }

    /// One tick: seat whoever is here and awake, run every knight, the foes
    /// and the Dimming. Returns the sounds the tick made.
    pub fn step(
        &mut self,
        inputs: &BTreeMap<u32, Input>,
        heroes: &BTreeMap<u32, Hero>,
        n: &Numbers,
    ) -> Vec<&'static str> {
        let mut snd: Vec<&'static str> = Vec::new();
        self.elapsed = (self.elapsed + 1).min(LIMIT);
        self.seats.retain(|id, _| heroes.get(id).is_some_and(|h| !h.away && !h.stone));
        let post = self.last_post();
        for (&id, _) in heroes.iter().filter(|(_, h)| !h.away && !h.stone) {
            if !self.seats.contains_key(&id) && self.seats.len() < MAX_PLAYERS as usize {
                let seat = self.new_seat(id, self.elapsed, post);
                self.seats.insert(id, seat);
            }
        }
        let seated = self.seats.len() as u32;
        if seated > self.provisioned {
            self.grow_warden(seated - self.provisioned);
            self.provisioned = seated;
        }
        if self.seats.is_empty() {
            return snd;
        }
        self.tick_foes();

        // Who is ahead of whom, for the wake behind a pack.
        let snap: Vec<(u32, i32, i32, i32, i32)> = self
            .seats
            .iter()
            .map(|(&id, s)| {
                let r = &s.rider;
                let live = r.down == 0 && r.finish.is_none();
                (id, r.x / Q, r.y / Q, r.dir, if live { r.speed() } else { 0 })
            })
            .collect();

        let ids: Vec<u32> = self.seats.keys().copied().collect();
        for id in ids {
            let Some(mut seat) = self.seats.remove(&id) else { continue };
            let input = inputs.get(&id).copied().filter(|i| i.valid()).unwrap_or_default();
            let keys = Keys::of(&input);
            if self.tally.is_some() && input.interact && !seat.rider.jump_key {
                if self.elapsed >= self.tally.as_ref().map_or(0, |t| t.at) + TALLY_HOLD {
                    self.again = true;
                }
            }
            self.ride(id, &mut seat.rider, keys, &snap, n, &mut snd);
            self.seats.insert(id, seat);
        }
        self.tick_dimming(n);
        self.tick_bell();
        let mut seen = Vec::new();
        snd.retain(|s| {
            let fresh = !seen.contains(s);
            seen.push(*s);
            fresh
        });
        snd
    }

    /// Foes' own clocks: flinches fade, smoke clears, a popped foe comes back
    /// if no knight is by.
    fn tick_foes(&mut self) {
        let lvl = level();
        self.warden_inv = self.warden_inv.saturating_sub(1);
        for i in 0..self.foes.len() {
            let spot = lvl.foes[i];
            let st = &mut self.foes[i];
            st.flinch = st.flinch.saturating_sub(1);
            if st.hp == 0 && spot.kind == FoeKind::Warden && st.pop > 0 && st.pop < WARDEN_FALL {
                st.pop += 1;
            }
            if st.hp == 0 && st.pop > 0 && spot.kind != FoeKind::Warden {
                st.pop -= 1;
                if st.pop == 0 {
                    let (fx, _, _) = foe_at(lvl, &spot, self.elapsed);
                    let near = self.seats.values().any(|s| (s.rider.x / Q - fx).abs() < 160);
                    if near {
                        self.foes[i].pop = 20;
                    } else {
                        self.foes[i].hp = 1;
                    }
                }
            }
        }
    }

    fn tick_dimming(&mut self, n: &Numbers) {
        if self.bell_t.is_some() {
            return;
        }
        let seated = self.seats.len() as i32;
        let act = Self::act_at(self.dim / Q);
        let pace = n.dimming * (100 + SEAT_PACE * (seated - 1).max(0)) / 100 * ACT_PACE[act] / 100;
        self.dim += pace;
        // Never so far behind the rearmost knight that the chase is lost.
        let rear = self.seats.values().filter(|s| s.rider.finish.is_none() && s.rider.down == 0).map(|s| s.rider.x).min();
        if let Some(rear) = rear {
            self.dim = self.dim.max(rear - LEAD_MAX * Q);
        }
    }

    fn tick_bell(&mut self) {
        let Some(t0) = self.bell_t else { return };
        if self.tally.is_some() {
            return;
        }
        let all = self.seats.values().all(|s| s.rider.finish.is_some());
        if all || self.elapsed >= t0 + GRACE {
            self.make_tally();
        }
    }

    /// The pack's slipstream: is a faster rider just ahead?
    fn drafting(snap: &[(u32, i32, i32, i32, i32)], id: u32, r: &Rider) -> bool {
        let (x, y) = (r.x / Q, r.y / Q);
        r.down == 0
            && r.finish.is_none()
            && snap.iter().any(|&(o, ox, oy, od, ospeed)| {
                o != id
                    && od == r.dir
                    && ospeed >= 640
                    && (oy - y).abs() <= 28
                    && (ox - x) * r.dir >= 14
                    && (ox - x) * r.dir <= 110
            })
    }

    /// One knight's tick.
    fn ride(
        &mut self,
        id: u32,
        r: &mut Rider,
        keys: Keys,
        snap: &[(u32, i32, i32, i32, i32)],
        n: &Numbers,
        snd: &mut Vec<&'static str>,
    ) {
        let lvl = level();
        let t = &lvl.terrain;
        if r.down > 0 {
            r.jump_key = keys.jump;
            r.spur_key = keys.spur;
            r.tumble();
            r.tick_loose(t);
            if r.down == 0 {
                self.carry_home(r);
                snd.push("potion");
            }
            return;
        }
        r.draft = Self::drafting(snap, id, r);
        self.steer_lock(r);
        let c0 = r.centre(t);
        let mut ev = Vec::new();
        r.step(keys, t, n, &mut ev);
        for e in &ev {
            if let Ev::Land(v) = e {
                if *v > 2600 {
                    r.shake = r.shake.max(4);
                }
            }
            snd.push(match e {
                Ev::Jump => "step_wood",
                Ev::Land(v) if *v > 1500 => "rock_land",
                Ev::Land(_) => continue,
                Ev::Roll => "roll",
                Ev::Rev(_) => "shot_spark",
                Ev::Launch => "engine_whistle",
                Ev::Spur => "spell_fire",
                Ev::Skid => "step_gravel",
                Ev::Loop => "wheel_spin",
                Ev::Fell => "knight_hit",
            });
        }
        if r.want_lock {
            r.want_lock = false;
            self.begin_lock(r, n, snd);
        }
        let c1 = r.centre(t);
        self.wall(r);
        // A knight who has rung the bell is past the Gloam's reach and every
        // foe's: the celebration is not interrupted.
        if r.finish.is_none() {
            self.touch_foes(r, c0, c1, snd);
        }
        if r.down > 0 {
            return;
        }
        self.touch_things(r, c0, c1, snd);
        r.tick_loose(t);
        self.take_loose(r, snd);
        // The Dimming takes the slowest, and the world's floor takes the rest.
        if r.finish.is_none() && (r.x < self.dim || r.y > lvl.dead_y * Q) {
            self.swallow(r, snd);
        }
    }

    /// The gate's warden bars the road: nothing rides past it alive.
    fn wall(&mut self, r: &mut Rider) {
        let lvl = level();
        let Some((i, w)) = lvl.foes.iter().enumerate().find(|(_, f)| f.kind == FoeKind::Warden) else {
            return;
        };
        if self.foes[i].hp == 0 || r.x / Q <= w.x + 6 {
            return;
        }
        let limit = w.x + 6;
        match r.ground {
            Some(g) => {
                let p = &lvl.terrain.paths[g.path as usize];
                let s = p.s_at_x(limit).min(g.s);
                r.ground = Some(Ground { path: g.path, s });
                let at = p.sample(s);
                r.x = at.x;
                r.y = at.y;
                r.gsp = r.gsp.min(0);
            }
            None => {
                r.x = limit * Q;
                r.vx = r.vx.min(0);
            }
        }
    }

    /// Point a lance-lock in flight at its foe, or give it up.
    fn steer_lock(&mut self, r: &mut Rider) {
        let lvl = level();
        let Some(lock) = r.lock else { return };
        let i = lock.target as usize;
        if i >= self.foes.len() || self.foes[i].hp == 0 || !self.awake(i) || lock.ticks == 0 {
            r.lock = None;
            r.lock_cd = 8;
            return;
        }
        let spot = &lvl.foes[i];
        let (fx, fy, _) = foe_at(lvl, spot, self.elapsed);
        let (_, fh) = foe_body(spot.kind);
        let cen = r.centre(&lvl.terrain);
        let (dx, dy) = (fx * Q - cen.0, (fy - fh / 2) * Q - cen.1);
        let (ux, uy) = math::unit(i64::from(dx), i64::from(dy));
        r.vx = ux * LOCK_SPEED / math::ONE;
        r.vy = uy * LOCK_SPEED / math::ONE;
        if dx.abs() > Q {
            r.dir = dx.signum();
        }
        if let Some(l) = &mut r.lock {
            l.ticks -= 1;
        }
    }

    /// The nearest foe in the lance's cone for a rider in the air: ahead of
    /// the horse (a little behind still counts), inside a cone about sixty
    /// degrees to either side of level, within the lock's reach.
    pub fn find_lock(&self, r: &Rider, n: &Numbers) -> Option<usize> {
        let lvl = level();
        let cen = r.centre(&lvl.terrain);
        let reach = i64::from(n.lock_reach) * i64::from(Q);
        let window = Level::near(&lvl.foes, cen.0 / Q, n.lock_reach + 60, |f| f.x);
        let mut best: Option<(i64, usize)> = None;
        for i in window {
            let spot = &lvl.foes[i];
            if spot.kind == FoeKind::Bramble || !self.awake(i) || self.foes[i].hp == 0 {
                continue;
            }
            let (fx, fy, _) = foe_at(lvl, spot, self.elapsed);
            let (_, fh) = foe_body(spot.kind);
            let (dx, dy) = (i64::from(fx * Q - cen.0), i64::from((fy - fh / 2) * Q - cen.1));
            if dx * i64::from(r.dir) < -i64::from(6 * Q) {
                continue;
            }
            if dy.abs() > dx.abs() * 2 + i64::from(24 * Q) {
                continue;
            }
            let d2 = dx * dx + dy * dy;
            if d2 > reach * reach {
                continue;
            }
            if best.is_none_or(|(b, _)| d2 < b) {
                best = Some((d2, i));
            }
        }
        best.map(|(_, i)| i)
    }

    /// A press of jump in the air with no leap left: lock the nearest foe in
    /// the lance's cone, or lunge once.
    fn begin_lock(&mut self, r: &mut Rider, n: &Numbers, snd: &mut Vec<&'static str>) {
        if r.lock_cd > 0 || r.lock.is_some() || r.grounded() {
            return;
        }
        match self.find_lock(r, n) {
            Some(i) => {
                r.lock = Some(Lock { target: i as u16, ticks: LOCK_TICKS });
                snd.push("swing");
            }
            None if !r.lunged => {
                r.lunge = LUNGE_TICKS;
                r.lunged = true;
                snd.push("swing");
            }
            None => {}
        }
    }

    /// What the rider's swept body does to the foes it meets.
    fn touch_foes(&mut self, r: &mut Rider, c0: (i32, i32), c1: (i32, i32), snd: &mut Vec<&'static str>) {
        let lvl = level();
        let (hw, h) = r.body();
        let (lox, hix) = (c0.0.min(c1.0) - hw * Q, c0.0.max(c1.0) + hw * Q);
        let (loy, hiy) = (c0.1.min(c1.1) - h * Q / 2, c0.1.max(c1.1) + h * Q / 2);
        let window = Level::near(&lvl.foes, c1.0 / Q, 200, |f| f.x);
        for i in window {
            let spot = lvl.foes[i];
            if !self.awake(i) || self.foes[i].hp == 0 {
                continue;
            }
            let (fx, fy, _) = foe_at(lvl, &spot, self.elapsed);
            let (fw, fh) = foe_body(spot.kind);
            if hix < (fx - fw) * Q || lox > (fx + fw) * Q || hiy < (fy - fh) * Q || loy > fy * Q {
                continue;
            }
            match spot.kind {
                FoeKind::Bramble => self.hurt(r, fx, snd),
                FoeKind::Warden => {
                    if r.attacking() {
                        self.hit_warden(i, r, snd);
                    } else {
                        self.hurt(r, fx, snd);
                    }
                    // Nothing passes: the blow throws the horse back.
                    if r.down == 0 {
                        r.ground = None;
                        r.roll = false;
                        r.vx = -r.dir.signum() * 700;
                        r.vy = -900;
                        r.ctrl = r.ctrl.max(4);
                    }
                }
                _ => {
                    if r.attacking() {
                        self.pop(i, r, snd);
                    } else {
                        self.hurt(r, fx, snd);
                    }
                }
            }
            if r.down > 0 {
                return;
            }
        }
    }

    /// A foe popped into cold smoke and a spark of returned light.
    fn pop(&mut self, i: usize, r: &mut Rider, snd: &mut Vec<&'static str>) {
        self.foes[i] = FoeState { hp: 0, pop: REFORM, flinch: 8 };
        let was_air = !r.grounded() || r.lock.is_some();
        let link = r.strike();
        r.freeze = 2;
        r.sparks += 1;
        r.collected += 1;
        if link && was_air {
            r.gauge = (r.gauge + CHAIN_GAUGE * r.chain.min(5) as i32).min(GAUGE_MAX);
            snd.push(CHAIN_PITCH[(r.chain.saturating_sub(1) as usize).min(CHAIN_PITCH.len() - 1)]);
        } else {
            r.gauge = (r.gauge + KILL_GAUGE).min(GAUGE_MAX);
            snd.push("kill");
        }
    }

    fn hit_warden(&mut self, i: usize, r: &mut Rider, snd: &mut Vec<&'static str>) {
        if self.warden_inv > 0 {
            return;
        }
        self.warden_inv = WARDEN_INV;
        let st = &mut self.foes[i];
        st.hp = st.hp.saturating_sub(1);
        st.flinch = 12;
        r.strike();
        r.freeze = 4;
        r.shake = 10;
        r.gauge = (r.gauge + 120).min(GAUGE_MAX);
        if st.hp == 0 {
            st.pop = 1;
            snd.push("boss_death");
            // The room's last foe is down: its light is given back to every
            // knight at the gate, and the spur refilled for the run to the bell.
            r.sparks += WARDEN_LIGHT;
            r.collected += WARDEN_LIGHT;
            r.gauge = GAUGE_MAX;
            for seat in self.seats.values_mut() {
                seat.rider.sparks += WARDEN_LIGHT;
                seat.rider.collected += WARDEN_LIGHT;
                seat.rider.gauge = GAUGE_MAX;
            }
        } else {
            snd.push("boss_roar");
        }
    }

    /// A blow lands on `r` from a foe at column `from`.
    fn hurt(&mut self, r: &mut Rider, from: i32, snd: &mut Vec<&'static str>) {
        match r.knock(from * Q) {
            Blow::None => {}
            Blow::Shielded => snd.push("shield_block"),
            Blow::Scattered(_) => {
                // The sparks chime as they fly.
                snd.push("hero_hurt");
                snd.push("spoil_pickup");
            }
            Blow::Down => {
                r.shake = 12;
                snd.push("knight_fall");
            }
        }
    }

    /// The Dimming has the rider.
    fn swallow(&mut self, r: &mut Rider, snd: &mut Vec<&'static str>) {
        if r.down > 0 {
            return;
        }
        r.sparks = 0;
        r.falls += 1;
        r.hits += 1;
        r.down = DOWN_TICKS;
        r.vx = 0;
        r.vy = -1200;
        r.ground = None;
        r.roll = false;
        r.ball = false;
        r.lock = None;
        snd.push("boss_roar");
    }

    /// The lantern carries a fallen rider to the last post and holds the
    /// Dimming back from it.
    fn carry_home(&mut self, r: &mut Rider) {
        let lvl = level();
        let post = self.last_post();
        let (path, s) = lvl.spawn(post);
        r.place(&lvl.terrain, path, s);
        let (at, hold) = post.map_or((lvl.start_x(), HOLD), |n| (lvl.posts[n].x, self.hold(n)));
        self.dim = self.dim.min((at - hold * 4 / 5) * Q);
    }

    /// Sparks, springs, pads, posts, crates and the bell.
    fn touch_things(&mut self, r: &mut Rider, c0: (i32, i32), c1: (i32, i32), snd: &mut Vec<&'static str>) {
        let lvl = level();
        let (hw, h) = r.body();
        let (lox, hix) = (c0.0.min(c1.0) - hw * Q, c0.0.max(c1.0) + hw * Q);
        let (loy, hiy) = (c0.1.min(c1.1) - h * Q / 2, c0.1.max(c1.1) + h * Q / 2);
        let over = |l: i32, rr: i32, tp: i32, bt: i32| hix >= l * Q && lox <= rr * Q && hiy >= tp * Q && loy <= bt * Q;
        let cx = c1.0 / Q;

        // Sparks: a bit each, taken once by this knight.
        let mut got = 0;
        for i in Level::near(&lvl.sparks, cx, 60, |s| s.0) {
            let (sx, sy) = lvl.sparks[i];
            if r.taken[i / 64] >> (i % 64) & 1 == 1 {
                continue;
            }
            if over(sx - 6, sx + 6, sy - 10, sy + 10) {
                r.taken[i / 64] |= 1 << (i % 64);
                r.sparks += 1;
                r.collected += 1;
                got += 1;
            }
        }
        if got > 0 {
            r.gauge = (r.gauge + SPARK_GAUGE * got).min(GAUGE_MAX);
            snd.push("gold");
        }

        // Hay-bale springs.
        for i in Level::near(&lvl.springs, cx, 40, |s| s.x) {
            let sp = lvl.springs[i];
            if r.spring_cd == 0 && over(sp.x - 14, sp.x + 14, sp.y - 18, sp.y + 4) {
                let carry = if r.grounded() { r.gsp.abs().max(r.vx.abs()) } else { r.vx.abs() };
                let dir = if r.dir >= 0 { 1 } else { -1 };
                r.ground = None;
                r.roll = false;
                r.ball = false;
                r.jumping = false;
                r.coyote = 0;
                r.spring_cd = 8;
                r.lunge = 0;
                r.lunged = false;
                match sp.kind {
                    SpringKind::Up => {
                        r.vx = dir * carry.min(1800);
                        r.vy = -SPRING_UP;
                    }
                    SpringKind::Diag => {
                        r.vx = dir * carry.clamp(SPRING_DIAG.0, 2400);
                        r.vy = -SPRING_DIAG.1;
                    }
                }
                self.spring_t[i] = self.elapsed + 1;
                snd.push("wall_up");
            }
        }

        // Boost pads.
        for i in Level::near(&lvl.pads, cx, 40, |p| p.x) {
            let p = lvl.pads[i];
            if r.grounded() && r.gsp >= 0 && over(p.x - 18, p.x + 18, p.y - 12, p.y + 6) {
                if r.gsp < PAD_SPEED {
                    r.gsp = PAD_SPEED;
                    r.dir = 1;
                    self.pad_t[i] = self.elapsed + 1;
                    snd.push("vent_fire");
                }
            }
        }

        // Lantern posts: any knight lights them for everyone.
        for (i, p) in lvl.posts.iter().enumerate() {
            if self.posts >> i & 1 == 0 && over(p.x - 22, p.x + 22, p.y - 70, p.y + 10) {
                self.posts |= 1 << i;
                self.post_t[i] = self.elapsed + 1;
                self.dim = self.dim.min((p.x - self.hold(i)) * Q);
                r.gauge = (r.gauge + POST_GAUGE).min(GAUGE_MAX);
                snd.push("circle_toll");
            }
        }

        // Lantern crates: broken by anything that attacks.
        for (i, c) in lvl.crates.iter().enumerate() {
            if r.crates >> i & 1 == 0 && r.attacking() && over(c.x - 12, c.x + 12, c.y - 24, c.y) {
                r.crates |= 1 << i;
                match c.kind {
                    CrateKind::Shield => {
                        r.shield = true;
                        snd.push("shield_up");
                    }
                    CrateKind::Sparks => {
                        r.sparks += 10;
                        r.collected += 10;
                        snd.push("chest_open");
                    }
                    CrateKind::Spur => {
                        r.gauge = GAUGE_MAX;
                        snd.push("chest_open");
                    }
                }
                if !r.grounded() && r.vy > 0 {
                    r.vy = -BOUNCE;
                }
            }
        }

        // The bell, once the warden has fallen.
        let open = lvl
            .foes
            .iter()
            .enumerate()
            .all(|(i, f)| f.kind != FoeKind::Warden || self.foes[i].hp == 0);
        let (bx, by) = lvl.bell;
        if open && r.finish.is_none() && over(bx - 28, bx + 28, by - 120, by + 20) {
            r.finish = Some(self.elapsed);
            self.bell_t.get_or_insert(self.elapsed);
            snd.push("circle_toll");
        }
    }

    /// Take back sparks thrown by a hit, once they have settled a moment.
    fn take_loose(&mut self, r: &mut Rider, snd: &mut Vec<&'static str>) {
        let c = r.centre(&level().terrain);
        let before = r.loose.len();
        let mut got = 0;
        r.loose.retain(|l| {
            let near = l.age >= LOOSE_DELAY && (l.x - c.0).abs() < 18 * Q && (l.y - c.1).abs() < 24 * Q;
            got += u32::from(near);
            !near
        });
        if got > 0 && r.loose.len() < before {
            r.sparks += got;
            snd.push("gold");
        }
    }

    /// Reckon the ride: the pack's time, sparks, chain and posts.
    fn make_tally(&mut self) {
        let lvl = level();
        let now = self.elapsed;
        for s in self.seats.values_mut() {
            if s.rider.finish.is_none() {
                s.rider.finish = Some(now);
            }
        }
        let seated = self.seats.len().max(1) as u32;
        let time = self.seats.values().filter_map(|s| s.rider.finish).max().unwrap_or(now);
        let sparks = self.seats.values().map(|s| s.rider.collected).sum::<u32>() / seated;
        let chain = self.seats.values().map(|s| s.rider.best_chain).max().unwrap_or(0);
        let posts = self.posts.count_ones();
        let falls = self.seats.values().map(|s| s.rider.falls).sum::<u32>();
        let total = lvl.sparks.len().max(1) as u32;
        let secs = time / HZ;
        let time_pts = 400 * (SLOW.saturating_sub(secs)).min(SLOW - FAST) / (SLOW - FAST);
        let personal = |collected: u32, best_chain: u32, falls: u32, finish: u32| {
            let s = finish / HZ;
            let t = 400 * (SLOW.saturating_sub(s)).min(SLOW - FAST) / (SLOW - FAST);
            (t + (300 * collected / total).min(300) + 25 * best_chain.min(8)).saturating_sub(falls * 15)
        };
        let points = (time_pts
            + (300 * sparks / total).min(300)
            + 25 * chain.min(8)
            + 100 * posts / lvl.posts.len().max(1) as u32)
            .saturating_sub((falls * 15).min(100))
            .min(1000);
        let rank = RANKS.iter().position(|&line| points >= line).map_or(0, |i| 3 - i as u8);
        let scores: BTreeMap<u32, u32> = self
            .seats
            .iter()
            .map(|(&id, s)| {
                let r = &s.rider;
                (id, personal(r.collected, r.best_chain, r.falls, r.finish.unwrap_or(now)).min(1000))
            })
            .collect();
        let best = scores.iter().max_by_key(|&(&id, &v)| (v, std::cmp::Reverse(id))).map_or(0, |(&id, _)| id);
        self.tally = Some(Tally { at: now, time, sparks, chain, posts, falls, points, rank, best, scores });
    }

    pub fn valid(&self) -> bool {
        let lvl = level();
        let t = &lvl.terrain;
        self.elapsed <= LIMIT
            && self.seats.len() <= MAX_PLAYERS as usize
            && self.seats.iter().all(|(&id, s)| {
                (1..=MAX_PLAYERS).contains(&id) && s.joined <= self.elapsed && s.rider.valid(t)
            })
            && (-4000 * Q..=30000 * Q).contains(&self.dim)
            && self.posts < 1 << lvl.posts.len()
            && self.post_t.len() == lvl.posts.len()
            && self.post_t.iter().all(|&p| p <= self.elapsed + 1)
            && (1..=MAX_PLAYERS).contains(&self.provisioned)
            && self.foes.len() == lvl.foes.len()
            && self.foes.iter().zip(&lvl.foes).all(|(f, spot)| {
                f.hp <= if spot.kind == FoeKind::Warden { WARDEN_BASE + MAX_PLAYERS as u8 - 1 } else { 1 }
                    && f.pop <= REFORM
                    && f.flinch <= 16
            })
            && self.spring_t.len() == lvl.springs.len()
            && self.pad_t.len() == lvl.pads.len()
            && self.spring_t.iter().chain(&self.pad_t).all(|&p| p <= self.elapsed + 1)
            && self.warden_inv <= WARDEN_INV
            && self.bell_t.is_none_or(|b| b <= self.elapsed)
            && self.tally.as_ref().is_none_or(|y| {
                y.at <= self.elapsed && y.points <= 1000 && y.rank <= 3 && y.scores.len() <= MAX_PLAYERS as usize
                    && y.scores.values().all(|&v| v <= 1000)
            })
            && (self.tally.is_none() || self.bell_t.is_some())
    }

    /// Mend what a bad save broke: whatever cannot be trusted is put back as
    /// at the start of the road.
    pub fn repair(&mut self) {
        let lvl = level();
        self.seats.retain(|&id, _| (1..=MAX_PLAYERS).contains(&id));
        self.elapsed = self.elapsed.min(LIMIT);
        let elapsed = self.elapsed;
        for (&id, seat) in &mut self.seats {
            seat.joined = seat.joined.min(elapsed);
            if !seat.rider.valid(&lvl.terrain) {
                let (path, s) = ride_from(None, id);
                seat.rider = Rider::new_on(&lvl.terrain, path, s);
            }
        }
        self.dim = self.dim.clamp(-4000 * Q, 30000 * Q);
        self.posts &= (1u32 << lvl.posts.len()) - 1;
        self.post_t.resize(lvl.posts.len(), 0);
        self.post_t.iter_mut().for_each(|p| *p = (*p).min(elapsed + 1));
        self.provisioned = self.provisioned.clamp(1, MAX_PLAYERS);
        if self.foes.len() != lvl.foes.len() {
            self.foes = lvl
                .foes
                .iter()
                .map(|f| FoeState { hp: if f.kind == FoeKind::Warden { WARDEN_BASE } else { 1 }, pop: 0, flinch: 0 })
                .collect();
            self.grow_warden(self.provisioned - 1);
        }
        for (f, spot) in self.foes.iter_mut().zip(&lvl.foes) {
            let most = if spot.kind == FoeKind::Warden { WARDEN_BASE + MAX_PLAYERS as u8 - 1 } else { 1 };
            f.hp = f.hp.min(most);
            f.pop = f.pop.min(REFORM);
            f.flinch = f.flinch.min(16);
        }
        self.spring_t.resize(lvl.springs.len(), 0);
        self.pad_t.resize(lvl.pads.len(), 0);
        self.spring_t.iter_mut().chain(self.pad_t.iter_mut()).for_each(|p| *p = (*p).min(elapsed + 1));
        self.warden_inv = self.warden_inv.min(WARDEN_INV);
        if self.bell_t.is_some_and(|b| b > elapsed) {
            self.bell_t = None;
        }
        if self.bell_t.is_none() || self.tally.as_ref().is_some_and(|y| y.points > 1000 || y.rank > 3 || y.at > elapsed) {
            self.tally = None;
        }
    }
}

/// The sounds of a chain of lance-locks, rising a step with each link.
pub const CHAIN_PITCH: [&str; 5] = ["hit_magic", "card_pickup", "gold", "spoil_pickup", "wish_grant"];
