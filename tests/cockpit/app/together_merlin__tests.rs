use super::*;
use crate::drive::together_realm::Spoils;
use crate::drive::together_shooter::home::{Home, Station};

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
