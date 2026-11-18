//! What the picture remembers of the flight from one tick to the next: the
//! moments a knight should *feel*. The rules (the Bend module) know nothing
//! of it. After every tick the session looks at what changed between the last
//! tick and this one (a shot fired, a lock snapped on, a volley let go, a foe
//! fallen, a wingman saved, a lamp broken, the Wyrm down) and writes a short
//! record of it here. The picture reads the records by their age, so it is a
//! pure function of the session, and a friend's mirror and a browser guest
//! see the same moments.
//!
//! Time is the session's own tick count. Nothing here moves a craft, and the
//! module never sees it (`Session::bare` leaves it out of the request).
use super::*;
use crate::rules::juice::Weight;

pub const LIFE: u32 = 90;
const MOST: usize = 64;
const FRESH: u8 = 14 - 1;
const NEAR_POP: i32 = dm(45);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum What {
    Muzzle,
    Lock,
    Volley,
    Roll,
    Spur,
    Pop,
    Rescue,
    LampOut,
    WyrmDown,
    Chain,
    Hurt,
    Fall,
    Gate,
    Reflect,
    Beat,
    Tell,
    Starfall,
    Beacon,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fx {
    pub what: What,
    pub seat: u8,
    pub n: u16,
    pub s: i32,
    pub x: i32,
    pub z: i32,
    pub weight: Weight,
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
    #[serde(default)]
    pub prior: u16,
    #[serde(default)]
    pub known: bool,
}

pub fn kind_n(k: Kind) -> u16 {
    match k {
        Kind::Wisp => 0,
        Kind::Kite => 1,
        Kind::Turret => 2,
        Kind::Bomber => 3,
        Kind::Lamp => 4,
    }
}

fn pop_weight(k: Kind, rescue: bool) -> Weight {
    match k {
        Kind::Wisp => Weight::Solid,
        Kind::Kite if rescue => Weight::Crushing,
        Kind::Kite => Weight::Solid,
        Kind::Bomber => Weight::Crushing,
        Kind::Turret => Weight::Kill,
        Kind::Lamp => Weight::Kill,
    }
}

impl Show {
    pub fn quiet(&self) -> bool {
        self.fx.is_empty() && !self.known
    }

    pub fn valid(&self, elapsed: u32) -> bool {
        self.fx.len() <= MOST
            && self.prior <= 1000
            && self.fx.iter().all(|f| {
                f.born <= elapsed
                    && f.seat <= 10
                    && (-40_000_000..=40_000_000).contains(&f.s)
                    && (-40_000_000..=40_000_000).contains(&f.x)
                    && (-40_000_000..=40_000_000).contains(&f.z)
            })
    }

    fn put(&mut self, what: What, seat: u32, n: u32, at: (i32, i32, i32), weight: Weight, now: u32) {
        if self.fx.len() >= MOST {
            self.fx.remove(0);
        }
        self.fx.push(Fx {
            what,
            seat: seat.min(10) as u8,
            n: n.min(u32::from(u16::MAX)) as u16,
            s: at.0,
            x: at.1,
            z: at.2,
            weight,
            born: now,
        });
    }

    pub fn of(&self, what: What, now: u32) -> impl Iterator<Item = &Fx> {
        self.fx
            .iter()
            .filter(move |f| f.what == what && f.age(now) < LIFE)
    }

    pub fn held(&self, now: u32) -> Option<Weight> {
        self.fx
            .iter()
            .filter(|f| f.born == now && f.weight >= Weight::Crushing)
            .map(|f| f.weight)
            .max()
    }

    pub fn know(&mut self, prior: u16) {
        if !self.known {
            self.prior = prior.min(1000);
            self.known = true;
        }
    }

    pub fn observe(&mut self, old: &Session, new: &Session) {
        let now = new.elapsed;
        self.fx.retain(|f| f.age(now) < LIFE);
        if now == old.elapsed {
            return;
        }
        let craft_at = |s: &Session, c: &Craft| (s.craft_s(c), c.x, c.z);
        for (&id, seat) in &new.seats {
            let Some(was) = old.seats.get(&id) else {
                continue;
            };
            let (c, o) = (&seat.craft, &was.craft);
            let at = craft_at(new, c);
            let up = c.down == 0;
            if up && c.cool > o.cool {
                self.put(What::Muzzle, id, 0, at, Weight::Graze, now);
            }
            let (held, had) = (locks(c), locks(o));
            if up && held > had {
                self.put(What::Lock, id, held, at, Weight::Graze, now);
            }
            let (stars, was_stars) = (stars_of(new, id), stars_of(old, id));
            if stars > was_stars {
                self.put(What::Volley, id, stars - was_stars, at, Weight::Graze, now);
            }
            if o.roll == 0 && c.roll > 0 {
                self.put(What::Roll, id, u32::from(c.dir > 0), at, Weight::Graze, now);
            }
            if !o.boost && c.boost && up {
                self.put(What::Spur, id, 0, at, Weight::Graze, now);
            }
            if c.shield < o.shield && c.down == o.down && o.down == 0 {
                let lost = (o.shield - c.shield) as u32;
                let weight = Weight::of(lost, SHIELD as u32, false, false);
                self.put(What::Hurt, id, lost, at, weight, now);
            }
            if o.down == 0 && c.down > 0 {
                self.put(What::Fall, id, 0, at, Weight::Kill, now);
            }
            if c.chain > o.chain && (c.chain == 3 || c.chain % 5 == 0) {
                self.put(What::Chain, id, u32::from(c.chain), at, Weight::Graze, now);
            }
            if c.gates > o.gates {
                self.put(What::Gate, id, 0, at, Weight::Graze, now);
            }
        }
        for w in &new.wing {
            if let Some(was) = old.wing.iter().find(|o| o.id == w.id) {
                if !was.lost && w.lost {
                    let at = craft_at(new, &w.craft);
                    self.put(What::Fall, u32::from(w.id), 0, at, Weight::Crushing, now);
                }
            }
        }
        let lamps_out = |s: &Session| {
            s.foes
                .iter()
                .filter(|f| f.kind == Kind::Lamp && f.st == St::Broken)
                .count() as u32
        };
        for f in new.fx.iter().filter(|f| f.ticks >= match f.what {
            Burst::Starfall => 36 - 1,
            Burst::Fall => 40 - 1,
            Burst::Lock => 8 - 1,
            _ => FRESH,
        }) {
            let at = (f.s, f.x, f.z);
            match f.what {
                Burst::Pop => {
                    let gone = old
                        .foes
                        .iter()
                        .filter(|o| !new.foes.iter().any(|n| n.id == o.id))
                        .min_by_key(|o| dist(at, (o.s, o.x, o.z)));
                    let Some(foe) = gone.filter(|o| dist(at, (o.s, o.x, o.z)) <= NEAR_POP) else {
                        continue;
                    };
                    let by = killer(old, new, f.n);
                    let rescue = foe.st == St::Latched;
                    self.put(
                        What::Pop,
                        by,
                        u32::from(kind_n(foe.kind)) | u32::from(f.n) << 4,
                        at,
                        pop_weight(foe.kind, rescue),
                        now,
                    );
                    if rescue {
                        self.put(What::Rescue, by, u32::from(foe.on), at, Weight::Crushing, now);
                    }
                }
                Burst::Lamp if f.n >= 2 => {
                    let out = lamps_out(new);
                    if out >= 3 {
                        self.put(What::WyrmDown, 0, out, at, Weight::Finale, now);
                    } else {
                        self.put(What::LampOut, 0, out, at, Weight::Kill, now);
                    }
                }
                Burst::Turn => self.put(What::Reflect, u32::from(f.n), 0, at, Weight::Graze, now),
                Burst::Starfall => self.put(What::Starfall, u32::from(f.n), 0, at, Weight::Kill, now),
                _ => {}
            }
        }
        if new.stage > old.stage {
            let at = (new.anchor(), 0, 0);
            self.put(What::Beat, 0, u32::from(old.stage), at, Weight::Solid, now);
        }
        if let (Some(b), Some(was)) = (&new.boss, &old.boss) {
            if b.st == Wyrm::Tell && was.st != Wyrm::Tell {
                let at = (new.anchor() + boss::HEAD, b.x, b.z);
                self.put(What::Tell, 0, u32::from(b.atk), at, Weight::Graze, now);
            }
        }
        if new.mode == Mode::Victory && old.mode != Mode::Victory {
            let at = (new.home, 0, dm(60));
            self.put(What::Beacon, 0, 0, at, Weight::Solid, now);
        }
    }
}

fn locks(c: &Craft) -> u32 {
    c.locks.iter().map(|l| u32::from(l.n)).sum()
}

fn stars_of(s: &Session, id: u32) -> u32 {
    s.bolts
        .iter()
        .filter(|b| b.shot == Arm::Star && u32::from(b.owner) == id)
        .count() as u32
}

fn dist(a: (i32, i32, i32), b: (i32, i32, i32)) -> i32 {
    let (ds, dx, dz) = (
        i64::from(a.0 - b.0),
        i64::from(a.1 - b.1),
        i64::from(a.2 - b.2),
    );
    ((ds * ds + dx * dx + dz * dz) as f64).sqrt() as i32
}

fn killer(old: &Session, new: &Session, chain: u8) -> u32 {
    new.seats
        .iter()
        .find(|(id, s)| {
            old.seats.get(id).is_some_and(|o| s.craft.hits > o.craft.hits)
                && (chain == 0 || s.craft.chain.min(99) as u8 == chain)
        })
        .or_else(|| {
            new.seats
                .iter()
                .find(|(id, s)| old.seats.get(id).is_some_and(|o| s.craft.hits > o.craft.hits))
        })
        .map_or(0, |(&id, _)| id)
}
