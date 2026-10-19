//! Powder Keg round two: ONE shared courtyard — every knight in a heat plays
//! in the same yard at the same time, so a keg can trap and soot a friend.
//! A lone knight keeps the clear-the-bales game; the roads of the Old Watch
//! stay open; the seat's screen shows only the game's board.
use super::*;
use crate::rules::pastimes::{FIRE, FUSE, POWDER_BALES as ALL_BALES, STOCK};

/// A three-seat run standing in the Colosseum's Powder Keg yard, ready to
/// start.
fn watch() -> Run {
    three_seats(41)
}

fn three_seats(seed: u64) -> Run {
    let mut run = Run::at_home(seed, 1, None, Default::default(), Default::default());
    for id in 2..=3 {
        run.join_seat(id, &format!("Seat {id}"));
    }
    let room = world::room_of(&run.dungeon, RoomKind::KegYard).unwrap();
    run.arrive(room, (12.0, 11.0));
    run
}

fn place(run: &mut Run, id: u32, (x, y): (f32, f32)) {
    let h = run.players.get_mut(&id).unwrap();
    (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
}

fn key(id: u32, (mx, my): (i8, i8), fire: bool) -> BTreeMap<u32, Input> {
    BTreeMap::from([(
        id,
        Input {
            move_x: mx,
            move_y: my,
            fire,
            ..Default::default()
        },
    )])
}

/// The pad, the start mark on the yard's west bench.
const PAD: (f32, f32) = (3.0, 5.0);

/// Start a heat: the starter on the pad, each friend beside them.
fn start_heat(run: &mut Run, seats: &[u32]) {
    place(run, seats[0], PAD);
    for (i, &id) in seats.iter().enumerate().skip(1) {
        place(run, id, (PAD.0 + i as f32, PAD.1 + 1.0));
    }
    run.tick_pastimes(&BTreeMap::new());
}

/// Drop a keg one tile below the first corner, where the east arm burns a
/// bale at (2,2), then walk clear down the open ring column 0 and let the
/// fuse finish.
fn drop_and_clear(run: &mut Run, id: u32) {
    for _ in 0..12 {
        run.tick_pastimes(&key(id, (0, 1), false));
    }
    run.tick_pastimes(&key(id, (0, 0), true));
    for _ in 0..14 {
        run.tick_pastimes(&key(id, (-1, 0), false));
    }
    for _ in 0..u32::from(FUSE) + u32::from(FIRE) + 4 {
        run.tick_pastimes(&key(id, (0, 1), false));
    }
}

#[test]
fn the_roads_of_the_old_watch_stay_open_past_the_pad() {
    let mut run = watch();
    // The pad sits off the walking roads: no knight passing the courtyard
    // along its lanes trips a heat.
    let (px, py) = (PAD.0 * TILE_UNITS, PAD.1 * TILE_UNITS);
    for &(x, y) in &[
        (9.0f32, 13.0), // the south road lane that tripped the pad in round one
        (12.0, 7.0),    // the cross roads
        (4.0, 7.0),
        (9.0, 1.0),
    ] {
        let d = ((x * TILE_UNITS - px).hypot(y * TILE_UNITS - py)) / TILE_UNITS;
        assert!(d > 1.25, "the road at ({x},{y}) is only {d:.2} tiles from the pad");
    }
    // And a knight walking that very lane does not start a heat.
    place(&mut run, 1, (12.0, 7.0));
    for _ in 0..HZ {
        run.tick_pastimes(&key(1, (-1, 1), false));
    }
    assert!(run.pastimes.heat.is_none(), "a walker on the road starts no heat");
    assert!(run.pastimes.round.is_none());
    assert!(run.valid_snapshot());
}

#[test]
fn a_dropped_keg_burns_a_cross_of_bales_and_stops_at_the_first() {
    let mut run = watch();
    run.players.retain(|&id, _| id == 1);
    start_heat(&mut run, &[1]);
    let round = run.pastimes.round.clone().expect("the round begins");
    let p = round.powder.as_ref().unwrap();
    // The knight stands in a corner on open ground: nothing beside them.
    let (c, r) = p.knights[0].tile();
    assert_eq!((c, r), (1, 1), "seat one starts in the first corner");
    assert_eq!(p.knights[0].stock, STOCK, "the stock starts full");
    assert_eq!(p.bales.len(), ALL_BALES.len());
    // The four corner lanes are open: no post or bale in a corner's own
    // tile or the two tiles out of it along the ring.
    for (cc, rr) in crate::rules::pastimes::POWDER_POSTS {
        let near_corner = [(1u8, 1u8), (9, 7), (9, 1), (1, 7)].iter().any(|&(a, b)| {
            (i16::from(cc) - i16::from(a)).abs() <= 1 && (i16::from(rr) - i16::from(b)).abs() <= 1
        });
        assert!(!near_corner, "post ({cc},{rr}) crowds a corner lane");
    }

    // F drops a keg at the feet; walk down the ring out of the cross; the
    // fuse runs out and burns a cross of bales, stopping at the first each
    // way (a post or a bale).
    drop_and_clear(&mut run, 1);
    let p = run.pastimes.round.as_ref().unwrap().powder.as_ref().unwrap();
    assert!(p.bales.len() < ALL_BALES.len(), "bales burned");
    assert!(p.kegs.is_empty() && p.flames.is_empty(), "the blast settled");
    assert!(run.sounds.contains(&"vent_fire"), "hay, not the folly's bombs");
    assert!(run.valid_snapshot());
}

#[test]
fn a_knight_caught_in_their_own_blast_is_sooted_out_in_under_a_second() {
    let mut run = watch();
    run.players.retain(|&id, _| id == 1);
    start_heat(&mut run, &[1]);
    // Drop the keg and stand on it: the cross catches the dropper.
    run.tick_pastimes(&key(1, (0, 0), true));
    let mut ticks = 0;
    loop {
        run.tick_pastimes(&key(1, (0, 0), false));
        ticks += 1;
        if run.pastimes.round.is_none() || ticks > 3 * HZ {
            break;
        }
    }
    assert!(
        ticks <= u32::from(FUSE) + 2,
        "the blast settles quickly (took {ticks})"
    );
    let soot_banner = run
        .found
        .as_ref()
        .map(|f| f.2.clone())
        .unwrap_or_default();
    assert!(soot_banner.contains("Sooted"), "{soot_banner}");
    // Fail-and-retry: the courtyard is free again at once. The knight steps
    // off the pad and back (the pad fires on the step back, as every game's
    // pad does), and a new heat starts within a second of the soot.
    place(&mut run, 1, (7.0, 5.0));
    run.tick_pastimes(&BTreeMap::new());
    place(&mut run, 1, PAD);
    for _i in 0..HZ {
        run.tick_pastimes(&BTreeMap::new());
        if run.pastimes.heat.is_some() {
            break;
        }
    }
    assert!(run.pastimes.heat.is_some(), "a new heat starts within a second");
    assert!(
        run.pastimes.round.is_some(),
        "the sooted knight is back in a fresh round"
    );
    let (_, _, said) = run.found.clone().unwrap();
    assert!(said.contains("begins"), "{said}: the retry speaks");
    assert!(run.valid_snapshot());
}

#[test]
fn a_lone_knight_clearing_bales_wins_and_the_purse_pays() {
    let mut run = watch();
    run.players.retain(|&id, _| id == 1);
    start_heat(&mut run, &[1]);
    let plays = run.home.countryside.games.powder.plays;
    assert_eq!(plays, 1, "the play is counted");
    // Clear the yard outright by hand, then let the clock judge it.
    let round = run.pastimes.round.as_mut().unwrap();
    let p = round.powder.as_mut().unwrap();
    let id = p.knights[0].id;
    p.bales.clear();
    *p.burned.entry(id).or_insert(0) = ALL_BALES.len() as u8;
    drop(round);
    run.tick_pastimes(&BTreeMap::new());
    let (_, _, said) = run.found.clone().unwrap();
    assert!(said.contains("complete!"), "{said}");
    assert!(run.home.countryside.games.powder.wins == 1);
    assert!(run.home.countryside.games.powder.best > 0);
    assert!(run.valid_snapshot());
}

/// THE round-two test: two knights in one heat, one shared courtyard, and a
/// keg that soots a friend.
#[test]
fn two_knights_in_one_heat_share_a_yard_and_soot_each_other() {
    let mut run = watch();
    run.players.retain(|&id, _| id <= 2);
    start_heat(&mut run, &[1, 2]);
    let heat = run.pastimes.heat.clone().expect("a heat begins");
    assert_eq!(heat.seats, vec![1, 2]);
    // ONE shared courtyard: both knights are in the same yard.
    let round = run.pastimes.round.clone().unwrap();
    let p = round.powder.as_ref().unwrap();
    assert_eq!(p.knights.len(), 2, "both knights in the one yard");
    assert_eq!(p.knights[0].tile(), (1, 1));
    assert_eq!(p.knights[1].tile(), (9, 7), "seat two in the far corner");
    // Each seat's screen shows the game's board and the yard, not another
    // game's board.
    let view = run.view_for_seat(2).unwrap();
    assert!(view.pastimes.round.is_some(), "seat two sees the yard");
    assert_eq!(
        view.pastimes.round.as_ref().unwrap().game,
        crate::rules::pastimes::Game::Powder
    );

    // The trap. Seat one walks their keg to the middle of the yard and
    // leaves it where seat two must pass — the two-tile-wide lane below
    // seat two's corner. Then seat two, walking out, is sooted by the
    // friend's blast.
    // Seat one: drop in place (corner 1,1), then step away down the ring.
    drop_and_clear(&mut run, 1);
    let p = run.pastimes.round.as_ref().unwrap().powder.as_ref().unwrap();
    assert!(p.bales.len() < ALL_BALES.len(), "the trap's keg burned bales");
    // Seat two is untouched so far: the cross stopped well short.
    assert!(p.alive().contains(&2), "seat two still standing");
    assert!(p.alive().contains(&1), "seat one walked clear of their own keg");

    // Now the friend-sooting: seat two drops their OWN keg at their feet
    // and stands on it. Seat one's round carries the shared yard, and the
    // friend's own blast soots them out of the heat — the trap works for
    // every knight in the one yard, not only the starter.
    run.tick_pastimes(&key(2, (0, 0), true));
    let mut sooted = false;
    let mut one_standing = false;
    // The heat may wind down the very tick the boom lands, so watch the
    // yard each tick from the drop until it goes.
    for _ in 0..u32::from(FUSE) + 6 {
        run.tick_pastimes(&key(2, (0, 0), false));
        let yard = run
            .pastimes
            .round
            .as_ref()
            .and_then(|r| r.powder.as_ref())
            .map(|p| (p.alive().clone(), p.kegs.len(), p.bales.len(), p.scorch.len()));
        if let Some((alive, kegs, bales, scorch)) = yard {
            if kegs > 0 || bales < ALL_BALES.len() || scorch > 0 {
                sooted = sooted || !alive.contains(&2);
                one_standing = one_standing || alive.contains(&1);
            }
        }
    }
    assert!(sooted, "seat two is sooted by the friend's keg");
    assert!(one_standing, "seat one still standing");
    for _ in 0..u32::from(FUSE) + u32::from(FIRE) + 8 {
        run.tick_pastimes(&key(2, (0, 0), false));
    }
    // ...and the last knight standing wins the heat.
    assert!(run.players[&1].laurel, "the last knight standing is crowned");
    assert!(run.players[&2].booby.is_some(), "the sooted friend wears the booby");
    let (_, _, said) = run.found.clone().unwrap();
    assert!(said.contains("WINS THE POWDER KEG"), "{said}");
    assert!(run.valid_snapshot());
}

#[test]
fn the_seventeenth_game_pays_and_maud_has_no_gossip_left() {
    // Game::ALL has grown past 16 entries; games_paid is a u32 now, so a
    // bit past the old u16's last one pays.
    let mut run = watch();
    let idx = crate::rules::pastimes::Game::Powder.index();
    assert!(idx >= 16, "Powder Keg sits past the old u16's last bit");
    assert!(idx < 32, "the bit fits the widened u32");
    run.home.countryside.games_paid = 1u32 << idx;
    run.maud_gossips_for_test();
    assert!(
        run.kingdom.moments.errand.is_none(),
        "the last game's first win paid: Maud has no gossip"
    );
    // The merlin test's own sentinel still works at the new width.
    run.home.countryside.games_paid = u32::MAX;
    run.maud_gossips_for_test();
    assert!(run.kingdom.moments.errand.is_none());
}

/// Film plates for review: `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve
/// write_keg_shots -- --ignored`. Start, two knights mid-heat with a lit keg,
/// the fire cross on screen, and the podium.
#[test]
#[ignore]
fn write_keg_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let frame = |run: &Run| crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
    let save = |name: &str, img: &crate::render::overworld::ink::Img| {
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = watch();
    run.players.retain(|&id, _| id <= 2);
    start_heat(&mut run, &[1, 2]);
    save("kegs-1-start", &frame(&run));
    // Two knights mid-heat, one keg lit with its fuse burning. Seat one
    // steps down to (1,2) before dropping, then walks the open ring west,
    // out of the cross.
    for _ in 0..12 {
        run.tick_pastimes(&key(1, (0, 1), false));
    }
    run.tick_pastimes(&key(1, (0, 0), true));
    for _ in 0..14 {
        run.tick_pastimes(&key(1, (-1, 0), false));
    }
    save("kegs-2-mid", &frame(&run));
    // The fire cross on screen: catch it while it burns (FIRE = 45).
    for _ in 0..u32::from(FUSE) - 14 + 3 {
        run.tick_pastimes(&key(1, (0, 1), false));
    }
    let p = run
        .pastimes
        .round
        .as_ref()
        .and_then(|r| r.powder.as_ref())
        .expect("the heat still runs");
    assert!(
        !p.flames.is_empty(),
        "the cross of fire is on screen for the shot"
    );
    save("kegs-3-firecross", &frame(&run));
    // The podium: the heat ends and the last knight standing is crowned.
    for _ in 0..4 * HZ {
        run.tick_pastimes(&key(1, (1, 0), false));
        if run.pastimes.round.is_none() {
            break;
        }
    }
    save("kegs-4-podium", &frame(&run));
}
