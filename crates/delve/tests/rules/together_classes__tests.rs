//! The class halls: a banner stone by its building, E to become the class.
use super::*;
use super::testing::*;
use crate::render::overworld::kingdom as geography;

fn press(id: u32) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, Input { interact: true, ..Default::default() })])
}

#[test]
fn every_class_has_a_banner_in_the_kingdom_on_open_ground() {
    let r = kingdom();
    let geo = geography::geography(&r.kingdom.wishes);
    let stones = stones(&geo);
    assert_eq!(stones.len(), CLASSES.len(), "{stones:?}");
    for &(_, (x, y)) in &stones {
        assert_eq!(geo.room.tile(x, y), Tile::Floor);
    }
}

#[test]
fn e_at_a_banner_changes_class_once_and_keeps_the_share_of_health() {
    let mut r = kingdom();
    let geo = geography::geography(&r.kingdom.wishes);
    let stones = stones(&geo);
    let (i, (x, y)) = stones[2]; // the Gunner, by the Smithy
    {
        let hero = r.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = ((x as f32 + 0.5) * TILE_UNITS, (y as f32 + 1.2) * TILE_UNITS);
        hero.max_hp = 100;
        hero.hp = 50;
    }
    r.step(&press(1));
    let hero = &r.players[&1];
    assert_eq!(of(hero).map(|c| c.name), Some(CLASSES[i].name));
    assert_eq!(hero.weapon, Weapon::Handgonne);
    assert_eq!(hero.hp * 2, hero.max_hp, "half health stays half");
    // Holding E is one change; a fresh press at the same banner is no change.
    let before = r.players[&1].clone();
    r.step(&press(1));
    r.step(&BTreeMap::new());
    r.step(&press(1));
    assert_eq!(r.players[&1].hp, before.hp);
    assert!(r.valid_snapshot());
}

#[test]
fn a_sanctuary_banner_changes_class_below_ground_for_gold() {
    let mut run = (0..40u64)
        .map(|seed| {
            let mut r = Run::new(seed, 1, None);
            r.descend_for_test();
            r
        })
        .find(|r| r.dungeon.rooms.iter().any(|room| room.kind == RoomKind::Sanctuary))
        .expect("a floor with a sanctuary");
    let index = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Sanctuary).unwrap();
    run.enter_for_test(index);
    run.enemies.clear();
    let (bx, by) = run.room().field_banner().expect("a field banner");
    {
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (bx, by + 0.5);
        hero.carried.add(Spoil::Gold, 25);
    }
    let before = of(&run.players[&1]).map(|c| c.name);
    run.step(&press(1));
    let after = of(&run.players[&1]).map(|c| c.name);
    assert_ne!(before, after, "the next class");
    assert_eq!(run.players[&1].carried.get(Spoil::Gold), 5, "for twenty gold");
    // Out of gold: no change.
    run.step(&BTreeMap::new());
    run.step(&press(1));
    assert_eq!(of(&run.players[&1]).map(|c| c.name), after);
    assert_eq!(run.found.as_ref().map(|f| f.2.as_str()), Some("NEEDS 20 GOLD"));
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_field_banner_shot -- --ignored`
#[test]
#[ignore]
fn write_field_banner_shot() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let mut run = (0..40u64)
        .map(|seed| {
            let mut r = Run::new(seed, 1, None);
            r.descend_for_test();
            r
        })
        .find(|r| r.dungeon.rooms.iter().any(|room| room.kind == RoomKind::Sanctuary))
        .unwrap();
    let index = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Sanctuary).unwrap();
    run.enter_for_test(index);
    run.enemies.clear();
    let (bx, by) = run.room().field_banner().unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (bx - 2.0, by + 2.0);
    for _ in 0..30 {
        run.step(&BTreeMap::new());
    }
    let img = crate::render::overworld::arena::frame_for(&run, 768, 448, Some(1));
    let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
    out.extend(img.rgb_bytes());
    std::fs::write(std::path::PathBuf::from(dir).join("field-banner.ppm"), out).unwrap();
}

#[test]
fn the_mage_learns_spells_and_leaves_them_behind() {
    let mut r = kingdom();
    let mage = CLASSES.iter().find(|c| c.name == "Mage").unwrap();
    let archer = CLASSES.iter().find(|c| c.name == "Archer").unwrap();
    r.become_class(1, mage);
    let hero = &r.players[&1];
    assert_eq!(of(hero).map(|c| c.name), Some("Mage"));
    assert!(hero.spells.iter().flatten().any(|s| s == "cabbage-storm"));
    assert!(hero.hand.iter().any(|c| c == "thunder-scroll"));
    r.become_class(1, archer);
    let hero = &r.players[&1];
    assert_eq!(of(hero).map(|c| c.name), Some("Archer"));
    assert!(!hero.spells.iter().flatten().any(|s| s == "cabbage-storm"), "a class's spells go with it");
    let healer = CLASSES.iter().find(|c| c.name == "Healer").unwrap();
    r.become_class(1, healer);
    assert!(r.players[&1].hand.iter().any(|c| c == "censer"));
    assert!(r.valid_snapshot());
}

/// The classes at play: a dungeon fight (two rooms, so all seven are in),
/// the kingdom by the halls, and a folly. Each is saved as the native
/// picture and as the 192-column terminal's half-blocks, under
/// `<dir>/classes-<art>/`, `art` being v1 or v2 (`ANGEL_ART=v2`).
/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_class_play_shots -- --ignored`
#[test]
#[ignore]
fn write_class_play_shots() {
    use crate::render::overworld::arena;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let art = if arena::art_v2::enabled() { "v2" } else { "v1" };
    let dir = std::path::PathBuf::from(dir).join(format!("classes-{art}"));
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = arena::frame_for(run, 768, 448, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
        let cells = arena::frame_for(run, 192, 112, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", cells.w, cells.h).into_bytes();
        out.extend(cells.rgb_bytes());
        std::fs::write(dir.join(format!("{name}-cells.ppm")), out).unwrap();
    };
    let named = |name: &str| CLASSES.iter().find(|c| c.name == name).unwrap();
    // Each knight in a pose: (class, aim, walking, striking).
    let pose = |run: &mut Run, id: u32, (aim_x, aim_y): (f32, f32), stride: f32, strike: bool| {
        let h = run.players.get_mut(&id).unwrap();
        (h.aim_x, h.aim_y) = (aim_x, aim_y);
        h.stride = stride;
        h.fire_cooldown = if strike { 6 } else { 0 };
        h.invulnerable = 0;
        h.struck = 0;
    };
    let fight = |classes: [&str; 4], seed: u64| {
        let mut run = (seed..seed + 40)
            .map(|s| {
                let mut r = Run::new(s, 1, Some("Friend"));
                r.descend_for_test();
                r
            })
            .find(|r| r.dungeon.rooms.iter().any(|room| room.kind == RoomKind::Fight))
            .unwrap();
        for (id, name) in [(2, "Two"), (3, "Three"), (4, "Four")] {
            run.join_seat(id, name);
        }
        let index = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight).unwrap();
        run.enter_for_test(index);
        for _ in 0..30 {
            run.step(&BTreeMap::new());
        }
        for (k, class) in classes.iter().enumerate() {
            run.become_class(k as u32 + 1, named(class));
        }
        let (cx, cy) = (run.room().cols as f32 * TILE_UNITS / 2.0, run.room().rows as f32 * TILE_UNITS / 2.0);
        let spots = [(-3.0, -1.0), (3.0, -1.0), (-3.0, 2.0), (3.0, 2.0)];
        for (k, (id, h)) in run.players.iter_mut().enumerate() {
            let _ = id;
            (h.x, h.y) = (cx + spots[k].0 * TILE_UNITS / 2.0, cy + spots[k].1 * TILE_UNITS / 2.0);
            h.hp = h.max_hp;
        }
        run
    };
    let mut run = fight(["Mage", "Healer", "Archer", "Swordsman"], 3);
    pose(&mut run, 1, (1.0, 0.0), 0.0, true);
    pose(&mut run, 2, (0.0, 1.0), 2.0, false);
    pose(&mut run, 3, (-1.0, 0.0), 0.0, true);
    pose(&mut run, 4, (0.0, -1.0), 1.0, false);
    save(&run, "fight-1");
    let mut run = fight(["Shieldmaiden", "Gunner", "Berserker", "Mage"], 3);
    pose(&mut run, 1, (1.0, 0.0), 0.0, false);
    pose(&mut run, 2, (-1.0, 0.0), 0.0, true);
    pose(&mut run, 3, (0.0, 1.0), 2.5, false);
    pose(&mut run, 4, (0.0, -1.0), 0.0, true);
    save(&run, "fight-2");
    // The kingdom: four knights on the road before the Keep.
    let mut run = kingdom();
    for (id, name) in [(2, "Two"), (3, "Three"), (4, "Four")] {
        run.join_seat(id, name);
    }
    let (sx, sy) = geography::keep_stand();
    for (k, class) in ["Archer", "Healer", "Gunner", "Mage"].iter().enumerate() {
        let id = k as u32 + 1;
        run.become_class(id, named(class));
        let h = run.players.get_mut(&id).unwrap();
        (h.x, h.y) = ((sx as f32 + 0.5 + k as f32 * 1.5 - 2.0) * TILE_UNITS, (sy as f32 + 3.5) * TILE_UNITS);
    }
    pose(&mut run, 1, (1.0, 0.0), 0.0, false);
    pose(&mut run, 2, (0.0, 1.0), 0.0, false);
    pose(&mut run, 3, (-1.0, 0.0), 2.0, false);
    pose(&mut run, 4, (0.0, -1.0), 0.0, false);
    save(&run, "kingdom");
    // A folly.
    let mut run = Run::new(41, 1, None);
    run.join_seat(2, "Two");
    run.enemies.clear();
    run.pick_folly();
    run.enter_for_test(run.folly_room.unwrap());
    for _ in 0..crate::rules::folly::TITLE + 30 {
        run.step(&BTreeMap::new());
    }
    run.become_class(1, named("Mage"));
    run.become_class(2, named("Shieldmaiden"));
    // Up off the action bar, in the maze's middle.
    let (mx, my) = (run.room().cols as f32 * TILE_UNITS / 2.0, run.room().rows as f32 * TILE_UNITS / 2.0);
    for (k, h) in run.players.values_mut().enumerate() {
        (h.x, h.y) = (mx + k as f32 * 1.5 * TILE_UNITS, my);
    }
    pose(&mut run, 1, (1.0, 0.0), 2.0, false);
    pose(&mut run, 2, (0.0, 1.0), 0.0, false);
    save(&run, "folly");
    // A side-on ledge hall.
    let mut run = Run::new(0, 1, Some("Matt"));
    run.join_seat(2, "Two");
    let at = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Ledge).unwrap();
    let back = run.dungeon.rooms[at].way_back().unwrap();
    run.enter(at, Some(back));
    for _ in 0..60 {
        run.step(&BTreeMap::new());
    }
    run.become_class(1, named("Healer"));
    run.become_class(2, named("Gunner"));
    pose(&mut run, 1, (1.0, 0.0), 2.0, false);
    pose(&mut run, 2, (1.0, 0.0), 0.0, true);
    save(&run, "ledge");
}

/// Each class's own shot in the air (the Mage's bolt, the Healer's spark,
/// the Gunner's balls, the Swordsman's thrown blade), and the classes in
/// the saddle at the lists and in the cab at the chariot race, under
/// `<dir>/classes-<art>/`.
/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_class_shot_and_rider_shots -- --ignored`
#[test]
#[ignore]
fn write_class_shot_and_rider_shots() {
    use crate::realm::chivalry::Stable;
    use crate::rules::home::{BUY_HOLD, Home};
    use crate::rules::world;
    use crate::render::overworld::arena;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let art = if arena::art_v2::enabled() { "v2" } else { "v1" };
    let dir = std::path::PathBuf::from(dir).join(format!("classes-{art}"));
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = arena::frame_for(run, 768, 448, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let named = |name: &str| CLASSES.iter().find(|c| c.name == name).unwrap();
    // The shots: four knights in a row loosing at the room's far side.
    let mut run = (3u64..43)
        .map(|s| {
            let mut r = Run::new(s, 1, Some("Friend"));
            r.descend_for_test();
            r
        })
        .find(|r| r.dungeon.rooms.iter().any(|room| room.kind == RoomKind::Fight))
        .unwrap();
    for (id, name) in [(2, "Two"), (3, "Three"), (4, "Four")] {
        run.join_seat(id, name);
    }
    let index = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight).unwrap();
    run.enter_for_test(index);
    for _ in 0..30 {
        run.step(&BTreeMap::new());
    }
    run.enemies.clear();
    for (k, class) in ["Mage", "Healer", "Gunner", "Swordsman"].iter().enumerate() {
        run.become_class(k as u32 + 1, named(class));
    }
    let (cx, cy) = (run.room().cols as f32 * TILE_UNITS / 2.0, run.room().rows as f32 * TILE_UNITS / 2.0);
    for (k, h) in run.players.values_mut().enumerate() {
        (h.x, h.y) = (cx - 6.0 * TILE_UNITS / 2.0, cy + (k as f32 - 1.5) * TILE_UNITS * 1.2);
        h.hp = h.max_hp;
    }
    let fire: BTreeMap<u32, Input> =
        (1..=4).map(|id| (id, Input { aim_x: 1, fire: true, ..Input::default() })).collect();
    for _ in 0..14 {
        run.step(&fire);
    }
    save(&run, "shots");
    let sounds: Vec<&str> = run.sounds.clone();
    std::fs::write(dir.join("shots-sounds.txt"), sounds.join("\n")).unwrap();
    // The lists: a Mage in the saddle.
    let mut run = Run::at_home(7, 1, None, Home { stable: Stable::default(), ..Home::default() }, Default::default());
    let lists = world::room_of(&run.dungeon, RoomKind::Lists).unwrap();
    run.arrive(lists, (1.9, 8.5));
    run.step(&BTreeMap::new());
    run.become_class(1, named("Mage"));
    let interact = BTreeMap::from([(1, Input { interact: true, ..Input::default() })]);
    for _ in 0..BUY_HOLD + 1 {
        run.step(&interact);
    }
    run.step(&BTreeMap::new());
    run.step(&BTreeMap::from([(1, Input { fire: true, ..Input::default() })]));
    for _ in 0..40 {
        run.step(&BTreeMap::new());
    }
    save(&run, "joust-mage");
    // The chariot race: a Gunner drives, a Healer friend takes a rival team.
    let mut stable = Stable::default();
    stable.herd.push(crate::realm::chivalry::Horse {
        name: "Bramble".into(),
        house: "qwen".into(),
        blood: crate::realm::chivalry::bloodline("qwen"),
        ..Default::default()
    });
    let mut run = Run::at_home(7, 1, None, Home { stable, ..Home::default() }, Default::default());
    run.join_seat(2, "Two");
    let hip = world::room_of(&run.dungeon, RoomKind::Hippodrome).unwrap();
    run.arrive(hip, (20.0, 10.5));
    run.become_class(1, named("Gunner"));
    run.become_class(2, named("Healer"));
    {
        let friend = run.players.get_mut(&2).unwrap();
        (friend.x, friend.y) = (12.0 * TILE_UNITS, 10.5 * TILE_UNITS);
    }
    run.step(&BTreeMap::new());
    for _ in 0..BUY_HOLD + 2 {
        run.step(&interact);
    }
    for _ in 0..crate::rules::chariot::GATE_TICKS + 60 {
        run.step(&BTreeMap::new());
    }
    save(&run, "chariot");
}
