use super::*;

fn stairs(depth: u32, seats: u32) -> Run {
    let mut run = Run::new(47, 9, None);
    for id in 2..=seats { run.join_seat(id, &format!("Knight {id}")); }
    run.dungeon = crawl_floor(depth, Pack::Crypt, 47);
    run.at = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).unwrap_or(0);
    run.boss_gates = None;
    run.enemies.clear();
    run.phase = Phase::Exploring;
    run.floor_plates.clear();
    for room in &mut run.dungeon.rooms { room.cleared = true; }
    run
}

fn vote(run: &mut Run, id: u32, choice: u8) {
    let serial = run.playtest.seats.get(&id).map_or(1, |s| s.serial + 1);
    run.step(&BTreeMap::from([(id, Input { vote: Some(Command { serial, action: Action::Rate(choice) }), ..Default::default() })]));
}

#[test]
fn only_every_third_cleared_floor_stops_before_the_next_floor() {
    for depth in 1..=3 {
        let mut run = stairs(depth, 2);
        run.descend();
        if depth == 3 {
            assert_eq!(run.dungeon.depth, 3);
            assert!(run.crossroads_voting());
            assert_eq!(run.crossroads.as_ref().unwrap().depth, 4);
        } else {
            assert!(run.crossroads.is_none());
            assert_eq!(run.dungeon.depth, depth + 1);
        }
    }
}

#[test]
fn ballot_counts_fallen_knights_retries_and_a_majority_launches_the_card() {
    let mut run = stairs(3, 3);
    run.players.get_mut(&2).unwrap().hp = 0;
    run.descend();
    let choice = run.crossroads.as_ref().unwrap().choices[1];
    vote(&mut run, 1, 2);
    vote(&mut run, 2, 2);
    assert_eq!(run.crossroads.as_ref().unwrap().counts(), [0, 2, 0]);
    let serial = run.playtest.seats[&2].serial;
    run.vote_command(2, Command { serial, action: Action::Rate(1) });
    assert_eq!(run.crossroads.as_ref().unwrap().counts(), [0, 2, 0]);
    vote(&mut run, 3, 1);
    let c = run.crossroads.as_ref().unwrap();
    assert_eq!(c.stage, Stage::Playing);
    assert_eq!(c.chosen, Some(1));
    assert_eq!(run.players[&2].hp, 0, "games don't spend or revive the delve's bodies");
    if let Some(course) = choice.course() {
        assert_eq!(run.race3d.as_ref().unwrap().course, course);
        assert_eq!(run.race3d.as_ref().unwrap().riders.len(), 3);
    } else {
        assert_eq!(run.pastimes.heat.as_ref().unwrap().seats.len(), 3);
    }
}

#[test]
fn equal_votes_and_no_votes_keep_the_first_dealt_card() {
    let mut run = stairs(3, 3);
    run.descend();
    vote(&mut run, 1, 3);
    vote(&mut run, 2, 2);
    vote(&mut run, 3, 1);
    assert_eq!(run.crossroads.as_ref().unwrap().chosen, Some(0));
    let mut run = stairs(3, 1);
    run.descend();
    for _ in 0..VOTE_TIME { run.step(&BTreeMap::new()); }
    assert_eq!(run.crossroads.as_ref().unwrap().chosen, Some(0));
}

#[test]
fn away_absent_and_departed_seats_do_not_hold_the_ballot_open() {
    let mut run = stairs(3, 4);
    run.players.get_mut(&2).unwrap().away = true;
    run.players.get_mut(&3).unwrap().absent = true;
    run.descend();
    run.players.remove(&4);
    vote(&mut run, 1, 3);
    assert_eq!(run.crossroads.as_ref().unwrap().chosen, Some(2));
    assert!(!run.crossroads.as_ref().unwrap().seats.contains(&3));
}

#[test]
fn an_afk_seat_cannot_stall_the_vote() {
    let mut run = stairs(3, 2);
    run.descend();
    vote(&mut run, 1, 2);
    for _ in 1..VOTE_TIME - 1 { run.step(&BTreeMap::new()); }
    assert!(run.crossroads_voting());
    run.step(&BTreeMap::new());
    assert_eq!(run.crossroads.as_ref().unwrap().chosen, Some(1));
}

#[test]
fn all_pool_games_seat_six_including_the_fallen_and_end_on_their_clock() {
    // Knights Wish ticks through the Bend leg, so its seating test runs in the cockpit
    // (`deathmatch_seats_six_including_the_fallen_and_ends_on_the_crossroads_clock`).
    for game in Game::ALL.into_iter().filter(|g| *g != Game::Deathmatch) {
        let mut run = stairs(3, 6);
        run.players.get_mut(&2).unwrap().hp = 0;
        run.descend();
        let c = run.crossroads.as_mut().unwrap();
        c.choices = [game, Game::ALL.into_iter().find(|g| *g != game).unwrap(), Game::ALL.into_iter().rev().find(|g| *g != game).unwrap()];
        for id in 1..=6 { vote(&mut run, id, 1); }
        assert_eq!(run.crossroads.as_ref().unwrap().stage, Stage::Playing);
        assert!(run.valid_snapshot(), "{game:?} launch");
        assert!(!run.predictable(1));
        if game.course().is_some() {
            let race = run.race3d.as_mut().unwrap();
            assert_eq!(race.riders.len(), 6);
            assert_eq!(race.time_limit(), race3d::RACE_TIME);
            if game == Game::Touge {
                assert!(race.riders.values().all(|r| r.x.abs() < race3d::DRIFT_WIDTH / 2.0));
            }
            race.t = race3d::TITLE + race.time_limit() - 1;
            assert!(race.ante.is_empty());
        } else {
            assert_eq!(run.pastimes.heat.as_ref().unwrap().seats.len(), 6);
            run.pastimes.round.as_mut().unwrap().left = 1;
            for r in run.pastimes.heats.values_mut() { r.left = 1; }
        }
        run.step(&BTreeMap::new());
        assert_eq!(run.crossroads.as_ref().unwrap().stage, Stage::Results, "{game:?}");
        assert_eq!(run.crossroads.as_ref().unwrap().order.len(), 6);
        assert!(run.valid_snapshot(), "{game:?} results");
        for _ in 0..RESULTS_TIME { run.step(&BTreeMap::new()); }
        assert!(run.crossroads.is_none());
        assert_eq!(run.dungeon.depth, 4);
        assert!(run.players[&2].hp > 0, "the usual descent revives the fallen");
        assert_eq!(run.bank.iter().filter(|h| h.why == "crossroads prize").count(), 6);
    }
}

#[test]
fn prizes_pay_once_and_the_delve_resumes_on_the_pending_floor() {
    let mut run = stairs(3, 3);
    run.descend();
    for id in 1..=3 { vote(&mut run, id, 1); }
    let wishes = run.players[&2].wishes;
    run.finish_crossroads(vec![2, 1, 3]);
    let prizes: Vec<_> = run.bank.iter().filter(|h| h.why == "crossroads prize").map(|h| (h.hero, h.spoils.get(Spoil::Gold))).collect();
    assert_eq!(prizes, vec![(2, 100), (1, 60), (3, 30)]);
    assert_eq!(run.players[&2].wishes, wishes + 1);
    run.finish_crossroads(vec![1, 2, 3]);
    assert_eq!(run.bank.iter().filter(|h| h.why == "crossroads prize").count(), 3);
    for _ in 0..RESULTS_TIME { run.step(&BTreeMap::new()); }
    assert!(run.crossroads.is_none());
    assert_eq!(run.dungeon.depth, 4);
    assert_eq!(run.players[&2].wishes, wishes + 1 + run.floor_wishes());
    assert!(run.valid_snapshot());
}

#[test]
fn the_fallen_sapper_drops_a_keg_in_the_same_yard_each_seat_sees() {
    let mut run = stairs(3, 6);
    run.players.get_mut(&2).unwrap().hp = 0;
    run.descend();
    run.crossroads.as_mut().unwrap().choices = [Game::Powder, Game::Gander, Game::Shaft];
    for id in 1..=6 { vote(&mut run, id, 1); }
    let yard = run.pastimes.round.as_ref().unwrap().powder.as_ref().unwrap();
    let starts: std::collections::BTreeSet<_> = yard.knights.iter().map(|k| k.tile()).collect();
    assert_eq!(starts.len(), 6, "six distinct starts");
    for &(x, y) in &starts {
        let exits = [(-1, 0), (1, 0), (0, -1), (0, 1)].into_iter().filter(|&(dx, dy)| {
            let (x, y) = (i16::from(x) + dx, i16::from(y) + dy);
            x >= 0 && y >= 0 && x < i16::from(pastimes::POWDER_W) && y < i16::from(pastimes::POWDER_H)
                && !yard.solid(x as u8, y as u8)
        }).count();
        assert!(exits >= 2, "a sapper must be able to dodge from {x},{y}");
    }
    run.step(&BTreeMap::from([(2, Input { fire: true, ..Default::default() })]));
    let shared = run.pastimes.round.as_ref().unwrap().powder.as_ref().unwrap();
    assert_eq!(shared.kegs.len(), 1);
    assert_eq!(shared.kegs[0].owner, 2);
    for id in 2..=6 {
        let view = run.view_for_seat(id).unwrap();
        assert_eq!(view.pastimes.round.as_ref().unwrap().powder.as_ref().unwrap(), shared);
    }
    for _ in 0..u32::from(pastimes::FUSE) + 2 { run.step(&BTreeMap::new()); }
    assert!(run.pastimes.heat.as_ref().unwrap().done.contains_key(&2));
    assert!(run.valid_snapshot());
    // A survivor always outranks the sapper already out of the yard.
    run.pastimes.round.as_mut().unwrap().left = 1;
    run.step(&BTreeMap::new());
    assert_eq!(run.crossroads.as_ref().unwrap().order.last(), Some(&2));
}

#[test]
fn a_friend_who_sits_down_during_the_vote_gets_a_card_and_a_place() {
    let mut run = stairs(3, 2);
    run.descend();
    vote(&mut run, 1, 1);
    run.join_seat(3, "Late friend");
    vote(&mut run, 2, 2);
    assert!(run.crossroads_voting());
    vote(&mut run, 3, 2);
    let c = run.crossroads.as_ref().unwrap();
    assert_eq!(c.seats, vec![1, 2, 3]);
    assert_eq!(c.chosen, Some(1));
}

#[test]
fn old_saves_and_each_crossroads_stage_restore_and_replay() {
    let run = stairs(3, 2);
    let mut old = serde_json::to_value(&run).unwrap();
    old.as_object_mut().unwrap().remove("crossroads");
    let mut run: Run = serde_json::from_value(old).unwrap();
    assert!(run.crossroads.is_none());
    run.descend();
    let mut same = stairs(3, 2);
    same.descend();
    assert_eq!(run.crossroads, same.crossroads);
    vote(&mut run, 1, 2);
    let mut restored: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    vote(&mut run, 2, 2);
    vote(&mut restored, 2, 2);
    assert_eq!(run.crossroads, restored.crossroads);
    let mut restored: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    for _ in 0..30 {
        run.step(&BTreeMap::new());
        restored.step(&BTreeMap::new());
    }
    assert_eq!(serde_json::to_value(&run).unwrap(), serde_json::to_value(&restored).unwrap());
    run.finish_crossroads(vec![2, 1]);
    let restored: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    assert_eq!(run.crossroads, restored.crossroads);
}

#[test]
#[ignore = "writes the crossroads card review frames"]
fn crossroads_shots() {
    let dir = std::path::PathBuf::from(std::env::var("DELVE_CROSSROADS_SHOTS").expect("set DELVE_CROSSROADS_SHOTS to the output directory"));
    std::fs::create_dir_all(&dir).unwrap();
    let mut run = stairs(3, 6);
    run.players.get_mut(&2).unwrap().hp = 0;
    run.descend();
    let save = |run: &Run, name: &str, seat: u32| {
        let im = crate::render::overworld::arena::seat_scene(run, 768, 592, Some(seat)).compose();
        image::RgbaImage::from_raw(im.w as u32, im.h as u32, im.rgba_bytes()).unwrap().save(dir.join(format!("{name}.png"))).unwrap();
    };
    save(&run, "01-dealt", 1);
    for (id, choice) in [(1, 1), (2, 2), (3, 2), (4, 3), (5, 1)] { vote(&mut run, id, choice); }
    save(&run, "02-votes-fallen-seat", 2);
    run.crossroads.as_mut().unwrap().left = HZ;
    save(&run, "03-timeout", 1);
    vote(&mut run, 6, 2);
    save(&run, "04-playing", 2);
    run.finish_crossroads(vec![2, 1, 3, 4, 5, 6]);
    save(&run, "05-prizes", 2);
}

fn deathmatch_card() -> Game {
    serde_json::from_str("\"deathmatch\"").expect("the pool must deserialize the deathmatch card")
}

fn deathmatch_round(seats: u32) -> Run {
    let mut run = stairs(3, seats);
    run.players.get_mut(&2).unwrap().hp = 0;
    for h in run.players.values_mut() { h.carried.add(Spoil::Gold, 200); }
    run.descend();
    run.crossroads.as_mut().unwrap().choices = [deathmatch_card(), Game::Powder, Game::Gander];
    for id in 1..=seats { vote(&mut run, id, 1); }
    run
}

#[test]
fn deathmatch_is_the_sixth_seeded_card_but_never_dealt_to_one_present_seat() {
    assert_eq!(Game::ALL.len(), 6);
    let dm = deathmatch_card();
    assert_eq!(dm.name(), "KNIGHTS WISH");
    assert_eq!(dm.words(), "TEN KILLS WINS");
    let mut seen = false;
    for seed in 1..=32 {
        let mut run = stairs(3, 2);
        run.seed = seed;
        let mut same = run.clone();
        let rng = run.rng.clone().next();
        run.descend();
        same.descend();
        assert_eq!(run.crossroads, same.crossroads);
        assert_eq!(run.rng.clone().next(), rng, "dealing cannot consume floor RNG");
        seen |= run.crossroads.as_ref().unwrap().choices.contains(&dm);
        for absent in [false, true] {
            let mut solo = stairs(3, if absent { 2 } else { 1 });
            solo.seed = seed;
            if absent { solo.players.get_mut(&2).unwrap().absent = true; }
            solo.descend();
            assert!(!solo.crossroads.as_ref().unwrap().choices.contains(&dm));
        }
    }
    assert!(seen, "the card must actually be dealt");
}

#[test]
fn deathmatch_selected_after_a_departure_falls_back_to_a_playable_card() {
    let mut run = stairs(3, 2);
    run.descend();
    run.crossroads.as_mut().unwrap().choices = [deathmatch_card(), Game::Powder, Game::Gander];
    run.players.get_mut(&2).unwrap().absent = true;
    vote(&mut run, 1, 1);
    assert!(!run.crossroads_voting());
    assert!(run.deathmatch.is_none());
    assert!(run.pastimes.heat.is_some() || run.race3d.is_some()
        || run.crossroads.as_ref().unwrap().stage == Stage::Results);
    assert!(run.valid_snapshot());
}


#[test]
fn deathmatch_first_person_dispatch_matches_town_for_every_crossroads_seat() {
    let mut run = deathmatch_round(6);
    run.deathmatch.as_mut().unwrap().ticks = deathmatch::TITLE + 1;
    let mut town_dispatch = run.clone();
    town_dispatch.crossroads = None;
    let mut hashes = std::collections::BTreeSet::new();
    for id in 1..=6 {
        let frame = crate::render::overworld::arena::frame_for(&run, 384, 224, Some(id));
        let town_frame = crate::render::overworld::arena::frame_for(&town_dispatch, 384, 224, Some(id));
        assert_eq!(frame.content_hash(), town_frame.content_hash(), "crossroads seat {id} must use the town first-person renderer");
        hashes.insert(frame.content_hash());
        let layers = crate::render::overworld::arena::terminal_layers(&run, Some(id)).unwrap();
        assert_eq!(layers.0, frame);
        let stage = crate::render::overworld::arena::scene_for(&run, 384, 224, Some(id));
        assert_eq!(stage.screen, frame, "browser stage uses the same first-person scene");
        assert!(stage.figures.is_empty());
    }
    assert_eq!(hashes.len(), 6, "every seat gets its own camera");
}

#[test]
#[ignore = "writes the deathmatch pool card and in-play PNGs"]
fn deathmatch_pool_shots() {
    let dir = &std::path::PathBuf::from(std::env::var_os("DELVE_DM_POOL_SHOTS").expect("set DELVE_DM_POOL_SHOTS to the output directory"));
    std::fs::create_dir_all(dir).unwrap();
    let save = |run: &Run, name: &str| {
        let im = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        image::save_buffer(dir.join(format!("{name}.png")), &im.rgba_bytes(),
            im.w as u32, im.h as u32, image::ColorType::Rgba8).unwrap();
    };
    // Find a real deterministic deal, rather than inserting the card by hand.
    let mut run = (1..=64).find_map(|seed| {
        let mut run = stairs(3, 6);
        run.seed = seed;
        run.descend();
        run.crossroads.as_ref().unwrap().choices.contains(&deathmatch_card()).then_some(run)
    }).expect("deathmatch is dealt");
    save(&run, "cards");
    let card = run.crossroads.as_ref().unwrap().choices.iter().position(|g| *g == deathmatch_card()).unwrap();
    for id in 1..=6 { vote(&mut run, id, card as u8 + 1); }
    let s = run.deathmatch.as_mut().unwrap();
    s.ticks = deathmatch::TITLE + 60;
    let f = s.fighters.get_mut(&1).unwrap();
    (f.hero.x, f.hero.y, f.yaw) = (13., 23., 0.);
    let f = s.fighters.get_mut(&2).unwrap();
    (f.hero.x, f.hero.y) = (25., 23.);
    save(&run, "playing");
}
