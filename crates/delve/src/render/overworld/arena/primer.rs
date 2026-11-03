//! First steps: the one key a knight new to the delve needs next, shown in
//! their own view at the moment it is wanted, and gone once it has been
//! used (docs/FIRST-SESSION.md).
//!
//! A friend who joins from a link has no page of controls: the game is the
//! only teacher. Measured on 2026-10-29 (`first_session`), nothing on a
//! beginner's screen named the key that fires, and the line that names the
//! wish key ("T TO MAKE THEM") was cut by the first fight a second after it
//! came up. So, in the first floors:
//!
//! - **ARROWS: FIRE** (or PUNCH, bare-handed), when a foe has been awake a
//!   moment near a knight who has neither loosed a shot nor landed a blow
//!   on this floor; gone as soon as they do.
//! - **T: MAKE A WISH**, in a calm moment on the first floor while a knight
//!   who holds wishes has made none there, for its first minute and a half;
//!   gone as soon as the scroll opens.
//!
//! A hint is the seat's own (as the minimap is): the run and the network
//! never hear of it, and its client keeps when each came and went (`Primer`)
//! so it can come in and leave on the shared clock (docs/MOTION.md: popped
//! up past its line, held to read, lifted and dithered away). Every timing
//! is a knob (`Knobs`, and the client's dev panel).

use super::*;
use crate::rules::{EnemyKind, HZ, TELEGRAPH};

/// The hints, in the order they are wanted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hint {
    Fire,
    Wish,
}

const HINTS: [Hint; 2] = [Hint::Fire, Hint::Wish];

/// Every timing a hint keeps, in ticks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Knobs {
    /// Off: no hint is ever shown.
    pub on: bool,
    /// The deepest floor hints are shown on.
    pub floors: u32,
    /// How long a foe has been awake near a knight who has not fought
    /// before the fire key is named.
    pub fire_after: u32,
    /// How long into the first floor before the wish key is named (after
    /// the floor's title card and its own wish sign have had their say),
    /// and until when it may come back in calm moments.
    pub wish_after: u32,
    pub wish_until: u32,
    /// How far a foe may be for the knight to count as in a fight, units.
    pub near: f32,
    /// The least a hint stays up once it has come, to be read.
    pub hold: u32,
}

impl Default for Knobs {
    fn default() -> Self {
        Knobs { on: true, floors: 2, fire_after: HZ * 3 / 2, wish_after: HZ * 5, wish_until: HZ * 90, near: 16.0, hold: HZ * 2 }
    }
}

/// Ticks a hint takes to leave (`notices::board_from`'s own).
const LEAVE: u64 = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Slot {
    /// The tick it came up, and the tick it began to leave.
    since: Option<u64>,
    gone: Option<u64>,
    /// Used: it never comes again on this seat.
    done: bool,
}

/// A seat's hints: which is up, since when, and which have been used, and
/// what it has seen of its knight (only what a seat's own run carries: the
/// floor's tally stays on the host, `wire::PRIVATE`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Primer {
    pub knobs: Knobs,
    slots: [Slot; HINTS.len()],
    /// The run's tick last drawn: a run begun again (a lower tick) clears
    /// what is up, not what was learned.
    last: u64,
    /// The floor the seat stands on (the raid, the depth) and the tick it
    /// first saw it, and the wishes its knight held there.
    floor: Option<(u64, u32)>,
    began: u64,
    held: Option<u8>,
}

/// Whether `hint` is used by knight `id` now (it is done with): the fire key
/// pressed (a shot loosed, a punch or a swing thrown); a wish spent, or the
/// scroll open.
fn used(run: &Run, id: u32, hint: Hint, held: Option<u8>) -> bool {
    let Some(hero) = run.players.get(&id) else { return false };
    match hint {
        Hint::Fire => hero.fire_cooldown > 0 || hero.swing > 0,
        Hint::Wish => hero.offer.is_some() || held.is_some_and(|was| hero.wishes + hero.great_wishes < was),
    }
}

/// The age a wanted `hint` has had its reason for, in ticks (the foe awake
/// a moment, the floor begun `into` ticks ago), or nothing when it is not
/// wanted now.
fn wanted(run: &Run, id: u32, hint: Hint, k: &Knobs, into: u64) -> Option<u64> {
    let hero = run.players.get(&id).filter(|h| h.hp > 0 && !h.absent)?;
    let depth = run.dungeon.depth;
    if !(1..=k.floors).contains(&depth) || hero.offer.is_some() || !run.active() {
        return None;
    }
    let awake = |reach: f32| {
        run.enemies
            .iter()
            .filter(move |e| e.hp > 0 && !e.dormant && e.kind != EnemyKind::Dummy && e.age >= TELEGRAPH)
            .filter(move |e| (e.x - hero.x).hypot(e.y - hero.y) < reach)
    };
    match hint {
        Hint::Fire => {
            let woke = awake(k.near).map(|e| u64::from(e.age - TELEGRAPH)).max()?;
            woke.checked_sub(u64::from(k.fire_after))
        }
        Hint::Wish => {
            if depth != 1 || !run.can_wish(id) || awake(k.near * 1.5).next().is_some() {
                return None;
            }
            // `then`, not `then_some`: the subtraction must wait for the guard.
            (into >= u64::from(k.wish_after) && into <= u64::from(k.wish_until)).then(|| into - u64::from(k.wish_after))
        }
    }
}

/// The hint's key and word, for knight `id`.
fn words(run: &Run, id: u32, hint: Hint) -> (&'static str, &'static str) {
    match hint {
        Hint::Fire if run.players.get(&id).is_some_and(|h| h.unarmed()) => ("ARROWS", "PUNCH"),
        Hint::Fire => ("ARROWS", "FIRE"),
        Hint::Wish => ("T", "MAKE A WISH"),
    }
}

/// The hint's plate: a keycap and its word on a dark strip.
fn plate(key: &str, word: &str, glint: bool) -> Img {
    let ink = super::super::ink::text_width;
    let cap = ink(key) + 4;
    let w = cap + ink(word) + 12;
    let mut im = Img::new(w, 15);
    chrome::panel(&mut im, (0, 0, w, 15), chrome::Tone::KEY);
    chrome::keycap(&mut im, (3, 2, cap, 11));
    im.text(5, 4, key, if glint { 'O' } else { 'i' });
    im.text(3 + cap + 4, 4, word, 'O');
    im
}

impl Primer {
    pub fn new(knobs: Knobs) -> Primer {
        Primer { knobs, ..Primer::default() }
    }

    /// Bring the hints up to knight `id`'s run as its seat sees it.
    pub fn update(&mut self, run: &Run, id: u32) {
        let now = run.tick;
        if now < self.last {
            for slot in &mut self.slots {
                (slot.since, slot.gone) = (None, None);
            }
        }
        self.last = now;
        let k = self.knobs;
        // A new floor: its clock starts, and the wishes held on it.
        let here = (run.raid_id, run.dungeon.depth);
        let held_now = run.players.get(&id).map(|h| h.wishes + h.great_wishes);
        if self.floor != Some(here) {
            (self.floor, self.began, self.held) = (Some(here), now, held_now);
        }
        let into = now.saturating_sub(self.began);
        let used_held = self.held;
        if let (Some(was), Some(n)) = (self.held, held_now) && n > was {
            self.held = Some(n);
        }
        let mut up = false;
        // Only a floor the hints are for says whether one has been used.
        let floor = (1..=k.floors).contains(&run.dungeon.depth);
        let held = used_held;
        for (slot, &hint) in self.slots.iter_mut().zip(&HINTS) {
            if !slot.done && floor && used(run, id, hint, held) {
                slot.done = true;
            }
            let want = k.on && !slot.done && wanted(run, id, hint, &k, into).is_some();
            match (slot.since, slot.gone) {
                // Up: it stays while wanted, and at least long enough to read
                // (a hint used leaves at once).
                (Some(since), None) => {
                    let read = now >= since + u64::from(k.hold);
                    if slot.done || (!want && read) {
                        slot.gone = Some(now);
                    } else {
                        up = true;
                    }
                }
                (Some(_), Some(gone)) if now >= gone + LEAVE => (slot.since, slot.gone) = (None, None),
                (Some(_), Some(_)) => up = true,
                (None, _) => {}
            }
            // One hint at a time.
            if slot.since.is_none() && want && !up {
                slot.since = Some(now);
                up = true;
            }
        }
    }

    /// The hint up now (coming, holding or leaving) over knight `id`'s view
    /// `out` (the room's view, as `genie::paint` takes it).
    pub fn paint(&mut self, out: &mut Img, run: &Run, id: u32) {
        self.update(run, id);
        let k = if out.w >= 2 * NATIVE_W && out.h >= 2 * NATIVE_H { 2 } else { 1 };
        if out.w < NATIVE_W || out.h < NATIVE_H / 2 {
            return;
        }
        let now = run.tick;
        for (slot, &hint) in self.slots.iter().zip(&HINTS) {
            let Some(since) = slot.since else { continue };
            let age = now.saturating_sub(since) as f32;
            let life = slot.gone.map(|g| (g + LEAVE).saturating_sub(since) as f32);
            let (key, word) = words(run, id, hint);
            // The key glints once a second, to be found among the rest.
            let glint = now % u64::from(HZ) < 4;
            let mut im = plate(key, word, glint);
            if k > 1 {
                im = enlarge(&im, k as f32);
            }
            let view_h = out.h / k;
            // At the foot of the view, clear of the knight: above them when
            // they stand there.
            let ky = run.players.get(&id).map_or(0, |h| frame_px(run, out.w, out.h, Some(id), h.x, h.y).1 / k);
            let foot = view_h - im.h / k - 8;
            let y = if ky > foot - 10 { 26 } else { foot };
            let x = (out.w / k - im.w / k) / 2;
            notices::board_from(out, &im, (x * k, y * k), age, life, 10.0);
        }
    }

    /// The hint up now, if any (for tests and the first-session bot).
    pub fn showing(&self) -> Option<Hint> {
        self.slots.iter().zip(&HINTS).find(|(s, _)| s.since.is_some() && s.gone.is_none()).map(|(_, &h)| h)
    }
}
