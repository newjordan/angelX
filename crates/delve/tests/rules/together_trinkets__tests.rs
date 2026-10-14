//! The kingdom's little things: bell, well, bench, bucket, and a bow.
use super::*;
use crate::render::overworld::kingdom as geography;

fn kingdom() -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.walk_kingdom(Some(geography::keep_stand()));
    r
}

fn press(id: u32) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, Input { interact: true, ..Default::default() })])
}

fn stand_at(r: &mut Run, id: u32, (x, y): (i32, i32)) {
    let h = r.players.get_mut(&id).unwrap();
    (h.x, h.y) = ((x as f32 + 0.5) * TILE_UNITS, (y as f32 + 0.5) * TILE_UNITS);
}

#[test]
fn every_thing_stands_on_open_ground_in_the_kingdom() {
    let r = kingdom();
    let geo = geography::geography(&r.kingdom.wishes);
    let all = things(&geo);
    assert_eq!(all.len(), PLACED.len(), "{all:?}");
    for (_, (x, y)) in all {
        assert_eq!(geo.room.tile(x, y), Tile::Floor);
    }
}

#[test]
fn the_bell_tolls_the_well_tells_and_the_bench_mends() {
    let mut r = kingdom();
    r.players.retain(|&id, _| id == 1);
    let geo = geography::geography(&r.kingdom.wishes);
    let all = things(&geo);
    let find = |t: Thing| all.iter().find(|(k, _)| *k == t).unwrap().1;
    stand_at(&mut r, 1, find(Thing::Bell));
    r.step(&press(1));
    assert!(r.sounds.iter().any(|s| *s == "circle_toll"));
    r.step(&BTreeMap::new());
    stand_at(&mut r, 1, find(Thing::Well));
    r.step(&press(1));
    assert_eq!(r.found.as_ref().unwrap().2, r.kingdom.moments.told());
    r.step(&BTreeMap::new());
    stand_at(&mut r, 1, find(Thing::Bench));
    r.players.get_mut(&1).unwrap().hp = 10;
    r.step(&press(1));
    assert!(r.players[&1].sitting);
    for _ in 0..HZ * 3 {
        r.step(&BTreeMap::new());
    }
    assert!(r.players[&1].hp > 10, "a bench mends");
    r.step(&BTreeMap::from([(1, Input { move_x: 1, ..Default::default() })]));
    assert!(!r.players[&1].sitting, "a step stands you up");
    assert!(r.valid_snapshot());
}

#[test]
fn a_bow_answered_is_a_huzzah() {
    let mut r = kingdom();
    r.join_seat(2, "Two");
    let geo = geography::geography(&r.kingdom.wishes);
    // Open ground away from any door or thing.
    let spot = (0..geo.room.rows as i32)
        .flat_map(|y| (0..geo.room.cols as i32).map(move |x| (x, y)))
        .find(|&(x, y)| {
            (0..3).all(|k| geo.room.tile(x + k, y) == Tile::Floor)
                && geo.sites.iter().all(|s| (s.stand.0 - x).abs() + (s.stand.1 - y).abs() > 8)
                && things(&geo).iter().all(|(_, p)| (p.0 - x).abs() + (p.1 - y).abs() > 8)
                && crate::rules::classes::stones(&geo).iter().all(|(_, p)| (p.0 - x).abs() + (p.1 - y).abs() > 8)
        })
        .unwrap();
    stand_at(&mut r, 1, spot);
    stand_at(&mut r, 2, (spot.0 + 1, spot.1));
    let score = r.score;
    r.step(&press(1));
    r.step(&BTreeMap::new());
    r.step(&press(2));
    assert_eq!(r.found.as_ref().map(|f| f.2.as_str()), Some("HUZZAH!"));
    assert!(r.score > score);
}

#[test]
fn a_knight_in_the_stocks_takes_tomatoes_from_friends() {
    let mut r = kingdom();
    r.join_seat(2, "Two");
    let geo = geography::geography(&r.kingdom.wishes);
    let (_, stocks) = things(&geo).into_iter().find(|(t, _)| *t == Thing::Stocks).unwrap();
    stand_at(&mut r, 2, stocks);
    r.step(&press(2));
    assert!(r.players[&2].stocked);
    let at = (r.players[&2].x, r.players[&2].y);
    // Locked in: walking goes nowhere.
    for _ in 0..10 {
        r.step(&BTreeMap::from([(2, Input { move_x: 1, ..Default::default() })]));
    }
    assert_eq!((r.players[&2].x, r.players[&2].y), at);
    // A friend's shot from the west is a tomato.
    let hp = r.players[&2].hp;
    let (hx, hy) = at;
    {
        let h = r.players.get_mut(&1).unwrap();
        (h.x, h.y) = (hx - 3.0 * TILE_UNITS, hy);
        h.weapon = Weapon::Bow;
    }
    for _ in 0..30 {
        r.step(&BTreeMap::from([(1, Input { fire: true, aim_x: 1, ..Default::default() })]));
        if r.players[&2].splats > 0 {
            break;
        }
    }
    assert!(r.players[&2].splats > 0, "splat");
    assert_eq!(r.players[&2].hp, hp, "tomatoes never hurt");
    // E again lets them out.
    r.step(&BTreeMap::new());
    r.step(&press(2));
    assert!(!r.players[&2].stocked);
    assert!(r.valid_snapshot());
}

#[test]
fn an_honked_goose_gives_chase_and_pecks() {
    let mut r = kingdom();
    r.players.retain(|&id, _| id == 1);
    let geo = geography::geography(&r.kingdom.wishes);
    let (_, green) = things(&geo).into_iter().find(|(t, _)| *t == Thing::Goose).unwrap();
    stand_at(&mut r, 1, green);
    for _ in 0..HZ * 2 {
        r.step(&BTreeMap::new());
    }
    r.step(&press(1));
    assert!(r.kingdom.moments.goose.is_some(), "honked");
    let mut pecked = false;
    for _ in 0..HZ * 3 {
        r.step(&BTreeMap::new());
        if r.found.as_ref().is_some_and(|f| f.2.contains("PECKED")) {
            pecked = true;
            break;
        }
    }
    assert!(pecked, "the goose catches a knight who stands still");
    assert!(r.valid_snapshot());
}

#[test]
fn the_coach_rides_round_the_realm_and_back() {
    let mut r = kingdom();
    r.players.retain(|&id, _| id == 1);
    let geo = geography::geography(&r.kingdom.wishes);
    let posts: Vec<(i32, i32)> = things(&geo)
        .into_iter()
        .filter(|(t, _)| *t == Thing::Coach)
        .map(|(_, p)| p)
        .collect();
    assert!(posts.len() >= 5, "{posts:?}");
    stand_at(&mut r, 1, posts[0]);
    for (k, _) in posts.iter().enumerate() {
        r.step(&press(1));
        r.step(&BTreeMap::new());
        let next = posts[(k + 1) % posts.len()];
        let h = &r.players[&1];
        let tile = ((h.x / TILE_UNITS) as i32, (h.y / TILE_UNITS) as i32);
        assert!((tile.0 - next.0).abs() <= 1 && (tile.1 - next.1).abs() <= 2, "post {k}: {tile:?} vs {next:?}");
        assert_eq!(geo.room.tile(tile.0, tile.1), Tile::Floor, "lands on open ground");
        // Step back onto the post for the next ride.
        stand_at(&mut r, 1, next);
    }
}

#[test]
fn a_quintain_beaten_hard_thwacks_back() {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    let yard = r.dungeon.rooms.iter().position(|room| room.kind == RoomKind::Yard).unwrap();
    r.enter_for_test(yard);
    r.step(&BTreeMap::new());
    let (x, y) = {
        let dummy = r.enemies.iter().find(|e| e.kind == EnemyKind::Dummy).expect("a quintain");
        (dummy.x, dummy.y)
    };
    {
        let h = r.players.get_mut(&1).unwrap();
        (h.x, h.y) = (x + 1.5, y);
    }
    let hp = r.players[&1].hp;
    let before = r.players[&1].x;
    r.enemies.iter_mut().find(|e| e.kind == EnemyKind::Dummy).unwrap().hp -= 160;
    r.step(&BTreeMap::new());
    assert_eq!(r.found.as_ref().map(|f| f.2.as_str()), Some("THWACK!"));
    assert!(r.players[&1].x > before, "shoved away from the post");
    assert_eq!(r.players[&1].hp, hp, "no harm");
}

#[test]
fn a_scratch_behind_the_ears_makes_tallow_purr() {
    let mut r = Run::new(41, 1, None);
    let (hx, hy) = (r.players[&1].x, r.players[&1].y);
    r.cat = Some(crate::rules::cat::Cat {
        x: hx + 1.0,
        y: hy,
        fetching: None,
        carrying: None,
        hiss: 0,
        hissed: 0,
        left: false,
        moving: false,
        fetched: 0,
        stuck: 0,
        petted: 0,
        pet_held: Default::default(),
    });
    r.players.get_mut(&1).unwrap().hp = 10;
    r.pet_tallow(&press(1));
    assert_eq!(r.found.as_ref().map(|f| f.2.as_str()), Some("PURRR"));
    assert_eq!(r.players[&1].hp, 12);
    // Holding E is one pat.
    r.pet_tallow(&press(1));
    assert_eq!(r.players[&1].hp, 12);
}

#[test]
fn the_crier_sends_you_to_an_errand_and_the_arrow_follows() {
    let mut r = kingdom();
    r.players.retain(|&id, _| id == 1);
    let geo = geography::geography(&r.kingdom.wishes);
    let (_, crier) = things(&geo).into_iter().find(|(t, _)| *t == Thing::Crier).unwrap();
    assert!(!r.open_errands().is_empty(), "a fresh realm has errands");
    stand_at(&mut r, 1, crier);
    r.step(&press(1));
    let said = r.found.clone().unwrap().2;
    assert!(said.starts_with("OYEZ!"), "{said}");
    let room = r.kingdom.moments.errand.expect("an errand is cried");
    let goal = r.kingdom_goal(1).unwrap();
    let site = geo.sites.iter().find(|s| s.room == room).unwrap();
    assert_eq!(goal.stand, site.stand, "the arrow points at the errand's place");
    // Getting there clears it.
    let index = geo.sites.iter().position(|s| s.room == room).unwrap();
    r.enter_kingdom_place(index);
    assert!(r.kingdom.moments.errand.is_none());
}

#[test]
fn once_met_the_dwarf_king_sends_the_knights_to_his_mines() {
    let mut r = kingdom();
    assert!(!r.open_errands().iter().any(|(room, _)| *room == RoomKind::MineHead));
    r.home.barony.court = crate::rules::barony::Court::Met;
    let errands = r.open_errands();
    assert_eq!(errands[0], (RoomKind::MineHead, "THE KING WANTS HIS MINES"));
    r.home.barony.done.push("workings".into());
    assert!(!r.open_errands().iter().any(|(room, _)| *room == RoomKind::MineHead));
}
