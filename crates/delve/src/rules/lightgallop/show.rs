//! What the picture remembers of the ride from one tick to the next: the
//! moments a knight should *feel*. The rules (the Bend module) know nothing
//! of it. After every tick the session looks at what changed between the
//! last tick and this (a spark taken, a foe popped, a blow, a loop closed, a
//! lantern lit, the warden down) and writes a short record of it here. The
//! picture reads the records by their age, so it is a pure function of the
//! session, and a friend's mirror and a browser guest see the same moments.
//!
//! Time is the session's own tick count. Nothing here moves a rider, and the
//! module never sees it (`ask` leaves it out of the request).
use super::level::level;
use super::math::Q;
use super::rider::{Rider, SPARK_WORDS};
use super::session::Session;
use super::world::{FoeKind, foe_at, foe_body};
use crate::rules::MAX_PLAYERS;
use crate::rules::juice::Weight;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The longest any record is worth showing, ticks.
pub const LIFE: u32 = 90;
/// The most records kept: the oldest go first.
const MOST: usize = 72;
/// A loop shorter than this many ticks is a clip of a ring, not a loop.
const LOOP_MIN: u32 = 14;
/// The pace (1/256 pixel a tick) a rider has to break to raise the wind.
const FAST: i32 = 1500;

/// What happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// A spark taken off the road, at the spark.
    Spark,
    /// A foe popped (`n`: 0 snuffer, 1 floater, 2 bramble or warden).
    Pop,
    /// A blow on a rider (`n`: sparks it scattered, 0 for a shield).
    Hit,
    /// A rider fell, or the Dimming had them.
    Fall,
    /// The warden took a blow (`n`: hits it has left).
    Warden,
    /// The warden is down: the room's last blow.
    WardenDown,
    /// A lantern post lit (`n`: which).
    Lantern,
    /// The first bell.
    Bell,
    /// A rider ran up into a loop.
    LoopIn,
    /// A rider came out of a loop (`n`: ticks it spent in it).
    LoopOut,
    /// Hooves down after air (`n`: how hard, whole pixels a tick).
    Land,
    /// Hooves up off the road.
    Hop,
    /// The spur lit.
    Spur,
    /// A rider broke into a faster pace: the wind comes up.
    Whoosh,
    /// A hay spring fired.
    Spring,
    /// A boost pad fired.
    Pad,
    /// A lance-lock chain grew (`n`: its length).
    Chain,
    /// A lantern crate opened (`n`: 0 shield, 1 sparks, 2 spur).
    Crate,
    /// A rider crossed the line.
    Finish,
}

/// One thing that happened: what, to whom, where (pixels) and when.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fx {
    pub kind: Kind,
    /// The knight it happened to (0: the road's).
    pub seat: u8,
    pub n: u16,
    pub x: i32,
    pub y: i32,
    pub weight: Weight,
    /// The tick it happened on.
    pub born: u32,
}

impl Fx {
    pub fn age(&self, now: u32) -> u32 {
        now.saturating_sub(self.born)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Show {
    pub fx: Vec<Fx>,
    /// The tick each seat ran up into a loop.
    pub looping: BTreeMap<u32, u32>,
}

impl Show {
    /// Nothing to show: the save leaves it out.
    pub fn quiet(&self) -> bool {
        self.fx.is_empty() && self.looping.is_empty()
    }

    pub fn valid(&self, elapsed: u32) -> bool {
        self.fx.len() <= MOST
            && self.looping.len() <= MAX_PLAYERS as usize
            && self.looping.iter().all(|(id, &t)| (1..=MAX_PLAYERS).contains(id) && t <= elapsed)
            && self.fx.iter().all(|f| {
                f.born <= elapsed
                    && u32::from(f.seat) <= MAX_PLAYERS
                    && (-2000..=22000).contains(&f.x)
                    && (-2000..=3000).contains(&f.y)
            })
    }

    fn put(&mut self, kind: Kind, seat: u32, n: u32, at: (i32, i32), weight: Weight, now: u32) {
        if self.fx.len() >= MOST {
            self.fx.remove(0);
        }
        self.fx.push(Fx { kind, seat: seat as u8, n: n.min(u32::from(u16::MAX)) as u16, x: at.0, y: at.1, weight, born: now });
    }

    /// The records of `kind` still showing at `now`, newest last.
    pub fn of(&self, kind: Kind, now: u32) -> impl Iterator<Item = &Fx> {
        self.fx.iter().filter(move |f| f.kind == kind && f.age(now) < LIFE)
    }

    /// The heaviest blow that landed this very tick, which is what holds the
    /// room still. Only the winning blow stops everyone: a fall or a hit has
    /// the rider's own freeze, and the road never pauses.
    pub fn held(&self, now: u32) -> Option<Weight> {
        self.fx.iter().filter(|f| f.born == now && f.weight >= Weight::Finale).map(|f| f.weight).max()
    }

    /// Look at what changed between `old` and `new` (the tick just ridden)
    /// and write it down.
    pub fn observe(&mut self, old: &Session, new: &Session) {
        let lvl = level();
        let t = &lvl.terrain;
        let now = new.elapsed;
        self.fx.retain(|f| f.age(now) < LIFE);
        self.looping.retain(|id, _| new.seats.contains_key(id));
        let centre = |r: &Rider| {
            let c = r.centre(t);
            (c.0 / Q, c.1 / Q)
        };
        // The seat whose kill count grew: who a pop was for.
        let popper = new
            .seats
            .iter()
            .find(|(id, s)| old.seats.get(id).is_some_and(|o| s.rider.kills > o.rider.kills))
            .map_or(0, |(&id, _)| id);
        for (&id, seat) in &new.seats {
            let Some(was) = old.seats.get(&id) else { continue };
            let (r, o) = (&seat.rider, &was.rider);
            let at = centre(r);
            // Sparks taken off the road.
            let mut taken = 0;
            for w in 0..SPARK_WORDS {
                let mut fresh = r.taken[w] & !o.taken[w];
                while fresh != 0 && taken < 8 {
                    let i = w * 64 + fresh.trailing_zeros() as usize;
                    fresh &= fresh - 1;
                    if let Some(&p) = lvl.sparks.get(i) {
                        self.put(Kind::Spark, id, 0, p, Weight::Graze, now);
                        taken += 1;
                    }
                }
            }
            // A blow: sparks scattered, a shield spent, or a fall.
            if r.hurt > o.hurt {
                let lost = o.sparks.min(16);
                let weight = match lost {
                    0 => Weight::Graze,
                    1..=5 => Weight::Solid,
                    6..=11 => Weight::Crushing,
                    _ => Weight::Kill,
                };
                self.put(Kind::Hit, id, lost, at, weight, now);
            }
            if o.down == 0 && r.down > 0 {
                self.put(Kind::Fall, id, o.sparks.min(16), at, Weight::Kill, now);
            }
            if r.chain > o.chain && r.chain >= 2 {
                self.put(Kind::Chain, id, r.chain, at, Weight::Graze, now);
            }
            // Loops: in, and out with the loop made.
            if !o.looping && r.looping {
                self.looping.insert(id, now);
                self.put(Kind::LoopIn, id, 0, at, Weight::Graze, now);
            } else if o.looping && !r.looping {
                if let Some(since) = self.looping.remove(&id) {
                    if now - since >= LOOP_MIN && r.down == 0 {
                        self.put(Kind::LoopOut, id, now - since, at, Weight::Solid, now);
                    }
                }
            }
            // Hooves: down after air, and up.
            if !o.grounded() && r.grounded() && r.down == 0 && o.down == 0 {
                let fall = o.vy / Q;
                if fall >= 3 {
                    self.put(Kind::Land, id, fall as u32, (r.x / Q, r.y / Q), Weight::Graze, now);
                }
            }
            if o.grounded() && !r.grounded() && r.jumping && r.down == 0 {
                self.put(Kind::Hop, id, 0, (r.x / Q, r.y / Q), Weight::Graze, now);
            }
            if o.speed() < FAST && r.speed() >= FAST && r.down == 0 && !r.roll {
                self.put(Kind::Whoosh, id, 0, at, Weight::Graze, now);
            }
            if !o.spur && r.spur && r.down == 0 {
                self.put(Kind::Spur, id, 0, at, Weight::Graze, now);
            }
            if o.finish.is_none() && r.finish.is_some() {
                self.put(Kind::Finish, id, 0, at, Weight::Solid, now);
            }
            // Crates opened.
            let mut fresh = r.crates & !o.crates;
            while fresh != 0 {
                let i = fresh.trailing_zeros() as usize;
                fresh &= fresh - 1;
                if let Some(c) = lvl.crates.get(i) {
                    use super::world::CrateKind::*;
                    let n = match c.kind {
                        Shield => 0,
                        Sparks => 1,
                        Spur => 2,
                    };
                    self.put(Kind::Crate, id, n, (c.x, c.y - 10), Weight::Graze, now);
                }
            }
        }
        // Lanterns lit, by whoever came by.
        let nearest = |x: i32| {
            new.seats.iter().min_by_key(|(_, s)| (s.rider.x / Q - x).abs()).map_or(0, |(&id, _)| id)
        };
        let lit = new.posts & !old.posts;
        for (i, p) in lvl.posts.iter().enumerate() {
            if lit >> i & 1 == 1 {
                self.put(Kind::Lantern, nearest(p.x), i as u32, (p.x, p.y - 46), Weight::Graze, now);
            }
        }
        if old.bell_t.is_none() {
            if let Some(_) = new.bell_t {
                let (bx, by) = lvl.bell;
                self.put(Kind::Bell, 0, 0, (bx, by - 40), Weight::Finale, now);
            }
        }
        // Foes: popped, or the warden struck.
        for (i, spot) in lvl.foes.iter().enumerate() {
            let (was, now_hp) = (old.foes[i].hp, new.foes[i].hp);
            if was <= now_hp {
                continue;
            }
            let (fx, fy, _) = foe_at(lvl, spot, new.elapsed);
            let (_, fh) = foe_body(spot.kind);
            let at = (fx, fy - fh / 2);
            if spot.kind == FoeKind::Warden {
                if now_hp == 0 {
                    self.put(Kind::WardenDown, popper, 0, at, Weight::Finale, now);
                } else {
                    self.put(Kind::Warden, popper, u32::from(now_hp), at, Weight::Crushing, now);
                }
            } else {
                let n = match spot.kind {
                    FoeKind::Snuffer => 0,
                    FoeKind::Floater => 1,
                    _ => 2,
                };
                self.put(Kind::Pop, popper, n, at, Weight::Solid, now);
            }
        }
        // Springs and pads fired.
        for (i, p) in lvl.pads.iter().enumerate() {
            if new.pad_t[i] != old.pad_t[i] && new.pad_t[i] > 0 {
                self.put(Kind::Pad, nearest(p.x), i as u32, (p.x, p.y), Weight::Graze, now);
            }
        }
        for (i, p) in lvl.springs.iter().enumerate() {
            if new.spring_t[i] != old.spring_t[i] && new.spring_t[i] > 0 {
                self.put(Kind::Spring, nearest(p.x), i as u32, (p.x, p.y), Weight::Graze, now);
            }
        }
    }
}
