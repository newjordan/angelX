use super::*;

fn hall() -> Run {
    let mut run = Run::at_home(5, 1, None, Default::default(), Default::default());
    let index = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Trophies)
        .expect("the hall is at home");
    run.enter_for_test(index);
    run
}

#[test]
fn the_runner_leads_south_from_the_undercroft_to_the_hall() {
    let run = Run::at_home(5, 1, None, Default::default(), Default::default());
    let home = &run.dungeon.rooms[0];
    let hall = run
        .dungeon
        .rooms
        .iter()
        .find(|r| r.kind == RoomKind::Trophies)
        .unwrap();
    assert!(home.doors[2], "a door in the Undercroft's south wall");
    assert!(hall.doors[0], "and the hall's north wall");
    assert_eq!((hall.cell.0, hall.cell.1), (home.cell.0, home.cell.1 + 1));
    for plinth in &PLINTHS {
        let (col, row, ..) = plinth.at;
        assert_eq!(hall.tile(col, row), Tile::Block, "{}", plinth.id);
    }
}

#[test]
fn what_falls_is_marked_for_the_hall() {
    let mut run = hall();
    let heard = run.cues.len();
    for cue in ["boss_fall:cinderjaw", "victory", "grail", "room_clear"] {
        run.cues.push(cue.into());
    }
    run.mark_trophies(heard);
    for id in ["cinderjaw", "dragon", "grail"] {
        assert_eq!(run.marks.get(&format!("trophy:{id}")), Some(&1), "{id}");
    }
    assert_eq!(
        run.marks
            .keys()
            .filter(|k| k.starts_with("trophy:"))
            .count(),
        3
    );
}

#[test]
fn a_knight_at_a_plinth_reads_its_plaque() {
    let (col, row, w, h) = PLINTHS[0].at;
    let front = (
        (col as f32 + w as f32 / 2.0) * TILE_UNITS,
        (row + h) as f32 * TILE_UNITS + 1.0,
    );
    assert_eq!(
        plinth_at(front.0, front.1).map(|p| p.id),
        Some(PLINTHS[0].id)
    );
    assert!(plinth_at(24.0, 2.5).is_none(), "nothing by the door");
}

#[test]
fn sir_kay_greets_whoever_walks_up_once() {
    let mut run = hall();
    let (kx, ky) = (KAY_AT.0 * TILE_UNITS, KAY_AT.1 * TILE_UNITS);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (kx + 2.0, ky + 1.5);
    for _ in 0..30 {
        run.step(&BTreeMap::new());
    }
    let greetings = |run: &Run| run.cues.iter().filter(|c| c.starts_with("npc:kay")).count();
    assert_eq!(greetings(&run), 1);
    assert!(
        run.cues.iter().any(|c| c == "npc:kay_empty"),
        "an empty hall"
    );
    // Walk away and back: a second greeting, now with something to show.
    run.home.trophies.insert("cinderjaw".into(), 1);
    run.players.get_mut(&1).unwrap().y = 20.0;
    run.step(&BTreeMap::new());
    run.players.get_mut(&1).unwrap().y = ky + 1.5;
    run.step(&BTreeMap::new());
    assert_eq!(greetings(&run), 2);
    assert!(run.cues.iter().any(|c| c == "npc:kay"));
}

#[test]
fn walking_down_the_runner_enters_the_hall() {
    let mut run = Run::at_home(5, 1, None, Default::default(), Default::default());
    let bottom = run.dungeon.rooms[0].height() - 0.3;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, bottom);
    run.step(&BTreeMap::new());
    assert_eq!(run.room().kind, RoomKind::Trophies);
    assert!(run.at_home_now() && run.active());
}
