//! Nooks below ground: every floor hangs one off a fight room without
//! taking a monster away, and each nook's business is played by the whole
//! party at once, pays into what the knights carry, and raises monsters
//! when it goes wrong.
use super::*;
use crate::rules::{Floor, Pack, Rng, fortune, layout};

const PACKS: [Pack; 6] = [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown];

fn walkable(room: &Room, x: f32, y: f32) -> bool {
    matches!(room.tile(x as i32, y as i32), Tile::Floor | Tile::Door | Tile::Stairs)
}

/// The tiles a knight can walk to from the nook's doorway.
fn reach(room: &Room) -> Vec<bool> {
    let door = (0..4).find(|&d| room.doors[d]).unwrap();
    let start = [(11, 0), (23, 6), (11, 13), (0, 6)][door];
    let mut seen = vec![false; room.cols * room.rows];
    let mut todo = vec![start];
    while let Some((x, y)) = todo.pop() {
        if x < 0 || y < 0 || x >= room.cols as i32 || y >= room.rows as i32 {
            continue;
        }
        let k = y as usize * room.cols + x as usize;
        if seen[k] || !walkable(room, x as f32, y as f32) {
            continue;
        }
        seen[k] = true;
        todo.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]);
    }
    seen
}

/// Every spot a nook asks a knight to stand at.
fn stations(kind: RoomKind) -> Vec<(f32, f32)> {
    match kind {
        RoomKind::Kitchen => vec![(12.0, 5.6), (10.2, 4.0), (13.8, 4.0)],
        RoomKind::Reading => (0..LECTERNS.len()).map(lectern_stand).collect(),
        RoomKind::Bellows => HANDLES.to_vec(),
        RoomKind::Ossuary => (0..PATH.len()).map(domino_at).collect(),
        RoomKind::Still => JARS.iter().map(|j| j.0).collect(),
        _ => (0..BELLS.len()).map(loft_under).collect(),
    }
}

fn roster(floor: &Floor) -> usize {
    floor.rooms.iter().map(|r| r.roster.len()).sum()
}

#[test]
fn every_floor_below_hangs_its_nook_and_takes_no_monster_away() {
    for pack in PACKS {
        for (depth, seed) in (1..DEEPEST).flat_map(|d| (0..8u64).map(move |s| (d, s))) {
            let mut rng = Rng::new(97 + seed * 131 + u64::from(depth));
            let mut floor = layout::floor_for(fortune::Mode::LongWayDown, depth, pack, &mut rng);
            let (rooms, monsters) = (floor.rooms.len(), roster(&floor));
            assert!(add_nook(&mut floor), "{pack:?} {depth}: no nook");
            assert!(!add_nook(&mut floor), "one a floor");
            assert_eq!(floor.rooms.len(), rooms + 1);
            assert_eq!(roster(&floor), monsters, "{pack:?} {depth}: monsters kept");
            let i = floor.rooms.iter().position(|r| r.kind.nook()).unwrap();
            let room = &floor.rooms[i];
            assert_eq!(room.kind, for_floor(pack, depth));
            // A dead end: one doorway, onto a room of the floor; the
            // bellows' never north, where their forge stands.
            assert_eq!(room.doors.iter().filter(|&&d| d).count(), 1);
            assert!(room.kind != RoomKind::Bellows || !room.doors[0]);
            let door = (0..4).find(|&d| room.doors[d]).unwrap();
            let host = floor.neighbour(i, door).expect("its doorway leads somewhere");
            assert!(matches!(floor.rooms[host].kind, RoomKind::Fight | RoomKind::Hall | RoomKind::Treasure | RoomKind::Start | RoomKind::Sanctuary));
            assert!(floor.valid_secret());
            // Every station is reachable from the doorway, and every
            // knight arriving through it lands on open floor.
            let seen = reach(room);
            for (x, y) in stations(room.kind) {
                assert!(seen[y as usize * room.cols + x as usize], "{:?}: {x},{y} unreachable", room.kind);
            }
            for offset in [-3.6f32, -1.2, 1.2, 3.6] {
                let (x, y) = match door {
                    0 => (room.width() / 2.0 + offset, 3.3),
                    1 => (room.width() - 3.3, room.height() / 2.0 + offset),
                    2 => (room.width() / 2.0 + offset, room.height() - 3.3),
                    _ => (3.3, room.height() / 2.0 + offset),
                };
                assert!(Grid { room, barred: false }.clear(x, y, HERO_RADIUS, Mover::Hero), "{:?} door {door} arrival", room.kind);
            }
        }
    }
}

#[test]
fn every_pack_keeps_two_nooks_and_every_nook_lives_in_two_packs() {
    for kind in KINDS {
        assert_eq!(PACKS.iter().filter(|&&p| of_pack(p).contains(&kind)).count(), 2, "{kind:?}");
    }
}

#[test]
fn going_down_readies_the_floors_nook() {
    let mut run = Run::new(41, 1, None);
    run.descend_for_test();
    let nook = run.nook.clone().expect("floor two has a nook");
    assert!(run.dungeon.rooms[nook.room].kind.nook());
    assert!(run.valid_snapshot());
    let friend = run.wire_copy();
    assert_eq!(friend.nook, run.nook);
}

/// A party of `seats` standing in a nook of `kind`, off a far cell.
fn in_nook(kind: RoomKind, seats: u32) -> Run {
    let mut run = Run::new(41, 1, None);
    for id in 2..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.enemies.clear();
    let room = nooks::room(kind, (12, 12), 3);
    run.dungeon.rooms.push(room);
    let index = run.dungeon.rooms.len() - 1;
    run.nook = Some(Nook {
        room: index,
        pot: SCOOPS,
        mimics: if kind == RoomKind::Reading { 0b0100_0110 } else { 0 },
        ..Default::default()
    });
    run.enter_for_test(index);
    run.enemies.clear();
    for hero in run.players.values_mut() {
        hero.invulnerable = 0;
    }
    run
}

fn put(run: &mut Run, id: u32, (x, y): (f32, f32)) {
    let h = run.players.get_mut(&id).unwrap();
    (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
}

fn e(id: u32) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, Input { interact: true, ..Default::default() })])
}

fn press(run: &mut Run, id: u32) {
    run.step(&e(id));
    run.step(&BTreeMap::new());
}

fn nook(run: &Run) -> &Nook {
    run.nook.as_ref().unwrap()
}

fn gold(run: &Run, id: u32) -> u32 {
    run.players[&id].carried.get(Spoil::Gold)
}

#[test]
fn the_kitchen_pays_scoops_behind_his_back_and_ladles_whoever_moves() {
    let mut run = in_nook(RoomKind::Kitchen, 2);
    put(&mut run, 1, (12.0, 5.8));
    put(&mut run, 2, (14.5, 6.0));
    press(&mut run, 1);
    assert_eq!(nook(&run).pot, SCOOPS - 1);
    assert_eq!(gold(&run, 1), kitchen::SCOOP_GOLD);
    // Wait for him to turn, then fidget.
    while cook(nook(&run)) != Cook::Looking {
        run.step(&BTreeMap::new());
    }
    let hp = run.players[&2].hp;
    run.step(&BTreeMap::from([(2, Input { move_x: 1, ..Default::default() })]));
    assert!(run.players[&2].hp < hp, "ladled");
    assert_eq!(run.enemies.len(), 1, "a cousin comes running");
    assert_eq!(run.enemies[0].kind, EnemyKind::Goblin);
    assert!(run.valid_snapshot());
    // A still knight is not caught.
    let still = run.players[&1].hp;
    run.step(&BTreeMap::new());
    assert_eq!(run.players[&1].hp, still);
}

#[test]
fn the_empty_pot_crowns_the_best_scooper() {
    let mut run = in_nook(RoomKind::Kitchen, 2);
    run.nook.as_mut().unwrap().pot = 1;
    run.nook.as_mut().unwrap().score.insert(2, 3);
    put(&mut run, 1, (12.0, 5.8));
    press(&mut run, 1);
    assert!(nook(&run).done);
    assert!(run.players[&2].laurel && !run.players[&1].laurel);
    assert!(run.found.as_ref().unwrap().2.contains("ATE MOST"));
}

#[test]
fn the_quiet_room_pays_pages_and_its_mimics_bite() {
    let mut run = in_nook(RoomKind::Reading, 2);
    put(&mut run, 1, lectern_stand(0));
    press(&mut run, 1);
    assert_eq!(gold(&run, 1), reading::PAGE_GOLD);
    assert_eq!(run.found.as_ref().unwrap().2, reading::TITLES[0]);
    let hp = run.players[&2].hp;
    put(&mut run, 2, lectern_stand(1));
    press(&mut run, 2);
    assert!(run.players[&2].hp < hp, "bitten");
    assert!(run.enemies.iter().any(|e| e.kind == EnemyKind::Mimic));
    // Read the rest.
    for i in 2..LECTERNS.len() {
        put(&mut run, 1, lectern_stand(i));
        press(&mut run, 1);
    }
    assert!(nook(&run).done);
    assert!(run.valid_snapshot());
}

#[test]
fn the_floor_picks_three_mimics_without_dice() {
    let mut rng = Rng::new(5);
    let floor = layout::floor_for(fortune::Mode::LongWayDown, 2, Pack::Archive, &mut rng);
    let mask = reading::mimics(&floor);
    assert_eq!(mask.count_ones(), reading::MIMICS);
    assert_eq!(mask, reading::mimics(&floor));
}

#[test]
fn the_bellows_want_pumping_in_turn_and_temper_every_arm() {
    let mut run = in_nook(RoomKind::Bellows, 2);
    for id in [1, 2] {
        let h = run.players.get_mut(&id).unwrap();
        h.weapon = Weapon::Bow;
        h.forged = None;
        h.temper = 0;
    }
    put(&mut run, 1, HANDLES[0]);
    put(&mut run, 2, HANDLES[1]);
    press(&mut run, 1);
    let heat = nook(&run).heat;
    press(&mut run, 1);
    assert!(run.found.as_ref().unwrap().2.contains("OTHER SIDE"));
    assert!(nook(&run).heat <= heat, "the same side twice does nothing");
    let mut turn = 2;
    while !nook(&run).done {
        press(&mut run, turn);
        turn = 3 - turn;
    }
    assert_eq!(run.enemies.iter().filter(|e| e.kind == EnemyKind::Imp).count(), 2, "the heat woke imps");
    assert!(run.players.values().all(|h| h.temper == 1));
    assert!(run.valid_snapshot());
}

#[test]
fn an_empty_gap_stalls_the_dominoes_and_raises_bones() {
    let mut run = in_nook(RoomKind::Ossuary, 1);
    put(&mut run, 1, domino_at(0));
    press(&mut run, 1);
    assert!(nook(&run).running);
    for _ in 0..(GAPS[0] as u32 + 2) * ossuary::FALL {
        run.step(&BTreeMap::new());
    }
    assert!(!nook(&run).running && nook(&run).stalled > 0);
    assert_eq!(run.enemies.iter().filter(|e| e.kind == EnemyKind::Skeleton).count(), 2);
    for _ in 0..ossuary::STAND_AGAIN {
        run.step(&BTreeMap::new());
    }
    assert_eq!(nook(&run).fallen, 0, "the line stands again");
}

#[test]
fn knights_in_the_gaps_carry_the_line_to_the_coffer() {
    let mut run = in_nook(RoomKind::Ossuary, 4);
    put(&mut run, 4, domino_at(0));
    press(&mut run, 4);
    for (k, &g) in GAPS.iter().enumerate() {
        put(&mut run, k as u32 + 1, domino_at(g));
    }
    for _ in 0..PATH.len() as u32 * ossuary::FALL + 4 {
        run.step(&BTreeMap::new());
    }
    assert!(nook(&run).done);
    assert!(run.enemies.is_empty());
    for id in 1..=4 {
        assert_eq!(gold(&run, id), ossuary::COFFER_GOLD);
    }
    assert!(nook(&run).score.values().all(|&n| n == 1));
    assert!(run.valid_snapshot());
}

#[test]
fn the_still_brews_from_what_a_knight_carries() {
    let mut run = in_nook(RoomKind::Still, 2);
    put(&mut run, 1, JARS[0].0);
    press(&mut run, 1);
    assert_eq!(run.found.as_ref().unwrap().2, "NO BONE ON YOU.");
    {
        let h = run.players.get_mut(&1).unwrap();
        h.carried.add(Spoil::Bone, 2);
        h.carried.add(Spoil::Wax, 2);
        h.hand.clear();
    }
    press(&mut run, 1);
    put(&mut run, 1, JARS[1].0);
    press(&mut run, 1);
    assert_eq!(run.players[&1].hand, vec!["potion".to_string()]);
    assert_eq!(run.players[&1].carried.get(Spoil::Bone), 1);
    assert_eq!(run.players[&1].carried.get(Spoil::Wax), 1);
    // The same jar twice: a frog.
    run.players.get_mut(&1).unwrap().carried.add(Spoil::Wax, 1);
    press(&mut run, 1);
    press(&mut run, 1);
    assert!(run.players[&1].frog());
    // Two brews a floor.
    run.players.get_mut(&1).unwrap().carried.add(Spoil::Wax, 1);
    press(&mut run, 1);
    assert!(run.found.as_ref().unwrap().2.contains("ENOUGH"));
    assert!(run.valid_snapshot());
}

#[test]
fn every_third_brew_burps_a_slime() {
    let mut run = in_nook(RoomKind::Still, 2);
    for id in [1, 2] {
        let h = run.players.get_mut(&id).unwrap();
        h.carried.add(Spoil::Bone, 4);
        h.carried.add(Spoil::Ore, 4);
    }
    for (id, n) in [(1, 0), (1, 1), (2, 0)] {
        let _ = n;
        put(&mut run, id, JARS[0].0);
        press(&mut run, id);
        put(&mut run, id, JARS[2].0);
        press(&mut run, id);
    }
    assert_eq!(run.enemies.iter().filter(|e| e.kind == EnemyKind::Slime).count(), 1);
    assert_eq!(run.players[&1].bombs.min(1), 1);
}

fn ring(run: &mut Run, id: u32, b: u8) {
    put(run, id, loft_under(usize::from(b)));
    press(run, id);
}

#[test]
fn the_loft_wants_its_tune_rung_back_and_a_wrong_bell_calls_a_wraith() {
    let mut run = in_nook(RoomKind::Loft, 2);
    // He rings first; nothing counts until he is done.
    ring(&mut run, 1, SEQS[0][0]);
    assert_eq!(nook(&run).pos, 0);
    while show(nook(&run)) != Show::YourTurn {
        run.step(&BTreeMap::new());
    }
    ring(&mut run, 1, SEQS[0][0]);
    ring(&mut run, 2, SEQS[0][1]);
    assert_eq!(nook(&run).pos, 2);
    ring(&mut run, 1, (SEQS[0][2] + 1) % 4);
    assert_eq!(nook(&run).pos, 0);
    assert_eq!(run.enemies.iter().filter(|e| e.kind == EnemyKind::Wraith).count(), 1);
    assert!(run.valid_snapshot());
}

#[test]
fn three_tunes_rung_sing_the_hymn() {
    let mut run = in_nook(RoomKind::Loft, 2);
    run.players.get_mut(&2).unwrap().hp = 1;
    for (r, seq) in SEQS.iter().enumerate() {
        while show(nook(&run)) != Show::YourTurn {
            run.step(&BTreeMap::new());
        }
        assert_eq!(usize::from(nook(&run).round), r);
        for (k, &b) in seq.iter().enumerate() {
            ring(&mut run, 1 + (k % 2) as u32, b);
        }
    }
    assert!(nook(&run).done);
    assert_eq!(run.players[&2].hp, run.players[&2].max_hp);
    assert_eq!(gold(&run, 1), loft::HYMN_GOLD);
    assert!(run.valid_snapshot());
}

/// Every nook in play: `ANGEL_ARENA_SHOTS=<dir> cargo test
/// write_nook_shots -- --ignored`.
#[test]
#[ignore]
fn write_nook_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("nooks");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    // Kitchen: scooping while he stirs, then caught while he looks.
    let mut k = in_nook(RoomKind::Kitchen, 3);
    put(&mut k, 1, (12.0, 5.8));
    put(&mut k, 2, (10.0, 6.5));
    put(&mut k, 3, (15.0, 7.0));
    press(&mut k, 1);
    save(&k, "a-kitchen-stirring");
    while cook(nook(&k)) != Cook::Turning {
        k.step(&BTreeMap::new());
    }
    save(&k, "b-kitchen-turning");
    while cook(nook(&k)) != Cook::Looking {
        k.step(&BTreeMap::new());
    }
    k.step(&BTreeMap::from([(3, Input { move_x: -1, ..Default::default() })]));
    for _ in 0..12 {
        k.step(&BTreeMap::new());
    }
    save(&k, "c-kitchen-caught");
    // Quiet Room: one read, one mimic.
    let mut r = in_nook(RoomKind::Reading, 2);
    put(&mut r, 1, lectern_stand(0));
    press(&mut r, 1);
    put(&mut r, 2, lectern_stand(1));
    press(&mut r, 2);
    for _ in 0..20 {
        r.step(&BTreeMap::new());
    }
    put(&mut r, 1, lectern_stand(4));
    r.step(&BTreeMap::new());
    save(&r, "d-quiet-room");
    // Bellows, half hot.
    let mut b = in_nook(RoomKind::Bellows, 2);
    put(&mut b, 1, HANDLES[0]);
    put(&mut b, 2, HANDLES[1]);
    for k in 0..6 {
        press(&mut b, 1 + (k % 2));
    }
    save(&b, "e-bellows-pumping");
    // Dominoes falling, a knight waiting in the first gap.
    let mut o = in_nook(RoomKind::Ossuary, 2);
    put(&mut o, 1, domino_at(0));
    press(&mut o, 1);
    put(&mut o, 2, domino_at(GAPS[0]));
    put(&mut o, 1, (10.0, 6.0));
    for _ in 0..8 * ossuary::FALL {
        o.step(&BTreeMap::new());
    }
    save(&o, "f-dominoes-falling");
    // Still: a first jar over a knight's head.
    let mut s = in_nook(RoomKind::Still, 2);
    for id in [1, 2] {
        let h = s.players.get_mut(&id).unwrap();
        h.carried.add(Spoil::Bone, 2);
        h.carried.add(Spoil::Wax, 2);
    }
    put(&mut s, 1, JARS[0].0);
    press(&mut s, 1);
    put(&mut s, 2, JARS[1].0);
    s.step(&BTreeMap::new());
    save(&s, "g-still");
    // Loft: the ghost ringing, then the party's turn.
    let mut l = in_nook(RoomKind::Loft, 2);
    put(&mut l, 1, loft_under(0));
    put(&mut l, 2, loft_under(3));
    for _ in 0..loft::REST * 2 + 6 {
        l.step(&BTreeMap::new());
    }
    save(&l, "h-loft-listen");
    while show(nook(&l)) != Show::YourTurn {
        l.step(&BTreeMap::new());
    }
    ring(&mut l, 1, SEQS[0][0]);
    save(&l, "i-loft-ringing");
}
