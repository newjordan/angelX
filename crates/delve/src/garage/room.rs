//! The shop as a room of the game: knights walk it, stand at a rack and
//! take a part with E; the lift swaps it onto their racer at once, and the
//! build is written into the ledger on the spot. Fixed-point and stepped by
//! the host like the rest of the rules, so a mirror and a test see the same
//! room.
//!
//! - arrows / WASD walk (`Input::move_x`, `move_y`);
//! - E (`interact`) takes the part the knight is standing at;
//! - F (`fire`) puts the last swap back, and again to put it forward;
//! - 1-4 (`cast`) fit one of the four starting racers.
//!
//! Nothing here draws; `scene` reads the room and draws it.

use super::ledger::{Ledger, Swap, SwapError};
use super::parts::{Part, Slot};
use super::racer::{Build, Stats};
use super::shop;
use crate::rules::{HZ, Input};
use std::collections::BTreeMap;

/// Fixed-point: this many units of position to a world unit.
pub const UNIT: i32 = 64;
/// How far a walking knight goes in a tick, in fixed-point: about five a second.
const STEP: i32 = 11;
/// The same on a diagonal.
const STEP_DIAG: i32 = 8;
/// How many knights the shop takes: one to a lift.
pub const SEATS: usize = 6;
/// How long a swap plays: the old part lifts off, the new one drops in,
/// sparks, and the bars settle.
pub const SWAP_TICKS: u32 = HZ * 6 / 5;
/// The tick of a swap at which the new part lands.
pub const LAND_TICK: u32 = SWAP_TICKS * 11 / 20;
/// How long the stat changes stay up after a swap.
pub const DELTA_TICKS: u32 = HZ * 3;
/// A knight is this far from a wall at the nearest.
const MARGIN: f32 = 0.8;

/// What a knight is about to take: the part their feet are at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pick {
    pub slot: Slot,
    pub index: usize,
    pub part: &'static Part,
}

/// A swap playing out on a lift.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwapAnim {
    /// The slot that changed; none when the whole racer was changed.
    pub slot: Option<Slot>,
    pub from: &'static Part,
    pub to: &'static Part,
    /// Ticks since it began.
    pub age: u32,
    pub before: Stats,
    pub after: Stats,
}

#[derive(Clone, Debug)]
pub struct Seat {
    pub id: u32,
    pub name: String,
    pub bay: usize,
    /// Position in fixed-point, `UNIT` to a world unit.
    pub x: i32,
    pub y: i32,
    /// Which way they face, in radians about +z (0 is +x).
    pub yaw: f32,
    /// Ticks spent walking, for the stride.
    pub stride: u32,
    pub build: Build,
    /// The racer before the last change, to put back.
    pub undo: Option<Build>,
    pub swap: Option<SwapAnim>,
    /// The stat change of the last swap, kept on show for a while.
    pub shown: Option<(Stats, u32)>,
    pub pick: Option<Pick>,
    pressed: u8,
}

impl Seat {
    pub fn pos(&self) -> (f32, f32) {
        (self.x as f32 / UNIT as f32, self.y as f32 / UNIT as f32)
    }

    pub fn walking(&self) -> bool {
        self.stride > 0
    }
}

/// What happened in a tick, for the host to save, say or sound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Joined { seat: u32, bay: usize },
    Left { seat: u32 },
    /// A part went on a racer, and the ledger has it.
    Swapped { seat: u32, swap: Swap },
    /// A whole racer was put on (a starting racer, an undo).
    Refit { seat: u32 },
    /// The knight tried to take a part they already wear.
    Already { seat: u32, slot: Slot },
}

#[derive(Clone, Debug, Default)]
pub struct Shop {
    pub tick: u64,
    pub seats: Vec<Seat>,
}

const E: u8 = 1;
const F: u8 = 2;
const KITS: u8 = 4;

impl Shop {
    pub fn new() -> Shop {
        Shop::default()
    }

    pub fn seat(&self, id: u32) -> Option<&Seat> {
        self.seats.iter().find(|s| s.id == id)
    }

    /// A knight walks in and is given a lift, the middle ones first. None
    /// when the shop is full, or the seat is already in.
    pub fn join(&mut self, id: u32, name: &str, ledger: &Ledger) -> Option<usize> {
        if self.seats.len() >= SEATS || self.seat(id).is_some() {
            return None;
        }
        let bay = shop::BAY_ORDER.into_iter().find(|b| self.seats.iter().all(|s| s.bay != *b))?;
        let (x, y) = shop::bay_door(bay);
        self.seats.push(Seat {
            id,
            name: name.trim().to_string(),
            bay,
            x: (x * UNIT as f32) as i32,
            y: (y * UNIT as f32) as i32,
            yaw: std::f32::consts::FRAC_PI_2,
            stride: 0,
            build: ledger.build(name),
            undo: None,
            swap: None,
            shown: None,
            pick: None,
            pressed: 0,
        });
        Some(bay)
    }

    pub fn leave(&mut self, id: u32) -> bool {
        let n = self.seats.len();
        self.seats.retain(|s| s.id != id);
        self.seats.len() != n
    }

    /// One tick. Each seat's input moves its knight and may take a part; a
    /// swap is written into `ledger` at once, and returned for the host to
    /// save (`Store::save_if_dirty`).
    pub fn step(&mut self, inputs: &BTreeMap<u32, Input>, ledger: &mut Ledger) -> Vec<Event> {
        let mut events = vec![];
        self.tick += 1;
        for seat in &mut self.seats {
            let input = inputs.get(&seat.id).copied().unwrap_or_default();
            walk(seat, &input);
            seat.pick = pick(seat);
            let kit = if (1..=4).contains(&input.cast) { KITS << (input.cast - 1) } else { 0 };
            let now = (u8::from(input.interact) * E) | (u8::from(input.fire) * F) | kit;
            let edge = now & !seat.pressed;
            seat.pressed = now;
            if edge & E != 0 {
                take(seat, ledger, &mut events);
            }
            if edge & F != 0 {
                put_back(seat, ledger, &mut events);
            }
            if let Some(k) = (0..4u8).find(|k| edge & (KITS << k) != 0) {
                fit_kit(seat, ledger, usize::from(k), &mut events);
            }
            if let Some(a) = &mut seat.swap {
                a.age += 1;
                if a.age >= SWAP_TICKS {
                    seat.swap = None;
                }
            }
            if let Some((_, age)) = &mut seat.shown {
                *age += 1;
                if *age >= DELTA_TICKS {
                    seat.shown = None;
                }
            }
        }
        events
    }
}

fn walk(seat: &mut Seat, input: &Input) {
    let (mx, my) = (i32::from(input.move_x.signum()), -i32::from(input.move_y.signum()));
    if mx == 0 && my == 0 {
        seat.stride = 0;
        return;
    }
    let step = if mx != 0 && my != 0 { STEP_DIAG } else { STEP };
    let moved = go(seat, mx * step, 0) | go(seat, 0, my * step);
    seat.yaw = (my as f32).atan2(mx as f32);
    seat.stride = if moved { seat.stride + 1 } else { 0 };
}

/// Move by (dx, dy) fixed-point unless a wall or a thing is in the way.
fn go(seat: &mut Seat, dx: i32, dy: i32) -> bool {
    if dx == 0 && dy == 0 {
        return false;
    }
    let (x, y) = (seat.x + dx, seat.y + dy);
    let (fx, fy) = (x as f32 / UNIT as f32, y as f32 / UNIT as f32);
    let free = fx > -shop::HX + MARGIN
        && fx < shop::HX - MARGIN
        && fy > 0.6
        && fy < shop::HY - MARGIN
        && shop::obstacles().iter().all(|&(x0, y0, x1, y1)| !(fx > x0 - 0.45 && fx < x1 + 0.45 && fy > y0 - 0.45 && fy < y1 + 0.45));
    if free {
        seat.x = x;
        seat.y = y;
    }
    free
}

/// The part a knight's feet are at, if any: the nearest cell of any rack.
fn pick(seat: &Seat) -> Option<Pick> {
    let (x, y) = seat.pos();
    shop::stations()
        .iter()
        .filter_map(|s| {
            let i = s.cell_for(x, y)?;
            let (cx, cy) = s.cell_at(i);
            let d = (cx - x).powi(2) + (cy - y).powi(2);
            Some((d, s, i))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .and_then(|(_, s, i)| super::parts::nth(s.slot, i).map(|part| Pick { slot: s.slot, index: i, part }))
}

fn take(seat: &mut Seat, ledger: &mut Ledger, events: &mut Vec<Event>) {
    let Some(p) = seat.pick else { return };
    let before = seat.build.clone();
    match ledger.swap(&seat.name, p.part.id) {
        Ok(swap) => {
            seat.build = ledger.build(&seat.name);
            seat.undo = Some(before);
            seat.swap = Some(SwapAnim { slot: Some(swap.slot), from: swap.from, to: swap.to, age: 0, before: swap.before, after: swap.after });
            seat.shown = Some((swap.after.delta(&swap.before), 0));
            events.push(Event::Swapped { seat: seat.id, swap });
        }
        Err(SwapError::Already) => events.push(Event::Already { seat: seat.id, slot: p.slot }),
        Err(SwapError::Unknown) => {}
    }
}

/// Make the racer `to`, remembering the one it was.
fn refit(seat: &mut Seat, ledger: &mut Ledger, to: Build, events: &mut Vec<Event>) {
    let from = seat.build.clone();
    if from == to {
        return;
    }
    let changed = from.differs(&to);
    let (before, after) = (from.stats(), to.stats());
    let slot = if changed.len() == 1 { Some(changed[0]) } else { None };
    let lead = slot.unwrap_or(Slot::Engine);
    seat.swap = Some(SwapAnim { slot, from: from.part(lead), to: to.part(lead), age: 0, before, after });
    seat.shown = Some((after.delta(&before), 0));
    ledger.set(&seat.name, to.clone());
    seat.build = to;
    seat.undo = Some(from);
    events.push(Event::Refit { seat: seat.id });
}

fn put_back(seat: &mut Seat, ledger: &mut Ledger, events: &mut Vec<Event>) {
    if let Some(prev) = seat.undo.clone() {
        refit(seat, ledger, prev, events);
    }
}

fn fit_kit(seat: &mut Seat, ledger: &mut Ledger, kit: usize, events: &mut Vec<Event>) {
    refit(seat, ledger, Build::kit(kit), events);
}
