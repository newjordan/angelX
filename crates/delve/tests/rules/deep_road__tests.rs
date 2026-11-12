//! The Deep Road: every other floor between villages holds a meeting (a
//! lost patrol, a caravan, a hermit, captives, a shrine, an envoy); each is
//! a 1/2/3 talk whose answers cost something and do what they say, feeding
//! each people's favour, the chapter's errands, the dread at the next
//! village's stair and what that village gives as the star lands.
use super::*;
use crate::rules::home::{Home, Station};

fn deep_home() -> Home {
    let mut home = Home { deepest: DEEPEST, ..Home::default() };
    home.levels.insert(Station::Map, 4);
    home
}

fn put(run: &mut Run, id: u32, (x, y): (f32, f32)) {
    let h = run.players.get_mut(&id).unwrap();
    (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
}

fn press(run: &mut Run, id: u32) {
    run.step(&BTreeMap::from([(id, Input { interact: true, ..Default::default() })]));
    run.step(&BTreeMap::new());
}

fn key(run: &mut Run, id: u32, cast: u8) {
    run.step(&BTreeMap::from([(id, Input { cast, ..Default::default() })]));
    run.step(&BTreeMap::new());
}

fn calm(run: &mut Run, ticks: u64) {
    for _ in 0..ticks {
        run.step(&BTreeMap::new());
    }
}

/// A party of `seats` on floor `floor` of the Deep, laid fresh.
fn road_run(seats: u32, floor: u32) -> Run {
    let mut run = Run::new(41, 1, None);
    for id in 2..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.home = deep_home();
    run.deep_floor_for_test(floor);
    for hero in run.players.values_mut() {
        hero.invulnerable = 0;
    }
    run
}

fn the_meeting(run: &Run) -> &road::Meeting {
    run.deep.as_ref().unwrap().meeting.as_ref().expect("a meeting on this floor")
}

fn meeting_mut(run: &mut Run) -> &mut road::Meeting {
    run.deep.as_mut().unwrap().meeting.as_mut().unwrap()
}

/// The meeting made a `kind` of `people`'s, as the floor would lay one.
fn force(run: &mut Run, kind: road::Kind, people: Culture) {
    let m = meeting_mut(run);
    m.kind = kind;
    m.people = people;
    m.stage = if kind == road::Kind::Captives { road::Stage::Waiting } else { road::Stage::Open };
    m.choice = None;
    m.sold = 0;
    m.talks.clear();
    m.foes.clear();
}

/// Knight 1 beside the meeting, in its room, nothing else in the room.
fn go(run: &mut Run) {
    let room = the_meeting(run).room;
    if run.at != room {
        run.enter_for_test(room);
    }
    run.enemies.clear();
    run.parked.clear();
    let (x, y) = the_meeting(run).tile_point();
    put(run, 1, (x, y + 1.0));
    calm(run, 2);
}

fn beside(run: &mut Run, id: u32) {
    let (x, y) = the_meeting(run).tile_point();
    put(run, id, (x, y + 1.0));
}

/// Knight `id` speaks with the meeting and goes on to what it asks.
fn speak(run: &mut Run, id: u32) {
    beside(run, id);
    press(run, id);
    press(run, id);
}

fn gold(run: &Run, id: u32) -> u32 {
    run.players[&id].carried.get(Spoil::Gold)
}

fn gems(run: &Run, id: u32) -> u32 {
    run.players[&id].carried.get(Spoil::Gem)
}

fn favour(run: &Run, c: Culture) -> i8 {
    run.home.deep.road.of(c)
}

fn give(run: &mut Run, id: u32, spoil: Spoil, n: u32) {
    run.players.get_mut(&id).unwrap().carried.add(spoil, n);
}

/// An open errand of `errand` from the village whose chapter `floor` is in.
fn ask(run: &mut Run, floor: u32, errand: Errand, need: u16) -> usize {
    let village = chapter(floor) * CHAPTER;
    let culture = run.home.deep.culture_at(village);
    run.home.deep.missions.push(Mission { errand, village, culture, giver: 0, have: 0, need, paid: false });
    run.home.deep.missions.len() - 1
}

fn kill_foes(run: &mut Run) {
    for e in run.enemies.iter_mut() {
        e.hp = 0;
    }
    calm(run, 3);
}

#[test]
fn every_other_floor_of_a_stretch_holds_a_meeting() {
    assert_eq!(road::stretch(6), None);
    assert_eq!(road::stretch(7), Some(0));
    assert_eq!(road::stretch(9), Some(2));
    assert_eq!(road::stretch(10), None, "a village is no road");
    assert_eq!(road::stretch(11), Some(0));
    assert_eq!(road::stretch(19), Some(8));
    let meets: Vec<u32> = (7..30).filter(|&f| road::meets(f)).collect();
    assert_eq!(meets, vec![7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29]);
    assert_eq!(road::ahead(8), 10);
    assert_eq!(road::ahead(13), 20);
}

#[test]
fn a_party_that_reaches_the_deep_meets_something_on_its_first_road_floor() {
    for open in [false, true] {
        for raid in 0..16u64 {
            let mut run = Run::new(41, 1, None);
            run.raid_id = raid * 7919 + 3;
            run.home = deep_home();
            run.open_floors = open;
            run.deep_floor_for_test(ROAD);
            let m = the_meeting(&run);
            assert!(run.valid_snapshot(), "open {open} raid {raid}");
            let room = &run.dungeon.rooms[m.room];
            assert_eq!(room.tile(i32::from(m.col), i32::from(m.row)), Tile::Floor, "open {open} raid {raid}");
            assert_ne!(m.room, 0, "never the entrance room (open {open} raid {raid})");
            for game in [run.folly_room, run.gantry_room, run.race3d_room, run.captive.as_ref().map(|c| c.room)] {
                assert_ne!(game, Some(m.room), "never in a room with a game or a cage (open {open} raid {raid})");
            }
            if open {
                assert_eq!(m.room, run.dungeon.open.as_ref().unwrap().map, "an open floor's meeting stands on its map");
                assert!(run.chamber_at(m.tile_point()).is_some(), "in a chamber");
            }
        }
    }
}

#[test]
fn the_six_turn_through_a_stretch_and_a_shrine_never_stands_at_a_dreads_stair() {
    for raid in 0..24u64 {
        for c in 0..3u32 {
            let seed = raid * 0x1234_5679 + 11;
            let n = if c == 0 { 2 } else { 5 };
            let kinds: Vec<road::Kind> = (0..n).map(|m| road::kind_for(seed, c, m, m == n - 1)).collect();
            assert_ne!(*kinds.last().unwrap(), road::Kind::Shrine, "the stair's floor has no shrine: {kinds:?}");
            if n == 5 {
                let mut seen = kinds[..4].to_vec();
                seen.sort_by_key(|k| *k as u8);
                seen.dedup();
                assert!(seen.len() >= 3, "a stretch varies: {kinds:?}");
            }
        }
    }
    // And laid on the floors themselves: the floor above a village never
    // lays one.
    for floor in [9, 19, 29] {
        for raid in 0..12u64 {
            let mut run = Run::new(41, 1, None);
            run.raid_id = raid * 104_729 + 1;
            run.home = deep_home();
            run.deep_floor_for_test(floor);
            assert!(the_meeting(&run).kind != road::Kind::Shrine, "floor {floor}");
        }
    }
}

#[test]
fn a_meeting_is_laid_the_same_way_every_time_and_clears_with_the_floor() {
    let a = road_run(1, 13);
    let b = road_run(1, 13);
    assert_eq!(the_meeting(&a), the_meeting(&b), "nothing in it draws from the run's dice");
    let mut run = road_run(1, 13);
    run.descend_for_test();
    assert_eq!(run.floor(), 14);
    assert!(run.deep.as_ref().unwrap().meeting.is_none(), "an off floor holds none");
    run.descend_for_test();
    assert!(run.deep.as_ref().unwrap().meeting.is_some(), "and the next on holds one");
}

#[test]
fn a_lost_patrol_shares_light_for_gold_and_lights_a_lamp_for_the_errand() {
    let mut run = road_run(1, 13);
    let beacons = ask(&mut run, 13, Errand::Beacons, 5);
    force(&mut run, road::Kind::Patrol, Culture::Emberfolk);
    go(&mut run);
    give(&mut run, 1, Spoil::Gold, 120);
    speak(&mut run, 1);
    let view = run.road_view(1).unwrap();
    assert_eq!(view.name, "CAPTAIN RIVET");
    assert_eq!(view.answers[0], "1: SHARE YOUR LIGHT (50 GOLD)", "{:?}", view.answers);
    key(&mut run, 1, 1);
    assert_eq!(gold(&run, 1), 120 - road::OIL, "the light costs what it says");
    assert_eq!(favour(&run, Culture::Emberfolk), 2);
    assert_eq!(run.home.deep.missions[beacons].have, 1, "they light a lamp on their way");
    assert!(the_meeting(&run).settled());
    let said = run.found.clone().unwrap().2;
    assert!(said.contains("ERRAND +1") && said.contains("FAVOUR WITH THE EMBERFOLK +2"), "{said}");
    // Spoken to again, they say what is done.
    beside(&mut run, 1);
    press(&mut run, 1);
    assert!(run.road_view(1).unwrap().words.contains("YOUR LIGHT IS IN OUR BRAZIER"));
    press(&mut run, 1);
    assert!(run.road_view(1).is_none(), "E closes it");
}

#[test]
fn a_patrol_asks_a_price_the_party_may_not_have() {
    let mut run = road_run(1, 13);
    force(&mut run, road::Kind::Patrol, Culture::Capfolk);
    go(&mut run);
    speak(&mut run, 1);
    assert!(run.road_view(1).unwrap().answers[0].contains("NEED 50 GOLD"));
    key(&mut run, 1, 1);
    assert!(!the_meeting(&run).settled(), "no gold, no light");
    assert_eq!(favour(&run, Culture::Capfolk), 0);
    assert!(run.found.clone().unwrap().2.contains("NEED 50 GOLD"));
    // And taking their gems costs them, not the party.
    key(&mut run, 1, 2);
    assert_eq!(gems(&run, 1), road::TAKEN_GEMS);
    assert_eq!(favour(&run, Culture::Capfolk), -2);
}

#[test]
fn seeing_a_patrol_to_the_stair_is_a_real_fight_and_pays_when_it_is_won() {
    let mut run = road_run(2, 13);
    force(&mut run, road::Kind::Patrol, Culture::Glimmerkin);
    go(&mut run);
    speak(&mut run, 1);
    key(&mut run, 1, 3);
    let m = the_meeting(&run);
    assert_eq!(m.stage, road::Stage::Fighting);
    assert!(!m.foes.is_empty(), "something came after them");
    assert!(run.enemies.iter().any(|e| e.hp > 0), "and it is in the room");
    assert!(run.enemies.len() >= road::PURSUERS, "{} foes", run.enemies.len());
    // Nobody talks while it lasts.
    press(&mut run, 1);
    assert!(run.road_view(1).is_none());
    assert_eq!(favour(&run, Culture::Glimmerkin), 0, "nothing is paid before the fight is won");
    let before = [gold(&run, 1), gold(&run, 2)];
    kill_foes(&mut run);
    assert!(the_meeting(&run).settled());
    assert_eq!(favour(&run, Culture::Glimmerkin), 3);
    let purse = purse(13);
    assert!(gold(&run, 1) >= before[0] + purse && gold(&run, 2) >= before[1] + purse, "every knight is paid {purse}");
    assert!(run.found.clone().unwrap().2.contains("FAVOUR WITH THE GLIMMERKIN +3"));
}

#[test]
fn leaving_the_room_in_the_middle_of_a_fight_puts_the_choice_back() {
    let mut run = road_run(1, 13);
    force(&mut run, road::Kind::Patrol, Culture::Capfolk);
    go(&mut run);
    speak(&mut run, 1);
    key(&mut run, 1, 3);
    assert_eq!(the_meeting(&run).stage, road::Stage::Fighting);
    // Out of the room: what rose falls back with it.
    run.enter_for_test(0);
    calm(&mut run, 2);
    let m = the_meeting(&run);
    assert_eq!(m.stage, road::Stage::Open, "back to the question");
    assert!(m.foes.is_empty() && m.choice.is_none());
    assert_eq!(favour(&run, Culture::Capfolk), 0);
}

#[test]
fn a_caravan_sells_each_ware_once_and_takes_a_gem() {
    let mut run = road_run(1, 15);
    force(&mut run, road::Kind::Caravan, Culture::Saltkin);
    go(&mut run);
    give(&mut run, 1, Spoil::Gold, 200);
    speak(&mut run, 1);
    let view = run.road_view(1).unwrap();
    assert_eq!(view.name, "TRADER GRAIN");
    assert!(view.answers[0].starts_with("1: A GIFT FOR"), "{:?}", view.answers);
    assert_eq!(view.answers[2], "3: SELL A GEM (NO GEM)", "{:?}", view.answers);
    assert_eq!(view.answers[1], "2: A DRAUGHT OF DEEP-HONEY (40 GOLD)");
    // A gift for the village ahead, carried.
    key(&mut run, 1, 1);
    assert_eq!(gold(&run, 1), 200 - road::GIFT);
    let cargo = run.deep.as_ref().unwrap().cargo.clone();
    assert_eq!(cargo, vec![road::Cargo { parcel: road::Parcel::Gift, from: Culture::Saltkin }]);
    // Not twice.
    key(&mut run, 1, 1);
    assert_eq!(gold(&run, 1), 200 - road::GIFT, "sold once");
    assert_eq!(run.deep.as_ref().unwrap().cargo.len(), 1);
    // A draught mends a hurt knight.
    run.players.get_mut(&1).unwrap().hp = 10;
    key(&mut run, 1, 2);
    assert_eq!(gold(&run, 1), 200 - road::GIFT - road::DRAUGHT);
    assert_eq!(run.players[&1].hp, 10 + run.players[&1].max_hp * road::MEND_PERCENT / 100, "half their health back");
    // A gem sold for what a gem is worth.
    give(&mut run, 1, Spoil::Gem, 1);
    let before = gold(&run, 1);
    key(&mut run, 1, 3);
    assert_eq!(gold(&run, 1), before + Spoil::Gem.worth());
    assert_eq!(gems(&run, 1), 0);
    assert!(the_meeting(&run).settled(), "all three sold");
}

#[test]
fn a_caravan_sells_a_relic_when_the_chapter_asks_for_one() {
    let mut run = road_run(1, 15);
    let gather = ask(&mut run, 15, Errand::Gather, 6);
    force(&mut run, road::Kind::Caravan, Culture::Rootkin);
    go(&mut run);
    give(&mut run, 1, Spoil::Gold, 100);
    speak(&mut run, 1);
    assert_eq!(run.road_view(1).unwrap().answers[0], "1: BUY A HEARTSEED (60 GOLD)");
    key(&mut run, 1, 1);
    assert_eq!(run.home.deep.missions[gather].have, 1, "bought, and it counts");
    assert!(run.deep.as_ref().unwrap().cargo.is_empty());
}

#[test]
fn the_hermit_lights_his_lamp_for_the_right_answer_and_the_dark_answers_a_wrong_one() {
    let mut right = road_run(1, 13);
    force(&mut right, road::Kind::Hermit, Culture::Capfolk);
    go(&mut right);
    speak(&mut right, 1);
    let m = the_meeting(&right).clone();
    let (_, answer, _) = road::RIDDLES[usize::from(m.riddle)];
    let view = right.road_view(1).unwrap();
    assert!(view.answers[usize::from(m.key)].ends_with(answer), "the right answer stands where the meeting says: {:?}", view.answers);
    right.players.get_mut(&1).unwrap().hp = 20;
    let ahead = right.home.deep.culture_at(20);
    key(&mut right, 1, m.key + 1);
    assert!(the_meeting(&right).settled());
    assert!(right.players[&1].hp > 20, "mended");
    assert_eq!(gems(&right, 1), road::RIDDLE_GEMS);
    assert_eq!(favour(&right, ahead), 1);
    assert!(right.enemies.is_empty(), "nothing answers a right answer");

    let mut wrong = road_run(2, 13);
    force(&mut wrong, road::Kind::Hermit, Culture::Capfolk);
    go(&mut wrong);
    speak(&mut wrong, 1);
    let m = the_meeting(&wrong).clone();
    let pick = (m.key + 1) % 3 + 1;
    key(&mut wrong, 1, pick);
    assert!(the_meeting(&wrong).settled(), "one answer each");
    assert_eq!(gems(&wrong, 1), 0);
    assert!(wrong.enemies.len() >= road::AMBUSH, "the dark answers: {} foes", wrong.enemies.len());
    assert!(wrong.found.clone().unwrap().2.contains("WRONG"));
}

#[test]
fn the_hermits_calm_does_not_reach_a_dread_already_risen() {
    // On the floor of the dread's own stair it is up already: no claim.
    let mut run = road_run(1, 19);
    force(&mut run, road::Kind::Hermit, Culture::Capfolk);
    run.deep.as_mut().unwrap().stir = 2;
    go(&mut run);
    speak(&mut run, 1);
    let key_slot = the_meeting(&run).key;
    key(&mut run, 1, key_slot + 1);
    assert_eq!(run.deep.as_ref().unwrap().stir, 2);
    assert!(!run.found.clone().unwrap().2.contains("LESS STIRRED"));
}

#[test]
fn the_riddles_are_each_three_answers_with_one_right() {
    for (question, right, wrong) in road::RIDDLES {
        assert!(question.ends_with('?') && !right.is_empty());
        let all = [right, wrong[0], wrong[1]];
        assert!(all.iter().all(|a| a.len() <= 24), "short enough for a line: {all:?}");
        assert!(wrong.iter().all(|w| *w != right));
    }
    // The right answer's place is the meeting's own, and moves.
    let places: std::collections::BTreeSet<u8> = (0..24u64)
        .map(|raid| {
            let mut run = Run::new(41, 1, None);
            run.raid_id = raid * 31 + 5;
            run.home = deep_home();
            run.deep_floor_for_test(ROAD);
            the_meeting(&run).key
        })
        .collect();
    assert!(places.len() >= 2, "the right answer is not always first: {places:?}");
}

#[test]
fn captives_wait_in_their_cage_until_a_knight_comes_and_their_captors_must_fall() {
    let mut run = road_run(2, 15);
    force(&mut run, road::Kind::Captives, Culture::Roostfolk);
    let room = the_meeting(&run).room;
    run.enter_for_test(room);
    run.enemies.clear();
    let (x, y) = the_meeting(&run).tile_point();
    // Far off, they stay quiet.
    put(&mut run, 1, (3.0, 3.0));
    put(&mut run, 2, (3.5, 3.0));
    calm(&mut run, 3);
    if ((x - 3.0) * TILE_UNITS).hypot((y - 3.0) * TILE_UNITS) > road::RISE_REACH + 2.0 {
        assert_eq!(the_meeting(&run).stage, road::Stage::Waiting);
    }
    // Close, the captors rise: real foes, in the room.
    put(&mut run, 1, (x, y + 2.0));
    put(&mut run, 2, (x + 1.0, y + 2.0));
    calm(&mut run, 2);
    assert_eq!(the_meeting(&run).stage, road::Stage::Fighting);
    let raised = the_meeting(&run).foes.len();
    assert!(raised >= road::CAPTORS, "{raised} captors");
    assert!(run.enemies.iter().filter(|e| e.hp > 0).count() >= raised);
    // They cannot be spoken to while captors stand.
    press(&mut run, 1);
    assert!(run.road_view(1).is_none());
    kill_foes(&mut run);
    assert_eq!(the_meeting(&run).stage, road::Stage::Open);
    beside(&mut run, 1);
    press(&mut run, 1);
    assert!(run.road_view(1).unwrap().words.contains("THREE DELVERS OF THE ROOSTFOLK"));
    press(&mut run, 1);
    assert!(run.road_view(1).unwrap().words.contains("OPEN THE CAGE"));
}

#[test]
fn captors_rise_in_the_real_production_path_too() {
    // The same, through `enter` (its guardian economy in force), not the
    // test helper that sets it aside.
    let mut run = road_run(1, 15);
    force(&mut run, road::Kind::Captives, Culture::Roostfolk);
    let room = the_meeting(&run).room;
    run.enter(room, Some(2));
    run.enemies.clear();
    let (x, y) = the_meeting(&run).tile_point();
    put(&mut run, 1, (x, y + 2.0));
    calm(&mut run, 2);
    assert_eq!(the_meeting(&run).stage, road::Stage::Fighting);
    assert!(!the_meeting(&run).foes.is_empty(), "raised through the economy");
    assert!(run.enemies.iter().any(|e| e.hp > 0));
}

#[test]
fn freed_captives_pay_dig_or_carry_word_ahead() {
    // Their purse, each knight.
    let mut run = road_run(2, 15);
    force(&mut run, road::Kind::Captives, Culture::Rootkin);
    meeting_mut(&mut run).stage = road::Stage::Open;
    go(&mut run);
    speak(&mut run, 1);
    speak(&mut run, 2);
    let before = gold(&run, 2);
    key(&mut run, 1, 1);
    assert_eq!(gold(&run, 2), before + purse(15));
    assert_eq!(gems(&run, 2), 1);
    assert_eq!(favour(&run, Culture::Rootkin), 3);
    assert!(the_meeting(&run).talks.is_empty(), "the other knight's talk closes with it");

    // Their help, when no errand wants a relic: they tend the knights.
    let mut run = road_run(1, 15);
    force(&mut run, road::Kind::Captives, Culture::Rootkin);
    meeting_mut(&mut run).stage = road::Stage::Open;
    go(&mut run);
    speak(&mut run, 1);
    assert_eq!(run.road_view(1).unwrap().answers[1], "2: FREE THEM, THEY TEND YOUR WOUNDS");
    run.players.get_mut(&1).unwrap().hp = 10;
    key(&mut run, 1, 2);
    assert!(run.players[&1].hp > 10, "mended");
    assert_eq!(favour(&run, Culture::Rootkin), 2);

    // Their help, when an errand wants a relic.
    let mut run = road_run(1, 15);
    let gather = ask(&mut run, 15, Errand::Gather, 6);
    force(&mut run, road::Kind::Captives, Culture::Rootkin);
    meeting_mut(&mut run).stage = road::Stage::Open;
    go(&mut run);
    speak(&mut run, 1);
    assert!(run.road_view(1).unwrap().answers[1].contains("DIG OUT A HEARTSEED"));
    key(&mut run, 1, 2);
    assert_eq!(run.home.deep.missions[gather].have, 1);
    assert_eq!(favour(&run, Culture::Rootkin), 2);

    // Word to the village ahead.
    let mut run = road_run(1, 15);
    force(&mut run, road::Kind::Captives, Culture::Rootkin);
    meeting_mut(&mut run).stage = road::Stage::Open;
    go(&mut run);
    speak(&mut run, 1);
    key(&mut run, 1, 3);
    assert_eq!(run.deep.as_ref().unwrap().cargo, vec![road::Cargo { parcel: road::Parcel::Word, from: Culture::Rootkin }]);
    assert_eq!(favour(&run, Culture::Rootkin), 1);
}

#[test]
fn a_drowned_shrine_offers_a_hard_choice() {
    // The hoard: rich now, and the dread of the village ahead wakes stronger.
    let mut run = road_run(2, 13);
    force(&mut run, road::Kind::Shrine, Culture::Chimefolk);
    go(&mut run);
    let before = gold(&run, 2);
    speak(&mut run, 1);
    key(&mut run, 1, 1);
    assert_eq!(gems(&run, 1), road::HOARD_GEMS);
    assert_eq!(gems(&run, 2), road::HOARD_GEMS, "every knight");
    assert_eq!(gold(&run, 2), before + road::HOARD_GOLD);
    assert_eq!(favour(&run, Culture::Chimefolk), -3);
    assert_eq!(run.deep.as_ref().unwrap().stir, 1);
    assert!(run.road_notes().iter().any(|n| n.contains("IS STIRRED")), "{:?}", run.road_notes());

    // An offering: poorer, mended, and a stirred dread settles.
    let mut run = road_run(1, 13);
    force(&mut run, road::Kind::Shrine, Culture::Chimefolk);
    run.deep.as_mut().unwrap().stir = 2;
    go(&mut run);
    give(&mut run, 1, Spoil::Gold, 100);
    run.players.get_mut(&1).unwrap().hp = 12;
    speak(&mut run, 1);
    key(&mut run, 1, 2);
    assert_eq!(gold(&run, 1), 100 - road::OFFERING);
    assert!(run.players[&1].hp > 12);
    assert_eq!(favour(&run, Culture::Chimefolk), 2);
    assert_eq!(run.deep.as_ref().unwrap().stir, 1, "one step calmer");

    // The bell: a fight, then a gem each and the shrine's relic.
    let mut run = road_run(1, 13);
    let gather = ask(&mut run, 13, Errand::Gather, 6);
    force(&mut run, road::Kind::Shrine, Culture::Chimefolk);
    go(&mut run);
    speak(&mut run, 1);
    key(&mut run, 1, 3);
    assert_eq!(the_meeting(&run).stage, road::Stage::Fighting);
    assert!(run.enemies.iter().any(|e| e.hp > 0));
    kill_foes(&mut run);
    assert!(the_meeting(&run).settled());
    assert_eq!(gems(&run, 1), 1);
    assert_eq!(run.home.deep.missions[gather].have, 1);
    assert_eq!(favour(&run, Culture::Chimefolk), 1);
}

#[test]
fn an_envoy_gives_a_letter_of_peace_or_a_challenge_to_carry() {
    let mut run = road_run(1, 13);
    force(&mut run, road::Kind::Envoy, Culture::Lampwick);
    go(&mut run);
    speak(&mut run, 1);
    let view = run.road_view(1).unwrap();
    assert_eq!(view.name, "ENVOY GUTTERWICK");
    assert_eq!(view.answers[1], "2: CARRY THEIR CHALLENGE (+100 GOLD)");
    key(&mut run, 1, 2);
    assert_eq!(gold(&run, 1), road::BOUNTY);
    assert_eq!(run.deep.as_ref().unwrap().cargo, vec![road::Cargo { parcel: road::Parcel::Challenge, from: Culture::Lampwick }]);

    let mut run = road_run(1, 13);
    force(&mut run, road::Kind::Envoy, Culture::Lampwick);
    go(&mut run);
    speak(&mut run, 1);
    key(&mut run, 1, 3);
    assert_eq!(favour(&run, Culture::Lampwick), -1);
    assert!(run.deep.as_ref().unwrap().cargo.is_empty());
}

#[test]
fn an_envoy_sent_home_and_captives_let_out_leave_the_room() {
    let mut run = road_run(1, 13);
    force(&mut run, road::Kind::Envoy, Culture::Lampwick);
    go(&mut run);
    speak(&mut run, 1);
    key(&mut run, 1, 3);
    // Still there a moment, to be spoken to.
    beside(&mut run, 1);
    press(&mut run, 1);
    assert!(run.road_view(1).is_some());
    press(&mut run, 1);
    calm(&mut run, road::ENVOY_LEAVES);
    assert!(the_meeting(&run).gone(run.tick));
    press(&mut run, 1);
    assert!(run.road_view(1).is_none(), "nobody is left to speak to");

    let mut run = road_run(1, 15);
    force(&mut run, road::Kind::Captives, Culture::Rootkin);
    meeting_mut(&mut run).stage = road::Stage::Open;
    go(&mut run);
    speak(&mut run, 1);
    key(&mut run, 1, 1);
    assert!(!the_meeting(&run).gone(run.tick));
    calm(&mut run, road::CAPTIVES_LEAVE + 2);
    assert!(the_meeting(&run).gone(run.tick));
    press(&mut run, 1);
    assert!(run.road_view(1).is_none());
    // A caravan, a hermit, a shrine, a patrol stay where they are.
    for kind in [road::Kind::Caravan, road::Kind::Hermit, road::Kind::Shrine, road::Kind::Patrol] {
        let mut run = road_run(1, 13);
        force(&mut run, kind, Culture::Rootkin);
        meeting_mut(&mut run).stage = road::Stage::Settled;
        meeting_mut(&mut run).at = Some(1);
        run.tick = 5000;
        assert!(!the_meeting(&run).gone(run.tick), "{kind:?}");
    }
}

#[test]
fn the_envoy_is_always_a_rival_of_the_village_ahead() {
    for raid in 0..80u64 {
        for floor in [ROAD, 9, 11, 13, 15, 17, 19] {
            let mut run = Run::new(41, 1, None);
            run.raid_id = raid * 977 + 9;
            run.home = deep_home();
            run.deep_floor_for_test(floor);
            let ahead = run.home.deep.culture_at(road::ahead(floor));
            let m = the_meeting(&run);
            if m.kind == road::Kind::Envoy {
                assert_ne!(m.people, ahead, "raid {raid} floor {floor}");
            }
        }
    }
}

#[test]
fn what_the_party_carries_is_delivered_as_the_star_lands() {
    let mut run = road_run(1, 19);
    let ahead = run.home.deep.culture_at(20);
    let envoy = WHEEL[(WHEEL.iter().position(|&c| c == ahead).unwrap() + 2) % WHEEL.len()];
    {
        let d = run.deep.as_mut().unwrap();
        d.cargo = vec![
            road::Cargo { parcel: road::Parcel::Gift, from: ahead },
            road::Cargo { parcel: road::Parcel::Peace, from: envoy },
        ];
        d.stir = 2;
    }
    run.descend_for_test();
    assert_eq!(run.floor(), 20);
    assert!(run.deep.as_ref().unwrap().cargo.is_empty(), "delivered");
    assert_eq!(run.deep.as_ref().unwrap().stir, 0, "the dread's stir is spent");
    assert_eq!(favour(&run, ahead), 2, "the gift and the letter");
    assert_eq!(favour(&run, envoy), 1);
    let v = run.deep.as_ref().unwrap().village.as_ref().unwrap();
    assert_eq!(v.boon, 2);
    assert!(v.news.contains("GIFT") && v.news.contains("LETTER OF PEACE"), "{}", v.news);
    let hope = v.hope;
    calm(&mut run, village::FALL + 2);
    let v = run.deep.as_ref().unwrap().village.as_ref().unwrap();
    assert!(v.hope >= hope + 3, "the star's own step and two more: {}", v.hope);
    assert!(run.valid_snapshot());
}

#[test]
fn a_village_already_lit_does_not_claim_a_step_it_cannot_take() {
    let mut run = road_run(1, 19);
    let ahead = run.home.deep.culture_at(20);
    run.home.deep.towns.push(Town { lit: true, ..Town::new(20, ahead) });
    run.deep.as_mut().unwrap().cargo = vec![road::Cargo { parcel: road::Parcel::Gift, from: ahead }];
    run.descend_for_test();
    let v = run.deep.as_ref().unwrap().village.as_ref().unwrap();
    assert_eq!(v.hope, village::HOPE_FULL);
    assert!(!v.news.contains("LIGHTS"), "{}", v.news);
    assert!(v.news.contains("REACHES"), "{}", v.news);
}

#[test]
fn a_village_of_friends_lights_a_step_more_and_pays_more_and_a_wary_one_holds_back() {
    let landing = |favour: i8| {
        let mut run = road_run(1, 19);
        let ahead = run.home.deep.culture_at(20);
        run.home.deep.road.shift(ahead, favour);
        run.descend_for_test();
        calm(&mut run, village::FALL + 2);
        (run.deep.as_ref().unwrap().village.as_ref().unwrap().hope, run)
    };
    let (plain, _) = landing(0);
    let (friend, run) = landing(road::FRIEND);
    let (wary, _) = landing(road::WARY);
    assert_eq!(friend, plain + 1, "friends: a step more");
    assert_eq!(wary, plain - 1, "wary: the star's own light is held back");
    assert!(run.deep.as_ref().unwrap().village.as_ref().unwrap().news.contains("KNOWS YOUR NAME"));
    // The elder says so too, and a wary village's says that.
    let elder = |favour: i8| {
        let mut run = road_run(1, 19);
        let ahead = run.home.deep.culture_at(20);
        run.home.deep.road.shift(ahead, favour);
        run.descend_for_test();
        calm(&mut run, village::FALL + 2);
        let (x, y) = village::SPEAKERS[0];
        put(&mut run, 1, (x, y + 0.6));
        press(&mut run, 1);
        run.talk_view(1).unwrap().1
    };
    assert!(elder(road::FRIEND).starts_with("WE KNOW YOU FROM THE ROAD"));
    assert!(elder(road::WARY).starts_with("THE ROAD HAS TOLD US OF YOU, AND NOT KINDLY"));
    assert!(!elder(0).contains("FROM THE ROAD") && !elder(0).contains("TOLD US OF YOU"), "a people with no opinion say nothing of it");
    // Errands pay friends more.
    let mut run = road_run(1, 15);
    let village = 10;
    let culture = run.home.deep.culture_at(village);
    run.home.deep.road.shift(culture, road::FRIEND);
    run.home.deep.towns.push(Town::new(village, culture));
    let m = ask(&mut run, 15, Errand::Gather, 1);
    let before = gold(&run, 1);
    run.count_errand(m, 1, 15);
    assert_eq!(gold(&run, 1), before + purse(15) + road::FRIEND_PURSE);
    assert!(run.found.clone().unwrap().2.contains("FRIENDS PAY MORE"));
    // Not friends, no more.
    let mut run = road_run(1, 15);
    run.home.deep.towns.push(Town::new(village, culture));
    let m = ask(&mut run, 15, Errand::Gather, 1);
    let before = gold(&run, 1);
    run.count_errand(m, 1, 15);
    assert_eq!(gold(&run, 1), before + purse(15));
}

#[test]
fn a_challenge_carried_costs_favour_where_it_is_read() {
    let mut run = road_run(1, 19);
    let ahead = run.home.deep.culture_at(20);
    let envoy = WHEEL[(WHEEL.iter().position(|&c| c == ahead).unwrap() + 3) % WHEEL.len()];
    run.home.deep.road.shift(ahead, 1);
    run.deep.as_mut().unwrap().cargo = vec![road::Cargo { parcel: road::Parcel::Challenge, from: envoy }];
    run.descend_for_test();
    assert_eq!(favour(&run, ahead), -1);
    assert_eq!(favour(&run, envoy), 1);
    let v = run.deep.as_ref().unwrap().village.as_ref().unwrap();
    assert_eq!(v.boon, 0);
    assert!(v.news.contains("NOT TAKEN KINDLY"));
}

#[test]
fn a_stirred_dread_is_more_and_angrier_inside_the_checkers_bounds() {
    for culture in WHEEL {
        let base = dread::boss(culture, 3);
        for stir in 1..=road::MOST_STIR {
            let mut b = dread::boss(culture, 3);
            dread::awaken(&mut b, stir);
            assert!(b.hp > base.hp || base.hp >= 900, "{culture:?} stir {stir}: {} vs {}", b.hp, base.hp);
            assert!(b.rage >= base.rage);
            assert!(b.windups[0].every <= base.windups[0].every);
            assert!(b.hp <= 900 && b.rage <= 0.6 && b.windups[0].every >= 150);
            assert_eq!(b.windups.len(), base.windups.len());
        }
        // No stir, no change.
        let mut same = dread::boss(culture, 3);
        dread::awaken(&mut same, 0);
        assert_eq!(same, base);
    }
}

#[test]
fn the_road_stirs_the_dread_that_waits_at_the_next_stair() {
    let at_stair = |stir: u8| {
        let mut run = Run::new(41, 1, None);
        run.home = deep_home();
        run.deep_floor_for_test(18);
        run.deep.as_mut().unwrap().stir = stir;
        run.descend_for_test();
        assert_eq!(run.floor(), 19);
        let i = run.dread_at().expect("the dread holds the stair");
        run.bosses[i].hp
    };
    let calm_hp = at_stair(0);
    let stirred = at_stair(2);
    assert_eq!(stirred, calm_hp + 2 * dread::STIR_HP, "two stirs, two helpings of health");
}

#[test]
fn an_old_save_with_no_road_in_it_reads_and_the_road_leaves_a_clean_save_alone() {
    // A Descent saved before the road existed.
    let old = r#"{"floor":13,"signs":[]}"#;
    let d: Descent = serde_json::from_str(old).unwrap();
    assert!(d.meeting.is_none() && d.cargo.is_empty() && d.stir == 0);
    // A chronicle with no ledger in it.
    let c: Chronicle = serde_json::from_str(r#"{"wheel":3,"deepest":12}"#).unwrap();
    assert!(c.road.is_empty() && c.valid());
    // An empty ledger leaves a save as it was.
    let json = serde_json::to_string(&Chronicle { wheel: 3, deepest: 12, ..Chronicle::default() }).unwrap();
    assert!(!json.contains("road"), "{json}");
    // A bare descent saves none of the road.
    let v = serde_json::to_string(&Descent { floor: 20, ..Default::default() }).unwrap();
    assert!(!v.contains("meeting") && !v.contains("cargo") && !v.contains("stir"), "{v}");
    // A village saved before the road's news reads.
    let before = r#"{"floor":10,"culture":"capfolk","arrived":5,"hope":2}"#;
    let village: Village = serde_json::from_str(before).unwrap();
    assert!(village.boon == 0 && !village.cold && village.news.is_empty());
}

#[test]
fn a_run_on_the_road_saves_and_reads_back_whole() {
    let mut run = road_run(2, 15);
    force(&mut run, road::Kind::Caravan, Culture::Saltkin);
    go(&mut run);
    give(&mut run, 1, Spoil::Gold, 100);
    speak(&mut run, 1);
    key(&mut run, 1, 1);
    run.home.deep.road.shift(Culture::Saltkin, 4);
    let json = serde_json::to_string(&run).unwrap();
    let back: Run = serde_json::from_str(&json).unwrap();
    assert!(back.valid_snapshot());
    assert_eq!(back.deep, run.deep);
    assert_eq!(back.home.deep, run.home.deep);
    assert_eq!(back.road_view(1), run.road_view(1));
}

#[test]
fn a_ledger_stays_inside_its_bounds() {
    let mut l = road::Ledger::default();
    for _ in 0..30 {
        l.shift(Culture::Capfolk, 3);
        l.shift(Culture::Saltkin, -3);
    }
    assert_eq!(l.of(Culture::Capfolk), road::FAVOUR_MOST);
    assert_eq!(l.of(Culture::Saltkin), -road::FAVOUR_MOST);
    assert!(l.valid());
    assert_eq!(l.tier(Culture::Capfolk), road::Tier::Friend);
    assert_eq!(l.tier(Culture::Saltkin), road::Tier::Wary);
    assert_eq!(l.tier(Culture::Rootkin), road::Tier::Known);
    l.favour[0] = 12;
    assert!(!l.valid());
}

#[test]
fn walking_off_ends_a_talk_and_two_knights_answer_one_meeting_once() {
    let mut run = road_run(2, 13);
    force(&mut run, road::Kind::Envoy, Culture::Lampwick);
    go(&mut run);
    speak(&mut run, 1);
    speak(&mut run, 2);
    assert_eq!(the_meeting(&run).talks.len(), 2);
    // Walk off: the talk is gone.
    let (x, y) = the_meeting(&run).tile_point();
    put(&mut run, 2, (x + 9.0, y + 9.0));
    calm(&mut run, 2);
    assert_eq!(the_meeting(&run).talks.len(), 1);
    speak(&mut run, 2);
    // Both answer the same tick: it settles once.
    run.step(&BTreeMap::from([(1, Input { cast: 1, ..Default::default() }), (2, Input { cast: 3, ..Default::default() })]));
    run.step(&BTreeMap::new());
    assert!(the_meeting(&run).settled());
    assert_eq!(the_meeting(&run).choice, Some(1), "the first answer in seat order stands");
    assert_eq!(run.deep.as_ref().unwrap().cargo.len(), 1);
    assert_eq!(favour(&run, Culture::Lampwick), 0);
}

#[test]
fn a_fight_holds_the_talk_back() {
    let mut run = road_run(1, 13);
    force(&mut run, road::Kind::Envoy, Culture::Lampwick);
    go(&mut run);
    let (x, y) = the_meeting(&run).tile_point();
    run.spawn_at_for_test(EnemyKind::Skeleton, (x + 6.0) * TILE_UNITS, (y + 3.0) * TILE_UNITS);
    put(&mut run, 1, (x, y + 1.0));
    press(&mut run, 1);
    assert!(run.road_view(1).is_none(), "they wait out the fighting");
    assert!(run.found.clone().unwrap().2.contains("WAIT FOR THE FIGHTING"));
    run.enemies.clear();
    press(&mut run, 1);
    assert!(run.road_view(1).is_some());
}

#[test]
fn the_tracker_says_what_the_road_holds() {
    let mut run = road_run(1, 13);
    force(&mut run, road::Kind::Patrol, Culture::Capfolk);
    let notes = run.road_notes();
    assert!(notes.iter().any(|n| n == "ON THIS FLOOR: A LOST PATROL"), "{notes:?}");
    run.deep.as_mut().unwrap().cargo.push(road::Cargo { parcel: road::Parcel::Word, from: Culture::Capfolk });
    run.deep.as_mut().unwrap().stir = 1;
    let notes = run.road_notes();
    assert!(notes.iter().any(|n| n.starts_with("CARRYING: WORD FOR")), "{notes:?}");
    assert!(notes.iter().any(|n| n.ends_with("IS STIRRED")), "{notes:?}");
    meeting_mut(&mut run).stage = road::Stage::Settled;
    assert!(!run.road_notes().iter().any(|n| n.starts_with("ON THIS FLOOR")), "a done meeting is not listed");
}

#[test]
fn the_trackers_road_lines_are_short_enough_to_stay_off_the_minimap() {
    // The longest town names, with every parcel and every notice up.
    for floor in [19u32, 89, 169] {
        for wheel in 1..=8u32 {
            let mut run = Run::new(41, 1, None);
            run.home = deep_home();
            run.home.deep.wheel = wheel;
            run.deep_floor_for_test(floor);
            let ahead = run.home.deep.culture_at(road::ahead(floor));
            run.home.deep.road.shift(ahead, road::FRIEND);
            let d = run.deep.as_mut().unwrap();
            d.stir = 2;
            d.cargo = [road::Parcel::Gift, road::Parcel::Word, road::Parcel::Peace, road::Parcel::Challenge]
                .into_iter()
                .map(|parcel| road::Cargo { parcel, from: Culture::Chimefolk })
                .collect();
            for kind in road::Kind::ALL {
                force(&mut run, kind, Culture::Lampwick);
                for note in run.road_notes() {
                    assert!(note.len() <= 44, "floor {floor} wheel {wheel} {kind:?}: {note:?} is {} long", note.len());
                }
            }
        }
    }
}

#[test]
fn the_meetings_work_on_open_floors_too() {
    let open_run = |floor: u32| {
        let mut run = Run::new(41, 1, None);
        run.home = deep_home();
        run.open_floors = true;
        run.deep_floor_for_test(floor);
        assert!(run.dungeon.open.is_some());
        run.enemies.clear();
        run
    };
    let mut run = open_run(15);
    force(&mut run, road::Kind::Patrol, Culture::Capfolk);
    assert_eq!(the_meeting(&run).room, run.dungeon.open.as_ref().unwrap().map);
    go(&mut run);
    assert_eq!(run.at, the_meeting(&run).room, "on the map");
    give(&mut run, 1, Spoil::Gold, 60);
    speak(&mut run, 1);
    key(&mut run, 1, 1);
    assert!(the_meeting(&run).settled());
    // A fight on the map: its foes keep to their chamber, and the talk
    // is held back until they are down.
    let mut run = open_run(15);
    force(&mut run, road::Kind::Shrine, Culture::Capfolk);
    go(&mut run);
    speak(&mut run, 1);
    key(&mut run, 1, 3);
    assert_eq!(the_meeting(&run).stage, road::Stage::Fighting);
    assert!(run.enemies.iter().any(|e| e.hp > 0));
    assert!(run.enemies.iter().filter(|e| e.hp > 0).all(|e| e.home.is_some()), "kept to their chamber");
    assert!(run.valid_snapshot());
}

#[test]
fn the_road_from_the_winding_stair_to_the_first_village_is_an_adventure_start_to_end() {
    // Down the Winding Stair onto the Deep Road, the way a party goes.
    let mut home = deep_home();
    home.landing = ROAD;
    let mut run = Run::at_home(4, 1, None, home, Default::default());
    run.descend_for_test();
    assert_eq!(run.floor(), ROAD);
    assert!(run.deep.as_ref().unwrap().meeting.is_some(), "something on the very first floor");
    assert!(run.valid_snapshot());
    // Meet it: an envoy's letter, whatever it is, taken along.
    let ahead = run.home.deep.culture_at(CHAPTER);
    let rival = super::cultures::WHEEL[(super::cultures::WHEEL.iter().position(|&c| c == ahead).unwrap() + 1) % 8];
    force(&mut run, road::Kind::Envoy, rival);
    go(&mut run);
    speak(&mut run, 1);
    key(&mut run, 1, 1);
    assert_eq!(run.deep.as_ref().unwrap().cargo.len(), 1);
    // Floor 8 holds none, floor 9 (the dread's stair) one more.
    run.descend_for_test();
    assert_eq!(run.floor(), ROAD + 1);
    assert!(run.deep.as_ref().unwrap().meeting.is_none());
    assert_eq!(run.deep.as_ref().unwrap().cargo.len(), 1, "carried down the stairs");
    run.descend_for_test();
    assert_eq!(run.floor(), 9);
    assert!(run.deep.as_ref().unwrap().meeting.is_some());
    assert_ne!(the_meeting(&run).kind, road::Kind::Shrine);
    // The village at the bottom takes the delivery as its star lands.
    run.descend_for_test();
    assert_eq!(run.floor(), CHAPTER);
    assert!(run.in_deep_village());
    let v = run.deep.as_ref().unwrap().village.as_ref().unwrap();
    assert!(v.news.contains("LETTER OF PEACE"), "{}", v.news);
    assert_eq!(favour(&run, ahead), 1);
    assert_eq!(favour(&run, rival), 1);
    assert!(run.deep.as_ref().unwrap().cargo.is_empty());
    assert!(run.valid_snapshot());
}

#[test]
fn the_first_gate_is_kept() {
    // The Deep opens below the Unknown only: a realm that has not stood on
    // its floor has no landing on the road.
    let mut home = Home { deepest: DEEPEST - 1, ..Home::default() };
    home.levels.insert(Station::Map, 4);
    assert!(!home.deep_road());
    assert!(!home.landings().contains(&ROAD));
    assert!(deep_home().landings().contains(&ROAD));
}

// ─── shots ───────────────────────────────────────────────────────────────────

fn shot_dir(name: &str) -> Option<std::path::PathBuf> {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS")?).join(name);
    std::fs::create_dir_all(&dir).unwrap();
    Some(dir)
}

fn save_img(dir: &std::path::Path, img: &crate::render::overworld::Img, name: &str) {
    let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
    out.extend(img.rgb_bytes());
    std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
}

/// What knight 1 sees.
fn save(dir: &std::path::Path, run: &Run, name: &str) {
    save_img(dir, &crate::render::overworld::arena::frame_for(run, 768, 448, Some(1)), name);
}

/// The room round the meeting, close: a `w` by `h` window on it.
fn close(dir: &std::path::Path, run: &Run, size: (i32, i32), name: &str) {
    let (x, y) = the_meeting(run).point();
    let whole = crate::render::overworld::arena::whole_room_for_test(run);
    let (cx, cy) = ((x * 8.0) as i32, (y * 8.0) as i32 - 10);
    let (ox, oy) = ((cx - size.0 / 2).clamp(0, (whole.w - size.0).max(0)), (cy - size.1 / 2).clamp(0, (whole.h - size.1).max(0)));
    save_img(dir, &crate::render::overworld::Img::from_view(&whole, ox, oy, size.0, size.1), name);
}

/// Whether the shots asked for (`DEEP_SHOTS`, a comma list; all if unset)
/// include `part`.
fn wanted(part: &str) -> bool {
    std::env::var("DEEP_SHOTS").map_or(true, |list| list.split(',').any(|p| p.trim() == part))
}

/// A party on floor `floor` with a `kind` of `people`'s, ready to be seen:
/// the floor's plate and dare put away, the knights past hurt, beside it.
fn scene(seats: u32, floor: u32, kind: road::Kind, people: Culture, open: bool) -> Run {
    let mut run = Run::new(41, 1, None);
    for id in 2..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.home = deep_home();
    run.open_floors = open;
    run.deep_floor_for_test(floor);
    for hero in run.players.values_mut() {
        hero.invulnerable = 100_000;
    }
    force(&mut run, kind, people);
    calm(&mut run, 30);
    run.skip_floor_plate(1);
    run.skip_floor_plate(2);
    run.dare = None;
    if kind == road::Kind::Captives {
        // The captors rise when a knight comes near: stand well off.
        run.enter_for_test(the_meeting(&run).room);
        run.enemies.clear();
        put(&mut run, 1, (3.0, 3.0));
        calm(&mut run, 4);
        return run;
    }
    go(&mut run);
    if seats > 1 {
        let (x, y) = the_meeting(&run).tile_point();
        put(&mut run, 2, (x - 2.0, y + 2.0));
    }
    calm(&mut run, 2);
    run.enemies.clear();
    run
}

#[test]
fn every_meeting_of_every_people_is_drawn_and_the_talk_is_put_up_as_a_board() {
    use crate::render::overworld::ink::said;
    let frame = |run: &Run| crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
    for culture in super::cultures::WHEEL {
        for kind in road::Kind::ALL {
            let run = scene(1, 13, kind, culture, false);
            let mut bare = run.clone();
            bare.deep.as_mut().unwrap().meeting = None;
            assert_ne!(frame(&run).content_hash(), frame(&bare).content_hash(), "{kind:?} of {culture:?} is drawn");
        }
    }
    // Every kind's talk puts its speaker's name up, with a portrait beside it.
    for kind in road::Kind::ALL {
        let mut run = scene(1, 13, kind, Culture::Capfolk, false);
        if kind == road::Kind::Captives {
            meeting_mut(&mut run).stage = road::Stage::Open;
            go(&mut run);
        }
        speak(&mut run, 1);
        let name = run.road_view(1).expect("the talk is open").name;
        let (_, read) = said::read(|| frame(&run));
        assert!(read.lines.iter().any(|l| l.text == name), "{kind:?}: {name} on the board");
    }
    let mut run = scene(1, 13, road::Kind::Patrol, Culture::Emberfolk, false);
    run.home.deep.road.shift(Culture::Emberfolk, 1);
    speak(&mut run, 1);
    let (_, read) = said::read(|| frame(&run));
    let words: Vec<&str> = read.lines.iter().map(|l| l.text.as_str()).collect();
    assert!(words.contains(&"CAPTAIN RIVET"), "{words:?}");
    assert!(words.iter().any(|w| w.starts_with("SHARE YOUR LIGHT")), "{words:?}");
    assert!(words.iter().any(|w| w.contains("ON THIS FLOOR: A LOST PATROL")), "the tracker names it: {words:?}");
    // And a settled meeting is not on the tracker, nor its talk on the board.
    meeting_mut(&mut run).stage = road::Stage::Settled;
    meeting_mut(&mut run).choice = Some(2);
    meeting_mut(&mut run).talks.clear();
    let (_, read) = said::read(|| frame(&run));
    assert!(!read.lines.iter().any(|l| l.text.contains("ON THIS FLOOR")));
}

#[test]
fn what_a_meeting_shows_is_its_state() {
    let frame = |run: &Run| crate::render::overworld::arena::frame_for(run, 768, 448, Some(1)).content_hash();
    // A patrol's lamp burns only once a light was shared.
    let cold = scene(1, 13, road::Kind::Patrol, Culture::Emberfolk, false);
    let mut shared = cold.clone();
    meeting_mut(&mut shared).stage = road::Stage::Settled;
    meeting_mut(&mut shared).choice = Some(1);
    let mut taken = cold.clone();
    meeting_mut(&mut taken).stage = road::Stage::Settled;
    meeting_mut(&mut taken).choice = Some(2);
    assert_ne!(frame(&cold), frame(&shared), "a shared light is lit");
    assert_ne!(frame(&shared), frame(&taken));
    // A cage with its captives, and one standing open and empty.
    let mut caged = scene(1, 15, road::Kind::Captives, Culture::Glimmerkin, false);
    let (x, y) = the_meeting(&caged).tile_point();
    put(&mut caged, 1, (x, y + 2.0));
    let mut freed = caged.clone();
    meeting_mut(&mut freed).stage = road::Stage::Settled;
    meeting_mut(&mut freed).choice = Some(1);
    meeting_mut(&mut freed).at = Some(0);
    assert_ne!(frame(&caged), frame(&freed));
    // A shrine's water: the hoard taken, the offering made.
    let shrine = scene(1, 13, road::Kind::Shrine, Culture::Chimefolk, false);
    let mut plundered = shrine.clone();
    meeting_mut(&mut plundered).stage = road::Stage::Settled;
    meeting_mut(&mut plundered).choice = Some(1);
    let mut offered = shrine.clone();
    meeting_mut(&mut offered).stage = road::Stage::Settled;
    meeting_mut(&mut offered).choice = Some(2);
    assert_ne!(frame(&shrine), frame(&plundered));
    assert_ne!(frame(&plundered), frame(&offered));
    // A caravan's wares, sold, leave the cloth.
    let caravan = scene(1, 15, road::Kind::Caravan, Culture::Saltkin, false);
    let mut sold = caravan.clone();
    meeting_mut(&mut sold).sold = 0b011;
    assert_ne!(frame(&caravan), frame(&sold));
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_deep_road_shots -- --ignored`:
/// each meeting of the road, in its states, close and as a knight sees it,
/// with its talk board; the tracker; a village taking delivery.
/// `DEEP_SHOTS` (a comma list: patrol, caravan, hermit, captives, shrine,
/// envoy, boards, tracker, delivery, peoples) draws only those.
#[test]
#[ignore]
fn write_deep_road_shots() {
    let Some(dir) = shot_dir("deep-road") else { return };
    let size = (224, 160);
    if wanted("patrol") {
        for (culture, tag) in [(Culture::Capfolk, "capfolk"), (Culture::Emberfolk, "emberfolk")] {
            let mut run = scene(2, 13, road::Kind::Patrol, culture, false);
            let (mx, my) = the_meeting(&run).tile_point();
            put(&mut run, 1, (mx + 8.0, my + 2.0));
            calm(&mut run, 20);
            close(&dir, &run, size, &format!("patrol-{tag}-a-cold"));
            let mut near = scene(2, 13, road::Kind::Patrol, culture, false);
            calm(&mut near, 20);
            save(&dir, &near, &format!("patrol-{tag}-b-seen"));
            close(&dir, &near, size, &format!("patrol-{tag}-c-near"));
            speak(&mut near, 1);
            save(&dir, &near, &format!("patrol-{tag}-d-ask"));
            give(&mut near, 1, Spoil::Gold, 100);
            let mut lit = near.clone();
            key(&mut lit, 1, 1);
            calm(&mut lit, 10);
            close(&dir, &lit, size, &format!("patrol-{tag}-e-lit"));
            save(&dir, &lit, &format!("patrol-{tag}-e-lit-seen"));
            calm(&mut lit, 60);
            close(&dir, &lit, size, &format!("patrol-{tag}-f-lit-later"));
        }
        let mut run = scene(2, 13, road::Kind::Patrol, Culture::Glimmerkin, false);
        speak(&mut run, 1);
        key(&mut run, 1, 3);
        calm(&mut run, 20);
        save(&dir, &run, "patrol-glimmerkin-g-escort-fight");
        kill_foes(&mut run);
        calm(&mut run, 12);
        close(&dir, &run, size, "patrol-glimmerkin-h-won");
        let mut run = scene(1, 13, road::Kind::Patrol, Culture::Saltkin, false);
        speak(&mut run, 1);
        key(&mut run, 1, 2);
        calm(&mut run, 12);
        close(&dir, &run, size, "patrol-saltkin-i-taken");
    }
    if wanted("caravan") {
        for (culture, tag) in [(Culture::Saltkin, "saltkin"), (Culture::Rootkin, "rootkin")] {
            let mut run = scene(2, 15, road::Kind::Caravan, culture, false);
            calm(&mut run, 20);
            close(&dir, &run, size, &format!("caravan-{tag}-a"));
            save(&dir, &run, &format!("caravan-{tag}-b-seen"));
            speak(&mut run, 1);
            give(&mut run, 1, Spoil::Gold, 200);
            give(&mut run, 1, Spoil::Gem, 1);
            key(&mut run, 1, 1);
            save(&dir, &run, &format!("caravan-{tag}-c-ask"));
            key(&mut run, 1, 2);
            calm(&mut run, 6);
            close(&dir, &run, size, &format!("caravan-{tag}-d-sold-some"));
        }
    }
    if wanted("hermit") {
        let mut run = scene(2, 13, road::Kind::Hermit, Culture::Capfolk, false);
        calm(&mut run, 20);
        close(&dir, &run, size, "hermit-a-waiting");
        save(&dir, &run, "hermit-b-seen");
        speak(&mut run, 1);
        save(&dir, &run, "hermit-c-riddle");
        let mut wrong = run.clone();
        let m = the_meeting(&run).clone();
        key(&mut run, 1, m.key + 1);
        calm(&mut run, 12);
        close(&dir, &run, size, "hermit-d-right");
        save(&dir, &run, "hermit-d-right-seen");
        key(&mut wrong, 1, (m.key + 1) % 3 + 1);
        calm(&mut wrong, 12);
        save(&dir, &wrong, "hermit-e-wrong");
        close(&dir, &wrong, size, "hermit-e-wrong-close");
    }
    if wanted("captives") {
        for (culture, tag) in [(Culture::Roostfolk, "roostfolk"), (Culture::Glimmerkin, "glimmerkin")] {
            let mut run = scene(2, 15, road::Kind::Captives, culture, false);
            put(&mut run, 2, (3.5, 3.0));
            calm(&mut run, 4);
            close(&dir, &run, size, &format!("captives-{tag}-a-caged"));
            let (x, y) = the_meeting(&run).tile_point();
            put(&mut run, 1, (x + 3.0, y + 3.0));
            put(&mut run, 2, (x - 3.0, y + 3.0));
            calm(&mut run, 40);
            save(&dir, &run, &format!("captives-{tag}-b-captors"));
            kill_foes(&mut run);
            calm(&mut run, 4);
            beside(&mut run, 1);
            close(&dir, &run, size, &format!("captives-{tag}-c-open"));
            press(&mut run, 1);
            press(&mut run, 1);
            save(&dir, &run, &format!("captives-{tag}-d-ask"));
            key(&mut run, 1, 1);
            for at in [6, 30, 60] {
                calm(&mut run, at);
                close(&dir, &run, (224, 224), &format!("captives-{tag}-e-freed-{at:03}"));
            }
        }
    }
    if wanted("shrine") {
        let mut run = scene(2, 13, road::Kind::Shrine, Culture::Chimefolk, false);
        calm(&mut run, 20);
        close(&dir, &run, size, "shrine-chimefolk-a");
        save(&dir, &run, "shrine-chimefolk-b-seen");
        speak(&mut run, 1);
        save(&dir, &run, "shrine-chimefolk-c-ask");
        let mut hoard = run.clone();
        let mut offer = run.clone();
        let mut bell = run.clone();
        key(&mut hoard, 1, 1);
        calm(&mut hoard, 20);
        close(&dir, &hoard, size, "shrine-chimefolk-d-hoard");
        save(&dir, &hoard, "shrine-chimefolk-d-hoard-seen");
        give(&mut offer, 1, Spoil::Gold, 100);
        key(&mut offer, 1, 2);
        calm(&mut offer, 20);
        close(&dir, &offer, size, "shrine-chimefolk-e-offering");
        key(&mut bell, 1, 3);
        calm(&mut bell, 25);
        close(&dir, &bell, size, "shrine-chimefolk-f-bell");
        save(&dir, &bell, "shrine-chimefolk-f-bell-seen");
        let run = scene(1, 17, road::Kind::Shrine, Culture::Lampwick, false);
        close(&dir, &run, size, "shrine-lampwick-g");
    }
    if wanted("envoy") {
        let mut run = scene(2, 13, road::Kind::Envoy, Culture::Lampwick, false);
        calm(&mut run, 20);
        close(&dir, &run, size, "envoy-lampwick-a");
        save(&dir, &run, "envoy-lampwick-b-seen");
        speak(&mut run, 1);
        save(&dir, &run, "envoy-lampwick-c-ask");
        let mut away = run.clone();
        key(&mut run, 1, 1);
        calm(&mut run, 14);
        close(&dir, &run, size, "envoy-lampwick-d-taken");
        key(&mut away, 1, 3);
        for at in [10, 40, 60] {
            calm(&mut away, at);
            close(&dir, &away, size, &format!("envoy-lampwick-e-away-{at:03}"));
        }
    }
    if wanted("peoples") {
        for culture in super::cultures::WHEEL {
            let tag = format!("{culture:?}").to_lowercase();
            for (kind, k) in [(road::Kind::Patrol, "patrol"), (road::Kind::Envoy, "envoy")] {
                let mut run = scene(1, 13, kind, culture, false);
                calm(&mut run, 20);
                close(&dir, &run, size, &format!("peoples-{tag}-{k}"));
            }
        }
    }
    if wanted("boards") {
        for kind in road::Kind::ALL {
            let mut run = scene(1, 13, kind, Culture::Rootkin, false);
            if kind == road::Kind::Captives {
                meeting_mut(&mut run).stage = road::Stage::Open;
            }
            calm(&mut run, 4);
            beside(&mut run, 1);
            press(&mut run, 1);
            save(&dir, &run, &format!("boards-{kind:?}-a-hello").to_lowercase());
            press(&mut run, 1);
            save(&dir, &run, &format!("boards-{kind:?}-b-ask").to_lowercase());
        }
    }
    if wanted("tracker") {
        let mut run = scene(1, 13, road::Kind::Patrol, Culture::Emberfolk, false);
        ask(&mut run, 13, Errand::Beacons, 5);
        run.deep.as_mut().unwrap().cargo.push(road::Cargo { parcel: road::Parcel::Peace, from: Culture::Capfolk });
        run.deep.as_mut().unwrap().stir = 2;
        let ahead = run.home.deep.culture_at(20);
        run.home.deep.road.shift(ahead, road::FRIEND);
        calm(&mut run, 4);
        save(&dir, &run, "tracker-a-road");
    }
    if wanted("delivery") {
        for (favour, tag) in [(road::FRIEND, "friend"), (0, "plain"), (road::WARY, "wary")] {
            let mut run = road_run(2, 19);
            let ahead = run.home.deep.culture_at(20);
            let envoy = super::cultures::WHEEL[(super::cultures::WHEEL.iter().position(|&c| c == ahead).unwrap() + 2) % 8];
            run.home.deep.road.shift(ahead, favour);
            if favour >= 0 {
                run.deep.as_mut().unwrap().cargo = vec![
                    road::Cargo { parcel: road::Parcel::Gift, from: ahead },
                    road::Cargo { parcel: road::Parcel::Peace, from: envoy },
                ];
            }
            run.descend_for_test();
            calm(&mut run, village::FALL + 6);
            save(&dir, &run, &format!("delivery-{tag}-a"));
            calm(&mut run, village::GATHER + 10);
            let frame = crate::render::overworld::arena::frame_for(&run, 768, 448, Some(1));
            save_img(&dir, &crate::render::overworld::Img::from_view(&frame, 220, 0, 300, 40), &format!("delivery-{tag}-b-hope"));
        }
    }
}
