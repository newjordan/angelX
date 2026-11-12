//! What a player meets on the surface once the old places are off the map
//! (`crate::world::plan`): nothing sends a knight where no door goes, and a
//! save made on the old map comes up with every knight on open ground and
//! every door where it can be walked to. Each test holds on either plan; the
//! ones that bite are the blank-slate plan's (`ANGEL_REALM_PLAN=tabula`).

use crate::rules::exits::Entry;
use crate::rules::holdings::{self, Built, Deed, Holding, Holdings};
use crate::rules::kingdom::Return;
use crate::rules::layout::{Pack, TILE_UNITS, Tile};
use crate::rules::map;
use crate::rules::{RoomKind, Run};
use crate::world::plan;
use std::collections::BTreeMap;

fn home_run() -> Run {
    Run::at_home(41, 1, None, Default::default(), Default::default())
}

/// A tile of the surface a knight cannot stand on, inside the map's rim.
fn closed_tile(run: &Run) -> (i32, i32) {
    let geo = map::geography(&run.kingdom.wishes);
    let (cols, rows) = (geo.room.cols as i32, geo.room.rows as i32);
    (4..rows - 4)
        .flat_map(|y| (4..cols - 4).map(move |x| (x, y)))
        .find(|&(x, y)| geo.room.tile(x, y) != Tile::Floor)
        .expect("a surface with something standing on it")
}

fn at(tile: (i32, i32)) -> (f32, f32) {
    ((tile.0 as f32 + 0.5) * TILE_UNITS, (tile.1 as f32 + 0.5) * TILE_UNITS)
}

/// The run's surface as a saved map of another shape would carry it: one tile
/// of the kingdom's room unlike the map's own, so the next `raise_kingdom`
/// takes it for an old map and brings its knights home onto this one.
fn as_an_old_save(run: &mut Run) {
    let i = crate::rules::world::room_of(&run.dungeon, RoomKind::Kingdom).expect("the surface");
    let room = &mut run.dungeon.rooms[i];
    let was = room.tile(2, 2);
    room.set(2, 2, if was == Tile::Floor { Tile::Wall } else { Tile::Floor });
}

fn standing_on_open_ground(run: &Run, (x, y): (f32, f32)) -> bool {
    let geo = map::geography(&run.kingdom.wishes);
    geo.room.tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32) == Tile::Floor
}

#[test]
fn an_old_saves_knight_on_ground_that_is_closed_now_comes_up_on_open_ground() {
    let mut run = home_run();
    run.walk_kingdom(None);
    let closed = closed_tile(&run);
    {
        let hero = run.players.get_mut(&1).expect("a knight");
        (hero.x, hero.y) = at(closed);
    }
    as_an_old_save(&mut run);
    run.raise_kingdom();
    let hero = &run.players[&1];
    assert!(standing_on_open_ground(&run, (hero.x, hero.y)), "the knight stands at {:?}", (hero.x, hero.y));
}

#[test]
fn a_return_door_to_a_place_that_is_gone_is_a_door_a_knight_can_come_out_of() {
    let mut run = home_run();
    run.walk_kingdom(None);
    let closed = closed_tile(&run);
    let spot = at(closed);
    run.kingdom.return_to = Some(Return {
        x: spot.0,
        y: spot.1,
        name: "A Place That Is Gone".into(),
        root: RoomKind::Village,
        party: BTreeMap::from([(1, spot)]),
    });
    as_an_old_save(&mut run);
    run.raise_kingdom();
    let back = run.kingdom.return_to.clone().expect("the return door is kept");
    assert!(standing_on_open_ground(&run, (back.x, back.y)), "the door is at {:?}", (back.x, back.y));
    assert!(back.party.values().all(|&spot| standing_on_open_ground(&run, spot)), "the party's spots {:?}", back.party);
    // And walking out of the place puts the knight there.
    run.leave_kingdom_place();
    let hero = &run.players[&1];
    assert_eq!(run.room().kind, RoomKind::Kingdom);
    assert!(standing_on_open_ground(&run, (hero.x, hero.y)));
}

#[test]
fn the_journal_of_where_a_knight_went_in_does_not_come_back_out_on_closed_ground() {
    let mut run = home_run();
    run.walk_kingdom(None);
    let kingdom = crate::rules::world::room_of(&run.dungeon, RoomKind::Kingdom).expect("the surface");
    let spot = at(closed_tile(&run));
    run.exits.entry = Some(Entry { at: kingdom, party: BTreeMap::from([(1, spot)]) });
    run.exits.outside.insert(1, Entry { at: kingdom, party: BTreeMap::from([(1, spot)]) });
    as_an_old_save(&mut run);
    run.raise_kingdom();
    let entry = run.exits.entry.clone().expect("the journal is kept");
    if plan::tabula() {
        assert!(entry.party.values().all(|&spot| standing_on_open_ground(&run, spot)), "{:?}", entry.party);
        assert!(run.exits.outside[&1].party.values().all(|&spot| standing_on_open_ground(&run, spot)));
    } else {
        // The full plan's journal is left exactly as it was written.
        assert_eq!(entry.party, BTreeMap::from([(1, spot)]));
        assert_eq!(run.exits.outside[&1].party, BTreeMap::from([(1, spot)]));
    }
}

#[test]
fn an_old_saves_errand_to_a_place_with_no_door_is_let_go_and_one_with_a_door_is_kept() {
    let mut run = home_run();
    run.walk_kingdom(None);
    run.kingdom.moments.errand = Some(RoomKind::LandOffice);
    as_an_old_save(&mut run);
    run.raise_kingdom();
    // The full plan keeps its Land Office; the blank slate has none.
    assert_eq!(run.kingdom.moments.errand.is_none(), plan::tabula());
    // The Delve's gate is a door on either map.
    run.kingdom.moments.errand = Some(RoomKind::Gate);
    as_an_old_save(&mut run);
    run.raise_kingdom();
    assert_eq!(run.kingdom.moments.errand, Some(RoomKind::Gate));
}

#[test]
fn a_gossip_or_a_wizard_never_sends_the_party_where_no_door_goes() {
    let mut run = home_run();
    run.home.residents.insert("maud".into());
    for paid in [0, 0b1, 0b1111, 0xFFFF, 0x3_FFFF, 0xF_FFFF] {
        run.home.countryside.games_paid = paid;
        run.kingdom.moments.errand = None;
        run.maud_gossips_for_test();
        if let Some(room) = run.kingdom.moments.errand {
            assert!(run.on_the_way(room), "Maud sent the party to {room:?} after {paid:#b} games paid");
        }
    }
    // Merlin, by the stair, names the first errand a door goes to.
    let (mx, my) = (crate::rules::merlin::MERLIN_AT.0 * TILE_UNITS, crate::rules::merlin::MERLIN_AT.1 * TILE_UNITS);
    let hero = run.players.get_mut(&1).expect("a knight");
    (hero.x, hero.y) = (mx, my);
    run.kingdom.moments.errand = None;
    run.greeted = 0;
    run.greet_merlin();
    if let Some(room) = run.kingdom.moments.errand {
        assert!(run.on_the_way(room), "Merlin sent the party to {room:?}");
    }
}

#[test]
fn the_delves_gate_is_on_the_way_and_the_full_plan_leaves_nothing_off_it() {
    let mut run = home_run();
    run.walk_kingdom(None);
    assert!(run.on_the_way(RoomKind::Gate));
    assert!(run.on_the_way(RoomKind::Village), "the Village is a trail from the courtyard");
    if plan::full() {
        for room in [RoomKind::Eyrie, RoomKind::LandOffice, RoomKind::ColosseumHall] {
            assert!(run.on_the_way(room));
        }
    } else {
        // Only the Delve's gate has a door on the surface.
        assert!(!run.on_the_way(RoomKind::LandOffice));
    }
}

#[test]
fn a_castle_is_raided_only_where_its_hall_is_on_the_map_and_a_floor_always() {
    let mut h = Holdings::default();
    let castle = Deed::Castle(0).key();
    let floor = Deed::Floor(Pack::Cavern).key();
    for key in [&castle, &floor] {
        h.owned.push(Holding { key: key.clone(), walls: 1, stars: 1, ..Holding::default() });
    }
    // A castle already under raiders in the save.
    h.owned[0].raid = 2;
    let _ = holdings::rent_day(&mut h, 1);
    let (c, f) = (&h.owned[0], &h.owned[1]);
    assert!(f.raid > 0, "a floor with low walls is raided on either plan");
    assert_eq!(c.raidable(), !plan::tabula());
    assert_eq!(c.raid == 0, plan::tabula(), "the castle's raid: {}", c.raid);
}

#[test]
fn the_storyteller_never_musters_a_band_against_a_castle_where_its_hall_is_off_the_map() {
    let built = || (0..holdings::SLOTS).map(|_| Built { id: "inn".into(), labour: holdings::ROOM_LABOUR }).collect::<Vec<_>>();
    let musters = |key: String| {
        let mut h = Holdings::default();
        h.owned.push(Holding { key, walls: holdings::WALLS, stars: 1, rooms: built(), ..Holding::default() });
        (0..40).any(|_| {
            let _ = holdings::rent_day(&mut h, 1);
            h.muster.is_some() || h.owned.iter().any(|w| w.raid > 0)
        })
    };
    assert!(musters(Deed::Floor(Pack::Cavern).key()), "a floor worth robbing is mustered against");
    assert_eq!(musters(Deed::Castle(0).key()), plan::full(), "a castle is, where its hall can be walked into");
}

#[test]
fn what_the_realm_owns_in_castles_stays_through_a_save_and_a_load() {
    let mut run = home_run();
    let ids: Vec<_> = crate::world::houses::ids().collect();
    for id in &ids {
        let key = Deed::Castle(id.0).key();
        assert_eq!(Deed::of_key(&key), Some(Deed::Castle(id.0)), "{key} names the same castle again");
        run.home.holdings.owned.push(Holding { key, walls: 10, stars: 1, coffer: 7, ..Holding::default() });
    }
    run.home.holdings.owned.push(Holding { key: Deed::Floor(Pack::Cavern).key(), walls: 10, stars: 1, ..Holding::default() });
    run.walk_kingdom(None);
    as_an_old_save(&mut run);
    run.raise_kingdom();
    for _ in 0..60 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.home.holdings.owned.len(), ids.len() + 1, "no deed is lost");
    let json = serde_json::to_value(&run.home.holdings).expect("saves");
    let back: Holdings = serde_json::from_value(json).expect("loads");
    assert_eq!(back.owned, run.home.holdings.owned);
    // Each castle's ground is on the map, inside it.
    for id in ids {
        let (tx, ty, tw, th) = crate::world::castles::of_house(id).foot;
        assert!(tx > 0 && ty > 0 && tx + tw < map::MAP_W && ty + th < map::MAP_H, "{:?}", (tx, ty, tw, th));
    }
}

/// A report, not a check: every room of the home floor no door leads to from
/// the Delve's gate on the blank-slate map, and each questline's teacher.
/// `ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib tabula_sweep_tests::report -- --ignored --nocapture`
#[test]
#[ignore = "a report"]
fn report_what_no_door_leads_to_and_who_teaches_there() {
    let mut run = home_run();
    run.walk_kingdom(None);
    let mut lost: Vec<String> = run
        .dungeon
        .rooms
        .iter()
        .map(|r| r.kind)
        .filter(|&k| !run.on_the_way(k))
        .map(|k| format!("{k:?}"))
        .collect();
    lost.sort();
    lost.dedup();
    println!("rooms on the home floor: {}; no door leads to {} of them: {lost:?}", run.dungeon.rooms.len(), lost.len());
    for q in &crate::rules::questline::book().lines {
        println!(
            "questline {:>10} teacher {:<22} room {:?} kingdom={} hall={:?} on_the_way={}",
            q.power,
            q.teacher,
            q.kind(),
            q.is_kingdom(),
            q.hall,
            run.on_the_way(q.kind())
        );
    }
}
