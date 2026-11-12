//! The Deep: the six floors stay as they were; the Winding Stair's Deep
//! landings take a party below the Unknown; the floors there count on and
//! climb; every tenth is a village of a turning people that lights up as
//! the knights help, asks errands that the floors below then keep, and
//! takes a visitor's wish that the realm remembers.
use super::city::{self, Gift};
use super::cultures::{self, WHEEL};
use super::village::{self, HOPE_FULL, SPEAKERS};
use super::*;
use crate::rules::home::{self, Home, Station};

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

/// A party of `seats` standing in the village on floor `floor`, the star
/// fallen.
fn village_run(seats: u32, floor: u32) -> Run {
    let mut run = Run::new(41, 1, None);
    for id in 2..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.home = deep_home();
    run.deep_floor_for_test(floor);
    for hero in run.players.values_mut() {
        hero.invulnerable = 0;
    }
    calm(&mut run, village::FALL + 2);
    run
}

fn the_village(run: &Run) -> &Village {
    run.deep.as_ref().unwrap().village.as_ref().unwrap()
}

/// Speak with speaker `who` and step through to what they ask.
fn speak(run: &mut Run, id: u32, who: usize) {
    let (x, y) = SPEAKERS[who];
    put(run, id, (x, y + 0.6));
    press(run, id);
    press(run, id);
}

#[test]
fn a_delve_of_six_floors_never_goes_below_the_unknown() {
    // No Deep Road without having stood on the Unknown's floor.
    let mut home = Home::default();
    home.levels.insert(Station::Map, 4);
    home.deepest = DEEPEST - 1;
    assert_eq!(home.landings(), vec![1, 2, 3, 4, 5]);
    let mut run = Run::new(41, 1, None);
    run.dungeon = layout::floor(DEEPEST, Pack::Unknown, &mut Rng::new(5));
    run.boss_gates = None;
    run.descend_for_test();
    assert!(run.deep.is_none(), "the six floors keep their bottom");
    assert_eq!(run.dungeon.depth, DEEPEST);
}

#[test]
fn the_deep_road_opens_below_the_unknown_and_lands_on_floor_seven() {
    let home = deep_home();
    assert_eq!(home.landings(), vec![1, 2, 3, 4, 5, ROAD]);
    let mut home = home;
    home.landing = ROAD;
    let mut run = Run::at_home(4, 1, None, home, Default::default());
    run.descend_for_test();
    assert_eq!(run.floor(), ROAD);
    assert_eq!(run.dungeon.depth, ECHO, "a floor of the Deep plays as the Fungal Deep's");
    assert_eq!(run.deep_floor(), Some(ROAD));
    assert!(run.home.deep.wheel > 0, "the realm's wheel is set on the first way down");
    assert!(run.valid_snapshot());
}

#[test]
fn the_deep_counts_on_turns_the_packs_and_comes_to_a_village() {
    let mut run = Run::new(41, 1, None);
    run.home = deep_home();
    run.deep_floor_for_test(ROAD);
    let mut packs = Vec::new();
    for floor in ROAD..CHAPTER {
        assert_eq!(run.floor(), floor);
        assert!(run.valid_snapshot(), "floor {floor}");
        assert_ne!(run.room().kind, RoomKind::DeepVillage);
        packs.push(run.dungeon.pack);
        run.descend_for_test();
    }
    assert_eq!(run.floor(), CHAPTER);
    assert_eq!(run.room().kind, RoomKind::DeepVillage, "the tenth floor is a village");
    assert!(run.in_deep_village());
    assert!(run.valid_snapshot());
    assert!(packs.windows(2).all(|w| w[0] != w[1]), "the packs turn floor by floor: {packs:?}");
    // And on below the village.
    run.descend_for_test();
    assert_eq!(run.floor(), CHAPTER + 1);
    assert!(run.deep.as_ref().unwrap().village.is_none());
    assert!(run.valid_snapshot());
}

#[test]
fn the_stakes_climb_below_the_unknown() {
    let mut six = Run::new(41, 1, None);
    six.dungeon = layout::floor(DEEPEST, Pack::Unknown, &mut Rng::new(5));
    let mut seven = Run::new(41, 1, None);
    seven.deep_floor_for_test(ROAD);
    let mut seventeen = Run::new(41, 1, None);
    seventeen.deep_floor_for_test(17);
    assert!(seven.toughness() > six.toughness());
    assert!(seventeen.toughness() > seven.toughness());
    assert!(seventeen.boss_health() > six.boss_health());
    assert_eq!(health(DEEPEST), 160, "the Deep starts where the Unknown's table ends");
    assert_eq!(hit(DEEPEST), 240);
}

#[test]
fn the_wheel_turns_and_never_sets_one_people_twice_running() {
    for wheel in 0..12 {
        let peoples: Vec<Culture> = (1..=12).map(|c| cultures::turn(wheel, c)).collect();
        assert!(peoples.windows(2).all(|w| w[0] != w[1]), "wheel {wheel}: {peoples:?}");
        for culture in WHEEL {
            assert!(peoples[..WHEEL.len()].contains(&culture), "every people comes round: {peoples:?}");
        }
    }
}

#[test]
fn the_star_falls_and_the_village_gathers() {
    let mut run = Run::new(41, 1, None);
    run.home = deep_home();
    run.deep_floor_for_test(CHAPTER);
    let (x, y) = village::units(village::LANDING);
    let before = (run.players[&1].x, run.players[&1].y);
    assert!((before.0 - x).abs() < 4.0 && (before.1 - y).abs() < 1.0, "the party lands in the plaza");
    // While the star falls nobody moves.
    run.step(&BTreeMap::from([(1, Input { move_x: 1, ..Default::default() })]));
    assert_eq!((run.players[&1].x, run.players[&1].y), before);
    assert_eq!(the_village(&run).hope, 0, "the village is dark before the star lands");
    calm(&mut run, village::FALL + 1);
    assert_eq!(the_village(&run).hope, 1, "the star's landing is its first light");
    assert!(run.light.is_some(), "the light home waits by the lift");
}

#[test]
fn villagers_speak_and_the_village_lights_up_as_the_knights_help() {
    let mut run = village_run(1, CHAPTER);
    let culture = the_village(&run).culture;
    speak(&mut run, 1, 0);
    let (name, words, answers) = run.talk_view(1).expect("the elder speaks");
    assert_eq!(name, culture.folk()[0].name);
    assert!(!words.is_empty() && answers.len() == 3, "{answers:?}");
    let hope = the_village(&run).hope;
    key(&mut run, 1, 1);
    assert_eq!(run.home.deep.missions.len(), 1, "the elder's errand is taken");
    assert!(the_village(&run).hope > hope);
    assert!(run.talk_view(1).is_none(), "taking it ends the talk");
    // Every errand, and a word with the one who only talks.
    for who in 1..=4 {
        speak(&mut run, 1, who);
        key(&mut run, 1, 1);
    }
    assert_eq!(run.home.deep.missions.len(), 4);
    assert!(run.home.deep.missions.iter().all(|m| m.village == CHAPTER && m.culture == culture));
    // A visitor's wish brings it to the full.
    speak(&mut run, 1, cultures::WISHKEEPER);
    key(&mut run, 1, 1);
    assert!(run.city_wishing(1));
    run.offer_wish(1, "a fountain of clean water for the children", None).unwrap();
    key(&mut run, 1, 1);
    assert_eq!(the_village(&run).hope, HOPE_FULL, "the village shines");
    let town = run.home.deep.town(CHAPTER).unwrap();
    assert!(town.lit, "the realm keeps it lit");
    assert_eq!(town.wishes.len(), 1);
    assert_eq!(town.wishes[0].gift, Gift::Fountain);
    assert!(run.home.landings().contains(&CHAPTER), "a lit village is a landing");
    assert!(run.valid_snapshot());
}

#[test]
fn a_visitors_wish_spends_nothing_and_stands_once_a_knight() {
    let mut run = village_run(2, CHAPTER);
    let wishes = run.players[&1].wishes;
    speak(&mut run, 1, cultures::WISHKEEPER);
    key(&mut run, 1, 1);
    run.offer_wish(1, "a statue of me, so they remember my name", None).unwrap();
    let offer = run.players[&1].offer.clone().unwrap();
    assert_eq!(offer.choices.len(), 3);
    assert_eq!(offer.choices[0].head, Gift::Statue.name());
    key(&mut run, 1, 1);
    assert_eq!(run.players[&1].wishes, wishes, "none of the knight's own is spent");
    assert!(run.players[&1].offer.is_none());
    // Once a knight; a friend has their own.
    speak(&mut run, 1, cultures::WISHKEEPER);
    assert!(run.talk_view(1).unwrap().2[0].contains("ALREADY"));
    key(&mut run, 1, 1);
    assert!(!run.city_wishing(1));
    speak(&mut run, 2, cultures::WISHKEEPER);
    key(&mut run, 2, 1);
    run.offer_wish(2, "books for the little ones", None).unwrap();
    key(&mut run, 2, 1);
    let town = run.home.deep.town(CHAPTER).unwrap();
    assert_eq!(town.wishes.iter().map(|w| (w.gift, w.slot)).collect::<Vec<_>>(), vec![(Gift::Statue, 0), (Gift::Library, 1)]);
    // Rolled up, a scroll builds nothing.
    assert!(city::read("").len() == 3);
}

#[test]
fn the_errands_lay_their_signs_on_the_floors_below_and_pay() {
    let mut run = village_run(1, CHAPTER);
    for who in 0..=3 {
        speak(&mut run, 1, who);
        key(&mut run, 1, 1);
    }
    run.descend_for_test();
    assert_eq!(run.floor(), CHAPTER + 1);
    let signs = run.deep.as_ref().unwrap().signs.clone();
    let kinds: Vec<_> = signs.iter().map(|s| s.kind).collect();
    assert!(kinds.contains(&missions::SignKind::Lamp), "a beacon on the first floor: {kinds:?}");
    assert!(kinds.contains(&missions::SignKind::Relic), "a relic on every floor: {kinds:?}");
    for sign in &signs {
        let room = &run.dungeon.rooms[sign.room];
        assert_eq!(room.tile(i32::from(sign.col), i32::from(sign.row)), Tile::Floor);
    }
    // Walk over the relic: it is gathered.
    let relic = signs.iter().find(|s| s.kind == missions::SignKind::Relic).unwrap().clone();
    run.enter_for_test(relic.room);
    run.enemies.clear();
    let (x, y) = relic.point();
    put(&mut run, 1, (x / TILE_UNITS, y / TILE_UNITS));
    calm(&mut run, 2);
    let gather = &run.home.deep.missions[usize::from(relic.mission)];
    assert_eq!(gather.have, 1);
    assert!(run.deep.as_ref().unwrap().signs.iter().any(|s| s.done));
    // Finish it: it pays, and its village remembers.
    let gold = run.players[&1].carried.get(crate::realm::Spoil::Gold);
    run.home.deep.missions[usize::from(relic.mission)].have = 5;
    let next = run.deep.as_ref().unwrap().signs.iter().position(|s| !s.done && s.kind == missions::SignKind::Lamp).unwrap();
    let lamp = run.deep.as_ref().unwrap().signs[next].clone();
    run.deep.as_mut().unwrap().signs[next].kind = missions::SignKind::Relic;
    run.deep.as_mut().unwrap().signs[next].mission = relic.mission;
    run.enter_for_test(lamp.room);
    run.enemies.clear();
    let (x, y) = lamp.point();
    put(&mut run, 1, (x / TILE_UNITS, y / TILE_UNITS));
    calm(&mut run, 2);
    assert!(run.home.deep.missions[usize::from(relic.mission)].paid);
    assert!(run.players[&1].carried.get(crate::realm::Spoil::Gold) > gold, "the errand pays");
    assert!(run.home.deep.town(CHAPTER).unwrap().deeds.contains(&Errand::Gather));
}

#[test]
fn a_lamp_is_lit_with_e_and_a_hunt_counts_its_kind() {
    let mut run = village_run(1, CHAPTER);
    for who in 0..=3 {
        speak(&mut run, 1, who);
        key(&mut run, 1, 1);
    }
    run.descend_for_test();
    let signs = run.deep.as_ref().unwrap().signs.clone();
    let lamp = signs.iter().find(|s| s.kind == missions::SignKind::Lamp).unwrap().clone();
    run.enter_for_test(lamp.room);
    run.enemies.clear();
    let (x, y) = lamp.point();
    put(&mut run, 1, (x / TILE_UNITS, y / TILE_UNITS));
    calm(&mut run, 3);
    assert_eq!(run.home.deep.missions[usize::from(lamp.mission)].have, 0, "walking past lights nothing");
    press(&mut run, 1);
    assert_eq!(run.home.deep.missions[usize::from(lamp.mission)].have, 1, "E lights it");
    let hunt = run.home.deep.missions.iter().position(|m| matches!(m.errand, Errand::Slay(_))).unwrap();
    let Errand::Slay(kind) = run.home.deep.missions[hunt].errand else { unreachable!() };
    run.mark_kill(kind);
    run.mark_kill(kind);
    assert_eq!(run.home.deep.missions[hunt].have, 2);
}

#[test]
fn the_deep_record_is_settled_into_the_realm_and_kept_across_a_delivery() {
    let mut run = village_run(1, CHAPTER);
    speak(&mut run, 1, 0);
    key(&mut run, 1, 1);
    let transfer = crate::rules::home_settlement::HomeTransfer::capture_and_reserve(&mut run).unwrap();
    let mut home = Home::default();
    let mut treasury = crate::realm::Spoils::default();
    let reclaimed = BTreeMap::new();
    let result = crate::rules::home_settlement::settle(
        &mut crate::rules::activity::Realm { home: &mut home, treasury: &mut treasury, reclaimed: &reclaimed, raids_won: 0 },
        &transfer,
        "host",
        false,
    )
    .unwrap();
    assert!(result.changed);
    assert_eq!(home.deep, run.home.deep, "the realm keeps the Deep's record");
    // A save written with no Deep in it reads; one with it reads it back.
    let json = serde_json::to_string(&home).unwrap();
    let back: Home = serde_json::from_str(&json).unwrap();
    assert_eq!(back.deep, home.deep);
    assert!(!serde_json::to_string(&Home::default()).unwrap().contains("\"deep\""), "an empty record is left out");
}

/// A village of `culture` on floor `floor`, the realm meeting it there.
fn village_of(culture: Culture, floor: u32, seats: u32) -> Run {
    let mut run = Run::new(41, 1, None);
    for id in 2..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.home = deep_home();
    run.home.deep.towns.push(Town::new(floor, culture));
    run.deep_floor_for_test(floor);
    for hero in run.players.values_mut() {
        hero.invulnerable = 0;
    }
    run
}

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

fn save(dir: &std::path::Path, run: &Run, name: &str) {
    let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
    save_img(dir, &img, name);
}

#[test]
fn eight_peoples_turn_on_the_wheel_each_whole() {
    assert_eq!(WHEEL.len(), 8);
    for culture in WHEEL {
        let folk = culture.folk();
        let errands: Vec<Errand> = folk.iter().filter_map(|f| f.asks.as_ref().map(|a| a.errand)).collect();
        assert_eq!(errands.len(), 4, "{culture:?}: four errands");
        for kind in [Errand::Beacons, Errand::Gather, Errand::Rescue] {
            assert!(errands.contains(&kind), "{culture:?} asks {kind:?}");
        }
        assert!(errands.iter().any(|e| matches!(e, Errand::Slay(_))), "{culture:?} asks a hunt");
        assert!(folk[cultures::TALKER].asks.is_none() && folk[cultures::WISHKEEPER].asks.is_none());
        assert!(!culture.music().is_empty());
        assert_ne!(culture.kin(), Pack::Swamp, "the Deep never goes to the swamp");
        // Every speaker's words fit the conversation board.
        for f in &folk {
            assert!(f.name.len() <= 20 && f.hello.len() <= 200 && f.after.len() <= 200, "{}", f.name);
        }
        let (id, _) = culture.dread();
        assert_eq!(dread::culture_of(id), Some(culture));
    }
    // The first four keep their places on the wheel.
    assert_eq!(&WHEEL[..4], &[Culture::Capfolk, Culture::Glimmerkin, Culture::Emberfolk, Culture::Lampwick]);
}

#[test]
fn the_floor_above_a_village_keeps_its_peoples_dread_at_the_stair() {
    for floor in [CHAPTER - 1, 2 * CHAPTER - 1] {
        let mut run = Run::new(41, 1, None);
        run.home = deep_home();
        run.deep_floor_for_test(floor);
        let culture = run.home.deep.culture_at(floor + 1);
        let i = run.dread_at().expect("the floor above a village keeps a dread");
        assert_eq!(run.bosses[i].id, culture.dread().0);
        assert_eq!(run.bosses[i].only_in, Pack::Swamp, "kept out of the Deep's own packs");
        if let Some(g) = &run.boss_gates {
            assert_eq!(g.guardian_boss, Some(i as u8), "the dread holds the stair");
        }
        assert!(run.valid_snapshot(), "floor {floor}");
        // In its hall, it is the guardian.
        let hall = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).unwrap();
        run.enter(hall, Some(2));
        let guardians: Vec<_> = run.enemies.iter().filter_map(|e| e.boss).collect();
        if !guardians.is_empty() {
            assert!(guardians.contains(&(i as u8)), "floor {floor}: {guardians:?}");
        }
        assert!(run.valid_snapshot(), "floor {floor} in the hall");
    }
    // A floor that is not above a village keeps none, and the six floors
    // never meet one.
    let mut run = Run::new(41, 1, None);
    run.home = deep_home();
    run.deep_floor_for_test(CHAPTER + 3);
    assert!(run.dread_at().is_none());
    assert!(!run.bosses.iter().any(dread::is));
    let six = Run::new(41, 1, None);
    assert!(!six.bosses.iter().any(dread::is));
}

#[test]
fn a_dread_grows_with_its_chapter_inside_the_checkers_bounds() {
    for culture in WHEEL {
        let first = dread::boss(culture, 1);
        let third = dread::boss(culture, 3);
        let ninth = dread::boss(culture, 9);
        assert_eq!(first.windups.len(), 1, "{culture:?}: its signature alone, the first time");
        assert_eq!(third.windups.len(), 2, "{culture:?}: a second charge-up joins it later");
        assert!(third.hp > first.hp && ninth.hp >= third.hp && ninth.hp <= 900);
        assert!(third.windups[0].every < first.windups[0].every);
        assert!(third.windups[0].damage >= first.windups[0].damage);
        assert_eq!(third.windups[0].from, crate::rules::bosses::Stage::Open, "from the third chapter it opens with it");
        for b in [&first, &third, &ninth] {
            for up in &b.windups {
                assert!(up.wind >= crate::rules::bosses::WIND_LEAST && (120..=600).contains(&up.every) && (8..=20).contains(&up.damage), "{}: {up:?}", b.id);
            }
            assert!(b.windups.len() <= crate::rules::bosses::WINDUPS_MOST);
        }
    }
}

#[test]
fn a_felled_dread_is_heard_in_the_village_below() {
    let mut run = Run::new(41, 1, None);
    run.home = deep_home();
    run.deep_floor_for_test(CHAPTER - 1);
    for hero in run.players.values_mut() {
        hero.invulnerable = 100_000;
    }
    let hall = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).unwrap();
    run.enter_for_test(hall);
    let i = run.dread_at().unwrap();
    let e = run.enemies.iter().position(|e| e.boss == Some(i as u8)).expect("the dread stands in its hall");
    run.enemies[e].hp = 0;
    run.step(&BTreeMap::new());
    let town = run.home.deep.town(CHAPTER).expect("the village below hears it");
    assert!(town.dread);
    assert!(run.cues.iter().any(|c| c == "deep_dread_felled") || run.found_line().is_some_and(|l| l.contains("FALLS")));
    // The star lands on a village a step brighter.
    let mut next = Run::new(41, 1, None);
    next.home = run.home.clone();
    next.deep_floor_for_test(CHAPTER);
    calm(&mut next, village::FALL + 2);
    assert_eq!(the_village(&next).hope, 2, "its first light, and the dread's fall");
    // And down the stair from the hall, as the party would go.
    calm(&mut run, 5);
    run.descend_for_test();
    assert_eq!(run.floor(), CHAPTER);
    calm(&mut run, village::FALL + 2);
    assert_eq!(the_village(&run).hope, 2, "down the stair from its fall");
}

#[test]
fn a_knight_with_no_wishes_left_can_still_wish_for_the_city() {
    let mut run = village_run(2, CHAPTER);
    for hero in run.players.values_mut() {
        (hero.wishes, hero.great_wishes) = (0, 0);
    }
    assert!(!run.can_wish(2));
    assert!(!run.wish_line_opens(2));
    speak(&mut run, 2, cultures::WISHKEEPER);
    key(&mut run, 2, 1);
    assert!(run.city_wishing(2));
    assert!(run.wish_line_opens(2), "a friend's cockpit opens the wish line for the city");
    assert!(run.special_wish_note(2).is_some_and(|n| n.contains("city")), "and says what it is for");
    run.offer_wish(2, "a bell for the tower", None).unwrap();
    key(&mut run, 2, 1);
    let town = run.home.deep.town(CHAPTER).unwrap();
    assert_eq!(town.wishes.len(), 1);
    assert_eq!(run.players[&2].wishes, 0, "none of their own spent, none given");
    assert!(!run.wish_line_opens(2), "and the line closes again after");
}

#[test]
fn the_one_sent_home_runs_to_whoever_asked_the_next_time() {
    let culture = Culture::Roostfolk;
    let giver = 1u8;
    let mut run = Run::new(41, 1, None);
    run.home = deep_home();
    run.home.deep.towns.push(Town { deeds: vec![Errand::Rescue], ..Town::new(CHAPTER, culture) });
    run.home.deep.missions.push(Mission { errand: Errand::Rescue, village: CHAPTER, culture, giver, have: 1, need: 1, paid: true });
    run.deep_floor_for_test(CHAPTER);
    let start = the_village(&run).reunion.expect("the reunion is due on the first visit back");
    assert!(run.home.deep.town(CHAPTER).unwrap().reunited);
    let wait = start + village::RUN_HOME - run.tick + 1;
    calm(&mut run, wait);
    assert!(run.found_line().is_some_and(|l| l.contains(culture.rescued().0)), "{:?}", run.found_line());
    // The one who asked says so.
    calm(&mut run, village::REUNION);
    speak(&mut run, 1, usize::from(giver));
    let (_, words, _) = run.talk_view(1).unwrap();
    assert_eq!(words, culture.rescued().1);
    assert!(run.valid_snapshot());
    // The next time, they are simply home.
    run.deep_floor_for_test(CHAPTER);
    assert!(the_village(&run).reunion.is_none());
}

#[test]
fn leaving_a_village_reads_as_leaving_it() {
    let mut run = village_run(1, CHAPTER);
    for who in 0..=1 {
        speak(&mut run, 1, who);
        key(&mut run, 1, 1);
    }
    run.descend_for_test();
    let results = run.floor_results.clone().expect("the village's board");
    assert_eq!(results.depth, CHAPTER);
    let left = results.village.expect("it says the village was left");
    assert_eq!(left.town, run.home.deep.culture_at(CHAPTER).town(1));
    assert_eq!(left.errands, 2);
    assert!(left.hope >= 3);
    // A floor below is cleared as ever.
    run.descend_for_test();
    assert!(run.floor_results.as_ref().unwrap().village.is_none());
    // And a board with no village on it saves as before.
    let plain = serde_json::to_string(run.floor_results.as_ref().unwrap()).unwrap();
    assert!(!plain.contains("village"));
}

/// `cargo test print_dread_art -- --ignored --nocapture`: each dread's
/// figure as `.boss` art rows, for its file (the trophy room draws them).
#[test]
#[ignore]
fn print_dread_art() {
    for culture in WHEEL {
        println!("== {}", culture.dread().0);
        for row in crate::render::overworld::arena::dread_art_for_test(culture) {
            println!("{row}");
        }
    }
}

/// Whether the shots asked for (`DEEP_SHOTS`, a comma list; all if unset)
/// include `part`.
fn wanted(part: &str) -> bool {
    std::env::var("DEEP_SHOTS").map_or(true, |list| list.split(',').any(|p| p.trim() == part))
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_deep_village_shots -- --ignored`:
/// each people's village dim as the party lands, then lit; the whole
/// cavern of each; the star's fall as a run of frames; a conversation; the
/// tracker on a floor below; a city with six wishes standing. `DEEP_SHOTS`
/// (a comma list: villages, home, fall, fall2, talk, tracker, cities, comets,
/// talks, dreads, rescue, reunion, leaving) draws only those.
#[test]
#[ignore]
fn write_deep_village_shots() {
    let Some(dir) = shot_dir("deep-villages") else { return };
    for culture in WHEEL.into_iter().filter(|_| wanted("villages")) {
        let tag = format!("{culture:?}").to_lowercase();
        let mut run = village_of(culture, CHAPTER, 3);
        calm(&mut run, village::FALL + village::GATHER + 20);
        put(&mut run, 1, (18.0, 12.4));
        put(&mut run, 2, (15.5, 13.2));
        put(&mut run, 3, (20.8, 12.8));
        calm(&mut run, 2);
        save(&dir, &run, &format!("{tag}-a-dim"));
        save_img(&dir, &crate::render::overworld::arena::whole_room_for_test(&run), &format!("{tag}-c-whole-dim"));
        let mut deep = run.deep.take().unwrap();
        deep.village.as_mut().unwrap().hope = HOPE_FULL;
        run.deep = Some(deep);
        calm(&mut run, 7);
        save(&dir, &run, &format!("{tag}-b-lit"));
        save_img(&dir, &crate::render::overworld::arena::whole_room_for_test(&run), &format!("{tag}-d-whole-lit"));
    }
    // The way in: the Winding Stair's landing on the Deep Road, and on a
    // village lit before.
    for (landing, name) in [(ROAD, "home-a-stair-deep-road"), (CHAPTER, "home-b-stair-village")].into_iter().filter(|_| wanted("home")) {
        let mut home = deep_home();
        home.landing = landing;
        home.deep.towns.push(Town { lit: true, ..Town::new(CHAPTER, Culture::Capfolk) });
        let mut run = Run::at_home(4, 1, None, home, Default::default());
        let (c, r, w, _) = home::STAIR_MOUTH;
        put(&mut run, 1, (c as f32 + w as f32 / 2.0, r as f32 + 1.6));
        calm(&mut run, 3);
        save(&dir, &run, name);
    }
    // The star's fall, a frame every other tick.
    if wanted("fall") {
        let Some(fall) = shot_dir("deep-villages/fall") else { return };
        let mut run = village_of(Culture::Capfolk, CHAPTER, 2);
        let frames = village::FALL + village::GATHER;
        for k in 0..frames {
            if k % 2 == 0 {
                save(&fall, &run, &format!("fall-{k:03}"));
            }
            run.step(&BTreeMap::new());
        }
    }
    // The star's fall into a roost, a frame every third tick.
    if wanted("fall2") {
        let Some(fall) = shot_dir("deep-villages/fall2") else { return };
        let mut run = village_of(Culture::Roostfolk, CHAPTER, 2);
        for k in 0..village::FALL + village::GATHER {
            if k % 3 == 0 {
                save(&fall, &run, &format!("fall-{k:03}"));
            }
            run.step(&BTreeMap::new());
        }
    }
    more_shots(&dir);
    // A conversation with the elder, on to what they ask.
    if wanted("talk") {
        let mut run = village_of(Culture::Glimmerkin, CHAPTER, 1);
        calm(&mut run, village::FALL + village::GATHER + 4);
        speak(&mut run, 1, 0);
        save(&dir, &run, "talk-elder-asks");
        key(&mut run, 1, 1);
        speak(&mut run, 1, cultures::WISHKEEPER);
        save(&dir, &run, "talk-wishkeeper");
    }
    if wanted("tracker") {
        tracker_shots(&dir);
    }
    city_shots(&dir);
}

/// The tracker, a floor below, by a lamp and a relic.
fn tracker_shots(dir: &std::path::Path) {
    let dir = dir;
    let mut run = village_of(Culture::Emberfolk, CHAPTER, 1);
    calm(&mut run, village::FALL + 2);
    for who in 0..=3 {
        speak(&mut run, 1, who);
        key(&mut run, 1, 1);
    }
    run.descend_for_test();
    let signs = run.deep.as_ref().unwrap().signs.clone();
    if let Some(lamp) = signs.iter().find(|s| s.kind == missions::SignKind::Lamp) {
        run.enter_for_test(lamp.room);
        run.enemies.clear();
        run.players.get_mut(&1).unwrap().invulnerable = 0;
        let (x, y) = lamp.point();
        put(&mut run, 1, (x / TILE_UNITS + 1.2, y / TILE_UNITS + 0.4));
        calm(&mut run, 3);
        save(&dir, &run, "tracker-a-lamp-cold");
        press(&mut run, 1);
        calm(&mut run, 8);
        save(&dir, &run, "tracker-b-lamp-lit");
    }
    if let Some(relic) = signs.iter().find(|s| s.kind == missions::SignKind::Relic) {
        run.enter_for_test(relic.room);
        run.enemies.clear();
        run.players.get_mut(&1).unwrap().invulnerable = 0;
        let (x, y) = relic.point();
        put(&mut run, 1, (x / TILE_UNITS - 2.0, y / TILE_UNITS));
        calm(&mut run, 3);
        save(&dir, &run, "tracker-c-relic");
    }
}

/// A city with six wishes standing, one rising.
fn city_shots(dir: &std::path::Path) {
    let dir = dir;
    for culture in WHEEL.into_iter().filter(|_| wanted("cities")) {
        let tag = format!("{culture:?}").to_lowercase();
        let mut run = village_of(culture, CHAPTER, 2);
        calm(&mut run, village::FALL + village::GATHER + 4);
        let gifts = [Gift::Statue, Gift::Lanterns, Gift::Fountain, Gift::Garden, Gift::Bell, Gift::Hearth];
        let words = ["a statue of me", "lanterns for every street", "clean water", "a garden that glows", "music on feast days", "warm hearths"];
        let town = run.home.deep.town_mut(CHAPTER).unwrap();
        for (slot, (gift, words)) in gifts.into_iter().zip(words).enumerate().skip(1) {
            town.wishes.push(city::CityWish { knight: format!("visitor-{slot}"), shown: format!("Sir Visitor {slot}"), words: words.into(), gift, slot: slot as u8 });
        }
        town.deeds = vec![Errand::Beacons, Errand::Gather, Errand::Rescue];
        town.lit = true;
        let mut deep = run.deep.take().unwrap();
        deep.village.as_mut().unwrap().hope = HOPE_FULL;
        run.deep = Some(deep);
        speak(&mut run, 1, cultures::WISHKEEPER);
        key(&mut run, 1, 1);
        run.offer_wish(1, "a statue of me, so they remember", None).unwrap();
        key(&mut run, 1, 1);
        put(&mut run, 1, (15.6, 13.4));
        put(&mut run, 2, (24.6, 14.6));
        calm(&mut run, 24);
        if culture == Culture::Capfolk {
            save(&dir, &run, "capfolk-e0-city-wish-rising");
        }
        calm(&mut run, 60);
        save(&dir, &run, &format!("{tag}-e-city-wishes"));
        save_img(&dir, &crate::render::overworld::arena::whole_room_for_test(&run), &format!("{tag}-f-whole-city"));
    }
}

#[test]
fn the_light_home_from_a_village_takes_the_party_up_with_everything() {
    let mut run = village_run(1, CHAPTER);
    let (x, y) = run.light.expect("the light home waits by the lift");
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y) = (x, y);
    calm(&mut run, 3);
    assert_eq!(run.phase, Phase::Won);
    assert!(run.valid_snapshot());
}

#[test]
fn a_run_in_the_deep_saves_and_reads_back_whole() {
    let mut run = village_run(2, CHAPTER);
    speak(&mut run, 1, 0);
    let json = serde_json::to_string(&run).unwrap();
    let back: Run = serde_json::from_str(&json).unwrap();
    assert!(back.valid_snapshot());
    assert_eq!(back.deep, run.deep);
    assert_eq!(back.home.deep, run.home.deep);
    // A run that never went down saves no Deep at all.
    let plain = serde_json::to_string(&Run::new(41, 1, None)).unwrap();
    assert!(!plain.contains("\"deep\""));
}

#[test]
fn an_open_floor_of_the_deep_lays_its_signs_too() {
    let mut run = village_run(1, CHAPTER);
    for who in 0..=3 {
        speak(&mut run, 1, who);
        key(&mut run, 1, 1);
    }
    run.open_floors = true;
    run.descend_for_test();
    assert!(run.dungeon.open.is_some(), "the floor below is laid open");
    let signs = &run.deep.as_ref().unwrap().signs;
    assert!(!signs.is_empty(), "the errands' signs stand on its map");
    let map = run.dungeon.open.as_ref().unwrap().map;
    assert!(signs.iter().all(|s| s.room == map && run.dungeon.rooms[map].tile(i32::from(s.col), i32::from(s.row)) == Tile::Floor));
    assert!(run.valid_snapshot());
}

#[test]
fn a_village_is_drawn_whole_and_never_stood_up_in_3d() {
    use crate::world3d::room::{Mode, stands_in_mode};
    assert!(!stands_in_mode(Mode::Deep, ECHO, RoomKind::DeepVillage));
    assert!(stands_in_mode(Mode::Deep, ECHO, RoomKind::Fight), "the Deep's other floors stand up as the deep ones above do");
}

/// The second night's shots (`write_deep_village_shots`): each people's
/// star coming down and a word with their elder; each dread winding up
/// and striking, a frame every few ticks; one of the lost below, found and
/// going home; their reunion in the village; and the board on leaving.
fn more_shots(dir: &std::path::Path) {
    if wanted("comets") {
        for culture in WHEEL {
            let tag = format!("{culture:?}").to_lowercase();
            let mut run = village_of(culture, CHAPTER, 3);
            calm(&mut run, village::FALL - 12);
            save(dir, &run, &format!("{tag}-g-comet-falling"));
            calm(&mut run, 20);
            save(dir, &run, &format!("{tag}-h-comet-landing"));
            calm(&mut run, 50);
            save(dir, &run, &format!("{tag}-i-gathering"));
        }
    }
    if wanted("talks") {
        for culture in WHEEL {
            let tag = format!("{culture:?}").to_lowercase();
            let mut run = village_of(culture, CHAPTER, 1);
            calm(&mut run, village::FALL + village::GATHER + 4);
            speak(&mut run, 1, 0);
            save(dir, &run, &format!("talk-{tag}-elder"));
            key(&mut run, 1, 1);
            speak(&mut run, 1, cultures::WISHKEEPER);
            save(dir, &run, &format!("talk-{tag}-wishkeeper"));
        }
    }
    if wanted("dreads") {
        dread_shots(dir);
    }
    if wanted("windups") {
        dread_windup_shots();
    }
    if wanted("rescue") {
        rescue_shots(dir);
    }
    if wanted("reunion") {
        reunion_shots(dir);
    }
    if wanted("leaving") {
        leaving_shots(dir);
    }
}

/// A party on the floor above chapter `chapter`'s village of `culture`,
/// in the guardian's hall with its dread.
fn dread_fight(culture: Culture, chapter: u32) -> Run {
    let mut run = Run::new(41, 1, None);
    run.join_seat(2, "Seat 2");
    run.home = deep_home();
    let village = chapter * CHAPTER;
    run.home.deep.towns.push(Town::new(village, culture));
    run.deep_floor_for_test(village - 1);
    for hero in run.players.values_mut() {
        hero.invulnerable = 100_000;
    }
    // Past the floor's plate and its dare.
    calm(&mut run, 30);
    run.skip_floor_plate(1);
    run.skip_floor_plate(2);
    run.dare = None;
    let hall = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).expect("a stairs hall");
    run.enter_for_test(hall);
    run
}

/// The whole room round enemy `i`, close: a 224 by 144 window on it.
fn close_on(run: &Run, i: usize) -> crate::render::overworld::Img {
    close_at(run, (run.enemies[i].x, run.enemies[i].y), (224, 144))
}

/// The whole room round arena point `(x, y)`, a `w` by `h` window on it.
fn close_at(run: &Run, (x, y): (f32, f32), (w, h): (i32, i32)) -> crate::render::overworld::Img {
    let whole = crate::render::overworld::arena::whole_room_for_test(run);
    let (x, y) = ((x * 8.0) as i32, (y * 8.0) as i32);
    let (ox, oy) = ((x - w / 2).clamp(0, (whole.w - w).max(0)), (y - h / 2 + 10).clamp(0, (whole.h - h).max(0)));
    crate::render::overworld::Img::from_view(&whole, ox, oy, w, h)
}

/// The dread in `run`, if it stands.
fn the_dread(run: &Run) -> Option<usize> {
    run.enemies.iter().position(|e| e.boss.is_some_and(|b| dread::is(&run.bosses[usize::from(b)])))
}

/// Each dread winding up its signature and striking with it, a frame
/// every three ticks, the knights stood off from it.
fn dread_shots(dir: &std::path::Path) {
    let Some(sub) = shot_dir("deep-villages/dreads") else { return };
    for culture in WHEEL {
        let tag = format!("{culture:?}").to_lowercase();
        let mut run = dread_fight(culture, 1);
        let Some(i) = the_dread(&run) else { continue };
        // Down from the wall into the middle of its hall, to be seen whole.
        let (bx, by) = (run.room().width() / 2.0, run.room().height() / 2.0 - 2.0);
        let e = &mut run.enemies[i];
        (e.x, e.y, e.origin_x, e.origin_y) = (bx, by, bx, by);
        for (id, (dx, dy)) in [(1u32, (-7.0f32, 4.0f32)), (2, (8.0, 5.0))] {
            let h = run.players.get_mut(&id).unwrap();
            (h.x, h.y) = (bx + dx, by + dy);
            h.invulnerable = 100_000;
        }
        run.projectiles.clear();
        calm(&mut run, 20);
        if let Some(i) = the_dread(&run) {
            let e = &mut run.enemies[i];
            (e.x, e.y) = (bx, by);
            for (id, (dx, dy)) in [(1u32, (-7.0f32, 4.0f32)), (2, (8.0, 5.0))] {
                let h = run.players.get_mut(&id).unwrap();
                (h.x, h.y) = (bx + dx, by + dy);
            }
        }
        run.projectiles.clear();
        save(dir, &run, &format!("dread-{tag}-a-stands"));
        let Some(i) = the_dread(&run) else { continue };
        save_img(dir, &close_on(&run, i), &format!("dread-{tag}-b-close"));
        let e = &mut run.enemies[i];
        e.hp = e.max_hp * 6 / 10;
        e.stage = 0;
        e.age = 1_000;
        let boss = run.bosses[usize::from(e.boss.unwrap())].clone();
        e.timer = boss.windups[0].every;
        let mut frames = 0;
        for k in 0..150u32 {
            run.step(&BTreeMap::new());
            for id in [1u32, 2] {
                if let Some(h) = run.players.get_mut(&id) {
                    h.invulnerable = 100_000;
                    h.hp = h.max_hp;
                }
            }
            let Some(i) = the_dread(&run) else { break };
            let stage = run.enemies[i].stage;
            if stage != 0 && k % 3 == 0 {
                save_img(&sub, &close_on(&run, i), &format!("{tag}-{frames:03}"));
                frames += 1;
            }
            if frames > 0 && stage == 0 {
                break;
            }
        }
    }
}

/// Each dread's two charge-ups at its third chapter, one after the other,
/// a PNG a tick from the wind-up to standing again, the knights stood off:
/// `deep-villages/windups/<people>-<charge-up>/`.
fn dread_windup_shots() {
    let Some(sub) = shot_dir("deep-villages/windups") else { return };
    for culture in WHEEL {
        let tag = format!("{culture:?}").to_lowercase();
        for which in 0..2u32 {
            let mut run = dread_fight(culture, 3);
            let Some(i) = the_dread(&run) else { continue };
            let (bx, by) = (run.room().width() / 2.0, run.room().height() / 2.0 - 2.0);
            let place = |run: &mut Run| {
                for (id, (dx, dy)) in [(1u32, (-9.0f32, 4.0f32)), (2, (9.0, 5.0))] {
                    let h = run.players.get_mut(&id).unwrap();
                    (h.x, h.y) = (bx + dx, by + dy);
                    h.invulnerable = 100_000;
                    h.hp = h.max_hp;
                }
            };
            let boss = run.bosses[usize::from(run.enemies[i].boss.unwrap())].clone();
            let Some(up) = boss.windups.get(which as usize) else { continue };
            let e = &mut run.enemies[i];
            (e.x, e.y, e.origin_x, e.origin_y) = (bx, by, bx, by);
            e.hp = e.max_hp * 6 / 10;
            (e.stage, e.age, e.signature.cycle, e.timer) = (0, 1_000, which, up.every);
            let name = format!("{tag}-{}", up.kind.name());
            let dir = sub.join(&name);
            std::fs::create_dir_all(&dir).unwrap();
            run.projectiles.clear();
            let mut frames = 0;
            let mut started = false;
            for _ in 0..170u32 {
                place(&mut run);
                run.step(&BTreeMap::new());
                let Some(i) = the_dread(&run) else { break };
                let stage = run.enemies[i].stage;
                started |= stage != 0;
                if started {
                    let img = close_on(&run, i);
                    let rgba = img.rgba_scaled(3);
                    image::RgbaImage::from_raw(img.w as u32 * 3, img.h as u32 * 3, rgba).unwrap().save(dir.join(format!("{frames:03}.png"))).unwrap();
                    frames += 1;
                }
                if started && stage == 0 {
                    break;
                }
            }
        }
    }
}

/// One of the lost below: huddled with nobody near, up and waving as a
/// knight comes, found, glad and going home.
fn rescue_shots(dir: &std::path::Path) {
    for culture in WHEEL {
        let tag = format!("{culture:?}").to_lowercase();
        let mut run = village_of(culture, CHAPTER, 1);
        calm(&mut run, village::FALL + 2);
        let giver = (1..=4).find(|&w| matches!(culture.folk()[w].asks.as_ref().map(|a| a.errand), Some(Errand::Rescue))).unwrap();
        speak(&mut run, 1, giver);
        key(&mut run, 1, 1);
        for _ in 0..4 {
            run.descend_for_test();
        }
        let signs = run.deep.as_ref().unwrap().signs.clone();
        let Some(lost) = signs.iter().find(|s| s.kind == missions::SignKind::Lost).cloned() else { continue };
        run.enter_for_test(lost.room);
        run.enemies.clear();
        run.projectiles.clear();
        run.players.get_mut(&1).unwrap().invulnerable = 100_000;
        let (x, y) = lost.point();
        put(&mut run, 1, (x / TILE_UNITS + 9.0, y / TILE_UNITS + 3.0));
        calm(&mut run, 3);
        let close = |run: &Run| close_at(run, (x, y - 4.0), (128, 80));
        save_img(dir, &close(&run), &format!("rescue-{tag}-a-huddled"));
        put(&mut run, 1, (x / TILE_UNITS + 1.4, y / TILE_UNITS + 0.4));
        calm(&mut run, 4);
        save_img(dir, &close(&run), &format!("rescue-{tag}-b-waving"));
        press(&mut run, 1);
        calm(&mut run, 10);
        save_img(dir, &close(&run), &format!("rescue-{tag}-c-found"));
        if culture == Culture::Roostfolk {
            save(dir, &run, "rescue-roostfolk-c0-found-board");
        }
        calm(&mut run, 34);
        save_img(dir, &close(&run), &format!("rescue-{tag}-d-going-home"));
    }
}

/// The one sent home, back at their village the next time: out of the
/// hall's door, running to whoever asked, holding on.
fn reunion_shots(dir: &std::path::Path) {
    let Some(sub) = shot_dir("deep-villages/reunion") else { return };
    for culture in WHEEL {
        let tag = format!("{culture:?}").to_lowercase();
        let mut run = Run::new(41, 1, None);
        run.home = deep_home();
        let giver = (1..=4).find(|&w| matches!(culture.folk()[w].asks.as_ref().map(|a| a.errand), Some(Errand::Rescue))).unwrap();
        run.home.deep.towns.push(Town { deeds: vec![Errand::Rescue], ..Town::new(CHAPTER, culture) });
        run.home.deep.missions.push(Mission { errand: Errand::Rescue, village: CHAPTER, culture, giver: giver as u8, have: 1, need: 1, paid: true });
        run.deep_floor_for_test(CHAPTER);
        let mut deep = run.deep.take().unwrap();
        deep.village.as_mut().unwrap().hope = 5;
        run.deep = Some(deep);
        let start = the_village(&run).reunion.expect("a reunion is due");
        let wait = start - run.tick;
        calm(&mut run, wait);
        let (gx, gy) = SPEAKERS[giver];
        put(&mut run, 1, (gx + 2.5, gy + 1.5));
        let (hx, hy) = village::units((gx - 1.5, gy - 0.6));
        for k in 0..(village::RUN_HOME + village::REUNION) / 3 {
            if culture == Culture::Capfolk || k % 4 == 0 {
                save_img(&sub, &close_at(&run, (hx, hy), (144, 88)), &format!("{tag}-{k:03}"));
            }
            if k == 20 {
                save(dir, &run, &format!("reunion-{tag}-hug"));
            }
            calm(&mut run, 3);
        }
        save(dir, &run, &format!("reunion-{tag}-after"));
    }
}

/// The board on leaving a village down the stair, and home by its light.
fn leaving_shots(dir: &std::path::Path) {
    let mut run = village_of(Culture::Roostfolk, CHAPTER, 2);
    calm(&mut run, village::FALL + village::GATHER + 4);
    for who in 0..=2 {
        speak(&mut run, 1, who);
        key(&mut run, 1, 1);
    }
    speak(&mut run, 1, cultures::WISHKEEPER);
    key(&mut run, 1, 1);
    run.offer_wish(1, "light a lantern on every bridge", None).unwrap();
    key(&mut run, 1, 1);
    calm(&mut run, 10);
    let mut down = run.clone();
    down.descend_for_test();
    // The floor's plate, which the helper puts away, and its board first.
    down.begin_floor_plate(crate::rules::floor_plates::Kind::of(down.dungeon.pack));
    for plate in down.floor_plates.values_mut() {
        plate.results = crate::rules::floor_stats::SHOWN;
    }
    let mut age = 0;
    for at in [8u32, 30, 100] {
        calm(&mut down, u64::from(at - age));
        age = at;
        save(dir, &down, &format!("leaving-a-down-{at:03}"));
    }
    let (x, y) = run.light.unwrap();
    for id in [1u32, 2] {
        let h = run.players.get_mut(&id).unwrap();
        (h.x, h.y) = (x, y);
    }
    calm(&mut run, 120);
    save(dir, &run, "leaving-b-home");
}

#[test]
fn a_city_scroll_crosses_to_the_rules_worker_and_back() {
    let mut run = village_run(2, CHAPTER);
    speak(&mut run, 1, cultures::WISHKEEPER);
    key(&mut run, 1, 1);
    run.offer_wish(1, "a statue of me, so they remember my name", None).unwrap();
    let sent = crate::rules::runtime_state::capture_for_step(&run).unwrap().into_value();
    let mut worker = crate::rules::runtime_state::restore_value(&sent).unwrap();
    worker.step(&BTreeMap::new());
    let answered = crate::rules::runtime_state::capture(&worker).unwrap().into_value();
    let mut host = crate::rules::runtime_state::restore_value(&answered).unwrap();
    assert!(host.city_wishing(1));
    key(&mut host, 1, 1);
    assert!(host.players[&1].offer.is_none());
    assert_eq!(host.home.deep.town(CHAPTER).unwrap().wishes[0].gift, Gift::Statue);
}
