//! What the picture remembers of the race from one tick to the next: the
//! moments a knight should *feel*. The race's rules (the Bend module, and the
//! Touge's native tick) know nothing of it. After every tick `observe` looks
//! at what changed between the field as it was and as it is (a wreck, a bump
//! of balls, a ring flown, a lead taken, a line crossed) and writes a short
//! record of it here. The picture reads the records by their age, so it is a
//! pure function of the race, and a friend's mirror and a browser guest see
//! the same moments the host does.
//!
//! Time is the race's own tick count. Nothing here moves a rider, and the
//! module never sees it (`tick_race3d` sets it aside while the leg runs).
use super::*;
use crate::rules::juice::Weight;

/// The longest any record is worth showing, ticks.
pub const LIFE: u32 = 80;
/// The most records kept: the oldest go first.
const MOST: usize = 56;
/// Two knights home within this many ticks of each other ran a photo finish.
pub const PHOTO: u32 = 15;
/// A rival must be this far clear (course units) for a pass to count, so a
/// pair running wheel to wheel never calls the lead back and forth.
const CLEAR: f32 = 0.9;
/// The pace (units a second) a wreck must be at to be a hard one, and a
/// crushing one.
const HARD: f32 = 6.0;
const CRUSHING: f32 = 12.0;
/// Ticks between near-miss cues for one knight.
const NEAR_REST: u32 = 40;

/// What happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// A knight wrecked (`wreck` says how).
    Wreck,
    /// Two balls or carts met (`other`: the second).
    Bump,
    /// A ball ran along a wall, a griffin brushed the gorge's edge.
    Scrape,
    /// Up off the rails or the planks.
    Hop,
    /// Down again (`n`: how hard, tenths of a unit a second).
    Land,
    /// Something picked up (`n`: 0 gem or nugget, 1 ring, 2 gargoyle).
    Pick,
    /// A burst of speed (a ring's, a gargoyle's, a drift's, a pad's).
    Burst,
    /// A griffin's barrel roll.
    Roll,
    /// A griffin hit by a bolt.
    Ruffle,
    /// A perfect shift: the Touge's kick.
    Kick,
    /// A knight passed another (`n`: their place now).
    Pass,
    /// A knight took the lead (`other`: the one who had it).
    Lead,
    /// Close to something, and clear (`n`: 0 stone, 1 timber, 2 cart, 3 edge).
    Near,
    /// A knight crossed the line (`n`: the place they came home in).
    Finish,
    /// The second knight home was this close to the first (`n`: ticks apart).
    Photo,
}

/// One thing that happened: what, to whom, where (course space) and when.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fx {
    pub kind: Kind,
    pub seat: u8,
    pub other: u8,
    pub n: u16,
    /// Along, across and up, as the rider's own `s`, `x` and `z`.
    pub at: [f32; 3],
    pub weight: Weight,
    pub wreck: Wreck,
    /// The tick it happened on.
    pub born: u32,
}

impl Fx {
    pub fn age(&self, now: u32) -> u32 {
        now.saturating_sub(self.born)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Show {
    pub fx: Vec<Fx>,
    /// Who is clear ahead of whom: `(a, b)` is a ahead of b. A pair is set
    /// down silently the first time they part, and a pass is called when it
    /// swaps by a clear margin.
    ahead: Vec<(u8, u8)>,
    /// The tick each knight last had a near-miss called, for the rest.
    last_near: Vec<(u8, u32)>,
}

impl Show {
    /// Nothing to show: the save leaves it out.
    pub fn quiet(&self) -> bool {
        *self == Self::default()
    }

    pub fn valid(&self, now: u32) -> bool {
        let seat = |id: u8| (1..=MAX_PLAYERS as u8).contains(&id);
        self.fx.len() <= MOST
            && self.ahead.len() <= (MAX_PLAYERS * MAX_PLAYERS) as usize
            && self.last_near.len() <= MAX_PLAYERS as usize
            && self.ahead.iter().all(|&(a, b)| seat(a) && seat(b) && a != b)
            && self.last_near.iter().all(|&(id, t)| seat(id) && t <= now)
            && self.fx.iter().all(|f| {
                f.born <= now
                    && (f.seat == 0 || seat(f.seat))
                    && (f.other == 0 || seat(f.other))
                    && f.at.iter().all(|v| v.is_finite() && v.abs() < 20_000.0)
            })
    }

    fn put(&mut self, kind: Kind, seat: u32, other: u32, n: u32, at: [f32; 3], weight: Weight, now: u32) {
        self.put_wreck(kind, seat, other, n, at, weight, Wreck::None, now);
    }

    #[allow(clippy::too_many_arguments)]
    fn put_wreck(&mut self, kind: Kind, seat: u32, other: u32, n: u32, at: [f32; 3], weight: Weight, wreck: Wreck, now: u32) {
        if self.fx.len() >= MOST {
            self.fx.remove(0);
        }
        self.fx.push(Fx {
            kind,
            seat: seat as u8,
            other: other as u8,
            n: n.min(u32::from(u16::MAX)) as u16,
            at,
            weight,
            wreck,
            born: now,
        });
    }

    /// The records of `kind` still showing at `now`, newest last.
    pub fn of(&self, kind: Kind, now: u32) -> impl Iterator<Item = &Fx> {
        self.fx.iter().filter(move |f| f.kind == kind && f.age(now) < LIFE)
    }

    /// The heaviest thing that happened this very tick, which is what holds
    /// the room still: a hard wreck, the first knight home. A graze never does.
    pub fn held(&self, now: u32) -> Option<Weight> {
        self.fx.iter().filter(|f| f.born == now).map(|f| f.weight).max()
    }

    /// How old the newest `kind` for `seat` is, if it is still showing.
    pub fn since(&self, kind: Kind, seat: u32, now: u32) -> Option<u32> {
        self.of(kind, now).filter(|f| u32::from(f.seat) == seat).map(|f| f.age(now)).min()
    }

    /// How far ahead of the rest a knight is not: the number of knights clear
    /// ahead of `id`, from the pairs the show keeps.
    pub fn ahead_of(&self, id: u32) -> usize {
        self.ahead.iter().filter(|&&(_, b)| u32::from(b) == id).count()
    }

    fn set_ahead(&mut self, a: u32, b: u32) -> bool {
        let (a, b) = (a as u8, b as u8);
        let swapped = self.ahead.contains(&(b, a));
        self.ahead.retain(|&(x, y)| !((x, y) == (a, b) || (x, y) == (b, a)));
        self.ahead.push((a, b));
        swapped
    }

    /// Look at what changed between `old` and `new` (the tick just run) and
    /// write it down.
    pub fn observe(&mut self, old: &Race3d, new: &Race3d) {
        let now = new.t;
        self.fx.retain(|f| f.age(now) < LIFE);
        self.last_near.retain(|&(id, _)| new.riders.contains_key(&u32::from(id)));
        self.ahead.retain(|&(a, b)| new.riders.contains_key(&u32::from(a)) && new.riders.contains_key(&u32::from(b)));
        let t = track(new.course);
        let pos = |r: &Rider| [r.s, r.x, r.z];
        let taken: Vec<u16> = new.taken.difference(&old.taken).copied().collect();
        for (&id, r) in &new.riders {
            let Some(o) = old.riders.get(&id) else { continue };
            let at = pos(r);
            // A wreck: how hard it was is how fast the knight was going.
            if o.crash == 0 && r.crash > 0 {
                let pace = o.vs.abs();
                let hard = !matches!(r.wreck, Wreck::Fall | Wreck::Gap);
                let weight = match () {
                    _ if !hard => Weight::Solid,
                    _ if pace >= CRUSHING => Weight::Kill,
                    _ if pace >= HARD => Weight::Crushing,
                    _ => Weight::Solid,
                };
                self.put_wreck(Kind::Wreck, id, 0, (pace * 10.0) as u32, at, weight, r.wreck, now);
            }
            // The ball and the cart: up, down, along the wall.
            if matches!(new.course, Course::Shaft | Course::Cart) && o.crash == 0 && r.crash == 0 {
                if o.z <= 0.0 && r.z > 0.0 && r.vz > 0.0 {
                    self.put(Kind::Hop, id, 0, 0, at, Weight::Graze, now);
                }
                if o.z > 0.0 && r.z <= 0.0 && o.vz < -1.0 {
                    self.put(Kind::Land, id, 0, (-o.vz * 10.0) as u32, at, Weight::Graze, now);
                }
            }
            if new.course == Course::Shaft && o.crash == 0 && r.crash == 0 && t.at(r.s).walls {
                let half = t.at(r.s).width * 0.5 - BALL_R;
                if r.x.abs() >= half - 0.002 && o.vx * r.x > 1.2 && r.vx * r.x <= 0.0 {
                    let hit = (o.vx.abs() * 10.0) as u32;
                    self.put(Kind::Scrape, id, 0, hit, at, if hit > 40 { Weight::Solid } else { Weight::Graze }, now);
                }
            }
            if new.course == Course::Griffin && o.crash == 0 && r.crash == 0 && r.x.abs() >= SKY_X - 0.01 && o.vx * r.x > 2.5 {
                self.put(Kind::Scrape, id, 0, (o.vx.abs() * 10.0) as u32, at, Weight::Graze, now);
            }
            // Pickups: the things that went into the taken set, and the rings.
            if r.haul > o.haul {
                let ring = r.rings & !o.rings;
                if ring != 0 {
                    let i = ring.trailing_zeros() as usize;
                    if let Some(item) = t.items.get(i) {
                        self.put(Kind::Pick, id, 0, 1, [item.s, item.x, item.z], Weight::Graze, now);
                    }
                }
                // Whatever went into the taken set this tick, to the knight
                // nearest it who gained one (a gem to its taker, a gargoyle
                // to the shooter).
                for &i in &taken {
                    let Some(item) = t.items.get(usize::from(i)) else { continue };
                    let nearest = new
                        .riders
                        .iter()
                        .filter(|(rid, rr)| rr.haul > old.riders.get(rid).map_or(0, |x| x.haul))
                        .min_by(|a, b| (a.1.s - item.s).abs().total_cmp(&(b.1.s - item.s).abs()))
                        .map(|(&rid, _)| rid);
                    if nearest == Some(id) {
                        let code = if item.kind == ItemKind::Gargoyle { 2 } else { 0 };
                        let weight = if code == 2 { Weight::Solid } else { Weight::Graze };
                        self.put(Kind::Pick, id, 0, code, [item.s, item.x, item.z], weight, now);
                    }
                }
            }
            // Speed given: a burst, a kick, a roll, a bolt in the feathers.
            if o.burst == 0 && r.burst > 0 {
                self.put(Kind::Burst, id, 0, 0, at, Weight::Graze, now);
            }
            if o.drift.boost == 0 && r.drift.boost > 0 {
                self.put(Kind::Burst, id, 0, u32::from(r.drift.boost_stage) + 1, at, Weight::Graze, now);
            }
            if o.drift.kick == 0 && r.drift.kick > 0 {
                self.put(Kind::Kick, id, 0, 0, at, Weight::Graze, now);
            }
            if o.rolling == 0 && r.rolling > 0 {
                self.put(Kind::Roll, id, 0, 0, at, Weight::Graze, now);
            }
            if o.stun == 0 && r.stun > 0 && r.crash == 0 && new.course == Course::Griffin {
                self.put(Kind::Ruffle, id, 0, 0, at, Weight::Solid, now);
            }
        }
        // Bumps between knights: the ball's push, the cart's shunt.
        let ids: Vec<u32> = new.riders.keys().copied().collect();
        for (k, &a) in ids.iter().enumerate() {
            for &b in &ids[k + 1..] {
                let (Some((oa, ob)), Some((ra, rb))) = (old.riders.get(&a).zip(old.riders.get(&b)), new.riders.get(&a).zip(new.riders.get(&b))) else {
                    continue;
                };
                let live = |r: &Rider| r.racing();
                if !(live(oa) && live(ob) && live(ra) && live(rb)) {
                    continue;
                }
                let (ds, dx) = (rb.s - ra.s, rb.x - ra.x);
                let near = match new.course {
                    Course::Shaft => ds.hypot(dx) < 1.2,
                    Course::Cart => ds.abs() < 1.7 && dx.abs() < 0.9,
                    _ => false,
                };
                if !near {
                    continue;
                }
                let jolt = |o: &Rider, r: &Rider| (r.vs - o.vs).hypot(r.vx - o.vx);
                let hit = jolt(oa, ra).max(jolt(ob, rb));
                if hit > if new.course == Course::Cart { 2.0 } else { 0.6 } {
                    let mid = [(ra.s + rb.s) * 0.5, (ra.x + rb.x) * 0.5, (ra.z + rb.z) * 0.5];
                    let weight = if hit > 2.5 { Weight::Solid } else { Weight::Graze };
                    self.put(Kind::Bump, a, b, (hit * 10.0) as u32, mid, weight, now);
                }
            }
        }
        self.near_misses(old, new, now);
        self.places(old, new, now);
        self.finishes(old, new, now);
    }

    /// Close to something, and clear of it.
    fn near_misses(&mut self, old: &Race3d, new: &Race3d, now: u32) {
        let t = track(new.course);
        for (&id, r) in &new.riders {
            let Some(o) = old.riders.get(&id) else { continue };
            if o.crash > 0 || r.crash > 0 || r.finish.is_some() || self.rested(id, now) {
                continue;
            }
            let at = [r.s, r.x, r.z];
            let mut near = None;
            match new.course {
                Course::Griffin => {
                    // Past the middle of a block, and the gap to it was a hair.
                    for b in &t.blocks {
                        let mid = b.s + b.len * 0.5;
                        if o.s <= mid && r.s > mid {
                            let clear = |x: f32, z: f32| {
                                let gx = (b.x.0 - (x + 0.6)).max((x - 0.6) - b.x.1);
                                let gz = (b.z.0 - (z + 0.4)).max((z - 0.4) - b.z.1);
                                if gx > 0.0 && gz > 0.0 { gx.hypot(gz) } else { gx.max(gz) }
                            };
                            if clear(r.x, r.z).min(clear(o.x, o.z)) < 0.3 {
                                near = Some(0);
                            }
                        }
                    }
                }
                Course::Cart => {
                    for item in &t.items {
                        if item.kind == ItemKind::Timber
                            && (item.x - r.x).abs() < 1.0
                            && o.s <= item.s
                            && r.s > item.s
                            && r.z >= TIMBER_H
                            && r.z < TIMBER_H + 0.4
                        {
                            near = Some(1);
                        }
                    }
                }
                Course::Shaft => {
                    let at_s = t.at(r.s);
                    // The edge of a bridge or an open plank, a hair from falling.
                    if !at_s.walls && r.z <= 0.05 && r.x.abs() > at_s.width * 0.5 - 0.22 && r.x.abs() <= at_s.width * 0.5 {
                        near = Some(3);
                    }
                    // A cart across the crossing that just missed: as the ball
                    // draws level with it, a hair outside the width it hits.
                    if at_s.ground == Ground::Crossing && r.z < 1.0 {
                        for (cs, cx) in crossing_carts(t, new.t) {
                            let gap = (cx - r.x).abs() - 1.6;
                            if o.s <= cs && r.s > cs && (0.0..0.5).contains(&gap) {
                                near = Some(2);
                            }
                        }
                    }
                }
                Course::Touge => {}
            }
            if let Some(n) = near {
                self.put(Kind::Near, id, 0, n, at, Weight::Graze, now);
                self.last_near.retain(|&(x, _)| u32::from(x) != id);
                self.last_near.push((id as u8, now));
            }
        }
    }

    fn rested(&self, id: u32, now: u32) -> bool {
        self.last_near.iter().any(|&(x, t)| u32::from(x) == id && now.saturating_sub(t) < NEAR_REST)
    }

    /// Passes and the lead: who is clear ahead of whom, and when it swaps.
    fn places(&mut self, old: &Race3d, new: &Race3d, now: u32) {
        let progress = |r: &Rider| r.finish.map_or(r.s, |f| 100_000.0 - f as f32);
        let ids: Vec<u32> = new.riders.keys().copied().collect();
        for (k, &a) in ids.iter().enumerate() {
            for &b in &ids[k + 1..] {
                let (ra, rb) = (&new.riders[&a], &new.riders[&b]);
                let gap = progress(ra) - progress(rb);
                if gap.abs() < CLEAR {
                    continue;
                }
                let (front, back) = if gap > 0.0 { (a, b) } else { (b, a) };
                if self.ahead.contains(&(front as u8, back as u8)) {
                    continue;
                }
                let swapped = self.set_ahead(front, back);
                // A pass is called only between two knights still running.
                let running = |id: u32| {
                    let (n, o) = (&new.riders[&id], old.riders.get(&id));
                    n.racing() && o.is_some_and(Rider::racing)
                };
                if swapped && running(front) && running(back) {
                    let place = 1 + self.ahead_of(front) as u32;
                    let kind = if place == 1 { Kind::Lead } else { Kind::Pass };
                    let weight = if place == 1 { Weight::Solid } else { Weight::Graze };
                    let at = [new.riders[&front].s, new.riders[&front].x, new.riders[&front].z];
                    self.put(kind, front, back, place, at, weight, now);
                }
            }
        }
    }

    /// Knights crossing the line, and a photo finish.
    fn finishes(&mut self, old: &Race3d, new: &Race3d, now: u32) {
        let mut home: Vec<u32> = new
            .riders
            .iter()
            .filter(|(id, r)| r.finish.is_some() && old.riders.get(id).is_some_and(|o| o.finish.is_none()))
            .map(|(&id, _)| id)
            .collect();
        home.sort();
        let before = old.riders.values().filter(|r| r.finish.is_some()).count() as u32;
        for (k, &id) in home.iter().enumerate() {
            let place = before + k as u32 + 1;
            let r = &new.riders[&id];
            let weight = if place == 1 { Weight::Finale } else { Weight::Solid };
            self.put(Kind::Finish, id, 0, place, [r.s, r.x, r.z], weight, now);
            if place == 2 {
                let first = new
                    .riders
                    .iter()
                    .filter(|(rid, x)| **rid != id && x.finish.is_some())
                    .min_by_key(|(_, x)| x.finish)
                    .map(|(&fid, x)| (fid, x.finish.unwrap_or(0)));
                if let Some((fid, at)) = first {
                    let gap = r.finish.unwrap_or(0).saturating_sub(at);
                    if gap <= PHOTO {
                        self.put(Kind::Photo, id, fid, gap, [r.s, r.x, r.z], Weight::Kill, now);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/rules/together_race3d_show__tests.rs"]
mod tests;
