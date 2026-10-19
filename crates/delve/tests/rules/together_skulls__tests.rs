use super::*;

/// Floor one of the Crypt, its first fight room emptied and calmed: knight
/// one standing on the left, knight two fallen in the middle, and knight
/// three (when asked for) standing on the right.
fn fallen_friend(three: bool) -> Run {
    let mut run = Run::new(4, 1, Some("Bree"));
    if three {
        run.join_seat(3, "Ana");
    }
    run.begin_in(Pack::Crypt);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.rune = None;
    for (id, x) in [(1, 10.0), (2, 20.0), (3, 30.0)] {
        if let Some(hero) = run.players.get_mut(&id) {
            (hero.x, hero.y, hero.invulnerable) = (x, 14.0, 0);
        }
    }
    let two = run.players.get_mut(&2).unwrap();
    two.hp = 0;
    two.fell = run.tick.max(1);
    // The bones settle before anyone can take the skull up.
    for _ in 0..=skulls::SKULL_SETTLE {
        idle(&mut run);
    }
    run
}

fn idle(run: &mut Run) {
    run.step(&BTreeMap::new());
}

fn press(run: &mut Run, id: u32) {
    run.step(&BTreeMap::from([(
        id,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
}

/// Knight one walks onto knight two's bones and takes the skull up.
fn carried(three: bool) -> Run {
    let mut run = fallen_friend(three);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (20.5, 14.0);
    idle(&mut run);
    run
}

/// A bat beside knight one, at `share` percent of its health.
fn weak_bat(run: &mut Run, share: u32) -> u32 {
    let (x, y) = (run.players[&1].x + 1.5, run.players[&1].y);
    run.spawn_at_for_test(EnemyKind::Bat, x, y);
    let bat = run.enemies.last_mut().unwrap();
    bat.hp = (bat.max_hp * share / 100).max(1);
    bat.frozen = 1000;
    bat.id
}

#[test]
fn a_friend_walks_a_fallen_knights_skull_up() {
    let mut run = fallen_friend(false);
    assert_eq!(run.skull_lying(2), Some((20.0, 14.0)), "the skull lies with the bones");
    idle(&mut run);
    assert_eq!(run.players[&1].carrying_skull, None, "too far to pick it up");
    let run = carried(false);
    assert_eq!(run.players[&1].carrying_skull, Some(2));
    assert_eq!(run.skull_carrier(2), Some(1));
    assert_eq!(run.skull_lying(2), None, "carried, it lies nowhere");
    assert!(run.cues.iter().any(|c| c == "skull_pickup"));
    let hud = run.hud(2, &hud::Keys::FIRST).unwrap();
    assert!(hud.status.contains("carries your skull"), "{}", hud.status);
    let hud = run.hud(1, &hud::Keys::FIRST).unwrap();
    assert!(hud.status.starts_with("SKULL: E"), "{}", hud.status);
}

#[test]
fn a_skull_waits_for_the_bones_to_settle() {
    let mut run = fallen_friend(false);
    run.players.get_mut(&2).unwrap().fell = run.tick;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (20.5, 14.0);
    idle(&mut run);
    assert_eq!(run.players[&1].carrying_skull, None, "still falling apart");
    for _ in 0..skulls::SKULL_SETTLE {
        idle(&mut run);
    }
    assert_eq!(run.players[&1].carrying_skull, Some(2));
}

#[test]
fn a_carrier_who_falls_drops_the_skull_at_their_feet() {
    let mut run = carried(true);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.hp) = (12.0, 9.0, 0);
    idle(&mut run);
    assert_eq!(run.players[&1].carrying_skull, None);
    assert_eq!(run.players[&2].skull_at, Some((12.0, 9.0)));
    assert_eq!(run.skull_lying(2), Some((12.0, 9.0)));
    assert_ne!(run.phase, Phase::Wiped, "knight three still stands");
    // Knight three walks over to where it fell and takes it up.
    let hero = run.players.get_mut(&3).unwrap();
    (hero.x, hero.y) = (12.4, 9.0);
    idle(&mut run);
    assert_eq!(run.skull_carrier(2), Some(3));
    // One skull in hand: knight one's own lies with its bones still.
    assert!(run.skull_lying(1).is_some());
}

#[test]
fn only_a_monster_at_a_tenth_of_its_health_or_less_takes_a_skull() {
    let mut run = carried(false);
    weak_bat(&mut run, 50);
    press(&mut run, 1);
    assert_eq!(run.players[&2].hp, 0, "a hale bat refuses the skull");
    assert_eq!(run.players[&1].carrying_skull, Some(2));
    assert_eq!(run.enemies.len(), 1);
}

#[test]
fn a_boss_never_takes_a_skull() {
    let mut run = carried(false);
    let (x, y) = (run.players[&1].x + 2.0, run.players[&1].y);
    run.spawn_boss_at_for_test(0, x, y);
    let boss = run.enemies.last_mut().unwrap();
    boss.hp = 1;
    boss.frozen = 1000;
    assert!(!Run::raisable(boss));
    press(&mut run, 1);
    assert_eq!(run.players[&2].hp, 0, "no knight rises in a guardian");
    assert_eq!(run.players[&1].carrying_skull, Some(2));
}

#[test]
fn the_skull_on_a_weak_monster_raises_the_knight_in_its_body() {
    let mut run = carried(false);
    let bat = weak_bat(&mut run, 10);
    let at = (run.enemies[0].x, run.enemies[0].y);
    press(&mut run, 1);
    assert!(run.enemies.iter().all(|e| e.id != bat), "the bat's body is the knight's now");
    let two = &run.players[&2];
    assert_eq!(two.possessed, Some(EnemyKind::Bat));
    assert_eq!(two.hp, (two.max_hp * skulls::RAISED_SHARE / 100).max(1));
    assert!((two.x - at.0).abs() < 0.01 && (two.y - at.1).abs() < 0.01, "it rises where the bat was");
    assert!(two.invulnerable > 0);
    assert_eq!(run.players[&1].carrying_skull, None, "the skull is spent");
    assert!(run.cues.iter().any(|c| c == "possess"));
    let hud = run.hud(2, &hud::Keys::FIRST).unwrap();
    assert!(hud.status.contains("bat"), "{}", hud.status);
    // It walks on its own keys, down a lane cleared of the room's pillars.
    let row = (run.players[&2].y / TILE_UNITS) as usize;
    let cols = run.room().cols;
    let at = run.at;
    for col in 1..cols - 1 {
        for r in row.saturating_sub(1)..=row + 1 {
            run.dungeon.rooms[at].set_for_test(col, r, Tile::Floor);
        }
    }
    let x = run.players[&2].x;
    for _ in 0..HZ / 2 {
        run.step(&BTreeMap::from([(
            2,
            Input {
                move_x: 1,
                ..Default::default()
            },
        )]));
    }
    assert!(run.players[&2].x > x + 1.0, "the raised knight walks");
    assert_eq!(run.players[&2].possessed, Some(EnemyKind::Bat));
}

#[test]
fn a_raised_knight_who_falls_is_bones_again_with_a_skull_to_carry() {
    let mut run = carried(false);
    weak_bat(&mut run, 5);
    press(&mut run, 1);
    assert!(run.players[&2].possessed.is_some());
    run.players.get_mut(&2).unwrap().hp = 0;
    idle(&mut run);
    assert_eq!(run.players[&2].possessed, None);
    assert!(run.skull_lying(2).is_some(), "the skull lies again, to be carried again");
}

#[test]
fn the_sanctuary_gives_a_raised_knight_their_own_shape() {
    let mut run = carried(false);
    weak_bat(&mut run, 10);
    press(&mut run, 1);
    let two = run.players.get_mut(&2).unwrap();
    two.hp = 1;
    let well = match run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Sanctuary) {
        Some(i) => i,
        None => {
            let i = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Treasure).unwrap_or(0);
            run.dungeon.rooms[i].kind = RoomKind::Sanctuary;
            i
        }
    };
    run.enter_for_test(well);
    let two = &run.players[&2];
    assert_eq!(two.possessed, None, "themselves again at the well");
    assert!(two.hp >= (two.max_hp * REVIVE_SHARE / 100).max(1), "{} of {}", two.hp, two.max_hp);
    assert!(run.cues.iter().any(|c| c == "unpossess"));
}

#[test]
fn the_stairs_down_reach_the_next_floors_well() {
    // A raised knight is standing, so the stairs do not revive them; the
    // next floor opens on its Sanctuary, and that gives them back their shape.
    let mut run = carried(false);
    weak_bat(&mut run, 10);
    press(&mut run, 1);
    let hp = run.players[&2].hp;
    run.descend_for_test();
    let two = &run.players[&2];
    assert_eq!(run.room().kind, RoomKind::Sanctuary);
    assert_eq!(two.possessed, None);
    assert!(two.hp >= hp, "standing, mended, not revived from nothing");
}

#[test]
fn a_raised_knight_holds_off_the_wipe_with_the_host_seat_absent() {
    // The live server's party: the host plays no knight (an absent seat),
    // two friends fight.
    let mut run = fallen_friend(true);
    run.set_absent(1, true);
    // Knight three carries knight two's skull to a weak bat.
    let hero = run.players.get_mut(&3).unwrap();
    (hero.x, hero.y) = (20.5, 14.0);
    idle(&mut run);
    assert_eq!(run.players[&3].carrying_skull, Some(2));
    let (x, y) = (run.players[&3].x + 1.5, run.players[&3].y);
    run.spawn_at_for_test(EnemyKind::Bat, x, y);
    let bat = run.enemies.last_mut().unwrap();
    bat.hp = 1;
    bat.frozen = 1000;
    press(&mut run, 3);
    assert_eq!(run.players[&2].possessed, Some(EnemyKind::Bat));
    // Knight three falls; the raised knight still stands.
    run.players.get_mut(&3).unwrap().hp = 0;
    idle(&mut run);
    assert_ne!(run.phase, Phase::Wiped, "a raised knight is a knight standing");
    // And when it falls too, the party has fallen.
    run.players.get_mut(&2).unwrap().hp = 0;
    idle(&mut run);
    assert_eq!(run.phase, Phase::Wiped);
}

#[test]
fn a_save_from_before_skulls_still_loads() {
    let run = fallen_friend(false);
    let json = serde_json::to_string(&run).unwrap();
    assert!(!json.contains("possessed") && !json.contains("carrying_skull") && !json.contains("skull_at"));
    let back: Run = serde_json::from_str(&json).unwrap();
    assert_eq!(back.players[&2].possessed, None);
    assert_eq!(back.players[&1].carrying_skull, None);
    // And a raised knight round-trips.
    let mut run = carried(false);
    weak_bat(&mut run, 10);
    press(&mut run, 1);
    let back: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    assert_eq!(back.players[&2].possessed, Some(EnemyKind::Bat));
}


/// Skulls in play: `ANGEL_ARENA_SHOTS=<dir> cargo test write_skull_shots
/// -- --ignored`.
#[test]
#[ignore]
fn write_skull_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("skulls");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str, focus: u32| {
        for (art, img) in [
            ("", crate::render::overworld::arena::frame_for(run, 768, 448, Some(focus))),
            ("-v1", crate::render::overworld::arena::frame_v1(run, 768, 448, Some(focus))),
        ] {
            let mut img = img;
            crate::render::overworld::arena::presence_marks(&mut img, run, Some(focus));
            let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
            out.extend(img.rgb_bytes());
            std::fs::write(dir.join(format!("{name}{art}.ppm")), out).unwrap();
        }
    };
    // The fallen knight's own view of its skull, and a friend's.
    let run = fallen_friend(false);
    save(&run, "a-skull-lying-own", 2);
    save(&run, "b-skull-lying-friend", 1);
    // Carried over the head, a weak bat marked.
    let mut run = carried(false);
    weak_bat(&mut run, 10);
    run.spawn_at_for_test(EnemyKind::Skeleton, 30.0, 10.0);
    run.enemies.last_mut().unwrap().frozen = 1000;
    idle(&mut run);
    save(&run, "c-carried", 1);
    // Raised in the bat's body.
    press(&mut run, 1);
    for _ in 0..HZ {
        idle(&mut run);
    }
    save(&run, "d-raised", 1);
    save(&run, "e-raised-own", 2);
}
