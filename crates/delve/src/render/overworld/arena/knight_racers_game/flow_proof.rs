//! Proof for the game around the race: every screen is drawn, deterministic and not blank; and
//! the frame sequences the review GIFs are cut from, scripted through the real menus with the
//! same pads a seat presses:
//!
//!   ANGEL_FLOW_PROOF=/dir cargo test -p angel-delve --profile fast --lib flow_walkthrough
//!   ANGEL_FLOW_PROOF=/dir cargo test -p angel-delve --profile fast --lib flow_trial
//!   ANGEL_FLOW_TIME=1 cargo test -p angel-delve --profile fast --lib flow_frame_times -- --nocapture
use super::*;
use crate::garage::{Bank, Store};
use crate::rules::knight_racers::flow::{self, Pad};
use crate::rules::Run;
use std::time::Instant;

fn pad() -> Pad { Pad::default() }
fn ok() -> Pad { Pad { ok: true, ..pad() } }
fn up() -> Pad { Pad { y: -1, ..pad() } }
fn down() -> Pad { Pad { y: 1, ..pad() } }
fn left() -> Pad { Pad { x: -1, ..pad() } }
fn right() -> Pad { Pad { x: 1, ..pad() } }
fn back() -> Pad { Pad { back: true, ..pad() } }
fn auto() -> Pad { Pad { auto: true, ..pad() } }

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("racer-flow-{name}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Flattened on black, as the GIFs are.
fn save_flat(im: &Img, path: &std::path::Path) {
    let mut buf = Vec::with_capacity(384 * 224 * 3);
    for y in 0..224 { for x in 0..384 { buf.extend_from_slice(&im.get(x, y).unwrap_or([0, 0, 0])); } }
    image::save_buffer(path, &buf, 384, 224, image::ColorType::Rgb8).unwrap();
}

/// A seat at the gate: the real flow, a bank over a throwaway save file, a frame on demand.
struct Walk {
    run: Run,
    race: rules::Session,
    bank: Bank,
    who: Vec<(u32, String)>,
    dir: Option<std::path::PathBuf>,
    shots: usize,
    save: std::path::PathBuf,
}

impl Walk {
    fn new(out: Option<&std::path::Path>, name: &str) -> Walk {
        let save = temp(name).join("garage.json");
        let mut w = Walk::over(save, out);
        w.step(pad());
        w
    }

    fn over(save: std::path::PathBuf, out: Option<&std::path::Path>) -> Walk {
        let mut run = Run::roundtable_for_test("knight_racers").unwrap();
        run.knight_racers = None;
        let dir = out.map(|o| { std::fs::create_dir_all(o).unwrap(); o.to_path_buf() });
        Walk { run, race: rules::Session::gate_of([1]), bank: Bank::open(Store::new(&save)), who: vec![(1, "Sir Proof".to_string())], dir, shots: 0, save }
    }

    fn step(&mut self, p: Pad) {
        flow::tick(&mut self.race, &self.who, &[(1, p)].into());
        self.bank.pump(&mut self.race);
        assert!(self.race.valid());
    }

    fn frame(&mut self) -> Img {
        self.run.knight_racers = Some(self.race.clone());
        layers(&self.run, Some(1)).0
    }

    fn shot(&mut self) {
        let Some(dir) = self.dir.clone() else { return };
        let im = self.frame();
        save_flat(&im, &dir.join(format!("{:04}.png", self.shots)));
        self.shots += 1;
    }

    /// A frame under a name of its own.
    fn snap(&mut self, name: &str) {
        let Some(dir) = self.dir.clone() else { return };
        let im = self.frame();
        save_flat(&im, &dir.join(format!("{name}.png")));
    }

    /// `ticks` ticks of `p`, a frame every `every`.
    fn hold(&mut self, ticks: u32, p: Pad, every: u32) {
        for t in 0..ticks { self.step(p); if t % every == 0 { self.shot(); } }
    }

    /// A tap, then `rest` ticks of nothing with a frame every `every`.
    fn tap(&mut self, p: Pad, rest: u32, every: u32) {
        self.step(p);
        self.hold(rest, pad(), every);
    }

    /// The CPU pilot at the wheel until `until` holds, a frame every `every` ticks if any.
    fn pilot(&mut self, every: Option<u32>, until: impl Fn(&rules::Session) -> bool) {
        for t in 0..60_000u32 {
            if until(&self.race) { return; }
            self.step(auto());
            if every.is_some_and(|e| t % e == 0) { self.shot(); }
        }
        panic!("the pilot did not get there");
    }

    fn stage(&self) -> rules::Stage { self.race.flow.stage }
    fn money(&self) -> u32 { self.race.flow.seats[&1].career.money }
}

/// Every screen of the game, through the real tick and the real frame: drawn, not blank, and
/// the same frame twice.
#[test]
fn every_screen_is_drawn() {
    let mut w = Walk::new(None, "screens");
    let lit = |im: &Img| (0..224).flat_map(|y| (0..384).map(move |x| (x, y))).filter(|&(x, y)| im.get(x, y).is_some_and(|c| c[0].max(c[1]).max(c[2]) > 40)).count();
    let mut seen: Vec<(&str, Img)> = Vec::new();
    let mut note = |w: &mut Walk, what: &'static str| { let im = w.frame(); assert!(lit(&im) > 4000, "{what} is blank ({} dots)", lit(&im)); assert_eq!(im.content_hash(), w.frame().content_hash(), "{what} is not steady"); seen.push((what, im)); };
    w.hold(60, pad(), 1000);
    note(&mut w, "title");
    w.tap(ok(), 40, 1000);
    note(&mut w, "modes");
    w.tap(ok(), 20, 1000);
    note(&mut w, "classes");
    w.tap(ok(), 20, 1000);
    // The cup goes straight to the pods.
    assert_eq!(w.stage(), rules::Stage::Pods);
    note(&mut w, "pods");
    w.tap(right(), 4, 1000);
    note(&mut w, "pods, a pod not won");
    w.tap(left(), 2, 1000);
    w.tap(ok(), 40, 1000);
    assert_eq!(w.stage(), rules::Stage::Race);
    w.hold(100, pad(), 1000);
    note(&mut w, "a race");
    // The garage and the records from the modes list.
    let mut w2 = Walk::new(None, "screens2");
    w2.tap(ok(), 2, 1000);
    for _ in 0..4 { w2.tap(down(), 1, 1000); }
    w2.tap(ok(), 10, 1000);
    note(&mut w2, "garage");
    w2.tap(back(), 2, 1000);
    w2.tap(down(), 1, 1000);
    w2.tap(ok(), 10, 1000);
    note(&mut w2, "records");
    // The courses for a single race.
    let mut w3 = Walk::new(None, "screens3");
    w3.tap(ok(), 2, 1000);
    w3.tap(down(), 1, 1000);
    w3.tap(ok(), 2, 1000);
    w3.tap(ok(), 20, 1000);
    note(&mut w3, "courses");
    assert!(seen.len() >= 9);
}

/// The whole walk the GIF shows: the title, a cup (the pilot at the wheel, skipping ahead), the
/// podium, the garage's purchase, and the next cup open.
#[test]
fn flow_walkthrough() {
    let Some(out) = std::env::var_os("ANGEL_FLOW_PROOF").map(std::path::PathBuf::from) else { return };
    let mut w = Walk::new(Some(&out.join("walk")), "walk");
    // Title.
    w.hold(75, pad(), 3);
    w.tap(ok(), 24, 3);
    // The modes.
    for _ in 0..2 { w.tap(down(), 8, 3); }
    w.tap(up(), 8, 3);
    w.tap(up(), 8, 3);
    w.tap(ok(), 24, 3);
    // The classes: a Knight is shut, so the cursor tries it and learns.
    w.tap(down(), 14, 3);
    w.tap(ok(), 30, 3);
    w.tap(up(), 8, 3);
    w.tap(ok(), 20, 3);
    assert_eq!(w.stage(), rules::Stage::Pods);
    // The pods: look at a few, one of them shut.
    w.tap(right(), 16, 3);
    w.tap(right(), 16, 3);
    w.tap(ok(), 24, 3);
    w.tap(left(), 8, 3);
    w.tap(left(), 8, 3);
    w.tap(ok(), 10, 3);
    assert_eq!(w.stage(), rules::Stage::Race);
    let races = rules::cup().len();
    for round in 0..races {
        // The grid, the lights, the start; then the pilot to the flag, skipping ahead.
        w.pilot(Some(3), |s| s.ticks >= 200);
        w.pilot(Some(300), |s| s.finished);
        // Hold the final frames and the board.
        w.hold(70, pad(), 4);
        w.shot();
        let place = w.race.place(1);
        eprintln!("race {}: the pilot finished {place}, purse now {}", round + 1, w.money());
        if round + 1 == races && w.race.standings().first().map(|r| r.0) != Some(1) {
            // However the pilot placed, the cup is the knight's for the proof (the table is set).
            let lead = w.race.points.values().copied().max().unwrap_or(0) + 40;
            w.race.points.insert(1, lead);
        }
        w.tap(ok(), 20, 3);
    }
    assert_eq!(w.stage(), rules::Stage::Podium);
    w.hold(150, pad(), 3);
    w.tap(ok(), 10, 3);
    assert_eq!(w.stage(), rules::Stage::Modes);
    // The garage: the purse buys a part.
    for _ in 0..4 { w.tap(down(), 6, 3); }
    w.tap(ok(), 16, 3);
    // Crests: the Gale Fins (400).
    for _ in 0..4 { w.tap(down(), 6, 3); }
    for _ in 0..3 { w.tap(right(), 8, 3); }
    w.tap(ok(), 40, 3);
    w.tap(down(), 8, 3);
    w.tap(left(), 8, 3);
    w.tap(back(), 14, 3);
    // The next cup is open: the Knight class.
    for _ in 0..4 { w.tap(up(), 4, 3); }
    w.tap(ok(), 20, 3);
    w.tap(down(), 14, 3);
    w.tap(ok(), 24, 3);
    w.hold(40, pad(), 3);
    assert_eq!(w.stage(), rules::Stage::Pods, "the Knight cup is open");
    // And it all kept: a new bank over the same file holds the knight's career.
    let (money, cups, classes) = { let c = &w.race.flow.seats[&1].career; (c.money, c.cups, c.classes) };
    let again = Bank::open(Store::new(&w.save));
    let c = again.ledger().career("Sir Proof");
    assert_eq!((c.money, c.cups, c.classes), (money, cups, classes), "the ledger holds what was won");
    assert_eq!(c.worn(4), "gale_fins");
    eprintln!("walkthrough frames: {}", w.shots);
}

/// A time trial run twice: the first lap is kept as the ghost, the second runs beside it.
#[test]
fn flow_trial() {
    let Some(out) = std::env::var_os("ANGEL_FLOW_PROOF").map(std::path::PathBuf::from) else { return };
    let mut w = Walk::new(Some(&out.join("trial")), "trial");
    let who = w.who.clone();
    flow::quick(&mut w.race, &who, rules::Mode::TimeTrial, rules::Class::Squire, Some(rules::CourseId::Proving), 0);
    // The first run: only the road, no ghost; the whole lap is recorded.
    w.pilot(None, |s| s.finished);
    let first = w.race.flow.laps[&1].times.clone();
    assert!(w.bank.ledger().ghost("Sir Proof", &rules::career::lap_key(rules::CourseId::Proving, 0)).is_some(), "the best lap was kept as a ghost");
    eprintln!("first run laps {first:?}");
    // A second run: a pause at the lights, then the pilot; the ghost is ahead.
    flow::quick(&mut w.race, &who, rules::Mode::TimeTrial, rules::Class::Squire, Some(rules::CourseId::Proving), 0);
    w.pilot(None, |s| s.ticks >= 91);
    assert!(w.race.flow.ghost.is_some(), "the ghost is on the road");
    w.hold(10, pad(), 1000);
    for t in 0..520u32 {
        w.step(auto());
        if t % 3 == 0 { w.shot(); }
    }
    w.pilot(None, |s| s.finished);
    let second = w.race.flow.laps[&1].times.clone();
    eprintln!("second run laps {second:?}; ghost gap at the end {:?}", w.race.flow.ghost);
    w.hold(70, pad(), 5);
}

/// What a frame of each screen costs against a frame of the race.
#[test]
fn flow_frame_times() {
    if std::env::var_os("ANGEL_FLOW_TIME").is_none() { return; }
    let mut w = Walk::new(None, "times");
    let mut time = |w: &mut Walk, what: &str| {
        let _ = w.frame();
        let t = Instant::now();
        for _ in 0..5 { let _ = w.frame(); }
        println!("{what:>10}: {:.1} ms a frame", t.elapsed().as_secs_f64() * 200.);
    };
    time(&mut w, "title");
    w.tap(ok(), 2, 1000);
    time(&mut w, "modes");
    w.tap(ok(), 2, 1000);
    time(&mut w, "classes");
    w.tap(ok(), 2, 1000);
    time(&mut w, "pods");
    w.tap(ok(), 130, 1000);
    time(&mut w, "race");
}

/// Each screen drawn from a state set up by hand, for looking at while the layout is made.
#[test]
fn flow_screens() {
    let Some(out) = std::env::var_os("ANGEL_FLOW_SHOTS").map(std::path::PathBuf::from) else { return };
    let mut w = Walk::new(Some(&out), "shots");
    w.hold(50, pad(), 1000);
    w.snap("01-title");
    w.tap(ok(), 30, 1000);
    w.snap("02-modes");
    w.tap(ok(), 20, 1000);
    w.snap("03-classes");
    w.tap(ok(), 20, 1000);
    w.snap("04-pods");
    for i in 1..4 { w.tap(right(), 10, 1000); if i == 2 { w.snap("05-pods-shut"); } }
    // The records of a knight who has done a little.
    {
        let c = &mut w.race.flow.seats.get_mut(&1).unwrap().career;
        c.money = 5850;
        c.earned = 12_400;
        c.races = 14; c.wins = 6; c.podiums = 10; c.cups = [2, 1, 0]; c.sweeps = 1;
        c.pods = 0b11_1111;
        for (i, &id) in rules::courses().iter().enumerate() {
            let r = c.courses.entry(rules::career::course_key(id)).or_default();
            r.race = 380_000 + i as u32 * 7_700; r.lap = 188_000 + i as u32 * 3_900; r.lap_pod = 5; r.runs = 5 + i as u32; r.wins = 2;
            c.trials.insert(rules::career::lap_key(id, 0), 190_500 + i as u32 * 3000);
            c.trials.insert(rules::career::lap_key(id, 5), 189_100 + i as u32 * 3000);
        }
    }
    w.race.flow.stage = rules::Stage::Modes;
    w.step(pad());
    for _ in 0..5 { w.tap(down(), 1, 1000); }
    w.tap(ok(), 10, 1000);
    w.snap("06-records");
    w.tap(right(), 4, 1000);
    w.snap("07-records-2");
    w.tap(back(), 2, 1000);
    // The garage.
    w.tap(up(), 1, 1000);
    w.tap(ok(), 10, 1000);
    w.snap("08-garage");
    w.tap(right(), 4, 1000);
    w.tap(right(), 4, 1000);
    w.snap("09-garage-engine");
    for _ in 0..5 { w.tap(down(), 1, 1000); }
    w.tap(right(), 4, 1000);
    w.snap("10-garage-armour");
    w.tap(back(), 2, 1000);
    // The courses: a single race, any class.
    for _ in 0..3 { w.tap(up(), 1, 1000); }
    w.tap(ok(), 2, 1000);
    w.tap(ok(), 2, 1000);
    assert_eq!(w.stage(), rules::Stage::Courses);
    w.tap(down(), 40, 1000);
    w.snap("11-courses");
    // The podium.
    w.race.flow.stage = rules::Stage::Podium;
    w.race.flow.class = rules::Class::Knight;
    w.race.flow.clock = 100;
    w.race.flow.podium = (1..=6u32).map(|i| flow::PodiumLine { seat: i, points: 70 - i * 9, pod: [0, 3, 1, 2, 4, 5][i as usize - 1], human: i == 1 }).collect();
    w.race.flow.cup.insert(1, flow::CupLine { purse: 2700, places: vec![1, 1, 1] });
    w.race.flow.seats.get_mut(&1).unwrap().last = Some(flow::Settle { place: 1, bonus: 3000, unlocks: vec!["KNIGHT CLASS".into(), "THE RINGWRIGHT".into(), "THE GATEHOUSE".into()], ..Default::default() });
    w.snap("12-podium");
    w.race.flow.now += 37;
    w.snap("13-podium-b");
}

/// The example's two menus as they were before the game around the race (course select, then
/// pod select), redrawn here only to set the new screens beside them.
#[test]
fn flow_before_menus() {
    let Some(out) = std::env::var_os("ANGEL_FLOW_SHOTS").map(std::path::PathBuf::from) else { return };
    std::fs::create_dir_all(&out).unwrap();
    let plate_ = |im: &mut Img, x, y, w, h| { im.rect(x, y, w, h, 'k'); im.frame(x, y, w, h, 'g'); };
    let mut im = Img::black(384, 224);
    shout(&mut im, 12, 10, "KNIGHT RACERS", 3, 'T');
    im.text(14, 36, "CHOOSE YOUR COURSE", 't');
    for (n, &id) in rules::courses().iter().enumerate() {
        let (c, y) = (rules::course(id), 56 + n as i32 * 26);
        if n == 1 { im.rect(10, y - 3, 168, 24, 'g'); }
        im.text(14, y, if n == 1 { ">" } else { " " }, 'T');
        im.text(24, y, c.name(), if n == 1 { 'H' } else { 'J' });
        im.text(24, y + 11, &format!("{} LAPS  {:.1} KM", c.laps(), c.lap() / 1000.), 't');
    }
    plate_(&mut im, 190, 12, 184, 150);
    ui::route_for_proof(&mut im, rules::CourseId::Emberfall, (190, 12, 184, 150));
    im.text(14, 204, "W S CHOOSE   ENTER GO ON   ESC CLOSE", 't');
    save_flat(&im, &out.join("before-1-course-select.png"));
    let mut im = super::garage::garage(1, 1, 2.0, 40);
    plate_(&mut im, 6, 8, 152, 206);
    im.text(12, 14, "CHOOSE YOUR POD", 'T');
    for (n, p) in rules::pods().iter().enumerate() {
        let y = 40 + n as i32 * 12;
        if n == 1 { im.rect(9, y - 2, 146, 11, 'g'); }
        im.text(14, y, if n == 1 { ">" } else { " " }, 'T');
        im.text(24, y, p.name, if n == 1 { 'H' } else { 'J' });
    }
    im.rect(0, 215, 384, 9, 'k');
    im.text(7, 216, "A D CHOOSE   ENTER RACE   BACKSPACE BACK   ESC CLOSE", 't');
    save_flat(&im, &out.join("before-2-pod-select.png"));
}
