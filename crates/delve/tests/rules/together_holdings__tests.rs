use super::*;
use super::testing::*;
use crate::realm::{Spoil, Spoils};
use crate::rules::home::{BUY_HOLD, Home};
use crate::rules::world;

// ── Buying ────────────────────────────────────────────────────────────────

#[test]
fn a_cleared_floor_is_bought_outright_from_the_realms_purse() {
    let mut h = cleared();
    let mut t = gold(7000);
    let out = settle(&mut h, &mut t, &marks(&[&buy_mark(crypt(), false)]));
    assert!(out.changed);
    assert_eq!(t.get(Spoil::Gold), 1000);
    let w = h.holding(&crypt_key()).expect("ours");
    assert_eq!((w.fief, w.walls, w.stars, w.coffer), (false, WALLS, 1, 0));
    assert_eq!(h.offered(crypt()), Err("OURS"));
    // Bought twice is bought once.
    settle(&mut h, &mut t, &marks(&[&buy_mark(crypt(), false)]));
    assert_eq!(h.owned.len(), 1);
    assert_eq!(t.get(Spoil::Gold), 1000);
}

#[test]
fn a_fief_is_cheaper_and_the_king_takes_his_tithe() {
    let mut h = cleared();
    let mut t = gold(2400);
    settle(&mut h, &mut t, &marks(&[&buy_mark(crypt(), true)]));
    assert_eq!(t.get(Spoil::Gold), 0);
    let key = crypt_key();
    assert!(h.holding(&key).unwrap().fief);
    // Its rent day pays the stars' rent (none at one star) less the tithe:
    // nothing to tithe yet.
    rent_day(&mut h, 1);
    assert_eq!(h.holding(&key).unwrap().coffer, 0);
    // With a room standing, the King takes his thirty first.
    let mut t = gold(1000);
    stage(&mut h, &key, "taproom");
    settle(&mut h, &mut t, &marks_of(&[aye()]));
    settle(&mut h, &mut t, &BTreeMap::from([(LABOUR.to_string(), ROOM_LABOUR)]));
    let rent = h.holding(&key).unwrap().rent();
    assert_eq!(rent, 20);
    rent_day(&mut h, 1);
    assert_eq!(h.holding(&key).unwrap().coffer, rent - TITHE.min(rent));
}

#[test]
fn an_uncleared_floor_and_an_unserved_castle_are_not_for_sale() {
    let mut h = Holdings::default();
    let mut t = gold(90_000);
    assert_eq!(h.offered(crypt()), Err("NOT CLEARED"));
    assert_eq!(h.offered(Deed::Castle(0)), Err("NEVER SERVED"));
    settle(&mut h, &mut t, &marks(&[&buy_mark(crypt(), false), &buy_mark(Deed::Castle(0), false)]));
    assert!(h.owned.is_empty());
    assert_eq!(t.get(Spoil::Gold), 90_000);
    // The realm's record opens them.
    let reclaimed = BTreeMap::from([(Pack::Crypt.name().to_string(), 1)]);
    let yards = BTreeMap::from([("sol".to_string(), 3)]);
    assert!(note_realm(&mut h, &reclaimed, &yards));
    assert!(!note_realm(&mut h, &reclaimed, &yards), "once");
    settle(&mut h, &mut t, &marks(&[&buy_mark(crypt(), false), &buy_mark(Deed::Castle(0), false)]));
    assert_eq!(h.owned.len(), 2);
    assert_eq!(t.get(Spoil::Gold), 90_000 - 6000 - 5000);
}

#[test]
fn a_short_purse_buys_nothing_and_says_so() {
    let mut h = cleared();
    let mut t = gold(100);
    let out = settle(&mut h, &mut t, &marks(&[&buy_mark(crypt(), false)]));
    assert!(h.owned.is_empty());
    assert_eq!(t.get(Spoil::Gold), 100);
    assert_eq!(out.said[0].0, "cant_afford");
    assert_eq!(buy_open(&h, crypt(), false, 100, &BTreeMap::new()), Err("PURSE SHORT"));
}

// ── Managing ──────────────────────────────────────────────────────────────

#[test]
fn rooms_come_by_the_foremans_petition_then_labour_ten_bays_three_of_a_family() {
    let mut h = cleared();
    let mut t = gold(50_000);
    let key = crypt_key();
    settle(&mut h, &mut t, &marks(&[&buy_mark(crypt(), false)]));
    let before = t.get(Spoil::Gold);
    stage(&mut h, &key, "smithy");
    settle(&mut h, &mut t, &marks_of(&[aye()]));
    assert_eq!(t.get(Spoil::Gold), before - 400);
    let w = h.holding(&key).unwrap();
    assert_eq!(w.rooms[0].id, "smithy");
    assert!(!w.rooms[0].standing());
    assert!(h.building().is_some());
    // A shift of play lays one course; the room stands at six.
    settle(&mut h, &mut t, &BTreeMap::from([(LABOUR.to_string(), ROOM_LABOUR - 1)]));
    assert!(!h.holding(&key).unwrap().rooms[0].standing());
    let out = settle(&mut h, &mut t, &BTreeMap::from([(LABOUR.to_string(), 1)]));
    assert!(h.holding(&key).unwrap().rooms[0].standing());
    assert!(out.said.iter().any(|(_, l)| l.contains("Smithy stands")));
    // A family has three rooms, then no more: a fourth smithy petition is
    // refused by the open gate.
    for room in ["fletcher", "armory"] {
        stage(&mut h, &key, room);
        settle(&mut h, &mut t, &marks_of(&[aye()]));
    }
    stage(&mut h, &key, "smithy");
    let out = settle(&mut h, &mut t, &marks_of(&[aye()]));
    assert!(out.said.iter().any(|(_, l)| l.contains("ALREADY BUILT")), "an honest refusal");
    let ids: Vec<_> = h.holding(&key).unwrap().rooms.iter().map(|r| r.id.clone()).collect();
    assert_eq!(ids, ["smithy", "fletcher", "armory"]);
    // Ten bays, of the fifteen rooms.
    for room in ["taproom", "brewery", "inn", "shrine", "infirmary", "belltower", "stall"] {
        stage(&mut h, &key, room);
        settle(&mut h, &mut t, &marks_of(&[aye()]));
    }
    assert_eq!(h.holding(&key).unwrap().rooms.len(), SLOTS);
    stage(&mut h, &key, "counting");
    let out = settle(&mut h, &mut t, &marks_of(&[aye()]));
    assert!(out.said.iter().any(|(_, l)| l.contains("BAYS FULL")));
}

#[test]
fn the_foreman_drafts_three_petitions_at_a_rent_day_and_his_quotes_ride_the_deal_counter() {
    let mut h = cleared();
    let mut t = gold(50_000);
    settle(&mut h, &mut t, &marks(&[&buy_mark(crypt(), false)]));
    // No ledge without a rent day.
    assert!(h.foreman.queue.is_empty());
    rent_day(&mut h, 1);
    assert_eq!(h.foreman.queue.len(), crate::rules::foreman::QUEUE);
    // The deals turn through the families in their own order: taproom,
    // smithy, shrine.
    let rooms: Vec<&str> = h.foreman.queue.iter().map(|p| p.room.as_str()).collect();
    assert_eq!(rooms, ["taproom", "smithy", "shrine"]);
    assert!(h.foreman.queue.iter().all(|p| p.gold == 400), "the first three at the list");
    // Answer them all: the next rent day brings the next three, and the
    // fifth deal (the second of the second round's third) is padded.
    settle(&mut h, &mut t, &BTreeMap::from([(aye(), 3)]));
    rent_day(&mut h, 1);
    let rooms: Vec<&str> = h.foreman.queue.iter().map(|p| p.room.as_str()).collect();
    assert_eq!(rooms, ["stall", "barracks", "brewery"]);
    // Deals 3, 4 and 5: the barracks quote is the fifth deal, padded ten
    // in the hundred.
    let barracks = &h.foreman.queue[1];
    assert_eq!(barracks.gold, 400 + 40, "padded: {}", barracks.says);
    // His line for an honest deal, and for a padded one.
    assert_ne!(h.foreman.queue[0].says, barracks.says);
}

#[test]
fn every_seventh_deal_is_the_dud_the_neighbours_like_least() {
    let mut h = cleared();
    let mut t = gold(50_000);
    settle(&mut h, &mut t, &marks(&[&buy_mark(crypt(), false)]));
    // An inn standing: a smithy beside it is the room the neighbours like
    // least (NOISY, -10).
    h.owned[0].rooms.push(holdings::Built { id: "inn".into(), labour: ROOM_LABOUR });
    let mut deals = Vec::new();
    while deals.len() < 7 {
        rent_day(&mut h, 1);
        while let Some(p) = h.foreman.queue.first().cloned() {
            h.foreman.queue.remove(0);
            deals.push(p);
        }
    }
    // Deal seven (index six) is the dud: the smithy beside the inn.
    let dud = &deals[6];
    assert_eq!(dud.room, "smithy");
    assert!(
        dud.says.contains("trust me") || dud.says.contains("neighbours") || dud.says.contains("feeling"),
        "{}",
        dud.says
    );
}

#[test]
fn an_aye_pays_his_quote_and_three_nays_stir_the_works() {
    let mut h = cleared();
    let mut t = gold(7000);
    settle(&mut h, &mut t, &marks(&[&buy_mark(crypt(), false)]));
    // A padded quote pays whole, padding and all.
    h.foreman.queue.push(crate::rules::foreman::Petition {
        key: crypt_key(),
        room: "taproom".into(),
        gold: 440,
        relic: None,
        says: "the wagon axle broke on the oak".into(),
    });
    let out = settle(&mut h, &mut t, &marks_of(&[aye()]));
    assert_eq!(t.get(Spoil::Gold), 7000 - 6000 - 440);
    assert!(out.said.iter().any(|(_, l)| l.contains("going up at The Crypt")), "{:?}", out.said);
    // Refusals cost nothing but his temper: three in a row stir unrest.
    for _ in 0..3 {
        stage(&mut h, &crypt_key(), "shrine");
        settle(&mut h, &mut t, &marks_of(&[nay()]));
    }
    assert_eq!(h.unrest, 1, "the works grumble");
    assert_eq!(h.holding(&crypt_key()).unwrap().rooms.len(), 1, "nothing built by refusal");
    // A short purse is told so, and the petition lapses off the ledge.
    let mut poor = gold(0);
    stage(&mut h, &crypt_key(), "smithy");
    let out = settle(&mut h, &mut poor, &marks_of(&[aye()]));
    assert!(out.said.iter().any(|(_, l)| l.contains("PURSE SHORT")));
    assert!(h.foreman.queue.is_empty());
}

#[test]
fn stewards_are_hired_one_to_a_holding_and_change_at_the_plate() {
    let (mut h, mut t) = held();
    settle(&mut h, &mut t, &marks(&[&buy_mark(Deed::Castle(0), false)]));
    let castle = Deed::Castle(0).key();
    let key = crypt_key();
    settle(&mut h, &mut t, &marks(&[&steward_mark(&key)]));
    assert_eq!(h.holding(&key).unwrap().steward.as_deref(), Some("wynn"));
    settle(&mut h, &mut t, &marks(&[&steward_mark(&castle)]));
    assert_eq!(h.holding(&castle).unwrap().steward.as_deref(), Some("aled"), "Wynn is taken");
    // Again at the Crypt: the next free one after Wynn.
    settle(&mut h, &mut t, &marks(&[&steward_mark(&key)]));
    assert_eq!(h.holding(&key).unwrap().steward.as_deref(), Some("gwen"));
}

#[test]
fn a_charter_needs_its_condition_gold_and_a_scroll() {
    let (mut h, mut t) = held();
    let key = crypt_key();
    assert_eq!(h.holding(&key).unwrap().charter(), Err("NEEDS A STEWARD"));
    settle(&mut h, &mut t, &marks(&[&steward_mark(&key)]));
    assert_eq!(h.holding(&key).unwrap().charter(), Ok((2, STAR_PRICE[2])));
    // No scroll: no charter.
    let out = settle(&mut h, &mut t, &marks(&[&charter_mark(&key)]));
    assert_eq!(h.holding(&key).unwrap().stars, 1);
    assert_eq!(out.said[0].0, "cant_afford");
    // A compaction copies one.
    feed(&mut h, &Ops { compactions: 1, ..Ops::default() });
    let before = t.get(Spoil::Gold);
    settle(&mut h, &mut t, &marks(&[&charter_mark(&key)]));
    assert_eq!(h.holding(&key).unwrap().stars, 2);
    assert_eq!((h.scrolls, t.get(Spoil::Gold)), (0, before - STAR_PRICE[2]));
    assert_eq!(h.holding(&key).unwrap().charter(), Err("NEEDS 5 ROOMS"));
}

#[test]
fn rent_falls_due_on_a_rent_day_and_a_knight_collects_it() {
    let (mut h, _) = held();
    let key = crypt_key();
    let w = h.holding(&key).unwrap();
    assert_eq!(w.standing(), 2);
    let rent = w.rent();
    // The taproom's crowd next door makes the shrine rowdy: ten less.
    assert_eq!(rent, 20 + 20 - 10);
    rent_day(&mut h, 2);
    assert_eq!(h.holding(&key).unwrap().coffer, rent * 2);
    // Two rent days wear the walls twice, no retainer to hold them.
    assert_eq!(h.holding(&key).unwrap().walls, WALLS - 2);
    let mut t = gold(0);
    let out = settle(&mut h, &mut t, &marks(&[COLLECT]));
    assert_eq!(t.get(Spoil::Gold), rent * 2);
    assert_eq!(h.holding(&key).unwrap().coffer, 0);
    assert_eq!(out.said[0].0, "holding_rent");
}

#[test]
fn unrest_halves_rents_and_a_riot_stops_them_and_patrols_walk_it_off() {
    let (mut h, _) = held();
    let key = crypt_key();
    let rent = h.holding(&key).unwrap().rent();
    h.unrest = RESTLESS;
    rent_day(&mut h, 1);
    assert_eq!(h.holding(&key).unwrap().coffer, rent / 2);
    h.unrest = UNREST_CAP;
    rent_day(&mut h, 1);
    assert_eq!(h.holding(&key).unwrap().coffer, rent / 2, "a riot pays nothing");
    // Patrols out first: three calm three.
    h.unrest = 6;
    h.patrols = 3;
    rent_day(&mut h, 1);
    assert_eq!((h.unrest, h.patrols), (3, 0));
    assert_eq!(h.holding(&key).unwrap().coffer, rent / 2 + rent, "calmed before the rent");
}

#[test]
fn breached_walls_pay_nothing_and_retainers_or_a_brave_steward_hold_them() {
    let (mut h, _) = held();
    let key = crypt_key();
    h.owned[0].walls = 1;
    let said = rent_day(&mut h, 1);
    assert_eq!(h.owned[0].walls, 0);
    assert!(said.iter().any(|l| l.contains("breached")));
    let coffer = h.owned[0].coffer;
    rent_day(&mut h, 1);
    assert_eq!(h.owned[0].coffer, coffer, "a breach pays nothing");
    // A retainer garrisons it for a rent day.
    h.owned[0].walls = 5;
    h.retainers = 1;
    rent_day(&mut h, 1);
    assert_eq!((h.owned[0].walls, h.retainers), (5, 0));
    // Gwen keeps them without one.
    h.owned[0].steward = Some("gwen".into());
    h.stores = 5;
    rent_day(&mut h, 3);
    assert_eq!(h.owned[0].walls, 5);
    assert_eq!(h.stores, 2, "a steward eats a sack a rent day");
    assert!(h.holding(&key).is_some());
}

#[test]
fn every_steward_has_a_way() {
    let (base, _) = held();
    let rent = base.owned[0].rent();
    let with = |id: &str| {
        let mut h = base.clone();
        h.owned[0].steward = Some(id.into());
        h.stores = 10;
        h.unrest = 2;
        h
    };
    // Greedy: more rent, more unrest.
    let mut h = with("wynn");
    assert_eq!(h.owned[0].rent(), rent + 30);
    rent_day(&mut h, 1);
    assert_eq!(h.unrest, 3);
    // Pious: calms one a rent day.
    let mut h = with("aled");
    rent_day(&mut h, 1);
    assert_eq!(h.unrest, 1);
    // Sot: taverns pay ten more each (one taproom here).
    assert_eq!(with("hob").owned[0].rent(), rent + 10);
    // Shrewd: builds twice as fast.
    let mut h = with("vell");
    let mut t = gold(5000);
    let key = crypt_key();
    stage(&mut h, &key, "stall");
    settle(&mut h, &mut t, &marks_of(&[aye()]));
    settle(&mut h, &mut t, &BTreeMap::from([(LABOUR.to_string(), ROOM_LABOUR / 2)]));
    assert!(h.owned[0].rooms.last().unwrap().standing());
}

// ── angelX's work keeps the holdings, never pays them ─────────────────────

#[test]
fn each_of_angelxs_works_moves_one_thing_by_a_fixed_number() {
    let (mut h, t) = held();
    let gold_before = t.get(Spoil::Gold);
    h.owned[0].walls = 4;
    h.owned[0].rooms.push(holdings::Built { id: "smithy".into(), labour: 0 });
    let said = feed(
        &mut h,
        &Ops {
            cache: CACHE_PER_SACK * 3 + 7,
            tokens: TOKENS_PER_SACK * 2,
            tests: 2,
            commits: 2,
            compactions: 1,
            subagents: 2,
            loops: 3,
            errors: 1,
            slow: 1,
        },
    );
    // Cache filled three sacks; wages took two.
    assert_eq!(h.stores, 1);
    assert_eq!(h.cache_rem, 7);
    // Two test runs mended two courses of the most worn walls.
    assert_eq!(h.owned[0].walls, 6);
    // Two commits laid six: the smithy stands.
    assert!(h.owned[0].rooms.last().unwrap().standing());
    assert!(said.iter().any(|l| l.contains("Smithy stands")));
    assert_eq!((h.scrolls, h.retainers, h.patrols, h.unrest), (1, 2, 3, 2));
    // The board's counts.
    for (op, n) in [(Op::Cache, 3), (Op::Tokens, 2), (Op::Tests, 2), (Op::Commits, 2), (Op::Errors, 2)] {
        assert_eq!(h.feed.get(op.word()).copied(), Some(n), "{op:?}");
    }
    // Never gold: feed has no purse to touch, and the treasury is as it was.
    assert_eq!(t.get(Spoil::Gold), gold_before);
    assert!(h.owned.iter().all(|w| w.coffer == 0));
}

#[test]
fn wages_the_granary_cannot_pay_stir_unrest() {
    let mut h = Holdings::default();
    feed(&mut h, &Ops { tokens: TOKENS_PER_SACK * 3, ..Ops::default() });
    assert_eq!((h.stores, h.unrest), (0, 3));
    // Every stock has its cap.
    feed(&mut h, &Ops { cache: CACHE_PER_SACK * 100, compactions: 99, subagents: 99, loops: 99, errors: 99, ..Ops::default() });
    assert_eq!(
        (h.stores, h.scrolls, h.retainers, h.patrols, h.unrest),
        (STORES_CAP, SCROLLS_CAP, RETAINERS_CAP, PATROLS_CAP, UNREST_CAP)
    );
}

#[test]
fn every_mapping_has_a_short_label_on_the_board() {
    for op in Op::ALL {
        assert!(op.word().len() <= 9 && op.source().len() <= 12, "{op:?}");
        assert!(op.word().split(' ').count() <= 2 && op.source().split(' ').count() <= 3);
    }
    for room in &holdings::ROOMS {
        assert!(room.says.split(' ').count() <= 2 && room.says.len() <= 8, "{}", room.id);
    }
}

// ── Saving ────────────────────────────────────────────────────────────────

// ── The run's half: the Court ─────────────────────────────────────────────

fn court(holdings: Holdings, purse: u32) -> Run {
    let home = Home { holdings, ..Home::default() };
    let mut run = Run::at_home(7, 1, None, home, gold(purse));
    let i = world::room_of(&run.dungeon, RoomKind::LandOffice).expect("the Court on the home floor");
    run.arrive(i, (12.0, 12.0));
    run.step(&BTreeMap::new());
    run
}

fn stand(run: &mut Run, plate: (i32, i32, i32, i32)) {
    let (c, r, w, h) = plate;
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (c as f32 + w as f32 / 2.0) * TILE_UNITS;
    hero.y = (r as f32 + h as f32 / 2.0) * TILE_UNITS;
}

fn hold(run: &mut Run, plate: (i32, i32, i32, i32)) {
    stand(run, plate);
    let e = BTreeMap::from([(1, Input { interact: true, ..Input::default() })]);
    for _ in 0..BUY_HOLD + 1 {
        run.step(&e);
    }
    run.step(&BTreeMap::new());
}

/// The cockpit's half, as `settle_home` does it.
fn cockpit(run: &mut Run) {
    let marks = std::mem::take(&mut run.marks);
    let mut h = run.home.holdings.clone();
    let mut t = run.treasury.clone();
    settle(&mut h, &mut t, &marks);
    run.home.holdings = h;
    run.treasury = t;
}

#[test]
fn the_court_stands_on_the_home_floor_and_on_the_kingdoms_map() {
    let run = court(Holdings::default(), 0);
    assert_eq!(run.room().kind, RoomKind::LandOffice);
    assert!(run.room().doors[2], "its door to the kingdom");
    for (_, rect) in Plate::all() {
        let (c, r, _, _) = rect;
        assert_eq!(run.room().tile(c, r), Tile::Floor, "{rect:?} walkable");
    }
    let geo = crate::render::overworld::kingdom::geography(&[]);
    let site = geo.sites.iter().find(|s| s.room == RoomKind::LandOffice).expect("a door on the map");
    assert_eq!(site.name, "Land Office");
    // Near the Keep.
    let keep = crate::render::overworld::kingdom::keep_stand();
    assert!((site.stand.0 - keep.0).abs() + (site.stand.1 - keep.1).abs() < 30, "{:?} {keep:?}", site.stand);
}

#[test]
fn the_courts_door_on_the_map_leads_in_and_back_out() {
    let mut run = Run::at_home(7, 1, None, Home::default(), Spoils::default());
    run.walk_kingdom(None);
    let geo = crate::render::overworld::kingdom::geography(&run.kingdom.wishes);
    let i = geo.sites.iter().position(|s| s.room == RoomKind::LandOffice).unwrap();
    let stand = geo.sites[i].stand;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = ((stand.0 as f32 + 0.5) * TILE_UNITS, (stand.1 as f32 + 0.7) * TILE_UNITS);
    run.kingdom.door_held = false;
    let e = BTreeMap::from([(1, Input { interact: true, ..Input::default() })]);
    run.step(&e);
    assert_eq!(run.room().kind, RoomKind::LandOffice);
    let hero = &run.players[&1];
    assert!(holdings::plate_under(&run, 1).is_none(), "nobody arrives on a plate");
    assert!(run.room().tile((hero.x / TILE_UNITS) as i32, (hero.y / TILE_UNITS) as i32) == Tile::Floor);
    // Out through the south door, back on the map at the Court's door.
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = 12.0 * TILE_UNITS;
    let down = BTreeMap::from([(1, Input { move_y: 1, ..Input::default() })]);
    for _ in 0..90 {
        run.step(&down);
        if run.room().kind == RoomKind::Kingdom {
            break;
        }
    }
    assert_eq!(run.room().kind, RoomKind::Kingdom);
}

#[test]
fn the_arrows_turn_the_board_and_the_plates_buy_build_and_collect() {
    let mut run = court(cleared(), 20_000);
    assert_eq!(run.home.holdings.picked(), crypt());
    stand(&mut run, NEXT);
    let e = BTreeMap::from([(1, Input { interact: true, ..Input::default() })]);
    run.step(&e);
    assert_eq!(run.marks.get(&pick_mark(1)), Some(&1), "a tap turns it");
    run.step(&e);
    assert_eq!(run.marks.get(&pick_mark(1)), Some(&1), "once a press");
    run.step(&BTreeMap::new());
    cockpit(&mut run);
    assert_eq!(run.home.holdings.picked(), Deed::Floor(Pack::Cavern));
    hold(&mut run, PREV);
    cockpit(&mut run);
    assert_eq!(run.home.holdings.picked(), crypt());
    // Claim it.
    hold(&mut run, CLAIM);
    assert_eq!(holdings::plate_open(&run, Plate::Claim), Err("OURS"), "promised before the save");
    cockpit(&mut run);
    assert!(run.home.holdings.holding(&crypt_key()).is_some());
    assert_eq!(run.treasury.get(Spoil::Gold), 14_000);
    // A steward, the Foreman's petition for a taproom, and the crew's
    // trips from play: two hands raise it.
    hold(&mut run, STEWARD);
    crate::rules::foreman::draft(&mut run.home.holdings);
    assert_eq!(run.home.holdings.foreman.queue[0].room, "taproom");
    hold(&mut run, AYE);
    cockpit(&mut run);
    let w = run.home.holdings.holding(&crypt_key()).unwrap();
    assert_eq!(w.steward.as_deref(), Some("wynn"));
    assert_eq!(w.rooms[0].id, "taproom");
    for _ in 0..(SHIFT_SECS * HZ * COURSE_TRIPS * ROOM_LABOUR / HANDS) {
        run.step(&BTreeMap::new());
    }
    cockpit(&mut run);
    assert!(run.home.holdings.holding(&crypt_key()).unwrap().rooms[0].standing());
    // Rent, then the strongbox.
    assert_eq!(holdings::plate_open(&run, Plate::Collect), Err("COFFERS EMPTY"));
    rent_day(&mut run.home.holdings, 1);
    hold(&mut run, STRONGBOX);
    let waiting = 20 + 30;
    cockpit(&mut run);
    assert_eq!(run.treasury.get(Spoil::Gold), 14_000 - 400 + waiting);
}

#[test]
fn shut_plates_do_nothing() {
    let mut run = court(Holdings::default(), 90_000);
    for rect in [CLAIM, FIEF, STEWARD, CHARTER, AYE, NAY, STRONGBOX] {
        hold(&mut run, rect);
    }
    assert!(run.marks.keys().all(|k| !k.starts_with("holdings:") || k == LABOUR));
    assert!(run.valid_snapshot());
}

// ── The run's half: a held floor ──────────────────────────────────────────

fn owned_floor() -> Holdings {
    let (mut h, _) = held();
    h.owned[0].steward = Some("aled".into());
    h.owned[0].coffer = 80;
    h
}

fn below(holdings: Holdings) -> Run {
    let home = Home { holdings, ..Home::default() };
    let mut run = Run::at_home(7, 1, None, home, Spoils::default());
    run.dungeon.pack = Pack::Crypt;
    run.descend_for_test();
    run
}

#[test]
fn a_held_floor_has_its_stewards_hall_as_built_and_others_do_not() {
    for seed in 0..8 {
        let home = Home { holdings: owned_floor(), ..Home::default() };
        let mut run = Run::at_home(seed, 1, None, home, Spoils::default());
        run.dungeon.pack = Pack::Crypt;
        run.descend_for_test();
        assert_eq!(run.dungeon.pack, Pack::Crypt);
        let i = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Holding).expect("the steward's hall");
        let room = &run.dungeon.rooms[i];
        let door = (0..4).find(|&d| room.doors[d]).unwrap();
        let host = run.dungeon.neighbour(i, door).expect("joined");
        assert!(run.dungeon.rooms[host].doors[(door + 2) % 4]);
        // Built bays are solid furniture; their plates are floor.
        for b in 0..2 {
            let ((c, r, _, _), (pc, pr, _, _)) = holdings::bay(b);
            assert_eq!(room.tile(c, r), Tile::Block);
            assert_eq!(room.tile(pc, pr), Tile::Floor);
        }
        assert!(run.valid_snapshot(), "seed {seed}");
        // The next floor (the Mines) is not held.
        run.descend_for_test();
        assert!(!run.dungeon.rooms.iter().any(|r| r.kind == RoomKind::Holding));
    }
    let run = below(cleared());
    assert!(!run.dungeon.rooms.iter().any(|r| r.kind == RoomKind::Holding), "not ours, no hall");
}

#[test]
fn a_bays_boon_serves_the_whole_party_once_a_floor() {
    let mut run = below(owned_floor());
    let i = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Holding).unwrap();
    run.arrive(i, (12.0, 8.0));
    for h in run.players.values_mut() {
        h.hp = 10;
    }
    // Bay 0 is the taproom: a brawler joins the party.
    hold(&mut run, holdings::bay(0).1);
    assert_eq!(run.holding_used, 1);
    assert_eq!(run.hireling.as_ref().map(|h| h.kind), Some(crate::rules::hireling::AllyKind::Brawler));
    run.hireling = None;
    hold(&mut run, holdings::bay(0).1);
    assert!(run.hireling.is_none(), "once a floor");
    // Bay 1 is the shrine: spells full.
    run.players.get_mut(&1).unwrap().mana = 0.0;
    hold(&mut run, holdings::bay(1).1);
    assert_eq!(run.players[&1].mana, MAX_MANA);
    // The steward hands over the rent.
    hold(&mut run, STEWARD_PLATE);
    assert_eq!(run.marks.get(COLLECT), Some(&1));
    assert!(run.valid_snapshot());
    // A new floor, a fresh hall.
    run.descend_for_test();
    assert_eq!(run.holding_used, 0);
}

#[test]
fn the_used_bays_ride_the_mirror() {
    let mut run = below(owned_floor());
    run.holding_used = 0b101;
    let friend = run.wire_copy();
    assert_eq!(friend.holding_used, 0b101);
    assert_eq!(friend.home.holdings, run.home.holdings);
}

// ── The run's half: a castle ──────────────────────────────────────────────

fn at_castle(holdings: Holdings, purse: u32) -> Run {
    use crate::render::overworld::{castles, kingdom};
    let home = Home { holdings, ..Home::default() };
    let mut run = Run::at_home(7, 1, None, home, gold(purse));
    run.walk_kingdom(None);
    let geo = kingdom::geography(&run.kingdom.wishes);
    let stand = castles::stand(crate::world::houses::HouseId(0));
    let i = geo.sites.iter().position(|s| s.stand == stand).expect("sol's castle door");
    run.enter_kingdom_place(i);
    run.step(&BTreeMap::new());
    run
}

#[test]
fn a_castle_is_bought_at_its_own_hall() {
    let mut run = at_castle(cleared(), 6000);
    assert_eq!(run.room().kind, RoomKind::RealmHall);
    assert_eq!(holdings::castle_here(&run), Some(0));
    hold(&mut run, CASTLE_PLATE);
    assert_eq!(run.marks.get(&buy_mark(Deed::Castle(0), false)), Some(&1));
    cockpit(&mut run);
    assert!(run.home.holdings.holding(&Deed::Castle(0).key()).is_some());
    assert_eq!(run.treasury.get(Spoil::Gold), 1000);
    // Unserved houses' castles are not for sale.
    let mut run = at_castle(Holdings::default(), 6000);
    hold(&mut run, CASTLE_PLATE);
    assert!(run.marks.keys().all(|k| !k.starts_with("holdings:claim")));
}

// ── Pictures ──────────────────────────────────────────────────────────────

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_holdings_shots -- --ignored`
#[test]
#[ignore]
fn write_holdings_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("holdings");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    // A realm well along: the Crypt built out, Sol's castle held, stores
    // from angelX's work.
    let mut h = owned_floor();
    let mut t = gold(60_000);
    let key = crypt_key();
    for room in ["smithy", "stall", "brewery"] {
        stage(&mut h, &key, room);
        settle(&mut h, &mut t, &marks_of(&[aye()]));
    }
    settle(&mut h, &mut t, &BTreeMap::from([(LABOUR.to_string(), ROOM_LABOUR * 2 + 2)]));
    // The great works, standing: forge and hearth and the way down.
    for room in ["greatforge", "greathall", "waypoint"] {
        h.holding_mut(&key).unwrap().rooms.push(holdings::Built { id: room.into(), labour: ROOM_LABOUR });
    }
    settle(&mut h, &mut t, &marks(&[&buy_mark(Deed::Castle(0), false)]));
    stage(&mut h, &Deed::Castle(0).key(), "shrine");
    settle(&mut h, &mut t, &marks_of(&[aye()]));
    feed(&mut h, &Ops { cache: CACHE_PER_SACK * 14, tokens: TOKENS_PER_SACK * 3, tests: 5, commits: 1, compactions: 2, subagents: 2, loops: 4, errors: 3, slow: 0 });
    h.owned[0].walls = 7;
    h.owned[0].stars = 2;
    h.owned[0].coffer = 240;
    let mut run = court(h.clone(), 8000);
    run.players.get_mut(&1).unwrap().buying = BUY_HOLD / 2;
    stand(&mut run, NAY);
    run.step(&BTreeMap::new());
    save(&run, "a-court-held");
    // A deed for sale on the board, a knight at the claim plate.
    let mut sale = h.clone();
    sale.cleared.insert(Pack::Cavern.name().to_string());
    sale.pick = 1;
    let mut run = court(sale, 8000);
    stand(&mut run, CLAIM);
    run.step(&BTreeMap::new());
    save(&run, "b-court-for-sale");
    // The kingdom: the Court's door by the Keep.
    let mut run = court(h.clone(), 8000);
    run.walk_kingdom(None);
    let geo = crate::render::overworld::kingdom::geography(&run.kingdom.wishes);
    let site = geo.sites.iter().find(|s| s.room == RoomKind::LandOffice).unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = ((site.stand.0 as f32 + 0.5) * TILE_UNITS, (site.stand.1 as f32 + 1.8) * TILE_UNITS);
    for _ in 0..60 {
        run.step(&BTreeMap::new());
    }
    save(&run, "c-kingdom-land-office");
    // The held Crypt below: its first hall's banners, its steward's hall.
    let mut run = below(h.clone());
    run.step(&BTreeMap::new());
    save(&run, "d-crypt-start-banners");
    let i = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Holding).unwrap();
    run.arrive(i, (12.0, 8.0));
    run.enemies.clear();
    stand(&mut run, holdings::bay(0).1);
    run.step(&BTreeMap::new());
    save(&run, "e-crypt-stewards-hall");
    hold(&mut run, holdings::bay(0).1);
    stand(&mut run, STEWARD_PLATE);
    run.step(&BTreeMap::new());
    save(&run, "f-crypt-hall-used");
    // Sol's castle, held, and the map with its pennon.
    let mut run = at_castle(h.clone(), 8000);
    stand(&mut run, CASTLE_PLATE);
    run.step(&BTreeMap::new());
    save(&run, "g-castle-held");
    let mut run = at_castle(cleared(), 8000);
    run.step(&BTreeMap::new());
    save(&run, "h-castle-for-sale");
    let mut run = court(h, 8000);
    run.walk_kingdom(Some(crate::render::overworld::castles::stand(crate::world::houses::HouseId(0))));
    for _ in 0..60 {
        run.step(&BTreeMap::new());
    }
    save(&run, "i-kingdom-castle-pennon");
    // The Foreman at his ledge: three petitions, the head one padded
    // (the sheet says 495 against the brewery's 450), a knight weighing
    // it at the AYE post; then the NAY post's refusal.
    let mut h = owned_floor();
    crate::rules::foreman::draft(&mut h);
    h.foreman.queue.insert(
        0,
        crate::rules::foreman::Petition {
            key: crypt_key(),
            room: "brewery".into(),
            gold: 495,
            relic: None,
            says: "the wagon axle broke on the oak".into(),
        },
    );
    let mut run = court(h.clone(), 8000);
    stand(&mut run, AYE);
    run.players.get_mut(&1).unwrap().buying = BUY_HOLD / 2;
    run.step(&BTreeMap::new());
    save(&run, "j-foreman-aye");
    stand(&mut run, NAY);
    run.players.get_mut(&1).unwrap().buying = BUY_HOLD / 3;
    run.step(&BTreeMap::new());
    save(&run, "k-foreman-nay");
    // A great work petitioned with its relic wanted, beside a purse that
    // holds one.
    let mut run = court(h.clone(), 8000);
    run.treasury.add(Spoil::Forgeheart, 1);
    run.home.holdings.foreman.queue.insert(
        0,
        crate::rules::foreman::Petition {
            key: crypt_key(),
            room: "greatforge".into(),
            gold: 800,
            relic: Some(Spoil::Forgeheart),
            says: "it beats, m'lord".into(),
        },
    );
    stand(&mut run, AYE);
    run.step(&BTreeMap::new());
    save(&run, "l-foreman-great-forge");
    // The same great work against an empty purse: the sheet names the
    // missing relic in red, the AYE post stays shut.
    let mut run = court(h.clone(), 8000);
    run.home.holdings.foreman.queue.insert(
        0,
        crate::rules::foreman::Petition {
            key: crypt_key(),
            room: "greatforge".into(),
            gold: 800,
            relic: Some(Spoil::Forgeheart),
            says: "it beats, m'lord".into(),
        },
    );
    stand(&mut run, AYE);
    run.step(&BTreeMap::new());
    save(&run, "m-foreman-bring-a-relic");
}

// ── Neighbours ────────────────────────────────────────────────────────────

fn built(ids: &[&str]) -> Vec<holdings::Built> {
    ids.iter().map(|id| holdings::Built { id: id.to_string(), labour: ROOM_LABOUR }).collect()
}

#[test]
fn neighbours_in_the_same_row_help_or_harm_a_rooms_rent() {
    // A brewery beside a taproom feeds it; a smithy beside an inn keeps it
    // awake.
    let rooms = built(&["brewery", "taproom", "smithy", "inn", "shrine", "warehouse"]);
    assert_eq!(holdings::neighbours(&rooms, 1, "taproom"), vec![(10, "BREWED")]);
    assert_eq!(holdings::neighbours(&rooms, 3, "inn"), vec![(-10, "NOISY")]);
    // Bay 4 ends the north row: the warehouse in bay 5 is across the hall.
    assert!(holdings::neighbours(&rooms, 4, "shrine").is_empty());
    // Only standing rooms count.
    let mut rising = rooms.clone();
    rising[0].labour = 0;
    assert!(holdings::neighbours(&rising, 1, "taproom").is_empty());
    let mut w = Holding { key: crypt_key(), walls: WALLS, stars: 1, rooms: built(&["taproom", "brewery"]), ..Holding::default() };
    // Taproom 20 + brewery 25, and the taproom brewed: 10 more.
    assert_eq!(w.rent(), 20 + 25 + 10);
    // The Court says so before a room goes in.
    w.rooms = built(&["inn"]);
    let smithy = holdings::room_def("smithy").unwrap();
    assert_eq!(w.preview(smithy), vec![(-10, "NOISY")]);
}

// ── The relics of the deep ─────────────────────────────────────────────────

#[test]
fn the_deep_floors_give_up_relics_by_a_fixed_table() {
    use crate::realm::Spoil as S;
    use crate::rules::relics;
    assert_eq!(relics::find(Pack::Archive, 0, relics::CHEST_EVERY), Some(S::Hearthstone));
    assert_eq!(relics::find(Pack::Archive, 6, relics::CHEST_EVERY), Some(S::Forgeheart));
    assert_eq!(relics::find(Pack::Archive, 12, relics::CHEST_EVERY), Some(S::Ledgerglass));
    assert_eq!(relics::find(Pack::Fungal, 0, relics::CHEST_EVERY), Some(S::Bellmetal));
    assert_eq!(relics::find(Pack::Unknown, 0, relics::GUARDIAN_EVERY), Some(S::Waystone));
    // The shallow floors give up nothing, and most draws are nothing.
    for draw in 1..6 {
        assert_eq!(relics::find(Pack::Crypt, draw, relics::CHEST_EVERY), None);
        assert_eq!(relics::find(Pack::Archive, draw, relics::CHEST_EVERY), None);
    }
}

#[test]
fn a_great_work_wants_its_relic_and_the_foreman_pays_it_in() {
    let mut h = cleared();
    let mut t = gold(20_000);
    settle(&mut h, &mut t, &marks(&[&buy_mark(crypt(), false)]));
    // A great forge petition without the heart in the treasury lapses with
    // its want named.
    let petition = crate::rules::foreman::Petition {
        key: crypt_key(),
        room: "greatforge".into(),
        gold: 800,
        relic: Some(Spoil::Forgeheart),
        says: "it beats, m'lord".into(),
    };
    h.foreman.queue.push(petition.clone());
    let out = settle(&mut h, &mut t, &marks_of(&[aye()]));
    assert!(out.said.iter().any(|(_, l)| l.contains("NEEDS A RELIC")), "{:?}", out.said);
    assert!(h.holding(&crypt_key()).unwrap().rooms.is_empty(), "nothing built without the heart");
    // With the heart in the treasury: gold and heart both paid.
    t.add(Spoil::Forgeheart, 1);
    h.foreman.queue.push(petition);
    settle(&mut h, &mut t, &marks_of(&[aye()]));
    let w = h.holding(&crypt_key()).unwrap();
    assert_eq!(w.rooms[0].id, "greatforge");
    assert_eq!(t.get(Spoil::Gold), 20_000 - 6000 - 800);
    assert_eq!(t.get(Spoil::Forgeheart), 0, "the heart is spent");
}

#[test]
fn a_standing_waypoint_is_a_landing_of_the_realms_own() {
    let waypoint = |key: String, labour| Holdings {
        owned: vec![Holding {
            key,
            walls: WALLS,
            stars: 1,
            rooms: vec![holdings::Built { id: "waypoint".into(), labour }],
            ..Holding::default()
        }],
        ..Holdings::default()
    };
    // Wren's ladder alone: floor 1.
    assert_eq!(Home::default().landings(), vec![1]);
    // A Waypoint standing on a held Archive floor: floor 4 joins the
    // stair, and half-built does not count.
    let standing = waypoint(Deed::Floor(Pack::Archive).key(), ROOM_LABOUR);
    assert_eq!(Home { holdings: standing, ..Home::default() }.landings(), vec![1, 4]);
    let rising = waypoint(Deed::Floor(Pack::Archive).key(), 0);
    assert_eq!(Home { holdings: rising, ..Home::default() }.landings(), vec![1]);
    // The Unknown floor's Waypoint is the only stair to floor 6: Wren
    // never sells one that deep.
    let deep = waypoint(Deed::Floor(Pack::Unknown).key(), ROOM_LABOUR);
    assert_eq!(Home { holdings: deep, ..Home::default() }.landings(), vec![1, 6]);
}

#[test]
fn the_deep_gives_a_relic_into_the_first_living_knights_purse() {
    let mut run = Run::at_home(7, 1, None, Home::default(), Spoils::default());
    run.dungeon.pack = Pack::Archive;
    run.dungeon.depth = 4;
    run.relic_find(0, crate::rules::relics::CHEST_EVERY);
    assert_eq!(run.players[&1].carried.get(Spoil::Hearthstone), 1);
    assert!(run.cues.iter().any(|c| c == "relic"));
    assert!(run.found.as_ref().is_some_and(|f| f.2.contains("hearthstone")));
    // A draw that finds nothing finds nothing.
    run.relic_find(1, crate::rules::relics::CHEST_EVERY);
    assert_eq!(run.players[&1].carried.get(Spoil::Hearthstone), 1);
    // A knight of stone waits for a living shoulder.
    run.players.get_mut(&1).unwrap().stone = true;
    run.relic_find(6, crate::rules::relics::CHEST_EVERY);
    assert_eq!(run.players[&1].carried.get(Spoil::Forgeheart), 0);
}

// ── Raids on the board ────────────────────────────────────────────────────

#[test]
fn a_raid_is_pinned_to_wrens_board_until_it_is_fought_off() {
    let (mut h, mut t) = held();
    h.owned[0].raid = 1;
    let mut home = Home { holdings: h.clone(), ..Home::default() };
    assert!(crate::rules::bounties::drift(&home));
    let said = crate::rules::bounties::note_raids(&mut home);
    assert_eq!(home.raids, [crypt_key()]);
    assert!(said.iter().any(|l| l.contains("Wren pins a raid: throw them out of The Crypt")));
    assert!(!crate::rules::bounties::drift(&home), "in sync");
    // The raid rides the missions: the Gate's gold arrow pulls the party
    // back down, and the board's row says whose hall.
    let run = Run::at_home(7, 1, None, home.clone(), Spoils::default());
    let held_names: Vec<String> = run.missions_held(1).into_iter().map(|m| m.name).collect();
    assert!(held_names.iter().any(|n| n == "Raiders hold The Crypt"), "{held_names:?}");
    // Fought off: the purse is won once (the raid's own settle), the row
    // comes down, and no second purse is minted.
    let before = t.get(Spoil::Gold);
    settle(&mut home.holdings, &mut t, &marks(&[&raids::won_mark(&crypt_key())]));
    assert_eq!(t.get(Spoil::Gold), before + RAID_PURSE[1]);
    let said = crate::rules::bounties::note_raids(&mut home);
    assert!(said.is_empty());
    assert!(home.raids.is_empty());
    assert!(!crate::rules::bounties::drift(&home));
}

#[test]
fn low_walls_bring_raiders_who_stop_the_rent_and_sack_a_room_a_day() {
    let (mut h, _) = held();
    let rent = h.owned[0].rent();
    h.owned[0].walls = RAID_WALLS + 1;
    let said = rent_day(&mut h, 1);
    assert_eq!(h.owned[0].raid, 1, "walls down to the raiders' mark");
    assert!(said.iter().any(|l| l.contains("Raiders take")));
    assert_eq!(h.owned[0].coffer, rent, "that day's rent came in first");
    rent_day(&mut h, 1);
    assert_eq!(h.owned[0].coffer, rent, "no rent while they hold it");
    assert_eq!(h.owned[0].raid, 2);
    assert_eq!(h.owned[0].standing(), 1, "a room sacked");
    // Won: free, walls mended, the raiders' purse paid.
    let mut t = gold(0);
    let walls = h.owned[0].walls;
    settle(&mut h, &mut t, &marks(&[&raids::won_mark(&crypt_key())]));
    assert_eq!((h.owned[0].raid, h.owned[0].walls), (0, walls + RAID_MEND));
    assert_eq!(t.get(Spoil::Gold), RAID_PURSE[2]);
    // A barracks keeps the walls from wearing.
    h.owned[0].rooms.push(holdings::Built { id: "barracks".into(), labour: ROOM_LABOUR });
    let walls = h.owned[0].walls;
    rent_day(&mut h, 3);
    assert_eq!(h.owned[0].walls, walls);
}

/// A held Crypt with a room of every family that fights, under raid.
fn raided_floor() -> Holdings {
    let (mut h, _) = held();
    h.owned[0].rooms = built(&["taproom", "shrine", "smithy", "stall", "range"]);
    h.owned[0].raid = 1;
    h
}

fn into_hall(run: &mut Run) {
    let i = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Holding).unwrap();
    run.arrive(i, (12.0, 11.0));
    run.step(&BTreeMap::new());
}

#[test]
fn a_raid_is_fought_in_the_stewards_hall_and_the_bays_fight_with_the_party() {
    let mut run = below(raided_floor());
    let armor = run.players[&1].armor;
    into_hall(&mut run);
    assert_eq!(run.phase, Phase::Fighting);
    // Four raiders, less one bought off by the stall.
    assert_eq!(run.enemies.len() as u32, crown::raiders(1, 1));
    assert_eq!(run.enemies.len(), 3);
    assert_eq!(run.players[&1].armor, armor + 1, "the smithy arms the party");
    assert!(run.valid_snapshot());
    // The tavern hurls stools, the range looses arrows, the chapel mends.
    run.players.get_mut(&1).unwrap().hp = 10;
    for h in run.players.values_mut() {
        h.invulnerable = 10_000;
    }
    let mut shots = 0;
    for _ in 0..raids::CHAPEL.0 {
        run.step(&BTreeMap::new());
        shots += run.projectiles.iter().filter(|p| !p.hostile).count();
    }
    assert!(shots > 0, "stools and arrows fly");
    assert!(run.players[&1].hp > 10, "the chapel mends");
    assert!(run.valid_snapshot());
    // The last raider falls: the holding is freed at the save.
    for e in &mut run.enemies {
        e.hp = 0;
    }
    for _ in 0..3 {
        run.step(&BTreeMap::new());
    }
    assert!(run.holding_raid.is_none());
    assert_eq!(run.marks.get(&raids::won_mark(&crypt_key())), Some(&1));
    cockpit(&mut run);
    assert_eq!(run.home.holdings.owned[0].raid, 0);
    assert!(run.valid_snapshot());
}

#[test]
fn the_raid_fight_rides_the_mirror() {
    let mut run = below(raided_floor());
    into_hall(&mut run);
    let fight = run.holding_raid.clone().expect("fighting");
    let friend = run.wire_copy();
    assert_eq!(friend.holding_raid, Some(fight));
}

#[test]
fn beaten_in_a_castles_hall_at_home_the_party_gets_up_and_the_raiders_stay() {
    let mut h = cleared();
    h.owned.push(Holding { key: Deed::Castle(0).key(), walls: 3, stars: 1, raid: 1, rooms: built(&["smithy"]), ..Holding::default() });
    let mut run = at_castle(h, 0);
    run.step(&BTreeMap::new());
    assert_eq!(run.phase, Phase::Fighting);
    assert!(!run.enemies.is_empty());
    for h in run.players.values_mut() {
        h.hp = 0;
    }
    run.step(&BTreeMap::new());
    assert_ne!(run.phase, Phase::Wiped, "never a wipe at home");
    assert_eq!(run.phase, Phase::Exploring);
    assert!(run.players.values().all(|h| h.hp > 0));
    assert!(run.enemies.is_empty());
    let key = Deed::Castle(0).key();
    assert_eq!(run.marks.get(&raids::lost_mark(&key)), Some(&1));
    cockpit(&mut run);
    let w = run.home.holdings.holding(&key).unwrap();
    assert_eq!((w.raid, w.standing()), (1, 0), "they stay, and the smithy is sacked");
    assert!(run.valid_snapshot());
}

// ── The King's favour and his cards ───────────────────────────────────────

#[test]
fn the_kings_favour_moves_with_the_terms_and_does_something_at_either_end() {
    let mut h = cleared();
    h.cleared.insert(Pack::Cavern.name().to_string());
    let mut t = gold(50_000);
    settle(&mut h, &mut t, &marks(&[&buy_mark(crypt(), true)]));
    assert_eq!(h.favour, 1);
    settle(&mut h, &mut t, &marks(&[&buy_mark(Deed::Floor(Pack::Cavern), false)]));
    assert_eq!(h.favour, 0);
    // High: his masons build for less.
    h.favour = holdings::ROYAL;
    let before = t.get(Spoil::Gold);
    stage(&mut h, &crypt_key(), "smithy");
    settle(&mut h, &mut t, &marks_of(&[aye()]));
    assert_eq!(before - t.get(Spoil::Gold), 400 - holdings::MASONS);
    assert_eq!(crown::favour_word(&h), Some("ROYAL MASONS"));
    // Low: his men tithe a claimed holding too.
    let mines = Deed::Floor(Pack::Cavern).key();
    h.favour = -holdings::ROYAL;
    let mut t2 = gold(5000);
    for room in ["stall", "counting", "warehouse"] {
        stage(&mut h, &mines, room);
    }
    settle(&mut h, &mut t2, &BTreeMap::from([(aye(), 3)]));
    settle(&mut h, &mut t2, &BTreeMap::from([(LABOUR.to_string(), ROOM_LABOUR * 3)]));
    let w = h.holding(&mines).unwrap();
    assert!(!w.fief);
    let rent = w.rent();
    rent_day(&mut h, 1);
    assert_eq!(h.holding(&mines).unwrap().coffer, rent - TITHE);
}

#[test]
fn a_rent_day_brings_a_card_and_an_answer_settles_it() {
    let (mut h, mut t) = held();
    assert_eq!(h.card, None);
    rent_day(&mut h, 1);
    assert_eq!(h.card, Some(0), "the first card: a royal visit");
    rent_day(&mut h, 1);
    assert_eq!(h.card, Some(0), "one at a time");
    // Feast him: 300 gold, favour up two.
    let before = t.get(Spoil::Gold);
    let out = settle(&mut h, &mut t, &marks(&[&crown::card_mark(0, 0)]));
    assert_eq!(h.card, None);
    assert_eq!(h.favour, 2 - 1 + 0, "claimed (-1), then feasted (+2)");
    assert_eq!(t.get(Spoil::Gold), before - 300);
    assert_eq!(out.said[0].0, "king_card");
    // A stale answer to an old card does nothing.
    let gold_now = t.get(Spoil::Gold);
    settle(&mut h, &mut t, &marks(&[&crown::card_mark(0, 0)]));
    assert_eq!(t.get(Spoil::Gold), gold_now);
    // The next card comes round; a short purse can still say no.
    rent_day(&mut h, 1);
    assert_eq!(h.card, Some(1));
    let mut poor = gold(0);
    settle(&mut h, &mut poor, &marks(&[&crown::card_mark(1, 0)]));
    assert_eq!(h.card, Some(1), "can't pay the tithe");
    settle(&mut h, &mut poor, &marks(&[&crown::card_mark(1, 1)]));
    assert_eq!(h.card, None);
    assert_eq!(h.favour, 1 - 2);
}

#[test]
fn every_card_is_short_and_every_answer_is_two_or_three_words() {
    for card in &crown::CARDS {
        assert!(card.title.split(' ').count() <= 3 && card.line.split(' ').count() <= 5, "{}", card.title);
        for c in &card.picks {
            assert!((1..=3).contains(&c.label.split(' ').count()), "{}", c.label);
        }
    }
    // The border baron's stand brings raiders to the weakest holding.
    let (mut h, mut t) = held();
    h.card = Some(7);
    settle(&mut h, &mut t, &marks(&[&crown::card_mark(7, 0)]));
    assert_eq!(h.owned[0].raid, 1);
}

#[test]
fn the_herald_waits_by_the_land_office_and_a_hold_at_a_post_answers() {
    let (mut h, _) = held();
    h.card = Some(2);
    let mut run = court(h, 1000);
    run.walk_kingdom(None);
    let geo = crate::render::overworld::kingdom::geography(&run.kingdom.wishes);
    let ((hx, hy), posts) = crown::herald(&geo).expect("a place for him");
    for (x, y) in [(hx, hy), posts[0], posts[1]] {
        assert_eq!(geo.room.tile(x, y), Tile::Floor);
    }
    let site = geo.sites.iter().find(|s| s.room == RoomKind::LandOffice).unwrap();
    assert!((hx - site.stand.0).abs() + (hy - site.stand.1).abs() <= 16);
    run.step(&BTreeMap::new());
    assert_eq!(run.kingdom.moments.errand, Some(RoomKind::LandOffice), "the arrow leads to him");
    // Hire the masons at post A.
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = ((posts[0].0 as f32 + 0.5) * TILE_UNITS, (posts[0].1 as f32 + 0.7) * TILE_UNITS);
    assert_eq!(crown::post_under(&run, 1), Some(0));
    let e = BTreeMap::from([(1, Input { interact: true, ..Input::default() })]);
    for _ in 0..BUY_HOLD + 1 {
        run.step(&e);
    }
    assert_eq!(run.room().kind, RoomKind::Kingdom, "no door taken");
    assert_eq!(run.marks.get(&crown::card_mark(2, 0)), Some(&1));
    let walls = run.home.holdings.owned[0].walls;
    cockpit(&mut run);
    assert_eq!(run.home.holdings.card, None);
    assert_eq!(run.home.holdings.owned[0].walls, (walls + 3).min(WALLS));
    assert_eq!(run.treasury.get(Spoil::Gold), 750);
}

// ── Boons that go with the party ─────────────────────────────────────────

#[test]
fn the_taproom_sends_a_brawler_and_the_kennels_a_hound_who_fight_on() {
    use crate::rules::hireling::{AllyKind, Hireling};
    let (mut h, _) = held();
    h.owned[0].rooms = built(&["taproom", "kennels"]);
    let mut run = below(h.clone());
    into_hall(&mut run);
    hold(&mut run, holdings::bay(1).1);
    assert_eq!(run.hireling.as_ref().map(|a| a.kind), Some(AllyKind::Hound));
    // The hound bites what is near it.
    let (x, y) = { let a = run.hireling.as_ref().unwrap(); (a.x + 1.0, a.y) };
    run.spawn_body(EnemyKind::Skeleton, x, y);
    let before = run.enemies[0].hp;
    for e in &mut run.enemies {
        e.age = TELEGRAPH;
    }
    for _ in 0..20 {
        run.step(&BTreeMap::new());
    }
    assert!(run.enemies.first().is_none_or(|e| e.hp < before), "bitten");
    // The ally follows the party out of the hall.
    run.enemies.clear();
    run.phase = Phase::Exploring;
    let host = (0..4).find_map(|d| run.dungeon.neighbour(run.at, d)).unwrap();
    run.arrive(host, (12.0, 8.0));
    assert!(run.hireling.is_some());
    // Beaumains keeps his place: no brawler while he is hired.
    let mut run = below(h);
    into_hall(&mut run);
    run.hireling = Some(Hireling::new(1, (10.0, 10.0)));
    hold(&mut run, holdings::bay(0).1);
    assert_eq!(run.hireling.as_ref().map(|a| a.kind), Some(AllyKind::Beaumains));
    assert_eq!(run.holding_used, 0);
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_holdings_raid_shots -- --ignored`
#[test]
#[ignore]
fn write_holdings_raid_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("raids");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    // The Court: a raided Crypt with neighbours, the King well disposed,
    // a knight weighing a smithy beside the inn.
    let (mut h, _) = held();
    h.owned[0].rooms = built(&["shrine", "infirmary", "inn"]);
    h.owned[0].raid = 2;
    h.owned[0].walls = 3;
    h.owned[0].steward = Some("hob".into());
    h.favour = 3;
    h.stores = 9;
    let mut run = court(h.clone(), 6000);
    stand(&mut run, NAY);
    run.players.get_mut(&1).unwrap().buying = BUY_HOLD / 3;
    run.step(&BTreeMap::new());
    save(&run, "a-court-raided-neighbours");
    // The raid in the Crypt's steward's hall, a few seconds in.
    let mut f = raided_floor();
    f.owned[0].rooms = built(&["taproom", "shrine", "smithy", "stall", "range", "brewery", "infirmary"]);
    f.owned[0].raid = 2;
    let mut run = below(f);
    run.dare = None;
    run.step(&BTreeMap::new());
    save(&run, "b-crypt-start-raided");
    into_hall(&mut run);
    for h in run.players.values_mut() {
        h.invulnerable = 10_000;
    }
    for _ in 0..62 {
        run.step(&BTreeMap::new());
    }
    save(&run, "c-raid-in-the-hall");
    // Sol's castle raided at home.
    let mut c = cleared();
    c.owned.push(Holding { key: Deed::Castle(0).key(), walls: 3, stars: 2, raid: 1, rooms: built(&["taproom", "smithy", "shrine", "kennels"]), ..Holding::default() });
    let mut run = at_castle(c, 0);
    for h in run.players.values_mut() {
        h.invulnerable = 10_000;
    }
    for _ in 0..62 {
        run.step(&BTreeMap::new());
    }
    save(&run, "d-castle-raid");
    // The King's herald by the Land Office, a knight at an answer post.
    let (mut h, _) = held();
    h.card = Some(0);
    let mut run = court(h.clone(), 1000);
    run.walk_kingdom(None);
    let geo = crate::render::overworld::kingdom::geography(&run.kingdom.wishes);
    let (_, posts) = crown::herald(&geo).unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = ((posts[0].0 as f32 + 0.5) * TILE_UNITS, (posts[0].1 as f32 + 0.7) * TILE_UNITS);
    for _ in 0..30 {
        run.step(&BTreeMap::new());
    }
    let e = BTreeMap::from([(1, Input { interact: true, ..Input::default() })]);
    for _ in 0..BUY_HOLD / 2 {
        run.step(&e);
    }
    save(&run, "e-kingdom-herald-card");
    // A brawler and a hound: from the hall into the floor with the party.
    let (mut h, _) = held();
    h.owned[0].rooms = built(&["taproom", "kennels", "barracks", "range", "smithy"]);
    let mut run = below(h);
    run.dare = None;
    into_hall(&mut run);
    hold(&mut run, holdings::bay(1).1);
    stand(&mut run, holdings::bay(0).1);
    run.step(&BTreeMap::new());
    save(&run, "f-hall-hound-and-barracks");
}
