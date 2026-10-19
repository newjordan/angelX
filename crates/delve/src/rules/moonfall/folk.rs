//! The six folk of the fen town and where each is at any moment.
//!
//! Every one follows a program of stops through the three days: they wake,
//! work, eat, light the lamps, cry the hours and go home. Where anyone is,
//! is a pure function of the clock (and, for Aster, of the one thing a
//! knight can change), so the party can follow them, wait for them and
//! cut them off, and the loop's rewind runs them backward for free.
use super::clock;
use super::town::{self, Spot, length};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Folk {
    Mira,
    Tovin,
    Nessa,
    Orin,
    Pella,
    Aster,
}

pub const CAST: [Folk; 6] = [
    Folk::Mira,
    Folk::Tovin,
    Folk::Nessa,
    Folk::Orin,
    Folk::Pella,
    Folk::Aster,
];

impl Folk {
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn from_index(i: usize) -> Option<Folk> {
        CAST.get(i).copied()
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Mira => "MIRA QUILL",
            Self::Tovin => "TOVIN REED",
            Self::Nessa => "NESSA FLINT",
            Self::Orin => "ORIN WREN",
            Self::Pella => "PELLA STARLING",
            Self::Aster => "ASTER VALE",
        }
    }
    pub fn short(self) -> &'static str {
        match self {
            Self::Mira => "MIRA",
            Self::Tovin => "TOVIN",
            Self::Nessa => "NESSA",
            Self::Orin => "ORIN",
            Self::Pella => "PELLA",
            Self::Aster => "ASTER",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Mira => "LAMPLIGHTER AND LEDGER KEEPER",
            Self::Tovin => "KEEPER OF THE NORTH BRIDGE",
            Self::Nessa => "BAKER AND SMITH",
            Self::Orin => "SCRIBE WHO LOST HIS INK",
            Self::Pella => "HERALD OF THE HOURS",
            Self::Aster => "LANTERN RIDER",
        }
    }
    /// The ink of their block and their portrait.
    pub fn ink(self) -> char {
        match self {
            Self::Mira => 'V',
            Self::Tovin => 'o',
            Self::Nessa => 'R',
            Self::Orin => 'z',
            Self::Pella => 'Y',
            Self::Aster => '6',
        }
    }
}

/// One thing a villager does at one place: from half-hour `at` after the
/// first dawn they are at `spot`.
#[derive(Clone, Copy, Debug)]
pub struct Stop {
    pub at: u32,
    pub spot: Spot,
    /// Behind the door: not seen once they have arrived.
    pub inside: bool,
    /// Hurrying (the way home when the Gloam draws near).
    pub fast: bool,
    /// The routine line it writes in the Ledger, or `NONE`.
    pub routine: u8,
    /// The street lantern they light on arriving.
    pub lamp: Option<u8>,
}

pub const NONE: u8 = 255;

/// Half-hours since dawn of the first day for `hour:half` of day 1 (hours
/// past midnight count on: 24 is midnight, 30 the next dawn).
const fn h(hour: u32, half: u32) -> u32 {
    (hour - 6) * 2 + half
}

struct Entry {
    at: u32,
    spot: Spot,
    inside: bool,
    lamp: Option<u8>,
    note: &'static str,
}

const fn e(at: u32, spot: Spot, inside: bool, note: &'static str) -> Entry {
    Entry { at, spot, inside, lamp: None, note }
}

const fn lamp(at: u32, spot: Spot, n: u8, note: &'static str) -> Entry {
    Entry { at, spot, inside: false, lamp: Some(n), note }
}

/// A day of one villager's routine, with the line the Ledger keeps for each
/// stop (empty: nothing is written).
fn day_of(folk: Folk) -> Vec<Entry> {
    match folk {
        Folk::Mira => vec![
            e(h(6, 0), Spot::ArchDoor, true, "ASLEEP AT THE ARCHIVE"),
            e(h(6, 1), Spot::ArchPorch, false, "READS ON THE ARCHIVE PORCH"),
            e(h(12, 1), Spot::BakeryAt, false, "LUNCH AT THE BAKERY"),
            e(h(14, 0), Spot::ArchPorch, false, "BACK AT HER DESK"),
            lamp(h(17, 0), Spot::Lamp1, 0, "LIGHTS THE LAMPS, WEST TO EAST"),
            lamp(h(17, 1), Spot::Lamp2, 1, ""),
            lamp(h(18, 0), Spot::Lamp3, 2, ""),
            lamp(h(18, 1), Spot::Lamp4, 3, ""),
            lamp(h(19, 0), Spot::Lamp5, 4, ""),
            lamp(h(19, 1), Spot::Lamp6, 5, ""),
            e(h(21, 0), Spot::WellSide, false, "KEEPS THE WELL WATCH"),
            e(h(22, 1), Spot::ArchDoor, true, "ASLEEP AT THE ARCHIVE"),
        ],
        Folk::Pella => vec![
            e(h(6, 0), Spot::HeraldDoor, true, "ASLEEP AT THE HERALD HOUSE"),
            e(h(7, 0), Spot::HeraldAt, false, "OPENS THE HERALD HOUSE"),
            e(h(12, 0), Spot::Podium, false, "CRIES NOON AT THE PODIUM"),
            e(h(13, 1), Spot::HeraldAt, false, "BACK AT THE HERALD HOUSE"),
            e(h(18, 0), Spot::Podium, false, "CRIES DUSK AT THE PODIUM"),
            e(h(19, 1), Spot::HeraldAt, false, ""),
            e(h(20, 1), Spot::HeraldDoor, true, "ASLEEP AT THE HERALD HOUSE"),
        ],
        Folk::Nessa => vec![
            e(h(6, 0), Spot::BakeryDoor, true, "ASLEEP ABOVE THE BAKERY"),
            e(h(6, 1), Spot::BakeryAt, false, "BAKES AT THE COUNTER"),
            e(h(12, 0), Spot::ForgeAt, false, "WORKS THE FORGE"),
            e(h(16, 1), Spot::BakeryAt, false, "BANKS THE OVEN"),
            e(h(18, 1), Spot::BakeryDoor, true, "ASLEEP ABOVE THE BAKERY"),
        ],
        Folk::Orin => vec![
            e(h(6, 0), Spot::ScriptDoor, true, "ASLEEP AT THE SCRIPTORIUM"),
            e(h(7, 0), Spot::ReedBank, false, "SEARCHES THE REEDS"),
            e(h(8, 1), Spot::ScriptDesk, false, "WRITES AT HIS DESK"),
            e(h(17, 1), Spot::BakeryAt, false, "SUPPER AT THE BAKERY"),
            e(h(20, 1), Spot::ScriptDoor, true, "ASLEEP AT THE SCRIPTORIUM"),
        ],
        Folk::Tovin => vec![
            e(h(6, 0), Spot::HutDoor, true, "ASLEEP IN THE BRIDGE HUT"),
            e(h(6, 1), Spot::BridgePost, false, "LOWERS THE BRIDGE AT SIX"),
            e(h(12, 0), Spot::HutDoor, true, "NOON MEAL IN THE HUT"),
            e(h(13, 0), Spot::BridgePost, false, "WATCHES THE BRIDGE"),
            e(h(20, 0), Spot::HutDoor, true, "RAISES THE BRIDGE AT EIGHT"),
        ],
        // Aster rides in on the second day only (`program`).
        Folk::Aster => vec![],
    }
}

pub fn routine_text(folk: Folk, id: u8) -> &'static str {
    let day = day_of(folk);
    day.get(usize::from(id)).map_or("", |s| s.note)
}

/// A villager's stop is "worth a line" in the Ledger when it names one.
pub fn routine_time(folk: Folk, id: u8) -> Option<u32> {
    day_of(folk).get(usize::from(id)).map(|s| s.at % 48)
}

/// A whole program: the day's routine three times over, with the second
/// day's iron cart for the forge, and Aster's one evening.
pub fn program(folk: Folk) -> Vec<Stop> {
    let mut out = Vec::new();
    for d in 0..clock::DAYS {
        let mut day = day_of(folk);
        if folk == Folk::Nessa && d == 1 {
            // The iron cart comes at dawn on the second day: the forge from
            // half past six, all day.
            day[1].spot = Spot::ForgeAt;
            day[1].note = "MEETS THE IRON CART AT THE FORGE";
        }
        for (i, entry) in day.iter().enumerate() {
            out.push(Stop {
                at: entry.at + d * 48,
                spot: entry.spot,
                inside: entry.inside,
                // The third evening everyone hurries home before the
                // Third Midnight.
                fast: d == 2 && entry.inside && entry.at >= h(19, 0),
                routine: if entry.note.is_empty() { NONE } else { i as u8 },
                lamp: entry.lamp,
            });
        }
    }
    if folk == Folk::Aster {
        let on = |at, spot, routine| Stop { at, spot, inside: false, fast: false, routine, lamp: None };
        out = vec![
            on(0, Spot::EastOut, NONE),
            on(h(16, 1) + 48, Spot::EastGate, 0),
            on(h(23, 1) + 48, Spot::EastOut, NONE),
        ];
    }
    out.sort_by_key(|s| s.at);
    out
}

pub fn aster_text(id: u8) -> &'static str {
    match id {
        0 => "RIDES IN AT THE EAST GATE, HIS HORSE LAME",
        _ => "",
    }
}

/// One leg of a villager's day: when they set out, when they arrive.
#[derive(Clone, Debug)]
pub struct Leg {
    pub depart: u32,
    pub arrive: u32,
    pub route: Arc<Vec<(i32, i32)>>,
    pub spot: Spot,
    pub inside: bool,
    pub routine: u8,
    pub lamp: Option<u8>,
    /// The tick the stop is nominally for (they should be there by then).
    pub at: u32,
    /// Q-pixels a tick on this leg.
    pub pace: i32,
}

/// Folk walk 1.5 pixels a tick; a hurrying one 2.4.
pub const PACE_Q: i32 = 384;
pub const HURRY_Q: i32 = 614;

/// Every villager's legs for a day of `day` ticks.
pub struct Timetable {
    pub legs: [Vec<Leg>; 6],
    /// When each villager can be seen, as ticks `[from, to)`, in order.
    pub seen: [Vec<(u32, u32)>; 6],
}

fn build(day: u32) -> Timetable {
    let legs = CAST.map(|folk| {
        let stops = program(folk);
        let mut out: Vec<Leg> = Vec::new();
        for (i, stop) in stops.iter().enumerate() {
            let at = clock::ticks(day, stop.at);
            if i == 0 {
                let px = stop.spot.px();
                out.push(Leg {
                    depart: 0,
                    arrive: 0,
                    route: Arc::new(vec![px]),
                    spot: stop.spot,
                    inside: stop.inside,
                    routine: stop.routine,
                    lamp: stop.lamp,
                    at: 0,
                    pace: PACE_Q,
                });
                continue;
            }
            let from = stops[i - 1].spot;
            let route = town::route(from, stop.spot);
            let pace = if stop.fast { HURRY_Q } else { PACE_Q };
            let travel = (i64::from(length(&route)) * i64::from(town::Q) / i64::from(pace)) as u32;
            let free = out.last().map_or(0, |l| l.arrive);
            let depart = at.saturating_sub(travel).max(free);
            out.push(Leg {
                depart,
                arrive: depart + travel,
                route,
                spot: stop.spot,
                inside: stop.inside,
                routine: stop.routine,
                lamp: stop.lamp,
                at,
                pace,
            });
        }
        out
    });
    let end = clock::end(day);
    let seen = CAST.map(|folk| {
        let l = &legs[folk.index()];
        let mut out: Vec<(u32, u32)> = Vec::new();
        let mut add = |a: u32, b: u32| {
            if b > a {
                match out.last_mut() {
                    Some(last) if last.1 >= a => last.1 = last.1.max(b),
                    _ => out.push((a, b)),
                }
            }
        };
        if folk == Folk::Aster {
            add(l[1].depart, l[2].arrive);
        } else {
            for (i, leg) in l.iter().enumerate() {
                let next = l.get(i + 1).map_or(end, |n| n.depart);
                if i > 0 {
                    add(leg.depart, leg.arrive);
                }
                if !leg.inside {
                    add(leg.arrive, next);
                }
            }
        }
        out
    });
    Timetable { legs, seen }
}

/// The next tick at or after `clock` that `folk` can be seen (the schedule
/// only: Aster after a shoe keeps his own).
pub fn next_seen(folk: Folk, clock: u32, day: u32) -> Option<u32> {
    let table = timetable(day);
    table.seen[folk.index()]
        .iter()
        .find(|&&(_, b)| b > clock)
        .map(|&(a, _)| a.max(clock))
}

/// The timetable for a day length (cached: a pure function of it).
pub fn timetable(day: u32) -> Arc<Timetable> {
    static CACHE: OnceLock<Mutex<BTreeMap<u32, Arc<Timetable>>>> = OnceLock::new();
    if let Some(t) = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner()).get(&day) {
        return t.clone();
    }
    let made = Arc::new(build(day));
    CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(day, made.clone());
    made
}

/// A villager, as the knights can see them this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Figure {
    pub folk: Folk,
    /// Pixels.
    pub x: i32,
    pub y: i32,
    pub visible: bool,
    pub moving: bool,
    /// 0 south, 1 north, 2 west, 3 east.
    pub face: u8,
    /// The leg they are on, and the routine line it earns once arrived.
    pub leg: usize,
    pub arrived: bool,
    pub routine: u8,
}

/// Where along `route` the walker is after `travelled` Q-pixels.
fn along(route: &[(i32, i32)], travelled: i32) -> ((i32, i32), u8) {
    let mut left = travelled / town::Q;
    let mut face = 0;
    for w in route.windows(2) {
        let (dx, dy) = (w[1].0 - w[0].0, w[1].1 - w[0].1);
        face = match (dx.signum(), dy.signum()) {
            (1, _) => 3,
            (-1, _) => 2,
            (_, 1) => 0,
            _ => 1,
        };
        let len = dx.abs() + dy.abs();
        if left <= len {
            return ((w[0].0 + dx.signum() * left, w[0].1 + dy.signum() * left), face);
        }
        left -= len;
    }
    (*route.last().unwrap_or(&(0, 0)), face)
}

/// Where `folk` is at `clock`. Aster, once shod at `shod`, rides to the
/// well and then home along a program of his own (`aster_after_shoe`).
pub fn figure(folk: Folk, clock: u32, day: u32, shod: Option<u32>) -> Figure {
    let table = timetable(day);
    let legs = &table.legs[folk.index()];
    if folk == Folk::Aster {
        if let Some(t) = shod.filter(|&t| clock >= t) {
            return aster_after_shoe(clock, day, t);
        }
    }
    let i = legs.partition_point(|l| l.depart <= clock).saturating_sub(1);
    let leg = &legs[i];
    let moving = clock < leg.arrive;
    let (pos, face) = if moving {
        along(&leg.route, ((clock - leg.depart) as i32).saturating_mul(leg.pace))
    } else {
        (leg.spot.px(), if leg.spot == Spot::EastGate { 3 } else { 0 })
    };
    // Aster is off the road until he rides in, and off it again once gone.
    let on_stage = folk != Folk::Aster || (legs[1].depart <= clock && clock < legs[2].arrive);
    Figure {
        folk,
        x: pos.0,
        y: pos.1,
        visible: on_stage && (moving || !leg.inside),
        moving,
        face,
        leg: i,
        arrived: !moving && on_stage,
        routine: if moving { NONE } else { leg.routine },
    }
}

/// Aster after his horse is shod at `t`: from the gate to the well's east
/// side, there until 22:00, then out of the east gate again.
fn aster_after_shoe(clock: u32, day: u32, t: u32) -> Figure {
    let there = town::route(Spot::EastGate, Spot::WellEast);
    let travel = (i64::from(length(&there)) * i64::from(town::Q) / i64::from(PACE_Q)) as u32;
    let leave = clock::ticks(day, h(22, 0) + 48).max(t + travel + 1);
    let back = town::route(Spot::WellEast, Spot::EastOut);
    let back_travel = (i64::from(length(&back)) * i64::from(town::Q) / i64::from(PACE_Q)) as u32;
    let (pos, face, moving, arrived) = if clock < t + travel {
        let (p, f) = along(&there, ((clock - t) as i32).saturating_mul(PACE_Q));
        (p, f, true, false)
    } else if clock < leave {
        (Spot::WellEast.px(), 2, false, true)
    } else if clock < leave + back_travel {
        let (p, f) = along(&back, ((clock - leave) as i32).saturating_mul(PACE_Q));
        (p, f, true, false)
    } else {
        (Spot::EastOut.px(), 3, false, false)
    };
    Figure {
        folk: Folk::Aster,
        x: pos.0,
        y: pos.1,
        visible: clock < leave + back_travel,
        moving,
        face,
        leg: 0,
        arrived,
        routine: NONE,
    }
}

/// The tick Aster stands at the well once he has been shod at `shod`.
pub fn aster_at_well(shod: u32) -> u32 {
    let there = town::route(Spot::EastGate, Spot::WellEast);
    shod + (i64::from(length(&there)) * i64::from(town::Q) / i64::from(PACE_Q)) as u32
}

/// Every villager now.
pub fn everyone(clock: u32, day: u32, shod: Option<u32>) -> Vec<Figure> {
    CAST.iter().map(|&f| figure(f, clock, day, shod)).collect()
}

/// How far into each day's dawn-to-dawn cycle lantern `n` is lit: the tick
/// the lamplighter reaches it (the same every day).
pub fn lamp_lit_at(n: u8, day: u32) -> Option<u32> {
    let table = timetable(day);
    table.legs[Folk::Mira.index()]
        .iter()
        .find(|l| l.lamp == Some(n))
        .map(|l| l.arrive % day)
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: u32 = 3000;

    fn spot_of(folk: Folk, clock: u32) -> (i32, i32) {
        let f = figure(folk, clock, D, None);
        (f.x, f.y)
    }

    #[test]
    fn every_villager_reaches_each_stop_by_its_hour_and_none_is_late() {
        for folk in CAST {
            let table = timetable(D);
            for l in &table.legs[folk.index()] {
                assert!(
                    l.arrive <= l.at + 2,
                    "{folk:?} reaches {:?} at {} for {}",
                    l.spot,
                    l.arrive,
                    l.at
                );
            }
        }
    }

    #[test]
    fn folk_keep_to_the_open_ground_and_never_stand_in_water() {
        for folk in CAST {
            for clock in (0..clock::end(D)).step_by(7) {
                let f = figure(folk, clock, D, None);
                if !f.visible {
                    continue;
                }
                let t = town::tile(f.x.div_euclid(town::TILE), f.y.div_euclid(town::TILE));
                let ground = town::Ground { bridge_up: clock::bridge_up(clock, D), swift: false, ditch_filled: false };
                assert!(
                    ground.open(t) || t == town::Tile::Gate,
                    "{folk:?} at tick {clock} on {t:?} {}",
                    clock::stamp(clock, D).text()
                );
            }
        }
    }

    #[test]
    fn mira_reads_by_day_lights_the_lamps_at_dusk_and_sleeps_at_night() {
        let porch = Spot::ArchPorch.px();
        let noon = clock::ticks(D, h(10, 0));
        assert_eq!(spot_of(Folk::Mira, noon), porch);
        let first = clock::ticks(D, h(17, 0));
        assert_eq!(spot_of(Folk::Mira, first), Spot::Lamp1.px());
        assert!(lamp_lit_at(5, D).unwrap() > lamp_lit_at(0, D).unwrap());
        let night = figure(Folk::Mira, clock::ticks(D, h(23, 0)), D, None);
        assert!(!night.visible, "asleep indoors");
    }

    #[test]
    fn the_baker_forges_all_day_on_the_iron_cart_day_and_not_on_the_others() {
        let ten = h(10, 0);
        let day1 = clock::ticks(D, ten);
        let day2 = clock::ticks(D, ten + 48);
        assert_eq!(spot_of(Folk::Nessa, day1), Spot::BakeryAt.px());
        assert_eq!(spot_of(Folk::Nessa, day2), Spot::ForgeAt.px());
        let dawn2 = clock::ticks(D, h(7, 0) + 48);
        assert_eq!(spot_of(Folk::Nessa, dawn2), Spot::ForgeAt.px(), "the cart comes at dawn");
    }

    #[test]
    fn aster_rides_in_on_the_second_evening_and_is_not_about_before() {
        assert!(!figure(Folk::Aster, clock::ticks(D, h(12, 0)), D, None).visible);
        let eve = figure(Folk::Aster, clock::ticks(D, h(17, 1) + 48), D, None);
        assert!(eve.visible && eve.arrived);
        assert_eq!((eve.x, eve.y), Spot::EastGate.px());
        // Shod, he is at the well some while later.
        let shod = clock::ticks(D, h(17, 0) + 48);
        let well = figure(Folk::Aster, aster_at_well(shod) + 5, D, Some(shod));
        assert_eq!((well.x, well.y), Spot::WellEast.px());
    }

    #[test]
    fn folk_walk_a_continuous_route_without_jumping() {
        for folk in CAST {
            let mut last = figure(folk, 0, D, None);
            for clock in 1..clock::end(D) {
                let now = figure(folk, clock, D, None);
                if last.visible && now.visible {
                    let jump = (now.x - last.x).abs() + (now.y - last.y).abs();
                    assert!(jump <= 4, "{folk:?} jumps {jump} px at {clock}");
                }
                last = now;
            }
        }
    }
}

#[cfg(test)]
mod table {
    use super::*;

    /// `cargo test -p angel-delve print_the_schedules -- --ignored --nocapture`:
    /// the legs of every day, for the brief.
    #[test]
    #[ignore = "prints the schedules"]
    fn print_the_schedules() {
        let table = timetable(3000);
        for folk in CAST {
            eprintln!("{}", folk.name());
            for leg in &table.legs[folk.index()] {
                let day = leg.at / 3000 + 1;
                let st = clock::stamp(leg.at, 3000);
                let arr = clock::stamp(leg.arrive, 3000);
                let dep = clock::stamp(leg.depart, 3000);
                eprintln!(
                    "  D{day} {} at {:?}{}  (leaves {} arrives {})",
                    st.text(),
                    leg.spot,
                    if leg.inside { " inside" } else { "" },
                    dep.text(),
                    arr.text()
                );
            }
        }
    }
}
