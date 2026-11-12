//! Knight Racers as a game: the title, the modes, the classes, the menus' state,
//! the cup over its three courses and what a race pays.
//!
//! The race itself (`race.rs`) knows nothing of this. `tick` is the one door the
//! realm's tick and the Glass example both go through: it reads what each seat
//! pressed (`Pad`), runs the menus while no race is on (with a CPU race for a
//! backdrop), builds the race the knights chose, steps it, settles its purses and
//! records when the flag falls, and walks the cup on to its podium.
//!
//! Everything here is a function of the pads and the tick, so every seat and every
//! replay agrees. Nothing here touches a file: the garage's `bank` carries each
//! knight's career in and out of the ledger (and lays the time-trial ghost on the
//! road), reading and writing this state between ticks.
use super::career::{self, Career, Notes};
use super::course::{courses, cup, CourseId};
use super::race::{Car, Drive, Session, COUNTDOWN, HZ};
use super::tune::Tune;
use crate::rules::Input;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// How a race is entered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// The championship: the three courses, points by place, a podium and a prize.
    #[default]
    Cup,
    /// One race on a course of your choosing, for a purse.
    Single,
    /// A lap against your own best, shown as a ghost.
    TimeTrial,
    /// The Proving Run, for nothing.
    Practice,
}

impl Mode {
    pub const ALL: [Mode; 4] = [Mode::Cup, Mode::Single, Mode::TimeTrial, Mode::Practice];
    pub fn name(self) -> &'static str {
        match self { Mode::Cup => "CUP", Mode::Single => "SINGLE RACE", Mode::TimeTrial => "TIME TRIAL", Mode::Practice => "PRACTICE" }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            Mode::Cup => "THREE COURSES. POINTS 10 7 5 3 2 1. A PODIUM, A TROPHY AND A PRIZE.",
            Mode::Single => "ONE RACE ON ANY COURSE. TWO THIRDS OF THE CUP'S PURSE.",
            Mode::TimeTrial => "ALONE ON THE ROAD WITH THE GHOST OF YOUR BEST LAP.",
            Mode::Practice => "THE PROVING RUN, NO PRIZE, FIVE EASY RIVALS.",
        }
    }
}

/// How hard the rivals are, and what the prizes are worth.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Class { #[default] Squire, Knight, Champion }

impl Class {
    pub const ALL: [Class; 3] = [Class::Squire, Class::Knight, Class::Champion];
    pub fn rank(self) -> usize { self as usize }
    pub fn name(self) -> &'static str { ["SQUIRE", "KNIGHT", "CHAMPION"][self.rank()] }
    pub fn blurb(self) -> &'static str {
        ["RIVALS RACE EASY. FOR LEARNING THE ROAD.", "RIVALS RACE SHARP. EXPECT A FIGHT FOR THE PODIUM.", "RIVALS RACE FAST AND GRIP HARD. NO MERCY."][self.rank()]
    }
    /// What it takes to open this class.
    pub fn hint(self) -> &'static str {
        ["", "WIN THE SQUIRE CUP", "WIN THE KNIGHT CUP"][self.rank()]
    }
    /// The multiples a rival's pod races at in this class. The pilots (`pilot.rs`) race at 97.5% of whatever
    /// the pod has, and draft, heat and slam on top, so these sit near 1: set against a clean human run
    /// (`flow_tests::class_balance_table`, 11-08) a Squire loses 18 of 18, a Knight 13 of 18 and a Champion 7 of 18.
    pub fn rivals(self) -> Tune {
        match self {
            Class::Squire => Tune { top: 0.93, boost: 0.93, accel: 0.88, grip: 0.96, mass: 1. },
            Class::Knight => Tune { top: 1.00, boost: 1.00, accel: 1.04, grip: 1.01, mass: 1. },
            Class::Champion => Tune { top: 1.02, boost: 1.02, accel: 1.12, grip: 1.06, mass: 1. },
        }
    }
}

/// Where the game is, for every seat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// The title: a CPU race behind the name.
    Title,
    /// The host picks a mode (and any seat may open its own garage or records).
    Modes,
    Classes,
    Courses,
    /// Every seat picks a pod; the race starts when all are ready.
    Pods,
    /// The race: the grid, the lights, the laps, the flag and the results board.
    #[default]
    Race,
    /// The cup is run: the podium.
    Podium,
}

/// A screen of a seat's own, which does not hold the others up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Local { #[default] None, Garage, Records }

/// What each seat presses. `x` is +1 for right (D) and `y` -1 for up (W), +1 for down (S).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pad {
    pub x: i8,
    pub y: i8,
    /// E, F, Enter: confirm, and E at the flag.
    pub ok: bool,
    /// Space: boost in the race, confirm in the menus.
    pub boost: bool,
    /// Q, Backspace: a step back.
    pub back: bool,
    /// Esc held: a short tap is a step back too, a long hold leaves the game.
    pub leave: bool,
    /// A CPU pilot has the wheel: skipping ahead, a demo or a proof.
    pub auto: bool,
}

impl Pad {
    pub fn from_input(i: &Input) -> Pad {
        Pad { x: i.move_x.clamp(-1, 1), y: i.move_y.clamp(-1, 1), ok: i.interact || i.fire, boost: i.dash, back: i.swing, leave: i.leave_held, auto: false }
    }
    pub fn drive(self) -> Drive {
        Drive { steer: self.x.clamp(-1, 1), gas: self.y < 0, brake: self.y > 0, boost: self.boost, interact: false }
    }
}

const UP: u8 = 1;
const DOWN: u8 = 2;
const LEFT: u8 = 4;
const RIGHT: u8 = 8;
const OK: u8 = 16;
const BOOST: u8 = 32;
const BACK: u8 = 64;
/// A tap of Esc shorter than this is a step back.
const TAP: u8 = 12;
/// Ticks the results stay up before a press moves them on.
const GRACE: u32 = 36;
/// A seat that is ready is waited for this long by the rest.
const WAIT: u32 = 8 * HZ as u32;
/// Ticks the attract race runs on a course before the next.
const ATTRACT: u32 = 25 * HZ as u32;
/// Ticks a shop or menu note stays up.
const NOTE: u32 = 2 * HZ as u32;

/// The entries of the mode menu: the four modes, then the two screens of a seat's own.
pub const MENU: [&str; 6] = ["CUP", "SINGLE RACE", "TIME TRIAL", "PRACTICE", "GARAGE", "RECORDS"];

/// The courses a single race or a trial may be run on: the cup's, then the proving run.
pub fn choices() -> [CourseId; 4] { [cup()[0], cup()[1], cup()[2], CourseId::Proving] }

/// What a seat's last race did for it, for the results board and the podium.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settle {
    pub place: usize,
    pub purse: u32,
    pub notes: Notes,
    /// A time trial's lap beat the best, or was the first.
    pub trial: bool,
    /// What the cup opened for the seat: words for the podium.
    pub unlocks: Vec<String>,
    /// The cup's own prize, once the cup is won.
    pub bonus: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SeatFlow {
    pub name: String,
    /// Whether this seat is a knight in the race now.
    pub here: bool,
    pub career: Career,
    /// The career is a blank one: the garage's bank replaces it with what the ledger holds.
    pub fresh: bool,
    /// The career has changed: the bank writes it down.
    pub dirty: bool,
    /// The pod this seat has its eye on, an index into the roster.
    pub pick: u8,
    pub ready: bool,
    /// The seat's place in whatever list it is looking at.
    pub cursor: u8,
    /// The part under the cursor in the garage.
    pub sub: u8,
    pub local: Local,
    pub tab: u8,
    held: u8,
    esc: u8,
    /// A line for the screen, and the tick it is up until.
    pub note: String,
    pub note_until: u32,
    pub last: Option<Settle>,
}

impl SeatFlow {
    fn new(name: &str) -> SeatFlow {
        let start = Career::default();
        SeatFlow { name: name.to_string(), here: true, career: start, fresh: true, ..SeatFlow::default() }
    }
    pub fn said(&mut self, now: u32, words: &str) { self.note = words.to_string(); self.note_until = now + NOTE; }
}

/// One seat's cup so far.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CupLine {
    pub purse: u32,
    pub places: Vec<usize>,
}

/// One row of the podium: a seat in the order the cup ended.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PodiumLine {
    pub seat: u32,
    pub points: u32,
    pub pod: u8,
    pub human: bool,
}

/// One seat's lap clock, as the sim sees it (the screen keeps its own for the HUD).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Laps {
    /// Finished laps, in seconds.
    pub times: Vec<f32>,
    /// Race seconds at which the lap now running began.
    pub begin: f32,
}

impl Laps {
    pub fn best(&self) -> Option<f32> { self.times.iter().copied().filter(|t| t.is_finite() && *t > 0.).min_by(f32::total_cmp) }

    fn update(&mut self, car: &Car, ticks: u32, len: f32, count: u32) {
        let now = ticks.saturating_sub(COUNTDOWN) as f32 / HZ;
        let reached = if car.finish.is_some() { count } else { ((car.d.max(0.) / len).floor() as u32).min(count) };
        while (self.times.len() as u32) < reached {
            let j = self.times.len() as u32 + 1;
            let at = if j == count && car.finish.is_some() { car.finish.unwrap_or(now) } else { now - (car.d - j as f32 * len).max(0.) / car.v.max(1.) };
            self.times.push((at - self.begin).max(0.));
            self.begin = at;
        }
    }
}

/// Where the time-trial ghost is this tick, set by the garage's bank from the saved lap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GhostPose {
    /// Metres into the lap.
    pub s: f32,
    pub y: f32,
    pub yaw: f32,
    pub lift: f32,
    pub route: u8,
    pub pod: u8,
    /// Seconds ahead (+) or behind (-) of the player at this point, for the screen.
    pub gap: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Flow {
    pub stage: Stage,
    pub mode: Mode,
    pub class: Class,
    /// The course chosen for a single race or a trial.
    pub course: CourseId,
    /// Ticks since the stage began.
    pub clock: u32,
    /// Ticks since the flow began: never reset, for notes and blinking.
    pub now: u32,
    pub seats: BTreeMap<u32, SeatFlow>,
    pub settled: bool,
    pub cup: BTreeMap<u32, CupLine>,
    pub podium: Vec<PodiumLine>,
    pub laps: BTreeMap<u32, Laps>,
    pub ghost: Option<GhostPose>,
    /// Which course of the cup the attract race runs on.
    pub attract: u8,
}

impl Default for Flow {
    /// A race with no menus: how a session begins when something else (a test, a proof, an
    /// old save) made it.
    fn default() -> Flow {
        Flow {
            stage: Stage::Race, mode: Mode::Cup, class: Class::Squire, course: cup()[0], clock: 0, now: 0, seats: BTreeMap::new(),
            settled: false, cup: BTreeMap::new(), podium: Vec::new(), laps: BTreeMap::new(), ghost: None, attract: 0,
        }
    }
}

impl Flow {
    /// A gate: the title is up.
    pub fn at_gate() -> Flow { Flow { stage: Stage::Title, ..Flow::default() } }

    /// The seat that decides: the lowest knight in the race.
    pub fn host(&self) -> Option<u32> { self.seats.iter().find(|(_, s)| s.here).map(|(id, _)| *id) }

    /// Menus are up over a CPU race.
    pub fn attracting(&self) -> bool { matches!(self.stage, Stage::Title | Stage::Modes | Stage::Classes | Stage::Courses) }

    /// A menu of any kind has the screen (the race's own instruments do not).
    pub fn menu(&self) -> bool { self.stage != Stage::Race }

    /// The time trial on the road (or just run, its board still up): the seat, the course and the
    /// pod, for the bank's ghost.
    pub fn trial(&self, s: &Session) -> Option<(u32, CourseId, u8)> {
        if self.stage != Stage::Race || self.mode != Mode::TimeTrial { return None; }
        let seat = self.host()?;
        let car = s.cars.get(&seat)?;
        Some((seat, s.course, car.pod))
    }

    fn enter(&mut self, stage: Stage) {
        self.stage = stage;
        self.clock = 0;
        for sf in self.seats.values_mut() { sf.ready = false; }
        if let Some(h) = self.host() { if let Some(sf) = self.seats.get_mut(&h) { sf.cursor = 0; } }
    }

    fn sync(&mut self, humans: &[(u32, String)]) {
        for (id, name) in humans {
            let sf = self.seats.entry(*id).or_insert_with(|| SeatFlow::new(name));
            if sf.name != *name { *sf = SeatFlow::new(name); }
            sf.here = true;
        }
        for (id, sf) in self.seats.iter_mut() {
            if !humans.iter().any(|(h, _)| h == id) { sf.here = false; sf.ready = false; sf.local = Local::None; }
        }
        self.seats.retain(|_, sf| sf.here || sf.dirty);
        self.now = self.now.wrapping_add(1);
    }
}

impl Session {
    /// A session at the race gate: the title over a CPU race, the knights in.
    pub fn at_gate(players: &BTreeMap<u32, crate::rules::Hero>) -> Session { Session::gate_of(players.keys().copied()) }

    /// The same for the seats `ids`.
    pub fn gate_of(ids: impl IntoIterator<Item = u32>) -> Session {
        let mut s = Session::blank();
        s.join_ids(ids);
        s.flow = Flow::at_gate();
        s.ticks = COUNTDOWN;
        s
    }

    /// Put the menus away: a race on the grid, as a test or a proof wants it.
    pub fn skip_menus(&mut self) { self.flow = Flow::default(); self.ticks = 0; }
}

/// Everything a seat can see about its own pod, for the pod select: the tuned pod a knight's
/// parts make of it.
pub fn tuned_pod(career: &Career, pod: u8) -> super::pods::PodSpec {
    super::pods::pods()[pod as usize % super::pods::pods().len()].tuned(&career.tune())
}

// ---- The tick ---------------------------------------------------------------

/// One tick of the whole game for the knights in `humans` (seat and name), who pressed `pads`.
pub fn tick(s: &mut Session, humans: &[(u32, String)], pads: &BTreeMap<u32, Pad>) {
    s.flow.sync(humans);
    let ids: Vec<u32> = humans.iter().map(|(id, _)| *id).collect();
    s.join_ids(ids.iter().copied());
    if s.flow.mode == Mode::TimeTrial && !s.flow.attracting() && s.flow.stage != Stage::Pods {
        s.cars.retain(|_, c| !c.cpu);
    } else {
        s.fill_rivals(ids.len());
    }
    // What each seat pressed fresh this tick.
    let mut fresh: BTreeMap<u32, u8> = BTreeMap::new();
    for (&id, sf) in s.flow.seats.iter_mut() {
        let pad = pads.get(&id).copied().unwrap_or_default();
        fresh.insert(id, edges(sf, &pad));
    }
    // A seat in its own screen is not in the others' menus.
    let mut local_done = Vec::new();
    for id in ids.iter().copied() {
        if s.flow.seats.get(&id).is_some_and(|sf| sf.local != Local::None) {
            seat_screen(s, id, fresh.get(&id).copied().unwrap_or(0));
            local_done.push(id);
        }
    }
    for id in &local_done { fresh.insert(*id, 0); }
    let host = s.flow.host();
    s.flow.clock = s.flow.clock.wrapping_add(1);
    match s.flow.stage {
        Stage::Title => {
            attract(s);
            if fresh.values().any(|f| f & (OK | BOOST) != 0) { s.flow.enter(Stage::Modes); }
        }
        Stage::Modes => {
            attract(s);
            for id in ids.iter().copied() { modes(s, id, fresh.get(&id).copied().unwrap_or(0), host == Some(id), ids.len()); }
        }
        Stage::Classes | Stage::Courses | Stage::Podium => {
            if s.flow.stage != Stage::Podium { attract(s); }
            if let Some(h) = host { host_menu(s, h, fresh.get(&h).copied().unwrap_or(0), ids.len()); }
        }
        Stage::Pods => pods_stage(s, &ids, &fresh, host),
        Stage::Race => race(s, &ids, pads, &fresh, host),
    }
}

/// The new presses of a seat this tick, as bits; and a tap of Esc as a step back.
fn edges(sf: &mut SeatFlow, pad: &Pad) -> u8 {
    let mut now = 0u8;
    if pad.y < 0 { now |= UP; }
    if pad.y > 0 { now |= DOWN; }
    if pad.x < 0 { now |= LEFT; }
    if pad.x > 0 { now |= RIGHT; }
    if pad.ok { now |= OK; }
    if pad.boost { now |= BOOST; }
    if pad.back { now |= BACK; }
    let mut fresh = now & !sf.held;
    sf.held = now;
    if pad.leave { sf.esc = sf.esc.saturating_add(1); }
    else { if sf.esc > 0 && sf.esc < TAP { fresh |= BACK; } sf.esc = 0; }
    fresh
}

/// Step a list cursor by a press, wrapping.
fn step_cursor(cursor: u8, len: usize, f: u8) -> u8 {
    let len = len.max(1) as i32;
    let mut c = cursor as i32;
    if f & (UP | LEFT) != 0 { c -= 1; }
    if f & (DOWN | RIGHT) != 0 { c += 1; }
    c.rem_euclid(len) as u8
}

// ---- The backdrop -----------------------------------------------------------

/// The CPU race behind the menus: every car under a pilot, round the cup's courses in turn.
fn attract(s: &mut Session) {
    let drives = s.bot_drives_all();
    s.step_drive(&drives);
    let browsing = s.flow.stage == Stage::Courses;
    if s.finished || (s.flow.now % ATTRACT == 0 && s.flow.now > 0 && !browsing) {
        // Browsing the courses, the race behind is the one under the cursor: it runs again.
        if !browsing { s.flow.attract = s.flow.attract.wrapping_add(1); }
        let next = if browsing { s.course } else { cup()[s.flow.attract as usize % cup().len()] };
        reseed(s, next);
    }
}

/// A new CPU race on `id`, under way already.
fn reseed(s: &mut Session, id: CourseId) {
    let mut n = Session::blank_on(id);
    let seats: Vec<(u32, u8)> = s.flow.seats.iter().filter(|(_, sf)| sf.here).map(|(i, sf)| (*i, sf.pick)).collect();
    n.join_ids(seats.iter().map(|(i, _)| *i));
    n.fill_rivals(seats.len());
    for (seat, pick) in &seats {
        if let Some(c) = n.cars.get_mut(seat) { if s.flow.stage != Stage::Title { c.pod = *pick; } }
    }
    n.ticks = COUNTDOWN;
    n.flow = std::mem::take(&mut s.flow);
    *s = n;
}

// ---- Menus ------------------------------------------------------------------

/// The mode menu: every seat has its own cursor on it; only the host's choice starts anything.
fn modes(s: &mut Session, id: u32, f: u8, host: bool, humans: usize) {
    let now = s.flow.now;
    let Some(sf) = s.flow.seats.get_mut(&id) else { return };
    sf.cursor = step_cursor(sf.cursor, MENU.len(), f & (UP | DOWN));
    if f & BACK != 0 && host { s.flow.enter(Stage::Title); return; }
    if f & (OK | BOOST) == 0 { return; }
    match sf.cursor {
        4 => { sf.local = Local::Garage; sf.cursor = 0; sf.sub = slot_cursor(sf, 0); }
        5 => { sf.local = Local::Records; sf.tab = 0; }
        n if !host => { let _ = n; sf.said(now, "THE HOST CHOOSES THE RACE"); }
        2 if humans > 1 => sf.said(now, "A TRIAL IS RUN ALONE"),
        n => {
            let mode = Mode::ALL[n as usize];
            s.flow.mode = mode;
            match mode {
                Mode::Cup | Mode::Single => s.flow.enter(Stage::Classes),
                Mode::TimeTrial => { s.flow.enter(Stage::Courses); }
                Mode::Practice => { s.flow.course = CourseId::Proving; s.flow.class = Class::Squire; s.flow.enter(Stage::Pods); }
            }
            if s.flow.stage == Stage::Courses { let c = choices()[0]; if s.course != c { reseed(s, c); } }
        }
    }
}

/// The classes, the courses and the podium: the host's cursor, for every seat to see.
fn host_menu(s: &mut Session, h: u32, f: u8, humans: usize) {
    let now = s.flow.now;
    let stage = s.flow.stage;
    let Some(sf) = s.flow.seats.get_mut(&h) else { return };
    match stage {
        Stage::Classes => {
            sf.cursor = step_cursor(sf.cursor, 3, f & (UP | DOWN | LEFT | RIGHT));
            if f & BACK != 0 { s.flow.enter(Stage::Modes); return; }
            if f & (OK | BOOST) != 0 {
                let class = Class::ALL[sf.cursor as usize % 3];
                if !sf.career.class_open(class) { sf.said(now, &format!("LOCKED: {}", class.hint())); return; }
                s.flow.class = class;
                if s.flow.mode == Mode::Cup { s.flow.course = cup()[0]; s.flow.enter(Stage::Pods); }
                else { s.flow.enter(Stage::Courses); let c = choices()[0]; if s.course != c { reseed(s, c); } }
            }
        }
        Stage::Courses => {
            let before = sf.cursor;
            sf.cursor = step_cursor(sf.cursor, 4, f & (UP | DOWN | LEFT | RIGHT));
            let at = sf.cursor as usize;
            let _ = humans;
            if f & BACK != 0 { let to = if s.flow.mode == Mode::TimeTrial { Stage::Modes } else { Stage::Classes }; s.flow.enter(to); return; }
            if f & (OK | BOOST) != 0 { s.flow.course = choices()[at]; s.flow.enter(Stage::Pods); return; }
            if sf.cursor != before { let c = choices()[at]; if s.course != c { reseed(s, c); } }
        }
        Stage::Podium => {
            if s.flow.clock > GRACE && f & (OK | BOOST) != 0 { s.flow.enter(Stage::Modes); let c = cup()[0]; reseed(s, c); }
        }
        _ => {}
    }
}

/// The pod select: every seat picks and readies; the race begins when all are ready.
fn pods_stage(s: &mut Session, ids: &[u32], fresh: &BTreeMap<u32, u8>, host: Option<u32>) {
    let now = s.flow.now;
    let n = super::pods::pods().len();
    for &id in ids {
        let f = fresh.get(&id).copied().unwrap_or(0);
        let Some(sf) = s.flow.seats.get_mut(&id) else { continue };
        if sf.ready {
            if f & BACK != 0 { sf.ready = false; }
            continue;
        }
        if f & (UP | LEFT) != 0 { sf.pick = ((sf.pick as usize + n - 1) % n) as u8; }
        if f & (DOWN | RIGHT) != 0 { sf.pick = ((sf.pick as usize + 1) % n) as u8; }
        if f & (OK | BOOST) != 0 {
            if sf.career.owns_pod(sf.pick) { sf.ready = true; }
            else { sf.said(now, "LOCKED: WIN A CUP TO OPEN IT"); }
        }
        if f & BACK != 0 && host == Some(id) {
            let to = match s.flow.mode { Mode::Cup => Stage::Classes, Mode::Single | Mode::TimeTrial => Stage::Courses, Mode::Practice => Stage::Modes };
            s.flow.enter(to);
            if to != Stage::Modes && s.flow.stage == Stage::Courses { let c = choices()[0]; if s.course != c { reseed(s, c); } }
            if s.flow.stage == Stage::Classes { let c = cup()[0]; if s.course != c { reseed(s, c); } }
            return;
        }
    }
    // The CPU race goes on behind the pod screen only for a look: hold the grid still.
    let all = !ids.is_empty() && ids.iter().all(|i| s.flow.seats.get(i).is_some_and(|sf| sf.ready));
    let waited = host.and_then(|h| s.flow.seats.get(&h)).is_some_and(|sf| sf.ready) && s.flow.clock > WAIT && ids.iter().any(|i| s.flow.seats.get(i).is_some_and(|sf| sf.ready && Some(*i) != host));
    if all || waited {
        begin(s, ids, None);
    }
}

/// A seat's own screens: the garage and the records.
fn seat_screen(s: &mut Session, id: u32, f: u8) {
    let now = s.flow.now;
    let Some(sf) = s.flow.seats.get_mut(&id) else { return };
    match sf.local {
        Local::Garage => {
            if f & BACK != 0 { sf.local = Local::None; sf.cursor = 4; return; }
            let slot = sf.cursor as usize % career::WORKING_SLOTS;
            let parts = career::wares(slot);
            if f & (UP | DOWN) != 0 {
                sf.cursor = step_cursor(sf.cursor, career::WORKING_SLOTS, f & (UP | DOWN));
                sf.sub = slot_cursor(sf, sf.cursor as usize);
            } else if f & (LEFT | RIGHT) != 0 {
                sf.sub = step_cursor(sf.sub, parts.len(), f & (LEFT | RIGHT));
            } else if f & (OK | BOOST) != 0 {
                let Some(part) = parts.get(sf.sub as usize) else { return };
                match sf.career.buy_and_fit(part.id) {
                    Ok(b) if b.price > 0 => { sf.dirty = true; sf.said(now, &format!("BOUGHT AND FITTED  -{}", b.price)); }
                    Ok(b) if b.fitted => { sf.dirty = true; sf.said(now, "FITTED"); }
                    Ok(_) => sf.said(now, "ALREADY FITTED"),
                    Err(career::ShopError::Poor(n)) => sf.said(now, &format!("{n} GOLD SHORT")),
                    Err(_) => sf.said(now, "NOT FOR SALE"),
                }
            }
        }
        Local::Records => {
            if f & BACK != 0 { sf.local = Local::None; sf.cursor = 5; return; }
            sf.tab = step_cursor(sf.tab, courses().len(), f & (LEFT | RIGHT | UP | DOWN));
        }
        Local::None => {}
    }
}

/// The index in a slot's rack of the part the seat wears there.
fn slot_cursor(sf: &SeatFlow, slot: usize) -> u8 {
    let worn = sf.career.worn(slot.min(career::WORKING_SLOTS - 1));
    career::wares(slot.min(career::WORKING_SLOTS - 1)).iter().position(|p| p.id == worn).unwrap_or(0) as u8
}

// ---- The race ---------------------------------------------------------------

/// Set a new race up: the course the mode and the cup say, every knight on the grid in the pod
/// they picked with their parts on it, the rivals in their class. `round` is the cup's next
/// race, or none to start a fresh cup.
fn begin(s: &mut Session, ids: &[u32], round: Option<u8>) {
    let (mode, class) = (s.flow.mode, s.flow.class);
    let id = match mode {
        Mode::Cup => cup()[round.unwrap_or(0) as usize % cup().len()],
        Mode::Practice => CourseId::Proving,
        Mode::Single | Mode::TimeTrial => s.flow.course,
    };
    let mut n = Session::blank_on(id);
    n.points = if mode == Mode::Cup && round.is_some() { std::mem::take(&mut s.points) } else { BTreeMap::new() };
    n.join_ids(ids.iter().copied());
    if mode != Mode::TimeTrial { n.fill_rivals(ids.len()); }
    // Practice is gentle whatever the class.
    let rivals = if mode == Mode::Practice { Class::Squire.rivals() } else { class.rivals() };
    for (seat, car) in n.cars.iter_mut() {
        match s.flow.seats.get(seat).filter(|_| !car.cpu) {
            Some(sf) => { car.pod = sf.pick; car.tune = sf.career.tune(); }
            None => car.tune = rivals,
        }
    }
    let mut flow = std::mem::take(&mut s.flow);
    flow.stage = Stage::Race;
    flow.clock = 0;
    flow.settled = false;
    flow.laps.clear();
    flow.ghost = None;
    flow.podium.clear();
    if mode != Mode::Cup || round.is_none() { flow.cup.clear(); }
    for sf in flow.seats.values_mut() { sf.last = None; sf.ready = true; }
    n.flow = flow;
    *s = n;
}

fn race(s: &mut Session, ids: &[u32], pads: &BTreeMap<u32, Pad>, fresh: &BTreeMap<u32, u8>, host: Option<u32>) {
    if s.finished {
        if !s.flow.settled { settle(s); }
        let f = host.and_then(|h| fresh.get(&h)).copied().unwrap_or(0);
        if s.flow.clock <= GRACE { return; }
        if f & OK != 0 { advance(s, ids); }
        else if f & BACK != 0 && s.flow.mode != Mode::Cup { go_menu(s); }
        return;
    }
    let mut drives: BTreeMap<u32, Drive> = s.bot_drives();
    let pilot = if pads.values().any(|p| p.auto) { Some(s.bot_drives_all()) } else { None };
    for &id in ids {
        let pad = pads.get(&id).copied().unwrap_or_default();
        let d = match (&pilot, pad.auto) { (Some(all), true) => all.get(&id).copied().unwrap_or_default(), _ => pad.drive() };
        drives.insert(id, d);
    }
    s.step_drive(&drives);
    let (len, count) = (s.course().lap(), s.laps());
    let ticks = s.ticks;
    for &id in ids {
        if let Some(car) = s.cars.get(&id) { s.flow.laps.entry(id).or_default().update(car, ticks, len, count); }
    }
    if s.finished { settle(s); }
}

/// Skip the menus: begin a race of `mode` and `class` on `course` (the cup's first, or the proving
/// run, if none) with the pod `pod` for every knight in `humans`. For a command line, a demo or a
/// proof; the menus come to the same.
pub fn quick(s: &mut Session, humans: &[(u32, String)], mode: Mode, class: Class, course: Option<CourseId>, pod: u8) {
    s.flow.sync(humans);
    s.flow.mode = mode;
    s.flow.class = class;
    s.flow.course = course.unwrap_or(if mode == Mode::Cup { cup()[0] } else { CourseId::Proving });
    for sf in s.flow.seats.values_mut() { sf.pick = pod; }
    let ids: Vec<u32> = humans.iter().map(|(id, _)| *id).collect();
    begin(s, &ids, None);
}

fn go_menu(s: &mut Session) {
    s.flow.enter(Stage::Modes);
    let c = cup()[0];
    reseed(s, c);
}

/// E at the board: the cup's next race, the podium after its last, or the same race again.
fn advance(s: &mut Session, ids: &[u32]) {
    match s.flow.mode {
        Mode::Cup => {
            let next = s.round as usize + 1;
            if next < cup().len() { begin(s, ids, Some(next as u8)); } else { podium(s); }
        }
        _ => begin(s, ids, None),
    }
}

/// The flag has fallen for everyone: pay the purses, keep the records.
fn settle(s: &mut Session) {
    s.flow.settled = true;
    s.flow.clock = 0;
    let (mode, class, id) = (s.flow.mode, s.flow.class, s.course);
    // With no CPU rivals the class has nothing to be hard on: the prizes are a Squire's.
    let class = if s.cars.values().any(|c| c.cpu) { class } else { Class::Squire };
    let order = s.order();
    for (n, seat) in order.iter().enumerate() {
        let place = n + 1;
        let Some(car) = s.cars.get(seat).filter(|c| !c.cpu) else { continue };
        let (pod, time) = (car.pod, car.finish.unwrap_or(0.));
        let best = s.flow.laps.get(seat).and_then(Laps::best);
        let Some(sf) = s.flow.seats.get_mut(seat) else { continue };
        let notes = sf.career.note_race(id, pod, place, time, best);
        let mut settle = Settle { place, notes, ..Settle::default() };
        match mode {
            Mode::TimeTrial => {
                let first = !sf.career.trials.contains_key(&career::lap_key(id, pod));
                if let Some(b) = best {
                    if sf.career.note_trial(id, pod, b) {
                        settle.trial = true;
                        settle.purse = if first { career::TRIAL_FIRST } else { career::TRIAL_BEST };
                    }
                }
            }
            _ => settle.purse = career::purse(mode, class, place),
        }
        sf.career.pay(settle.purse);
        sf.dirty = true;
        sf.last = Some(settle.clone());
        if mode == Mode::Cup {
            let line = s.flow.cup.entry(*seat).or_default();
            line.purse += settle.purse;
            line.places.push(place);
        }
    }
}

/// The last race of the cup is run: its standings, the prize and what winning it opens.
fn podium(s: &mut Session) {
    let class = if s.cars.values().any(|c| c.cpu) { s.flow.class } else { Class::Squire };
    let rows = s.standings();
    let mut lines = Vec::new();
    for (n, (seat, points)) in rows.iter().enumerate() {
        let car = s.cars.get(seat);
        lines.push(PodiumLine { seat: *seat, points: *points, pod: car.map_or(0, |c| c.pod), human: car.is_some_and(|c| !c.cpu) });
        let place = n + 1;
        let swept = s.flow.cup.get(seat).is_some_and(|l| l.places.len() == cup().len() && l.places.iter().all(|&p| p == 1));
        let Some(sf) = s.flow.seats.get_mut(seat).filter(|sf| sf.here) else { continue };
        let bonus = career::cup_purse(class, place);
        sf.career.pay(bonus);
        let mut unlocks = Vec::new();
        if place == 1 {
            sf.career.cups[class.rank()] += 1;
            let (next, pod) = career::cup_unlock(class);
            if sf.career.cups[class.rank()] == 1 {
                if let Some(c) = next { if sf.career.open_class(c) { unlocks.push(format!("{} CLASS", c.name())); } }
                if let Some(p) = pod { if sf.career.unlock_pod(p) { unlocks.push(format!("THE {}", super::pods::pods()[p as usize].name)); } }
            }
            if swept {
                sf.career.sweeps += 1;
                if sf.career.unlock_pod(career::SWEEP_POD) { unlocks.push(format!("THE {}", super::pods::pods()[career::SWEEP_POD as usize].name)); }
            }
        }
        sf.dirty = true;
        let last = sf.last.get_or_insert_with(Settle::default);
        last.bonus = bonus;
        last.unlocks = unlocks;
    }
    s.flow.podium = lines;
    s.flow.enter(Stage::Podium);
}

#[cfg(test)]
#[path = "flow_tests.rs"]
mod tests;
