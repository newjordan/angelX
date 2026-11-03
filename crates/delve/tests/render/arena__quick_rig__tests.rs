//! The runtime/export boundary, plus the actual knight/wish integration.
use super::*;
use quick_rig::{Clip, Playback, Rig};
use serde_json::{Value, json};
use std::sync::Arc;

pub(super) fn export() -> Value {
    let frame = |ticks, row: &str, grip: [i32; 2]| {
        json!({
            "ticks": ticks,
            "rows": [row, ".VV..", ".BB..", ".WW.."],
            "anchors": {"grip":grip, "off_hand":[1,2], "head":[2,0],
                "back":[2,2], "feet":[2,3], "tip":[4,1]}
        })
    };
    let idle = vec![frame(2, ".VW..", [3, 2]), frame(3, ".WV..", [4, 2])];
    let mut clips = serde_json::Map::new();
    for clip in [
        "idle", "walk", "run", "roll", "hurt", "die", "cast", "slash", "overhead", "thrust",
        "flail", "bow", "gun",
    ] {
        clips.insert(clip.into(), json!(idle));
    }
    json!({"version":1,"canvas":[5,4],"pivot":[1,3],"clips":clips})
}

fn rig() -> Rig {
    Rig::parse(&export().to_string()).unwrap()
}
fn pixels(im: &Img) -> Vec<Option<[u8; 3]>> {
    (0..im.h)
        .flat_map(|y| (0..im.w).map(move |x| im.get(x, y)))
        .collect()
}
fn knight() -> crate::rules::Hero {
    let mut run = Run::new(21, 1, Some("Rig"));
    let h = run.players.get_mut(&1).unwrap();
    h.avatar = None;
    h.shape = None;
    h.knight = None;
    h.gear = 0;
    h.owned = crate::rules::gear::Owned::equipped();
    h.hp = h.max_hp;
    h.clone()
}

fn rolling(h: &crate::rules::Hero, ticks: u32) -> crate::rules::Hero {
    let mut saved = serde_json::to_value(h).unwrap();
    saved["dash_ticks"] = json!(ticks);
    serde_json::from_value(saved).unwrap()
}

#[test]
fn quick_rig_parses_all_standard_clips_and_variable_tick_boundaries() {
    let rig = rig();
    assert_eq!(rig.canvas(), [5, 4]);
    for clip in Clip::ALL {
        assert!(rig.has(clip));
        assert_eq!(rig.duration(clip), 5);
        for (at, index) in [
            (0, 0),
            (1, 0),
            (2, 1),
            (4, 1),
            (5, 0),
            (7, 1),
            (u64::MAX, 0),
        ] {
            assert_eq!(rig.sample(clip, at, Playback::Loop, false).index, index);
        }
        assert_eq!(rig.sample(clip, u64::MAX, Playback::Once, false).index, 1);
        assert_eq!(rig.action(clip, 10, 10, false).index, 0);
        assert_eq!(rig.action(clip, 5, 10, false).index, 1);
        assert_eq!(rig.action(clip, 0, 10, false).index, 1);
    }
}

#[test]
fn quick_rig_mirrors_pixels_pivot_and_all_sockets_with_pixel_index_rule() {
    let rig = rig();
    let right = rig.sample(Clip::Idle, 0, Playback::Loop, false);
    let left = rig.sample(Clip::Idle, 0, Playback::Loop, true);
    assert_eq!(left.rows()[0], "..WV.");
    assert_eq!(left.anchors().grip, [1, 2]);
    assert_eq!(left.anchors().off_hand, [3, 2]);
    assert_eq!(left.anchors().tip, [0, 1]);
    assert_eq!(left.anchors().mirrored(5), right.anchors());
    assert_eq!(left.pivot(), [3, 3]);
    let (r, l) = (right.body(), left.body());
    assert_eq!(r.right.anchor, (3, 2));
    assert_eq!(l.right.anchor, (1, 2));
    assert_eq!(l.left.anchor, (3, 2));
    assert_eq!(l.head.anchor, (2, 0));
    assert_eq!(l.feet.anchor, (2, 3));
    assert_eq!(l.back.anchor, (2, 2));
    assert_eq!(
        r.bounds,
        rig.sample(Clip::Slash, 4, Playback::Once, false)
            .body()
            .bounds
    );
    assert_eq!(pixels(&left.image()), pixels(&right.image().flip_h()));
    let mut target = Img::new(5, 4);
    right.attach(
        &mut target,
        &Img::from_rows(&["W"]),
        [0, 0],
        right.anchors().grip,
    );
    assert_eq!(target.get(3, 2), super::super::ink::ink('W'));
}

#[test]
fn quick_rig_rejects_malformed_exports_without_partial_load() {
    for mutate in [
        |j: &mut Value| j["version"] = json!(2),
        |j: &mut Value| j["canvas"] = json!([0, 4]),
        |j: &mut Value| j["canvas"] = json!([129, 4]),
        |j: &mut Value| j["pivot"] = json!([5, 0]),
        |j: &mut Value| {
            j["clips"]
                .as_object_mut()
                .unwrap()
                .remove("idle")
                .map(drop)
                .unwrap()
        },
        |j: &mut Value| j["clips"]["idle"] = json!([]),
        |j: &mut Value| j["clips"]["idle"][0]["ticks"] = json!(0),
        |j: &mut Value| j["clips"]["idle"][0]["ticks"] = json!(-1),
        |j: &mut Value| j["clips"]["idle"][0]["rows"][0] = json!("bad"),
        |j: &mut Value| j["clips"]["idle"][0]["rows"][0] = json!("*****"),
        |j: &mut Value| j["clips"]["idle"][0]["rows"][0] = json!("é...."),
        |j: &mut Value| j["clips"]["idle"][0]["anchors"]["grip"] = json!([-1, 2]),
        |j: &mut Value| {
            j["clips"]["idle"][0]["anchors"]
                .as_object_mut()
                .unwrap()
                .remove("tip")
                .map(drop)
                .unwrap()
        },
        |j: &mut Value| j["clips"]["wobble"] = j["clips"]["idle"].clone(),
        |j: &mut Value| j["surprise"] = json!(true),
        |j: &mut Value| j["clips"]["idle"] = json!(vec![j["clips"]["idle"][0].clone(); 129]),
    ] {
        let mut json = export();
        mutate(&mut json);
        assert!(Rig::parse(&json.to_string()).is_err(), "accepted {json}");
    }
    assert!(Rig::parse("{}").is_err());
    assert!(Rig::parse("not json").is_err());
    assert!(Rig::parse(&" ".repeat(8 * 1024 * 1024 + 1)).is_err());
}

#[test]
fn quick_rig_sparse_clips_have_explicit_fallbacks() {
    let mut json = export();
    json["clips"]
        .as_object_mut()
        .unwrap()
        .retain(|k, _| k == "idle" || k == "walk" || k == "slash");
    let rig = Rig::parse(&json.to_string()).unwrap();
    assert_eq!(rig.resolve(Clip::Run), Clip::Walk);
    assert_eq!(rig.resolve(Clip::Overhead), Clip::Slash);
    assert_eq!(rig.resolve(Clip::Thrust), Clip::Slash);
    assert_eq!(rig.resolve(Clip::Flail), Clip::Slash);
    assert_eq!(rig.resolve(Clip::Roll), Clip::Idle);
    assert!(!rig.has(Clip::Roll));
}

#[test]
fn quick_rig_knight_selection_uses_gameplay_countdowns_and_stride() {
    let rig = rig();
    let mut h = knight();
    let sample = |h: &crate::rules::Hero, tick| knights::rig_sample(&rig, h, tick, false).clip;
    assert_eq!(sample(&h, 0), Clip::Idle);
    h.stride = 1.8;
    assert_eq!(sample(&h, 900), Clip::Walk);
    h.hero_kit.windrun = 20;
    assert_eq!(sample(&h, 900), Clip::Run);
    h.swing = 8;
    h.sword = true;
    assert_eq!(sample(&h, 900), Clip::Slash);
    h = rolling(&h, 10);
    assert_eq!(sample(&h, 900), Clip::Roll);
    h = rolling(&h, 0);
    h.swing = 0;
    h.struck = crate::rules::spectacle::STRUCK;
    assert_eq!(sample(&h, 900), Clip::Hurt);
    h.struck = 0;
    h.stride = 0.0;
    h.fire_cooldown = 3;
    h.weapon = crate::rules::Weapon::Bow;
    assert_eq!(sample(&h, 900), Clip::Bow);
    h.weapon = crate::rules::Weapon::Handgonne;
    assert_eq!(sample(&h, 900), Clip::Gun);
    assert_eq!(knights::rig_sample(&rig, &h, 4, true).clip, Clip::Cast);
    h.hp = 0;
    h.fell = 100;
    assert_eq!(sample(&h, 900), Clip::Die);
    assert_eq!(knights::rig_sample(&rig, &h, 100, false).index, 0);
    // Locomotion depends on distance, not render rate; action progress depends
    // on countdown, not the absolute time the host/mirror happened to draw it.
    h.hp = 100;
    h.fire_cooldown = 0;
    h.stride = 1.8;
    assert_eq!(
        knights::rig_sample(&rig, &h, 0, false).index,
        knights::rig_sample(&rig, &h, 900, false).index
    );
}

#[test]
fn quick_rig_wished_sheet_override_is_live_scoped_and_old_assets_still_work() {
    let h = knight();
    let card = crate::rules::cards::check(
        "rig-test-1",
        "name Rig Test\nkind hold\narmor 1\ndraw worn\n.V.\n.V.\n",
    )
    .unwrap()
    .card;
    let sprite = card.looks.worn.as_ref().unwrap().clone();
    let mut run = Run::new(21, 1, Some("Rig"));
    run.book.insert(card.clone());
    let mut h = h;
    h.deck.push(card.id.clone());
    h = rolling(&h, 10);
    let old = knights::figure(&h, 1);
    let legacy = knights::dressed_hero(&old, &h, &run.book, 0, 0).0;
    let rig = Arc::new(rig());
    let overridden = quick_rig::with_sprite_rig(&sprite, rig.clone(), || {
        assert!(knights::rigged_roll(&h, &run.book));
        let image = knights::dressed_hero(&old, &h, &run.book, 0, 0).0;
        // The actual worn-sheet pipeline selects the rig, not merely a parser.
        assert_ne!(pixels(&image), pixels(&legacy));
        image
    });
    assert_ne!(pixels(&overridden), pixels(&legacy));
    let restored = knights::dressed_hero(&old, &h, &run.book, 0, 0).0;
    assert_eq!(pixels(&restored), pixels(&legacy));
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        quick_rig::with_sprite_rig(&sprite, rig, || panic!("restore test"));
    }));
    assert_eq!(
        pixels(&knights::dressed_hero(&old, &h, &run.book, 0, 0).0),
        pixels(&legacy)
    );
}

#[test]
fn quick_rig_live_knight_body_and_worn_attachment_share_selected_frame() {
    let mut h = knight();
    h.weapon = crate::rules::Weapon::Bow;
    h.owned.shield = false;
    h.guard = crate::rules::cards::Guard::Roll;
    let rig = rig();
    let body = knights::rig_figure(&h, 1, false, Default::default(), &rig, 0, false);
    let sample = knights::rig_sample(&rig, &h, 0, false);
    let fit = knights::staged_fit(&sample);
    // Off-centre source pivot becomes the exact middle pixel of staging.
    let dx = fit.right.anchor.0 - sample.anchors().grip[0];
    assert_eq!(dx + rig.pivot()[0], body.w / 2);
    let mark = crate::rules::looks::Sprite {
        frames: vec![vec!["9".into()]],
        at: Some("right".into()),
        ..Default::default()
    };
    let (dressed, _) = knights::dressed_as(&body, fit, &[&mark], 0, 0);
    assert_eq!(
        dressed.get(fit.right.anchor.0, fit.right.anchor.1),
        super::super::ink::ink('9')
    );
    let next = knights::rig_sample(&rig, &h, 2, false);
    assert_eq!(next.anchors().grip[0], sample.anchors().grip[0] + 1);
    assert_eq!(
        knights::staged_fit(&next).right.anchor.0,
        fit.right.anchor.0 + 1
    );
    assert_eq!((dressed.w, dressed.h), (body.w, body.h));
    let walking = {
        h.stride = 1.8;
        knights::rig_figure(&h, 1, false, Default::default(), &rig, 0, false)
    };
    assert_ne!(pixels(&body), pixels(&walking));
    h.swing = 4;
    h.sword = true;
    let swinging = knights::rig_figure(&h, 1, false, Default::default(), &rig, 0, false);
    assert_ne!(pixels(&body), pixels(&swinging));
    let rolling = {
        h = rolling(&h, 5);
        knights::rig_figure(&h, 1, false, Default::default(), &rig, 0, false)
    };
    assert_eq!((rolling.w, rolling.h), (body.w, body.h));
}

#[test]
fn quick_rig_default_asset_or_legacy_fallback_preserves_unique_bodies_and_saves() {
    let h = knight();
    let saved = serde_json::to_value(&h).unwrap();
    let restored: crate::rules::Hero = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(serde_json::to_value(&restored).unwrap(), saved);
    // Untimed callers keep the original sixteen-pixel figure/fit contract;
    // the live entry owns the timed quick-rig integration.
    assert_eq!(knights::figure(&h, 1).h, 16);
    let plain = knights::figure_at(&h, 1, false, Default::default(), false, 0);
    assert!(pixels(&plain).iter().any(Option::is_some));
    if quick_rig::seat_knight(1).is_some() {
        // Real compiled export is connected to figure(), not just parsed.
        assert_eq!(plain.h, quick_rig::seat_knight(1).unwrap().canvas()[1] + 12);
    } else {
        // Malformed compiled assets must use the old sixteen-pixel knight.
        assert_eq!((plain.w, plain.h), (16, 16));
    }
    let mut shape = h.clone();
    shape.shape = Some(crate::rules::identity::Shape::Circle);
    let shaped = knights::figure(&shape, 1);
    assert_ne!(pixels(&plain), pixels(&shaped));
    let mut naked = h.clone();
    naked.gear = 2;
    naked.owned = crate::rules::gear::Owned::default();
    assert_eq!(knights::figure(&naked, 1).w, 16);
    let mut dwarf = h.clone();
    dwarf.knight = Some(knights::DWARF.into());
    assert_ne!(pixels(&knights::figure(&dwarf, 1)), pixels(&plain));
    let mut run = Run::new(21, 1, Some("Rig"));
    assert!(run.take_up(1, "scryglass"));
    let mage = knights::figure(&run.players[&1], 1);
    assert_ne!(pixels(&plain), pixels(&mage));
}

#[test]
fn quick_rig_roll_bypass_is_actual_composition_and_sparse_sheets_still_tumble() {
    let mut h = rolling(&knight(), 5);
    let card = crate::rules::cards::check(
        "rig-roll-1",
        "name Rig Roll\nkind hold\narmor 1\ndraw worn\n.V.\n.V.\n",
    )
    .unwrap()
    .card;
    let sprite = card.looks.worn.as_ref().unwrap().clone();
    h.deck.push(card.id.clone());
    let mut run = Run::new(21, 1, Some("Rig"));
    run.book.insert(card);
    let body = knights::rig_figure(&h, 1, true, Default::default(), &rig(), 0, false);
    quick_rig::with_sprite_rig(&sprite, Arc::new(rig()), || {
        assert_eq!(
            pixels(&knights::rolled_hero(&body, &h, &run.book, 5)),
            pixels(&body)
        );
        // Override state does not change ordinary, unregistered wish drawing.
        let mut sparse = export();
        sparse["clips"].as_object_mut().unwrap().remove("roll");
        quick_rig::with_sprite_rig(
            &sprite,
            Arc::new(Rig::parse(&sparse.to_string()).unwrap()),
            || {
                assert!(!knights::rigged_roll(&h, &run.book));
                assert_eq!(
                    pixels(&knights::rolled_hero(&body, &h, &run.book, 5)),
                    pixels(&knights::rolled(&body, 5))
                );
            },
        );
        assert!(knights::rigged_roll(&h, &run.book));
    });
    let frame = export()["clips"]["idle"].to_string();
    let duplicate = format!(
        "{{\"version\":1,\"canvas\":[5,4],\"pivot\":[1,3],\"clips\":{{\"idle\":{frame},\"idle\":{frame}}}}}"
    );
    assert!(Rig::parse(&duplicate).is_err());
}

#[test]
fn quick_rig_wished_body_is_rendered_by_the_real_arena_for_walk_roll_and_swing() {
    let mut run = Run::new(21, 1, Some("Rig"));
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(room);
    run.enemies.clear();
    run.projectiles.clear();
    run.players.retain(|&id, _| id == 1);
    let card = crate::rules::cards::check(
        "live-rig-1",
        "name Live Rig\nkind hold\narmor 1\ndraw worn\n.V.\n.V.\n",
    )
    .unwrap()
    .card;
    let sprite = card.looks.worn.as_ref().unwrap().clone();
    run.book.insert(card.clone());
    let mut h = knight();
    h.x = run.players[&1].x;
    h.y = run.players[&1].y;
    h.invulnerable = 0;
    h.deck.push(card.id);
    let rig = Arc::new(rig());
    for mode in 0..3 {
        let mut h = h.clone();
        match mode {
            0 => h.stride = 1.8,
            1 => h = rolling(&h, 5),
            _ => {
                h.swing = 4;
                h.sword = true;
            }
        }
        run.players.insert(1, h.clone());
        let legacy = native(&run).content_hash();
        quick_rig::with_sprite_rig(&sprite, rig.clone(), || {
            let right = native(&run).content_hash();
            assert_ne!(right, legacy, "mode {mode} did not reach arena rendering");
            assert_eq!(right, native(&run).content_hash());
            assert!(knights::rigged_roll(&h, &run.book));
            run.players.get_mut(&1).unwrap().aim_x = -1.0;
            assert_ne!(right, native(&run).content_hash(), "mirror mode {mode}");
        });
    }
}

#[test]
fn quick_rig_override_respects_newer_legacy_sheets_and_pivot_height() {
    let first = crate::rules::cards::check(
        "rig-first-1",
        "name First Rig\nkind hold\narmor 1\ndraw worn\n.V.\n.V.\n",
    )
    .unwrap()
    .card;
    let second = crate::rules::cards::check(
        "rig-second-1",
        "name Newer Sheet\nkind hold\narmor 1\ndraw worn\n.W.\n.W.\n",
    )
    .unwrap()
    .card;
    let sprite = first.looks.worn.as_ref().unwrap().clone();
    let mut run = Run::new(21, 1, Some("Rig"));
    let mut h = knight();
    h.deck.push(first.id.clone());
    run.book.insert(first);
    let mut export = export();
    export["pivot"] = json!([0, 1]);
    let rig = Arc::new(Rig::parse(&export.to_string()).unwrap());
    quick_rig::with_sprite_rig(&sprite, rig, || {
        assert_eq!(knights::rig_floor_offset(&h, &run.book), 2);
        h.deck.push(second.id.clone());
        run.book.insert(second);
        h = rolling(&h, 5);
        assert!(!knights::rigged_roll(&h, &run.book));
        assert_eq!(knights::rig_floor_offset(&h, &run.book), 0);
        let probe = Img::from_rows(&["WWW", "W..", "W..", "W.."]);
        assert_eq!(
            pixels(&knights::rolled_hero(&probe, &h, &run.book, 5)),
            pixels(&knights::rolled(&probe, 5))
        );
        let body = knights::figure(&h, 1);
        let with_old_registration = knights::dressed_hero(&body, &h, &run.book, 0, 0).0;
        // The registration belongs to the older sheet; the newer legacy sheet
        // must not unexpectedly turn into the rig of a previous wish.
        let expected =
            knights::dressed_as(&body, knights::body_of(&h, &body), &h.worn(&run.book), 0, 0).0;
        assert_eq!(pixels(&with_old_registration), pixels(&expected));
    });
    let mut blank = export;
    for frame in blank["clips"]["idle"].as_array_mut().unwrap() {
        frame["rows"] = json!([".....", ".....", ".....", "....."]);
    }
    assert!(Rig::parse(&blank.to_string()).is_err());
}

/// Long, irregular one-shots make a fixed short elapsed cap immediately visible.
fn action_rig() -> Rig {
    let mut j = export();
    let mut frames = Vec::new();
    for (n, ticks) in [6, 12, 3, 3, 9, 9, 9].into_iter().enumerate() {
        let mut f = j["clips"]["idle"][n % 2].clone();
        f["ticks"] = json!(ticks);
        frames.push(f);
    }
    for clip in ["overhead", "flail", "thrust", "slash", "cast"] {
        j["clips"][clip] = json!(frames);
    }
    Rig::parse(&j.to_string()).unwrap()
}

fn melee_hero(name: &str) -> crate::rules::Hero {
    let mut h = knight();
    h.weapon = crate::rules::Weapon::Fists;
    h.forged = Some(
        crate::realm::forge::check(
            &format!("name {name}\nlook steel blade\nmelee damage=10 reach=2 arc=120 every=45"),
            crate::realm::forge::START_LOC,
        )
        .unwrap()
        .weapon,
    );
    h.sword = false;
    h.swing = 6;
    h.fire_cooldown = 45;
    h
}

#[test]
fn quick_rig_slow_heavy_attacks_reach_contact_and_full_recovery_from_cooldown() {
    let rig = action_rig();
    for (name, clip) in [
        ("War Hammer", Clip::Overhead),
        ("Iron Flail", Clip::Flail),
        ("Long Spear", Clip::Thrust),
        ("Sword", Clip::Slash),
    ] {
        let mut h = melee_hero(name);
        // 51 export ticks mapped onto 45 gameplay ticks: 0, 22, 38, 49.
        // These literal frame boundaries do not derive the expectation through
        // Rig::action or any of the selection helpers under test.
        for (remaining, expected) in [(45, 0), (25, 3), (11, 5), (1, 6)] {
            h.fire_cooldown = remaining;
            h.swing = if remaining == 45 { 6 } else { 0 };
            let sample = knights::rig_sample(&rig, &h, 500, false);
            assert_eq!(
                (sample.clip, sample.index),
                (clip, expected),
                "{name}, {remaining}"
            );
        }
        h.fire_cooldown = 0;
        assert_eq!(knights::rig_sample(&rig, &h, 500, false).clip, Clip::Idle);
    }
    let mut h = melee_hero("Hammer");
    h.bonus.rate = 50;
    h.fire_cooldown = 30;
    assert_eq!(knights::rig_sample(&rig, &h, 0, false).index, 0);
    h.fire_cooldown = 1;
    h.swing = 0;
    assert_eq!(knights::rig_sample(&rig, &h, 0, false).index, 6);
    h.bonus.rate = 0;
    h.champ.fury = 1;
    h.fire_cooldown = 22;
    assert_eq!(knights::rig_sample(&rig, &h, 0, false).index, 0);
}

#[test]
fn quick_rig_live_selector_uses_equipped_arm_card_not_an_unrelated_deck_card() {
    let rig = action_rig();
    let mut h = melee_hero("Unclassified");
    let mut run = Run::new(21, 1, Some("Rig"));
    let card = crate::rules::cards::check(
        "equipped-heavy-1",
        "name Storm Flail\nkind arm\nmelee damage=10 reach=2 arc=120 every=45\nlook steel blade\n",
    )
    .unwrap()
    .card;
    // The actual checked card carries forge runes; no animation field needed.
    assert!(matches!(
        card.arm,
        Some(crate::rules::cards::Arm::Forged(_))
    ));
    h.arm = Some(card.id.clone());
    h.deck.push(card.id.clone());
    run.book.insert(card);
    assert_eq!(
        knights::rig_sample_event(&rig, &h, &run.book, 100, None).clip,
        Clip::Flail
    );
    h.arm = None;
    assert_eq!(
        knights::rig_sample_event(&rig, &h, &run.book, 100, None).clip,
        Clip::Slash
    );
}

#[test]
fn quick_rig_grip_free_forged_melee_draws_to_the_exported_tip() {
    let mut j = export();
    j["clips"]["idle"][0]["anchors"]["grip"] = json!([1, 2]);
    j["clips"]["idle"][0]["anchors"]["tip"] = json!([4, 0]);
    j["clips"]["idle"][0]["rows"] = json!([".....", ".....", ".B...", "....."]);
    let rig = Rig::parse(&j.to_string()).unwrap();
    let mut h = melee_hero("Sword");
    h.owned.shield = false;
    h.swing = 0;
    h.fire_cooldown = 0;
    let im = knights::rig_figure(&h, 1, true, Default::default(), &rig, 0, false);
    // pivot [1,3] stages at x=15, dx=14; top is 12. The blade runs from
    // the grip out through the exported tip, steel all the way (`blades`).
    let steel = |x, y| {
        ['v', 'V', 'W', 'U'].iter().any(|&c| im.get(x, y) == super::super::ink::ink(c))
    };
    assert!(steel(18, 12), "the blade at the exported tip");
    assert!(steel(17, 13), "the blade on the way to it");
    assert!(
        im.get(15, 17).is_none(),
        "must not draw a fixed downward sword"
    );
}

#[test]
fn quick_rig_shield_has_rest_block_aim_throw_and_hit_feedback() {
    let mut j = export();
    // Large empty body leaves shield pixels visible, including its back layer.
    j["canvas"] = json!([20, 20]);
    j["pivot"] = json!([10, 18]);
    for frames in j["clips"].as_object_mut().unwrap().values_mut() {
        for f in frames.as_array_mut().unwrap() {
            f["rows"] = json!(vec!["...................."; 20]);
            f["rows"][18] = json!("..........V.........");
            f["anchors"] = json!({"grip":[14,10],"tip":[18,10],"off_hand":[6,10],
                "head":[10,2],"back":[10,9],"feet":[10,18]});
        }
    }
    let rig = Rig::parse(&j.to_string()).unwrap();
    let mut h = knight();
    h.weapon = crate::rules::Weapon::Fists;
    h.aim_x = 0.0;
    h.aim_y = 1.0;
    let image = |h: &crate::rules::Hero, thrown, struck| {
        pixels(&knights::rig_figure(h, 1, thrown, struck, &rig, 0, false))
    };
    let rest = image(&h, false, Default::default());
    let hit = knights::Struck {
        dx: 2,
        dy: -1,
        flash: 2,
    };
    assert_eq!(
        rest,
        image(&h, false, hit),
        "hanging shield does not block/flash"
    );
    h.shielding = true;
    let front = image(&h, false, Default::default());
    assert_ne!(front, rest);
    assert_ne!(front, image(&h, false, hit));
    h.aim_x = 1.0;
    h.aim_y = 0.0;
    let side = image(&h, false, Default::default());
    assert_ne!(front, side);
    h.aim_x = 0.0;
    h.aim_y = -1.0;
    let back = image(&h, false, Default::default());
    assert_ne!(side, back);
    assert_ne!(front, back);
    let thrown = image(&h, true, hit);
    h.shielding = false;
    assert_eq!(thrown, image(&h, true, Default::default()));
    assert_ne!(back, thrown);
}

#[test]
fn quick_rig_cast_uses_event_age_once_and_keeps_image_and_gear_in_step() {
    let rig = action_rig();
    let h = knight();
    let run = Run::new(21, 1, Some("Rig"));
    for world_tick in [0, 37, 900] {
        for (age, index) in [(0, 0), (6, 1), (22, 3), (50, 6), (500, 6)] {
            let sample = knights::rig_sample_event(&rig, &h, &run.book, world_tick, Some(age));
            assert_eq!((sample.clip, sample.index), (Clip::Cast, index));
        }
    }
    let mut run = Run::new(21, 1, Some("Rig"));
    let card = crate::rules::cards::check(
        "cast-body-1",
        "name Cast Body\nkind hold\narmor 1\ndraw worn\n.V.\n.V.\n",
    )
    .unwrap()
    .card;
    let sprite = card.looks.worn.as_ref().unwrap().clone();
    let mut h = h;
    h.deck.push(card.id.clone());
    run.book.insert(card);
    let mut mark = crate::rules::cards::check(
        "cast-mark-1",
        "name Cast Mark\nkind hold\narmor 1\ndraw worn at=right\n9\n",
    )
    .unwrap()
    .card;
    mark.looks.worn.as_mut().unwrap().at = Some("right".into());
    h.deck.push(mark.id.clone());
    run.book.insert(mark);
    quick_rig::with_sprite_rig(&sprite, Arc::new(rig), || {
        let body = knights::figure(&h, 1);
        let draw = |world, age| {
            knights::dressed_event(
                &body,
                &h,
                &run.book,
                world,
                0,
                Some(age),
                true,
                Default::default(),
                Some(1),
            )
            .0
        };
        let start = draw(0, 0);
        assert_eq!(pixels(&start), pixels(&draw(37, 0)));
        // grip x=3 on frame 0 and x=4 on frame 1, dx=14 and top=12.
        assert_eq!(start.get(17, 14), super::super::ink::ink('9'));
        assert_eq!(draw(37, 6).get(18, 14), super::super::ink::ink('9'));
    });
}

#[test]
fn quick_rig_pivot_survives_stone_and_all_toss_rotations() {
    let mut j = export();
    j["pivot"] = json!([0, 1]);
    let rig = Arc::new(Rig::parse(&j.to_string()).unwrap());
    let mut run = Run::new(21, 1, Some("Rig"));
    let card = crate::rules::cards::check(
        "pivot-body-1",
        "name Pivot Body\nkind hold\narmor 1\ndraw worn\n.V.\n.V.\n",
    )
    .unwrap()
    .card;
    let sprite = card.looks.worn.as_ref().unwrap().clone();
    let mut h = knight();
    h.deck.push(card.id.clone());
    run.book.insert(card);
    quick_rig::with_sprite_rig(&sprite, rig, || {
        let body = knights::dressed_hero(&knights::figure(&h, 1), &h, &run.book, 0, 0).0;
        assert_eq!((body.w, body.h), (33, 16));
        assert_eq!(knights::rig_pivot(&h, &run.book, &body), [16, 13]);
        let stone = sprites::stone(&body);
        assert_eq!(knights::rig_pivot(&h, &run.book, &stone), [16, 13]);
        for (turns, size, pivot) in [
            (0, (33, 16), [16, 13]),
            (1, (16, 33), [2, 16]),
            (2, (33, 16), [16, 2]),
            (3, (16, 33), [13, 16]),
            (4, (33, 16), [16, 13]),
        ] {
            let (turned, actual) = knights::tossed_figure(&body, &h, &run.book, turns);
            assert_eq!(((turned.w, turned.h), actual), (size, pivot));
        }
    });
}

#[test]
fn quick_rig_default_keeps_original_painted_worn_piece_contract_without_missing_helmet() {
    let mut run = Run::new(11, 1, Some("Friend"));
    let index = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(index);
    assert!(run.players[&1].owned.helmet);
    let card = crate::rules::cards::check("pavise",
        "name Pavise\nkind hold\narmor 1\ndraw worn at=left paint=abababababababababababababababababababababababababababababababab\nHHH..H\n.HiiH.\n").unwrap().card;
    run.players.get_mut(&1).unwrap().deck.push(card.id.clone());
    run.book.insert(card);
    let scene = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
    let p = &scene
        .figures
        .iter()
        .find(|f| !f.paints.is_empty())
        .unwrap()
        .paints[0];
    assert_eq!((p.rect[2], p.rect[3]), (6.0, 6.0));
    run.players.get_mut(&1).unwrap().aim_x = -1.0;
    let left = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
    let q = &left
        .figures
        .iter()
        .find(|f| !f.paints.is_empty())
        .unwrap()
        .paints[0];
    assert!(q.flip);
    assert_eq!(
        q.rect[0] - p.rect[0],
        12.0,
        "original worn-canvas mirror contract"
    );
    assert_eq!(q.inks, p.inks.flip_h());
}

#[test]
fn quick_rig_live_arena_cast_body_starts_from_the_event_not_the_world_phase() {
    let mut run = Run::new(21, 1, Some("Rig"));
    let index = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(index);
    run.enemies.clear();
    run.projectiles.clear();
    run.players.retain(|&id, _| id == 1);
    let card = crate::rules::cards::check(
        "event-body-1",
        "name Event Body\nkind hold\narmor 1\ndraw worn\n.V.\n.V.\n",
    )
    .unwrap()
    .card;
    let sprite = card.looks.worn.as_ref().unwrap().clone();
    let mut h = knight();
    (h.x, h.y) = (run.players[&1].x, run.players[&1].y);
    h.invulnerable = 0;
    h.weapon = crate::rules::Weapon::Fists;
    h.owned.shield = false;
    h.deck.push(card.id.clone());
    run.book.insert(card);
    run.players.insert(1, h);
    quick_rig::with_sprite_rig(&sprite, Arc::new(action_rig()), || {
        let draw = |run: &mut Run, world, age| {
            run.tick = world;
            run.minions.casts.insert(
                1,
                crate::rules::minions::Cast {
                    tick: world - age,
                    rite: crate::rules::minions::Rite::Raise,
                    x: run.players[&1].x,
                    y: run.players[&1].y,
                    torn: Vec::new(),
                },
            );
            let scene = scene_for(run, NATIVE_W, NATIVE_H, Some(1));
            scene
                .figures
                .iter()
                .flat_map(|f| &f.bodies)
                .find(|b| b.img.w == 31 && b.img.h == 16)
                .unwrap()
                .img
                .clone()
        };
        let start = draw(&mut run, 100, 0);
        assert_eq!(pixels(&start), pixels(&draw(&mut run, 137, 0)));
        let next = draw(&mut run, 106, 6);
        assert_ne!(pixels(&start), pixels(&next));
        assert_eq!(pixels(&next), pixels(&draw(&mut run, 943, 6)));
    });
}

#[test]
fn quick_rig_live_arena_places_stone_and_toss_at_the_transformed_contact() {
    let mut run = Run::new(21, 1, Some("Rig"));
    let index = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(index);
    run.enemies.clear();
    run.projectiles.clear();
    run.players.retain(|&id, _| id == 1);
    let mut h = knight();
    (h.x, h.y) = (run.players[&1].x, run.players[&1].y);
    h.invulnerable = 0;
    h.weapon = crate::rules::Weapon::Fists;
    h.owned.shield = false;
    let (hx, hy) = at(h.x, h.y);
    let card = crate::rules::cards::check(
        "contact-body-1",
        "name Contact Body\nkind hold\narmor 1\ndraw worn\n.V.\n.V.\n",
    )
    .unwrap()
    .card;
    let sprite = card.looks.worn.as_ref().unwrap().clone();
    h.deck.push(card.id.clone());
    run.book.insert(card);
    run.players.insert(1, h);
    let mut j = export();
    j["pivot"] = json!([0, 1]);
    quick_rig::with_sprite_rig(
        &sprite,
        Arc::new(Rig::parse(&j.to_string()).unwrap()),
        || {
            let normal = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
            let base = normal
                .figures
                .iter()
                .flat_map(|f| &f.bodies)
                .find(|b| b.img.w == 33 && b.img.h == 16)
                .unwrap();
            assert_eq!((base.x + 16, base.y + 13), (hx, hy));
            run.players.get_mut(&1).unwrap().stone = true;
            let stone = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
            let statue = stone
                .figures
                .iter()
                .flat_map(|f| &f.bodies)
                .find(|b| b.img.w == 33 && b.img.h == 16)
                .unwrap();
            assert_eq!(
                (statue.x, statue.y),
                (base.x, base.y),
                "no petrification jump"
            );
            run.players.get_mut(&1).unwrap().stone = false;
            for (left, dimensions, pivot) in [
                (12, (33, 16), [16, 13]),
                (3, (16, 33), [2, 16]),
                (6, (33, 16), [16, 2]),
                (9, (16, 33), [13, 16]),
            ] {
                run.players.get_mut(&1).unwrap().tossed = left;
                let scene = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
                let thrown = scene
                    .figures
                    .iter()
                    .flat_map(|f| &f.bodies)
                    .find(|b| (b.img.w, b.img.h) == dimensions)
                    .unwrap();
                let flight =
                    (crate::rules::tide::TOSSED - left) as f32 / crate::rules::tide::TOSSED as f32;
                let lift = ((flight * std::f32::consts::PI).sin() * 10.0) as i32;
                assert_eq!(
                    (thrown.x + pivot[0], thrown.y + pivot[1]),
                    (hx, hy - lift),
                    "toss {left}"
                );
            }
        },
    );
}

#[test]
fn quick_rig_registration_cannot_lend_roll_or_pivot_to_an_authored_identity() {
    let mut run = Run::new(21, 1, Some("Rig"));
    let card = crate::rules::cards::check(
        "identity-body-1",
        "name Identity Body\nkind hold\narmor 1\ndraw worn\n.V.\n.V.\n",
    )
    .unwrap()
    .card;
    let sprite = card.looks.worn.as_ref().unwrap().clone();
    let mut h = rolling(&knight(), 5);
    h.deck.push(card.id.clone());
    run.book.insert(card);
    quick_rig::with_sprite_rig(&sprite, Arc::new(rig()), || {
        assert!(knights::rigged_roll(&h, &run.book));
        h.shape = Some(crate::rules::identity::Shape::Circle);
        assert!(!knights::rigged_roll(&h, &run.book));
        assert_eq!(knights::rig_floor_offset(&h, &run.book), 0);
    });
}

#[test]
fn quick_rig_eventless_caller_keeps_metadata_for_its_actual_timed_canvas() {
    let mut run = Run::new(21, 1, Some("Rig"));
    let mut h = rolling(&knight(), 5);
    let card = crate::rules::cards::check(
        "portable-mark-1",
        "name Portable Mark\nkind hold\narmor 1\ndraw worn at=left\n9\n",
    )
    .unwrap()
    .card;
    h.deck.push(card.id.clone());
    run.book.insert(card);
    let timed = knights::figure_at(&h, 1, false, Default::default(), false, 6);
    let timed = knights::dressed_casting(
        &timed,
        &h,
        &run.book,
        6,
        0,
        false,
        false,
        Default::default(),
    )
    .0;
    if let Some(rig) = quick_rig::seat_knight(1) {
        assert_eq!(
            knights::rig_pivot(&h, &run.book, &timed),
            [timed.w / 2, timed.h - rig.canvas()[1] + rig.pivot()[1]]
        );
        assert!(knights::rigged_roll_in(&timed, &h, &run.book));
        assert_eq!(
            pixels(&knights::rolled_hero(&timed, &h, &run.book, 5)),
            pixels(&timed)
        );
    }
    let native = knights::dressed_hero(&knights::figure(&h, 1), &h, &run.book, 6, 0).0;
    assert_eq!(
        knights::rig_pivot(&h, &run.book, &native),
        [native.w / 2, native.h - 1]
    );
    assert!(!knights::rigged_roll_in(&native, &h, &run.book));
    assert_eq!(
        pixels(&knights::rolled_hero(&native, &h, &run.book, 5)),
        pixels(&knights::rolled(&native, 5))
    );
}


#[path = "arena__quick_rig__shots.rs"]
mod shots;

#[path = "arena__knights_v2__shots.rs"]
mod knights_v2_shots;

/// Three portable wishes, not a body-sheet override: all must follow the
/// production knight's timed sockets without opting out of the rig.
fn equip_runtime_gear(run: &mut Run) {
    for (id, at, rows) in [
        ("rig-weapon-1", "right", "W\nW\nB\n"),
        ("rig-head-1", "head", "44444\n4TTT4\n"),
        ("rig-back-1", "back", "888\n8qq\n8qq\n888\n"),
    ] {
        let card = crate::rules::cards::check(
            id,
            &format!("name Runtime Gear\nkind hold\narmor 1\ndraw worn at={at}\n{rows}"),
        )
        .unwrap()
        .card;
        run.players.get_mut(&1).unwrap().deck.push(card.id.clone());
        run.book.insert(card);
    }
}

pub(super) fn arena_run() -> Run {
    let mut run = Run::new(11, 1, Some("Rig"));
    let index = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(index);
    run.enemies.clear();
    run.tick = 100;
    let old = &run.players[&1];
    let mut hero = knight();
    (hero.x, hero.y) = (old.x, old.y);
    hero.invulnerable = 0;
    hero.owned.shield = false;
    hero.guard = crate::rules::cards::Guard::Roll;
    run.players.insert(1, hero);
    run
}

#[test]
fn quick_rig_ordinary_worn_gear_uses_one_timed_sample_in_the_real_arena() {
    let rig = quick_rig::seat_knight(1).expect("compiled production knight");
    let mut run = arena_run();
    equip_runtime_gear(&mut run);
    let mut base = run.players[&1].clone();
    // Isolate the card-owned gear from the native placeholder bow.
    base.weapon = crate::rules::Weapon::Fists;
    for (mode, clip) in [(0, Clip::Walk), (1, Clip::Roll), (2, Clip::Slash)] {
        let mut previous = None;
        for phase in [0, 1, 2] {
            let mut hero = base.clone();
            match mode {
                0 => hero.stride = 0.2 + phase as f32 * 0.9,
                1 => hero = rolling(&hero, 9 - phase * 3),
                _ => {
                    hero.sword = true;
                    hero.swing = 6 - phase * 2;
                }
            }
            run.players.insert(1, hero.clone());
            let sample = knights::rig_sample_event(rig, &hero, &run.book, run.tick, None);
            assert_eq!(sample.clip, clip);
            let composed = knights::dressed_frame(
                || panic!("worn gear bypassed rig"),
                &hero,
                &run.book,
                run.tick,
                0,
                None,
                false,
                Default::default(),
                Some(1),
            );
            // Reconstruct from the selected sample, not rig_figure's native
            // placeholder weapon or the old axis-aligned dressing path.
            let radius = sample.pivot()[0].max(rig.canvas()[0] - 1 - sample.pivot()[0]) + 12;
            let shift = [radius - sample.pivot()[0], 12];
            let mut body = Img::new(radius * 2 + 1, rig.canvas()[1] + 12);
            body.stamp(
                &sample.image().recolor(knights::realm_colours(1)),
                shift[0],
                shift[1],
            );
            let fit = knights::body_of(&hero, &body);
            let anchors = sample.anchors();
            let mut pieces = Vec::new();
            for (part, socket, sprite) in [
                (fit.right, anchors.grip, "rig-weapon-1"),
                (fit.head, anchors.head, "rig-head-1"),
                (fit.back, anchors.back, "rig-back-1"),
            ] {
                assert_eq!(part.anchor, (socket[0] + shift[0], socket[1] + shift[1]));
                let sprite = run.book.get(sprite).unwrap().looks.worn.as_ref().unwrap();
                let fitted = wear::fitted(sprite, part);
                let source = knights::frame_img(fitted.frame(run.tick));
                let contact = [
                    source.w / 2,
                    if part.bottom {
                        source.h - 1
                    } else {
                        source.h / 2
                    },
                ];
                let (image, at) = if sprite.at.as_deref() == Some("right") {
                    let grip = [part.anchor.0, part.anchor.1];
                    let tip = [anchors.tip[0] + shift[0], anchors.tip[1] + shift[1]];
                    let orientation = quick_rig::WeaponOrientation::new(contact, grip, tip);
                    assert_eq!(
                        orientation.point([contact[0] as f32, contact[1] as f32]),
                        [grip[0] as f32, grip[1] as f32]
                    );
                    let angle = ((tip[0] - grip[0]) as f32).atan2((grip[1] - tip[1]) as f32);
                    assert!((orientation.angle - angle).abs() < 1e-5);
                    let (image, at) = orientation.stamp(&source);
                    assert_eq!(
                        image.get(grip[0] - at[0], grip[1] - at[1]),
                        source.get(contact[0], contact[1])
                    );
                    (image, at)
                } else {
                    let at = part.origin((source.w, source.h));
                    assert_eq!((at.0 + contact[0], at.1 + contact[1]), part.anchor);
                    (source, [at.0, at.1])
                };
                pieces.push((image, at, part.under));
            }
            let mut pad = 0;
            let mut above = 0;
            let mut below = 0;
            for (image, at, _) in &pieces {
                pad = pad.max(-at[0]).max(at[0] + image.w - body.w);
                above = above.max(-at[1]);
                below = below.max(at[1] + image.h - body.h);
            }
            let mut expected = Img::new(body.w + 2 * pad, body.h + above + below);
            for under in [true, false] {
                if !under {
                    expected.stamp(&body, pad, above);
                }
                for (image, at, behind) in &pieces {
                    if *behind == under {
                        expected.stamp(image, at[0] + pad, at[1] + above);
                    }
                }
            }
            assert_eq!(composed.image, expected, "{clip:?} phase {phase}");
            let pivot = [radius + pad, sample.pivot()[1] + 12 + above];
            assert_eq!(composed.pivot, Some(pivot));
            assert_eq!(composed.authored_roll, mode == 1);
            assert!(composed.paints.is_empty());
            let scene = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
            let captured = scene
                .figures
                .iter()
                .flat_map(|f| &f.bodies)
                .find(|f| f.img == composed.image)
                .expect("geared body in scene");
            let feet = if run.side_on() { FOOT } else { 0.0 };
            assert_eq!(
                (captured.x + pivot[0], captured.y + pivot[1]),
                at(hero.x, hero.y + feet)
            );
            assert!(
                scene
                    .figures
                    .iter()
                    .flat_map(|f| &f.bodies)
                    .any(|f| f.img == composed.image),
                "geared clip {clip:?} phase {phase} did not reach scene capture"
            );
            let actual = frame(&run, NATIVE_W, NATIVE_H).content_hash();
            let legacy =
                quick_rig::with_legacy_knight(|| frame(&run, NATIVE_W, NATIVE_H).content_hash());
            assert_ne!(actual, legacy, "gear must not silently select the old body");
            assert_eq!(actual, frame(&run, NATIVE_W, NATIVE_H).content_hash());
            if let Some(previous) = previous {
                assert_ne!(actual, previous);
            }
            previous = Some(actual);
            run.players.get_mut(&1).unwrap().aim_x = -1.0;
            assert_ne!(actual, frame(&run, NATIVE_W, NATIVE_H).content_hash());
        }
    }
}

#[test]
fn quick_rig_geared_contact_survives_fit_padding_stone_and_toss() {
    let mut run = arena_run();
    equip_runtime_gear(&mut run);
    // Gear shares the selected body's contact, including its canvas margins.
    let hero = &run.players[&1];
    let composed = knights::dressed_frame(
        || panic!("expected rig"),
        hero,
        &run.book,
        run.tick,
        0,
        None,
        false,
        Default::default(),
        Some(1),
    );
    let pivot = composed.pivot.unwrap();
    let rig = quick_rig::seat_knight(1).unwrap();
    assert_eq!(
        pivot,
        [
            composed.image.w / 2,
            composed.image.h - rig.canvas()[1] + rig.pivot()[1]
        ]
    );
    run.players.get_mut(&1).unwrap().stone = true;
    let stone = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
    let expected = sprites::stone(&composed.image);
    let figure = stone
        .figures
        .iter()
        .flat_map(|f| &f.bodies)
        .find(|f| f.img == expected)
        .unwrap();
    let feet = if run.side_on() { FOOT } else { 0.0 };
    let ground = at(hero_position(&run).0, hero_position(&run).1 + feet);
    assert_eq!((figure.x + pivot[0], figure.y + pivot[1]), ground);
    for turns in 0..4 {
        let mut h = run.players[&1].clone();
        h.stone = false;
        h.tossed = 3 * (turns + 1);
        run.players.insert(1, h.clone());
        let (expected, contact) =
            knights::tossed_at(&composed.image, composed.pivot, (h.tossed / 3) % 4);
        let scene = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
        let figure = scene
            .figures
            .iter()
            .flat_map(|f| &f.bodies)
            .find(|f| f.img == expected)
            .unwrap();
        let flight = (crate::rules::tide::TOSSED - h.tossed.min(crate::rules::tide::TOSSED)) as f32
            / crate::rules::tide::TOSSED as f32;
        let lift = ((flight * std::f32::consts::PI).sin() * 10.0) as i32;
        assert_eq!(
            (figure.x + contact[0], figure.y + contact[1]),
            (ground.0, ground.1 - lift)
        );
    }
}

fn hero_position(run: &Run) -> (f32, f32) {
    (run.players[&1].x, run.players[&1].y)
}

#[test]
fn quick_rig_padded_equipment_carries_the_contact_without_resampling() {
    // A tall weapon forces top padding; a wide hat at the canvas edge forces
    // side padding. Keep this fixture's weapon upright: rotation is tested below.
    let mut j = export();
    j["canvas"] = json!([64, 40]);
    j["pivot"] = json!([1, 33]);
    for frames in j["clips"].as_object_mut().unwrap().values_mut() {
        for frame in frames.as_array_mut().unwrap() {
            frame["rows"] = json!(vec!["V".repeat(64); 40]);
            frame["anchors"]["grip"] = json!([3, 1]);
            frame["anchors"]["tip"] = json!([3, 0]);
            frame["anchors"]["head"] = json!([63, 0]);
        }
    }
    let rig = Arc::new(Rig::parse(&j.to_string()).unwrap());
    let mut run = arena_run();
    let card = crate::rules::cards::check(
        "padded-body-1",
        "name Tall Body\nkind hold\narmor 1\ndraw worn\nV\n",
    )
    .unwrap()
    .card;
    let sprite = card.looks.worn.as_ref().unwrap().clone();
    run.players.get_mut(&1).unwrap().deck.push(card.id.clone());
    run.book.insert(card);
    let weapon = crate::rules::cards::check(
        "padded-gear-1",
        &format!(
            "name Tall Weapon\nkind hold\narmor 1\ndraw worn at=right\n{}",
            "W\n".repeat(20)
        ),
    )
    .unwrap()
    .card;
    run.players
        .get_mut(&1)
        .unwrap()
        .deck
        .push(weapon.id.clone());
    run.book.insert(weapon);
    let hat = crate::rules::cards::check(
        "padded-hat-1",
        &format!(
            "name Wide Hat\nkind hold\narmor 1\ndraw worn at=head\n{}\n",
            "4".repeat(20)
        ),
    )
    .unwrap()
    .card;
    run.players.get_mut(&1).unwrap().deck.push(hat.id.clone());
    run.book.insert(hat);
    run.players.get_mut(&1).unwrap().weapon = crate::rules::Weapon::Fists;
    quick_rig::with_sprite_rig(&sprite, rig.clone(), || {
        let hero = &run.players[&1];
        let composed = knights::dressed_frame(
            || panic!("expected rig"),
            hero,
            &run.book,
            run.tick,
            0,
            None,
            false,
            Default::default(),
            Some(1),
        );
        assert!(
            composed.image.h > 52,
            "equipment must actually add top padding"
        );
        let sample = knights::rig_sample_event(&rig, hero, &run.book, run.tick, None);
        let radius = sample.pivot()[0].max(rig.canvas()[0] - 1 - sample.pivot()[0]) + 12;
        let mut bare = Img::new(radius * 2 + 1, 52);
        bare.stamp(&sample.image(), radius - sample.pivot()[0], 12);
        let gear: Vec<_> = hero
            .worn(&run.book)
            .into_iter()
            .filter(|s| s.at.as_deref().is_some_and(|at| at != "whole"))
            .collect();
        let expected =
            knights::dressed_as(&bare, knights::staged_fit(&sample), &gear, run.tick, 0).0;
        assert_eq!(
            composed.image, expected,
            "padding must retain every body and gear pixel"
        );
        assert!(
            composed.image.w > bare.w,
            "equipment must actually add side padding"
        );
        let pivot = composed.pivot.unwrap();
        assert_eq!(
            pivot,
            [
                bare.w / 2 + (composed.image.w - bare.w) / 2,
                composed.image.h - 40 + 33
            ]
        );
        assert!(!composed.authored_roll);
        assert!(composed.paints.is_empty());
        let scene = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
        let body = scene
            .figures
            .iter()
            .flat_map(|f| &f.bodies)
            .find(|b| b.img == composed.image)
            .unwrap();
        assert_eq!((body.x + pivot[0], body.y + pivot[1]), at(hero.x, hero.y));
        for turns in 0..4 {
            let (image, transformed) = knights::tossed_at(&composed.image, composed.pivot, turns);
            assert!(transformed[0] >= 0 && transformed[0] < image.w);
            assert!(transformed[1] >= 0 && transformed[1] < image.h);
        }
    });
}


#[test]
fn quick_rig_painted_gear_keeps_its_ink_motion_pivot_after_live_fit() {
    let mut run = arena_run();
    let card = crate::rules::cards::check("moving-paint-1", &format!(
        "name Moving Shield\nkind hold\narmor 1\ndraw worn at=left paint={} move=bob,wobble,stretch\nHHH..H\n.HiiH.\n", "ab".repeat(32)))
        .unwrap().card;
    run.players.get_mut(&1).unwrap().deck.push(card.id.clone());
    run.book.insert(card);
    // Locomotion fixes the sample by stride; body_of's historical API has no tick.
    run.players.get_mut(&1).unwrap().stride = 1.8;
    for aim in [1.0, -1.0] {
        run.players.get_mut(&1).unwrap().aim_x = aim;
        for tick in [0, 7, 31, 63] {
            let hero = &run.players[&1];
            let composed = knights::dressed_frame(
                || panic!("expected rig"),
                hero,
                &run.book,
                tick,
                37,
                None,
                false,
                Default::default(),
                Some(1),
            );
            let p = &composed.paints[0];
            let rig_body = knights::figure_at(hero, 1, false, Default::default(), false, tick);
            let fit = knights::body_of(hero, &rig_body);
            let sprite = hero.worn(&run.book)[0];
            let fitted = wear::fitted(sprite, fit.left);
            let inks = knights::frame_img(fitted.frame(tick));
            let [x, y, w, h] = crate::rules::looks::ink_bounds(&sprite.frames).unwrap();
            let [sx, sy, sw, sh] = paintings::square(sprite, false, 1.0).unwrap();
            let local = (
                (sx + sw / 2.0 - x as f32) * inks.w as f32 / w as f32,
                (sy + sh / 2.0 - y as f32) * inks.h as f32 / h as f32,
            );
            let (expected, _) = p.pose.apply(&inks, local);
            assert_eq!(
                p.inks, expected,
                "painting and fallback motion share the fitted pivot"
            );
            // The square stays at the exact fitted origin: no paint-only
            // half-pixel correction after the ink motion has been resolved.
            let origin = fit.left.origin((inks.w, inks.h));
            let pad = (composed.image.w - rig_body.w) / 2;
            assert_eq!(
                p.rect[0],
                (origin.0 + pad) as f32 + (sx - x as f32) * inks.w as f32 / w as f32
            );
        }
    }
}

// Rig orientation intentionally does not redefine the saved Sprite contact:
// this asymmetric blade has both a tail below the centre and a marked hilt.
fn oriented_weapon_rows() -> Vec<String> {
    [
        ".7.", ".W.", ".W.", ".WJ", ".W.", ".W.", "69.", ".B.", ".B.", ".B.", ".B.", ".B.", ".B.",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[test]
fn quick_rig_weapon_rotation_keeps_exact_centre_and_bottom_contacts() {
    use quick_rig::WeaponOrientation;
    let source = knights::frame_img(&oriented_weapon_rows());
    for contact in [[1, 6], [1, 12]] {
        for direction in [[0, -10], [10, 0], [0, 10], [-10, 0], [0, 0]] {
            let grip = [20, 20];
            let tip = [grip[0] + direction[0], grip[1] + direction[1]];
            let geometry = WeaponOrientation::new(contact, grip, tip);
            let (rotated, at) = geometry.stamp(&source);
            assert_eq!(
                rotated.get(grip[0] - at[0], grip[1] - at[1]),
                source.get(contact[0], contact[1])
            );
            assert_eq!(rotated.pixels().count(), source.pixels().count());
            for y in 0..source.h {
                for x in 0..source.w {
                    let p = geometry.point([x as f32, y as f32]);
                    assert_eq!(
                        rotated.get(p[0].round() as i32 - at[0], p[1].round() as i32 - at[1]),
                        source.get(x, y)
                    );
                }
            }
            assert!(geometry.angle.is_finite());
        }
    }
    // Oblique inverse-nearest rotations mirror both silhouette and handed
    // detail. Compare in world coordinates, independently of tight crop sizes.
    for tip in [[27, 17], [29, 26], [14, 27]] {
        let right = WeaponOrientation::new([1, 6], [20, 20], tip);
        let left = WeaponOrientation::new([1, 6], [40, 20], [60 - tip[0], tip[1]]);
        let (r, ra) = right.stamp(&source);
        let (l, la) = left.stamp(&source.flip_h());
        for y in 0..45 {
            for x in 0..61 {
                assert_eq!(
                    r.get(x - ra[0], y - ra[1]),
                    l.get(60 - x - la[0], y - la[1])
                );
            }
        }
        assert_eq!(r.get(20 - ra[0], 20 - ra[1]), source.get(1, 6));
    }
}

#[test]
fn quick_rig_oriented_right_slot_keeps_left_native_and_all_padding_at_the_hand() {
    let mut j = export();
    // Four cardinal poses exercise above/below-floor and both side extents.
    for frames in j["clips"].as_object_mut().unwrap().values_mut() {
        frames[0]["anchors"]["grip"] = json!([4, 3]);
        frames[0]["anchors"]["tip"] = json!([4, 0]);
        frames[1]["anchors"]["grip"] = json!([4, 3]);
        frames[1]["anchors"]["tip"] = json!([0, 3]);
    }
    j["clips"]["thrust"][0]["anchors"]["grip"] = json!([0, 0]);
    j["clips"]["thrust"][0]["anchors"]["tip"] = json!([4, 0]);
    j["clips"]["thrust"][1]["anchors"]["grip"] = json!([0, 0]);
    j["clips"]["thrust"][1]["anchors"]["tip"] = json!([0, 3]);
    let rig = Rig::parse(&j.to_string()).unwrap();
    let right = crate::rules::looks::Sprite {
        frames: vec![oriented_weapon_rows()],
        at: Some("right".into()),
        ..Default::default()
    };
    let left = crate::rules::looks::Sprite {
        frames: vec![vec!["hH".into(), "Hh".into(), "hh".into()]],
        at: Some("left".into()),
        ..Default::default()
    };
    let saved = serde_json::to_value(&right).unwrap();
    for clip in [Clip::Slash, Clip::Thrust] {
        for tick in [0, 2] {
            for bottom in [false, true] {
                let sample = rig.sample(clip, tick, Playback::Once, false);
                let template =
                    knights::rig_figure(&knight(), 1, false, Default::default(), &rig, tick, false);
                let body = Img::new(template.w, template.h);
                let mut fit = knights::staged_fit(&sample);
                fit.right.size = (3, 13);
                fit.right.bottom = bottom;
                fit.left.size = (2, 3);
                fit.left.anchor = (2, body.h - 2);
                let frame = knights::dressed_rig(&body, fit, &[&right, &left], tick, 0, &sample);
                let pivot = frame.pivot.unwrap();
                let shift = [pivot[0] - body.w / 2, pivot[1] - sample.pivot()[1] - 12];
                let source = knights::frame_img(&right.frames[0]);
                let contact = [1, if bottom { 12 } else { 6 }];
                let grip = [fit.right.anchor.0, fit.right.anchor.1];
                let tip = [
                    sample.anchors().tip[0] + body.w / 2 - sample.pivot()[0],
                    sample.anchors().tip[1] + 12,
                ];
                let geometry = quick_rig::WeaponOrientation::new(contact, grip, tip);
                for y in 0..source.h {
                    for x in 0..source.w {
                        if let Some(c) = source.get(x, y) {
                            let p = geometry.point([x as f32, y as f32]);
                            let (x, y) = (
                                p[0].round() as i32 + shift[0],
                                p[1].round() as i32 + shift[1],
                            );
                            assert!(
                                x >= 0 && x < frame.image.w && y >= 0 && y < frame.image.h,
                                "weapon clipped at {clip:?}/{tick}"
                            );
                            assert_eq!(frame.image.get(x, y), Some(c));
                        }
                    }
                }
                let left_im = knights::frame_img(&left.frames[0]);
                let origin = fit.left.origin((2, 3));
                for y in 0..3 {
                    for x in 0..2 {
                        assert_eq!(
                            frame
                                .image
                                .get(origin.0 + x + shift[0], origin.1 + y + shift[1]),
                            left_im.get(x, y),
                            "left slot must remain axis aligned"
                        );
                    }
                }
                let legacy = knights::dressed_as(&body, fit, &[&right], tick, 0).0;
                if tip[0] != grip[0] || tip[1] >= grip[1] {
                    assert_ne!(frame.image, legacy);
                }
            }
        }
    }
    assert_eq!(
        serde_json::to_value(&right).unwrap(),
        saved,
        "no Sprite save mutation"
    );
}

#[test]
fn quick_rig_live_right_weapon_and_painting_swing_with_slash_and_thrust() {
    let rig = quick_rig::seat_knight(1).unwrap();
    let mut run = arena_run();
    let card = crate::rules::cards::check(
        "oriented-blade-1",
        &format!(
            "name Blade\nkind hold\narmor 1\ndraw worn at=right paint={}\n{}\n",
            "ab".repeat(32),
            oriented_weapon_rows().join("\n")
        ),
    )
    .unwrap()
    .card;
    let saved = serde_json::to_value(card.looks.worn.as_ref().unwrap()).unwrap();
    let id = card.id.clone();
    run.book.insert(card);
    for (name, clip) in [("Sword", Clip::Slash), ("Spear", Clip::Thrust)] {
        let mut angles = Vec::new();
        for cooldown in [45, 28, 15, 1] {
            let mut hero = melee_hero(name);
            hero.fire_cooldown = cooldown;
            hero.aim_x = 1.0;
            hero.aim_y = 0.0;
            hero.deck.push(id.clone());
            run.players.insert(1, hero.clone());
            let sample = knights::rig_sample_event(rig, &hero, &run.book, run.tick, None);
            assert_eq!(sample.clip, clip);
            let composed = knights::dressed_frame(
                || panic!("rig bypassed"),
                &hero,
                &run.book,
                run.tick,
                0,
                None,
                false,
                Default::default(),
                Some(1),
            );
            let p = &composed.paints[0];
            let grip = sample.anchors().grip;
            let tip = sample.anchors().tip;
            let angle = ((tip[0] - grip[0]) as f32).atan2((grip[1] - tip[1]) as f32);
            assert!((p.placed().1 - angle).abs() < 1e-5);
            angles.push(angle);
            let pivot = composed.pivot.unwrap();
            let world_grip = [
                grip[0] + pivot[0] - sample.pivot()[0],
                grip[1] + pivot[1] - sample.pivot()[1],
            ];
            // The built-in profile is bottom=true; wished Body::human rigs
            // use centre contacts. Neither is the PNG's square pivot.
            let native = crate::rules::looks::profile("realm_right").right;
            let sprite = run.book.get(&id).unwrap().looks.worn.as_ref().unwrap();
            let fitted = wear::fitted(sprite, native);
            let source = knights::frame_img(fitted.frame(run.tick));
            let cy = if native.bottom {
                source.h - 1
            } else {
                source.h / 2
            };
            assert_eq!(
                p.inks.get(world_grip[0] - p.at.0, world_grip[1] - p.at.1),
                source.get(source.w / 2, cy)
            );
            let (rect, _) = p.placed();
            let source_centre_offset = quick_rig::WeaponOrientation::new(
                [0, 0],
                [0, 0],
                [tip[0] - grip[0], tip[1] - grip[1]],
            )
            .point([
                source.w as f32 / 2.0 - (source.w / 2) as f32 - 0.5,
                source.h as f32 / 2.0 - cy as f32 - 0.5,
            ]);
            assert!(
                (rect[0] + rect[2] / 2.0 - (world_grip[0] as f32 + 0.5 + source_centre_offset[0]))
                    .abs()
                    < 1e-4
            );
            assert!(
                (rect[1] + rect[3] / 2.0 - (world_grip[1] as f32 + 0.5 + source_centre_offset[1]))
                    .abs()
                    < 1e-4
            );
            assert!(
                p.at.0 >= 0
                    && p.at.1 >= 0
                    && p.at.0 + p.inks.w <= composed.image.w
                    && p.at.1 + p.inks.h <= composed.image.h
            );
            let scene = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
            assert!(
                scene
                    .figures
                    .iter()
                    .flat_map(|f| &f.bodies)
                    .any(|b| b.img == composed.image)
            );
            assert!(
                scene
                    .figures
                    .iter()
                    .flat_map(|f| &f.paints)
                    .any(|q| q.inks == p.inks && (q.placed().1 - angle).abs() < 1e-5)
            );
            // The real stage compositor must retain the turn, not just
            // capture metadata while leaving the rendered PNG axis aligned.
            const TEST_PAINT: u32 = 1_234_567;
            let stage = stage_frame::StageMaker::default().make_with(
                &Arc::new(scene.clone()),
                |_| None,
                |_| Some(TEST_PAINT),
            );
            assert!(stage.layers.iter().any(|l| {
                matches!(l.kind, angel_stage::LayerKind::Picture(TEST_PAINT))
                    && (l.angle - angle).abs() < 1e-5
            }));
            run.players.get_mut(&1).unwrap().aim_x = -1.0;
            let mirrored = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
            assert!(
                mirrored
                    .figures
                    .iter()
                    .flat_map(|f| &f.bodies)
                    .any(|b| b.img == composed.image.flip_h())
            );
            assert!(
                mirrored
                    .figures
                    .iter()
                    .flat_map(|f| &f.paints)
                    .any(|q| q.flip
                        && q.inks == p.inks.flip_h()
                        && (q.placed().1 + angle).abs() < 1e-5)
            );
        }
        assert!(
            angles.windows(2).any(|p| (p[0] - p[1]).abs() > 0.5),
            "{clip:?} never swung"
        );
    }
    assert_eq!(
        serde_json::to_value(run.book.get(&id).unwrap().looks.worn.as_ref().unwrap()).unwrap(),
        saved
    );
}

#[test]
fn quick_rig_weapon_alias_and_authored_motion_keep_shared_ink_paint_transform() {
    // In an up-pointing pose rig rotation is identity, so every pre-existing
    // motion/PNG pivot result must be byte-for-byte the native composition.
    let mut j = export();
    j["clips"]["idle"][0]["anchors"]["grip"] = json!([4, 3]);
    j["clips"]["idle"][0]["anchors"]["tip"] = json!([4, 0]);
    let rig = Rig::parse(&j.to_string()).unwrap();
    let sample = rig.sample(Clip::Idle, 0, Playback::Once, false);
    let template = knights::rig_figure(&knight(), 1, false, Default::default(), &rig, 0, false);
    // Extra floor space keeps this test about motion rather than floor lift.
    let body = Img::new(template.w, template.h + 30);
    let mut fit = knights::staged_fit(&sample);
    fit.right.size = (3, 13);
    let card = crate::rules::cards::check("moving-blade-1", &format!(
        "name Moving Blade\nkind hold\narmor 1\ndraw worn at=right paint={} move=bob,wobble,stretch\n{}\n",
        "ab".repeat(32), oriented_weapon_rows().join("\n"))).unwrap().card;
    let mut sprite = card.looks.worn.as_ref().unwrap().clone();
    // Old saves may use this alias even though the checker canonicalizes it.
    sprite.at = Some("weapon".into());
    for tick in [0, 7, 31, 63] {
        let native = knights::dressed_as(&body, fit, &[&sprite], tick, 37);
        let frame = knights::dressed_rig(&body, fit, &[&sprite], tick, 37, &sample);
        assert_eq!(frame.image, native.0);
        let (p, q) = (&frame.paints[0], &native.1[0]);
        assert_eq!(p.inks, q.inks);
        assert_eq!(p.at, q.at);
        assert_eq!(p.placed(), q.placed());
    }
    // An oblique tip rotates the already moved hard inks and the already
    // placed painting, not the unmoved PNG square about its centre.
    j["clips"]["idle"][0]["anchors"]["tip"] = json!([0, 1]);
    let rig = Rig::parse(&j.to_string()).unwrap();
    let tilted = rig.sample(Clip::Idle, 0, Playback::Once, false);
    for tick in [7, 31, 63] {
        let native = knights::dressed_as(&body, fit, &[&sprite], tick, 37);
        let q = &native.1[0];
        let frame = knights::dressed_rig(&body, fit, &[&sprite], tick, 37, &tilted);
        let p = &frame.paints[0];
        let origin = [fit.right.anchor.0 - q.at.0, fit.right.anchor.1 - q.at.1];
        let geometry = quick_rig::WeaponOrientation::new(
            origin,
            [fit.right.anchor.0, fit.right.anchor.1],
            [fit.right.anchor.0 - 4, fit.right.anchor.1 - 2],
        );
        let (expected, at) = geometry.stamp(&q.inks);
        assert_eq!(p.inks, expected);
        assert_eq!(p.at, (at[0], at[1]));
        let (rect, angle) = q.placed();
        let centre = geometry.point([
            rect[0] + rect[2] / 2.0 - q.at.0 as f32 - 0.5,
            rect[1] + rect[3] / 2.0 - q.at.1 as f32 - 0.5,
        ]);
        let (placed, turned) = p.placed();
        assert!((turned - angle - geometry.angle).abs() < 1e-5);
        assert!((placed[0] + placed[2] / 2.0 - centre[0] - 0.5).abs() < 1e-5);
        assert!((placed[1] + placed[3] / 2.0 - centre[1] - 0.5).abs() < 1e-5);
    }
}

// ─── the four seat knights ─────────────────────────────────────────────────

#[test]
fn every_seat_design_is_its_own_full_rig_on_the_old_knights_geometry() {
    let old = quick_rig::knight().expect("the old knight stays as the fallback");
    for design in quick_rig::Design::ALL {
        let rig = quick_rig::design(design).expect("a compiled design");
        // Its own asset parsed: a fallback would be the old knight itself.
        assert!(!std::ptr::eq(rig, old), "{design:?} fell back to the old knight");
        // Staging, rolls and pivots are measured on seat one's design for
        // every seat (`with_selected_rig`): the geometry must agree.
        assert_eq!(rig.canvas(), old.canvas(), "{design:?}");
        assert_eq!(rig.pivot(), old.pivot(), "{design:?}");
        for clip in Clip::ALL {
            assert!(rig.has(clip), "{design:?} lacks {clip:?}");
        }
    }
}

#[test]
fn each_seat_plays_its_own_knight_in_its_own_colour() {
    use quick_rig::Design;
    assert_eq!(
        [1, 2, 3, 4].map(Design::of_seat),
        [
            Design::GreatHelm,
            Design::SugarloafCrusader,
            Design::PlumedBucket,
            Design::KettleHat
        ]
    );
    // Ivory cloth would hide the crusader's cross in its white.
    assert_ne!(Design::of_seat(6), Design::SugarloafCrusader);
    let mut h = knight();
    h.owned.shield = false;
    let shape = |im: &Img| pixels(im).iter().map(Option::is_some).collect::<Vec<_>>();
    let figures: Vec<Img> = (1..=4)
        .map(|id| knights::figure_at(&h, id, false, Default::default(), false, 0))
        .collect();
    for a in 0..4 {
        for b in a + 1..4 {
            assert_ne!(
                shape(&figures[a]),
                shape(&figures[b]),
                "seats {} and {} look alike",
                a + 1,
                b + 1
            );
        }
    }
    // Every knight wears their seat's cloth where it reads at 1x.
    for (seat, cloth) in [(1, '7'), (2, '1'), (3, 'A'), (4, '4')] {
        let ink = super::super::ink::ink(cloth);
        let n = figures[seat - 1].pixels().filter(|&c| Some(c) == ink).count();
        assert!(n >= 8, "seat {seat} shows only {n} pixels of its cloth");
    }
}

#[test]
fn seat_knights_cut_with_the_crescent_cast_in_the_rite_and_roll_a_frame_a_tick() {
    for design in quick_rig::Design::ALL {
        let rig = quick_rig::design(design).unwrap();
        // The sword's 8-tick swing shows every slash frame, in order, from
        // the top of the cut on its first tick (the crescent is already
        // mid-sweep then) to the contact as the body's own blade reappears.
        let shown: Vec<usize> = (0..8)
            .map(|e| rig.action(Clip::Slash, 8 - e, 8, false).index)
            .collect();
        assert_eq!(shown, [0, 1, 2, 3, 3, 4, 4, 5], "{design:?}");
        // The cut comes down as the game's crescent does: raised, then low.
        let tip = |i: usize| {
            let s = (0..rig.duration(Clip::Slash))
                .map(|t| rig.sample(Clip::Slash, t, Playback::Once, false))
                .find(|s| s.index == i)
                .unwrap();
            let a = s.anchors();
            (a.tip[0] - a.grip[0], a.tip[1] - a.grip[1])
        };
        assert!(tip(0).1 < 0, "{design:?} starts with the blade raised");
        for contact in [2, 3] {
            let (dx, dy) = tip(contact);
            assert!(dx > 0 && dy > 0, "{design:?} lands the cut down and forward");
        }
        // A rite shows its caster for CAST_SHOWN ticks from the event: the
        // whole gesture, gather to the held release, lands inside it.
        let cast: Vec<usize> = (0..u64::from(crate::rules::minions::CAST_SHOWN))
            .map(|age| rig.sample(Clip::Cast, age, Playback::Once, false).index)
            .collect();
        assert_eq!(cast.first(), Some(&0), "{design:?}");
        assert_eq!(cast.last(), Some(&4), "{design:?} holds the release");
        assert!(cast.contains(&3), "{design:?} shows the release");
        // The 10-tick roll shows each of its frames for one tick.
        assert_eq!(rig.duration(Clip::Roll), 40, "{design:?}");
        let rolled: Vec<usize> = (0..10)
            .map(|e| rig.action(Clip::Roll, 10 - e, 10, false).index)
            .collect();
        assert_eq!(rolled, (0..10).collect::<Vec<_>>(), "{design:?}");
    }
}
