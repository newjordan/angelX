use super::*;
use crate::drive::chivalry::{Mount, Stable};
use crate::drive::together_shooter::home::{BUY_HOLD, Home};
use crate::drive::together_shooter::world;

/// A party at the lists, the realm's stable as given.
fn at_the_lists(stable: Stable) -> Run {
    let home = Home {
        stable,
        ..Home::default()
    };
    let mut run = Run::at_home(7, 1, None, home, Default::default());
    let lists = world::room_of(&run.dungeon, RoomKind::Lists).unwrap();
    run.arrive(lists, (1.9, 8.5));
    run.step(&BTreeMap::new());
    run
}

fn keys(fire: bool, dash: bool, move_y: i8) -> BTreeMap<u32, Input> {
    BTreeMap::from([(
        1,
        Input {
            fire,
            dash,
            move_y,
            ..Input::default()
        },
    )])
}

fn mount(run: &mut Run) {
    for _ in 0..BUY_HOLD + 1 {
        run.step(&keys(true, false, 0));
    }
    run.step(&keys(false, false, 0));
    assert!(run.joust.is_some(), "mounted at the plate");
}

/// Ride one course from the end: spur, aim, strike at tick `strike` of the
/// charge and brace at `brace` (if any), and ride on until the next course
/// or the verdict.
fn ride(run: &mut Run, aim: Aim, strike: Option<u32>, brace: Option<u32>) -> (Hit, Hit) {
    assert_eq!(run.joust.as_ref().unwrap().stage, Stage::Ready);
    run.step(&keys(true, false, 0));
    run.step(&keys(false, false, 0));
    let course = run.joust.as_ref().unwrap().course;
    let mut hits = None;
    for _ in 0..600 {
        let Some(j) = run.joust.as_ref() else {
            break;
        };
        if j.course != course || j.stage != Stage::Charge {
            break;
        }
        hits = hits.or(j.hits);
        let next = j.t + 1;
        let fire = strike == Some(next);
        let dash = brace == Some(next);
        let dy = if aim == Aim::High { -1 } else { 1 };
        run.step(&keys(fire, dash, dy));
    }
    hits.expect("the riders met")
}

fn open(run: &Run) -> Aim {
    match run.joust.as_ref().unwrap().guard {
        Aim::High => Aim::Low,
        Aim::Low => Aim::High,
    }
}

fn meeting(run: &Run) -> u32 {
    run.joust.as_ref().unwrap().meeting()
}

#[test]
fn a_knight_mounts_at_the_lists_plate_against_the_first_rival() {
    let mut run = at_the_lists(Stable::default());
    mount(&mut run);
    let j = run.joust.as_ref().unwrap();
    assert_eq!(j.knight, 1);
    assert_eq!(j.rival().id, "kay");
    assert_eq!(j.mount, Mount::Bramble);
    assert_eq!(j.stage, Stage::Ready);
    assert!(run.marks.contains_key("joust:start"));
    // In the saddle at the west end of the near lane.
    let hero = &run.players[&1];
    assert!((hero.x - WEST_END).abs() < 0.01 && (hero.y - SOUTH_LANE).abs() < 0.01);
}

#[test]
fn the_riders_charge_faster_and_faster_and_meet_in_the_middle() {
    let mut run = at_the_lists(Stable::default());
    mount(&mut run);
    run.step(&keys(true, false, 0));
    let mut xs = Vec::new();
    while run.joust.as_ref().unwrap().t < meeting(&run) {
        xs.push(run.players[&1].x);
        run.step(&keys(false, false, 0));
    }
    let steps: Vec<f32> = xs.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(
        steps.windows(2).all(|w| w[1] >= w[0] - 1e-4),
        "accelerating"
    );
    let ((kx, _, _), (rx, _, _)) = run.joust.as_ref().unwrap().riders();
    assert!((kx - MEETING).abs() < 0.01 && (rx - MEETING).abs() < 0.01);
}

#[test]
fn a_strike_where_the_shield_isnt_lands_clean_and_at_the_moment_mightily() {
    let mut run = at_the_lists(Stable::default());
    mount(&mut run);
    let m = meeting(&run);
    let aim = open(&run);
    let (mine, _) = ride(&mut run, aim, Some(m), None);
    assert_eq!(mine, Hit::Smite);
    let j = run.joust.as_ref().unwrap();
    assert!(j.score.0 >= 3);
    // A good-but-not-perfect strike at the open target is clean.
    let mut run = at_the_lists(Stable::default());
    mount(&mut run);
    let m = meeting(&run);
    let w = Mount::Bramble.steed().strike;
    let aim = open(&run);
    let late = if aim == Aim::High { w * 2 / 3 } else { w };
    let (mine, _) = ride(&mut run, aim, Some(m + late), None);
    assert_eq!(mine, Hit::Clean);
}

#[test]
fn a_strike_on_his_shield_is_a_point_and_a_mistimed_one_misses() {
    let mut run = at_the_lists(Stable::default());
    mount(&mut run);
    let guard = run.joust.as_ref().unwrap().guard;
    let m = meeting(&run);
    let (mine, _) = ride(&mut run, guard, Some(m), None);
    assert_eq!(mine, Hit::Broke);
    let mut run = at_the_lists(Stable::default());
    mount(&mut run);
    let m = meeting(&run);
    let (mine, _) = ride(&mut run, Aim::Low, Some(m - 30), None);
    assert_eq!(mine, Hit::Miss);
    let mut run = at_the_lists(Stable::default());
    mount(&mut run);
    let (mine, _) = ride(&mut run, Aim::Low, None, None);
    assert_eq!(mine, Hit::Miss);
}

#[test]
fn the_mount_changes_the_timing_not_just_a_number() {
    // Nine ticks early: inside Mist's strike, outside Cinder's.
    for (m, lands) in [(Mount::Mist, true), (Mount::Cinder, false)] {
        let mut run = at_the_lists(Stable {
            selected: m,
            ..Stable::default()
        });
        mount(&mut run);
        let meet = meeting(&run);
        let guard = run.joust.as_ref().unwrap().guard;
        let (mine, _) = ride(&mut run, guard, Some(meet - 9), None);
        assert_eq!(mine != Hit::Miss, lands, "{m:?}");
    }
    // Cinder meets soonest, Bramble last.
    let charge = |m: Mount| m.steed().charge;
    assert!(charge(Mount::Cinder) < charge(Mount::Mist));
    assert!(charge(Mount::Mist) < charge(Mount::Bramble));
}

#[test]
fn a_brace_in_time_takes_his_lance_a_grade_lighter() {
    // Sir Lancelot's lance, braced or not: the same course, the same bout.
    let wins =
        std::collections::BTreeMap::from([("kay".to_string(), 1), ("palamedes".to_string(), 1)]);
    let stable = Stable {
        wins,
        ..Stable::default()
    };
    let mut bare = at_the_lists(stable.clone());
    mount(&mut bare);
    let m = meeting(&bare);
    let (_, unbraced) = ride(&mut bare, Aim::Low, None, None);
    let mut braced = at_the_lists(stable);
    mount(&mut braced);
    let (_, lighter) = ride(&mut braced, Aim::Low, None, Some(m));
    assert_eq!(braced.joust.as_ref().unwrap().rival().id, "lancelot");
    assert!(
        lighter.knocks() < unbraced.knocks(),
        "{lighter:?} vs {unbraced:?}"
    );
}

#[test]
fn a_tended_mount_carries_its_rider_a_knock_longer() {
    let tended = Stable {
        tended: [true, false, false],
        ..Stable::default()
    };
    let mut run = at_the_lists(tended);
    mount(&mut run);
    let j = run.joust.as_ref().unwrap();
    assert!(j.tended);
    assert_eq!(j.balance.0, Mount::Bramble.steed().balance + 1);
}

#[test]
fn unhorsing_the_rival_ends_the_bout_and_the_knight_dismounts() {
    let mut run = at_the_lists(Stable::default());
    mount(&mut run);
    // Sir Kay takes four knocks: two mighty hits put him on the sand.
    for _ in 0..2 {
        if run.joust.as_ref().unwrap().stage != Stage::Ready {
            break;
        }
        let m = meeting(&run);
        let aim = open(&run);
        ride(&mut run, aim, Some(m), None);
    }
    let j = run.joust.as_ref().unwrap();
    assert!(j.fallen.1, "Sir Kay is down");
    assert_eq!(j.stage, Stage::Done);
    assert_eq!(j.verdict, Some(Verdict::Won));
    assert!(run.marks.contains_key("joust:kay:won"));
    assert!(run.marks.contains_key("joust:unhorsed"));
    assert!(run.feats.contains(&"unhorsed"));
    for _ in 0..VERDICT_TICKS + 2 {
        run.step(&BTreeMap::new());
    }
    assert!(run.joust.is_none(), "down from the saddle");
    let hero = &run.players[&1];
    assert!(run.settlement_walkable_for_test(hero.x, hero.y));
}

#[test]
fn three_courses_without_a_fall_are_counted() {
    let mut run = at_the_lists(Stable::default());
    mount(&mut run);
    for _ in 0..3 {
        let guard = run.joust.as_ref().unwrap().guard;
        let m = meeting(&run);
        // Always on his shield, never quite at the moment: a point a course.
        let w = Mount::Bramble.steed().strike;
        ride(&mut run, guard, Some(m + w), Some(m));
    }
    let j = run.joust.as_ref().unwrap();
    assert_eq!(j.stage, Stage::Done);
    assert_eq!(j.score.0, 3);
    assert!(j.verdict.is_some());
}

#[test]
fn the_knight_in_the_saddle_is_carried_not_walked() {
    let mut run = at_the_lists(Stable::default());
    mount(&mut run);
    let before = run.players[&1].x;
    for _ in 0..20 {
        run.step(&BTreeMap::from([(
            1,
            Input {
                move_x: -1,
                ..Input::default()
            },
        )]));
    }
    assert_eq!(run.players[&1].x, before, "waiting at the end, keys or no");
}

#[test]
fn holding_space_at_the_end_withdraws_from_the_bout() {
    let mut run = at_the_lists(Stable::default());
    mount(&mut run);
    for _ in 0..WITHDRAW_HOLD + 2 {
        run.step(&keys(false, true, 0));
    }
    assert!(run.joust.is_none());
    assert!(
        !run.marks
            .keys()
            .any(|m| m.ends_with(":won") || m.ends_with(":lost"))
    );
}

#[test]
fn a_friends_mirror_sees_the_bout() {
    let mut host = at_the_lists(Stable::default());
    let mut friend: Run = serde_json::from_slice(&serde_json::to_vec(&host).unwrap()).unwrap();
    mount(&mut host);
    host.step(&keys(true, false, 0));
    for _ in 0..10 {
        host.step(&keys(false, false, 0));
    }
    assert!(friend.apply_live(host.live()));
    assert_eq!(friend.joust, host.joust);
}

#[test]
fn the_same_keys_ride_the_same_bout() {
    let play = || {
        let mut run = at_the_lists(Stable::default());
        mount(&mut run);
        let m = meeting(&run);
        ride(&mut run, Aim::High, Some(m - 2), Some(m + 1));
        serde_json::to_value(&run.joust).unwrap()
    };
    assert_eq!(play(), play());
}

#[test]
fn the_realms_stable_keeps_saddles_tending_and_bouts() {
    let mut stable = Stable::default();
    let marks = |list: &[&str]| -> BTreeMap<String, u32> {
        list.iter().map(|m| (m.to_string(), 1)).collect()
    };
    settle_stable(&mut stable, &marks(&["stable:select:cinder"]));
    assert_eq!(stable.selected, Mount::Cinder);
    settle_stable(&mut stable, &marks(&["stable:tend"]));
    assert!(stable.is_tended(Mount::Cinder));
    let again = settle_stable(&mut stable, &marks(&["stable:tend"]));
    assert!(again[0].1.contains("already"));
    settle_stable(&mut stable, &marks(&["joust:start"]));
    assert!(
        !stable.is_tended(Mount::Cinder),
        "a bout spends the tending"
    );
    assert_eq!(next_rival(&stable), 0);
    settle_stable(&mut stable, &marks(&["joust:kay:won", "joust:unhorsed"]));
    assert_eq!((stable.bouts, stable.unhorsed), (1, 1));
    assert_eq!(next_rival(&stable), 1, "Sir Palamedes next");
    settle_stable(&mut stable, &marks(&["joust:palamedes:lost"]));
    assert_eq!(stable.bouts, 2);
    assert_eq!(next_rival(&stable), 1);
}

#[test]
fn the_stall_plates_saddle_and_the_trough_tends() {
    let mut run = Run::at_home(7, 1, None, Home::default(), Default::default());
    let stables = world::room_of(&run.dungeon, RoomKind::Stables).unwrap();
    run.arrive(stables, (12.0, 9.0));
    for (plate, mark) in [
        (STALL_PLATES[1], "stable:select:cinder"),
        (TEND_PLATE, "stable:tend"),
    ] {
        let (c, r, w, h) = plate;
        let hero = run.players.get_mut(&1).unwrap();
        hero.x = (c as f32 + w as f32 / 2.0) * TILE_UNITS;
        hero.y = (r as f32 + h as f32 / 2.0) * TILE_UNITS;
        run.marks.clear();
        for _ in 0..BUY_HOLD + 1 {
            run.step(&keys(true, false, 0));
        }
        run.step(&keys(false, false, 0));
        assert!(run.marks.contains_key(mark), "{mark}");
    }
}
