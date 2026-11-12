use super::*;

fn planning(first: Pack, parcel: Parcel) -> Run {
    let mut run = Run::new(511, 7, None);
    run.first = first;
    run.dungeon.depth = 0;
    run.home.industry.start(crate::kingdom::catalog());
    run.home.industry.survey_goal = Some(parcel);
    run
}

#[test]
fn frontier_objectives_follow_real_floor_routes_and_explicitly_request_new_trips() {
    for (first, parcel, expected_pack, expected_floor, new_trip) in [
        (Pack::Crypt, (2, 0), Pack::Cavern, 1, true),
        (Pack::Cavern, (2, 0), Pack::Cavern, 1, false),
        (Pack::Crypt, (2, 1), Pack::Cavern, 2, false),
        (Pack::Cavern, (2, 1), Pack::Archive, 4, false),
        (Pack::Crypt, (2, 2), Pack::Hellforge, 3, false),
        (Pack::Swamp, (2, 2), Pack::Hellforge, 3, true),
    ] {
        let run = planning(first, parcel);
        let goal = run.frontier_objective().unwrap();
        assert_eq!(goal.route.pack, expected_pack);
        assert_eq!(goal.route.floor, expected_floor);
        assert_eq!(goal.route.next_expedition, new_trip);
        assert_eq!(
            industry::ore_at(goal.route.pack, goal.route.floor).0,
            goal.resource
        );
        assert!(!goal.route.here);
        assert!(!goal.warning.is_empty() && !goal.counter.is_empty());
        assert!(!goal.guardian.is_empty());
    }
    let mut run = planning(Pack::Cavern, (2, 0));
    run.dungeon.depth = 1;
    run.dungeon.pack = Pack::Cavern;
    assert!(run.frontier_objective().unwrap().route.here);
    run.dungeon.depth = 3;
    run.dungeon.pack = Pack::Hellforge;
    let later = run.frontier_objective().unwrap();
    assert!(later.route.next_expedition && !later.route.here);
    assert!(route_for(&run, "unobtainable-ore").is_none());
}

#[test]
fn frontier_objectives_survive_home_restore_and_use_visible_notices() {
    let mut run = planning(Pack::Crypt, (2, 0));
    let home = serde_json::to_string(&run.home).unwrap();
    run.home = serde_json::from_str(&home).unwrap();
    let state = serde_json::to_string(&run.home).unwrap();
    let goal = run.frontier_objective().unwrap();
    assert_eq!(goal.district, "Ironridge");
    assert!(goal.purpose.contains("fortified settlement"));
    run.announce_frontier_objective();
    assert!(run.notices.last().unwrap().contains("Ironridge"));
    assert!(run.notices.last().unwrap().contains("Next trip"));
    assert_eq!(
        serde_json::to_string(&run.home).unwrap(),
        state,
        "planning grants nothing"
    );
    run.home.industry.survey_goal = None;
    assert!(run.frontier_objective().is_none());
    run.home.industry.survey_goal = Some((1, 0));
    assert!(
        run.frontier_objective().is_none(),
        "production-only milestone does not invent a delve objective"
    );
}

#[test]
fn frontier_objectives_require_real_claim_security_and_keep_survey_payment_separate() {
    let (mut run, d, _, _) = claim_bosses::tests::play_claim(claim_bosses::Guardian::Ironroot);
    assert!(run.claim_guardian_defeated(&d.id));
    run.home.industry.survey_goal = Some((2, 0));
    assert_eq!(
        run.frontier_objective().unwrap().status,
        ObjectiveStatus::Secure
    );
    assert!(!run.home.industry.world.parcels.contains(&(2, 0)));
    // Walk and interact at the real vein. Merely winning the fight above did
    // not grant a parcel, a claim, or any factory resources.
    for _ in 0..HZ * 20 {
        let hero = &run.players[&1];
        if (hero.x - d.x).hypot(hero.y - d.y) < 2.0 {
            break;
        }
        let (move_x, move_y) = balance_sim_tests::dir8(d.x - hero.x, d.y - hero.y);
        run.step(&BTreeMap::from([(
            1,
            Input {
                move_x,
                move_y,
                ..Default::default()
            },
        )]));
    }
    run.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            take: true,
            ..Default::default()
        },
    )]));
    assert!(run.home.industry.claims[&d.id].secured);
    let goal = run.frontier_objective().unwrap();
    assert_eq!(goal.status, ObjectiveStatus::SurveySupplies);
    assert!(goal.blocked_reason.unwrap().contains("neighboring"));
    assert_eq!(run.home.industry.amount("iron-ore"), 0);
    assert!(!run.home.industry.world.parcels.contains(&(2, 0)));
    run.announce_frontier_objective();
    assert!(run.notices.last().unwrap().contains("claim is secured"));
    let restored: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    assert!(restored.frontier_objective().unwrap().status.claim_won());
}

fn work_until(
    kingdom: &mut crate::kingdom::Kingdom,
    enough: impl Fn(&crate::kingdom::Kingdom) -> bool,
) {
    for _ in 0..900 {
        if enough(kingdom) {
            return;
        }
        kingdom.advance_second(crate::kingdom::catalog());
        assert!(kingdom.valid(crate::kingdom::catalog()));
    }
    panic!(
        "village stalled before its expedition: {:?}; {:?}",
        kingdom.stock,
        kingdom.report(crate::kingdom::catalog())
    );
}

/// Shared by the integration test and optional renderer review. Production
/// commands spend founding supplies and actual deliveries; combat uses real
/// starter inputs, and the final survey uses client-shaped desk inputs.
pub(crate) fn play_village_expedition() -> Run {
    use crate::kingdom::{Command, Kingdom, catalog};
    let mut kingdom = Kingdom::default();
    kingdom.found(catalog(), 511);
    for (plan, tile) in [
        ("forester", (8, 8)),
        ("quarry", (15, 8)),
        ("farm", (10, 8)),
        ("sawmill", (12, 8)),
        ("cottage", (10, 10)),
    ] {
        kingdom
            .apply(
                catalog(),
                Command::Build {
                    plan: plan.into(),
                    tile,
                },
            )
            .unwrap();
    }
    work_until(&mut kingdom, |k| {
        k.amount("plank") >= 8 && k.amount("stone") >= 30
    });
    kingdom
        .apply(catalog(), Command::Research("masonry".into()))
        .unwrap();
    kingdom
        .apply(
            catalog(),
            Command::Build {
                plan: "mason".into(),
                tile: (14, 8),
            },
        )
        .unwrap();
    work_until(&mut kingdom, |k| {
        k.amount("timber") >= 12
            && k.amount("stone") >= 6
            && k.delivered.get("timber").copied().unwrap_or(0) >= 24
    });
    kingdom.apply(catalog(), Command::Survey((1, 0))).unwrap();
    kingdom
        .apply(catalog(), Command::SurveyGoal(Some((2, 0))))
        .unwrap();
    work_until(&mut kingdom, |k| {
        k.amount("plank") >= 16 && k.amount("block") >= 8
    });
    assert!(!kingdom.world.legacy_core);
    assert!(kingdom.claims.is_empty());
    assert!(!kingdom.world.owned((60, 9)));
    let before = kingdom.clone();
    assert!(kingdom.apply(catalog(), Command::Survey((2, 0))).is_err());
    assert_eq!(
        kingdom, before,
        "resources cannot substitute for winning the claim"
    );
    let (mut fought, deposit, _, _) = claim_bosses::tests::play_claim_with_industry(
        claim_bosses::Guardian::Ironroot,
        Some(kingdom),
    );
    assert!(fought.players[&1].hp > 0 && fought.claim_guardian_defeated(&deposit.id));
    assert_eq!(
        fought.frontier_objective().unwrap().status,
        ObjectiveStatus::Secure
    );
    for _ in 0..HZ * 20 {
        let hero = &fought.players[&1];
        if (hero.x - deposit.x).hypot(hero.y - deposit.y) < 2.0 {
            break;
        }
        let (move_x, move_y) = balance_sim_tests::dir8(deposit.x - hero.x, deposit.y - hero.y);
        fought.step(&BTreeMap::from([(
            1,
            Input {
                move_x,
                move_y,
                ..Default::default()
            },
        )]));
    }
    fought.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            take: true,
            ..Default::default()
        },
    )]));
    assert_eq!(
        fought.frontier_objective().unwrap().status,
        ObjectiveStatus::SurveyReady
    );
    assert_eq!(fought.home.industry.amount("iron-ore"), 0);
    // Cross the normal persisted-Home boundary. The encounter test does not
    // pretend that its isolated room also contains a procedural route home.
    let saved = serde_json::to_string(&fought.home).unwrap();
    let mut returned = Run::at_home(
        511,
        8,
        None,
        serde_json::from_str(&saved).unwrap(),
        Default::default(),
    );
    returned.walk_kingdom(Some(crate::world::kingdom::keep_stand()));
    returned.enter_frontier();
    let press = |r: &mut Run, input: Input| {
        r.step(&BTreeMap::from([(1, input)]));
        r.step(&BTreeMap::new());
    };
    press(
        &mut returned,
        Input {
            cast: 4,
            ..Default::default()
        },
    );
    for _ in 0..7 {
        press(
            &mut returned,
            Input {
                cycle: true,
                ..Default::default()
            },
        );
    }
    for _ in 0..2 {
        press(
            &mut returned,
            Input {
                swing: true,
                ..Default::default()
            },
        );
    }
    assert_eq!(returned.kingdom.industry[&1].page, 7);
    assert_eq!(returned.kingdom.industry[&1].selection, 2);
    let (plank, block) = (
        returned.home.industry.amount("plank"),
        returned.home.industry.amount("block"),
    );
    press(
        &mut returned,
        Input {
            interact: true,
            take: true,
            ..Default::default()
        },
    );
    assert!(returned.home.industry.world.owned((60, 9)));
    assert!(returned.home.industry.survey_goal.is_none());
    assert_eq!(returned.home.industry.amount("plank"), plank - 16);
    assert_eq!(returned.home.industry.amount("block"), block - 8);
    assert!(returned.home.industry.valid(catalog()));
    assert!(returned.valid_snapshot());
    returned
}

#[test]
fn frontier_objectives_turn_real_production_and_a_won_delve_into_new_building_land() {
    let run = play_village_expedition();
    let json = serde_json::to_string(&run).unwrap();
    let mut restored: Run = serde_json::from_str(&json).unwrap();
    assert!(restored.valid_snapshot());
    assert_eq!(restored.home.industry.world.parcels.len(), 3);
    for _ in 0..HZ * 10 {
        restored.step(&BTreeMap::new());
    }
    assert!(restored.home.industry.valid(crate::kingdom::catalog()));
    assert_eq!(
        restored.home.industry.world.parcels,
        run.home.industry.world.parcels
    );
}
