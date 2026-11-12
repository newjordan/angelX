use super::*;

fn discover(k: &mut Kingdom, id: &str, resource: &str) -> bool {
    k.discover(
        catalog(),
        id.into(),
        Claim {
            resource: resource.into(),
            name: "Test seam".into(),
            source: "Mines / floor 1".into(),
            secured: false,
            rate: 2,
        },
    )
}

#[test]
fn claims_can_retire_at_capacity_and_discover_a_new_ore_without_free_goods() {
    let mut k = Kingdom::default();
    for i in 0..MAX_CLAIMS {
        let id = format!("old-{i}");
        assert!(discover(&mut k, &id, "coal"));
        assert!(k.secure(&id));
    }
    let before = k.clone();
    assert!(!discover(&mut k, "new-iron", "iron-ore"));
    assert_eq!(k, before);
    let stock = k.stock.clone();
    k.apply(catalog(), Command::ReleaseClaim("old-0".into()))
        .unwrap();
    assert_eq!(k.claims.len(), MAX_CLAIMS - 1);
    assert!(discover(&mut k, "new-iron", "iron-ore"));
    assert!(!k.has_secured_resource("iron-ore"));
    assert!(k.has_secured_resource("coal"));
    assert_eq!(k.stock, stock);
    let restored: Kingdom = serde_json::from_str(&serde_json::to_string(&k).unwrap()).unwrap();
    assert_eq!(restored, k);
    assert!(restored.valid(catalog()));
}

#[test]
fn claims_unlink_keeps_real_mined_goods_and_linked_release_is_atomic() {
    let mut k = Kingdom::default();
    k.start(catalog());
    k.apply(
        catalog(),
        Command::Build {
            plan: "mine".into(),
            tile: (8, 8),
        },
    )
    .unwrap();
    let mine = k.at((8, 8)).unwrap();
    assert!(discover(&mut k, "iron", "iron-ore"));
    assert!(k.secure("iron"));
    k.apply(
        catalog(),
        Command::Link {
            building: mine,
            claim: "iron".into(),
        },
    )
    .unwrap();
    for _ in 0..30 {
        k.advance_second(catalog());
    }
    let total = k.total("iron-ore");
    assert!(total > 0);
    assert!(
        k.buildings[&mine]
            .output
            .get("iron-ore")
            .copied()
            .unwrap_or(0)
            > 0
            || k.in_transit("iron-ore") > 0
    );
    let before = k.clone();
    assert!(
        k.apply(catalog(), Command::ReleaseClaim("iron".into()))
            .is_err()
    );
    assert_eq!(k, before);
    k.buildings.get_mut(&mine).unwrap().work_credit = 75;
    k.apply(catalog(), Command::Unlink(mine)).unwrap();
    assert_eq!(k.buildings[&mine].work_credit, 0);
    assert_eq!(k.total("iron-ore"), total);
    k.apply(catalog(), Command::ReleaseClaim("iron".into()))
        .unwrap();
    for _ in 0..30 {
        k.advance_second(catalog());
    }
    assert_eq!(k.total("iron-ore"), total);
    assert_eq!(k.amount("iron-ore"), total);
    assert!(k.valid(catalog()));
}

#[test]
fn claims_legacy_release_preserves_won_charter_evidence_and_paid_land() {
    let mut k = Kingdom::default();
    assert!(discover(&mut k, "legacy-iron", "iron-ore"));
    k.claims.get_mut("legacy-iron").unwrap().secured = true;
    let mut old = serde_json::to_value(&k).unwrap();
    old.as_object_mut().unwrap().remove("secured_resources");
    let mut k: Kingdom = serde_json::from_value(old).unwrap();
    assert!(k.secured_resources.is_empty());
    assert!(k.has_secured_resource("iron-ore"));
    k.apply(catalog(), Command::ReleaseClaim("legacy-iron".into()))
        .unwrap();
    assert!(k.has_secured_resource("iron-ore"));
    k.world.parcels.insert((1, 0));
    let offer = k.expansion_offer((2, 0)).unwrap();
    assert!(offer.blocked.is_none());
    for (r, n) in &offer.cost {
        k.stock.insert(r.clone(), *n);
    }
    let before = k.stock.clone();
    k.apply(catalog(), Command::Survey((2, 0))).unwrap();
    for (r, n) in offer.cost {
        assert_eq!(k.amount(&r), before[&r] - n);
    }
    assert!(k.world.parcels.contains(&(2, 0)));
    assert!(k.valid(catalog()));
}

#[test]
fn claims_unsecured_retirement_does_not_grant_evidence_and_bad_history_is_rejected() {
    let mut k = Kingdom::default();
    assert!(discover(&mut k, "unwon", "ember"));
    k.apply(catalog(), Command::ReleaseClaim("unwon".into()))
        .unwrap();
    assert!(!k.has_secured_resource("ember"));
    for command in [Command::ReleaseClaim("missing".into()), Command::Unlink(1)] {
        let before = k.clone();
        assert!(k.apply(catalog(), command).is_err());
        assert_eq!(k, before);
    }
    k.secured_resources.insert("unknown-ore".into());
    assert!(!k.valid(catalog()));
}
