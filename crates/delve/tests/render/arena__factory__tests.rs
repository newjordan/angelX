//! Factory gate and real arena consumption. Fixtures are Rust values, never
//! replacement catalog files or a test-only compositor.
use super::*;
use factory::{Catalog, Kind, Placement, Replace};
use quick_rig::{Clip, Playback};
use serde_json::{Value, json};

fn arena_run() -> Run {
    let mut run = quick_rig_tests::arena_run();
    let centre = (run.room().width() / 2.0, run.room().height() / 2.0);
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y) = centre;
    run
}

fn entry(id: &str, kind: &str) -> Value {
    let mut value = json!({
        "id":id, "name":id, "brief":"Factory test asset", "kind":kind,
        "frames":[[".VW..", ".VV..", ".BB..", ".WW.."]],
        "pivot":[1,3], "anchors":{"grip":[3,2], "feet":[2,3]}, "part":"body"
    });
    if kind == "body" {
        value["rig"] = quick_rig_tests::export();
    }
    if kind == "weapon" {
        value["attack"] = json!({"clip":"flail", "sound":"swing", "vfx":[[".V.","VWV"]]});
    }
    value
}
fn parse(assets: Vec<Value>) -> Result<Catalog, factory::Error> {
    Catalog::parse(&json!({"version":1,"assets":assets}).to_string())
}

#[test]
fn factory_checks_complete_rigs_and_native_source_rows() {
    let catalog = parse(vec![
        entry("body", "body"),
        entry("weapon", "weapon"),
        entry("mail", "worn"),
        entry("spark", "vfx"),
        entry("cask", "prop"),
        entry("floor", "tile"),
    ])
    .unwrap();
    let body = catalog.get("body").unwrap();
    assert_eq!(body.sprite().size(), (5, 4));
    let card = body.card().unwrap();
    assert_eq!(
        card.looks.worn.as_ref().unwrap().frames,
        body.sprite().frames
    );
    assert_eq!(
        card.looks.worn.as_ref().unwrap().at.as_deref(),
        Some("whole")
    );
    for clip in Clip::ALL {
        let rig = body.rig().unwrap();
        assert!(rig.has(clip));
        assert_eq!(
            rig.sample(clip, 0, Playback::Loop, false).rows(),
            body.sprite().frames[0]
        );
    }
    let weapon = catalog.get("weapon").unwrap();
    assert_eq!(weapon.attack().unwrap().clip(), Clip::Flail);
    assert_eq!(weapon.attack().unwrap().sound(), "swing");
    assert!(weapon.card().unwrap().arm.is_some());
    for asset in catalog.assets() {
        assert_eq!(asset.pivot(), [1, 3]);
        if !matches!(asset.kind(), Kind::Body | Kind::Worn | Kind::Weapon) {
            assert!(asset.card().is_err());
        }
    }
}

#[test]
fn factory_rejects_bad_contracts_atomically() {
    let good = entry("body", "body");
    assert!(parse(vec![good.clone(), good.clone()]).is_err());
    let mut alias = good.clone();
    alias["id"] = json!("alias");
    assert!(parse(vec![good.clone(), alias]).is_err());
    let mut cases = Vec::new();
    let mut v = good.clone();
    v["rig"]["clips"].as_object_mut().unwrap().remove("flail");
    cases.push(v);
    let mut v = good.clone();
    v["rig"]["version"] = json!(2);
    cases.push(v);
    let mut v = good.clone();
    v["rig"]["clips"]["idle"][0]["anchors"]
        .as_object_mut()
        .unwrap()
        .remove("tip");
    cases.push(v);
    let mut v = good.clone();
    v["frames"] = json!([["V".repeat(25)]]);
    cases.push(v);
    let mut v = good.clone();
    v["frames"] = json!([vec!["V"; 25]]);
    cases.push(v);
    let mut v = good.clone();
    v["frames"] = json!([["~"]]);
    cases.push(v);
    let mut v = good.clone();
    v["frames"] = json!([["."]]);
    cases.push(v);
    let mut v = good.clone();
    v["frames"] = json!([["VV", "V"]]);
    cases.push(v);
    let mut v = good.clone();
    v["pivot"] = json!([-1, 0]);
    cases.push(v);
    let mut v = good.clone();
    v["part"] = json!("torso-unknown");
    cases.push(v);
    let mut v = entry("weapon", "weapon");
    v["attack"]["sound"] = json!("../../sound.wav");
    cases.push(v);
    let mut v = entry("weapon", "weapon");
    v["attack"]["sound"] = json!("not-a-shipped-game-sound");
    cases.push(v);
    let mut v = entry("weapon", "weapon");
    v.as_object_mut().unwrap().remove("attack");
    cases.push(v);
    let mut v = good.clone();
    v["local_path"] = json!("/tmp/body.png");
    cases.push(v);
    let mut v = good.clone();
    v["name"] = json!("oops\nkind take");
    cases.push(v);
    for (n, bad) in cases.into_iter().enumerate() {
        assert!(parse(vec![bad]).is_err(), "bad case {n}");
    }
    assert!(Catalog::parse(r#"{"version":2,"assets":[]}"#).is_err());
    let duplicate = json!({"version":1,"assets":[entry("prop", "prop")]})
        .to_string()
        .replace("\"grip\":[3,2]", "\"grip\":[3,2],\"grip\":[2,2]");
    assert!(Catalog::parse(&duplicate).is_err());
}

#[test]
fn factory_install_apply_and_placements_reach_the_normal_arena() {
    let catalog = parse(vec![
        entry("body", "body"),
        entry("weapon", "weapon"),
        entry("mail", "worn"),
        entry("spark", "vfx"),
        entry("cask", "prop"),
    ])
    .unwrap();
    let mut run = arena_run();
    assert_eq!(
        catalog.install(&mut run.book, Replace::No).unwrap().len(),
        3
    );
    let revision = run.book.revision;
    catalog.install(&mut run.book, Replace::No).unwrap();
    assert_eq!(revision, run.book.revision);
    let before = native(&run).content_hash();
    for id in ["body", "mail", "weapon"] {
        catalog
            .get(id)
            .unwrap()
            .apply(&mut run, 1, Replace::No)
            .unwrap();
    }
    assert_eq!(run.players[&1].arm.as_deref(), Some("factory-weapon"));
    assert_eq!(
        run.players[&1]
            .deck
            .iter()
            .filter(|s| s.as_str() == "factory-body")
            .count(),
        1
    );
    catalog
        .get("body")
        .unwrap()
        .apply(&mut run, 1, Replace::No)
        .unwrap();
    let equipped = native(&run).content_hash();
    assert_ne!(equipped, before);
    let hero = &run.players[&1];
    let placements = [
        Placement::new(catalog.get("cask").unwrap(), hero.x + 3.0, hero.y, 0, false).unwrap(),
        Placement::new(catalog.get("spark").unwrap(), hero.x - 3.0, hero.y, 0, true).unwrap(),
    ];
    let drawn = factory::with_placements(&placements, || native(&run).content_hash());
    assert_ne!(drawn, equipped);
    assert_eq!(equipped, native(&run).content_hash());
    factory::with_placements(&placements, || {
        assert_eq!(drawn, native(&run).content_hash());
        factory::with_placements(&[], || assert_eq!(equipped, native(&run).content_hash()));
        assert_eq!(drawn, native(&run).content_hash());
    });
    let mut conflict = run.book.get("factory-mail").unwrap().clone();
    conflict.text = "conflict".into();
    run.book.insert(conflict);
    let old = serde_json::to_string(&run.book).unwrap();
    assert!(catalog.install(&mut run.book, Replace::No).is_err());
    assert_eq!(old, serde_json::to_string(&run.book).unwrap());
    catalog.install(&mut run.book, Replace::Yes).unwrap();
}

#[test]
fn factory_compiled_catalog_registers_exact_body_sprites_automatically() {
    let catalog = factory::catalog().expect("factory supplies a checked compiled catalog");
    for asset in catalog.assets() {
        if asset.kind() != Kind::Body {
            continue;
        }
        let mut run = arena_run();
        asset.apply(&mut run, 1, Replace::No).unwrap();
        let sprite = run
            .book
            .get(&asset.card_id())
            .unwrap()
            .looks
            .worn
            .as_ref()
            .unwrap();
        assert!(std::sync::Arc::ptr_eq(
            &quick_rig::sprite_rig(sprite).unwrap(),
            asset.rig().unwrap()
        ));
        let mut changed = sprite.clone();
        changed.frames[0][0].push('.');
        assert!(quick_rig::sprite_rig(&changed).is_none());
        assert!(knights::rigged_roll(&run.players[&1], &run.book));
        run.players.get_mut(&1).unwrap().stride = 1.8;
        let sample = knights::rig_sample_event(
            asset.rig().unwrap(),
            &run.players[&1],
            &run.book,
            run.tick,
            None,
        );
        assert_eq!(sample.clip, Clip::Walk);
        assert_eq!(native(&run).content_hash(), native(&run).content_hash());
    }
}

#[test]
#[ignore = "writes actual arena samples when ANGEL_ARENA_SHOTS is set"]
fn write_factory_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let catalog = factory::catalog().unwrap();
    assert!(
        catalog.assets().len() > 0,
        "no accepted factory assets to capture"
    );
    for asset in catalog.assets() {
        let mut run = arena_run();
        catalog.install(&mut run.book, Replace::No).unwrap();
        {
            let hero = run.players.get_mut(&1).unwrap();
            hero.sword = false;
            hero.weapon = crate::rules::Weapon::Fists;
            hero.forged = None;
        }
        let mut placements = Vec::new();
        let mut enemy_skins = Vec::new();
        if asset.kind() == Kind::Body {
            let h = &run.players[&1];
            let (x, y) = (h.x + 3.0, h.y);
            run.spawn_at_for_test(EnemyKind::Hob, x, y);
            let enemy = run.enemies.last_mut().unwrap();
            enemy.dir = (1.0, 0.0);
            enemy_skins.push((enemy.id, asset.id()));
        }
        match asset.kind() {
            Kind::Body | Kind::Weapon | Kind::Worn => {
                asset.apply(&mut run, 1, Replace::No).unwrap();
            }
            _ => {
                let h = &run.players[&1];
                placements.push(Placement::new(asset, h.x + 3.0, h.y, 0, false).unwrap());
            }
        }
        if let Some(attack) = asset.attack() {
            let h = run.players.get_mut(&1).unwrap();
            h.swing = 0;
            h.sword = false;
            let total = h.forged.as_ref().unwrap().melee.unwrap().every;
            h.fire_cooldown = total * 3 / 5;
            assert!(matches!(
                attack.clip(),
                Clip::Slash | Clip::Overhead | Clip::Thrust | Clip::Flail
            ));
        }
        let image = factory::with_enemy_skins(&enemy_skins, || {
            factory::with_placements(&placements, || frame_for(&run, NATIVE_W, NATIVE_H, Some(1)))
        })
        .unwrap();
        if matches!(asset.kind(), Kind::Body | Kind::Weapon | Kind::Worn) {
            let mut hero = run.players[&1].clone();
            for (pose, cooldown) in [
                ("rest", 0),
                (
                    "attack",
                    hero.forged
                        .as_ref()
                        .and_then(|w| w.melee)
                        .map_or(0, |m| m.every * 3 / 5),
                ),
            ] {
                hero.fire_cooldown = cooldown;
                let figure = knights::dressed_frame(
                    || panic!("factory equipment must use a real rig"),
                    &hero,
                    &run.book,
                    run.tick,
                    0,
                    None,
                    false,
                    knights::Struck::default(),
                    Some(1),
                )
                .image;
                for scale in [1, 8] {
                    image::RgbaImage::from_raw(
                        figure.w as u32 * scale,
                        figure.h as u32 * scale,
                        figure.rgba_scaled(scale),
                    )
                    .unwrap()
                    .save(dir.join(format!("{}-{pose}-{scale}x.png", asset.id())))
                    .unwrap();
                }
            }
        }
        let detail = Img::from_view(&image, image.w / 2 - 40, image.h / 2 - 40, 80, 64);
        for scale in [1, 4] {
            image::RgbaImage::from_raw(
                detail.w as u32 * scale,
                detail.h as u32 * scale,
                detail.rgba_scaled(scale),
            )
            .unwrap()
            .save(dir.join(format!("{}-detail-{scale}x.png", asset.id())))
            .unwrap();
        }
        for scale in [1, 4] {
            image::RgbaImage::from_raw(
                image.w as u32 * scale,
                image.h as u32 * scale,
                image.rgba_scaled(scale),
            )
            .unwrap()
            .save(dir.join(format!("{}-{scale}x.png", asset.id())))
            .unwrap();
        }
    }
}

#[test]
fn factory_body_caps_apply_to_source_idle_not_the_padded_rig_canvas() {
    let mut body = entry("padded-body", "body");
    let rig = body["rig"].as_object_mut().unwrap();
    rig.insert("canvas".into(), json!([46, 46]));
    rig.insert("pivot".into(), json!([21, 23]));
    for clip in rig["clips"].as_object_mut().unwrap().values_mut() {
        for frame in clip.as_array_mut().unwrap() {
            let mut rows = vec![".".repeat(46); 46];
            for (y, row) in frame["rows"].as_array().unwrap().iter().enumerate() {
                rows[y + 20].replace_range(20..25, row.as_str().unwrap());
            }
            frame["rows"] = json!(rows);
            for anchor in frame["anchors"].as_object_mut().unwrap().values_mut() {
                let p = anchor.as_array().unwrap();
                *anchor = json!([p[0].as_i64().unwrap() + 20, p[1].as_i64().unwrap() + 20]);
            }
        }
    }
    let catalog = parse(vec![body.clone()]).unwrap();
    let asset = catalog.get("padded-body").unwrap();
    assert_eq!(asset.sprite().size(), (5, 4));
    assert_eq!(asset.rig().unwrap().canvas(), [46, 46]);
    assert_eq!(asset.card().unwrap().looks.worn.unwrap().size(), (5, 4));
    body["frames"] = body["rig"]["clips"]["idle"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["rows"].clone())
        .collect();
    assert!(
        parse(vec![body]).is_err(),
        "a padded rig canvas is not a portable Sprite"
    );
}

#[test]
fn factory_placement_scope_restores_after_panic_and_culls_offscreen_points() {
    let catalog = parse(vec![entry("spark", "vfx"), entry("weapon", "weapon")]).unwrap();
    let mut run = arena_run();
    let hero = &run.players[&1];
    let placements = [Placement::new(
        catalog.get("spark").unwrap(),
        hero.x + 2.0,
        hero.y,
        0,
        false,
    )
    .unwrap()];
    let baseline = frame_for(&run, NATIVE_W, NATIVE_H, Some(1)).content_hash();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        factory::with_placements(&placements, || panic!("scope restoration"));
    }));
    assert!(panic.is_err());
    assert_eq!(
        baseline,
        frame_for(&run, NATIVE_W, NATIVE_H, Some(1)).content_hash()
    );
    let offscreen =
        [Placement::new(catalog.get("spark").unwrap(), f32::MIN, f32::MAX, 0, false).unwrap()];
    factory::with_placements(&offscreen, || {
        assert_eq!(
            baseline,
            frame_for(&run, NATIVE_W, NATIVE_H, Some(1)).content_hash()
        )
    });
    assert!(Placement::new(catalog.get("spark").unwrap(), f32::NAN, 0.0, 0, false).is_err());
    assert!(Placement::new(catalog.get("weapon").unwrap(), 0.0, 0.0, 0, false).is_err());
    assert!(Placement::attack(catalog.get("spark").unwrap(), 0.0, 0.0, 0, false).is_err());
    let effect = [Placement::attack(
        catalog.get("weapon").unwrap(),
        hero.x + 2.0,
        hero.y,
        0,
        false,
    )
    .unwrap()];
    factory::with_placements(&effect, || {
        assert_ne!(
            baseline,
            frame_for(&run, NATIVE_W, NATIVE_H, Some(1)).content_hash()
        )
    });
    // Apply failures are atomic, including missing heroes and non-equipment.
    let before = serde_json::to_string(&run).unwrap();
    assert!(
        catalog
            .get("weapon")
            .unwrap()
            .apply(&mut run, u32::MAX, Replace::No)
            .is_err()
    );
    assert!(
        catalog
            .get("spark")
            .unwrap()
            .apply(&mut run, 1, Replace::No)
            .is_err()
    );
    assert_eq!(before, serde_json::to_string(&run).unwrap());
}

#[test]
fn factory_compiled_equipment_survives_existing_save_shapes_and_selects_authored_clips() {
    let catalog = factory::catalog().unwrap();
    let mut run = arena_run();
    for asset in catalog
        .assets()
        .filter(|a| matches!(a.kind(), Kind::Body | Kind::Weapon | Kind::Worn))
    {
        asset.apply(&mut run, 1, Replace::No).unwrap();
        asset.apply(&mut run, 1, Replace::No).unwrap();
        assert_eq!(
            run.players[&1]
                .deck
                .iter()
                .filter(|id| **id == asset.card_id())
                .count(),
            1
        );
        let saved_book = serde_json::to_value(&run.book).unwrap();
        let restored_book: crate::rules::cards::Book = serde_json::from_value(saved_book).unwrap();
        let restored_hero: crate::rules::Hero =
            serde_json::from_value(serde_json::to_value(&run.players[&1]).unwrap()).unwrap();
        let card = restored_book.get(&asset.card_id()).unwrap();
        assert_eq!(card, &asset.card().unwrap());
        if asset.kind() == Kind::Body {
            assert!(quick_rig::sprite_rig(card.looks.worn.as_ref().unwrap()).is_some());
            assert!(knights::rigged_roll(&restored_hero, &restored_book));
        }
        if let Some(attack) = asset.attack() {
            assert_eq!(
                factory::equipped_attack_clip(&restored_hero, &restored_book),
                Some(attack.clip())
            );
            // A same-id edited card is not checked factory equipment anymore.
            let mut card = card.clone();
            let mut changed = restored_book;
            card.text.push('!');
            changed.insert(card);
            assert_eq!(
                factory::equipped_attack_clip(&restored_hero, &changed),
                None
            );
        }
    }
}

#[test]
fn factory_shipped_assets_have_live_monster_prop_and_attack_consumers() {
    let catalog = factory::catalog().unwrap();
    assert_eq!(catalog.assets().count(), 9);
    let mut run = quick_rig_tests::arena_run();
    catalog
        .get("chain-flail")
        .unwrap()
        .apply(&mut run, 1, Replace::No)
        .unwrap();
    {
        let hero = run.players.get_mut(&1).unwrap();
        hero.swing = 0;
        hero.sword = false;
        hero.fire_cooldown = 20;
    }
    let mut canvas = Img::new(NATIVE_W, NATIVE_H);
    factory::attacks(&mut canvas, &run);
    assert!((0..canvas.h).any(|y| (0..canvas.w).any(|x| canvas.get(x, y).is_some())));
    let cask = factory::prop_image("tavern-cask", 0).unwrap();
    assert_eq!((cask.w, cask.h), (16, 16));
    assert!(factory::prop_image("chain-flail", 0).is_none());
    let mut live = Run::new(11, 1, Some("Factory"));
    let fight = live
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    live.enter_for_test(fight);
    let mut enemy = live.enemies[0].clone();
    enemy.boss = None;
    enemy.kind = EnemyKind::Sapper;
    let first = factory::with_enemy_skins(&[(enemy.id, "marsh-watchman")], || {
        factory::enemy_image(&enemy, 0).unwrap()
    })
    .unwrap();
    enemy.kind = EnemyKind::Hob;
    assert_ne!(
        first.content_hash(),
        factory::with_enemy_skins(&[(enemy.id, "cinder-knave")], || factory::enemy_image(
            &enemy, 0
        )
        .unwrap()
        .content_hash())
        .unwrap()
    );
    enemy.kind = EnemyKind::Bat;
    assert!(factory::enemy_image(&enemy, 0).is_none());
}

#[test]
fn factory_weapons_keep_a_painted_bottom_grip_through_real_worn_fit() {
    let catalog = factory::catalog().unwrap();
    for asset in catalog.assets().filter(|a| a.kind() == Kind::Weapon) {
        let card = asset.card().unwrap();
        let original = card.looks.worn.as_ref().unwrap();
        let rig = quick_rig::seat_knight(1).unwrap();
        let sample = rig.sample(Clip::Idle, 0, Playback::Loop, false);
        let part = factory::weapon_part(original, sample.body().right);
        assert!(part.bottom);
        assert_eq!(part.size, crate::rules::looks::profile("knight").right.size);
        let fitted = wear::fitted(original, part);
        let im = knights::frame_img(fitted.frame(0));
        let origin = factory::weapon_origin(original, &fitted, part).unwrap();
        assert!(
            im.get(part.anchor.0 - origin.0, part.anchor.1 - origin.1)
                .is_some(),
            "{} grip lost: part={part:?}, original={:?}, fitted={:?}",
            asset.id(),
            original.frames,
            fitted.frames
        );
        for clip in [Clip::Slash, Clip::Overhead, Clip::Thrust, Clip::Flail] {
            for tick in 0..60 {
                let sample = rig.sample(clip, tick, Playback::Once, false);
                let part = factory::weapon_part(original, sample.body().right);
                let origin = factory::weapon_origin(original, &fitted, part).unwrap();
                let orientation = quick_rig::WeaponOrientation::new(
                    [part.anchor.0 - origin.0, part.anchor.1 - origin.1],
                    [part.anchor.0, part.anchor.1],
                    sample.anchors().tip,
                );
                let (turned, at) = orientation.stamp(&im);
                assert!(
                    turned
                        .get(part.anchor.0 - at[0], part.anchor.1 - at[1])
                        .is_some(),
                    "{} {clip:?} tick={tick}: rotated grip lost",
                    asset.id()
                );
            }
        }
    }
}
