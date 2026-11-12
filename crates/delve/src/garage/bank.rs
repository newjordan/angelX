//! The bank: carries a knight's career between the ledger on disk and a running race.
//!
//! The rules never touch files, so the host (the realm's host, or the Glass example) owns a
//! `Bank` and calls [`Bank::pump`] once after every tick of the race. The bank
//!
//! - loads each seat's career from the ledger when the seat sits down (the race starts it
//!   blank and marked `fresh`), and writes it back, by the knight's name, when the race
//!   marked it `dirty` (a purchase, a purse, a record);
//! - in a time trial, records the lap being driven five points a second, keeps it as the
//!   knight's ghost for that course and pod when it beats the one they have, and lays the
//!   saved ghost on the road for the screen (`Flow::ghost`);
//! - writes the ledger down when anything changed.
//!
//! Ghosts live here, not in the session: the session is copied whole on every tick (the
//! hot engine round-trips it), and a lap is thousands of bytes.
use super::ghost::{Path, Pt, RATE};
use super::ledger::{Ghost, Ledger};
use super::store::Store;
use crate::rules::knight_racers::career::{lap_key, ms};
use crate::rules::knight_racers::flow::GhostPose;
use crate::rules::knight_racers::race::COUNTDOWN;
use crate::rules::knight_racers::Session;
use std::collections::BTreeMap;

/// A time trial being driven: what is being recorded and what is being raced.
struct Track {
    pod: u8,
    key: String,
    /// The best lap on the road when the trial began, in ms: a lap must beat it to be kept.
    best: u32,
    ghost: Option<Path>,
    rec: Vec<Pt>,
    /// Laps already folded into the record.
    laps: usize,
    last_k: i64,
    last_ticks: u32,
}

pub struct Bank {
    store: Store,
    ledger: Ledger,
    tracks: BTreeMap<u32, Track>,
    /// What went wrong the last time the ledger was read or written, if anything did.
    pub error: Option<String>,
}

impl Bank {
    /// A bank over the store, holding what the store holds (an empty ledger if it holds
    /// nothing, or nothing readable: the bad file is kept beside it, see `Store::load`).
    pub fn open(store: Store) -> Bank {
        let (ledger, error) = match store.load() {
            Ok(l) => (l, None),
            Err(e) => (Ledger::new(), Some(e)),
        };
        Bank { store, ledger, tracks: BTreeMap::new(), error }
    }

    pub fn ledger(&self) -> &Ledger { &self.ledger }
    pub fn ledger_mut(&mut self) -> &mut Ledger { &mut self.ledger }

    /// Once after each tick: see the module's doc. True if the ledger was written.
    pub fn pump(&mut self, s: &mut Session) -> bool {
        for sf in s.flow.seats.values_mut() {
            if sf.fresh {
                sf.career = self.ledger.career(&sf.name);
                sf.fresh = false;
                // A pod the knight has not got is not the one to start on.
                if !sf.career.owns_pod(sf.pick) { sf.pick = sf.career.open_pods().first().copied().unwrap_or(0); }
            } else if sf.dirty {
                self.ledger.set_career(&sf.name, &sf.career);
                sf.dirty = false;
            }
        }
        self.trial(s);
        if !self.ledger.dirty() { return false; }
        match self.store.save(&mut self.ledger) {
            Ok(()) => { self.error = None; true }
            Err(e) => { tracing::warn!(%e, "the garage ledger could not be written"); self.error = Some(e); false }
        }
    }

    fn trial(&mut self, s: &mut Session) {
        let Some((seat, course, pod)) = s.flow.trial(s) else {
            self.tracks.clear();
            s.flow.ghost = None;
            return;
        };
        let Some(name) = s.flow.seats.get(&seat).map(|sf| sf.name.clone()) else { return };
        let key = lap_key(course, pod);
        // A new trial: another course or pod, or the same one started over.
        let begun = self.tracks.get(&seat).is_none_or(|t| t.key != key || s.ticks < t.last_ticks);
        if begun {
            let ghost = self.ledger.ghost(&name, &key).and_then(|g| Path::decode(&g.path));
            let best = self.ledger.ghost(&name, &key).map_or(0, |g| g.ms);
            self.tracks.insert(seat, Track { pod, key: key.clone(), best, ghost, rec: Vec::new(), laps: 0, last_k: -1, last_ticks: 0 });
        }
        let Some(t) = self.tracks.get_mut(&seat) else { return };
        t.last_ticks = s.ticks;
        let Some(car) = s.cars.get(&seat).cloned() else { return };
        let len = s.course().lap();
        let now = s.ticks.saturating_sub(COUNTDOWN) as f32 / crate::rules::knight_racers::race::HZ;
        let (done, begin, lap_time) = s.flow.laps.get(&seat).map_or((0, 0., None), |l| (l.times.len(), l.begin, l.times.get(t.laps).copied()));
        // A lap just completed: close its path and keep it if it is the best.
        if done > t.laps {
            if let Some(l) = lap_time {
                let want = (l * RATE).round() as usize + 1;
                let end = Pt { s: len, y: car.y, lift: 0., route: car.route };
                t.rec.truncate(want);
                while t.rec.len() < want { t.rec.push(end); }
                if let Some(last) = t.rec.last_mut() { *last = end; }
                let lap_ms = ms(l);
                if lap_ms > 0 && (t.best == 0 || lap_ms < t.best) {
                    let path = Path { pts: std::mem::take(&mut t.rec) }.encode();
                    self.ledger.set_ghost(&name, &t.key, Ghost { ms: lap_ms, seq: 0, path });
                    t.best = lap_ms;
                }
            }
            t.rec.clear();
            t.laps = done;
            t.last_k = -1;
        }
        if car.finish.is_some() { s.flow.ghost = None; return; }
        // The lap being run: one point each fifth of a second.
        let into = (now - begin).max(0.);
        let k = (into * RATE).floor() as i64;
        let at = (car.d - t.laps as f32 * len).max(0.);
        while t.last_k < k && t.rec.len() < 2000 {
            t.rec.push(Pt { s: at, y: car.y, lift: car.lift.max(0.), route: car.route });
            t.last_k += 1;
        }
        // The saved lap, laid on the road: it starts as the knight's lap does and waits at the
        // line when its lap is done.
        s.flow.ghost = t.ghost.as_ref().and_then(|g| {
            if into > g.seconds() + 2. { return None; }
            let p = g.at(into)?;
            Some(GhostPose { s: p.s, y: p.y, yaw: 0., lift: p.lift, route: p.route, pod: t.pod, gap: (p.s - at) / car.v.max(20.) })
        });
    }
}
