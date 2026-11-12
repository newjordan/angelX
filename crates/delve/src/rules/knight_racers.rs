//! Knight Racers: big authored courses shared by six seats, raced for a cup.
//! Walk to the race gate at kingdom tile (128, 22), east of Lanternward, and
//! press E. It uses the normal kingdom door: hold Esc to return to the gate.
//! Each seat uses W/Up to accelerate, A/D to steer, S/Down to brake, Space to
//! boost. E at the flag starts the next course of the cup.
//!
//! The Bend mirror in `delve/rules/games_c` still describes the retired 864
//! oval. The courses live in `knight_racers/course.rs` (the road, the land and
//! the set pieces), `courses.rs` (the authored maps), `race.rs` (the sim, the
//! rival pilots and the cup) and `pods.rs`; this file joins them to the Run.
use super::*;
use super::roundtable::Adventure;
use super::activity::Activity;

#[path = "knight_racers/course.rs"]
pub mod course;
#[path = "knight_racers/courses.rs"]
mod courses;
#[path = "knight_racers/author.rs"]
mod author;
#[path = "knight_racers/emberfall.rs"]
mod emberfall;
#[path = "knight_racers/glassway.rs"]
mod glassway;
#[path = "knight_racers/moatwalk.rs"]
mod moatwalk;
#[path = "knight_racers/pods.rs"]
pub mod pods;
#[path = "knight_racers/tune.rs"]
pub mod tune;
#[path = "knight_racers/career.rs"]
pub mod career;
#[path = "knight_racers/flow.rs"]
pub mod flow;
#[path = "knight_racers/race.rs"]
pub mod race;
#[cfg(test)]
#[path = "knight_racers/course_tests.rs"]
mod course_tests;
#[cfg(test)]
#[path = "knight_racers/report.rs"]
mod report;
#[cfg(test)]
#[path = "knight_racers/law_tests.rs"]
mod law_tests;

pub use course::{course, courses, cup, Biome, Course, CourseId, Cp, Fork, Frame, Gap, Hazard, HazardKind, Line, Piece, PieceKind, Sample, Shape, Slag, Strip, Stretch, Zone};
pub use pods::{pods, PodSpec};
pub use race::{Car, Drive, Session};
pub use career::Career;
pub use flow::{Class, Flow, Mode, Pad, Stage};
pub use tune::Tune;

pub const NAME: &str = "Knight Racer";
pub const ENTRY: (i32, i32) = (128, 22);
/// Laps of the retired proving circuit; each course now carries its own count.
pub const LAPS: u32 = 2;
const NO_NUMBERS: [super::roundtable::Number; 5] = [super::roundtable::Number::stub("racer_unused", "No tuning", "knight_racers::LAPS"); 5];
const _: () = assert!(race::SEATS == MAX_PLAYERS && race::HZ == HZ as f32);
#[cfg(test)]
const GRID: [(f32, f32); 6] = race::GRID;

// The old free functions: thin wrappers over course 0, the proving circuit, for
// the renderer until it moves to `course(id)`.
fn proving() -> &'static Course { course(CourseId::Proving) }
/// Centreline length of one lap of the proving circuit.
pub fn lap() -> f32 { proving().lap() }
pub fn slag() -> &'static [Slag] { proving().slag() }
pub fn strips() -> &'static [Strip] { proving().strips() }
/// Section starts, excluding the grid line (the start gate already marks it).
pub fn landmarks() -> Vec<f32> { proving().landmarks() }
pub fn stretch_at(s: f32) -> (Stretch, &'static str) { let c = proving(); (c.stretch(s), c.zone(s).1) }
/// Half-width of the racing surface. Narrows through the cavern and hairpins.
pub fn wall_at(s: f32) -> f32 { let (l, r) = proving().width(s); l.min(r) }
/// Road height along the centreline. Closes on itself so the lap seam is level.
pub fn height(s: f32) -> f32 { proving().height(s) }
pub fn grade(s: f32) -> f32 { proving().grade(s) }
/// Inside line of the next corner, in track-left metres. Straights stay central.
pub fn racing_line(s: f32) -> f32 { proving().racing_line(s) }
pub fn pose(s: f32) -> (f32, f32, f32) { proving().pose(s) }
pub fn curve(s: f32) -> f32 { proving().curve(s) }

fn drive_of(input: &Input) -> Drive {
    Drive { steer: input.move_x.clamp(-1, 1), gas: input.move_y < 0, brake: input.move_y > 0, boost: input.dash, interact: input.interact }
}
fn input_of(d: Drive) -> Input {
    Input { move_x: d.steer, move_y: if d.brake { 1 } else if d.gas { -1 } else { 0 }, dash: d.boost, ..Input::default() }
}

/// What an empty seat presses when it is a CPU rival on the proving circuit.
/// A live human seat must not come through here: idle input stays idle. On the
/// other courses use `Session::bot_drives`, which knows the course.
pub fn bot_drive(id: u32, car: &Car, ticks: u32) -> Input {
    input_of(race::bot_drive_on(proving(), id, car, ticks))
}

fn bot_input(run: &Run, id: u32) -> Input {
    let Some(race) = &run.knight_racers else { return Input::default(); };
    let Some(car) = race.cars.get(&id) else { return Input::default(); };
    if !car.cpu { return Input::default(); }
    race.bot_drives().get(&id).map_or_else(Input::default, |d| input_of(*d))
}

impl Session {
    pub fn new(players: &BTreeMap<u32, Hero>) -> Self {
        let mut s = Self::blank();
        s.join_ids(players.keys().copied());
        s
    }
    /// The same, on a chosen course (and so a chosen place in the cup).
    pub fn new_on(players: &BTreeMap<u32, Hero>, course: CourseId) -> Self {
        let mut s = Self::blank_on(course);
        s.join_ids(players.keys().copied());
        s
    }
    pub fn step(&mut self, inputs: &BTreeMap<u32, Input>) {
        let drives: BTreeMap<u32, Drive> = inputs.iter().map(|(id, i)| (*id, drive_of(i))).collect();
        self.step_drive(&drives);
    }
}

pub const ADVENTURE: Adventure = Adventure {
    slug: "knight_racers", name: NAME, entry: ENTRY, slot: usize::MAX,
    marks: &[], numbers: &NO_NUMBERS, showing: Run::knight_racers_showing,
    start: Run::start_knight_racers, valid: |r| r.knight_racers.as_ref().is_none_or(Session::valid),
    clear: |r| r.knight_racers = None,
    seated: |r| r.knight_racers.as_ref().map_or_else(Vec::new, |s| s.cars.keys().copied().collect()),
    bot: bot_input,
};
pub const ACTIVITY: Activity = Activity {
    name: NAME, on: Run::knight_racers_showing,
    ride: Some(|run, inputs| { run.tick_knight_racers(inputs); Some(BTreeMap::new()) }),
    tick: |_, _| {}, end: |run| run.knight_racers = None, ends_outside: true,
    settle: None, fields: &["knight_racers"],
};
impl Run {
    pub fn knight_racers_showing(&self) -> bool {
        self.knight_racers.is_some() && (self.on_stage() || self.crossroads_racing())
    }
    pub fn start_knight_racers(&mut self) {
        if !self.on_stage() { return; }
        self.forest.leave();
        self.knight_racers = Some(Session::at_gate(&self.players));
        self.found = Some((self.tick, 0, "Knight Racers: the cup, a single race, a time trial. W/S and A/D choose, E or Space picks, Q steps back. In a race WASD steers, Space boosts. Hold Esc to leave.".into()));
    }
    pub(super) fn tick_knight_racers(&mut self, inputs: &BTreeMap<u32, Input>) {
        let Some(s) = &mut self.knight_racers else { return; };
        let humans: Vec<(u32, String)> = self.players.iter().filter(|(id, _)| !self.exits.outside.contains_key(id)).map(|(id, h)| (*id, h.name.clone())).collect();
        let pads: BTreeMap<u32, Pad> = inputs.iter().map(|(id, i)| (*id, Pad::from_input(i))).collect();
        flow::tick(s, &humans, &pads);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boost_lockout_braking_wall_feedback_and_old_saves() {
        let old = r#"{"d":0,"y":0,"v":0,"vy":0,"boost":1,"finish":null}"#;
        let legacy: Car = serde_json::from_str(old).unwrap();
        assert!(!legacy.boosting && !legacy.boost_locked && legacy.impact == 0 && !legacy.cpu);
        let mut session = Session { cars: [(1, legacy)].into(), ticks: 90, ..Session::blank() };
        let boost = [(1, Input { move_y: -1, dash: true, ..Input::default() })].into();
        for _ in 0..101 { session.step(&boost); }
        assert!(session.cars[&1].boost_locked, "depletion must latch cooling");
        session.step(&boost); // last residual charge powered the depletion tick
        assert!(!session.cars[&1].boosting, "held Space must not flutter on recharge");
        for _ in 0..10 { session.step(&boost); assert!(!session.cars[&1].boosting); }
        session.cars.get_mut(&1).unwrap().boost = 0.5;
        session.step(&boost);
        assert!(session.cars[&1].boosting && !session.cars[&1].boost_locked);
        session.step(&[(1, Input { move_y: 1, dash: true, ..Input::default() })].into());
        assert!(!session.cars[&1].boosting, "brake always cancels thrust");
        let c = session.cars.get_mut(&1).unwrap(); c.y = 16.; c.vy = 13.;
        session.step(&[(1, Input { move_x: 1, ..Input::default() })].into());
        assert_eq!(session.cars[&1].impact, 12);
        assert!(session.valid());
        let wire = serde_json::to_string(&session).unwrap();
        assert!(serde_json::from_str::<Session>(&wire).unwrap().valid());
    }

    #[test]
    fn circuit_closes_and_is_larger_than_the_old_oval() {
        let n = 96;
        let mut max_span = 0f32;
        for i in 0..n {
            let (x, y, _) = pose(i as f32 / n as f32 * lap());
            for j in (i + 1)..n {
                let (u, v, _) = pose(j as f32 / n as f32 * lap());
                max_span = max_span.max((x - u).hypot(y - v));
            }
        }
        assert!(lap() > 1800., "lap {} is still the old oval", lap());
        assert!(max_span > 600., "course span {max_span} did not grow");
        assert!(wall_at(0.) > 11., "the start straight should be wide enough to pass");
        assert!(wall_at(900.) < wall_at(0.), "the cavern must pinch");
        let (x0, y0, _) = pose(0.);
        let (x1, y1, _) = pose(lap() - 0.05);
        assert!((x0 - x1).hypot(y0 - y1) < 1.5, "lap seam jumps");
    }

    #[test]
    fn one_lap_six_seats_and_reset() {
        let mut r = Run::new(7, 1, None);
        for id in 2..=6 { r.join_seat(id, "Racer"); }
        // Six seats flat out with no steering only finish on the old oval, whose walls
        // guide them round; the cup courses need a pilot (see course_tests.rs).
        let mut s = Session::new_on(&r.players, CourseId::Proving);
        assert_eq!(s.cars.len(), 6);
        assert_eq!(s.cars.keys().copied().collect::<Vec<_>>(), (1..=6).collect::<Vec<_>>());
        let inputs = (1..=6).map(|id| (id, Input { move_y: -1, ..Input::default() })).collect();
        for _ in 0..12000 { s.step(&inputs); if s.finished { break; } }
        let line = lap() * LAPS as f32;
        assert!(s.finished, "all six must finish the race: {:?}", s.cars.iter().map(|(id, c)| (id, c.d, c.finish)).collect::<Vec<_>>());
        assert!(s.cars.values().all(|c| c.finish.is_some() && (c.d - line).abs() < 0.05));
        s.step(&[(1, Input { interact: true, ..Input::default() })].into());
        assert!(!s.finished);
        assert_eq!(s.cars.len(), 6);
        assert!(s.cars.values().all(|c| c.finish.is_none()));
    }

    #[test]
    fn solo_grid_fields_rivals_that_leave_the_line() {
        let mut r = Run::roundtable_for_test("knight_racers").unwrap();
        for _ in 0..220 { r.step(&[(1, Input { move_y: -1, ..Input::default() })].into()); }
        let race = r.knight_racers.as_ref().unwrap();
        assert_eq!(race.cars.len(), 6, "a solo knight should not race an empty course");
        assert!(race.cars[&1].d > 0.);
        assert!(race.cars.values().filter(|c| c.cpu).count() >= 4);
        assert!(race.cars.values().filter(|c| c.cpu).all(|c| c.d > 0.), "rivals must take the start");
    }


    #[test]
    fn the_live_run_races_every_course_and_the_cup_starts_on_the_first() {
        let mut r = Run::roundtable_for_test("knight_racers").unwrap();
        assert_eq!(r.knight_racers.as_ref().unwrap().course, cup()[0], "the gate opens on the first course of the cup");
        for &id in courses() {
            let mut r = Run::roundtable_for_test("knight_racers").unwrap();
            let players = r.players.clone();
            r.knight_racers = Some(Session::new_on(&players, id));
            for _ in 0..700 { r.step(&[(1, Input { move_y: -1, ..Input::default() })].into()); }
            let race = r.knight_racers.as_ref().unwrap();
            assert_eq!(race.course, id);
            assert!(race.valid(), "{id:?}");
            assert!(race.cars.values().filter(|c| c.cpu).all(|c| c.d > 100.), "{id:?}: rivals must pull away on every course");
        }
        // Pods and the wire: a chosen pod survives the serialised race.
        let race = r.knight_racers.as_mut().unwrap();
        race.choose_pod(1, 5);
        let back: Session = serde_json::from_str(&serde_json::to_string(race).unwrap()).unwrap();
        assert_eq!(back.cars[&1].pod, 5);
    }


    /// `cargo test -p angel-delve --lib frame_time -- --ignored --nocapture`: the cost of one race
    /// frame at the seat's 384x224, on the proving circuit and on each cup course, and of the land
    /// under it. A measurement, not a gate.
    #[test]
    #[ignore]
    fn frame_time() {
        for &id in courses() {
            let mut r = Run::roundtable_for_test("knight_racers").unwrap();
            let players = r.players.clone();
            r.knight_racers = Some(Session::new_on(&players, id));
            for _ in 0..400 { r.step(&[(1, Input { move_y: -1, ..Input::default() })].into()); }
            let n = 30;
            let t = std::time::Instant::now();
            for _ in 0..n { r.step(&[(1, Input { move_y: -1, ..Input::default() })].into()); let _ = crate::render::overworld::arena::frame_for(&r, 384, 224, Some(1)); }
            let ms = t.elapsed().as_secs_f64() * 1000. / n as f64;
            let c = course(id);
            let t = std::time::Instant::now();
            let mut sum = 0.;
            for i in 0..20_000 { sum += c.ground(i as f32 * 0.37 % 900. - 100., i as f32 * 0.53 % 700. - 100.); }
            let g = t.elapsed().as_secs_f64() * 1000.;
            println!("{:?}: {ms:.1} ms a frame; 20000 ground() calls {g:.1} ms ({sum:.0})", id);
        }
    }

    #[test]
    fn the_pilots_never_turn_about_and_a_held_brake_does() {
        for &id in courses() {
            let mut race = Session::blank_on(id);
            race.join_ids(1..=6);
            for _ in 0..1800 { race.step_drive(&race.bot_drives_all()); }
            assert!(race.cars.values().all(|c| c.yaw == 0.), "a pilot turned about on {id:?}");
        }
        // A knight who holds the brake and a direction at a standstill turns the pod right round.
        let mut race = Session::blank_on(CourseId::Proving);
        race.join_ids(1..=1);
        race.ticks = 120;
        { let c = race.cars.get_mut(&1).unwrap(); c.d = 300.; c.v = 0.; c.y = 0.; }
        let turn: BTreeMap<u32, Drive> = [(1, Drive { steer: -1, brake: true, ..Drive::default() })].into();
        for _ in 0..30 { race.step_drive(&turn); }
        let yaw = race.cars[&1].yaw;
        assert!(yaw.abs() > 2.0, "a second's turn gets the nose well round: {yaw}");
        assert!(race.valid(), "a pod turned about is still a valid pod");
        // At speed the nose is brought back along the road, whatever was held.
        { let c = race.cars.get_mut(&1).unwrap(); c.yaw = 0.8; c.v = 30.; }
        let go: BTreeMap<u32, Drive> = [(1, Drive { gas: true, ..Drive::default() })].into();
        for _ in 0..60 { race.step_drive(&go); }
        assert!(race.cars[&1].yaw.abs() < 0.01, "a pod under way straightens: {}", race.cars[&1].yaw);
    }

    #[test]
    fn six_live_seats_drive_independently_and_have_own_views() {
        let mut r = Run::roundtable_for_test("knight_racers").unwrap();
        for id in 2..=6 { r.join_seat(id, &format!("Knight {id}")); }
        let inputs: BTreeMap<_, _> = [(1, Input { move_y: -1, ..Input::default() }),
            (6, Input { move_y: -1, move_x: 1, ..Input::default() })].into();
        for _ in 0..130 { r.step(&inputs); }
        let race = r.knight_racers.as_ref().unwrap();
        assert_eq!(race.cars.len(), 6);
        assert!(race.ticks > 90, "the live Run advances the race");
        assert!(race.cars[&1].d > GRID[0].0 && race.cars[&6].d > GRID[5].0);
        assert!(race.cars[&2].d < 0., "an idle seat must not drive another seat's car");
        assert!(!race.cars[&2].cpu);
        assert_ne!(race.cars[&1].y, race.cars[&6].y);
        let a = crate::render::overworld::arena::frame_for(&r, 384, 224, Some(1));
        let b = crate::render::overworld::arena::frame_for(&r, 384, 224, Some(6));
        assert_ne!(a.content_hash(), b.content_hash(), "each seat gets its own chase view");
        assert!(r.leave_activity(6));
        r.step(&inputs);
        assert_eq!(r.knight_racers.as_ref().unwrap().cars.len(), 5,
            "a knight at the gate is no longer a racer");
        assert!(r.knight_racers.as_ref().unwrap().cars.contains_key(&1));
    }

    #[test]
    fn kingdom_entry_exit_and_reentry() {
        use super::super::roundtable;
        assert_eq!(roundtable::ADVENTURES.len(), 5, "existing minigames stay registered");
        assert!(roundtable::ADVENTURES.iter().all(|old| roundtable::adventure(old.slug).is_some()));
        let a = roundtable::adventure("knight_racers").unwrap();
        let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
        r.walk_kingdom(Some(ENTRY));
        assert_eq!(r.room().tile(ENTRY.0, ENTRY.1), Tile::Floor, "gate tile must be walkable");
        // The sign does not hide another activity's door, shop or errand.
        let geo = map::geography(&r.kingdom.wishes);
        for a in roundtable::ADVENTURES.iter() {
            assert!((a.entry.0 - ENTRY.0).abs() + (a.entry.1 - ENTRY.1).abs() >= 6);
        }
        for site in &geo.sites {
            assert!((site.stand.0 - ENTRY.0).abs() + (site.stand.1 - ENTRY.1).abs() >= 6, "{}", site.name);
        }
        // An open tile is not enough if the gate is marooned behind a wall.
        let mut seen = std::collections::BTreeSet::from([geo.sites[0].stand]);
        let mut queue = std::collections::VecDeque::from([geo.sites[0].stand]);
        while let Some((x, y)) = queue.pop_front() {
            for p in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                if geo.room.tile(p.0, p.1) == Tile::Floor && seen.insert(p) {
                    queue.push_back(p);
                }
            }
        }
        assert!(seen.contains(&ENTRY), "race gate must be reachable from the Keep");
        let (x, y) = ((ENTRY.0 as f32 + 0.5) * TILE_UNITS, (ENTRY.1 as f32 + 0.5) * TILE_UNITS);
        r.kingdom.door_held = false;
        r.kingdom.door_latched.clear();
        let h = r.players.get_mut(&1).unwrap();
        h.x = x; h.y = y; h.stone = false;
        assert!(r.tick_roundtable_entry(&[(1, Input { interact: true, ..Input::default() })].into()));
        assert!(r.knight_racers_showing());
        assert_eq!(r.roundtable_under_way().unwrap().slug, a.slug);
        assert!(r.can_hold_leave(1));
        assert!(r.leave_activity(1));
        assert!(!r.knight_racers_showing());
        assert!(r.knight_racers.is_none());
        roundtable::enter(&mut r, a);
        assert!(r.knight_racers_showing());
        assert!(r.knight_racers.as_ref().unwrap().cars.contains_key(&1));
    }
}

#[cfg(all(test, feature = "bend-cases-games-c"))]
#[path = "../../tests/rules/knight_racers_bend_cases__tests.rs"]
mod knight_racers_bend_cases_tests;
