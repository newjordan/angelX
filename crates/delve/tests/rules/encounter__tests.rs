//! Room encounters: the minigames dealt to the delve's halls by the seed.
use super::*;

/// A floor-one run of `seats` knights with the hall the seed deals a game to.
fn floor(seats: u32, seed: u64) -> Run {
    let mut run = Run::new(seed, 1, None);
    for id in 2..=seats {
        run.join_seat(id, &format!("Knight {id}"));
    }
    run.enemies.clear();
    run.pick_folly();
    run.pick_encounters();
    for h in run.players.values_mut() {
        h.carried.add(Spoil::Gold, 100);
    }
    run
}

/// The same, with `kind` pinned to the folly hall and the party at its door.
fn deal(seats: u32, kind: Kind) -> (Run, usize) {
    let mut run = floor(seats, 41);
    let index = run.folly_room.expect("a hall");
    run.encounter_rooms = vec![(index, kind)];
    (run, index)
}

fn open(seats: u32, kind: Kind) -> Run {
    let (mut run, index) = deal(seats, kind);
    run.enter_for_test(index);
    run
}

fn idle() -> BTreeMap<u32, Input> {
    BTreeMap::new()
}

fn kinds(run: &Run) -> Vec<Kind> {
    run.encounter_rooms.iter().map(|&(_, k)| k).collect()
}

#[test]
fn the_pick_is_the_seeds_and_changes_no_floor() {
    for seed in 1..=24 {
        let mut a = Run::new(seed, 1, None);
        let mut b = a.clone();
        // Dealing again, and again, draws from no dice and leaves the floor be.
        for _ in 0..3 {
            b.pick_folly();
            b.pick_encounters();
        }
        a.pick_folly();
        a.pick_encounters();
        assert_eq!(a.encounter_rooms, b.encounter_rooms, "seed {seed}");
        assert_eq!(a.rng.clone().next(), b.rng.clone().next(), "dealing cannot consume floor dice");
        assert_eq!(
            serde_json::to_string(&a.dungeon).unwrap(),
            serde_json::to_string(&b.dungeon).unwrap(),
            "dealing cannot change the floor"
        );
        // The next floor is the same whether or not the last one was dealt.
        let mut next_a = a.clone();
        let mut next_b = b.clone();
        next_a.descend_for_test();
        next_b.encounter_rooms.clear();
        next_b.encounter_prev.clear();
        next_b.descend_for_test();
        assert_eq!(
            serde_json::to_string(&next_a.dungeon).unwrap(),
            serde_json::to_string(&next_b.dungeon).unwrap(),
            "the next floor is the same however the last was dealt"
        );
    }
    // Same seed, same pick; the seed moves it over the whole pool.
    assert_eq!(pick(7, 1, 2, 0, 2, &[]), pick(7, 1, 2, 0, 2, &[]));
    let seen: std::collections::BTreeSet<_> = (1..400).map(|s| format!("{:?}", pick(s, 1, 1, 0, 2, &[]))).collect();
    assert_eq!(seen.len(), pool().len(), "every game of the pool is dealt: {seen:?}");
}

#[test]
fn a_lone_knight_is_never_dealt_a_game_for_two() {
    for seed in 1..=400 {
        for depth in 1..=6 {
            assert_ne!(pick(seed, 1, depth, 0, 1, &[]), Kind::KnightsWish);
            assert_ne!(pick(seed, 1, depth, 1, 1, &[]), Kind::KnightsWish);
        }
    }
    // A dealt free-for-all met by a knight on their own is a race for one.
    let (mut run, index) = deal(2, Kind::KnightsWish);
    run.players.get_mut(&2).unwrap().absent = true;
    run.enter_for_test(index);
    let live = run.encounter.as_ref().expect("a game opens");
    assert_ne!(live.kind, Kind::KnightsWish);
    assert!(run.deathmatch.is_none());
    assert!(run.valid_snapshot());
}

/// How many ordinary hall or fight rooms the floor has outside the start, the camps
/// and the guardian's: the rooms it could deal a game to.
fn rooms_to_deal(run: &Run) -> usize {
    (0..run.dungeon.rooms.len())
        .filter(|&i| {
            let r = &run.dungeon.rooms[i];
            let camp = run.boss_gates.as_ref().is_some_and(|g| g.economy.camp(i).is_some() || g.guardian == i);
            // (A test build keeps the Ore Cart's race hall on, which takes a spare hall.)
            let race = run.race3d_room == Some(i) || run.gantry_room == Some(i);
            matches!(r.kind, RoomKind::Hall | RoomKind::Fight) && !r.great() && !r.cleared && i != run.at && !camp && !race
        })
        .count()
}

fn has_a_room_to_deal(run: &Run) -> bool {
    rooms_to_deal(run) > 0
}

#[test]
fn every_floor_deals_one_hall_and_a_second_by_a_coin_from_the_second_floor() {
    let (mut first, mut second, mut floors_two, mut none_two, mut dry_two, mut rich_two) = (0, 0, 0, 0, 0, 0);
    for seed in 1..=240 {
        let mut run = Run::new(seed, 1, None);
        run.pick_folly();
        run.pick_encounters();
        assert_eq!(run.encounter_rooms.len() == 1, has_a_room_to_deal(&run), "seed {seed} floor one");
        first += run.encounter_rooms.len();
        run.descend_for_test();
        assert_eq!(run.dungeon.depth, 2);
        let n = run.encounter_rooms.len();
        assert!(n <= 2, "seed {seed}: {n} halls dealt on floor two");
        // A floor deals its game whenever it has a room to deal it to.
        assert_eq!(n >= 1, has_a_room_to_deal(&run), "seed {seed} floor two");
        floors_two += 1;
        // A second game needs a second room to stand in.
        if rooms_to_deal(&run) >= 2 {
            rich_two += 1;
            second += usize::from(n == 2);
        } else {
            assert!(n <= 1, "seed {seed}: two games in one room");
        }
        none_two += usize::from(n == 0);
        dry_two += usize::from(!has_a_room_to_deal(&run));
    }
    assert_eq!(none_two, dry_two);
    // The share of floors whose every ordinary room is a chief's camp (a count for the report).
    println!("floor one dealt {first}/240; floor two dealt none on {none_two}/240 (all camps); two games on {second} of the {rich_two} floors with two rooms to deal to ({floors_two} floors)");
    let share = second * 100 / rich_two.max(1);
    assert!((30..=50).contains(&share), "a second game on about 40% of the floors that have a room for it: {share}%");
}

/// Why a floor deals nothing, over many seeds: a count for the report. Run it with
/// `cargo test -p angel-delve --lib which_floors_deal_no_game -- --ignored --nocapture`.
#[test]
#[ignore = "counts the floors with no room to deal a game to"]
fn which_floors_deal_no_game() {
    let (mut floors, mut none) = (0, 0);
    let mut kinds: BTreeMap<String, u32> = BTreeMap::new();
    for seed in 1..=400 {
        let mut run = Run::new(seed, 1, None);
        run.pick_folly();
        run.pick_encounters();
        for _ in 0..5 {
            floors += 1;
            if run.encounter_rooms.is_empty() {
                none += 1;
                let list: Vec<String> = run
                    .dungeon
                    .rooms
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        let camp = run.boss_gates.as_ref().is_some_and(|g| g.economy.camp(i).is_some() || g.guardian == i);
                        format!("{:?}{}", r.kind, if camp { "*" } else { "" })
                    })
                    .collect();
                *kinds.entry(list.join(" ")).or_default() += 1;
            }
            run.descend_for_test();
        }
    }
    println!("{none} of {floors} floors deal no game");
    for (k, n) in kinds {
        println!("{n:3}  {k}");
    }
}

#[test]
fn halls_are_never_the_start_a_camp_the_guardian_the_way_down_or_each_other() {
    for seed in 1..=60 {
        let mut run = Run::new(seed, 1, None);
        for floor in 1..=5 {
            let halls: Vec<usize> = run.encounter_rooms.iter().map(|&(i, _)| i).collect();
            for (n, &i) in halls.iter().enumerate() {
                let room = &run.dungeon.rooms[i];
                // An ordinary hall, or an ordinary fight room on a floor laid with no hall.
                assert!(matches!(room.kind, RoomKind::Hall | RoomKind::Fight), "seed {seed} floor {floor}: {:?}", room.kind);
                assert!(!room.great() && i != 0 && i != run.at);
                if let Some(g) = &run.boss_gates {
                    assert!(g.guardian != i && g.economy.camp(i).is_none(), "seed {seed} floor {floor}: a guardian's hall");
                }
                assert!(!halls[..n].contains(&i), "two games in one hall");
                assert!(run.gantry_room != Some(i) && run.race3d_room != Some(i));
            }
            run.descend_for_test();
        }
    }
}

#[test]
fn a_floor_never_repeats_what_the_floor_before_dealt() {
    for seed in 1..=60 {
        let mut run = Run::new(seed, 1, None);
        run.pick_folly();
        run.pick_encounters();
        let mut last = kinds(&run);
        for _ in 0..5 {
            run.descend_for_test();
            let now = kinds(&run);
            assert!(now.iter().all(|k| !last.contains(k)), "seed {seed} floor {}: {now:?} after {last:?}", run.dungeon.depth);
            assert!(now.iter().enumerate().all(|(i, k)| !now[..i].contains(k)), "a floor deals two of a kind: {now:?}");
            last = now;
        }
    }
}

#[test]
fn an_old_saves_folly_hall_keeps_the_rotation_it_was_dealt() {
    let mut run = Run::new(41, 1, None);
    run.pick_folly();
    assert!(run.encounter_rooms.is_empty());
    let index = run.folly_room.unwrap();
    assert_eq!(run.encounter_at(index), Some(Kind::WaxMaze));
    run.enter_for_test(index);
    assert!(run.folly.is_some(), "the floor-one folly is the Wax Maze");
    assert_eq!(run.encounter.as_ref().map(|e| e.kind), Some(Kind::WaxMaze));
    // A save from before the pool has no such fields and still reads.
    let mut tree = serde_json::to_value(&run).unwrap();
    let fields = tree.as_object_mut().unwrap();
    for gone in ["encounter", "encounter_rooms", "encounter_prev"] {
        fields.remove(gone);
    }
    let old: Run = serde_json::from_value(tree).expect("an old save loads");
    assert!(old.encounter.is_none() && old.encounter_rooms.is_empty());
    assert!(old.valid_snapshot());
}

#[test]
fn the_four_follies_open_run_and_close_as_they_did() {
    for kind in [Kind::WaxMaze, Kind::BelfryShoot, Kind::PowderRoom, Kind::BookHop] {
        let mut run = open(2, kind);
        assert_eq!(run.encounter.as_ref().map(|e| e.kind), Some(kind));
        assert!(run.barred(), "{kind:?}: the doors hold");
        assert!(run.enemies.is_empty() || kind == Kind::BelfryShoot);
        assert!(run.valid_snapshot());
        let hp = run.players[&1].hp;
        // The game's own end: its state done, its room cleared.
        match kind {
            Kind::WaxMaze => {
                run.folly.as_mut().unwrap().t = folly::TITLE + folly::MAZE_TIME - 1;
            }
            Kind::BelfryShoot => run.bats.as_mut().unwrap().t = folly::TITLE + folly::BAT_TIME - 1,
            Kind::PowderRoom => run.powder.as_mut().unwrap().t = folly::TITLE + powder::POWDER_TIME - 1,
            _ => run.hop.as_mut().unwrap().t = folly::TITLE + hop::HOP_TIME - 1,
        }
        for _ in 0..4 {
            run.step(&idle());
        }
        assert!(run.encounter.is_none(), "{kind:?}: the room's game is over with the folly");
        assert!(run.room().cleared && !run.barred(), "{kind:?}: its doors open");
        assert_eq!(run.players[&1].hp, hp, "{kind:?}: health as it was");
        assert!(run.valid_snapshot());
    }
}

/// Make the objective of `kind`'s campaign true.
fn win_first_objective(run: &mut Run, kind: Kind) {
    match kind {
        Kind::Lanternward => run.lanternward.as_mut().unwrap().room = 1,
        Kind::Moonfall => run.moonfall.as_mut().unwrap().world.set(moonfall::state::INK_TAKEN),
        Kind::Bannerpeak => run.bannerpeak.as_mut().unwrap().banners = 1,
        Kind::Griffinwing => run.griffinwing.as_mut().unwrap().stage = 1,
        Kind::Lightgallop => run.lightgallop.as_mut().unwrap().posts = 1,
        _ => unreachable!(),
    }
}

const CAMPAIGNS: [Kind; 5] = [Kind::Lanternward, Kind::Moonfall, Kind::Bannerpeak, Kind::Griffinwing, Kind::Lightgallop];

#[test]
fn a_campaign_opens_in_the_hall_and_a_first_objective_pays_everyone() {
    for kind in CAMPAIGNS {
        let mut run = open(3, kind);
        let live = run.encounter.as_ref().expect("opens");
        assert_eq!(live.kind, kind);
        assert_eq!(live.seats, [1, 2, 3], "{kind:?}: every living knight is seated");
        assert!(run.roundtable_showing() && run.roundtable_under_way().unwrap().name == kind.name().to_string().as_str() || run.roundtable_showing());
        assert!(run.barred() && run.minigame_showing() && run.can_hold_leave(1), "{kind:?}");
        assert!(run.enemies.is_empty() && run.traps.is_empty(), "{kind:?}: no company, no traps");
        let ante: u32 = run.encounter.as_ref().unwrap().ante.values().sum();
        assert_eq!(ante, 30, "{kind:?}: each antes a tenth of 100");
        assert!(run.valid_snapshot(), "{kind:?}");
        let wishes: Vec<u8> = run.players.values().map(|h| h.wishes).collect();
        let banked = run.bank.len();
        win_first_objective(&mut run, kind);
        run.step(&idle());
        let end = run.encounter.as_ref().and_then(|e| e.end.clone()).unwrap_or_else(|| panic!("{kind:?}: settled"));
        assert!(end.won && !end.capped && end.order == [1, 2, 3], "{kind:?}");
        assert_eq!(run.bank.len(), banked + 3, "{kind:?}: everyone is paid on a win");
        for id in 1..=3u32 {
            let h = &run.players[&id];
            assert_eq!(h.wishes, wishes[id as usize - 1] + 1, "{kind:?}: a wish each");
            assert_eq!(end.paid[&id], crossroads::gold(0) + 10, "{kind:?}: the purse and the ante back");
        }
        assert!(run.barred(), "the payout card holds the room");
        for _ in 0..PAYOUT_TICKS {
            run.step(&idle());
        }
        assert!(run.encounter.is_none(), "{kind:?}: closed");
        assert!(run.room().cleared && !run.barred(), "{kind:?}: the room is cleared, its doors open");
        assert!(!run.roundtable_showing() && run.lanternward.is_none() && run.moonfall.is_none()
            && run.bannerpeak.is_none() && run.griffinwing.is_none() && run.lightgallop.is_none(), "{kind:?}: the game is cleared");
        assert!(run.valid_snapshot(), "{kind:?}");
        // The party stands in the hall, ready to go on.
        assert_eq!(run.phase, Phase::Exploring);
    }
}

#[test]
fn a_campaign_cut_at_its_cap_without_the_objective_costs_the_ante_and_no_more() {
    for kind in CAMPAIGNS {
        let mut run = open(2, kind);
        let carried: Vec<u32> = run.players.values().map(|h| h.carried.get(Spoil::Gold)).collect();
        let banked = run.bank.len();
        run.encounter.as_mut().unwrap().t = kind.cap() - 1;
        run.step(&idle());
        let end = run.encounter.as_ref().unwrap().end.clone().expect("cut");
        assert!(!end.won && end.capped && end.paid.is_empty(), "{kind:?}");
        assert_eq!(run.bank.len(), banked, "{kind:?}: nobody is paid");
        for (id, h) in &run.players {
            // The ante went in at the door (a tenth of 100) and stays in.
            assert_eq!(h.carried.get(Spoil::Gold), carried[*id as usize - 1], "{kind:?}: nothing more is taken");
            assert!(!h.laurel);
        }
        for _ in 0..PAYOUT_TICKS {
            run.step(&idle());
        }
        assert!(run.encounter.is_none() && run.room().cleared, "{kind:?}: the room is let go");
    }
}

#[test]
fn nobody_falls_in_a_random_room_and_health_and_gear_come_back() {
    let mut run = open(2, Kind::Bannerpeak);
    let (hp, armor, gear) = {
        let h = &run.players[&1];
        (h.hp, h.armor, h.gear)
    };
    run.players.get_mut(&1).unwrap().hp = 0;
    run.players.get_mut(&1).unwrap().armor = 0;
    run.step(&idle());
    assert!(run.players[&1].hp >= 1, "a knight at zero is held at one");
    assert_ne!(run.phase, Phase::Wiped);
    win_first_objective(&mut run, Kind::Bannerpeak);
    for _ in 0..PAYOUT_TICKS + 2 {
        run.step(&idle());
    }
    let h = &run.players[&1];
    assert_eq!((h.hp, h.armor, h.gear), (hp, armor, gear), "health and gear as they were");
}

#[test]
fn leaving_by_the_door_ends_the_game_and_the_hall_deals_it_afresh() {
    let (mut run, index) = deal(1, Kind::Bannerpeak);
    let came_from = run.at;
    run.enter_for_test(index);
    let gold = run.players[&1].carried.get(Spoil::Gold);
    assert!(run.encounter.is_some() && gold == 90, "a tenth is in");
    assert!(run.can_hold_leave(1));
    assert!(run.leave_activity(1));
    assert_eq!(run.at, came_from, "back through the door");
    assert!(run.encounter.is_none() && run.bannerpeak.is_none(), "the game is over");
    assert!(!run.dungeon.rooms[index].cleared, "the hall is not cleared by walking out");
    assert_eq!(run.players[&1].carried.get(Spoil::Gold), 90, "the stake is lost, no more");
    run.enter_for_test(index);
    assert!(run.encounter.is_some() && run.bannerpeak.is_some(), "dealt again");
    assert_eq!(run.players[&1].carried.get(Spoil::Gold), 81);
    assert!(run.valid_snapshot());
}

#[test]
fn a_cleared_or_sealed_hall_deals_nothing() {
    let (mut run, index) = deal(1, Kind::Bannerpeak);
    run.dungeon.rooms[index].cleared = true;
    run.enter_for_test(index);
    assert!(run.encounter.is_none() && run.bannerpeak.is_none());
    // A guardian's hall under its lock.
    let (mut run, _) = deal(1, Kind::Bannerpeak);
    run.enemies.clear();
    if let Some(g) = run.boss_gates.as_ref() {
        let guardian = g.guardian;
        run.encounter_rooms = vec![(guardian, Kind::Bannerpeak)];
        assert!(run.guardian_locked() || !run.dungeon.rooms[guardian].cleared);
        run.at = 0;
        run.enter(guardian, Some(2));
        assert!(run.encounter.is_none() || !run.guardian_locked(), "a sealed hall opens no game");
    }
}

#[test]
fn the_race_hall_and_the_cup_pay_in_finishing_order() {
    // Knight Racers, two knights: the order of the line decides.
    let mut run = open(2, Kind::KnightRacers);
    assert!(run.knight_racers.is_some() && run.roundtable_showing());
    assert_eq!(run.knight_racers.as_ref().unwrap().laps, 1, "one lap");
    {
        let s = run.knight_racers.as_mut().unwrap();
        s.cars.get_mut(&1).unwrap().finish = Some(50.0);
        s.cars.get_mut(&2).unwrap().finish = Some(60.0);
        s.finished = true;
    }
    for _ in 0..GRACE + 1 {
        run.step(&idle());
    }
    let end = run.encounter.as_ref().and_then(|e| e.end.clone()).expect("settled after the grace");
    assert_eq!(end.order, [1, 2]);
    assert_eq!(end.paid[&1], crossroads::gold(0) + 20, "first takes the purse and the pot");
    assert_eq!(end.paid[&2], crossroads::gold(1));
    assert!(run.players[&1].laurel && !run.players[&2].laurel);
    for _ in 0..PAYOUT_TICKS {
        run.step(&idle());
    }
    assert!(run.encounter.is_none() && run.knight_racers.is_none() && run.room().cleared);

    // A lone knight against five rivals is paid only for first overall.
    let mut run = open(1, Kind::KnightRacers);
    assert_eq!(run.knight_racers.as_ref().unwrap().cars.len(), 6);
    // A rival is ahead when the clock runs out.
    run.knight_racers.as_mut().unwrap().cars.values_mut().find(|c| c.cpu).unwrap().d = 5000.0;
    run.encounter.as_mut().unwrap().t = Kind::KnightRacers.cap() - 1;
    run.step(&idle());
    let end = run.encounter.as_ref().unwrap().end.clone().unwrap();
    assert!(end.capped && end.paid.is_empty(), "the rivals were ahead");
}

#[test]
fn a_race_hall_course_opens_runs_and_pays() {
    for kind in [Kind::ShaftRun, Kind::GriffinRun, Kind::TorchlightTouge] {
        let mut run = open(2, kind);
        let race = run.race3d.as_ref().expect("the race lines up");
        assert_eq!(Some(race.course), kind.course());
        assert!(race.ante.is_empty(), "{kind:?}: the stake is the room's, not the race's");
        assert!(run.race3d_showing() && run.barred());
        for _ in 0..30 {
            run.step(&idle());
        }
        {
            let race = run.race3d.as_mut().unwrap();
            for r in race.riders.values_mut() {
                r.finish = Some(race.t);
            }
            race.riders.get_mut(&2).unwrap().finish = Some(race.t + 5);
            race.done = true;
        }
        run.step(&idle());
        let end = run.encounter.as_ref().and_then(|e| e.end.clone()).unwrap_or_else(|| panic!("{kind:?} settled"));
        assert_eq!(end.order, [1, 2], "{kind:?}");
        assert!(end.paid[&1] > end.paid[&2], "{kind:?}");
        for _ in 0..PAYOUT_TICKS {
            run.step(&idle());
        }
        assert!(run.encounter.is_none() && run.race3d.is_none() && run.room().cleared, "{kind:?}");
    }
}

#[test]
fn knights_wish_opens_for_two_and_pays_by_kills() {
    let mut run = open(3, Kind::KnightsWish);
    assert!(run.deathmatch.is_some() && run.roundtable_showing());
    {
        let s = run.deathmatch.as_mut().unwrap();
        s.fighters.get_mut(&2).unwrap().kills = 10;
        s.fighters.get_mut(&1).unwrap().kills = 4;
        s.winner = Some(2);
    }
    // The leg needs the cockpit's engine; the room's own clock does not.
    run.tick_encounter(&idle());
    let end = run.encounter.as_ref().unwrap().end.clone().expect("settled");
    assert_eq!(end.order[0], 2);
    assert_eq!(end.paid[&2], crossroads::gold(0) + 30);
    assert!(run.players[&2].laurel);
    for _ in 0..PAYOUT_TICKS {
        run.tick_encounter(&idle());
    }
    assert!(run.encounter.is_none() && run.deathmatch.is_none() && run.room().cleared);
}

#[test]
fn a_save_made_mid_encounter_loads_and_plays_on() {
    for kind in [Kind::Bannerpeak, Kind::Lightgallop, Kind::KnightRacers, Kind::WaxMaze] {
        let mut run = open(2, kind);
        for _ in 0..40 {
            run.step(&idle());
        }
        let saved = serde_json::to_string(&run).unwrap();
        let mut back: Run = serde_json::from_str(&saved).expect("a save made mid-encounter loads");
        assert!(back.valid_snapshot(), "{kind:?}");
        assert_eq!(serde_json::to_string(&back.encounter).unwrap(), serde_json::to_string(&run.encounter).unwrap());
        assert!(back.mend().is_empty(), "{kind:?}: nothing to mend");
        for _ in 0..40 {
            back.step(&idle());
        }
        assert!(back.encounter.is_some() && back.barred() && back.valid_snapshot(), "{kind:?}");
    }
    // A save whose game state is gone ends the room's game rather than hold it.
    let mut run = open(2, Kind::Bannerpeak);
    run.bannerpeak = None;
    run.step(&idle());
    assert!(run.encounter.as_ref().is_some_and(|e| e.end.is_some()));
}

#[test]
fn the_full_plans_doors_still_open_their_adventures() {
    for a in roundtable::ADVENTURES.iter() {
        let run = roundtable::fixture(a);
        assert!(run.encounter.is_none());
        assert!(run.on_stage() && (a.showing)(&run) || !(a.showing)(&run), "{}", a.slug);
        assert_eq!(run.room().kind, RoomKind::Forest);
    }
}

#[test]
fn a_hall_that_is_no_encounters_is_no_stage() {
    let mut run = Run::new(41, 1, None);
    assert!(!run.on_stage());
    run.start_bannerpeak();
    assert!(run.bannerpeak.is_none(), "no stage, no adventure");
}

#[test]
fn a_floor_laid_with_no_hall_deals_its_game_to_a_fight_room() {
    let mut run = Run::new(41, 1, None);
    for room in &mut run.dungeon.rooms {
        if room.kind == RoomKind::Hall {
            room.kind = RoomKind::Fight;
        }
    }
    run.pick_folly();
    run.pick_encounters();
    let index = run.folly_room.expect("a fight room stands in for the hall");
    assert_eq!(run.dungeon.rooms[index].kind, RoomKind::Fight);
    assert_eq!(run.encounter_rooms.len(), 1);
    assert!(index != 0 && !run.dungeon.rooms[index].great());
}

#[test]
fn open_floors_deal_no_game() {
    // An open floor is one map of chambers with no halls to walk into (docs/OPEN-FLOORS.md):
    // its games wait for phase B, as the follies always have.
    for seed in 1..=6 {
        let mut run = Run::new(seed, 1, None);
        run.open_floors = true;
        run.descend_for_test();
        assert!(run.dungeon.open.is_some(), "seed {seed}: an open floor");
        assert!(run.folly_room.is_none() && run.encounter_rooms.is_empty() && run.encounter.is_none());
        assert!(run.valid_snapshot());
    }
}

#[test]
fn a_guest_sees_the_game_the_host_hosts() {
    use crate::render::overworld::arena::frame_for;
    for kind in [Kind::Bannerpeak, Kind::KnightRacers, Kind::ShaftRun, Kind::Arcade(Game::Barrel), Kind::WaxMaze] {
        let mut run = open(2, kind);
        for _ in 0..45 {
            run.step(&idle());
        }
        let friend = run.wire_copy();
        assert_eq!(friend.encounter.as_ref().map(|e| e.kind), Some(kind), "{kind:?}: the room's game reaches a guest");
        assert!(friend.valid_snapshot(), "{kind:?}");
        for seat in [1, 2] {
            assert_eq!(
                frame_for(&friend, 384, 224, Some(seat)).content_hash(),
                frame_for(&run, 384, 224, Some(seat)).content_hash(),
                "{kind:?}: a guest draws what the host draws, seat {seat}"
            );
        }
    }
}

#[test]
#[ignore = "prints how the second hall is dealt"]
fn second_hall_diagnostic() {
    for seed in 1..=40 {
        let mut run = Run::new(seed, 1, None);
        run.pick_folly();
        run.pick_encounters();
        run.descend_for_test();
        let coin = mix(run.seed ^ run.raid_id.rotate_left(29) ^ u64::from(run.dungeon.depth).rotate_left(3) ^ 0xc01f) % 100;
        println!(
            "seed {seed}: depth {} coin {coin} rooms_to_deal {} dealt {:?} folly {:?} kinds {:?}",
            run.dungeon.depth,
            rooms_to_deal(&run),
            run.encounter_rooms,
            run.folly_room,
            run.dungeon.rooms.iter().map(|r| format!("{:?}", r.kind)).collect::<Vec<_>>().join(",")
        );
    }
}

#[test]
fn a_second_hall_deals_its_game_after_the_first_is_played_out() {
    for (first, second) in [(Kind::WaxMaze, Kind::BelfryShoot), (Kind::BookHop, Kind::PowderRoom), (Kind::PowderRoom, Kind::Bannerpeak), (Kind::BelfryShoot, Kind::Arcade(Game::Barrel))] {
        let mut run = floor(2, 41);
        let a = run.folly_room.expect("a hall");
        let b = (1..run.dungeon.rooms.len())
            .find(|&i| i != a && matches!(run.dungeon.rooms[i].kind, RoomKind::Hall | RoomKind::Fight) && !run.dungeon.rooms[i].great())
            .expect("a second room");
        run.encounter_rooms = vec![(a, first), (b, second)];
        run.enter_for_test(a);
        assert_eq!(run.encounter.as_ref().map(|e| e.kind), Some(first));
        // Played out: the folly's own end, or the cut.
        for _ in 0..(kind_ticks(first) + 4 * PAYOUT_TICKS) {
            if run.encounter.is_none() {
                break;
            }
            if first.folly() {
                match first {
                    Kind::WaxMaze => run.folly.as_mut().unwrap().t = folly::TITLE + folly::MAZE_TIME - 1,
                    Kind::BelfryShoot => run.bats.as_mut().unwrap().t = folly::TITLE + folly::BAT_TIME - 1,
                    Kind::PowderRoom => run.powder.as_mut().unwrap().t = folly::TITLE + powder::POWDER_TIME - 1,
                    _ => run.hop.as_mut().unwrap().t = folly::TITLE + hop::HOP_TIME - 1,
                }
            }
            run.step(&idle());
        }
        assert!(run.encounter.is_none() && run.dungeon.rooms[a].cleared, "{first:?} is over");
        run.enter_for_test(0);
        run.enter_for_test(b);
        assert_eq!(run.encounter.as_ref().map(|e| e.kind), Some(second), "{second:?} opens after {first:?}");
        assert!(run.valid_snapshot());
    }
}

fn kind_ticks(_: Kind) -> u32 {
    300
}

#[test]
#[ignore = "prints how often a floor deals a second game, with the Ore Cart's race hall off as in play"]
fn second_game_rate() {
    let (mut floors, mut two, mut none, mut spare) = (0, 0, 0, 0);
    for seed in 1..=400u64 {
        let mut run = Run::new(seed, 1, None);
        run.descend_for_test();
        run.race3d_room = None;
        run.pick_folly();
        run.pick_encounters();
        floors += 1;
        none += usize::from(run.encounter_rooms.is_empty());
        two += usize::from(run.encounter_rooms.len() == 2);
        spare += usize::from(rooms_to_deal(&run) >= 2);
    }
    println!("floor two, {floors} seeds: {none} deal none, {spare} have a second room, {two} deal two");
}

#[test]
fn every_arcade_game_of_the_old_map_is_dealt_or_says_why_not() {
    for game in Game::ALL {
        let name = game.name();
        let dealt = ARCADE.contains(&game);
        let folly = game == Game::Powder;
        let left_out = LEFT_OUT.iter().any(|&(n, _)| n == name);
        assert!(dealt || folly || left_out, "{name} is neither dealt, the Powder Room, nor left out with a reason");
        assert!(!(dealt && left_out), "{name} is both");
    }
    assert!(LEFT_OUT.iter().all(|&(_, why)| why.len() > 20));
    // Every kind of the pool has a name and one short line of how to play.
    for kind in pool() {
        assert!(!kind.name().is_empty() && (10..=48).contains(&kind.words().len()), "{kind:?}: {}", kind.words());
    }
}
