//! The writer's side: what one client holds, and its next snapshot.

use super::world::{Entity, State, World};
use super::{BUDGET, DEFERRED, GLOBALS_PATCH, GLOBALS_SAME, GLOBALS_WHOLE, varint_len};
use crate::codec::put_varint;
use crate::doc::delta::diff;
use crate::doc::value::Value;
use crate::doc::wire::Writer;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interest {
    pub x: i32,
    pub y: i32,
    pub half_w: i32,
    pub half_h: i32,
    pub keep: i32,
    pub own: Option<u32>,
}

impl Interest {
    pub fn sees(&self, x: i32, y: i32) -> bool {
        self.within(x, y, 0)
    }

    fn within(&self, x: i32, y: i32, slack: i32) -> bool {
        let dx = (i64::from(x) - i64::from(self.x)).abs();
        let dy = (i64::from(y) - i64::from(self.y)).abs();
        dx <= i64::from(self.half_w) + i64::from(slack) && dy <= i64::from(self.half_h) + i64::from(slack)
    }

    fn reach(&self, x: i32, y: i32) -> u64 {
        let dx = (i64::from(x) - i64::from(self.x)).unsigned_abs();
        let dy = (i64::from(y) - i64::from(self.y)).unsigned_abs();
        let w = u64::from(self.half_w.max(1).unsigned_abs());
        let h = u64::from(self.half_h.max(1).unsigned_abs());
        (dx * REACH_EDGE / w).max(dy * REACH_EDGE / h)
    }
}

const REACH_EDGE: u64 = 1024;
const BAND_WEIGHT: [u64; 4] = [4, 3, 2, 1];
const NEW_WAIT: u64 = 8;
const COUNTS: usize = 6;

struct Held {
    version: u32,
    state: State,
    born: u32,
    sent: u32,
    seen: u32,
}

struct Need {
    id: u32,
    index: u32,
    whole: bool,
    mask: Option<u32>,
    score: u64,
    reach: u64,
    cost: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Written {
    pub whole: bool,
    pub partial: bool,
    pub spawns: u32,
    pub updates: u32,
    pub despawns: u32,
    pub held: u32,
    pub bytes: usize,
}

#[derive(Default)]
pub struct Viewer {
    held: HashMap<u32, Held>,
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

    pub fn holds(&self, id: u32) -> bool {
        self.held.contains_key(&id)
    }

    pub fn held(&self) -> impl Iterator<Item = u32> + '_ {
        self.held.keys().copied()
    }

    pub fn write(&mut self, world: &World, globals: &Value, interest: &Interest, out: &mut Vec<u8>) -> Written {
        self.write_within(world, globals, interest, BUDGET, out)
    }

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
        self.wanted.retain(|_, (_, seen)| *seen == pass);
        written.held = self.held.len() as u32;
        written.bytes = out.len() - start;
        written
    }
}

fn changed(held: &State, now: &State) -> Option<u32> {
    let (a, b) = (held.as_slice(), now.as_slice());
    (a.len() == b.len()).then(|| a.iter().zip(b).enumerate().filter(|(_, (x, y))| x != y).fold(0, |m, (i, _)| m | 1 << i))
}

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
