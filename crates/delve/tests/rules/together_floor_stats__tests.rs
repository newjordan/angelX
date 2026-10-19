use super::*;
use crate::rules::floor_plates::DURATION;

fn firing(ids: &[u32]) -> BTreeMap<u32, Input> {
    ids.iter()
        .map(|&id| (id, Input { aim_x: 1, fire: true, ..Default::default() }))
        .collect()
}

/// A party of two on its first floor, the stairs below it open.
fn first_floor() -> Run {
    let mut run = Run::at_home(4, 2, Some("Matt"), Default::default(), Default::default());
    run.descend_for_test();
    run
}

/// The floor's guardian and its prerequisites behind the party.
fn stairs_open(run: &mut Run) {
    run.clear_prerequisites_for_test();
    if let Some(g) = &mut run.boss_gates {
        g.guardian_defeated = true;
        run.dungeon.rooms[g.guardian].visited = true;
        run.dungeon.rooms[g.guardian].cleared = true;
    }
}

fn fights(run: &Run) -> Vec<usize> {
    run.dungeon
        .rooms
        .iter()
        .enumerate()
        .filter(|(_, r)| r.kind == RoomKind::Fight)
        .map(|(i, _)| i)
        .collect()
}

#[test]
fn a_floors_tally_counts_across_its_rooms_and_starts_over_on_the_next_floor() {
    let mut run = first_floor();
    let rooms = fights(&run);
    assert!(rooms.len() >= 2);
    let mut shots = 0;
    for (n, &room) in rooms.iter().take(2).enumerate() {
        run.enter_for_test(room);
        for hero in run.players.values_mut() {
            hero.invulnerable = 100_000;
        }
        for _ in 0..40 {
            run.step(&firing(&[1]));
        }
        let now = run.floor_tally.knights[&1].shots;
        assert!(now > shots, "room {n}: the shots loosed here are added to the floor's");
        shots = now;
        run.clear_for_test();
    }
    // A blow dealt and one taken, as the tick counts them.
    run.tally_floor(&BTreeMap::from([(2, 30)]));
    assert!(run.floor_tally.knights[&2].damage >= 30);
    let fought = run.floor_tally.knights[&1].fought;
    assert!(fought > 0, "the ticks stood in a fight are counted");
    stairs_open(&mut run);
    run.descend();
    let results = run.floor_results.as_ref().expect("the floor behind is closed");
    assert_eq!((results.depth, results.end), (1, End::Cleared));
    assert_eq!(results.row(1).unwrap().line.shots, shots);
    assert!(results.row(2).unwrap().line.damage >= 30);
    assert_eq!(results.row(1).unwrap().name, run.players[&1].shown_name());
    assert!(run.floor_tally.knights.is_empty(), "a new floor's tally starts over");
    assert!(run.floor_tally.wishes.is_empty());
    assert!(run.floor_tally.pace[&1].1 >= u64::from(fought), "the delve's pace carries on");
}

#[test]
fn the_wish_that_lifted_its_knights_pace_most_is_the_floors_most_impactful() {
    let mut run = Run::new(4, 2, Some("Wren"));
    run.join_seat(3, "Bors");
    let mut tally = Tally::default();
    let fight = |tally: &mut Tally, id: u32, damage: u32, ticks: u64| {
        for t in 0..ticks {
            tally.count(id, if t == 0 { damage } else { 0 }, true);
        }
    };
    // Each fought ten seconds at 30 a second before wishing.
    for id in 1..=3 {
        fight(&mut tally, id, 300, 10 * u64::from(HZ));
    }
    // Knight 1 makes two wishes together and doubles their pace; knight 2
    // lifts theirs by a third; knight 3 deals a great deal, but at the
    // pace they kept before.
    tally.wished(1, "Twin Bolts");
    tally.wished(1, "Bounce");
    tally.wished(2, "Fork");
    tally.wished(3, "Bigger Arrows");
    fight(&mut tally, 1, 600, 10 * u64::from(HZ));
    fight(&mut tally, 2, 400, 10 * u64::from(HZ));
    fight(&mut tally, 3, 300, 10 * u64::from(HZ));
    assert_eq!(tally.wishes.len(), 3, "wishes made with no fight between them are measured as one");
    let top = impact(&tally, &run.players).unwrap();
    assert_eq!((top.knight, top.wish.as_str()), (1, "Twin Bolts and Bounce"));
    assert_eq!((top.damage, top.gain), (300, Some(100)), "300 over the old pace: twice as fast");
    assert_eq!(top.name, run.players[&1].shown_name());

    // Wishes made before any fight cannot be measured: the one held while
    // its knight dealt the most is named for that, and nothing more.
    let mut fresh = Tally::default();
    fresh.wished(2, "Fork");
    fresh.wished(3, "Bigger Arrows");
    fight(&mut fresh, 2, 200, 10 * u64::from(HZ));
    fight(&mut fresh, 3, 500, 10 * u64::from(HZ));
    let held = impact(&fresh, &run.players).unwrap();
    assert_eq!((held.knight, held.wish.as_str(), held.damage, held.gain), (3, "Bigger Arrows", 500, None));
    assert_eq!(impact(&Tally::default(), &run.players), None, "no wish, none named");
}

#[test]
fn honours_go_to_whoever_tops_them_and_the_mvp_holds_the_biggest_share() {
    let mut run = Run::new(4, 2, Some("Wren"));
    run.join_seat(3, "Bors");
    let mut tally = Tally::default();
    *tally.line(1) = Line { kills: 9, damage: 400, taken: 30, shots: 40, hits: 30, fought: 600, ..Line::default() };
    *tally.line(2) = Line { kills: 3, damage: 900, taken: 0, shots: 10, hits: 10, fought: 600, gold: 12, ..Line::default() };
    *tally.line(3) = Line { kills: 2, damage: 100, taken: 80, falls: 1, raised: 1, fought: 600, ..Line::default() };
    let results = Results::of(&tally, &run.players, 2, End::Cleared, 900);
    let honours = |id: u32| results.row(id).unwrap().honours.clone();
    assert_eq!(honours(1), vec![Honour::MostKills, Honour::Sharpshooter], "ten shots are too few to judge an aim by");
    assert_eq!(honours(2), vec![Honour::HardestHitter, Honour::Untouched, Honour::DeepPockets]);
    assert_eq!(honours(3), vec![Honour::Lifesaver]);
    assert_eq!(results.row(1).unwrap().line.aim(), Some(75));
    // Shares: 1 holds 9 of 14 kills and 4 of 14 in damage, 2 holds 9 of
    // 14 in damage and 3 of the kills, 3 the only raising, at half.
    assert_eq!(results.mvp, Some(1), "the biggest share of the floor's work");
    *tally.line(3) = Line { raised: 2, kills: 4, damage: 300, ..tally.knights[&3] };
    let raised = Results::of(&tally, &run.players, 2, End::Cleared, 900);
    assert_eq!(raised.mvp, Some(3), "friends raised count");
    let alone = Results::of(&tally, &BTreeMap::from([(1, run.players[&1].clone())]), 2, End::Cleared, 900);
    assert_eq!(alone.mvp, None, "a knight alone is nobody's MVP");
    assert_eq!(alone.rows[0].honours, vec![Honour::Sharpshooter], "only what is no contest is honoured alone");
}

#[test]
fn the_results_open_the_next_floors_plate_for_each_seat_and_a_press_or_a_fight_cuts_them_short() {
    let mut run = first_floor();
    stairs_open(&mut run);
    run.descend();
    let plate = run.floor_plate(Some(1)).unwrap();
    assert_eq!((plate.age, plate.results, plate.length()), (0, SHOWN, SHOWN + DURATION));
    assert_eq!(run.results_showing(Some(2)).map(|(r, age)| (r.depth, age)), Some((1, 0)));
    run.step(&BTreeMap::from([(1, Input { skip_plate: true, ..Default::default() })]));
    assert!(run.floor_plate(Some(1)).is_none(), "a press skips the results and the plate");
    assert_eq!(run.results_showing(Some(2)).map(|(_, age)| age), Some(1), "the other seat's run on");
    for _ in 1..SHOWN {
        run.step(&BTreeMap::new());
    }
    assert!(run.results_showing(Some(2)).is_none());
    assert_eq!(run.floor_plate(Some(2)).and_then(|p| p.title()).map(|t| t.age), Some(0), "the floor's own title follows");

    // A fight breaking out while the results show cuts them short.
    let mut run = first_floor();
    stairs_open(&mut run);
    run.descend();
    let room = fights(&run)[0];
    let plates = run.floor_plates.clone();
    run.enter_for_test(room);
    run.floor_plates = plates;
    assert!(run.fight_on());
    run.step(&BTreeMap::new());
    assert!(run.results_showing(Some(1)).is_none(), "a fight never waits on the results");
    assert!(run.floor_plate(Some(1)).is_some_and(|p| p.title().is_some()));
}

#[test]
fn a_fallen_party_sees_its_last_floor_after_the_verdict_stands_a_moment() {
    let mut run = first_floor();
    let room = fights(&run)[0];
    run.enter_for_test(room);
    for _ in 0..20 {
        run.step(&firing(&[1, 2]));
    }
    for hero in run.players.values_mut() {
        hero.hp = 0;
        (hero.winds, hero.talisman) = (0, false);
    }
    run.deeds = vec!["first_delve".into(), "party_of_four".into()];
    run.step(&BTreeMap::new());
    assert_eq!(run.phase, Phase::Wiped);
    let results = run.floor_results.clone().unwrap();
    assert_eq!((results.depth, results.end), (1, End::Fallen));
    assert!(results.row(1).unwrap().line.shots > 0);
    assert_eq!(results.deeds, run.deeds, "the deeds of the delve go with its last floor");
    assert_eq!(run.verdict_hold(), crate::rules::again::VERDICT_HOLD + DEEDS_HOLD, "deeds to show hold the verdict longer");
    assert!(run.results_showing(Some(1)).is_none(), "the verdict stands first");
    for _ in 0..VERDICT_FIRST + 10 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.results_showing(Some(1)).map(|(_, age)| age), Some(11), "every step the verdict stands counts");
    let stood = run.floor_results.as_ref().unwrap().stood;
    assert_eq!(Results { stood, ..results }, run.floor_results.clone().unwrap(), "the run's end is closed once");
    // A new delve starts with no deeds and no results.
    let mut home = Run::at_home(4, 2, Some("Matt"), Default::default(), Default::default());
    home.deeds = vec!["first_delve".into()];
    home.descend();
    assert!(home.deeds.is_empty() && home.floor_results.is_none());
}

#[test]
fn old_saves_load_without_the_floors_tally_its_results_or_a_plates_results() {
    let mut run = first_floor();
    stairs_open(&mut run);
    run.descend();
    let mut save = serde_json::to_value(&run).unwrap();
    let fields = save.as_object_mut().unwrap();
    assert!(fields.contains_key("floor_tally") && fields.contains_key("floor_results"));
    fields.remove("floor_tally");
    fields.remove("floor_results");
    for plate in fields["floor_plates"].as_object_mut().unwrap().values_mut() {
        plate.as_object_mut().unwrap().remove("results");
    }
    let old: Run = serde_json::from_value(save).unwrap();
    assert_eq!(old.floor_tally, Tally::default());
    assert_eq!(old.floor_results, None);
    assert_eq!(old.floor_plate(Some(1)).map(|p| (p.results, p.length())), Some((0, DURATION)));
    // And the tally never goes down the wire: its results do.
    let tree = crate::wire::publish(&run).unwrap();
    let back = crate::wire::read(&tree).unwrap();
    assert_eq!(back.floor_tally, Tally::default());
    assert_eq!(back.floor_results, run.floor_results);
}
