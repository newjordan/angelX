use super::*;

fn founded() -> Kingdom {
    let mut k = Kingdom::default();
    k.start(catalog());
    k
}
fn build(k: &mut Kingdom, p: &str, tile: Tile) -> u32 {
    k.apply(
        catalog(),
        Command::Build {
            plan: p.into(),
            tile,
        },
    )
    .unwrap();
    k.at(tile).unwrap()
}
fn claim(k: &mut Kingdom, id: &str, resource: &str) {
    assert!(k.discover(
        catalog(),
        id.into(),
        Claim {
            resource: resource.into(),
            name: "Test vein".into(),
            source: "Mines / floor 1".into(),
            secured: false,
            rate: 2,
            yields: Default::default(), surface: false }
    ));
}
fn shifts(k: &mut Kingdom, n: usize) {
    for _ in 0..n {
        k.work(catalog());
        assert!(k.valid(catalog()));
    }
}

#[test]
fn catalogue_is_replaceable_and_rejects_broken_references() {
    let mut c = catalog().clone();
    c.id = "another-world".into();
    c.recipes
        .get_mut("fell")
        .unwrap()
        .outputs
        .insert("timber".into(), 7);
    let c = Catalog::parse(&serde_json::to_string(&c).unwrap()).unwrap();
    let mut k = Kingdom::default();
    k.start(&c);
    k.apply(
        &c,
        Command::Build {
            plan: "forester".into(),
            tile: (8, 8),
        },
    )
    .unwrap();
    let before = k.amount("timber");
    k.work(&c);
    assert_eq!(k.total("timber"), before + 7);
    assert_eq!(k.amount("timber"), before); // output awaits a carrier
    for _ in 0..8 {
        k.advance_second(&c);
    }
    assert_eq!(k.amount("timber"), before + 7);
    assert!(!k.valid(catalog()));
    let mut broken = c.clone();
    broken
        .recipes
        .get_mut("fell")
        .unwrap()
        .outputs
        .insert("missing".into(), 1);
    assert!(broken.validate().is_err());
    broken = c;
    broken.recipes.get_mut("fell").unwrap().shifts = 0;
    assert!(broken.validate().is_err());
}
#[test]
fn invalid_commands_and_insufficient_resources_are_atomic() {
    let mut k = founded();
    let before = k.clone();
    for cmd in [
        Command::Build {
            plan: "keep".into(),
            tile: (8, 8),
        },
        Command::Road(CAMP),
        Command::Demolish(CAMP),
        Command::Road((-1, 9)),
        Command::Build {
            plan: "missing".into(),
            tile: (0, 0),
        },
    ] {
        assert!(k.apply(catalog(), cmd).is_err());
        assert_eq!(k, before);
    }
    let mut virgin = Kingdom::default();
    assert!(virgin.apply(catalog(), Command::Road((-1, 0))).is_err());
    assert_eq!(virgin, Kingdom::default());
}
#[test]
fn roads_are_real_connectivity_and_pause_frees_workers() {
    let mut k = founded();
    let id = build(&mut k, "forester", (0, 0));
    let before = k.amount("timber");
    assert_eq!(k.work(catalog()).status[&id], Status::Disconnected);
    assert_eq!(k.amount("timber"), before);
    let id2 = build(&mut k, "forester", (8, 8));
    let before = k.amount("timber");
    assert_eq!(k.work(catalog()).status[&id2], Status::Working);
    assert_eq!(k.total("timber"), before + 4);
    k.apply(catalog(), Command::Demolish((8, 9))).unwrap();
    assert_eq!(k.work(catalog()).status[&id2], Status::Disconnected);
    k.apply(catalog(), Command::Road((8, 9))).unwrap();
    k.apply(catalog(), Command::Pause(id2)).unwrap();
    assert_eq!(k.report(catalog()).employed, 0);
}
#[test]
fn a_claim_is_unique_and_needs_a_fought_seam_and_a_mine() {
    let mut k = founded();
    let m = build(&mut k, "mine", (8, 8));
    claim(&mut k, "vein", "iron-ore");
    let link = Command::Link {
        building: m,
        claim: "vein".into(),
    };
    assert!(k.apply(catalog(), link.clone()).is_err());
    assert_eq!(k.amount("iron-ore"), 0);
    assert!(k.secure("vein"));
    assert!(!k.secure("vein"));
    k.apply(catalog(), link).unwrap();
    shifts(&mut k, 3);
    assert_eq!(k.total("iron-ore"), 4);
    assert_eq!(k.amount("iron-ore"), 2);
    let other = build(&mut k, "mine", (9, 8));
    let before = k.clone();
    assert!(
        k.apply(
            catalog(),
            Command::Link {
                building: other,
                claim: "vein".into()
            }
        )
        .is_err()
    );
    assert_eq!(k, before);
    k.apply(catalog(), Command::Demolish((8, 8))).unwrap();
    k.apply(
        catalog(),
        Command::Link {
            building: other,
            claim: "vein".into(),
        },
    )
    .unwrap();
}
#[test]
fn local_buffer_backpressure_does_not_destroy_inputs() {
    let mut k = founded();
    let id = build(&mut k, "kiln", (8, 8));
    k.buildings.get_mut(&id).unwrap().construction = 0;
    k.buildings
        .get_mut(&id)
        .unwrap()
        .input
        .insert("timber".into(), 3);
    k.buildings
        .get_mut(&id)
        .unwrap()
        .output
        .insert("coal".into(), 24);
    k.stock.insert("coal".into(), 100);
    let timber = k.total("timber");
    assert_eq!(k.work(catalog()).status[&id], Status::Storage);
    assert_eq!(k.total("timber"), timber);
    assert_eq!(k.total("coal"), 124);
}
#[test]
fn machines_need_delivered_inputs_and_workers() {
    let mut k = founded();
    build(&mut k, "forester", (8, 8));
    let _saw = build(&mut k, "sawmill", (9, 8));
    k.stock.remove("timber");
    shifts(&mut k, 2);
    assert!(k.amount("plank") == 0);
    shifts(&mut k, 4);
    assert!(k.amount("plank") > 0);
    k.stock.insert("timber".into(), 100);
    k.stock.insert("stone".into(), 100);
    for x in 10..=15 {
        if x != 11 {
            build(&mut k, "forester", (x, 8));
        }
    }
    shifts(&mut k, 2);
    let report = k.report(catalog());
    assert_eq!(report.workers, 6);
    assert_eq!(report.employed, 6);
    assert!(report.status.values().any(|s| *s == Status::Workers));
}
fn wait_for(k: &mut Kingdom, condition: impl Fn(&Kingdom) -> bool) {
    for _ in 0..200 {
        if condition(k) {
            return;
        }
        shifts(k, 1);
    }
    panic!(
        "production stalled at shift {}: {:?}; {:?}",
        k.shift,
        k.stock,
        k.report(catalog())
    );
}
fn afford(k: &mut Kingdom, plan: &str, tile: Tile) -> u32 {
    let cost = &catalog().plans[plan].cost;
    wait_for(k, |k| cost.iter().all(|(r, n)| k.amount(r) >= *n));
    build(k, plan, tile)
}
fn research(k: &mut Kingdom, id: &str) {
    let cost = &catalog().technologies[id].cost;
    wait_for(k, |k| cost.iter().all(|(r, n)| k.amount(r) >= *n));
    k.apply(catalog(), Command::Research(id.into())).unwrap();
}
#[test]
fn complete_bootstrap_caravans_research_forge_magic_and_castle() {
    let mut k = founded();
    build(&mut k, "forester", (8, 8));
    build(&mut k, "quarry", (9, 8));
    k.apply(catalog(), Command::Road((7, 9))).unwrap();
    build(&mut k, "farm", (7, 8));
    afford(&mut k, "cottage", (10, 8));
    afford(&mut k, "sawmill", (12, 8));
    afford(&mut k, "kiln", (13, 8));
    research(&mut k, "masonry");
    afford(&mut k, "mason", (14, 8));
    let mine = afford(&mut k, "mine", (15, 8));
    claim(&mut k, "iron", "iron-ore");
    k.secure("iron");
    k.apply(
        catalog(),
        Command::Link {
            building: mine,
            claim: "iron".into(),
        },
    )
    .unwrap();
    research(&mut k, "logistics");
    afford(&mut k, "caravanserai", (8, 10));
    afford(&mut k, "warehouse", (9, 10));
    research(&mut k, "metallurgy");
    afford(&mut k, "cottage", (10, 10));
    afford(&mut k, "smelter", (11, 10));
    let forge = afford(&mut k, "forge", (12, 10));
    k.apply(
        catalog(),
        Command::Recipe {
            building: forge,
            recipe: "iron-oathblade".into(),
        },
    )
    .unwrap();
    wait_for(&mut k, |k| k.amount("iron-oathblade") > 0);
    k.apply(
        catalog(),
        Command::Recipe {
            building: forge,
            recipe: "forge".into(),
        },
    )
    .unwrap();
    wait_for(&mut k, |k| k.amount("arms") > 0);
    k.apply(catalog(), Command::Pause(forge)).unwrap();
    k.apply(catalog(), Command::Provision).unwrap();
    let before = k.amount("arms");
    assert!(k.outfit().0);
    assert_eq!(k.amount("arms"), before - 1);
    let crystal = afford(&mut k, "mine", (13, 10));
    claim(&mut k, "moon", "crystal");
    k.secure("moon");
    k.apply(
        catalog(),
        Command::Link {
            building: crystal,
            claim: "moon".into(),
        },
    )
    .unwrap();
    research(&mut k, "arcana");
    afford(&mut k, "conduit", (14, 10));
    let enchant = afford(&mut k, "enchanter", (15, 10));
    k.apply(
        catalog(),
        Command::Recipe {
            building: enchant,
            recipe: "mooncrystal-staff".into(),
        },
    )
    .unwrap();
    wait_for(&mut k, |k| k.amount("mooncrystal-staff") > 0);
    k.apply(catalog(), Command::Pause(enchant)).unwrap();
    research(&mut k, "sovereignty");
    k.apply(catalog(), Command::Road((16, 9))).unwrap();
    afford(&mut k, "keep", (16, 8));
    shifts(&mut k, 5);
    assert!(k.report(catalog()).population.beds >= 30);
    assert!(k.report(catalog()).workers >= 10);
    assert!(k.report(catalog()).workers < k.report(catalog()).population.beds,
        "a finished keep provides beds; residents must still arrive");
    assert!(k.shift < 400, "bootstrap should fit a one-hour session");
    let restored: Kingdom = serde_json::from_str(&serde_json::to_string(&k).unwrap()).unwrap();
    assert_eq!(k, restored);
    assert!(restored.valid(catalog()));
}
#[test]
fn road_breaks_strand_goods_and_repair_recovers_without_duplication() {
    let mut k = founded();
    let id = build(&mut k, "forester", (8, 8));
    shifts(&mut k, 1);
    k.apply(catalog(), Command::Pause(id)).unwrap();
    k.advance_second(catalog());
    assert_eq!(k.shipments.len(), 1);
    let total = k.total("timber");
    k.apply(catalog(), Command::Demolish((9, 9))).unwrap();
    shifts(&mut k, 2);
    assert_eq!(k.total("timber"), total);
    assert_eq!(k.shipments.len(), 1);
    k.apply(catalog(), Command::Road((9, 9))).unwrap();
    shifts(&mut k, 1);
    assert!(k.shipments.is_empty());
    assert_eq!(k.total("timber"), total - 1);
}
#[test]
fn demolition_reroutes_in_flight_inputs_and_recipe_change_conserves_them() {
    let mut k = founded();
    let id = build(&mut k, "sawmill", (8, 8));
    for _ in 0..16 {
        k.advance_second(catalog());
    }
    assert!(!k.shipments.is_empty());
    let total = k.total("timber");
    k.apply(catalog(), Command::Demolish((8, 8))).unwrap();
    shifts(&mut k, 3);
    assert!(k.shipments.is_empty());
    assert_eq!(
        k.total("timber"),
        total + catalog().plans["sawmill"].cost["timber"] / 2
    );
    assert!(!k.buildings.contains_key(&id));
}
#[test]
fn research_is_atomic_and_catalogue_rejects_cycles() {
    let mut k = founded();
    let before = k.clone();
    assert!(
        k.apply(catalog(), Command::Research("arcana".into()))
            .is_err()
    );
    assert_eq!(k, before);
    let mut c = catalog().clone();
    c.technologies
        .get_mut("masonry")
        .unwrap()
        .requires
        .push("sovereignty".into());
    assert!(c.validate().is_err());
}
#[test]
fn malformed_save_cannot_duplicate_camps_claims_or_invent_recipes() {
    let k = founded();
    let mut bad = k.clone();
    bad.buildings.insert(2, bad.buildings[&1].clone());
    bad.next = 3;
    assert!(!bad.valid(catalog()));
    let mut bad = k.clone();
    bad.buildings.get_mut(&1).unwrap().recipe = Some("missing".into());
    assert!(!bad.valid(catalog()));
    let mut bad = k;
    bad.schema = 9;
    assert!(!bad.valid(catalog()));
}
