use super::*;
use std::collections::BTreeMap;

fn input(i: Input) -> BTreeMap<u32, Input> {
    BTreeMap::from([(1, i)])
}

fn fire() -> Input {
    Input {
        fire: true,
        ..Default::default()
    }
}

/// A run standing in a floor-two Sanctuary, the knight at the privy's door.
fn at_the_door() -> Run {
    let mut run = Run::new(0, 1, Some("Matt"));
    run.descend_for_test();
    assert_eq!(run.room().kind, RoomKind::Sanctuary);
    run.window = None;
    let (px, py) = run.room().privy().expect("a privy in the Sanctuary");
    for hero in run.players.values_mut() {
        hero.hp = hero.max_hp / 2;
        hero.mana = 10.0;
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (px, py);
    run
}

/// Whether pressing next tick lands the marker in `band` of the middle.
fn lands(run: &Run, band: f32) -> bool {
    let stop = run.players[&1].pit.stop.as_ref().unwrap();
    let phase = (stop.phase + 2.0 / Stop::period(stop.straps)).rem_euclid(2.0);
    let marker = if phase < 1.0 { phase } else { 2.0 - phase };
    (marker - 0.5).abs() <= band
}

/// Play a strap stage perfectly: press on the middle, let go between.
fn strap_cleanly(run: &mut Run) {
    let stage = run.players[&1].pit.stop.as_ref().unwrap().stage;
    let mut down = false;
    for _ in 0..600 {
        let Some(stop) = run.players[&1].pit.stop.as_ref() else {
            return;
        };
        if stop.stage != stage {
            return;
        }
        let press = !down && lands(run, PERFECT_BAND * 0.6) && stop.lock == 0;
        down = press;
        run.step(&input(if press { fire() } else { Input::default() }));
    }
    panic!("the straps never came undone");
}

/// Let the stop run until its stage is `stage`, with no input.
fn wait_for(run: &mut Run, stage: Stage) {
    for _ in 0..1000 {
        if run.players[&1]
            .pit
            .stop
            .as_ref()
            .is_some_and(|s| s.stage == stage)
        {
            return;
        }
        run.step(&input(Input::default()));
    }
    panic!("never reached {stage:?}");
}

#[test]
fn walking_up_to_the_privy_starts_a_stop_in_the_pit_box() {
    let mut run = at_the_door();
    run.step(&BTreeMap::new());
    let stop = run.players[&1].pit.stop.as_ref().expect("a stop begins");
    assert_eq!(stop.stage, Stage::Strip);
    let (px, py) = run.room().privy().unwrap();
    let hero = &run.players[&1];
    assert!((hero.x - px).hypot(hero.y - py) > 2.0, "waved to the box");
    assert!(run.cues.iter().any(|c| c == "privy"));
    assert!(
        run.players[&2].pit.stop.is_none(),
        "the other seat has not approached"
    );
}

#[test]
fn a_full_stop_runs_every_stage_and_keeps_the_time() {
    let mut run = at_the_door();
    run.step(&BTreeMap::new());
    strap_cleanly(&mut run);
    assert_eq!(run.players[&1].gear, 2, "armour off, in smallclothes");
    wait_for(&mut run, Stage::Choose);
    // Still holding the stick the way they were walking: no choice yet.
    for _ in 0..4 {
        run.step(&input(Input {
            move_x: 1,
            ..Default::default()
        }));
    }
    assert_eq!(
        run.players[&1].pit.stop.as_ref().unwrap().stage,
        Stage::Choose,
        "a held stick is not a choice"
    );
    run.step(&input(Input::default()));
    run.step(&input(Input {
        move_x: 1,
        ..Default::default()
    }));
    let stop = run.players[&1].pit.stop.as_ref().unwrap();
    assert_eq!(stop.stage, Stage::Inside);
    assert!(stop.poop);
    assert!(run.players[&1].privy > 0, "behind the shut door");
    wait_for(&mut run, Stage::RunOut);
    assert_eq!(run.players[&1].privy, 0, "out");
    wait_for(&mut run, Stage::Dress);
    strap_cleanly(&mut run);
    let hero = &run.players[&1];
    assert!(hero.pit.stop.is_none(), "done");
    assert_eq!(hero.pit.visits, 1);
    assert_eq!(hero.relieved.map(|r| r.0), Some(run.dungeon.depth));
    assert_eq!(hero.mana, MAX_MANA);
    assert!(hero.hp >= hero.max_hp / 2 + MEND + POOP_MEND, "mended");
    assert!(hero.pit.speed > 0, "a poop's speed buff");
    let entry = &run.pits.entries[0];
    assert_eq!(entry.seat, 1);
    assert!(entry.best > POOP_TICKS, "the time includes the seat");
    assert!(run.found.as_ref().unwrap().2.contains("privy"));
    assert!(run.cues.iter().any(|c| c.starts_with("privy_")));
    // Once a Sanctuary.
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
    }
    assert!(run.players[&1].pit.stop.is_none());
}

#[test]
fn a_fumbled_strap_costs_time_and_a_clean_one_does_not() {
    let clean = {
        let mut run = at_the_door();
        run.step(&BTreeMap::new());
        strap_cleanly(&mut run);
        run.players[&1].pit.stop.as_ref().unwrap().times[0]
    };
    let mut run = at_the_door();
    run.step(&BTreeMap::new());
    // Hit the first strap at the very end of the bar: a miss, hands locked.
    for _ in 0..200 {
        let stop = run.players[&1].pit.stop.as_ref().unwrap();
        if stop.marker() < 0.04 && stop.lock == 0 {
            break;
        }
        run.step(&input(Input::default()));
    }
    run.step(&input(fire()));
    let stop = run.players[&1].pit.stop.as_ref().unwrap();
    assert_eq!(stop.straps, 0, "nothing came undone");
    assert_eq!(stop.last.map(|l| l.0), Some(Grade::Missed));
    assert!(stop.lock > 0, "the hands are locked");
    run.step(&input(Input::default()));
    strap_cleanly(&mut run);
    let fumbled = run.players[&1].pit.stop.as_ref().unwrap().times[0];
    assert!(fumbled > clean, "{fumbled} > {clean}");
}

#[test]
fn every_action_key_works_the_straps() {
    for key in 0..5 {
        let mut run = at_the_door();
        run.step(&BTreeMap::new());
        for _ in 0..200 {
            if lands(&run, 0.03) {
                break;
            }
            run.step(&input(Input::default()));
        }
        let mut press = Input::default();
        match key {
            0 => press.fire = true,
            1 => press.bomb = true,
            2 => press.dash = true,
            3 => press.swing = true,
            _ => press.interact = true,
        }
        run.step(&input(press));
        assert_eq!(
            run.players[&1].pit.stop.as_ref().unwrap().straps,
            1,
            "key {key}"
        );
    }
}

#[test]
fn a_pee_is_quicker_than_a_poop_and_pays_less() {
    let go = |side: i8| {
        let mut run = at_the_door();
        run.step(&BTreeMap::new());
        strap_cleanly(&mut run);
        wait_for(&mut run, Stage::Choose);
        run.step(&input(Input::default()));
        run.step(&input(Input {
            move_x: side,
            ..Default::default()
        }));
        wait_for(&mut run, Stage::Dress);
        strap_cleanly(&mut run);
        let hero = run.players[&1].clone();
        (run.pits.entries[0].best, hero)
    };
    let (pee, pee_hero) = go(-1);
    let (poop, poop_hero) = go(1);
    assert!(pee < poop);
    assert!(poop - pee >= POOP_TICKS - PEE_TICKS - 2);
    assert!(poop_hero.hp > pee_hero.hp, "poop pays more");
    assert_eq!(pee_hero.pit.speed, 0);
    assert!(poop_hero.pit.speed > 0);
    assert!(pee_hero.pit.damage > 0, "a quick pee is a quick stop");
}

#[test]
fn an_unanswered_choice_is_a_pee() {
    let mut run = at_the_door();
    run.step(&BTreeMap::new());
    strap_cleanly(&mut run);
    wait_for(&mut run, Stage::Choose);
    wait_for(&mut run, Stage::Inside);
    assert!(!run.players[&1].pit.stop.as_ref().unwrap().poop);
}

#[test]
fn the_board_keeps_each_knights_best_and_calls_a_record() {
    let mut board = Board::default();
    assert_eq!(
        board.set(1, "Ada", 400),
        Record::Party,
        "the first is the best"
    );
    assert_eq!(board.set(2, "Bors", 500), Record::Personal);
    assert_eq!(
        board.set(2, "Bors", 520),
        Record::None,
        "slower than their own"
    );
    assert_eq!(
        board.set(2, "Bors", 450),
        Record::Personal,
        "their best, not the party's"
    );
    assert_eq!(board.set(2, "Bors", 380), Record::Party);
    assert_eq!(board.set(1, "Ada", 390), Record::Personal);
    let ranked = board.ranked();
    assert_eq!(ranked[0].name, "Bors");
    assert_eq!((ranked[0].best, ranked[0].stops), (380, 4));
    assert_eq!(ranked[1].best, 390);
    assert_eq!(seconds(380), "12.66");
}

#[test]
fn a_record_stop_says_so_in_the_herald_line() {
    let mut run = at_the_door();
    run.pits.set(1, "You", 305);
    run.pits.set(2, "Sir Bors", 230);
    run.step(&BTreeMap::new());
    strap_cleanly(&mut run);
    wait_for(&mut run, Stage::Choose);
    run.step(&input(Input::default()));
    run.step(&input(Input {
        move_x: -1,
        ..Default::default()
    }));
    wait_for(&mut run, Stage::Dress);
    strap_cleanly(&mut run);
    let result = run.players[&1].pit.result.as_ref().unwrap();
    assert!(result.total < 230, "beat Sir Bors's 7.66");
    assert!(result.party_record);
    let ranked = run.pits.ranked();
    assert_eq!(
        (ranked[0].seat, ranked[0].best, ranked[0].last),
        (1, result.total, result.total)
    );
    assert_eq!(ranked[0].stops, 2);
    assert_eq!((ranked[1].seat, ranked[1].best), (2, 230));
    assert!(run.found.as_ref().unwrap().2.contains("party record"));
    assert!(run.cues.iter().any(|c| c == "privy_record"));
    let total = result.total;
    for _ in 0..RESULT_TICKS {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.pits.ranked()[0].best, total);
    assert_eq!(
        run.pits.ranked()[0].stops,
        2,
        "the grade never records the stop again"
    );
}

#[test]
fn the_grade_keeps_personal_bests_without_celebrating_a_party_record() {
    let mut run = at_the_door();
    run.pits.set(1, "You", 305);
    run.pits.set(2, "Sir Bors", 200);
    for total in [212, 260] {
        run.pit_events(vec![(
            1,
            Event::Done {
                total,
                poop: false,
                quality: Quality::Royal,
            },
        )]);
        let result = run.players[&1].pit.result.as_ref().unwrap();
        assert_eq!(result.total, total);
        assert!(!result.party_record);
        let ranked = run.pits.ranked();
        assert_eq!((ranked[0].seat, ranked[0].best), (2, 200));
        assert_eq!(
            (ranked[1].seat, ranked[1].best, ranked[1].last),
            (1, 212, total)
        );
    }
    assert_eq!(run.pits.ranked()[1].stops, 3);
}

#[test]
fn leaving_a_floor_unrelieved_costs_a_quarter_but_never_the_last_point() {
    let mut run = Run::new(0, 1, Some("Matt"));
    run.players.get_mut(&1).unwrap().hp = 100;
    run.players.get_mut(&2).unwrap().hp = 20;
    run.unwashed_toll();
    assert_eq!(run.players[&1].hp, 75, "a quarter of full health");
    assert_eq!(run.players[&2].hp, 1, "never the last point");
    assert!(run.cues.iter().any(|c| c == "unwashed_toll"));
    // Fallen knights are not docked, and a relieved one is not either.
    run.players.get_mut(&1).unwrap().hp = 0;
    run.players.get_mut(&2).unwrap().hp = 60;
    run.players.get_mut(&2).unwrap().relieved = Some((run.dungeon.depth, 3));
    run.unwashed_toll();
    assert_eq!(run.players[&1].hp, 0);
    assert_eq!(run.players[&2].hp, 60);
    // A visit on another floor does not count for this one.
    run.players.get_mut(&2).unwrap().relieved = Some((run.dungeon.depth + 1, 3));
    run.unwashed_toll();
    assert_eq!(run.players[&2].hp, 35);
}

#[test]
fn the_stairs_take_the_toll_and_one_knights_stop_spares_the_party() {
    // Nobody went: the stairs mend ten after the toll, and the Sanctuary
    // two tenths: 60 - 25 + 10 + 20 for each.
    let mut run = Run::new(0, 1, Some("Matt"));
    for hero in run.players.values_mut() {
        hero.hp = 60;
    }
    run.descend_for_test();
    assert_eq!((run.players[&1].hp, run.players[&2].hp), (65, 65));
    // One knight went: it counts for the party, so nobody is docked:
    // 60 + 10 + 20 for both.
    let mut run = Run::new(0, 1, Some("Matt"));
    let depth = run.dungeon.depth;
    run.players.get_mut(&2).unwrap().relieved = Some((depth, 0));
    for hero in run.players.values_mut() {
        hero.hp = 60;
    }
    run.descend_for_test();
    assert_eq!((run.players[&1].hp, run.players[&2].hp), (90, 90));
}

#[test]
fn beating_the_dragon_without_a_visit_is_the_great_unwashed() {
    let mut run = Run::new(2, 1, None);
    run.lair_for_test();
    run.calm_for_test();
    run.clear_for_test();
    assert_eq!(run.triumph, Some(Triumph::Dragon));
    assert!(run.feats.contains(&"unwashed"), "{:?}", run.feats);
    let feat = feats::feat("unwashed").unwrap();
    assert_eq!(feat.name, "The Great Unwashed");
}

#[test]
fn a_knight_who_visited_the_privy_is_not_unwashed() {
    let mut run = Run::new(2, 1, None);
    run.players.get_mut(&1).unwrap().pit.visits = 1;
    run.unwashed_check();
    assert!(!run.feats.contains(&"unwashed"));
    run.players.get_mut(&1).unwrap().pit.visits = 0;
    run.unwashed_check();
    assert!(run.feats.contains(&"unwashed"));
}

#[test]
fn a_quick_stop_sharpens_damage_for_a_while_and_then_stops() {
    let mut hero = Hero::new("Ada", 1.0, 1.0);
    let plain = hero.scaled(100);
    hero.pit.damage = DAMAGE_TICKS;
    assert_eq!(hero.scaled(100), plain + plain * BUFF / 100);
    for _ in 0..DAMAGE_TICKS {
        hero.dress();
    }
    assert_eq!(hero.scaled(100), plain);
    hero.pit.speed = 5;
    assert!(hero.pit.pace() > 1.1);
}

#[test]
fn the_room_changing_ends_a_stop_with_the_armour_back_on() {
    let mut run = at_the_door();
    run.step(&BTreeMap::new());
    assert!(run.players[&1].pit.stop.is_some());
    let at = run.at;
    run.enter_for_test(at);
    assert!(run.players[&1].pit.stop.is_none());
    assert_eq!(run.players[&1].privy, 0);
}

#[test]
fn the_panel_shows_for_a_browser_guests_frame_and_the_hud_reads_it() {
    let mut run = at_the_door();
    run.step(&BTreeMap::new());
    let plain = {
        let mut calm = at_the_door();
        calm.step(&BTreeMap::new());
        calm.players.get_mut(&1).unwrap().pit.stop = None;
        crate::render::overworld::arena::frame_for(&calm, 384, 224, Some(1))
    };
    let with = crate::render::overworld::arena::frame_for(&run, 384, 224, Some(1));
    assert_ne!(plain.rgb_bytes(), with.rgb_bytes(), "the bar is painted in");
    let hud = run.hud(1, &hud::Keys::FIRST).unwrap();
    let pit = hud.pit.expect("the HUD reads the stop");
    assert_eq!(pit.stage, "ARMOUR OFF");
    assert_eq!(pit.straps, (0, STRAPS));
}

#[test]
fn job_quality_uses_fixed_time_and_accuracy_boundaries() {
    for poop in [false, true] {
        let quick = if poop { QUICK_POOP } else { QUICK_PEE };
        assert_eq!(quality(quick, poop, 1, 0), Quality::Royal);
        assert_eq!(quality(quick + 1, poop, 1, 0), Quality::Fresh);
        assert_eq!(quality(quick, poop, 2, 0), Quality::Fresh);
        assert_eq!(quality(quick, poop, 0, 1), Quality::Fresh);
        assert_eq!(quality(quick + 3 * HZ, poop, 4, 2), Quality::Fresh);
        assert_eq!(quality(quick + 3 * HZ + 1, poop, 4, 2), Quality::Whiffy);
        assert_eq!(quality(quick, poop, 5, 0), Quality::Whiffy);
        assert_eq!(quality(quick, poop, 0, 3), Quality::Whiffy);
        assert_eq!(quality(quick + 8 * HZ, poop, 0, 5), Quality::Whiffy);
        assert_eq!(quality(quick + 8 * HZ + 1, poop, 0, 5), Quality::Disaster);
        assert_eq!(quality(quick, poop, 0, 6), Quality::Disaster);
    }
}

#[test]
fn the_finish_split_fires_once_at_the_stripe_before_dressing() {
    let mut run = at_the_door();
    run.step(&BTreeMap::new());
    wait_for(&mut run, Stage::RunOut);
    let before = (RUN_TICKS as f32 * FINISH_PROGRESS).ceil() as u32 - 1;
    for _ in 0..before {
        run.step(&BTreeMap::new());
    }
    let stop = run.players[&1].pit.stop.as_ref().unwrap();
    assert_eq!(stop.finish, None);
    let point = stop.finish_point();
    run.step(&BTreeMap::new());
    let stop = run.players[&1].pit.stop.as_ref().unwrap();
    let split = stop.finish.unwrap();
    assert_eq!(split, stop.total);
    assert!((run.players[&1].x - point.0).hypot(run.players[&1].y - point.1) < 0.4);
    assert_eq!(run.cues.iter().filter(|c| *c == "privy_finish").count(), 1);
    for _ in 0..3 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(
        run.players[&1].pit.stop.as_ref().unwrap().finish,
        Some(split)
    );
    assert_eq!(run.cues.iter().filter(|c| *c == "privy_finish").count(), 1);
    wait_for(&mut run, Stage::Dress);
    strap_cleanly(&mut run);
    assert!(run.players[&1].pit.result.as_ref().unwrap().total > split);
}

#[test]
fn two_stalls_race_while_the_next_pair_strip_and_queue() {
    let mut run = at_the_door();
    let door = run.room().privy().unwrap();
    assert_eq!(run.room().privy_stalls().len(), 2);
    for id in 2..=4 {
        let mut hero = run.players[&1].clone();
        hero.name = format!("Knight {id}");
        (hero.x, hero.y) = door;
        run.players.insert(id, hero);
    }
    run.step(&BTreeMap::new());
    assert_eq!(
        run.players
            .values()
            .filter(|h| h.pit.stop.is_some())
            .count(),
        4
    );
    let spots: Vec<_> = run
        .players
        .values()
        .map(|h| h.pit.stop.as_ref().unwrap().from)
        .collect();
    for (i, p) in spots.iter().enumerate() {
        assert!(!spots[..i].contains(p));
    }
    let mut inputs = BTreeMap::new();
    for _ in 0..170 {
        inputs.clear();
        for (&id, h) in &run.players {
            let Some(s) = &h.pit.stop else {
                continue;
            };
            let phase = (s.phase + 2.0 / Stop::period(s.straps)).rem_euclid(2.0);
            let marker = if phase < 1.0 { phase } else { 2.0 - phase };
            if s.stage == Stage::Strip
                && s.straps < STRAPS
                && !s.pressed
                && (marker - 0.5).abs() < PERFECT_BAND
            {
                inputs.insert(id, fire());
            }
        }
        run.step(&inputs);
        if run
            .players
            .values()
            .filter(|h| {
                h.pit
                    .stop
                    .as_ref()
                    .is_some_and(|s| s.stage == Stage::Choose)
            })
            .count()
            == 2
        {
            break;
        }
    }
    let stages: Vec<_> = run
        .players
        .values()
        .map(|h| h.pit.stop.as_ref().unwrap())
        .collect();
    assert_eq!(
        stages.iter().filter(|s| s.stage == Stage::Choose).count(),
        2
    );
    assert_eq!(
        stages
            .iter()
            .filter(|s| s.waiting && s.straps == STRAPS)
            .count(),
        2
    );
    // Existing owners retain admission even if a queued stop has a larger clock.
    assert_eq!(
        run.pit_admissions(),
        std::collections::BTreeSet::from([1, 2])
    );
    for _ in 0..CHOOSE_WAIT + POOP_TICKS + RUN_TICKS + 4 {
        run.step(&BTreeMap::new());
    }
    assert!(run.players[&3].pit.stop.as_ref().unwrap().stage != Stage::Strip);
    assert!(run.players[&4].pit.stop.as_ref().unwrap().stage != Stage::Strip);
}

#[test]
fn legacy_stops_default_new_fields_and_still_complete() {
    let mut run = at_the_door();
    run.step(&BTreeMap::new());
    let mut value = serde_json::to_value(&run.players[&1].pit).unwrap();
    value.as_object_mut().unwrap().remove("result");
    let stop = value["stop"].as_object_mut().unwrap();
    for key in [
        "lane", "waiting", "tossed", "fitted", "loose", "misses", "finish",
    ] {
        stop.remove(key);
    }
    run.players.get_mut(&1).unwrap().pit = serde_json::from_value(value).unwrap();
    wait_for(&mut run, Stage::Dress);
    strap_cleanly(&mut run);
    assert!(run.players[&1].pit.stop.is_none());
    assert_eq!(run.players[&1].pit.visits, 1);
}

#[test]
fn a_seats_window_is_private_and_the_result_closes_without_moving() {
    let mut run = at_the_door();
    run.step(&BTreeMap::new());
    for _ in 0..14 {
        run.step(&BTreeMap::new());
    }
    let mut local = crate::render::overworld::ink::Img::new(384, 224);
    let mut guest = local.clone();
    // Verify the actual final frame overlay, separately from world occupancy.
    crate::render::overworld::arena::privy_panel_for_test(&mut local, &run, Some(1));
    crate::render::overworld::arena::privy_panel_for_test(&mut guest, &run, Some(2));
    assert!(local.get(16, 13).is_some());
    assert_eq!(
        guest,
        crate::render::overworld::ink::Img::new(384, 224)
    );
    wait_for(&mut run, Stage::Dress);
    strap_cleanly(&mut run);
    let hero = &run.players[&1];
    let position = (hero.x, hero.y);
    let left = hero.pit.result.as_ref().unwrap().left;
    for _ in 0..left - 1 {
        run.step(&input(Input {
            move_x: 1,
            fire: true,
            ..Default::default()
        }));
        assert_eq!((run.players[&1].x, run.players[&1].y), position);
    }
    assert!(run.players[&1].pit.result.is_some());
    run.step(&BTreeMap::new());
    assert!(run.players[&1].pit.result.is_none());
}

#[test]
fn the_result_animation_preserves_the_full_reward_duration() {
    let mut pit = Pit {
        speed: SPEED_TICKS,
        damage: DAMAGE_TICKS,
        result: Some(Result {
            quality: Quality::Royal,
            total: 180,
            left: RESULT_TICKS,
            party_record: true,
        }),
        ..Default::default()
    };
    for _ in 0..RESULT_TICKS {
        pit.tick();
    }
    assert_eq!(pit.result, None);
    assert_eq!(pit.speed, SPEED_TICKS);
    assert_eq!(pit.damage, DAMAGE_TICKS);
    pit.tick();
    assert_eq!(pit.speed, SPEED_TICKS - 1);
    assert_eq!(pit.damage, DAMAGE_TICKS - 1);
}

#[test]
fn animation_state_survives_guest_snapshots_and_replays_identically() {
    let mut host = at_the_door();
    host.step(&BTreeMap::new());
    for _ in 0..12 {
        host.step(&BTreeMap::new());
    }
    while !lands(&host, 0.03) {
        host.step(&BTreeMap::new());
    }
    host.step(&input(fire()));
    let mut guest = host.wire_copy();
    assert_eq!(host.players[&1].pit, guest.players[&1].pit);
    let frame =
        |r: &Run| crate::render::overworld::arena::frame_for(r, 384, 224, Some(1));
    assert_eq!(frame(&host), frame(&guest));
    assert_eq!(frame(&host), frame(&host));
    assert!(
        frame(&host).pixels().all(|c| c == [0, 0, 0]
            || crate::render::overworld::ink::palette_index(c).is_some())
    );
    wait_for(&mut host, Stage::Dress);
    strap_cleanly(&mut host);
    guest = host.wire_copy();
    assert_eq!(host.players[&1].pit.result, guest.players[&1].pit.result);
    assert_eq!(frame(&host), frame(&guest));
}

#[test]
fn legacy_results_default_to_no_party_record() {
    let result: Result = serde_json::from_value(serde_json::json!({
        "quality": "royal", "total": 212, "left": RESULT_TICKS
    }))
    .unwrap();
    assert!(!result.party_record);
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test privy_popup_shots -- --ignored`
#[test]
#[ignore]
fn write_privy_popup_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let mut captures = Vec::new();
    let mut save = |run: &Run, name: &str, seat: u32| {
        let img = crate::render::overworld::arena::frame_for(run, 384, 224, Some(seat));
        let out = image::RgbaImage::from_raw(768, 448, img.rgba_scaled(2)).unwrap();
        out.save(dir.join(format!("{name}.png"))).unwrap();
        captures.push((name.to_string(), out));
    };
    let mut run = at_the_door();
    run.home.privy_wall = vec!["BORS SAT HERE AND PONDERED".into(), "HONK - GARETH".into()];
    run.pits.set(2, "Matt", 9 * HZ + 12);
    run.pits.set(3, "Sir Bors", 7 * HZ + 20);
    save(&run, "00_carved_board", 1);
    run.step(&BTreeMap::new());
    for t in 0..18 {
        if [0, 3, 7, 12].contains(&t) {
            save(&run, &format!("01_open_{t:02}"), 1);
        }
        run.step(&BTreeMap::new());
    }
    // Six distinct hop frames and the first successful toss at its apex.
    for i in 0..6 {
        save(&run, &format!("02_hop_{i}"), 1);
        for _ in 0..4 {
            run.step(&BTreeMap::new());
        }
    }
    while !lands(&run, 0.03) {
        run.step(&BTreeMap::new());
    }
    run.step(&input(fire()));
    for _ in 0..6 {
        run.step(&BTreeMap::new());
    }
    save(&run, "03_helm_flight", 1);
    run.step(&input(fire()));
    save(&run, "04_fumble", 1);
    strap_cleanly(&mut run);
    for i in 0..6 {
        save(&run, &format!("05_waddle_in_{i}"), 1);
        for _ in 0..2 {
            run.step(&BTreeMap::new());
        }
    }
    wait_for(&mut run, Stage::Choose);
    run.step(&BTreeMap::new());
    save(&run, "06_choose", 1);
    run.step(&input(Input {
        move_x: 1,
        ..Default::default()
    }));
    save(&run, "07_door_slam", 1);
    for i in 0..3 {
        for _ in 0..13 {
            run.step(&BTreeMap::new());
        }
        save(&run, &format!("08_inside_{i}"), 1);
    }
    wait_for(&mut run, Stage::RunOut);
    for t in 0..RUN_TICKS {
        if [6, 10, 11, 14, 20].contains(&t) {
            save(&run, &format!("09_finish_{t:02}"), 1);
        }
        run.step(&BTreeMap::new());
    }
    save(&run, "10_dress_floor", 1);
    while !lands(&run, 0.03) {
        run.step(&BTreeMap::new());
    }
    run.step(&input(fire()));
    for _ in 0..7 {
        run.step(&BTreeMap::new());
    }
    save(&run, "11_chest_return", 1);
    strap_cleanly(&mut run);
    let before_grades = run.clone();
    for q in [
        Quality::Royal,
        Quality::Fresh,
        Quality::Whiffy,
        Quality::Disaster,
    ] {
        run = before_grades.clone();
        // Exercise the same board/result transaction as a real completed stop.
        run.pit_events(vec![(
            1,
            Event::Done {
                quality: q,
                total: 212,
                poop: true,
            },
        )]);
        assert_eq!(run.pits.ranked()[0].seat, 1);
        assert_eq!(run.pits.ranked()[0].best, 212);
        for age in [12, 24, 40] {
            run.players
                .get_mut(&1)
                .unwrap()
                .pit
                .result
                .as_mut()
                .unwrap()
                .left = RESULT_TICKS - age;
            run.tick += 4;
            save(&run, &format!("12_{q:?}_{age:02}"), 1);
        }
    }
    for left in [10, 6, 2] {
        run.players
            .get_mut(&1)
            .unwrap()
            .pit
            .result
            .as_mut()
            .unwrap()
            .left = left;
        save(&run, &format!("13_close_{left:02}"), 1);
    }
    run.players.get_mut(&1).unwrap().pit.result = None;
    save(&run, "14_board", 1);
    let mut race = at_the_door();
    let door = race.room().privy().unwrap();
    for id in 2..=4 {
        let mut hero = race.players[&1].clone();
        hero.name = format!("Knight {id}");
        (hero.x, hero.y) = door;
        race.players.insert(id, hero);
    }
    race.step(&BTreeMap::new());
    for _ in 0..14 {
        race.step(&BTreeMap::new());
    }
    save(&race, "15_race_seat_1", 1);
    save(&race, "15_race_seat_2", 2);
    for hero in race.players.values_mut() {
        let s = hero.pit.stop.as_mut().unwrap();
        s.straps = STRAPS;
        s.tossed = [Some(0); 3];
        s.total = 30;
    }
    race.step(&BTreeMap::new());
    for _ in 0..6 {
        race.step(&BTreeMap::new());
    }
    save(&race, "15_queue_seat_3", 3);
    race.players.get_mut(&1).unwrap().pit.stop = None;
    save(&race, "16_outside_seat", 1);
    // A calm arena view checks both narrow stall signs without the pop-up.
    race.tick += 8 * HZ as u64;
    race.hallowed = None;
    race.found = None;
    for id in [2, 3] {
        let hero = race.players.get_mut(&id).unwrap();
        hero.pit.stop.as_mut().unwrap().stage = Stage::Inside;
        hero.privy = 24;
    }
    race.players.get_mut(&4).unwrap().pit.stop = None;
    save(&race, "17_two_occupied", 1);
    // An offline contact sheet of every exported frame, with filename captions.
    let cols = 4u32;
    let cell_w = 768u32;
    let cell_h = 470u32;
    let mut sheet = image::RgbaImage::from_pixel(
        cols * cell_w,
        captures.len().div_ceil(cols as usize) as u32 * cell_h,
        image::Rgba([11, 10, 8, 255]),
    );
    for (i, (name, img)) in captures.iter().enumerate() {
        let x = i as u32 % cols * cell_w;
        let y = i as u32 / cols * cell_h;
        image::imageops::overlay(&mut sheet, img, i64::from(x), i64::from(y));
        let mut caption = crate::render::overworld::ink::Img::new(384, 10);
        caption.text(3, 1, &name.to_uppercase(), 'h');
        let caption = image::RgbaImage::from_raw(768, 20, caption.rgba_scaled(2)).unwrap();
        image::imageops::overlay(&mut sheet, &caption, i64::from(x), i64::from(y + 448));
    }
    sheet.save(dir.join("contact_sheet.png")).unwrap();
}

#[test]
fn a_finished_stop_scratches_a_line_on_the_wall_that_stays() {
    let mut run = at_the_door();
    run.step(&BTreeMap::new());
    strap_cleanly(&mut run);
    wait_for(&mut run, Stage::Choose);
    run.step(&input(Input::default()));
    run.step(&input(Input { move_x: -1, ..Default::default() }));
    wait_for(&mut run, Stage::Dress);
    strap_cleanly(&mut run);
    assert_eq!(run.home.privy_wall.len(), 1, "{:?}", run.home.privy_wall);
    assert!(run.valid_snapshot());
    // The wall keeps its last eight lines.
    for i in 0..12 {
        run.home.privy_wall.push(format!("LINE {i}"));
        if run.home.privy_wall.len() > WALL {
            run.home.privy_wall.remove(0);
        }
    }
    assert_eq!(run.home.privy_wall.len(), WALL);
    // It travels with the home record into the next delve.
    let next = Run::at_home(5, 1, None, run.home.clone(), Default::default());
    assert_eq!(next.home.privy_wall, run.home.privy_wall);
}
