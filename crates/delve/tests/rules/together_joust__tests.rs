use super::*;
use crate::realm::chivalry::{Mount, Stable};
use crate::rules::home::{BUY_HOLD, Home};
use crate::rules::world;

/// A party of `seats` at the lists with `gold` in the realm's purse: knight
/// 1 on the mount plate, knight 2 nearest to it, any more further off.
fn party(stable: Stable, gold: u32, seats: u32) -> Run {
    let home = Home {
        stable,
        ..Home::default()
    };
    let mut treasury = Spoils::default();
    treasury.add(Spoil::Gold, gold);
    let mut run = Run::at_home(7, 1, None, home, treasury);
    for id in 2..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    let lists = world::room_of(&run.dungeon, RoomKind::Lists).unwrap();
    run.arrive(lists, (1.9, 8.5));
    for id in 1..=seats {
        let h = run.players.get_mut(&id).unwrap();
        // Past the grace a knight joins with.
        h.invulnerable = 0;
        (h.x, h.y) = match id {
            1 => (1.9 * TILE_UNITS, 8.5 * TILE_UNITS),
            n => ((3.0 + 5.0 * n as f32) * TILE_UNITS / 2.0, 21.0),
        };
    }
    run.step(&BTreeMap::new());
    run
}

/// A lone knight at the lists.
fn at_the_lists(stable: Stable) -> Run {
    party(stable, 0, 1)
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

/// The interact key held, and nothing else: how a plate is pressed.
fn interact() -> BTreeMap<u32, Input> {
    BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Input::default()
        },
    )])
}

fn mount(run: &mut Run) {
    for _ in 0..BUY_HOLD + 1 {
        run.step(&interact());
    }
    run.step(&keys(false, false, 0));
    assert!(run.joust.is_some(), "mounted at the plate");
}

/// How a rider runs a pass: lance and shield (set early in the charge) and
/// the tick of the charge F strikes at.
#[derive(Clone, Copy)]
struct Plan {
    aim: Aim,
    shield: Aim,
    strike: Option<u32>,
}

fn plan(aim: Aim, shield: Aim, strike: Option<u32>) -> Plan {
    Plan {
        aim,
        shield,
        strike,
    }
}

/// One rider's keys this tick of the charge (`next` its tick), pressing E
/// to bring the shield round when it is wrong and E was up last tick.
fn plan_keys(p: Plan, shield_now: Aim, next: u32, e_down: &mut bool) -> Input {
    let e = shield_now != p.shield && !*e_down && next < 20;
    *e_down = e;
    Input {
        fire: p.strike == Some(next),
        move_y: if p.aim == Aim::High { -1 } else { 1 },
        interact: e,
        ..Input::default()
    }
}

/// Run one pass from the ends: everyone spurs, each rider rides their plan
/// (`b` for the friend in the far saddle, if any), on to the next pass or
/// the verdict. Returns what the lances did.
fn pass(run: &mut Run, a: Plan, b: Option<Plan>) -> (Hit, Hit) {
    assert_eq!(run.joust.as_ref().unwrap().stage, Stage::Ready);
    let foe = run.joust.as_ref().unwrap().foe.as_ref().map(|f| f.id);
    let spur = |fire| {
        let mut m = BTreeMap::from([(
            1,
            Input {
                fire,
                ..Input::default()
            },
        )]);
        if let Some(id) = foe {
            m.insert(
                id,
                Input {
                    fire,
                    ..Input::default()
                },
            );
        }
        m
    };
    run.step(&spur(true));
    assert_eq!(run.joust.as_ref().unwrap().stage, Stage::Charge, "off");
    run.step(&spur(false));
    let course = run.joust.as_ref().unwrap().course;
    let mut hits = None;
    let (mut ea, mut eb) = (false, false);
    for _ in 0..600 {
        let Some(j) = run.joust.as_ref() else {
            break;
        };
        if j.course != course || j.stage != Stage::Charge {
            break;
        }
        hits = hits.or(j.hits);
        let next = j.t + 1;
        let mut inputs = BTreeMap::from([(1, plan_keys(a, j.shield, next, &mut ea))]);
        if let (Some(id), Some(b), Some(f)) = (foe, b, j.foe.as_ref()) {
            inputs.insert(id, plan_keys(b, f.shield, next, &mut eb));
        }
        run.step(&inputs);
    }
    hits.or(run.joust.as_ref().and_then(|j| j.hits))
        .expect("the riders met")
}

/// A lone knight's pass against a champion, shield left low.
fn ride(run: &mut Run, aim: Aim, strike: Option<u32>) -> (Hit, Hit) {
    pass(run, plan(aim, Aim::Low, strike), None)
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

fn finish(run: &mut Run) {
    for _ in 0..VERDICT_TICKS + 2 {
        run.step(&BTreeMap::new());
    }
    assert!(run.joust.is_none(), "down from the saddle");
}

fn banked(run: &Run, id: u32) -> u32 {
    run.bank
        .iter()
        .filter(|h| h.hero == id)
        .map(|h| h.spoils.get(Spoil::Gold))
        .sum()
}

fn owed(run: &Run) -> u32 {
    run.marks.get(PAID_MARK).copied().unwrap_or(0)
}

// ── A lone knight: a champion rides against them ──────────────────────────

#[test]
fn a_lone_knight_mounts_at_the_lists_plate_against_the_first_champion() {
    let mut run = at_the_lists(Stable::default());
    mount(&mut run);
    let j = run.joust.as_ref().unwrap();
    assert_eq!(j.knight, 1);
    assert!(j.foe.is_none(), "no friend at the lists: a champion rides");
    assert_eq!(j.rival().id, "kay");
    assert_eq!(j.mount, Mount::Bramble);
    assert_eq!(j.stage, Stage::Ready);
    assert!(j.gallery.is_empty());
    assert!(run.marks.contains_key("joust:start"));
    // In the saddle at the west end of the near lane.
    let hero = &run.players[&1];
    assert!((hero.x - WEST_END).abs() < 0.01 && (hero.y - SOUTH_LANE).abs() < 0.01);
}

#[test]
fn a_friend_who_cannot_ride_leaves_the_champion_to_it() {
    let mut run = party(Stable::default(), 0, 2);
    run.players.get_mut(&2).unwrap().stone = true;
    mount(&mut run);
    let j = run.joust.as_ref().unwrap();
    assert!(j.foe.is_none());
    assert!(j.gallery.is_empty(), "a knight at vigil is not in the gallery");
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
    let (mine, _) = ride(&mut run, aim, Some(m));
    assert_eq!(mine, Hit::Smite);
    assert!(run.joust.as_ref().unwrap().score.0 >= 3);
    // A good-but-not-perfect strike at the open target is clean.
    let mut run = at_the_lists(Stable::default());
    mount(&mut run);
    let m = meeting(&run);
    let w = Mount::Bramble.steed().strike;
    let aim = open(&run);
    let late = if aim == Aim::High { w * 2 / 3 } else { w };
    let (mine, _) = ride(&mut run, aim, Some(m + late));
    assert_eq!(mine, Hit::Clean);
    assert_eq!(mine.flash(aim), if aim == Aim::High { "HELM" } else { "CHEST" });
}

#[test]
fn a_strike_on_his_shield_is_a_point_and_a_mistimed_one_misses() {
    let mut run = at_the_lists(Stable::default());
    mount(&mut run);
    let guard = run.joust.as_ref().unwrap().guard;
    let m = meeting(&run);
    let (mine, _) = ride(&mut run, guard, Some(m));
    assert_eq!(mine, Hit::Broke);
    let mut run = at_the_lists(Stable::default());
    mount(&mut run);
    let m = meeting(&run);
    let (mine, _) = ride(&mut run, Aim::Low, Some(m - 30));
    assert_eq!(mine, Hit::Miss);
    let mut run = at_the_lists(Stable::default());
    mount(&mut run);
    let (mine, _) = ride(&mut run, Aim::Low, None);
    assert_eq!(mine, Hit::Miss);
    assert_eq!(mine.flash(Aim::Low), "MISS");
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
        let (mine, _) = ride(&mut run, guard, Some(meet - 9));
        assert_eq!(mine != Hit::Miss, lands, "{m:?}");
    }
    // Cinder meets soonest, Bramble last.
    let charge = |m: Mount| m.steed().charge;
    assert!(charge(Mount::Cinder) < charge(Mount::Mist));
    assert!(charge(Mount::Mist) < charge(Mount::Bramble));
}

#[test]
fn a_shield_where_his_lance_points_takes_it_a_grade_lighter() {
    // Sir Lancelot's lance, met with the shield or not: the same course,
    // the same bout.
    let wins =
        std::collections::BTreeMap::from([("kay".to_string(), 1), ("palamedes".to_string(), 1)]);
    let stable = Stable {
        wins,
        ..Stable::default()
    };
    let mut run = at_the_lists(stable.clone());
    mount(&mut run);
    assert_eq!(run.joust.as_ref().unwrap().rival().id, "lancelot");
    let lance = run.joust.as_ref().unwrap().lance;
    let mut bare = at_the_lists(stable.clone());
    mount(&mut bare);
    let (_, unshielded) = pass(&mut bare, plan(Aim::Low, lance.other(), None), None);
    let mut covered = at_the_lists(stable);
    mount(&mut covered);
    let (_, lighter) = pass(&mut covered, plan(Aim::Low, lance, None), None);
    assert!(
        lighter.knocks() < unshielded.knocks(),
        "{lighter:?} vs {unshielded:?}"
    );
    assert!(matches!(lighter, Hit::Shield | Hit::Broke));
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
fn unhorsing_the_champion_ends_the_bout_and_the_knight_dismounts() {
    let mut run = at_the_lists(Stable::default());
    mount(&mut run);
    // Sir Kay takes four knocks: two mighty hits put him on the sand.
    for _ in 0..2 {
        if run.joust.as_ref().unwrap().stage != Stage::Ready {
            break;
        }
        let m = meeting(&run);
        let aim = open(&run);
        ride(&mut run, aim, Some(m));
    }
    let j = run.joust.as_ref().unwrap();
    assert!(j.fallen.1, "Sir Kay is down");
    assert_eq!(j.stage, Stage::Done);
    assert_eq!(j.verdict, Some(Verdict::Won));
    assert!(run.marks.contains_key("joust:kay:won"));
    assert!(run.marks.contains_key("joust:unhorsed"));
    assert!(run.feats.contains(&"unhorsed"));
    assert!(run.players[&1].laurel, "the laurel for beating a champion");
    finish(&mut run);
    let hero = &run.players[&1];
    assert!(run.settlement_walkable_for_test(hero.x, hero.y));
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
    let mut host = party(Stable::default(), 200, 3);
    mount(&mut host);
    assert!(host.joust.as_ref().unwrap().foe.is_some());
    host.step(&BTreeMap::from([(
        3,
        Input {
            move_x: -1,
            ..Input::default()
        },
    )]));
    host.step(&keys(true, false, 0));
    for _ in 0..10 {
        host.step(&keys(false, false, 0));
    }
    let friend = host.wire_copy();
    assert_eq!(friend.joust, host.joust);
}

#[test]
fn the_same_keys_ride_the_same_bout() {
    let play = || {
        let mut run = party(Stable::default(), 200, 2);
        mount(&mut run);
        let m = meeting(&run);
        pass(
            &mut run,
            plan(Aim::High, Aim::Low, Some(m - 2)),
            Some(plan(Aim::Low, Aim::High, Some(m + 1))),
        );
        serde_json::to_value(&run.joust).unwrap()
    };
    assert_eq!(play(), play());
}

// ── Two knights: head to head, the gallery wagering ───────────────────────

#[test]
fn the_nearest_friend_takes_the_far_saddle_and_the_rest_the_gallery() {
    let mut run = party(Stable::default(), 200, 4);
    mount(&mut run);
    let j = run.joust.as_ref().unwrap().clone();
    let foe = j.foe.as_ref().expect("a friend rides");
    assert_eq!(foe.id, 2, "the nearest friend");
    assert_eq!(foe.mount, Mount::Cinder, "the next horse along the stalls");
    assert_eq!(j.gallery.keys().copied().collect::<Vec<_>>(), vec![3, 4]);
    assert_eq!(j.seated(), vec![1, 2, 3, 4], "everybody plays");
    assert_eq!(j.purse, PURSE);
    let ((ax, ay, _), (bx, by, _)) = j.riders();
    assert_eq!((run.players[&1].x, run.players[&1].y), (ax, ay));
    assert_eq!((run.players[&2].x, run.players[&2].y), (bx, by));
    assert!((by - NORTH_LANE).abs() < 0.01 && (bx - EAST_END).abs() < 0.01);
    for (i, id) in [3, 4].into_iter().enumerate() {
        assert_eq!((run.players[&id].x, run.players[&id].y), GALLERY_SEATS[i]);
    }
    // The gallery's keys do nothing else while the bout is on.
    let before = (run.players[&3].x, run.players[&3].y);
    for _ in 0..10 {
        run.step(&BTreeMap::from([(
            3,
            Input {
                move_y: 1,
                ..Input::default()
            },
        )]));
    }
    assert_eq!((run.players[&3].x, run.players[&3].y), before);
    assert!(run.valid_snapshot());
}

#[test]
fn each_rider_picks_a_horse_at_their_end_before_the_first_pass() {
    let mut run = party(Stable::default(), 0, 2);
    mount(&mut run);
    let side = |x: i8| Input {
        move_x: x,
        ..Input::default()
    };
    run.step(&BTreeMap::from([(1, side(1)), (2, side(-1))]));
    let j = run.joust.as_ref().unwrap();
    assert_eq!(j.mount, Mount::Cinder);
    assert_eq!(j.foe.as_ref().unwrap().mount, Mount::Bramble);
    assert_eq!(j.balance, (Mount::Cinder.steed().balance, Mount::Bramble.steed().balance));
    // Held, it moves once.
    run.step(&BTreeMap::from([(1, side(1))]));
    assert_eq!(run.joust.as_ref().unwrap().mount, Mount::Cinder);
    // The faster horse meets sooner, and further down the lists.
    let j = run.joust.as_ref().unwrap();
    assert!(j.meeting() < Mount::Bramble.steed().charge);
    assert!(j.meeting() > Mount::Cinder.steed().charge);
    assert!(j.meeting_x() > MEETING, "Cinder runs further from the west");
}

#[test]
fn the_flag_drops_when_both_have_spurred_or_the_herald_tires() {
    let mut run = party(Stable::default(), 0, 2);
    mount(&mut run);
    run.step(&keys(true, false, 0));
    let j = run.joust.as_ref().unwrap();
    assert!(j.spurred && j.stage == Stage::Ready, "waiting for the friend");
    let spur2 = BTreeMap::from([(
        2,
        Input {
            fire: true,
            ..Input::default()
        },
    )]);
    run.step(&spur2);
    assert_eq!(run.joust.as_ref().unwrap().stage, Stage::Charge);
    // Nobody spurs: the herald drops the flag himself.
    let mut run = party(Stable::default(), 0, 2);
    mount(&mut run);
    for _ in 0..FIRST_FLAG_TICKS + 1 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.joust.as_ref().unwrap().stage, Stage::Charge);
}

#[test]
fn a_lance_on_the_shield_is_a_point_and_round_it_on_the_helm_is_two() {
    for (shield, want) in [(Aim::High, Hit::Shield), (Aim::Low, Hit::Clean)] {
        let mut run = party(Stable::default(), 0, 2);
        mount(&mut run);
        let m = meeting(&run);
        let late = Mount::Bramble.steed().strike * 2 / 3;
        let (mine, theirs) = pass(
            &mut run,
            plan(Aim::High, Aim::Low, Some(m + late)),
            Some(plan(Aim::Low, shield, None)),
        );
        assert_eq!(mine, want, "against a shield held {shield:?}");
        assert_eq!(theirs, Hit::Miss);
        let j = run.joust.as_ref().unwrap();
        assert_eq!(j.passes, (1, 0), "the better lance takes the pass");
        assert_eq!(j.course, 1);
    }
}

#[test]
fn lances_and_shields_are_set_at_the_couch() {
    let mut run = party(Stable::default(), 0, 2);
    mount(&mut run);
    let both = |fire| {
        let i = Input {
            fire,
            ..Input::default()
        };
        BTreeMap::from([(1, i), (2, i)])
    };
    run.step(&both(true));
    let m = meeting(&run);
    while run.joust.as_ref().unwrap().t + COUCH < m {
        run.step(&both(false));
    }
    assert!(!run.joust.as_ref().unwrap().unset());
    let late = Input {
        move_y: -1,
        interact: true,
        ..Input::default()
    };
    run.step(&BTreeMap::from([(1, late), (2, late)]));
    let j = run.joust.as_ref().unwrap();
    assert_eq!((j.aim, j.shield), (Aim::Low, Aim::Low), "too late to change");
    let f = j.foe.as_ref().unwrap();
    assert_eq!((f.aim, f.shield), (Aim::Low, Aim::Low));
}

#[test]
fn two_passes_won_take_the_bout_the_laurel_and_the_purse() {
    let mut run = party(Stable::default(), 200, 2);
    mount(&mut run);
    for _ in 0..2 {
        let m = meeting(&run);
        let w = Mount::Bramble.steed().strike;
        pass(
            &mut run,
            plan(Aim::Low, Aim::Low, Some(m + w)),
            Some(plan(Aim::Low, Aim::Low, None)),
        );
    }
    let j = run.joust.as_ref().unwrap();
    assert_eq!(j.passes, (2, 0));
    assert_eq!(j.stage, Stage::Done, "best of three: two is enough");
    assert_eq!(j.verdict, Some(Verdict::Won));
    assert!(!j.fallen.1);
    assert!(run.players[&1].laurel);
    assert!(run.players[&2].booby.is_some(), "the loser wears the cap");
    assert!(run.feats.contains(&"laurel_of_the_lists"));
    assert_eq!(banked(&run, 1), PURSE - ENTRY);
    assert_eq!(owed(&run), ENTRY, "the loser's entry");
    finish(&mut run);
    for id in [1, 2] {
        let h = &run.players[&id];
        assert!(run.settlement_walkable_for_test(h.x, h.y), "knight {id} on foot");
    }
}

#[test]
fn three_passes_with_nothing_landed_are_a_draw_and_nothing_is_paid() {
    let mut run = party(Stable::default(), 200, 3);
    mount(&mut run);
    run.step(&BTreeMap::from([(
        3,
        Input {
            move_x: 1,
            ..Input::default()
        },
    )]));
    for _ in 0..3 {
        pass(
            &mut run,
            plan(Aim::Low, Aim::Low, None),
            Some(plan(Aim::Low, Aim::Low, None)),
        );
    }
    let j = run.joust.as_ref().unwrap();
    assert_eq!((j.course, j.stage, j.verdict), (2, Stage::Done, Some(Verdict::Drawn)));
    assert!(run.bank.is_empty());
    assert_eq!(owed(&run), 0, "a draw hands every stake back");
}

#[test]
fn a_friend_put_on_the_sand_ends_the_bout_at_once() {
    let mut run = party(Stable::default(), 0, 2);
    mount(&mut run);
    // Two mighty hits round Cinder's shield (four knocks) put Seat 2 down.
    for _ in 0..2 {
        if run.joust.as_ref().unwrap().stage != Stage::Ready {
            break;
        }
        let m = meeting(&run);
        pass(
            &mut run,
            plan(Aim::Low, Aim::Low, Some(m)),
            Some(plan(Aim::Low, Aim::High, None)),
        );
    }
    let j = run.joust.as_ref().unwrap();
    assert!(j.fallen.1 && !j.fallen.0);
    assert_eq!(j.verdict, Some(Verdict::Won));
    assert_eq!(j.purse, 0, "an empty treasury: a bout for honour");
    assert!(run.bank.is_empty());
    assert_eq!(owed(&run), 0);
}

#[test]
fn withdrawing_after_a_pass_forfeits_the_bout() {
    let mut run = party(Stable::default(), 200, 2);
    mount(&mut run);
    pass(
        &mut run,
        plan(Aim::Low, Aim::Low, None),
        Some(plan(Aim::Low, Aim::Low, None)),
    );
    let hold = BTreeMap::from([(
        2,
        Input {
            dash: true,
            ..Input::default()
        },
    )]);
    for _ in 0..WITHDRAW_HOLD + 1 {
        run.step(&hold);
    }
    let j = run.joust.as_ref().unwrap();
    assert_eq!(j.verdict, Some(Verdict::Won), "seat two gave it up");
    assert_eq!(banked(&run, 1), PURSE - ENTRY);
}

#[test]
fn a_rider_who_leaves_the_lists_forfeits_and_a_bettor_who_leaves_is_void() {
    let mut run = party(Stable::default(), 200, 3);
    mount(&mut run);
    run.step(&BTreeMap::from([(
        3,
        Input {
            move_x: 1,
            ..Input::default()
        },
    )]));
    run.depart_lists(3);
    assert!(run.joust.as_ref().unwrap().gallery.is_empty(), "the wager is void");
    pass(
        &mut run,
        plan(Aim::Low, Aim::Low, None),
        Some(plan(Aim::Low, Aim::Low, None)),
    );
    run.depart_lists(2);
    assert!(run.joust.is_none(), "the bout is over");
    assert_eq!(banked(&run, 1), PURSE - ENTRY, "forfeit: the purse to the one who stayed");
    assert_eq!(owed(&run), ENTRY);
    // Before a pass is ridden, the friend left in the saddle rides on
    // against the champion.
    let mut run = party(Stable::default(), 200, 2);
    mount(&mut run);
    run.depart_lists(1);
    let j = run.joust.as_ref().unwrap();
    assert_eq!((j.knight, j.mount), (2, Mount::Cinder));
    assert!(j.foe.is_none() && run.bank.is_empty() && owed(&run) == 0);
    assert!(run.valid_snapshot());
    // A lone knight leaving ends it.
    let mut run = at_the_lists(Stable::default());
    mount(&mut run);
    run.depart_lists(1);
    assert!(run.joust.is_none());
}

// ── The gallery's gold ─────────────────────────────────────────────────────

#[test]
fn the_gallery_backs_a_rider_and_raises_before_the_flag() {
    let mut run = party(Stable::default(), 200, 4);
    mount(&mut run);
    let k = |move_x, fire| Input {
        move_x,
        fire,
        ..Input::default()
    };
    // Seat three backs the knight (west, A) and raises twice; seat four
    // backs the friend (east, D).
    run.step(&BTreeMap::from([(3, k(-1, false)), (4, k(1, false))]));
    run.step(&BTreeMap::from([(3, k(0, true)), (4, k(0, false))]));
    run.step(&BTreeMap::from([(3, k(0, false))]));
    run.step(&BTreeMap::from([(3, k(0, true))]));
    run.step(&BTreeMap::from([(3, k(0, false))]));
    run.step(&BTreeMap::from([(3, k(0, true))]));
    let j = run.joust.as_ref().unwrap();
    assert_eq!((j.gallery[&3].side, j.gallery[&3].stake), (Some(0), MAX_STAKE));
    assert_eq!((j.gallery[&4].side, j.gallery[&4].stake), (Some(1), STAKE));
    // Changing sides keeps the stake; once the flag drops, nothing moves.
    run.step(&BTreeMap::from([(4, k(-1, false))]));
    assert_eq!(run.joust.as_ref().unwrap().gallery[&4].side, Some(0));
    run.step(&BTreeMap::from([(4, k(0, false))]));
    let both = Input {
        fire: true,
        ..Input::default()
    };
    run.step(&BTreeMap::from([(1, both), (2, both)]));
    run.step(&BTreeMap::from([(4, k(1, true))]));
    let j = run.joust.as_ref().unwrap();
    assert_eq!(j.stage, Stage::Charge);
    assert_eq!((j.gallery[&4].side, j.gallery[&4].stake), (Some(0), STAKE));
}

#[test]
fn wagers_settle_into_the_treasury_at_evens() {
    let mut run = party(Stable::default(), 200, 4);
    mount(&mut run);
    let k = |move_x, fire| Input {
        move_x,
        fire,
        ..Input::default()
    };
    run.step(&BTreeMap::from([(3, k(-1, false)), (4, k(1, false))]));
    run.step(&BTreeMap::from([(3, k(0, true))]));
    // The knight takes two passes on the friend's shield.
    for _ in 0..2 {
        let m = meeting(&run);
        let w = Mount::Bramble.steed().strike;
        pass(
            &mut run,
            plan(Aim::Low, Aim::Low, Some(m + w)),
            Some(plan(Aim::Low, Aim::Low, None)),
        );
    }
    let j = run.joust.as_ref().unwrap();
    assert_eq!(j.verdict, Some(Verdict::Won));
    assert_eq!(j.gallery[&3].won, Some(true));
    assert_eq!(j.gallery[&4].won, Some(false));
    assert_eq!(banked(&run, 3), 2 * STAKE, "seat three's twenty, won at evens");
    assert_eq!(banked(&run, 4), 0);
    assert_eq!(owed(&run), ENTRY + STAKE, "the loser's entry and seat four's ten");
    // The save: winnings bank under each name, losses leave the treasury.
    let mut treasury = run.treasury.clone();
    let said = settle_lists(&mut treasury, &run.marks);
    assert_eq!(treasury.get(Spoil::Gold), 200 - ENTRY - STAKE);
    assert!(said[0].1.contains("30 gold"));
    // The gallery is back on its feet where it stood.
    finish(&mut run);
    let h = &run.players[&3];
    assert!(run.settlement_walkable_for_test(h.x, h.y));
}

#[test]
fn a_short_purse_rides_for_honour_and_wagers_stop_at_the_treasury() {
    // Twenty gold: not the two entries, so the bout is for honour; the
    // gallery may wager what is left, and no more.
    let mut run = party(Stable::default(), ENTRY, 3);
    mount(&mut run);
    assert_eq!(run.joust.as_ref().unwrap().purse, 0);
    let k = |move_x, fire| Input {
        move_x,
        fire,
        ..Input::default()
    };
    run.step(&BTreeMap::from([(3, k(-1, false))]));
    run.step(&BTreeMap::from([(3, k(0, true))]));
    run.step(&BTreeMap::from([(3, k(0, false))]));
    run.step(&BTreeMap::from([(3, k(0, true))]));
    assert_eq!(run.joust.as_ref().unwrap().gallery[&3].stake, ENTRY, "no more to raise with");
    // With nothing at all, nothing goes on.
    let mut broke = party(Stable::default(), 0, 3);
    mount(&mut broke);
    broke.step(&BTreeMap::from([(3, k(-1, false))]));
    let w = &broke.joust.as_ref().unwrap().gallery[&3];
    assert_eq!((w.side, w.stake), (None, 0));
}

#[test]
fn the_lists_take_no_more_than_the_treasury_holds() {
    let mut treasury = Spoils::default();
    treasury.add(Spoil::Gold, 15);
    let marks = BTreeMap::from([(PAID_MARK.to_string(), 40)]);
    settle_lists(&mut treasury, &marks);
    assert_eq!(treasury.get(Spoil::Gold), 0);
    assert!(settle_lists(&mut treasury, &BTreeMap::new()).is_empty());
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
            run.step(&interact());
        }
        run.step(&keys(false, false, 0));
        assert!(run.marks.contains_key(mark), "{mark}");
    }
}

/// The stables with gold in the realm's purse, a knight on market pen `i`.
fn at_the_market(gold: u32, i: usize) -> Run {
    let mut treasury = crate::realm::Spoils::default();
    treasury.add(Spoil::Gold, gold);
    let mut run = Run::at_home(7, 1, None, Home::default(), treasury);
    run.arrive(world::room_of(&run.dungeon, RoomKind::Stables).unwrap(), (12.0, 9.0));
    let (c, r, _, _) = MARKET_PLATES[i];
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y) = ((c as f32 + 1.0) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS);
    run
}

fn hold_e(run: &mut Run) {
    let e = BTreeMap::from([(1, Input { interact: true, ..Default::default() })]);
    for _ in 0..BUY_HOLD + 1 {
        run.step(&e);
    }
}

#[test]
fn the_market_pens_sell_todays_horses_for_the_realms_gold() {
    let mut run = at_the_market(2000, 1);
    let horse = run.home.stable.todays_market()[1].clone();
    hold_e(&mut run);
    assert_eq!(run.marks.get(&market_mark(1)), Some(&1));
    assert_eq!(market_open(&run.home.stable, 2000, &run.marks, 1), Err("SOLD"));
    // The save pays and the herd grows.
    let mut stable = run.home.stable.clone();
    let mut treasury = run.treasury.clone();
    let before = stable.herd.len();
    let said = settle_market(&mut stable, &mut treasury, &run.marks);
    assert_eq!(stable.herd.len(), before + 1);
    assert_eq!(stable.herd.last().unwrap().name, horse.name);
    assert_eq!(treasury.get(Spoil::Gold), 2000 - horse.value(&run.home.stable));
    assert!(said[0].1.contains(&horse.name));
}

#[test]
fn a_short_purse_buys_nothing() {
    let mut run = at_the_market(1, 0);
    hold_e(&mut run);
    assert!(run.marks.keys().all(|k| !k.starts_with("stable:buy")));
    assert_eq!(market_open(&run.home.stable, 1, &run.marks, 0), Err("PURSE SHORT"));
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_market_shots -- --ignored`
#[test]
#[ignore]
fn write_market_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("market");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = at_the_market(400, 2);
    run.step(&BTreeMap::new());
    save(&run, "a-on-pen");
    hold_e(&mut run);
    save(&run, "b-bought");
}


/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_joust_shots -- --ignored`:
/// the plate, the wagers, a pass, a hit, an unhorsing and the result.
#[test]
#[ignore]
fn write_joust_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("joust");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str, viewer: u32| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(viewer));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let k = |move_x, fire| Input {
        move_x,
        fire,
        ..Input::default()
    };
    let named = |run: &mut Run| {
        for (id, name) in [(1, "Alys"), (2, "Rowan"), (3, "Elaine"), (4, "Piers")] {
            if let Some(h) = run.players.get_mut(&id) {
                h.name = name.into();
            }
        }
    };
    let both = Input {
        fire: true,
        ..Input::default()
    };
    // Ride one pass to the meeting on two plans, saving at the given ticks.
    let ride_to_meeting = |run: &mut Run, a: Plan, b: Plan, shots: &[(u32, &str)]| {
        let (mut ea, mut eb) = (false, false);
        loop {
            let j = run.joust.as_ref().unwrap();
            if j.hits.is_some() {
                break;
            }
            let next = j.t + 1;
            let f = j.foe.as_ref().unwrap();
            let inputs = BTreeMap::from([
                (1, plan_keys(a, j.shield, next, &mut ea)),
                (2, plan_keys(b, f.shield, next, &mut eb)),
            ]);
            run.step(&inputs);
            let t = run.joust.as_ref().unwrap().t;
            for &(at, name) in shots {
                if at == t {
                    save(run, name, 1);
                }
            }
        }
    };
    // On the plate, three friends about.
    let mut run = party(Stable::default(), 300, 4);
    named(&mut run);
    for _ in 0..BUY_HOLD / 2 {
        run.step(&interact());
    }
    save(&run, "a-plate", 1);
    // Mounted: horses picked, the gallery wagering.
    let mut run = party(Stable::default(), 300, 4);
    named(&mut run);
    mount(&mut run);
    run.step(&BTreeMap::from([(2, k(1, false)), (3, k(-1, false)), (4, k(1, false))]));
    run.step(&BTreeMap::from([(3, k(0, true))]));
    run.step(&BTreeMap::new());
    save(&run, "b-wagers-gallery", 3);
    save(&run, "c-ends-rider", 1);
    // The first pass: mid-charge, the strike window, the hit.
    run.step(&BTreeMap::from([(1, both), (2, both)]));
    let m = meeting(&run);
    ride_to_meeting(
        &mut run,
        plan(Aim::High, Aim::High, Some(m)),
        plan(Aim::Low, Aim::Low, Some(m + 3)),
        &[(m / 2, "d-charge"), (m - 2, "e-strike")],
    );
    for _ in 0..3 {
        run.step(&BTreeMap::new());
    }
    save(&run, "f-hit", 1);
    // On to the second pass, and Rowan goes over.
    while run.joust.as_ref().unwrap().stage != Stage::Ready {
        run.step(&BTreeMap::new());
    }
    run.step(&BTreeMap::from([(1, both), (2, both)]));
    let m = meeting(&run);
    ride_to_meeting(
        &mut run,
        plan(Aim::Low, Aim::High, Some(m)),
        plan(Aim::Low, Aim::High, None),
        &[],
    );
    assert!(run.joust.as_ref().unwrap().fallen.1, "Rowan goes over");
    let mut age = 0;
    for (at, name) in [(4, "g-unhorse-1"), (12, "g-unhorse-2"), (40, "g-unhorse-3")] {
        while age < at {
            run.step(&BTreeMap::new());
            age += 1;
        }
        save(&run, name, 1);
    }
    while run.joust.as_ref().unwrap().stage != Stage::Done {
        run.step(&BTreeMap::new());
    }
    for _ in 0..20 {
        run.step(&BTreeMap::new());
    }
    save(&run, "h-result", 3);
    // A lone knight against a champion.
    let mut run = at_the_lists(Stable::default());
    named(&mut run);
    mount(&mut run);
    run.step(&keys(true, false, 0));
    for _ in 0..50 {
        run.step(&keys(false, false, -1));
    }
    save(&run, "i-champion", 1);
}
