use super::*;
use crate::kingdom::{Claim, Command, catalog};

fn home() -> Run {
    let mut r = Run::at_home(
        41,
        1,
        Some("Friend"),
        Default::default(),
        Default::default(),
    );
    r.walk_kingdom(Some(crate::world::kingdom::keep_stand()));
    r
}
fn press(r: &mut Run, id: u32, input: Input) {
    r.step(&BTreeMap::from([(id, input)]));
    r.step(&BTreeMap::new());
}

#[test]
fn pointer_builds_consecutive_plots_without_repeating_keys_or_moving_camera() {
    use crate::rules::builder_input::BuilderAction;
    let mut r = home();
    press(
        &mut r,
        1,
        Input {
            cast: 4,
            ..Default::default()
        },
    );
    let click = |r: &mut Run, action| {
        r.step(&BTreeMap::from([(
            1,
            Input {
                builder: Some(action),
                ..Default::default()
            },
        )]));
    };
    click(&mut r, BuilderAction::Plan(1));
    click(
        &mut r,
        BuilderAction::Plot {
            tile: (8, 8),
            build: true,
        },
    );
    click(
        &mut r,
        BuilderAction::Plot {
            tile: (7, 8),
            build: true,
        },
    );
    assert!(r.home.industry.at((8, 8)).is_some());
    assert!(r.home.industry.at((7, 8)).is_some());
    assert_eq!(r.kingdom.industry[&1].camera, Some((8, 8)));
    let stock = r.home.industry.stock.clone();
    click(
        &mut r,
        BuilderAction::Plot {
            tile: (6, 8),
            build: false,
        },
    );
    assert!(r.home.industry.at((6, 8)).is_none());
    assert_eq!(r.home.industry.stock, stock);
    assert_eq!(r.kingdom.industry[&1].tile, (6, 8));
    for _ in 0..5 {
        r.step(&BTreeMap::from([(
            1,
            Input {
                move_y: -1,
                ..Default::default()
            },
        )]));
    }
    assert_eq!(r.kingdom.industry[&1].camera, None);
    assert!(r.valid_snapshot());
}

#[test]
fn pointer_commands_require_an_open_desk_and_revalidate_server_bounds() {
    use crate::rules::builder_input::BuilderAction;
    let mut r = home();
    let send = |r: &mut Run, action| {
        r.step(&BTreeMap::from([(
            1,
            Input {
                builder: Some(action),
                ..Default::default()
            },
        )]));
    };
    send(
        &mut r,
        BuilderAction::Plot {
            tile: (8, 8),
            build: true,
        },
    );
    assert!(r.home.industry.is_empty());
    press(
        &mut r,
        1,
        Input {
            cast: 4,
            ..Default::default()
        },
    );
    send(&mut r, BuilderAction::Page(8));
    send(&mut r, BuilderAction::Selection(511));
    assert_eq!(r.kingdom.industry[&1].selection, 0);
    let before = r.kingdom.industry[&1].tile;
    send(
        &mut r,
        BuilderAction::Plot {
            tile: (255, 255),
            build: true,
        },
    );
    send(&mut r, BuilderAction::Page(9));
    assert_eq!(r.kingdom.industry[&1].tile, before);
    assert_eq!(r.kingdom.industry[&1].page, 8);
    send(&mut r, BuilderAction::Page(0));
    send(&mut r, BuilderAction::Plan(1));
    send(
        &mut r,
        BuilderAction::Plot {
            tile: (8, 8),
            build: true,
        },
    );
    let stock = r.home.industry.stock.clone();
    send(
        &mut r,
        BuilderAction::Plot {
            tile: (8, 8),
            build: true,
        },
    );
    assert_eq!(
        r.home.industry.stock, stock,
        "an occupied plot is inspected without purchasing it again"
    );
    assert!(r.valid_snapshot());
}

#[test]
fn unused_claim_release_needs_two_separate_presses_and_keeps_earned_progress() {
    use crate::rules::builder_input::BuilderAction;
    let mut r = home();
    press(
        &mut r,
        1,
        Input {
            cast: 4,
            ..Default::default()
        },
    );
    assert!(r.home.industry.discover(
        catalog(),
        "retire-iron".into(),
        crate::kingdom::Claim {
            resource: "iron-ore".into(),
            name: "Old Ironroot".into(),
            source: "test seam".into(),
            secured: false,
            rate: 2,
        },
    ));
    assert!(r.home.industry.secure("retire-iron"));
    press(
        &mut r,
        1,
        Input {
            builder: Some(BuilderAction::Page(3)),
            ..Default::default()
        },
    );
    let release = Input {
        bomb: true,
        ..Default::default()
    };
    r.step(&BTreeMap::from([(1, release)]));
    assert!(r.home.industry.claims.contains_key("retire-iron"));
    assert_eq!(
        r.kingdom.industry[&1].pending_release.as_deref(),
        Some("retire-iron")
    );
    r.step(&BTreeMap::from([(1, release)]));
    assert!(
        r.home.industry.claims.contains_key("retire-iron"),
        "holding X cannot confirm its own release"
    );
    r.step(&BTreeMap::new());
    press(&mut r, 1, release);
    assert!(!r.home.industry.claims.contains_key("retire-iron"));
    assert!(r.home.industry.has_secured_resource("iron-ore"));
    assert!(r.kingdom.industry[&1].pending_release.is_none());
    assert!(r.valid_snapshot());
}

#[test]
fn board_is_per_seat_latched_and_blocks_movement_and_attacks() {
    let mut r = home();
    let old = (r.players[&1].x, r.players[&1].y);
    press(
        &mut r,
        1,
        Input {
            cast: 4,
            ..Default::default()
        },
    );
    assert!(r.kingdom.industry[&1].open);
    press(
        &mut r,
        1,
        Input {
            swing: true,
            ..Default::default()
        },
    );
    assert_eq!(r.kingdom.industry[&1].plan, 1);
    press(
        &mut r,
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    );
    assert!(r.home.industry.at((8, 8)).is_some());
    let stock = r.home.industry.stock.clone();
    r.step(&BTreeMap::from([(
        1,
        Input {
            move_x: 1,
            swing: true,
            interact: true,
            ..Default::default()
        },
    )]));
    assert_eq!((r.players[&1].x, r.players[&1].y), old);
    assert!(!r.kingdom.industry.get(&2).is_some_and(|b| b.open));
    assert!(!stock.is_empty());
    let json = serde_json::to_string(&r).unwrap();
    let restored: Run = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.home.industry, r.home.industry);
    assert!(restored.valid_snapshot());
}
#[test]
fn old_home_defaults_and_newer_run_survives_delayed_sync() {
    let h: home::Home = serde_json::from_str("{}").unwrap();
    assert!(h.industry.is_empty());
    let mut r = home();
    r.home.industry.start(catalog());
    let old = r.home.clone();
    r.home
        .industry
        .apply(
            catalog(),
            Command::Build {
                plan: "forester".into(),
                tile: (8, 8),
            },
        )
        .unwrap();
    let new = r.home.industry.clone();
    r.rebuild_home(old, Default::default());
    assert_eq!(r.home.industry, new);
}
#[test]
fn settlement_persists_factory_and_rebases_work_after_capture() {
    use crate::rules::home_settlement::{HomeContext, HomePolicyRequest, HomeTransfer};
    let mut r = home();
    r.home.industry.start(catalog());
    let context = HomeContext {
        home: Default::default(),
        treasury: Default::default(),
        reclaimed: Default::default(),
        raids_won: 0,
    };
    let transfer = HomeTransfer::capture_and_reserve(&mut r).unwrap();
    let req = HomePolicyRequest {
        context,
        transfer: transfer.clone(),
        host: "Host".into(),
        unsaved: false,
    };
    let result = req.evaluate().unwrap();
    assert_eq!(result.home.industry, r.home.industry);
    r.home
        .industry
        .apply(
            catalog(),
            Command::Build {
                plan: "forester".into(),
                tile: (8, 8),
            },
        )
        .unwrap();
    let newer = r.home.industry.clone();
    result.deliver(&mut r, &transfer, "industry-test").unwrap();
    assert_eq!(r.home.industry, newer);
    assert!(!transfer.observed(&result).matches(&r));
}
#[test]
fn claim_requires_a_real_cleared_encounter_a_living_nearby_knight_and_e() {
    let mut r = Run::new(51, 2, None);
    r.dungeon = layout::floor(1, Pack::Cavern, &mut Rng::new(10));
    let i = (1..r.dungeon.rooms.len())
        .find(|&i| i % 3 == 1 && r.dungeon.rooms[i].kind == RoomKind::Fight)
        .unwrap();
    r.enter(i, None);
    let d = r.industry_deposits().into_iter().next().unwrap();
    let hero = r.players.get_mut(&1).unwrap();
    hero.x = d.x;
    hero.y = d.y;
    let hands = BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]);
    r.tick_industry(&hands, r.tick);
    assert!(!r.home.industry.claims[&d.id].secured);
    r.enemies.clear();
    r.waves = Default::default();
    r.phase = Phase::Fighting;
    r.room_end();
    assert!(r.room().cleared);
    r.tick_industry(&BTreeMap::new(), r.tick);
    assert!(!r.home.industry.claims[&d.id].secured);
    r.players.get_mut(&1).unwrap().hp = 0;
    r.tick_industry(&hands, r.tick);
    assert!(!r.home.industry.claims[&d.id].secured);
    r.players.get_mut(&1).unwrap().hp = 10;
    r.tick_industry(&hands, r.tick);
    assert!(!r.home.industry.claims[&d.id].secured);
    for e in &mut r.enemies {
        e.hp = 0;
    }
    r.settle_kills();
    r.tick_industry(&hands, r.tick);
    let guardian = r.kingdom.claim_bosses[&d.id].enemy;
    r.enemies.iter_mut().find(|e| e.id == guardian).unwrap().hp = 0;
    r.settle_kills();
    r.tick_industry(&hands, r.tick);
    assert!(r.home.industry.claims[&d.id].secured);
    let rev = r.home.industry.rev;
    r.tick_industry(&hands, r.tick);
    assert_eq!(r.home.industry.rev, rev);
}
#[test]
fn automatic_work_is_simulation_time_and_outfitting_consumes_stock() {
    let mut r = home();
    r.home.industry.start(catalog());
    r.home
        .industry
        .apply(
            catalog(),
            Command::Build {
                plan: "forester".into(),
                tile: (8, 8),
            },
        )
        .unwrap();
    r.tick = SHIFT_TICKS - 1;
    let before = r.home.industry.amount("timber");
    let forester = r.home.industry.at((8, 8)).unwrap();
    let construction = r.home.industry.buildings[&forester].construction;
    r.step(&BTreeMap::new());
    assert_eq!(r.home.industry.amount("timber"), before);
    assert_eq!(
        r.home.industry.buildings[&forester].construction,
        construction.saturating_sub(1)
    );
    for _ in 0..(HZ * 40) {
        r.step(&BTreeMap::new());
    }
    assert!(
        r.home.industry.amount("timber") > before,
        "constructed forester output travels home before becoming stock"
    );
    r.home.industry.stock.insert("arms".into(), 1);
    r.home.industry.provision = true;
    let armour = r.players[&1].armor;
    r.industry_outfit();
    assert_eq!(r.players[&1].armor, armour + 2);
    assert_eq!(r.home.industry.amount("arms"), 0);
    r.industry_outfit();
    assert_eq!(r.players[&1].armor, armour + 2);
}
#[test]
fn built_frontier_is_walkable_saved_and_returns_to_campaign_gate() {
    let mut r = home();
    r.home.industry.start(catalog());
    r.home
        .industry
        .apply(
            catalog(),
            Command::Build {
                plan: "forester".into(),
                tile: (8, 8),
            },
        )
        .unwrap();
    r.enter_frontier();
    assert!(r.kingdom.frontier);
    assert_eq!(
        (r.room().cols, r.room().rows),
        (
            crate::kingdom::WIDTH as usize,
            crate::kingdom::HEIGHT as usize
        )
    );
    assert_eq!(r.room().tile(8, 8), layout::Tile::Wall);
    let before = r.players[&1].x;
    for _ in 0..10 {
        r.step(&BTreeMap::from([(
            1,
            Input {
                move_x: 1,
                ..Default::default()
            },
        )]));
    }
    assert!(r.players[&1].x > before);
    assert!(r.valid_snapshot());
    r.rebuild_home(r.home.clone(), r.treasury.clone());
    assert_eq!(r.room().cols, crate::kingdom::WIDTH as usize);
    assert_eq!(r.room().tile(8, 8), layout::Tile::Wall);
    let mut restored: Run = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    restored.migrate_kingdom();
    assert!(restored.kingdom.frontier);
    assert_eq!(restored.room().cols, crate::kingdom::WIDTH as usize);
    let hero = restored.players.get_mut(&1).unwrap();
    hero.x = 23.0;
    hero.y = 33.0;
    restored.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    assert!(!restored.kingdom.frontier);
    assert!(restored.room().cols > 24);
    assert!(restored.valid_snapshot());
    assert!(restored.home.industry.at((8, 8)).is_some());
}
#[cfg(feature = "renderer")]
#[test]
fn browser_wire_keeps_the_same_build_board_and_resources() {
    let mut r = home();
    press(
        &mut r,
        1,
        Input {
            cast: 4,
            ..Default::default()
        },
    );
    let tree = crate::wire::publish(&r).unwrap();
    let restored = crate::wire::read(&tree).unwrap();
    assert_eq!(restored.home.industry, r.home.industry);
    assert!(restored.kingdom.industry[&1].open);
    r.enter_frontier();
    let restored = crate::wire::read(&crate::wire::publish(&r).unwrap()).unwrap();
    assert!(restored.kingdom.frontier);
    assert_eq!(
        (restored.room().cols, restored.room().rows),
        (
            crate::kingdom::WIDTH as usize,
            crate::kingdom::HEIGHT as usize
        )
    );
}

#[test]
fn open_floor_claim_uses_its_own_chambers_fight() {
    let mut r = Run::new(51, 2, None);
    r.open_floor_for_test(1);
    let i = 1;
    let centre = r.dungeon.open.as_ref().unwrap().chambers[i].centre();
    r.players.get_mut(&1).unwrap().x = centre.0;
    r.players.get_mut(&1).unwrap().y = centre.1;
    r.tick_open();
    let d = r
        .industry_deposits()
        .into_iter()
        .find(|d| (d.x - centre.0).abs() < 0.01 && (d.y - centre.1).abs() < 0.01)
        .unwrap();
    let hands = BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]);
    r.tick_industry(&hands, r.tick);
    assert!(!r.home.industry.claims[&d.id].secured);
    r.enemies.retain(|e| e.home != Some(i as u8));
    r.tick_open();
    assert!(r.dungeon.open.as_ref().unwrap().chambers[i].cleared);
    r.tick_industry(&hands, r.tick);
    assert!(!r.home.industry.claims[&d.id].secured);
    for e in &mut r.enemies {
        e.hp = 0;
    }
    r.settle_kills();
    r.tick_industry(&hands, r.tick);
    let guardian = r.kingdom.claim_bosses[&d.id].enemy;
    r.enemies.iter_mut().find(|e| e.id == guardian).unwrap().hp = 0;
    r.settle_kills();
    r.tick_industry(&hands, r.tick);
    assert!(r.home.industry.claims[&d.id].secured);
}

#[test]
fn browser_e_builds_once_and_p_changes_page_after_missing_input_releases() {
    let mut r = home();
    press(
        &mut r,
        1,
        Input {
            cast: 4,
            ..Default::default()
        },
    );
    // Browser E is deliberately both interact and pickup; P is cycle.
    let e = Input {
        interact: true,
        take: true,
        ..Default::default()
    };
    let before = r.home.industry.rev;
    r.step(&BTreeMap::from([(1, e)]));
    let built = r.home.industry.rev;
    assert!(built > before);
    assert_eq!(r.kingdom.industry[&1].page, 0);
    r.step(&BTreeMap::from([(1, e)]));
    assert_eq!(r.home.industry.rev, built, "holding E cannot build twice");
    r.step(&BTreeMap::new());
    let p = Input {
        cycle: true,
        ..Default::default()
    };
    r.step(&BTreeMap::from([(1, p)]));
    assert_eq!(r.kingdom.industry[&1].page, 1);
    r.step(&BTreeMap::from([(1, p)]));
    assert_eq!(r.kingdom.industry[&1].page, 1);
    r.step(&BTreeMap::new());
    r.step(&BTreeMap::from([(1, p)]));
    assert_eq!(
        r.kingdom.industry[&1].page, 2,
        "an absent input releases the page key"
    );
}

#[test]
fn ore_forecast_matches_real_route_and_reaches_player_notice_wire() {
    let mut r = home();
    for (first, current, expected_next) in [
        (Pack::Crypt, Pack::Crypt, "Moon crystal"),
        (Pack::Cavern, Pack::Cavern, "Coal"),
    ] {
        r.first = first;
        r.dungeon.depth = 1;
        r.dungeon.pack = current;
        r.notices.clear();
        r.announce_ore_forecast();
        let notices = crate::wire::take_notices(&mut r);
        assert!(notices.iter().any(|n|matches!(n,crate::wire::Event::Notice{text,..} if text.contains("Prospector's warning")&&text.contains(expected_next))));
        assert!(r.notices.is_empty());
    }
    r.dungeon.depth = 2;
    r.dungeon.pack = Pack::Crypt;
    r.notices.clear();
    r.announce_ore_forecast();
    assert!(r.notices[0].contains("Living ember"));
    r.dungeon.depth = 0;
    r.notices.clear();
    r.announce_ore_forecast();
    assert!(r.notices.is_empty());
}
#[test]
fn each_actual_deposit_has_an_ore_threat_and_useful_field_guide() {
    for first in [Pack::Crypt, Pack::Cavern, Pack::Swamp] {
        for depth in 1..=6 {
            let (resource, _) = ore_at(Pack::at(depth, first), depth);
            assert!(catalog().resources.contains_key(resource));
            let kind = crate::rules::ore_threats::OreKind::for_resource(resource).unwrap();
            assert!(!kind.warning().is_empty());
            assert!(!kind.counter().is_empty());
            assert!(ORES.iter().any(|(id, _, _, _)| *id == resource));
        }
    }
}

#[test]
fn changing_region_cannot_alias_another_ore_and_legacy_claims_keep_their_identity() {
    let mut r = Run::new(51, 2, None);
    r.dungeon = layout::floor(1, Pack::Cavern, &mut Rng::new(10));
    let i = (1..r.dungeon.rooms.len())
        .find(|&i| i % 3 == 1 && r.dungeon.rooms[i].kind == RoomKind::Fight)
        .unwrap();
    r.enter(i, None);
    let iron = r.industry_deposits().remove(0);
    r.dungeon.pack = Pack::Crypt;
    let coal = r.industry_deposits().remove(0);
    assert_ne!(iron.id, coal.id);
    r.home.industry.discover(
        catalog(),
        coal.id.clone(),
        Claim {
            resource: "coal".into(),
            name: "old coal".into(),
            source: "test".into(),
            secured: false,
            rate: 2,
        },
    );
    r.home.industry.secure(&coal.id);
    r.dungeon.pack = Pack::Cavern;
    assert_eq!(r.industry_deposits()[0].id, iron.id);
    let legacy = format!("{:x}-{}-{}-{}", r.seed, r.raid_id, r.dungeon.depth, i);
    r.home.industry.discover(
        catalog(),
        legacy.clone(),
        Claim {
            resource: "iron-ore".into(),
            name: "legacy iron".into(),
            source: "test".into(),
            secured: false,
            rate: 2,
        },
    );
    r.home.industry.secure(&legacy);
    assert_eq!(r.industry_deposits()[0].id, legacy);
    r.dungeon.pack = Pack::Crypt;
    assert_eq!(r.industry_deposits()[0].id, coal.id);
    r.dungeon.depth = DEEPEST + 1;
    assert!(
        r.industry_deposits().is_empty(),
        "the infinite Deep needs its own encounter adapter"
    );
}
#[test]
fn fresh_expeditions_retire_abandoned_sightings_but_preserve_mine_output_nodes() {
    let mut r = home();
    r.home.industry.start(catalog());
    for (id, resource) in [("kept", "iron-ore"), ("abandoned", "crystal")] {
        r.home.industry.discover(
            catalog(),
            id.into(),
            Claim {
                resource: resource.into(),
                name: id.into(),
                source: "old expedition".into(),
                secured: false,
                rate: 2,
            },
        );
    }
    r.home.industry.secure("kept");
    r.home
        .industry
        .apply(
            catalog(),
            Command::Build {
                plan: "mine".into(),
                tile: (8, 8),
            },
        )
        .unwrap();
    let mine = r.home.industry.at((8, 8)).unwrap();
    r.home
        .industry
        .apply(
            catalog(),
            Command::Link {
                building: mine,
                claim: "kept".into(),
            },
        )
        .unwrap();
    let rev = r.home.industry.rev;
    r.prepare_industry_expedition();
    assert_eq!(r.home.industry.claims.len(), 1);
    assert!(r.home.industry.claims["kept"].secured);
    assert_eq!(
        r.home.industry.buildings[&mine].claim.as_deref(),
        Some("kept")
    );
    assert!(r.home.industry.rev > rev);
    assert!(r.home.industry.valid(catalog()));
    for _ in 0..6 {
        r.home.industry.work(catalog());
    }
    assert!(r.home.industry.amount("iron-ore") > 0);
    let rev = r.home.industry.rev;
    r.prepare_industry_expedition();
    assert_eq!(r.home.industry.rev, rev);
}
