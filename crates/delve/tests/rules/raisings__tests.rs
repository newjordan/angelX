use super::*;
use crate::rules::home_settlement::settle as settle_home;

fn run() -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.walk_kingdom(Some(crate::world::kingdom::keep_stand()));
    r
}

fn rich() -> Spoils {
    let mut treasury = Spoils::default();
    for spoil in Spoil::ALL {
        treasury.add(spoil, 100_000);
    }
    treasury
}

fn press(r: &mut Run, input: impl Fn(&mut Input)) {
    let mut held = Input::default();
    input(&mut held);
    r.step(&BTreeMap::from([(1, held)]));
    r.step(&BTreeMap::new());
}

/// A knight stands where plan `pick` may rise, the rod up.
fn at_ground(r: &mut Run, pick: u8) -> (f32, f32) {
    r.treasury = rich();
    r.kingdom.survey.insert(1, Survey { pick, hold: 0 });
    for ty in 8..168 {
        for tx in 8..232 {
            let at = ((tx as f32 + 0.5) * TILE_UNITS, (ty as f32 + 0.7) * TILE_UNITS);
            let hero = r.players.get_mut(&1).unwrap();
            (hero.x, hero.y) = at;
            if r.survey_target(1).is_some_and(|t| t.refused.is_none()) {
                return at;
            }
        }
    }
    panic!("no ground for plan {pick}");
}

fn raised(plan: &str, tx: i32, ty: i32, level: u8) -> Raised {
    Raised { plan: plan.into(), tx, ty, level, by: "Host".into() }
}

fn order(plan: &str, tx: i32, ty: i32) -> Order {
    Order { plan: plan.into(), tx, ty, by: "Host".into() }
}

fn motes(r: &Run) -> u32 {
    r.bank.iter().map(|h| h.spoils.get(Spoil::Wishmote)).sum::<u32>()
        + r.players.values().map(|h| h.carried.get(Spoil::Wishmote)).sum::<u32>()
}

#[test]
fn a_stair_walked_down_and_back_up_makes_no_wishmotes() {
    let mut r = Run::at_home(3, 1, None, Default::default(), Default::default());
    r.descend_for_test();
    assert_eq!(r.dungeon.depth, 1);
    assert!(r.players[&1].wishes > 0);
    r.retreat();
    assert_eq!(motes(&r), 0);
}

#[test]
fn a_delves_end_turns_unspent_wishes_to_motes_within_the_floors_behind_it() {
    let mut r = Run::at_home(3, 1, None, Default::default(), Default::default());
    r.descend_for_test();
    r.dungeon.depth = 3;
    let hero = r.players.get_mut(&1).unwrap();
    (hero.wishes, hero.great_wishes) = (6, 1);
    r.wishes_home(false);
    let hero = &r.players[&1];
    assert_eq!(hero.carried.get(Spoil::Wishmote), 2, "two floors are behind the party");
    assert_eq!((hero.wishes, hero.great_wishes), (0, 0));
    // Nothing is left to turn twice.
    r.wishes_home(false);
    assert_eq!(r.players[&1].carried.get(Spoil::Wishmote), 2);
}

#[test]
fn a_cleared_floor_banks_the_haul_and_leaves_the_wishes_to_spend() {
    let mut r = Run::at_home(3, 1, None, Default::default(), Default::default());
    r.descend_for_test();
    let before = r.players[&1].wishes;
    r.boss_wishes();
    assert_eq!(r.players[&1].carried.get(Spoil::Wishmote), 1);
    assert_eq!(r.players[&1].great_wishes, 1);
    r.bank_all(true, "floor 1 cleared");
    assert_eq!(r.players[&1].wishes, before);
    assert_eq!(r.bank.iter().map(|h| h.spoils.get(Spoil::Wishmote)).sum::<u32>(), 1);
}

#[test]
fn a_wiped_party_carries_half_its_motes_home() {
    let mut r = Run::at_home(3, 1, None, Default::default(), Default::default());
    r.descend_for_test();
    r.players.get_mut(&1).unwrap().carried.add(Spoil::Wishmote, 5);
    r.bank_all(false, "the party fell");
    assert_eq!(r.bank.iter().map(|h| h.spoils.get(Spoil::Wishmote)).sum::<u32>(), 2);
}

#[test]
fn a_plan_is_paid_once_then_climbs_to_its_top_level() {
    let (mut standing, mut treasury) = (Vec::new(), rich());
    let shrine = order("wayside-shrine", 40, 40);
    let plan = plan("wayside-shrine").unwrap();
    let start = treasury.get(Spoil::Wishmote);
    assert!(settle(&mut standing, &mut treasury, std::slice::from_ref(&shrine)).0);
    assert_eq!(standing[0].level, 1);
    assert_eq!(start - treasury.get(Spoil::Wishmote), 3);
    assert!(settle(&mut standing, &mut treasury, &[shrine.clone(), shrine.clone()]).0);
    assert_eq!(standing[0].level, TOP_LEVEL);
    assert_eq!(start - treasury.get(Spoil::Wishmote), 3 + 6 + 9);
    assert_eq!(plan.price(2).get(Spoil::Wishmote), 9);
    let spent = treasury.clone();
    assert!(!settle(&mut standing, &mut treasury, std::slice::from_ref(&shrine)).0);
    assert_eq!(treasury, spent, "a top level takes nothing");
    assert_eq!(standing.len(), 1);
}

#[test]
fn the_treasury_decides_and_refuses_with_what_is_short() {
    let (mut standing, mut treasury) = (Vec::new(), Spoils::default());
    treasury.add(Spoil::Gold, 50);
    treasury.add(Spoil::Bond, 6);
    let (changed, said) = settle(&mut standing, &mut treasury, &[order("fellowship-stone", 40, 40)]);
    assert!(!changed);
    assert!(said[0].contains("wishmote"), "{said:?}");
    assert_eq!(treasury.get(Spoil::Gold), 50);
    assert!(standing.is_empty());
}

#[test]
fn two_seats_on_one_lot_raise_one_building_and_pay_once() {
    let (mut standing, mut treasury) = (Vec::new(), rich());
    let start = treasury.get(Spoil::Gold);
    settle(&mut standing, &mut treasury, &[order("tavern", 40, 40), order("miners-lodge", 41, 41)]);
    assert_eq!(standing.len(), 1);
    assert_eq!(start - treasury.get(Spoil::Gold), 950);
}

#[test]
fn a_friend_beside_a_raising_is_worth_a_level() {
    let far = BESIDE + 30;
    let near = [raised("tavern", 40, 40, 2), raised("miners-lodge", 44, 40, 1)];
    assert_eq!(strength(&near, 0), 3);
    assert_eq!(strength(&near, 1), 2);
    let apart = [raised("tavern", 40, 40, 2), raised("miners-lodge", 40 + far, 40, 1)];
    assert_eq!(strength(&apart, 0), 2);
}

#[test]
fn every_plan_has_art_that_fits_a_wish_and_a_gift_of_its_own() {
    for (i, plan) in PLANS.iter().enumerate() {
        let art = plan.art();
        assert!(!art.is_empty() && art.len() <= 32, "{}", plan.id);
        assert!(art.iter().all(|r| r.chars().count() <= 32), "{}", plan.id);
        assert!(self::plan(plan.friend).is_some_and(|f| f.id != plan.id), "{}", plan.id);
        assert!(PLANS[..i].iter().all(|p| p.gift != plan.gift), "{} repeats a gift", plan.id);
        assert!(plan.price(0).get(Spoil::Wishmote) > 0, "{} needs no delve", plan.id);
    }
}

#[test]
fn a_chosen_lot_stands_where_it_was_chosen_and_the_map_walls_it() {
    let mut r = run();
    at_ground(&mut r, 0);
    let target = r.survey_target(1).unwrap();
    let standing_now = standing(&[raised(target.plan.id, target.lot.tx, target.lot.ty, 1)]);
    let lots = wishes::lots(&standing_now);
    assert_eq!(lots[0], Some(target.lot));
    let geo = map::geography(&standing_now);
    assert_ne!(geo.room.tile(target.lot.tx, target.lot.ty), Tile::Floor);
    // The lot and a tile round it are taken now.
    assert!(!wishes::open_for(&standing_now, target.lot));
}

#[test]
fn the_realm_builds_its_own_wishes_around_a_chosen_lot() {
    let mut r = run();
    at_ground(&mut r, 0);
    let target = r.survey_target(1).unwrap();
    let mut wishes_there = standing(&[raised(target.plan.id, target.lot.tx, target.lot.ty, 1)]);
    let mut wish = wishes_there[0].clone();
    wish.id = "found".into();
    wish.lot = None;
    wish.near = "keep".into();
    wishes_there.insert(0, wish);
    let lots = wishes::lots(&wishes_there);
    let (found, chosen) = (lots[0].unwrap(), lots[1].unwrap());
    assert_eq!(chosen, target.lot);
    assert!(!found.overlaps(&chosen));
}

#[test]
fn the_rod_comes_up_moves_through_the_plans_and_is_put_away() {
    let mut r = run();
    press(&mut r, |i| i.cycle = true);
    assert_eq!(r.kingdom.survey[&1].pick, 0);
    for pick in 1..PLANS.len() {
        press(&mut r, |i| i.cycle = true);
        assert_eq!(usize::from(r.kingdom.survey[&1].pick), pick);
    }
    press(&mut r, |i| i.cycle = true);
    assert!(!r.kingdom.survey.contains_key(&1));
    assert!(r.valid_snapshot());
}

#[test]
fn one_held_key_is_one_plan() {
    let mut r = run();
    let held = BTreeMap::from([(1, Input { cycle: true, ..Input::default() })]);
    for _ in 0..30 {
        r.step(&held);
    }
    assert_eq!(r.kingdom.survey[&1].pick, 0);
}

#[test]
fn e_held_on_open_ground_orders_the_plan_and_a_release_is_wanted_for_another() {
    let mut r = run();
    at_ground(&mut r, 3);
    let lot = r.survey_target(1).unwrap().lot;
    let held = BTreeMap::from([(1, Input { interact: true, ..Input::default() })]);
    for _ in 0..home::BUY_HOLD * 3 {
        r.tick_survey(&held);
    }
    assert_eq!(
        r.raise_orders,
        vec![Order { plan: "fellowship-stone".into(), tx: lot.tx, ty: lot.ty, by: r.players[&1].name.clone() }]
    );
}

#[test]
fn a_short_hold_or_a_refused_spot_orders_nothing() {
    let mut r = run();
    at_ground(&mut r, 0);
    let held = BTreeMap::from([(1, Input { interact: true, ..Input::default() })]);
    for _ in 0..home::BUY_HOLD - 1 {
        r.tick_survey(&held);
    }
    r.tick_survey(&BTreeMap::new());
    assert_eq!(r.kingdom.survey[&1].hold, 0);
    assert!(r.raise_orders.is_empty());
    r.treasury = Spoils::default();
    for _ in 0..home::BUY_HOLD {
        r.tick_survey(&held);
    }
    assert!(r.raise_orders.is_empty());
    assert!(r.survey_target(1).unwrap().refused.unwrap().contains("NEED"));
}

#[test]
fn a_rod_is_not_a_door() {
    let mut r = run();
    let geo = map::geography(&r.kingdom.wishes);
    let site = geo.sites.iter().find(|s| s.room != RoomKind::Gate).unwrap();
    let hero = r.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = ((site.stand.0 as f32 + 0.5) * TILE_UNITS, (site.stand.1 as f32 + 0.7) * TILE_UNITS);
    r.kingdom.survey.insert(1, Survey::default());
    press(&mut r, |i| i.interact = true);
    assert_eq!(r.room().kind, RoomKind::Kingdom);
}

#[test]
fn standing_near_a_raising_of_the_plan_offers_its_next_level() {
    let mut r = run();
    at_ground(&mut r, 0);
    let lot = r.survey_target(1).unwrap().lot;
    r.home.raised.push(raised("wayside-shrine", lot.tx, lot.ty, 1));
    let target = r.survey_target(1).unwrap();
    assert_eq!((target.level, target.lot), (2, lot));
    assert_eq!(target.price.get(Spoil::Wishmote), 6);
    r.home.raised[0].level = TOP_LEVEL;
    assert!(r.survey_target(1).unwrap().refused.is_some());
}

#[test]
fn a_full_realm_refuses_a_new_plan_and_the_door_of_a_place_is_no_ground() {
    let mut r = run();
    at_ground(&mut r, 0);
    r.home.raised = (0..MOST_RAISED as i32).map(|i| raised("tavern", i * 40, 200, 1)).collect();
    assert_eq!(r.survey_target(1).unwrap().refused.as_deref(), Some("THE REALM IS FULL"));
    r.home.raised.clear();
    let geo = map::geography(&r.kingdom.wishes);
    for site in &geo.sites {
        let lot = PLANS[0].lot((site.stand.0 - 1, site.stand.1 - 1));
        assert!(!r.ground_for(lot), "{}", site.name);
    }
}

#[test]
fn raisings_go_down_the_stair_with_the_party() {
    let mut r = Run::at_home(3, 1, None, Default::default(), Default::default());
    r.home.raised = vec![
        raised("wayside-shrine", 40, 40, 2),
        raised("miners-lodge", 80, 80, 1),
        raised("fellowship-stone", 120, 40, 1),
        raised("beacon-tower", 44, 40, 1),
        raised("tavern", 120, 120, 1),
    ];
    r.descend_for_test();
    let hero = &r.players[&1];
    // The shrine's two levels and its friend the beacon beside it.
    assert_eq!(hero.wishes, overclass::START_WISHES + 3);
    assert!(hero.armor >= 1);
    assert!(hero.invulnerable >= WARD);
    assert_eq!(hero.carried.get(Spoil::Gold), 0, "a stair walked pays no gold");
    r.raised_gifts(false);
    assert_eq!(r.players[&1].carried.get(Spoil::Gold), PURSE);
}

#[test]
fn nothing_raised_gives_nothing() {
    let mut r = Run::at_home(3, 1, None, Default::default(), Default::default());
    r.descend_for_test();
    let hero = r.players[&1].clone();
    r.raised_gifts(true);
    r.raised_gifts(false);
    assert_eq!(r.players[&1].wishes, hero.wishes);
    assert_eq!(r.players[&1].armor, hero.armor);
    assert_eq!(r.players[&1].bombs, hero.bombs);
}

#[test]
fn an_order_settles_through_the_home_claim_and_comes_back_raised() {
    let mut r = run();
    let shrine = order("wayside-shrine", 40, 40);
    r.raise_orders.push(shrine);
    let mut journal = host_commands::Journal::new("session-a".into(), r.raid_id);
    let claim = journal.claim_home(&mut r).unwrap().unwrap();
    let (mut home, mut treasury) = (home::Home::default(), rich());
    let result = settle_home(
        &mut activity::Realm { home: &mut home, treasury: &mut treasury, reclaimed: &Default::default(), raids_won: 0 },
        &claim.transfer,
        "Host",
        false,
    )
    .unwrap();
    assert_eq!(result.home.raised.len(), 1);
    assert_eq!(rich().get(Spoil::Wishmote) - result.treasury.get(Spoil::Wishmote), 3);
    journal.complete_home(&mut r, &claim, result).unwrap();
    assert!(r.raise_orders.is_empty());
    assert_eq!(r.home.raised.len(), 1);
    assert_eq!(standing(&r.home.raised)[0].lot, Some((40, 40)));
}

#[test]
fn a_save_from_before_the_raisings_loads() {
    let home = serde_json::json!({ "deepest": 4 });
    let home: home::Home = serde_json::from_value(home).unwrap();
    assert!(home.raised.is_empty());
    assert_eq!(home.deepest, 4);
    let old = serde_json::json!({ "id": "a", "name": "A", "words": "a", "by": "b", "status": "built" });
    let wish: Wish = serde_json::from_value(old).unwrap();
    assert_eq!(wish.lot, None);
    assert!(!serde_json::to_string(&wish).unwrap().contains("lot"));
    let spoils: Spoils = serde_json::from_value(serde_json::json!({ "gold": 1 })).unwrap();
    assert_eq!(spoils.get(Spoil::Wishmote), 0);
    assert_eq!(Spoil::from_word("wishmotes"), Some(Spoil::Wishmote));
}

#[test]
fn the_rod_stands_its_ghost_and_panel_in_the_view() {
    use crate::render::overworld::arena;
    let mut r = run();
    at_ground(&mut r, 1);
    r.follow_kingdom_cameras();
    for _ in 0..60 {
        r.follow_kingdom_cameras();
    }
    let with = arena::frame_for(&r, arena::NATIVE_W, arena::NATIVE_H, Some(1));
    if let Some(path) = std::env::var_os("ANGEL_RAISINGS_SHOT") {
        let rgb: Vec<u8> = (0..with.h)
            .flat_map(|y| (0..with.w).map(move |x| (x, y)))
            .flat_map(|(x, y)| with.get(x, y).unwrap_or([0, 0, 0]))
            .collect();
        image::save_buffer(path, &rgb, with.w as u32, with.h as u32, image::ColorType::Rgb8).unwrap();
    }
    let mut quiet = r.clone();
    quiet.kingdom.survey.clear();
    let without = arena::frame_for(&quiet, arena::NATIVE_W, arena::NATIVE_H, Some(1));
    let lot = r.survey_target(1).unwrap().lot;
    let origin = arena::camera(&r, Some(1));
    let on_screen = (lot.tx * crate::world::map::TILE - origin.0, lot.ty * crate::world::map::TILE - origin.1);
    let at = |im: &crate::render::overworld::ink::Img, x, y| im.get(on_screen.0 + x, on_screen.1 + y);
    assert_ne!(at(&with, 0, 0), at(&without, 0, 0), "the lot is framed");
    // The panel, at the top of the view.
    assert_ne!(with.get(arena::NATIVE_W / 2, 12), without.get(arena::NATIVE_W / 2, 12));
}
