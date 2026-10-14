//! The writer's side: what one client holds, and its next snapshot.

use super::world::{Entity, State, World};
use super::{BUDGET, DEFERRED, GLOBALS_PATCH, GLOBALS_SAME, GLOBALS_WHOLE, varint_len};
use crate::codec::put_varint;
use crate::doc::delta::diff;
use crate::doc::value::Value;
use crate::doc::wire::Writer;
use std::collections::HashMap;

/// What one client can see this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interest {
    /// The middle of its view, in the game's units.
    pub x: i32,
    pub y: i32,
    /// Half its view each way, plus whatever margin the game wants so a
    /// thing is held before it walks on screen.
    pub half_w: i32,
    pub half_h: i32,
    /// How much further a held thing may go before it is let go, so one
    /// pacing along the edge is not sent whole again and again.
    pub keep: i32,
    /// The client's own thing (its knight): first in every snapshot it
    /// changed in, because the client predicts from it.
    pub own: Option<u32>,
}

impl Interest {
    /// Whether a point is in view (the margin included).
    pub fn sees(&self, x: i32, y: i32) -> bool {
        self.within(x, y, 0)
    }

    fn within(&self, x: i32, y: i32, slack: i32) -> bool {
        let dx = (i64::from(x) - i64::from(self.x)).abs();
        let dy = (i64::from(y) - i64::from(self.y)).abs();
        dx <= i64::from(self.half_w) + i64::from(slack) && dy <= i64::from(self.half_h) + i64::from(slack)
    }

    /// How far out a point is, as a share of the view: 0 in the middle,
    /// `REACH_EDGE` at the edge of the view.
    fn reach(&self, x: i32, y: i32) -> u64 {
        let dx = (i64::from(x) - i64::from(self.x)).unsigned_abs();
        let dy = (i64::from(y) - i64::from(self.y)).unsigned_abs();
        let w = u64::from(self.half_w.max(1).unsigned_abs());
        let h = u64::from(self.half_h.max(1).unsigned_abs());
        (dx * REACH_EDGE / w).max(dy * REACH_EDGE / h)
    }
}

/// `Interest::reach` at the edge of the view.
const REACH_EDGE: u64 = 1024;
/// Priority is the ticks a thing has waited times its band's weight: the
/// nearest quarter of the view counts four times what the outer quarter
/// (and anything held past the edge) does, so near things go first and a
/// far thing is never starved.
const BAND_WEIGHT: [u64; 4] = [4, 3, 2, 1];
/// A thing the client has never seen counts as having waited this many
/// ticks more than it has: seeing someone at all comes before a fresher
/// place for someone already seen.
const NEW_WAIT: u64 = 8;
/// The three list counts are written after the budget is spent; each is
/// under 16,384, so two bytes each are kept for them.
const COUNTS: usize = 6;

/// What the client holds of one thing.
struct Held {
    version: u32,
    /// The bytes the client holds, so an update need carry only the ones
    /// that changed.
    state: State,
    born: u32,
    /// The tick it was last written.
    sent: u32,
    /// The write it was last found in interest, so the ones that were not
    /// can be let go.
    seen: u32,
}

/// One thing that needs sending.
struct Need {
    id: u32,
    index: u32,
    /// Sent whole (new to the client, or born again) rather than as state.
    whole: bool,
    /// For an update: which of the held bytes changed (bit i, byte i), or
    /// `None` when the state's length changed and it goes whole.
    mask: Option<u32>,
    score: u64,
    reach: u64,
    cost: usize,
}

/// What one write put in its snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Written {
    /// The first snapshot to this client: it starts over.
    pub whole: bool,
    /// Something did not fit and waits for the next snapshot.
    pub partial: bool,
    pub spawns: u32,
    pub updates: u32,
    pub despawns: u32,
    /// Things the client holds after this snapshot.
    pub held: u32,
    pub bytes: usize,
}

/// What one client holds, kept by its writer for the life of the connection.
#[derive(Default)]
pub struct Viewer {
    held: HashMap<u32, Held>,
    /// Things in view the client does not hold yet: the tick each was first
    /// left out (so its wait grows like everyone else's) and the write it
    /// was last in view.
    wanted: HashMap<u32, (u32, u32)>,
    globals: Option<Value>,
    names: Writer,
    pass: u32,
    needs: Vec<Need>,
    gone: Vec<u32>,
    taken: Vec<usize>,
}

impl Viewer {
    pub fn new() -> Viewer {
        Viewer::default()
    }

    /// Whether the client holds this thing.
    pub fn holds(&self, id: u32) -> bool {
        self.held.contains_key(&id)
    }

    /// The ids the client holds, in no order.
    pub fn held(&self) -> impl Iterator<Item = u32> + '_ {
        self.held.keys().copied()
    }

    /// This client's next snapshot body of `world`, within `BUDGET`.
    pub fn write(&mut self, world: &World, globals: &Value, interest: &Interest, out: &mut Vec<u8>) -> Written {
        self.write_within(world, globals, interest, BUDGET, out)
    }

    /// The same within a chosen budget. The globals go first and are never
    /// held back, so they must be small: past the budget alone, they are
    /// sent anyway and nothing else is.
    pub fn write_within(
        &mut self,
        world: &World,
        globals: &Value,
        interest: &Interest,
        budget: usize,
        out: &mut Vec<u8>,
    ) -> Written {
        let start = out.len();
        let whole = self.globals.is_none();
        self.pass = self.pass.wrapping_add(1);
        let pass = self.pass;
        let tick = world.tick();

        out.push(0);
        match &self.globals {
            None => {
                out.push(GLOBALS_WHOLE);
                self.names.value(out, globals);
                self.globals = Some(globals.clone());
            }
            Some(old) => match diff(old, globals) {
                None => out.push(GLOBALS_SAME),
                Some(patch) => {
                    out.push(GLOBALS_PATCH);
                    self.names.patch(out, &patch);
                    self.globals = Some(globals.clone());
                }
            },
        }
        let mut used = out.len() - start + COUNTS;

        // What is in view: held things still in reach, new things in view.
        self.needs.clear();
        let entities = world.entities();
        let keep = interest.keep.max(0);
        let x0 = interest.x.saturating_sub(interest.half_w.saturating_add(keep));
        let x1 = interest.x.saturating_add(interest.half_w.saturating_add(keep));
        let y0 = interest.y.saturating_sub(interest.half_h.saturating_add(keep));
        let y1 = interest.y.saturating_add(interest.half_h.saturating_add(keep));
        let held = &mut self.held;
        let wanted = &mut self.wanted;
        let needs = &mut self.needs;
        world.within(x0, y0, x1, y1, |e| {
            let reach = interest.reach(e.x, e.y);
            let band = BAND_WEIGHT[((reach * 4 / REACH_EDGE) as usize).min(BAND_WEIGHT.len() - 1)];
            let own = interest.own == Some(e.id);
            let need = match held.get_mut(&e.id) {
                Some(h) => {
                    h.seen = pass;
                    if h.born != e.born {
                        Some((true, NEW_WAIT, None))
                    } else if h.version != e.version {
                        let mask = changed(&h.state, &e.state);
                        if mask == Some(0) {
                            // Changed and changed back since it was sent:
                            // the client holds these very bytes.
                            h.version = e.version;
                            None
                        } else {
                            Some((false, u64::from(tick.wrapping_sub(h.sent)).max(1), mask))
                        }
                    } else {
                        None
                    }
                }
                None if interest.within(e.x, e.y, 0) || own => {
                    let (since, seen) = wanted.entry(e.id).or_insert((tick, pass));
                    *seen = pass;
                    Some((true, NEW_WAIT + u64::from(tick.wrapping_sub(*since)), None))
                }
                None => None,
            };
            if let Some((whole, waited, mask)) = need {
                let index = entities.partition_point(|o| o.id < e.id) as u32;
                needs.push(Need {
                    id: e.id,
                    index,
                    whole,
                    mask,
                    score: if own { u64::MAX } else { waited * band },
                    reach,
                    cost: cost(e, whole, mask),
                });
            }
        });

        // What left: held things no longer in reach, or no longer at all.
        self.gone.clear();
        self.gone.extend(self.held.iter().filter(|(_, h)| h.seen != pass).map(|(&id, _)| id));
        self.gone.sort_unstable();
        let mut despawns = 0;
        for &id in &self.gone {
            let c = varint_len(u64::from(id));
            if used + c > budget {
                break;
            }
            used += c;
            despawns += 1;
        }

        // What fits, nearest and longest waiting first.
        let total: usize = self.needs.iter().map(|n| n.cost).sum();
        self.taken.clear();
        let mut deferred_held = false;
        if used + total <= budget {
            self.taken.extend(0..self.needs.len());
        } else {
            self.needs.sort_unstable_by(|a, b| b.score.cmp(&a.score).then(a.reach.cmp(&b.reach)).then(a.id.cmp(&b.id)));
            for (i, n) in self.needs.iter().enumerate() {
                if used + n.cost <= budget {
                    used += n.cost;
                    self.taken.push(i);
                } else if self.held.contains_key(&n.id) {
                    deferred_held = true;
                }
            }
        }
        self.taken.sort_unstable_by_key(|&i| self.needs[i].id);
        let partial = despawns < self.gone.len() || self.taken.len() < self.needs.len();
        if deferred_held || despawns < self.gone.len() {
            out[start] |= DEFERRED;
        }

        // Despawns, spawns, updates; each list by rising id.
        put_varint(out, despawns as u64);
        let mut last = 0u32;
        for &id in &self.gone[..despawns] {
            put_varint(out, u64::from(id - last));
            last = id;
            self.held.remove(&id);
        }
        let mut written = Written { whole, partial, despawns: despawns as u32, ..Written::default() };
        for whole_pass in [true, false] {
            let count = self.taken.iter().filter(|&&i| self.needs[i].whole == whole_pass).count();
            put_varint(out, count as u64);
            let mut last = 0u32;
            for &i in &self.taken {
                let n = &self.needs[i];
                if n.whole != whole_pass {
                    continue;
                }
                let e = &entities[n.index as usize];
                put_varint(out, u64::from(e.id - last));
                last = e.id;
                let state = e.state.as_slice();
                if whole_pass {
                    put_varint(out, u64::from(e.kind));
                    put_varint(out, e.spawn.len() as u64);
                    out.extend_from_slice(&e.spawn);
                    put_varint(out, state.len() as u64);
                    out.extend_from_slice(state);
                    written.spawns += 1;
                } else {
                    put_update(out, state, n.mask);
                    written.updates += 1;
                }
                self.held.insert(e.id, Held { version: e.version, state: e.state, born: e.born, sent: tick, seen: pass });
                self.wanted.remove(&e.id);
            }
        }
        // A thing wanted but no longer in view starts its wait over.
        self.wanted.retain(|_, (_, seen)| *seen == pass);
        written.held = self.held.len() as u32;
        written.bytes = out.len() - start;
        written
    }
}

/// Which bytes differ (bit i for byte i), or `None` when the lengths do.
fn changed(held: &State, now: &State) -> Option<u32> {
    let (a, b) = (held.as_slice(), now.as_slice());
    (a.len() == b.len()).then(|| a.iter().zip(b).enumerate().filter(|(_, (x, y))| x != y).fold(0, |m, (i, _)| m | 1 << i))
}

/// An update's head: the state's length times two (the bytes follow
/// whole), or the mask of changed bytes times two plus one (those bytes
/// follow, in order).
fn update_head(state: &[u8], mask: Option<u32>) -> u64 {
    match mask {
        Some(m) => (u64::from(m) << 1) | 1,
        None => (state.len() as u64) << 1,
    }
}

fn put_update(out: &mut Vec<u8>, state: &[u8], mask: Option<u32>) {
    put_varint(out, update_head(state, mask));
    match mask {
        Some(m) => out.extend(state.iter().enumerate().filter(|(i, _)| m >> i & 1 == 1).map(|(_, b)| *b)),
        None => out.extend_from_slice(state),
    }
}

/// The most a thing's record can take: its id written whole bounds the
/// step from the id before it.
fn cost(e: &Entity, whole: bool, mask: Option<u32>) -> usize {
    let state = e.state.as_slice();
    let id = varint_len(u64::from(e.id));
    if whole {
        id + varint_len(u64::from(e.kind))
            + varint_len(e.spawn.len() as u64)
            + e.spawn.len()
            + varint_len(state.len() as u64)
            + state.len()
    } else {
        let bytes = mask.map_or(state.len(), |m| m.count_ones() as usize);
        id + varint_len(update_head(state, mask)) + bytes
    }
}
