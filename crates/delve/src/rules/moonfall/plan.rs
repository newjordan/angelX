//! What a bot presses for a knight: the walk through the favours.
//!
//! A bot is a stand-in for a player who has read the Ledger: it knows only
//! what the party's Ledger holds (a request heard, a rider seen), so a bot
//! with an empty Ledger learns first and a bot with a full one plays the
//! loop straight. It is stateless: every tick it works out the table's
//! tasks from the session, gives each task to the knight best placed for it
//! (the same answer for every bot at the table), and does its own. When
//! nothing is left that this loop can still do, the table rings the bell.
use super::clock;
use super::folk::{self, Folk};
use super::ledger::{Fact, NORTH_KEY, OIL, SOUTH_SEAL, STOUT, SWIFT};
use super::state::*;
use super::story::{PASS_FRESH, PASS_HOURS};
use super::town::{self, Q, TILE};
use super::walk::key;
use super::Session;
use crate::rules::Input;

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
    s.seats.iter().filter(|(_, st)| st.knight.up()).map(|(&id, _)| id).collect()
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
        out.push(Task { name, hand, aim, act, opens, closes });
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
        add("scroll to Mira", Hand::Holds(Item::Scroll), Aim::Folk(Folk::Mira), Act::Tap, at(s, d, 20, 55), at(s, d, 22, 15));
    }
    add("reeds to Nessa", Hand::Holds(Item::Reeds), Aim::Folk(Folk::Nessa), Act::Tap, at(s, 1, 8, 30), at(s, 1, 10, 30));
    add("pass to Tovin", Hand::Holds(Item::Pass), Aim::Folk(Folk::Tovin), Act::Tap, at(s, 1, 17, 0), at(s, 1, 19, 40));
    add("shoe to Aster", Hand::Holds(Item::Shoe), Aim::Folk(Folk::Aster), Act::Tap, at(s, 1, 16, 35), at(s, 1, 23, 0));
    add("ink to Orin", Hand::Holds(Item::Ink), Aim::Folk(Folk::Orin), Act::Tap, at(s, 0, 7, 0), at(s, 0, 8, 25));
    // What there is to witness, at the hour it happens.
    if !m.knows(Fact::AsterComing) {
        add("hear the herald", Hand::Any, Aim::Tile(town::Spot::Podium.tile()), Act::Stand, at(s, 0, 17, 40), at(s, 0, 18, 10));
    }
    if !m.knows(Fact::AsterLame) {
        add("see the rider", Hand::Any, Aim::Tile(town::Spot::EastGate.tile()), Act::Stand, at(s, 1, 16, 20), at(s, 1, 23, 0));
    }
    if !m.knows(Fact::Verse) {
        add("hear the verse", Hand::Any, Aim::Tile(town::Spot::WellSide.tile()), Act::Stand, at(s, 2, 20, 45), at(s, 2, 22, 0));
    }
    // The seal at dawn, for a knight who wears both masks.
    if m.knows(Fact::MiraSeal) && m.has_mask(SWIFT) && m.has_mask(STOUT) && !m.has_keep(SOUTH_SEAL) {
        for d in 0..3 {
            add("seal from Mira", Hand::Any, Aim::Folk(Folk::Mira), Act::Tap, at(s, d, 6, 50), at(s, d, 9, 0));
        }
    }
    // The shoe on the iron cart day, for the rider.
    if m.knows(Fact::AsterLame) && s.world.shod.is_none() && !m.has_keep(OIL) && !held_by_any(s, Item::Shoe) {
        add("shoe from Nessa", Hand::Free, Aim::Folk(Folk::Nessa), Act::Tap, at(s, 1, 6, 35), at(s, 1, 7, 55));
    }
    // The pass, once it is written.
    if let Some(t) = s.world.ink_given {
        if !m.has_keep(NORTH_KEY) && !held_by_any(s, Item::Pass) {
            add("pass from Orin", Hand::Free, Aim::Folk(Folk::Orin), Act::Tap, t + PASS_HOURS * hour, t + (PASS_HOURS + PASS_FRESH) * hour);
        }
    }
    // The favours that earn the masks and the pass.
    if m.knows(Fact::OrinLost) && s.world.ink_given.is_none() && !s.world.has(INK_TAKEN) && !m.has_keep(NORTH_KEY) {
        add("ink pot", Hand::Free, Aim::Tile(INK_STAND), Act::Tap, at(s, 0, 6, 15), at(s, 0, 8, 0));
    }
    if m.knows(Fact::PellaScroll) && !m.has_mask(SWIFT) && !held_by_any(s, Item::Scroll) {
        add("scroll from Pella", Hand::Free, Aim::Folk(Folk::Pella), Act::Tap, at(s, 0, 7, 0), at(s, 0, 8, 30));
    }
    if m.knows(Fact::NessaCold) && !m.has_mask(STOUT) && !held_by_any(s, Item::Reeds) {
        add("reeds", Hand::Free, Aim::Tile(REED_STAND), Act::Tap, at(s, 0, 18, 0), at(s, 0, 22, 0));
    }
    // Ask each of them once, to hear what they want: in the order they
    // are met, the nearest first.
    let from = up_seats(s).first().map_or(town::Spot::Plaza.px(), |id| s.seats[id].knight.px());
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
        add("south door", Hand::Any, Aim::Tile(SOUTH_STAND), Act::South, at(s, 2, 8, 0), end);
    }
    if m.has_keep(NORTH_KEY) && !s.world.has(NORTH_OPEN) {
        add("north door", Hand::Any, Aim::Tile(NORTH_STAND), Act::Tap, at(s, 2, 17, 0), end);
    }
    // Spare hands: keep the lamps lit.
    // (Not in the hours before the three lights: everyone is wanted at a tower.)
    if clock::night(now, day) && (now + 2 * hour < fin || now >= fin) {
        for (n, &(tx, ty)) in town::LAMPS.iter().enumerate() {
            if s.lantern_scheduled(n as u8) && s.world.snuffed & (1 << n) != 0 {
                add("relight", Hand::Free, Aim::Tile((tx, ty + 1)), Act::Tap, 0, end);
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
            let Some(target) = aim_px(s, t.aim, t.opens) else { continue };
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
fn walk_to(s: &Session, k: &Knight, goal: (i32, i32), crate_at: Option<(i32, i32)>) -> (i8, i8) {
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
        Panel::Ledger(_) => return Input { swing: tap(k, key::LEDGER), ..none },
        Panel::Bell(line) => {
            return if line > 0 {
                Input { move_y: if tap(k, key::UP) { -1 } else { 0 }, ..none }
            } else {
                Input { interact: tap(k, key::USE), ..none }
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
                return Input { ult: tap(k, key::BELL), ..none };
            }
            return none;
        }
        return Input { ult: tap(k, key::BELL), ..none };
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
        return Input { interact: tap(k, key::USE), ..none };
    }
    // Hands full of something nobody wants: put it down.
    if k.held != Item::Empty && !tasks(s).iter().any(|t| t.hand == Hand::Holds(k.held)) {
        return Input { bomb: tap(k, key::DROP), ..none };
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

fn go(s: &Session, k: &Knight, goal: (i32, i32), crate_at: Option<(i32, i32)>, reach: i32) -> Input {
    let (px, py) = k.px();
    let target = (goal.0 * TILE + TILE / 2, goal.1 * TILE + TILE / 2);
    if (px - target.0).abs() + (py - target.1).abs() <= reach {
        return Input::default();
    }
    let (mx, my) = walk_to(s, k, goal, crate_at);
    Input { move_x: mx, move_y: my, ..Default::default() }
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
        Act::Tap => Input { interact: tap(k, key::USE), ..Default::default() },
        Act::Hold => Input { interact: true, ..Default::default() },
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
                    return Input { move_x: (centre - px).signum() as i8, ..Default::default() };
                }
                if k.mask == Mask::Stout {
                    return Input { move_y: 1, ..Default::default() };
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
    Input { interact: tap(k, key::USE), ..Default::default() }
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
