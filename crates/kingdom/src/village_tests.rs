use super::*;

fn build(k: &mut Kingdom, plan: &str, tile: Tile) -> u32 {
    k.apply(
        catalog(),
        Command::Build {
            plan: plan.into(),
            tile,
        },
    )
    .unwrap();
    k.at(tile).unwrap()
}
fn wait(k: &mut Kingdom, limit: usize, goal: impl Fn(&Kingdom) -> bool) {
    for _ in 0..limit {
        if goal(k) {
            return;
        }
        k.advance_second(catalog());
        assert!(k.valid(catalog()));
    }
    panic!("village stalled: {:?}; {:?}", k.stock, k.report(catalog()));
}

#[test]
fn founding_materials_become_a_brick_home_and_walk_through_archway() {
    let mut k = Kingdom::default();
    k.start(catalog());
    build(&mut k, "forester", (8, 8));
    build(&mut k, "quarry", (9, 8));
    build(&mut k, "farm", (10, 8));
    build(&mut k, "sawmill", (12, 8));
    build(&mut k, "clay-pit", (13, 8));
    let kiln = build(&mut k, "brick-kiln", (14, 8));
    assert_eq!(k.amount("brick"), 0);
    wait(&mut k, 900, |k| {
        k.amount("brick") >= 24 && k.amount("plank") >= 10
    });
    assert!(k.produced["clay"] >= 24);
    assert!(k.produced["brick"] >= 24);
    assert!(k.delivered["brick"] >= 24);
    let cottage = build(&mut k, "brick-cottage", (10, 10));
    let gate = build(&mut k, "brick-gate", (12, 9));
    build(&mut k, "brick-wall", (12, 10));
    wait(&mut k, 180, |k| {
        k.buildings[&cottage].construction == 0 && k.buildings[&gate].construction == 0
    });
    assert!(k.roads.contains(&(12, 9)));
    assert!(k.route(k.buildings[&kiln].tile, 1).is_some());
    let stock = k.stock.clone();
    let saved = serde_json::to_string(&k).unwrap();
    let restored: Kingdom = serde_json::from_str(&saved).unwrap();
    assert!(restored.valid(catalog()));
    assert_eq!(restored.stock, stock);
    assert_eq!(restored, k);
    k.apply(catalog(), Command::Demolish((12, 9))).unwrap();
    assert!(
        k.roads.contains(&(12, 9)),
        "dismantling an arch must preserve its road"
    );
}

#[test]
fn production_targets_include_buffered_and_travelling_goods_and_resume_after_spending() {
    let mut k = Kingdom::default();
    k.start(catalog());
    let id = build(&mut k, "forester", (8, 8));
    k.apply(
        catalog(),
        Command::ProductionLimit {
            building: id,
            amount: 64,
        },
    )
    .unwrap();
    wait(&mut k, 120, |k| k.total("timber") >= 64);
    let total = k.total("timber");
    for _ in 0..40 {
        k.advance_second(catalog());
    }
    assert_eq!(k.total("timber"), total);
    assert_eq!(k.report(catalog()).status[&id], Status::Target);
    build(&mut k, "cottage", (10, 8));
    wait(&mut k, 120, |k| k.total("timber") >= 64);
    assert!(k.produced["timber"] > 10);
    let snapshot = k.clone();
    assert!(
        k.apply(
            catalog(),
            Command::Priority {
                building: id,
                priority: 3
            }
        )
        .is_err()
    );
    assert_eq!(k, snapshot);
}

#[test]
fn a_seeded_village_feeds_new_residents_fires_bricks_and_earns_more_land() {
    let mut k = Kingdom::default();
    k.found(catalog(), 41);
    assert!(!k.world.legacy_core);
    build(&mut k, "forester", (8, 8));
    build(&mut k, "farm", (10, 8));
    build(&mut k, "quarry", (15, 8));
    build(&mut k, "sawmill", (12, 8));
    build(&mut k, "clay-pit", (8, 10));
    build(&mut k, "brick-kiln", (13, 8));
    wait(&mut k, 900, |k| {
        k.amount("brick") >= 12 && k.amount("plank") >= 6
    });
    let home = build(&mut k, "brick-cottage", (10, 10));
    assert_eq!(k.population.citizens.len(), 6);
    wait(&mut k, 600, |k| k.population.citizens.len() >= 7);
    assert_eq!(k.buildings[&home].construction, 0);
    assert!(k.report(catalog()).population.housed >= 7);
    assert_eq!(k.report(catalog()).population.hungry, 0);
    wait(&mut k, 300, |k| {
        k.amount("timber") >= 12 && k.amount("stone") >= 6
    });
    let before = k.clone();
    assert!(k.apply(catalog(), Command::Survey((2, 0))).is_err());
    assert_eq!(k, before);
    k.apply(catalog(), Command::Survey((1, 0))).unwrap();
    assert!(k.world.owned((30, 9)));
    assert!(!k.world.owned((60, 9)));
    assert!(k.valid(catalog()));
    // Actual site rules reject unsuitable positions without charging supplies.
    let before = k.clone();
    assert!(
        k.apply(
            catalog(),
            Command::Build {
                plan: "clay-pit".into(),
                tile: (11, 1)
            }
        )
        .is_err()
    );
    assert_eq!(k, before);
    let saved = serde_json::to_string(&k).unwrap();
    let mut resumed: Kingdom = serde_json::from_str(&saved).unwrap();
    for _ in 0..300 {
        k.advance_second(catalog());
        resumed.advance_second(catalog());
    }
    assert_eq!(
        k, resumed,
        "population and transport replay must be deterministic"
    );
}
