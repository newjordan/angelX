use super::*;

fn map(world: &World) -> Vec<Terrain> {
    (0..WORLD_HEIGHT)
        .flat_map(|y| (0..WORLD_WIDTH).map(move |x| world.terrain((x, y))))
        .collect()
}
fn complete_evidence() -> (Stock, Stock, BTreeSet<String>) {
    (
        Stock::from([("plank".into(), 100)]),
        Stock::from([
            ("timber".into(), 100),
            ("plank".into(), 100),
            ("food".into(), 100),
            ("block".into(), 100),
        ]),
        BTreeSet::from(["iron-ore".into(), "crystal".into(), "ember".into()]),
    )
}
fn reach(world: &World) -> BTreeSet<Tile> {
    let mut seen = BTreeSet::from([crate::CAMP]);
    let mut queue = VecDeque::from([crate::CAMP]);
    while let Some(tile) = queue.pop_front() {
        for next in adjacent(tile) {
            if world.owned(next) && world.terrain(next).walkable() && seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    seen
}

#[test]
fn every_seed_starts_with_accessible_production_and_preserved_portal() {
    for seed in (0..128).chain([u64::MAX]) {
        let world = World::new(seed);
        assert!(world.valid());
        assert_eq!(world.terrain(crate::CAMP), Terrain::Meadow);
        assert_eq!(world.terrain(crate::GATE), Terrain::Meadow);
        for x in 8..=15 {
            assert_eq!(world.terrain((x, 9)), Terrain::Meadow);
        }
        let reached = reach(&world);
        for terrain in [Terrain::Forest, Terrain::Stone, Terrain::ClayBank] {
            assert!(
                reached.iter().any(|&tile| {
                    world.terrain(tile) == terrain
                        && (8..=15).any(|x| (tile.0 - x).abs() + (tile.1 - 9).abs() <= 3)
                }),
                "seed {seed}: {terrain:?} must be within three steps of a founding road"
            );
        }
        assert!(reached.iter().any(|&tile| world.water_adjacent(tile)));
        assert!(
            reached
                .iter()
                .any(|&tile| world.terrain(tile) == Terrain::Ford)
        );
        assert!(!world.owned((24, 9)));
    }
}

#[test]
fn migration_preserves_every_legacy_plot_and_expands_without_moving_it() {
    let mut world: World = serde_json::from_str("{}").unwrap();
    assert_eq!(world, World::default());
    assert!(world.valid());
    for y in 0..PARCEL_HEIGHT {
        for x in 0..PARCEL_WIDTH {
            assert!(world.owned((x, y)) && world.legacy_plot((x, y)));
            assert_eq!(world.terrain((x, y)), Terrain::Meadow);
        }
    }
    let (produced, delivered, secured_resources) = complete_evidence();
    let evidence = ExpansionEvidence {
        produced: &produced,
        delivered: &delivered,
        secured_resources: &secured_resources,
    };
    world.unlock((1, 0), &evidence).unwrap();
    assert!(world.valid());
    assert!(world.owned((24, 9)));
    assert!(!world.legacy_plot((24, 9)));
    assert_eq!(world.terrain(crate::CAMP), Terrain::Meadow);
    assert_eq!(world.terrain(crate::GATE), Terrain::Meadow);
    assert_eq!(world.bounds(), (72, 54));
}

#[test]
fn surveys_need_real_evidence_adjacency_and_cannot_be_repeated() {
    let mut world = World::new(17);
    let empty = Stock::new();
    let no_claims = BTreeSet::new();
    let mut delivered = Stock::from([("timber".into(), 23)]);
    let evidence = ExpansionEvidence {
        produced: &empty,
        delivered: &delivered,
        secured_resources: &no_claims,
    };
    let offer = world.offer((1, 0), &evidence).unwrap();
    assert!(offer.blocked.unwrap().contains("23/24"));
    assert_eq!(
        offer.cost,
        Stock::from([("timber".into(), 12), ("stone".into(), 6)])
    );
    let before = world.clone();
    assert!(world.unlock((1, 0), &evidence).is_err());
    assert_eq!(world, before);
    assert!(
        world
            .offer((2, 0), &evidence)
            .unwrap()
            .blocked
            .unwrap()
            .contains("neighboring")
    );
    delivered.insert("timber".into(), 24);
    let evidence = ExpansionEvidence {
        produced: &empty,
        delivered: &delivered,
        secured_resources: &no_claims,
    };
    world.unlock((1, 0), &evidence).unwrap();
    assert_eq!(
        delivered["timber"], 24,
        "world never spends evidence or stock"
    );
    assert!(world.offer((1, 0), &evidence).is_err());
    assert!(world.unlock((1, 0), &evidence).is_err());
    let iron = world.offer((2, 0), &evidence).unwrap();
    assert_eq!(
        iron.requirement,
        ExpansionRequirement::Claim {
            resource: "iron-ore".into()
        }
    );
    assert!(iron.blocked.unwrap().contains("Win and secure"));
    assert!(world.valid());
}

#[test]
fn a_full_valley_has_connected_dry_routes_and_no_dependency_dead_end() {
    let (produced, delivered, secured_resources) = complete_evidence();
    let evidence = ExpansionEvidence {
        produced: &produced,
        delivered: &delivered,
        secured_resources: &secured_resources,
    };
    for seed in 0..32 {
        let mut world = World::new(seed);
        while world.parcels.len() < 9 {
            let before = world.parcels.len();
            for y in 0..3 {
                for x in 0..3 {
                    if world
                        .offer((x, y), &evidence)
                        .is_ok_and(|o| o.blocked.is_none())
                    {
                        let offer = world.offer((x, y), &evidence).unwrap();
                        assert!(crate::catalog().valid_stock(&offer.cost));
                        world.unlock((x, y), &evidence).unwrap();
                    }
                }
            }
            assert!(
                world.parcels.len() > before,
                "charter graph cannot deadlock"
            );
        }
        assert!(world.valid());
        let reached = reach(&world);
        for y in 0..WORLD_HEIGHT {
            for x in 0..WORLD_WIDTH {
                if world.terrain((x, y)).walkable() {
                    assert!(
                        reached.contains(&(x, y)),
                        "seed {seed}: isolated land at {x},{y}"
                    );
                }
            }
        }
    }
}

#[test]
fn forest_clearance_is_persistent_bounded_and_never_repaid() {
    let mut world = World::new(99);
    assert_eq!(world.terrain((3, 3)), Terrain::Forest);
    assert!(world.clear_forest((3, 3)));
    assert_eq!(world.terrain((3, 3)), Terrain::Meadow);
    assert!(!world.clear_forest((3, 3)));
    assert!(
        !world.clear_forest((27, 3)),
        "unowned trees cannot be harvested"
    );
    assert!(!world.clear_forest((i16::MAX, i16::MIN)));
    assert!(!world.near((i16::MAX, i16::MIN), Terrain::Forest, u8::MAX));
    assert!(world.valid());
    let restored: World = serde_json::from_str(&serde_json::to_string(&world).unwrap()).unwrap();
    assert_eq!(world, restored);
    assert_eq!(map(&world), map(&restored));
}

#[test]
fn terrain_generation_is_stable_and_seeded() {
    assert_eq!(map(&World::new(42)), map(&World::new(42)));
    assert_ne!(map(&World::new(42)), map(&World::new(43)));
    // Specific samples protect the authored starting resource sites and axes.
    let world = World::new(42);
    assert_eq!(world.terrain((3, 3)), Terrain::Forest);
    assert_eq!(world.terrain((15, 4)), Terrain::Stone);
    assert_eq!(world.terrain((8, 11)), Terrain::ClayBank);
    assert_eq!(world.terrain((11, 16)), Terrain::Meadow);
    assert!(world.near((8, 8), Terrain::Forest, 3));
    assert!(world.near((15, 8), Terrain::Stone, 1));
    assert!(!world.near((23, 17), Terrain::Forest, 3));
}

#[test]
fn malformed_worlds_cannot_gain_hidden_land_or_clear_outside_their_charter() {
    let good = World::new(3);
    for change in [0, 1, 2, 3, 4, 5] {
        let mut world = good.clone();
        match change {
            // Version 2 is the hillside now; 3 is a layout no code knows.
            0 => world.version = 3,
            1 => {
                world.parcels.remove(&(0, 0));
            }
            2 => {
                world.parcels.insert((2, 2));
            }
            3 => {
                world.parcels.insert((-1, 0));
            }
            4 => {
                world.cleared.insert(crate::CAMP);
            }
            _ => {
                world.cleared.insert((27, 3));
            }
        }
        assert!(!world.valid(), "malformed case {change}");
    }
}
