//! The painted cast: the five villagers, Aster on his horse, the knights in
//! their six house colours and three masks, the wisp, and the little things a
//! knight carries (`assets/moonfall/art`, emitted to `art_data`). A frame is
//! looked up by slot, state, seat and tick; the stage draws its labelled
//! block only when there is none. Nothing here sets a collision shape.
use super::super::super::ink::Img;
use super::art_data::{SETS, Set};
use std::{collections::BTreeMap, sync::OnceLock};

/// One animation: its frames, how long each lasts, and the slot's canvas.
pub(super) struct Anim {
    pub frames: Vec<Img>,
    pub ticks: &'static [u32],
}

impl Anim {
    pub(super) fn total(&self) -> u32 {
        self.ticks.iter().sum::<u32>().max(1)
    }

    /// The frame showing `tick` ticks in, the animation looping.
    pub(super) fn at(&self, tick: u32) -> &Img {
        let mut time = tick % self.total();
        let mut index = 0;
        while index + 1 < self.ticks.len() && time >= self.ticks[index] {
            time -= self.ticks[index];
            index += 1;
        }
        &self.frames[index.min(self.frames.len() - 1)]
    }

    /// Frame number `index`, held on the last when it runs past.
    pub(super) fn index(&self, index: usize) -> &Img {
        &self.frames[index.min(self.frames.len() - 1)]
    }
}

/// A seat's house colour: shadow, base and light, in the realm's inks. The
/// base is the seat's own ink (`ground::seat_ink`).
pub(super) fn house(seat: u32) -> [char; 3] {
    match (seat.max(1) as usize - 1) % 6 {
        0 => ['0', '2', '3'],
        1 => ['R', '4', '@'],
        2 => ['l', 'C', 'Y'],
        3 => ['!', '7', 'R'],
        4 => ['1', 'z', 'V'],
        _ => ['L', 'Y', 'T'],
    }
}

fn paint(set: &Set, seat: u32) -> Anim {
    let ramp = house(seat);
    let frames = set
        .frames
        .iter()
        .map(|f| {
            let mut im = Img::new(i32::from(set.size.0), i32::from(set.size.1));
            for (y, row) in f.rows.iter().enumerate() {
                for (x, ch) in row.chars().enumerate() {
                    let ch = match ch {
                        '#' => ramp[0],
                        '&' => ramp[1],
                        '*' => ramp[2],
                        c => c,
                    };
                    if ch != '.' {
                        im.put(i32::from(f.x) + x as i32, i32::from(f.y) + y as i32, ch);
                    }
                }
            }
            im
        })
        .collect();
    Anim { frames, ticks: set.ticks }
}

type Bank = BTreeMap<(&'static str, &'static str, u32), Anim>;

fn bank() -> &'static Bank {
    static BANK: OnceLock<Bank> = OnceLock::new();
    BANK.get_or_init(|| {
        let mut out = Bank::new();
        for set in SETS {
            if set.slot.starts_with("knight-") {
                for seat in 1..=6 {
                    out.insert((set.slot, set.state, seat), paint(set, seat));
                }
            } else {
                out.insert((set.slot, set.state, 0), paint(set, 0));
            }
        }
        out
    })
}

/// The state of `kind` ("idle", "walk" or "roll") facing `face` (0 south, 1
/// north, 2 west, 3 east, as the rules count it).
pub(super) fn facing(kind: &str, face: u8) -> &'static str {
    match (kind, face) {
        ("walk", 1) => "walk-up",
        ("walk", 2) => "walk-left",
        ("walk", 3) => "walk-right",
        ("walk", _) => "walk-down",
        ("roll", 1) => "roll-up",
        ("roll", 2) => "roll-left",
        ("roll", 3) => "roll-right",
        ("roll", _) => "roll-down",
        (_, 1) => "idle-up",
        (_, 2) => "idle-left",
        (_, 3) => "idle-right",
        _ => "idle-down",
    }
}

/// A villager's, the wisp's, Aster's or an item's animation.
pub(super) fn anim(slot: &'static str, state: &'static str) -> Option<&'static Anim> {
    bank().get(&(slot, state, 0))
}

/// A knight's animation in `seat`'s house colour, wearing `mask`.
pub(super) fn knight(mask: crate::rules::moonfall::state::Mask, state: &'static str, seat: u32) -> Option<&'static Anim> {
    use crate::rules::moonfall::state::Mask;
    let slot = match mask {
        Mask::Bare => "knight-bare",
        Mask::Swift => "knight-swift",
        Mask::Stout => "knight-stout",
    };
    bank().get(&(slot, state, (seat.max(1) - 1) % 6 + 1))
}

/// The frame of a villager's, the wisp's, Aster's or an item's `state` at `tick`.
pub(super) fn frame(slot: &'static str, state: &'static str, tick: u32) -> Option<&'static Img> {
    anim(slot, state).map(|a| a.at(tick))
}
