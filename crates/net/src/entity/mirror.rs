//! The client's side: the things it was sent, and a few samples of each so
//! everyone else can be drawn between two ticks.

use super::world::State;
use super::{DEFERRED, GLOBALS_PATCH, GLOBALS_SAME, GLOBALS_WHOLE, HISTORY, MAX_SPAWN, MAX_STATE};
use crate::clock::bracket;
use crate::codec::{Bytes, Malformed};
use crate::doc::delta::apply;
use crate::doc::value::Value;
use crate::doc::wire::Reader;
use std::collections::BTreeMap;

/// One thing as the client holds it.
#[derive(Clone, Debug)]
pub struct Mirrored {
    pub kind: u16,
    pub spawn: Vec<u8>,
    /// (tick, state), oldest first; never empty.
    samples: [(u32, State); HISTORY],
    len: usize,
    /// The newest snapshot that said this thing was as its newest sample.
    held_through: u32,
    /// The snapshot tick it was let go on, kept until drawing passes it.
    gone: Option<u32>,
}

impl Mirrored {
    fn new(kind: u16, spawn: &[u8], tick: u32, state: State) -> Mirrored {
        let mut samples = [(0, State::default()); HISTORY];
        samples[0] = (tick, state);
        Mirrored { kind, spawn: spawn.to_vec(), samples, len: 1, held_through: tick, gone: None }
    }

    fn push(&mut self, tick: u32, state: State) {
        let (last_tick, _) = self.samples[self.len - 1];
        if tick <= last_tick {
            self.samples[self.len - 1] = (tick.max(last_tick), state);
            return;
        }
        if self.len == HISTORY {
            self.samples.copy_within(1.., 0);
            self.len -= 1;
        }
        self.samples[self.len] = (tick, state);
        self.len += 1;
    }

    /// A new state on `tick`. If a snapshot since the newest sample said
    /// the thing was unchanged, that is kept as a sample first, so a knight
    /// that stood still and then walked is drawn standing until it walked.
    fn changed(&mut self, tick: u32, state: State) {
        let (last_tick, last_state) = self.samples[self.len - 1];
        if self.held_through > last_tick && self.held_through < tick {
            self.push(self.held_through, last_state);
        }
        self.push(tick, state);
        self.held_through = tick;
    }

    /// The newest state the client holds.
    pub fn latest(&self) -> &[u8] {
        self.samples[self.len - 1].1.as_slice()
    }

    /// The tick of the newest state.
    pub fn latest_tick(&self) -> u32 {
        self.samples[self.len - 1].0
    }

    /// The tick this thing was let go on, if it was.
    pub fn gone(&self) -> Option<u32> {
        self.gone
    }

    /// The two states around a fractional tick, and how far between them
    /// (0: the first, 1: the second). Before the oldest sample the oldest
    /// is held, after the newest the newest.
    pub fn at(&self, tick: f64) -> (&[u8], &[u8], f32) {
        let mut ticks = [0u32; HISTORY];
        for (t, (tick, _)) in ticks.iter_mut().zip(&self.samples[..self.len]) {
            *t = *tick;
        }
        match bracket(&ticks[..self.len], tick) {
            Some((a, b, alpha)) => (self.samples[a].1.as_slice(), self.samples[b].1.as_slice(), alpha),
            None => (self.latest(), self.latest(), 0.0),
        }
    }

    /// The samples held, oldest first.
    pub fn samples(&self) -> impl Iterator<Item = (u32, &[u8])> {
        self.samples[..self.len].iter().map(|(t, s)| (*t, s.as_slice()))
    }
}

/// Everything the client was sent, applied in order.
pub struct Mirror {
    entities: BTreeMap<u32, Mirrored>,
    globals: Value,
    names: Reader,
    newest: u32,
}

impl Default for Mirror {
    fn default() -> Mirror {
        Mirror { entities: BTreeMap::new(), globals: Value::Unit, names: Reader::default(), newest: 0 }
    }
}

impl Mirror {
    pub fn new() -> Mirror {
        Mirror::default()
    }

    /// One snapshot body. A whole one starts over.
    pub fn apply(&mut self, tick: u32, whole: bool, body: &[u8]) -> Result<(), Malformed> {
        if whole {
            *self = Mirror::default();
        }
        let mut b = Bytes::new(body);
        let flags = b.byte()?;
        if flags & !DEFERRED != 0 {
            return Err(Malformed);
        }
        match b.byte()? {
            GLOBALS_SAME => {}
            GLOBALS_WHOLE => self.globals = self.names.value(&mut b)?,
            GLOBALS_PATCH => {
                let patch = self.names.patch(&mut b)?;
                apply(&mut self.globals, patch).map_err(|_| Malformed)?;
            }
            _ => return Err(Malformed),
        }

        let mut id = 0u32;
        for _ in 0..count(&mut b)? {
            id = next_id(&mut b, id)?;
            match self.entities.get_mut(&id) {
                Some(e) if e.gone.is_none() => e.gone = Some(tick),
                _ => return Err(Malformed),
            }
        }
        let mut id = 0u32;
        for _ in 0..count(&mut b)? {
            id = next_id(&mut b, id)?;
            let kind = u16::try_from(b.varint()?).map_err(|_| Malformed)?;
            let spawn = b.blob(MAX_SPAWN)?;
            let state = State::new(b.blob(MAX_STATE)?);
            self.entities.insert(id, Mirrored::new(kind, spawn, tick, state));
        }
        let mut id = 0u32;
        for _ in 0..count(&mut b)? {
            id = next_id(&mut b, id)?;
            let e = match self.entities.get_mut(&id) {
                Some(e) if e.gone.is_none() => e,
                _ => return Err(Malformed),
            };
            let head = b.varint()?;
            let state = if head & 1 == 0 {
                let len = usize::try_from(head >> 1).map_err(|_| Malformed)?;
                if len > MAX_STATE {
                    return Err(Malformed);
                }
                State::new(b.take(len)?)
            } else {
                // Only the bytes that changed, over the ones held.
                let mask = head >> 1;
                let mut bytes = [0u8; MAX_STATE];
                let held = e.latest();
                if mask == 0 || mask >> held.len() != 0 {
                    return Err(Malformed);
                }
                bytes[..held.len()].copy_from_slice(held);
                for (i, byte) in bytes[..held.len()].iter_mut().enumerate() {
                    if mask >> i & 1 == 1 {
                        *byte = b.byte()?;
                    }
                }
                State::new(&bytes[..held.len()])
            };
            e.changed(tick, state);
        }
        b.end()?;

        // Nothing held was left out: whatever was not mentioned is as it was.
        if flags & DEFERRED == 0 {
            for e in self.entities.values_mut() {
                if e.gone.is_none() && e.held_through < tick {
                    e.held_through = tick;
                }
            }
        }
        self.newest = self.newest.max(tick);
        Ok(())
    }

    /// Let go of things whose despawn the drawing has passed.
    pub fn prune(&mut self, render_tick: f64) {
        self.entities.retain(|_, e| e.gone.is_none_or(|g| f64::from(g) > render_tick));
    }

    /// A thing, including one let go but not yet pruned.
    pub fn get(&self, id: u32) -> Option<&Mirrored> {
        self.entities.get(&id)
    }

    /// Every thing by rising id, including those let go but not yet pruned.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &Mirrored)> {
        self.entities.iter().map(|(&id, e)| (id, e))
    }

    /// The things the client holds now (not let go).
    pub fn live(&self) -> impl Iterator<Item = (u32, &Mirrored)> {
        self.iter().filter(|(_, e)| e.gone.is_none())
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// The small document beside the things (`Value::Unit` before any).
    pub fn globals(&self) -> &Value {
        &self.globals
    }

    /// The newest snapshot tick applied.
    pub fn newest(&self) -> u32 {
        self.newest
    }
}

/// A list's count: no list can claim more items than there are bytes left.
fn count(b: &mut Bytes) -> Result<usize, Malformed> {
    let n = b.varint()?;
    if n > b.rest.len() as u64 {
        return Err(Malformed);
    }
    Ok(n as usize)
}

/// The next id of a rising list: the step from the one before.
fn next_id(b: &mut Bytes, last: u32) -> Result<u32, Malformed> {
    let step = b.u32()?;
    last.checked_add(step).ok_or(Malformed)
}
