//! The native Moonfall rules, kept as the oracle for the Bend module `moonfall-rules`.
//! These are the bodies that lived in crates/delve/src/rules/moonfall.rs, story.rs, wisps.rs,
//! walk.rs and plan.rs (and the few helpers of town.rs, folk.rs and ledger.rs), moved here
//! unchanged apart from paths. The parity tests run them side by side with the module.
#![allow(dead_code, clippy::all, unused_imports, unused_variables)]
use angel_delve::rules::moonfall::town::{Q, TILE};
use angel_delve::rules::moonfall::{
    CALL_TICKS, LIMIT, NUMBERS, SKIP_LEN, Session, WON_LEN, clock, control,
    folk::{CAST, Figure, Folk},
    ledger::{self as ledger_types, Fact, Memory},
    says::{self, Ctx, Sign, Topic},
    state::*,
};
use angel_delve::rules::{Hero, Input, MAX_PLAYERS};
use ledger::{NORTH_KEY, OIL, SOUTH_SEAL, STOUT, SWIFT};
use std::collections::BTreeMap;
pub use story::Use;
use story::dist2;
use story::{DRY_HOURS, IRON, KEYS, LIGHT_HOLD, OVEN, PASS_FRESH, PASS_HOURS, PEN, POST};
use walk::{key, keys_of, pressed};
use wisps::{COMING, DRIFT, SNUFF, TOUCH, WARD, isqrt};

pub mod town {
    pub use angel_delve::rules::moonfall::town::*;
    use angel_delve::rules::moonfall::town::{flood, index};
    use std::collections::BTreeMap;
    use std::sync::{Mutex, OnceLock};

    /// Whether a knight's feet (an 8 by 6 box, `x` and `y` in pixels times `Q`)
    /// stand clear of every solid tile and crate.
    pub fn clear(x: i32, y: i32, ground: Ground, crates: &[(i32, i32)]) -> bool {
        let (px, py) = (x.div_euclid(Q), y.div_euclid(Q));
        for (cx, cy) in [(-4, -3), (3, -3), (-4, 2), (3, 2)] {
            let (tx, ty) = (tile_of(px + cx), tile_of(py + cy));
            if !ground.open(tile(tx, ty)) || crates.contains(&(tx, ty)) {
                return false;
            }
        }
        true
    }

    type FieldKey = ((i32, i32), u8, Option<(i32, i32)>, bool);

    /// The flood from `goal` (cached: a pure function of its arguments).
    pub fn field(
        goal: (i32, i32),
        ground: Ground,
        crate_at: Option<(i32, i32)>,
    ) -> std::sync::Arc<Field> {
        static CACHE: OnceLock<Mutex<BTreeMap<FieldKey, std::sync::Arc<Field>>>> = OnceLock::new();
        let key = (
            goal,
            u8::from(ground.bridge_up)
                | u8::from(ground.swift) << 1
                | u8::from(ground.ditch_filled) << 2,
            crate_at,
            false,
        );
        if let Some(f) = CACHE
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
        {
            return f.clone();
        }
        let made = std::sync::Arc::new(flood(goal, ground, crate_at, false));
        CACHE
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, made.clone());
        made
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_knights_feet_are_stopped_by_walls_and_water() {
            let at = |t: i32| (t * TILE + 8) * Q;
            assert!(clear(at(17), at(15), Ground::default(), &[]));
            assert!(!clear(at(17), at(12), Ground::default(), &[]), "the well");
            assert!(!clear(
                at(17),
                at(8),
                Ground {
                    bridge_up: true,
                    ..Ground::default()
                },
                &[]
            ));
            assert!(
                !clear(at(20), at(17), Ground::default(), &[(20, 17)]),
                "a crate"
            );
        }
    }
}

pub mod walk {
    use super::town;
    use angel_delve::rules::Input;
    pub use angel_delve::rules::moonfall::folk::PACE_Q;
    use angel_delve::rules::moonfall::state::*;
    pub use angel_delve::rules::moonfall::state::{PUSH_TICKS, ROLL, ROLL_REST};
    use angel_delve::rules::moonfall::town::{Ground, Q, TILE, Tile};

    /// Q-pixels a tick a knight walks at.
    pub fn speed(walk_percent: u32, mask: Mask) -> i32 {
        let base = i64::from(PACE_Q) * i64::from(walk_percent) / 100;
        (match mask {
            Mask::Bare => base,
            Mask::Swift => base * 16 / 10,
            Mask::Stout => base * 7 / 10,
        }) as i32
    }

    /// The key bits a tick's input is read as.
    pub mod key {
        pub const USE: u16 = 1;
        pub const LEDGER: u16 = 2;
        pub const BELL: u16 = 4;
        pub const MASK: u16 = 8;
        pub const DROP: u16 = 16;
        pub const SIDESTEP: u16 = 32;
        pub const UP: u16 = 64;
        pub const DOWN: u16 = 128;
        pub const LEFT: u16 = 256;
        pub const RIGHT: u16 = 512;
    }

    pub fn keys_of(i: Input) -> u16 {
        let mut k = 0;
        if i.interact || i.take {
            k |= key::USE;
        }
        if i.swing {
            k |= key::LEDGER;
        }
        if i.ult {
            k |= key::BELL;
        }
        if i.play > 0 {
            k |= key::MASK;
        }
        if i.bomb {
            k |= key::DROP;
        }
        if i.dash {
            k |= key::SIDESTEP;
        }
        if i.move_y < 0 || i.aim_y < 0 {
            k |= key::UP;
        }
        if i.move_y > 0 || i.aim_y > 0 {
            k |= key::DOWN;
        }
        if i.move_x < 0 || i.aim_x < 0 {
            k |= key::LEFT;
        }
        if i.move_x > 0 || i.aim_x > 0 {
            k |= key::RIGHT;
        }
        k
    }

    /// Whether `bit` went down this tick.
    pub fn pressed(now: u16, before: u16, bit: u16) -> bool {
        now & bit != 0 && before & bit == 0
    }

    fn crate_at(crates: &[(i32, i32)], x: i32, y: i32) -> Option<usize> {
        for (cx, cy) in [(-4, -3), (3, -3), (-4, 2), (3, 2)] {
            let t = (
                (x.div_euclid(Q) + cx).div_euclid(TILE),
                (y.div_euclid(Q) + cy).div_euclid(TILE),
            );
            if let Some(i) = crates.iter().position(|&c| c == t) {
                return Some(i);
            }
        }
        None
    }

    /// What a step of walking did to the world.
    #[derive(Default)]
    pub struct Walked {
        /// A crate that went into the ditch.
        pub filled: bool,
        /// A crate moved a tile.
        pub moved: bool,
        /// Leant on a crate too heavy to move.
        pub heavy: bool,
    }

    /// One tick of walking for a knight who is free to move.
    pub fn step(
        k: &mut Knight,
        input: Input,
        pace: i32,
        ground: Ground,
        crates: &mut Vec<(i32, i32)>,
        rolled: bool,
    ) -> Walked {
        let mut out = Walked::default();
        let (mut dx, mut dy) = (i32::from(input.move_x), i32::from(input.move_y));
        if k.roll > 0 {
            dx = i32::from(k.roll_dir.0);
            dy = i32::from(k.roll_dir.1);
        } else if rolled && k.roll_rest == 0 && k.up() {
            let dir = if (dx, dy) == (0, 0) {
                facing(k.face)
            } else {
                (dx, dy)
            };
            k.roll = ROLL;
            k.roll_rest = ROLL_REST;
            k.roll_dir = (dir.0 as i8, dir.1 as i8);
            dx = dir.0;
            dy = dir.1;
        }
        k.roll_rest = k.roll_rest.saturating_sub(1);
        let rolling = k.roll > 0;
        k.roll = k.roll.saturating_sub(1);
        if (dx, dy) == (0, 0) {
            k.stride = 0;
            k.push = 0;
            return out;
        }
        k.stride = (k.stride + 1).min(60_000);
        let speed = if rolling { pace * 22 / 10 } else { pace };
        // A diagonal is not faster: 181/256 of the pace on each axis.
        let each = if dx != 0 && dy != 0 {
            speed * 181 / 256
        } else {
            speed
        };
        let mut leaning = false;
        for (ax, ay) in [(dx, 0), (0, dy)] {
            if (ax, ay) == (0, 0) {
                continue;
            }
            let (nx, ny) = (k.x + ax * each, k.y + ay * each);
            if !town::clear(nx, ny, ground, &[]) {
                continue;
            }
            if let Some(i) = crate_at(crates, nx, ny) {
                if k.mask == Mask::Stout {
                    leaning = true;
                    k.push += 1;
                    if k.push >= PUSH_TICKS {
                        k.push = 0;
                        let from = crates[i];
                        let to = (from.0 + ax.signum(), from.1 + ay.signum());
                        let ditch = town::tile(to.0, to.1) == Tile::Ditch && !ground.ditch_filled;
                        let open = ground.open(town::tile(to.0, to.1)) && !crates.contains(&to);
                        if ditch {
                            crates.remove(i);
                            out.filled = true;
                        } else if open {
                            crates[i] = to;
                            out.moved = true;
                        }
                    }
                } else {
                    out.heavy = true;
                }
                continue;
            }
            k.x = nx;
            k.y = ny;
        }
        if !leaning {
            k.push = 0;
        }
        k.face = if dx != 0 && (dy == 0 || k.face < 2) {
            if dx < 0 { 2 } else { 3 }
        } else if dy != 0 {
            if dy < 0 { 1 } else { 0 }
        } else {
            k.face
        };
        out
    }

    /// A facing as a step.
    pub fn facing(face: u8) -> (i32, i32) {
        match face {
            0 => (0, 1),
            1 => (0, -1),
            2 => (-1, 0),
            _ => (1, 0),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn at(tx: i32, ty: i32) -> Knight {
            Knight::at((tx, ty))
        }

        fn go(
            k: &mut Knight,
            dx: i8,
            dy: i8,
            ticks: u32,
            mask: Mask,
            crates: &mut Vec<(i32, i32)>,
            ground: Ground,
        ) {
            k.mask = mask;
            for _ in 0..ticks {
                let input = Input {
                    move_x: dx,
                    move_y: dy,
                    ..Default::default()
                };
                step(k, input, speed(150, mask), ground, crates, false);
            }
        }

        #[test]
        fn the_masks_set_the_stride() {
            let bare = speed(150, Mask::Bare);
            assert_eq!(bare, 576);
            assert!(
                speed(150, Mask::Swift) > bare * 3 / 2 && speed(150, Mask::Stout) < bare * 3 / 4
            );
        }

        #[test]
        fn walls_and_water_stop_a_knight_and_open_ground_does_not() {
            let mut k = at(17, 15);
            let mut crates = vec![];
            go(
                &mut k,
                0,
                -1,
                60,
                Mask::Bare,
                &mut crates,
                Ground::default(),
            );
            // The Moonwell is in the way north of the plaza's middle.
            assert!(k.y > 14 * TILE * Q, "the well holds {}", k.y / Q);
            let mut k = at(15, 15);
            go(&mut k, 1, 0, 20, Mask::Bare, &mut crates, Ground::default());
            assert!(k.x > 15 * TILE * Q + 40 * Q);
        }

        #[test]
        fn only_a_stout_knight_moves_the_crate_and_the_ditch_swallows_it() {
            let ground = Ground::default();
            let mut crates = vec![town::CRATE_START];
            // A bare knight leans on it and it stays.
            let mut k = at(17, 19);
            go(&mut k, 0, 1, 80, Mask::Bare, &mut crates, ground);
            assert_eq!(crates, vec![town::CRATE_START]);
            // A stout one pushes it south a tile at a time, then into the ditch.
            let mut k = at(17, 19);
            let mut filled = false;
            for _ in 0..400 {
                k.mask = Mask::Stout;
                let input = Input {
                    move_y: 1,
                    ..Default::default()
                };
                let w = step(
                    &mut k,
                    input,
                    speed(150, Mask::Stout),
                    Ground {
                        ditch_filled: filled,
                        ..ground
                    },
                    &mut crates,
                    false,
                );
                filled |= w.filled;
            }
            assert!(filled && crates.is_empty());
            assert!(k.y / Q >= 23 * TILE, "walked onto the filled ditch");
        }

        #[test]
        fn a_sidestep_is_quick_and_has_to_rest() {
            let mut k = at(16, 15);
            let mut crates = vec![];
            let ground = Ground::default();
            let go = |k: &mut Knight, crates: &mut Vec<(i32, i32)>, rolled: bool| {
                step(
                    k,
                    Input {
                        move_x: 1,
                        ..Default::default()
                    },
                    speed(150, Mask::Bare),
                    ground,
                    crates,
                    rolled,
                );
            };
            let x0 = k.x;
            go(&mut k, &mut crates, true);
            for _ in 0..5 {
                go(&mut k, &mut crates, false);
            }
            let dash = k.x - x0;
            assert!(dash > 6 * speed(150, Mask::Bare) * 3 / 2, "{dash}");
            let rest = k.roll_rest;
            assert!(rest > 0);
            go(&mut k, &mut crates, true);
            assert_eq!(k.roll, 0, "still resting");
        }
    }
}

pub mod story {
    use angel_delve::rules::moonfall::folk::Folk;
    pub use angel_delve::rules::moonfall::story::*;

    /// What E may mean where a knight stands.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Use {
        Folk(Folk),
        Ink,
        Reeds,
        North,
        South,
        Well,
        /// The Moonwell Bell on its post by the well.
        Bell,
        Lantern(u8),
    }

    /// Hours of Orin's writing.
    pub const PASS_HOURS: u32 = 5;
    /// Hours reeds are kept in the hand before they are dry.
    pub const DRY_HOURS: u32 = 12;
    /// Hours a finished pass keeps its ink: after them it smudges.
    pub const PASS_FRESH: u32 = 2;
    /// How long E is held to light a flame, in ticks.
    pub const LIGHT_HOLD: u32 = 24;
    /// The windows, in half-hours into a day (06:00 is 0): Pella sorts the post
    /// 07:00 to 08:30 on the first morning; Orin takes the pot back in the same
    /// hour; Nessa's oven wants its reeds 08:30 to 10:30 on the second day; Mira's
    /// pen is fresh 06:30 to 09:00; Tovin airs the keys 17:00 to 20:00 on the
    /// second day; Nessa's iron cart is at the forge 06:30 to 08:00 on the second
    /// day.
    pub const POST: (u32, u32) = (2, 5);
    pub const OVEN: (u32, u32) = (5, 9);
    pub const PEN: (u32, u32) = (1, 6);
    pub const KEYS: (u32, u32) = (22, 28);
    pub const IRON: (u32, u32) = (1, 4);

    pub fn dist2(a: (i32, i32), b: (i32, i32)) -> i32 {
        (a.0 - b.0).pow(2) + (a.1 - b.1).pow(2)
    }
}

pub mod wisps {
    use angel_delve::rules::moonfall::state::*;
    use angel_delve::rules::moonfall::town::{Q, TILE};

    /// Pixels (times `Q`) a wisp drifts a tick: one, and a tenth more for each
    /// knight beyond the first.
    pub const DRIFT: i32 = 256;
    /// Ticks a wisp spends putting a flame out.
    pub const SNUFF: u32 = 60;
    /// How near a knight must stand to a flame to keep a wisp off it, pixels.
    pub const WARD: i32 = 40;
    /// How near a wisp must come to a knight to touch.
    pub const TOUCH: i32 = 9;
    /// Ticks a knight is warded after a touch.
    pub const WARDED: u32 = 75;
    /// Ticks a snuffed knight lies before the well carries them back.
    pub const DOWN: u32 = 90;

    /// Where the wisps come in from, around the edge of the town, in pixels.
    pub const COMING: [(i32, i32); 8] = [
        (-20, 400),
        (150, 440),
        (300, 450),
        (470, 440),
        (630, 300),
        (630, 120),
        (330, -20),
        (-20, 180),
    ];

    /// The length of `(dx, dy)`, in integers: the same answer on every machine.
    pub fn isqrt(dx: i32, dy: i32) -> i32 {
        let n = (i64::from(dx) * i64::from(dx) + i64::from(dy) * i64::from(dy)) as u64;
        let (mut lo, mut hi) = (0u64, 1u64 << 32);
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            if mid * mid <= n {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        lo as i32
    }
}

pub mod ledger {
    use super::{Session, folk};
    use angel_delve::rules::moonfall::folk::Folk;
    pub use angel_delve::rules::moonfall::ledger::*;
    pub use angel_delve::rules::moonfall::ledger::{Fact, Memory};

    /// Appointments the Ledger reminds the party of: a routine line known,
    /// the hour before it begins. (folk, routine id, on which day or any).
    pub const APPOINTMENTS: [(Folk, u8, Option<u32>); 6] = [
        (Folk::Pella, 2, None),
        (Folk::Pella, 4, None),
        (Folk::Mira, 4, None),
        (Folk::Mira, 10, None),
        (Folk::Tovin, 4, None),
        (Folk::Aster, 0, Some(1)),
    ];

    /// The appointments that fall in the hour starting at `clock`: those
    /// known to the party whose stop is `ahead` ticks off. Returns
    /// (folk, routine).
    pub fn reminders(mem: &Memory, clock: u32, day: u32) -> Vec<(Folk, u8)> {
        let hour = day / 24;
        let table = folk::timetable(day);
        let mut out = Vec::new();
        for &(who, id, only) in &APPOINTMENTS {
            let known =
                mem.witnessed(who, id) || (who == Folk::Aster && mem.knows(Fact::AsterComing));
            if !known {
                continue;
            }
            for leg in table.legs[who.index()].iter().filter(|l| l.routine == id) {
                if only.is_some_and(|d| leg.at / day != d) {
                    continue;
                }
                if leg.at.saturating_sub(hour) == clock {
                    out.push((who, id));
                }
            }
        }
        out
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_reminder_pops_an_hour_before_a_known_appointment_and_not_for_the_unknown() {
            let d = 3000;
            let mut m = Memory::default();
            assert!(reminders(&m, 0, d).is_empty());
            m.witness(Folk::Mira, 4);
            let leg = folk::timetable(d).legs[Folk::Mira.index()]
                .iter()
                .find(|l| l.routine == 4)
                .map(|l| l.at)
                .unwrap();
            assert_eq!(reminders(&m, leg - d / 24, d), vec![(Folk::Mira, 4)]);
            assert!(reminders(&m, leg - d / 24 + 1, d).is_empty());
        }
    }
}

pub mod folk {
    use angel_delve::rules::moonfall::clock;
    pub use angel_delve::rules::moonfall::folk::*;

    /// The next tick at or after `clock` that `folk` can be seen (the schedule
    /// only: Aster after a shoe keeps his own).
    pub fn next_seen(folk: Folk, clock: u32, day: u32) -> Option<u32> {
        let table = timetable(day);
        table.seen[folk.index()]
            .iter()
            .find(|&&(_, b)| b > clock)
            .map(|&(a, _)| a.max(clock))
    }
}

pub mod plan {
    use super::ledger::{Fact, NORTH_KEY, OIL, SOUTH_SEAL, STOUT, SWIFT};
    use super::story::{PASS_FRESH, PASS_HOURS};
    use super::walk::key;
    use super::{Old, Session};
    use super::{clock, folk, town, walk};
    use angel_delve::rules::Input;
    use angel_delve::rules::moonfall::folk::Folk;
    use angel_delve::rules::moonfall::state::*;
    use angel_delve::rules::moonfall::town::{Q, TILE};

    /// What a knight must hold for a task.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Hand {
        Free,
        Holds(Item),
        Any,
    }

    /// Where a task is done.
    #[derive(Clone, Copy, Debug)]
    enum Aim {
        /// Beside a villager, where they are.
        Folk(Folk),
        /// On a tile, in reach of what is there.
        Tile((i32, i32)),
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Act {
        /// Press E once, and again to turn the page.
        Tap,
        /// Hold E until it is done.
        Hold,
        /// Stand and be there (a thing to witness).
        Stand,
        /// Bring the crate to the ditch, then the door.
        South,
    }

    #[derive(Clone, Copy, Debug)]
    struct Task {
        name: &'static str,
        hand: Hand,
        aim: Aim,
        act: Act,
        /// The tick it can be done from, and the tick it can no longer be.
        opens: u32,
        closes: u32,
    }

    /// The tile to stand on for each of the places E means something.
    const NORTH_STAND: (i32, i32) = (18, 4);
    const SOUTH_STAND: (i32, i32) = (17, 24);
    const WELL_STAND: (i32, i32) = (17, 14);
    const INK_STAND: (i32, i32) = (11, 3);
    const REED_STAND: (i32, i32) = (13, 5);

    /// A tick on a day (0 to 2) at a label hour and minute.
    fn at(s: &Session, d: u32, hh: u32, mm: u32) -> u32 {
        let since_dawn = if hh < 6 { hh + 24 - 6 } else { hh - 6 };
        let minutes = (d * 24 + since_dawn) * 60 + mm;
        (u64::from(minutes) * u64::from(s.nums.day) / 1440) as u32
    }

    /// Whether any knight at the table holds `item`.
    fn held_by_any(s: &Session, item: Item) -> bool {
        s.seats.values().any(|st| st.knight.held == item)
    }

    fn up_seats(s: &Session) -> Vec<u32> {
        s.seats
            .iter()
            .filter(|(_, st)| st.knight.up())
            .map(|(&id, _)| id)
            .collect()
    }

    /// The table's tasks in the order they matter, whether they can be done
    /// yet or not.
    fn tasks(s: &Session) -> Vec<Task> {
        let day = s.nums.day;
        let end = clock::end(day);
        let now = s.clock;
        let m = &s.memory;
        let mut out: Vec<Task> = Vec::new();
        let mut add = |name, hand, aim, act, opens: u32, closes: u32| {
            out.push(Task {
                name,
                hand,
                aim,
                act,
                opens,
                closes,
            });
        };
        let hour = day / 24;
        // The three lights, once the verse is known and the Final Hours come.
        let fin = clock::final_start(day);
        let ready_soon = |i: usize| match i {
            0 => m.has_keep(NORTH_KEY),
            1 => m.has_keep(SOUTH_SEAL),
            _ => m.has_keep(OIL),
        } && m.knows(Fact::Verse);
        // The farthest first, then back through the middle: the north tower
        // (over the planks), the well, the south tower. The first to be lit
        // has the shortest wait for the last.
        for (i, stand) in [(0, NORTH_STAND), (2, WELL_STAND), (1, SOUTH_STAND)] {
            if ready_soon(i) && !s.flame(i) {
                add("light", Hand::Any, Aim::Tile(stand), Act::Hold, fin, end);
            }
        }
        // A knight beside a lit flame keeps the wisps off it.
        for (i, stand) in [(2, WELL_STAND), (1, SOUTH_STAND), (0, NORTH_STAND)] {
            if s.flame(i) {
                add("guard", Hand::Any, Aim::Tile(stand), Act::Stand, 0, end);
            }
        }
        // What is in hand goes where it is wanted, at the hour it is wanted.
        for d in 1..3 {
            add(
                "scroll to Mira",
                Hand::Holds(Item::Scroll),
                Aim::Folk(Folk::Mira),
                Act::Tap,
                at(s, d, 20, 55),
                at(s, d, 22, 15),
            );
        }
        add(
            "reeds to Nessa",
            Hand::Holds(Item::Reeds),
            Aim::Folk(Folk::Nessa),
            Act::Tap,
            at(s, 1, 8, 30),
            at(s, 1, 10, 30),
        );
        add(
            "pass to Tovin",
            Hand::Holds(Item::Pass),
            Aim::Folk(Folk::Tovin),
            Act::Tap,
            at(s, 1, 17, 0),
            at(s, 1, 19, 40),
        );
        add(
            "shoe to Aster",
            Hand::Holds(Item::Shoe),
            Aim::Folk(Folk::Aster),
            Act::Tap,
            at(s, 1, 16, 35),
            at(s, 1, 23, 0),
        );
        add(
            "ink to Orin",
            Hand::Holds(Item::Ink),
            Aim::Folk(Folk::Orin),
            Act::Tap,
            at(s, 0, 7, 0),
            at(s, 0, 8, 25),
        );
        // What there is to witness, at the hour it happens.
        if !m.knows(Fact::AsterComing) {
            add(
                "hear the herald",
                Hand::Any,
                Aim::Tile(town::Spot::Podium.tile()),
                Act::Stand,
                at(s, 0, 17, 40),
                at(s, 0, 18, 10),
            );
        }
        if !m.knows(Fact::AsterLame) {
            add(
                "see the rider",
                Hand::Any,
                Aim::Tile(town::Spot::EastGate.tile()),
                Act::Stand,
                at(s, 1, 16, 20),
                at(s, 1, 23, 0),
            );
        }
        if !m.knows(Fact::Verse) {
            add(
                "hear the verse",
                Hand::Any,
                Aim::Tile(town::Spot::WellSide.tile()),
                Act::Stand,
                at(s, 2, 20, 45),
                at(s, 2, 22, 0),
            );
        }
        // The seal at dawn, for a knight who wears both masks.
        if m.knows(Fact::MiraSeal)
            && m.has_mask(SWIFT)
            && m.has_mask(STOUT)
            && !m.has_keep(SOUTH_SEAL)
        {
            for d in 0..3 {
                add(
                    "seal from Mira",
                    Hand::Any,
                    Aim::Folk(Folk::Mira),
                    Act::Tap,
                    at(s, d, 6, 50),
                    at(s, d, 9, 0),
                );
            }
        }
        // The shoe on the iron cart day, for the rider.
        if m.knows(Fact::AsterLame)
            && s.world.shod.is_none()
            && !m.has_keep(OIL)
            && !held_by_any(s, Item::Shoe)
        {
            add(
                "shoe from Nessa",
                Hand::Free,
                Aim::Folk(Folk::Nessa),
                Act::Tap,
                at(s, 1, 6, 35),
                at(s, 1, 7, 55),
            );
        }
        // The pass, once it is written.
        if let Some(t) = s.world.ink_given {
            if !m.has_keep(NORTH_KEY) && !held_by_any(s, Item::Pass) {
                add(
                    "pass from Orin",
                    Hand::Free,
                    Aim::Folk(Folk::Orin),
                    Act::Tap,
                    t + PASS_HOURS * hour,
                    t + (PASS_HOURS + PASS_FRESH) * hour,
                );
            }
        }
        // The favours that earn the masks and the pass.
        if m.knows(Fact::OrinLost)
            && s.world.ink_given.is_none()
            && !s.world.has(INK_TAKEN)
            && !m.has_keep(NORTH_KEY)
        {
            add(
                "ink pot",
                Hand::Free,
                Aim::Tile(INK_STAND),
                Act::Tap,
                at(s, 0, 6, 15),
                at(s, 0, 8, 0),
            );
        }
        if m.knows(Fact::PellaScroll) && !m.has_mask(SWIFT) && !held_by_any(s, Item::Scroll) {
            add(
                "scroll from Pella",
                Hand::Free,
                Aim::Folk(Folk::Pella),
                Act::Tap,
                at(s, 0, 7, 0),
                at(s, 0, 8, 30),
            );
        }
        if m.knows(Fact::NessaCold) && !m.has_mask(STOUT) && !held_by_any(s, Item::Reeds) {
            add(
                "reeds",
                Hand::Free,
                Aim::Tile(REED_STAND),
                Act::Tap,
                at(s, 0, 18, 0),
                at(s, 0, 22, 0),
            );
        }
        // Ask each of them once, to hear what they want: in the order they
        // are met, the nearest first.
        let from = up_seats(s)
            .first()
            .map_or(town::Spot::Plaza.px(), |id| s.seats[id].knight.px());
        let mut asks: Vec<(i64, Fact, Folk)> = [
            (Fact::OrinLost, Folk::Orin),
            (Fact::NessaCold, Folk::Nessa),
            (Fact::PellaScroll, Folk::Pella),
            (Fact::TovinPass, Folk::Tovin),
            (Fact::MiraSeal, Folk::Mira),
        ]
        .into_iter()
        .filter(|&(fact, _)| !m.knows(fact))
        .map(|(fact, who)| {
            let at = folk::figure(who, now.max(1), day, None);
            (manhattan(from, (at.x, at.y)), fact, who)
        })
        .collect();
        asks.sort_by_key(|&(d, _, who)| (d, who));
        for (_, _, who) in asks {
            add("ask", Hand::Free, Aim::Folk(who), Act::Tap, 0, end);
        }
        // Make ready for the night: the crate into the ditch, the doors.
        if m.has_keep(SOUTH_SEAL) && !s.world.has(SOUTH_OPEN) {
            add(
                "south door",
                Hand::Any,
                Aim::Tile(SOUTH_STAND),
                Act::South,
                at(s, 2, 8, 0),
                end,
            );
        }
        if m.has_keep(NORTH_KEY) && !s.world.has(NORTH_OPEN) {
            add(
                "north door",
                Hand::Any,
                Aim::Tile(NORTH_STAND),
                Act::Tap,
                at(s, 2, 17, 0),
                end,
            );
        }
        // Spare hands: keep the lamps lit.
        // (Not in the hours before the three lights: everyone is wanted at a tower.)
        if clock::night(now, day) && (now + 2 * hour < fin || now >= fin) {
            for (n, &(tx, ty)) in town::LAMPS.iter().enumerate() {
                if s.lantern_scheduled(n as u8) && s.world.snuffed & (1 << n) != 0 {
                    add(
                        "relight",
                        Hand::Free,
                        Aim::Tile((tx, ty + 1)),
                        Act::Tap,
                        0,
                        end,
                    );
                }
            }
        }
        out
    }

    fn aim_px(s: &Session, aim: Aim, when: u32) -> Option<(i32, i32)> {
        match aim {
            Aim::Tile((tx, ty)) => Some((tx * TILE + TILE / 2, ty * TILE + TILE / 2)),
            Aim::Folk(f) => {
                let now = s.clock;
                // Visible now: where they are. Not: where they will be.
                let fig = s.figure(f);
                if fig.visible && when <= now {
                    Some((fig.x, fig.y))
                } else {
                    let at = folk::next_seen(f, when.max(now), s.nums.day)?;
                    let fig = folk::figure(f, at + 2, s.nums.day, s.world.shod);
                    Some((fig.x, fig.y))
                }
            }
        }
    }

    /// Whether a task can be done this tick.
    fn doable(s: &Session, t: &Task) -> bool {
        if s.clock < t.opens || s.clock >= t.closes {
            return false;
        }
        match t.aim {
            Aim::Folk(f) => s.figure(f).visible,
            Aim::Tile(_) => true,
        }
    }

    /// When a task will next be doable, if it still can be this loop.
    fn when(s: &Session, t: &Task) -> Option<u32> {
        if s.clock >= t.closes {
            return None;
        }
        let opens = t.opens.max(s.clock);
        match t.aim {
            Aim::Tile(_) => Some(opens),
            Aim::Folk(f) => {
                let at = folk::next_seen(f, opens, s.nums.day)?;
                (at < t.closes).then_some(at)
            }
        }
    }

    fn hand_ok(held: Item, h: Hand) -> bool {
        match h {
            Hand::Any => true,
            Hand::Free => held == Item::Empty,
            Hand::Holds(i) => held == i,
        }
    }

    fn manhattan(a: (i32, i32), b: (i32, i32)) -> i64 {
        i64::from((a.0 - b.0).abs() + (a.1 - b.1).abs())
    }

    /// Who does what, as every bot at the table works it out: each task, in
    /// order of importance, to the knight nearest it who can do it.
    fn assign(s: &Session) -> Vec<(u32, Option<Task>)> {
        let ids = up_seats(s);
        if ids.is_empty() {
            return Vec::new();
        }
        let all = tasks(s);
        let mut free: Vec<u32> = ids.clone();
        let mut given: Vec<(u32, Option<Task>)> = Vec::new();
        // What can be done now goes first, in order of importance; then what
        // opens soonest goes to whoever is still free.
        let mut later: Vec<(u32, usize)> = Vec::new();
        for pass in 0..2 {
            let order: Vec<usize> = if pass == 0 {
                (0..all.len()).filter(|&i| doable(s, &all[i])).collect()
            } else {
                later.clear();
                for (i, t) in all.iter().enumerate() {
                    if !doable(s, t) {
                        if let Some(w) = when(s, t) {
                            later.push((w, i));
                        }
                    }
                }
                later.sort();
                later.iter().map(|&(_, i)| i).collect()
            };
            for i in order {
                let t = &all[i];
                let Some(target) = aim_px(s, t.aim, t.opens) else {
                    continue;
                };
                let best = if t.name == "light" {
                    // Each flame has its knight, by seat order, however they
                    // have wandered: the hands do not change their minds.
                    let role = match t.aim {
                        Aim::Tile(NORTH_STAND) => 0,
                        Aim::Tile(WELL_STAND) => 1,
                        _ => 2,
                    };
                    Some(ids[role % ids.len().max(1)]).filter(|id| free.contains(id))
                } else {
                    free.iter()
                        .copied()
                        .filter(|&id| hand_ok(s.seats[&id].knight.held, t.hand))
                        .min_by_key(|&id| (manhattan(s.seats[&id].knight.px(), target), id))
                };
                if let Some(id) = best {
                    free.retain(|&f| f != id);
                    given.push((id, Some(*t)));
                }
            }
        }
        for id in free {
            given.push((id, None));
        }
        given
    }

    /// The step toward a tile, one tile at a time, centred in each so the
    /// feet never catch a corner.
    fn walk_to(
        s: &Session,
        k: &Knight,
        goal: (i32, i32),
        crate_at: Option<(i32, i32)>,
    ) -> (i8, i8) {
        let swift = k.mask == Mask::Swift;
        let ground = s.world.ground(s.clock, s.nums.day, swift);
        let field = town::field(goal, ground, crate_at);
        let (px, py) = k.px();
        let here = (px.div_euclid(TILE), py.div_euclid(TILE));
        let value = |t: (i32, i32)| {
            if (0..town::COLS).contains(&t.0) && (0..town::ROWS).contains(&t.1) {
                field[(t.1 * town::COLS + t.0) as usize]
            } else {
                u16::MAX
            }
        };
        let centre = (here.0 * TILE + TILE / 2, here.1 * TILE + TILE / 2);
        if here == goal {
            let (dx, dy) = (centre.0 - px, centre.1 - py);
            return (
                if dx.abs() > 2 { dx.signum() as i8 } else { 0 },
                if dy.abs() > 2 { dy.signum() as i8 } else { 0 },
            );
        }
        let mut best = (value(here), (0, 0));
        for d in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
            let v = value((here.0 + d.0, here.1 + d.1));
            if v < best.0 {
                best = (v, d);
            }
        }
        let (_, (dx, dy)) = best;
        if (dx, dy) == (0, 0) {
            return (0, 0);
        }
        // Centre across the way first, then go.
        if dx != 0 {
            let off = centre.1 - py;
            if off.abs() > 3 {
                return (0, off.signum() as i8);
            }
        } else {
            let off = centre.0 - px;
            if off.abs() > 3 {
                return (off.signum() as i8, 0);
            }
        }
        (dx as i8, dy as i8)
    }

    /// Alternating presses: a key is down on one tick and up on the next.
    fn tap(k: &Knight, bit: u16) -> bool {
        k.keys & bit == 0
    }

    /// A wisp close enough to touch: sidestep through it, or step away.
    fn danger(s: &Session, k: &Knight) -> Option<(i32, i32, bool)> {
        if k.hurt > 0 || k.roll > 0 || k.panel != Panel::Shut || !k.up() {
            return None;
        }
        let me = k.px();
        let near = s
            .world
            .wisps
            .iter()
            .map(|w| (w.x / Q, w.y / Q))
            .filter(|&p| manhattan(p, me) < 16)
            .min_by_key(|&p| manhattan(p, me))?;
        Some((me.0 - near.0, me.1 - near.1, k.roll_rest == 0))
    }

    /// What `id` presses this tick.
    pub fn bot(s: &Session, id: u32) -> Input {
        let Some(seat) = s.seats.get(&id) else {
            return Input::default();
        };
        let k = &seat.knight;
        if !s.phase_is_play() || !k.up() {
            return Input::default();
        }
        let none = Input::default();
        // A menu is open: finish with it.
        match k.panel {
            Panel::Ledger(_) => {
                return Input {
                    swing: tap(k, key::LEDGER),
                    ..none
                };
            }
            Panel::Bell(line) => {
                return if line > 0 {
                    Input {
                        move_y: if tap(k, key::UP) { -1 } else { 0 },
                        ..none
                    }
                } else {
                    Input {
                        interact: tap(k, key::USE),
                        ..none
                    }
                };
            }
            Panel::Shut => {}
        }
        if s.completed.is_some() {
            return none;
        }
        let plan = assign(s);
        // Nothing left that anyone can do this loop: ring the bell, together.
        let anything = plan.iter().any(|(_, t)| t.is_some());
        let waiting = s.final_hours() || tasks(s).iter().any(|t| when(s, t).is_some());
        if !anything && !waiting {
            if let Some(c) = s.call {
                if c.ayes & (1 << (id - 1)) == 0 {
                    return Input {
                        ult: tap(k, key::BELL),
                        ..none
                    };
                }
                return none;
            }
            return Input {
                ult: tap(k, key::BELL),
                ..none
            };
        }
        let mine = plan.iter().find(|(i, _)| *i == id).and_then(|(_, t)| *t);
        // The mask for the job: Swift for the road, Stout for the crate.
        let want_stout = mine.is_some_and(|t| t.act == Act::South) && !s.world.has(DITCH_FILLED);
        let want = if want_stout && s.memory.has_mask(STOUT) {
            Mask::Stout
        } else if s.memory.has_mask(SWIFT) {
            Mask::Swift
        } else {
            Mask::Bare
        };
        if k.mask != want && tap(k, key::MASK) {
            return Input { play: 1, ..none };
        }
        // A talk in progress: turn the page.
        if k.say.is_some() {
            return Input {
                interact: tap(k, key::USE),
                ..none
            };
        }
        // Hands full of something nobody wants: put it down.
        if k.held != Item::Empty && !tasks(s).iter().any(|t| t.hand == Hand::Holds(k.held)) {
            return Input {
                bomb: tap(k, key::DROP),
                ..none
            };
        }
        let mut input = match mine {
            Some(t) => do_task(s, k, &t),
            None => idle(s, k, id),
        };
        // Something is about to touch us: roll, in the way we were going or away.
        if let Some((dx, dy, can_roll)) = danger(s, k) {
            if (input.move_x, input.move_y) == (0, 0) {
                if dx.abs() >= dy.abs() {
                    input.move_x = dx.signum() as i8;
                } else {
                    input.move_y = dy.signum() as i8;
                }
            }
            if can_roll {
                input.dash = true;
            }
        }
        input
    }

    /// A knight with nothing to do waits near the next thing to happen.
    fn idle(s: &Session, k: &Knight, id: u32) -> Input {
        let mut later: Vec<(u32, Task)> = tasks(s)
            .into_iter()
            .filter(|t| !doable(s, t))
            .filter_map(|t| when(s, &t).map(|w| (w, t)))
            .collect();
        later.sort_by_key(|&(w, _)| w);
        // Spread the spare hands over the next few.
        let spare = up_seats(s).iter().position(|&i| i == id).unwrap_or(0);
        let spot = later
            .get(spare % later.len().max(1))
            .and_then(|(w, t)| aim_px(s, t.aim, *w))
            .unwrap_or(town::Spot::Plaza.px());
        let goal = (spot.0.div_euclid(TILE), spot.1.div_euclid(TILE));
        go(s, k, goal, None, 8)
    }

    fn go(
        s: &Session,
        k: &Knight,
        goal: (i32, i32),
        crate_at: Option<(i32, i32)>,
        reach: i32,
    ) -> Input {
        let (px, py) = k.px();
        let target = (goal.0 * TILE + TILE / 2, goal.1 * TILE + TILE / 2);
        if (px - target.0).abs() + (py - target.1).abs() <= reach {
            return Input::default();
        }
        let (mx, my) = walk_to(s, k, goal, crate_at);
        Input {
            move_x: mx,
            move_y: my,
            ..Default::default()
        }
    }

    fn do_task(s: &Session, k: &Knight, t: &Task) -> Input {
        let Some(target) = aim_px(s, t.aim, s.clock.max(t.opens)) else {
            return idle(s, k, 0);
        };
        let (px, py) = k.px();
        let near = (px - target.0).abs() + (py - target.1).abs();
        let goal = (target.0.div_euclid(TILE), target.1.div_euclid(TILE));
        let crate_at = s.world.crates.first().copied();
        if t.act == Act::South {
            return south(s, k, crate_at);
        }
        // A villager is spoken to from a step away; a place is stood on.
        let reach = match t.aim {
            Aim::Folk(_) => 18,
            Aim::Tile(_) => 5,
        };
        if !doable(s, t) {
            // Early: be there when it opens.
            return go(s, k, goal, crate_at, 6);
        }
        if near > reach {
            return go(s, k, goal, crate_at, reach.min(5));
        }
        match t.act {
            Act::Tap => Input {
                interact: tap(k, key::USE),
                ..Default::default()
            },
            Act::Hold => Input {
                interact: true,
                ..Default::default()
            },
            Act::Stand | Act::South => Input::default(),
        }
    }

    /// The south route: the crate into the ditch, then the seal to the door.
    fn south(s: &Session, k: &Knight, crate_at: Option<(i32, i32)>) -> Input {
        let (px, py) = k.px();
        if s.world.has(SOUTH_OPEN) {
            return Input::default();
        }
        if !s.world.has(DITCH_FILLED) {
            if let Some((cx, cy)) = crate_at {
                let top = (cx, cy - 1);
                let here = (px.div_euclid(TILE), py.div_euclid(TILE));
                if here == top || (here.0 == cx && here.1 < cy && cy - here.1 <= 1) {
                    // Centred over the crate, lean on it.
                    let centre = cx * TILE + TILE / 2;
                    if (px - centre).abs() > 2 {
                        return Input {
                            move_x: (centre - px).signum() as i8,
                            ..Default::default()
                        };
                    }
                    if k.mask == Mask::Stout {
                        return Input {
                            move_y: 1,
                            ..Default::default()
                        };
                    }
                    return Input::default();
                }
                return go(s, k, top, crate_at, 3);
            }
        }
        let door = SOUTH_STAND;
        let target = (door.0 * TILE + TILE / 2, door.1 * TILE + TILE / 2);
        if (px - target.0).abs() + (py - target.1).abs() > 8 {
            return go(s, k, door, None, 6);
        }
        Input {
            interact: tap(k, key::USE),
            ..Default::default()
        }
    }

    /// What a bot is up to, in words (for the tests that watch one).
    pub fn why(s: &Session, id: u32) -> String {
        let plan = assign(s);
        let mine = plan.iter().find(|(i, _)| *i == id).and_then(|(_, t)| *t);
        match mine {
            Some(t) => format!("{} ({:?}, {:?})", t.name, t.aim, t.act),
            None => "nothing".into(),
        }
    }
}

/// The native rules of Moonfall, as they ran in Rust: every method the module now answers.
pub trait Old {
    fn sit(&mut self, id: u32, joined: u32);
    fn old_step(
        &mut self,
        inputs: &BTreeMap<u32, Input>,
        heroes: &BTreeMap<u32, Hero>,
    ) -> Vec<&'static str>;
    fn age(&mut self);
    fn play(&mut self, inputs: &BTreeMap<u32, Input>, cues: &mut Vec<&'static str>);
    fn act(&mut self, id: u32, input: Input, cues: &mut Vec<&'static str>);
    fn cycle_mask(&mut self, id: u32);
    fn ask_bell(&mut self, id: u32, ring: Ring, cues: &mut Vec<&'static str>);
    fn put_hand(&mut self, id: u32);
    fn bell_tick(&mut self, cues: &mut Vec<&'static str>);
    fn ring(&mut self, ring: Ring, cues: &mut Vec<&'static str>);
    fn calm(&mut self);
    fn begin_rewind_for_test(&mut self);
    fn begin_rewind(&mut self, fell: bool, cues: &mut Vec<&'static str>);
    fn rewinding(&mut self, t: u32, from: u32, fell: bool, cues: &mut Vec<&'static str>);
    fn reset(&mut self);
    fn skipping(&mut self, t: u32, from: u32, to: u32, cues: &mut Vec<&'static str>);
    fn winning(&mut self, t: u32, cues: &mut Vec<&'static str>);
    fn world_tick(&mut self, cues: &mut Vec<&'static str>);
    fn cries(&mut self);
    fn witness(&mut self);
    fn phase_is_play(&self) -> bool;
    fn burn(&self) -> u32;
    fn nearest_use(&self, who: u32) -> Option<Use>;
    fn note(&mut self, who: u32, n: Note);
    fn announce(&mut self, n: Note);
    fn take(&mut self, who: u32, item: Item);
    fn held(&self, who: u32) -> Item;
    fn band(&self) -> u8;
    fn half(&self) -> u32;
    fn within(&self, day: Option<u32>, w: (u32, u32)) -> bool;
    fn dry(&self, k: &Knight) -> bool;
    fn press_use(&mut self, who: u32);
    fn relight(&mut self, who: u32, n: u8);
    fn door(&mut self, who: u32, north: bool);
    fn hold_use(&mut self, who: u32) -> Option<u8>;
    fn talk(&mut self, who: u32, to: Folk);
    fn give_away(&mut self, who: u32);
    fn keep_mask(&mut self, who: u32, bit: u8, mask: Mask, ticked: Folk);
    fn learn(&mut self, f: Fact);
    fn cry_number(&self) -> u8;
    fn shoe_day(&self) -> bool;
    fn wisp_want(&self) -> usize;
    fn guarded(&self, at: (i32, i32)) -> bool;
    fn put_out(&mut self, aim: u8);
    fn tick_wisps(&mut self) -> Vec<&'static str>;
}

impl Old for Session {
    fn sit(&mut self, id: u32, joined: u32) {
        let seat = self.parked.remove(&id).unwrap_or_else(|| {
            let at = SPAWN[(id as usize + 5) % SPAWN.len()];
            Seat {
                joined,
                knight: Knight::at(at),
            }
        });
        self.seats.insert(id, seat);
        self.peak = self.peak.max(self.seats.len() as u32);
    }

    /// One tick: the seats are whoever is here and awake, so a knight who
    /// leaves stands up and one who joins late sits down at the well.
    /// Returns the sounds to make.
    fn old_step(
        &mut self,
        inputs: &BTreeMap<u32, Input>,
        heroes: &BTreeMap<u32, Hero>,
    ) -> Vec<&'static str> {
        let mut cues = Vec::new();
        self.elapsed = (self.elapsed + 1).min(LIMIT);
        let gone: Vec<u32> = self
            .seats
            .keys()
            .copied()
            .filter(|id| !heroes.get(id).is_some_and(|h| !h.away && !h.stone))
            .collect();
        for id in gone {
            if let Some(seat) = self.seats.remove(&id) {
                // Only a knight who is still in the party is kept for later.
                if heroes.contains_key(&id) {
                    self.parked.insert(id, seat);
                }
            }
        }
        self.parked.retain(|id, _| heroes.contains_key(id));
        let late: Vec<u32> = heroes
            .iter()
            .filter(|(id, h)| !h.away && !h.stone && !self.seats.contains_key(*id))
            .map(|(&id, _)| id)
            .collect();
        for id in late {
            self.sit(id, self.elapsed);
        }
        self.age();
        self.clock = self.clock.min(clock::end(self.nums.day));
        match self.phase {
            Phase::Play => self.play(inputs, &mut cues),
            Phase::Rewind { t, from, fell } => self.rewinding(t, from, fell, &mut cues),
            Phase::Skip { t, from, to } => self.skipping(t, from, to, &mut cues),
            Phase::Won { t } => self.winning(t, &mut cues),
        }
        cues.append(&mut self.pending);
        cues
    }

    /// The timers that only run down.
    fn age(&mut self) {
        self.card = self.card.saturating_sub(1);
        for (_, n) in &mut self.news {
            *n = n.saturating_sub(1);
        }
        self.news.retain(|&(_, n)| n > 0);
        if let Some((_, _, age)) = &mut self.cry {
            *age += 1;
        }
        if self.cry.is_some_and(|(_, _, age)| age > 150) {
            self.cry = None;
        }
        for s in self.seats.values_mut() {
            let k = &mut s.knight;
            if let Some((_, left)) = &mut k.note {
                *left = left.saturating_sub(1);
            }
            if k.note.is_some_and(|(_, left)| left == 0) {
                k.note = None;
            }
            if let Some(say) = &mut k.say {
                say.age += 1;
            }
            if k.say.is_some_and(|say| say.age > 360) {
                k.say = None;
            }
        }
    }

    fn play(&mut self, inputs: &BTreeMap<u32, Input>, cues: &mut Vec<&'static str>) {
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        for id in ids {
            let input = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .unwrap_or_default();
            self.act(id, input, cues);
        }
        self.bell_tick(cues);
        if self.completed.is_some() {
            return;
        }
        self.world_tick(cues);
        if (0..3).all(|i| self.flame(i)) {
            self.phase = Phase::Won { t: 0 };
            cues.push("circle_toll");
            cues.push("wish_grant");
        } else if self.clock >= clock::end(self.nums.day) {
            self.begin_rewind(true, cues);
        }
    }

    /// One knight's tick.
    fn act(&mut self, id: u32, input: Input, cues: &mut Vec<&'static str>) {
        let keys = keys_of(input);
        let nums = self.nums;
        let clock = self.clock;
        let before = {
            let Some(s) = self.seats.get_mut(&id) else {
                return;
            };
            let before = s.knight.keys;
            s.knight.keys = keys;
            s.knight.hurt = s.knight.hurt.saturating_sub(1);
            if s.knight.down > 0 {
                s.knight.down -= 1;
                if s.knight.down == 0 {
                    let at = SPAWN[(id as usize + 5) % SPAWN.len()];
                    let fresh = Knight::at(at);
                    s.knight.x = fresh.x;
                    s.knight.y = fresh.y;
                    s.knight.candles = 1;
                    s.knight.hurt = 90;
                }
                return;
            }
            before
        };
        let press = |bit| pressed(keys, before, bit);
        let panel = self.seats[&id].knight.panel;
        match panel {
            Panel::Ledger(page) => {
                let written = self.memory.written();
                let k = &mut self.seats.get_mut(&id).unwrap().knight;
                k.read = written;
                if press(key::LEDGER) || press(key::USE) || press(key::SIDESTEP) {
                    k.panel = Panel::Shut;
                } else if press(key::RIGHT) {
                    k.panel = Panel::Ledger((page + 1) % 7);
                } else if press(key::LEFT) {
                    k.panel = Panel::Ledger((page + 6) % 7);
                }
                return;
            }
            Panel::Bell(line) => {
                let k = &mut self.seats.get_mut(&id).unwrap().knight;
                if press(key::UP) {
                    k.panel = Panel::Bell((line + 2) % 3);
                } else if press(key::DOWN) {
                    k.panel = Panel::Bell((line + 1) % 3);
                } else if press(key::LEDGER) || press(key::SIDESTEP) || press(key::MASK) {
                    k.panel = Panel::Shut;
                } else if press(key::USE) || press(key::BELL) {
                    k.panel = Panel::Shut;
                    let ring = [Ring::Rewind, Ring::Dusk, Ring::Dawn][usize::from(line)];
                    self.ask_bell(id, ring, cues);
                }
                return;
            }
            Panel::Shut => {}
        }
        if press(key::LEDGER) {
            let written = self.memory.written();
            let k = &mut self.seats.get_mut(&id).unwrap().knight;
            k.panel = Panel::Ledger(0);
            k.say = None;
            k.read = written;
            return;
        }
        // The moon has turned back: the bell has nothing left to wind.
        if press(key::BELL) && self.completed.is_none() {
            if self.call.is_some_and(|c| c.ayes & (1 << (id - 1)) == 0) {
                self.put_hand(id);
            } else if self.call.is_none() {
                let k = &mut self.seats.get_mut(&id).unwrap().knight;
                k.panel = Panel::Bell(0);
                k.say = None;
            }
            return;
        }
        if press(key::MASK) {
            self.cycle_mask(id);
        }
        if press(key::DROP) {
            let held = self.seats[&id].knight.held;
            if held != Item::Empty {
                if held == Item::Ink {
                    self.world.flags &= !INK_TAKEN;
                }
                let k = &mut self.seats.get_mut(&id).unwrap().knight;
                k.held = Item::Empty;
                k.note = Some((Note::Sign(says::Sign::Dropped), 60));
            }
        }
        // E: the next page of a talk, or what E means here.
        let talking = self.seats[&id].knight.say;
        if press(key::USE) {
            if let Some(say) = talking {
                let pages = self.say_pages(&say);
                let k = &mut self.seats.get_mut(&id).unwrap().knight;
                if usize::from(say.page) + 1 >= pages {
                    k.say = None;
                } else if let Some(s) = &mut k.say {
                    s.page += 1;
                }
            } else {
                self.press_use(id);
            }
        } else if keys & key::USE != 0 && talking.is_none() {
            if self.hold_use(id).is_some() {
                cues.push("wish_grant");
                cues.push("circle_toll");
            }
        } else if let Some(s) = self.seats.get_mut(&id) {
            s.knight.hold = 0;
        }
        // Walking. Moving away from someone ends the talk.
        let swift = self.seats[&id].knight.mask == Mask::Swift;
        let ground = self.world.ground(clock, nums.day, swift);
        let mut crates = std::mem::take(&mut self.world.crates);
        let moved = {
            let k = &mut self.seats.get_mut(&id).unwrap().knight;
            let pace = walk::speed(nums.walk, k.mask);
            let w = walk::step(k, input, pace, ground, &mut crates, press(key::SIDESTEP));
            if (input.move_x, input.move_y) != (0, 0) {
                k.say = None;
            }
            w
        };
        self.world.crates = crates;
        if moved.filled {
            self.world.set(DITCH_FILLED);
            cues.push("rock_land");
        }
        if moved.moved {
            cues.push("rock_land");
        }
        if moved.heavy {
            let k = &mut self.seats.get_mut(&id).unwrap().knight;
            if k.note.is_none() {
                k.note = Some((Note::Sign(says::Sign::TooHeavy), 60));
            }
        }
    }

    fn cycle_mask(&mut self, id: u32) {
        let owned = |m: Mask| match m {
            Mask::Bare => true,
            Mask::Swift => self.memory.has_mask(ledger::SWIFT),
            Mask::Stout => self.memory.has_mask(ledger::STOUT),
        };
        let order = [Mask::Bare, Mask::Swift, Mask::Stout];
        let now = self.seats[&id].knight.mask;
        let at = order.iter().position(|&m| m == now).unwrap_or(0);
        let next = (1..=3)
            .map(|d| order[(at + d) % 3])
            .find(|&m| owned(m))
            .unwrap_or(Mask::Bare);
        self.seats.get_mut(&id).unwrap().knight.mask = next;
    }

    fn ask_bell(&mut self, id: u32, ring: Ring, cues: &mut Vec<&'static str>) {
        if !self.ring_ok(ring) {
            return;
        }
        // The same ring already asked for: this knight's hand is another.
        if let Some(c) = &mut self.call {
            if c.ring == ring {
                c.ayes |= 1 << (id - 1);
            }
            return;
        }
        let call = Call {
            ring,
            by: id,
            ayes: 1 << (id - 1),
            left: CALL_TICKS,
        };
        self.announce(Note::Ring(id));
        if call.ayes.count_ones() >= self.bell_need() {
            self.ring(ring, cues);
        } else {
            self.call = Some(call);
        }
    }

    fn put_hand(&mut self, id: u32) {
        if let Some(c) = &mut self.call {
            c.ayes |= 1 << (id - 1);
        }
    }

    fn bell_tick(&mut self, cues: &mut Vec<&'static str>) {
        let need = self.bell_need();
        let Some(call) = &mut self.call else { return };
        // A knight who left takes their hand off the rope.
        let seated: u8 = self.seats.keys().fold(0, |m, &id| m | (1 << (id - 1)));
        call.ayes &= seated;
        call.left = call.left.saturating_sub(1);
        if call.ayes.count_ones() >= need {
            let ring = call.ring;
            self.call = None;
            self.ring(ring, cues);
        } else if call.left == 0 || call.ayes == 0 {
            self.call = None;
        }
    }

    /// Strike the bell: the ritual, or time run on.
    fn ring(&mut self, ring: Ring, cues: &mut Vec<&'static str>) {
        match ring {
            Ring::Rewind => self.begin_rewind(false, cues),
            Ring::Dusk | Ring::Dawn => {
                if !self.ring_ok(ring) {
                    return;
                }
                let day = self.nums.day;
                let to = if ring == Ring::Dusk {
                    clock::next_dusk(self.clock, day)
                } else {
                    clock::next_dawn(self.clock, day)
                };
                self.phase = Phase::Skip {
                    t: 0,
                    from: self.clock,
                    to,
                };
                self.calm();
                cues.push("circle_toll");
            }
        }
    }

    /// Menus shut and talk ended, for a change of time.
    fn calm(&mut self) {
        self.call = None;
        for s in self.seats.values_mut() {
            s.knight.panel = Panel::Shut;
            s.knight.say = None;
            s.knight.hold = 0;
        }
    }

    /// Ring the rewind from outside a knight's hands (the tests').
    #[cfg(test)]
    fn begin_rewind_for_test(&mut self) {
        let mut cues = Vec::new();
        self.begin_rewind(false, &mut cues);
    }

    fn begin_rewind(&mut self, fell: bool, cues: &mut Vec<&'static str>) {
        self.phase = Phase::Rewind {
            t: 0,
            from: self.clock.min(clock::end(self.nums.day)),
            fell,
        };
        self.calm();
        cues.push("circle_toll");
        if fell {
            cues.push("boss_roar");
        }
    }

    fn rewinding(&mut self, t: u32, from: u32, fell: bool, cues: &mut Vec<&'static str>) {
        let t = t + 1;
        let control = control(&self.nums);
        if t == control / 2 {
            cues.push("wall_up");
        }
        if t >= control {
            self.reset();
            cues.push("play_card");
        } else {
            self.phase = Phase::Rewind { t, from, fell };
        }
    }

    /// The town returns to the first dawn. The Ledger and the masks stay;
    /// everything else, carried items and opened doors and the rest, is
    /// wound back.
    fn reset(&mut self) {
        self.memory.loops += 1;
        self.clock = 0;
        self.world = World::dawn();
        self.phase = Phase::Play;
        self.card = self.nums.rewind.saturating_sub(control(&self.nums)) + 24;
        self.call = None;
        self.cry = None;
        self.news.clear();
        for (&id, s) in self.seats.iter_mut().chain(self.parked.iter_mut()) {
            let mask = s.knight.mask;
            let at = SPAWN[(id as usize + 5) % SPAWN.len()];
            s.knight = Knight::at(at);
            s.knight.mask = mask;
        }
    }

    fn skipping(&mut self, t: u32, from: u32, to: u32, cues: &mut Vec<&'static str>) {
        let t = t + 1;
        if t >= SKIP_LEN {
            let day = self.nums.day;
            self.clock = to;
            if to / day != from / day || to % day == 0 {
                self.world.snuffed = 0;
            }
            self.world.wisps.clear();
            self.phase = Phase::Play;
            cues.push("circle_toll");
        } else {
            self.phase = Phase::Skip { t, from, to };
        }
    }

    fn winning(&mut self, t: u32, cues: &mut Vec<&'static str>) {
        let t = t + 1;
        if t >= WON_LEN {
            self.completed = Some(self.elapsed);
            self.clock = clock::end(self.nums.day);
            // The three lights burn on: the moon has turned back.
            self.world.lit_until = [self.clock * 2; 3];
            self.world.wisps.clear();
            self.phase = Phase::Play;
            cues.push("wish_grant");
        } else {
            self.phase = Phase::Won { t };
        }
    }

    /// The town's own tick: the clock, the bells, the Gloam, the Ledger.
    fn world_tick(&mut self, cues: &mut Vec<&'static str>) {
        let day = self.nums.day;
        let hour = (day / 24).max(1);
        let before_bridge = clock::bridge_up(self.clock, day);
        // The loop ends where it ends, however the day knob has moved.
        self.clock = (self.clock + 1).min(clock::end(day));
        if self.clock % hour == 0 {
            let h = clock::stamp(self.clock, day).hour;
            if h % 6 == 0 || self.final_hours() {
                cues.push("circle_toll");
            }
        }
        if clock::bridge_up(self.clock, day) != before_bridge {
            cues.push("door_open");
        }
        if self.clock % day == 0 {
            self.world.snuffed = 0;
        }
        // The moon's shadow puts the street lanterns out, one by one.
        if self.final_hours() {
            let fin = clock::final_start(day);
            for n in 0..6u32 {
                if self.clock == fin + n * hour / 2 && self.world.snuffed & (1 << n) == 0 {
                    self.world.snuffed |= 1 << n;
                    self.announce(Note::Out);
                }
            }
        }
        // Aster's oil reaches the well.
        if let Some(t) = self.world.shod {
            if !self.memory.has_keep(ledger::OIL) && self.clock >= folk::aster_at_well(t) {
                self.memory.keeps |= ledger::OIL;
                if self.memory.tick(Folk::Aster) {
                    self.announce(Note::Ticked(Folk::Aster));
                }
                cues.push("wish_grant");
            }
        }
        self.cries();
        if self.clock % 5 == 0 {
            self.witness();
        }
        for (who, id) in ledger::reminders(&self.memory, self.clock, day) {
            self.announce(Note::Remind(who, id));
        }
        cues.extend(self.tick_wisps());
    }

    /// Pella cries the hours and Mira says her verse when they reach their
    /// places.
    fn cries(&mut self) {
        let day = self.nums.day;
        let table = folk::timetable(day);
        for (who, routine) in [(Folk::Pella, 2u8), (Folk::Pella, 4), (Folk::Mira, 10)] {
            for leg in table.legs[who.index()]
                .iter()
                .filter(|l| l.routine == routine)
            {
                if leg.arrive != self.clock || leg.arrive == 0 {
                    continue;
                }
                let d = self.day_index();
                let topic = match (who, routine) {
                    (Folk::Pella, 2) => Topic::Cry(0),
                    (Folk::Pella, _) => Topic::Cry([1, 2, 3][d as usize]),
                    _ if d == 2 => Topic::Cry(0),
                    _ => continue,
                };
                self.cry = Some((who, topic, 0));
            }
        }
    }

    /// The Ledger writes what a knight is near enough to see.
    fn witness(&mut self) {
        let notice = self.nums.notice as i32;
        let knights: Vec<(i32, i32)> = self
            .seats
            .values()
            .filter(|s| s.knight.up())
            .map(|s| s.knight.px())
            .collect();
        for who in CAST {
            let fig = self.figure(who);
            if !fig.arrived {
                continue;
            }
            let near = knights
                .iter()
                .any(|&(x, y)| (x - fig.x).pow(2) + (y - fig.y).pow(2) <= notice * notice);
            if !near {
                continue;
            }
            if self.memory.witness(who, fig.routine) {
                self.announce(Note::Wrote(who, fig.routine));
            }
            // What a person's doing may also be a thing to learn.
            let d = self.day_index();
            let fact = match (who, fig.routine) {
                (Folk::Pella, 4) if d == 0 => Some(ledger::Fact::AsterComing),
                (Folk::Aster, 0) => Some(ledger::Fact::AsterLame),
                (Folk::Mira, 10) if d == 2 => Some(ledger::Fact::Verse),
                _ => None,
            };
            if let Some(f) = fact {
                if self.memory.learn(f) {
                    self.announce(Note::Learned(f));
                }
            }
        }
    }

    fn phase_is_play(&self) -> bool {
        self.phase == Phase::Play
    }

    /// Ticks a flame burns once lit: tighter with every extra seat.
    fn burn(&self) -> u32 {
        let hour = self.nums.day / 24;
        (hour * 9 / 2)
            .saturating_sub(hour * (self.seats_peak() - 1) / 4)
            .max(hour * 5 / 2)
    }

    /// The thing nearest a knight's feet that E could mean.
    fn nearest_use(&self, who: u32) -> Option<Use> {
        let k = &self.seats.get(&who)?.knight;
        let me = k.px();
        let mut best: Option<(i32, Use)> = None;
        let mut offer = |d2: i32, reach: i32, u: Use| {
            if d2 <= reach * reach && best.is_none_or(|(b, _)| d2 < b) {
                best = Some((d2, u));
            }
        };
        for f in CAST {
            let fig = self.figure(f);
            if fig.visible {
                offer(dist2(me, (fig.x, fig.y)), 26, Use::Folk(f));
            }
        }
        offer(dist2(me, town::INK_POT), 22, Use::Ink);
        offer(dist2(me, town::REED_STACK), 22, Use::Reeds);
        offer(dist2(me, town::NORTH_DOOR), 26, Use::North);
        offer(dist2(me, town::SOUTH_DOOR), 26, Use::South);
        offer(dist2(me, town::WELL), 38, Use::Well);
        offer(dist2(me, town::BELL_POST), 22, Use::Bell);
        for (n, &(tx, ty)) in town::LAMPS.iter().enumerate() {
            let at = (tx * town::TILE + 8, ty * town::TILE + 4);
            offer(dist2(me, at), 18, Use::Lantern(n as u8));
        }
        best.map(|(_, u)| u)
    }

    fn note(&mut self, who: u32, n: Note) {
        if let Some(s) = self.seats.get_mut(&who) {
            s.knight.note = Some((n, 90));
        }
    }

    /// Everyone's: the Ledger wrote, or an hour is near.
    fn announce(&mut self, n: Note) {
        match n {
            Note::Wrote(..) | Note::Learned(_) => self.pending.push("card_pickup"),
            Note::Ticked(_) => self.pending.push("wish_grant"),
            Note::Remind(..) => self.pending.push("play_card"),
            _ => {}
        }
        self.news.push((n, 120));
        if self.news.len() > 3 {
            self.news.remove(0);
        }
    }

    fn take(&mut self, who: u32, item: Item) {
        let clock = self.clock;
        if let Some(s) = self.seats.get_mut(&who) {
            s.knight.held = item;
            s.knight.held_at = clock;
        }
        self.note(who, Note::Got(item));
    }

    fn held(&self, who: u32) -> Item {
        self.seats.get(&who).map_or(Item::Empty, |s| s.knight.held)
    }

    /// The part of the day by the clock, for small talk.
    fn band(&self) -> u8 {
        says::band(clock::stamp(self.clock, self.nums.day).hour)
    }

    /// Half-hours since the first dawn.
    fn half(&self) -> u32 {
        clock::half(self.clock, self.nums.day)
    }

    /// Whether it is the `day` (0 to 2) and within `w` half-hours of its dawn.
    fn within(&self, day: Option<u32>, w: (u32, u32)) -> bool {
        let h = self.half();
        day.is_none_or(|d| h / 48 == d) && (w.0..w.1).contains(&(h % 48))
    }

    /// Whether the reeds in a knight's hand are dry.
    fn dry(&self, k: &Knight) -> bool {
        k.held == Item::Reeds && self.clock >= k.held_at + DRY_HOURS * self.nums.day / 24
    }

    /// E pressed: do the one thing it means here.
    fn press_use(&mut self, who: u32) {
        match self.nearest_use(who) {
            Some(Use::Folk(f)) => self.talk(who, f),
            Some(Use::Ink) => {
                if self.held(who) != Item::Empty {
                    self.note(who, Note::Sign(Sign::HandsFull));
                } else if !self.memory.knows(Fact::OrinLost) || self.world.has(INK_TAKEN) {
                    self.note(who, Note::Sign(Sign::NoInk));
                } else {
                    self.world.set(INK_TAKEN);
                    self.take(who, Item::Ink);
                }
            }
            Some(Use::Reeds) => {
                if self.held(who) != Item::Empty {
                    self.note(who, Note::Sign(Sign::HandsFull));
                } else if !self.memory.knows(Fact::NessaCold) {
                    self.note(who, Note::Sign(Sign::NoInk));
                } else {
                    self.take(who, Item::Reeds);
                }
            }
            Some(Use::North) => self.door(who, true),
            Some(Use::South) => self.door(who, false),
            Some(Use::Well) => {
                let sign = if !self.memory.has_keep(ledger::OIL) {
                    Sign::WellIdle
                } else if self.final_hours() && !self.memory.knows(Fact::Verse) {
                    Sign::TowerNeedsVerse
                } else {
                    Sign::WellOiled
                };
                self.note(who, Note::Sign(sign));
            }
            Some(Use::Bell) => {
                // E at the post is the bell's own menu, as R is.
                if self.completed.is_none() && self.call.is_none() {
                    if let Some(s) = self.seats.get_mut(&who) {
                        s.knight.panel = Panel::Bell(0);
                        s.knight.say = None;
                    }
                }
            }
            Some(Use::Lantern(n)) => self.relight(who, n),
            None => {}
        }
    }

    fn relight(&mut self, who: u32, n: u8) {
        if self.lantern_scheduled(n) && self.world.snuffed & (1 << n) != 0 {
            self.world.snuffed &= !(1 << n);
            self.pending.push("spoil_pickup");
            self.note(who, Note::Sign(Sign::Relit));
        }
    }

    /// The tower doors open to the key and the seal the party keeps.
    fn door(&mut self, who: u32, north: bool) {
        let (flag, keep) = if north {
            (NORTH_OPEN, NORTH_KEY)
        } else {
            (SOUTH_OPEN, SOUTH_SEAL)
        };
        if self.world.has(flag) {
            if self.final_hours() {
                if !self.memory.knows(Fact::Verse) {
                    self.note(who, Note::Sign(Sign::TowerNeedsVerse));
                }
                return;
            }
            self.note(who, Note::Sign(Sign::TowerIdle));
        } else if self.memory.has_keep(keep) {
            self.world.set(flag);
            self.note(
                who,
                Note::Sign(if north {
                    Sign::NorthOpened
                } else {
                    Sign::SouthOpened
                }),
            );
        } else {
            self.note(
                who,
                Note::Sign(if north {
                    Sign::NorthLocked
                } else {
                    Sign::SouthLocked
                }),
            );
        }
    }

    /// E held: at a tower that is ready, light it when the hold is full.
    fn hold_use(&mut self, who: u32) -> Option<u8> {
        let target = self.nearest_use(who);
        let i = match target {
            Some(Use::North) => 0,
            Some(Use::South) => 1,
            Some(Use::Well) => 2,
            _ => {
                if let Some(s) = self.seats.get_mut(&who) {
                    s.knight.hold = 0;
                }
                return None;
            }
        };
        if !self.tower_ready(i) {
            if let Some(s) = self.seats.get_mut(&who) {
                s.knight.hold = 0;
            }
            return None;
        }
        let full = {
            let k = &mut self.seats.get_mut(&who)?.knight;
            k.hold += 1;
            k.hold >= LIGHT_HOLD
        };
        if full {
            if let Some(s) = self.seats.get_mut(&who) {
                s.knight.hold = 0;
            }
            if !self.flame(i) {
                let burn = self.burn();
                self.world.lit_until[i] = self.clock + burn;
                self.announce(Note::Lit(i as u8));
                return Some(i as u8);
            }
        }
        None
    }

    /// Start a conversation: choose what is said, and do what it means.
    fn talk(&mut self, who: u32, to: Folk) {
        let held = self.held(who);
        let band = self.band();
        let day_ticks = self.nums.day;
        let hour = clock::stamp(self.clock, day_ticks).hour;
        let me = self.seats[&who].knight.clone();
        let topic = match to {
            Folk::Mira => {
                if held == Item::Scroll {
                    // She reads the post aloud at the well on the second and
                    // third evenings.
                    if self.day_index() >= 1 && self.figure(Folk::Mira).routine == 10 {
                        self.give_away(who);
                        self.keep_mask(who, SWIFT, Mask::Swift, Folk::Pella);
                        Topic::Thanks
                    } else {
                        Topic::Nag(2)
                    }
                } else if !self.memory.knows(Fact::MiraSeal) {
                    self.learn(Fact::MiraSeal);
                    Topic::Request
                } else if self.cry_active(Folk::Mira) {
                    Topic::Cry(0)
                } else if self.memory.has_keep(SOUTH_SEAL) {
                    Topic::Done
                } else if self.within(None, PEN) {
                    if self.memory.has_mask(SWIFT) && self.memory.has_mask(STOUT) {
                        self.memory.keeps |= SOUTH_SEAL;
                        self.note(who, Note::Sign(Sign::SealKept));
                        if self.memory.tick(Folk::Mira) {
                            self.announce(Note::Ticked(Folk::Mira));
                        }
                        Topic::Give
                    } else {
                        Topic::Nag(0)
                    }
                } else if self.memory.has_mask(SWIFT) && self.memory.has_mask(STOUT) {
                    Topic::Nag(1)
                } else {
                    Topic::Hello(band)
                }
            }
            Folk::Pella => {
                if !self.memory.knows(Fact::PellaScroll) {
                    self.learn(Fact::PellaScroll);
                    Topic::Request
                } else if self.cry_active(Folk::Pella) {
                    Topic::Cry(self.cry_number())
                } else if self.memory.has_mask(SWIFT) {
                    Topic::Done
                } else if held == Item::Scroll {
                    Topic::Nag(0)
                } else if !self.within(Some(0), POST) {
                    Topic::Nag(1)
                } else if held == Item::Empty {
                    self.take(who, Item::Scroll);
                    Topic::Give
                } else {
                    self.note(who, Note::Sign(Sign::HandsFull));
                    Topic::Hello(band)
                }
            }
            Folk::Nessa => {
                if !self.memory.knows(Fact::NessaCold) {
                    self.learn(Fact::NessaCold);
                    Topic::Request
                } else if held == Item::Reeds {
                    if !self.dry(&me) {
                        Topic::Nag(2)
                    } else if !self.within(Some(1), OVEN) {
                        Topic::Nag(3)
                    } else {
                        self.give_away(who);
                        self.keep_mask(who, STOUT, Mask::Stout, Folk::Nessa);
                        Topic::Thanks
                    }
                } else if self.shoe_day() {
                    if !self.memory.knows(Fact::AsterLame) {
                        Topic::Nag(0)
                    } else if self.world.shod.is_some() {
                        Topic::Done
                    } else if held == Item::Empty {
                        self.take(who, Item::Shoe);
                        Topic::Give
                    } else {
                        self.note(who, Note::Sign(Sign::HandsFull));
                        Topic::Hello(band)
                    }
                } else if self.memory.has_mask(STOUT) {
                    if self.memory.knows(Fact::AsterLame) && !self.memory.has_keep(ledger::OIL) {
                        Topic::Nag(1)
                    } else {
                        Topic::Done
                    }
                } else {
                    Topic::Hello(band)
                }
            }
            Folk::Orin => {
                if !self.memory.knows(Fact::OrinLost) {
                    self.learn(Fact::OrinLost);
                    Topic::Request
                } else if held == Item::Ink {
                    if self.within(Some(0), POST) {
                        self.give_away(who);
                        self.world.ink_given = Some(self.clock);
                        Topic::Thanks
                    } else {
                        Topic::Nag(1)
                    }
                } else if let Some(t) = self.world.ink_given {
                    let ready = t + PASS_HOURS * day_ticks / 24;
                    if self.clock < ready {
                        Topic::Nag(0)
                    } else if self.memory.ticked(Folk::Orin) && self.memory.has_keep(NORTH_KEY) {
                        Topic::Done
                    } else if self.clock > ready + PASS_FRESH * day_ticks / 24 {
                        Topic::Nag(2)
                    } else if held == Item::Empty {
                        self.take(who, Item::Pass);
                        if self.memory.tick(Folk::Orin) {
                            self.announce(Note::Ticked(Folk::Orin));
                        }
                        Topic::Give
                    } else {
                        self.note(who, Note::Sign(Sign::HandsFull));
                        Topic::Hello(band)
                    }
                } else {
                    Topic::Hello(band)
                }
            }
            Folk::Tovin => {
                if !self.memory.knows(Fact::TovinPass) {
                    self.learn(Fact::TovinPass);
                    Topic::Request
                } else if held == Item::Pass {
                    if self.within(Some(1), KEYS) {
                        self.give_away(who);
                        self.memory.keeps |= NORTH_KEY;
                        self.note(who, Note::Sign(Sign::KeyKept));
                        if self.memory.tick(Folk::Tovin) {
                            self.announce(Note::Ticked(Folk::Tovin));
                        }
                        Topic::Thanks
                    } else {
                        Topic::Nag(1)
                    }
                } else if self.memory.has_keep(NORTH_KEY) {
                    Topic::Done
                } else if hour >= 20 || hour < 6 {
                    Topic::Hello(3)
                } else {
                    Topic::Hello(band)
                }
            }
            Folk::Aster => {
                if held == Item::Shoe && self.world.shod.is_none() {
                    self.give_away(who);
                    self.world.shod = Some(self.clock);
                    Topic::Thanks
                } else if self.memory.has_keep(ledger::OIL) {
                    Topic::Done
                } else if self.world.shod.is_some() {
                    Topic::Give
                } else {
                    if self.memory.learn(Fact::AsterLame) {
                        self.announce(Note::Learned(Fact::AsterLame));
                    }
                    Topic::Request
                }
            }
        };
        if let Some(s) = self.seats.get_mut(&who) {
            s.knight.say = Some(Say {
                folk: to,
                topic,
                page: 0,
                age: 0,
            });
        }
    }

    fn give_away(&mut self, who: u32) {
        if let Some(s) = self.seats.get_mut(&who) {
            s.knight.held = Item::Empty;
        }
    }

    /// A mask is earned and kept, and the person's tick goes in the Ledger.
    fn keep_mask(&mut self, who: u32, bit: u8, mask: Mask, ticked: Folk) {
        self.memory.masks |= bit;
        if self.memory.tick(ticked) {
            self.announce(Note::Ticked(ticked));
        }
        self.note(who, Note::Mask(mask));
    }

    fn learn(&mut self, f: Fact) {
        if self.memory.learn(f) {
            self.announce(Note::Learned(f));
        }
    }

    fn cry_number(&self) -> u8 {
        match self.cry {
            Some((_, Topic::Cry(n), _)) => n,
            _ => 0,
        }
    }

    /// The second day's iron cart: Nessa's rack of shoes is out.
    fn shoe_day(&self) -> bool {
        self.within(Some(1), IRON) && self.figure(Folk::Nessa).arrived
    }

    /// How many wisps the night holds for the table: the number, and two
    /// more for every knight beyond the first, and one more in the Final Hours.
    fn wisp_want(&self) -> usize {
        if !clock::night(self.clock, self.nums.day) || self.completed.is_some() {
            return 0;
        }
        let seats = self.seats_peak() as usize;
        let more = usize::from(self.final_hours());
        (self.nums.wisps as usize + 2 * (seats - 1) + more).min(MAX_WISPS)
    }

    /// Whether a knight stands close enough to keep wisps off a light.
    fn guarded(&self, at: (i32, i32)) -> bool {
        self.seats.values().any(|s| {
            let (x, y) = s.knight.px();
            s.knight.up() && (x - at.0).pow(2) + (y - at.1).pow(2) <= WARD * WARD
        })
    }

    fn put_out(&mut self, aim: u8) {
        if aim < 6 {
            self.world.snuffed |= 1 << aim;
        } else {
            self.world.lit_until[usize::from(aim) - 6] = 0;
        }
        self.pending.push("mana_empty");
        self.announce(Note::Out);
    }

    /// One tick of the Gloam. Returns the cues to sound.
    fn tick_wisps(&mut self) -> Vec<&'static str> {
        let mut cues = Vec::new();
        let want = self.wisp_want();
        if want == 0 {
            self.world.wisps.clear();
            return cues;
        }
        if self.world.wisps.len() < want && self.clock % 45 == 0 {
            let born = self.elapsed;
            let (x, y) = COMING[(born / 45) as usize % COMING.len()];
            self.world.wisps.push(Wisp {
                x: x * Q,
                y: y * Q,
                mode: WispMode::Hunt,
                aim: 0,
                born,
            });
        }
        let drift = DRIFT + DRIFT * (self.seats_peak() as i32 - 1) / 10;
        let lights = self.lights();
        let knights: Vec<(i32, i32)> = self
            .seats
            .values()
            .filter(|s| s.knight.up())
            .map(|s| s.knight.px())
            .collect();
        let mut wisps = std::mem::take(&mut self.world.wisps);
        let mut out_now: Vec<u8> = Vec::new();
        for w in &mut wisps {
            let here = (w.x / Q, w.y / Q);
            match w.mode {
                WispMode::Hunt => {
                    if lights.is_empty() {
                        continue;
                    }
                    // The nearest few, one each by the wisp's birth.
                    let mut order: Vec<_> = lights
                        .iter()
                        .map(|&(id, p)| ((p.0 - here.0).pow(2) + (p.1 - here.1).pow(2), id, p))
                        .collect();
                    order.sort();
                    let (_, id, target) = order[(w.born as usize) % order.len().min(3)];
                    w.aim = id;
                    let (dx, dy) = (target.0 * Q - w.x, target.1 * Q - w.y);
                    let len = isqrt(dx, dy);
                    if len <= 5 * Q {
                        let guarded = knights.iter().any(|&(x, y)| {
                            (x - target.0).pow(2) + (y - target.1).pow(2) <= WARD * WARD
                        });
                        w.mode = if guarded {
                            WispMode::Shy(90)
                        } else {
                            WispMode::Snuff(SNUFF)
                        };
                    } else {
                        w.x += (i64::from(dx) * i64::from(drift) / i64::from(len)) as i32;
                        w.y += (i64::from(dy) * i64::from(drift) / i64::from(len)) as i32;
                    }
                }
                WispMode::Snuff(t) => match lights.iter().find(|&&(id, _)| id == w.aim) {
                    None => w.mode = WispMode::Hunt,
                    Some(&(_, at)) => {
                        let guarded = knights
                            .iter()
                            .any(|&(x, y)| (x - at.0).pow(2) + (y - at.1).pow(2) <= WARD * WARD);
                        if guarded {
                            w.mode = WispMode::Shy(90);
                        } else if t <= 1 {
                            out_now.push(w.aim);
                            w.mode = WispMode::Hunt;
                        } else {
                            w.mode = WispMode::Snuff(t - 1);
                        }
                    }
                },
                WispMode::Shy(t) => {
                    // Away from the nearest knight, or from the light.
                    let from = knights
                        .iter()
                        .copied()
                        .min_by_key(|&(x, y)| (x - here.0).pow(2) + (y - here.1).pow(2))
                        .or_else(|| lights.iter().find(|&&(id, _)| id == w.aim).map(|&(_, p)| p))
                        .unwrap_or(here);
                    let (dx, dy) = (here.0 - from.0, here.1 - from.1);
                    let len = isqrt(dx, dy).max(1);
                    w.x += (i64::from(dx) * i64::from(drift * 3 / 2) / i64::from(len)) as i32;
                    w.y += (i64::from(dy) * i64::from(drift * 3 / 2) / i64::from(len)) as i32;
                    w.mode = if t <= 1 {
                        WispMode::Hunt
                    } else {
                        WispMode::Shy(t - 1)
                    };
                }
            }
        }
        // A touch costs a candle.
        let mut hurts: Vec<u32> = Vec::new();
        for w in &wisps {
            let here = (w.x / Q, w.y / Q);
            for (&id, s) in &self.seats {
                let k = &s.knight;
                let (x, y) = k.px();
                if k.up()
                    && k.hurt == 0
                    && k.roll == 0
                    && k.panel == Panel::Shut
                    && !hurts.contains(&id)
                    && (x - here.0).pow(2) + (y - here.1).pow(2) <= TOUCH * TOUCH
                {
                    hurts.push(id);
                }
            }
        }
        self.world.wisps = wisps;
        for aim in out_now {
            self.put_out(aim);
        }
        for id in hurts {
            let ground = self.world.ground(self.clock, self.nums.day, false);
            let crates = self.world.crates.clone();
            let me = self.seats[&id].knight.px();
            // Pushed away from the nearest wisp.
            let from = self
                .world
                .wisps
                .iter()
                .map(|w| (w.x / Q, w.y / Q))
                .min_by_key(|p| (p.0 - me.0).pow(2) + (p.1 - me.1).pow(2))
                .unwrap_or(me);
            let Some(s) = self.seats.get_mut(&id) else {
                continue;
            };
            let k = &mut s.knight;
            k.candles = k.candles.saturating_sub(1);
            k.hurt = WARDED;
            k.say = None;
            let (dx, dy) = ((me.0 - from.0).signum(), (me.1 - from.1).signum());
            for step in (1..=3).rev() {
                let (nx, ny) = (k.x + dx * 4 * Q * step, k.y + dy * 4 * Q * step);
                if town::clear(nx, ny, ground, &crates) {
                    k.x = nx;
                    k.y = ny;
                    break;
                }
            }
            if k.candles == 0 {
                k.down = DOWN;
                if k.held == Item::Ink {
                    self.world.flags &= !INK_TAKEN;
                }
                k.held = Item::Empty;
                k.panel = Panel::Shut;
                k.note = Some((Note::Snuffed, 90));
                cues.push("knight_fall");
            } else {
                k.note = Some((Note::Candle(k.candles), 90));
                cues.push("hero_hurt");
            }
        }
        cues
    }
}

/// What one tick did beside the session, as the Run used to settle it after the native step.
#[derive(Default, Debug, PartialEq)]
pub struct Ticked {
    pub cues: Vec<&'static str>,
    pub leaving: Vec<u32>,
    pub won: Option<(u32, u16)>,
}

/// `Run::tick_moonfall` as it was, less the writes into the Run: the native step, the
/// first deliverance, and the knights who walk out after the moon has turned back.
pub fn tick(
    s: &mut Session,
    inputs: &BTreeMap<u32, Input>,
    heroes: &BTreeMap<u32, Hero>,
) -> Ticked {
    let was_free = s.completed.is_some();
    let cues = s.old_step(inputs, heroes);
    let won = if let (Some(_), false) = (s.completed, was_free) {
        let seats = s.seats.len().clamp(1, 6) as u32;
        let best = (1000u32.saturating_sub(150 * s.memory.loops)).clamp(100, 1000) as u16;
        Some((seats, best))
    } else {
        None
    };
    let freed = s.completed.is_some() && s.phase == Phase::Play;
    let leaving: Vec<u32> = if freed {
        inputs
            .iter()
            .filter(|(id, i)| i.interact && s.nearest_use(**id).is_none())
            .map(|(&id, _)| id)
            .collect()
    } else {
        Vec::new()
    };
    Ticked { cues, leaving, won }
}
