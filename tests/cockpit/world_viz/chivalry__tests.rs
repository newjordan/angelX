use super::*;
use crate::ui::viz::lifecycle_viz::MotionMode;
use crate::{
    drive::chivalry::{Choice, Mount, Phase},
    stage::world_viz::world3d::chivalry as mesh,
};
fn dots(image: &ColoredBrailleImage) -> u32 {
    image
        .cells
        .iter()
        .map(|c| (c.glyph as u32).saturating_sub(0x2800).count_ones())
        .sum()
}
#[test]
fn chivalry_live_normal_frame_keys_cover_state_place_entry_station_size_and_motion() {
    let mut world = World::new(2024);
    assert_eq!(world.visit_overworld("stables"), Some("STABLES"));
    assert!(!world.live_adventure_view());
    let draw = |w: &World, m| {
        w.scryglass_frame_with_motion(72, 26, true, 0.0, 0.0, 1.05, m)
            .unwrap()
    };
    let outside = draw(&world, MotionMode::Off);
    assert!(dots(&outside) > 80);
    assert!(Arc::ptr_eq(&outside, &draw(&world, MotionMode::Off)));
    assert!(world.enter_interior());
    let inside = draw(&world, MotionMode::Off);
    assert_ne!(outside, inside);
    world.chivalry.select(Mount::Cinder).unwrap();
    let selected = draw(&world, MotionMode::Off);
    assert_ne!(inside, selected);
    world.chivalry.tend().unwrap();
    let tended = draw(&world, MotionMode::Off);
    assert_ne!(selected, tended);
    world.chivalry_step(1);
    assert_ne!(tended, draw(&world, MotionMode::Off));
    let reduced = draw(&world, MotionMode::Reduced);
    assert!(!Arc::ptr_eq(&reduced, &draw(&world, MotionMode::Off)));
    world.tick();
    world.tick();
    let off = draw(&world, MotionMode::Off);
    for _ in 0..20 {
        world.tick();
    }
    assert!(
        Arc::ptr_eq(&off, &draw(&world, MotionMode::Off)),
        "Off is not keyed by idle clock"
    );
    assert!(
        world
            .scryglass_frame_with_motion(0, 12, false, 0.0, 0.0, 1.0, MotionMode::Full)
            .is_none()
    );
    let tiny = world
        .scryglass_frame_with_motion(1, 1, false, 0.0, 0.0, 1.0, MotionMode::Off)
        .unwrap();
    assert_eq!((tiny.width, tiny.height), (1, 1));
    let oversized = world
        .scryglass_frame_with_motion(1000, 1000, false, 0.0, 0.0, 1.0, MotionMode::Off)
        .unwrap();
    assert_eq!((oversized.width, oversized.height), (256, 80));
    world.visit_overworld("tournament").unwrap();
    world.enter_interior();
    let ready = draw(&world, MotionMode::Off);
    world.chivalry.start().unwrap();
    let underway = draw(&world, MotionMode::Off);
    assert_ne!(ready, underway);
    world.chivalry.choose(1, Choice::Guard).unwrap();
    assert_ne!(underway, draw(&world, MotionMode::Off));
    world.chivalry.choose(2, Choice::Aim).unwrap();
    world.chivalry.choose(3, Choice::Charge).unwrap();
    assert_eq!(world.chivalry.tournament.phase, Phase::Finished);
    assert_ne!(underway, draw(&world, MotionMode::Off));
    world.leave_interior();
    assert!(!world.inside_interior());
    world.follow_overworld();
    assert!(world.chivalry_visit.is_none());
    assert!(world.chivalry_cache.borrow().is_none());
    world.visit_overworld("stables").unwrap();
    world.select_landmark(super::super::Building::Keep);
    assert!(world.chivalry_visit.is_none());
}
#[test]
fn chivalry_geometry_is_bounded_deterministic_and_camera_stations_are_connected() {
    let state = Chivalry::default();
    for place in [Place::Stables, Place::Tournament] {
        for inside in [false, true] {
            let v = Visit {
                place,
                inside,
                station: 0,
            };
            let scene = mesh::scene(&state, v);
            assert!(scene.tris.len() < 3500, "{} tris", scene.tris.len());
            for tri in &scene.tris {
                assert!(tri.normal.length() > 0.99);
                for p in tri.v {
                    assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite());
                }
            }
            let a = mesh::frame(&state, v, 144, 104, [0.0, 0.0, 0.0, 1.05]);
            assert_eq!(a, mesh::frame(&state, v, 144, 104, [0.0, 0.0, 0.0, 1.05]));
            assert_eq!(mesh::frame(&state, v, 0, 0, [0.0; 4]).dimensions(), (0, 0));
            // The aisle and outer arena path stay clear of stalls, horses and rails.
            // Test the full connecting segments against every upright face at
            // walking-body height, not just camera endpoints.
            if inside {
                for index in 0..3 {
                    let a = mesh::station(place, index);
                    let b = mesh::station(place, index + 1);
                    for step in 0..=32 {
                        let t = step as f32 / 32.0;
                        let x = a.x + (b.x - a.x) * t;
                        let y = a.y + (b.y - a.y) * t;
                        for tri in &scene.tris {
                            let low = tri.v.iter().map(|v| v.z).fold(f32::INFINITY, f32::min);
                            let high = tri.v.iter().map(|v| v.z).fold(f32::NEG_INFINITY, f32::max);
                            if low > 1.7 || high < 0.15 {
                                continue;
                            }
                            let minx = tri.v.iter().map(|v| v.x).fold(f32::INFINITY, f32::min);
                            let maxx = tri.v.iter().map(|v| v.x).fold(f32::NEG_INFINITY, f32::max);
                            let miny = tri.v.iter().map(|v| v.y).fold(f32::INFINITY, f32::min);
                            let maxy = tri.v.iter().map(|v| v.y).fold(f32::NEG_INFINITY, f32::max);
                            // Horizontal floor faces are not obstacles.
                            if high - low < 0.05 {
                                continue;
                            }
                            assert!(
                                x < minx - 0.22
                                    || x > maxx + 0.22
                                    || y < miny - 0.22
                                    || y > maxy + 0.22,
                                "station path clips a prop at {x},{y}"
                            );
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn chivalry_real_renderer_review_captures() {
    // Not a fixture display mode: normal runtime frames and the very same
    // Dotmax mesh/raster/Bayer bridge are asserted with or without artifact I/O.
    let directory = std::env::var_os("ANGEL_CHIVALRY_REVIEW").map(std::path::PathBuf::from);
    if let Some(d) = &directory {
        std::fs::create_dir_all(d).unwrap();
    }
    let mut world = World::new(2024);
    for (label, place, inside) in [
        ("district-stables", Place::Stables, false),
        ("district-lists", Place::Tournament, false),
        ("stable", Place::Stables, true),
        ("tournament-ready", Place::Tournament, true),
        ("tournament-running", Place::Tournament, true),
        ("tournament-result", Place::Tournament, true),
    ] {
        if label == "stable" {
            world.chivalry.select(Mount::Cinder).unwrap();
            world.chivalry.tend().unwrap();
        }
        if label == "tournament-running" {
            world.chivalry.start().unwrap();
            world.chivalry.choose(1, Choice::Guard).unwrap();
        }
        if label == "tournament-result" {
            world.chivalry.choose(2, Choice::Aim).unwrap();
            world.chivalry.choose(3, Choice::Charge).unwrap();
        }
        world.open_chivalry(place, inside);
        for (w, h) in [(72, 26), (16, 6), (1, 1)] {
            let live = world
                .scryglass_frame_with_motion(w, h, false, 0.0, 0.0, 1.05, MotionMode::Off)
                .unwrap();
            assert_eq!(
                live,
                world
                    .scryglass_frame_with_motion(w, h, false, 0.0, 0.0, 1.05, MotionMode::Off)
                    .unwrap()
            );
            if let Some(d) = &directory {
                mesh::frame(
                    &world.chivalry,
                    world.chivalry_visit.unwrap(),
                    w * 2,
                    h * 4,
                    [0.0, 0.0, 0.0, 1.05],
                )
                .save(d.join(format!("normal-{label}-{w}x{h}-raw.png")))
                .unwrap();
                save_braille(
                    &live,
                    &d.join(format!("normal-{label}-{w}x{h}-terminal.png")),
                );
            }
        }
    }
    // Representative ordinary court before/after the *additive* village pass.
    let after =
        super::super::world3d::scene::scene_for(super::super::world3d::scene::SceneKey::COURT);
    let dressing = mesh::village_dressing();
    let mut before = (*after).clone();
    before
        .tris
        .retain(|t| !dressing.tris.iter().any(|d| t.mat == d.mat && t.v == d.v));
    assert!(before.tris.len() < after.tris.len());
    for (w, h) in [(144, 104), (32, 24)] {
        use super::super::world3d::{
            math::v3,
            raster::{View3, render_scene},
        };
        let view = View3 {
            pos: v3(-13.0, -22.0, 4.8),
            heading_rad: 0.75,
            pitch: -0.23,
            fov_rad: 1.05,
        };
        let a = render_scene(&before, &view, w, h);
        let b = render_scene(&after, &view, w, h);
        assert_ne!(a, b, "village additions must actually be visible");
        if let Some(d) = &directory {
            a.save(d.join(format!("normal-village-before-{w}x{h}.png")))
                .unwrap();
            b.save(d.join(format!("normal-village-after-{w}x{h}.png")))
                .unwrap();
        }
    }
}
fn save_braille(image: &ColoredBrailleImage, path: &std::path::Path) {
    let mut out = image::RgbaImage::from_pixel(
        image.width as u32 * 8,
        image.height as u32 * 16,
        image::Rgba([9, 12, 18, 255]),
    );
    for y in 0..image.height {
        for x in 0..image.width {
            let c = image.cells[y * image.width + x];
            let bits = (c.glyph as u32).saturating_sub(0x2800) as u8;
            for dy in 0..4 {
                for dx in 0..2 {
                    if bits & crate::ui::term::art::braille_dot_bit(dx, dy) != 0 {
                        for yy in 0..3 {
                            for xx in 0..3 {
                                out.put_pixel(
                                    x as u32 * 8 + dx as u32 * 4 + xx + 1,
                                    y as u32 * 16 + dy as u32 * 4 + yy + 1,
                                    image::Rgba([c.fg[0], c.fg[1], c.fg[2], 255]),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    out.save(path).unwrap();
}

#[test]
fn chivalry_composition_keeps_focal_profiles_props_rails_and_sky_in_budget() {
    use crate::stage::world_viz::world3d::{mesh::mat, raster};
    let state = Chivalry::default();
    for (name, place, inside) in [
        ("stable", Place::Stables, true),
        ("district-stables", Place::Stables, false),
        ("lists", Place::Tournament, true),
    ] {
        for (w, h) in [(144, 104), (64, 48), (236, 144), (92, 36)] {
            let (scene, mut camera) = mesh::composition(
                &state,
                Visit {
                    place,
                    inside,
                    station: 0,
                },
            );
            camera.fov_rad = mesh::lens(w, h, 1.05); // Same local lens as production.
            let visible = raster::visible_materials(&scene, &camera, w, h);
            if let Some(dir) = std::env::var_os("ANGEL_CHIVALRY_REVIEW") {
                let dir = std::path::PathBuf::from(dir);
                let pixels = raster::render_scene(&scene, &camera, w, h);
                pixels
                    .save(dir.join(format!("composition-{name}-{w}x{h}.png")))
                    .unwrap();
                std::fs::write(
                    dir.join(format!("composition-{name}-{w}x{h}.materials.json")),
                    serde_json::to_vec(&visible).unwrap(),
                )
                .unwrap();
            }
            let count = |mats: &[u8]| visible.iter().filter(|m| mats.contains(m)).count();
            let horse = count(&[mat::HORSE_BAY, mat::HORSE_CHESTNUT, mat::HORSE_GREY]);
            let sky = count(&[0]);
            let timber = count(&[mat::STABLE_TIMBER]);
            let cloth = count(&[mat::CLOTH_GOLD, mat::CLOTH_BLUE]);
            let hay = count(&[mat::HAY]);
            let tack = count(&[mat::TACK_LEATHER]);
            let roof = count(&[mat::STABLE_ROOF]);
            println!(
                "COMPOSITION {name} {w}x{h}: sky={sky}/{} horse={horse} rails/posts={timber} cloth={cloth} hay={hay} tack={tack} roof={roof}",
                w * h
            );
            assert!(horse >= w * h / 50, "{name}: profile is too small");
            assert!(timber >= w * h / 100, "{name}: rails/posts lost");
            assert!(cloth >= w * h / 200, "{name}: selection cloth lost");
            if name == "stable" {
                assert_eq!(sky, 0, "stable is indoors, not under a starfield");
                assert!(hay >= w * h / 200, "hay bale must be a subject");
                assert!(
                    visible
                        .iter()
                        .enumerate()
                        .filter(|(_, m)| **m == mat::HAY)
                        .all(|(i, _)| {
                            let (x, y) = (i % w, i / w);
                            x > 0 && x < w - 1 && y > 0 && y < h - 1
                        }),
                    "hay bale should not be clipped at the pane edge"
                );
                assert!(tack >= w * h / 200, "saddle/tack must be visible");
                // Nearest Bramble's profile has four separate lower-leg runs,
                // not two merged poles. Check actual visible fragments, so
                // hidden geometry cannot satisfy this silhouette contract.
                let four_legs = (h / 2..h)
                    .filter(|y| {
                        let mut runs = 0;
                        let mut previous = false;
                        for x in 0..w {
                            let coat = visible[y * w + x] == mat::HORSE_BAY;
                            if coat && !previous {
                                runs += 1;
                            }
                            previous = coat;
                        }
                        runs == 4
                    })
                    .count();
                assert!(
                    four_legs >= (h / 32).max(1),
                    "{w}x{h}: four legs must separate in the visible profile"
                );
                let profile: Vec<_> = visible
                    .iter()
                    .enumerate()
                    .filter(|(_, mat)| **mat == mat::HORSE_BAY)
                    .map(|(i, _)| (i % w, i / w))
                    .collect();
                assert!(
                    profile
                        .iter()
                        .all(|(x, y)| *x > 0 && *x < w - 1 && *y > 0 && *y < h - 1),
                    "nearest horse is not cropped"
                );
            } else {
                assert!(sky < w * h / 4, "{name}: sky consumes the composition");
            }
            if name == "district-stables" {
                // A short pane samples a pitched roof at fewer vertical dots;
                // retain at least 4% there (normal panes measure above 6%).
                assert!(roof > w * h / 25, "stable roof silhouette lost");
            }
            if name == "lists" {
                assert_eq!(roof, 0, "lists are open, not roofed");
            }
        }
    }
}

#[test]
fn chivalry_tournament_staging_uses_real_phase_and_result_without_animation() {
    let visit = Visit {
        place: Place::Tournament,
        inside: true,
        station: 0,
    };
    let mut state = Chivalry::default();
    let render = |state: &Chivalry| mesh::frame(state, visit, 144, 104, [0.0, 0.0, 0.0, 1.05]);
    let ready = render(&state);
    state.start().unwrap();
    let running = render(&state);
    let delta = |a: &image::RgbaImage, b: &image::RgbaImage| {
        a.pixels().zip(b.pixels()).filter(|(a, b)| a != b).count()
    };
    assert!(
        delta(&ready, &running) > 144 * 104 / 10,
        "lowered lances and advanced riders must change a subject, not a lamp"
    );
    let running_scene = mesh::scene(&state, visit);
    let mut outcomes = Vec::new();
    for choices in [
        [Choice::Guard, Choice::Aim, Choice::Charge],
        [Choice::Charge, Choice::Guard, Choice::Charge],
        [Choice::Aim, Choice::Aim, Choice::Aim],
    ] {
        let mut game = Chivalry::default();
        game.start().unwrap();
        for (i, choice) in choices.into_iter().enumerate() {
            game.choose(i + 1, choice).unwrap();
        }
        let pixels = render(&game);
        let result = game.tournament.result();
        println!(
            "RESULT STAGING {result}: actual score={} delta_from_running={}",
            game.tournament.score(),
            delta(&pixels, &running)
        );
        assert!(delta(&pixels, &running) > 144 * 104 / 10);
        // Only the real winner has a raised arm above the rider body. A draw
        // raises both. The same standing-ready rider geometry is the baseline.
        for (left, expected) in [(true, result != "LOSS"), (false, result != "WIN")] {
            let mesh = mesh::scene(&game, visit);
            let count = |scene: &crate::stage::world_viz::world3d::mesh::Mesh| {
                scene
                    .tris
                    .iter()
                    .filter(|t| {
                        t.mat == crate::stage::world_viz::world3d::mesh::mat::ARMOR
                            && t.v.iter().any(|p| p.z > 3.9)
                            && t.v.iter().all(|p| if left { p.x < 0.0 } else { p.x > 0.0 })
                    })
                    .count()
            };
            assert_eq!(
                count(&mesh) > count(&running_scene),
                expected,
                "{result}: raised winner must follow actual result"
            );
        }
        if let Some(dir) = std::env::var_os("ANGEL_CHIVALRY_REVIEW") {
            pixels
                .save(
                    std::path::PathBuf::from(dir)
                        .join(format!("normal-result-{}.png", result.to_lowercase())),
                )
                .unwrap();
        }
        assert_eq!(pixels, render(&game), "no outcome/idle animation");
        outcomes.push((result.to_string(), pixels));
    }
    assert_eq!(
        outcomes.iter().map(|(r, _)| r.as_str()).collect::<Vec<_>>(),
        ["WIN", "DRAW", "LOSS"]
    );
    assert_ne!(outcomes[0].1, outcomes[1].1);
    assert_ne!(outcomes[1].1, outcomes[2].1);
}
