use super::*;
use crate::realm::Spoils;
use crate::rules::home::{Home, Station};

fn at_home(home: Home) -> Run {
    Run::at_home(3, 1, None, home, Spoils::default())
}

#[test]
fn merlin_says_the_next_thing_worth_doing() {
    let tip = |home: &Home| at_home(home.clone()).merlin_tip();
    let mut home = Home::default();
    assert_eq!(tip(&home), "build", "a new realm: build something");
    home.levels.insert(Station::Forge, 1);
    assert_eq!(tip(&home), "wheel", "then spin Fortune's wheel");
    home.feats.insert("spin".into());
    home.boxes.push(feats::Tier::Bronze);
    assert_eq!(tip(&home), "coffer", "a box waits in the coffer");
    home.boxes.clear();
    assert_eq!(
        tip(&home),
        "wisdom",
        "nothing pressing before the first delve"
    );
    home.deepest = 1;
    assert_eq!(tip(&home), "wing", "dig out the west wing");
    home.levels.insert(Station::Wing, 1);
    assert_eq!(tip(&home), "cages");
    for who in ["mabel", "anselm", "pip", "maud"] {
        home.residents.insert(who.into());
    }
    assert_eq!(tip(&home), "walls");
    home.feats.insert("secret_room".into());
    assert_eq!(tip(&home), "runes");
    home.feats.insert("rune_runner".into());
    assert_eq!(tip(&home), "wisdom", "floor one done: nothing pressing");
    home.deepest = 2;
    assert_eq!(tip(&home), "pit");
    home.feats.insert("pit_tyrant".into());
    assert_eq!(tip(&home), "frogs");
    home.feats.insert("ribbit".into());
    assert_eq!(tip(&home), "wisdom");
}

#[test]
fn merlin_speaks_once_as_a_knight_walks_up() {
    let mut run = at_home(Home::default());
    let (mx, my) = (
        merlin::MERLIN_AT.0 * TILE_UNITS,
        merlin::MERLIN_AT.1 * TILE_UNITS,
    );
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (mx + 2.0, my + 1.0);
    let heard = run.cues.len();
    for _ in 0..30 {
        run.step(&BTreeMap::new());
    }
    let said: Vec<&String> = run.cues[heard..]
        .iter()
        .filter(|c| c.starts_with("merlin:"))
        .collect();
    assert_eq!(said, ["merlin:build"], "once per approach");
    // Walk away and back: he speaks again.
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (mx + 12.0, my + 8.0);
    run.step(&BTreeMap::new());
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (mx + 2.0, my + 1.0);
    run.step(&BTreeMap::new());
    assert_eq!(
        run.cues[heard..]
            .iter()
            .filter(|c| c.starts_with("merlin:"))
            .count(),
        2
    );
}

#[test]
fn merlin_and_maud_send_the_party_somewhere_and_the_arrow_follows() {
    use crate::rules::{countryside, pastimes::Game, world};
    let mut home = Home::default();
    home.residents.insert("maud".into());
    let mut run = at_home(home);
    // Merlin, walked up to: an open errand, and the arrow on it.
    let (mx, my) = (MERLIN_AT.0 * TILE_UNITS, MERLIN_AT.1 * TILE_UNITS);
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y) = (mx, my + 1.0);
    run.greeted = 0;
    run.greet_merlin();
    let sent = run.kingdom.moments.errand.expect("Merlin sends the party somewhere");
    assert!(run.found.as_ref().unwrap().2.starts_with("MERLIN:"));
    // Up in the kingdom the gold arrow points at the road there.
    run.walk_kingdom(Some(crate::render::overworld::kingdom::keep_stand()));
    let goal = run.kingdom_goal(1).expect("a goal");
    assert_ne!(goal.name, "The Delve", "the errand's road, not the default");
    // Arriving where he sent you puts the arrow away.
    run.arrive(world::room_of(&run.dungeon, sent).unwrap(), (12.0, 7.0));
    run.tick_countryside(&BTreeMap::new());
    assert!(run.kingdom.moments.errand.is_none());
    // Maud, at the bar: a game nobody has won, and the arrow on its place.
    run.kingdom.moments.errand = None;
    run.found = None;
    // (A race already won, or she sends you to the griffins first.)
    run.home.countryside.race_won = true;
    run.maud_gossips_for_test();
    let room = run.kingdom.moments.errand.expect("Maud gossips");
    assert_eq!(room, Game::ALL[0].room());
    assert!(run.found.as_ref().unwrap().2.contains("NOBODY'S WON"));
    // Once every game's first win is paid she has no gossip.
    run.kingdom.moments.errand = None;
    run.home.countryside.games_paid = u16::MAX;
    run.maud_gossips_for_test();
    assert!(run.kingdom.moments.errand.is_none());
    let _ = countryside::FIRST_WIN_GOLD;
}

#[test]
fn every_countryside_place_has_a_road_from_the_kingdom_map() {
    use crate::rules::{aerie, coastal, pastimes::Game, wetland, windcountry, woodland};
    let run = at_home(Home::default());
    let geo = crate::render::overworld::kingdom::geography(&run.kingdom.wishes);
    let places = coastal::PLACES
        .iter()
        .chain(&woodland::PLACES)
        .chain(&wetland::PLACES)
        .chain(&aerie::PLACES)
        .chain(&windcountry::PLACES)
        .copied()
        .chain(Game::ALL.iter().map(|g| g.room()));
    for room in places {
        assert!(run.gateway_for_test(room, &geo), "no road from the map to {room:?}");
    }
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_sent_shots -- --ignored`
#[test]
#[ignore]
fn write_sent_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("sent");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = at_home(Home::default());
    let (mx, my) = (MERLIN_AT.0 * TILE_UNITS, MERLIN_AT.1 * TILE_UNITS);
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y) = (mx + 1.5, my + 1.0);
    run.step(&BTreeMap::new());
    save(&run, "a-merlin");
    run.walk_kingdom(Some(crate::render::overworld::kingdom::keep_stand()));
    run.step(&BTreeMap::new());
    save(&run, "b-kingdom-arrow");
}
