//! Knight Racer: one lap on the example's 864-unit oval, shared by six seats.
//! Walk to the race gate at kingdom tile (128, 22), east of Lanternward, and
//! press E. It uses the normal kingdom door: hold Esc to return to the gate.
//! Each browser/native seat uses W/Up to accelerate, A/D to steer, S/Down
//! to brake, Space to boost. E at the flag resets the whole field.
use super::*;
use super::roundtable::Adventure;
use super::activity::Activity;
use serde::{Deserialize, Serialize};

pub const NAME: &str = "Knight Racer";
pub const ENTRY: (i32, i32) = (128, 22);
pub const LAP: f32 = 864.0;
pub const LAPS: u32 = 1;
const NO_NUMBERS: [super::roundtable::Number; 5] = [super::roundtable::Number::stub("racer_unused", "No tuning", "knight_racers::LAPS"); 5];
const BEND: f32 = 50.0;
const STRAIGHT: f32 = (LAP - 2.0 * std::f32::consts::PI * BEND) / 2.0;
const GRID: [(f32, f32); 6] = [(-6., -3.6), (-10., 3.6), (-22., -3.6), (-26., 3.6), (-38., -3.6), (-42., 3.6)];

/// The same bend and slide as the example's car drive, at Delve's fixed 30 Hz.
pub fn pose(s: f32) -> (f32, f32, f32) {
    let s = s.rem_euclid(LAP);
    let arc = std::f32::consts::PI * BEND;
    if s < STRAIGHT { (s, 0., 0.) }
    else if s < STRAIGHT + arc { let f = (s - STRAIGHT) / BEND; (STRAIGHT + BEND * f.sin(), BEND * (1. - f.cos()), f) }
    else if s < 2. * STRAIGHT + arc { (STRAIGHT - (s - STRAIGHT - arc), 2. * BEND, std::f32::consts::PI) }
    else { let f = (s - 2. * STRAIGHT - arc) / BEND; (-BEND * f.sin(), BEND * (1. + f.cos()), std::f32::consts::PI + f) }
}
fn curve(s: f32) -> f32 {
    let s = s.rem_euclid(LAP);
    if (STRAIGHT..STRAIGHT + std::f32::consts::PI * BEND).contains(&s) || s >= 2. * STRAIGHT + std::f32::consts::PI * BEND { 1. / BEND } else { 0. }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Car {
    pub d: f32, pub y: f32, pub v: f32, pub vy: f32, pub boost: f32,
    pub finish: Option<f32>,
}
impl Car {
    fn grid(slot: usize) -> Self { Self { d: GRID[slot].0, y: GRID[slot].1, v: 0., vy: 0., boost: 1., finish: None } }
    fn valid(&self) -> bool { [self.d, self.y, self.v, self.vy, self.boost].iter().all(|n| n.is_finite()) && self.d >= -100. && self.d <= LAP + 100. && self.y.abs() <= 12. && (0. ..=60.).contains(&self.v) && (0. ..=1.).contains(&self.boost) && self.finish.is_none_or(|t| t.is_finite() && t >= 0.) }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub cars: BTreeMap<u32, Car>,
    pub ticks: u32,
    /// After the flag, E starts a fresh grid without leaving the room.
    pub finished: bool,
}
impl Session {
    pub fn new(players: &BTreeMap<u32, Hero>) -> Self {
        let mut s = Self { cars: BTreeMap::new(), ticks: 0, finished: false };
        s.join_ids(players.keys().copied());
        s
    }
    /// Keep only knights still inside the race. Waiting outside at the
    /// kingdom gate must not remain a ghost racer in the finish condition.
    pub fn join_ids(&mut self, ids: impl IntoIterator<Item = u32>) {
        let ids: std::collections::BTreeSet<_> = ids.into_iter().filter(|id| (1..=MAX_PLAYERS).contains(id)).collect();
        self.cars.retain(|id, _| ids.contains(id));
        for id in ids.into_iter().take(MAX_PLAYERS as usize) {
            self.cars.entry(id).or_insert_with(|| Car::grid((id - 1) as usize));
        }
        self.finished = !self.cars.is_empty() && self.cars.values().all(|c| c.finish.is_some());
    }
    pub fn valid(&self) -> bool { self.cars.len() <= MAX_PLAYERS as usize && self.cars.iter().all(|(id, car)| (1..=MAX_PLAYERS).contains(id) && car.valid()) && (!self.finished || self.cars.values().all(|c| c.finish.is_some())) }
    pub fn step(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.finished {
            if self.cars.keys().any(|id| inputs.get(id).is_some_and(|i| i.interact)) {
                for (id, car) in &mut self.cars { *car = Car::grid((*id - 1) as usize); }
                self.ticks = 0; self.finished = false;
            }
            return;
        }
        self.ticks = self.ticks.saturating_add(1);
        // A short grid countdown, shared by all seats.
        if self.ticks <= 90 { return; }
        let dt = 1. / HZ as f32;
        let time = (self.ticks - 90) as f32 * dt;
        for (id, c) in &mut self.cars {
            if c.finish.is_some() { continue; }
            let input = inputs.get(id).copied().unwrap_or_default();
            let steer = f32::from(input.move_x.clamp(-1, 1));
            let brake = input.move_y > 0;
            let boosting = input.dash && c.boost > 0. && !brake;
            let gas = input.move_y < 0 || boosting;
            if boosting { c.boost = (c.boost - 0.30 * dt).max(0.); }
            else { c.boost = (c.boost + 0.11 * dt).min(1.); }
            let top = if boosting { 56. } else { 40. };
            if brake { c.v = (c.v - 46. * dt).max(0.); }
            else if gas { if c.v < top { c.v += (if boosting { 62.4 } else { 26. }) * (1. - c.v / top) * dt; } }
            else { c.v = (c.v - (3. + 0.004 * c.v * c.v) * dt).max(0.); }
            if c.v > top { c.v = (c.v - 16. * dt).max(top); }
            let lat = 13. * (0.4 + 0.6 * (c.v / 16.).min(1.)) * if boosting { 0.8 } else { 1. };
            let cmd = steer * lat;
            c.vy += (cmd - c.vy).clamp(-90. * dt, 90. * dt);
            let k = curve(c.d);
            c.y += (c.vy - 0.30 * c.v * c.v * k) * dt;
            // Wall catches the engine, but does not trap it at a dead stop.
            if c.y.abs() > 8. { c.y = c.y.signum() * 8.; c.v *= 0.88; c.vy = -c.y.signum() * 4.; }
            c.d += c.v * dt / (1. - k * c.y).max(0.4);
            if c.d >= LAP * LAPS as f32 { c.finish = Some(time - (c.d - LAP) / c.v.max(1.)); c.d = LAP; }
        }
        self.finished = !self.cars.is_empty() && self.cars.values().all(|c| c.finish.is_some());
    }
}

pub const ADVENTURE: Adventure = Adventure {
    slug: "knight_racers", name: NAME, entry: ENTRY, slot: usize::MAX,
    marks: &[], numbers: &NO_NUMBERS, showing: Run::knight_racers_showing,
    start: Run::start_knight_racers, valid: |r| r.knight_racers.as_ref().is_none_or(Session::valid),
    clear: |r| r.knight_racers = None,
    seated: |r| r.knight_racers.as_ref().map_or_else(Vec::new, |s| s.cars.keys().copied().collect()),
    bot: |_, _| Input::default(),
};
pub const ACTIVITY: Activity = Activity {
    name: NAME, on: Run::knight_racers_showing,
    ride: Some(|run, inputs| { run.tick_knight_racers(inputs); Some(BTreeMap::new()) }),
    tick: |_, _| {}, end: |run| run.knight_racers = None, ends_outside: true,
    settle: None, fields: &["knight_racers"],
};
impl Run {
    pub fn knight_racers_showing(&self) -> bool { self.room().kind == RoomKind::Forest && self.knight_racers.is_some() }
    pub fn start_knight_racers(&mut self) {
        if self.room().kind != RoomKind::Forest { return; }
        self.forest.leave();
        self.knight_racers = Some(Session::new(&self.players));
        self.found = Some((self.tick, 0, "Knight Racer: arrows/WASD drive, Space boosts. Hold Esc to leave; E after the flag to race again.".into()));
    }
    fn tick_knight_racers(&mut self, inputs: &BTreeMap<u32, Input>) {
        let Some(s) = &mut self.knight_racers else { return; };
        s.join_ids(self.players.keys().filter(|id| !self.exits.outside.contains_key(id)).copied());
        s.step(inputs);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_lap_six_seats_and_reset() {
        let mut r = Run::new(7, 1, None);
        for id in 2..=6 { r.join_seat(id, "Racer"); }
        let mut s = Session::new(&r.players);
        assert_eq!(s.cars.len(), 6);
        assert_eq!(s.cars.keys().copied().collect::<Vec<_>>(), (1..=6).collect::<Vec<_>>());
        let inputs = (1..=6).map(|id| (id, Input { move_y: -1, ..Input::default() })).collect();
        for _ in 0..2500 { s.step(&inputs); if s.finished { break; } }
        assert!(s.finished, "all six must cross the one-lap line: {:?}", s.cars);
        assert!(s.cars.values().all(|c| c.finish.is_some() && c.d == LAP));
        s.step(&[(1, Input { interact: true, ..Input::default() })].into());
        assert!(!s.finished);
        assert_eq!(s.cars.len(), 6);
        assert!(s.cars.values().all(|c| c.finish.is_none()));
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
