use super::*;
use crate::realm::{Spoil, Spoils};
use crate::rules::barony::{Court, FORGE_PLATE, Work, work};
use crate::rules::bounties::PINNED;
use crate::rules::holdings::{
    self, Muster, ROOM_LABOUR, STORES_CAP, WALLS, rent_day, testing::held,
};
use crate::rules::home::{BUY_HOLD, Home};

/// A realm whose King holds the workings and whose named forges burn.
fn burning(forges: &[&str]) -> Home {
    let mut home = Home::default();
    home.barony.court = Court::Sworn;
    home.barony.done.push("workings".into());
    for id in forges {
        home.barony.works.push(Work {
            id: (*id).into(),
            labour: work(id).unwrap().labour,
        });
        home.barony.lit.push((*id).into());
    }
    home
}

fn purse(list: &[(Spoil, u32)]) -> Spoils {
    let mut s = Spoils::default();
    for &(spoil, n) in list {
        s.add(spoil, n);
    }
    s
}

fn stocked() -> Spoils {
    purse(&[(Spoil::Wax, 40), (Spoil::Ember, 80), (Spoil::Bone, 40)])
}

const ALL: [&str; 5] = [
    "ore-forge",
    "bell-forge",
    "ember-forge",
    "type-foundry",
    "spore-kiln",
];

#[test]
fn the_mine_digs_and_the_ore_forge_smelts_it_into_ingots() {
    let mut home = burning(&["ore-forge"]);
    let mut t = Spoils::default();
    assert!(rent_days(&mut home, &mut t, 1));
    let g = &home.goods;
    // Eight ore dug; four batches of two ore, an ingot each.
    assert_eq!((g.have(Good::Ore), g.have(Good::Ingot)), (0, 4));
    assert_eq!(g.flow.get("mine"), Some(&Flow::Working));
    assert_eq!(g.burn(&home.barony, "ore-forge"), Burn::Working);
    rent_days(&mut home, &mut t, 1);
    assert_eq!(home.goods.have(Good::Ingot), 8);
    assert_eq!(home.goods.days, 2);
}

#[test]
fn nothing_is_dug_before_the_workings_are_his() {
    let mut home = Home::default();
    let mut t = Spoils::default();
    assert!(!rent_days(&mut home, &mut t, 3));
    assert!(home.goods.is_empty());
}

#[test]
fn a_stockpile_that_fills_stops_the_mine_and_the_miners_rest() {
    // The Ore-Forge is cold: the ore piles up to its cap, and the mine says so.
    let mut home = burning(&[]);
    let mut t = Spoils::default();
    for _ in 0..3 {
        rent_days(&mut home, &mut t, 1);
    }
    assert_eq!(home.goods.have(Good::Ore), Good::Ore.cap());
    assert!(home.goods.mine_digging(), "the last load fitted");
    rent_days(&mut home, &mut t, 1);
    assert!(home.goods.mine_full());
    assert!(!home.goods.mine_digging());
}

#[test]
fn a_forge_short_of_an_input_stalls_and_says_which() {
    // The Bell-Forge burns but nothing smelts the ingots: it is starved of
    // them, and it makes nothing.
    let mut home = burning(&["bell-forge"]);
    let mut t = stocked();
    rent_days(&mut home, &mut t, 1);
    assert_eq!(
        home.goods.burn(&home.barony, "bell-forge"),
        Burn::Starved("ingot".into())
    );
    assert_eq!(home.goods.have(Good::Bell), 0);
    assert_eq!(t.get(Spoil::Wax), 40, "a starved forge draws nothing");
    // With ingots but no wax, it is wax it wants.
    home.barony.lit.push("ore-forge".into());
    home.goods.stock.insert(Good::Ingot, 4);
    let mut bare = purse(&[(Spoil::Ember, 5)]);
    rent_days(&mut home, &mut bare, 1);
    assert_eq!(
        home.goods.burn(&home.barony, "bell-forge"),
        Burn::Starved("wax".into())
    );
}

#[test]
fn a_chain_forge_draws_ingots_and_spoils_and_makes_its_goods() {
    let mut home = burning(&["ore-forge", "bell-forge"]);
    home.goods.stock.insert(Good::Ingot, 4);
    let mut t = stocked();
    rent_days(&mut home, &mut t, 1);
    // The Ore-Forge adds four; the Bell-Forge takes two batches of two
    // ingots and a wax each, and the two bells hang at the holdings.
    assert_eq!(home.goods.have(Good::Ingot), 4 + 4 - 4);
    assert_eq!(t.get(Spoil::Wax), 38);
    assert_eq!(home.holdings.bells, 2);
    assert_eq!(home.goods.have(Good::Bell), 0);
    assert_eq!(home.goods.burn(&home.barony, "bell-forge"), Burn::Working);
}

#[test]
fn a_full_rack_idles_a_forge_without_drawing_anything() {
    let mut home = burning(&["ore-forge", "type-foundry"]);
    home.goods.stock.insert(Good::Ingot, 8);
    let mut t = stocked();
    // Six type a day onto a rack of twelve.
    rent_days(&mut home, &mut t, 2);
    assert_eq!(home.goods.have(Good::Type), 12);
    let bone = t.get(Spoil::Bone);
    rent_days(&mut home, &mut t, 1);
    assert_eq!(home.goods.burn(&home.barony, "type-foundry"), Burn::Full);
    assert_eq!(t.get(Spoil::Bone), bone);
}

#[test]
fn the_racks_empty_into_the_realm_while_it_has_room() {
    let mut home = burning(&["ore-forge", "bell-forge", "ember-forge"]);
    home.goods.stock.insert(Good::Ingot, 16);
    let mut t = stocked();
    for _ in 0..6 {
        rent_days(&mut home, &mut t, 1);
        home.goods.stock.insert(Good::Ingot, 16);
    }
    // Four bells hang; the rest wait on the rack, and then it is full.
    assert_eq!(home.holdings.bells, goods::BELLS_HUNG);
    assert_eq!(home.holdings.blades, goods::ARMOURY);
    assert_eq!(home.goods.have(Good::Bell), Good::Bell.cap());
    assert_eq!(home.goods.burn(&home.barony, "bell-forge"), Burn::Full);
}

#[test]
fn brew_pours_sacks_into_the_granary_until_a_whole_brew_will_not_fit() {
    let mut home = burning(&["spore-kiln"]);
    let mut t = stocked();
    rent_days(&mut home, &mut t, 1);
    // Six spores grown, two batches of three, an ember each.
    assert_eq!(home.holdings.stores, 2 * BREW_SACKS);
    assert_eq!(t.get(Spoil::Ember), 78);
    home.holdings.stores = STORES_CAP - 1;
    rent_days(&mut home, &mut t, 1);
    assert_eq!(
        home.holdings.stores,
        STORES_CAP - 1,
        "no room for a whole brew"
    );
    assert_eq!(home.goods.have(Good::Brew), 2);
}

#[test]
fn a_banked_forge_draws_nothing_and_a_stoked_one_works_again() {
    let mut home = burning(&["ore-forge", "bell-forge"]);
    home.goods.stock.insert(Good::Ingot, 4);
    let mut t = stocked();
    let mut treasury = Spoils::default();
    let mut realm = activity::Realm {
        home: &mut home,
        treasury: &mut treasury,
        reclaimed: &BTreeMap::new(),
        raids_won: 0,
    };
    let marks = BTreeMap::from([("goods:bank:bell-forge:1".to_string(), 1)]);
    let out = settle(&mut realm, &marks);
    assert!(out.changed);
    assert!(out.said[0].1.contains("banked"));
    assert!(home.goods.is_banked("bell-forge"));
    rent_days(&mut home, &mut t, 1);
    assert_eq!(t.get(Spoil::Wax), 40);
    assert_eq!(home.goods.burn(&home.barony, "bell-forge"), Burn::Banked);
    let mut realm = activity::Realm {
        home: &mut home,
        treasury: &mut treasury,
        reclaimed: &BTreeMap::new(),
        raids_won: 0,
    };
    let out = settle(&mut realm, &marks);
    assert!(out.said[0].1.contains("stoked"));
    assert_eq!(home.goods.burn(&home.barony, "bell-forge"), Burn::Kindled);
    // A forge that is not lit cannot be banked.
    let mut realm = activity::Realm {
        home: &mut home,
        treasury: &mut treasury,
        reclaimed: &BTreeMap::new(),
        raids_won: 0,
    };
    let cold = BTreeMap::from([("goods:bank:spore-kiln:1".to_string(), 1)]);
    assert!(!settle(&mut realm, &cold).changed);
}

#[test]
fn a_rent_day_is_a_floor_cleared_and_pay_tribute_runs_it() {
    let mut home = burning(&["ore-forge"]);
    let mut t = Spoils::default();
    barony::pay_tribute(&mut home, &mut t, 0);
    assert_eq!(home.goods.days, 0);
    barony::pay_tribute(&mut home, &mut t, 2);
    assert_eq!(home.goods.days, 2);
    assert_eq!(home.goods.have(Good::Ingot), 8);
    // A settlement never runs more than a delve's worth at once.
    barony::pay_tribute(&mut home, &mut t, 100);
    assert_eq!(home.goods.days, 2 + DAYS_MAX as u32);
}

#[test]
fn the_whole_chain_runs_for_a_long_while_and_never_overfills() {
    let mut home = burning(&ALL);
    let mut t = stocked();
    for day in 0..40 {
        rent_days(&mut home, &mut t, 1);
        // The realm wears its goods out: bands, wages, notices.
        if day % 5 == 4 {
            home.holdings.bells = 0;
            home.holdings.blades = 0;
            home.holdings.stores = 0;
            home.goods.take(Good::Type, 6);
        }
        for good in Good::ALL {
            assert!(home.goods.have(good) <= good.cap(), "{good:?} on day {day}");
        }
        assert!(home.holdings.valid());
        assert!(home.holdings.stores <= STORES_CAP);
    }
    // Every forge has had a shift, and the Ore-Forge's ingots are what
    // limit the rest: some forge went short of them at some point, so the
    // rack of ingots is not simply full.
    for id in ALL {
        assert!(home.goods.flow.contains_key(id), "{id}");
    }
}

#[test]
fn what_old_saves_hold_loads_and_goods_round_trip() {
    let old: Home = serde_json::from_str("{}").unwrap();
    assert!(old.goods.is_empty());
    let json = serde_json::to_string(&old).unwrap();
    assert!(!json.contains("goods"), "an empty store is not written");
    let mut home = burning(&["ore-forge"]);
    let mut t = Spoils::default();
    rent_days(&mut home, &mut t, 2);
    home.goods.banked.push("bell-forge".into());
    let again: Home = serde_json::from_str(&serde_json::to_string(&home).unwrap()).unwrap();
    assert_eq!(again.goods, home.goods);
    // A newer build's unknown good resets the store, not the realm.
    let mut value = serde_json::to_value(&home).unwrap();
    value["goods"]["stock"] = serde_json::json!({"mithril": 3});
    let lossy: Home = serde_json::from_value(value).unwrap();
    assert!(lossy.goods.is_empty());
    assert_eq!(lossy.barony, home.barony);
}

#[test]
fn bells_and_blades_cover_what_the_walls_fall_short_of() {
    // Nothing short, nothing used.
    assert_eq!(cover(0, 3, 3), Some((0, 0)));
    // A bell calls two; the fewest items win, bells spared.
    assert_eq!(cover(2, 4, 0), Some((1, 0)));
    assert_eq!(cover(2, 0, 4), Some((0, 2)));
    assert_eq!(cover(3, 4, 4), Some((1, 1)));
    assert_eq!(cover(7, 4, 0), Some((4, 0)));
    assert_eq!(cover(9, 4, 0), None);
    assert_eq!(cover(9, 4, 1), Some((4, 1)));
    assert_eq!(cover(1, 0, 0), None);
}

/// A worth-robbing Crypt with a brave steward, so the walls only wear to
/// the band, and the next band `days` off.
fn muster_against(strength: u32, walls: u32) -> holdings::Holdings {
    let (mut h, _) = held();
    h.owned[0].rooms = ["taproom", "shrine", "smithy"]
        .iter()
        .map(|id| holdings::Built {
            id: id.to_string(),
            labour: ROOM_LABOUR,
        })
        .collect();
    h.owned[0].steward = Some("gwen".into());
    h.owned[0].walls = walls;
    h.stores = STORES_CAP;
    h.muster = Some(Muster {
        key: h.owned[0].key.clone(),
        days: 1,
        strength,
    });
    h
}

#[test]
fn bells_turn_a_band_the_walls_would_lose_to() {
    // Seven courses and Gwen's two against ten raiders: one short.
    let mut h = muster_against(10, 7);
    assert_eq!(h.defence(&h.owned[0]), 9);
    let said = rent_day(&mut h, 1);
    assert!(
        said.iter().any(|l| l.contains("fell to 10 raiders")),
        "{said:?}"
    );
    // The same band against the same walls with a bell hung.
    let mut h = muster_against(10, 7);
    h.bells = 2;
    assert_eq!(h.defence(&h.owned[0]), 13);
    let said = rent_day(&mut h, 1);
    assert!(
        said.iter()
            .any(|l| l
                .contains("held: 10 raiders broke on its walls; 1 bell rang the folk to the wall")),
        "{said:?}"
    );
    assert_eq!(h.bells, 1, "the bell that rang is spent");
    assert_eq!(h.owned[0].raid, 0);
}

#[test]
fn blades_arm_the_men_there_are_and_no_more() {
    // Two garrison men (Gwen's) can carry two blades, however many are in
    // the armoury.
    let mut h = muster_against(11, 7);
    h.blades = 6;
    assert_eq!(h.defence(&h.owned[0]), 7 + 2 + 2);
    let said = rent_day(&mut h, 1);
    assert!(
        said.iter().any(|l| l.contains("held: 11 raiders")),
        "{said:?}"
    );
    assert!(
        said.iter()
            .any(|l| l.contains("2 forge blades armed its men")),
        "{said:?}"
    );
    assert_eq!(h.blades, 4);
    // Three short is more than two blades make good.
    let mut h = muster_against(12, 7);
    h.blades = 6;
    let said = rent_day(&mut h, 1);
    assert!(
        said.iter().any(|l| l.contains("fell to 12 raiders")),
        "{said:?}"
    );
    assert_eq!(
        h.blades, 6,
        "blades that did not save the day are not spent"
    );
}

#[test]
fn a_band_the_walls_already_hold_costs_the_forges_nothing() {
    let mut h = muster_against(6, WALLS);
    h.bells = 3;
    h.blades = 3;
    rent_day(&mut h, 1);
    assert_eq!((h.bells, h.blades), (3, 3));
}

#[test]
fn type_prints_wrens_extra_notices_and_costs_type() {
    let mut home = Home {
        deepest: 6,
        ..Home::default()
    };
    assert_eq!(home.pin_bounties().len(), PINNED);
    // No type, no more notices.
    assert!(home.pin_bounties().is_empty());
    home.goods.stock.insert(Good::Type, TYPE_PER_NOTICE * 2 + 1);
    let printed = home.pin_bounties();
    assert_eq!(printed.len(), NOTICES_PRINTED);
    assert_eq!(home.bounties.len(), PINNED + NOTICES_PRINTED);
    assert_eq!(home.goods.have(Good::Type), 1);
    // A finished notice comes down; a new one is printed only with type.
    home.bounties.pop();
    assert!(home.pin_bounties().is_empty());
    home.bounties.pop();
    home.bounties.pop();
    home.goods.stock.insert(Good::Type, TYPE_PER_NOTICE);
    // Back to three, which are free; the fourth takes the type.
    assert_eq!(home.pin_bounties().len(), 1 + 1);
    assert_eq!(home.goods.have(Good::Type), 0);
}

// ── The run's half ───────────────────────────────────────────────────────

fn hold(run: &mut Run, plate: (i32, i32, i32, i32)) {
    let (c, r, w, h) = plate;
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (c as f32 + w as f32 / 2.0) * TILE_UNITS;
    hero.y = (r as f32 + h as f32 / 2.0) * TILE_UNITS;
    let fire = BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Input::default()
        },
    )]);
    for _ in 0..BUY_HOLD + 1 {
        run.step(&fire);
    }
    run.step(&BTreeMap::new());
}

fn forge_run(home: Home, pack: Pack) -> Run {
    let mut run = Run::at_home(7, 1, None, home, stocked());
    run.dungeon.pack = pack;
    run.descend_for_test();
    let forge = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Forge)
        .expect("a forge-hall");
    run.enter_for_test(forge);
    run.marks.clear();
    run
}

#[test]
fn holding_e_on_a_lit_forges_plate_marks_a_bank_and_an_unlit_forges_does_not() {
    let mut run = forge_run(burning(&["ore-forge"]), Pack::Cavern);
    hold(&mut run, FORGE_PLATE);
    assert!(
        run.marks.contains_key("goods:bank:ore-forge:1"),
        "{:?}",
        run.marks
    );
    assert!(!run.marks.keys().any(|m| m.starts_with("barony:relight")));
    // The Crypt's forge is rebuilt but cold: its plate relights, not banks.
    let mut home = burning(&[]);
    home.barony.works.push(Work {
        id: "bell-forge".into(),
        labour: work("bell-forge").unwrap().labour,
    });
    let mut run = forge_run(home, Pack::Crypt);
    hold(&mut run, FORGE_PLATE);
    assert!(
        run.marks.contains_key("barony:relight:bell-forge:1"),
        "{:?}",
        run.marks
    );
    assert!(!run.marks.keys().any(|m| m.starts_with("goods:")));
}

#[test]
fn every_forge_hall_has_its_bins_as_walls_to_walk_round() {
    for pack in [
        Pack::Cavern,
        Pack::Crypt,
        Pack::Hellforge,
        Pack::Archive,
        Pack::Fungal,
    ] {
        let run = forge_run(burning(&[]), pack);
        let room = run.room();
        for (c, r, w, h) in [IN_BIN, OUT_BIN] {
            assert_eq!(room.tile(c, r), Tile::Block, "{pack:?}");
            assert_eq!(room.tile(c + w - 1, r + h - 1), Tile::Block, "{pack:?}");
        }
        assert!(run.valid_snapshot(), "{pack:?}");
    }
}

#[test]
fn the_smiths_hammer_only_while_the_forge_works_and_the_miners_rest_when_the_pile_is_full() {
    use crate::rules::folk::{Folk, Pose, figures};
    let mut home = burning(&["ore-forge"]);
    let pose = |home: &Home| {
        let smiths: Vec<_> = figures(home, RoomKind::Forge, Pack::Cavern, 40)
            .into_iter()
            .filter(|f| f.folk == Folk::Smith)
            .collect();
        assert_eq!(smiths.len(), 2);
        smiths[0].pose
    };
    assert_eq!(pose(&home), Pose::Idle, "lit, nothing made yet");
    let mut t = Spoils::default();
    rent_days(&mut home, &mut t, 1);
    assert_eq!(pose(&home), Pose::Work);
    // A full pile: the miners stand by the heap instead of walking the shaft.
    let mut full = burning(&[]);
    for _ in 0..4 {
        rent_days(&mut full, &mut t, 1);
    }
    assert!(full.goods.mine_full());
    for tick in (0..HZ as u64 * 90).step_by(11) {
        for f in figures(&full, RoomKind::MineHead, Pack::Cavern, tick) {
            if f.folk == Folk::Miner {
                assert_eq!(f.pose, Pose::Idle);
            }
        }
    }
}

// ── Through the King's own plates ────────────────────────────────────────

#[test]
fn two_forges_are_ordered_built_relit_and_then_make_goods_every_rent_day() {
    use crate::rules::barony::testing::{marks, rich, who};
    let mut home = Home::default();
    home.barony.court = Court::Met;
    home.barony.done.push("workings".into());
    let mut t = rich();
    t.add(Spoil::Wax, 20);
    t.add(Spoil::Ember, 40);
    for id in ["ore-forge", "bell-forge"] {
        let order = format!("barony:order:{id}:1");
        barony::settle(&mut home, &mut t, 1, &marks(&[(order.as_str(), 1)]), &who);
        assert_eq!(home.barony.labour(id), Some(0), "{id} paid for");
    }
    let shift = barony::SHIFT_SECS * barony::crew(&home.barony);
    for _ in 0..4 * 3 * 30 * 60 / shift {
        barony::settle(
            &mut home,
            &mut t,
            1,
            &marks(&[("barony:labour", barony::SHIFT_SECS)]),
            &who,
        );
    }
    for id in ["ore-forge", "bell-forge"] {
        assert!(home.barony.built(id), "{id} stands");
        let relight = format!("barony:relight:{id}:1");
        barony::settle(&mut home, &mut t, 1, &marks(&[(relight.as_str(), 1)]), &who);
        assert!(home.barony.is_lit(id), "{id} burns");
    }
    // A delve to the Keep: three floors cleared, three shifts.
    let wax = t.get(Spoil::Wax);
    let tribute = barony::pay_tribute(&mut home, &mut t, 3).expect("a tribute");
    assert!(tribute.get(Spoil::Gold) > 0);
    // Ore to ingots, ingots and wax to bells, hung at the holdings.
    assert_eq!(home.goods.days, 3);
    assert_eq!(home.holdings.bells, 4);
    assert_eq!(
        t.get(Spoil::Wax),
        wax - 6,
        "a wax a bell: four hung, two on the rack"
    );
    assert_eq!(home.goods.have(Good::Bell), 2);
    assert_eq!(home.goods.burn(&home.barony, "bell-forge"), Burn::Working);
}
