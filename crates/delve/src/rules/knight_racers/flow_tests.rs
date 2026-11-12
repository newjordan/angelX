//! The game around the race: what a place pays, what a part does, the menus, the cup to its
//! podium, the time trial, and that a seat that joins a race by other means still races.
use super::*;
use crate::rules::knight_racers::career::{self, Career, ShopError};
use crate::rules::knight_racers::pods;

fn knight() -> Vec<(u32, String)> { vec![(1, "Sir Test".to_string())] }

fn pad() -> Pad { Pad::default() }
fn ok() -> Pad { Pad { ok: true, ..pad() } }
fn up() -> Pad { Pad { y: -1, ..pad() } }
fn down() -> Pad { Pad { y: 1, ..pad() } }
fn back() -> Pad { Pad { back: true, ..pad() } }

/// One tick with `p` held by seat 1, then one with nothing: a tap.
fn tap(s: &mut Session, who: &[(u32, String)], p: Pad) {
    tick(s, who, &[(1, p)].into());
    tick(s, who, &BTreeMap::new());
}

fn idle(s: &mut Session, who: &[(u32, String)], n: u32) { for _ in 0..n { tick(s, who, &BTreeMap::new()); } }

/// Race on with the CPU pilot at seat 1's wheel until the flag falls for all.
fn run(s: &mut Session, who: &[(u32, String)]) {
    let auto = [(1, Pad { auto: true, ..pad() })].into();
    for _ in 0..40_000 { if s.finished { return; } tick(s, who, &auto); }
    panic!("the race did not finish: {:?}", s.cars.iter().map(|(i, c)| (i, c.d)).collect::<Vec<_>>());
}

/// From the title to the grid: a cup in the class `c`, in pod 0.
fn to_grid(s: &mut Session, who: &[(u32, String)], mode: usize, class: usize) {
    tap(s, who, ok());
    assert_eq!(s.flow.stage, Stage::Modes);
    for _ in 0..mode { tap(s, who, down()); }
    tap(s, who, ok());
    if mode < 2 {
        assert_eq!(s.flow.stage, Stage::Classes);
        for _ in 0..class { tap(s, who, down()); }
        tap(s, who, ok());
    }
    if s.flow.stage == Stage::Courses { tap(s, who, ok()); }
    assert_eq!(s.flow.stage, Stage::Pods);
    tap(s, who, ok());
    assert_eq!(s.flow.stage, Stage::Race);
}

fn fresh_gate() -> Session {
    let mut s = Session::gate_of([1]);
    // As the garage's bank would have given it: a career with the first class open.
    tick(&mut s, &knight(), &BTreeMap::new());
    s.flow.seats.get_mut(&1).unwrap().fresh = false;
    s
}

#[test]
fn a_place_pays_by_class_and_mode_and_the_tables_only_go_down() {
    assert_eq!(career::purse(Mode::Cup, Class::Squire, 1), 900);
    assert_eq!(career::purse(Mode::Cup, Class::Squire, 6), 80);
    assert_eq!(career::purse(Mode::Cup, Class::Champion, 1), 2400);
    assert_eq!(career::purse(Mode::Single, Class::Knight, 1), 1000, "a single race pays two thirds of a cup race");
    assert_eq!(career::purse(Mode::Practice, Class::Champion, 1), 0);
    assert_eq!(career::purse(Mode::TimeTrial, Class::Champion, 1), 0);
    for c in Class::ALL {
        for p in 1..6 {
            assert!(career::purse(Mode::Cup, c, p) > career::purse(Mode::Cup, c, p + 1));
            assert!(career::cup_purse(c, p) > career::cup_purse(c, p + 1));
        }
    }
    for p in 1..=6 {
        assert!(career::purse(Mode::Cup, Class::Knight, p) > career::purse(Mode::Cup, Class::Squire, p));
        assert!(career::purse(Mode::Cup, Class::Champion, p) > career::purse(Mode::Cup, Class::Knight, p));
    }
    assert_eq!(career::purse(Mode::Cup, Class::Squire, 99), 80, "a place past sixth pays sixth");
}

#[test]
fn a_part_is_paid_for_whole_or_not_at_all_and_an_owned_one_refits_free() {
    let mut c = Career::default();
    assert_eq!(c.money, career::START_MONEY);
    assert_eq!(c.buy_and_fit("scale_mail"), Err(ShopError::Poor(700)), "1100 against 400");
    assert_eq!(c.money, career::START_MONEY, "a refused sale costs nothing");
    assert_eq!(c.worn(5), "bare_plate");
    let b = c.buy_and_fit("gale_fins").unwrap();
    assert_eq!((b.price, b.fitted), (400, true));
    assert_eq!((c.money, c.worn(4)), (0, "gale_fins"));
    assert!(c.owns("gale_fins") && c.owns("bare_crest") && !c.owns("pennon_mast"));
    // Back to the bare cowl for nothing, and the fins again for nothing.
    assert_eq!(c.buy_and_fit("bare_crest").unwrap().price, 0);
    let again = c.buy_and_fit("gale_fins").unwrap();
    assert_eq!((again.price, again.fitted, c.money), (0, true, 0));
    assert_eq!(c.buy_and_fit("gale_fins").unwrap().fitted, false, "already on");
    assert_eq!(c.buy_and_fit("sun_gold"), Err(ShopError::Unknown), "paint is not sold here");
    assert_eq!(c.buy_and_fit("nonsense"), Err(ShopError::Unknown));
    c.pay(5000);
    assert_eq!((c.money, c.earned), (5000, 5000));
}

#[test]
fn a_knight_with_nothing_bought_races_the_pod_as_it_comes_and_parts_move_the_numbers() {
    let stock = Career::default();
    assert!(stock.tune().is_stock(), "Excalibur tunes nothing");
    let warden = pods()[0];
    let same = warden.tuned(&stock.tune());
    assert_eq!((same.top, same.boost, same.accel, same.grip, same.mass), (warden.top, warden.boost, warden.accel, warden.grip, warden.mass));
    let mut fast = Career::default();
    fast.pay(10_000);
    fast.buy_and_fit("wraith_needle").unwrap();
    assert!(fast.tune().top > 1. && fast.tune().top < 1.1, "a faster engine is a few percent: {:?}", fast.tune());
    let mut plated = Career::default();
    plated.pay(10_000);
    plated.buy_and_fit("bastion_plate").unwrap();
    assert!(plated.tune().mass > 1.1, "plate makes a pod harder to stop: {:?}", plated.tune());
    assert!(plated.tune().top < 1., "and slower: weight is taxed off the speed");
    // Whatever a build adds up to, the sim's limits hold.
    let mut all = Career::default();
    all.pay(100_000);
    for slot in 0..career::WORKING_SLOTS { for w in career::wares(slot) { all.buy_and_fit(w.id).unwrap(); } }
    let t = all.tune();
    for v in [t.top, t.boost, t.accel, t.grip, t.mass] { assert!((0.7..=1.3).contains(&v)); }
    assert!(warden.tuned(&t).boost * 1.0 < 72., "the fastest build stays under the speed the sim allows");
}

#[test]
fn a_gate_opens_on_the_title_and_a_plain_session_is_a_race() {
    assert_eq!(Session::blank().flow.stage, Stage::Race, "a session made by anything else is a race with no menus");
    let s = Session::gate_of([1]);
    assert_eq!(s.flow.stage, Stage::Title);
    assert!(s.flow.attracting());
}

#[test]
fn the_menus_walk_from_the_title_to_a_race_and_back_out() {
    let who = knight();
    let mut s = fresh_gate();
    // Behind the menus the CPU race runs.
    let before = s.cars[&1].d;
    idle(&mut s, &who, 120);
    assert!(s.cars[&1].d > before + 50., "the attract race is under way");
    tap(&mut s, &who, ok());
    assert_eq!(s.flow.stage, Stage::Modes);
    // The cursor wraps, and back goes up a step.
    tap(&mut s, &who, up());
    assert_eq!(s.flow.seats[&1].cursor, 5);
    tap(&mut s, &who, down());
    assert_eq!(s.flow.seats[&1].cursor, 0);
    tap(&mut s, &who, ok());
    assert_eq!((s.flow.stage, s.flow.mode), (Stage::Classes, Mode::Cup));
    tap(&mut s, &who, back());
    assert_eq!(s.flow.stage, Stage::Modes);
    tap(&mut s, &who, ok());
    // Knight and Champion are locked to a new knight.
    tap(&mut s, &who, down());
    tap(&mut s, &who, ok());
    assert_eq!(s.flow.stage, Stage::Classes);
    assert!(s.flow.seats[&1].note.starts_with("LOCKED"), "{}", s.flow.seats[&1].note);
    tap(&mut s, &who, up());
    tap(&mut s, &who, ok());
    assert_eq!(s.flow.stage, Stage::Pods);
    // A pod not yet won stays shut.
    tap(&mut s, &who, down());
    assert_eq!(s.flow.seats[&1].pick, 1);
    tap(&mut s, &who, ok());
    assert_eq!(s.flow.stage, Stage::Pods);
    assert!(s.flow.seats[&1].note.starts_with("LOCKED"));
    tap(&mut s, &who, up());
    tap(&mut s, &who, ok());
    assert_eq!(s.flow.stage, Stage::Race);
    assert_eq!((s.ticks <= 1, s.course, s.round), (true, cup()[0], 0), "the grid, the first course of the cup");
    assert_eq!(s.cars.len(), 6, "a lone knight has five rivals");
    assert_eq!(s.cars.values().filter(|c| c.cpu).count(), 5);
    assert!(s.cars[&2].tune == Class::Squire.rivals(), "the rivals race in the class chosen");
    assert!(s.cars[&1].tune.is_stock(), "the knight's own parts, none bought");
}

#[test]
fn a_single_race_goes_by_the_course_picked_and_a_practice_run_is_the_proving_run() {
    let who = knight();
    let mut s = fresh_gate();
    tap(&mut s, &who, ok());
    tap(&mut s, &who, down());
    tap(&mut s, &who, ok());
    tap(&mut s, &who, ok()); // Squire
    assert_eq!(s.flow.stage, Stage::Courses);
    // The course under the cursor is the race behind the menu.
    tap(&mut s, &who, down());
    tap(&mut s, &who, down());
    assert_eq!(s.course, CourseId::Moatwalk);
    tap(&mut s, &who, ok());
    tap(&mut s, &who, ok());
    assert_eq!((s.flow.stage, s.course, s.flow.mode), (Stage::Race, CourseId::Moatwalk, Mode::Single));
    // Practice.
    let mut s = fresh_gate();
    to_grid(&mut s, &who, 3, 0);
    assert_eq!((s.course, s.flow.mode), (CourseId::Proving, Mode::Practice));
}

#[test]
fn a_time_trial_is_alone_on_the_road() {
    let who = knight();
    let mut s = fresh_gate();
    to_grid(&mut s, &who, 2, 0);
    assert_eq!(s.flow.mode, Mode::TimeTrial);
    assert_eq!(s.cars.len(), 1, "no rivals in a trial");
    idle(&mut s, &who, 200);
    assert_eq!(s.cars.len(), 1, "and none come");
    assert_eq!(s.flow.trial(&s).map(|t| t.0), Some(1));
}

#[test]
fn only_the_host_chooses_the_race_and_every_seat_has_its_own_garage() {
    let who: Vec<(u32, String)> = vec![(1, "Host".into()), (2, "Guest".into())];
    let mut s = Session::gate_of([1, 2]);
    let tap2 = |s: &mut Session, p: Pad| {
        tick(s, &who, &[(2, p)].into());
        tick(s, &who, &BTreeMap::new());
    };
    for (_, sf) in s.flow.seats.iter_mut() { sf.fresh = false; }
    tick(&mut s, &who, &BTreeMap::new());
    tap(&mut s, &who, ok());
    assert_eq!(s.flow.stage, Stage::Modes);
    tap2(&mut s, ok());
    assert_eq!(s.flow.stage, Stage::Modes, "the guest's pick starts nothing");
    assert!(s.flow.seats[&2].note.contains("HOST"));
    // The guest opens its own garage; the host's screen stays where it was.
    for _ in 0..4 { tap2(&mut s, down()); }
    tap2(&mut s, ok());
    assert_eq!(s.flow.seats[&2].local, Local::Garage);
    assert_eq!(s.flow.seats[&1].local, Local::None);
    // A trial is run alone.
    for _ in 0..2 { tap(&mut s, &who, down()); }
    tap(&mut s, &who, ok());
    assert_eq!(s.flow.stage, Stage::Modes);
    assert!(s.flow.seats[&1].note.contains("ALONE"));
    // The cup, for both: nobody to race but each other.
    tap(&mut s, &who, down());
    tap(&mut s, &who, down());
    tap(&mut s, &who, down());
    tap(&mut s, &who, ok());
    assert_eq!(s.flow.stage, Stage::Modes, "{:?}", s.flow.stage);
}

#[test]
fn the_garage_sells_and_fits_with_the_menu_keys_and_says_so() {
    let who = knight();
    let mut s = fresh_gate();
    tap(&mut s, &who, ok());
    for _ in 0..4 { tap(&mut s, &who, down()); }
    tap(&mut s, &who, ok());
    assert_eq!(s.flow.seats[&1].local, Local::Garage);
    // Engines: Gryphon Six (worn) first; right to Crown Cathedral (500) is more than 400 gold.
    tap(&mut s, &who, Pad { x: 1, ..pad() });
    tap(&mut s, &who, ok());
    assert!(s.flow.seats[&1].note.contains("GOLD SHORT"), "{}", s.flow.seats[&1].note);
    assert_eq!(s.flow.seats[&1].career.money, 400);
    // The crests: Pennon Mast (150) is in reach.
    for _ in 0..4 { tap(&mut s, &who, down()); }
    assert_eq!(s.flow.seats[&1].cursor, 4);
    tap(&mut s, &who, Pad { x: 1, ..pad() });
    assert_eq!(career::wares(4)[s.flow.seats[&1].sub as usize].id, "pennon_mast");
    tap(&mut s, &who, ok());
    let sf = &s.flow.seats[&1];
    assert_eq!((sf.career.money, sf.career.worn(4), sf.dirty), (250, "pennon_mast", true));
    assert!(sf.note.contains("BOUGHT"));
    tap(&mut s, &who, back());
    assert_eq!(s.flow.seats[&1].local, Local::None);
    assert_eq!(s.flow.stage, Stage::Modes);
}

#[test]
fn a_short_tap_of_escape_is_a_step_back_and_a_long_hold_is_not() {
    let who = knight();
    let mut s = fresh_gate();
    tap(&mut s, &who, ok());
    tap(&mut s, &who, ok());
    assert_eq!(s.flow.stage, Stage::Classes);
    let held = Pad { leave: true, ..pad() };
    for _ in 0..5 { tick(&mut s, &who, &[(1, held)].into()); }
    tick(&mut s, &who, &BTreeMap::new());
    assert_eq!(s.flow.stage, Stage::Modes, "a tap goes back");
    tap(&mut s, &who, ok());
    for _ in 0..20 { tick(&mut s, &who, &[(1, held)].into()); }
    tick(&mut s, &who, &BTreeMap::new());
    assert_eq!(s.flow.stage, Stage::Classes, "a hold is the realm's leave, not a step back");
}

/// Race a whole cup with the pilot at the wheel, set the points so the knight wins it, and see
/// the prizes, the podium and what winning opens.
#[test]
fn a_cup_runs_its_three_courses_pays_each_and_ends_on_a_podium_that_opens_the_next_class_and_a_pod() {
    let who = knight();
    let mut s = fresh_gate();
    to_grid(&mut s, &who, 0, 0);
    let mut paid = 0;
    for round in 0..cup().len() {
        assert_eq!((s.course, s.round as usize), (cup()[round], round));
        run(&mut s, &who);
        assert!(s.flow.settled);
        let sf = &s.flow.seats[&1];
        let last = sf.last.clone().expect("the race settled");
        assert_eq!(last.place, s.place(1));
        assert_eq!(last.purse, career::purse(Mode::Cup, Class::Squire, last.place));
        paid += last.purse;
        assert_eq!(sf.career.money, career::START_MONEY + paid, "paid once, not once a tick");
        assert!(sf.career.laps.values().all(|&l| l > 20_000), "lap times are kept in ms");
        assert_eq!(sf.career.races as usize, round + 1);
        assert!(sf.dirty);
        idle(&mut s, &who, 10);
        assert_eq!(s.flow.seats[&1].career.money, career::START_MONEY + paid);
        if round + 1 == cup().len() {
            // The knight takes the cup, whatever the pilot managed.
            s.points.insert(1, 999);
        }
        idle(&mut s, &who, 40);
        tap(&mut s, &who, ok());
        if round + 1 < cup().len() {
            assert_eq!((s.flow.stage, s.round as usize, s.ticks <= 1), (Stage::Race, round + 1, true), "the next course, on the grid");
            assert!(s.points.values().sum::<u32>() > 0, "the points carry from race to race");
        }
    }
    assert_eq!(s.flow.stage, Stage::Podium);
    assert_eq!(s.flow.podium[0].seat, 1);
    let sf = &s.flow.seats[&1];
    let last = sf.last.clone().unwrap();
    assert_eq!(last.bonus, career::cup_purse(Class::Squire, 1));
    assert_eq!(sf.career.money, career::START_MONEY + paid + last.bonus);
    assert_eq!(sf.career.cups, [1, 0, 0]);
    assert_eq!(sf.career.classes, 2, "the Knight class is open");
    assert!(sf.career.owns_pod(2), "the Ringwright is won");
    // The cup opens the class and the pod; a pilot who won every race of it (the CPU pilot does, in
    // the Squire class) opens the Gatehouse as well.
    assert_eq!(last.unlocks[..2], ["KNIGHT CLASS".to_string(), "THE RINGWRIGHT".to_string()]);
    let swept = s.flow.cup[&1].places.iter().all(|&p| p == 1);
    assert_eq!(last.unlocks.len(), if swept { 3 } else { 2 });
    assert_eq!(sf.career.owns_pod(career::SWEEP_POD), swept);
    // The podium waits a moment, then goes back to the menus; the cup is over.
    idle(&mut s, &who, 40);
    tap(&mut s, &who, ok());
    assert_eq!(s.flow.stage, Stage::Modes);
    // A second cup win opens nothing more.
    assert!(!s.flow.seats.get_mut(&1).unwrap().career.unlock_pod(2));
}

#[test]
fn winning_every_race_of_a_cup_opens_the_gatehouse_and_the_champion_cup_the_lancewain() {
    let mut c = Career::default();
    assert_eq!(c.open_pods(), vec![0, 5]);
    assert!(c.class_open(Class::Squire) && !c.class_open(Class::Knight));
    assert!(c.open_class(Class::Knight) && !c.open_class(Class::Knight));
    assert!(c.class_open(Class::Knight) && !c.class_open(Class::Champion));
    assert!(c.unlock_pod(career::SWEEP_POD) && !c.unlock_pod(career::SWEEP_POD));
    assert_eq!(career::cup_unlock(Class::Squire), (Some(Class::Knight), Some(2)));
    assert_eq!(career::cup_unlock(Class::Knight), (Some(Class::Champion), Some(1)));
    assert_eq!(career::cup_unlock(Class::Champion), (None, Some(3)));
    let all: u8 = [Class::Squire, Class::Knight, Class::Champion].iter().filter_map(|&k| career::cup_unlock(k).1).fold(1 << career::SWEEP_POD | 0b10_0001, |m, p| m | 1 << p);
    assert_eq!(all, 0b11_1111, "the six pods are all to be won");
}

#[test]
fn records_keep_the_best_race_and_lap_in_milliseconds_and_say_when_one_falls() {
    let mut c = Career::default();
    let n = c.note_race(CourseId::Emberfall, 0, 3, 400.5, Some(200.25));
    assert!(!n.new_race && !n.new_lap && n.first_lap, "a first time is not a record broken");
    let rec = c.courses["emberfall"];
    assert_eq!((rec.race, rec.lap, rec.runs, rec.wins), (400_500, 200_250, 1, 0));
    let n = c.note_race(CourseId::Emberfall, 0, 1, 395., Some(201.));
    assert!(n.new_race && !n.new_lap);
    let n = c.note_race(CourseId::Emberfall, 0, 1, 399., Some(199.9));
    assert!(!n.new_race && n.new_lap);
    assert_eq!(c.laps["emberfall:0"], 199_900);
    // A lap in another pod is its own record.
    let n = c.note_race(CourseId::Emberfall, 5, 2, 410., Some(205.));
    assert!(n.first_lap && !n.new_lap);
    assert_eq!((c.races, c.wins, c.podiums), (4, 2, 4));
    assert!(c.note_trial(CourseId::Glassway, 0, 190.) && !c.note_trial(CourseId::Glassway, 0, 191.) && c.note_trial(CourseId::Glassway, 0, 189.));
    assert_eq!(c.trials["glassway:0"], 189_000);
}

#[test]
fn a_career_round_trips_through_json_and_an_old_save_without_it_loads() {
    let mut c = Career::default();
    c.pay(1234);
    c.buy_and_fit("war_wheels").unwrap();
    c.unlock_pod(2);
    c.note_race(CourseId::Moatwalk, 2, 1, 410., Some(200.));
    let json = serde_json::to_string(&c).unwrap();
    assert!(!json.contains("build"), "what is worn is the ledger's build, not the career's");
    let mut back: Career = serde_json::from_str(&json).unwrap();
    back.build = c.build.clone();
    assert_eq!(back, c);
    let old: Career = serde_json::from_str("{}").unwrap();
    assert_eq!((old.money, old.pods, old.classes), (career::START_MONEY, career::START_PODS, 1));
    let weird: Career = serde_json::from_str(r#"{"money": 5, "someday": [1, 2], "courses": {"x": {"race": 9}}}"#).unwrap();
    assert_eq!((weird.money, weird.courses["x"].race), (5, 9));
}

#[test]
fn the_whole_flow_survives_the_wire_the_hot_engine_gives_the_run() {
    let who = knight();
    let mut s = fresh_gate();
    to_grid(&mut s, &who, 0, 0);
    idle(&mut s, &who, 200);
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.len() < 20_000, "a session on the wire is small: {} bytes", json.len());
    let mut back: Session = serde_json::from_str(&json).unwrap();
    assert!(back.valid());
    assert_eq!((back.flow.stage, back.flow.mode, back.ticks), (s.flow.stage, s.flow.mode, s.ticks));
    // Both go on the same way.
    idle(&mut s, &who, 60);
    idle(&mut back, &who, 60);
    assert_eq!(serde_json::to_string(&s).unwrap(), serde_json::to_string(&back).unwrap());
}

#[test]
fn a_session_from_before_the_flow_existed_loads_and_races() {
    let mut s = Session::blank();
    s.join_ids([1]);
    let mut v: serde_json::Value = serde_json::to_value(&s).unwrap();
    v.as_object_mut().unwrap().remove("flow");
    for car in v["cars"].as_object_mut().unwrap().values_mut() { car.as_object_mut().unwrap().remove("tune"); }
    let old: Session = serde_json::from_value(v).unwrap();
    assert_eq!(old.flow.stage, Stage::Race);
    assert!(old.cars[&1].tune.is_stock());
}

/// A measurement, not a gate: how a stock Warden under the CPU pilot places against each class.
/// `cargo test -p angel-delve --profile fast --lib class_calibration -- --ignored --nocapture`
#[test]
#[ignore]
fn class_calibration() {
    for class in Class::ALL {
        for &id in cup() {
            let mut places = Vec::new();
            for pod in [0u8, 5] {
                let mut s = Session::blank_on(id);
                s.join_ids([1]);
                s.fill_rivals(1);
                s.cars.get_mut(&1).unwrap().pod = pod;
                for (seat, c) in s.cars.iter_mut() { if *seat != 1 { c.tune = class.rivals(); } }
                for _ in 0..40_000 { if s.finished { break; } let d = s.bot_drives_all(); s.step_drive(&d); }
                places.push(s.place(1));
            }
            println!("{:>9} {:?}: pilot in warden / wyvern places {:?}", class.name(), id, places);
        }
    }
}

/// How a clean human run (LAW's pilot with no slips, in a seat of its own, the rivals' band measured
/// from it) fares against five rivals in `tune`: one race for each of the six pod rotations round the
/// seats on each of the three courses (18), for each seat in `seats`. Returns (won, races, mean margin
/// when it won, mean place, mean time lost to the best rival when it lost).
fn class_races(tune: Tune, seats: &[u32]) -> (u32, u32, f32, f32, f32) {
    let (mut wins, mut races, mut margin, mut place, mut lost_by) = (0u32, 0u32, 0f32, 0f32, 0f32);
    for &me in seats {
        for rot in 0..6u8 {
            for &id in cup() {
                let mut s = Session::blank_on(id);
                s.join_ids(1..=6);
                for (seat, c) in s.cars.iter_mut() {
                    c.cpu = true;
                    c.pod = ((seat - 1) as u8 + rot) % 6;
                    if *seat != me { c.tune = tune; }
                }
                for _ in 0..60_000u32 {
                    let mut drives = s.bot_drives();
                    if s.cars[&me].finish.is_none() { s.cars.get_mut(&me).unwrap().cpu = false; drives.insert(me, s.drive_clean(me)); }
                    s.step_drive(&drives);
                    if s.finished { break; }
                }
                assert!(s.finished, "{id:?} did not finish with the rivals in {tune:?}: a pod is stuck");
                let mine = s.cars[&me].finish.unwrap();
                let best_rival = s.cars.iter().filter(|(n, _)| **n != me).map(|(_, c)| c.finish.unwrap()).fold(f32::MAX, f32::min);
                races += 1;
                place += s.place(me) as f32;
                if mine < best_rival { wins += 1; margin += best_rival - mine; } else { lost_by += mine - best_rival; }
            }
        }
    }
    (wins, races, margin / wins.max(1) as f32, place / races as f32, lost_by / (races - wins).max(1) as f32)
}

/// The classes against a clean human run: the Squire's field is beaten easily, the Knight's is beaten
/// most of the time, the Champion's is a fight the run can lose. And every race in every class runs to
/// the flag (a slow rival once fell into Moatwalk's chasm for ever).
#[test]
fn the_classes_are_an_easy_win_a_likely_win_and_a_fight_for_a_clean_run() {
    let wins: Vec<(u32, u32, f32)> = Class::ALL.iter().map(|k| { let r = class_races(k.rivals(), &[1]); (r.0, r.1, r.3) }).collect();
    println!("clean run wins: squire {}/{}, knight {}/{}, champion {}/{}", wins[0].0, wins[0].1, wins[1].0, wins[1].1, wins[2].0, wins[2].1);
    assert!(wins[0].0 as f32 >= wins[0].1 as f32 * 0.9, "Squire is not an easy win: {}/{}", wins[0].0, wins[0].1);
    assert!(wins[1].0 as f32 >= wins[1].1 as f32 * 0.55 && wins[1].0 as f32 <= wins[1].1 as f32 * 0.85, "Knight does not win most races: {}/{}", wins[1].0, wins[1].1);
    assert!(wins[2].0 as f32 >= wins[2].1 as f32 * 0.15 && wins[2].0 as f32 <= wins[2].1 as f32 * 0.5, "Champion is not a fight: {}/{}", wins[2].0, wins[2].1);
    assert!(wins[0].0 > wins[1].0 && wins[1].0 > wins[2].0, "each class is harder than the last");
}

/// The table, per class: `ANGEL_CLASS_SEATS=1,2,3` puts the clean run in more seats for a bigger
/// sample, `ANGEL_CLASS=KNIGHT ANGEL_CLASS_KNOBS=top,boost,accel,grip` tries other numbers for a class.
/// `cargo test -p angel-delve --profile fast --lib class_balance_table -- --ignored --nocapture`
#[test]
#[ignore]
fn class_balance_table() {
    let seats: Vec<u32> = std::env::var("ANGEL_CLASS_SEATS").map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_else(|_| vec![1]);
    for class in Class::ALL {
        let mut t = class.rivals();
        if std::env::var("ANGEL_CLASS").ok().as_deref() == Some(class.name()) {
            if let Ok(k) = std::env::var("ANGEL_CLASS_KNOBS") {
                let v: Vec<f32> = k.split(',').filter_map(|x| x.parse().ok()).collect();
                if v.len() == 4 { t = Tune { top: v[0], boost: v[1], accel: v[2], grip: v[3], mass: 1. }; }
            }
        }
        let (w, n, m, p, l) = class_races(t, &seats);
        println!("CLASS {:>9}  tune top {:.3} boost {:.3} accel {:.3} grip {:.3}: clean run won {w:>2} of {n}  by {m:>5.1} s when it won, mean place {p:.2}, lost by {l:>5.1} s when it lost", class.name(), t.top, t.boost, t.accel, t.grip);
    }
}
