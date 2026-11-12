//! The Keep's wall by hand: the stone chain from outcrop to course, the
//! setter's plumb-and-level, the marks and the true stones, the hired hands
//! beside the knights, the bays the masons wait for, and a knight who
//! drops out mid-carry. These step `tick_hands` themselves: the site's own
//! ticking needs the blank-slate plan, the hands do not.

use super::*;
use crate::rules::trades::{JOURNEYMAN, Ledger, MASTER, Tool};

/// Where no job is within nine metres of a knight, and the site is still near.
const FAR: [f32; 2] = [19.0, -19.0];

fn castle() -> &'static Castle {
    works::castle(works::KEEP).expect("the great tower")
}

/// A party of `n` knights in the kingdom, on their feet.
fn party(n: u32) -> Run {
    let mut run = Run::at_home(7, 1, None, Default::default(), Default::default());
    for k in 2..=n {
        run.join_seat(k, &format!("Knight {k}"));
    }
    run.walk_kingdom(None);
    for h in run.players.values_mut() {
        h.stone = false;
        h.idle = false;
        h.away = false;
    }
    run
}

fn stand(run: &mut Run, id: u32, p: [f32; 2]) {
    let (x, y) = hands::units(p);
    let h = run.players.get_mut(&id).expect("a knight");
    (h.x, h.y) = (x, y);
}

/// One tick of the hands, the knights holding `keys`.
fn step(run: &mut Run, keys: &[(u32, Input)]) {
    run.tick += 1;
    let inputs: BTreeMap<u32, Input> = keys.iter().copied().collect();
    run.tick_hands(&inputs, castle());
    // What the site does after the hands each tick, without the plan's map.
    run.follow_site_tiles();
}

fn swing() -> Input {
    Input { swing: true, ..Default::default() }
}

/// The chain's stores as a test wants them; the rough stone is blocks of
/// limestone in the one stockyard the benches cut from.
fn stock(run: &mut Run, rough: u32, cut: u32, course: u32, mortar: u32) {
    run.home.stockyard.stone.clear();
    if rough > 0 {
        run.home.stockyard.stone.insert("limestone".into(), rough);
    }
    let y = &mut run.hands.yard;
    (y.cut, y.course, y.mortar) = (cut, course, mortar);
}

fn laid(run: &Run) -> u32 {
    run.sites.keep().map_or(0, |s| s.laid)
}

/// What a setter who knows the timing does this tick: start the stone, tap
/// when the bob hangs true, tap when the bubble rides the mark. `late` ticks
/// of waiting before the first tap (a bob that has swung round more than
/// once), `bad` taps on the worst moment of the swing instead.
fn setter(run: &Run, id: u32, late: u64, bad: (bool, bool)) -> Input {
    let now = run.tick + 1;
    let press = match run.hands.seats.get(&id).and_then(|h| h.work) {
        None => run.tick % 2 == 0,
        Some(Work::Set { began, plumb: None, .. }) => {
            let since = now - began;
            since >= hands::FLINCH + late && if bad.0 { hands::plumb_dev(since) == 9 } else { hands::plumb_dev(since) == 0 }
        }
        Some(Work::Set { tapped, plumb: Some(_), lands: 0, .. }) => {
            let since = now - tapped;
            if bad.1 { hands::level_dev(since) == 22 } else { hands::level_dev(since) <= 1 && since >= 20 }
        }
        _ => false,
    };
    if press { swing() } else { Input::default() }
}

/// A knight who strikes `job` on every perfect beat.
fn striker(run: &Run, id: u32) -> Input {
    let now = run.tick + 1;
    let press = match run.hands.seats.get(&id).and_then(|h| h.work) {
        None => run.tick % 2 == 0,
        Some(Work::Strike { job, began, .. }) => hands::beat_dev(job, now - began) == 0 && now > began,
        _ => false,
    };
    if press { swing() } else { Input::default() }
}

#[test]
fn a_perfect_set_is_a_true_stone_with_the_knights_mark_and_the_stones_chain_is_spent() {
    let mut run = party(1);
    run.teach_trade(1, Trade::Setter, 1);
    let start = laid(&run);
    stock(&mut run, 0, 0, 3, 3);
    stand(&mut run, 1, hands::stone_at(castle(), start));
    for _ in 0..300 {
        let input = setter(&run, 1, 0, (false, false));
        step(&mut run, &[(1, input)]);
        if laid(&run) > start {
            break;
        }
    }
    let site = run.sites.keep().expect("site");
    assert_eq!(site.laid, start + 1, "the stone is set");
    assert!(site.is_true(start), "plumb and level both: a true stone, stored");
    assert_eq!(site.setter(start), 1, "the knight's mark");
    assert_eq!(site.true_count(), 1);
    assert_eq!((run.hands.yard.course, run.hands.yard.mortar), (2, 2), "a stone and a dab spent");
    assert_eq!(run.deeds_of(1, Trade::Setter), 1 + 3, "the lesson, and a perfect true set");
    assert!(run.hands.yard.settled.is_some_and(|(i, _)| i == start));
    assert!(run.sounds.contains(&"rock_land"), "the stone settling");
    assert!(run.hands.seats[&1].said.as_ref().is_some_and(|(_, t)| t.contains("PERFECT")));
}

#[test]
fn a_rough_set_still_sets_the_stone_and_a_slip_loses_nothing() {
    // The plumb true and the level wild: set, but not true.
    let mut run = party(1);
    run.teach_trade(1, Trade::Setter, 1);
    let start = laid(&run);
    stock(&mut run, 0, 0, 3, 3);
    stand(&mut run, 1, hands::stone_at(castle(), start));
    for _ in 0..300 {
        let input = setter(&run, 1, 0, (false, true));
        step(&mut run, &[(1, input)]);
        if laid(&run) > start {
            break;
        }
    }
    let site = run.sites.keep().expect("site");
    assert_eq!(site.laid, start + 1);
    assert!(!site.is_true(start), "a rough set is not a true stone");
    assert_eq!(site.setter(start), 1, "but it is the knight's");
    assert_eq!(run.deeds_of(1, Trade::Setter), 2, "a deed for it");
    // Both wild: the stone slips back, the stock and the wall as they were.
    let mut run = party(1);
    run.teach_trade(1, Trade::Setter, 1);
    stock(&mut run, 0, 0, 3, 3);
    stand(&mut run, 1, hands::stone_at(castle(), start));
    let mut slipped = false;
    for _ in 0..110 {
        let input = setter(&run, 1, 0, (true, true));
        step(&mut run, &[(1, input)]);
        slipped |= run.hands.seats[&1].said.as_ref().is_some_and(|(_, t)| t.contains("SLIPS"));
        assert_eq!(laid(&run), start, "nothing is set");
        assert_eq!((run.hands.yard.course, run.hands.yard.mortar), (3, 3), "nothing is spent");
    }
    assert!(slipped, "the stone slipped back");
}

#[test]
fn a_setter_with_no_stone_or_no_mortar_at_the_wall_is_told_so_and_a_hod_is_wanted() {
    let mut run = party(1);
    run.teach_trade(1, Trade::Setter, 1);
    let start = laid(&run);
    stand(&mut run, 1, hands::stone_at(castle(), start));
    stock(&mut run, 0, 0, 0, 5);
    step(&mut run, &[(1, swing())]);
    assert!(run.hands.seats[&1].work.is_none());
    assert!(run.hands.seats[&1].said.as_ref().is_some_and(|(_, t)| t.contains("NO STONE")));
    assert_eq!(run.hand_prompt(1).as_deref(), Some("NO STONE AT THE WALL: HOD ONE UP"));
    stock(&mut run, 0, 0, 2, 0);
    step(&mut run, &[(1, Input::default())]);
    step(&mut run, &[(1, swing())]);
    assert!(run.hands.seats[&1].said.as_ref().is_some_and(|(_, t)| t.contains("MORTAR")));
}

#[test]
fn three_knights_set_a_course_together_each_stone_the_mark_of_who_set_it_in_lay_order() {
    let mut run = party(3);
    for id in 1..=3 {
        run.teach_trade(id, Trade::Setter, 1);
    }
    let start = laid(&run);
    stock(&mut run, 0, 0, 12, 12);
    for id in 1..=3 {
        stand(&mut run, id, hands::stone_at(castle(), start + id - 1));
    }
    // Seat 1 takes its time at the plumb; seats 2 and 3 do not: their stones
    // are done first and wait for the stone before them.
    let mut waited = false;
    for _ in 0..400 {
        let keys: Vec<(u32, Input)> = (1..=3).map(|id| (id, setter(&run, id, if id == 1 { 36 } else { 0 }, (false, false)))).collect();
        step(&mut run, &keys);
        if laid(&run) == start && !run.hands.yard.done.is_empty() {
            waited = true;
        }
        if laid(&run) >= start + 3 {
            break;
        }
    }
    assert!(waited, "a stone set before its neighbour waits to be laid");
    let site = run.sites.keep().expect("site");
    assert_eq!(site.laid, start + 3);
    assert!(run.hands.yard.done.is_empty());
    // Strictly in lay order, each stone its setter's: seats 1, 2, 3 on stones 0, 1, 2.
    assert_eq!((site.setter(start), site.setter(start + 1), site.setter(start + 2)), (1, 2, 3), "{:?} {:?}", site.marks, run.hands.seats);
    assert_eq!(site.true_count(), 3);
    let tally = site.tally();
    for seat in 1..=3u8 {
        assert!(tally.iter().any(|&(s, n)| s == seat && n == 1), "{tally:?}");
    }
    assert!(site.valid(), "{site:?}");
}

#[test]
fn the_chain_by_hand_from_the_outcrop_to_the_wall_and_the_ranks_come_with_the_work() {
    let mut run = party(4);
    run.teach_trade(1, Trade::Quarryman, 1);
    run.teach_trade(2, Trade::BankerMason, 1);
    run.teach_trade(3, Trade::Labourer, 1);
    run.teach_trade(4, Trade::Setter, 1);
    let start = laid(&run);
    stock(&mut run, 0, 0, 0, 12);
    run.hands.yard.tub = 18;
    let (bench, yard) = (life::BANKERS[1], life::YARD);
    stand(&mut run, 1, hands::QUARRY);
    stand(&mut run, 2, [bench[0], bench[1] + 1.15]);
    stand(&mut run, 3, yard);
    stand(&mut run, 4, hands::stone_at(castle(), start));
    // The labourer: take a stone from the yard when there is one, walk it to
    // the wall (we put the knight there), hold E to climb, and back.
    let mut presses = 0u32;
    for _ in 0..4_000 {
        // The setter walks along the course as it grows.
        if run.hands.seats[&4].work.is_none() {
            let next = hands::stone_at(castle(), laid(&run));
            stand(&mut run, 4, next);
        }
        let mut keys = vec![(1, striker(&run, 1)), (2, striker(&run, 2)), (4, setter(&run, 4, 0, (false, false)))];
        let h3 = &run.hands.seats[&3];
        let want_stone = h3.carry == Carry::Nothing;
        if want_stone && run.hands.yard.cut > 0 {
            stand(&mut run, 3, yard);
            presses += 1;
            keys.push((3, Input { interact: presses % 2 == 0, ..Default::default() }));
        } else if !want_stone {
            let next = hands::stone_at(castle(), laid(&run));
            stand(&mut run, 3, next);
            keys.push((3, Input { interact: true, ..Default::default() }));
        }
        step(&mut run, &keys);
        if laid(&run) >= start + 6 {
            break;
        }
    }
    let site = run.sites.keep().expect("site");
    assert!(site.laid >= start + 6, "the chain by hand set stones: {}", site.laid - start);
    // The hired setters may have set some; the knight's own are theirs.
    assert!(site.tally().iter().any(|&(s, n)| s == 4 && n >= 3), "{:?}", site.tally());
    assert!(run.deeds_of(1, Trade::Quarryman) > 1 && run.deeds_of(2, Trade::BankerMason) > 1 && run.deeds_of(3, Trade::Labourer) > 1 && run.deeds_of(4, Trade::Setter) > 1);
    assert!(run.hands.yard.cut + run.hands.yard.course + site.laid - start > 0);
}

#[test]
fn the_quarry_the_bench_and_the_bay_take_their_blows_on_the_beat_and_a_miss_takes_none() {
    let mut run = party(1);
    run.teach_trade(1, Trade::Quarryman, 1);
    stand(&mut run, 1, hands::QUARRY);
    stock(&mut run, 3, 0, 0, 0);
    // Mashing the key off the beat lands little: no block.
    for k in 0..200 {
        step(&mut run, &[(1, Input { swing: k % 2 == 0, ..Default::default() })]);
    }
    assert!(run.home.stockyard.held() <= 3 + 1, "{}", run.home.stockyard.held());
    // On the beat: a block comes off in five blows (about three seconds).
    run.hands.seats.get_mut(&1).expect("hand").work = None;
    run.hands.yard.acc = [0; 8];
    let (before, rubble) = (run.home.stockyard.held(), run.home.stockyard.count("rubble"));
    for _ in 0..400 {
        let input = striker(&run, 1);
        step(&mut run, &[(1, input)]);
        if run.home.stockyard.held() > before {
            break;
        }
    }
    // The hillside's outcrop gives rubble, into the one stockyard.
    assert_eq!(run.home.stockyard.held(), before + 1);
    assert_eq!(run.home.stockyard.count("rubble"), rubble + 1);
    assert!(run.sounds.contains(&"hit_axe") && run.sounds.contains(&"rock_land"));
    assert!(run.hands.seats[&1].struck > 0);
    // The bench: a rough block becomes a cut one.
    run.teach_trade(1, Trade::BankerMason, 1);
    let bench = life::BANKERS[0];
    stand(&mut run, 1, [bench[0], bench[1] + 1.15]);
    stock(&mut run, 2, 0, 0, 0);
    run.hands.yard.acc = [0; 8];
    for _ in 0..400 {
        let input = striker(&run, 1);
        step(&mut run, &[(1, input)]);
        if run.hands.yard.cut > 0 {
            break;
        }
    }
    assert_eq!((run.home.stockyard.held(), run.hands.yard.cut), (1, 1));
    // With nothing to cut the bench says so and nothing starts.
    stock(&mut run, 0, 0, 0, 0);
    step(&mut run, &[(1, Input::default())]);
    step(&mut run, &[(1, swing())]);
    assert!(run.hands.seats[&1].work.is_none());
    assert!(run.hands.seats[&1].said.as_ref().is_some_and(|(_, t)| t.contains("NO ROUGH")));
}

#[test]
fn the_hod_carries_slower_for_the_weight_and_a_load_climbs_to_the_wall() {
    let mut run = party(1);
    run.teach_trade(1, Trade::Labourer, 1);
    let start = laid(&run);
    stock(&mut run, 0, 2, 0, 0);
    stand(&mut run, 1, life::YARD);
    assert_eq!(run.hands.pace(1), 1.0);
    // E at the yard's heap: a stone on the hod.
    step(&mut run, &[(1, Input { interact: true, ..Default::default() })]);
    assert_eq!(run.hands.seats[&1].carry, Carry::Stone);
    assert_eq!(run.hands.yard.cut, 1);
    assert!(run.hands.pace(1) < 0.7, "a stone is a weight: {}", run.hands.pace(1));
    assert_eq!(Tool::unpack(run.players[&1].tool).map(|(t, l)| (t, l)), Some((Tool::Hod, 1)), "the stone is on the hod in the figure");
    // At the wall, E held: the scaffold climbed, the stone set down at the course.
    stand(&mut run, 1, hands::stone_at(castle(), start));
    let held = Input { interact: true, ..Default::default() };
    for _ in 0..hands::CLIMB / 2 {
        step(&mut run, &[(1, held)]);
    }
    assert_eq!(run.hands.yard.course, 0, "half a climb sets nothing down");
    assert!(run.hands.busy.contains(&1), "E held at the scaffold is for the climb, not the Chronicle");
    for _ in 0..hands::CLIMB {
        step(&mut run, &[(1, held)]);
    }
    assert_eq!(run.hands.yard.course, 1);
    assert_eq!(run.hands.seats[&1].carry, Carry::Nothing);
    assert_eq!(run.hands.pace(1), 1.0);
    // A load of mortar, from the tub: six dabs at the wall.
    stand(&mut run, 1, hands::TUB);
    run.hands.yard.tub = 12;
    step(&mut run, &[(1, Input::default())]);
    step(&mut run, &[(1, held)]);
    assert_eq!(run.hands.seats[&1].carry, Carry::Mortar);
    assert_eq!(run.hands.yard.tub, 12 - hands::LOAD_DABS);
    assert!(run.hands.pace(1) > hands::PACE_STONE && run.hands.pace(1) < 1.0, "mortar is lighter than stone");
}

#[test]
fn walking_the_wheel_turns_the_crane_faster_and_a_stone_rises_to_the_wall() {
    let wheel = hands::wheel_stand(castle(), site::START).expect("a crane stands");
    let lifted = |on_wheel: bool| {
        let mut run = party(1);
        run.teach_trade(1, Trade::Labourer, 1);
        stock(&mut run, 0, 12, 0, 0);
        stand(&mut run, 1, if on_wheel { wheel } else { FAR });
        let held = Input { interact: true, ..Default::default() };
        for _ in 0..HZ * 30 {
            step(&mut run, &[(1, if on_wheel { held } else { Input::default() })]);
        }
        run
    };
    let (on, off) = (lifted(true), lifted(false));
    assert!(on.crane_seconds() > 30.0 * 0.35 + 25.0, "a knight on the wheel turns it faster than the hired hand's pace: {}", on.crane_seconds());
    assert!(off.crane_seconds() > 5.0 && off.crane_seconds() < on.crane_seconds() / 2.0, "the crane still turns, slowly: {}", off.crane_seconds());
    // The stone that rose: the yard's heap is lower by the crane's lifts and the hired hods'.
    let moved = |r: &Run| 12 - r.hands.yard.cut;
    assert!(moved(&on) >= moved(&off) + 2, "stones rose to the wall for the knight on the wheel: {} against {}", moved(&on), moved(&off));
    assert!(on.deeds_of(1, Trade::Labourer) > 1);
    assert!(on.players[&1].stride > 0.0, "the knight treads in place");
    assert_eq!(on.players[&1].tool, 0, "both hands are on the wheel");
}

#[test]
fn a_knight_who_drops_out_mid_carry_returns_the_load_and_no_claim_is_left() {
    // Leaving: the stone goes back to the yard.
    let mut run = party(2);
    run.teach_trade(2, Trade::Labourer, 1);
    stock(&mut run, 0, 3, 0, 0);
    stand(&mut run, 2, life::YARD);
    step(&mut run, &[(2, Input { interact: true, ..Default::default() })]);
    assert_eq!((run.hands.seats[&2].carry, run.hands.yard.cut), (Carry::Stone, 2));
    run.leave(2);
    assert_eq!(run.hands.yard.cut, 3, "the stone is back in the yard");
    assert!(!run.hands.seats.contains_key(&2));
    // Falling: the next tick returns it.
    let mut run = party(1);
    run.teach_trade(1, Trade::Labourer, 1);
    stock(&mut run, 0, 3, 0, 0);
    stand(&mut run, 1, life::YARD);
    step(&mut run, &[(1, Input { interact: true, ..Default::default() })]);
    run.players.get_mut(&1).expect("a knight").hp = 0;
    step(&mut run, &[]);
    assert_eq!((run.hands.seats[&1].carry, run.hands.yard.cut), (Carry::Nothing, 3));
    assert_eq!(run.players[&1].tool, 0);
    // A player gone away turns their knight to stone: the load is let go.
    let mut run = party(1);
    run.teach_trade(1, Trade::Labourer, 1);
    stock(&mut run, 0, 3, 0, 0);
    stand(&mut run, 1, life::YARD);
    step(&mut run, &[(1, Input { interact: true, ..Default::default() })]);
    run.set_away(1, true);
    step(&mut run, &[]);
    assert_eq!((run.hands.seats[&1].carry, run.hands.yard.cut), (Carry::Nothing, 3));
    // Going below with a load: the same.
    let mut run = party(1);
    run.teach_trade(1, Trade::Labourer, 1);
    stock(&mut run, 0, 3, 0, 0);
    stand(&mut run, 1, life::YARD);
    step(&mut run, &[(1, Input { interact: true, ..Default::default() })]);
    run.release_all_hands();
    assert_eq!((run.hands.seats[&1].carry, run.hands.yard.cut, run.players[&1].tool), (Carry::Nothing, 3, 0));
    // Mid-set: the claim is let go, nothing spent, and a friend can take the stone.
    let mut run = party(2);
    for id in 1..=2 {
        run.teach_trade(id, Trade::Setter, 1);
    }
    let start = laid(&run);
    stock(&mut run, 0, 0, 3, 3);
    for id in 1..=2 {
        stand(&mut run, id, hands::stone_at(castle(), start));
    }
    step(&mut run, &[(1, swing())]);
    assert!(matches!(run.hands.seats[&1].work, Some(Work::Set { index, .. }) if index == start));
    run.players.get_mut(&1).expect("a knight").hp = 0;
    step(&mut run, &[(2, swing())]);
    assert!(run.hands.seats[&1].work.is_none());
    assert!(matches!(run.hands.seats[&2].work, Some(Work::Set { index, .. }) if index == start), "the stone is the friend's now");
    assert_eq!((run.hands.yard.course, run.hands.yard.mortar), (3, 3), "nothing was spent");
    // And the hired hands set the wall on whatever stands unclaimed: no softlock.
    let mut run = party(1);
    stock(&mut run, 0, 0, 5, 5);
    stand(&mut run, 1, FAR);
    let start = laid(&run);
    for _ in 0..HZ * 40 {
        step(&mut run, &[]);
    }
    assert!(laid(&run) > start, "the hired setters set the stones with no knight at the course");
}

#[test]
fn the_hired_hands_work_slowly_and_a_knight_beside_them_speeds_the_gang() {
    let run_for = |beside: bool| {
        let mut run = party(1);
        // Far from every job, or among them.
        stand(&mut run, 1, if beside { [-8.0, -8.0] } else { FAR });
        for _ in 0..HZ * 120 {
            step(&mut run, &[]);
        }
        let y = &run.hands.yard;
        (laid(&run) - site::START, run.home.stockyard.held() + y.cut + y.course, run)
    };
    let (alone_laid, alone_stock, alone) = run_for(false);
    let (beside_laid, beside_stock, beside) = run_for(true);
    assert!(alone_laid + alone_stock > 0, "the hired hands alone make progress: laid {alone_laid}, stock {alone_stock}");
    assert!(
        beside_laid + beside_stock > alone_laid + alone_stock,
        "a knight beside the work speeds it: {beside_laid}+{beside_stock} against {alone_laid}+{alone_stock}"
    );
    // Slower than a knight: two minutes of the whole gang is a few dozen stones, not hundreds.
    assert!(alone_laid < 60, "{alone_laid}");
    // The hired setters' stones are the realm's, not a knight's.
    let site = alone.sites.keep().expect("site");
    assert!(site.marks.iter().skip(1).all(|&(_, seat)| seat == 0));
    let _ = beside;
}

#[test]
fn the_masons_wait_on_the_carpenter_for_a_bay_and_raising_it_lets_them_on() {
    let mut run = party(1);
    // Just under the first bay to be raised: one stone short of it.
    let bay = hands::bays(castle()).into_iter().find(|b| b.appears > site::START).expect("a bay ahead");
    run.lay_to("great-tower", bay.appears - 1);
    stock(&mut run, 0, 0, 5, 12);
    stand(&mut run, 1, FAR);
    // Twelve seconds: the hired setters set the stone and the hired
    // carpenter, who takes half a minute to a bay, has not come to it.
    for _ in 0..HZ * 12 {
        step(&mut run, &[]);
    }
    assert_eq!(laid(&run), bay.appears, "the masons lay up to the bay and stop");
    assert!(hands::limit(castle(), &run.hands.yard, laid(&run)) <= bay.appears);
    assert!(run.clerk_line(castle()).contains("CARPENTER"), "{}", run.clerk_line(castle()));
    // The carpenter raises it, six knocks on the beat.
    run.teach_trade(1, Trade::Carpenter, 1);
    stand(&mut run, 1, bay.at);
    run.hands.yard.acc = [0; 8];
    assert!(hands::next_bay(castle(), &run.hands.yard, laid(&run)).is_some_and(|b| b.appears == bay.appears));
    for _ in 0..HZ * 10 {
        let input = striker(&run, 1);
        step(&mut run, &[(1, input)]);
        if run.hands.yard.timber >= bay.appears {
            break;
        }
    }
    assert_eq!(run.hands.yard.timber, bay.appears, "the bay stands");
    assert!(hands::limit(castle(), &run.hands.yard, laid(&run)) > bay.appears);
    stand(&mut run, 1, FAR);
    for _ in 0..HZ * 40 {
        step(&mut run, &[]);
    }
    assert!(laid(&run) > bay.appears, "the masons go on");
    assert!(run.deeds_of(1, Trade::Carpenter) > 1);
    // Only bays the masons are coming to are raised: not one a lift away.
    let mut far = party(1);
    far.teach_trade(1, Trade::Carpenter, 1);
    far.lay_to("great-tower", site::START);
    assert!(hands::limit(castle(), &far.hands.yard, site::START) - site::START > hands::LOOKAHEAD);
    assert!(hands::next_bay(castle(), &far.hands.yard, site::START).is_none());
}

#[test]
fn an_arch_stone_wants_a_template_and_only_the_master_mason_draws_them_by_hand() {
    let mut run = party(2);
    let c = castle();
    let at = c.stones.iter().position(|s| c.role(s.role) == "voussoir").expect("an arch") as u32;
    run.lay_to("great-tower", at);
    run.teach_trade(1, Trade::BankerMason, 1);
    run.teach_trade(2, Trade::MasterMason, 1);
    stock(&mut run, 3, 0, 0, 0);
    assert_eq!(run.next_cut_role(c, at), "voussoir");
    let bench = life::BANKERS[0];
    stand(&mut run, 1, [bench[0], bench[1] + 1.15]);
    step(&mut run, &[(1, Input::default())]);
    step(&mut run, &[(1, swing())]);
    assert!(run.hands.seats[&1].work.is_none(), "no cutting without a template");
    assert!(run.hands.seats[&1].said.as_ref().is_some_and(|(_, t)| t.contains("TEMPLATE")));
    assert!(run.clerk_line(c).contains("TEMPLATE"));
    // The Master Mason draws one at the tracing floor.
    stand(&mut run, 2, hands::FLOOR);
    for _ in 0..HZ * 10 {
        let input = striker(&run, 2);
        step(&mut run, &[(2, input)]);
        if run.hands.yard.templates > 0 {
            break;
        }
    }
    assert_eq!(run.hands.yard.templates, hands::TEMPLATE_USES);
    // Now the bench cuts it, and uses a template up.
    for _ in 0..400 {
        let input = striker(&run, 1);
        step(&mut run, &[(1, input)]);
        if run.hands.yard.cut > 0 {
            break;
        }
    }
    assert_eq!(run.hands.yard.cut, 1);
    assert_eq!(run.hands.yard.templates, hands::TEMPLATE_USES - 1);
    // A Master Mason pegs out a castle; with only the great tower baked there is none to peg.
    assert!(run.peg_out(2).is_err(), "an Apprentice cannot");
    run.teach_trade(2, Trade::MasterMason, MASTER);
    let said = run.peg_out(2).expect_err("no new plan is baked");
    assert!(said.contains("BAKED"), "{said}");
    // Only the Master may.
    run.teach_trade(1, Trade::MasterMason, JOURNEYMAN);
    assert!(run.peg_out(1).is_err());
}

#[test]
fn the_clerk_of_works_reads_the_line_and_a_round_of_the_board_is_a_deed() {
    let mut run = party(1);
    run.teach_trade(1, Trade::ClerkOfWorks, 1);
    // No arch stone is next: the line is the stone chain's.
    run.hands.yard.templates = 99;
    stock(&mut run, 0, 0, 0, 5);
    // (The stone is the stockyard's now, one store for the hillside's rubble and the Delve's blocks.)
    assert!(run.clerk_line(castle()).contains("STOCKYARD"), "{}", run.clerk_line(castle()));
    stock(&mut run, 2, 0, 0, 5);
    assert!(run.clerk_line(castle()).contains("BANKERS"));
    stock(&mut run, 0, 2, 0, 5);
    assert!(run.clerk_line(castle()).contains("YARD"));
    stock(&mut run, 0, 0, 3, 0);
    assert!(run.clerk_line(castle()).contains("MORTAR"));
    stock(&mut run, 0, 0, 3, 3);
    assert!(run.clerk_line(castle()).contains("SETTERS"));
    stand(&mut run, 1, hands::BOARD);
    step(&mut run, &[(1, Input { interact: true, ..Default::default() })]);
    assert_eq!(run.deeds_of(1, Trade::ClerkOfWorks), 2);
    assert!(run.found.as_ref().is_some_and(|(_, id, t)| *id == 1 && t.contains("SETTERS")));
    // Not twice in one breath.
    step(&mut run, &[(1, Input::default())]);
    step(&mut run, &[(1, Input { interact: true, ..Default::default() })]);
    assert_eq!(run.deeds_of(1, Trade::ClerkOfWorks), 2);
}

#[test]
fn a_teacher_teaches_a_trade_to_a_held_e_and_the_master_masons_plans_wait_for_a_journeyman() {
    let mut run = party(1);
    let kekkit = trades::SCHOOLS.iter().position(|s| s.power == "bomrekedan:kekkit").expect("a school");
    stand(&mut run, 1, trades::SCHOOLS[kekkit].at);
    // A knight with no trade is shown who teaches what is offered.
    let prompt = run.hand_prompt(1).expect("a line");
    assert!(prompt.contains("HOLD E TO TAKE UP"), "{prompt}");
    // Wait for a fresh offer, then hold E for the lesson.
    while run.tick % trades::OFFER_TICKS != 0 {
        step(&mut run, &[]);
    }
    let offered = hands::offer(kekkit, run.tick, |_| false).expect("an offer");
    let held = Input { interact: true, ..Default::default() };
    for _ in 0..trades::LEARN_HOLD + 2 {
        step(&mut run, &[(1, held)]);
    }
    let key = run.trade_key(1).expect("a key");
    assert!(run.trades().holds(&key, offered), "{offered:?} taken up");
    assert_eq!(run.deeds_of(1, offered), 1);
    assert_eq!(run.hands.seats[&1].trade, Some(offered), "its tool in hand");
    assert_eq!(run.players[&1].tool, offered.tool().pack(0));
    assert!(run.found.as_ref().is_some_and(|(_, _, t)| t.contains("BOMREK") || t.contains("TAKES UP")), "{:?}", run.found);
    assert!(run.sounds.contains(&"card_pickup"));
    // A lesson not held for is not given: letting go starts it over.
    let mut run = party(1);
    stand(&mut run, 1, trades::SCHOOLS[kekkit].at);
    for _ in 0..trades::LEARN_HOLD * 3 {
        step(&mut run, &[(1, Input::default())]);
    }
    assert!(run.trades().held(&run.trade_key(1).expect("a key")).is_empty());
    // The Master Mason's plans (the mayor's school) wait for a journeyman setter and cutter.
    let mut run = party(1);
    let erinys = trades::SCHOOLS.iter().position(|s| s.power == "bomrekedan:erinys").expect("a school");
    run.teach_trade(1, Trade::ClerkOfWorks, 1);
    stand(&mut run, 1, trades::SCHOOLS[erinys].at);
    // The clerk's is held, so the master's plans are what is offered.
    assert_eq!(hands::offer(erinys, 0, |t| t == Trade::ClerkOfWorks), Some(Trade::MasterMason));
    let prompt = run.hand_prompt(1).expect("a line");
    assert!(prompt.contains("COME BACK") && prompt.contains("JOURNEYMAN SETTER"), "{prompt}");
    for _ in 0..trades::LEARN_HOLD * 4 {
        step(&mut run, &[(1, held)]);
    }
    assert_eq!(run.deeds_of(1, Trade::MasterMason), 0, "not taught");
    run.teach_trade(1, Trade::Setter, JOURNEYMAN);
    run.teach_trade(1, Trade::BankerMason, JOURNEYMAN);
    while run.tick % trades::OFFER_TICKS != 0 {
        step(&mut run, &[]);
    }
    for _ in 0..trades::LEARN_HOLD + 2 {
        step(&mut run, &[(1, held)]);
    }
    assert_eq!(run.deeds_of(1, Trade::MasterMason), 1, "taught once ready");
}

#[test]
fn a_press_at_a_job_of_a_trade_held_takes_its_tool_up_and_a_trade_not_held_says_who_teaches_it() {
    let mut run = party(1);
    run.teach_trade(1, Trade::Quarryman, 1);
    run.teach_trade(1, Trade::BankerMason, 1);
    run.hands.seats.get_mut(&1).expect("hand").trade = Some(Trade::Quarryman);
    let bench = life::BANKERS[2];
    stand(&mut run, 1, [bench[0], bench[1] + 1.15]);
    let prompt = run.hand_prompt(1).expect("a line");
    assert!(prompt.contains("TAKE UP THE BANKER MASON"), "{prompt}");
    step(&mut run, &[(1, swing())]);
    assert_eq!(run.hands.seats[&1].trade, Some(Trade::BankerMason), "the chisel is in hand");
    assert_eq!(run.players[&1].tool, Trade::BankerMason.tool().pack(0));
    // At the outcrop with no trade for it: who teaches it.
    let mut run = party(1);
    stand(&mut run, 1, hands::QUARRY);
    let prompt = run.hand_prompt(1).expect("a line");
    assert!(prompt.contains("NEEDS A QUARRYMAN") && prompt.contains("BALDNESSWINDS"), "{prompt}");
    // And the blade stays sheathed for a knight at a job of their trade.
    run.teach_trade(1, Trade::Quarryman, 1);
    step(&mut run, &[(1, Input::default())]);
    assert!(run.hands.striking.contains(&1));
}

#[test]
fn the_keeps_stands_can_be_walked_to_at_every_stage_and_the_setters_reach_the_wall() {
    let c = castle();
    let tile = |p: [f32; 2]| (works::ORIGIN.0 + (p[0] / 1.5).floor() as i32, works::ORIGIN.1 + (-p[1] / 1.5).floor() as i32);
    let middle = |t: (i32, i32)| [((t.0 - works::ORIGIN.0) as f32 + 0.5) * 1.5, -((t.1 - works::ORIGIN.1) as f32 + 0.5) * 1.5];
    // The ground a knight can walk to from the gate's front, with `laid` stones up.
    let walkable = |laid: u32| {
        let (x0, y0, w, h) = c.tiles();
        let inside = |t: (i32, i32)| t.0 >= x0 && t.1 >= y0 && t.0 < x0 + w && t.1 < y0 + h;
        let start = tile([0.0, -15.0]);
        let mut seen = std::collections::HashSet::from([start]);
        let mut queue = std::collections::VecDeque::from([start]);
        while let Some(t) = queue.pop_front() {
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = (t.0 + dx, t.1 + dy);
                if inside(n) && !c.blocked(n.0, n.1, laid) && seen.insert(n) {
                    queue.push_back(n);
                }
            }
        }
        seen
    };
    // Ground to stand on within `reach` metres of `p` that a knight can walk to.
    let reachable = |seen: &std::collections::HashSet<(i32, i32)>, p: [f32; 2], reach: f32| {
        let (tx0, ty0) = tile(p);
        (-5..=5).flat_map(|dy| (-5..=5).map(move |dx| (dx, dy))).any(|(dx, dy)| {
            let t = (tx0 + dx, ty0 + dy);
            let m = middle(t);
            seen.contains(&t) && (m[0] - p[0]).hypot(m[1] - p[1]) <= reach
        })
    };
    let mut stands: Vec<(&str, [f32; 2], f32)> = vec![
        ("outcrop", hands::QUARRY, hands::REACH_QUARRY),
        ("tub", hands::TUB, hands::REACH_TUB),
        ("floor", hands::FLOOR, hands::REACH_FLOOR),
        ("board", hands::BOARD, hands::REACH_BOARD),
        ("yard", life::YARD, hands::REACH_YARD),
    ];
    for b in life::BANKERS {
        stands.push(("bench", [b[0], b[1] + 1.15], hands::REACH_BENCH));
    }
    for s in trades::SCHOOLS.iter() {
        stands.push(("teacher", s.at, hands::REACH_SCHOOL));
    }
    let marks: Vec<u32> = std::iter::once(site::START).chain(c.stages.iter().map(|s| s.first + s.count)).chain([c.len()]).collect();
    for &laid in &marks {
        let seen = walkable(laid);
        for (what, p, reach) in &stands {
            assert!(reachable(&seen, *p, *reach), "no ground a knight can walk to within {reach} m of the {what} at {p:?} at {laid} stones");
        }
        // The first crane stands in the open; the curtain's is in a ward walled shut
        // round it, so its wheel is the hired hand's.
        if let Some(w) = hands::wheel_stand(c, laid).filter(|w| w[0] < 0.0) {
            assert!(reachable(&seen, w, hands::REACH_WHEEL), "no ground by the wheel {w:?} at {laid}");
        }
    }
    // A setter can stand on ground they can walk to within reach of four in
    // five of the stones from where the Keep begins, as it is when the stone is
    // next (measured: 80%). The rest lie in the walls' own pockets, the tower's
    // sealed inner rooms, and are the hired setters'.
    let (mut total, mut got) = (0, 0);
    let mut seen = walkable(site::START);
    for i in (site::START..c.len()).step_by(3) {
        if i % 100 < 3 {
            seen = walkable(i);
        }
        total += 1;
        got += usize::from(reachable(&seen, hands::stone_at(c, i), hands::REACH_WALL));
    }
    let share = got as f32 / total as f32;
    println!("a knight can reach {:.1}% of the stones ({got} of {total})", share * 100.0);
    assert!(share > 0.75, "a knight can reach {:.0}% of the stones ({got} of {total})", share * 100.0);
}

#[test]
fn the_hands_the_yard_and_the_tool_are_saved_and_an_old_save_has_none() {
    let mut run = party(2);
    run.teach_trade(1, Trade::Setter, 40);
    run.home.stockyard.stone.insert("limestone".into(), 7);
    run.hands.yard.timber = 1992;
    run.hands.yard.done.insert(1_700, (2, true));
    run.hands.seats.entry(2).or_default().carry = Carry::Mortar;
    run.players.get_mut(&1).expect("a knight").tool = Tool::Mallet.pack(0);
    let saved = serde_json::to_value(&run).expect("a run writes");
    assert!(saved.get("hands").is_some());
    assert_eq!(saved["players"]["1"]["tool"], Tool::Mallet.pack(0));
    let back: Run = serde_json::from_value(saved.clone()).expect("reads");
    assert_eq!(back.hands.yard, run.hands.yard);
    assert_eq!(back.hands.seats, run.hands.seats);
    assert_eq!(back.players[&1].tool, run.players[&1].tool);
    assert_eq!(back.deeds_of(1, Trade::Setter), 0, "a save leaves the run's marks out: the realm has settled them");
    let mut settled = run.clone();
    settled.home.trades.settle(&settled.marks);
    settled.marks.clear();
    let kept: Run = serde_json::from_value(serde_json::to_value(&settled).expect("writes")).expect("reads");
    assert_eq!(kept.deeds_of(1, Trade::Setter), 40, "and what the realm keeps rides the save");
    // The yard as a run starts is left out, and an old save loads it so.
    let plain = serde_json::to_value(Run::at_home(7, 1, None, Default::default(), Default::default())).expect("writes");
    assert!(plain.get("hands").is_none());
    let mut old = saved;
    old.as_object_mut().expect("a map").remove("hands");
    let loaded: Run = serde_json::from_value(old).expect("an old save reads");
    assert!(loaded.hands.is_default());
    // A knight with no tool writes none.
    let bare = serde_json::to_value(&Run::at_home(7, 1, None, Default::default(), Default::default())).expect("writes");
    assert!(bare["players"]["1"].get("tool").is_none());
}

#[test]
fn the_deeds_are_marks_the_home_outbox_carries_and_the_realm_settles_into_the_knight() {
    let mut run = party(1);
    run.teach_trade(1, Trade::Quarryman, 1);
    run.earn_deeds(1, Trade::Quarryman, 4);
    let key = run.trade_key(1).expect("a key");
    assert_eq!(run.marks.get(&trades::mark_key(Trade::Quarryman, &key)), Some(&5));
    // The private outbox carries the mark and accepts it.
    let outbox = crate::rules::home_settlement::HomeOutbox::capture(&run).expect("the outbox takes trade marks");
    assert!(outbox.marks.keys().any(|k| k.starts_with("trade:quarryman:")));
    // The realm settles it into the record; the run's reading is the same after.
    let before = run.deeds_of(1, Trade::Quarryman);
    let mut ledger = Ledger::default();
    ledger.settle(&run.marks);
    run.home.trades = ledger;
    run.marks.clear();
    assert_eq!(run.deeds_of(1, Trade::Quarryman), before);
    assert_eq!(before, 5);
    // A rank earned says so.
    run.earn_deeds(1, Trade::Quarryman, JOURNEYMAN);
    assert!(run.found.as_ref().is_some_and(|(_, id, t)| *id == 1 && t.contains("JOURNEYMAN QUARRYMAN")), "{:?}", run.found);
}

#[test]
fn a_trade_is_worth_something_below_the_blade_the_barricade_and_the_hollow_wall() {
    let mut run = party(1);
    let hero = run.players[&1].clone();
    assert_eq!((run.trades().edge(&hero), run.brace_for(1), run.eye_for(1)), (0, 0, 0.0));
    run.teach_trade(1, Trade::Smith, MASTER);
    run.teach_trade(1, Trade::Carpenter, JOURNEYMAN);
    run.teach_trade(1, Trade::BankerMason, 1);
    assert_eq!(run.trades().edge(&hero), trades::keen_edge(Some(Rank::Master)));
    assert_eq!(run.brace_for(1), trades::brace(Some(Rank::Journeyman)));
    assert_eq!(run.eye_for(1), trades::eye_reach(Some(Rank::Apprentice)));
}

#[test]
fn the_gauges_follow_the_clocks_the_taps_are_judged_by() {
    // The plumb hangs true every half swing, the bubble rides the mark once a drift.
    for since in [18, 36, 54] {
        assert_eq!(hands::plumb_dev(since), 0);
        assert!(hands::bob(since).abs() < 0.01);
    }
    assert_eq!(hands::plumb_dev(9), 9);
    assert_eq!(hands::level_dev(22), 1);
    assert!(hands::level_dev(0) >= 22, "the worst moment is the drift's start: {}", hands::level_dev(0));
    assert!(hands::bubble(0) < -0.99 && hands::bubble(45) > 0.99 && hands::bubble(90) < -0.99);
    assert!(hands::bubble(22).abs() < 0.1);
    assert_eq!(Grade::of(0, Rank::Apprentice), Grade::Perfect);
    assert_eq!(Grade::of(3, Rank::Apprentice), Grade::Good);
    assert_eq!(Grade::of(4, Rank::Apprentice), Grade::Off);
    assert_eq!(Grade::of(4, Rank::Journeyman), Grade::Good, "a steadier hand has more room");
    // A hand's meter is what the renderer draws.
    let hand = Hand { work: Some(Work::Strike { job: Job::Quarry, began: 100, hits: 2, flaws: 0, last: 130 }), ..Default::default() };
    let Some(Meter::Beat { off, hits, need, .. }) = hand.meter(112) else { panic!("a beat gauge") };
    assert!((off - 0.5).abs() < 0.01 && hits == 2 && need == 5);
}

#[test]
fn a_guest_reads_the_hands_the_yard_and_the_tool_off_the_wire() {
    use crate::wire::{publish, read};
    let mut run = party(2);
    run.teach_trade(1, Trade::Setter, 3);
    run.hands.yard.course = 4;
    run.hands.seats.entry(2).or_default().carry = Carry::Stone;
    run.players.get_mut(&2).expect("a knight").tool = Tool::Hod.pack(1);
    run.sites.keep_mut().expect("site").lay_set(1, true);
    let tree = publish(&run).expect("publishes");
    assert!(tree.field("hands").is_some(), "the hands are on the wire");
    let seen = read(&tree).expect("a guest reads it");
    assert_eq!(seen.hands.yard, run.hands.yard);
    assert_eq!(seen.hands.seats, run.hands.seats);
    assert_eq!(seen.players[&2].tool, Tool::Hod.pack(1), "the tool is in the knight's hand on every screen");
    assert_eq!(seen.sites, run.sites, "and the true stones, with the marks");
    assert!(seen.sites.keep().expect("site").is_true(site::START));
    assert_eq!(seen.deeds_of(1, Trade::Setter), 3, "and the deeds the run has marked");
    assert_eq!(publish(&seen).expect("publishes"), tree, "the guest's run is the same tree");
}

#[test]
fn a_masons_eye_hears_the_hollow_wall_below_and_a_masters_mallet_works_it_free() {
    // A first floor with a vault behind a cracked wall, the party in its room.
    let floor = || {
        let mut run = (0..200u64)
            .map(|seed| {
                let mut run = Run::new(seed, 1, None);
                run.begin_in(Pack::Crypt);
                run
            })
            .find(|run| run.dungeon.secret.is_some())
            .expect("a first floor with a vault");
        let host = run.dungeon.secret.expect("a vault").host;
        run.enter_for_test(host);
        run.calm_for_test();
        run.rune = None;
        run
    };
    // A knight `d` units from the crack, inward.
    let at = |run: &mut Run, d: f32| {
        let (_, cx, cy) = run.dungeon.crack().expect("a crack");
        let (w, h) = (run.room().width(), run.room().height());
        let (ux, uy) = (w / 2.0 - cx, h / 2.0 - cy);
        let n = ux.hypot(uy).max(0.001);
        let hero = run.players.get_mut(&1).expect("a knight");
        (hero.x, hero.y, hero.hp) = (cx + ux / n * d, cy + uy / n * d, hero.max_hp);
    };
    let heard = |run: &Run| run.found.as_ref().is_some_and(|(_, _, t)| t.contains("MASON'S EYE"));
    // No trade: nothing is heard, a knight at the crack or far from it.
    let mut run = floor();
    at(&mut run, 2.0);
    run.step(&BTreeMap::new());
    assert!(!heard(&run));
    // An Apprentice hears it only with a hand on it; a Journeyman across the room.
    let mut run = floor();
    run.teach_trade(1, Trade::BankerMason, 1);
    at(&mut run, 6.0);
    run.step(&BTreeMap::new());
    assert!(!heard(&run), "an Apprentice does not hear it from six units");
    at(&mut run, 2.0);
    run.step(&BTreeMap::new());
    assert!(heard(&run), "but with a hand on it");
    let mut run = floor();
    run.teach_trade(1, Trade::Setter, JOURNEYMAN);
    at(&mut run, 8.0);
    run.step(&BTreeMap::new());
    assert!(heard(&run), "a Journeyman hears it across the room");
    assert!(!run.dungeon.secret.expect("a vault").found, "hearing is not opening");
    // A Master's swing at it works it free: no bomb.
    let mut run = floor();
    run.teach_trade(1, Trade::Setter, MASTER);
    at(&mut run, 14.0);
    run.step(&BTreeMap::from([(1, swing())]));
    assert!(!run.dungeon.secret.expect("a vault").found, "not from across the hall");
    // (A swing in the empty hall has nothing to hold the room's time for; a
    // few quiet ticks anyway.)
    for _ in 0..12 {
        run.step(&BTreeMap::new());
    }
    at(&mut run, 2.0);
    run.step(&BTreeMap::new());
    run.step(&BTreeMap::from([(1, swing())]));
    let (_, cx, cy) = run.dungeon.crack().unwrap_or((0, 0.0, 0.0));
    let h = &run.players[&1];
    assert!(
        run.dungeon.secret.expect("a vault").found,
        "the wall is a doorway: tick {} knight ({}, {}) crack ({cx}, {cy}) found {:?} rank {:?} reach {} stone {} hp {}",
        run.tick, h.x, h.y, run.found, run.rank_of(1, Trade::Setter), run.eye_for(1), h.stone, h.hp
    );
    assert!(run.cues.iter().any(|c| c == "secret_found"));
}

#[test]
fn the_squires_guilds_hired_squires_are_hands_at_the_yard_for_the_knight_that_hired_them() {
    use crate::rules::hireling::AllyKind;
    use crate::rules::squire_guild::{Roster, Squire};
    let mut run = party(1);
    assert_eq!(run.squire_hands(), 0);
    let name = run.players[&1].name.clone();
    let squire = |id: u64| Squire { id, kind: AllyKind::Ninja, name: format!("Tam {id}"), vitality: 40, attack: 6, level: 1, training: None };
    run.home.squire_guild.rosters.insert(name, Roster { members: vec![squire(1), squire(2)], active: None });
    assert_eq!(run.squire_hands(), 2, "two hired squires, two more hands");
    // And a knight who has fallen brings none.
    run.players.get_mut(&1).expect("a knight").hp = 0;
    assert_eq!(run.squire_hands(), 0);
}

/// A party of knights each at `trades[k]`'s work for `ticks`, who play it by
/// the clock: the quarryman at the outcrop, the banker at a bench, the hodman
/// between the heap and the wall, the setter along the course.
fn work_party(trades: &[Trade], ticks: u32) -> Run {
    let mut run = party(trades.len() as u32);
    for (k, &t) in trades.iter().enumerate() {
        run.teach_trade(k as u32 + 1, t, 1);
    }
    run.hands.yard.mortar = 12;
    run.hands.yard.tub = hands::TUB_MAX;
    for _ in 0..ticks {
        let mut keys = Vec::new();
        for (k, &t) in trades.iter().enumerate() {
            let id = k as u32 + 1;
            let input = match t {
                Trade::Quarryman => {
                    stand(&mut run, id, hands::QUARRY);
                    striker(&run, id)
                }
                Trade::BankerMason => {
                    let b = life::BANKERS[1];
                    stand(&mut run, id, [b[0], b[1] + 1.15]);
                    striker(&run, id)
                }
                Trade::Labourer => {
                    let carrying = run.hands.seats[&id].carry != Carry::Nothing;
                    if carrying {
                        let next = hands::stone_at(castle(), laid(&run));
                        stand(&mut run, id, next);
                        Input { interact: true, ..Default::default() }
                    } else if run.hands.yard.cut > 0 {
                        stand(&mut run, id, life::YARD);
                        Input { interact: run.tick % 2 == 0, ..Default::default() }
                    } else {
                        Input::default()
                    }
                }
                _ => {
                    if run.hands.seats[&id].work.is_none() {
                        let next = hands::stone_at(castle(), laid(&run));
                        stand(&mut run, id, next);
                    }
                    setter(&run, id, 0, (false, false))
                }
            };
            keys.push((id, input));
        }
        step(&mut run, &keys);
    }
    run
}

#[test]
fn a_chain_of_knights_sets_the_wall_faster_than_one_with_the_hired_hands_alone() {
    let start = site::START;
    let ticks = HZ * 90;
    let solo = work_party(&[Trade::Setter], ticks);
    let chain = work_party(&[Trade::Quarryman, Trade::BankerMason, Trade::Labourer, Trade::Setter], ticks);
    let (a, b) = (laid(&solo) - start, laid(&chain) - start);
    println!("ninety seconds: a setter alone {a} stones, a chain of four knights {b}");
    assert!(a > 0, "the hired hands feed even a lone setter");
    assert!(b > a * 3 / 2, "a chain of knights is faster: {b} against {a}");
    // And the stones they set are theirs: the setter's seat leads the tally.
    let site = chain.sites.keep().expect("site");
    let mine = site.tally().into_iter().find(|&(seat, _)| seat == 4).map_or(0, |(_, n)| n);
    assert!(mine * 2 >= b, "the setter set most of them: {mine} of {b}");
}

#[test]
fn the_bench_cuts_from_the_stockyard_a_block_from_below_stands_in() {
    let mut run = party(1);
    run.teach_trade(1, Trade::BankerMason, 1);
    let bench = life::BANKERS[0];
    stand(&mut run, 1, [bench[0], bench[1] + 1.15]);
    stock(&mut run, 0, 0, 0, 0);
    // One block of limestone is set down where the Delve's carry sets them.
    assert_eq!(run.stock_rough("limestone", 1), 1);
    assert_eq!(run.home.stockyard.count("limestone"), 1);
    run.hands.yard.acc = [0; 8];
    for _ in 0..400 {
        let input = striker(&run, 1);
        step(&mut run, &[(1, input)]);
        if run.hands.yard.cut > 0 {
            break;
        }
    }
    assert_eq!((run.hands.yard.cut, run.home.stockyard.held()), (1, 0), "the one store gave its block to the bench");
    // The hillside's rubble is cut as any rough stone, and fills no more of the yard than its share.
    assert_eq!(run.stock_rough("rubble", 50), crate::rules::stockyard::Stockyard::RUBBLE_MOST);
    assert_eq!(run.stock_rough("rubble", 1), 0);
}

#[test]
fn a_knight_sets_a_stone_a_raid_knocked_out_where_it_stood_before_any_new_stone() {
    let mut run = party(1);
    run.teach_trade(1, Trade::Setter, 1);
    let start = laid(&run);
    // The wall's height is what it was, with a stone out of the lift in hand.
    let hole = start - 5;
    assert_eq!(run.sites.keep_mut().unwrap().lose(&[hole]), 1);
    assert_eq!(run.sites.keep().unwrap().next(), hole);
    stock(&mut run, 0, 0, 3, 3);
    // The course is at the hole: the knight stands there, not at the top of the wall.
    stand(&mut run, 1, hands::stone_at(castle(), hole));
    for _ in 0..300 {
        let input = setter(&run, 1, 0, (false, false));
        step(&mut run, &[(1, input)]);
        if run.sites.keep().unwrap().lost.is_empty() {
            break;
        }
    }
    let site = run.sites.keep().expect("site");
    assert!(site.lost.is_empty(), "the stone is set again");
    assert_eq!(site.laid, start, "the wall did not go higher for it");
    assert!(site.is_true(hole), "a true set is stored, for the new stone");
    assert!(site.stands(hole) && site.valid());
    assert_eq!((run.hands.yard.course, run.hands.yard.mortar), (2, 2), "a stone and a dab spent");
}
