//! The rules Lanternward ran on before they moved into Bend, kept verbatim as
//! the parity oracle: the session, its tick and every file under
//! `rules/lanternward/` as they stood when the Rust rules were deleted. Only
//! the Round Table shell (registration, settlement, the run adapter), the
//! types the saves and the picture read and the picture's own queries were
//! left behind in the crate; the review fixtures (`review.rs`) play moments
//! on the shipped rules. The session here is its own
//! copy of the types, so a test steps the oracle and the shipped Bend rules on
//! the same inputs and compares the two sessions as JSON, tick by tick.
#![allow(dead_code, unused_imports, unused_variables, clippy::all)]
use angel_delve::rules::{HZ, Hero, Input, MAX_PLAYERS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub mod blow;
pub mod boss;
pub mod bot;
pub mod ctx;
pub mod drive;
pub mod foes;
pub mod kit;
pub mod props;
pub mod review;
pub mod stage;
pub mod world;

pub use blow::{Blow, Hit};
pub use boss::{Atk, Warden, Ws};
pub use ctx::{Banner, Hold, Party};
pub use foes::{Bk, Bolt, Danger, Fk, Foe, Fs};
pub use kit::{Act, Carry, Ctx, Lock, Seat};
pub use props::{Dk, Drop, Fuse, Fx, Fxk, Item, Pk, Prop};
// Named, so they win over the glob of the Delve's own `unit` and friends.
#[allow(unused_imports)]
use world::{
    PLANS, Plan, Q, ROOMS, TILE, TILE_SHIFT, angle_of, cos_q, dir, dist, dot, isqrt, middle, plan,
    sin_q, tile_of, turn, unit, wall,
};

/// The five numbers as the rules read them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    /// Lock-on reach in `Q` pixels.
    pub lock: i32,
    pub parry: u32,
    pub combo: u32,
    pub flame: u32,
    pub tell: u32,
}

impl Default for Rules {
    fn default() -> Self {
        Rules {
            lock: 112 * Q,
            parry: 8,
            combo: 14,
            flame: 150,
            tell: 56,
        }
    }
}

impl Rules {
    /// A big foe's tell for a table of `seats`: tighter the more knights,
    /// never under the floor.
    pub fn tell_for(&self, seats: u32) -> u32 {
        self.tell
            .saturating_sub(TELL_STEP * seats.saturating_sub(1))
            .max(TELL_FLOOR)
    }
}

// ----- the fixed numbers (pixels unless said; ticks are 1/30 s) -----------

/// A run never runs past an hour; a longer one is not a run to play.
pub const LIMIT: u32 = 60 * 60 * HZ;
/// Quarter hearts: five hearts to start, one more per container.
pub const HEART: u32 = 4;
pub const START_HP: u32 = 5 * HEART;
pub const LOW_HP: u32 = 2 * HEART;
pub const BEEP_EVERY: u32 = 40;
pub const SPAWN_INV: u32 = 60;
pub const REVIVE_HP: u32 = 3 * HEART;
pub const KNIGHT_R: i32 = 5 * Q;
pub const VIEW_W: i32 = 384;
pub const VIEW_H: i32 = 224;
pub const FX_CAP: usize = 80;
pub const DROP_CAP: usize = 32;
pub const BOLT_CAP: usize = 40;
pub const FOE_CAP: usize = 16;
pub const FUSE_CAP: usize = 8;
pub const PROP_CAP: usize = 160;
pub const BOMB_CAP: u32 = 10;

// the lock
pub const EASE_OUT: u32 = 10;
pub const RELOCK: u32 = 6;
pub const SNAP_TICKS: u32 = 6;
/// How far the view leans from the knight toward a locked foe, percent.
pub const LOCK_LEAN: i32 = 38;
/// How far the view may lean toward the awake Warden, in pixels.
pub const BOSS_PULL: i32 = 52;
pub const ORBIT_MIN: i32 = 12;

// walking, in `Q` pixels a tick
pub const WALK: i32 = 440;
pub const LOCKED_WALK: i32 = 340;
pub const GUARD_WALK: i32 = 230;
pub const CARRY_WALK: i32 = 260;
pub const CHARGE_WALK_AFTER: u32 = 12;

// the sword
pub const CHARGE_TICKS: u32 = 22;
pub const LUNGE_FORWARD: i32 = 128;
pub const LUNGE_WIND: u32 = 3;
pub const LUNGE_ACTIVE: u32 = 6;
pub const LUNGE_REST: u32 = 10;
pub const LUNGE_STEP: i32 = 1100;
pub const LUNGE_REACH: i32 = 30;
pub const LUNGE_DAMAGE: u32 = 3;
pub const SPIN_TICKS: u32 = 14;
pub const SPIN_REACH: i32 = 22;
pub const SPIN_DAMAGE: u32 = 3;
pub const PLUNGE_REACH: i32 = 18;
pub const PLUNGE_DAMAGE: u32 = 4;
pub const PLUNGE_FALL: i32 = 3;
pub const PLUNGE_REST: u32 = 8;

// the dodges
pub const ROLL_SPEED: i32 = 1100;
pub const ROLL_MOVE: u32 = 12;
pub const ROLL_TICKS: u32 = 15;
pub const ROLL_IFRAMES: (u32, u32) = (2, 10);
pub const HOP_SIDE: i32 = 640;
pub const HOP_BACK: i32 = 683;
pub const HOP_FORWARD: i32 = 640;
pub const HOP_TICKS: u32 = 18;
pub const HOP_HEIGHT: i32 = 14;
pub const HOP_IFRAMES: (u32, u32) = (2, 14);
pub const LOW_CLEAR: i32 = 5;

// the shield
pub const GUARD_REST: u32 = 10;
/// cos 65 degrees, in `Q`.
pub const SHIELD_COS: i32 = 108;
pub const SHIELD_REACH: i32 = 9;
pub const BLOCK_PUSH: i32 = 520;
pub const PARRY_FREEZE: u32 = 4;
pub const PARRY_FLASH: u32 = 6;
pub const REFLECT_COS: i32 = -64;

// the lantern
pub const FLARE_AFTER: u32 = 8;
pub const SPUTTER: u32 = 45;
pub const LAMP_REST: u32 = 14;
pub const BOLT_COST_PARTS: u32 = 5;
pub const LIGHT_SPEED: i32 = 1150;
pub const LIGHT_LIFE: u16 = 30;
pub const FLARE_R: i32 = 78;
pub const GLOW_R: i32 = 26;
pub const BRAZIER_R: i32 = 64;
/// How near a friend's flare must be to kindle a knight's own flame.
pub const KINDLE_R: i32 = 28;
pub const TAPER: u32 = 420;
pub const BURN: u16 = 600;
pub const BURN_STEP: u16 = 60;
pub const BURN_MIN: u16 = 300;
pub const SIGIL_TICKS: u16 = 24;

// being hurt
pub const HURT_TICKS: u32 = 10;
pub const HURT_INV: u32 = 36;
pub const HURT_PUSH: i32 = 560;
pub const FALL_TICKS: u32 = 18;
pub const FALL_DAMAGE: u32 = 2;
pub const SHAKE_HIT: u32 = 5;
pub const SHAKE_HEAVY: u32 = 8;

// the party
pub const ITEM_TICKS: u32 = 60;
pub const READ_TICKS: u32 = 120;
pub const CTX_REACH: i32 = 20;
/// cos 70 degrees, in `Q`.
pub const CTX_FRONT: i32 = 88;
pub const REVIVE_REACH: i32 = 24;
pub const REVIVE_HOLD: u32 = 45;
pub const DOWN_AUTO: u32 = 300;
pub const WIPE_TICKS: u32 = 24;
pub const EXIT_TICKS: u32 = 14;
pub const PICK_R: i32 = 10;
pub const DROP_LIFE: u16 = 900;
pub const PLATE_R: i32 = 9;
pub const PUSH_LEAN: u16 = 10;
pub const FUSE_TICKS: u8 = 75;
pub const BLAST_R: i32 = 38;
pub const BLAST_DAMAGE: u32 = 8;
pub const TELL_STEP: u32 = 3;
pub const TELL_FLOOR: u32 = 40;
/// Ticks after the Beacon's light before E leaves.
pub const FINALE_TICKS: u32 = 120;
/// How long the cameras look at a door or switch that came right.
pub const CUE_TICKS: u32 = 44;

// the small foes
pub const SNUFFER_HP: u32 = 6;
pub const TURRET_HP: u32 = 8;
pub const SNUFFER_DRIFT: i32 = 200;
pub const LUNGE_SPEED: i32 = 1400;
pub const LUNGE_TICKS: u16 = 8;
pub const LUNGE_LEN: i32 = 44;
pub const SNUFFER_TELL: u16 = 24;
pub const SNUFFER_RECOVER: u16 = 28;
pub const SNUFFER_DAMAGE: u32 = 2;
pub const AGGRO: i32 = 120;
pub const RING: i32 = 22;
pub const STRIKE_FROM: i32 = 50;
pub const TURRET_RANGE: i32 = 260;
pub const TURRET_RECOVER: u16 = 60;
pub const TURRET_STUN: u16 = 90;
pub const BOLT_SPEED: i32 = 620;
pub const BOLT_LIFE: u16 = 160;
pub const BOLT_DAMAGE: u32 = 2;
pub const BRIGHT_DAMAGE: u32 = 4;
pub const DIM_DAMAGE: u32 = 1;
pub const DIM_LIFE: u16 = 24;
pub const POT_DAMAGE: u32 = 2;
pub const POT_SPEED: i32 = 1400;
pub const POT_LIFE: u16 = 10;
pub const DEATH_TICKS: u8 = 14;
pub const STAGGER_TICKS: u16 = 30;
pub const LIT_HOLD: u16 = 60;
pub const FLINCH_TICKS: u8 = 8;
pub const HIT_FREEZE: u8 = 3;
pub const FOE_PUSH: i32 = 380;

// the Warden
pub const WARDEN_HP: u32 = 90;
pub const WARDEN_R: i32 = 18;
pub const WARDEN_WALK: i32 = 128;
pub const WARDEN_WAKE: i32 = 150;
pub const WARDEN_NAP: u32 = 150;
pub const HEART_R: i32 = 9;
pub const HEART_FLARE_R: i32 = 70;
pub const EXPOSE_TICKS: u32 = 90;
pub const STAGGER_BOSS: u16 = 75;
pub const BEAM_TICKS: u16 = 36;
pub const BEAM_HALF: i32 = 50;
pub const BEAM_LEN: i32 = 150;
pub const BEAM_DAMAGE: u32 = 4;
pub const SLAM_TICKS: u16 = 30;
pub const SLAM_SPEED: i32 = 4;
pub const SLAM_REACH: i32 = 132;
pub const SLAM_DAMAGE: u32 = 4;
pub const RECOVER_TICKS: u16 = 44;
pub const REST_TICKS: u16 = 60;
pub const BEACON_TICKS: u16 = 90;

/// What a tick did that the rest of the Delve must hear about.
#[derive(Default, Debug)]
pub struct Events {
    pub sounds: Vec<&'static str>,
    pub stop: Option<Stop>,
    /// The Beacon was relit this tick.
    pub won: bool,
}

/// A weight of impact, for the engine's own hit-stop (`hitstop`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stop {
    /// A blade landed: the engine's short hold, rate-limited by its cooldown.
    Heavy,
    /// A foe fell.
    Kill,
    Phase,
    Boss,
}

impl Events {
    pub fn sound(&mut self, name: &'static str) {
        if !self.sounds.contains(&name) {
            self.sounds.push(name);
        }
    }
    pub fn stop(&mut self, s: Stop) {
        self.stop = Some(self.stop.map_or(s, |o| o.max(s)));
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub seats: BTreeMap<u32, Seat>,
    pub elapsed: u32,
    pub room: u8,
    pub stage: u8,
    pub stage_t: u32,
    pub props: Vec<Prop>,
    pub foes: Vec<Foe>,
    pub bolts: Vec<Bolt>,
    pub drops: Vec<Drop>,
    pub fx: Vec<Fx>,
    pub fuses: Vec<Fuse>,
    pub boss: Option<Warden>,
    pub inv: Party,
    pub banner: Option<Banner>,
    pub seed: u64,
    pub shake: u32,
    pub flash: u32,
    pub exiting: u32,
    pub wipe: u32,
    pub tries: u32,
    /// Where every camera is looking for a moment: a door or switch that
    /// came right, and the ticks left.
    pub cue: Option<(i32, i32, u32)>,
    pub next_foe: u8,
    /// The tick the Beacon was relit, once it has been.
    pub done: Option<u32>,
    /// Seats that have been paid their purse, one bit each (seat 1 is bit 1).
    pub paid: u8,
    /// What a seat that stood up carried (gems, bombs), for when it sits down
    /// again.
    #[serde(default)]
    pub kept: BTreeMap<u32, (u32, u32)>,
}

impl Session {
    pub fn new(players: &BTreeMap<u32, Hero>) -> Self {
        let mut s = Self {
            seats: BTreeMap::new(),
            elapsed: 0,
            room: 0,
            stage: 0,
            stage_t: 0,
            props: Vec::new(),
            foes: Vec::new(),
            bolts: Vec::new(),
            drops: Vec::new(),
            fx: Vec::new(),
            fuses: Vec::new(),
            boss: None,
            inv: Party::new(),
            banner: None,
            seed: 0x4c41_4e54_4552_4e57,
            shake: 0,
            flash: 0,
            exiting: 0,
            wipe: 0,
            tries: 0,
            cue: None,
            next_foe: 0,
            done: None,
            paid: 0,
            kept: BTreeMap::new(),
        };
        for (&id, _) in players.iter().filter(|(_, h)| !h.away && !h.stone) {
            let at = s.start_of(id);
            s.seats.insert(id, Seat::new(id, 0, at));
        }
        s.enter_room(0, &Rules::default());
        s
    }

    /// One tick: whoever is here and awake sits at the table (a knight who
    /// leaves stands up, one who joins late sits down at the room's start
    /// and scales what is standing), then every body acts in its order.
    pub fn step(
        &mut self,
        inputs: &BTreeMap<u32, Input>,
        heroes: &BTreeMap<u32, Hero>,
        r: &Rules,
    ) -> Events {
        let mut ev = Events::default();
        self.elapsed = (self.elapsed + 1).min(LIMIT);
        let here = |id: &u32| heroes.get(id).is_some_and(|h| !h.away && !h.stone);
        for (&id, k) in self.seats.iter().filter(|(id, _)| !here(id)) {
            self.kept.insert(id, (k.gems, k.bombs));
        }
        self.seats.retain(|id, _| here(id));
        for (&id, _) in heroes.iter().filter(|(_, h)| !h.away && !h.stone) {
            if !self.seats.contains_key(&id) {
                let at = self.start_of(id);
                let mut k = Seat::new(id, self.elapsed, at);
                if let Some((gems, bombs)) = self.kept.remove(&id) {
                    (k.gems, k.bombs) = (gems, bombs);
                }
                k.max_hp = START_HP + HEART * u32::from(self.inv.containers);
                k.hp = k.max_hp;
                if self.inv.lantern {
                    k.flame = r.flame;
                }
                self.seats.insert(id, k);
            }
        }
        self.provision();
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        for id in ids {
            let inp = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .unwrap_or_default();
            self.step_seat(id, inp, r, &mut ev);
        }
        self.tick_props(&mut ev);
        self.unstick();
        self.step_foes(r, &mut ev);
        self.step_boss(r, &mut ev);
        self.step_bolts(r, &mut ev);
        self.step_fuses(&mut ev);
        self.pickups(r, &mut ev);
        self.revive_pass(&mut ev);
        self.tick_stage(r, &mut ev);
        self.tick_wipe(r, &mut ev);
        self.age();
        ev
    }

    /// `step`, with a snapshot of the session and the sounds so far after each
    /// phase of the tick, for localising a divergence. The phases and their
    /// order are `step`'s own.
    pub fn step_traced(
        &mut self,
        inputs: &BTreeMap<u32, Input>,
        heroes: &BTreeMap<u32, Hero>,
        r: &Rules,
        trace: &mut Vec<(&'static str, Session, Vec<&'static str>, Option<Stop>)>,
    ) -> Events {
        let mut ev = Events::default();
        self.elapsed = (self.elapsed + 1).min(LIMIT);
        let here = |id: &u32| heroes.get(id).is_some_and(|h| !h.away && !h.stone);
        for (&id, k) in self.seats.iter().filter(|(id, _)| !here(id)) {
            self.kept.insert(id, (k.gems, k.bombs));
        }
        self.seats.retain(|id, _| here(id));
        for (&id, _) in heroes.iter().filter(|(_, h)| !h.away && !h.stone) {
            if !self.seats.contains_key(&id) {
                let at = self.start_of(id);
                let mut k = Seat::new(id, self.elapsed, at);
                if let Some((gems, bombs)) = self.kept.remove(&id) {
                    (k.gems, k.bombs) = (gems, bombs);
                }
                k.max_hp = START_HP + HEART * u32::from(self.inv.containers);
                k.hp = k.max_hp;
                if self.inv.lantern {
                    k.flame = r.flame;
                }
                self.seats.insert(id, k);
            }
        }
        self.provision();
        let snap = |name: &'static str, s: &Session, ev: &Events, trace: &mut Vec<_>| {
            trace.push((name, s.clone(), ev.sounds.clone(), ev.stop));
        };
        snap("join", self, &ev, trace);
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        for id in ids {
            let inp = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .unwrap_or_default();
            self.step_seat(id, inp, r, &mut ev);
        }
        snap("seats", self, &ev, trace);
        self.tick_props(&mut ev);
        snap("props", self, &ev, trace);
        self.unstick();
        snap("unstick", self, &ev, trace);
        self.step_foes(r, &mut ev);
        snap("foes", self, &ev, trace);
        self.step_boss(r, &mut ev);
        snap("boss", self, &ev, trace);
        self.step_bolts(r, &mut ev);
        snap("bolts", self, &ev, trace);
        self.step_fuses(&mut ev);
        snap("fuses", self, &ev, trace);
        self.pickups(r, &mut ev);
        snap("pickups", self, &ev, trace);
        self.revive_pass(&mut ev);
        snap("revive", self, &ev, trace);
        self.tick_stage(r, &mut ev);
        snap("stage", self, &ev, trace);
        self.tick_wipe(r, &mut ev);
        snap("wipe", self, &ev, trace);
        self.age();
        snap("age", self, &ev, trace);
        ev
    }

    /// Whether the finale is on: the Beacon is lit and the party may leave.
    pub fn finale(&self) -> bool {
        self.done.is_some_and(|t| self.elapsed >= t + FINALE_TICKS)
    }

    pub fn valid(&self) -> bool {
        self.problem().is_none()
    }

    /// What is wrong with the session, if anything: the first bound it
    /// breaks, by name.
    pub fn problem(&self) -> Option<&'static str> {
        let p = plan(usize::from(self.room.min(4)));
        let (w, h) = (p.w() * TILE * Q, p.h() * TILE * Q);
        let inside = |x: i32, y: i32| (0..=w).contains(&x) && (0..=h).contains(&y);
        let checks: [(&'static str, bool); 23] = [
            ("clock", self.elapsed <= LIMIT && self.stage_t <= LIMIT),
            ("room", self.room < ROOMS as u8 && self.stage <= 6),
            ("seats", self.seats.len() <= MAX_PLAYERS as usize),
            (
                "knight",
                self.seats.iter().all(|(&id, k)| {
                    (1..=MAX_PLAYERS).contains(&id) && k.valid(id, self.elapsed, w, h)
                }),
            ),
            ("props", self.props.len() <= PROP_CAP),
            (
                "prop",
                self.props.iter().all(|q| {
                    i32::from(q.tx) < p.w()
                        && i32::from(q.ty) < p.h()
                        && q.on <= 4
                        && inside(q.x, q.y)
                }),
            ),
            ("foes", self.foes.len() <= FOE_CAP),
            (
                "foe",
                self.foes.iter().all(|f| {
                    f.hp <= f.full
                        && f.full <= TURRET_HP.max(SNUFFER_HP) * MAX_PLAYERS
                        && (1..=MAX_PLAYERS as u8).contains(&f.prov)
                        && inside(f.x, f.y)
                        && f.t <= 600
                        && f.dead <= DEATH_TICKS
                        && dist(f.fx, f.fy) <= Q + 8
                }),
            ),
            ("bolts", self.bolts.len() <= BOLT_CAP),
            (
                "bolt",
                self.bolts.iter().all(|b| {
                    inside(b.x, b.y)
                        && (-(8 * Q)..=8 * Q).contains(&b.vx)
                        && (-(8 * Q)..=8 * Q).contains(&b.vy)
                        && b.ttl <= 400
                        && u32::from(b.owner) <= MAX_PLAYERS
                }),
            ),
            (
                "drops",
                self.drops.len() <= DROP_CAP && self.drops.iter().all(|d| inside(d.x, d.y)),
            ),
            ("fx", self.fx.len() <= FX_CAP),
            (
                "fuses",
                self.fuses.len() <= FUSE_CAP
                    && self
                        .fuses
                        .iter()
                        .all(|f| inside(f.x, f.y) && f.t <= FUSE_TICKS),
            ),
            (
                "boss",
                self.boss
                    .as_ref()
                    .is_none_or(|b| self.room == 4 && b.valid() && inside(b.x, b.y)),
            ),
            ("keys", self.inv.small_keys <= 8 && self.inv.containers <= 8),
            ("shake", self.shake <= 64 && self.flash <= 64),
            (
                "exit",
                self.exiting <= EXIT_TICKS && self.wipe <= WIPE_TICKS,
            ),
            (
                "cue",
                self.cue
                    .is_none_or(|(x, y, t)| inside(x, y) && t <= CUE_TICKS),
            ),
            ("paid", self.paid & 1 == 0 && self.paid < 128),
            ("done", self.done.is_none_or(|t| t <= self.elapsed)),
            (
                "banner",
                self.banner.is_none_or(|b| u32::from(b.t) <= ITEM_TICKS),
            ),
            ("tries", self.tries < 1_000_000),
            (
                "kept",
                self.kept.len() <= MAX_PLAYERS as usize
                    && self.kept.iter().all(|(id, &(g, b))| {
                        (1..=MAX_PLAYERS).contains(id) && g <= 100_000 && b <= BOMB_CAP
                    }),
            ),
        ];
        checks.iter().find(|(_, ok)| !ok).map(|&(name, _)| name)
    }

    /// Mend what a bad save broke.
    pub fn repair(&mut self) {
        self.seats.retain(|&id, _| (1..=MAX_PLAYERS).contains(&id));
        self.elapsed = self.elapsed.min(LIMIT);
        let elapsed = self.elapsed;
        for seat in self.seats.values_mut() {
            seat.joined = seat.joined.min(elapsed);
        }
        if self.valid() {
            return;
        }
        // Whatever else is wrong, the room is rebuilt as it opens and the
        // party stands at its start with full hearts.
        self.room = self.room.min(ROOMS as u8 - 1);
        self.done = self.done.filter(|&t| t <= elapsed);
        self.paid &= 0b0111_1110;
        self.kept.retain(|id, _| (1..=MAX_PLAYERS).contains(id));
        self.fx.clear();
        self.shake = 0;
        self.flash = 0;
        self.banner = None;
        self.exiting = 0;
        self.wipe = 0;
        self.cue = None;
        self.inv.small_keys = self.inv.small_keys.min(8);
        self.inv.containers = self.inv.containers.min(8);
        let hearts = START_HP + HEART * u32::from(self.inv.containers);
        let room = usize::from(self.room);
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        for id in ids {
            let joined = self.seats[&id].joined;
            let at = self.start_of(id);
            let mut fresh = Seat::new(id, joined, at);
            fresh.max_hp = hearts;
            fresh.hp = hearts;
            self.seats.insert(id, fresh);
        }
        self.enter_room(room, &Rules::default());
    }
}
