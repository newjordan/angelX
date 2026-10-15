use super::*;
use super::testing::*;
#[test]
fn v2_is_deterministic_rgb_and_small_frames_use_v1() {
    for (_, mut r) in fixtures() {
        let a = frame(&r, 768, 448, Some(1));
        assert_eq!(a, frame(&r, 768, 448, Some(1)));
        assert_eq!((a.w, a.h), (768, 448));
        assert!(
            a.pixels()
                .any(|c| super::super::super::ink::palette_index(c).is_none())
        );
        for (w, h) in [(0, 0), (53, 30), (135, 78), (300, 180)] {
            assert_eq!(
                frame(&r, w, h, Some(1)),
                super::super::frame_v1(&r, w, h, Some(1))
            );
        }
        r.tick += 3;
        assert_ne!(a, frame(&r, 768, 448, Some(1)));
    }
}
#[test]
fn v2_preserves_every_gameplay_overlay_pixel() {
    let r = fight();
    assert!(!r.projectiles.is_empty());
    let (_, over) = super::super::layers_with(&r, Some(1), true, true);
    let image = frame(&r, 768, 448, Some(1));
    for y in 0..over.h {
        for x in 0..over.w {
            if let Some(c) = over.get(x, y).filter(|c| {
                *c != super::super::super::ink::ink('K').unwrap()
                    && *c != super::super::super::ink::ink('k').unwrap()
            }) {
                for dy in 0..2 {
                    for dx in 0..2 {
                        assert_eq!(
                            image.get(x * 2 + dx, y * 2 + dy),
                            Some(c),
                            "protected cue at {x},{y}"
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn v2_cache_invalidates_for_doors_and_camera() {
    let r = fight();
    let original = scenery(&r, (0, 0));
    let mut animated = r.clone();
    animated.tick += 11;
    assert_eq!(key(&r, (0, 0)), key(&animated, (0, 0)));
    animated.enemies.clear();
    animated.phase = Phase::Exploring;
    assert_ne!(key(&r, (0, 0)), key(&animated, (0, 0)));
    assert_ne!(key(&r, (0, 0)), key(&r, (8, 0)));
    assert_eq!(original.canvas.px, scenes::bake(&r, (0, 0)).canvas.px);
}
#[test]
fn coloured_shapes_have_half_pixel_detail_and_no_black_rim() {
    let r = fight();
    let mut cv = Canvas::new(64, 64, material('q'));
    figures::knight(&mut cv, 16.0, 27.0, &r.players[&1], 1, 1, false);
    assert!(!cv.px.contains(&BLACK));
    assert!(
        cv.px
            .chunks_exact(64)
            .any(|row| row.chunks_exact(2).any(|p| p[0] != p[1]))
    );
}
#[test]
fn moving_map_camera_crops_cached_world_samples_without_seams() {
    let r = kingdom();
    let origin = super::super::camera(&r, Some(1));
    let anchor = (origin.0.div_euclid(64) * 64, origin.1.div_euclid(64) * 64);
    let scene = scenery(&r, anchor);
    let a = scene.canvas.view(origin);
    let b = scene.canvas.view((origin.0 + 1, origin.1 + 1));
    for y in 0..446 {
        assert_eq!(
            &a.px[(y + 2) * 768 + 2..(y + 2) * 768 + 768],
            &b.px[y * 768..y * 768 + 766]
        );
    }
}
#[test]
fn live_bake_finishes_with_the_exact_snapshot_pixels() {
    let mut r = home(RoomKind::CollegeHall, 83);
    r.raid_id += 999_999;
    let start = std::time::Instant::now();
    let scene = loop {
        if let Some(scene) = scenery_live(&r, (0, 0)) {
            break scene;
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(3),
            "bounded worker completes the bake"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert_eq!(scene.canvas.px, scenes::bake(&r, (0, 0)).canvas.px);
    assert!(pending().lock().unwrap_or_else(|e| e.into_inner()).len() <= 4);
}
#[test]
fn art_dispatch_and_special_body_fallbacks_keep_their_contract() {
    let mut r = fight();
    let expected = if enabled() {
        frame(&r, 768, 448, Some(1))
    } else {
        super::super::frame_v1(&r, 768, 448, Some(1))
    };
    assert_eq!(super::super::frame_for(&r, 768, 448, Some(1)), expected);
    assert!(knight_supported(&r, &r.players[&1], 1));
    // A stage of dress or a shield raised away is drawn here too, as the
    // arena's own figure brought up to this canvas, the same size as the
    // kit's studies; only a body in a state of its own goes to v1.
    r.players.get_mut(&1).unwrap().gear = 1;
    assert!(knight_supported(&r, &r.players[&1], 1));
    r.players.get_mut(&1).unwrap().gear = 0;
    r.players.get_mut(&1).unwrap().shielding = true;
    r.players.get_mut(&1).unwrap().aim_y = -1.0;
    r.players.get_mut(&1).unwrap().aim_x = 0.0;
    assert!(knight_supported(&r, &r.players[&1], 1));
    r.players.get_mut(&1).unwrap().stone = true;
    assert!(!knight_supported(&r, &r.players[&1], 1));
    r.players.get_mut(&1).unwrap().stone = false;
    r.dungeon.pack = Pack::Cavern;
    assert_eq!(
        frame(&r, 768, 448, Some(1)),
        super::super::frame_v1(&r, 768, 448, Some(1))
    );
}
#[test]
fn accepted_plate_scenes_use_v2_and_are_exact_after_cache_eviction() {
    for (name, r) in matching::fixtures() {
        assert!(supported(&r), "{name} must not quietly fall back");
        let a = frame(&r, 768, 448, Some(1));
        assert_ne!(
            a,
            super::super::frame_v1(&r, 768, 448, Some(1)),
            "{name} has an upgraded runtime frame"
        );
        assert_eq!(a, frame(&r, 768, 448, Some(1)), "{name} exact snapshot");
        assert_eq!(
            frame(&r, 135, 78, Some(1)),
            super::super::frame_v1(&r, 135, 78, Some(1))
        );
        // Force a distinct static region; rebuilding must not change the output.
        for i in 1..=5 {
            let mut other = r.clone();
            other.raid_id += 777 + i;
            let _ = frame(&other, 768, 448, Some(1));
        }
        assert_eq!(
            a,
            frame(&r, 768, 448, Some(1)),
            "{name} scheduling independent"
        );
    }
}
#[test]
fn upgraded_panels_stay_local_to_their_seat() {
    for (name, r) in matching::fixtures()
        .into_iter()
        .filter(|(n, _)| matches!(*n, "privy" | "vote"))
    {
        let a = frame(&r, 768, 448, Some(1));
        let mut closed = r.clone();
        closed.players.get_mut(&1).unwrap().pit.result = None;
        if let Some(s) = closed.playtest.seats.get_mut(&1) {
            s.open = false;
        }
        assert_ne!(a, frame(&closed, 768, 448, Some(1)), "{name} panel shown");
        assert_eq!(
            frame(&r, 768, 448, Some(2)),
            frame(&closed, 768, 448, Some(2)),
            "{name} independent seat"
        );
    }
}
#[test]
fn course_changes_invalidate_the_static_region_without_tick_churn() {
    let (_, r) = matching::fixtures()
        .into_iter()
        .find(|(n, _)| *n == "race")
        .unwrap();
    let mut changed = r.clone();
    changed.tick += 20;
    assert_eq!(key(&r, (0, 0)), key(&changed, (0, 0)));
    changed.chariot.as_mut().unwrap().track += 1;
    assert_ne!(key(&r, (0, 0)), key(&changed, (0, 0)));
}

#[test]
fn rail_scanlines_match_full_coverage_even_outside_the_cached_region() {
    for (a, b, width) in [
        ((-12.0, -8.0), (85.0, 60.0), 4.5),
        ((70.0, 20.0), (65.0, 80.0), 1.7),
        ((8.0, 40.0), (54.0, 40.0), 0.7),
        ((62.0, 4.0), (3.0, 43.0), 3.0),
        ((1.0, 1.0), (1.0, 1.0), 2.0),
    ] {
        let mut actual = Canvas::new(128, 96, material('q'));
        actual.origin = (2.0, -1.0);
        let mut expected = actual.clone();
        let c = material('T');
        actual.line(a, b, width, c);
        let (vx, vy) = (b.0 - a.0, b.1 - a.1);
        let d = (vx * vx + vy * vy).max(0.001);
        for y in 0..expected.h {
            for x in 0..expected.w {
                let px = (x as f32 + 0.5) * 0.5 + expected.origin.0 - a.0;
                let py = (y as f32 + 0.5) * 0.5 + expected.origin.1 - a.1;
                let t = ((px * vx + py * vy) / d).clamp(0.0, 1.0);
                let coverage =
                    ((width * 0.5 - (px - vx * t).hypot(py - vy * t)) * 2.0 + 0.5).clamp(0.0, 1.0);
                expected.blend(x, y, c, coverage);
            }
        }
        assert_eq!(actual.px, expected.px, "clipped rail {a:?} {b:?}");
    }
}

#[test]
fn helios_flame_tips_extinguish_at_rest_while_the_material_body_remains() {
    let mut calm = Canvas::new(180, 110, material('q'));
    let mut gallop = calm.clone();
    figures::horse(&mut calm, 45.0, 50.0, true, 160, true, false, 0);
    figures::horse(&mut gallop, 45.0, 50.0, true, 160, true, true, 0);
    let hot = |cv: &Canvas| {
        cv.px
            .iter()
            .chain(&cv.fx)
            .filter(|c| c[0] > 195 && c[1] > 145 && c[2] < 190)
            .count()
    };
    assert!(
        hot(&gallop) > hot(&calm) + 35,
        "flame pixels belong to the gallop"
    );
    let body = kit::get("helios-calm");
    let full = kit::get("helios");
    assert!(
        (35..110)
            .any(|x| (28..60).any(|y| body.get(x, y).is_some_and(|c| Some(c) == full.get(x, y)))),
        "draft body survives extinguishing"
    );
}

#[test]
fn mini_walk_uses_the_kingdom_kit_and_keeps_published_positions() {
    let mut s = super::super::super::Scene::resting();
    let geo = super::super::super::kingdom::geography(&[]);
    let at = geo
        .sites
        .iter()
        .find(|s| s.room == RoomKind::CollegeGrounds)
        .unwrap()
        .stand;
    s.knight.x = at.0 as f32 * 16.0;
    s.knight.y = at.1 as f32 * 16.0;
    let view = super::super::super::View::around(s.knight.x, s.knight.y, 384, 224);
    let a = walk_layers(&s, view);
    assert_eq!(a, walk_layers(&s, view));
    s.knight.x += 16.0;
    let b = walk_layers(&s, view);
    assert_ne!(a.0, b.0);
    assert_ne!(a.1, b.1, "published lantern moves with the walker");
}

#[test]
fn mini_building_animation_cannot_overwrite_the_shared_facade() {
    let mut scene = super::super::super::Scene::resting();
    let mut props = Vec::new();
    let mut lights = Vec::new();
    super::super::super::college::stage(0, &mut props, &mut lights);
    let tower = props.iter().max_by_key(|p| p.img.h).unwrap();
    let view = super::super::super::View::around(
        (tower.x + tower.img.w / 2) as f32,
        (tower.base - tower.img.h / 2) as f32,
        384,
        224,
    );
    scene.knight.x = -1000.0;
    scene.knight.y = -1000.0;
    scene.tick = 0;
    let a = walk_layers(&scene, view).0;
    scene.tick = 80;
    let b = walk_layers(&scene, view).0;
    for y in 20..tower.img.h - 16 {
        for x in 8..tower.img.w - 8 {
            let (px, py) = (
                (tower.x + x - view.x) * 2,
                (tower.base - tower.img.h + y - view.y) * 2,
            );
            assert_eq!(
                a.get(px, py),
                b.get(px, py),
                "v2 tower material stays canonical at {x},{y}"
            );
        }
    }
}

#[test]
fn black_space_and_braille_are_shared_by_every_reviewed_surface() {
    use crate::render::art;
    for (name, r) in matching::fixtures() {
        let (base, fx) = terminal_layers(&r, Some(1)).unwrap();
        assert!(
            base.pixels().any(|c| c == BLACK),
            "{name} keeps black paper"
        );
        assert!(fx.pixels().any(|c| c != BLACK), "{name} carries atmosphere");
        assert!(
            art::braille(&fx, 96, 28)
                .cells
                .iter()
                .any(|c| crate::render::braille::braille_bits(c.glyph) != 0),
            "{name} has real dots"
        );
    }
    assert_eq!(
        art::irradiance(&[Point::new(10.0, 10.0, 3.0, '6', 1.0)], 14.0, 10.0),
        [0.0; 3]
    );
}

#[test]
fn creature_finishing_keeps_sample_bounds_and_alpha() {
    for name in ["ghost","slime","skeleton","imp","ripper","engine-boiler"] {
        let source=kit::get(name);
        let dressed=figures::foe_sprite(name);
        assert_eq!((source.w,source.h),(dressed.w,dressed.h));
        assert_ne!(source,dressed,"material pass for {name}");
        for y in 0..source.h { for x in 0..source.w {
            assert_eq!(source.get(x,y).is_some(),dressed.get(x,y).is_some(),"{name} alpha {x},{y}");
        }}
    }
}

#[test]
#[ignore]
fn write_v2_foe_samples() {
    let Some(dir)=std::env::var_os("ANGEL_FOE_REVIEW") else{return;};
    for name in ["ghost","slime","skeleton","imp","ripper","engine-boiler"] {
        let im=figures::foe_sprite(name);
        image::RgbaImage::from_raw(im.w as u32,im.h as u32,im.rgba_scaled(1)).unwrap()
            .save(std::path::PathBuf::from(&dir).join(format!("{name}.png"))).unwrap();
    }
}

#[test]
fn approved_creature_sample_pixels_stay_exact() {
    let expected: std::collections::BTreeMap<String,u64> =
        serde_json::from_str(include_str!("fixtures/foe-art-v2.json")).unwrap();
    assert_eq!(expected.len(),6);
    for (name,hash) in expected {
        assert_eq!(figures::foe_sprite(&name).content_hash(),hash,"approved {name} sample");
    }
}
