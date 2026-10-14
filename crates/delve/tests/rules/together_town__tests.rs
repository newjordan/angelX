//! The town's row: the square buys the realm's spare spoils by haggling,
//! the bathhouse scrubs a knight clean of one privy toll, and the smith
//! tempers and mends for the treasury's gold. Each room stands on the home
//! floor and behind a door in the kingdom.
use super::*;
use crate::rules::{home::Home, merchant::Shop, world};
use crate::render::overworld::kingdom as geography;

fn in_room(kind: RoomKind, seats: u32) -> Run {
    let mut run = Run::at_home(61, 1, None, Home::default(), Spoils::default());
    for id in 2..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.treasury.add(Spoil::Gold, 200);
    run.treasury.add(Spoil::Ore, 3);
    run.arrive(world::room_of(&run.dungeon, kind).unwrap(), (12.0, 11.0));
    run
}

fn put(run: &mut Run, id: u32, (x, y): (f32, f32)) {
    let h = run.players.get_mut(&id).unwrap();
    (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
}

fn e(id: u32) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, Input { interact: true, ..Default::default() })])
}

/// The cockpit's part: pay each asked purchase from the treasury, as
/// `settle_home` does.
fn pay(run: &mut Run) -> u32 {
    let mut spent = 0;
    for p in std::mem::take(&mut run.peddled) {
        if let Some(gold) = run.peddle_price(p) {
            let mut cost = Spoils::default();
            cost.add(Spoil::Gold, gold);
            run.treasury.take(&cost);
            spent += gold;
            run.peddle(p);
        }
    }
    spent
}

#[test]
fn the_town_stands_on_the_home_floor_behind_kingdom_doors() {
    let run = in_room(RoomKind::Market, 1);
    for kind in PLACES {
        let i = world::room_of(&run.dungeon, kind).expect("raised with the world");
        assert!(run.dungeon.rooms[i].doors.iter().any(|&d| d), "{kind:?} has a door");
    }
    // The market behind the Artisan Quarter's door, the smithy behind the
    // Smithy's, the bathhouse behind its own; each on a road.
    let geo = geography::geography(&[]);
    for (kind, name) in [(RoomKind::Market, "Artisan Quarter"), (RoomKind::Smithy, "Smithy"), (RoomKind::Bathhouse, "Bathhouse")] {
        let site = geo.sites.iter().find(|s| s.room == kind).unwrap_or_else(|| panic!("{kind:?} has no door"));
        assert_eq!(site.name, name);
        assert!(geography::is_road(&geo, site.stand));
    }
    assert!(!geo.sites.iter().any(|s| s.room == RoomKind::RealmHall && (s.name == "Smithy" || s.name == "Artisan Quarter")));
}

#[test]
fn the_smithy_door_in_the_kingdom_leads_to_old_dint() {
    let mut run = Run::at_home(61, 1, None, Home::default(), Spoils::default());
    run.walk_kingdom(Some(geography::keep_stand()));
    run.floor_plates.clear();
    let geo = geography::geography(&run.kingdom.wishes);
    let site = geo.sites.iter().position(|s| s.room == RoomKind::Smithy).unwrap();
    run.enter_kingdom_place(site);
    assert_eq!(run.room().kind, RoomKind::Smithy);
    assert!(run.valid_snapshot());
}

#[test]
fn haggling_climbs_to_a_sale_and_the_realm_settles_it() {
    let mut run = in_room(RoomKind::Market, 1);
    put(&mut run, 1, STALLS[0].at);
    run.step(&e(1));
    assert_eq!(run.town.haggles.get(&1), Some(&(0, 0)));
    // Hold a while: the offer climbs.
    for _ in 0..HAGGLE_STEP * 3 {
        run.step(&e(1));
    }
    assert_eq!(offer(0, run.town.haggles[&1].1), STALLS[0].base + 3);
    let friend = run.wire_copy();
    assert_eq!(friend.town, run.town);
    assert!(run.valid_snapshot());
    // Let go: sold.
    run.step(&BTreeMap::new());
    assert!(run.town.haggles.is_empty());
    let mark = sale_mark(0, 1);
    let gold = run.marks[&mark];
    assert_eq!(gold, STALLS[0].base + 3);
    let mut country = run.home.countryside.clone();
    let mut treasury = run.treasury.clone();
    let (changed, news) = countryside::settle(&mut country, &mut treasury, &run.marks);
    assert!(changed);
    assert_eq!(treasury.get(Spoil::Ore), 2);
    assert_eq!(treasury.get(Spoil::Gold), 200 + gold);
    assert!(news.iter().any(|n| n.contains("MAGDA")));
}

#[test]
fn haggling_too_long_shuts_the_stall() {
    let mut run = in_room(RoomKind::Market, 1);
    put(&mut run, 1, STALLS[0].at);
    for _ in 0..haggle_limit(0) + 2 {
        run.step(&e(1));
    }
    assert!(run.town.haggles.is_empty());
    assert!(run.town.shut[0] > 0, "the shutters are down");
    assert!(run.marks.keys().all(|k| !k.starts_with("country:sell")), "no sale");
    // A fresh press at a shut stall is refused.
    run.step(&BTreeMap::new());
    run.step(&e(1));
    assert!(run.town.haggles.is_empty());
    assert!(run.found.as_ref().unwrap().2.contains("SULKING"));
    for _ in 0..SHUT {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.town.shut[0], 0);
    assert!(run.valid_snapshot());
}

#[test]
fn nothing_to_sell_is_said_plainly() {
    let mut run = in_room(RoomKind::Market, 1);
    put(&mut run, 1, STALLS[1].at);
    run.step(&e(1));
    assert!(run.town.haggles.is_empty());
    assert_eq!(run.found.as_ref().unwrap().2, "NO WAX TO SELL.");
}

#[test]
fn a_soak_costs_five_gold_and_spares_the_next_privy_toll() {
    let mut run = in_room(RoomKind::Bathhouse, 2);
    put(&mut run, 1, (TUBS[1].0, TUBS[1].1 + 1.6));
    run.step(&e(1));
    assert_eq!(run.peddled.len(), 1);
    assert_eq!(run.peddled[0].shop, Shop::Bath);
    assert_eq!(pay(&mut run), BATH_GOLD);
    assert_eq!(run.players[&1].soak, SOAK);
    assert_eq!(tub_of(&run.players[&1]), Some(1));
    // A friend cannot climb into the same tub.
    put(&mut run, 2, (TUBS[1].0 + 0.5, TUBS[1].1 + 1.6));
    run.step(&BTreeMap::new());
    run.step(&e(2));
    assert!(run.peddled.is_empty());
    assert!(run.found.as_ref().unwrap().2.contains("TAKEN"));
    // The soaker stays in, walking or not, until the soak is done.
    for _ in 0..SOAK {
        run.step(&BTreeMap::from([(1, Input { move_x: 1, ..Default::default() })]));
    }
    assert!(run.players[&1].scrubbed);
    assert_eq!(run.players[&1].soak, 0);
    assert!(run.valid_snapshot());
    // Down a floor without the privy: the scrubbed knight is let off once.
    let full = run.players[&1].hp;
    run.unwashed_toll();
    assert_eq!(run.players[&1].hp, full);
    assert!(!run.players[&1].scrubbed);
    run.unwashed_toll();
    assert!(run.players[&1].hp < full);
}

#[test]
fn the_smith_tempers_the_arm_in_hand_and_refuses_bare_fists() {
    let mut run = in_room(RoomKind::Smithy, 1);
    put(&mut run, 1, AT_ANVIL);
    run.players.get_mut(&1).unwrap().weapon = Weapon::Fists;
    run.players.get_mut(&1).unwrap().forged = None;
    run.step(&e(1));
    assert!(run.peddled.is_empty());
    assert_eq!(run.found.as_ref().unwrap().2, "NO ARM TO TEMPER.");
    run.players.get_mut(&1).unwrap().weapon = Weapon::Bow;
    run.players.get_mut(&1).unwrap().temper = 0;
    run.step(&BTreeMap::new());
    run.step(&e(1));
    assert_eq!(pay(&mut run), TEMPER_GOLD);
    assert_eq!(run.players[&1].temper, 1);
    assert!(run.town.hammer.is_some());
    assert!(run.found.as_ref().unwrap().2.starts_with("CLANG!"));
    assert!(run.valid_snapshot());
}

#[test]
fn the_smith_mends_missing_kit_and_tests_the_helmet() {
    let mut run = in_room(RoomKind::Smithy, 1);
    put(&mut run, 1, AT_RACK);
    run.players.get_mut(&1).unwrap().owned = Default::default();
    run.step(&e(1));
    assert_eq!(pay(&mut run), MEND_GOLD);
    let h = &run.players[&1];
    assert!(h.owned.helmet && h.owned.body && h.owned.shield);
    assert!(run.home.wardrobe.knights.contains_key(&h.name), "kept for next time");
    assert!(run.town.ding.is_some());
    // Whole kit: nothing to mend, nothing charged.
    run.step(&BTreeMap::new());
    run.step(&e(1));
    assert!(run.peddled.is_empty());
    assert_eq!(run.found.as_ref().unwrap().2, "NOTHING TO MEND.");
}

#[test]
fn a_purchase_lapses_if_the_knight_walks_out_before_it_is_paid() {
    let mut run = in_room(RoomKind::Smithy, 1);
    put(&mut run, 1, AT_RACK);
    run.players.get_mut(&1).unwrap().owned = Default::default();
    run.step(&e(1));
    let market = world::room_of(&run.dungeon, RoomKind::Market).unwrap();
    run.arrive(market, (12.0, 11.0));
    assert_eq!(pay(&mut run), 0);
    assert!(!run.players[&1].owned.helmet);
}

/// The town in play: `ANGEL_ARENA_SHOTS=<dir> cargo test write_town_shots
/// -- --ignored`.
#[test]
#[ignore]
fn write_town_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("town");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut m = in_room(RoomKind::Market, 2);
    put(&mut m, 1, (12.0, 9.0));
    put(&mut m, 2, (14.0, 7.0));
    m.step(&BTreeMap::new());
    save(&m, "a-market");
    put(&mut m, 1, STALLS[0].at);
    for _ in 0..HAGGLE_STEP * 2 {
        m.step(&e(1));
    }
    save(&m, "b-haggle-low");
    for _ in 0..(STALLS[0].top - STALLS[0].base) * HAGGLE_STEP {
        m.step(&e(1));
    }
    save(&m, "c-haggle-top");
    for _ in 0..PATIENCE + 2 {
        m.step(&e(1));
    }
    m.step(&BTreeMap::new());
    save(&m, "d-shut");
    let mut b = in_room(RoomKind::Bathhouse, 2);
    put(&mut b, 1, (TUBS[1].0, TUBS[1].1 + 1.6));
    put(&mut b, 2, (TUBS[2].0, TUBS[2].1 + 1.8));
    b.step(&BTreeMap::new());
    save(&b, "e-bathhouse");
    b.step(&e(1));
    pay(&mut b);
    for _ in 0..HZ {
        b.step(&BTreeMap::new());
    }
    save(&b, "f-soaking");
    for _ in 0..SOAK {
        b.step(&BTreeMap::new());
    }
    save(&b, "g-scrubbed");
    let mut s = in_room(RoomKind::Smithy, 2);
    put(&mut s, 1, AT_ANVIL);
    put(&mut s, 2, (16.0, 8.0));
    s.players.get_mut(&1).unwrap().weapon = Weapon::Crossbow;
    s.step(&BTreeMap::new());
    save(&s, "h-smithy");
    s.step(&e(1));
    pay(&mut s);
    s.step(&BTreeMap::new());
    save(&s, "i-tempering");
    put(&mut s, 2, AT_RACK);
    s.players.get_mut(&2).unwrap().owned = Default::default();
    for _ in 0..HZ {
        s.step(&BTreeMap::new());
    }
    s.step(&e(2));
    pay(&mut s);
    s.step(&BTreeMap::new());
    save(&s, "j-mended-ding");
    // In the kingdom: the town's three doors.
    let mut k = Run::at_home(61, 1, None, Home::default(), Spoils::default());
    k.walk_kingdom(Some(geography::keep_stand()));
    k.floor_plates.clear();
    let geo = geography::geography(&k.kingdom.wishes);
    for (n, kind) in PLACES.into_iter().enumerate() {
        let site = geo.sites.iter().find(|s| s.room == kind).unwrap();
        let h = k.players.get_mut(&1).unwrap();
        (h.x, h.y) = ((site.stand.0 as f32 + 0.5) * TILE_UNITS, (site.stand.1 as f32 + 1.5) * TILE_UNITS);
        h.camera = Some((h.x, h.y - 1.0));
        k.step(&BTreeMap::new());
        save(&k, &format!("k-kingdom-{n}"));
    }
}
