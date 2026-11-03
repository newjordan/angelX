//! Export the actual renderer, never a separately redrawn approximation.
use super::*;

fn rows(im: &Img) -> Vec<String> {
    let palette: Vec<_> = (33u8..=126)
        .filter_map(|c| super::super::ink::ink(c as char).map(|rgb| (c as char, rgb)))
        .collect();
    (0..im.h)
        .map(|y| {
            (0..im.w)
                .map(|x| {
                    im.get(x, y).map_or('.', |rgb| {
                        palette
                            .iter()
                            .find(|(_, c)| *c == rgb)
                            .map(|(c, _)| *c)
                            .expect("native palette")
                    })
                })
                .collect()
        })
        .collect()
}

#[test]
fn export_worn_native() {
    if std::env::var_os("WORN_NATIVE_LEGACY").is_some() {
        return quick_rig::with_legacy_knight(export_frames);
    }
    export_frames();
}

fn export_frames() {
    let Ok(out) = std::env::var("WORN_NATIVE_OUT") else {
        return;
    };
    if let Ok(path) = std::env::var("QUICK_RIG_SOURCE_OUT") {
        use super::super::kit::hero::{self, Dress, Held, View};
        let im = hero::draw(Dress {
            view: View::Across, helm: true, harness: true, shield: false,
            step: 0, held: Held::Nothing, strike: false,
        });
        let mut png = image::RgbaImage::new(im.w as u32, im.h as u32);
        for y in 0..im.h {
            for x in 0..im.w {
                if let Some([r, g, b]) = im.get(x, y) {
                    png.put_pixel(x as u32, y as u32, image::Rgba([r, g, b, 255]));
                }
            }
        }
        png.save(path).unwrap();
    }
    let mut run = Run::new(21, 1, Some("Fit"));
    let original = run.players[&1].clone();
    let mut frames = Vec::new();
    let items: Vec<serde_json::Value> = std::env::var("WORN_ITEMS").ok().map(|p| {
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(p).unwrap()).unwrap()["items"].as_array().unwrap().clone()
    }).unwrap_or_default();
    let mut dressed = Vec::new();
    let kits = std::iter::once(("realm", None)).chain(
        crate::rules::knights::COMPANY
            .iter()
            .map(|c| (c.id, Some(c.id))),
    );
    for (kit, id) in kits {
        run.players.insert(1, original.clone());
        if let Some(id) = id {
            assert!(run.take_up(1, id), "{id}");
        }
        for (view, x, y, mirror) in [
            ("front", 0.0, 1.0, false),
            ("right", 1.0, 0.0, false),
            ("back", 0.0, -1.0, false),
            ("left", 1.0, 0.0, true),
        ] {
            for (pose, stride, moving) in [
                ("stand", 0.0, false),
                ("walk0", 0.0, true),
                ("walk1", 1.0, true),
                ("walk2", 2.0, true),
                ("walk3", 3.0, true),
                ("strike", 0.0, false),
            ] {
                let mut hero = run.players[&1].clone();
                hero.aim_x = x;
                hero.aim_y = y;
                hero.stride = if moving { stride * 0.85 + 0.01 } else { 0.0 };
                if pose == "strike" {
                    hero.fire_cooldown = 4;
                }
                let tick = if pose.starts_with("walk") {
                    (stride as u64) * 6
                } else {
                    0
                };
                let im = knights::figure(&hero, 1);
                let fit = knights::body_of(&hero, &im);
                let im = if mirror { im.flip_h() } else { im };
                let file = format!("{}-{}-{}.png", kit, view, pose);
                let dir = std::path::Path::new(&out).parent().unwrap();
                std::fs::create_dir_all(dir).unwrap();
                image::RgbaImage::from_raw(im.w as u32, im.h as u32, im.rgba_scaled(1))
                    .unwrap()
                    .save(dir.join(&file))
                    .unwrap();
                frames.push(serde_json::json!({"kit":kit,"view":view,"pose":pose,"tick":tick,"w":im.w,"h":im.h,"rows":rows(&im),"fit":fit,"mirror":mirror,"png":file}));
                for item in &items {
                    let sprite: crate::rules::looks::Sprite =
                        serde_json::from_value(item["sprite"].clone()).unwrap();
                    let unmirrored = if mirror { im.flip_h() } else { im.clone() };
                    let (composite, paints) =
                        knights::dressed_as(&unmirrored, fit, &[&sprite], tick, 0);
                    let composite = if mirror {
                        composite.flip_h()
                    } else {
                        composite
                    };
                    let paints: Vec<_> = paints
                        .into_iter()
                        .map(|p| if mirror { p.mirrored(composite.w) } else { p })
                        .collect();
                    let dressed_file = format!(
                        "{}-{}-{}-{}.png",
                        kit,
                        view,
                        pose,
                        item["name"].as_str().unwrap()
                    );
                    image::RgbaImage::from_raw(
                        composite.w as u32,
                        composite.h as u32,
                        composite.rgba_scaled(1),
                    )
                    .unwrap()
                    .save(dir.join(&dressed_file))
                    .unwrap();
                    let rects: Vec<_> = paints.iter().map(|p| serde_json::json!({"rect":p.rect,"at":p.at,"under":p.under,"frame":p.frame,"flip":p.flip,"subject":p.subject})).collect();
                    dressed.push(serde_json::json!({"kit":kit,"view":view,"pose":pose,"item":item["name"],"w":composite.w,"h":composite.h,"rows":rows(&composite),"fit":fit,"paintings":rects,"png":dressed_file}));
                }
            }
        }
    }
    std::fs::write(
        &out,
        serde_json::to_vec_pretty(&serde_json::json!({"frames":frames})).unwrap(),
    )
    .unwrap();
    std::fs::write(
        std::path::Path::new(&out).with_file_name("dressed.json"),
        serde_json::to_vec_pretty(&dressed).unwrap(),
    )
    .unwrap();
}

#[test]
fn worn_profiles_cover_every_kit_slot_and_pose() {
    let mut run = Run::new(21, 1, Some("Fit"));
    let original = run.players[&1].clone();
    for kit in
        std::iter::once(None).chain(crate::rules::knights::COMPANY.iter().map(|c| Some(c.id)))
    {
        run.players.insert(1, original.clone());
        if let Some(kit) = kit {
            assert!(run.take_up(1, kit));
        }
        for (x, y) in [(0.0, 1.0), (1.0, 0.0), (0.0, -1.0)] {
            for stride in [0.0, 0.01, 0.86, 1.71, 2.56] {
                let mut hero = run.players[&1].clone();
                hero.aim_x = x;
                hero.aim_y = y;
                hero.stride = stride;
                let im = knights::figure(&hero, 1);
                let fit = knights::body_of(&hero, &im);
                assert_eq!(fit.back.under, y >= 0.0, "cape layer {kit:?} aim=({x},{y})");
                for slot in [
                    "head",
                    "face",
                    "body",
                    "back",
                    "shoulders",
                    "left",
                    "right",
                    "feet",
                    "aura",
                ] {
                    let part = fit.part(slot).unwrap();
                    assert!(part.size.0 > 0 && part.size.1 > 0, "{kit:?} {slot}");
                    let source = crate::rules::looks::Sprite {
                        frames: vec![vec!["H".repeat(24); 24]],
                        at: Some(slot.into()),
                        paint: Some("ab".repeat(32)),
                        ..Default::default()
                    };
                    let fitted = wear::fitted(&source, part);
                    let [_, _, w, h] = crate::rules::looks::ink_bounds(&fitted.frames).unwrap();
                    assert_eq!((w, h), part.fitted_size((24, 24)), "{kit:?} {slot}");
                    let (origin_x, origin_y) = part.origin((w, h));
                    assert_eq!(origin_x + w / 2, part.anchor.0);
                    assert_eq!(
                        origin_y + if part.bottom { h - 1 } else { h / 2 },
                        part.anchor.1
                    );
                    let (_, p) = knights::dressed_as(&im, fit, &[&source], 0, 0);
                    assert_eq!(p[0].under, part.under);
                    assert!(
                        p[0].at.1 + p[0].inks.h <= im.h + (-(origin_y)).max(0),
                        "floor {kit:?} {slot}"
                    );
                }
            }
        }
    }
}

#[test]
fn worn_skeleton_skull_tracks_each_dance_drawing() {
    use crate::rules::hireling::AllyKind;
    for (phase, (dx, dy)) in [
        (-2, 0),
        (-1, 1),
        (-2, 0),
        (1, -1),
        (0, 0),
        (3, 1),
        (0, 0),
        (1, -1),
    ]
    .into_iter()
    .enumerate()
    {
        let fit = minions::body_at(AllyKind::Skeleton, phase as u64 * 6).unwrap();
        assert_eq!(fit.head.anchor, (12 + dx, 4 + dy));
        assert_eq!(fit.face.anchor, (12 + dx, 5 + dy));
    }
}

#[test]
fn worn_native_slot_calibration_and_painted_aspect() {
    use crate::rules::looks::{Sprite, profile};
    let fit = profile("realm");
    assert_eq!(fit.head.anchor, (8, 4));
    assert!(
        !fit.head.bottom,
        "helmet wraps skull instead of sitting on brow"
    );
    assert_eq!(fit.body.size, (12, 6));
    assert_eq!(fit.feet.size, (12, 3));
    assert!(profile("mage_right").body.size.0 < profile("gravecaller").body.size.0);
    assert!(!profile("realm_back").left.under);
    assert!(
        !profile("realm_back").back.under,
        "cape is visible on the rear of the body"
    );
    assert!(fit.right.bottom, "blade hilt meets hand, not blade centre");
    let source = Sprite {
        frames: vec![vec![".HH.".into(); 8]],
        at: Some("body".into()),
        paint: Some("ab".repeat(32)),
        ..Default::default()
    };
    let body = Img::new(16, 16);
    let (_, paints) = knights::dressed_as(&body, fit, &[&source], 0, 0);
    // Original square is 8x8 with a 2x8 subject: each source axis must
    // scale independently. A new square around fitted ink would shrink it.
    assert_eq!(paints[0].inks.w, 12);
    assert_eq!(paints[0].inks.h, 6);
    assert!(paints[0].subject);
    assert_eq!(paints[0].rect[2], 12.0);
    assert_eq!(paints[0].rect[3], 6.0);
}

#[test]
fn worn_subject_crop_uses_png_anatomy_not_fallback_aspect() {
    let (w, h) = (8u32, 6u32);
    let mut rgba = vec![0; w as usize * h as usize * 4];
    // Two frames, a tall local subject in different positions. Common
    // crop must remain fixed; normalized output reaches the part edges.
    for (x, y) in [(1, 1), (2, 4), (5, 2), (6, 4)] {
        let i = ((y * w + x) * 4) as usize;
        rgba[i..i + 4].copy_from_slice(&[200, 10, 20, 255]);
    }
    let (side, frames) = paintings::cut_subject(&rgba, (w, h), 2, 8).unwrap();
    assert_eq!(side, 8);
    assert_eq!(frames.len(), 2);
    assert_eq!(&frames[0][..4], &[200, 10, 20, 255]);
    assert_eq!(
        &frames[0][(7 * 8 + 7) * 4..(7 * 8 + 8) * 4],
        &[200, 10, 20, 255]
    );
    assert_eq!(&frames[1][..4], &[0, 0, 0, 0], "frame stays on shared crop");
    assert!(paintings::cut_subject(&rgba, (w, h), 3, 8).is_none());
}

#[test]
fn worn_every_minion_slot_has_positive_native_geometry() {
    use crate::rules::hireling::AllyKind::*;
    for kind in [
        Ghoul, Warden, Totem, Zombie, Skeleton, FleshGolem, BoneGolem, GraveGolem, Colossus,
    ] {
        for tick in (0..48).step_by(6) {
            let fit = minions::body_at(kind, tick).unwrap();
            for slot in [
                "head", "face", "body", "back", "left", "right", "feet", "aura",
            ] {
                let part = fit.part(slot).unwrap();
                assert!(part.size.0 > 0 && part.size.1 > 0);
            }
            if kind.golem() {
                let expected = match kind {
                    FleshGolem => (17, 20),
                    BoneGolem => (22, 25),
                    GraveGolem => (27, 30),
                    _ => (34, 38),
                };
                assert_eq!((fit.bounds[2], fit.bounds[3]), expected);
            }
        }
    }
}

#[test]
fn worn_motion_uses_the_same_pivot_for_png_mask_and_padding() {
    use crate::rules::looks::{Move, Sprite, profile};
    // Frame zero only occupies one corner. The union canvas and the
    // original square's centre differ by half a pixel on the other axis.
    for slot in ["body", "head", "right"] {
        let sprite = Sprite {
            frames: vec![
                vec!["H..".into(), "...".into()],
                vec!["...".into(), "..H".into()],
            ],
            at: Some(slot.into()),
            paint: Some("ab".repeat(32)),
            moves: vec![Move::Bob, Move::Wobble, Move::Stretch],
            ..Default::default()
        };
        let fit = profile("realm");
        let source = wear::fitted(&sprite, fit.part(slot).unwrap());
        for tick in 0..48 {
            let im = knights::frame_img(source.frame(tick));
            let (_, paints) = knights::dressed_as(&Img::new(16, 16), fit, &[&sprite], tick, 37);
            let p = &paints[0];
            let local_pivot = if p.subject {
                (im.w as f32 / 2.0, im.h as f32 / 2.0)
            } else {
                let [x, y, w, h] = crate::rules::looks::ink_bounds(&sprite.frames).unwrap();
                let [sx, sy, sw, sh] = paintings::square(&sprite, false, 1.0).unwrap();
                (
                    (sx + sw / 2.0 - x as f32) * im.w as f32 / w as f32,
                    (sy + sh / 2.0 - y as f32) * im.h as f32 / h as f32,
                )
            };
            let (expected, _) = p.pose.apply(&im, local_pivot);
            assert_eq!(
                (p.inks.w, p.inks.h),
                (expected.w, expected.h),
                "{slot} tick={tick}"
            );
            assert_eq!(rows(&p.inks), rows(&expected), "{slot} tick={tick}");
            let (canvas, _) = knights::dressed_as(&Img::new(16, 16), fit, &[&sprite], tick, 37);
            assert!(p.at.0 >= 0 && p.at.1 >= 0);
            assert!(p.at.0 + p.inks.w <= canvas.w && p.at.1 + p.inks.h <= canvas.h);
        }
    }
}
