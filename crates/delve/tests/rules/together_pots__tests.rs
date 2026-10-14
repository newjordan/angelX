//! Clay pots: placed from the seed, broken by shots and swings, a coin in
//! every other one, and broken for the rest of the floor.
use super::*;

/// The first floor below, standing in a room that keeps pots.
fn potted_room() -> Run {
    (0..60u64)
        .find_map(|seed| {
            let mut run = Run::new(seed, 1, None);
            run.descend_for_test();
            let index = (0..run.dungeon.rooms.len()).find(|&i| run.pots_of(i).len() == pots::POTS)?;
            run.enter_for_test(index);
            run.enemies.clear();
            Some(run)
        })
        .expect("a room with pots")
}

#[test]
fn pots_stand_on_the_floor_by_the_walls_of_ordinary_rooms() {
    let run = potted_room();
    for (i, room) in run.dungeon.rooms.iter().enumerate() {
        if room.kind.in_world() || matches!(room.kind, RoomKind::Home | RoomKind::Ledge) {
            assert!(run.pots_of(i).is_empty(), "no pots in {:?}", room.kind);
        }
    }
    let room = run.room();
    for (_, (x, y)) in run.pots_of(run.at) {
        let (tx, ty) = ((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32);
        assert_eq!(room.tile(tx, ty), Tile::Floor);
        assert!(ty == 1 || ty == room.rows as i32 - 2, "by a wall");
    }
    // The seed places them: a friend's copy agrees.
    let twin = run.clone();
    assert_eq!(twin.pots_of(twin.at), run.pots_of(run.at));
}

#[test]
fn a_swing_breaks_a_pot_and_it_stays_broken_for_the_floor() {
    let mut run = potted_room();
    let (i, (x, y)) = run.pots_of(run.at)[0];
    {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (x + 1.0, y + 1.0);
        h.swing = 4;
    }
    run.tick_pots();
    assert!(run.pots_of(run.at).iter().all(|&(k, _)| k != i), "the pot is gone");
    assert_eq!(run.pot_shards.len(), 1);
    assert!(run.sounds.contains(&"bones_clatter"));
    assert!(run.valid_snapshot());
    // Swinging again breaks nothing new.
    let left = run.pots_of(run.at).len();
    run.players.get_mut(&1).unwrap().swing = 0;
    run.tick_pots();
    assert_eq!(run.pots_of(run.at).len(), left);
}

#[test]
fn a_knights_shot_breaks_a_pot_and_is_spent() {
    let mut run = potted_room();
    let (i, (x, y)) = run.pots_of(run.at)[1];
    run.projectiles.push(Projectile {
        x,
        y: y + 0.3,
        vx: 0.0,
        vy: 0.0,
        hostile: false,
        kind: Shot::Arrow,
        look: None,
        damage: 1,
        pierce: 0,
        last_hit: None,
        ttl: 10,
        empowered: false,
        traits: ShotTraits::default(),
    });
    run.tick_pots();
    assert!(run.pots_of(run.at).iter().all(|&(k, _)| k != i));
    assert_eq!(run.projectiles.last().unwrap().ttl, 0, "the arrow stops in the pot");
}

#[test]
fn every_other_pot_holds_a_coin_and_a_new_floor_has_whole_pots() {
    let mut run = potted_room();
    let all = run.pots_of(run.at);
    let coins_before = run.feel.coins.len();
    for &(_, (x, y)) in &all {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (x, y + 1.0);
        h.swing = 4;
        run.tick_pots();
    }
    assert!(run.pots_of(run.at).is_empty());
    let coins = run.feel.coins.len() - coins_before;
    assert!(coins < all.len(), "not every pot pays");
    run.descend_for_test();
    assert!(run.pots_broken.is_empty(), "a new floor, whole pots");
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_pot_shots -- --ignored`
#[test]
#[ignore]
fn write_pot_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("pots");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = potted_room();
    let (_, (x, y)) = run.pots_of(run.at)[0];
    {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (x + 2.5, y + 1.5);
    }
    run.step(&BTreeMap::new());
    save(&run, "a-whole");
    {
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (x + 1.8, y + 0.8);
        h.swing = 4;
    }
    run.tick_pots();
    for _ in 0..4 {
        run.step(&BTreeMap::new());
    }
    save(&run, "b-broken");
}
